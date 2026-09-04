//! WP-U1.9 PCE force-hook option gates (`Set`/`Get` `InjCurrent`/`ITerminal`/
//! `YPrim`/`StateVar`/`IterNumber`/`CtrlIterNumber`/`IntegrationFlag`).
//!
//! Ported from dss_capi **0.15.0b4** (`ExecOptions.pas` @ `e936d210`, the exact
//! capi015 oracle revision; the vendored working tree is checked out at a later
//! `master` where these options were removed, so the spec is read via
//! `git show 0.15.0b4:`). The pinned numbers below are the capi015 probe
//! (`scratchpad/probe_deck.py`): forcing a load's injection currents perturbs
//! the solve, the forced `InjCurrent` is frozen across the re-solve, and the
//! reported `ITerminal` stays at its pre-force value.

use crate::exec::*;
use num_complex::Complex64;

fn bus_vmag(dss: &Dss, bus: &str) -> f64 {
    let ckt = dss.circuit().unwrap();
    let b = ckt
        .buses
        .iter()
        .find(|b| b.name.eq_ignore_ascii_case(bus))
        .expect("bus exists");
    ckt.solution.node_v[b.ref_no[0]].norm()
}

/// The capi015 probe deck: a 3-phase source → line → constant-PQ load.
fn force_deck() -> Dss {
    let mut dss = Dss::new();
    dss.command("new circuit.fhk basekv=12.47 phases=3 bus1=sourcebus");
    dss.command(
        "new line.l1 phases=3 bus1=sourcebus bus2=b2 r1=0.3 x1=0.6 r0=0.5 x0=1.0 \
         c1=0 c0=0 length=1",
    );
    dss.command("new load.ld bus1=b2 phases=3 kv=12.47 kw=500 kvar=150 model=1");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcv");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

/// Parse a `get InjCurrent`/`ITerminal` result string into `(re, im)` pairs.
fn parse_complex_result(s: &str) -> Vec<(f64, f64)> {
    let inner = s.trim().trim_start_matches('[').trim_end_matches(']');
    inner
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| {
            // "re+imi" / "re-imi" / "re" (real only) / "imi" (pure imaginary).
            if let Some(ipos) = t.find(['i', 'j']) {
                let body = &t[..ipos];
                // Split on the sign that separates re from im (not a leading sign).
                let sep = body
                    .char_indices()
                    .skip(1)
                    .find(|&(_, c)| c == '+' || c == '-')
                    .map(|(k, _)| k);
                match sep {
                    Some(k) => (
                        body[..k].parse().unwrap_or(0.0),
                        body[k..].parse().unwrap_or(0.0),
                    ),
                    None => (0.0, body.parse().unwrap_or(0.0)),
                }
            } else {
                (t.parse().unwrap_or(0.0), 0.0)
            }
        })
        .collect()
}

/// Forcing a load's `InjCurrent` moves the solve to the capi015 operating point,
/// the forced values are frozen across the re-solve, and `ITerminal` reports its
/// stale (pre-force) value — all pinned to the capi015 0.15.0b4 probe.
#[test]
fn force_inj_current_matches_capi015() {
    let mut dss = force_deck();

    // Baseline (capi015: b2 Vmag = 7187.452797).
    let base_vmag = bus_vmag(&dss, "b2");
    assert!(
        (base_vmag - 7187.452797).abs() / 7187.452797 < 1e-6,
        "baseline b2 Vmag {base_vmag} != capi015 7187.452797"
    );
    dss.command("select load.ld");
    dss.command("get ITerminal");
    let base_iterm = dss.result().to_string();
    assert!(
        base_iterm.contains("23.17"),
        "baseline load.ld ITerminal: {base_iterm}"
    );

    // Force the injection currents and re-solve.
    dss.command("set InjCurrent=[80 0 80 0 80 0]");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    // The forced injection moved the operating point (capi015: 7224.143523).
    let forced_vmag = bus_vmag(&dss, "b2");
    assert!(
        (forced_vmag - 7224.143523).abs() / 7224.143523 < 1e-6,
        "forced b2 Vmag {forced_vmag} != capi015 7224.143523 (injection loop not honoring force?)"
    );

    // `get InjCurrent` returns the frozen forced value (NConds entries).
    dss.command("get InjCurrent");
    let inj = parse_complex_result(dss.result());
    assert_eq!(inj.len(), 4, "4 conductors, got {inj:?}");
    assert_eq!(inj[0], (80.0, 0.0));
    assert_eq!(inj[1], (0.0, 0.0));
    assert_eq!(inj[2], (80.0, 0.0));

    // `get ITerminal` is frozen at the pre-force value (Pascal `ForceInjCurrents`
    // skips `CalcLoadModelContribution`, `ITerminalUpdated` stays true).
    dss.command("get ITerminal");
    assert_eq!(
        dss.result(),
        base_iterm,
        "ITerminal must stay frozen at the pre-force value"
    );
}

/// `Clear` (a fresh circuit) drops the force flags: after re-creating the same
/// deck without re-forcing, the load solves at its model operating point, not
/// the forced one.
#[test]
fn clear_resets_force_flags() {
    let mut dss = force_deck();
    dss.command("select load.ld");
    dss.command("set InjCurrent=[80 0 80 0 80 0]");
    dss.command("solve");
    assert!(
        (bus_vmag(&dss, "b2") - 7224.143523).abs() < 1.0,
        "forcing took effect"
    );

    // `Clear` destroys the circuit (and every element's force flag with it).
    // Rebuilding the identical deck WITHOUT re-forcing must solve at the model
    // operating point — no forced injection leaks across the Clear.
    dss.command("clear");
    dss.command("new circuit.fhk basekv=12.47 phases=3 bus1=sourcebus");
    dss.command(
        "new line.l1 phases=3 bus1=sourcebus bus2=b2 r1=0.3 x1=0.6 r0=0.5 x0=1.0 \
         c1=0 c0=0 length=1",
    );
    dss.command("new load.ld bus1=b2 phases=3 kv=12.47 kw=500 kvar=150 model=1");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcv");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let vmag = bus_vmag(&dss, "b2");
    assert!(
        (vmag - 7187.452797).abs() / 7187.452797 < 1e-6,
        "after Clear + rebuild, no forcing should survive (Vmag {vmag})"
    );
    dss.command("select load.ld");
    dss.command("get InjCurrent");
    let inj = parse_complex_result(dss.result());
    // Un-forced: the model injection is tiny (< 1 A), never the forced 80 A.
    assert!(
        inj[0].0.abs() < 1.0,
        "fresh load must not carry a forced 80 A injection: {inj:?}"
    );
}

/// `Get IterNumber`/`CtrlIterNumber`/`IntegrationFlag` read the solution
/// counters; `Set` on them errors "read-only".
#[test]
fn iter_counters_readback_and_readonly() {
    let mut dss = force_deck();
    dss.command("get IterNumber");
    let iter: i32 = dss.result().parse().expect("integer");
    assert!((1..=3).contains(&iter), "IterNumber {iter}");

    dss.command("get CtrlIterNumber");
    assert!(dss.result().parse::<i32>().is_ok());

    dss.command("get IntegrationFlag");
    assert_eq!(
        dss.result(),
        "0",
        "IntegrationFlag = NewTimeStep (0) outside dynamics"
    );

    dss.command("set IterNumber=5");
    assert!(
        dss.errors().iter().any(|e| e.contains("read-only")),
        "Set IterNumber must be read-only: {:?}",
        dss.errors()
    );
}

/// `Set PyPath=` is a loud NOT_PORTED (pyControl co-simulation, §0).
#[test]
fn pypath_is_not_ported() {
    let mut dss = force_deck();
    dss.command("set PyPath=C:\\server.py");
    assert!(
        dss.errors().iter().any(|e| e.contains("not supported")),
        "Set PyPath must be a loud NOT_PORTED: {:?}",
        dss.errors()
    );
}

/// `Set/Get YPrim` forces the primitive Y and it survives a Y rebuild
/// (`ReCalcAllYPrims` skips `CalcYPrim` for a `ForceYPrim` element).
#[test]
fn force_yprim_survives_rebuild() {
    let mut dss = force_deck();
    dss.command("select load.ld");
    // A 4x4 diagonal YPrim (NConds=4): forced diagonal 5+0i.
    let row = |i: usize| {
        (0..4)
            .map(|j| if i == j { "5" } else { "0" })
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mat = (0..4).map(row).collect::<Vec<_>>().join(" | ");
    dss.command(&format!("set YPrim=[{mat}]"));
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    // Force a Y rebuild by re-solving.
    dss.command("solve");
    dss.command("get YPrim");
    let yp = parse_complex_result(&dss.result().replace('|', ","));
    // The forced diagonal (5) must have survived the rebuild's ReCalcAllYPrims.
    assert_eq!(
        yp[0],
        (5.0, 0.0),
        "forced YPrim[0,0] overwritten by CalcYPrim"
    );
    assert_eq!(
        yp[5],
        (5.0, 0.0),
        "forced YPrim[1,1] overwritten by CalcYPrim"
    );
}

/// `Set YPrim=` with a single row longer than `NConds²` (17 > 4² = 16) is the
/// only size-mismatch capi015's `ParseAsComplexMatrix` actually rejects (a
/// too-*few*-row matrix like `[5 0 | 0 5]` is silently zero-padded and accepted
/// by both engines — `Result := ExpectedOrder`). Pinned to the capi015 probe:
/// `#3004 The size of the matrix provided does not match with the number of
/// conductors of the active PCE.`
#[test]
fn force_yprim_oversize_row_errors_3004() {
    let mut dss = force_deck();
    dss.command("select load.ld");
    let big = std::iter::repeat_n("1", 17).collect::<Vec<_>>().join(" ");
    dss.command(&format!("set YPrim=[{big}]"));
    assert!(
        dss.errors().iter().any(|e| e.text()
            == "The size of the matrix provided does not match with the number of conductors \
                of the active PCE."),
        "an oversize YPrim row must be the 3004 size-mismatch error: {:?}",
        dss.errors()
    );
}

/// `Set ITerminal=` forces the active PCE's terminal currents and they freeze
/// across a re-solve (`Flg.ForceInjCurrents` makes `GetTerminalCurrents` skip
/// the model recompute). Pinned to the capi015 0.15.0b4 probe: baseline
/// `ITerminal[0]` ≈ 23.175527, forced `[10, 0, 10, <neutral>]` frozen through
/// `solve`.
#[test]
fn force_iterminal_freezes_like_capi015() {
    let mut dss = force_deck();
    dss.command("select load.ld");

    dss.command("get ITerminal");
    let base = parse_complex_result(dss.result());
    assert!(
        (base[0].0 - 23.175527).abs() < 1e-3,
        "baseline ITerminal[0] {base:?} != capi015 23.175527"
    );

    dss.command("set ITerminal=[10 0 10 0 10 0]");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.command("get ITerminal");
    let forced = dss.result().to_string();
    let f = parse_complex_result(&forced);
    assert_eq!((f[0], f[1], f[2]), ((10.0, 0.0), (0.0, 0.0), (10.0, 0.0)));

    // The forced terminal currents survive a Y rebuild / re-solve (frozen).
    dss.command("solve");
    dss.command("get ITerminal");
    assert_eq!(
        dss.result(),
        forced,
        "forced ITerminal must stay frozen across the re-solve"
    );
}

/// The forced-current model-recompute skip is not Load-specific: a Generator's
/// `GetTerminalCurrents` also honors `Flg.ForceInjCurrents`. Forcing its
/// `InjCurrent` and re-solving freezes the reported `ITerminal` at its
/// pre-force value (the skip returns the stored terminal currents instead of
/// re-running the generator model). Covers a second of the five PCE that share
/// the identical guard.
#[test]
fn generator_force_inj_freezes_iterminal() {
    let mut dss = Dss::new();
    dss.command("new circuit.g basekv=12.47 phases=3 bus1=sb");
    dss.command(
        "new line.l1 phases=3 bus1=sb bus2=b2 r1=0.3 x1=0.6 r0=0.5 x0=1.0 c1=0 c0=0 length=1",
    );
    dss.command("new generator.g1 bus1=b2 phases=3 kv=12.47 kw=200 model=1");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcv");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    dss.command("select generator.g1");
    dss.command("get ITerminal");
    let base = dss.result().to_string();

    dss.command("set InjCurrent=[50 0 50 0 50 0]");
    dss.command("solve");
    dss.command("get ITerminal");
    assert_eq!(
        dss.result(),
        base,
        "Generator GetTerminalCurrents must freeze at its pre-force value when forced"
    );
}

/// The sixth flag-checking class: a WindGen's `GetTerminalCurrents` also honors
/// `Flg.ForceInjCurrents` (r4133 WindGen.pas:2148 `and (not ForceInjCurr)`; the
/// 0.15.x-adoption sweep found the port's WindGen `get_currents` was the only one
/// of the six missing the guard, so it recomputed at the new operating point
/// where r4133 freezes). The observable is the reported *terminal currents*
/// (`ActiveCktElement.Currents` → `GetCurrents`, the port's `snapshot_elements`
/// path) — NOT the `Get ITerminal` executive readback, which serves the cached
/// buffer and is guard-blind. Once `InjCurrent` is forced, the reported currents
/// must stay frozen at the pre-force operating point (the model recompute at the
/// forced operating point is skipped).
///
/// Discriminating: a weak source + a large `[400,0,400]` A force pulls the
/// recompute far from the frozen value. Dropping the guard makes the port
/// recompute `[-110.744, -31.467, …]` (imag +18.88 → −31.47, a ≈50 A miss) and
/// energises the neutral conductor, so this assertion fails without the fix
/// (re-verified by toggling the guard, RP3.10).
///
/// **Both engines' numbers.** The deck's WindGen types `pf=0.95` and no
/// `QMode=`, so since R4133_PROPS_PLAN RP3.10 the frozen operating point itself
/// differs by design: this engine dispatches `kvarBase` in the constant-Q mode
/// (`elements/pc/windgen/nominal.rs`), r4133 dispatches nothing (`Else kvarCalc
/// := 0`, `WindGen.pas:1320-1321` — an upstream omission that CLAUDE.md's
/// 2026-08-02 policy forbids reproducing; cause `windgen-qmode0-no-arm`, pinned
/// by `elements::pc::windgen::tests::qmode0_dispatches_the_base_kvar`). The
/// **own r4133 probe** (epri-worker, this exact deck, `set InjCurrent=[400 0 400
/// 0 400 0]` + re-solve, `ActiveCktElement.Currents`) reported the frozen
/// `[-89.231224, -11.202775, 34.913725, 82.877894, 54.317500, -71.675120]`,
/// which the port matched to a faer-vs-KLU floor while it still reproduced the
/// missing arm; the port now freezes at the values below — the same real part on
/// conductor 1 to ≈0.1 A, with the reactive dispatch showing in the imaginary
/// parts. Both sets are asserted: the port must sit on its own frozen point
/// **and** stay off r4133's (`> 1e-6`), so the oracle leg survives as an
/// assertion instead of as prose, and a regression of the constant-Q arm —
/// which would put the port back on r4133's numbers — reds this test as well
/// as the four `windgen::tests` pins. What the test asserts about the guard
/// itself is unchanged: it freezes `GetCurrents` at the pre-force operating
/// point, whatever that point is.
#[test]
fn windgen_force_inj_freezes_iterminal() {
    let mut dss = Dss::new();
    dss.command("new circuit.wg basekv=12.47 phases=3 bus1=sb");
    dss.command("edit vsource.source r1=2 x1=8 r0=2 x0=8");
    dss.command(
        "new line.l1 phases=3 bus1=sb bus2=wbus r1=1.0 x1=2.0 r0=1.5 x0=3.0 c1=0 c0=0 length=1",
    );
    dss.command("new windgen.w1 bus1=wbus phases=3 kv=12.47 kw=2000 pf=0.95 conn=wye model=1");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcv");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    // Force the injection currents (weak source ⇒ the recompute is far from the
    // frozen value) and re-solve.
    dss.command("select windgen.w1");
    dss.command("set InjCurrent=[400 0 400 0 400 0]");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.command("solve");

    // Read the terminal currents the corpus gate compares (GetCurrents path).
    let snap = dss.snapshot_elements();
    let wg = snap
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case("WindGen.w1"))
        .expect("WindGen.w1 in snapshot");
    // The frozen pre-force currents of THIS engine (RP3.10 constant-Q
    // dispatch). WITHOUT the guard the port recomputes [-110.744, -31.467,
    // …] (fails here).
    let frozen = [
        Complex64::new(-89.136_414_184_261_7, 18.879_903_493_579_636),
        Complex64::new(60.918_683_146_319_42, 67.754_447_332_089_28),
        Complex64::new(28.217_731_026_031_643, -86.634_350_838_908_26),
    ];
    // r4133's own frozen set on this exact deck (epri-worker, EPRI r4133 DLL
    // 11.0.0.1, `set InjCurrent=[400 0 400 0 400 0]` + re-solve, re-probed at
    // the RP3.10 audit settlement): it freezes at its zero-var operating
    // point, which is a DIFFERENT point — that is the divergence this deck
    // carries, and asserting it keeps the oracle inside the assertion instead
    // of only in the prose. It is also a second way to catch a regression of
    // the constant-Q arm: revert `nominal.rs`'s mode-0 arm and the port lands
    // ON these numbers, which reds the second assertion below.
    let r4133 = [
        Complex64::new(-89.231_224_458_194_29, -11.202_774_501_306_344),
        Complex64::new(34.913_724_927_561_354, 82.877_894_438_227_46),
        Complex64::new(54.317_499_523_801_17, -71.675_119_953_123_74),
    ];
    for (k, (&want, &oracle)) in frozen.iter().zip(r4133.iter()).enumerate() {
        let got = wg.currents[k];
        assert!(
            (got.re - want.re).abs() < 1e-6 && (got.im - want.im).abs() < 1e-6,
            "WindGen forced current[{k}] {got} must stay frozen at the pre-force \
             value {want} (dropping the ForceInjCurr guard recomputes it ≈50 A \
             off; r4133 freezes at its own zero-var operating point, cause \
             `windgen-qmode0-no-arm`)"
        );
        assert!(
            (got - oracle).norm() > 1e-6,
            "WindGen forced current[{k}] {got} must NOT be r4133's frozen \
             {oracle}: the deck types no `QMode=`, so this engine dispatches \
             kvarBase where r4133 dispatches 0 (cause `windgen-qmode0-no-arm`) \
             — landing on the oracle's point means the constant-Q arm \
             regressed"
        );
    }
}

/// Sub-fix (c) regression pin (0.15.x-adoption sweep item 3): the forced-injection
/// honor is **inert** for the three PCE classes that recompute their injection
/// unconditionally in BOTH oracles — VCCS/UPFC/VSConverter (0 `ForceInjCurr` hits
/// in r4133 and 0.15.0b4). The fix moved the force check INSIDE each of the six
/// flag-checking classes' `inj_currents` and removed the central force-branch from
/// `power_flow.rs`, so forcing one of these three must NOT move the solve. A
/// regression that re-added a central force-branch (or wired a force check into
/// VSConverter/VCCS/UPFC) would freeze the injection and shift every node voltage —
/// this pin catches it. The ≈5 kA forced vector on the stiff `src` bus (|Z|≈0.051 Ω)
/// would move `src` by ≈255 V if honored; when inert the forced re-solve only drifts
/// by the fixpoint-convergence floor (≈2e-5 V here), so the 1e-2 V gate is
/// discriminating (4 orders below the honored signal, 2 orders above the floor).
#[test]
fn force_inj_current_is_inert_for_vsconverter() {
    let node_v = |dss: &Dss| dss.circuit().unwrap().solution.node_v.clone();
    let mut dss = Dss::new();
    dss.command("Set DefaultBaseFrequency=60");
    dss.command(
        "New Circuit.t basekv=0.48 phases=3 bus1=src pu=1.0 r1=0.01 x1=0.05 r0=0.01 x0=0.05",
    );
    dss.command("New Vsource.dc bus1=src.4 basekv=1.0 pu=1.0 phases=1 r1=0.001 x1=0.0");
    dss.command(
        "New VSConverter.v1 phases=4 Ndc=1 bus1=src.1.2.3.4 kVac=0.48 kVdc=1.0 kW=50 \
         Rac=0.05 Xac=0.2 m0=0.5 d0=0",
    );
    dss.command("Set voltagebases=[0.48, 1.0]");
    dss.command("calcv");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "vsc solve: {:?}", dss.errors());

    // Converged reference operating point.
    let v0 = node_v(&dss);

    // Force the VSConverter's injection and re-solve from the converged state.
    dss.command("select vsconverter.v1");
    dss.command("set InjCurrent=[5000 0 5000 0 5000 0 5000 0]");
    assert!(
        dss.errors().is_empty(),
        "set InjCurrent on a VSConverter: {:?}",
        dss.errors()
    );
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let v1 = node_v(&dss);

    let drift = v0
        .iter()
        .zip(&v1)
        .map(|(a, b)| (a - b).norm())
        .fold(0.0_f64, f64::max);
    assert!(
        drift < 1e-2,
        "forcing a VSConverter's InjCurrent must be inert (0 ForceInjCurr in r4133 / \
         0.15.0b4); node V moved by {drift:.3e} V — a re-added central force-branch would \
         honor the ≈5 kA force and shift src by ≈255 V"
    );
}

/// `Set/Get AllowForms`/`AllowProgressBar` are accepted no-ops that round-trip
/// (capi015 silently accepts them; a headless engine has no console forms, so
/// the default is `No`). Erroring — the pre-fix behavior — diverged from
/// capi015, which stores `NoFormsAllowed := not InterpretYesNo(Param)`.
#[test]
fn allow_forms_round_trip_like_capi015() {
    let mut dss = force_deck();
    dss.command("get AllowForms");
    assert_eq!(dss.result(), "No", "headless default: forms disallowed");

    dss.command("set AllowForms=Yes");
    assert!(
        dss.errors().is_empty(),
        "set AllowForms must not error: {:?}",
        dss.errors()
    );
    dss.command("get AllowForms");
    assert_eq!(dss.result(), "Yes");

    dss.command("set AllowForms=No");
    dss.command("get AllowForms");
    assert_eq!(dss.result(), "No");

    dss.command("get AllowProgressBar");
    assert_eq!(dss.result(), "No");
    dss.command("set AllowProgressBar=Yes");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.command("get AllowProgressBar");
    assert_eq!(dss.result(), "Yes");
}

/// A snapshot-solved generator deck for the `StateVar` tests (model-3 exposes
/// `Frequency`, `Theta (Deg)`, ...). A `line.ln` gives a non-PCE for the 7103
/// guard.
fn statevar_deck() -> Dss {
    let mut dss = Dss::new();
    dss.command("new circuit.sv basekv=12.47 phases=3 bus1=sb");
    dss.command("new generator.g1 bus1=sb phases=3 kv=12.47 kw=100 model=3");
    dss.command("new line.ln bus1=sb bus2=sb2 phases=3");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcv");
    dss.command("solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

/// `Get StateVar <pce> <name>` reads the element's dynamic state variable
/// (capi015: `Frequency` = 60). This is the functional half of the option —
/// see `set_state_var_text_syntax_is_upstream_broken` for why the *write* half
/// is not reachable through the natural text syntax.
#[test]
fn get_state_var_reads_solved_variable() {
    let mut dss = statevar_deck();
    dss.command("get StateVar generator.g1 Frequency");
    let f: f64 = dss.result().parse().expect("frequency value");
    assert!((f - 60.0).abs() < 1e-6, "Frequency = {f}, expected 60");

    // Unknown variable errors (Pascal 7102).
    dss.command("get StateVar generator.g1 NotAVar");
    assert!(
        dss.errors().iter().any(|e| e.contains("not found")),
        "unknown state var must error: {:?}",
        dss.errors()
    );
}

/// `Get/Set StateVar` on a **non-PCE** errors with the Pascal 7103 message
/// ("is not a valid PC element"), reproducing capi015 — the `is TPCElement`
/// guard runs *before* the NumVariables (7101) check. Pinned against the
/// capi015 probe (`get StateVar line.ln Frequency` → `#7103 Object "Line.ln"
/// is not a valid PC element.`).
#[test]
fn state_var_non_pce_errors_7103() {
    let mut dss = statevar_deck();
    dss.command("get StateVar line.ln Frequency");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.text() == "Object \"Line.ln\" is not a valid PC element."),
        "get StateVar on a line must be the 7103 message: {:?}",
        dss.errors()
    );
    // Set side takes the same guard (reached via the capi015 `=<dummy>` form so
    // the arm's element token is `line.ln`, not the skipped-forward token).
    let mut dss = statevar_deck();
    dss.command("set StateVar=x line.ln Frequency 55");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.text() == "Object \"Line.ln\" is not a valid PC element."),
        "set StateVar on a line must be the 7103 message: {:?}",
        dss.errors()
    );
}

/// `Set StateVar` through the natural text syntax is an upstream quirk in
/// capi015: `DoSetCmd` matches bare tokens **positionally** (only `name=value`
/// pairs are looked up by name), so `set StateVar generator.g1 Frequency 55`
/// never routes to the StateVar arm — the tokens land on options 1/3/4 and the
/// integer option (`hour`) rejects `Frequency` (capi015 `#303`). The port
/// reproduces this: the command errors and the state variable is left
/// unchanged. (Empirically confirmed on the capi015 0.15.0b4 oracle — no text
/// syntax writes a state variable that survives a readback; the write is
/// exercised via the classic/Alt API upstream, not this option.)
#[test]
fn set_state_var_text_syntax_is_upstream_broken() {
    let mut dss = statevar_deck();
    dss.command("get StateVar generator.g1 Frequency");
    let before: f64 = dss.result().parse().unwrap();

    dss.command("set StateVar generator.g1 Frequency 55");
    assert!(
        !dss.errors().is_empty(),
        "natural `set StateVar` syntax must error (positional misroute), like capi015 #303"
    );

    dss.command("get StateVar generator.g1 Frequency");
    let after: f64 = dss.result().parse().unwrap();
    assert!(
        (after - before).abs() < 1e-9,
        "the misrouted `set StateVar` must not change the variable: {before} -> {after}"
    );
}

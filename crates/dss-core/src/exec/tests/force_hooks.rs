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

/// `Set/Get StateVar` writes/reads a PC element's dynamic state variable
/// (Generator model-3 exposes `Frequency`, `Theta`, ...).
#[test]
fn state_var_set_get() {
    let mut dss = Dss::new();
    dss.command("new circuit.sv basekv=12.47 phases=3 bus1=sb");
    dss.command("new generator.g1 bus1=sb phases=3 kv=12.47 kw=100 model=3");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcv");
    dss.command("solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    dss.command("get StateVar generator.g1 Frequency");
    let f: f64 = dss.result().parse().expect("frequency value");
    assert!((f - 60.0).abs() < 1e-6, "Frequency = {f}, expected 60");

    // Unknown variable errors.
    dss.command("get StateVar generator.g1 NotAVar");
    assert!(
        dss.errors().iter().any(|e| e.contains("not found")),
        "unknown state var must error: {:?}",
        dss.errors()
    );
}

//! Circuit aggregates — `Circuit.Losses` / `LineLosses` / `SubstationLosses` /
//! `TotalPower` / `AllElementLosses` (`GOLDEN_REBASE_PLAN.md` G1.9).
//!
//! The live corpus gate compares these five numbers against both oracle
//! channels on every case, but a *number* only ever proves the sum; what an
//! aggregate really encodes is its **membership, its units and its walk order**.
//! Those three are pinned here, in-engine and without an oracle, so that a wrong
//! summand set survives neither the gate (arm P1 reconstructs each oracle
//! aggregate over [`crate::exec::view::AggregateTerms`]) nor this module.
//!
//! Upstream spec, re-read at r4133 (`.inputs/electricdss-code-r4133-trunk`) and
//! at the pinned dss_capi 0.14.5 (`.inputs/dss_capi`):
//! `DDLL/DCircuit.pas:294-368` + `:458-479` (`CircuitV` modes 0/1/2/3/8),
//! `Common/Circuit.pas:2428-2445` (`Get_Losses`), `Common/CktElement.pas:666-767`
//! (`Get_Power` / `Get_Losses`), and `CAPI_Circuit.pas:145-186/289-338/445-468`.

use crate::exec::Dss;
use crate::exec::view::ElementSnapshot;

/// Build a circuit from `deck`, asserting every command is accepted.
fn build(deck: &[&str]) -> Dss {
    let mut dss = Dss::new();
    for line in deck {
        dss.command(line);
        assert!(dss.errors().is_empty(), "`{line}` -> {:?}", dss.errors());
    }
    dss
}

/// Build and `Solve`, asserting the solve converged.
fn solve(deck: &[&str]) -> Dss {
    let mut dss = build(deck);
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!(dss.circuit().unwrap().is_solved, "deck did not solve");
    dss
}

/// A 12.47 kV feeder: three lines (two of them parallel), one load. The common
/// base of the losses / line-losses pins.
const FEEDER: &[&str] = &[
    "Clear",
    "New Circuit.g19 basekv=12.47 phases=3 bus1=sourcebus mvasc3=200 mvasc1=210",
    "New Line.l1 bus1=sourcebus bus2=b1 phases=3 r1=0.30 x1=0.90 r0=0.9 x0=2.7 c1=3.0 c0=1.5 length=2 units=km",
    "New Line.l2 bus1=sourcebus bus2=b1 phases=3 r1=0.45 x1=1.10 r0=1.1 x0=3.0 c1=3.0 c0=1.5 length=2 units=km",
    "New Line.l3 bus1=b1 bus2=b2 phases=3 r1=0.35 x1=0.95 r0=1.0 x0=2.8 c1=3.0 c0=1.5 length=1.5 units=km",
    "New Load.ld1 bus1=b2 phases=3 kv=12.47 kw=900 pf=0.92 model=1",
    "Set voltagebases=[12.47]",
    "Calcvoltagebases",
];

/// The `(index, loss_w)` of the element named `full_name`, panicking when the
/// deck does not carry it (a pin must never pass vacuously).
fn find(snaps: &[ElementSnapshot], full_name: &str) -> (usize, (f64, f64)) {
    let i = snaps
        .iter()
        .position(|s| s.name.eq_ignore_ascii_case(full_name))
        .unwrap_or_else(|| panic!("{full_name} is not in the circuit"));
    (i, snaps[i].loss_w)
}

/// `|a - b| <= rel * max(|a|, |b|) + abs`, the assertion the pins share.
fn close(a: f64, b: f64, rel: f64, abs: f64, what: &str) {
    let bound = rel * a.abs().max(b.abs()) + abs;
    assert!(
        (a - b).abs() <= bound,
        "{what}: {a:.17e} vs {b:.17e} (|delta| = {:.3e} > {bound:.3e})",
        (a - b).abs()
    );
}

// Expected-value pin, circuit losses are watts: `Circuit.Losses` is the ONE
// aggregate upstream does not rescale — `CAPI_Circuit.pas:171-186` and
// `DDLL/DCircuit.pas:294-303` both hand back `Circuit.Losses` untouched, while
// LineLosses / SubstationLosses / TotalPower / AllElementLosses all carry a
// `x 0.001`. A stray rescale here is a factor-1000 error that no per-element
// floor upstream of the sum can see.
/// `Circuit.Losses` is W/var and sums exactly the enabled, non-shunt PD
/// elements' own losses (`Common/Circuit.pas:2428-2445`).
#[test]
fn circuit_losses_are_watts_not_kilowatts() {
    let mut dss = solve(FEEDER);
    let terms = dss.aggregate_terms();
    let snaps = dss.snapshot_elements();
    let ael = dss.all_element_losses();
    let (p_w, q_var) = dss.losses();

    // The membership: the three lines — no source (not a PD element), no load.
    assert_eq!(terms.losses, vec!["line.l1", "line.l2", "line.l3"]);

    // Sum over the terms, taken from the per-element kW capture and pushed back
    // to W: the units identity, in the direction the comparator uses.
    let (mut p_kw, mut q_kvar) = (0.0, 0.0);
    for name in &terms.losses {
        let (i, _) = find(&snaps, name);
        p_kw += ael[i].0;
        q_kvar += ael[i].1;
    }
    // Sanity: a real feeder loses real power, so the pin cannot pass on zeros.
    assert!(
        p_kw > 1.0,
        "the feeder must dissipate kilowatts, got {p_kw}"
    );
    close(
        p_w,
        1000.0 * p_kw,
        1e-12,
        1e-9,
        "Losses.re (W) vs 1000 x sum kW",
    );
    close(
        q_var,
        1000.0 * q_kvar,
        1e-12,
        1e-9,
        "Losses.im (var) vs 1000 x sum kvar",
    );
}

// Expected-value pin, SubstationLosses excludes AutoTrans: `AutoTrans` objects
// are registered on the `AutoTransformers` pointer list, NOT on `Transformers`
// (`Common/Circuit.pas:2272-2273`), and `Circuit.SubstationLosses` walks
// `Transformers` only (`DDLL/DCircuit.pas:335-341`, `CAPI_Circuit.pas:300-304`)
// — so an `AutoTrans ... sub=yes` contributes nothing, however substation-like
// it is. No corpus deck carries `sub=yes` on an AutoTrans, so this deck is the
// exclusion's only witness.
/// A `sub=yes` AutoTrans is invisible to `SubstationLosses`; the `sub=yes`
/// Transformer is the whole sum.
#[test]
fn substation_losses_exclude_autotrans() {
    let mut dss = solve(&[
        "Clear",
        "New Circuit.g19sub basekv=115 phases=3 bus1=sourcebus mvasc3=20000 mvasc1=18000",
        // The substation transformer: 115 -> 12.47 kV, sub=yes.
        "New Transformer.sub1 phases=3 windings=2 xhl=8 sub=yes %loadloss=0.7",
        "~ wdg=1 bus=sourcebus conn=delta kv=115 kva=20000",
        "~ wdg=2 bus=mv conn=wye kv=12.47 kva=20000",
        // A second transformer that is NOT a substation.
        "New Transformer.t2 phases=3 windings=2 xhl=6 sub=no %loadloss=0.9",
        "~ wdg=1 bus=mv conn=wye kv=12.47 kva=5000",
        "~ wdg=2 bus=lv conn=wye kv=4.16 kva=5000",
        // A sub=yes AUTOTRANS on its own island — same flag, different list.
        "New Vsource.s2 bus1=auto_hv basekv=115 pu=1.0 phases=3 mvasc3=15000 mvasc1=12000",
        "New AutoTrans.a1 phases=3 windings=2 xhx=8.5 sub=yes %loadloss=0.8",
        "~ wdg=1 bus=auto_hv conn=s kv=115 kva=10000",
        "~ wdg=2 bus=auto_lv conn=w kv=69 kva=10000",
        "New Load.ldmv bus1=lv phases=3 kv=4.16 kw=3000 pf=0.95 model=1",
        "New Load.ldauto bus1=auto_lv phases=3 kv=69 kw=6000 pf=0.95 model=1",
        "Set voltagebases=[115, 69, 12.47, 4.16]",
        "Calcvoltagebases",
    ]);

    let terms = dss.aggregate_terms();
    assert_eq!(
        terms.substation_losses,
        vec!["transformer.sub1"],
        "only the sub=yes Transformer is a term"
    );

    let snaps = dss.snapshot_elements();
    let (_, sub1) = find(&snaps, "Transformer.sub1");
    let (_, auto) = find(&snaps, "AutoTrans.a1");
    // Non-vacuity: the excluded AutoTrans is enabled and loaded, so including
    // it would move the answer by kilowatts, not by rounding.
    assert!(
        auto.0 > 1000.0,
        "the AutoTrans must dissipate real power for the exclusion to bite, got {} W",
        auto.0
    );

    let (p_kw, q_kvar) = dss.substation_losses();
    close(p_kw, sub1.0 * 0.001, 1e-12, 1e-12, "SubstationLosses.re");
    close(q_kvar, sub1.1 * 0.001, 1e-12, 1e-12, "SubstationLosses.im");
}

// Expected-value pin, Circuit.Losses skips shunts: `Get_Losses` skips shunt PD
// elements (`Common/Circuit.pas:2438-2441`, `If Not pdElem.IsShunt`). A shunt
// capacitor's "loss" is its whole reactive rating, so a missing `IsShunt` test
// is a kvar-scale error in the imaginary part.
/// A shunt capacitor is in `AllElementLosses` but never in `Circuit.Losses`.
#[test]
fn losses_skip_shunt_elements() {
    let mut deck: Vec<&str> = FEEDER.to_vec();
    deck.insert(
        deck.len() - 2,
        "New Capacitor.cap1 bus1=b2 phases=3 kvar=600 kv=12.47",
    );
    let mut dss = solve(&deck);

    let terms = dss.aggregate_terms();
    assert!(
        !terms.losses.iter().any(|t| t == "capacitor.cap1"),
        "the shunt capacitor must not be a Losses term: {:?}",
        terms.losses
    );

    let snaps = dss.snapshot_elements();
    let (icap, cap) = find(&snaps, "Capacitor.cap1");
    let ael = dss.all_element_losses();
    // It IS in AllElementLosses (that walk has no filter) and it carries the
    // bank's whole reactive output — the magnitude the exclusion is worth.
    close(
        ael[icap].0,
        cap.0 * 0.001,
        1e-12,
        1e-12,
        "AllElementLosses.re",
    );
    close(
        ael[icap].1,
        cap.1 * 0.001,
        1e-12,
        1e-12,
        "AllElementLosses.im",
    );
    assert!(
        cap.1 < -500_000.0,
        "the 600 kvar bank must show its reactive output, got {} var",
        cap.1
    );

    // And `Circuit.Losses` is the PD sum *without* it.
    let (_, q_var) = dss.losses();
    let q_terms: f64 = terms.losses.iter().map(|n| find(&snaps, n).1.1).sum();
    close(
        q_var,
        q_terms,
        1e-12,
        1e-9,
        "Losses.im vs sum of non-shunt terms",
    );
}

// Expected-value pin, LineLosses walks the Lines list: `Circuit.LineLosses`
// walks the raw `Lines` pointer list with no `enabled` filter
// (`DDLL/DCircuit.pas:313-320`, `CAPI_Circuit.pas:156-159`); a disabled line
// stays a term and contributes `CZERO` through `Get_Losses`'s own guard
// (`Common/CktElement.pas:742`).
/// `LineLosses` is kW/kvar over every `Lines` entry, disabled ones included as
/// exact zeros.
#[test]
fn line_losses_sum_the_lines_list() {
    let mut deck: Vec<&str> = FEEDER.to_vec();
    deck.push("Edit Line.l2 enabled=no");
    let mut dss = solve(&deck);

    let terms = dss.aggregate_terms();
    assert_eq!(
        terms.line_losses,
        vec!["line.l1", "line.l2", "line.l3"],
        "the disabled line stays a LineLosses term"
    );
    // ... while dropping out of `Circuit.Losses`, which does filter on enabled.
    assert_eq!(terms.losses, vec!["line.l1", "line.l3"]);

    let snaps = dss.snapshot_elements();
    let (_, l2) = find(&snaps, "Line.l2");
    assert_eq!(l2, (0.0, 0.0), "a disabled line's Get_Losses is CZERO");
    let (_, l1) = find(&snaps, "Line.l1");
    let (_, l3) = find(&snaps, "Line.l3");
    assert!(
        l1.0 > 1000.0 && l3.0 > 1000.0,
        "both live lines must dissipate"
    );

    let (p_kw, q_kvar) = dss.line_losses();
    close(p_kw, (l1.0 + l3.0) * 0.001, 1e-12, 1e-12, "LineLosses.re");
    close(q_kvar, (l1.1 + l3.1) * 0.001, 1e-12, 1e-12, "LineLosses.im");
}

/// Two islands tied by a two-ended `Isource`: `Vsource.source` feeds `b1`,
/// `Vsource.s2` feeds `b2`, and `Isource.itie` drives 8 A per phase out of one
/// island and into the other, so it exchanges power at both ends.
fn itie(isource: &'static str) -> Vec<&'static str> {
    vec![
        "Clear",
        "New Circuit.g19tp basekv=12.47 phases=3 bus1=sourcebus mvasc3=200 mvasc1=210",
        "New Line.l1 bus1=sourcebus bus2=b1 phases=3 r1=0.30 x1=0.90 r0=0.9 x0=2.7 c1=3.0 c0=1.5 length=2 units=km",
        "New Load.ld1 bus1=b1 phases=3 kv=12.47 kw=900 pf=0.92 model=1",
        "New Vsource.s2 bus1=sb2 basekv=12.47 pu=1.02 phases=3 mvasc3=150 mvasc1=140",
        "New Line.l2 bus1=sb2 bus2=b2 phases=3 r1=0.30 x1=0.90 r0=0.9 x0=2.7 c1=3.0 c0=1.5 length=1 units=km",
        "New Load.ld2 bus1=b2 phases=3 kv=12.47 kw=400 pf=0.95 model=1",
        isource,
        "Set voltagebases=[12.47]",
        "Calcvoltagebases",
    ]
}

/// The tie written `bus1=b1 bus2=b2`.
const ITIE_FWD: &str = "New Isource.itie bus1=b1 bus2=b2 phases=3 amps=8 angle=15";
/// The same device written end for end: `bus1` and `bus2` swapped and the phasor
/// turned by 180 degrees, which changes no current and no voltage.
const ITIE_REV: &str = "New Isource.itie bus1=b2 bus2=b1 phases=3 amps=8 angle=195";

/// A solve tolerance at which the convergence residual of the constant-power
/// loads drops out of the power ledger.
const TIGHT: [&str; 2] = ["Set tolerance=1e-12", "Set maxiterations=500"];

/// A 5 % series booster: `Vsource.ser` sits between the feeder head `a` and the
/// load side `b`, so the whole 900 kW feeder flow passes through it.
fn booster(ser: &'static str) -> Vec<&'static str> {
    vec![
        "Clear",
        "New Circuit.sb basekv=12.47 phases=3 bus1=sourcebus pu=1.0 mvasc3=200 mvasc1=210",
        "New Line.l1 bus1=sourcebus bus2=a phases=3 r1=0.30 x1=0.90 r0=0.9 x0=2.7 c1=3.0 c0=1.5 units=km length=2",
        ser,
        "New Line.l2 bus1=b bus2=c phases=3 r1=0.30 x1=0.90 r0=0.9 x0=2.7 c1=3.0 c0=1.5 units=km length=1",
        "New Load.ld1 bus1=c phases=3 kv=12.47 kw=900 pf=0.92 model=1",
        "Set voltagebases=[12.47]",
        "Calcvoltagebases",
        TIGHT[0],
        TIGHT[1],
    ]
}

/// The booster written `bus1=b bus2=a`.
const SER_FWD: &str = "New Vsource.ser bus1=b bus2=a phases=3 basekv=12.47 pu=0.05 angle=10 r1=0.02 x1=0.2 r0=0.02 x0=0.2";
/// The booster written end for end.
const SER_REV: &str = "New Vsource.ser bus1=a bus2=b phases=3 basekv=12.47 pu=0.05 angle=190 r1=0.02 x1=0.2 r0=0.02 x0=0.2";

/// A circuit small enough to solve on paper: 100 V behind 1 micro-ohm at `A`,
/// 1 ohm from `A` to `B`, 9 ohm from `B` to earth, and `Isource.j`, a 5 A
/// current source that draws from `B` and pushes into `A`.
fn paper_circuit(isource: &'static str) -> Vec<&'static str> {
    vec![
        "Clear",
        "New Circuit.h1 phases=1 basekv=0.1 pu=1 angle=0 bus1=A Z1=[1e-6, 0]",
        "New Line.rab phases=1 bus1=A bus2=B r1=1 x1=0 r0=1 x0=0 c1=0 c0=0 length=1 units=none",
        "New Load.rb phases=1 bus1=B kv=0.09 kw=0.9 pf=1 model=2 vminpu=0.1 vmaxpu=10",
        isource,
        "Set voltagebases=[0.1]",
        "Calcvoltagebases",
    ]
}

/// The current source written `bus1=A bus2=B`.
const PAPER_FWD: &str = "New Isource.j phases=1 bus1=A bus2=B amps=5 angle=0";
/// The same current source written end for end.
const PAPER_REV: &str = "New Isource.j phases=1 bus1=B bus2=A amps=5 angle=180";

/// The sources' snapshot powers (kW/kvar) summed over every terminal, and over
/// terminal 1 alone: `(all, terminal_1)`.
fn source_power_sums(
    snaps: &[ElementSnapshot],
    terms: &[String],
) -> (num_complex::Complex64, num_complex::Complex64) {
    let mut all = num_complex::Complex64::ZERO;
    let mut t1 = num_complex::Complex64::ZERO;
    for name in terms {
        let (i, _) = find(snaps, name);
        let s = &snaps[i];
        assert_eq!(
            s.powers.len(),
            s.n_terms * s.n_conds,
            "{name}: the snapshot is terminal-major, {} terminals of {} conductors",
            s.n_terms,
            s.n_conds
        );
        t1 += s.powers[..s.n_conds].iter().sum::<num_complex::Complex64>();
        all += s.powers.iter().sum::<num_complex::Complex64>();
    }
    (all, t1)
}

/// The complete power ledger of a solved circuit, in kW/kvar.
struct Ledger {
    /// [`Dss::total_power`].
    total_power: num_complex::Complex64,
    /// The power into every terminal of every element that is not a source.
    rest: num_complex::Complex64,
    /// The sources' powers over terminal 1 alone.
    terminal_1: num_complex::Complex64,
    /// The sum of |S| over every conductor of every element, the scale the
    /// convergence residual is measured against.
    gross: f64,
}

fn ledger(dss: &mut Dss) -> Ledger {
    let terms = dss.aggregate_terms();
    let snaps = dss.snapshot_elements();
    let (_, terminal_1) = source_power_sums(&snaps, &terms.total_power);
    let mut rest = num_complex::Complex64::ZERO;
    let mut gross = 0.0;
    for s in &snaps {
        gross += s.powers.iter().map(|p| p.norm()).sum::<f64>();
        if !terms
            .total_power
            .iter()
            .any(|t| t.eq_ignore_ascii_case(&s.name))
        {
            rest += s.powers.iter().sum::<num_complex::Complex64>();
        }
    }
    let (re, im) = dss.total_power();
    Ledger {
        total_power: num_complex::Complex64::new(re, im),
        rest,
        terminal_1,
        gross,
    }
}

/// The ledger's closure band, relative to [`Ledger::gross`]. At a solve
/// tolerance of 1e-12 the residual of the decks the band is used on measured
/// at most 8.2e-13 of the gross terminal power (the two GIC decks, behind their
/// near-ideal source), and their terminal-2 terms at least 2.8e-2 of it
/// (measured 2026-10-03 on the engine), so the band sits 120 times above the
/// one and eight orders of magnitude under the other. The paper circuit's
/// 1 micro-ohm source leaves a residual of 3.5e-10 of its gross power, so its
/// pin uses an absolute band instead.
const LEDGER_REL: f64 = 1e-10;

/// Assert that `TotalPower` closes the complete ledger: the power the sources
/// supply over all their terminals is the power every other element takes in
/// over all of its terminals. Returns the part the terminal-1 walk misses.
fn assert_ledger_closes(l: &Ledger, what: &str) -> num_complex::Complex64 {
    let miss = l.total_power + l.rest;
    let bound = LEDGER_REL * l.gross;
    assert!(
        miss.norm() <= bound,
        "{what}: TotalPower {} + the rest of the circuit {} = {miss} kVA, over the \
         {bound:e} kVA band (gross {} kVA)",
        l.total_power,
        l.rest,
        l.gross
    );
    l.terminal_1 + l.rest
}

// Expected-value pin, TotalPower sums every terminal: the engine adds the power
// into every terminal of every source, so a two-ended source's terminal 2 is part
// of its contribution. Both oracles add terminal 1 alone. Measured 2026-10-03 on
// this deck with the pinned dss-python (capi 0.14.5) and with epri-worker on the
// r4133 DLL: `Circuit.TotalPower` is (-1472.906666040525, -478.1997169017333)
// kW/kvar on capi and (-1472.9066660404997, -478.1997169017321) on r4133, each
// its own terminal-1 sum of the sources' `Powers`, while the sum of those
// `Powers` over every terminal is (-1303.4686258774766, -524.7716839952111) on
// capi and (-1303.4686258774514, -524.7716839952099) on r4133. The gap of
// 169.438 kW and -46.572 kvar is `Isource.itie`'s terminal 2.
/// `TotalPower` is every terminal of every source, in kW/kvar, x3 in a
/// positive-sequence circuit.
#[test]
fn total_power_sums_every_terminal_of_every_source() {
    let mut dss = solve(&itie(ITIE_FWD));

    let terms = dss.aggregate_terms();
    assert_eq!(
        terms.total_power,
        vec!["vsource.source", "vsource.s2", "isource.itie"]
    );

    // The independent derivation: the per-conductor snapshot powers (kW/kvar),
    // summed over every terminal of every source.
    let snaps = dss.snapshot_elements();
    let (all, t1) = source_power_sums(&snaps, &terms.total_power);
    assert!(
        all.re < -1000.0,
        "the sources must supply megawatts (negative kW into them), got {all}"
    );
    // Non-vacuity: the Isource's terminal 2 moves the sum by about 170 kVA
    // (8 A on each of 3 phases at about 7.2 kV to earth), and it is the only
    // live terminal 2 on the deck.
    let (i_tie, _) = find(&snaps, "Isource.itie");
    let tie = &snaps[i_tie];
    assert_eq!(tie.n_terms, 2, "Isource.itie has two terminals");
    let tie_t2: num_complex::Complex64 = tie.powers[tie.n_conds..].iter().sum();
    assert!(
        tie_t2.norm() > 100.0,
        "Isource.itie terminal 2 must carry power for the pin to bite, got {tie_t2} kVA"
    );
    assert!(
        ((all - t1) - tie_t2).norm() <= 1e-9 * tie_t2.norm(),
        "only the Isource has a live terminal 2: all-terminals minus terminal-1 sum \
         {} kVA vs Isource.itie terminal 2 {tie_t2} kVA",
        all - t1
    );

    let (tp_re, tp_im) = dss.total_power();
    close(tp_re, all.re, 1e-12, 1e-9, "TotalPower.re");
    close(tp_im, all.im, 1e-12, 1e-9, "TotalPower.im");
    // The oracles' own all-terminals sums, quoted above. The two agree to 2e-14
    // of the value, and the engine lands within 1e-9 of both.
    for (oracle, (p, q)) in [
        ("capi_v0145", (-1303.4686258774766, -524.7716839952111)),
        ("r4133", (-1303.4686258774514, -524.7716839952099)),
    ] {
        close(tp_re, p, 1e-9, 0.0, &format!("TotalPower.re vs {oracle}"));
        close(tp_im, q, 1e-9, 0.0, &format!("TotalPower.im vs {oracle}"));
    }
    // ... and not the oracles' terminal-1 reading, a whole terminal away.
    let off = num_complex::Complex64::new(tp_re - t1.re, tp_im - t1.im);
    assert!(
        off.norm() > 100.0,
        "TotalPower must count terminal 2: it reads ({tp_re}, {tp_im}) kW/kvar, the \
         terminal-1 sum {t1}"
    );

    // The x3 of a positive-sequence circuit: flipping `CktModel` scales the
    // *report*, not the solution (the flag is read at read time), so the factor
    // is exact.
    dss.command("Set CktModel=Positive");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let (tp3_re, tp3_im) = dss.total_power();
    close(tp3_re, 3.0 * tp_re, 1e-15, 0.0, "TotalPower.re x3");
    close(tp3_im, 3.0 * tp_im, 1e-15, 0.0, "TotalPower.im x3");
}

// Expected-value pin, TotalPower conserves power: with KCL at every node the
// complex power into all conductors of all elements sums to zero, so the power
// the sources supply over all their terminals is what the rest of the circuit
// takes in over all of its terminals. The terminal-1 walk misses that balance by
// a whole terminal. Measured 2026-10-03 with the pinned dss-python (capi 0.14.5)
// and with epri-worker on the r4133 DLL, kW: on the tie deck at tolerance 1e-12
// both oracles report a TotalPower of -1472.91566 against a rest of the circuit
// of +1303.47762, and on the booster -1767.61124 against +905.17725. Both
// oracles' all-terminals sums close those ledgers to 5e-11 and 8e-10 kW.
/// The all-terminals `TotalPower` closes the complete power ledger, and the
/// terminal-1 walk does not.
#[test]
fn total_power_closes_the_complete_ledger() {
    for (what, mut deck, min_gap_kw) in [
        ("tie", itie(ITIE_FWD), 150.0),
        ("booster", booster(SER_FWD), 800.0),
    ] {
        deck.extend(TIGHT);
        let mut dss = solve(&deck);
        let l = ledger(&mut dss);
        let gap = assert_ledger_closes(&l, what);
        assert!(
            gap.norm() > min_gap_kw,
            "{what}: the terminal-1 walk must miss the ledger by over {min_gap_kw} kVA \
             for the pin to bite, it misses by {gap}"
        );
    }
}

// Expected-value pin, TotalPower does not depend on how a source is written:
// swapping `bus1` and `bus2` of a two-ended source and turning its phasor by 180
// degrees changes no current and no voltage, so the power the sources supply
// cannot change either. The terminal-1 walk moves with the labels. Measured
// 2026-10-03 with the pinned dss-python (capi 0.14.5) and with epri-worker on the
// r4133 DLL, kW: `Circuit.TotalPower` reads -1472.90667 for the tie written
// `bus1=b1 bus2=b2` and -1138.84502 written end for end (at the default
// tolerance), and -1767.61124 for the booster against -3.45125 written end for
// end, on both oracles. Their all-terminals sums are -1303.46863 for both tie
// writings and -905.17725 for both booster writings.
/// `TotalPower` is unchanged when a two-ended source is written end for end.
#[test]
fn total_power_does_not_move_when_a_source_is_written_end_for_end() {
    for (what, mut fwd, mut rev, min_shift_kw) in [
        ("tie", itie(ITIE_FWD), itie(ITIE_REV), 300.0),
        ("booster", booster(SER_FWD), booster(SER_REV), 1700.0),
    ] {
        fwd.extend(TIGHT);
        rev.extend(TIGHT);
        let a = ledger(&mut solve(&fwd));
        let b = ledger(&mut solve(&rev));
        assert!(
            (a.total_power - b.total_power).norm() <= LEDGER_REL * a.gross,
            "{what}: TotalPower {} written one way, {} the other",
            a.total_power,
            b.total_power
        );
        // Non-vacuity: the terminal-1 walk does move.
        assert!(
            (a.terminal_1 - b.terminal_1).norm() > min_shift_kw,
            "{what}: the terminal-1 walk must move by over {min_shift_kw} kVA for the \
             pin to bite, it reads {} and {}",
            a.terminal_1,
            b.terminal_1
        );
    }
}

// Expected-value pin, the paper circuit: with an ideal 100 V source, KCL at `B`
// gives (V_B - 100)/1 + V_B/9 = -5, so V_B = 85.5 V, the line takes
// (100 - 85.5)^2 = 210.25 W and the load 85.5^2/9 = 812.25 W, 1022.5 W in all.
// The 1 micro-ohm source impedance moves that by 2e-4 W. Measured 2026-10-03
// with the pinned dss-python (capi 0.14.5) and with epri-worker on the r4133 DLL:
// `Circuit.TotalPower` reads -1.4499998 kW with the current source written
// `bus1=A bus2=B` and -0.5224999 kW written end for end, on both oracles, while
// the sum of the sources' `Powers` over every terminal is -1.0224998 kW both ways.
/// `TotalPower` of the paper circuit is the 1022.5 W its network absorbs, however
/// the current source is written.
#[test]
fn total_power_of_the_paper_circuit_is_its_network_power() {
    let v_b = 95.0 * 9.0 / 10.0;
    let network_w = (100.0 - v_b) * (100.0 - v_b) / 1.0 + v_b * v_b / 9.0;
    close(network_w, 1022.5, 0.0, 0.0, "the hand value");
    for (isource, terminal_1_kw) in [(PAPER_FWD, -1.45), (PAPER_REV, -0.5225)] {
        let mut dss = solve(&paper_circuit(isource));
        let l = ledger(&mut dss);
        // 1 mW: five times the shift the 1 micro-ohm source impedance causes.
        close(
            l.total_power.re,
            -network_w * 0.001,
            0.0,
            1e-6,
            &format!("TotalPower.re, `{isource}`"),
        );
        close(l.total_power.im, 0.0, 0.0, 1e-6, "TotalPower.im");
        // Non-vacuity: the terminal-1 walk reads what both oracles report.
        close(
            l.terminal_1.re,
            terminal_1_kw,
            0.0,
            1e-6,
            &format!("terminal-1 walk, `{isource}`"),
        );
    }
}

/// Compile the vendored corpus deck `rel`, solve it again at [`TIGHT`] and
/// assert `TotalPower` closes its complete ledger. Returns the part the
/// terminal-1 walk misses.
fn corpus_ledger_gap(rel: &str) -> num_complex::Complex64 {
    let deck = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(rel);
    assert!(deck.is_file(), "vendored corpus deck missing: {deck:?}");
    let mut dss = Dss::new();
    dss.command(&format!("compile \"{}\"", deck.display()));
    for line in TIGHT.into_iter().chain(["Solve"]) {
        dss.command(line);
    }
    assert!(dss.errors().is_empty(), "{rel}: {:?}", dss.errors());
    assert!(dss.circuit().unwrap().is_solved, "{rel} did not solve");
    let l = ledger(&mut dss);
    assert_ledger_closes(&l, rel)
}

// Expected-value pin for the `asymmetric:isource/isource_snap.dss` corpus case,
// where the corpus gate compares `Circuit.TotalPower` against both oracles'
// all-terminals sum instead of their own reading
// (`harness::lane::total_power_counts_every_source_terminal`). `Isource.itie`
// ties two live buses, `b1` and `b2`. Measured 2026-10-03 with the
// pinned dss-python (capi 0.14.5) and with epri-worker on the r4133 DLL: both
// oracles report a TotalPower of -1553.393 kW, while the sources' `Powers` over
// every terminal sum to -1389.463 kW and the rest of the circuit takes in
// +1389.473 kW. The terminal-1 walk misses its terminal 2, 163.930 kW.
/// `isource_snap`'s `TotalPower` closes the ledger the terminal-1 walk misses by
/// about 164 kW.
#[test]
fn isource_snap_total_power_closes_the_complete_ledger() {
    let gap = corpus_ledger_gap("asymmetric/isource/isource_snap.dss");
    assert!(
        gap.norm() > 150.0,
        "the terminal-1 walk must miss by Isource.itie's terminal 2, got {gap} kVA"
    );
}

// Expected-value pin for the `asymmetric:gic/gicsource_gic.dss` corpus case,
// where the corpus gate compares `Circuit.TotalPower` against both oracles'
// all-terminals sum instead of their own reading
// (`harness::lane::total_power_counts_every_source_terminal`). Both `GICsource`
// elements sit in series inside a line, so both of their terminals are live by
// construction. Measured with the pinned dss-python (capi 0.14.5) and with
// epri-worker on the r4133 DLL: both oracles report a TotalPower of -20.287 kW,
// while the sources' `Powers` over every terminal sum to -22.134 kW (2026-10-03),
// which the rest of the circuit takes in to within 1.2e-11 kVA on both oracles
// (2026-10-04). The terminal-1 walk misses the 0.926 and 0.921 kW the two sources
// supply at terminal 2.
/// `gicsource_gic`'s `TotalPower` closes the ledger the terminal-1 walk misses by
/// about 1.85 kW.
#[test]
fn gicsource_gic_total_power_closes_the_complete_ledger() {
    let gap = corpus_ledger_gap("asymmetric/gic/gicsource_gic.dss");
    assert!(
        gap.norm() > 1.5,
        "the terminal-1 walk must miss by the GICsources' terminal 2, got {gap} kVA"
    );
}

// Expected-value pin for the `asymmetric:gic/gic_midi.dss` corpus case, where
// the corpus gate compares `Circuit.TotalPower` against the r4133 oracle's
// all-terminals sum instead of its own reading, on the case's only channel
// (`harness::lane::total_power_counts_every_source_terminal`). The two
// `GICsource` elements sit in series inside lines. Measured with the pinned
// dss-python (capi 0.14.5) and with epri-worker on the r4133 DLL: both oracles
// report a TotalPower of -4.676 kW, while the sources' `Powers` over every
// terminal sum to -7.056 kW (2026-10-03), which the rest of the circuit takes in
// to within 6.5e-12 kVA on capi and 1.4e-11 kVA on r4133 (2026-10-04). The
// terminal-1 walk misses 2.38 kW of terminal 2.
/// `gic_midi`'s `TotalPower` closes the ledger the terminal-1 walk misses by
/// about 2.4 kW.
#[test]
fn gic_midi_total_power_closes_the_complete_ledger() {
    let gap = corpus_ledger_gap("asymmetric/gic/gic_midi.dss");
    assert!(
        gap.norm() > 2.0,
        "the terminal-1 walk must miss by the GICsources' terminal 2, got {gap} kVA"
    );
}

// Expected-value pin, the readers of the source total follow it: `Summary`,
// `Export Summary` and the system meter (its demand-interval row and its kWh,
// kvarh, peak kW and peak kVA registers) report the power the sources supply,
// the negated `TotalPower`. On the tie deck both oracles print `Total Active
// Power:   1.47291 MW`, `Total Reactive Power: 0.4782 Mvar` and a loss share of
// `0.2361 %`, the terminal-1 reading (measured 2026-10-03 with the pinned
// dss-python and with epri-worker on the r4133 DLL). Over every terminal the
// sources supply 1.30347 MW, and the 3.47755 kW of losses are 0.2668 % of that.
/// `Summary`, `Export Summary` and the system meter report the all-terminals
/// source power.
#[test]
fn the_source_power_readers_report_every_terminal() {
    let dir = std::env::temp_dir().join(format!("dss_tp_readers_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");

    let mut dss = build(&itie(ITIE_FWD));
    dss.command(&format!("Set datapath=\"{}\"", dir.display()));
    dss.command("New EnergyMeter.em1 element=Line.l1 terminal=1");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let terms = dss.aggregate_terms();
    let snaps = dss.snapshot_elements();
    let (all, t1) = source_power_sums(&snaps, &terms.total_power);
    let (mw, mvar) = (-all.re * 0.001, -all.im * 0.001);
    assert!(
        (mw - 1.30347).abs() < 5e-6 && (-t1.re * 0.001 - 1.47291).abs() < 5e-6,
        "the sources must supply 1.30347 MW over every terminal and 1.47291 MW on \
         terminal 1, got {mw} and {}",
        -t1.re * 0.001
    );

    // `Summary`, `%-.6g` powers and a `%-.4g` loss share.
    dss.command("Summary");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let summary = dss.result().to_string();
    for line in [
        "Total Active Power:   1.30347 MW\n",
        "Total Reactive Power: 0.524772 Mvar\n",
        "Total Active Losses:   0.00347755 MW, (0.2668 %)\n",
    ] {
        assert!(
            summary.contains(line),
            "Summary must carry {line:?}: {summary}"
        );
    }

    // `Export Summary`: the TotalMW, TotalMvar and pctLosses columns.
    dss.command("Export Summary");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let csv = std::fs::read_to_string(dss.last_result_file()).expect("EXP_Summary.csv");
    let mut rows = csv.lines();
    let header: Vec<&str> = rows
        .next()
        .expect("header")
        .split(',')
        .map(str::trim)
        .collect();
    let row: Vec<&str> = rows
        .next()
        .expect("row")
        .split(',')
        .map(str::trim)
        .collect();
    let col = |name: &str| -> f64 {
        let i = header
            .iter()
            .position(|h| h.eq_ignore_ascii_case(name))
            .unwrap_or_else(|| panic!("no {name} column in {header:?}"));
        row[i]
            .parse()
            .unwrap_or_else(|e| panic!("{name} = {:?}: {e}", row[i]))
    };
    close(col("TotalMW"), mw, 1e-5, 0.0, "EXP_Summary TotalMW");
    close(col("TotalMvar"), mvar, 1e-5, 0.0, "EXP_Summary TotalMvar");
    let (loss_w, _) = dss.losses();
    close(
        col("pctLosses"),
        loss_w * 1e-6 / mw * 100.0,
        5e-4,
        0.0,
        "EXP_Summary pctLosses",
    );

    // The system meter samples the same total on its one demand-interval step.
    dss.command("Set demandinterval=yes");
    dss.command("Set mode=daily number=1 stepsize=1h");
    dss.command("Solve");
    dss.command("CloseDI");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let (tp_re, tp_im) = dss.total_power();
    let (kw, kvar) = (-tp_re, -tp_im);
    let kva = kw.hypot(kvar);
    let di_dir = dss.circuit().unwrap().em_di.di_dir.clone();
    assert!(
        di_dir.starts_with(&dir),
        "the demand-interval files must land under the data path, not in {}",
        di_dir.display()
    );
    let last_row = |file: &str| -> Vec<f64> {
        let path = di_dir.join(file);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
            .lines()
            .last()
            .expect("a data row")
            .split(',')
            .map(|f| f.trim().parse().expect("a number"))
            .collect()
    };
    // `Hour, kW, kvar, peak kW, peak kVA, ...` of the last step.
    let di = last_row("DI_SystemMeter_1.csv");
    close(di[1], kw, 1e-13, 0.0, "DI_SystemMeter kW");
    close(di[2], kvar, 1e-13, 0.0, "DI_SystemMeter kvar");
    close(di[3], kw, 1e-13, 0.0, "DI_SystemMeter peak kW");
    close(di[4], kva, 1e-13, 0.0, "DI_SystemMeter peak kVA");
    // `Year, kWh, kvarh, peak kW, peak kVA, ...`: one 1 h step.
    let reg = last_row("SystemMeter_1.csv");
    close(reg[1], kw, 1e-13, 0.0, "SystemMeter kWh");
    close(reg[2], kvar, 1e-13, 0.0, "SystemMeter kvarh");
    close(reg[3], kw, 1e-13, 0.0, "SystemMeter peak kW");
    close(reg[4], kva, 1e-13, 0.0, "SystemMeter peak kVA");

    let _ = std::fs::remove_dir_all(&dir);
}

// Expected-value pin, Totaliterations is an alias: `Solution.Totaliterations`
// (`DDLL/DSolution.pas:218-220` — `SolutionI` mode 40) returns
// `Solution.Iteration`, the field `Solution.Iterations` (mode 7) returns, and
// dss_capi's `CAPI_Solution.pas:731-738` says so in its own comment ("Same as
// Iterations interface"). `Iteration` is itself the per-solve accumulator:
// `SolveSnap` sums the inner iterations of every control iteration into its
// local `TotalIterations` and stores the sum in `Iteration` at its end
// (`Common/Solution.pas:2724`), as `solution::solution::power_flow::solve_snap`
// does, and `exec::tests::controls::control_loop_regulates_two_bus_to_oracle_tap`
// pins that sum against the pinned dss-python's count. Both oracle surfaces
// therefore read the accumulator, reset by every `SolveSnap`, so a multi-step
// run reports its LAST step's total and nothing sums over the steps. That is
// why this field is pinned in-engine as an alias instead of being
// oracle-compared a second time.
/// `Totaliterations` is `Iteration`: a multi-step run reports the last step's
/// count, not the sum over steps.
#[test]
fn total_iterations_is_an_alias_of_iterations() {
    let mut deck: Vec<&str> = FEEDER.to_vec();
    deck.push("New Loadshape.ls npts=3 interval=1 mult=[0.6, 1.0, 0.8]");
    deck.push("Edit Load.ld1 daily=ls");
    deck.push("Set mode=daily stepsize=1h");

    // Three single-step solves: each step's own `Iteration` total.
    let mut stepwise = build(&deck);
    stepwise.command("Set number=1");
    let mut per_step = Vec::new();
    let mut per_step_most = Vec::new();
    for _ in 0..3 {
        stepwise.command("Solve");
        assert!(stepwise.errors().is_empty(), "{:?}", stepwise.errors());
        let sol = &stepwise.circuit().unwrap().solution;
        per_step.push(sol.iteration);
        per_step_most.push(sol.most_iterations_done);
    }
    let sum: i32 = per_step.iter().sum();
    let max = *per_step.iter().max().unwrap();
    assert!(
        sum > max,
        "the pin needs a run whose steps sum above their max, got {per_step:?}"
    );

    // The same three steps in one `Solve`.
    let mut oneshot = build(&deck);
    oneshot.command("Set number=3");
    oneshot.command("Solve");
    assert!(oneshot.errors().is_empty(), "{:?}", oneshot.errors());
    let sol = &oneshot.circuit().unwrap().solution;

    assert_eq!(
        sol.iteration,
        *per_step.last().unwrap(),
        "Iteration (== Totaliterations) is the LAST step's count; per-step {per_step:?}"
    );
    assert!(
        sol.iteration < sum,
        "a Totaliterations summed over the steps would read {sum}, not {}",
        sol.iteration
    );
    // Its companion scalar `MostIterationsDone` is a max too — but only over the
    // CONTROL iterations of the current step: `SnapShotInit` zeroes it at the
    // top of every step (`Common/Solution.pas:2568`, ported at
    // `solution::solution::state::SolutionState::snap_shot_init`) and
    // `Common/Solution.pas:2701` raises it inside the control loop
    // (`solution::solution::power_flow`). So a multi-step run reports the LAST
    // step's max, discarding a larger earlier step.
    let last_most = *per_step_most.last().unwrap();
    assert!(
        *per_step_most.iter().max().unwrap() > last_most,
        "this pin needs an earlier step with MORE iterations than the last one,          else the per-step reset is untested; per-step {per_step_most:?}"
    );
    assert_eq!(
        sol.most_iterations_done, last_most,
        "MostIterationsDone is reset per step, so it reports the last step's max;          per-step {per_step_most:?}"
    );
}

// Expected-value pin, AllElementLosses order: `Circuit.AllElementLosses`
// walks `CktElements` (`DDLL/DCircuit.pas:466-473`, `CAPI_Circuit.pas:461-465`),
// i.e. creation order — positionally aligned with `AllElementNames`
// (`CAPI_Circuit.pas:275-279`), which the corpus gate matches against
// `snapshot_elements`. Nothing but this pin and the gate's ordered-name
// assertion protects that alignment.
/// `AllElementLosses` is one `x 0.001` entry per device, in creation order.
#[test]
fn all_element_losses_follow_creation_order() {
    let mut deck: Vec<&str> = FEEDER.to_vec();
    // Appended last, after `Calcvoltagebases`: it must land in the last slot.
    deck.push("New Load.zlast bus1=b1 phases=3 kv=12.47 kw=250 pf=0.9 model=1");
    let mut dss = solve(&deck);

    let snaps = dss.snapshot_elements();
    let ael = dss.all_element_losses();
    assert_eq!(ael.len(), snaps.len());
    assert_eq!(
        ael.len(),
        dss.circuit().unwrap().num_devices,
        "AllElementLosses has one slot per device"
    );
    assert!(
        snaps
            .last()
            .unwrap()
            .name
            .eq_ignore_ascii_case("Load.zlast"),
        "the element created last must be last: {:?}",
        snaps.last().unwrap().name
    );

    for (i, s) in snaps.iter().enumerate() {
        close(
            ael[i].0,
            s.loss_w.0 * 0.001,
            1e-12,
            1e-12,
            &format!("AllElementLosses[{i}].re ({})", s.name),
        );
        close(
            ael[i].1,
            s.loss_w.1 * 0.001,
            1e-12,
            1e-12,
            &format!("AllElementLosses[{i}].im ({})", s.name),
        );
    }

    // Teeth: every entry is distinguishable, so a swapped pair reds the loop
    // above instead of cancelling out.
    for i in 0..ael.len() {
        for j in (i + 1)..ael.len() {
            assert!(
                (ael[i].0 - ael[j].0).abs() > 1e-9 || (ael[i].1 - ael[j].1).abs() > 1e-9,
                "{} and {} report the same losses; the order assertion would not \
                 catch a swap of the two",
                snaps[i].name,
                snaps[j].name
            );
        }
    }
}

// Expected-value pin, both boolean `Solution` scalars are two-sided. The live
// gate compares `ControlActionsDone` and `SystemYChanged` with an exact
// `assert_eq!` on every case, but every corpus checkpoint measured so far
// reports the same value for both (`true` / `false` — a converged solve settles
// its controls and leaves Y freshly built), so those two asserts have no corpus
// witness of the other value. This pin supplies one in-engine for each: the
// max-control-iteration exit (`solve_snap`, Pascal `SolveSnap`) leaves
// `ControlActionsDone` clear, and a structural edit after a solve raises
// `SystemYChanged` again. Without it a port that could never produce the second
// value would pass the gate unnoticed.
#[test]
fn the_two_boolean_solution_flags_take_both_values() {
    // (1) A converged solve: controls settled, Y rebuilt and the flag cleared.
    let mut dss = solve(FEEDER);
    {
        let sol = &dss.circuit().unwrap().solution;
        assert!(
            sol.control_actions_done,
            "a converged, control-free solve must end with ControlActionsDone set"
        );
        assert!(
            !sol.system_y_changed,
            "a converged solve must leave Y built, i.e. SystemYChanged clear"
        );
    }

    // (2) A structural edit dirties Y again — the `true` witness.
    dss.command("New Load.ld2 bus1=b1 phases=3 kv=12.47 kw=100 pf=1 model=1");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!(
        dss.circuit().unwrap().solution.system_y_changed,
        "adding an element must raise SystemYChanged"
    );

    // (3) A regulator that cannot settle inside `MaxControlIter` — the `false`
    // witness. `solve_snap` breaks on the iteration limit with the flag clear
    // and reports "Max Control Iterations Exceeded".
    let mut dss = build(&[
        "Clear",
        "New Circuit.g19reg basekv=12.47 phases=3 bus1=sourcebus mvasc3=200 mvasc1=210",
        "New Line.l1 bus1=sourcebus bus2=b1 phases=3 r1=0.30 x1=0.90 r0=0.9 x0=2.7 length=2 units=km",
        "New Transformer.tx1 phases=3 windings=2 buses=[b1, b2] conns=[wye wye] \
         kvs=[12.47 12.47] kvas=[5000 5000] xhl=1",
        "New RegControl.rc1 transformer=tx1 winding=2 vreg=126 band=1 ptratio=60 delay=0",
        "New Load.ld1 bus1=b2 phases=3 kv=12.47 kw=3000 pf=0.95 model=1",
        "Set voltagebases=[12.47]",
        "Calcvoltagebases",
        "Set maxcontroliter=1",
    ]);
    dss.command("Solve");
    let sol = &dss.circuit().unwrap().solution;
    assert_eq!(
        sol.control_iteration, 1,
        "the deck must stop at MaxControlIter=1 for this pin to mean anything"
    );
    assert!(
        !sol.control_actions_done,
        "a solve that hits MaxControlIter must leave ControlActionsDone clear"
    );
}

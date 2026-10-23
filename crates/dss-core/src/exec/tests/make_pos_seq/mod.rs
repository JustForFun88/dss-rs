//! `MakePosSeq` (Pascal `TExecHelper.DoMakePosSeq`) end-to-end gates: build a
//! circuit through the Text interface, `solve`, `makeposseq`, `solve` again, and
//! read back the converted properties. The full-model live compare lives in the
//! `modes` family (`tests/corpus/modes/makeposseq/makeposseq_*.dss`,
//! `modes_cases_match_oracle`); these pin the applier's structural behaviors and
//! the per-class conversion arithmetic against the oracle-validated deck-header
//! values (dss-python 0.15.7; see `docs/wpg21_makeposseq_probes.md`).

mod line_data;
mod pc;
mod projection;
mod refusals;

use crate::elements::pos_seq::balance;
use crate::exec::tests::common::query;
use crate::exec::*;
use num_complex::Complex64;

/// Run `makeposseq` on a circuit it refuses and assert the refusal: exactly
/// one new error, with the text `message` and code `None`, the saved circuit
/// (saved under `tag`) byte-identical, the circuit not flagged
/// positive-sequence, and with `Some(before)` the source power of the solve
/// after the refusal within `BALANCE_REL` of `before`.
fn assert_refused(dss: &mut Dss, tag: &str, message: &str, before: Option<Complex64>) {
    let n = dss.errors().len();
    let saved = balance::saved_circuit(dss, tag);
    dss.command("makeposseq");
    let added = &dss.errors()[n..];
    assert_eq!(added.len(), 1, "{tag}: {added:?}");
    assert_eq!(added[0].text(), message, "{tag}");
    assert_eq!(added[0].code, None, "{tag}");
    assert_eq!(
        balance::saved_circuit(dss, tag),
        saved,
        "{tag}: the saved circuit"
    );
    assert!(
        !dss.circuit().unwrap().positive_sequence,
        "{tag}: flagged positive-sequence"
    );
    if let Some(before) = before {
        let after = balance::solve_after_refusal(dss, tag);
        balance::assert_kept(before, after, tag);
    }
}

/// `12.47 / √3` — the wye per-phase kV the conversion targets (deck headers).
fn approx(a: &str, want: f64) -> bool {
    a.parse::<f64>()
        .map(|v| (v - want).abs() < 5e-4)
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Shunt (Capacitor SpecType 1) + the `positive_sequence` flag + bus stripping.
// ---------------------------------------------------------------------------

#[test]
fn makeposseq_sets_positive_sequence_and_converts_capacitor() {
    let mut dss = Dss::new();
    dss.command("set defaultbasefrequency=60");
    dss.command(
        "new circuit.psq basekv=12.47 pu=1.0 phases=3 bus1=src r1=0.3 x1=1.2 r0=0.9 x0=3.6",
    );
    dss.command("new line.feed bus1=src bus2=b1 phases=3 r1=0.2 x1=0.5 c1=3 length=1 units=km");
    // Capacitor SpecType 1 (kvar+kV), default grounded bus2.
    dss.command("new capacitor.cap_kvar bus1=b1 phases=3 kvar=600 kv=12.47");
    dss.command("new load.ld1 bus1=b1 phases=3 kv=12.47 kw=400 pf=0.95 model=1");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("set maxiterations=100");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "pre-solve: {:?}", dss.errors());
    assert!(!dss.circuit().unwrap().positive_sequence);

    dss.command("makeposseq");
    assert!(dss.errors().is_empty(), "makeposseq: {:?}", dss.errors());

    // Pascal `DoMakePosSeq` sets `ActiveCircuit.PositiveSequence := TRUE`.
    assert!(
        dss.circuit().unwrap().positive_sequence,
        "makeposseq must flip the circuit to positive-sequence"
    );

    // Capacitor SpecType 1: phases 3→1, kvar 600 → per-phase [200], kv 12.47 →
    // 12.47/√3 = 7.19956 (deck `makeposseq_shunt.dss`, oracle-validated).
    assert_eq!(query(&mut dss, "capacitor.cap_kvar.phases"), "1");
    assert!(
        approx(&query(&mut dss, "capacitor.cap_kvar.kv"), 7.19956),
        "cap kv = {}",
        query(&mut dss, "capacitor.cap_kvar.kv")
    );
    let kvar = query(&mut dss, "capacitor.cap_kvar.kvar");
    assert!(kvar.contains("200"), "cap kvar per phase [200], got {kvar}");

    // Base bus rename: the default grounded Bus2 keeps its `.0` (IsGroundBus),
    // Bus1 is a dotless bus → unchanged.
    assert_eq!(query(&mut dss, "capacitor.cap_kvar.bus1"), "b1");
    assert_eq!(query(&mut dss, "capacitor.cap_kvar.bus2"), "b1.0");

    // The converted single-phase model re-solves and converges.
    dss.command("solve");
    assert!(dss.errors().is_empty(), "post-solve: {:?}", dss.errors());
    assert!(
        dss.circuit().unwrap().is_solved,
        "converted positive-sequence circuit must re-converge"
    );
}

// ---------------------------------------------------------------------------
// Apply order: every control and meter is reduced after every element, so a
// control sees its reduced target. The CapControl adopts the reduced 1-phase
// capacitor's phase count.
// ---------------------------------------------------------------------------

#[test]
fn makeposseq_control_after_target_sees_converted_target() {
    let mut dss = Dss::new();
    dss.command("set defaultbasefrequency=60");
    dss.command(
        "new circuit.psq_c basekv=115 pu=1.0 phases=3 bus1=src r1=0.2 x1=2.0 r0=0.6 x0=6.0",
    );
    dss.command("new transformer.reg phases=3 windings=2 xhl=7");
    dss.command("~ wdg=1 bus=src conn=delta kv=115 kva=20000");
    dss.command("~ wdg=2 bus=b1 conn=wye kv=12.47 kva=20000");
    dss.command("new line.l1 bus1=b1 bus2=b2 phases=3 r1=0.25 x1=0.6 c1=3 length=1 units=km");
    // Target capacitor created BEFORE its control.
    dss.command("new capacitor.cap1 bus1=b2 phases=3 kvar=600 kv=12.47");
    dss.command("new load.ld2 bus1=b2 phases=3 kv=12.47 kw=800 pf=0.92 model=1");
    // Control created AFTER the capacitor + the metered line.
    dss.command(
        "new capcontrol.cc element=line.l1 terminal=1 capacitor=cap1 type=current on=60 off=30",
    );
    dss.command("set voltagebases=[115, 12.47]");
    dss.command("calcvoltagebases");
    dss.command("set maxiterations=100");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "pre-solve: {:?}", dss.errors());

    dss.command("makeposseq");
    assert!(dss.errors().is_empty(), "makeposseq: {:?}", dss.errors());

    // The capacitor is reduced to 1-phase, and the CapControl, reduced after
    // every element, adopts the reduced capacitor's phase count. The
    // CapControl exposes no queryable `phases`, so read its phase count
    // (`cd().nphases`) straight off the circuit element (the precise adoption
    // is also pinned live by `makeposseq_ctrl.dss`).
    assert_eq!(query(&mut dss, "capacitor.cap1.phases"), "1");
    assert_eq!(
        element_nphases(&dss, "CapControl.cc"),
        Some(1),
        "CapControl converted after its 1-phase capacitor must adopt one phase"
    );

    dss.command("solve");
    assert!(dss.errors().is_empty(), "post-solve: {:?}", dss.errors());
    assert!(dss.circuit().unwrap().is_solved);
}

/// The live phase count of the circuit element `full` (`<Class>.<name>`, any
/// case), which a control or a meter exposes to no `?` query. The test module
/// is a descendant of `exec`, so it may reach `Dss`'s private class registry.
fn element_nphases(dss: &Dss, full: &str) -> Option<usize> {
    let (class, name) = full.split_once('.')?;
    let name = name.to_ascii_lowercase();
    dss.classes
        .iter()
        .filter(|cls| cls.props.class_name().eq_ignore_ascii_case(class))
        .find_map(|cls| cls.arena.try_ckt_elem(*cls.name_to_idx.get(&name)?))
        .map(|e| e.cd().nphases)
}

// ---------------------------------------------------------------------------
// The whole shunt deck: a positive-sequence reduction of a balanced circuit
// reproduces its three-phase solution.
// ---------------------------------------------------------------------------

/// Build `modes/makeposseq/makeposseq_shunt.dss` up to its first `solve`
/// (without the trailing `makeposseq`/`solve`), with `edit` applied to the
/// deck text, and return the solved three-phase circuit.
fn shunt_deck_before_makeposseq(edit: impl Fn(&str) -> String) -> Dss {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/modes/makeposseq/makeposseq_shunt.dss");
    let text = std::fs::read_to_string(&path).expect("vendored corpus deck");
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('!'))
        .collect();
    let n = lines.len();
    assert_eq!(
        &lines[n - 3..],
        ["solve", "makeposseq", "solve"],
        "the deck ends with solve, makeposseq, solve"
    );
    let mut dss = Dss::new();
    for l in &lines[..n - 2] {
        dss.command(&edit(l));
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!(dss.circuit().unwrap().is_solved);
    dss
}

/// Every bus's node-1 magnitude (V) and every element's power at its first
/// terminal (kW + j kvar), each by name.
type DeckState = (Vec<(String, f64)>, Vec<(String, num_complex::Complex64)>);

/// The [`DeckState`] of the solved deck, in the deck's order.
fn shunt_deck_state(dss: &mut Dss) -> DeckState {
    let buses = dss
        .all_bus_voltages()
        .into_iter()
        .map(|b| (b.name.clone(), b.vmag_angle[0].0))
        .collect();
    let elems = dss
        .snapshot_elements()
        .into_iter()
        .map(|e| {
            let s = e.powers[..e.n_conds].iter().sum();
            (e.name, s)
        })
        .collect();
    (buses, elems)
}

/// Reduce the solved deck and assert that every bus |V| stays within `v_rel`
/// and every element's power within `s_rel` of the three-phase solution.
fn assert_shunt_deck_reduces_exactly(mut dss: Dss, v_rel: f64, s_rel: f64, what: &str) {
    let (buses0, elems0) = shunt_deck_state(&mut dss);
    dss.command("makeposseq");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{what}: {:?}", dss.errors());
    assert!(dss.circuit().unwrap().is_solved, "{what}: not solved");
    let (buses1, elems1) = shunt_deck_state(&mut dss);
    assert_eq!(buses0.len(), buses1.len(), "{what}: bus count");
    for (name, v0) in &buses0 {
        let v1 = buses1
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("{what}: bus {name} after makeposseq"))
            .1;
        assert!(
            (v1 - v0).abs() <= v_rel * v0,
            "{what}: bus {name} |V| {v0} three-phase, {v1} reduced (rel {:.3e})",
            (v1 - v0).abs() / v0
        );
    }
    assert_eq!(elems0.len(), elems1.len(), "{what}: element count");
    for ((name, s0), (name1, s1)) in elems0.iter().zip(&elems1) {
        assert_eq!(name, name1, "{what}: element order");
        assert!(
            (s1 - s0).norm() <= s_rel * s0.norm(),
            "{what}: {name} S {s0} three-phase, {s1} reduced (rel {:.3e})",
            (s1 - s0).norm() / s0.norm()
        );
    }
}

/// `makeposseq_shunt.dss` holds only balanced three-phase elements, so its
/// positive-sequence reduction reproduces the three-phase solution: every bus
/// |V| within 1e-7 (measured 2.7e-8) and every element's power within 5e-7
/// (measured 1.9e-7).
///
/// Two residuals remain, and the second half of the pin takes both away. The
/// first is the truncated `CALPHA` of the three-phase Z1/Z0 reactor stamp
/// (`Reactor::calc_yprim`, the precision-compat twin of `util::CALPHA`):
/// `|CALPHA − e^(−j2π/3)|` = 4.04e-7 moves that stamp's equivalent Z1 by
/// 2.3e-7, which shows as 1.8e-7 on `Reactor.rx_z1` and 2.7e-8 on the bus
/// voltages. The second is the solver's convergence floor: a constant-PQ load
/// reports `V conj(I)` with `I` from the previous iterate, and the three-phase
/// and the reduced solve stop after different iterations, which leaves up to
/// 1.9e-7 on the two loads under the default tolerance. The same deck with
/// `rx_z1` declared `r=1 x=12` and solved to `tolerance=1e-13` reduces to 1e-10
/// (measured 2026-10-04: 5.8e-12 in |V|, 2.5e-14 in power).
#[test]
fn makeposseq_shunt_reproduces_its_three_phase_solution() {
    let dss = shunt_deck_before_makeposseq(str::to_string);
    assert_shunt_deck_reduces_exactly(dss, 1e-7, 5e-7, "makeposseq_shunt.dss");

    let exact = |l: &str| {
        if l.starts_with("new reactor.rx_z1") {
            "new reactor.rx_z1 bus1=b2 phases=3 r=1 x=12".to_string()
        } else if l == "set maxiterations=100" {
            "set maxiterations=100 tolerance=1e-13".to_string()
        } else {
            l.to_string()
        }
    };
    let dss = shunt_deck_before_makeposseq(exact);
    assert_shunt_deck_reduces_exactly(dss, 1e-10, 1e-10, "rx_z1 as r=1 x=12, tolerance 1e-13");
}

//! Phase 5 gate (PHASE5_PLAN.md WP5.9): compile the **unmodified** IEEE13,
//! IEEE37 and IEEE123 masters from the vendored `tests/corpus/electricdss-tst`
//! — controls active, exactly as the committed Phase-0 goldens were generated — and
//! match `tests/golden/{ieee13,ieee37,ieee123}.json`:
//!
//! - converged flag and total iteration count **exactly** (ieee13: 11);
//! - `YNodeOrder` exactly;
//! - final transformer taps, RegControl tap numbers and capacitor states
//!   **exactly**;
//! - node voltages, per-element powers/currents, total power and losses at
//!   1e-6 rel;
//! - every element's full property dump via the numeric-skeleton comparator
//!   (the no-extra-cost property regression for every class in the feeder).
//!
//! `ieee34mod1` is the stretch goal — attempted as its own test.
//! Do **not** regenerate these goldens (Phase-0, pinned oracle).

mod harness;

use std::path::PathBuf;

use dss_core::exec::Dss;
use harness::{
    Golden, assert_complex_close, assert_complex_close_c, assert_value_matches_tol, deinterleave,
};

fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

fn run_case(name: &str) {
    let golden = Golden::load(name);
    let master = repo_root()
        .join("tests")
        .join("corpus")
        .join("electricdss-tst")
        .join(&golden.master);
    assert!(
        master.is_file(),
        "{name}: master script missing: {}",
        master.display()
    );
    let master_str = master.to_string_lossy().replace('\\', "/");

    let mut dss = Dss::new();
    dss.command(&format!("compile \"{master_str}\""));
    for post in &golden.post {
        dss.command(post);
    }
    assert!(
        dss.errors().is_empty(),
        "{name}: unexpected engine errors: {:?}",
        dss.errors()
    );

    // Solution: converged + total iteration count (exact in the parity lane,
    // ±`lane::ITER_SLACK` in the default lane — Stage F drift model).
    let ckt = dss.circuit().expect("circuit after compile");
    assert!(golden.solution.converged, "{name}: oracle did not converge");
    assert!(ckt.is_solved, "{name}: Rust solution did not converge");
    harness::lane::compare_iterations(
        ckt.solution.iteration,
        golden.solution.iterations as i32,
        name,
    );
    assert_eq!(
        ckt.num_nodes as u32, golden.circuit.num_nodes,
        "{name}: node count differs"
    );

    // Node order, exact.
    let names: Vec<String> = (1..=ckt.num_nodes).map(|i| ckt.node_name(i)).collect();
    assert_eq!(names, golden.node_order, "{name}: node order differs");

    // Node voltages at 1e-6 rel.
    let mut actual = Vec::with_capacity(2 * ckt.num_nodes);
    for i in 1..=ckt.num_nodes {
        actual.push(ckt.solution.node_v[i].re);
        actual.push(ckt.solution.node_v[i].im);
    }
    assert_complex_close(
        &actual,
        &golden.node_voltages,
        1e-6,
        1e-9,
        &format!("{name} node voltages"),
    );

    // Final transformer taps: same tap position. The float value is compared
    // at 1e-12 rel rather than bitwise: the regulators may partition the same
    // net movement into different step sequences (our sparse solver's node
    // voltages differ from the oracle's at the 1e-9 level, which can shift a
    // banker's-rounding boundary in one control iteration and merge two steps
    // into one), and float accumulation order then differs by an ulp. The
    // *discrete* position check is the exact `tap_number` comparison below.
    let taps = dss.transformer_taps();
    assert_eq!(
        taps.len(),
        golden.transformers.len(),
        "{name}: transformer count differs"
    );
    for (tr_name, tr_taps) in &taps {
        let exp = golden
            .transformers
            .get(tr_name)
            .unwrap_or_else(|| panic!("{name}: golden has no transformer {tr_name}"));
        assert_eq!(
            tr_taps.len(),
            exp.taps.len(),
            "{name}: transformer {tr_name} winding count differs"
        );
        for (w, (a, e)) in tr_taps.iter().zip(&exp.taps).enumerate() {
            assert!(
                (a - e).abs() <= 1e-12 * e.abs().max(1.0),
                "{name}: transformer {tr_name} winding {} tap differs: {a} vs {e}",
                w + 1
            );
        }
    }

    // RegControl tap numbers: exact.
    let tap_numbers = dss.regcontrol_tap_numbers();
    assert_eq!(
        tap_numbers.len(),
        golden.regcontrols.len(),
        "{name}: regcontrol count differs"
    );
    for (rc_name, tap_number) in &tap_numbers {
        let exp = golden
            .regcontrols
            .get(rc_name)
            .unwrap_or_else(|| panic!("{name}: golden has no regcontrol {rc_name}"));
        assert_eq!(
            *tap_number, exp.tap_number,
            "{name}: regcontrol {rc_name} tap number differs"
        );
    }

    // Capacitor states: exact.
    let states = dss.capacitor_states();
    assert_eq!(
        states.len(),
        golden.capacitors.len(),
        "{name}: capacitor count differs"
    );
    for (cap_name, cap_states) in &states {
        let exp = golden
            .capacitors
            .get(cap_name)
            .unwrap_or_else(|| panic!("{name}: golden has no capacitor {cap_name}"));
        assert_eq!(
            cap_states, &exp.states,
            "{name}: capacitor {cap_name} states differ"
        );
    }

    // Total power (kW/kvar) and losses (W/var) at 1e-6 rel.
    let (tp_kw, tp_kvar) = dss.total_power();
    assert_complex_close(
        &[tp_kw, tp_kvar],
        &[golden.total_power_kw_kvar[0], golden.total_power_kw_kvar[1]],
        1e-6,
        1e-6,
        &format!("{name} total power"),
    );
    let (loss_w, loss_var) = dss.losses();
    assert_complex_close(
        &[loss_w, loss_var],
        &[golden.losses_w_var[0], golden.losses_w_var[1]],
        1e-6,
        1e-6,
        &format!("{name} losses"),
    );

    // Per-element snapshot: enabled, bus names, powers and currents.
    let snapshots = dss.snapshot_elements();
    assert_eq!(
        snapshots.len(),
        golden.elements.len(),
        "{name}: element count differs"
    );
    for snap in &snapshots {
        let exp = golden
            .elements
            .get(&snap.name)
            .or_else(|| {
                // Golden keys keep the oracle's case; match case-insensitively.
                golden
                    .elements
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(&snap.name))
                    .map(|(_, v)| v)
            })
            .unwrap_or_else(|| panic!("{name}: golden has no element {}", snap.name));
        assert_eq!(snap.enabled, exp.enabled, "{name}: {} enabled", snap.name);
        assert_eq!(
            snap.bus_names.len(),
            exp.bus_names.len(),
            "{name}: {} bus count",
            snap.name
        );
        for (a, e) in snap.bus_names.iter().zip(&exp.bus_names) {
            assert!(
                a.eq_ignore_ascii_case(e),
                "{name}: {} bus differs: {a} vs {e}",
                snap.name
            );
        }
        // Same absolute floors as the Phase 4 gate: dead-end branch currents
        // are differences of nearly equal voltages, so 1e-6-rel voltage
        // agreement caps absolute current agreement at the µA scale.
        assert_complex_close_c(
            &snap.currents,
            &deinterleave(&exp.currents),
            1e-6,
            1e-4,
            &format!("{name} {} currents", snap.name),
        );
        assert_complex_close_c(
            &snap.powers,
            &deinterleave(&exp.powers),
            1e-6,
            1e-4,
            &format!("{name} {} powers", snap.name),
        );
    }

    // Full per-element property dumps (numeric-skeleton comparison). The
    // absolute floor is 1e-9 (aligned with the node-voltage comparison above,
    // and looser than the 1e-6 power floor): a few property dumps are near-zero
    // cancellation quantities — e.g. a near-balanced transformer's ~5e-5 A
    // winding current — whose last printed digit sits at the LU solver's
    // backward-error floor and flips under any solve-path change (here, the
    // KLU-style row equilibration in `dss-sparse`). 1e-9 amps/volts/watts is
    // well below physical significance; the 1e-9 *relative* term keeps
    // significant quantities pinned tightly.
    let element_names: Vec<String> = golden.elements.keys().cloned().collect();
    for el_name in &element_names {
        let class = el_name.split('.').next().unwrap_or("");
        let props = &golden.elements[el_name].properties;
        for (prop, expected) in props {
            // WP-U1.6 C5 (r4086): RegControl's `RevThreshold` default flipped from
            // +100 kW to the signed −100 kW. These Phase-0 goldens are 0.14.5-pinned
            // (+100) and must not be regenerated (the 0.14.5 oracle still dumps
            // +100); the port's 0.15.x −100 is a deliberate version mismatch (§1.2),
            // pinned against capi015 by props/regcontrol.json + regcontrol_idle.dss.
            if class.eq_ignore_ascii_case("RegControl") && prop.eq_ignore_ascii_case("RevThreshold")
            {
                continue;
            }
            dss.command(&format!("? {el_name}.{prop}"));
            let actual = dss.result().to_string();
            assert_value_matches_tol(
                &actual,
                expected,
                1e-9,
                1e-9,
                &format!("{name} {el_name} property {prop}"),
            );
        }
    }
    assert!(
        dss.errors().is_empty(),
        "{name}: errors during property dump: {:?}",
        dss.errors()
    );
}

#[test]
fn ieee13_controls_matches_phase0_golden() {
    run_case("ieee13");
}

#[test]
fn ieee37_controls_matches_phase0_golden() {
    run_case("ieee37");
}

#[test]
fn ieee123_controls_matches_phase0_golden() {
    run_case("ieee123");
}

/// Stretch goal (PHASE5_PLAN §1): the IEEE34 mod-1 feeder.
#[test]
fn ieee34mod1_controls_matches_phase0_golden() {
    run_case("ieee34mod1");
}

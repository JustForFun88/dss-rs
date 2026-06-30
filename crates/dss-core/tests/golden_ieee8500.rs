//! Phase 6 headline gate (PHASE6_PLAN.md WP6.9 / §1.1): compile the
//! **unmodified** IEEE 8500-Node master, attach `Energymeter.m1` on
//! `Line.ln5815900-1`, raise `Maxiterations=20` and `Solve` — exactly as
//! `Run_8500Node.dss` does (Show/Export/Plot/Interpolate dropped: file/UI
//! output, Phase 8). Then run a 24-step daily segment so the meter integrates
//! a full day over its zone. Against `tests/golden/ieee8500.json` (pinned
//! oracle) the Rust engine must match:
//!
//!   - converged flag and total iteration count **exactly** (67);
//!   - `YNodeOrder` exactly (8531 nodes);
//!   - node voltages at 1e-6 rel; total power + losses at 1e-6 rel;
//!   - the 12 regulated transformer taps (1e-12 rel), all 12 RegControl tap
//!     numbers and all 10 capacitor states **exactly** — the controls-at-scale
//!     regression (4 regulator banks + 10 CapControls);
//!   - the EnergyMeter `m1` registers at 1e-4 rel after the daily segment
//!     (integration results — looser tolerance per PORTING_PLAN §4), register
//!     names exact.
//!
//! Regenerate only manually: `python tools/golden/gen_ieee8500.py`.

mod harness;

use std::path::PathBuf;

use dss_core::exec::Dss;
use harness::{MonitorCap, assert_complex_close, compare_monitor, tol_for};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Golden {
    schema: u32,
    master: String,
    snap_commands: Vec<String>,
    daily_commands: Vec<String>,
    node_order: Vec<String>,
    snap: Snap,
    registers: Registers,
    /// Per-step feeder-head P/Q over the daily segment (a `mode=1 ppolar=no` monitor):
    /// pins the daily run at every hour, not only by the cumulative registers.
    #[serde(default)]
    monitors: Vec<MonitorCap>,
}

#[derive(Debug, Deserialize)]
struct Snap {
    iterations: i32,
    converged: bool,
    v_re: Vec<f64>,
    v_im: Vec<f64>,
    total_power: Vec<f64>,
    losses: Vec<f64>,
    transformers: std::collections::BTreeMap<String, Vec<f64>>,
    regcontrols: std::collections::BTreeMap<String, i32>,
    capacitors: std::collections::BTreeMap<String, Vec<i32>>,
}

#[derive(Debug, Deserialize)]
struct Registers {
    names: Vec<String>,
    values: Vec<f64>,
}

fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

fn load_golden() -> Golden {
    let path = repo_root()
        .join("tests")
        .join("golden")
        .join("ieee8500.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let g: Golden = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("cannot parse {}: {e}", path.display()));
    assert_eq!(g.schema, 1, "ieee8500 golden schema mismatch");
    g
}

#[test]
fn ieee8500_matches_oracle() {
    let golden = load_golden();
    let master = repo_root()
        .join("tests")
        .join("corpus")
        .join("electricdss-tst")
        .join(&golden.master);
    assert!(
        master.is_file(),
        "8500 master script missing: {}",
        master.display()
    );
    let master_str = master.to_string_lossy().replace('\\', "/");

    // --- snapshot solve ---------------------------------------------------
    let mut dss = Dss::new();
    dss.command(&format!("compile \"{master_str}\""));
    for cmd in &golden.snap_commands {
        dss.command(cmd);
    }
    assert!(
        dss.errors().is_empty(),
        "unexpected engine errors after snap: {:?}",
        dss.errors()
    );

    // Convergence + total iteration count, exact.
    let ckt = dss.circuit().expect("circuit after compile");
    assert!(golden.snap.converged, "oracle did not converge");
    assert!(ckt.is_solved, "Rust solution did not converge");
    assert_eq!(
        ckt.solution.iteration, golden.snap.iterations,
        "iteration count differs"
    );

    // Node order, exact.
    assert_eq!(ckt.num_nodes, golden.node_order.len(), "node count differs");
    let names: Vec<String> = (1..=ckt.num_nodes).map(|i| ckt.node_name(i)).collect();
    assert_eq!(names, golden.node_order, "node order differs");

    // Node voltages at 1e-6 rel (1e-9 abs floor).
    let mut actual = Vec::with_capacity(2 * ckt.num_nodes);
    for i in 1..=ckt.num_nodes {
        actual.push(ckt.solution.node_v[i].re);
        actual.push(ckt.solution.node_v[i].im);
    }
    let mut expected = Vec::with_capacity(actual.len());
    for (re, im) in golden.snap.v_re.iter().zip(&golden.snap.v_im) {
        expected.push(*re);
        expected.push(*im);
    }
    assert_complex_close(&actual, &expected, 1e-6, 1e-9, "8500 node voltages");

    // Total power (kW/kvar) and losses (W/var) at 1e-6 rel.
    let (tp_kw, tp_kvar) = dss.total_power();
    assert_complex_close(
        &[tp_kw, tp_kvar],
        &[golden.snap.total_power[0], golden.snap.total_power[1]],
        1e-6,
        1e-6,
        "8500 total power",
    );
    let (loss_w, loss_var) = dss.losses();
    assert_complex_close(
        &[loss_w, loss_var],
        &[golden.snap.losses[0], golden.snap.losses[1]],
        1e-6,
        1e-6,
        "8500 losses",
    );

    // Regulated transformer taps: same position (1e-12 rel; the discrete
    // check is the exact tap_number below). Only the moved regulators are in
    // the golden — the 1178 fixed load xfmrs stay at 1.0.
    let taps: std::collections::HashMap<String, Vec<f64>> =
        dss.transformer_taps().into_iter().collect();
    for (tr_name, exp_taps) in &golden.snap.transformers {
        let act = taps
            .get(tr_name)
            .unwrap_or_else(|| panic!("engine has no transformer {tr_name}"));
        assert_eq!(
            act.len(),
            exp_taps.len(),
            "transformer {tr_name} winding count differs"
        );
        for (w, (a, e)) in act.iter().zip(exp_taps).enumerate() {
            assert!(
                (a - e).abs() <= 1e-12 * e.abs().max(1.0),
                "transformer {tr_name} winding {} tap differs: {a} vs {e}",
                w + 1
            );
        }
    }

    // Every *other* transformer must stay at nominal. The golden lists exactly
    // the transformers the oracle found moved off 1.0 (|tap-1|>1e-9, see
    // gen_ieee8500.py:capture_controls), so the ~1178 fixed load xfmrs + the
    // substation must read 1.0 — this catches a spurious tap on an *uncontrolled*
    // transformer, which the moved-only golden would otherwise miss.
    for (tr_name, act) in &taps {
        if golden.snap.transformers.contains_key(tr_name) {
            continue;
        }
        for (w, a) in act.iter().enumerate() {
            assert!(
                (a - 1.0).abs() <= 1e-9,
                "uncontrolled transformer {tr_name} winding {} moved off nominal: {a}",
                w + 1
            );
        }
    }

    // RegControl tap numbers: exact.
    let tap_numbers: std::collections::HashMap<String, i32> =
        dss.regcontrol_tap_numbers().into_iter().collect();
    assert_eq!(
        tap_numbers.len(),
        golden.snap.regcontrols.len(),
        "regcontrol count differs"
    );
    for (rc_name, exp) in &golden.snap.regcontrols {
        let act = tap_numbers
            .get(rc_name)
            .unwrap_or_else(|| panic!("engine has no regcontrol {rc_name}"));
        assert_eq!(act, exp, "regcontrol {rc_name} tap number differs");
    }

    // Capacitor states: exact.
    let states: std::collections::HashMap<String, Vec<i32>> =
        dss.capacitor_states().into_iter().collect();
    assert_eq!(
        states.len(),
        golden.snap.capacitors.len(),
        "capacitor count differs"
    );
    for (cap_name, exp) in &golden.snap.capacitors {
        let act = states
            .get(cap_name)
            .unwrap_or_else(|| panic!("engine has no capacitor {cap_name}"));
        assert_eq!(act, exp, "capacitor {cap_name} states differ");
    }

    // --- daily segment: meter register integration ------------------------
    for cmd in &golden.daily_commands {
        dss.command(cmd);
    }
    assert!(
        dss.errors().is_empty(),
        "unexpected engine errors after daily: {:?}",
        dss.errors()
    );
    let regs = dss.meter_registers("m1").expect("EnergyMeter m1 registers");
    assert_eq!(
        regs.len(),
        golden.registers.names.len(),
        "register count differs"
    );
    for (i, (name, value)) in regs.iter().enumerate() {
        assert_eq!(
            name, &golden.registers.names[i],
            "register {i} name differs"
        );
        let exp = golden.registers.values[i];
        // 1e-4 rel (integration results, PORTING_PLAN §4); abs floor covers the
        // exactly-zero registers and the unsampled -1e50 drag hands.
        let tol = 1e-4 * exp.abs().max(1.0);
        assert!(
            (value - exp).abs() <= tol,
            "register {name} differs: {value} vs {exp}"
        );
    }

    // Per-step feeder-head P/Q over the daily segment: pins each hour against the
    // oracle (not only the cumulative registers above), via the shared comparator.
    let tol = tol_for("large");
    for m in &golden.monitors {
        compare_monitor(&dss, m, &tol, "ieee8500");
    }
}

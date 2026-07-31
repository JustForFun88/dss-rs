//! Phase 7 WP7.2 step-4 **protection** golden (PHASE7_PLAN §1 focused gate 3):
//! command-replay trip/reclose sequences that pin the Fault + Recloser / Relay /
//! Fuse / SwtControl control path against the pinned oracle
//! (`tools/golden/gen_protection.py`). Each scenario builds a small radial
//! feeder, drives the **ported** `mode=duty controlmode=time` control sweep (the
//! corpus relay demos use dynamics mode, unported until WP7.7), solves step by
//! step, and must match its file under `tests/golden/protection/` (one
//! `<scenario>.json` per scenario; the gate runs every file in the dir):
//!
//!   (The recloser_temp/recloser_perm scenarios were retired in WP-U2.2, and
//!   relay_current in WP-U2.3: the r4133 per-phase rewrites move their event-log
//!   behavior off the 0.14.5 oracle — new per-phase wording + an unconditional
//!   `Debug Sample` line — so they are no longer byte-golden-able against the
//!   pinned 0.14.5 oracle; their coverage is now the live r4133 family gates
//!   under `tests/corpus/controls/{recloser,relay}/`.)
//!   - fuse_blow:     per-phase fuse on tlink -> PHASE 3/2/1 BLOWN;
//!   - swt_manual:    manual switch opened by a mid-run `edit ... action=open`
//!     — pinned on **EPRI r4133** (`oracle: oddie:r4133`), where WP-U2.4 D6 makes
//!     the deprecated `Action` force the actual state immediately (no queue/delay,
//!     empty event log), unlike the r4088/0.14.5 queued form.
//!
//! Pins: per-step dblHour + iteration count + converged + node voltages (the
//! feeder voltage collapses on every step the line is held open, so the voltage
//! trajectory encodes the discrete state); the **event log line-for-line**
//! (normalized — the trip/reclose/blow/reset sequence); and every element's
//! terminal currents/powers at the **final** step (the controlled line carries
//! ~0 A when left open, full load current when reclosed — the final switch/
//! recloser/fuse state pinned exactly). Unlike the live corpus gate this golden is
//! committed, so it guards the protection control path offline.
//!
//! Regenerate only manually: `python tools/golden/gen_protection.py`.

mod harness;

use std::collections::BTreeMap;
use std::path::PathBuf;

use dss_core::exec::Dss;
use harness::{
    ElementCap, assert_complex_close, assert_value_matches_tol, compare_element, tol_for,
};
use serde::Deserialize;

/// One scenario file: `{schema, oracle, scenario}` (the `oracle` block is ignored
/// here).
#[derive(Debug, Deserialize)]
struct ScenarioFile {
    schema: u32,
    scenario: Scenario,
}

#[derive(Debug, Deserialize)]
struct Scenario {
    name: String,
    commands: Vec<String>,
    /// Step-index (as a string key) -> commands to issue *before* that step's
    /// `solve` (e.g. a mid-run `edit swtcontrol.x action=open`). Default empty.
    #[serde(default)]
    pre_solve: BTreeMap<String, Vec<String>>,
    n_steps: usize,
    node_order: Vec<String>,
    steps: Vec<Step>,
    event_log: Vec<String>,
    /// Every element's terminal currents/powers at the final committed step.
    final_elements: Vec<ElementCap>,
}

#[derive(Debug, Deserialize)]
struct Step {
    dbl_hour: f64,
    iterations: i32,
    converged: bool,
    v_re: Vec<f64>,
    v_im: Vec<f64>,
}

/// Load every `*.json` scenario file from `tests/golden/protection/`,
/// sorted by file name for deterministic run order.
fn load_scenarios() -> Vec<Scenario> {
    let dir: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "golden",
        "protection",
    ]
    .iter()
    .collect();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    files
        .iter()
        .map(|p| {
            let text = std::fs::read_to_string(p)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()));
            let f: ScenarioFile = serde_json::from_str(&text)
                .unwrap_or_else(|e| panic!("cannot parse {}: {e}", p.display()));
            assert_eq!(
                f.schema,
                1,
                "{}: protection golden schema mismatch",
                p.display()
            );
            f.scenario
        })
        .collect()
}

fn run_scenario(sc: &Scenario, tol: &harness::Tolerances) {
    let mut dss = Dss::new();
    dss.command("clear");
    for c in &sc.commands {
        dss.command(c);
    }
    assert!(
        dss.errors().is_empty(),
        "{}: unexpected engine errors after setup: {:?}",
        sc.name,
        dss.errors()
    );
    assert_eq!(sc.steps.len(), sc.n_steps, "{}: malformed golden", sc.name);

    for (i, step) in sc.steps.iter().enumerate() {
        for c in sc.pre_solve.get(&i.to_string()).into_iter().flatten() {
            dss.command(c);
        }
        dss.command("solve");
        assert!(
            dss.errors().is_empty(),
            "{} step {i}: engine errors: {:?}",
            sc.name,
            dss.errors()
        );
        let ckt = dss.circuit().expect("circuit exists");
        assert!(
            step.converged,
            "{} step {i}: oracle did not converge",
            sc.name
        );
        assert!(ckt.is_solved, "{} step {i}: Rust did not converge", sc.name);
        assert!(
            (ckt.solution.dbl_hour - step.dbl_hour).abs() < 1e-12,
            "{} step {i}: dblHour {} vs {}",
            sc.name,
            ckt.solution.dbl_hour,
            step.dbl_hour
        );
        harness::lane::compare_iterations(
            ckt.solution.iteration,
            step.iterations,
            &format!("{} step {i}", sc.name),
        );

        let names: Vec<String> = (1..=ckt.num_nodes).map(|j| ckt.node_name(j)).collect();
        assert_eq!(names, sc.node_order, "{} step {i}: node order", sc.name);

        let mut actual = Vec::with_capacity(2 * ckt.num_nodes);
        for n in 1..=ckt.num_nodes {
            actual.push(ckt.solution.node_v[n].re);
            actual.push(ckt.solution.node_v[n].im);
        }
        let mut expected = Vec::with_capacity(actual.len());
        for (re, im) in step.v_re.iter().zip(&step.v_im) {
            expected.push(*re);
            expected.push(*im);
        }
        assert_complex_close(
            &actual,
            &expected,
            tol.v_rel,
            tol.v_abs,
            &format!("{} step {i} node voltages", sc.name),
        );
    }

    // Final element currents/powers — pins the controlled line's open/closed state
    // (a held-open line carries ~0 A; a reclosed line carries full load current).
    let snaps = dss.snapshot_elements();
    // Assert the element name sets match exactly (no dropped *or* extra element),
    // like the live gate (corpus_live.rs) — `compare_element` alone only catches a
    // dropped element.
    let rust_names: std::collections::BTreeSet<String> =
        snaps.iter().map(|s| s.name.to_lowercase()).collect();
    let oracle_names: std::collections::BTreeSet<String> = sc
        .final_elements
        .iter()
        .map(|e| e.name.to_lowercase())
        .collect();
    assert_eq!(
        rust_names,
        oracle_names,
        "{}: element name sets differ (Rust∖oracle={:?}, oracle∖Rust={:?})",
        sc.name,
        rust_names.difference(&oracle_names).collect::<Vec<_>>(),
        oracle_names.difference(&rust_names).collect::<Vec<_>>(),
    );
    for ec in &sc.final_elements {
        compare_element(&snaps, ec, tol, &format!("{} final", sc.name));
    }

    // Event log: normalized per-line equality (numbers at 1e-6 rel) — the full
    // trip/reclose/blow/reset sequence, line-for-line.
    let log = dss.event_log();
    assert_eq!(
        log.len(),
        sc.event_log.len(),
        "{}: event log length differs:\n  actual:\n    {}\n  expected:\n    {}",
        sc.name,
        log.join("\n    "),
        sc.event_log.join("\n    ")
    );
    for (i, (a, e)) in log.iter().zip(&sc.event_log).enumerate() {
        assert_value_matches_tol(a, e, 1e-6, 1e-9, &format!("{} event-log line {i}", sc.name));
    }
}

#[test]
fn protection_scenarios_match_oracle() {
    let scenarios = load_scenarios();
    // Every protection device must stay represented, so a future edit can't
    // silently drop a device's trip/reclose coverage (mirrors the count guards in
    // golden_{timeseries_controls,metering_monitors,der_controls} and corpus_live's depth guard).
    // NB: the recloser_temp/recloser_perm scenarios were retired in WP-U2.2 and
    // relay_current in WP-U2.3 — the r4133 per-phase rewrites (event-log wording
    // overhaul + unconditional Debug Sample line + inst-delay single-count) move
    // their behavior off the pinned 0.14.5 oracle, so they are no longer
    // byte-golden-able (§1.3-2). Their trip/reclose coverage lives in the live
    // r4133 family gates (tests/corpus/controls/{recloser,relay}/*, oracle:
    // "r4133").
    for must in ["fuse_blow", "swt_manual"] {
        assert!(
            scenarios.iter().any(|s| s.name == must),
            "protection golden missing required scenario {must}"
        );
    }
    let tol = tol_for("large");
    for sc in &scenarios {
        run_scenario(sc, &tol);
    }
}

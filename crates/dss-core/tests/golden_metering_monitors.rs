//! Phase 6 golden (PHASE6_PLAN.md WP6.9 / §1.2): command-replay scenarios that
//! pin the meter / monitor / generator / topology machinery against the pinned
//! oracle (`tools/golden/gen_metering_monitors.py`). The Rust engine replays each
//! scenario's command list and must match its file under `tests/golden/metering_monitors/`
//! (one `<scenario>.json` per scenario; the gate runs every file in the dir):
//!
//!   - monitor_daily_ieee13: per-monitor header + SampleCount exact, channel
//!     sample arrays elementwise (only the mode-5 wall-clock timing channels
//!     are skipped — the port records 0 for them; the iteration-count channels
//!     now match the oracle exactly);
//!   - meter_daily_ieee13: registers 1e-4 rel + names exact, zone branch / end
//!     / PCE counts exact;
//!   - generator_snap: iterations + node order exact, voltages 1e-6 rel, each
//!     generator's terminal powers 1e-6 rel;
//!   - meter_zone_micro: zone membership exact for the meter and its sub-meter.
//!
//! Regenerate only manually: `python tools/golden/gen_metering_monitors.py`.

mod harness;

use std::path::PathBuf;

use dss_core::exec::Dss;
use harness::{
    MeterCap, MonitorCap, assert_complex_close, compare_meter, compare_monitor, tol_for,
};
use serde::Deserialize;

/// One scenario file: `{schema, oracle, scenario}` (the `oracle` block is
/// ignored here).
#[derive(Debug, Deserialize)]
struct ScenarioFile {
    schema: u32,
    scenario: Scenario,
}

#[derive(Debug, Deserialize)]
struct Scenario {
    name: String,
    commands: Vec<String>,
    #[serde(default)]
    monitors: Vec<MonitorCap>,
    #[serde(default)]
    meters: Vec<MeterCap>,
    #[serde(default)]
    meter_zones: Vec<ZoneExpect>,
    // generator_snap fields:
    #[serde(default)]
    iterations: i32,
    #[serde(default)]
    converged: bool,
    #[serde(default)]
    node_order: Vec<String>,
    #[serde(default)]
    v_re: Vec<f64>,
    #[serde(default)]
    v_im: Vec<f64>,
    #[serde(default)]
    generators: Vec<GenExpect>,
}

#[derive(Debug, Deserialize)]
struct ZoneExpect {
    name: String,
    branches: Vec<String>,
    ends: Vec<String>,
    pce: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct GenExpect {
    name: String,
    powers: Vec<f64>,
}

/// Load every `*.json` scenario file from `tests/golden/metering_monitors/`, sorted by
/// file name for deterministic run order.
fn load_scenarios() -> Vec<Scenario> {
    let dir: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "golden",
        "metering_monitors",
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
                "{}: metering_monitors golden schema mismatch",
                p.display()
            );
            f.scenario
        })
        .collect()
}

fn replay(sc: &Scenario) -> Dss {
    let mut dss = Dss::new();
    dss.command("clear");
    for cmd in &sc.commands {
        dss.command(cmd);
    }
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "{}: unexpected engine errors: {:?}",
        sc.name,
        dss.errors()
    );
    dss
}

fn run_monitors(sc: &Scenario, dss: &Dss) {
    // Channels are stored f32; the underlying f64 daily trajectory tracks the
    // oracle to ~1e-9 (the load-Yeq restamp fix in `build_y_matrix`), so the
    // narrowed f32 samples match at the feeder class tolerance (1e-6 rel /
    // 1e-4 abs, PHASE6_PLAN §1.2). Header strings, SampleCount and the discrete
    // structure are matched exactly; the mode-5 wall-clock channels are skipped
    // via `skip_channels`. Identical comparator as the live corpus gate.
    let tol = tol_for("large");
    for m in &sc.monitors {
        compare_monitor(dss, m, &tol, &sc.name);
    }
}

fn run_meters(sc: &Scenario, dss: &Dss) {
    // Registers at the energy-accumulation policy (PORTING_PLAN §4
    // `energy_rel`/`energy_abs`), names exact, zone branch/end/PCE counts exact.
    // Identical comparator as the live corpus gate.
    let tol = tol_for("large");
    for m in &sc.meters {
        compare_meter(dss, m, &tol, &sc.name);
    }
}

fn run_meter_zones(sc: &Scenario, dss: &Dss) {
    for z in &sc.meter_zones {
        let zone = dss
            .meter_zone(&z.name)
            .unwrap_or_else(|| panic!("{}: no meter zone {}", sc.name, z.name));
        assert_lists_eq_ci(
            &zone.all_branches_in_zone,
            &z.branches,
            &sc.name,
            &z.name,
            "branches",
        );
        assert_lists_eq_ci(&zone.all_end_elements, &z.ends, &sc.name, &z.name, "ends");
        assert_lists_eq_ci(&zone.zone_pce, &z.pce, &sc.name, &z.name, "pce");
    }
}

fn assert_lists_eq_ci(
    actual: &[String],
    expected: &[String],
    scenario: &str,
    meter: &str,
    what: &str,
) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "{scenario}: meter {meter} {what} count differs: {actual:?} vs {expected:?}"
    );
    for (a, e) in actual.iter().zip(expected) {
        assert!(
            a.eq_ignore_ascii_case(e),
            "{scenario}: meter {meter} {what} differs: {a} vs {e}"
        );
    }
}

fn run_generator_snap(sc: &Scenario, dss: &mut Dss) {
    let ckt = dss.circuit().expect("circuit after compile");
    assert!(sc.converged, "{}: oracle did not converge", sc.name);
    assert!(ckt.is_solved, "{}: Rust solution did not converge", sc.name);
    assert_eq!(
        ckt.solution.iteration, sc.iterations,
        "{}: iteration count differs",
        sc.name
    );
    let names: Vec<String> = (1..=ckt.num_nodes).map(|i| ckt.node_name(i)).collect();
    assert_eq!(names, sc.node_order, "{}: node order differs", sc.name);

    let mut actual = Vec::with_capacity(2 * ckt.num_nodes);
    for i in 1..=ckt.num_nodes {
        actual.push(ckt.solution.node_v[i].re);
        actual.push(ckt.solution.node_v[i].im);
    }
    let mut expected = Vec::with_capacity(actual.len());
    for (re, im) in sc.v_re.iter().zip(&sc.v_im) {
        expected.push(*re);
        expected.push(*im);
    }
    assert_complex_close(
        &actual,
        &expected,
        1e-6,
        1e-9,
        &format!("{} voltages", sc.name),
    );

    // Generator terminal powers (kW/kvar interleaved) in creation order.
    let snaps = dss.snapshot_elements();
    for g in &sc.generators {
        let snap = snaps
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(&g.name))
            .unwrap_or_else(|| panic!("{}: no element {}", sc.name, g.name));
        assert_complex_close(
            &snap.powers,
            &g.powers,
            1e-6,
            1e-4,
            &format!("{} {} powers", sc.name, g.name),
        );
    }
}

#[test]
fn metering_monitors_scenarios_match_oracle() {
    let scenarios = load_scenarios();
    assert_eq!(scenarios.len(), 4, "expected 4 metering_monitors scenarios");
    for sc in &scenarios {
        let mut dss = replay(sc);
        match sc.name.as_str() {
            "monitor_daily_ieee13" => run_monitors(sc, &dss),
            "meter_daily_ieee13" => run_meters(sc, &dss),
            "generator_snap" => run_generator_snap(sc, &mut dss),
            "meter_zone_micro" => run_meter_zones(sc, &dss),
            other => panic!("unknown scenario {other}"),
        }
    }
}

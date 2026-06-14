//! Phase 6 golden (PHASE6_PLAN.md WP6.9 / §1.2): command-replay scenarios that
//! pin the meter / monitor / generator / topology machinery against the pinned
//! oracle (`tools/golden/gen_phase6.py`). The Rust engine replays each
//! scenario's command list and must match its file under `tests/golden/phase6/`
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
//! Regenerate only manually: `python tools/golden/gen_phase6.py`.

mod harness;

use std::path::PathBuf;

use dss_core::exec::Dss;
use harness::assert_complex_close;
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
    monitors: Vec<MonitorExpect>,
    #[serde(default)]
    meters: Vec<MeterExpect>,
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
struct MonitorExpect {
    name: String,
    header: Vec<String>,
    sample_count: i32,
    channels: Vec<Vec<f64>>,
    /// 0-based channel indices to skip (mode-5 wall-clock timings).
    skip_channels: Vec<usize>,
}

#[derive(Debug, Deserialize)]
struct MeterExpect {
    name: String,
    register_names: Vec<String>,
    register_values: Vec<f64>,
    n_branches: usize,
    n_ends: usize,
    n_pce: usize,
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

/// Load every `*.json` scenario file from `tests/golden/phase6/`, sorted by
/// file name for deterministic run order.
fn load_scenarios() -> Vec<Scenario> {
    let dir: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "golden",
        "phase6",
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
                "{}: phase6 golden schema mismatch",
                p.display()
            );
            f.scenario
        })
        .collect()
}

/// Scalar element-wise closeness for one monitor channel.
fn assert_scalar_close(actual: &[f32], expected: &[f64], rel: f64, abs: f64, what: &str) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "{what}: sample count mismatch ({} vs {})",
        actual.len(),
        expected.len()
    );
    for (i, (a, e)) in actual.iter().zip(expected).enumerate() {
        let a = *a as f64;
        let allowed = abs + rel * e.abs();
        assert!(
            (a - e).abs() <= allowed,
            "{what}: sample {i} differs: actual {a} vs expected {e} \
             (|diff| = {:e} > allowed {:e})",
            (a - e).abs(),
            allowed
        );
    }
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
    for m in &sc.monitors {
        let view = dss
            .monitor_view(&m.name)
            .unwrap_or_else(|| panic!("{}: no monitor {}", sc.name, m.name));
        // Our `monitor_view.header` carries the full Pascal header, which leads
        // with the `hour` / `t(sec)` time columns; dss-python's `Monitors.Header`
        // is the data channels only (the hour column is its separate `dblHour`).
        // Compare the data channels (`channel(i)` already skips the time slots).
        assert_eq!(
            &view.header[2..],
            m.header.as_slice(),
            "{}: monitor {} header differs",
            sc.name,
            m.name
        );
        assert_eq!(
            view.sample_count, m.sample_count,
            "{}: monitor {} sample count differs",
            sc.name, m.name
        );
        assert_eq!(
            view.channels.len(),
            m.channels.len(),
            "{}: monitor {} channel count differs",
            sc.name,
            m.name
        );
        for (ch, (act, exp)) in view.channels.iter().zip(&m.channels).enumerate() {
            if m.skip_channels.contains(&ch) {
                continue;
            }
            // Channels are stored f32; the underlying f64 daily trajectory now
            // tracks the oracle to ~1e-9 (the load-Yeq restamp fix in
            // `build_y_matrix`), so the narrowed f32 samples match to a few
            // ULPs. 1e-6 rel / 1e-4 abs is the planned f32 channel tolerance
            // (PHASE6_PLAN §1.2); header strings, SampleCount and the discrete
            // structure are matched exactly.
            assert_scalar_close(
                act,
                exp,
                1e-6,
                1e-4,
                &format!(
                    "{} monitor {} channel {} ({})",
                    sc.name,
                    m.name,
                    ch + 1,
                    m.header[ch]
                ),
            );
        }
    }
}

fn run_meters(sc: &Scenario, dss: &Dss) {
    for m in &sc.meters {
        let regs = dss
            .meter_registers(&m.name)
            .unwrap_or_else(|| panic!("{}: no meter {}", sc.name, m.name));
        assert_eq!(
            regs.len(),
            m.register_names.len(),
            "{}: meter {} register count differs",
            sc.name,
            m.name
        );
        for (i, (name, value)) in regs.iter().enumerate() {
            assert_eq!(
                name, &m.register_names[i],
                "{}: meter {} register {i} name differs",
                sc.name, m.name
            );
            let exp = m.register_values[i];
            // 1e-4 rel — the PORTING_PLAN §4 energy-accumulation policy, same as
            // the 8500 gate. The threshold-crossing overload/EEN/UE registers
            // used to need 1e-3 here because the IEEE13 daily fixed-point path
            // drifted from the oracle (the load-Yeq accelerator was frozen at
            // the first step's load level — see `build_y_matrix`); with that
            // fixed the per-step trajectory tracks the oracle to ~1e-9 and every
            // register matches at 1e-4 (worst observed ~1e-7).
            let tol = 1e-4 * exp.abs().max(1.0);
            assert!(
                (value - exp).abs() <= tol,
                "{}: meter {} register {name} differs: {value} vs {exp}",
                sc.name,
                m.name
            );
        }
        let zone = dss
            .meter_zone(&m.name)
            .unwrap_or_else(|| panic!("{}: no meter zone {}", sc.name, m.name));
        assert_eq!(
            zone.all_branches_in_zone.len(),
            m.n_branches,
            "{}: meter {} branch count differs",
            sc.name,
            m.name
        );
        assert_eq!(
            zone.all_end_elements.len(),
            m.n_ends,
            "{}: meter {} end count differs",
            sc.name,
            m.name
        );
        assert_eq!(
            zone.zone_pce.len(),
            m.n_pce,
            "{}: meter {} PCE count differs",
            sc.name,
            m.name
        );
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
fn phase6_scenarios_match_oracle() {
    let scenarios = load_scenarios();
    assert_eq!(scenarios.len(), 4, "expected 4 phase6 scenarios");
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

//! Phase 8 report goldens (PHASE8_PLAN §2.3): pin the **report file the oracle
//! writes**. `tools/golden/gen_phase8.py` runs a fixture on the pinned engine,
//! issues the `Export`/`Show`/... command, and captures the produced file's
//! bytes into `tests/golden/phase8/<report>.txt` (+ a `<report>.meta.json` with
//! the exact deck, so the Rust and oracle fixtures can never drift). This driver
//! replays the same deck, writes its own report into a temp dir, and diffs the
//! two **after parsing numbers out** via `harness::compare_export` — never a raw
//! float-string diff.
//!
//! WP8.1 self-test: `Export Counts` — a text dump of every DSS class and its
//! instance count. The Rust class registry is a *proper subset* of the oracle's
//! (only a subset of classes is ported), so the comparison is `RustSubsetByKey`:
//! every ported class's count is pinned against the oracle; the classes we don't
//! yet register are ignored (see `tests/TOLERANCE_NOTES.md`). This is a real
//! gate (it shakes out the whole output path: registry walk → format → file IO →
//! the `compare_export` harness), not a self-comparison.
//!
//! Regenerate only manually: `python tools/golden/gen_phase8.py`.

mod harness;

use std::path::PathBuf;

use dss_core::exec::Dss;
use harness::{
    ColSel, ColTol, ExportPolicy, GateSpec, RowPolicy, assert_value_matches_tol, compare_export,
};
use serde::Deserialize;

fn phase8_dir() -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "golden",
        "phase8",
    ]
    .iter()
    .collect()
}

#[derive(Debug, Deserialize)]
struct ReportMeta {
    report: String,
    fixture: String,
    deck: Vec<String>,
}

/// Meta for a **deck-based** export golden (a self-contained `New`-circuit deck
/// with no master, like `ReportMeta` but carrying the oracle default-filename
/// suffix so the produced path is pinned — the reliability/capacity fixtures).
#[derive(Debug, Deserialize)]
struct DeckMeta {
    report: String,
    fixture: String,
    suffix: String,
    deck: Vec<String>,
}

/// Drive one deck-based export: replay the deck (no compile), route the report
/// into a scratch dir, export, and diff the produced file against the captured
/// oracle file via `compare_export`. The deck-fixture twin of `run_feeder_export`.
fn run_deck_export(stem: &str, policy: &ExportPolicy) {
    let dir = phase8_dir();
    let meta: DeckMeta = {
        let p = dir.join(format!("{stem}.meta.json"));
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };
    let oracle = {
        let p = dir.join(format!("{stem}.txt"));
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    };

    let scratch = scratch_dir(stem);
    let mut dss = Dss::new();
    dss.command("clear");
    for c in &meta.deck {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command(&format!("export {}", meta.report));
    assert!(dss.errors().is_empty(), "{stem}: {:?}", dss.errors());

    let produced = dss.last_result_file();
    let want = format!("{}_{}", meta.fixture, meta.suffix).to_lowercase();
    assert!(
        produced.to_lowercase().ends_with(&want),
        "{stem}: unexpected produced path {produced:?} (want …{want})"
    );
    let rust = std::fs::read_to_string(produced)
        .unwrap_or_else(|e| panic!("read produced {produced}: {e}"));

    compare_export(&oracle, &rust, policy, stem);

    std::fs::remove_dir_all(&scratch).ok();
}

/// A unique scratch dir for this test process (no `tempfile` dep; cleaned up at
/// the end). `Set DataPath=` points the engine's report output here.
fn scratch_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("dss_phase8_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap_or_else(|e| panic!("mkdir {}: {e}", d.display()));
    d
}

/// `Export Counts` (Pascal `ExportCounts`): every class + its instance count.
/// The Rust file must be a `RustSubsetByKey` subset of the oracle's (the Rust
/// class registry is a proper subset during the port); shared classes' counts
/// are pinned exactly (integers — zero tolerance).
#[test]
fn export_counts_matches_oracle() {
    let dir = phase8_dir();
    let meta: ReportMeta = {
        let p = dir.join("export_counts.meta.json");
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };
    assert_eq!(meta.report, "Counts");
    let oracle = {
        let p = dir.join("export_counts.txt");
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    };

    let scratch = scratch_dir("counts");
    let mut dss = Dss::new();
    dss.command("clear");
    for c in &meta.deck {
        dss.command(c);
    }
    // Route the report into the scratch dir (the same way the oracle fixture
    // did), then export. Quote the path in case of spaces.
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("export counts");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let produced = dss.last_result_file();
    assert!(
        produced
            .to_lowercase()
            .ends_with(&format!("{}_exp_counts.csv", meta.fixture)),
        "unexpected produced path: {produced:?}"
    );
    let rust = std::fs::read_to_string(produced)
        .unwrap_or_else(|e| panic!("read produced {produced}: {e}"));

    let policy = ExportPolicy {
        sep: '=',
        header_lines: 1, // "Format: DSS Class Name = Instance Count"
        rows: RowPolicy::RustSubsetByKey {
            key: 0,
            // Must-emit classes: the deck-created (Line/Load/Vsource) + the
            // default DSS items (TCC_Curve/Spectrum/LoadShape/GrowthShape). Guards
            // against a registry-walk regression that drops a class or empties the
            // body (the subset compare alone can't see a missing row).
            require: [
                "line",
                "load",
                "vsource",
                "tcc_curve",
                "spectrum",
                "loadshape",
                "growthshape",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        },
        rel: 0.0,
        abs: 0.0, // integer counts — exact
        col_tol: vec![],
    };
    compare_export(&oracle, &rust, &policy, "export_counts");

    std::fs::remove_dir_all(&scratch).ok();
}

/// Meta for a solution-export golden (a feeder compiled + solved on both
/// engines, then one report captured): the master to compile (relative to
/// `tests/corpus/electricdss-tst`), the post commands, the circuit/CaseName, and
/// the oracle's default-filename suffix.
#[derive(Debug, Deserialize)]
struct FeederMeta {
    report: String,
    master: String,
    post: Vec<String>,
    fixture: String,
    suffix: String,
}

/// Drive one solution export: compile the same master the oracle used, replay
/// the post commands, route the report into a scratch dir, export, and diff the
/// produced file against the captured oracle file via `compare_export`.
fn run_feeder_export(stem: &str, policy: &ExportPolicy) {
    let dir = phase8_dir();
    let meta: FeederMeta = {
        let p = dir.join(format!("{stem}.meta.json"));
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };
    let oracle = {
        let p = dir.join(format!("{stem}.txt"));
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    };

    let master: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "electricdss-tst",
    ]
    .iter()
    .collect::<PathBuf>()
    .join(&meta.master);
    assert!(master.is_file(), "master missing: {}", master.display());

    let scratch = scratch_dir(stem);
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        master.to_string_lossy().replace('\\', "/")
    ));
    for c in &meta.post {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command(&format!("export {}", meta.report));
    assert!(dss.errors().is_empty(), "{stem}: {:?}", dss.errors());

    let produced = dss.last_result_file();
    let want = format!("{}_{}", meta.fixture, meta.suffix).to_lowercase();
    assert!(
        produced.to_lowercase().ends_with(&want),
        "{stem}: unexpected produced path {produced:?} (want …{want})"
    );
    let rust = std::fs::read_to_string(produced)
        .unwrap_or_else(|e| panic!("read produced {produced}: {e}"));

    compare_export(&oracle, &rust, policy, stem);

    std::fs::remove_dir_all(&scratch).ok();
}

/// Compile a **heavy** master once and diff several reports against the oracle,
/// avoiding a per-report recompile (IEEE 8500 is ~6100 devices / 8531 nodes).
/// Each `(stem, policy)` reads its own `<stem>.meta.json`; all must agree on the
/// master/post/fixture (asserted — they are the same solved circuit), single-
/// sourced exactly like `run_feeder_export`.
fn run_shared_exports(reports: &[(&str, ExportPolicy)]) {
    let dir = phase8_dir();
    let metas: Vec<FeederMeta> = reports
        .iter()
        .map(|(stem, _)| {
            let p = dir.join(format!("{stem}.meta.json"));
            let text =
                std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
        })
        .collect();
    let m0 = &metas[0];
    for m in &metas[1..] {
        assert_eq!(m.master, m0.master, "shared exports disagree on master");
        assert_eq!(m.post, m0.post, "shared exports disagree on post");
        assert_eq!(m.fixture, m0.fixture, "shared exports disagree on fixture");
    }

    let master: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "electricdss-tst",
    ]
    .iter()
    .collect::<PathBuf>()
    .join(&m0.master);
    assert!(master.is_file(), "master missing: {}", master.display());

    let scratch = scratch_dir(reports[0].0);
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        master.to_string_lossy().replace('\\', "/")
    ));
    for c in &m0.post {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));

    for ((stem, policy), m) in reports.iter().zip(&metas) {
        dss.command(&format!("export {}", m.report));
        assert!(dss.errors().is_empty(), "{stem}: {:?}", dss.errors());
        let produced = dss.last_result_file();
        let want = format!("{}_{}", m.fixture, m.suffix).to_lowercase();
        assert!(
            produced.to_lowercase().ends_with(&want),
            "{stem}: unexpected produced path {produced:?} (want …{want})"
        );
        let rust = std::fs::read_to_string(produced)
            .unwrap_or_else(|e| panic!("read produced {produced}: {e}"));
        let oracle = {
            let p = dir.join(format!("{stem}.txt"));
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
        };
        compare_export(&oracle, &rust, policy, stem);
    }

    std::fs::remove_dir_all(&scratch).ok();
}

/// Default per-number tolerance for the `%g`-formatted value columns of the
/// solution exports: the oracle writes 5–6 significant digits, so the report is
/// known only to ~1e-5 rel; `1e-4` clears that formatting floor plus the two
/// independent solves with margin. The **primary** voltage-correctness gate is
/// the live full-model compare (`corpus_live.rs`, 1e-8 rel) — this golden pins
/// the report *layout* (header, column set/order, row order, scaling), not the
/// physics. See `tests/TOLERANCE_NOTES.md`.
const EXPORT_REL: f64 = 1e-4;
const EXPORT_ABS: f64 = 1e-6;

/// `Export Voltages` (Pascal `ExportVoltages`) on solved IEEE13: per-bus node
/// magnitude/angle/pu, zero-filled to the max node count.
#[test]
fn export_voltages_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: EXPORT_REL,
        abs: EXPORT_ABS,
        // The `Angle%d` columns are `%6.1f` (one decimal). The floor is purely
        // additive (a ±0.1 last-digit boundary between two independent solves —
        // no multiplicative `%f` component), so `rel = 0` / `abs = 0.11` is the
        // exact proven printing floor; the magnitude/pu columns keep the tight
        // default. The angle is `arg(V)`, gated by the engine's voltage physics
        // in `corpus_live`; here it is only a printing-floor layout check.
        // (tests/TOLERANCE_NOTES.md)
        col_tol: vec![ColTol {
            sel: ColSel::Prefix("angle".to_string()),
            rel: 0.0,
            abs: 0.11,
            gate: None,
        }],
    };
    run_feeder_export("export_voltages", &policy);
}

/// `Export BusCoords` (Pascal `ExportBusCoords`): X/Y of every coord-defined bus.
/// No header row; coordinates are `%-13.11g` (11 sig) loaded from the same file,
/// so they match tightly.
#[test]
fn export_buscoords_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: EXPORT_REL,
        abs: EXPORT_ABS,
        col_tol: vec![],
    };
    run_feeder_export("export_buscoords", &policy);
}

/// `Export NodeNames` (Pascal `ExportNodeNames`): `BusName.NodeNum` per line,
/// pure text (compared case-insensitively).
#[test]
fn export_nodenames_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_export("export_nodenames", &policy);
}

/// `Export YNodeList` (Pascal `ExportYNodeList`): node names in Y-matrix order,
/// quoted, pure text.
#[test]
fn export_ynodelist_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_export("export_ynodelist", &policy);
}

// --- WP8.2 sub-step 2a: the real-power element exports ----------------------
// These walk the Sources/PDElements/Faults/PCElements lists calling the mutating
// terminal getters (`Power`/`GetLosses`/`ComputeVterminal`/`ComputeIterminal`).
// All columns are real (kW/kvar/W) — no angle/sequence columns — so the floors
// are the plain fixed-decimal / `%g` printing floors, NOT a physics relaxation:
// the engine's V/I/P physics is pinned to 1e-8 by `corpus_live.rs`; here we gate
// the report layout (header, column set, element order, scaling).
// (tests/TOLERANCE_NOTES.md)

/// `Export Powers` (Pascal `ExportPowers`): per-terminal kW/kvar of every PD then
/// PC element, plus each PD's terminal-1 normal/emergency excess kVA. Every value
/// column is `%11.1f` (one decimal): an additive ±0.1 last-digit boundary between
/// two independent solves, so `rel = 0` / `abs = 0.11` is the exact printing floor
/// (the integer Terminal column is exact within it).
#[test]
fn export_powers_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.11,
        col_tol: vec![],
    };
    run_feeder_export("export_powers", &policy);
}

/// `Export Losses` (Pascal `ExportLosses`): per-PD-element total / load / no-load
/// losses in W and var, `%.7g` (7 sig). The proven floor is the **7-sig printing
/// floor**: empirically the max Rust↔oracle divergence on a substantial loss is
/// 1.53e-7 rel (one ULP at 7 sig, on REG2's 65.3 W) — so `rel = 1e-6` (≈6× over
/// the floor) is the report's printing resolution, **not** the looser `EXPORT_REL`
/// of the 6-sig Voltages report. `abs = 1e-6` absorbs the handful of near-zero
/// no-load/var cells (cancellation noise ≤ 5.6e-9 W, e.g. an open-circuit shunt
/// term) with ~180× margin. (tests/TOLERANCE_NOTES.md)
const LOSSES_REL: f64 = 1e-6;
#[test]
fn export_losses_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: LOSSES_REL,
        abs: EXPORT_ABS,
        col_tol: vec![],
    };
    run_feeder_export("export_losses", &policy);
}

/// `Export P_byphase` (Pascal `ExportPbyphase`): per-conductor kW/kvar over the
/// full Yorder. Values are `%10.3f` (three decimals) — a purely **additive** 1-ulp
/// floor (empirically the max divergence is exactly 1e-3, on the largest −1342.212
/// conductor; the corresponding 7.45e-7 rel is just `abs/value`). So like the
/// `Powers` `%11.1f` floor the whole policy is `rel = 0` / `abs = 0.0011`: no
/// multiplicative band (a per-conductor scale drift ≥ ~0.01% is caught, not
/// masked — mutation-confirmed), the integer NumTerminals/NumConductors/NumPhases
/// columns exact within abs. (tests/TOLERANCE_NOTES.md)
#[test]
fn export_p_byphase_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0011,
        col_tol: vec![],
    };
    run_feeder_export("export_p_byphase", &policy);
}

/// `Export Powers mva` (`opt=1`): the MVA option — the `m…` `Parm2` flag selects
/// MW/Mvar headers + the extra `×0.001` scaling. Backstops the `opt=1` branch
/// (scale + header) wired this WP, which the kVA golden can't reach. Same `%11.1f`
/// additive floor as the kVA twin (`rel = 0`, `abs = 0.11`, now in MW): a missing
/// `×0.001` would print kW values ~1000× larger and fail loudly.
#[test]
fn export_powers_mva_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.11,
        col_tol: vec![],
    };
    run_feeder_export("export_powers_mva", &policy);
}

/// `Export P_byphase mva` (`opt=1`): the MVA option for P_byphase — MW/Mvar header
/// plus the single extra `×0.001`. Same `%10.3f` additive floor as the kVA twin
/// (`rel = 0`, `abs = 0.0011`, in MW).
#[test]
fn export_p_byphase_mva_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0011,
        col_tol: vec![],
    };
    run_feeder_export("export_p_byphase_mva", &policy);
}

// --- WP8.2 sub-step 2b: the symmetrical-component family ---------------------
// `Phase2SymComp` + `PctNemaUnbalance` + PD ratings. Magnitude columns are `%g`
// (V1/V2/V0/Vresidual, I1/I2/I0/Iresidual = 6 sig); the ratio/unbalance columns
// (`%V2/V1`, `%V0/V1`, `%NEMA`, `%Normal`, `%Emergency`, `%I2/I1`, `%I0/I1`) are
// `%8.4g` = 4 sig. The sequence quantities V0/V2/I0/I2 are *near-cancellation
// residuals* of three balanced phasors: each phasor is pinned to 1e-8 rel by
// `corpus_live`, so the residual's ABSOLUTE error is ~phase_scale·1e-8, while its
// relative error is large — hence an `abs` floor on the magnitude columns and the
// 4-sig printing floor (`rel = 1e-3`) on the ratio columns. The physics is gated
// by `corpus_live` (node V / Iterminal to 1e-8); this golden pins report layout.
// (tests/TOLERANCE_NOTES.md)

/// `%g` ratio-column override: the `%8.4g` (4-sig) printing floor is `rel ≈ 1e-3`
/// (one ulp in the 4th significant digit); `abs` absorbs the *near-zero* ratio of
/// a small numerator over a healthy denominator (e.g. `%I0/I1` of a residual `I0`
/// over a loaded `I1`, measured max abs 1.2e-10). `gate` (the denominator column +
/// a meaningful-magnitude threshold) skips a ratio cell where its denominator is a
/// nonzero near-zero cancellation residual (an open/unloaded terminal); `None`
/// when the denominator is never near-zero (`SeqVoltages`' V1 is always kV-scale),
/// `Some((I1_col, …))` for `SeqCurrents`. `prefix` scopes the override — a gate
/// must cover only the columns whose denominator it actually tests (`%I…`/`%NEMA`
/// divide by I1; `%Normal`/`%Emergency` divide by NormAmps and stay ungated).
fn pct_ratio_tol(prefix: &str, abs: f64, gate: Option<GateSpec>) -> ColTol {
    ColTol {
        sel: ColSel::Prefix(prefix.to_string()),
        rel: 1e-3,
        abs,
        gate,
    }
}

/// `Export SeqVoltages` (Pascal `ExportSeqVoltages`): per-bus V1/pu/baseKV/V2/
/// %V2V1/V0/%V0V1/Vresidual/%NEMA. Magnitudes keep the 6-sig/5-sig `EXPORT_REL`
/// default; `abs = 1e-9` absorbs the volt-scale cancellation residual of
/// V0/V2/Vresidual (**measured** max abs divergence on a balanced bus is 1e-11 V at
/// the source bus's V0, so 1e-9 is a 100× proven floor — far tighter than a guess);
/// the `%`-prefixed ratio columns use the 4-sig `%8.4g` printing floor (no gate —
/// every bus's V1 denominator is kV-scale).
#[test]
fn export_seqvoltages_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: EXPORT_REL,
        abs: 1e-9,
        col_tol: vec![pct_ratio_tol("%", 1e-9, None)],
    };
    run_feeder_export("export_seqvoltages", &policy);
}

/// `Export SeqCurrents` (Pascal `ExportSeqCurrents`): per-terminal I1/%Normal/
/// %Emergency/I2/%I2I1/I0/%I0I1/Iresidual/%NEMA over Sources→PD→PC→Faults. Same
/// floor structure as SeqVoltages (6-sig magnitudes + 4-sig ratios); `abs = 1e-8`
/// absorbs the amp-scale residual of I0/I2/Iresidual (**measured** max abs
/// divergence 1e-9 A on a near-zero Iresidual, so 1e-8 is a 10× proven floor — and
/// it is *below* the smallest real printed magnitude, `I1 = 5.8e-4 A`, so those
/// cells stay pinned, unlike the earlier loose 1e-3). The ratio columns are
/// **gated on I1 (col 2)**: at an open/unloaded terminal (e.g. `Line.671680.2`, the
/// open 680 end) I1/I2/I0 are ~1e-12 cancellation noise pinned to 0 by `abs`, so
/// `%I2/I1`/`%I0/I1`/`%NEMA` are a faer-vs-KLU noise/noise form (147.7 vs 61.8) —
/// uncheckable, skipped where `0 < |I1| < 1e-6 A` (the band-limit keeps the 32
/// *exactly*-zero-current rows' `0 == 0` ratio checks; only the 1 genuine-noise row
/// is skipped). A proven cancellation floor; the magnitudes stay tight on every
/// row. (tests/TOLERANCE_NOTES.md)
#[test]
fn export_seqcurrents_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: EXPORT_REL,
        abs: 1e-8,
        // The I1 gate covers only the columns that actually divide by I1
        // (`%I2/I1`, `%I0/I1`) or are a same-noise form of the phase currents
        // (`%NEMA`); `%Normal`/`%Emergency` divide by NormAmps (never near-zero)
        // and fall through to the ungated `%` catch-all, so a regression there
        // stays checked even on the one gated noise row (audit follow-up).
        col_tol: vec![
            pct_ratio_tol("%i", 1e-8, Some(GateSpec::Col(2, 1e-6))),
            pct_ratio_tol("%nema", 1e-8, Some(GateSpec::Col(2, 1e-6))),
            pct_ratio_tol("%", 1e-8, None),
        ],
    };
    run_feeder_export("export_seqcurrents", &policy);
}

/// `Export SeqPowers` (Pascal `ExportSeqPowers`): per-terminal sequence powers
/// P1/Q1/P2/Q2/P0/Q0 (+ PD excess columns on terminal 1). All value columns are
/// `…:1` (one decimal), so — like `Powers` — the floor is the additive ±0.1
/// last-digit boundary: `rel = 0`, `abs = 0.11`. PD rows carry 12 fields (the
/// excess columns), PC rows 8; the comparator pins each row's field count.
#[test]
fn export_seqpowers_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.11,
        col_tol: vec![],
    };
    run_feeder_export("export_seqpowers", &policy);
}

// --- WP8.2 sub-step 2c: the per-terminal/per-conductor element exports -------
// `Currents`/`ElemCurrents`/`ElemVoltages` are magnitude (`%10.6g`, 6 sig) +
// angle (`%8.2f`, 2 decimals) reports over Sources→PD→Faults→PC; `ElemPowers`
// prints per-conductor kW/kvar (`%10.6g`); `NodeOrder` is integer node numbers;
// `Taps` is the RegControl tap table. As with 2a/2b the engine physics (node V /
// Iterminal) is pinned to 1e-8 by `corpus_live`; these goldens pin report layout.
//
// The **angle** of a near-zero magnitude (a per-terminal residual, or an
// open-terminal / grounded-neutral conductor) is faer-vs-KLU noise carrying no
// information, so the `ang`-prefixed columns are gated on their **paired
// magnitude** (the immediately-preceding column, `GateSpec::PrevCol`): skipped
// only where that magnitude is a nonzero sub-threshold residual, keeping every
// exactly-zero row's `0.00 == 0.00` check and every real-magnitude row's angle.
// A proven cancellation floor, not a relaxation. (tests/TOLERANCE_NOTES.md)

/// The `%8.2f` (2-decimal) additive printing floor for the angle columns of a
/// paired magnitude/angle report: two independent solves round the last 0.01
/// digit apart, so `rel = 0`, `abs = 0.011` (the exact ±0.01 last-digit
/// boundary). Selected by index parity (`start`, then every other column is an
/// angle) because these reports have **truncated headers** (`…, I_1, Ang_1,
/// ...`) that name only the first pair. Gated on the paired magnitude (the
/// immediately-preceding column) so the angle of a near-zero residual /
/// open-terminal / grounded-neutral conductor (which is faer-vs-KLU noise) is
/// skipped; `thresh` (1e-6) sits far above that noise (≤ ~5e-9) and below the
/// smallest real printed magnitude, so only genuine-noise angles are skipped.
fn ang_tol(start: usize) -> ColTol {
    ColTol {
        sel: ColSel::Parity { start, parity: 1 },
        rel: 0.0,
        abs: 0.011,
        gate: Some(GateSpec::PrevCol(1e-6)),
    }
}

/// `Export Currents` (Pascal `ExportCurrents` + `CalcAndWriteCurrents`):
/// per-terminal, per-conductor `|I|`/angle over the widest element, plus a
/// per-terminal residual. Magnitudes keep the 6-sig `EXPORT_REL`; `abs = 1e-6`
/// absorbs the near-zero `Iresid` cancellation residual (measured ≤ 1e-8 A) and
/// the exactly-zero conductor-width fill (`0 == 0`). Angle columns use the
/// `%8.2f` floor gated on their paired magnitude (the residual/fill angles are
/// noise).
#[test]
fn export_currents_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: EXPORT_REL,
        abs: 1e-6,
        col_tol: vec![ang_tol(1)],
    };
    run_feeder_export("export_currents", &policy);
}

/// `Export NodeOrder` (Pascal `ExportNodeOrder` + `WriteNodeList`): `"Element",
/// Nterms, Nconds, Node-1, …` — all integers (node numbers, 0 = ground), exact.
#[test]
fn export_nodeorder_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_export("export_nodeorder", &policy);
}

/// `Export ElemCurrents` (Pascal `WriteElemCurrents`): per-conductor `|I|`/angle
/// over `NConds·Nterms`. Same floor structure as `Currents` (magnitudes 6-sig +
/// `abs = 1e-6` for the near-zero open-terminal conductors; angles gated on their
/// paired magnitude).
#[test]
fn export_elemcurrents_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: EXPORT_REL,
        abs: 1e-6,
        col_tol: vec![ang_tol(3)],
    };
    run_feeder_export("export_elemcurrents", &policy);
}

/// `Export ElemVoltages` (Pascal `WriteElemVoltages`): per-conductor `|V|`/angle.
/// Magnitudes 6-sig; `abs = 1e-6` V absorbs the exactly-zero grounded-neutral
/// conductor; angles gated on their paired magnitude.
#[test]
fn export_elemvoltages_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: EXPORT_REL,
        abs: 1e-6,
        col_tol: vec![ang_tol(3)],
    };
    run_feeder_export("export_elemvoltages", &policy);
}

/// `Export ElemPowers` (Pascal `WriteElemPowers`): per-conductor `S =
/// Vterminal·conj(Iterminal)`, printed kW/kvar (`%10.6g`, 6 sig). Formed straight
/// from `ComputeVterminal`/`ComputeIterminal` (like `P_byphase`, no `×3`); on the
/// solved feeder equals the canonical terminal power (the `Vsource` row matches
/// `CktElement.Powers`, oracle-probed — the isolated-source 2a divergence never
/// appears here). Magnitudes 6-sig; `abs = 1e-6` kW absorbs the ~0 neutral-
/// conductor powers.
#[test]
fn export_elempowers_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: EXPORT_REL,
        abs: 1e-6,
        col_tol: vec![],
    };
    run_feeder_export("export_elempowers", &policy);
}

/// `Export Taps` (Pascal `ExportTaps`): one row per RegControl — the controlled
/// transformer's present/min/max tap + increment (`%8.5f`), integer tap position
/// and winding, and the Forward/Reverse + True/False mode text. The tap value is
/// a discrete `mid + position·increment`, so both engines land the identical
/// value once they converge to the same integer tap position (already pinned by
/// `corpus_live` and the feeder-controls gate) — `rel = 0`, `abs = 1e-4` catches
/// any tap-position divergence loudly (one position = 0.00625 ≫ 1e-4) while
/// clearing the `%8.5f` print floor.
#[test]
fn export_taps_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 1e-4,
        col_tol: vec![],
    };
    run_feeder_export("export_taps", &policy);
}

// --- WP8.2 sub-step 3: the matrix/summary exports ----------------------------
// `Y`/`Yprims` serialize the assembled/per-element admittance matrices (already
// pinned entry-by-entry by the checkpoint + live full-Y gates); `SeqZ` reads the
// per-bus short-circuit impedances; `Summary` is a one-row status line; `Result`
// dumps the `@result` parser var. These goldens pin the report *layout*.

/// `Export Result` (Pascal `ExportResult`): the `@result` parser var, one line.
/// In the pinned PM-build oracle `@result` is always `null` (the update is
/// compiled out), which our engine reproduces (`ParserVars::new` seeds `null`,
/// never rewritten). Pure text, no header.
#[test]
fn export_result_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_export("export_result", &policy);
}

/// `Export Summary` (Pascal `ExportSummary`): one status row (solve mode, counts,
/// iterations, pu-voltage extremes, total MW/Mvar, losses). Column 0 is the
/// wall-clock `DateTimeToStr(Now)` — non-deterministic, **masked**
/// (`GateSpec::Mask`). Every other column is deterministic: text fields
/// (`CaseName`/`Status`/`Mode`/`ControlMode`) compare case-insensitively; the
/// integer counts (NumDevices/Buses/Nodes, iteration counts) exact within `abs`;
/// the `%g` scalars (MaxPuVoltage/TotalMW/losses) keep the 5–6-sig `EXPORT_REL`
/// printing floor (the physics is pinned by `corpus_live`).
#[test]
fn export_summary_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: EXPORT_REL,
        abs: EXPORT_ABS,
        col_tol: vec![ColTol {
            sel: ColSel::Index(0), // DateTime — masked (non-deterministic clock)
            rel: 0.0,
            abs: 0.0,
            gate: Some(GateSpec::Mask),
        }],
    };
    run_feeder_export("export_summary", &policy);
}

/// `Export SeqZ` (Pascal `ExportSeqZ`): per-bus symmetrical-component short-circuit
/// impedances after a **FaultStudy** solve (a plain snapshot leaves `Zsc` zero).
/// `R1/X1/R0/X0/Z1/Z0` are `%10.6g` (6 sig) — the same `Zsc1`/`Zsc0` the
/// `fault_study.rs` gate pins to 1e-9·mag — so they keep the tight `EXPORT_REL`;
/// the `X1/R1`/`X0/R0` ratio columns (indices 8/9) are `%8.4g` (4 sig), so
/// `rel = 1e-3` is their printing floor. The integer NumNodes column is exact.
#[test]
fn export_seqz_matches_oracle() {
    let ratio = |i: usize| ColTol {
        sel: ColSel::Index(i),
        rel: 1e-3,
        abs: EXPORT_ABS,
        gate: None,
    };
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: EXPORT_REL,
        abs: EXPORT_ABS,
        col_tol: vec![ratio(8), ratio(9)],
    };
    run_feeder_export("export_seqz", &policy);
}

/// `Export Faultstudy` (Pascal `ExportFaultStudy`) on the FaultStudy-solved
/// IEEE13 feeder: per-bus 3-phase / 1-phase / L-L prospective fault currents.
/// The 3-phase column reads the precomputed `BusCurrent`; the 1-phase/L-L columns
/// are local per-bus `YFault` scratch inversions over the same precomputed `Ysc`
/// (PHASE8_PLAN §2.1). All three are `%10f` (2 decimals). The floor is the
/// standard 6-sig report floor (`EXPORT_REL`) — the faultstudy `Zsc`/`Ysc` are
/// pinned to 1e-9·mag by `exec/tests/fault_study.rs`, and the `YFault` inversions
/// run the same bit-faithful `CMatrix::invert` on both engines; `abs = 0.011`
/// absorbs the `%.2f` additive rounding on the exactly-`0.00` L-L rows of the
/// single-node buses (611/652). Not a physics relaxation.
///
/// The floor is purely the **`%.2f` additive printing floor** (`rel = 0` /
/// `abs = 0.011`): a rel-tolerance sweep confirmed the currents match to ≤1e-8 rel
/// (the golden passes unchanged at `rel = 1e-8`), so both engines compute the same
/// f64 and the only observable difference is a 0.005-boundary rounding split
/// (≤0.01), exactly as `Powers`/`P_byphase` are pinned. Measured, not guessed.
#[test]
fn export_faultstudy_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.011,
        col_tol: vec![],
    };
    run_feeder_export("export_faultstudy", &policy);
}

/// The assembled/primitive admittance matrices are exact deterministic stamps
/// (no faer solve enters them), printed to 10 sig, so both engines agree to
/// ~1e-10 rel; `1e-6` clears that printing floor plus the two independent builds
/// with wide margin. `abs = 1e-6` absorbs any near-zero off-diagonal cell.
const YMATRIX_REL: f64 = 1e-6;

/// `Export Y triplet` (Pascal `ExportY` `TripletOpt`): the assembled system Y as
/// `Row,Col,G,B` for the lower triangle (`row >= col`), column-major. Integer
/// Row/Col exact; G/B (`%.10g`) at the `YMATRIX_REL` printing floor.
#[test]
fn export_y_triplet_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: YMATRIX_REL,
        abs: EXPORT_ABS,
        col_tol: vec![],
    };
    run_feeder_export("export_y_triplet", &policy);
}

/// `Export Yprims` (Pascal `ExportYprim`): every enabled PD/PC element's
/// primitive Y, in device order — a `Class.NAME` header line then `Yorder` rows
/// of `re, im,` pairs (`%.10g`). No fixed header (the first line is an element
/// name), so `header_lines = 0`; the name lines are single text fields (matched
/// case-insensitively), the matrix cells at the `YMATRIX_REL` printing floor. The
/// element set/order is the report's contract (`ExactOrdered`).
#[test]
fn export_yprims_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: YMATRIX_REL,
        abs: EXPORT_ABS,
        col_tol: vec![],
    };
    run_feeder_export("export_yprims", &policy);
}

// --- WP8.2 follow-up: the remaining solution-family exports ------------------
// `VoltagesElements` (per-element terminal voltages — the by-element companion to
// `Voltages`) and the raw Y-ordered node vectors `YVoltages` (NodeV) / `YCurrents`
// (Solution.Currents). Read-only; the engine physics (node V / node currents) is
// pinned to 1e-8 by `corpus_live`, so these pin report layout.

/// `Export VoltagesElements` (Pascal `ExportVoltagesElements`): per-element,
/// per-terminal, per-conductor `Node`(index)/`Magnitude`(kV,`%10.6g`)/`Angle`
/// (`%6.3f`)/`pu`(`%9.5g`) + the per-terminal `Bus`/`BasekV`(`%6.3f`). Magnitude/
/// pu/BasekV keep the 6-sig `EXPORT_REL`; the `Angle*` columns take the `%6.3f`
/// additive printing floor (`rel = 0`, `abs = 0.0011`) — no gate needed (every
/// conductor is either energized or exactly-ground `0`, no cancellation residual).
/// Rows are ragged (an element writes only its own `NTerms` terminal blocks, not
/// padded to `MaxNumTerminals`); `ExactOrdered` pins each row's fields.
#[test]
fn export_voltageselements_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: EXPORT_REL,
        abs: EXPORT_ABS,
        col_tol: vec![ColTol {
            sel: ColSel::Prefix("angle".to_string()),
            rel: 0.0,
            abs: 0.0011, // %6.3f additive last-digit floor (3 decimals)
            gate: None,
        }],
    };
    run_feeder_export("export_voltageselements", &policy);
}

/// `Export YVoltages` (Pascal `ExportYVoltages`): the node voltage vector `NodeV`
/// for nodes `1..NumNodes`, one `re, im` pair per line, no header. `%10.6g` (6
/// sig) → `EXPORT_REL`; node voltages are kV-scale so `EXPORT_ABS` only backs the
/// occasional near-zero component.
#[test]
fn export_yvoltages_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: EXPORT_REL,
        abs: EXPORT_ABS,
        col_tol: vec![],
    };
    run_feeder_export("export_yvoltages", &policy);
}

/// `Export YCurrents` (Pascal `ExportYCurrents`): the node injection-current
/// vector `Solution.Currents` for nodes `1..NumNodes`, one `re, im` pair per line,
/// no header. Passive nodes carry an **exact** `0` injection (both engines); the
/// source/load nodes carry a large (~1e4 A) current pinned at `EXPORT_REL`, so
/// there is no near-zero cancellation floor (unlike `Iresidual`).
#[test]
fn export_ycurrents_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: EXPORT_REL,
        abs: EXPORT_ABS,
        col_tol: vec![],
    };
    run_feeder_export("export_ycurrents", &policy);
}

// --- WP8.2 completion gate: the bus/summary exports at scale (IEEE 8500) ------
// PHASE8_PLAN §WP8.2 step 4: `Voltages`/`Summary`/`Counts` on the solved IEEE
// 8500-Node feeder (8531 nodes, 6103 devices), completing the export-diff over
// 13/34/37/123/8500. The per-element/matrix dumps are omitted as enormous (the
// established 8500-golden discipline — `golden_ieee8500.rs`). All three share the
// one heavy compile+solve via `run_shared_exports`. The reports pin layout at
// scale (header/column set/order/scaling + the full 4876-bus row set); the engine
// physics is pinned by the always-on `corpus_live.rs` 8500 model compare.

/// `Voltages`/`Summary`/`Counts` on the solved IEEE 8500-Node feeder, from a
/// single compile. Voltages: the 6-sig `%g` magnitude/pu floor + the `%6.1f`
/// additive angle floor (identical to the IEEE13 voltages policy). Summary: the
/// masked non-deterministic `DateTime` column + the deterministic status row.
/// Counts: the `RustSubsetByKey` subset compare (`=`-separated), pinning every
/// ported class's instance count at scale (Line=3703/Transformer=1190/etc).
#[test]
fn export8500_reports_match_oracle() {
    let voltages = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: EXPORT_REL,
        abs: EXPORT_ABS,
        col_tol: vec![ColTol {
            sel: ColSel::Prefix("angle".to_string()),
            rel: 0.0,
            abs: 0.11,
            gate: None,
        }],
    };
    let summary = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: EXPORT_REL,
        abs: EXPORT_ABS,
        col_tol: vec![ColTol {
            sel: ColSel::Index(0), // DateTime — masked (non-deterministic clock)
            rel: 0.0,
            abs: 0.0,
            gate: Some(GateSpec::Mask),
        }],
    };
    let counts = ExportPolicy {
        sep: '=',
        header_lines: 1, // "Format: DSS Class Name = Instance Count"
        rows: RowPolicy::RustSubsetByKey {
            key: 0,
            // The core Phase 4-6 classes the 8500 feeder instantiates in bulk —
            // guards against a registry-walk regression dropping a class or
            // emptying the body (the subset compare alone can't see a missing row).
            require: [
                "line",
                "load",
                "transformer",
                "capacitor",
                "regcontrol",
                "capcontrol",
                "vsource",
                "reactor",
                "linecode",
                "xfmrcode",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        },
        rel: 0.0,
        abs: 0.0, // integer counts — exact
        col_tol: vec![],
    };
    run_shared_exports(&[
        ("export8500_voltages", voltages),
        ("export8500_summary", summary),
        ("export8500_counts", counts),
    ]);
}

// --- WP8.3 step 1: Export Monitors (Monitor.TranslateToCSV) -------------------
// `Export Monitors <name>` writes each monitor's in-memory f32 sample buffer to
// its own CSV (`<case>_Mon_<name>_1.csv`; the `_1` is the PM-build primary-context
// `DSS._Name`). Three monitors cover the distinct CSV shapes on one daily-solved
// IEEE13 compile: mode 0 (general V/I — paired magnitude/angle, unquoted header),
// mode 1 (power — the `S (kVA)`/`Ang` header whose spaces force `CommaText`
// quoting), and mode 2 (the single quoted `Tap (pu)` channel). The header line is
// compared **verbatim** (the `Header.CommaText` contract — quoting included); the
// data rows (incl. the `hour`/`t(sec)` time columns the live gate's channel
// compare skips) parse numbers out. Values are f32 (the monitor stream is
// single-precision on both engines), printed `%-.6g` (6 sig): the theoretical
// floor is the f32-quantization + 6-sig-print resolution (~1e-6 rel on the kV/A
// magnitudes; ~1e-7 abs on the one near-zero cell, `VAngle1 ≈ -0.013`) across the
// two independent solves — in practice the printed values are byte-identical here
// (measured max Rust↔oracle gap 0). `rel = 1e-4` / `abs = 1e-5` sit ~100× above
// that theoretical floor, so a real channel/stride/ordering bug (degrees-/amps-
// scale) or a rad↔deg swap fails loudly while the printing floor never flakes.
// The monitor *sampling code path* is 1e-8-gated by the always-on `corpus_live.rs`
// on equivalent `line.650632` daily monitors; this golden cross-checks these exact
// monitors' values (at the print floor) and pins the CSV **layout** (header
// CommaText, column order, row stride, the `_1` filename). All three share the one
// compile+daily-solve via `run_shared_exports`.
#[test]
fn export_monitors_match_oracle() {
    let mon = || ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 1e-4,
        abs: 1e-5,
        col_tol: vec![],
    };
    run_shared_exports(&[
        ("export_mon_vi", mon()),
        ("export_mon_pow", mon()),
        ("export_mon_tap", mon()),
    ]);
}

// --- WP8.3 step 2: the register/load dumps -----------------------------------
// `Export Meters`/`Generators`/`PVSystem_Meters`/`Storage_Meters` dump each enabled
// element's registers (`Year, LDCurve, Hour, <Name>` + `%10.0f` per register) under
// a `"quoted"` register-name header; `Export Loads` dumps the present allocation
// view. No corpus deck uses these keywords, so the fixtures are synthesized
// (PHASE8_PLAN §1). The header line is compared **verbatim** (the register-name
// set/quoting is the report's contract); the register values are `%10.0f` integers,
// so `rel = 0` / `abs = 0.5` pins each register's scale (one kWh unit ≫ 0.5) while
// clearing the last-integer rounding boundary. Two fixtures keep every value
// tightly oracle-pinned (no masking):
//   A) plain IEEE13 + an EnergyMeter, daily → Meters + Loads. The meter registers
//      match to ~1e-8 (the same daily meter path `corpus_live.rs` pins), so `%10.0f`
//      is identical.
//   B) IEEE13 + Generator + PVSystem + Storage (off the metered zone), daily → the
//      DER register dumps. Their own round kWh/kW registers match cleanly; keeping
//      the DER out of the metered, regulated zone avoids the ~3e-5 metered-element
//      coupling that would straddle the meter's Max kW rounding boundary.

/// Register rows: `%10.0f` integers — exact but for the last-digit rounding
/// boundary; `abs = 0.5` clears it, `rel = 0` keeps every register scale-pinned.
fn register_policy() -> ExportPolicy {
    ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.5,
        col_tol: vec![],
    }
}

/// `Export Meters` (Pascal `ExportMeters`/`WriteSingleMeterFile`) + `Export Loads`
/// (Pascal `ExportLoads`) on the daily-solved plain IEEE13 + EnergyMeter fixture.
#[test]
fn export_meters_loads_match_oracle() {
    // Loads columns: Load(0), ConnectedKVA(1,`%8.1f`), AllocFactor(2,`%5.3f`),
    // Phases(3), kW(4,`%8.1f`), kvar(5,`%8.1f`), PF(6,`%5.3f`), Model(7). Default
    // `abs = 0.05` = the `%8.1f` additive last-digit floor; the two `%5.3f`
    // columns (AllocFactor, PF) get the tighter `abs = 5e-4` (they are static
    // input echoes, so they should match exactly — this keeps them pinned rather
    // than letting the coarse default mask a 0.05 field-mapping slip).
    let three_dec = |i: usize| ColTol {
        sel: ColSel::Index(i),
        rel: 0.0,
        abs: 5e-4,
        gate: None,
    };
    let loads = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.05,
        col_tol: vec![three_dec(2), three_dec(6)],
    };
    run_shared_exports(&[
        ("export_meters", register_policy()),
        ("export_loads", loads),
    ]);
}

/// `Export Generators`/`PVSystem_Meters`/`Storage_Meters` (Pascal `ExportGenMeters`/
/// `ExportPVSystemMeters`/`ExportStorageMeters`) on the daily-solved IEEE13 + DER
/// fixture — each DER's kWh/kvarh/Max kW/Max kVA/Hours/Price registers.
#[test]
fn export_der_registers_match_oracle() {
    run_shared_exports(&[
        ("export_generators", register_policy()),
        ("export_pvsystem_meters", register_policy()),
        ("export_storage_meters", register_policy()),
    ]);
}

/// The `/m` multi-file switch (Pascal `WriteMultipleMeterFiles`): `Export Meters
/// /m` writes one `EXP_MTR_<NAME>.csv` per enabled meter (no `CaseName_` prefix)
/// and leaves `@lastexportfile` = the literal `/m` (Pascal's `DoExportCmd` tail
/// quirk). Self-consistency gate (no separate oracle capture): the per-meter file's
/// data row must carry the **same** register values as the single-file
/// `export_meters` golden already pins against the oracle — so a `/m` path bug
/// (wrong file name, dropped header, wrong element) fails loudly, while the values
/// stay oracle-anchored transitively.
#[test]
fn export_meters_multifile_switch() {
    let dir = phase8_dir();
    let meta: FeederMeta = {
        let p = dir.join("export_meters.meta.json");
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };
    let single = {
        let p = dir.join("export_meters.txt");
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    };

    let master: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "electricdss-tst",
    ]
    .iter()
    .collect::<PathBuf>()
    .join(&meta.master);
    assert!(master.is_file(), "master missing: {}", master.display());

    let scratch = scratch_dir("meters_multifile");
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        master.to_string_lossy().replace('\\', "/")
    ));
    for c in &meta.post {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("export meters /m");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    // The `/m` tail sets @lastexportfile / GlobalResult to the literal switch.
    assert_eq!(dss.last_result_file(), "/m");

    // The one enabled meter (EM1) got its own file, header + data row, values
    // identical to the single-file golden's EM1 row (transitively oracle-pinned).
    let em1 = scratch.join("EXP_MTR_EM1.csv");
    let multi =
        std::fs::read_to_string(&em1).unwrap_or_else(|e| panic!("read {}: {e}", em1.display()));
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.5,
        col_tol: vec![],
    };
    compare_export(&single, &multi, &policy, "export_meters_multifile");

    std::fs::remove_dir_all(&scratch).ok();
}

/// Compile the master + replay the post commands from `<stem>.meta.json`, route
/// reports into a fresh scratch dir, and hand the driven `Dss` + scratch path to
/// `body`. Shared by the append / Storage-`/m` self-consistency tests below.
fn with_register_fixture(stem: &str, tag: &str, body: impl FnOnce(&mut Dss, &std::path::Path)) {
    let dir = phase8_dir();
    let meta: FeederMeta = {
        let p = dir.join(format!("{stem}.meta.json"));
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };
    let master: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "electricdss-tst",
    ]
    .iter()
    .collect::<PathBuf>()
    .join(&meta.master);
    assert!(master.is_file(), "master missing: {}", master.display());

    let scratch = scratch_dir(tag);
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        master.to_string_lossy().replace('\\', "/")
    ));
    for c in &meta.post {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    body(&mut dss, &scratch);
    std::fs::remove_dir_all(&scratch).ok();
}

/// The single-file **append** path (Pascal `WriteSingleMeterFile`: append to a
/// file whose first line begins `Year`, no re-emitted header). Two `Export
/// Meters` into the same datapath must leave one header + two data rows — pinning
/// `register_need_rewrite`'s "append, don't rewrite" branch (the fresh-dir goldens
/// only ever exercise the create branch).
#[test]
fn export_meters_append_accumulates() {
    with_register_fixture("export_meters", "meters_append", |dss, _scratch| {
        dss.command("export meters");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        let path = dss.last_result_file().to_string();
        dss.command("export meters"); // second export → append, not rewrite
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        assert_eq!(
            dss.last_result_file(),
            path.as_str(),
            "append reused the same file"
        );

        let content = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
        let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(
            lines.len(),
            3,
            "append: expected 1 header + 2 rows, got {lines:?}"
        );
        assert!(lines[0].starts_with("Year"), "header line: {:?}", lines[0]);
        // Both rows are the same EM1 sample (a re-emitted header on append, or a
        // truncation, would break this).
        assert!(lines[1].contains("\"EM1\""), "row 1: {:?}", lines[1]);
        assert_eq!(lines[1], lines[2], "the two appended rows differ");
    });
}

/// The Storage `/m` path reproduces an **upstream copy-paste bug** — its per-file
/// prefix is `EXP_PV_`, not `EXP_STORAGE_` (`ExportResults.pas:2240`,
/// `TODO(compat)`). Pin that exact filename (and the transitively-oracle-anchored
/// row) so the deliberately-faithful quirk cannot silently drift to `EXP_STORAGE_`.
#[test]
fn export_storage_multifile_uses_pv_prefix() {
    let single = {
        let p = phase8_dir().join("export_storage_meters.txt");
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    };
    with_register_fixture(
        "export_storage_meters",
        "storage_multifile",
        |dss, scratch| {
            dss.command("export storage_meters /m");
            assert!(dss.errors().is_empty(), "{:?}", dss.errors());
            assert_eq!(dss.last_result_file(), "/m");

            // The copy-paste-bug prefix: EXP_PV_<NAME>.csv, NOT EXP_STORAGE_.
            let pv = scratch.join("EXP_PV_ST1.csv");
            assert!(
                pv.is_file(),
                "storage /m must write EXP_PV_ST1.csv (the EXP_PV_ prefix bug)"
            );
            assert!(
                !scratch.join("EXP_STORAGE_ST1.csv").exists(),
                "storage /m must NOT use an EXP_STORAGE_ prefix"
            );
            let multi = std::fs::read_to_string(&pv)
                .unwrap_or_else(|e| panic!("read {}: {e}", pv.display()));
            compare_export(
                &single,
                &multi,
                &register_policy(),
                "export_storage_multifile",
            );
        },
    );
}

// --- WP8.3 step 3a: the event/error-log dumps ------------------------------
// `Export EventLog` / `Export ErrorLog` (Pascal `ExportEventLog`/`ExportErrorLog`)
// are `TStringList.SaveToFile` dumps of `DSS.EventStrings` / `DSS.ErrorStrings`.
// The event-log lines are `Hour=…, Sec=…, Iteration=…, …` records: the stamps and
// the in-action tap values are *numbers*, so — like the phase5 event-log gate —
// they are compared line-for-line with numbers parsed out (`numeric_skeleton`),
// never as raw float-strings.

/// Compile the fixture, replay its post commands, export the log report into a
/// scratch dir, and compare the produced file to the oracle's line-for-line with
/// numbers parsed out at 1e-6 rel (the phase5 event-log policy). A count mismatch
/// (a missing/extra LogThisEvent marker) fails loudly before the per-line compare.
fn run_log_export(stem: &str) {
    let dir = phase8_dir();
    let meta: FeederMeta = {
        let p = dir.join(format!("{stem}.meta.json"));
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };
    let oracle = {
        let p = dir.join(format!("{stem}.txt"));
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    };

    let master: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "electricdss-tst",
    ]
    .iter()
    .collect::<PathBuf>()
    .join(&meta.master);
    assert!(master.is_file(), "master missing: {}", master.display());

    let scratch = scratch_dir(stem);
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        master.to_string_lossy().replace('\\', "/")
    ));
    for c in &meta.post {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command(&format!("export {}", meta.report));
    assert!(dss.errors().is_empty(), "{stem}: {:?}", dss.errors());

    let produced = dss.last_result_file();
    let want = format!("{}_{}", meta.fixture, meta.suffix).to_lowercase();
    assert!(
        produced.to_lowercase().ends_with(&want),
        "{stem}: unexpected produced path {produced:?} (want …{want})"
    );
    let rust = std::fs::read_to_string(produced)
        .unwrap_or_else(|e| panic!("read produced {produced}: {e}"));

    let ol: Vec<&str> = oracle.lines().filter(|l| !l.trim().is_empty()).collect();
    let rl: Vec<&str> = rust.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(
        rl.len(),
        ol.len(),
        "{stem}: log line count differs (rust {} vs oracle {})\n  rust:\n    {}\n  oracle:\n    {}",
        rl.len(),
        ol.len(),
        rl.join("\n    "),
        ol.join("\n    ")
    );
    for (i, (a, e)) in rl.iter().zip(&ol).enumerate() {
        assert_value_matches_tol(a, e, 1e-6, 1e-9, &format!("{stem} line {i}"));
    }

    std::fs::remove_dir_all(&scratch).ok();
}

/// `Export EventLog` (Pascal `ExportEventLog`): the full `Set Log=yes`
/// LogThisEvent marker stream (bus-def reprocess → meter-zone reset → Yprim
/// recalc → Y build → per-iteration/control markers → Solution Done) **plus**
/// the regulators' `AppendToEventLog` tap-change lines, over a daily-3 IEEE13
/// solve driven by a swinging load so all three regulators actually move taps
/// (18 `CHANGED n TAPS TO <pu>` lines — the non-integer tap values exercise the
/// numeric-tolerance compare, not just the integer stamps). Pins the export
/// **and** that our engine emits every LogThisEvent call site + every tap-change
/// action line-for-line vs the oracle (the Circuit-build markers landed in WP8.3
/// step 3a; the tap logic is the same the phase5 `daily_ieee13` gate pins).
#[test]
fn export_eventlog_matches_oracle() {
    run_log_export("export_eventlog");
}

/// `Export ErrorLog` (Pascal `ExportErrorLog`): a clean IEEE13 solve logs no
/// `DoSimpleMsg`, so the dump is empty — pins the plumbing + `EXP_ErrorLog.txt`
/// naming. The non-empty content path is gated by `export_errorlog_captures_errors`.
#[test]
fn export_errorlog_matches_oracle() {
    run_log_export("export_errorlog");
}

/// The `Export ErrorLog` **content** path (no oracle golden — cross-engine error
/// *message text* is not a Phase-8 axis): trigger a recoverable `DoSimpleMsg`
/// (an unknown property on an existing element), export the log, and assert the
/// message reaches the file. Guards against the dump silently dropping the
/// accumulated `Dss::errors` (the empty-golden alone can't catch that).
#[test]
fn export_errorlog_captures_errors() {
    let scratch = scratch_dir("errorlog_content");
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.elog basekv=12.47 bus1=src");
    dss.command("new line.l1 bus1=src bus2=b");
    // A recoverable error: an unknown property on an existing element
    // (`DoSimpleMsg` records + continues, growing `ErrorStrings`).
    dss.command("edit line.l1 bogusproperty=42");
    assert!(
        !dss.errors().is_empty(),
        "the bogus edit should have logged an error"
    );
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("export errorlog");

    let produced = dss.last_result_file();
    assert!(
        produced.to_lowercase().ends_with("elog_exp_errorlog.txt"),
        "unexpected produced path: {produced:?}"
    );
    let content = std::fs::read_to_string(produced)
        .unwrap_or_else(|e| panic!("read produced {produced}: {e}"));
    // The recorded `DoSimpleMsg` names the offending property — assert that exact
    // token reaches the file (a dropped/empty dump would fail this).
    assert!(
        content.to_lowercase().contains("bogusproperty"),
        "the exported ErrorLog must contain the recorded error; got:\n{content}"
    );
    std::fs::remove_dir_all(&scratch).ok();
}

// --- WP8.3 step 3c: the reliability + capacity exports -----------------------
// `Export BusReliability`/`BranchReliability` (Pascal `ExportBusReliability`/
// `ExportBranchReliability`) dump the per-bus / per-branch reliability indices a
// prior `RelCalc` populated; `Export Capacity` (`ExportCapacity` +
// `CalcAndWriteMaxCurrents`) dumps each PDElement's max phase current vs its
// rating + branch customer counts. No corpus deck exports these, so the fixture is
// synthesized (PHASE8_PLAN §1) as a self-contained deck: a two-section radial
// feeder + per-line fault data + a recloser (the OCP device `RelCalc` needs) +
// loads with `numcust` + an EnergyMeter, `solve`d then `relcalc`'d. High recloser
// pickups keep the snapshot from tripping so `Capacity` sees real currents. This
// is the `relcalc_head_recloser_matches_oracle` feeder — the reliability unit test
// proves both engines' `RelCalc` arithmetic agrees.

/// `Export BusReliability` (Pascal `ExportBusReliability`): per-bus Lambda/
/// Num-Interruptions/Num-Customers/Cust-Interruptions/Duration/Total-Miles. All
/// value columns are `%-.11g` (11 sig) computed by the `RelCalc` sweep — pure
/// arithmetic from identical line fault-data on both engines (the reliability unit
/// tests pin the same accumulators to ~1e-12), so `rel = 1e-8` is ~1e4× over the
/// 11-sig print floor; the integer Num-Customers column is exact within `abs`.
#[test]
fn export_busreliability_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 1e-8,
        abs: 1e-9,
        col_tol: vec![],
    };
    run_deck_export("export_busreliability", &policy);
}

/// `Export BranchReliability` (Pascal `ExportBranchReliability`): per-branch
/// Lambda/Accumulated-Lambda/customers/interrupts/durations/miles/Cust-Miles/SAIFI
/// (`%-.11g` + integer customer counts). Same arithmetic-identity floor as
/// BusReliability (`rel = 1e-8`, `abs = 1e-9`).
#[test]
fn export_branchreliability_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 1e-8,
        abs: 1e-9,
        col_tol: vec![],
    };
    run_deck_export("export_branchreliability", &policy);
}

/// `Export Capacity` (Pascal `ExportCapacity` + `CalcAndWriteMaxCurrents`):
/// per-PDElement `Imax`/`%normal`/`%emergency`/`kW`/`kvar`/customers/`NumPhases`/
/// `kVBase`. `Imax`/`kW`/`kvar` are solve-derived (`%10.6g`), so they take the
/// corpus-wide `EXPORT_REL` **two-independent-solves** (faer-vs-KLU) floor — well
/// above the ~1e-8 agreement on this tiny circuit, so a real scale/mapping
/// regression (wrong ×0.001, a dropped phase in the `Imax` max, a swapped kW/kvar)
/// fails loudly while the two solves never flake (`abs = 1e-6`). The `%normal`/
/// `%emergency` columns are `%8.2f` (2 dec → additive `abs = 0.011`); `kVBase` is
/// `%-.3g` (3 sig → `rel = 1e-3`). The integer customer/phase columns are exact
/// within `abs`.
#[test]
fn export_capacity_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: EXPORT_REL,
        abs: EXPORT_ABS,
        col_tol: vec![
            // %normal (2), %emergency (3): %8.2f additive last-digit floor.
            ColTol {
                sel: ColSel::Index(2),
                rel: 0.0,
                abs: 0.011,
                gate: None,
            },
            ColTol {
                sel: ColSel::Index(3),
                rel: 0.0,
                abs: 0.011,
                gate: None,
            },
            // kVBase (9): %-.3g 3-sig printing floor.
            ColTol {
                sel: ColSel::Index(9),
                rel: 1e-3,
                abs: EXPORT_ABS,
                gate: None,
            },
        ],
    };
    run_deck_export("export_capacity", &policy);
}

/// Split every non-empty CSV data line (after the header) into trimmed fields.
fn csv_rows(content: &str) -> Vec<Vec<String>> {
    content
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split(',').map(|f| f.trim().to_string()).collect())
        .collect()
}

/// The **degenerate no-`RelCalc`** path (audit-tests follow-up): `Export
/// BusReliability`/`BranchReliability` without a prior `RelCalc` must emit the
/// same all-zero reliability columns the oracle produces (the `Bus*`/`Branch*`
/// fields are their zero defaults). A Rust-only structural guard (no oracle — the
/// zeros are the default-branch by construction, like the step-3b `Ysc=None`
/// guard): the golden always runs `relcalc` first, so it never exercises this.
/// Guards against a regression that emitted garbage (or populated fields before
/// `RelCalc`) on the unrun path.
#[test]
fn export_reliability_without_relcalc_is_zeroed() {
    let scratch = scratch_dir("rel_norelcalc");
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.t basekv=12.47 bus1=src phases=3");
    dss.command("new line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1 faultrate=0.2");
    dss.command("new line.l2 bus1=b1 bus2=b2 length=1 units=mi r1=0.1 x1=0.1 faultrate=0.3");
    dss.command("new load.ld1 bus1=b1 phases=3 kv=12.47 kw=100 numcust=10");
    dss.command("new energymeter.m1 element=line.l1 terminal=1");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve mode=snap"); // NB: no `relcalc`
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));

    // BusReliability: every value column (Lambda, Num-Interruptions,
    // Cust-Interruptions, Duration, Total-Miles — cols 1,2,4,5,6) is 0; the
    // integer Num-Customers (col 3) is also 0 without RelCalc's customer roll-up.
    dss.command("export busreliability");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let bus = std::fs::read_to_string(dss.last_result_file()).unwrap();
    let bus_rows = csv_rows(&bus);
    assert!(
        bus_rows.len() >= 3,
        "expected >=3 bus rows, got {bus_rows:?}"
    );
    for row in &bus_rows {
        for (c, field) in row.iter().enumerate().skip(1) {
            assert_eq!(
                field.parse::<f64>().unwrap_or(f64::NAN),
                0.0,
                "BusReliability col {c} must be 0 without RelCalc, row {row:?}"
            );
        }
    }

    // BranchReliability: the RelCalc-computed columns — Lambda(1),
    // Accumulated-Lambda(2), Num-Interrupts(5), Cust-Interruptions(6),
    // Cust-Durations(7), Total-Miles(8), Cust-Miles(9), SAIFI(10) — are all 0
    // without RelCalc. The Num-/Total-Customers columns (3,4) are *not* checked:
    // the branch customer counts are populated by the meter-zone build at solve
    // time (existing Phase-6 behavior, pinned by the with-RelCalc golden), not by
    // RelCalc — so a nonzero count there is faithful, not garbage.
    dss.command("export branchreliability");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let br = std::fs::read_to_string(dss.last_result_file()).unwrap();
    let br_rows = csv_rows(&br);
    assert_eq!(br_rows.len(), 2, "expected 2 branch rows, got {br_rows:?}");
    for row in &br_rows {
        for (c, field) in row.iter().enumerate().skip(1) {
            if c == 3 || c == 4 {
                continue; // zone-build customer counts, not RelCalc-populated
            }
            assert_eq!(
                field.parse::<f64>().unwrap_or(f64::NAN),
                0.0,
                "BranchReliability col {c} must be 0 without RelCalc, row {row:?}"
            );
        }
    }

    std::fs::remove_dir_all(&scratch).ok();
}

/// The **enabled-only** PDElement filter (audit-tests follow-up): a disabled PD
/// element must not appear in `Export Capacity` / `BranchReliability` (Pascal's
/// `if pElem.Enabled` guard — `for_each_enabled_elem` for Capacity, the explicit
/// `cd.enabled` skip in `export_branch_reliability`). The shared oracle fixture
/// has no disabled element, so a regression dropping the filter would produce no
/// diff there; this Rust-only structural test pins it (no oracle needed — the
/// element's presence/absence is structural).
#[test]
fn export_capacity_reliability_skip_disabled_pd() {
    let scratch = scratch_dir("rel_disabled");
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.t basekv=12.47 bus1=src phases=3");
    dss.command("new line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1");
    // l2 disabled; b2 carries no load, so the solve stays well-posed.
    dss.command("new line.l2 bus1=b1 bus2=b2 length=1 units=mi r1=0.1 x1=0.1 enabled=no");
    dss.command("new load.ld1 bus1=b1 phases=3 kv=12.47 kw=100 numcust=10");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve mode=snap");
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));

    for keyword in ["capacity", "branchreliability"] {
        dss.command(&format!("export {keyword}"));
        assert!(dss.errors().is_empty(), "{keyword}: {:?}", dss.errors());
        let content = std::fs::read_to_string(dss.last_result_file()).unwrap();
        let names: Vec<String> = csv_rows(&content)
            .iter()
            .map(|r| r[0].to_lowercase())
            .collect();
        assert!(
            names.iter().any(|n| n.contains("l1")),
            "{keyword}: enabled Line.l1 must appear, got {names:?}"
        );
        assert!(
            names.iter().all(|n| !n.contains("l2")),
            "{keyword}: disabled Line.l2 must be filtered out, got {names:?}"
        );
    }

    std::fs::remove_dir_all(&scratch).ok();
}

// --- WP8.3 step 3c part 2: Overloads / Unserved / AllocationFactors ----------
// No corpus deck exports these, so each is a synthesized deck fixture
// (PHASE8_PLAN §1) tailored to make the report non-empty: an under-rated
// overloaded line (Overloads), a voltage-sagged feeder (Unserved), and two
// allocation-spec loads (AllocationFactors). `gen_phase8.py` captures the oracle
// output; the Rust golden replays the same deck.

/// `Export Overloads` (Pascal `ExportOverloads`): per-overloaded-PDElement I1/
/// AmpsOver/kVAOver/%Normal/%Emergency + symmetrical-component currents. Every
/// column is fixed-point — I1/AmpsOver/kVAOver `%…2f` (additive last-digit floor
/// `abs = 0.011`), the %-loading/sequence columns `%…1f` (`abs = 0.11`). On this
/// one-line circuit the two solves agree to ~1e-9, so the printing floor
/// dominates and a real regression (wrong over-amps, a dropped phase in `Cmax`,
/// a swapped %Normal/%Emergency) fails loudly. `rel = 0` (pure printing floor,
/// the `Powers`/`P_byphase` discipline — no masking).
#[test]
fn export_overloads_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.011,
        col_tol: (5..=10)
            .map(|i| ColTol {
                sel: ColSel::Index(i),
                rel: 0.0,
                abs: 0.11,
                gate: None,
            })
            .collect(),
    };
    run_deck_export("export_overloads", &policy);
}

/// `Export Unserved` (Pascal `ExportUnserved`): per-load Bus/kW/EEN_Factor/
/// UE_Factor for loads over the normal voltage-drop criterion. `kW` is the deck
/// constant (`%8.0f` → `abs = 0.5`); `EEN_Factor`/`UE_Factor` are solve-derived
/// (`%9.3f` → additive `abs = 0.0011`) — the two solves agree ~1e-8, so a real
/// regression (wrong criterion, a missing/extra load, a swapped EEN/UE) fails
/// loudly. `rel = 0` (printing floor).
#[test]
fn export_unserved_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0011,
        col_tol: vec![ColTol {
            // kW: %8.0f integer-rounding floor.
            sel: ColSel::Index(2),
            rel: 0.0,
            abs: 0.5,
            gate: None,
        }],
    };
    run_deck_export("export_unserved", &policy);
}

/// `Export AllocationFactors` (Pascal `DumpAllocationFactors`): one
/// `Load.<name>.AllocationFactor=<f>` / `.CFactor=<f>` line per allocation-spec
/// load — no header, `=`-separated. The factors are deck constants (`%-.5g`,
/// no solve dependency), so they match exactly (`rel = abs = 1e-9`). Guards the
/// spec-type dispatch (only ConnectedkVA/kWh loads emit) + the native-case name.
#[test]
fn export_allocationfactors_matches_oracle() {
    let policy = ExportPolicy {
        sep: '=',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 1e-9,
        abs: 1e-9,
        col_tol: vec![],
    };
    run_deck_export("export_allocationfactors", &policy);
}

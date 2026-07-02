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
use harness::{ColSel, ColTol, ExportPolicy, GateSpec, RowPolicy, compare_export};
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
/// `Some((I1_col, …))` for `SeqCurrents`.
fn pct_ratio_tol(abs: f64, gate: Option<GateSpec>) -> ColTol {
    ColTol {
        sel: ColSel::Prefix("%".to_string()),
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
        col_tol: vec![pct_ratio_tol(1e-9, None)],
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
        col_tol: vec![pct_ratio_tol(1e-8, Some(GateSpec::Col(2, 1e-6)))],
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

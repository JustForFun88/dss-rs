//! Phase 2 gate: property round-trip against the dss-python oracle.
//!
//! For each scenario under `tests/golden/props/` (one `<class>.json` per DSS
//! class, each holding that class's scenarios; the gate runs every file in the
//! directory), replay the identical command script through the Rust [`Dss`]
//! executive and check that every property reads back the same value the oracle
//! produced. Per PORTING_PLAN.md §4, numbers are compared with tolerance and the
//! surrounding structure exactly, never by raw float-string diffing.

use std::collections::BTreeMap;
use std::path::PathBuf;

use dss_core::exec::Dss;
use serde::Deserialize;

/// One per-class file: `{schema, oracle, class, scenarios}` (`oracle`/`class`
/// ignored here).
#[derive(Debug, Deserialize)]
struct PropsFile {
    schema: u32,
    scenarios: Vec<Scenario>,
}

#[derive(Debug, Deserialize)]
struct Scenario {
    name: String,
    commands: Vec<String>,
    target: String,
    /// Property name → oracle value string.
    properties: BTreeMap<String, String>,
    /// Some objects log a non-fatal error during their own RecalcElementData
    /// even when fully specified (e.g. a StorageController on a circuit with no
    /// Storage element always logs 37201 — faithfully reproduced). Those
    /// scenarios still pin the property dump; the error itself is covered by a
    /// dedicated unit test, so skip the "no engine errors" assertion here.
    #[serde(default)]
    allow_errors: bool,
}

/// How many `<class>.json` files the gate replays, how many scenarios they hold
/// in total, and how many property cells those scenarios pin — the **population
/// lock** of this gate (`GOLDEN_REBASE_PLAN.md` G0.2).
///
/// [`props_roundtrip_matches_oracle`] used to assert only that the flattened
/// scenario list was non-empty, so a class file (or a scenario, or a property
/// inside one) could leave the gate and take its coverage with it silently: 50
/// files still pass a non-emptiness check exactly as well as 51 do.
/// `golden_lock.rs` (G0.1) digests each artifact's committed content, so an
/// edit that drops a scenario or a property *does* red the lock as a `DIGEST
/// MOVED` — what it cannot see is the same drop made across a deliberate
/// `DSS_UPDATE_GOLDEN_LOCK=1` regen, where the digest moves with the bytes.
/// Nor does the lock say anything about the *driver*: what this file loads,
/// replays and actually compares is a separate question from what is committed.
/// These three constants tie the two together as equalities in both lanes, so
/// shrinking the gate — from the corpus side or the code side — is a reviewed
/// diff here rather than a silent loss.
///
/// They are counts, not a content fingerprint: the artifacts' bytes are pinned
/// by `golden.lock.json`, and their values by this file's comparators. Moving
/// any of them belongs in the commit that argues for the new population.
///
/// [`PROPS_PROPERTY_CELLS`] counts every `(scenario, property)` pair the driver
/// reaches, and is checked as `compared + lane_skips`, both counted at the
/// comparison site itself — an added `continue` that stopped comparing cells
/// would move `compared` without moving the corpus.
const PROPS_CLASS_FILES: usize = 51;
const PROPS_SCENARIOS: usize = 322;
const PROPS_PROPERTY_CELLS: usize = 8343;

/// The `props/` corpus as the gate sees it: the flattened scenarios plus the
/// number of class files they came from ([`PROPS_CLASS_FILES`]).
struct PropsCorpus {
    class_files: usize,
    scenarios: Vec<Scenario>,
}

/// Load every `*.json` class file from `tests/golden/props/` (sorted by file
/// name for deterministic order) and flatten their scenarios.
fn load_scenarios() -> PropsCorpus {
    let dir: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "golden",
        "props",
    ]
    .iter()
    .collect();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    let mut scenarios = Vec::new();
    for p in &files {
        let text = std::fs::read_to_string(p)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()));
        let f: PropsFile = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("cannot parse {}: {e}", p.display()));
        assert_eq!(f.schema, 1, "{}: props golden schema mismatch", p.display());
        assert!(
            !f.scenarios.is_empty(),
            "{}: props class file holds no scenarios",
            p.display()
        );
        scenarios.extend(f.scenarios);
    }
    PropsCorpus {
        class_files: files.len(),
        scenarios,
    }
}

/// Split a value string into its non-numeric "skeleton" (each number replaced
/// by `#`) and the list of numbers it contains.
fn numeric_skeleton(s: &str) -> (String, Vec<f64>) {
    let mut skeleton = String::new();
    let mut nums = Vec::new();
    let mut i = 0;
    while i < s.len() {
        if let Some((value, len)) = scan_number(&s[i..]) {
            nums.push(value);
            skeleton.push('#');
            i += len;
        } else {
            // values are ASCII; advance one byte
            skeleton.push(s.as_bytes()[i] as char);
            i += 1;
        }
    }
    (skeleton, nums)
}

/// Longest numeric prefix of `s` that parses as `f64` (and contains a digit).
fn scan_number(s: &str) -> Option<(f64, usize)> {
    let bytes = s.as_bytes();
    let first = bytes[0];
    if !(first.is_ascii_digit() || first == b'.' || first == b'+' || first == b'-') {
        return None;
    }
    let mut end = 0;
    while end < bytes.len() {
        let c = bytes[end];
        if c.is_ascii_digit() || matches!(c, b'.' | b'+' | b'-' | b'e' | b'E') {
            end += 1;
        } else {
            break;
        }
    }
    while end > 0 {
        let cand = &s[..end];
        if cand.bytes().any(|b| b.is_ascii_digit())
            && let Ok(v) = cand.parse::<f64>()
        {
            return Some((v, end));
        }
        end -= 1;
    }
    None
}

/// Stage F deliberate divergences: `(scenario, property)` pairs whose **value**
/// the *default* lane does not compare against the oracle, because its clean fix
/// intentionally reports a different one. The **parity** lane still compares
/// every pair, and both lanes still compare every *other* property of these
/// scenarios — the exclusion is value-only and per-property, never per-scenario.
///
/// * `isource_bus2_clobbered_by_bus1` / `Bus2` — the scenario exists to pin the
///   upstream quirk that `TIsourceObj.PropertySideEffects` (`Isource.pas:221`)
///   has no `Bus2` case, so `Bus2Defined` never latches and the `Bus1` side
///   effect re-derives `b1.0.0.0` over the explicit `b2` this deck wrote first.
///   `TVsourceObj.PropertySideEffects` (`Vsource.pas:498`) latches it on the
///   very same property; the default lane does too
///   (`compat::ISOURCE_BUS2_NEVER_LATCHES`), so it reports `b2`. Pinned in both
///   lanes by `elements::pc::isource::tests::bus2_latching_is_the_lane_kernel`.
const LANE_SKIP_SCENARIO_PROPS: &[(&str, &str)] = &[("isource_bus2_clobbered_by_bus1", "Bus2")];

/// An **oracle-bug** exclusion, as `(class, property)` pairs, applied in **both
/// lanes**: the `DoubleSymMatrixProperty` text getter. dss_capi's generic arm
/// addresses the *pointer field* as if it were the array
/// (`src/General/DSSObjectHelper.pas:2296-2313`), so it reads uninitialized
/// memory and prints ~0 whatever was stored — every one of these oracle values
/// is an all-zero matrix. The authority renders the stored values by hand
/// (r4133 `PDElements/Fault.pas:695-717`) or from the stored property text
/// (`General/DSSObject.pas:112-115`), and reading uninitialized memory is UB, so
/// the engine renders the stored matrix in both lanes and neither lane compares
/// these numbers against the capture.
///
/// The class half is load-bearing, not decoration. `DoubleSymMatrixProperty` is
/// declared by exactly three classes in the pinned backend — `Capacitor.pas`,
/// `Fault.pas`, `Reactor.pas` — but `RMatrix`/`XMatrix`/`CMatrix` are *also*
/// property names on `Line` and `LineCode`, where they are ordinary matrices
/// whose oracle values are real numbers. Keyed by name alone this list dropped
/// the value compare on 69 pairs instead of these 33, and corrupting a
/// `LineCode.RMatrix` golden number went undetected (reproduced, F-settle W4).
///
/// The exclusion is **values only**: [`assert_shape_matches`] still compares the
/// numeric *skeleton*, so the parenthesised `(v |v v |v v v )` shape, the matrix
/// order and the row split stay gated against the capture in both lanes. The
/// rendered numbers are pinned by `dss_core::exec::tests::compat_quirks::
/// sym_matrix_text_getter_renders_the_stored_matrix`.
const LANE_SKIP_PROP_VALUES: &[(&str, &str)] = &[
    ("Capacitor", "CMatrix"),
    ("Fault", "GMatrix"),
    ("Reactor", "RMatrix"),
    ("Reactor", "XMatrix"),
];

/// How many `(class, property)` cells [`LANE_SKIP_PROP_VALUES`] actually
/// removes from the value compare, measured over the committed goldens:
/// Capacitor `CMatrix` ×7, Fault `GMatrix` ×6, Reactor `RMatrix`/`XMatrix` ×20.
///
/// Asserted as an **equality in both lanes**, so the list is fail-on-stale in
/// both directions: an entry that stops matching shrinks it, and one that starts
/// matching something new grows it. The `>=` it replaced was satisfied by any
/// count at all.
const LANE_SKIP_PROP_VALUE_CELLS: usize = 33;

/// The value-free half of [`assert_value_matches`]: the rendered *shape* only —
/// the literal text around the numbers and how many numbers there are. Used in
/// the default lane for [`LANE_SKIP_PROP_VALUES`], where the values are a
/// deliberate divergence but the layout is not.
fn assert_shape_matches(actual: &str, expected: &str, ctx: &str) {
    let (askel, anums) = numeric_skeleton(actual);
    let (eskel, enums) = numeric_skeleton(expected);
    assert_eq!(
        askel, eskel,
        "{ctx}: structure differs (actual {actual:?} vs expected {expected:?})"
    );
    assert_eq!(
        anums.len(),
        enums.len(),
        "{ctx}: number count differs (actual {actual:?} vs expected {expected:?})"
    );
}

fn assert_value_matches(actual: &str, expected: &str, ctx: &str) {
    assert_shape_matches(actual, expected, ctx);
    let (_, anums) = numeric_skeleton(actual);
    let (_, enums) = numeric_skeleton(expected);
    for (i, (a, e)) in anums.iter().zip(&enums).enumerate() {
        let allowed = 1e-12 + 1e-9 * e.abs();
        assert!(
            (a - e).abs() <= allowed,
            "{ctx}: number {i} differs: actual {a} vs expected {e} \
             (from {actual:?} vs {expected:?})"
        );
    }
}

#[test]
fn props_roundtrip_matches_oracle() {
    let corpus = load_scenarios();
    let scenarios = &corpus.scenarios;
    assert!(!scenarios.is_empty(), "no scenarios in golden");

    // The population lock (see the constants): a class file or a scenario
    // cannot leave this gate without moving a number here.
    assert_eq!(
        corpus.class_files, PROPS_CLASS_FILES,
        "the props gate replays {} class files, {PROPS_CLASS_FILES} are locked. A file that \
         leaves takes its whole class's property coverage with it — move this number only in the \
         commit that argues for the new population.",
        corpus.class_files
    );
    assert_eq!(
        scenarios.len(),
        PROPS_SCENARIOS,
        "the props gate replays {} scenarios, {PROPS_SCENARIOS} are locked. A scenario dropped \
         inside a class file survives a deliberate golden_lock regen (the digest moves with the \
         bytes) — move this number only in the commit that argues for the new population.",
        scenarios.len()
    );

    // Stale-entry guard: every exclusion below must still name a real
    // `(scenario, property)` pair, so a renamed or deleted scenario fails the
    // gate instead of silently widening it.
    for (scenario, prop) in LANE_SKIP_SCENARIO_PROPS {
        let sc = scenarios
            .iter()
            .find(|s| s.name == *scenario)
            .unwrap_or_else(|| panic!("stale lane exclusion: no scenario {scenario:?}"));
        assert!(
            sc.properties.keys().any(|k| k.eq_ignore_ascii_case(prop)),
            "stale lane exclusion: scenario {scenario:?} has no property {prop:?}"
        );
    }

    let mut value_skips = 0usize;
    // The cell half of the population lock, counted where the comparison
    // happens: `compared` is every cell this run actually checked (by value or,
    // for the sym-matrix exclusion, by shape), `lane_skips` every cell the
    // default lane deliberately does not compare.
    let mut compared = 0usize;
    let mut lane_skips = 0usize;
    for sc in scenarios {
        let mut dss = Dss::new();
        // gen_props.py runs this preamble before every scenario (the `?`
        // query is circuit-gated in ProcessCommand, so the oracle needed a
        // circuit too).
        dss.command("clear");
        dss.command("new circuit.propsprobe");
        for cmd in &sc.commands {
            dss.command(cmd);
        }
        assert!(
            sc.allow_errors || dss.errors().is_empty(),
            "scenario {}: unexpected engine errors: {:?}",
            sc.name,
            dss.errors()
        );

        for (prop, expected) in &sc.properties {
            if !dss_core::compat::ORACLE_PARITY
                && LANE_SKIP_SCENARIO_PROPS
                    .iter()
                    .any(|(s, p)| *s == sc.name && p.eq_ignore_ascii_case(prop))
            {
                lane_skips += 1;
                continue;
            }
            dss.command(&format!("? {}.{}", sc.target, prop));
            let actual = dss.result().to_string();
            let ctx = format!("scenario {} property {prop}", sc.name);
            let target_class = sc.target.split('.').next().unwrap_or(&sc.target);
            if LANE_SKIP_PROP_VALUES
                .iter()
                .any(|(c, p)| c.eq_ignore_ascii_case(target_class) && p.eq_ignore_ascii_case(prop))
            {
                assert_shape_matches(&actual, expected, &ctx);
                value_skips += 1;
                compared += 1;
                continue;
            }
            assert_value_matches(&actual, expected, &ctx);
            compared += 1;
        }
    }

    // Fail-on-stale, both directions and both lanes: the exclusion must remove
    // exactly the measured set of cells, so neither a rename that makes an entry
    // inert nor a widening that swallows a class it was never meant to cover can
    // pass.
    // The cell half of the population lock, both directions: every pinned
    // property cell was reached, and every cell the default lane drops is one
    // the exclusion register names.
    assert_eq!(
        compared + lane_skips,
        PROPS_PROPERTY_CELLS,
        "the props gate reached {} property cells ({compared} compared + {lane_skips} lane-\
         skipped), {PROPS_PROPERTY_CELLS} are locked. Cells leave this gate either with their \
         golden or through a code path that stops comparing them; both are a reviewed diff.",
        compared + lane_skips
    );
    assert_eq!(
        lane_skips,
        if dss_core::compat::ORACLE_PARITY {
            0
        } else {
            LANE_SKIP_SCENARIO_PROPS.len()
        },
        "the default lane skipped {lane_skips} scenario-property cells; \
         LANE_SKIP_SCENARIO_PROPS names {} (and the parity lane must skip none)",
        LANE_SKIP_SCENARIO_PROPS.len()
    );

    assert_eq!(
        value_skips, LANE_SKIP_PROP_VALUE_CELLS,
        "the sym-matrix value exclusion moved: {value_skips} cells skipped, \
         {LANE_SKIP_PROP_VALUE_CELLS} recorded. Every skipped cell is a value \
         the oracle no longer checks, so this number only moves in the commit \
         that argues for the new set."
    );
}

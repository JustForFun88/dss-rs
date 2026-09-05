//! **GOLDEN_REBASE G1.6b — the `PDElements` surface's gate-level pins and the
//! read-order contract its two oracle captures must obey.**
//!
//! # The pins
//!
//! `harness::PD_SKIP_FIELDS` stops the value comparison of four
//! `(channel, class, field)` cells per oracle channel — and only on the elements
//! the defect reaches, an **in-zone shunt** Capacitor/Reactor
//! (`harness::pd_skip_applies`) — because both oracles read them out of
//! **uninitialized memory**: `TEnergyMeter.MakeMeterZoneLists` files
//! shunt Capacitors and Reactors on the **PC** adjacency list and then writes
//! through a `TPCElement` cursor that is really pointing at a `TPDElement`
//! (r4133 `Version8/Source/Meters/EnergyMeter.pas:1868-1869`, capi
//! `.inputs/dss_capi/src/Meters/EnergyMeter.pas:1927-1929`; the two class
//! layouts differ, so a different pair of doubles is hit on each channel).
//! CLAUDE.md's discipline is that such an exclusion is pinned by its own
//! expected-value test naming **both** numbers, or it is a mask over nothing —
//! and here the oracle half of "both numbers" is a heap pointer that changes on
//! every run, so an envelope is impossible and only a pin can hold the port's
//! value. The table's rows name
//! [`pd_elements_shunt_reliability_inputs_survive_the_meter_zone`] and
//! [`pd_elements_shunt_branch_flt_rate_survives_the_meter_zone`]; both live
//! below.
//!
//! Each pin runs on the gated corpus deck of **its own** channel, and both are
//! `controls:combo` siblings that carry three in-zone shunt capacitors and an
//! in-zone shunt reactor — every class of the table at once:
//! `midi_controls.dss` (`engines: "capi_v0145"`) for the four capi rows and
//! `midi_protection.dss` (`engines: "r4133"` — capi 0.14.5 cannot parse its
//! `Fuse.curvemultiplier`) for the four r4133 rows. The oracle garbage quoted
//! at each pin was measured on that same deck and that same channel (part R's
//! corpus-wide scan).
//!
//! [`pd_elements_walk_on_the_gated_combo_deck`] is the third pin: the walk's
//! membership, parent linkage, 1-based `FromTerminal` and all-zero `RelCalc`
//! fields — the claims `exec::tests::pd_elements` pins on IEEE 123, restated on
//! a deck the live gate actually walks (and on a circuit that, unlike IEEE 123,
//! has branches entered through terminal **2**).
//!
//! # The read-order contract
//!
//! `PDElements.ParentPDElement` reassigns `ActiveCktElement` to the parent and
//! never restores it (capi `CAPI/CAPI_PDElements.pas:245-257`, r4133
//! `Version8/Source/DDLL/DPDELements.pas:88-97`), so **every** field read after
//! it in the same record returns the parent's value. fastdss reads it second in
//! `IPDElements._columns` order and contaminates 215 cells of the 138-element
//! IEEE 123 walk — identically on both channels (measured, G1.6b part R). Both
//! captures therefore read it last and only then read the parent's name off the
//! hijacked cursor.
//!
//! Nothing in the type system can enforce that: one transport is Python
//! (`tools/oracle/oracle_server.py`) and the other a `libloading` bridge
//! (`crates/dss-epri/src/capture.rs`), and a contaminated capture would still
//! *look* fine — it would simply compare the port against the parent's numbers.
//! The four static tests below therefore read both capture sources and assert
//! the read sequence and the call-site slot literally, in the
//! `oracle_parity_cfg_gate.rs` citation-walk idiom;
//! [`the_read_order_guard_rejects_a_contaminated_capture`] proves the scan is not
//! vacuous by re-running it over a deliberately corrupted copy of each body.
//! The r4133 side additionally has a *behavioural* proof in
//! `crates/dss-epri/tests/modes.rs` (the DDLL is a process-global singleton, so
//! it cannot live here), and a contaminated capi capture would red hundreds of
//! cells of the live gate on its first run.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use dss_core::exec::Dss;

// ---------------------------------------------------------------------------
// Plumbing
// ---------------------------------------------------------------------------

/// The repository root — the worktree that holds `crates/`, `tools/` and
/// `tests/`.
fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

/// Read one repository-relative source file, failing with its path.
fn read_source(rel: &str) -> String {
    let path = repo_root().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Every regular file under `dir`, recursively, with its byte length.
fn walk_dir(dir: &Path, out: &mut BTreeMap<PathBuf, u64>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let path = e.path();
        match e.file_type() {
            Ok(t) if t.is_dir() => walk_dir(&path, out),
            Ok(t) if t.is_file() => {
                out.insert(path, e.metadata().map(|m| m.len()).unwrap_or_default());
            }
            _ => {}
        }
    }
}

/// Keeps the vendored deck directory exactly as it was found — the miniature of
/// `corpus_gate/runner.rs`'s `CorpusGuard`. The two decks this file compiles are
/// definition-only today (measured: compile + solve + `RelCalc` leaves
/// `git status -- tests/corpus` clean), so the guard is insurance against a
/// later edit that adds an `Export`/`Show`, not a live need; the pins never
/// nest a guard inside another, so no shared-snapshot machinery is needed here
/// (unlike `props_r4133_pins.rs`).
struct DeckDirGuard {
    dir: PathBuf,
    before: BTreeMap<PathBuf, u64>,
}

impl DeckDirGuard {
    fn new(dir: &Path) -> Self {
        let mut before = BTreeMap::new();
        walk_dir(dir, &mut before);
        Self {
            dir: dir.to_path_buf(),
            before,
        }
    }
}

impl Drop for DeckDirGuard {
    fn drop(&mut self) {
        let mut after = BTreeMap::new();
        walk_dir(&self.dir, &mut after);
        let mut changed = Vec::new();
        for (path, len) in &after {
            match self.before.get(path) {
                None => {
                    let _ = std::fs::remove_file(path);
                }
                Some(was) if was != len => changed.push(path.display().to_string()),
                Some(_) => {}
            }
        }
        // Never mask the real failure by panicking while another panic unwinds.
        if !std::thread::panicking() {
            assert!(
                changed.is_empty(),
                "a pinned deck rewrote vendored corpus bytes (not merely added \
                 output files), which this guard cannot restore: {changed:?}"
            );
        }
    }
}

/// Compile and solve one vendored corpus deck (path relative to
/// `tests/corpus/`), requiring a clean run.
fn deck(rel: &str) -> (Dss, DeckDirGuard) {
    let path = repo_root().join("tests").join("corpus").join(rel);
    assert!(
        path.is_file(),
        "vendored deck is missing: {}",
        path.display()
    );
    let guard = DeckDirGuard::new(path.parent().expect("a deck has a directory"));
    let mut dss = Dss::new();
    dss.command(&format!("compile \"{}\"", path.display()));
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "{rel} must compile and solve clean: {:?}",
        dss.errors()
    );
    (dss, guard)
}

/// `controls:combo/midi_controls.dss` — the gated case of the **`capi_v0145`**
/// channel (`engines=capi_v0145` in `population.lock.json`): a 15-bus backbone
/// with an `EnergyMeter` on `Transformer.sub`, three in-zone shunt capacitors
/// (`cpin8`, `cmat`, `cdel`), one in-zone 1-phase shunt reactor (`rsh`) and one
/// **series** reactor (`rser`) — every class `harness::PD_SKIP_FIELDS` names,
/// plus a series member of a named class.
fn midi_controls() -> (Dss, DeckDirGuard) {
    deck("controls/combo/midi_controls.dss")
}

/// `controls:combo/midi_protection.dss` — the same topology with the protection
/// layer, and the gated case of the **`r4133`** channel (`engines=r4133`: capi
/// 0.14.5 aborts on its `Fuse.curvemultiplier`, DSS error 110). Its four shunt
/// rows are the ones the r4133 skip rows exclude, and unlike its sibling it
/// defines a `Fuse`, so a `RelCalc` here completes instead of aborting.
fn midi_protection() -> (Dss, DeckDirGuard) {
    deck("controls/combo/midi_protection.dss")
}

/// One row of a `Dss::pd_elements()` walk by full name (the port renders
/// `Class.name` with the class capitalized and the object name lowercased).
///
/// A macro rather than a function so the borrow of `$walk` stays in the
/// caller's scope: a `fn(&[PdElementView], &str) -> &PdElementView` would work
/// too (the type is public since `exec/mod.rs:82`), but every call site here
/// holds the walk in a local and reads several rows out of it, which is exactly
/// what `harness/mod.rs:5009`'s `compare_pd_elements` does with its own.
macro_rules! row {
    ($walk:expr, $name:expr) => {{
        let name: &str = $name;
        $walk
            .iter()
            .find(|v| v.name == name)
            .unwrap_or_else(|| panic!("{name} is not in the PDElements walk"))
    }};
}

// ---------------------------------------------------------------------------
// The read-order contract, asserted over both capture sources
// ---------------------------------------------------------------------------

/// `tools/oracle/oracle_server.py::capture_pd_elements`, docstring and `#`
/// comments removed.
fn capi_capture_body() -> String {
    let src = read_source("tools/oracle/oracle_server.py");
    let start = src
        .find("def capture_pd_elements(ckt) -> list:")
        .expect("oracle_server.py has no capture_pd_elements");
    let body = &src[start..];
    let end = body
        .find("\n    return out")
        .expect("capture_pd_elements must end in `return out`");
    let body = &body[..end];
    // Drop the docstring: it quotes `pde.ParentPDElement`'s Pascal arms.
    let body = match (body.find("\"\"\""), body.rfind("\"\"\"")) {
        (Some(a), Some(b)) if b > a => format!("{}{}", &body[..a], &body[b + 3..]),
        _ => body.to_string(),
    };
    strip_line_comments(&body, "#")
}

/// `crates/dss-epri/src/capture.rs::capture_pd_elements`, `//` comments removed
/// (its `///` doc block sits above the signature and is never in the slice).
fn r4133_capture_body() -> String {
    let src = read_source("crates/dss-epri/src/capture.rs");
    let start = src
        .find("pub fn capture_pd_elements(engine: &Engine)")
        .expect("capture.rs has no capture_pd_elements");
    let body = &src[start..];
    let end = body
        .find("\n}")
        .expect("capture_pd_elements must close at column 0");
    strip_line_comments(&body[..end], "//")
}

/// `crates/dss-epri/src/capture.rs::run_case`'s body.
fn r4133_run_case_body() -> String {
    let src = read_source("crates/dss-epri/src/capture.rs");
    let start = src
        .find("pub fn run_case(engine: &Engine, req: &RunRequest)")
        .expect("capture.rs has no run_case");
    let body = &src[start..];
    let end = body.find("\n}").expect("run_case must close at column 0");
    strip_line_comments(&body[..end], "//")
}

fn strip_line_comments(src: &str, marker: &str) -> String {
    src.lines()
        .map(|l| match l.find(marker) {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every read the capi capture issues, in source order (Python evaluates a dict
/// literal's values left to right, so source order **is** read order).
fn capi_read_sequence(body: &str) -> Vec<String> {
    let mut hits: Vec<(usize, String)> = Vec::new();
    for (i, _) in body.match_indices("pde.") {
        let ident = leading_ident(&body[i + "pde.".len()..]);
        if !ident.is_empty() {
            hits.push((i, format!("pde.{ident}")));
        }
    }
    for (i, _) in body.match_indices("ckt.ActiveCktElement.Name") {
        hits.push((i, "ckt.ActiveCktElement.Name".to_string()));
    }
    hits.sort_by_key(|(i, _)| *i);
    hits.into_iter().map(|(_, s)| s).collect()
}

/// Every typed mode accessor the r4133 capture calls, in source order.
fn r4133_read_sequence(body: &str) -> Vec<String> {
    body.match_indices("engine.pd_elements_")
        .map(|(i, _)| leading_ident(&body[i + "engine.".len()..]).to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn leading_ident(s: &str) -> &str {
    let end = s
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(s.len());
    &s[..end]
}

/// The capi capture's read sequence, `First`/`Next` included.
const CAPI_RECORD_ORDER: &[&str] = &[
    "pde.First",
    "pde.Name",
    "pde.AccumulatedL",
    "pde.FromTerminal",
    "pde.IsShunt",
    "pde.Numcustomers",
    "pde.SectionID",
    "pde.FaultRate",
    "pde.RepairTime",
    "pde.TotalMiles",
    "pde.Totalcustomers",
    "pde.pctPermanent",
    "pde.Lambda",
    "pde.ParentPDElement",
    "ckt.ActiveCktElement.Name",
    "pde.Next",
];

/// The r4133 capture's read sequence. `pd_elements_name` appears twice: once for
/// the record's own name and once, conditionally, for the parent's — the second
/// one **after** `pd_elements_parent_pd_element`, which is what turns the
/// hijacked cursor from a hazard into the `parent_name` field.
const R4133_RECORD_ORDER: &[&str] = &[
    "pd_elements_first",
    "pd_elements_name",
    "pd_elements_accumulated_l",
    "pd_elements_from_terminal",
    "pd_elements_is_shunt",
    "pd_elements_num_customers",
    "pd_elements_section_id",
    "pd_elements_fault_rate",
    "pd_elements_repair_time",
    "pd_elements_total_miles",
    "pd_elements_total_customers",
    "pd_elements_pct_permanent",
    "pd_elements_lambda",
    "pd_elements_parent_pd_element",
    "pd_elements_name",
    "pd_elements_next",
];

fn check_read_order(actual: &[String], expected: &[&str], who: &str) -> Result<(), String> {
    if actual.len() == expected.len() && actual.iter().zip(expected).all(|(a, b)| a == b) {
        return Ok(());
    }
    Err(format!(
        "{who}: the PDElements capture's read order changed.\n  expected: \
         {expected:?}\n  found:    {actual:?}\n\
         `ParentPDElement` must be the LAST field read of a record, with only \
         the parent-name read after it. Both oracles do \
         `ActiveCktElement := elem.ParentPDElement` and never restore it (capi \
         `CAPI/CAPI_PDElements.pas:245-257`, r4133 \
         `Version8/Source/DDLL/DPDELements.pas:88-97`), so every field read \
         after it returns the PARENT's value: measured, 215 cells of the \
         138-element IEEE 123 walk move, identically on both channels (e.g. \
         `Line.l1.Totalcustomers` 1 -> 91)."
    ))
}

/// The capi transport reads `ParentPDElement` last, and only the parent-name
/// read follows it.
#[test]
fn the_capi_pd_capture_reads_parentpdelement_last() {
    let seq = capi_read_sequence(&capi_capture_body());
    if let Err(msg) = check_read_order(&seq, CAPI_RECORD_ORDER, "tools/oracle/oracle_server.py") {
        panic!("{msg}");
    }
}

/// The r4133 transport reads the same fields in the same order, so a divergence
/// between the two channels can never be a capture-order artefact.
#[test]
fn the_r4133_pd_capture_reads_parentpdelement_last() {
    let seq = r4133_read_sequence(&r4133_capture_body());
    if let Err(msg) = check_read_order(&seq, R4133_RECORD_ORDER, "crates/dss-epri/src/capture.rs") {
        panic!("{msg}");
    }
}

/// Assert `anchors` occur exactly once each, in this order, in `body`.
fn assert_call_order(body: &str, anchors: &[&str], who: &str) {
    let mut last = 0usize;
    for (n, anchor) in anchors.iter().enumerate() {
        let hits: Vec<usize> = body.match_indices(anchor).map(|(i, _)| i).collect();
        assert_eq!(
            hits.len(),
            1,
            "{who}: `{anchor}` must appear exactly once in the capture slot, \
             found {} occurrence(s)",
            hits.len()
        );
        assert!(
            n == 0 || hits[0] > last,
            "{who}: `{anchor}` must come after `{}`. The PDElements walk is an \
             ACTIVE-ELEMENT mutator (`First`/`Next`/`ParentPDElement`), so its \
             slot — after the meters, before the probes — is the contract that \
             keeps the two transports comparable and keeps the walk from \
             perturbing another capture.",
            anchors[n - 1]
        );
        last = hits[0];
    }
}

/// The capi capture runs after the meters and before the probes.
#[test]
fn the_capi_pd_capture_runs_between_the_meters_and_the_probes() {
    assert_call_order(
        &read_source("tools/oracle/oracle_server.py"),
        &[
            "\"meters\": capture_all_meters(",
            "\"pd_elements\": capture_pd_elements(",
            "\"probes\": capture_probes(",
        ],
        "tools/oracle/oracle_server.py::run_case",
    );
}

/// …and so does the r4133 capture, in the identical slot.
#[test]
fn the_r4133_pd_capture_runs_between_the_meters_and_the_probes() {
    assert_call_order(
        &r4133_run_case_body(),
        &[
            "capture_meters(engine)",
            "capture_pd_elements(engine)",
            "capture_probes(engine",
        ],
        "crates/dss-epri/src/capture.rs::run_case",
    );
}

/// **The static guard is not vacuous.** Both scans are re-run over corrupted
/// copies of the *real* bodies — the fastdss corruption itself,
/// `ParentPDElement` hoisted into an earlier column, and a dropped parent-name
/// read — and must reject them. A scan that silently matched nothing would pass
/// the four tests above and prove nothing.
#[test]
fn the_read_order_guard_rejects_a_contaminated_capture() {
    // fastdss reads the parent second; hoisting it over `Lambda` is the same
    // class of mistake and the smallest textual one.
    let capi_bad = capi_capture_body().replacen("pde.Lambda", "pde.ParentPDElement", 1);
    let seq = capi_read_sequence(&capi_bad);
    assert!(
        check_read_order(&seq, CAPI_RECORD_ORDER, "probe").is_err(),
        "the capi scan accepted a capture that reads ParentPDElement early: {seq:?}"
    );

    let r4133_bad =
        r4133_capture_body().replacen("pd_elements_lambda", "pd_elements_parent_pd_element", 1);
    let seq = r4133_read_sequence(&r4133_bad);
    assert!(
        check_read_order(&seq, R4133_RECORD_ORDER, "probe").is_err(),
        "the r4133 scan accepted a capture that reads ParentPDElement early: {seq:?}"
    );

    // A dropped parent-name read is the other half of the contract.
    let capi_bad = capi_capture_body().replacen("ckt.ActiveCktElement.Name", "\"\"", 1);
    assert!(
        check_read_order(&capi_read_sequence(&capi_bad), CAPI_RECORD_ORDER, "probe").is_err(),
        "the capi scan accepted a capture that never reads the parent's name"
    );
    let r4133_bad = r4133_capture_body().replacen("pd_elements_name", "pd_elements_first", 2);
    assert!(
        check_read_order(
            &r4133_read_sequence(&r4133_bad),
            R4133_RECORD_ORDER,
            "probe"
        )
        .is_err(),
        "the r4133 scan accepted a capture that never reads the parent's name"
    );

    // And the slot scan reads exactly the one call it claims to order.
    let moved = r4133_run_case_body().replacen("capture_pd_elements(engine)", "", 1);
    assert_eq!(
        moved.match_indices("capture_pd_elements(engine)").count(),
        0,
        "the run_case slice must contain exactly the one call the slot test reads"
    );
}

// ---------------------------------------------------------------------------
// The pins
// ---------------------------------------------------------------------------

/// **Pin (G1.6b-1) — `PD_SKIP_FIELDS`' four `capi_v0145` rows.**
///
/// `FaultRate` and `pctPermanent` are the *stored* `TPDElement` reliability
/// **inputs**; on a shunt Capacitor or Reactor inside an EnergyMeter zone the
/// pinned capi oracle reads them out of uninitialized memory, because
/// `TEnergyMeter.MakeMeterZoneLists` files shunt PD elements on the **PC**
/// adjacency list and then writes `SensorObj`/`MeterObj` through a
/// `TPCElement` cursor aimed at a `TPDElement`
/// (`.inputs/dss_capi/src/Meters/EnergyMeter.pas:1927-1929`, decl `:1783`;
/// r4133's identical code at `Version8/Source/Meters/EnergyMeter.pas:1868-1869`
/// hits a different pair of doubles because the class layouts differ — hence
/// the per-channel rows).
///
/// **Both numbers.** The port reports the class defaults it parsed —
/// Capacitor `FaultRate` **0.0005** / `PctPerm` **100.0** / `HrsToRepair`
/// **3.0** (`PDElements/Capacitor.pas:555-557`), Reactor the same
/// (`PDElements/Reactor.pas:601-603`) — where the capi oracle returned, on this
/// very deck, `6.54067550755e-312` (`FaultRate`) and `6.54066734598e-312`
/// (`pctPermanent`) on `Capacitor.cpin8`, `Capacitor.cmat`, `Capacitor.cdel`
/// and `Reactor.rsh` alike; on IEEE 123's `Capacitor.c83` the same cell came
/// back `1.043284808626e-311`, `8.210645577926e-312` and
/// `1.511547243828e-311` in three separate processes — a heap pointer under
/// ASLR, which is why the cells are excluded rather than enveloped.
///
/// The discriminating half is the edit: the same accessor answers a deck's own
/// numbers, so the assertion above is about the parsed value and not about a
/// hardwired constant.
#[test]
fn pd_elements_shunt_reliability_inputs_survive_the_meter_zone() {
    let (mut dss, _guard) = midi_controls();
    let walk = dss.pd_elements();
    for name in [
        "Capacitor.cpin8",
        "Capacitor.cmat",
        "Capacitor.cdel",
        "Reactor.rsh",
    ] {
        let v = row!(walk, name);
        assert!(v.is_shunt, "{name} must be the shunt member the bug needs");
        assert_eq!(v.fault_rate, 0.0005, "{name}.fault_rate");
        assert_eq!(v.pct_permanent, 100.0, "{name}.pct_permanent");
        assert_eq!(v.repair_time, 3.0, "{name}.repair_time");
    }
    // The series Reactor of the same deck reads clean on both oracles, and it
    // is fully compared there: `harness::pd_skip_applies` scopes the rows to an
    // in-zone SHUNT element, which is where `MakeMeterZoneLists` writes. Both
    // members carry the same parsed defaults, so this pin holds the value for
    // the excluded and the compared one alike.
    let rser = row!(walk, "Reactor.rser");
    assert!(!rser.is_shunt);
    assert_eq!(rser.fault_rate, 0.0005);
    assert_eq!(rser.pct_permanent, 100.0);

    // Discriminating half: the getter is live, not a frozen default.
    dss.command("edit Capacitor.cmat faultrate=0.02 pctperm=55 repair=7");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let edited = dss.pd_elements();
    let v = row!(edited, "Capacitor.cmat");
    assert_eq!(v.fault_rate, 0.02);
    assert_eq!(v.pct_permanent, 55.0);
    assert_eq!(v.repair_time, 7.0);
}

/// **Pin (G1.6b-2) — `PD_SKIP_FIELDS`' four `r4133` rows.**
///
/// `Lambda` (`BranchFltRate`) and `AccumulatedL` (`AccumulatedBrFltRate`) are
/// the reliability sweep's **accumulators**, written only by `CalcFltRate` /
/// `AccumFltRate` inside `CalcReliabilityIndices`
/// (r4133 `Version8/Source/Meters/EnergyMeter.pas:2474-2482`; port side
/// `solution/meters/reliability.rs:113-115,180`). No live corpus deck runs
/// `RelCalc`, so the correct value everywhere on this surface today is **0.0** —
/// and it stays 0.0 on the shunt rows even after a `RelCalc`, because a shunt PD
/// element is filed on the meter's PC list and never enters the branch sequence
/// the sweep walks.
///
/// **Both numbers.** The port reports `lambda` = `accumulated_l` = **0.0** where
/// the r4133 DLL returned, on this deck, `7.857816697254e-312` for **both**
/// fields on all four shunt rows (`Capacitor.cpin8`, `Capacitor.cmat`,
/// `Capacitor.cdel`, `Reactor.rsh`); on the sibling `midi_controls.dss` the same
/// cells read `7.85781695385e-312` / `7.857781098757e-312`, and on an IEEE 13
/// fixture `7.20016348108e-312` then `7.20016742902e-312` **inside one worker
/// process** — so no envelope exists even within a single run.
///
/// The discriminating half is a real `RelCalc`: this deck defines a `Fuse`, so
/// `CalcReliabilityIndices` completes and fills `lambda`, `accumulated_l`,
/// `total_miles` **and** `section_id` on the series branches (asserted below),
/// which proves the four zeros above are live state and not getters that always
/// answer 0. The shunt rows stay 0 even then, because a shunt PD element is on
/// the meter's PC list and never enters `SequenceList`. This pin is the
/// port-side half; the oracle-compared half is **discharged** — G1.6(i) makes
/// the gate drive `RelCalc` itself on the cases that carry
/// `compare_reliability: true`, so the four fields are live and compared on
/// both channels (`tests/reliability_pins.rs::`
/// `pd_elements_relcalc_fields_are_live_after_relcalc`).
#[test]
fn pd_elements_shunt_branch_flt_rate_survives_the_meter_zone() {
    let (mut dss, _guard) = midi_protection();
    let walk = dss.pd_elements();
    for name in [
        "Capacitor.cpin8",
        "Capacitor.cmat",
        "Capacitor.cdel",
        "Reactor.rsh",
    ] {
        let v = row!(walk, name);
        assert!(v.is_shunt, "{name} must be the shunt member the bug needs");
        assert_eq!(v.lambda, 0.0, "{name}.lambda");
        assert_eq!(v.accumulated_l, 0.0, "{name}.accumulated_l");
    }

    // Discriminating half: a complete `RelCalc` (this deck has a `Fuse`, so
    // `SectionCount != 0` and `CalcReliabilityIndices` does not take the
    // 52902 exit at r4133 `EnergyMeter.pas:2500-2504`) fills all four fields on
    // the series branches.
    dss.command("RelCalc");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let after = dss.pd_elements();
    let head = row!(after, "Line.bb1_2");
    assert_eq!(head.lambda, 0.04);
    assert_eq!(head.accumulated_l, 1.0605);
    assert_eq!(head.total_miles, 14.204545454545457);
    assert_eq!(head.section_id, 1);
    // …and the shunt rows stay 0: they are on the PC list, never in the branch
    // sequence the sweep accumulates over.
    for name in [
        "Capacitor.cpin8",
        "Capacitor.cmat",
        "Capacitor.cdel",
        "Reactor.rsh",
    ] {
        let v = row!(after, name);
        assert_eq!(v.lambda, 0.0, "{name}.lambda after RelCalc");
        assert_eq!(v.accumulated_l, 0.0, "{name}.accumulated_l after RelCalc");
        assert_eq!(v.total_miles, 0.0, "{name}.total_miles after RelCalc");
        assert_eq!(v.section_id, 0, "{name}.section_id after RelCalc");
    }
}

/// **Pin (G1.6b-3) — the gated deck's walk, parent linkage and `RelCalc`
/// fields.**
///
/// `exec::tests::pd_elements` pins membership/order, the 1-based `FromTerminal`
/// and the parent linkage on IEEE 123 and on an in-file synthetic feeder; this
/// restates them on the deck the live gate actually walks, which adds two things
/// IEEE 123 cannot: thirteen branches the zone build entered through terminal
/// **2** (IEEE 123 reports 1 on all 138 rows), and a mixed four-class walk
/// (Line / Transformer / Capacitor / Reactor).
///
/// Measured: **42** enabled PD elements, opening
/// `Transformer.sub, Line.bb1_2, Line.bb2_3, Line.bb3_4, Line.bb4_5`; exactly
/// four shunt rows; `Line.l1b`'s parent is `Line.l1a` at `ClassIndex` **16**
/// (1-based, per class — `General/DSSObject.pas:43`); every shunt row is
/// parentless (`0` / `""`), because a shunt element is filed on the meter's PC
/// list; and all four `RelCalc`-fed fields are **0** on all 42 rows.
#[test]
fn pd_elements_walk_on_the_gated_combo_deck() {
    let (dss, _guard) = midi_controls();
    let walk = dss.pd_elements();
    assert_eq!(walk.len(), 42);
    let names: Vec<&str> = walk.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(
        &names[..5],
        &[
            "Transformer.sub",
            "Line.bb1_2",
            "Line.bb2_3",
            "Line.bb3_4",
            "Line.bb4_5"
        ]
    );

    // 1-based `FromTerminal`, with terminal-2 entries present.
    assert!(
        walk.iter()
            .all(|v| v.from_terminal == 1 || v.from_terminal == 2)
    );
    assert_eq!(walk.iter().filter(|v| v.from_terminal == 2).count(), 13);
    assert_eq!(row!(walk, "Line.l6a").from_terminal, 2);
    assert_eq!(row!(walk, "Line.l1b").from_terminal, 1);

    // Parent linkage: an interior branch, the meter's head element, the shunts.
    let l1b = row!(walk, "Line.l1b");
    assert_eq!(l1b.parent_name, "Line.l1a");
    assert_eq!(l1b.parent_class_index, 16);
    let sub = row!(walk, "Transformer.sub");
    assert_eq!(sub.parent_class_index, 0);
    assert_eq!(sub.parent_name, "");
    for v in walk.iter().filter(|v| v.is_shunt) {
        assert_eq!(v.parent_class_index, 0, "{}", v.name);
        assert_eq!(v.parent_name, "", "{}", v.name);
    }
    assert_eq!(walk.iter().filter(|v| v.is_shunt).count(), 4);

    // The four `RelCalc`-fed fields are zero on every row without `RelCalc`.
    // Their multi-valued half is `tests/reliability_pins.rs::
    // pd_elements_relcalc_fields_are_live_after_relcalc` (G1.6(i)), which runs
    // `RelCalc` on three gated decks — one per channel shape — and pins a
    // non-zero witness row on each.
    for v in &walk {
        assert_eq!(v.section_id, 0, "{}", v.name);
        assert_eq!(v.total_miles, 0.0, "{}", v.name);
        assert_eq!(v.lambda, 0.0, "{}", v.name);
        assert_eq!(v.accumulated_l, 0.0, "{}", v.name);
    }
}

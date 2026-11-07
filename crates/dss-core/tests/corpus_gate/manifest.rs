//! Manifest schema + loading for the unified corpus gate.
//!
//! A case's `engines: "capi_v0145" | "r4133" | "both"` (default `"both"`) names
//! the gating channel(s). `capi_v0145` is the pinned dss-python 0.15.7 /
//! dss_capi 0.14.5 (`oracle_server.py`, persistent pool). `r4133` is the
//! official EPRI `OpenDSSDirect.dll` through the `epri-worker` bridge.
//! `isolate` runs the case on a throwaway one-shot worker, and `note` is read
//! to enforce `isolate ⇒ note`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::harness;

// ---------------------------------------------------------------------------
// Manifest structs.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub(crate) struct SolvableManifest {
    #[serde(default)]
    pub(crate) cases: Vec<SolvableCase>,
}

/// One element-specific property-probe spec: compare `element`'s listed
/// property values (OpenDSS `Properties(p).Val` against the Rust `?` query).
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ProbeSpec {
    pub(crate) element: String,
    pub(crate) props: Vec<String>,
}

/// One manifest case.
///
/// `deny_unknown_fields`: every field below is `#[serde(default)]`, so without
/// it a misspelled key (`compare_zsc_`, `compare_zsC`) deserializes cleanly to
/// `false`, the unwired-flag gate never fires, the lock records the flag as off
/// and the author believes the surface is on. An unknown key is therefore a
/// load error naming the case.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SolvableCase {
    pub(crate) path: String,
    /// Free-text authoring rationale (`tests/corpus/modes/manifest.json`). Never
    /// read by the gate; declared so `deny_unknown_fields` above can refuse a
    /// key that is *not* one of ours.
    #[serde(default)]
    #[allow(dead_code)]
    pub(crate) notes: String,
    #[serde(default = "default_kind")]
    pub(crate) kind: String,
    #[serde(default)]
    pub(crate) post: Vec<String>,
    #[serde(default = "default_steps")]
    pub(crate) n_steps: usize,
    /// YPrim focus set; `["*"]` = every element (small decks).
    #[serde(default)]
    pub(crate) selected_elements: Vec<String>,
    /// Opt in to comparing this case's monitor channels + EnergyMeter
    /// registers/zone (set only for cases that define them in deterministic
    /// modes — e.g. the daily IEEE13 case). Incidental master-defined monitors
    /// are not compared (their bare-snapshot sampling is ill-defined).
    #[serde(default)]
    pub(crate) check_meters_monitors: bool,
    /// Element-specific state channels, all opt-in:
    /// property probes, PC-element state variables, the cumulative event log,
    /// and the pending control-action queue — compared per step.
    #[serde(default)]
    pub(crate) probes: Vec<ProbeSpec>,
    #[serde(default)]
    pub(crate) compare_variables: Vec<String>,
    #[serde(default)]
    pub(crate) compare_eventlog: bool,
    #[serde(default)]
    pub(crate) compare_ctrlqueue: bool,
    /// Compare every element's every property value (Rust `?` surface against
    /// OpenDSS `Properties(p).Val`) per step, on top of the full-model compare.
    /// Off by default (heavy).
    #[serde(default)]
    pub(crate) compare_all_properties: bool,
    /// Compare `DSS.GlobalResult` (`Text.Result`) per step: the AutoAdd
    /// winner + improvement figure. Tokenized via `compare_export`.
    #[serde(default)]
    pub(crate) compare_global_result: bool,
    /// Compare the `<CircuitName_>AutoAddLog.csv` the AutoAdd solve writes.
    #[serde(default)]
    pub(crate) compare_autoadd_log: bool,
    // -- Compare-depth surface flags. A flag absent from
    //    `population_lock.rs::rigor()` is invisible to the anti-shrink guard.
    //    Every one is `#[serde(default)]` ⇒ `false`, and none may be set by a
    //    manifest before its request + comparator are wired: see
    //    [`G1_SURFACE_FLAGS`] and `no_unwired_g1_surface_flag_is_set_in_any_manifest`.
    /// The per-element **derived** channels: `Enabled`,
    /// `VoltagesMagAng`/`CurrentsMagAng`/`Residuals`, `SeqVoltages`/`SeqCurrents`/
    /// `SeqPowers`, `CplxSeqVoltages`/`CplxSeqCurrents` and `TotalPowers`.
    /// A case that opts in compares all of them.
    #[serde(default)]
    pub(crate) compare_derived: bool,
    /// The per-element **discrete extras**: `PhaseLosses`, `NodeOrder`,
    /// `EnergyMeter`, `OCPDevType`/`OCPDevIndex`, `HasVoltControl`/
    /// `HasSwitchControl`, `NumControls`, `NumTerminals`/`NumPhases`/
    /// `NumConductors`. `NodeOrder` is read only on elements that are `Enabled`
    /// and carry `NumTerminals > 0`, the two conditions under which neither
    /// OpenDSS transport dereferences a nil `NodeRef`.
    #[serde(default)]
    pub(crate) compare_element_extras: bool,
    /// The **bus** surface: `puVoltages`, `VMagAngle`, `puVmagAngle`, the
    /// per-bus `Nodes`/`kVBase` and the circuit-level `AllBusVmagPu`; the
    /// sequence and line-to-line voltages `SeqVoltages`, `CplxSeqVoltages`,
    /// `VLL` and `puVLL` (`harness::compare_bus_seq_and_vll`); `Distance` with
    /// the circuit-level `AllBusDistances` and `AllNodeDistances`
    /// (`harness::compare_bus_distances`); and `AllPCEatBus`/`AllPDEatBus`
    /// (`harness::compare_bus_at_bus`), read last in the walk because they are
    /// its only `ModeEffect::Impure` reads on the r4133 channel. A further bus
    /// quantity extends this flag, not the flag set.
    #[serde(default)]
    pub(crate) compare_bus: bool,
    /// The bus **short-circuit** surface: `Zsc1`/`Zsc0`/`ZscMatrix`/
    /// `YscMatrix`/`Isc`/`Voc`. Split from [`Self::compare_bus`] because it needs
    /// a fault study, not just a solve.
    #[serde(default)]
    pub(crate) compare_zsc: bool,
    /// The **meter extras + per-bus reliability** surface: `CalcCurrent`,
    /// `AllocFactors`, `SAIFI`/`SAIFIKW`/`SAIDI`/`CustInterrupts`, the ordered
    /// zone vectors, the active-section fields, `Meters.Totals` and the per-bus
    /// reliability columns (`Bus.Lambda`/`N_interrupts`/`N_Customers`/
    /// `Cust_Interrupts`/`Cust_Duration`/`Int_Duration`/`TotalMiles`/
    /// `SectionID`).
    ///
    /// It is the ONLY flag that also DRIVES an executive command: no vendored
    /// deck runs `RelCalc` (the decks that do are `expect_solve_abort`),
    /// so without driving it the whole reliability half would compare `0 == 0`.
    /// The gate therefore runs it once per case on the LAST step, on all three
    /// engines (`corpus_gate/runner.rs`); it is not idempotent, so never per
    /// step.
    ///
    /// Set by the MANIFESTS, never scheduler-forced. There is no
    /// `force_reliability` beside [`super::scheduler::force_pdelements`]: the
    /// predicate is "this deck defines an EnergyMeter", which is not a manifest
    /// field, and forcing the flag circuit-wide would fire the executive's
    /// `28724 No EnergyMeter Objects Defined` on every meterless case on three
    /// engines. The population is
    /// therefore the manifest set, which `population.lock.json` records through
    /// `population_lock::rigor`'s `rel=` token; `harness::
    /// assert_reliability_compare_ran` is the fail-on-nothing-ran half.
    #[serde(default)]
    pub(crate) compare_reliability: bool,
    /// The **PDElements** interface walk: all thirteen `IPDElements` fields
    /// (`Name`, `AccumulatedL`, `ParentPDElement`, `FromTerminal`, `IsShunt`,
    /// `Numcustomers`, `SectionID`, `FaultRate`, `RepairTime`, `TotalMiles`,
    /// `Totalcustomers`, `pctPermanent`, `Lambda`) plus the parent's full name,
    /// which the `ParentPDElement` active-element hijack hands out for free.
    /// Forced on every live non-`large` case by
    /// `scheduler::force_pdelements`, so no manifest sets it (the
    /// `compare_all_properties` shape).
    #[serde(default)]
    pub(crate) compare_pdelements: bool,
    /// The **topology** interface: `NumLoops`, `NumIsolatedBranches`/
    /// `NumIsolatedLoads`, `AllLoopedPairs`, `AllIsolatedBranches`/
    /// `AllIsolatedLoads`.
    #[serde(default)]
    pub(crate) compare_topology: bool,
    /// The **incidence-matrix** surface: `IncMatrix`, `IncMatrixCols`,
    /// `IncMatrixRows`, `Laplacian`, after a `CalcIncMatrix`.
    #[serde(default)]
    pub(crate) compare_inc_matrix: bool,
    /// The **run-produced file SET**: the filesystem entries the run
    /// (`clear → compile → post → n_steps × solve`) creates under the case
    /// directory, as `/`-joined case-dir-relative names, ASCII-case-folded, a
    /// trailing `/` marking a created directory. The same flag compares the
    /// contents of the files `dss_epri::guard::RUN_FILE_CONTENTS_PATTERNS`
    /// selects (`harness::run_file_contents`). Monitor CSVs contribute their
    /// NAMES (circuit, monitor, channel index), while their data stays out of
    /// the gate: a monitor the run never sampled has no defined content.
    /// `save circuit` has no corpus surface (no `save` command in the reachable
    /// corpus); its gate is the fixture `save_roundtrip.rs::save_forms_structural_file_set`.
    #[serde(default)]
    pub(crate) compare_run_files: bool,
    /// The CONTENTS of the run-created **demand-interval tree**
    /// (`<OutputDirectory><CaseName>/DI_yr_<year>/*.csv`), the subset of the
    /// created-file SET above that only a deck enabling `Set DemandInterval=`
    /// writes. Split off from `compare_run_files` because its population is
    /// different (most producers are `kind=large`, which that surface's force
    /// rule excludes) and because its cells are compared by quantity class, not
    /// as a set (`harness::di::compare_di`).
    #[serde(default)]
    pub(crate) compare_di: bool,
    /// The feature this case covers is not ported yet:
    /// the gate asserts the Rust engine errors loudly instead of live-comparing.
    #[serde(default)]
    pub(crate) pending: bool,
    /// This deck aborts the solve on BOTH engines; the value is the error
    /// substring both must produce (malformed input → `DSS.SolutionAbort`, or
    /// control non-settling → `#485 Max Control Iterations Exceeded`). Mutually
    /// exclusive with `pending`.
    #[serde(default)]
    pub(crate) expect_solve_abort: Option<String>,
    /// Names the work that ports this case's feature. Mandatory while
    /// `pending` or `defer_ledger`.
    #[serde(default)]
    pub(crate) wp: Option<String>,
    /// This case reproduces on neither channel (0.14.5 lacks the feature or
    /// differs, r4133 diverges), so it is parked from the live comparison until
    /// it has a ledger entry. The value is the cause (mandatory), and `wp` names
    /// the follow-up. The Rust engine is still smoke-run (compile + solve, must
    /// converge with no new errors), so a convergence or error regression never
    /// hides here, but a numeric regression that still converges is NOT caught.
    /// Membership is preserved: this never drops a case (the population lock
    /// records the `defer` flag). Mutually exclusive with `pending` /
    /// `expect_solve_abort`.
    #[serde(default)]
    pub(crate) defer_ledger: Option<String>,
    /// Non-fatal messages this deck's compile produces on the engine and the
    /// solve goes on: a user-written model DLL that safe Rust cannot load, and
    /// the refusal of input the documentation does not allow, such as a typed
    /// Transformer `NormAmps` (read only, the token is refused and the derived
    /// rating stays). Each string is a substring an actual engine error must
    /// contain, a whole message where the engine's refusal is meant. Every
    /// actual error must match one, and every listed substring must actually
    /// appear. Checked after the compile on the live path and, before the
    /// `post` commands, on the abort path. A non-empty list also sends
    /// `warn_and_continue` to both OpenDSS channels on the live path. Empty ⇒ zero errors are
    /// tolerated.
    #[serde(default)]
    pub(crate) expect_warnings: Vec<String>,
    /// Which live channel(s) gate this case. `"capi_v0145"` = the pinned
    /// dss-python 0.15.7 / dss_capi 0.14.5 (persistent `oracle_server.py` pool),
    /// `"r4133"` = the official EPRI `OpenDSSDirect.dll` r4133 through the
    /// `epri-worker` bridge, `"both"` (the default) gates on both channels.
    #[serde(default = "default_engines")]
    pub(crate) engines: String,
    /// A-Diakoptics disposition (mandatory on every family-manifest case).
    #[serde(default)]
    pub(crate) ad: Option<String>,
    /// Run every engine execution of this case on a throwaway one-shot worker
    /// process instead of a persistent pool worker. Set only for a proven
    /// worker-state-contamination case, and it must carry a `note`. Default
    /// false.
    #[serde(default)]
    pub(crate) isolate: bool,
    /// Free-text provenance. Read only to enforce the `isolate ⇒ note`
    /// structural rule, otherwise ignored by the gate.
    #[serde(default)]
    pub(crate) note: Option<String>,
}

pub(crate) fn default_kind() -> String {
    "feeder".to_string()
}
pub(crate) fn default_steps() -> usize {
    1
}
pub(crate) fn default_engines() -> String {
    "both".to_string()
}

/// The three legal `engines` channel selectors.
pub(crate) const ENGINES_SPECS: &[&str] = &["capi_v0145", "r4133", "both"];

/// One live gating channel a case is compared against.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum EngineChannel {
    /// Pinned dss-python 0.15.7 / dss_capi 0.14.5 through `oracle_server.py`.
    CapiV0145,
    /// Official EPRI `OpenDSSDirect.dll` r4133 via `epri-worker`.
    R4133,
}

impl EngineChannel {
    /// The eventlog-mask spec key for this channel (`harness::compare_eventlog`):
    /// the pinned dss_capi needs no masking (`None`), r4133 selects its per-rev row.
    pub(crate) fn eventlog_spec(self) -> Option<&'static str> {
        match self {
            EngineChannel::CapiV0145 => None,
            EngineChannel::R4133 => Some("r4133"),
        }
    }
    /// Iteration policy: the pinned dss_capi must match the iteration count
    /// exactly. r4133 (a different engine line) allows this engine to converge
    /// in FEWER iterations, never more (`rust_le_oracle`).
    pub(crate) fn iterations_exact(self) -> bool {
        matches!(self, EngineChannel::CapiV0145)
    }
    /// This channel as the **harness's** own channel type.
    ///
    /// `EngineChannel` is `pub(crate)` to this one test binary, and the harness
    /// library cannot name a type of it, so the property comparator cannot take it
    /// (the channel-threading trap). Every corpus_gate
    /// call site maps through here — one mapping, not one per call site.
    pub(crate) fn props_channel(self) -> harness::PropsChannel {
        match self {
            EngineChannel::CapiV0145 => harness::PropsChannel::CapiV0145,
            EngineChannel::R4133 => harness::PropsChannel::R4133,
        }
    }
}

impl SolvableCase {
    /// The live channel(s) this case gates against (schema v2 `engines`).
    pub(crate) fn engine_channels(&self) -> Vec<EngineChannel> {
        match self.engines.as_str() {
            "capi_v0145" => vec![EngineChannel::CapiV0145],
            "r4133" => vec![EngineChannel::R4133],
            "both" => vec![EngineChannel::CapiV0145, EngineChannel::R4133],
            other => panic!(
                "{}: unknown engines spec {other:?} — expected one of {ENGINES_SPECS:?}",
                self.path
            ),
        }
    }
    /// Does this case gate the pinned dss_capi 0.14.5 channel?
    ///
    /// Not a property-forcing predicate:
    /// [`crate::scheduler::force_properties`] forces properties on every live
    /// non-`large` case regardless of `engines`. The reader is the
    /// `DSS_LIVE_PROPS` pilot in `corpus_gate.rs`, which drives the pinned
    /// dss_capi directly and therefore skips r4133-only cases.
    pub(crate) fn gates_capi(&self) -> bool {
        matches!(self.engines.as_str(), "capi_v0145" | "both")
    }
}

// ---------------------------------------------------------------------------
// Path helpers.
// ---------------------------------------------------------------------------

pub(crate) fn manifests_dir() -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "manifests",
    ]
    .iter()
    .collect()
}

/// Absolute, forward-slashed path to a file under the **vendored** corpus
/// (`tests/corpus/electricdss-tst`). The live gate never reads `.inputs`.
pub(crate) fn corpus_file(rel: &str) -> String {
    let p: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "electricdss-tst",
    ]
    .iter()
    .collect::<PathBuf>()
    .join(rel);
    assert!(p.is_file(), "corpus case missing: {}", p.display());
    p.to_string_lossy().replace('\\', "/")
}

pub(crate) fn family_dir(name: &str) -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        name,
    ]
    .iter()
    .collect()
}

/// Absolute, forward-slashed path to a deck under `tests/corpus/<name>/`.
pub(crate) fn family_file(name: &str, rel: &str) -> String {
    let p = family_dir(name).join(rel);
    assert!(p.is_file(), "{name} deck missing: {}", p.display());
    p.to_string_lossy().replace('\\', "/")
}

pub(crate) fn load_solvable() -> Vec<SolvableCase> {
    let p = manifests_dir().join("solvable_now.json");
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    let m: SolvableManifest =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()));
    m.cases
}

pub(crate) fn load_family(name: &str) -> Vec<SolvableCase> {
    let p = family_dir(name).join("manifest.json");
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    let m: SolvableManifest =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()));
    m.cases
}

/// Recursively collect every `.dss` under `dir` as forward-slashed paths
/// relative to `base`.
fn collect_family_decks(dir: &Path, base: &Path, out: &mut BTreeSet<String>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
        let p = entry.expect("dir entry").path();
        if p.is_dir() {
            collect_family_decks(&p, base, out);
        } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("dss")) {
            out.insert(
                p.strip_prefix(base)
                    .expect("under base")
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Synthetic deck families.
// ---------------------------------------------------------------------------

/// One synthetic deck family under `tests/corpus/<name>/`.
pub(crate) struct Family {
    pub(crate) name: &'static str,
    /// Pinned deck floor: removing a deck (even together with its manifest
    /// entry) fails `*_manifest_is_complete`.
    pub(crate) required: &'static [&'static str],
    /// Per-family structural invariant, applied to every case.
    pub(crate) check_case: fn(&SolvableCase),
    /// Gate every element's every property value on every live case in this
    /// family.
    pub(crate) compare_all_properties: bool,
}

/// Engine-free structural guard shared by the families: deck dir ↔ manifest
/// bijection, the pinned coverage floor, `wp` present on every pending case,
/// valid `ad`/`engines`/`isolate` fields, and the family's per-case invariant.
pub(crate) fn family_manifest_is_complete(fam: &Family) {
    let dir = family_dir(fam.name);
    assert!(
        dir.is_dir(),
        "{} deck dir missing: {}",
        fam.name,
        dir.display()
    );
    let mut disk: BTreeSet<String> = BTreeSet::new();
    collect_family_decks(&dir, &dir, &mut disk);
    let cases = load_family(fam.name);
    let manifested: BTreeSet<String> = cases.iter().map(|c| c.path.replace('\\', "/")).collect();
    assert_eq!(
        manifested.len(),
        cases.len(),
        "duplicate paths in {} manifest",
        fam.name
    );
    assert_eq!(
        disk, manifested,
        "{} decks on disk and manifest entries must be a bijection \
         (disk∖manifest = unclassified deck, manifest∖disk = ghost entry)",
        fam.name
    );
    for req in fam.required {
        assert!(
            manifested.contains(*req),
            "required {} deck missing: {req} (pinned coverage floor)",
            fam.name
        );
    }
    for c in &cases {
        if c.pending {
            assert!(
                c.wp.is_some(),
                "{}: pending case must name in `wp` the work that ports it",
                c.path
            );
        }
        assert_defer_ledger_is_valid(&format!("{}:{}", fam.name, c.path), c);
        assert!(
            ENGINES_SPECS.contains(&c.engines.as_str()),
            "{}: unknown engines spec {:?} — expected one of {ENGINES_SPECS:?}",
            c.path,
            c.engines
        );
        assert_isolate_carries_note(&format!("{}:{}", fam.name, c.path), c);
        let ad = c.ad.as_deref().unwrap_or_else(|| {
            panic!(
                "{}:{}: missing mandatory `ad` disposition (one of \
                 full|pf|off:<reason>)",
                fam.name, c.path
            )
        });
        assert!(
            ad_disposition_is_valid(ad),
            "{}:{}: invalid `ad` disposition {ad:?} — expected full|pf|off:<reason>",
            fam.name,
            c.path
        );
        (fam.check_case)(c);
    }
}

/// Structural rule: a `defer_ledger` case carries a non-empty cause + a `wp`,
/// and never also claims `pending`/`expect_solve_abort`.
pub(crate) fn assert_defer_ledger_is_valid(label: &str, c: &SolvableCase) {
    if let Some(cause) = &c.defer_ledger {
        assert!(
            !cause.trim().is_empty(),
            "{label}: `defer_ledger` requires a non-empty cause"
        );
        assert!(
            c.wp.is_some(),
            "{label}: `defer_ledger` requires a `wp` naming the follow-up"
        );
        assert!(
            !c.pending && c.expect_solve_abort.is_none(),
            "{label}: `defer_ledger` is mutually exclusive with `pending`/`expect_solve_abort`"
        );
    }
}

/// Structural rule: an `isolate: true` case must
/// carry a `note` documenting the proven contamination that justifies it.
pub(crate) fn assert_isolate_carries_note(label: &str, c: &SolvableCase) {
    if c.isolate {
        assert!(
            c.note.as_deref().is_some_and(|n| !n.trim().is_empty()),
            "{label}: `isolate: true` requires a non-empty `note` documenting the \
             proven worker-state contamination"
        );
    }
}

// ---------------------------------------------------------------------------
// Compare-depth surface flags.
// ---------------------------------------------------------------------------

/// One compare-depth surface flag: the manifest field name, a short name of
/// its surface, whether its request + comparator are wired yet, and the reader
/// that pulls it off a case.
///
/// A flag whose surface is not wired yet is a field the loader accepts and the
/// runtime ignores: the request builder would not send it, the OpenDSS engine
/// would return nothing, and the case would "compare" an empty capture against
/// an empty capture, green and silently vacuous. A row flips to `true` **in
/// the same commit** that adds the request field and the comparator (the
/// `pending ⇒ wp` / `isolate ⇒ note` structural-gate pattern of
/// [`family_manifest_is_complete`]).
pub(crate) struct G1Flag {
    /// The `serde` field name, exactly as a manifest spells it.
    pub(crate) name: &'static str,
    /// A short name of the surface, named by the refusal.
    pub(crate) sub_step: &'static str,
    /// Is the capture request + comparator in the tree yet?
    pub(crate) wired: bool,
    pub(crate) get: fn(&SolvableCase) -> bool,
}

/// The whole compare-depth flag vocabulary, declared once. Order matches the
/// declaration order in [`SolvableCase`] and the token order in
/// `population_lock.rs::rigor()`.
pub(crate) const G1_SURFACE_FLAGS: &[G1Flag] = &[
    // Request key `derived` in `engines::build_run_request`, capture in
    // `tools/oracle/oracle_server.py` + `crates/dss-epri/src/capture.rs`,
    // comparator `harness::compare_element_derived` behind
    // `capture_guard::require_capture` in `runner::compare_capture`.
    G1Flag {
        name: "compare_derived",
        sub_step: "element derived channels",
        wired: true,
        get: |c| c.compare_derived,
    },
    // Request key `element_extras` in `engines::build_run_request`, capture in
    // `tools/oracle/oracle_server.py` + `crates/dss-epri/src/capture.rs`
    // (`Engine::element_extras` in `dss.rs` for the four pure scalars, the
    // conditional `NodeOrder` read at the call site), comparator
    // `harness::compare_element_extras` behind `capture_guard::require_capture`
    // in `runner::compare_capture`.
    G1Flag {
        name: "compare_element_extras",
        sub_step: "element discrete extras",
        wired: true,
        get: |c| c.compare_element_extras,
    },
    G1Flag {
        name: "compare_bus",
        sub_step: "bus surface",
        wired: true,
        get: |c| c.compare_bus,
    },
    G1Flag {
        name: "compare_zsc",
        sub_step: "bus short-circuit surface",
        wired: true,
        get: |c| c.compare_zsc,
    },
    G1Flag {
        name: "compare_reliability",
        sub_step: "meter extras and reliability",
        wired: true,
        get: |c| c.compare_reliability,
    },
    G1Flag {
        name: "compare_pdelements",
        sub_step: "PDElements walk",
        wired: true,
        get: |c| c.compare_pdelements,
    },
    G1Flag {
        name: "compare_topology",
        sub_step: "topology interface",
        wired: true,
        get: |c| c.compare_topology,
    },
    G1Flag {
        name: "compare_inc_matrix",
        sub_step: "incidence matrix",
        wired: true,
        get: |c| c.compare_inc_matrix,
    },
    // Request key `run_files` in `engines::build_run_request`, capture on both
    // transports from the very `CorpusGuard` that sweeps the case dir
    // (`tools/oracle/corpus_guard.py`'s `CorpusGuard.created` + its Rust twin
    // `crates/dss-epri/src/guard.rs`, read strictly last of the run), comparator
    // `harness::run_files::compare_run_files` behind
    // `capture_guard::require_capture_opt` in `runner::compare_with_result`
    // (an empty created set is a legitimate answer, a missing capture is not).
    // The same flag compares the contents of the selected files
    // (`harness::run_file_contents`, selection
    // `dss_epri::guard::RUN_FILE_CONTENTS_PATTERNS`). The demand-interval tree
    // is `compare_di`'s.
    G1Flag {
        name: "compare_run_files",
        sub_step: "run-produced file set",
        wired: true,
        get: |c| c.compare_run_files,
    },
    // Request key `di` in `engines::build_run_request`, captured on BOTH
    // transports from the very `CorpusGuard` classification `compare_run_files`
    // compares as a SET (`dss_epri::guard::CorpusGuard::created_di_files` + its
    // Python twin `tools/oracle/corpus_guard.py`), copied into the gate's own
    // per-(case, channel) sidecar and compared cell by cell against this
    // engine's tree by `harness::di::compare_di` behind
    // `capture_guard::require_capture_opt` in `runner::compare_with_result`. An
    // EMPTY tree is a legitimate answer (`Storage-Quasi-Static-Example/Run_Demo1.dss`
    // runs `closedi` but never enables `DemandInterval`, so all three engines
    // write nothing), and a missing capture is not.
    G1Flag {
        name: "compare_di",
        sub_step: "demand-interval tree",
        wired: true,
        get: |c| c.compare_di,
    },
];

/// Refuse a manifest case that sets a [`G1_SURFACE_FLAGS`] flag whose surface is
/// not wired yet. Shared by the manifest walk below and its
/// synthetic non-vacuity drive.
pub(crate) fn assert_no_unwired_g1_flag(label: &str, c: &SolvableCase) {
    for f in G1_SURFACE_FLAGS {
        assert!(
            !((f.get)(c) && !f.wired),
            "{label}: sets `{}`, but that surface ({}) is not wired yet. \
             The request builder would not send it and the comparator does not \
             exist, so the case would compare an empty capture against an empty \
             capture and pass. Flip `G1Flag::wired` in the SAME commit that adds \
             the capture request + the comparator.",
            f.name,
            f.sub_step
        );
    }
}

fn check_asymmetric_case(c: &SolvableCase) {
    assert!(
        !c.selected_elements.is_empty(),
        "{}: asymmetric case must name selected_elements (live YPrim compare \
         is the direct transposed-stamp catch)",
        c.path
    );
}

fn check_controls_case(c: &SolvableCase) {
    if c.expect_solve_abort.is_some() {
        return;
    }
    assert!(
        !c.probes.is_empty()
            || !c.compare_variables.is_empty()
            || c.compare_eventlog
            || c.compare_ctrlqueue
            || c.check_meters_monitors,
        "{}: controls case must opt into at least one element-specific \
         state channel (probes/variables/eventlog/ctrlqueue/meters)",
        c.path
    );
}

fn check_modes_case(c: &SolvableCase) {
    assert!(
        !c.selected_elements.is_empty(),
        "{}: modes case must name selected_elements (full-model live compare \
         once the feature is ported)",
        c.path
    );
}

/// The pinned element-coverage floor for the asymmetric family.
const ASYMMETRIC_REQUIRED: &[&str] = &[
    "vsource/vsource_asym.dss",
    "reactor/reactor_asym.dss",
    "capacitor/capacitor_asym.dss",
    "line/line_asym.dss",
    "line/line_geometry_asym.dss",
    "line/line_cable_asym.dss",
    "line/line_spacing_asym.dss",
    "line/line_llc_harm_asym.dss",
    "line/line_ground_z_asym.dss",
    "line/line_fullcarson_asym.dss",
    "transformer/transformer_asym.dss",
    "transformer/transformer_wyedelta_asym.dss",
    "fault/fault_asym.dss",
    "load/load_asym.dss",
    "load/midi_load_vregion_asym.dss",
    "generator/generator_asym.dss",
    "generator/gen_currentlimited_asym.dss",
    "der/der_asym.dss",
    "der/der_state_asym.dss",
    "indmach/indmach_asym.dss",
    "vccs/vccs_asym.dss",
    "upfc/upfc_asym.dss",
    "combo/combo_chain_asym.dss",
    "combo/combo_mesh_asym.dss",
    "combo/midi_asym.dss",
    "combo/midi_geometry_cable_asym.dss",
    "vsource/midi_vsource_asym.dss",
    "reactor/midi_reactor_asym.dss",
    "capacitor/midi_capacitor_asym.dss",
    "line/midi_line_asym.dss",
    "transformer/midi_transformer_asym.dss",
    "fault/midi_fault_asym.dss",
    "load/midi_load_asym.dss",
    "generator/midi_generator_asym.dss",
    "der/midi_der_asym.dss",
    "indmach/midi_indmach_asym.dss",
    "vccs/midi_vccs_asym.dss",
    "upfc/midi_upfc_asym.dss",
    // pending (unported element classes; wp fields name the porting WP)
    "isource/isource_snap.dss",
    "isource/midi_isource_asym.dss",
    "autotrans/autotrans_snap.dss",
    "autotrans/midi_autotrans_asym.dss",
    "autotrans/autotrans_gic.dss",
    // The only deck in the corpus that drives an AutoTrans from an `XfmrCode`.
    "autotrans/autotrans_xfmrcode.dss",
    "gic/gicline_gic.dss",
    "gic/gictransformer_gic.dss",
    "gic/gicsource_gic.dss",
    "gic/gic_midi.dss",
];

pub(crate) const ASYMMETRIC: Family = Family {
    name: "asymmetric",
    required: ASYMMETRIC_REQUIRED,
    check_case: check_asymmetric_case,
    compare_all_properties: true,
};

const CONTROLS_REQUIRED: &[&str] = &[
    "regcontrol/regcontrol_sym.dss",
    "regcontrol/regcontrol_asym.dss",
    "regcontrol/regcontrol_ldc.dss",
    "regcontrol/regcontrol_reverse.dss",
    "regcontrol/regcontrol_remotebus.dss",
    "regcontrol/regcontrol_inversetime.dss",
    "capcontrol/capcontrol_sym.dss",
    "capcontrol/capcontrol_asym.dss",
    "capcontrol/capcontrol_pf.dss",
    "capcontrol/capcontrol_time.dss",
    "capcontrol/capcontrol_voverride.dss",
    "invcontrol/invcontrol_vv_sym.dss",
    "invcontrol/invcontrol_vvvw_asym.dss",
    "invcontrol/invcontrol_drc.dss",
    "invcontrol/invcontrol_vv_drc.dss",
    "invcontrol/invcontrol_wattpf.dss",
    "invcontrol/invcontrol_wattvar.dss",
    "invcontrol/invcontrol_avr.dss",
    "invcontrol/invcontrol_monbus.dss",
    "invcontrol/midi_invcontrol_drc.dss",
    "storagecontroller/storagectrl_peakshave.dss",
    "storagecontroller/storagectrl_time.dss",
    "storagecontroller/storagectrl_follow.dss",
    "storagecontroller/storagectrl_support.dss",
    "storagecontroller/storagectrl_ipeakshave.dss",
    "storagecontroller/storagectrl_loadshape.dss",
    "storagecontroller/storagectrl_chargelow.dss",
    "gendispatcher/gendispatcher.dss",
    "recloser/recloser_temp.dss",
    "recloser/recloser_perm.dss",
    "recloser/recloser_ground.dss",
    "recloser/recloser_1ph.dss",
    "recloser/recloser_pickup_split.dss",
    "fuse/fuse_blow_3ph.dss",
    "swtcontrol/swtcontrol_lock.dss",
    "gendispatcher/gendispatcher_kvarlimit.dss",
    "relay/relay_oc_sym.dss",
    "relay/relay_4647_asym.dss",
    "relay/relay_voltage.dss",
    "relay/relay_revpower.dss",
    "relay/relay_generic.dss",
    "relay/relay_distance.dss",
    "relay/relay_td21.dss",
    "relay/relay_doc.dss",
    "fuse/fuse_blow_asym.dss",
    "swtcontrol/swtcontrol_time.dss",
    "energymeter/energymeter_sym.dss",
    "energymeter/energymeter_asym.dss",
    "energymeter/energymeter_options.dss",
    "monitor/monitor_modes_hi.dss",
    "monitor/monitor_seqmag.dss",
    "expcontrol/expcontrol_basic.dss",
    "monitor/monitor_modes.dss",
    "sensor/sensor_map.dss",
    "upfc/upfc_vreg.dss",
    "upfc/upfc_doubleref.dss",
    // UPFC modes 2 (StatCOM) / 3 (Dual) / 5 (DoubleRef Dual).
    "upfc/upfc_statcom.dss",
    "upfc/upfc_dual.dss",
    "upfc/upfc_doubleref_dual.dss",
    "combo/combo_protection.dss",
    "combo/combo_voltvar_asym.dss",
    "combo/combo_metering.dss",
    "combo/midi_controls.dss",
    "combo/midi_protection.dss",
    "regcontrol/midi_regcontrol.dss",
    "capcontrol/midi_capcontrol.dss",
    "invcontrol/midi_invcontrol.dss",
    "storagecontroller/midi_storagectrl.dss",
    "gendispatcher/midi_gendispatcher.dss",
    "recloser/midi_recloser_temp.dss",
    "recloser/midi_recloser_perm.dss",
    "relay/midi_relay_4647.dss",
    "fuse/midi_fuse.dss",
    "swtcontrol/midi_swtcontrol.dss",
    "energymeter/midi_energymeter.dss",
    "monitor/midi_monitor.dss",
    "sensor/midi_sensor.dss",
    "monitor/monitor_pst.dss",
    "monitor/midi_monitor_pst.dss",
    // pending / feature decks (anti-deletion floor)
    "capcontrol/capcontrol_follow.dss",
    "invcontrol/invcontrol_expmodel.dss",
    "invcontrol/invcontrol_storage_vw.dss",
    "invcontrol/invcontrol_storage_vv_vw.dss",
    "storagecontroller/storagecontroller_seasonal.dss",
    "isource/isource_daily.dss",
    "isource/isource_both.dss",
    "isource/midi_isource.dss",
    "isource/midi_isource_both.dss",
    "autotrans/autotrans_reg.dss",
    "autotrans/autotrans_both.dss",
    "autotrans/midi_autotrans.dss",
    "autotrans/midi_autotrans_both.dss",
    "gfm/gfm_micro.dss",
    "gfm/gfm_invcontrol.dss",
    "gfm/gfm_dynamics.dss",
    "gfm/pv_gfm_dynamics.dss",
    "invcontrol/invcontrol_multi_vv_wye.dss",
    "invcontrol/invcontrol_vv_delta.dss",
];

pub(crate) const CONTROLS: Family = Family {
    name: "controls",
    required: CONTROLS_REQUIRED,
    check_case: check_controls_case,
    compare_all_properties: true,
};

const MODES_REQUIRED: &[&str] = &[
    "inputformat/shape_binfiles/shape_binfiles.dss",
    "inputformat/xycurve_files/xycurve_files.dss",
    "inputformat/shape_mmf/shape_mmf.dss",
    "inputformat/shape_filearr/shape_filearr.dss",
    "time/generaltime.dss",
    "time/ld1.dss",
    "time/ld2.dss",
    "montecarlo/monte1.dss",
    "montecarlo/monte2.dss",
    "montecarlo/monte3.dss",
    "montecarlo/montefault.dss",
    "autoadd/autoadd.dss",
    "autoadd/autoadd_cap.dss",
    "newton/newton.dss",
    "newton/newton_feeder.dss",
    "harmonics/reactor_rlcurve.dss",
    "harmonics/isource_harm.dss",
    "batchedit/batchedit.dss",
    "batchedit/midi_batchedit.dss",
    "reduce/reduce_default.dss",
    "reduce/reduce_shortlines.dss",
    "reduce/reduce_dangling.dss",
    "reduce/reduce_switches.dss",
    "reduce/reduce_laterals.dss",
    "reduce/reduce_mergeparallel.dss",
    "reduce/reduce_breakloop.dss",
    "reduce/reduce_keeplist.dss",
    "reduce/reduce_remove.dss",
    "reduce/midi_reduce.dss",
    "upgrade/upgrade_pilot.dss",
    "upgrade/upgrade_growth_year0.dss",
    "upgrade/upgrade_forcehooks.dss",
    "makeposseq/makeposseq_line.dss",
    "makeposseq/makeposseq_xfmr.dss",
    "makeposseq/makeposseq_shunt.dss",
    // The GICTransformer arm of the shunt deck, its own `engines: "r4133"` case
    // because capi 0.14.5 is nondeterministic on any deck that instantiates a
    // GICTransformer.
    "makeposseq/makeposseq_gic.dss",
    "makeposseq/makeposseq_pc.dss",
    "makeposseq/makeposseq_pc_repeat.dss",
    "makeposseq/makeposseq_ctrl.dss",
    "makeposseq/makeposseq_report.dss",
    "time/daily.dss",
    "time/daily_bigstep.dss",
    "time/yearly.dss",
    "time/duty.dss",
    "time/midi_duty_ctrl.dss",
    "harmonics/harmonic_hlist.dss",
    "harmonics/harmonict.dss",
    "reset/mode_reset.dss",
    "windgen/windgen_snap.dss",
    "windgen/windgen_snap_delta.dss",
    "windgen/windgen_daily.dss",
    "windgen/windgen_dyn.dss",
    "windgen/windgen_dyn_fault.dss",
    "upgrade/upgrade_linecs_eqspacing.dss",
    // The micro-tier witness for the bus short-circuit surface. The corpus' vendored
    // fault-study decks are all `kind: "feeder"`, so this is the only case
    // that compares `Zsc1`/`Zsc0`/`ZscMatrix`/`YscMatrix`/`Isc`/`Voc` at the
    // `micro` band — and the only one whose `b2` has an insertion node order
    // (`[2,1,3]`) different from its ascending one, which is what makes the
    // surface's indexing convention observable at all.
    "faultstudy/faultstudy_micro.dss",
];

pub(crate) const MODES: Family = Family {
    name: "modes",
    required: MODES_REQUIRED,
    check_case: check_modes_case,
    compare_all_properties: true,
};

/// All three synthetic families, in canonical order.
pub(crate) const FAMILIES: [&Family; 3] = [&ASYMMETRIC, &CONTROLS, &MODES];

// ---------------------------------------------------------------------------
// A-Diakoptics disposition (shared by the family checks and the AD sweep).
// ---------------------------------------------------------------------------

/// The closed set of `off:` reasons an AD disposition may carry.
pub(crate) const AD_OFF_REASONS: &[&str] = &[
    "non-3ph-cut-only",
    "too-small",
    "already-torn-artifact",
    "mode-outside-AD-scope",
    "deck-aborts-by-design",
    "save-roundtrip-geometry",
    "save-roundtrip-relpath",
    "save-roundtrip-userdll",
    "save-roundtrip-regxfmr",
    "save-roundtrip-autotrans",
    "save-roundtrip-relay",
    "save-roundtrip-control",
    "ad-regulator-divergence",
    "ad-switched-divergence",
    "ad-islanded-divergence",
    "ad-nonconvergent",
    // The AD probe has no *baseline*: the normal arm itself (`compile; set
    // controlmode=off; solve mode=snap`, `ad_solve_normal`) does not converge —
    // and does not converge on OpenDSS either, so there is nothing to compare AD
    // against and nothing to fix in this engine. Distinct from `ad-nonconvergent`,
    // which is about the AD arm failing on a singular torn zone.
    "ad-baseline-nonconvergent",
    "ad-singular-zone",
    "ad-divergent",
    "ad-floor-above-tier",
    "unclassified-new-deck",
];

/// A valid `ad` disposition is `full`, `pf`, or `off:<reason>` where `reason` is
/// one of the reviewed [`AD_OFF_REASONS`].
pub(crate) fn ad_disposition_is_valid(s: &str) -> bool {
    s == "full"
        || s == "pf"
        || s.strip_prefix("off:")
            .is_some_and(|r| AD_OFF_REASONS.contains(&r))
}

// ---------------------------------------------------------------------------
// Engine-free structural tests (fast, no engine spawned).
// ---------------------------------------------------------------------------

#[test]
fn solvable_now_engines_specs_are_valid() {
    for c in load_solvable() {
        assert!(
            ENGINES_SPECS.contains(&c.engines.as_str()),
            "{}: unknown engines spec {:?} — expected one of {ENGINES_SPECS:?}",
            c.path,
            c.engines
        );
        assert_isolate_carries_note(&format!("solvable_now:{}", c.path), &c);
        assert_defer_ledger_is_valid(&format!("solvable_now:{}", c.path), &c);
    }
}

#[test]
fn solvable_now_has_multistep_depth() {
    let cases = load_solvable();
    assert!(!cases.is_empty(), "solvable_now must not be empty");
    let multistep_metered = cases
        .iter()
        .filter(|c| c.n_steps > 1 && c.check_meters_monitors)
        .count();
    assert!(
        multistep_metered >= 1,
        "solvable_now must keep ≥1 multi-step (n_steps>1) case with \
         check_meters_monitors=true (live multi-step + meter + monitor coverage); \
         found {multistep_metered}"
    );
    let with_yprim = cases
        .iter()
        .filter(|c| !c.selected_elements.is_empty())
        .count();
    assert!(
        with_yprim >= 1,
        "solvable_now must keep ≥1 case with selected_elements (live YPrim \
         coverage); found {with_yprim}"
    );
}

#[test]
fn asymmetric_manifest_is_complete() {
    family_manifest_is_complete(&ASYMMETRIC);
}

#[test]
fn controls_manifest_is_complete() {
    family_manifest_is_complete(&CONTROLS);
}

#[test]
fn modes_manifest_is_complete() {
    family_manifest_is_complete(&MODES);
}

/// No manifest may switch on a compare-depth surface flag before its capture
/// request and comparator are wired.
///
/// Walks all four manifests (`solvable_now` + the three synthetic families), so
/// the moment a flag appears in a deck's JSON without its `G1Flag::wired` flip,
/// the gate names the case and the flag.
#[test]
fn no_unwired_g1_surface_flag_is_set_in_any_manifest() {
    for c in load_solvable() {
        assert_no_unwired_g1_flag(&format!("solvable_now:{}", c.path), &c);
    }
    for fam in FAMILIES {
        for c in load_family(fam.name) {
            assert_no_unwired_g1_flag(&format!("{}:{}", fam.name, c.path), &c);
        }
    }
}

/// Non-vacuity for the rail above: few flags are declared in the manifests
/// (`compare_topology` on its witness decks,
/// `scheduler::TOPOLOGY_DECLARED_IN_MANIFEST`), so for the others the walk
/// passes on an empty premise. Each flag is driven on a synthetic case: the
/// refusal fires, and only a wired row lets one through.
#[test]
fn an_unwired_g1_surface_flag_on_a_case_is_refused() {
    for f in G1_SURFACE_FLAGS {
        let mut c = SolvableCase {
            path: "synthetic.dss".to_string(),
            ..Default::default()
        };
        set_g1_flag(&mut c, f.name);
        assert!(
            (f.get)(&c),
            "{}: `set_g1_flag` did not set the field the row reads",
            f.name
        );
        let err = std::panic::catch_unwind(|| {
            assert_no_unwired_g1_flag("synthetic:case.dss", &c);
        });
        if f.wired {
            assert!(
                err.is_ok(),
                "{}: the flag is wired, so setting it must be allowed",
                f.name
            );
        } else {
            let payload = err.expect_err(&format!(
                "{}: an unwired flag set on a case must be refused",
                f.name
            ));
            let msg = crate::runner::panic_msg(payload);
            assert!(
                msg.contains(f.name) && msg.contains(f.sub_step) && msg.contains("synthetic"),
                "{}: refusal must name the flag, its surface and the case; got {msg:?}",
                f.name
            );
        }
    }
}

/// Test-only writer mirroring [`G1Flag::get`]: the pair is what makes the drive
/// above non-vacuous (a row whose reader and writer disagree fails its own
/// `set_g1_flag` assertion). Kept next to the drive rather than in `G1Flag` so
/// the production table stays read-only.
fn set_g1_flag(c: &mut SolvableCase, name: &str) {
    match name {
        "compare_derived" => c.compare_derived = true,
        "compare_element_extras" => c.compare_element_extras = true,
        "compare_bus" => c.compare_bus = true,
        "compare_zsc" => c.compare_zsc = true,
        "compare_reliability" => c.compare_reliability = true,
        "compare_pdelements" => c.compare_pdelements = true,
        "compare_topology" => c.compare_topology = true,
        "compare_inc_matrix" => c.compare_inc_matrix = true,
        "compare_run_files" => c.compare_run_files = true,
        "compare_di" => c.compare_di = true,
        other => panic!("G1_SURFACE_FLAGS row {other:?} has no writer in `set_g1_flag`"),
    }
}

// ---------------------------------------------------------------------------
// GICTransformer decks gate on r4133 alone: dss_capi 0.14.5 is nondeterministic
// on them.
// ---------------------------------------------------------------------------

/// Every corpus deck that `new`s a `GICTransformer`, relative to
/// `tests/corpus/`, sorted.
///
/// Pinned as a closed set, not merely walked, because the per-case scan below
/// reads only the deck a case names: a GICTransformer hidden one `Redirect`
/// deeper would slip past it. A new deck here fails this test and forces the
/// channel review. The element makes the pinned dss_capi 0.14.5
/// **nondeterministic across processes** (some runs land the whole no-load
/// solve elsewhere and punt `Bus.kVBase` to 0), while EPRI r4133 is
/// deterministic, so such a deck may gate on `r4133` only.
const GICTRANSFORMER_DECKS: &[&str] = &[
    "asymmetric/gic/gic_midi.dss",
    "asymmetric/gic/gictransformer_gic.dss",
    "electricdss-tst/Version8/Distrib/Examples/GICExample/GIC_Example.dss",
    "modes/makeposseq/makeposseq_gic.dss",
];

/// `tests/corpus/` itself (the four manifests' two deck roots live under it).
fn corpus_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", "..", "tests", "corpus"]
        .iter()
        .collect()
}

/// Does this deck text create a `GICTransformer`?
///
/// Comment-stripped (`!` and `//` run to end of line, the DSS comment rules) so
/// a prose mention — `makeposseq_shunt.dss`'s header now carries one — is not an
/// instantiation, and `New` must be the verb: `edit`/`~` lines touch an element
/// that already exists and cannot introduce the nondeterminism.
fn deck_instantiates_a_gictransformer(text: &str) -> bool {
    for line in text.lines() {
        let code = line.split('!').next().unwrap_or_default();
        let code = code.split("//").next().unwrap_or_default();
        let lower = code.to_ascii_lowercase();
        let mut toks = lower.split_whitespace();
        while let Some(tok) = toks.next() {
            if tok != "new" {
                continue;
            }
            let Some(obj) = toks.next() else { continue };
            let obj = obj.trim_start_matches('"');
            if obj == "gictransformer" || obj.starts_with("gictransformer.") {
                return true;
            }
        }
    }
    false
}

/// Collect every `.dss` under `dir` that [`deck_instantiates_a_gictransformer`]
/// accepts, as forward-slashed paths relative to `base`.
fn collect_gictransformer_decks(dir: &Path, base: &Path, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
        let p = entry.expect("dir entry").path();
        if p.is_dir() {
            collect_gictransformer_decks(&p, base, out);
            continue;
        }
        if !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("dss")) {
            continue;
        }
        // Decks are ASCII/Latin-1 in practice; a non-UTF-8 byte must not hide a
        // GICTransformer, so read lossily instead of skipping the file.
        let text = String::from_utf8_lossy(
            &std::fs::read(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display())),
        )
        .into_owned();
        if deck_instantiates_a_gictransformer(&text) {
            out.push(
                p.strip_prefix(base)
                    .expect("under corpus root")
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}

/// The refusal itself, factored out so the negative drive can fire it without
/// mutating the corpus.
fn assert_gictransformer_case_gates_r4133(label: &str, engines: &str) {
    assert_eq!(
        engines, "r4133",
        "{label}: this deck instantiates a GICTransformer, so it must gate on the r4133 channel \
         ALONE (`engines: \"r4133\"`), not `{engines}`. The pinned dss_capi 0.14.5 \
         disagrees with ITSELF across fresh processes on such decks (some runs land the whole \
         no-load solve elsewhere and punt Bus.kVBase to 0), and a nondeterministic channel \
         cannot gate. r4133 is deterministic."
    );
}

/// **No capi-gated case instantiates a GICTransformer.**
///
/// Two rails in one test: the corpus-wide census pins WHICH decks carry the
/// element ([`GICTRANSFORMER_DECKS`]), and the manifest walk pins that every
/// gated case naming one of them declares `engines: "r4133"`.
#[test]
fn no_capi_gated_case_instantiates_a_gictransformer() {
    let root = corpus_root();
    let mut found: Vec<String> = Vec::new();
    collect_gictransformer_decks(&root, &root, &mut found);
    found.sort();
    assert_eq!(
        found, GICTRANSFORMER_DECKS,
        "the set of corpus decks that `new` a GICTransformer moved. Every one of them must gate \
         on the r4133 channel alone (capi 0.14.5 is nondeterministic on them). Update \
         GICTRANSFORMER_DECKS in the same commit that adds or removes one."
    );

    let mut checked: Vec<String> = Vec::new();
    for c in load_solvable() {
        let rel = format!("electricdss-tst/{}", c.path.replace('\\', "/"));
        if GICTRANSFORMER_DECKS.contains(&rel.as_str()) {
            let label = format!("solvable_now:{}", c.path);
            assert_gictransformer_case_gates_r4133(&label, &c.engines);
            checked.push(label);
        }
    }
    for fam in FAMILIES {
        for c in load_family(fam.name) {
            let rel = format!("{}/{}", fam.name, c.path.replace('\\', "/"));
            if GICTRANSFORMER_DECKS.contains(&rel.as_str()) {
                let label = format!("{}:{}", fam.name, c.path);
                assert_gictransformer_case_gates_r4133(&label, &c.engines);
                checked.push(label);
            }
        }
    }
    assert_eq!(
        checked.len(),
        GICTRANSFORMER_DECKS.len(),
        "every GICTransformer deck is a gated case exactly once; walked {checked:?}"
    );
}

/// Non-vacuity for the guard above: the scanner must separate a prose
/// mention from an instantiation, and the refusal must fire on both capi-gating
/// spellings.
#[test]
fn the_gictransformer_channel_guard_refuses_a_capi_gated_deck() {
    // The scanner, driven on a MUTATED COPY of a real deck (never written to
    // disk): `makeposseq_shunt.dss` mentions the element in prose, and must
    // read as clean until a `new` line is appended.
    let shunt =
        std::fs::read_to_string(corpus_root().join("modes/makeposseq/makeposseq_shunt.dss"))
            .expect("read makeposseq_shunt.dss");
    assert!(
        shunt.to_ascii_lowercase().contains("gictransformer"),
        "the drive is vacuous unless the deck still mentions the element in prose"
    );
    assert!(
        !deck_instantiates_a_gictransformer(&shunt),
        "a `!` comment mentioning a GICTransformer is not an instantiation"
    );
    let mutated =
        format!("{shunt}\nnew gictransformer.gt busH=b1 busNH=b1.4.4.4 R1=0.1 type=GSU\n");
    assert!(
        deck_instantiates_a_gictransformer(&mutated),
        "appending the `new` line must be seen"
    );
    for clean in [
        "// new gictransformer.gt busH=b1",
        "edit gictransformer.gt R1=0.2",
        "~ gictransformer=no",
    ] {
        assert!(
            !deck_instantiates_a_gictransformer(clean),
            "{clean:?} creates no GICTransformer"
        );
    }
    for created in [
        "New GICTransformer.tg1 busH=b1 R1=0.12 type=GSU",
        "  new \"gictransformer.gt\" busH=b1",
    ] {
        assert!(
            deck_instantiates_a_gictransformer(created),
            "{created:?} creates a GICTransformer"
        );
    }

    // The refusal: both capi-gating spellings must panic, `r4133` must not.
    for engines in ["capi_v0145", "both"] {
        let payload = std::panic::catch_unwind(|| {
            assert_gictransformer_case_gates_r4133("modes:mutated_gic.dss", engines)
        })
        .expect_err("a capi-gated GICTransformer case must be refused");
        let msg = crate::runner::panic_msg(payload);
        assert!(
            msg.contains("mutated_gic.dss")
                && msg.contains("r4133")
                && msg.contains("nondeterministic"),
            "the refusal must name the case, the channel and the reason; got {msg:?}"
        );
    }
    assert!(
        std::panic::catch_unwind(|| assert_gictransformer_case_gates_r4133("ok", "r4133")).is_ok(),
        "an r4133-gated GICTransformer case must be allowed"
    );
}

// ---------------------------------------------------------------------------
// Which decks enable demand-interval metering: the derivation behind
// `scheduler::DI_DECLARED_IN_MANIFEST`, so a new DI deck
// cannot arrive in the corpus without a `compare_di` declaration.
// ---------------------------------------------------------------------------

/// Does this deck text turn **demand-interval metering on**?
///
/// The option is `DemandInterval`, and the executive matches an option name by
/// **shortest unique prefix**: a LINEAR scan of the option list returning the
/// first entry whose leading characters match. The corpus uses both spellings:
/// `set demand=true` (`EPRITestCircuits/ckt5/Run_ckt5.dss`,
/// `IEEETestCases/123Bus/Run_YearlySim.dss`) and `DemandInterval=True/yes`
/// inside a multi-option `set` (`Examples/StoCtrl_Current_PeakShave/master.dss`,
/// `Examples/StoCtrl_SeasonTarget/Run_example.dss`).
///
/// Three characters is the shortest prefix that reaches this option: the option
/// list declares `Defaultdaily`, `Defaultyearly` and `DefaultBaseFrequency`
/// BEFORE it, so the linear scan resolves `de` and shorter to `Defaultdaily`. A
/// scanner that accepted them would read a default-loadshape line as
/// demand-interval metering.
///
/// Comment-stripped on the DSS rules (`!` to end of line anywhere, `//` when it
/// opens the statement) so the many decks that carry the commented-out demand
/// interval block (`IEEETestCases/LVTestCase/Master.dss` is the corpus's
/// example) read as what they are: off.
///
/// The VALUE is read exactly as the executive reads a yes/no option: the first
/// character answers TRUE only for `y` and `t`. `n`, `f` and **everything
/// else, `1` included**, are FALSE in both OpenDSS engines, so a deck spelled
/// `set demand=1` writes no DI tree and demanding a `compare_di` declaration
/// for it would pin an empty surface.
pub(crate) fn deck_enables_demand_interval(text: &str) -> bool {
    for line in text.lines() {
        for (name, value) in option_pairs(line) {
            if name.len() >= 3
                && "demandinterval".starts_with(&name)
                && matches!(value.as_bytes().first(), Some(b'y' | b't'))
            {
                return true;
            }
        }
    }
    false
}

/// The `name=value` pairs of one comment-stripped, lower-cased deck line.
///
/// The DSS parser takes `name=value`, `name =value`, `name= value` and
/// `name = value` alike (it splits on `=` after skipping white space), and a
/// `set` line may carry several of them
/// (`set mode=yearly DemandInterval=yes overloadreport=yes`), so the scan is
/// over pairs rather than over the line's text.
fn option_pairs(line: &str) -> Vec<(String, String)> {
    let code = line.split('!').next().unwrap_or_default();
    if code.trim_start().starts_with("//") {
        return Vec::new();
    }
    let lower = code.to_ascii_lowercase().replace(',', " ");
    // `a=b` → ["a", "=", "b"], so the four spacings above become one shape.
    let spaced = lower.replace('=', " = ");
    let toks: Vec<&str> = spaced.split_whitespace().collect();
    let mut out = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        if *t != "=" || i == 0 {
            continue;
        }
        let name = toks[i - 1].trim_matches(['"', '\'']).to_string();
        let value = toks
            .get(i + 1)
            .map(|v| v.trim_matches(['"', '\'']).to_string())
            .unwrap_or_default();
        out.push((name, value));
    }
    out
}

/// The `Redirect`/`Compile` closure of one deck, as `(path, text)` pairs.
///
/// A deck reaches its options through the files it pulls in, so "does this CASE
/// enable demand-interval metering" is a question about the whole closure, not
/// about the entry file. Relative paths resolve against the **including file's**
/// directory: the executive changes the current directory to the file it is
/// about to run and restores it afterwards for a redirect but not for a compile.
/// The distinction does not matter here — the walk resolves every name against
/// its own file's directory and visits every reachable file once — and a name
/// that does not resolve is skipped rather than guessed at, because this scan
/// answers a question about the files that DO exist.
fn deck_closure(entry: &Path) -> Vec<(PathBuf, String)> {
    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
    let mut stack = vec![entry.to_path_buf()];
    let mut out = Vec::new();
    while let Some(p) = stack.pop() {
        let Ok(canon) = p.canonicalize() else {
            continue;
        };
        if !canon.is_file() || !seen.insert(canon.clone()) {
            continue;
        }
        // Decks are ASCII/Latin-1 in practice; a stray non-UTF-8 byte must not
        // hide an option line, so read lossily instead of skipping the file.
        let text = String::from_utf8_lossy(
            &std::fs::read(&canon).unwrap_or_else(|e| panic!("read {}: {e}", canon.display())),
        )
        .into_owned();
        let dir = canon.parent().unwrap_or(Path::new(".")).to_path_buf();
        for line in text.lines() {
            if let Some(arg) = redirect_argument(line) {
                stack.push(dir.join(arg.replace('\\', "/")));
            }
        }
        out.push((canon, text));
    }
    out
}

/// The file name a `Redirect`/`Compile` line names, if it is one.
///
/// Both verbs are abbreviatable like every executive command (shortest
/// prefix, above), and the corpus writes the argument bare, `[bracketed]` or
/// `"quoted"` — `Compile [Master_ckt5.dss]`,
/// `Compile "IEEE123Master.dss"`, `Redirect Vsource.dss`. Four characters is
/// the shortest prefix accepted here: it is past every collision in the command
/// list and past the one-letter forms a prose line could produce.
fn redirect_argument(line: &str) -> Option<String> {
    let code = line.split('!').next().unwrap_or_default();
    let code = code.trim();
    if code.starts_with("//") {
        return None;
    }
    let (verb, rest) = code.split_once(char::is_whitespace)?;
    let verb = verb.to_ascii_lowercase();
    let is_redirect =
        verb.len() >= 4 && ("redirect".starts_with(&verb) || "compile".starts_with(&verb));
    if !is_redirect {
        return None;
    }
    let rest = rest.trim();
    let arg = match rest.chars().next()? {
        '[' => rest[1..].split(']').next()?.to_string(),
        '"' => rest[1..].split('"').next()?.to_string(),
        '\'' => rest[1..].split('\'').next()?.to_string(),
        _ => rest.split_whitespace().next()?.to_string(),
    };
    (!arg.is_empty()).then_some(arg)
}

/// Does this case's deck — or anything it `Redirect`s / `Compile`s — enable
/// demand-interval metering? The predicate behind
/// `scheduler::every_deck_that_enables_demand_interval_declares_compare_di`.
pub(crate) fn case_enables_demand_interval(entry: &str) -> bool {
    deck_closure(Path::new(entry))
        .iter()
        .any(|(_, text)| deck_enables_demand_interval(text))
}

/// The scanner reads the abbreviation the corpus actually uses, and refuses the
/// prefixes the executive resolves to a DIFFERENT option, every value the
/// executive reads as FALSE (an explicit `no`, and `1`/`0`) and both comment
/// forms.
#[test]
fn the_demand_interval_scanner_reads_the_option_and_its_abbreviation() {
    for on in [
        "set demand=true",
        "Set DemandInterval=True",
        "set mode=yearly DemandInterval=yes overloadreport=yes DIVerbose=yes",
        "set demand = true",
        "set demandinterval =yes",
        "  set dem=yes   ! the shortest prefix that reaches this option",
    ] {
        assert!(
            deck_enables_demand_interval(on),
            "{on:?} enables demand-interval metering"
        );
    }
    for off in [
        "set demand=false",
        "set demandinterval=no",
        "! set demand=true",
        "// set demand=true",
        "// ! Set \"DemandInterval\" to true so that energy quantities are recorded",
        // `de` and shorter resolve to `Defaultdaily`, which the linear prefix
        // scan reaches first.
        "set de=true",
        "set d=true",
        "set defaultdaily=true",
        "set defaultyearly=true",
        // A prose mention is not a setting.
        "! demand interval files are closed below",
        "closedi",
        // A yes/no option is TRUE for `y`/`t` only: `1`, `0` and every other
        // spelling are FALSE in BOTH OpenDSS engines, so a deck spelled this
        // way writes no DI tree and must not be required to declare `compare_di`.
        "set dem=1",
        "set demandinterval=0",
    ] {
        assert!(
            !deck_enables_demand_interval(off),
            "{off:?} does not enable demand-interval metering"
        );
    }
    // …and on the corpus's own decks, both ways round.
    let root = corpus_root().join("electricdss-tst/Version8/Distrib");
    for (deck, want) in [
        ("EPRITestCircuits/ckt5/Run_ckt5.dss", true),
        ("IEEETestCases/123Bus/Run_YearlySim.dss", true),
        ("Examples/StoCtrl_Current_PeakShave/master.dss", true),
        (
            "Examples/Scripts/Storage-Quasi-Static-Example/Run_Demo1.dss",
            false,
        ),
        ("IEEETestCases/LVTestCase/Master.dss", false),
    ] {
        let text = std::fs::read_to_string(root.join(deck)).expect("read corpus deck");
        assert_eq!(
            deck_enables_demand_interval(&text),
            want,
            "{deck}: demand-interval metering {}",
            if want { "is enabled" } else { "is not enabled" }
        );
    }
    // The closure half: `Run_example.dss` enables it in its own text, and the
    // `Redirect`ed `IEEE13NodecktMOD.dss` does not — so the walk must not smear
    // one case's option onto its sibling.
    let season = root.join("Examples/StoCtrl_SeasonTarget");
    assert!(case_enables_demand_interval(
        season.join("Run_example.dss").to_str().expect("utf-8 path")
    ));
    assert!(!case_enables_demand_interval(
        season
            .join("IEEE13NodecktMOD.dss")
            .to_str()
            .expect("utf-8 path")
    ));
    // …and the walk does reach a redirected file: the peak-shave master pulls
    // its whole model in by `Redirect`.
    let peakshave = root.join("Examples/StoCtrl_Current_PeakShave/master.dss");
    let closure = deck_closure(&peakshave);
    assert!(
        closure.len() > 10,
        "the peak-shave master redirects its model file by file; walked {} file(s)",
        closure.len()
    );
    assert!(
        redirect_argument("Compile [Master_ckt5.dss]").as_deref() == Some("Master_ckt5.dss")
            && redirect_argument("Compile \"IEEE123Master.dss\"").as_deref()
                == Some("IEEE123Master.dss")
            && redirect_argument("redirect Vsource.dss").as_deref() == Some("Vsource.dss")
            && redirect_argument("// redirect Vsource.dss").is_none()
            && redirect_argument("new line.l1 bus1=a").is_none(),
        "the three argument spellings, the comment form and a non-redirect line"
    );
}

// ---------------------------------------------------------------------------
// A MERGING `Reduce` renames elements on r4133, so its deck cannot gate on
// r4133.
// ---------------------------------------------------------------------------

/// Every corpus deck whose `Reduce` runs a strategy that MERGES lines,
/// relative to `tests/corpus/`, sorted.
///
/// Pinned as a closed set for the reason [`GICTRANSFORMER_DECKS`] is: the
/// per-case scan below reads only the deck a case names, so a `Reduce` hidden
/// one `Redirect` deeper would slip past it. A new deck here fails this test and
/// forces the channel review. r4133's line merge renames the surviving object
/// in place without updating the circuit's device list, so activating it by
/// name finds nothing, leaves the active element where it was and the DDLL
/// silently captures ANOTHER element. Gating such a deck on r4133 would ledger
/// a mis-addressed capture as if it were a divergence. This OpenDSS bug has an
/// OpenDSS report. capi 0.14.5 does not share it, so these decks gate on
/// `capi_v0145` alone until OpenDSS fixes it.
const REDUCE_MERGE_DECKS: &[&str] = &[
    "modes/reduce/midi_reduce.dss",
    "modes/reduce/reduce_default.dss",
    "modes/reduce/reduce_keeplist.dss",
    "modes/reduce/reduce_mergeparallel.dss",
    "modes/reduce/reduce_shortlines.dss",
    "modes/reduce/reduce_switches.dss",
];

/// Does this `Set ReduceOption=` value select a strategy that merges lines
/// (and therefore renames)?
///
/// r4133 decides on the option's FIRST LETTER only. The merging strategies are
/// shortlines, mergeparallel, switches and the default, i.e. `D`, `M`, `S`
/// (both `SWITCH…` and `SHORTLINES`). `B` (breakloop), `E` (ends / dangling)
/// and `L` (laterals) disable or re-parent branches and never merge. An EMPTY
/// or UNKNOWN option is merging too: r4133 sets the default strategy before it
/// reads the option and only warns on an unknown one, and a circuit that was
/// never given the option starts at the default, so a bare `Reduce` merges.
fn reduce_strategy_merges(option: &str) -> bool {
    !matches!(
        option.trim().to_ascii_uppercase().chars().next(),
        Some('B') | Some('E') | Some('L')
    )
}

/// Does this deck text issue a `Reduce` that merges?
///
/// Comment-stripped (`!` and `//`, the DSS comment rules) so a prose mention is
/// not a command, and the verb must be the statement's FIRST token — the
/// `reduce=y` of a `LineGeometry` (Kron reduction, a property on a `~`
/// continuation) is a different token in a different position and is never a
/// `Reduce` command.
fn deck_reduces_by_merging(text: &str) -> bool {
    let mut option = String::new(); // never set ⇒ rsDefault ⇒ merging
    for line in text.lines() {
        let code = line.split('!').next().unwrap_or_default();
        let code = code.split("//").next().unwrap_or_default();
        let lower = code.to_ascii_lowercase();
        let mut toks = lower.split_whitespace();
        let Some(first) = toks.next() else { continue };
        if first == "set" || first == "~" {
            // `set reduceoption=ends`, and the spaced `set reduceoption = ends`.
            let rest: Vec<&str> = toks.collect();
            let joined = rest.join(" ").replace(" = ", "=").replace(" =", "=");
            for tok in joined.split_whitespace() {
                if let Some(v) = tok.strip_prefix("reduceoption=") {
                    option = v.trim_matches('"').to_string();
                }
            }
            continue;
        }
        if first == "reduce" && reduce_strategy_merges(&option) {
            return true;
        }
    }
    false
}

/// Collect every `.dss` under `dir` that [`deck_reduces_by_merging`] accepts,
/// as forward-slashed paths relative to `base`.
fn collect_reduce_merge_decks(dir: &Path, base: &Path, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
        let p = entry.expect("dir entry").path();
        if p.is_dir() {
            collect_reduce_merge_decks(&p, base, out);
            continue;
        }
        if !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("dss")) {
            continue;
        }
        let text = String::from_utf8_lossy(
            &std::fs::read(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display())),
        )
        .into_owned();
        if deck_reduces_by_merging(&text) {
            out.push(
                p.strip_prefix(base)
                    .expect("under corpus root")
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}

/// The refusal itself, factored out so the negative drive can fire it without
/// mutating the corpus.
fn assert_reduce_merge_case_gates_capi(label: &str, engines: &str) {
    assert_eq!(
        engines, "capi_v0145",
        "{label}: this deck's `Reduce` MERGES lines, and r4133's line merge renames the \
         surviving object without updating the circuit's device list, so every later activation \
         of a merged line by name silently leaves the PREVIOUS element active and the DDLL \
         captures another element. Such a deck must gate on `capi_v0145` ALONE, not `{engines}`, \
         until OpenDSS fixes the rename bug: otherwise the ledger would carry a mis-addressed \
         capture as if it were a divergence."
    );
}

/// **No r4133-gated case reduces by merging.**
///
/// Two rails in one test, the [`no_capi_gated_case_instantiates_a_gictransformer`]
/// shape: the corpus-wide census pins WHICH decks merge
/// ([`REDUCE_MERGE_DECKS`]), and the manifest walk pins that every gated case
/// naming one of them declares `engines: "capi_v0145"`.
#[test]
fn no_r4133_gated_case_reduces_by_merging() {
    let root = corpus_root();
    let mut found: Vec<String> = Vec::new();
    collect_reduce_merge_decks(&root, &root, &mut found);
    found.sort();
    assert_eq!(
        found, REDUCE_MERGE_DECKS,
        "the set of corpus decks whose `Reduce` merges lines moved. Every one of them must gate on \
         capi_v0145 alone (r4133's line-merge rename leaves its device list stale). Update \
         REDUCE_MERGE_DECKS in the same commit that adds or removes one."
    );

    let mut checked: Vec<String> = Vec::new();
    for c in load_solvable() {
        let rel = format!("electricdss-tst/{}", c.path.replace('\\', "/"));
        if REDUCE_MERGE_DECKS.contains(&rel.as_str()) {
            let label = format!("solvable_now:{}", c.path);
            assert_reduce_merge_case_gates_capi(&label, &c.engines);
            checked.push(label);
        }
    }
    for fam in FAMILIES {
        for c in load_family(fam.name) {
            let rel = format!("{}/{}", fam.name, c.path.replace('\\', "/"));
            if REDUCE_MERGE_DECKS.contains(&rel.as_str()) {
                let label = format!("{}:{}", fam.name, c.path);
                assert_reduce_merge_case_gates_capi(&label, &c.engines);
                checked.push(label);
            }
        }
    }
    assert_eq!(
        checked.len(),
        REDUCE_MERGE_DECKS.len(),
        "every merging-reduce deck is a gated case exactly once; walked {checked:?}"
    );
}

/// Non-vacuity for the guard above: the scanner must separate a
/// merging strategy from a non-merging one, a command from a property, and the
/// refusal must fire on both r4133-gating spellings.
#[test]
fn the_reduce_merge_channel_guard_refuses_an_r4133_gated_deck() {
    // The strategy rule, letter by letter.
    for merging in [
        "default",
        "d",
        "mergeparallel",
        "shortlines",
        "switch",
        "switches",
    ] {
        assert!(
            reduce_strategy_merges(merging),
            "{merging:?} calls MergeWith"
        );
    }
    for keeps in ["breakloop", "ends", "laterals", "Ends", "LATERALS"] {
        assert!(!reduce_strategy_merges(keeps), "{keeps:?} never merges");
    }
    // Empty and unknown leave `rsDefault` in place — merging.
    assert!(reduce_strategy_merges(""));
    assert!(reduce_strategy_merges("zzz"));

    // The scanner, driven on real deck text (never written to disk).
    let merging = std::fs::read_to_string(corpus_root().join("modes/reduce/reduce_default.dss"))
        .expect("read reduce_default.dss");
    assert!(deck_reduces_by_merging(&merging));
    let breakloop =
        std::fs::read_to_string(corpus_root().join("modes/reduce/reduce_breakloop.dss"))
            .expect("read reduce_breakloop.dss");
    assert!(
        breakloop.to_ascii_lowercase().contains("reduce"),
        "the drive is vacuous unless the deck really issues a Reduce"
    );
    assert!(
        !deck_reduces_by_merging(&breakloop),
        "breakloop opens a loop; it never calls MergeWith"
    );
    // A bare `Reduce` with no option merges (the circuit starts at rsDefault).
    assert!(deck_reduces_by_merging("new circuit.x\nreduce\n"));
    // …and the option is honoured whichever spacing the deck uses.
    assert!(!deck_reduces_by_merging(
        "set reduceoption = laterals\nReduce\n"
    ));
    for clean in [
        "// reduce",
        "! set reduceoption=default\n! reduce",
        "~ reduce=y",
        "new linegeometry.g1 nconds=4 reduce=y",
    ] {
        assert!(
            !deck_reduces_by_merging(clean),
            "{clean:?} issues no Reduce command"
        );
    }

    // The refusal: both r4133-gating spellings must panic, `capi_v0145` must not.
    for engines in ["r4133", "both"] {
        let payload = std::panic::catch_unwind(|| {
            assert_reduce_merge_case_gates_capi("modes:mutated_reduce.dss", engines)
        })
        .expect_err("an r4133-gated merging-reduce case must be refused");
        let msg = crate::runner::panic_msg(payload);
        assert!(
            msg.contains("mutated_reduce.dss")
                && msg.contains("capi_v0145")
                && msg.contains("rename bug"),
            "the refusal must name the case, the channel and the reason; got {msg:?}"
        );
    }
    assert!(
        std::panic::catch_unwind(|| assert_reduce_merge_case_gates_capi("ok", "capi_v0145"))
            .is_ok(),
        "a capi-gated merging-reduce case must be allowed"
    );
}

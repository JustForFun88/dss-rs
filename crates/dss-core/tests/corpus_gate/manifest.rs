//! Manifest schema + loading for the unified corpus gate.
//!
//! Schema **v2** (`UNIFIED_GATE_PLAN.md` §1.2 / Phase C): the retired `oracle`
//! target-rev field is replaced by `engines: "capi_v0145" | "r4133" | "both"`
//! (default `"both"`) naming the gating channel(s). `capi_v0145` = the pinned
//! dss-python 0.15.7 / dss_capi 0.14.5 oracle (`oracle_server.py`, persistent
//! pool); `r4133` = the official EPRI `OpenDSSDirect.dll` via the in-house
//! `epri-worker` bridge. `ORACLE_SPECS` is gone. `isolate` (throwaway one-shot
//! worker; §1.2/§4 Phase B) and `note` (read to enforce `isolate ⇒ note`) remain.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::harness;

// ---------------------------------------------------------------------------
// Manifest structs (schema unchanged; `isolate`/`note` additive).
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub(crate) struct SolvableManifest {
    #[serde(default)]
    pub(crate) cases: Vec<SolvableCase>,
}

/// One element-specific property-probe spec: compare `element`'s listed
/// property values (oracle `Properties(p).Val` vs the Rust `?` query).
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ProbeSpec {
    pub(crate) element: String,
    pub(crate) props: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct SolvableCase {
    pub(crate) path: String,
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
    /// Element-specific state channels (CONTROL_COVERAGE_PLAN.md), all opt-in:
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
    /// WP8.5b corpus property parity: compare EVERY element's EVERY property
    /// value (Rust `?`-surface vs oracle `Properties(p).Val`) per step, on top
    /// of the full-model compare. Off by default (heavy); flipped `true` only on
    /// families proven fully clean by the `corpus_live_properties` pilot triage.
    #[serde(default)]
    pub(crate) compare_all_properties: bool,
    /// WPG.5: compare `DSS.GlobalResult` (`Text.Result`) per step — the AutoAdd
    /// winner + improvement figure. Tokenized via `compare_export`.
    #[serde(default)]
    pub(crate) compare_global_result: bool,
    /// WPG.5: compare the `<CircuitName_>AutoAddLog.csv` the AutoAdd solve writes.
    #[serde(default)]
    pub(crate) compare_autoadd_log: bool,
    // -- WP-G1 compare-depth surface flags (`GOLDEN_REBASE_PLAN.md` §1.1(d),
    //    sub-step G1.0). Declared here ONCE for the whole work package so the
    //    population lock's rigor fingerprint gains its ten tokens in a single
    //    regen instead of one per surface sub-step (a flag absent from
    //    `population_lock.rs::rigor()` is invisible to the anti-shrink guard).
    //    Every one is `#[serde(default)]` ⇒ `false`, and none may be set by a
    //    manifest before its sub-step wires the request + comparator — see
    //    [`G1_SURFACE_FLAGS`] and `no_unwired_g1_surface_flag_is_set_in_any_manifest`.
    /// G1.3a–c: the per-element **derived** channels of the fastdss facade —
    /// `VoltagesMagAng`/`CurrentsMagAng`/`Residuals`, `SeqVoltages`/`SeqCurrents`/
    /// `SeqPowers`, `CplxSeqVoltages`/`CplxSeqCurrents`, `TotalPowers`
    /// (`.inputs/DSS-Python` `origin/fastdss` `dss/ICktElement.py:53,58-69`
    /// `_columns`; r4133 transport `DDLL/DCktElement.pas` `CktElementV`).
    #[serde(default)]
    pub(crate) compare_derived: bool,
    /// G1.3d: the per-element **discrete extras** — `PhaseLosses`, `NodeOrder`,
    /// `EnergyMeter`, `OCPDevType`/`OCPDevIndex`, `HasVoltControl`/
    /// `HasSwitchControl`, `NumControls`, `NumTerminals`/`NumPhases`/
    /// `NumConductors` (`origin/fastdss` `dss/ICktElement.py:35-38,46-52,56,69`;
    /// r4133 `DDLL/DCktElement.pas` `CktElementI`/`CktElementS`).
    #[serde(default)]
    pub(crate) compare_element_extras: bool,
    /// G1.4: the **bus** surface — `origin/fastdss` `dss/IBus.py:19-53`
    /// `_columns`; r4133 `DDLL/DBus.pas`, capi `CAPI/CAPI_Alt.pas`.
    ///
    /// **Wired by G1.4a (2026-09-04, settlement D8):** `puVoltages`
    /// (`DBus.pas:399-430` == `CAPI_Alt.pas:2251-2280`), `VMagAngle`
    /// (`:659-689` == `:2573-2597`), `puVmagAngle` (`:690-723` == `:2540-2571`),
    /// the per-bus `Nodes`/`kVBase` (`:319-345` == `:2143-2163`) and the
    /// circuit-level `AllBusVmagPu` (`DCircuit.pas:481-500` ==
    /// `CAPI_Circuit.pas:521-548`) — the arms both oracles run identically.
    /// **Still to come on this same flag:** `SeqVoltages`/`CplxSeqVoltages` and
    /// `VLL`/`puVLL` (G1.4c — the channels disagree there), `AllPCEatBus`/
    /// `AllPDEatBus`/`Distance` + `AllBusDistances`/`AllNodeDistances` (G1.4b).
    #[serde(default)]
    pub(crate) compare_bus: bool,
    /// G1.5: the bus **short-circuit** surface — `Zsc1`/`Zsc0`/`ZscMatrix`/
    /// `YscMatrix`/`Isc`/`Voc` (`origin/fastdss` `dss/IBus.py:30,39,41-44`;
    /// r4133 `DDLL/DBus.pas`). Split from [`Self::compare_bus`] because it needs
    /// a fault study, not just a solve.
    #[serde(default)]
    pub(crate) compare_zsc: bool,
    /// G1.6: the **meter extras + per-bus reliability** surface — `CalcCurrent`,
    /// `AllocFactors`, `SAIFI`/`SAIFIKW`/`SAIDI`/`CustInterrupts`, the ordered
    /// zone vectors and the active-section fields (`origin/fastdss`
    /// `dss/IMeters.py:13-42` `_columns`), plus `Bus.Lambda`/`N_interrupts`/
    /// `N_Customers`/`Cust_Interrupts`/`Cust_Duration`/`Int_Duration`/
    /// `TotalMiles`/`SectionID` (`dss/IBus.py:25-36`). Also drives the executive
    /// `RelCalc` the surface needs (`save_outputs.py:117-129`); r4133
    /// `DDLL/DMeters.pas`.
    #[serde(default)]
    pub(crate) compare_reliability: bool,
    /// G1.6b: the **PDElements** interface walk — `AccumulatedL`,
    /// `ParentPDElement`, `FromTerminal`, `IsShunt`, `Numcustomers`, `SectionID`,
    /// `RepairTime`, `Totalcustomers`, `Lambda` (`origin/fastdss`
    /// `dss/IPDElements.py:26-40` `_columns`; r4133 `DDLL/DPDELements.pas`).
    #[serde(default)]
    pub(crate) compare_pdelements: bool,
    /// G1.7: the **topology** interface — `NumLoops`, `NumIsolatedBranches`/
    /// `NumIsolatedLoads`, `AllLoopedPairs`, `AllIsolatedBranches`/
    /// `AllIsolatedLoads` (`origin/fastdss` `dss/ITopology.py:10-20` `_columns`;
    /// r4133 `DDLL/DTopology.pas`).
    #[serde(default)]
    pub(crate) compare_topology: bool,
    /// G1.8: the **incidence-matrix** surface — `IncMatrix`, `IncMatrixCols`,
    /// `IncMatrixRows`, `Laplacian`: the fastdss-branch-only Solution additions
    /// (`origin/fastdss` `tests/save_outputs.py:187`, fed by the `CalcIncMatrix`
    /// at `:117`; r4133 `DDLL/DSolution.pas`).
    #[serde(default)]
    pub(crate) compare_inc_matrix: bool,
    /// G1.10a: the **run-produced file set** — every non-monitor `*.csv` the deck
    /// emits under DataPath plus the `save circuit` output file set
    /// (`origin/fastdss` `tests/save_outputs.py:597-609`, which archives every
    /// `*.csv` under the run dir; the `.dss` set is archived but never compared —
    /// our file-set + round-trip check is strictly stronger).
    #[serde(default)]
    pub(crate) compare_run_files: bool,
    /// G1.10b: the **demand-interval tree** — the `closedi` subset of the CSV set
    /// above (`origin/fastdss` `tests/save_outputs.py:64,597`), split off because
    /// a DI tree exists only for a deck that runs `closedi`.
    #[serde(default)]
    pub(crate) compare_di: bool,
    /// The feature this case covers is not ported yet (GAPS_PLAN.md §2.3/§3.1):
    /// the gate asserts the Rust engine errors loudly instead of live-comparing.
    #[serde(default)]
    pub(crate) pending: bool,
    /// This deck aborts the solve on BOTH engines; the value is the error
    /// substring both must produce (see the original `corpus_live.rs` field doc:
    /// malformed input → Pascal `DSS.SolutionAbort`, or control non-settling →
    /// `#485 Max Control Iterations Exceeded`). Mutually exclusive with `pending`.
    #[serde(default)]
    pub(crate) expect_solve_abort: Option<String>,
    /// Work package that ports this case's feature. Mandatory while `pending`
    /// or `defer_ledger`.
    #[serde(default)]
    pub(crate) wp: Option<String>,
    /// Schema v2 (§4 Phase C ladder step c / §5 R9): this case's validated
    /// behavior was pinned against the **retired capi015 (0.15.0b4)** engine line
    /// and reproduces on NEITHER surviving channel (0.14.5 lacks the feature or
    /// differs; r4133-11.0 diverges). It is PARKED from live oracle comparison
    /// pending its **Phase D ledger** entry (which will re-gate it against r4133
    /// / capi_v0145 within a pinned envelope). The value is the Phase-D-ledger
    /// seed cause (mandatory); `wp` names the follow-up. The Rust engine is still
    /// smoke-run (compile + solve, must converge with no new errors) so a Rust
    /// *convergence/error* regression never hides here — but a *numeric* regression
    /// that still converges is NOT caught (no physical value is compared; that
    /// coverage returns with the Phase D ledger). Membership is preserved — this
    /// NEVER drops a case (the population lock records the `defer` flag). Mutually
    /// exclusive with `pending` / `expect_solve_abort`.
    #[serde(default)]
    pub(crate) defer_ledger: Option<String>,
    /// Non-fatal warnings this deck's compile deliberately produces, which the
    /// port reproduces 1:1 (CF-C Port 2: a user-written model DLL that safe Rust
    /// cannot load). Each string is a substring an actual engine error must
    /// contain; every actual error must match one, and every listed substring
    /// must actually appear. Empty ⇒ zero errors are tolerated.
    #[serde(default)]
    pub(crate) expect_warnings: Vec<String>,
    /// Schema v2 (UNIFIED_GATE_PLAN.md §1.2): which live channel(s) gate this
    /// case. `"capi_v0145"` = the pinned dss-python 0.15.7 / dss_capi 0.14.5
    /// oracle (persistent `oracle_server.py` pool); `"r4133"` = the official
    /// EPRI `OpenDSSDirect.dll` r4133 via the in-house `epri-worker` bridge;
    /// `"both"` (the default) gates on both channels (Phase D). Replaces the
    /// retired `oracle` target-rev field.
    #[serde(default = "default_engines")]
    pub(crate) engines: String,
    /// WP-AD.4 A-Diakoptics disposition (mandatory on every family-manifest case).
    #[serde(default)]
    pub(crate) ad: Option<String>,
    /// UNIFIED_GATE Phase B (§1.2/§4): run every engine execution of this case on
    /// a throwaway one-shot worker process instead of a persistent pool worker.
    /// Seeded only for a PROVEN worker-state-contamination case (the AutoAdd
    /// exit-segfault deck is the expected seed); must carry a `note`. Default
    /// false — absent in every manifest until the contamination proof demands it.
    #[serde(default)]
    pub(crate) isolate: bool,
    /// Free-text provenance. Read (no longer silently dropped) only to enforce
    /// the `isolate ⇒ note` structural rule; otherwise ignored by the gate.
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

/// The three legal `engines` channel selectors (schema v2, §1.2).
pub(crate) const ENGINES_SPECS: &[&str] = &["capi_v0145", "r4133", "both"];

/// One live gating channel a case is compared against.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum EngineChannel {
    /// Pinned dss-python 0.15.7 / dss_capi 0.14.5 via `oracle_server.py`.
    CapiV0145,
    /// Official EPRI `OpenDSSDirect.dll` r4133 via `epri-worker`.
    R4133,
}

impl EngineChannel {
    /// The eventlog-mask spec key for this channel (`harness::compare_eventlog`):
    /// the pinned oracle needs no masking (`None`); r4133 selects its per-rev row.
    pub(crate) fn eventlog_spec(self) -> Option<&'static str> {
        match self {
            EngineChannel::CapiV0145 => None,
            EngineChannel::R4133 => Some("r4133"),
        }
    }
    /// Iteration policy: the pinned oracle is an exact 1:1 contract; r4133 (a
    /// different engine line) allows the port to converge in FEWER iterations —
    /// never more (`rust_le_oracle`, the former target-rev precedent, §4 Phase C).
    pub(crate) fn iterations_exact(self) -> bool {
        matches!(self, EngineChannel::CapiV0145)
    }
    /// This channel as the **harness's** own channel type.
    ///
    /// `EngineChannel` is `pub(crate)` to this one test binary while `harness/`
    /// compiles into ~20 others, so the property comparator cannot take it
    /// (`R4133_PROPS_PLAN.md` §1.2, the channel-threading trap). Every corpus_gate
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
                "{}: unknown engines spec {other:?} — expected one of {ENGINES_SPECS:?} (§1.2)",
                self.path
            ),
        }
    }
    /// Does this case gate the pinned dss_capi 0.14.5 channel?
    ///
    /// **No longer a property-forcing predicate.** It used to select the cases
    /// whose property table was compared (§1.2 kept `compare_all_properties` on
    /// the capi_v0145 channel only); R4133_PROPS RP4.1 (2026-09-03) unmasked the
    /// r4133 channel and [`crate::scheduler::force_properties`] now forces
    /// properties on every live non-`large` case regardless of `engines`. What is
    /// left is the `DSS_LIVE_PROPS` pilot in `corpus_gate.rs`, which drives the
    /// pinned oracle directly and therefore still has to skip r4133-only cases.
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
    /// WP8.5b: gate EVERY element's EVERY property value on every live case in
    /// this family (only vs the pinned capi oracle).
    pub(crate) compare_all_properties: bool,
}

/// Oracle-free structural guard shared by the families: deck dir ↔ manifest
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
                "{}: pending case must name the WP that ports it (GAPS_PLAN.md §3.1)",
                c.path
            );
        }
        assert_defer_ledger_is_valid(&format!("{}:{}", fam.name, c.path), c);
        assert!(
            ENGINES_SPECS.contains(&c.engines.as_str()),
            "{}: unknown engines spec {:?} — expected one of {ENGINES_SPECS:?} (§1.2)",
            c.path,
            c.engines
        );
        assert_isolate_carries_note(&format!("{}:{}", fam.name, c.path), c);
        let ad = c.ad.as_deref().unwrap_or_else(|| {
            panic!(
                "{}:{}: missing mandatory `ad` disposition (WP-AD.4 — one of \
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

/// Schema v2 structural rule (§4 Phase C step c): a `defer_ledger` case carries a
/// non-empty cause + a `wp`, and never also claims `pending`/`expect_solve_abort`.
pub(crate) fn assert_defer_ledger_is_valid(label: &str, c: &SolvableCase) {
    if let Some(cause) = &c.defer_ledger {
        assert!(
            !cause.trim().is_empty(),
            "{label}: `defer_ledger` requires a non-empty Phase-D-ledger-seed cause (§4 Phase C)"
        );
        assert!(
            c.wp.is_some(),
            "{label}: `defer_ledger` requires a `wp` naming the Phase D follow-up"
        );
        assert!(
            !c.pending && c.expect_solve_abort.is_none(),
            "{label}: `defer_ledger` is mutually exclusive with `pending`/`expect_solve_abort`"
        );
    }
}

/// UNIFIED_GATE Phase B structural rule (§1.2): an `isolate: true` case must
/// carry a `note` documenting the proven contamination that justifies it.
pub(crate) fn assert_isolate_carries_note(label: &str, c: &SolvableCase) {
    if c.isolate {
        assert!(
            c.note.as_deref().is_some_and(|n| !n.trim().is_empty()),
            "{label}: `isolate: true` requires a non-empty `note` documenting the \
             proven worker-state contamination (UNIFIED_GATE_PLAN §1.2/§4 Phase B)"
        );
    }
}

// ---------------------------------------------------------------------------
// WP-G1 compare-depth surface flags (`GOLDEN_REBASE_PLAN.md` §1.1(d), G1.0).
// ---------------------------------------------------------------------------

/// One WP-G1 compare-depth surface flag: the manifest field name, the sub-step
/// that owns it, whether its request + comparator are wired yet, and the reader
/// that pulls it off a case.
///
/// `wired` is the structural half of the rails. A flag whose surface is not
/// wired yet is a field the loader accepts and the runtime ignores: the request
/// builder would not send it, the oracle would return nothing, and the case
/// would "compare" an empty capture against an empty capture — green, silently
/// vacuous. Each surface sub-step flips its own row to `true` **in the same
/// commit** that adds the request field and the comparator (the
/// `pending ⇒ wp` / `isolate ⇒ note` structural-gate pattern of
/// [`family_manifest_is_complete`]).
pub(crate) struct G1Flag {
    /// The `serde` field name, exactly as a manifest spells it.
    pub(crate) name: &'static str,
    /// The `GOLDEN_REBASE_PLAN.md` sub-step that owns the surface.
    pub(crate) sub_step: &'static str,
    /// Is the capture request + comparator in the tree yet?
    pub(crate) wired: bool,
    pub(crate) get: fn(&SolvableCase) -> bool,
}

/// The whole WP-G1 flag vocabulary, declared once (G1.0). Order matches the
/// declaration order in [`SolvableCase`] and the token order in
/// `population_lock.rs::rigor()`.
pub(crate) const G1_SURFACE_FLAGS: &[G1Flag] = &[
    G1Flag {
        name: "compare_derived",
        sub_step: "G1.3a-c",
        wired: false,
        get: |c| c.compare_derived,
    },
    G1Flag {
        name: "compare_element_extras",
        sub_step: "G1.3d",
        wired: false,
        get: |c| c.compare_element_extras,
    },
    G1Flag {
        name: "compare_bus",
        sub_step: "G1.4",
        wired: true,
        get: |c| c.compare_bus,
    },
    G1Flag {
        name: "compare_zsc",
        sub_step: "G1.5",
        wired: true,
        get: |c| c.compare_zsc,
    },
    G1Flag {
        name: "compare_reliability",
        sub_step: "G1.6",
        wired: false,
        get: |c| c.compare_reliability,
    },
    G1Flag {
        name: "compare_pdelements",
        sub_step: "G1.6b",
        wired: false,
        get: |c| c.compare_pdelements,
    },
    G1Flag {
        name: "compare_topology",
        sub_step: "G1.7",
        wired: false,
        get: |c| c.compare_topology,
    },
    G1Flag {
        name: "compare_inc_matrix",
        sub_step: "G1.8",
        wired: false,
        get: |c| c.compare_inc_matrix,
    },
    G1Flag {
        name: "compare_run_files",
        sub_step: "G1.10a",
        wired: false,
        get: |c| c.compare_run_files,
    },
    G1Flag {
        name: "compare_di",
        sub_step: "G1.10b",
        wired: false,
        get: |c| c.compare_di,
    },
];

/// Refuse a manifest case that sets a [`G1_SURFACE_FLAGS`] flag whose surface is
/// not wired yet (G1.0 rails). Shared by the manifest walk below and its
/// synthetic non-vacuity drive.
pub(crate) fn assert_no_unwired_g1_flag(label: &str, c: &SolvableCase) {
    for f in G1_SURFACE_FLAGS {
        assert!(
            !((f.get)(c) && !f.wired),
            "{label}: sets `{}`, but that surface is not wired yet ({} owns it). \
             The request builder would not send it and the comparator does not \
             exist, so the case would compare an empty capture against an empty \
             capture and pass. Flip `G1Flag::wired` in the SAME commit that adds \
             the capture request + the comparator (GOLDEN_REBASE_PLAN.md §1.1(d)/(f)).",
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
    // R4133_PROPS RP1.2: the only deck in the corpus that drives an AutoTrans
    // from an `XfmrCode` (r4133 property 39).
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
    // OG-1.7 orphaned-gaps: UPFC modes 2 (StatCOM) / 3 (Dual) / 5 (DoubleRef Dual).
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
    // GOLDEN_REBASE G1.4a (D12/D14, 2026-09-04): the GICTransformer arm of the
    // shunt deck, split into its own `engines: "r4133"` case because capi 0.14.5
    // is nondeterministic on any deck that instantiates a GICTransformer.
    "makeposseq/makeposseq_gic.dss",
    "makeposseq/makeposseq_pc.dss",
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
    // GOLDEN_REBASE G1.5 (2026-09-05, lane `lane-b`): the micro-tier witness
    // for the bus short-circuit surface. The corpus' four vendored
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
    // and does not converge upstream either, so there is nothing to compare AD
    // against and nothing to fix in the port. Distinct from `ad-nonconvergent`,
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
// Oracle-free structural tests (fast; no engine spawned).
// ---------------------------------------------------------------------------

#[test]
fn solvable_now_engines_specs_are_valid() {
    for c in load_solvable() {
        assert!(
            ENGINES_SPECS.contains(&c.engines.as_str()),
            "{}: unknown engines spec {:?} — expected one of {ENGINES_SPECS:?} (§1.2)",
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

/// G1.0 rails: no manifest may switch on a WP-G1 surface flag before the
/// sub-step that owns it has wired the capture request and the comparator.
///
/// Walks all four manifests (`solvable_now` + the three synthetic families), so
/// the moment a surface sub-step's flag appears in a deck's JSON without its
/// `G1Flag::wired` flip, the gate names the case and the flag.
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

/// Non-vacuity for the rail above (§1.1(f)): the manifests set none of the ten
/// flags today, so the walk passes on an empty premise. Drive each flag on a
/// synthetic case and assert the refusal actually fires — and that a wired flag
/// (simulated by reading the row's own `wired`) is the only thing that lets one
/// through.
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
                "{}: refusal must name the flag, its sub-step and the case; got {msg:?}",
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
// GICTransformer decks gate on r4133 alone (GOLDEN_REBASE G1.4a, coordinator
// decisions D12/D14, 2026-09-04 — see `DIVERGENCES.md` and `TESTING.md`).
// ---------------------------------------------------------------------------

/// Every corpus deck that `new`s a `GICTransformer`, relative to
/// `tests/corpus/`, sorted.
///
/// Pinned as a closed set, not merely walked, because the per-case scan below
/// reads only the deck a case names: a GICTransformer hidden one `Redirect`
/// deeper would slip past it. A new deck here fails this test and forces the
/// channel review — the element makes the pinned dss_capi 0.14.5 oracle
/// **nondeterministic across processes** (7 bad runs of 60 with one in the deck,
/// 0 of 40 without; the whole no-load solve lands elsewhere and `Bus.kVBase` is
/// punted to 0), while EPRI r4133 is deterministic (80/80 bit-identical), so
/// such a deck may gate on `r4133` only.
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
         ALONE (`engines: \"r4133\"`), not `{engines}`. The pinned dss_capi 0.14.5 oracle \
         disagrees with ITSELF across fresh processes on such decks (7 bad runs of 60 with the \
         element, 0 of 40 without — the whole no-load solve lands elsewhere and Bus.kVBase is \
         punted to 0), and an oracle channel that disagrees with itself cannot gate; r4133 is \
         deterministic (80/80). GOLDEN_REBASE G1.4a, coordinator decisions D12/D14 — see \
         DIVERGENCES.md."
    );
}

/// **No capi-gated case instantiates a GICTransformer** (D12/D14).
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
         on the r4133 channel alone (capi 0.14.5 is nondeterministic on them — D12/D14); update \
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

/// Non-vacuity for the guard above (§1.1(f)): the scanner must separate a prose
/// mention from an instantiation, and the refusal must fire on both capi-gating
/// spellings.
#[test]
fn the_gictransformer_channel_guard_refuses_a_capi_gated_deck() {
    // The scanner, driven on a MUTATED COPY of a real deck (never written to
    // disk): `makeposseq_shunt.dss` mentions the element in prose since G1.4a
    // split it out, and must read as clean until a `new` line is appended.
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
            msg.contains("mutated_gic.dss") && msg.contains("r4133") && msg.contains("D12/D14"),
            "the refusal must name the case, the channel and the decision; got {msg:?}"
        );
    }
    assert!(
        std::panic::catch_unwind(|| assert_gictransformer_case_gates_r4133("ok", "r4133")).is_ok(),
        "an r4133-gated GICTransformer case must be allowed"
    );
}

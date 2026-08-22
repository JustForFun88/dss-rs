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
    /// The single primary channel for weighting / property-forcing decisions
    /// (a `"both"` case is capi-forced for property parity; §1.2 keeps
    /// `compare_all_properties` on the capi_v0145 channel only).
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

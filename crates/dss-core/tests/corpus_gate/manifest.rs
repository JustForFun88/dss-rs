//! Manifest schema + loading for the unified corpus gate.
//!
//! Schema is UNCHANGED from the pre-Phase-B `corpus_live.rs` (`UNIFIED_GATE_PLAN.md`
//! Phase B): the `oracle` target-rev field is still honored and `ORACLE_SPECS`
//! stays. The only additive fields are `isolate` (the one permitted early Phase-B
//! addition — run a case on a throwaway one-shot worker; §1.2/§4 Phase B) and
//! `note` (read only to enforce `isolate ⇒ note`). Schema v2 (`engines`) is Phase C.

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
    /// Work package that ports this case's feature. Mandatory while `pending`.
    #[serde(default)]
    pub(crate) wp: Option<String>,
    /// Non-fatal warnings this deck's compile deliberately produces, which the
    /// port reproduces 1:1 (CF-C Port 2: a user-written model DLL that safe Rust
    /// cannot load). Each string is a substring an actual engine error must
    /// contain; every actual error must match one, and every listed substring
    /// must actually appear. Empty ⇒ zero errors are tolerated.
    #[serde(default)]
    pub(crate) expect_warnings: Vec<String>,
    /// Target oracle for this case's live compare (UPGRADE_PLAN.md): absent =
    /// the pinned dss-python 0.15.7 / dss_capi 0.14.5 oracle; `"capi015"` =
    /// the dss_capi 0.15.x-line oracle; `"r3723"|"r4088"|"r4133"` = an
    /// official EPRI `OpenDSSDirect.dll` via the Oddie bridge.
    #[serde(default)]
    pub(crate) oracle: Option<String>,
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

/// Legal manifest `oracle` values for target-rev cases (UPGRADE_PLAN.md):
/// the dss_capi 0.15.x-line oracle plus the three vendored EPRI revisions.
/// `None`/absent = the pinned capi oracle.
pub(crate) const ORACLE_SPECS: &[&str] = &["capi015", "r3723", "r4088", "r4133"];

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
/// valid `ad`/`oracle`/`isolate` fields, and the family's per-case invariant.
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
        if let Some(spec) = &c.oracle {
            assert!(
                ORACLE_SPECS.contains(&spec.as_str()),
                "{}: unknown oracle spec {spec:?} — expected one of {ORACLE_SPECS:?} \
                 (UPGRADE_PLAN.md target-rev gating)",
                c.path
            );
        }
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
fn solvable_now_oracle_specs_are_valid() {
    for c in load_solvable() {
        if let Some(spec) = &c.oracle {
            assert!(
                ORACLE_SPECS.contains(&spec.as_str()),
                "{}: unknown oracle spec {spec:?} — expected one of {ORACLE_SPECS:?} \
                 (UPGRADE_PLAN.md target-rev gating)",
                c.path
            );
        }
        assert_isolate_carries_note(&format!("solvable_now:{}", c.path), &c);
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

use super::*;
use crate::elements::pc::storage::{StorageDispatchMode, StorageState};
use crate::elements::traits::ElemId;
use crate::obj::base::DssObject;
use crate::obj::props::PropType;
use crate::solution::SolveMode;

#[test]
fn default_shape_and_defaults() {
    let sc = StorageController::new("sc1");
    assert_eq!(sc.ccd.cd.nphases, 3);
    assert_eq!(sc.ccd.cd.nconds, 3);
    assert_eq!(sc.ccd.cd.nterms, 1);
    assert_eq!(sc.ccd.element_terminal, 1);
    assert_eq!(sc.f_mon_phase, MonPhase::Max);
    assert_eq!(sc.f_kw_target, 8000.0);
    assert_eq!(sc.f_kw_target_low, 4000.0);
    assert_eq!(sc.f_kw_threshold, 6000.0);
    assert_eq!(sc.f_kw_band, 160.0);
    assert_eq!(sc.f_kw_band_low, 80.0);
    assert_eq!(sc.discharge_mode, StorageCtrlMode::PeakShave);
    assert_eq!(sc.charge_mode, StorageCtrlMode::Time);
    assert_eq!(sc.fleet_state, StorageState::Idling);
    assert_eq!(sc.seasons, 1);
    assert_eq!(sc.season_targets, vec![8000.0]);
    assert_eq!(sc.season_targets_low, vec![4000.0]);
    assert!(sc.ccd.cd.yprim.is_none());
}

#[test]
fn prop_table_shape_matches_pascal() {
    let enums = EnumRegistry::new();
    let cp = class_props(&enums);
    // 37 class props + basefreq + enabled + Like.
    assert_eq!(cp.num_properties(), prop::NUM_PROPS);
    // Spot-check a few key types.
    assert_eq!(cp.prop(prop::ELEMENT).ptype, PropType::ObjectRef);
    assert_eq!(cp.prop(prop::MON_PHASE).ptype, PropType::MappedStringEnum);
    // Pascal `DoubleDArrayProperty` + IndirectCount over ElementList: modeled as a
    // `DoubleArray` (renders `ArrayOrFilePath` + `$dssLength: ElementList`), the
    // count resolved via `get_i32(ELEMENT_LIST)` = FleetSize (OG-1.5c B4).
    assert_eq!(cp.prop(prop::WEIGHTS).ptype, PropType::DoubleArray);
    assert_eq!(cp.prop(prop::SEASON_TARGETS).ptype, PropType::DoubleArray);
    // Pascal `[SilentReadOnly, ReadByFunction]` fleet-aggregate double (OG-1.5c B4).
    assert_eq!(cp.prop(prop::KWH_TOTAL).ptype, PropType::Double);
}

#[test]
fn kw_target_side_effect_syncs_band_and_threshold() {
    // kWTarget=5000 with default %kWBand=2: threshold=5000*0.75=3750,
    // HalfkWBand=2/200*5000=50, kWBand=100, pctkWBand re-synced to 2.
    let mut sc = StorageController::new("sc1");
    sc.set_f64(prop::KW_TARGET, 5000.0);
    sc.side_effects(prop::KW_TARGET, 0);
    assert_eq!(sc.f_kw_threshold, 3750.0);
    assert_eq!(sc.f_kw_band, 100.0);
    assert_eq!(sc.f_pct_kw_band, 2.0);
}

#[test]
fn pct_kw_band_side_effect_sets_kw_band() {
    // %kWBand=5 over kWTarget=5000: HalfkWBand=5/200*5000=125, kWBand=250.
    let mut sc = StorageController::new("sc1");
    sc.set_f64(prop::KW_TARGET, 5000.0);
    sc.side_effects(prop::KW_TARGET, 0);
    sc.set_f64(prop::PCT_KW_BAND, 5.0);
    sc.side_effects(prop::PCT_KW_BAND, 0);
    assert_eq!(sc.f_kw_band, 250.0);
    assert!(!sc.f_kw_band_specified);
}

/// D10 (WP-U1.6, `a14c3f1f`, SVN r4058, in the capi015 backend 0.15.0b4 = SVN
/// r4103): setting `kWBandLow` syncs the **Low** percent/target pair
/// (`FpctkWBandLow := FkWBandLow / FkWTargetLow * 100`), not the typo'd
/// `FpctkWBand := FkWBandLow / FkWTarget * 100` the 0.14.5 baseline reproduced.
///
/// Oracle-validated against capi015 (`/tmp/probe_d10.py`, 2026-07-16;
/// `StorageController kWTarget=300 kWTargetLow=100 kWBand=50 kWBandLow=20`,
/// applied in that order, then `? %kWBand`/`? %kWBandLow`):
///
/// | engine | `%kWBand` | `%kWBandLow` |
/// |---|---|---|
/// | capi015 (0.15.0b4, the fix) | `16.6667` | `20` |
/// | capi 0.14.5 (the typo)      | `6.6667`  | `2`  |
///
/// The typo corrupts BOTH: it overwrites the correct `%kWBand` (16.667) with
/// `20/300*100 = 6.667` and never syncs `%kWBandLow` (kept at its 2.0 default).
/// Both assertions below flip if the fix is reverted — feature-sensitive.
#[test]
fn kw_band_low_side_effect_syncs_the_low_pct_pair() {
    let mut sc = StorageController::new("sc1");
    // Apply in the probe's order (kWTarget, kWTargetLow, kWBand, kWBandLow).
    sc.set_f64(prop::KW_TARGET, 300.0);
    sc.side_effects(prop::KW_TARGET, 0);
    sc.set_f64(prop::KW_TARGET_LOW, 100.0);
    sc.side_effects(prop::KW_TARGET_LOW, 0);
    sc.set_f64(prop::KW_BAND, 50.0);
    sc.side_effects(prop::KW_BAND, 0);
    sc.set_f64(prop::KW_BAND_LOW, 20.0);
    sc.side_effects(prop::KW_BAND_LOW, 0);
    // capi015: %kWBand stays 16.667 (kWBand arm), %kWBandLow syncs to 20.
    assert!(
        (sc.f_pct_kw_band - 16.666_666_666_666_7).abs() < 1e-9,
        "%kWBand {} (typo would be 6.667)",
        sc.f_pct_kw_band
    );
    assert!(
        (sc.f_pct_kw_band_low - 20.0).abs() < 1e-9,
        "%kWBandLow {} (typo would leave the 2.0 default)",
        sc.f_pct_kw_band_low
    );
    assert_eq!(sc.f_kw_band_low, 20.0);
}

#[test]
fn mode_discharge_follow_sets_noon_trigger() {
    let mut sc = StorageController::new("sc1");
    sc.set_i32(prop::MODE_DISCHARGE, StorageCtrlMode::Follow.ordinal());
    sc.side_effects(prop::MODE_DISCHARGE, 0);
    assert_eq!(sc.discharge_trigger_time, 12.0);
}

#[test]
fn element_list_side_effect_levelizes_weights() {
    let mut sc = StorageController::new("sc1");
    sc.set_string_list(prop::ELEMENT_LIST, vec!["sa".into(), "sb".into()]);
    sc.side_effects(prop::ELEMENT_LIST, 0);
    assert!(sc.element_list_specified);
    assert!(sc.fleet_list_changed);
    assert_eq!(sc.fleet_size, 2);
    assert_eq!(sc.weights, vec![1.0, 1.0]);
    assert_eq!(sc.array_size(prop::WEIGHTS), 2);
}

#[test]
fn seasons_side_effect_resizes_targets() {
    let mut sc = StorageController::new("sc1");
    sc.set_i32(prop::SEASONS, 3);
    sc.side_effects(prop::SEASONS, 0);
    assert_eq!(sc.season_targets.len(), 3);
    assert_eq!(sc.season_targets_low.len(), 3);
}

#[test]
fn disp_factor_out_of_range_resets_to_one() {
    let mut sc = StorageController::new("sc1");
    sc.set_f64(prop::DISP_FACTOR, 1.5);
    sc.side_effects(prop::DISP_FACTOR, 0);
    assert_eq!(sc.disp_factor, 1.0);
    sc.set_f64(prop::DISP_FACTOR, 0.8);
    sc.side_effects(prop::DISP_FACTOR, 0);
    assert_eq!(sc.disp_factor, 0.8);
}

#[test]
fn inhibit_time_floored_at_one() {
    let mut sc = StorageController::new("sc1");
    sc.set_i32(prop::INHIBIT_TIME, 0);
    sc.side_effects(prop::INHIBIT_TIME, 0);
    assert_eq!(sc.inhibit_hrs, 1);
}

#[test]
fn recalc_missing_element_errors_372() {
    let mut sc = StorageController::new("sc1");
    sc.recalc();
    let errs = sc.ccd.cd.obj.take_errors();
    assert!(errs.iter().any(|e| e.contains("is not set")));
}

#[test]
fn mon_phase_above_nphases_errors_and_resets() {
    // Default nphases = 3; MonPhase = 4 is out of range → error + reset to 1.
    let mut sc = StorageController::new("sc1");
    sc.set_i32(prop::MON_PHASE, 4);
    sc.side_effects(prop::MON_PHASE, 0);
    assert_eq!(sc.f_mon_phase, MonPhase::Phase(1));
    let errs = sc.ccd.cd.obj.take_errors();
    assert!(
        errs.iter().any(|e| e.contains("Monitored phase")),
        "{errs:?}"
    );
}

#[test]
fn recalc_syncs_nphases_to_monitored_element() {
    // Pascal: FNphases := MonitoredElement.Nphases; NConds := FNphases.
    let mut sc = StorageController::new("sc1");
    assert_eq!(sc.ccd.cd.nphases, 3); // ctor default
    sc.mon_snap = Some(RefSnapshot {
        full_name: "line.l1".into(),
        nphases: 1,
        nterms: 2,
        buses: vec!["b1".into(), "b2".into()],
    });
    sc.recalc();
    assert_eq!(sc.ccd.cd.nphases, 1);
    assert_eq!(sc.ccd.cd.nconds, 1);
    assert_eq!(sc.get_bus_name(1), "b1");
}

#[test]
fn make_like_copies_dispatch_settings() {
    let mut base = StorageController::new("base");
    base.f_kw_target = 5000.0;
    base.f_kw_band = 250.0;
    base.f_pct_kw_band = 5.0;
    base.discharge_mode = StorageCtrlMode::Follow;
    base.discharge_trigger_time = 12.0;
    base.pct_fleet_reserve = 20.0;
    base.seasons = 2;
    base.season_targets = vec![5000.0, 4500.0];
    base.season_targets_low = vec![2500.0, 2200.0];
    base.ccd.element_terminal = 1;

    let mut sc = StorageController::new("sc1");
    sc.make_like(&base);
    assert_eq!(sc.f_kw_target, 5000.0);
    assert_eq!(sc.f_kw_band, 250.0);
    assert_eq!(sc.f_pct_kw_band, 5.0);
    assert_eq!(sc.discharge_mode, StorageCtrlMode::Follow);
    assert_eq!(sc.discharge_trigger_time, 12.0);
    assert_eq!(sc.pct_fleet_reserve, 20.0);
    assert_eq!(sc.seasons, 2);
    assert_eq!(sc.season_targets, vec![5000.0, 4500.0]);
    assert_eq!(sc.season_targets_low, vec![2500.0, 2200.0]);
}

// ---------------------------------------------------------------------------
// Mock dispatch environment: a fixed monitored signal plus a tiny in-memory
// Storage fleet (an ideal inverter — no curve/clamps), so the `Sample` dispatch
// arithmetic is testable in isolation (mirrors gen_dispatcher's MockEnv).
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct MockStorage {
    name: String,
    enabled: bool,
    dispatch_mode: StorageDispatchMode,
    state: StorageState,
    kw_out: f64,
    present_kw: f64,
    present_kv: f64,
    kw_rating: f64,
    kwh_stored: f64,
    kwh_rating: f64,
    kwh_reserve: f64,
    kw_out_idling: f64,
    cut_in_kw_ac: f64,
    cut_out_kw_ac: f64,
    inverter_on: bool,
    nphases: usize,
    pct_kw_out: f64,
    pct_kw_in: f64,
    pct_reserve: f64,
    state_desired: StorageState,
    nominal_calls: usize,
}

impl MockStorage {
    fn new(name: &str, kw_rating: f64, kwh_rating: f64, stored_frac: f64) -> Self {
        Self {
            name: name.into(),
            enabled: true,
            dispatch_mode: StorageDispatchMode::Default,
            state: StorageState::Idling,
            kw_out: 0.0,
            present_kw: 0.0,
            present_kv: 12.47,
            kw_rating,
            kwh_stored: kwh_rating * stored_frac,
            kwh_rating,
            kwh_reserve: kwh_rating * 0.2,
            kw_out_idling: 0.0, // ideal, no idling loss → clean arithmetic
            cut_in_kw_ac: 0.0,
            cut_out_kw_ac: 0.0,
            inverter_on: true,
            nphases: 3,
            pct_kw_out: 100.0,
            pct_kw_in: 100.0,
            pct_reserve: 20.0,
            state_desired: StorageState::Idling,
            nominal_calls: 0,
        }
    }

    /// Pascal `Set_StorageState`: decline a change past the kWh limits.
    fn set_storage_state(&mut self, value: StorageState) {
        self.state = match value {
            StorageState::Charging if self.kwh_stored < self.kwh_rating => value,
            StorageState::Discharging if self.kwh_stored > self.kwh_reserve => value,
            StorageState::Charging
            | StorageState::Discharging
            | StorageState::Idling
            | StorageState::Other(_) => StorageState::Idling,
        };
    }

    /// Pascal `Set_kW`.
    fn set_kw(&mut self, value: f64) {
        if value > 0.0 {
            self.state = StorageState::Discharging;
            self.pct_kw_out = value / self.kw_rating * 100.0;
        } else if value < 0.0 {
            self.state = StorageState::Charging;
            self.pct_kw_in = value.abs() / self.kw_rating * 100.0;
        } else {
            self.state = StorageState::Idling;
        }
    }

    /// Pascal `SetNominalDEROutput` (ideal inverter).
    fn set_nominal(&mut self) {
        self.nominal_calls += 1;
        match self.state {
            StorageState::Discharging => {
                if self.kwh_stored > self.kwh_reserve {
                    self.kw_out = self.kw_rating * self.pct_kw_out / 100.0;
                } else {
                    self.state = StorageState::Idling;
                }
            }
            StorageState::Charging => {
                if self.kwh_stored < self.kwh_rating {
                    self.kw_out = -self.kw_rating * self.pct_kw_in / 100.0;
                } else {
                    self.state = StorageState::Idling;
                }
            }
            _ => {}
        }
        if self.state == StorageState::Idling {
            self.kw_out = -self.kw_out_idling;
        }
        self.present_kw = self.kw_out;
    }
}

struct MockEnv {
    monitored_power: Complex64,
    monitored_current: f64,
    fleet: Vec<MockStorage>,
    events: Vec<String>,
    errors: crate::diag::ErrorLog,
    pushes: Vec<StorageState>,
    release_inhibit_pushes: usize,
    loads_need_updating: bool,
    time_of_day: f64,
    dyna_h: f64,
    dbl_hour: f64,
    control_iter: i32,
    mode: SolveMode,
    /// `DSS.SeasonalRating` (default `false`, matching Pascal).
    season_rating: bool,
    /// The `Get_DynamicTarget` `RatingIdx` this mock hands back — `None`
    /// models an empty `DSS.SeasonSignal`.
    season_rating_idx: Option<i32>,
}

impl MockEnv {
    fn new(power_kw: f64, fleet: Vec<MockStorage>) -> Self {
        Self {
            monitored_power: Complex64::new(power_kw * 1000.0, 0.0),
            monitored_current: 0.0,
            fleet,
            events: Vec::new(),
            errors: crate::diag::ErrorLog::new(),
            pushes: Vec::new(),
            release_inhibit_pushes: 0,
            loads_need_updating: false,
            time_of_day: 0.0,
            dyna_h: 3600.0,
            dbl_hour: 0.0,
            control_iter: 1,
            mode: SolveMode::Daily,
            season_rating: false,
            season_rating_idx: None,
        }
    }
    fn idx(r: ElemId) -> usize {
        r.index()
    }
}

impl StorageDispatchEnv for MockEnv {
    fn control_power(&mut self, _mon_phase: MonPhase, _fnphases: usize) -> Complex64 {
        self.monitored_power
    }
    fn control_current(&mut self, _mon_phase: MonPhase, _fnphases: usize) -> f64 {
        self.monitored_current
    }
    fn monitored_vterminal1_abs(&mut self) -> f64 {
        7200.0
    }
    fn monitored_nphases(&self) -> usize {
        3
    }
    fn find_storage(&self, name: &str) -> FleetFind {
        match self
            .fleet
            .iter()
            .position(|s| s.name.eq_ignore_ascii_case(name))
        {
            None => FleetFind::NotFound,
            Some(i) => {
                if self.fleet[i].enabled {
                    FleetFind::Found(ElemId::new(0, i))
                } else {
                    FleetFind::Disabled
                }
            }
        }
    }
    fn all_fleet_storage(&self) -> Vec<(String, ElemId)> {
        self.fleet
            .iter()
            .enumerate()
            .filter(|(_, s)| s.enabled && s.dispatch_mode != StorageDispatchMode::ExternalMode)
            .map(|(i, s)| (s.name.clone(), ElemId::new(0, i)))
            .collect()
    }
    fn push_error(&mut self, diag: crate::diag::DssDiagnostic) {
        self.errors.push(diag);
    }
    fn snap(&self, r: ElemId) -> StorageSnap {
        let s = &self.fleet[Self::idx(r)];
        StorageSnap {
            state: s.state,
            present_kw: s.present_kw,
            present_kv: s.present_kv,
            kw: s.kw_out,
            kwh_stored: s.kwh_stored,
            kwh_rating: s.kwh_rating,
            kwh_reserve: s.kwh_reserve,
            kw_rating: s.kw_rating,
            nphases: s.nphases,
            cut_in_kw_ac: s.cut_in_kw_ac,
            cut_out_kw_ac: s.cut_out_kw_ac,
            kw_out_idling: s.kw_out_idling,
            inverter_on: s.inverter_on,
        }
    }
    fn set_state(&mut self, r: ElemId, state: StorageState) {
        self.fleet[Self::idx(r)].set_storage_state(state);
    }
    fn set_kw(&mut self, r: ElemId, kw: f64) {
        self.fleet[Self::idx(r)].set_kw(kw);
    }
    fn set_pct_kw_out(&mut self, r: ElemId, pct: f64) {
        self.fleet[Self::idx(r)].pct_kw_out = pct;
    }
    fn set_pct_kw_in(&mut self, r: ElemId, pct: f64) {
        self.fleet[Self::idx(r)].pct_kw_in = pct;
    }
    fn set_pct_reserve(&mut self, r: ElemId, pct: f64) {
        self.fleet[Self::idx(r)].pct_reserve = pct;
    }
    fn set_state_desired(&mut self, r: ElemId, state: StorageState) {
        self.fleet[Self::idx(r)].state_desired = state;
    }
    fn set_dispatch_external(&mut self, r: ElemId) {
        self.fleet[Self::idx(r)].dispatch_mode = StorageDispatchMode::ExternalMode;
    }
    fn set_nominal(&mut self, r: ElemId) {
        self.fleet[Self::idx(r)].set_nominal();
    }
    fn present_kw(&self, r: ElemId) -> f64 {
        self.fleet[Self::idx(r)].present_kw
    }
    fn storage_full_name(&self, r: ElemId) -> String {
        format!("Storage.{}", self.fleet[Self::idx(r)].name)
    }
    fn push_immediate(&mut self, code: StorageState) {
        self.loads_need_updating = true;
        self.pushes.push(code);
    }
    fn push_release_inhibit(&mut self, _inhibit_hrs: i32) {
        self.loads_need_updating = true;
        self.release_inhibit_pushes += 1;
    }
    fn append_event(&mut self, msg: &str) {
        self.events.push(msg.to_string());
    }
    fn set_loads_need_updating(&mut self) {
        self.loads_need_updating = true;
    }
    fn time_of_day(&self) -> f64 {
        self.time_of_day
    }
    fn dyna_h(&self) -> f64 {
        self.dyna_h
    }
    fn dbl_hour(&self) -> f64 {
        self.dbl_hour
    }
    fn control_iteration(&self) -> i32 {
        self.control_iter
    }
    fn solve_mode(&self) -> SolveMode {
        self.mode
    }
    fn season_rating(&self) -> bool {
        self.season_rating
    }
    fn season_rating_idx(&mut self) -> Option<i32> {
        self.season_rating_idx
    }
}

/// A PeakShave controller with the given target, watching a single storage.
fn peakshave_controller(target_kw: f64) -> StorageController {
    let mut sc = StorageController::new("sc1");
    sc.discharge_mode = StorageCtrlMode::PeakShave;
    sc.set_f64(prop::KW_TARGET, target_kw);
    sc.side_effects(prop::KW_TARGET, 0);
    sc.mon_snap = Some(RefSnapshot {
        full_name: "line.l1".into(),
        nphases: 3,
        nterms: 2,
        buses: vec!["b1".into(), "b2".into()],
    });
    sc.recalc();
    sc
}

#[test]
fn sample_first_run_sets_fleet_external_and_values() {
    // The deferred RecalcElementData: the first Sample builds the fleet, sets
    // every member to external dispatch + pushes the controller's rates.
    let mut sc = peakshave_controller(10_000.0);
    let mut env = MockEnv::new(10_000.0, vec![MockStorage::new("a", 2000.0, 500.0, 0.7)]);
    sc.sample(&mut env);
    assert_eq!(
        env.fleet[0].dispatch_mode,
        StorageDispatchMode::ExternalMode
    );
    assert_eq!(env.fleet[0].pct_kw_out, sc.pct_kw_rate);
    assert_eq!(env.fleet[0].pct_reserve, sc.pct_fleet_reserve);
    assert_eq!(sc.fleet.len(), 1);
}

#[test]
fn sample_peakshave_discharges_overage() {
    // Monitored 11_000 kW vs target 10_000 (half-band 100): PDiff = +1000 kW,
    // dispatched to the single storage (weight 1) → kW = min(2000, 0 + 1000).
    let mut sc = peakshave_controller(10_000.0);
    let mut env = MockEnv::new(11_000.0, vec![MockStorage::new("a", 2000.0, 500.0, 0.7)]);
    sc.sample(&mut env);
    assert_eq!(env.fleet[0].state, StorageState::Discharging);
    assert!(
        (env.fleet[0].present_kw - 1000.0).abs() < 1e-9,
        "present_kw = {}",
        env.fleet[0].present_kw
    );
    assert!(sc.fleet_state == StorageState::Discharging);
    assert!(env.loads_need_updating);
}

#[test]
fn d10_discharge_transition_forces_resolve_on_first_iteration() {
    // D10 (WP-U1.6, `1b3123ce`, SVN r4058): a peakshave discharge that moves the
    // fleet OUT of a non-discharging state forces a new power flow on control
    // iteration 1 — even when the per-element kW dispatch itself does NOT change
    // (Storage already sitting at its rating). Pre-D14 no push happened, so
    // Storage.kW could stay stale across matching steps.
    fn run(control_iter: i32) -> MockEnv {
        let mut sc = peakshave_controller(10_000.0);
        let mut st = MockStorage::new("a", 2000.0, 500.0, 0.7);
        st.present_kw = 2000.0; // already at rating → the dispatch is a no-op
        st.kw_out = 2000.0;
        let mut env = MockEnv::new(12_000.0, vec![st]); // PDiff +2000 > half-band
        env.control_iter = control_iter;
        sc.sample(&mut env);
        env
    }
    // Iter 1: IDLING→DISCHARGING with no kW change ⇒ D10 forces the re-solve.
    let e1 = run(1);
    assert_eq!(e1.fleet[0].state, StorageState::Discharging);
    assert!(
        e1.pushes.contains(&StorageState::Discharging),
        "iter 1 must force a re-solve, pushes = {:?}",
        e1.pushes
    );
    // Iter > 1: same transition, but with no kW change D10 does NOT push.
    let e2 = run(2);
    assert_eq!(e2.fleet[0].state, StorageState::Discharging);
    assert!(
        !e2.pushes.contains(&StorageState::Discharging),
        "iter 2 must not push without a kW change, pushes = {:?}",
        e2.pushes
    );
}

#[test]
fn sample_peakshave_in_band_does_not_dispatch() {
    // Monitored 10_050 kW vs target 10_000: |PDiff| = 50 < half-band 100 → no
    // discharge; the storage stays idle and charging becomes allowed.
    let mut sc = peakshave_controller(10_000.0);
    let mut env = MockEnv::new(10_050.0, vec![MockStorage::new("a", 2000.0, 500.0, 0.7)]);
    sc.sample(&mut env);
    assert_eq!(env.fleet[0].state, StorageState::Idling);
    assert!(sc.charging_allowed);
}

#[test]
fn sample_peakshave_weighted_split() {
    // PDiff = +1000 kW across weights [3, 1] (total 4) → +750 / +250.
    let mut sc = peakshave_controller(10_000.0);
    sc.set_string_list(prop::ELEMENT_LIST, vec!["a".into(), "b".into()]);
    sc.side_effects(prop::ELEMENT_LIST, 0);
    sc.weights = vec![3.0, 1.0];
    let mut env = MockEnv::new(
        11_000.0,
        vec![
            MockStorage::new("a", 2000.0, 500.0, 0.7),
            MockStorage::new("b", 2000.0, 500.0, 0.7),
        ],
    );
    sc.sample(&mut env);
    assert!(
        (env.fleet[0].present_kw - 750.0).abs() < 1e-9,
        "a = {}",
        env.fleet[0].present_kw
    );
    assert!(
        (env.fleet[1].present_kw - 250.0).abs() < 1e-9,
        "b = {}",
        env.fleet[1].present_kw
    );
    assert_eq!(sc.total_weight, 4.0);
}

#[test]
fn sample_out_of_oomph_sets_idle_flag() {
    // The storage is at its reserve (kWhStored == kWhReserve): an overage cannot
    // be served → OutOfOomph, fleet held idle.
    let mut sc = peakshave_controller(10_000.0);
    let mut store = MockStorage::new("a", 2000.0, 500.0, 0.2); // stored == reserve
    store.kwh_stored = store.kwh_reserve;
    let mut env = MockEnv::new(11_000.0, vec![store]);
    sc.sample(&mut env);
    assert!(sc.out_of_oomph);
    assert_eq!(env.fleet[0].state, StorageState::Idling);
}

#[test]
fn sample_time_mode_discharges_at_trigger() {
    // Time mode, trigger time == time-of-day: the fleet is set to discharge at
    // pctkWRate (= 20%): kW = 2000 * 0.20 = 400.
    let mut sc = StorageController::new("sc1");
    sc.discharge_mode = StorageCtrlMode::Time;
    sc.discharge_trigger_time = 6.0;
    sc.mon_snap = Some(RefSnapshot {
        full_name: "line.l1".into(),
        nphases: 3,
        nterms: 2,
        buses: vec!["b1".into(), "b2".into()],
    });
    sc.recalc();
    let mut env = MockEnv::new(0.0, vec![MockStorage::new("a", 2000.0, 500.0, 0.7)]);
    env.time_of_day = 6.0;
    sc.sample(&mut env);
    // DoTimeMode sets the fleet to discharge + pctkWRate; the element is sampled
    // via the next solve, but the controller has already committed the state.
    assert_eq!(env.fleet[0].state, StorageState::Discharging);
    assert_eq!(env.fleet[0].pct_kw_out, sc.pct_kw_rate);
}

#[test]
fn sample_peakshavelow_charges_below_target() {
    // Discharge mode PeakShave with monitored below target → in-band, charging
    // allowed. Charge mode PeakShaveLow with monitored below kWTargetLow →
    // charge the fleet.
    let mut sc = StorageController::new("sc1");
    sc.discharge_mode = StorageCtrlMode::PeakShave;
    sc.charge_mode = StorageCtrlMode::PeakShaveLow;
    sc.set_f64(prop::KW_TARGET, 10_000.0);
    sc.side_effects(prop::KW_TARGET, 0);
    sc.set_f64(prop::KW_TARGET_LOW, 4000.0);
    sc.side_effects(prop::KW_TARGET_LOW, 0);
    sc.mon_snap = Some(RefSnapshot {
        full_name: "line.l1".into(),
        nphases: 3,
        nterms: 2,
        buses: vec!["b1".into(), "b2".into()],
    });
    sc.recalc();
    // Monitored 3000 kW: below target (discharge in-band) AND below target_low
    // (charge): PDiff_low = 3000 - 4000 = -1000, |.| > half-band-low.
    let mut env = MockEnv::new(3000.0, vec![MockStorage::new("a", 2000.0, 500.0, 0.5)]);
    sc.sample(&mut env);
    assert!(sc.charging_allowed);
    assert_eq!(env.fleet[0].state, StorageState::Charging);
    assert!(
        env.fleet[0].present_kw < 0.0,
        "present_kw = {}",
        env.fleet[0].present_kw
    );
}

#[test]
fn sample_named_missing_storage_errors_14403() {
    // A named fleet member that does not resolve: 14403 at first Sample (the
    // Pascal parse-time 37201 for a Storage-less circuit is NOT_PORTED).
    let mut sc = peakshave_controller(10_000.0);
    sc.set_string_list(prop::ELEMENT_LIST, vec!["ghost".into()]);
    sc.side_effects(prop::ELEMENT_LIST, 0);
    let mut env = MockEnv::new(11_000.0, vec![MockStorage::new("a", 2000.0, 500.0, 0.7)]);
    sc.sample(&mut env);
    // Exactly one 14403 per Sample — `ensure_fleet` builds the fleet once; the
    // dispatch modes no longer re-call `MakeFleetList` (so no double-emission).
    let count = env
        .errors
        .iter()
        .filter(|e| e.contains("\"ghost\" not found"))
        .count();
    assert_eq!(count, 1, "{:?}", env.errors);
    // The rebuild stays pending (FleetListChanged left set).
    assert!(sc.fleet_list_changed);
}

#[test]
fn reset_idles_the_fleet() {
    let mut sc = peakshave_controller(10_000.0);
    let mut store = MockStorage::new("a", 2000.0, 500.0, 0.7);
    store.state = StorageState::Discharging;
    let mut env = MockEnv::new(11_000.0, vec![store]);
    sc.reset(&mut env);
    assert_eq!(env.fleet[0].state, StorageState::Idling);
    assert_eq!(sc.fleet_state, StorageState::Idling);
}

/// A single-storage controller in the given discharge mode, watching one
/// monitored line, fleet resolved at first Sample.
fn controller_in_mode(discharge_mode: StorageCtrlMode) -> StorageController {
    let mut sc = StorageController::new("sc1");
    sc.discharge_mode = discharge_mode;
    sc.mon_snap = Some(RefSnapshot {
        full_name: "line.l1".into(),
        nphases: 3,
        nterms: 2,
        buses: vec!["b1".into(), "b2".into()],
    });
    sc.recalc();
    sc
}

#[test]
fn sample_support_mode_discharges_for_export() {
    // Support keeps the load *above* the target: PDiff = S.re·0.001 + kWtarget.
    // A net export of 3000 kW (S.re = −3e6) with a 4000 kW target → PDiff = +1000
    // → discharge the storage by the deficit.
    let mut sc = controller_in_mode(StorageCtrlMode::Support);
    sc.set_f64(prop::KW_TARGET, 4000.0);
    sc.side_effects(prop::KW_TARGET, 0);
    let mut env = MockEnv::new(-3000.0, vec![MockStorage::new("a", 2000.0, 500.0, 0.7)]);
    sc.sample(&mut env);
    assert_eq!(env.fleet[0].state, StorageState::Discharging);
    assert!(
        (env.fleet[0].present_kw - 1000.0).abs() < 1e-9,
        "present_kw = {}",
        env.fleet[0].present_kw
    );
}

#[test]
fn sample_schedule_mode_ramps_on_trigger() {
    // Schedule: at 0.1 h past the discharge trigger (within the up-ramp 0.25 h),
    // the rate ramps linearly: pctDischargeRate = min(pctkWRate, pctkWRate·tdiff/
    // UpRampTime) = min(20, 20·0.1/0.25) = 8.
    let mut sc = controller_in_mode(StorageCtrlMode::Schedule);
    sc.discharge_trigger_time = 6.0;
    let mut env = MockEnv::new(0.0, vec![MockStorage::new("a", 2000.0, 500.0, 0.7)]);
    env.time_of_day = 6.1;
    sc.sample(&mut env);
    assert_eq!(env.fleet[0].state, StorageState::Discharging);
    assert!(
        (env.fleet[0].pct_kw_out - 8.0).abs() < 1e-9,
        "pct = {}",
        env.fleet[0].pct_kw_out
    );
    assert!((sc.last_pct_discharge_rate - 8.0).abs() < 1e-9);
    assert!(env.pushes.contains(&StorageState::Discharging));
}

#[test]
fn sample_current_peakshave_converts_amps_to_kw() {
    // I-Peakshave: the control signal is amps. kWtarget in kA (0.2), %kWBand·1000
    // (HalfkWBand = 2/200·0.2·1000 = 2). Monitored 300 A → PDiff = 300 − 200 =
    // 100 A; kWNeeded := PresentkV·√3·AmpsDiff = 12.47·√3·100 ≈ 2160 kW → the
    // single battery caps at its 2000 kWrated.
    let mut sc = StorageController::new("sc1");
    sc.discharge_mode = StorageCtrlMode::CurrentPeakShave; // set before kWTarget so the side effect uses ×1000
    sc.set_f64(prop::KW_TARGET, 0.2);
    sc.side_effects(prop::KW_TARGET, 0);
    sc.mon_snap = Some(RefSnapshot {
        full_name: "line.l1".into(),
        nphases: 3,
        nterms: 2,
        buses: vec!["b1".into(), "b2".into()],
    });
    sc.recalc();
    let mut env = MockEnv::new(0.0, vec![MockStorage::new("a", 2000.0, 500.0, 0.7)]);
    env.monitored_current = 300.0;
    sc.sample(&mut env);
    assert_eq!(env.fleet[0].state, StorageState::Discharging);
    assert!(
        (env.fleet[0].present_kw - 2000.0).abs() < 1e-9,
        "present_kw = {}",
        env.fleet[0].present_kw
    );
}

#[test]
fn sample_time_mode_charges_and_arms_release_inhibit() {
    // Discharge PeakShave below target → in-band → charging allowed; charge mode
    // Time with the charge trigger == time-of-day → set fleet to charge, inhibit
    // discharge, push CHARGING + the delayed RELEASE_INHIBIT.
    let mut sc = peakshave_controller(10_000.0);
    sc.charge_mode = StorageCtrlMode::Time;
    sc.charge_trigger_time = 2.0;
    let mut store = MockStorage::new("a", 2000.0, 500.0, 0.5); // not full → chargeable
    store.kwh_stored = 250.0;
    let mut env = MockEnv::new(2000.0, vec![store]); // below target → discharge in-band
    env.time_of_day = 2.0;
    sc.sample(&mut env);
    assert_eq!(env.fleet[0].state, StorageState::Charging);
    assert!(sc.discharge_inhibited);
    assert_eq!(env.release_inhibit_pushes, 1);
    assert!(env.pushes.contains(&StorageState::Charging));
}

#[test]
fn do_pending_action_release_inhibit_clears_flag() {
    // RELEASE_INHIBIT lifts the inhibit — but only when the discharge mode is not
    // Follow (Pascal `and (DischargeMode <> MODEFOLLOW)`).
    let mut sc = StorageController::new("sc1");
    sc.discharge_mode = StorageCtrlMode::PeakShave;
    sc.discharge_inhibited = true;
    sc.do_pending_action(StorageCtrlAction::ReleaseInhibit.ordinal());
    assert!(!sc.discharge_inhibited);

    let mut follow = StorageController::new("sc2");
    follow.discharge_mode = StorageCtrlMode::Follow;
    follow.discharge_inhibited = true;
    follow.do_pending_action(StorageCtrlAction::ReleaseInhibit.ordinal());
    assert!(
        follow.discharge_inhibited,
        "Follow mode must keep the inhibit"
    );
}

#[test]
fn sample_dispatch_below_cutout_with_inverter_off_overrides_to_idle() {
    // A small overage wants a dispatch below the inverter cut-out, and the
    // inverter is already OFF → the controller overrides the (just-set)
    // discharging state back to idling instead of dispatching.
    let mut sc = peakshave_controller(4000.0);
    let mut store = MockStorage::new("a", 2000.0, 500.0, 0.7);
    store.cut_in_kw_ac = 500.0;
    store.cut_out_kw_ac = 500.0;
    store.inverter_on = false;
    // Monitored 4100 kW vs target 4000 → PDiff = +100 (> half-band 40) so the
    // dispatch loop runs, but DispatchkW ≈ 100 < CutOut 500 with the inverter
    // off → override to idling.
    let mut env = MockEnv::new(4100.0, vec![store]);
    sc.sample(&mut env);
    assert_eq!(env.fleet[0].state, StorageState::Idling);
    assert!(env.fleet[0].present_kw.abs() < 1e-9);
}

#[test]
fn sample_loadshape_mode_discharges_without_shape() {
    // LoadShape mode with no controller shape → LoadShapeMult = CDoubleOne
    // (1 + j1) → Re > 0 → discharge at NewkWRate = Re·100 = 100%, forcing a
    // power-flow update.
    let mut sc = controller_in_mode(StorageCtrlMode::LoadShape);
    let mut env = MockEnv::new(0.0, vec![MockStorage::new("a", 2000.0, 500.0, 0.7)]);
    env.mode = SolveMode::Daily;
    sc.sample(&mut env);
    assert_eq!(env.fleet[0].state, StorageState::Discharging);
    assert!(
        (sc.pct_kw_rate - 100.0).abs() < 1e-9,
        "pct = {}",
        sc.pct_kw_rate
    );
    assert!(env.loads_need_updating);
    assert!(env.pushes.contains(&StorageState::Idling)); // PushTimeOntoControlQueue(0)
}

/// A controller with distinct seasonal targets so each `get_dynamic_target`
/// branch (valid index / OOB index / `seasons<=1`) resolves to a different,
/// distinguishable value from the non-seasonal `FkWTarget`/`FkWTargetLow`
/// fallback (8000/4000, the class defaults — left untouched).
fn seasonal_controller() -> StorageController {
    let mut sc = StorageController::new("sc1");
    sc.seasons = 3;
    sc.season_targets = vec![100.0, 200.0, 300.0];
    sc.season_targets_low = vec![10.0, 20.0, 30.0];
    sc
}

#[test]
fn dynamic_target_none_idx_returns_zero_not_fkwtarget() {
    // Pascal `Result` stays its `0` init when `DSS.SeasonSignal` is empty —
    // NOT the non-seasonal `FkWTarget`/`FkWTargetLow` fallback (8000/4000).
    let sc = seasonal_controller();
    let mut env = MockEnv::new(0.0, vec![]);
    env.season_rating_idx = None;
    assert_eq!(sc.get_dynamic_target(&mut env, true), 0.0);
    assert_eq!(sc.get_dynamic_target(&mut env, false), 0.0);
}

#[test]
fn dynamic_target_valid_idx_reads_season_targets() {
    let sc = seasonal_controller();
    let mut env = MockEnv::new(0.0, vec![]);

    env.season_rating_idx = Some(0);
    assert_eq!(sc.get_dynamic_target(&mut env, true), 100.0);
    assert_eq!(sc.get_dynamic_target(&mut env, false), 10.0);

    env.season_rating_idx = Some(1);
    assert_eq!(sc.get_dynamic_target(&mut env, true), 200.0);
    assert_eq!(sc.get_dynamic_target(&mut env, false), 20.0);
}

#[test]
fn dynamic_target_idx_equal_seasons_falls_back_to_fkwtarget() {
    // RatingIdx == Seasons (3): passes the `<=` guard (a valid one-past-end
    // array length) but is out of bounds for the 0..Seasons-1 slots — the
    // `arr.get(i) == None` branch, not the `rating_idx > seasons` guard.
    let sc = seasonal_controller();
    let mut env = MockEnv::new(0.0, vec![]);
    env.season_rating_idx = Some(3);
    assert_eq!(sc.get_dynamic_target(&mut env, true), sc.f_kw_target);
    assert_eq!(sc.get_dynamic_target(&mut env, false), sc.f_kw_target_low);
    assert_eq!(sc.f_kw_target, 8000.0);
    assert_eq!(sc.f_kw_target_low, 4000.0);
}

#[test]
fn dynamic_target_idx_beyond_seasons_falls_back_to_fkwtarget() {
    // RatingIdx > Seasons: the deliberate divergence from Pascal's OOB read
    // (CLAUDE.md UB policy) — falls back instead of indexing past the array.
    let sc = seasonal_controller();
    let mut env = MockEnv::new(0.0, vec![]);
    env.season_rating_idx = Some(5);
    assert_eq!(sc.get_dynamic_target(&mut env, true), sc.f_kw_target);
    assert_eq!(sc.get_dynamic_target(&mut env, false), sc.f_kw_target_low);
}

#[test]
fn dynamic_target_seasons_le_one_falls_back_even_with_valid_looking_idx() {
    // Pascal `(RatingIdx <= Seasons) and (Seasons > 1)`: a single-season
    // config never uses the seasonal array, even for RatingIdx=0.
    let mut sc = seasonal_controller();
    sc.seasons = 1;
    sc.season_targets = vec![999.0];
    sc.season_targets_low = vec![99.0];
    let mut env = MockEnv::new(0.0, vec![]);
    env.season_rating_idx = Some(0);
    assert_eq!(sc.get_dynamic_target(&mut env, true), sc.f_kw_target);
    assert_eq!(sc.get_dynamic_target(&mut env, false), sc.f_kw_target_low);
}

#[test]
fn sample_logs_event_when_eventlog_enabled() {
    // With ShowEventLog on, a PeakShave discharge step appends the "Attempting to
    // dispatch …" + per-storage "Requesting …" messages.
    let mut sc = peakshave_controller(10_000.0);
    sc.ccd.show_event_log = true;
    let mut env = MockEnv::new(11_000.0, vec![MockStorage::new("a", 2000.0, 500.0, 0.7)]);
    sc.sample(&mut env);
    assert!(
        env.events
            .iter()
            .any(|e| e.contains("Attempting to dispatch")),
        "events: {:?}",
        env.events
    );
    assert!(
        env.events
            .iter()
            .any(|e| e.contains("Requesting Storage.a")),
        "events: {:?}",
        env.events
    );
}

#[cfg(test)]
mod make_pos_seq_tests {
    use super::super::*;
    use crate::elements::pos_seq::{PosSeqCtx, PosSeqElemInfo};
    use crate::elements::traits::{CktElement, ElemId};
    use crate::obj::base::DssObject;

    /// Pascal `TStorageControllerObj.MakePosSequence` (StorageController.pas:834):
    /// phases/conds + bus from the monitored element (probe `S6`: makeposseq-safe).
    #[test]
    fn resyncs_to_monitored() {
        let mut sc = StorageController::new("sc1");
        sc.ccd.monitored_element = Some(ElemId::new(1, 4));
        sc.ccd.element_terminal = 1;
        let ctx = PosSeqCtx {
            monitored: Some(PosSeqElemInfo {
                nphases: 1,
                nconds: 1,
                yorder: 2,
                bus_names: vec!["b1".into(), "b2".into()],
                ..Default::default()
            }),
            ..Default::default()
        };
        let plan = sc.make_pos_sequence(&ctx);
        assert_eq!(sc.ccd.cd.nphases, 1);
        assert_eq!(sc.ccd.cd.nconds, 1);
        assert_eq!(sc.get_bus_name(1), "b1");
        assert!(plan.run_base && plan.actions.is_empty());
        assert_eq!(sc.monitored_element_ref(), Some(ElemId::new(1, 4)));
    }

    #[test]
    fn nil_monitored_runs_base_only() {
        let mut sc = StorageController::new("sc1");
        let np = sc.ccd.cd.nphases;
        let plan = sc.make_pos_sequence(&PosSeqCtx::default());
        assert_eq!(sc.ccd.cd.nphases, np);
        assert!(plan.run_base);
    }
}

/// Discharge / charge mode + queue-action ordinals, pinned against
/// `StorageController.pas:268-276` (`MODEFOLLOW=1` … `CURRENTPEAKSHAVELOW=9`)
/// and `:279` (`RELEASE_INHIBIT = 999`), plus the two `DssEnum` value lists in
/// `obj/dss_enum/registry/control.rs` (`Discharge Mode` `[5,1,3,2,4,6,8]`,
/// `Charge Mode` `[2,4,7,9]`).
#[test]
fn storage_ctrl_mode_and_action_pin_pascal_ordinals() {
    for (ord, m) in [
        (1, StorageCtrlMode::Follow),
        (2, StorageCtrlMode::LoadShape),
        (3, StorageCtrlMode::Support),
        (4, StorageCtrlMode::Time),
        (5, StorageCtrlMode::PeakShave),
        (6, StorageCtrlMode::Schedule),
        (7, StorageCtrlMode::PeakShaveLow),
        (8, StorageCtrlMode::CurrentPeakShave),
        (9, StorageCtrlMode::CurrentPeakShaveLow),
    ] {
        assert_eq!(m.ordinal(), ord);
        assert_eq!(StorageCtrlMode::from_ordinal(ord), Some(m));
    }
    assert_eq!(StorageCtrlMode::from_ordinal(0), None);
    assert_eq!(StorageCtrlMode::from_ordinal(10), None);

    // Every registry ordinal of BOTH enums must resolve (one shared space).
    for ord in [5, 1, 3, 2, 4, 6, 8] {
        assert!(
            StorageCtrlMode::from_ordinal(ord).is_some(),
            "discharge {ord}"
        );
    }
    for ord in [2, 4, 7, 9] {
        assert!(StorageCtrlMode::from_ordinal(ord).is_some(), "charge {ord}");
    }

    assert_eq!(StorageCtrlAction::ReleaseInhibit.ordinal(), 999);
    assert_eq!(
        StorageCtrlAction::from_ordinal(999),
        Some(StorageCtrlAction::ReleaseInhibit)
    );
    // The storage-state markers `Sample` pushes onto the same generic queue are
    // NOT this class's action codes — `DoPendingAction` must ignore them.
    for code in [
        StorageState::Charging.ordinal(),
        StorageState::Idling.ordinal(),
        StorageState::Discharging.ordinal(),
    ] {
        assert_eq!(StorageCtrlAction::from_ordinal(code), None);
    }
}

// EXPECTED-VALUE-PIN(STORAGE_CONTROLLER_IDLE_TEST_COMPLEMENTS_THE_ORDINAL): the
// terminal branches idle the fleet in every non-idle state, in both lanes —
// asserted on the predicate over all three fleet states and on the observable a
// deck sees when a discharging fleet runs out of energy.
/// The "is the fleet already idling?" guard both terminal branches put in front
/// of `SetFleetToIdle` answers the state test it reads as — in **both** lanes.
///
/// Upstream answers a different question. "Ran out of OOMPH"
/// (`.inputs/dss_capi/src/Controls/StorageController.pas:1350`; r4133
/// `Version8/Source/Controls/StorageController.pas:1771`) and "Fully charged"
/// (`:1619`; r4133 `:2042`) both guard the call with
/// `if not FleetState = STORE_IDLING`, and Object Pascal binds `not` tighter
/// than `=` over an `Integer` field — so the guard is `(not FleetState) = 0`,
/// true only for `STORE_CHARGING = -1`. It therefore fires in the one state
/// those branches do NOT reach by discharging, and a fleet that exhausts its
/// energy is left discharging. Both gating oracles carry it; neither lane
/// reproduces it (`GOLDEN_REBASE_PLAN.md` G2.1e; `issue-18`).
///
/// Two levels, because the predicate alone would not notice a caller that
/// stopped consulting it:
///
/// 1. the predicate, exhaustive over the three fleet states — the
///    discriminating one is `Discharging`, where upstream's complement answers
///    `false` and this engine answers `true`, so it cannot be dropped silently;
/// 2. the observable — a fleet reaching "Ran out of OOMPH" *while discharging*
///    ends the sample with every member idled and `STORE_IDLING` pushed onto
///    the control queue ("force a new power flow solution"), which is the whole
///    point of the branch and what upstream skips.
#[test]
fn fleet_idle_guard_fires_unless_the_fleet_is_already_idling() {
    let mut sc = StorageController::new("sc1");
    for state in [
        StorageState::Charging,
        StorageState::Idling,
        StorageState::Discharging,
    ] {
        sc.fleet_state = state;
        assert_eq!(
            sc.fleet_needs_idling(),
            state != StorageState::Idling,
            "fleet state {state:?}: the guard asks `FleetState <> STORE_IDLING`; \
             upstream complements the ordinal instead — `(not FleetState) = 0` — \
             and answers {} here",
            (!state.ordinal()) == StorageState::Idling.ordinal()
        );
    }

    // The complement reading, spelled out: it is what makes `Charging` the only
    // state upstream idles from (`not (-1) = 0`, `not 0 = -1`, `not 1 = -2`), so
    // `Discharging` above is a genuine disagreement and not a restatement.
    assert_eq!(!StorageState::Charging.ordinal(), 0);
    assert_eq!(!StorageState::Idling.ordinal(), -1);
    assert_eq!(!StorageState::Discharging.ordinal(), -2);

    // The observable. A PeakShave fleet that is discharging into a 1 MW overage
    // with nothing above its reserve left takes the "Ran out of OOMPH" branch
    // (`remaining_kWh <= reserve_kWh`) in the state upstream's guard misses.
    let mut sc = peakshave_controller(10_000.0);
    sc.fleet_state = StorageState::Discharging;
    let mut store = MockStorage::new("a", 2000.0, 500.0, 0.2); // stored == reserve
    store.kwh_stored = store.kwh_reserve;
    store.state = StorageState::Discharging;
    store.kw_out = 400.0;
    store.present_kw = 400.0;
    let mut env = MockEnv::new(11_000.0, vec![store]);
    sc.sample(&mut env);

    assert!(sc.out_of_oomph, "the OOMPH branch is the one under test");
    assert_eq!(
        sc.fleet_state,
        StorageState::Idling,
        "`SetFleetToIdle` must run: upstream's guard is false while discharging, \
         so upstream leaves the fleet discharging and only its event log claims \
         otherwise"
    );
    assert_eq!(
        env.fleet[0].state,
        StorageState::Idling,
        "`SetFleetToIdle` walks the fleet: `StorageState := STORE_IDLING; kW := 0`"
    );
    assert!(
        env.pushes.contains(&StorageState::Idling),
        "`PushTimeOntoControlQueue(STORE_IDLING)` — without it the stale kW is \
         never re-solved on this step"
    );
}

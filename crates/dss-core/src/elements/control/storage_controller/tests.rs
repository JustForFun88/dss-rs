use super::*;
use crate::elements::pc::storage::{
    STORE_CHARGING, STORE_DISCHARGING, STORE_EXTERNALMODE, STORE_IDLING,
};
use crate::elements::traits::ElemRef;
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
    assert_eq!(sc.f_mon_phase, MAXPHASE);
    assert_eq!(sc.f_kw_target, 8000.0);
    assert_eq!(sc.f_kw_target_low, 4000.0);
    assert_eq!(sc.f_kw_threshold, 6000.0);
    assert_eq!(sc.f_kw_band, 160.0);
    assert_eq!(sc.f_kw_band_low, 80.0);
    assert_eq!(sc.discharge_mode, MODE_PEAKSHAVE);
    assert_eq!(sc.charge_mode, MODE_TIME);
    assert_eq!(sc.fleet_state, STORE_IDLING);
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
    assert_eq!(cp.prop(prop::WEIGHTS).ptype, PropType::DoubleVArray);
    assert_eq!(cp.prop(prop::SEASON_TARGETS).ptype, PropType::DoubleArray);
    assert_eq!(cp.prop(prop::KWH_TOTAL).ptype, PropType::String);
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

#[test]
fn mode_discharge_follow_sets_noon_trigger() {
    let mut sc = StorageController::new("sc1");
    sc.set_i32(prop::MODE_DISCHARGE, MODE_FOLLOW);
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
    assert_eq!(sc.f_mon_phase, 1);
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
    base.discharge_mode = MODE_FOLLOW;
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
    assert_eq!(sc.discharge_mode, MODE_FOLLOW);
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
    dispatch_mode: i32,
    state: i32,
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
    state_desired: i32,
    nominal_calls: usize,
}

impl MockStorage {
    fn new(name: &str, kw_rating: f64, kwh_rating: f64, stored_frac: f64) -> Self {
        Self {
            name: name.into(),
            enabled: true,
            dispatch_mode: 0, // STORE_DEFAULT
            state: STORE_IDLING,
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
            state_desired: STORE_IDLING,
            nominal_calls: 0,
        }
    }

    /// Pascal `Set_StorageState`: decline a change past the kWh limits.
    fn set_storage_state(&mut self, value: i32) {
        self.state = match value {
            STORE_CHARGING if self.kwh_stored < self.kwh_rating => value,
            STORE_DISCHARGING if self.kwh_stored > self.kwh_reserve => value,
            STORE_CHARGING | STORE_DISCHARGING => STORE_IDLING,
            _ => STORE_IDLING,
        };
    }

    /// Pascal `Set_kW`.
    fn set_kw(&mut self, value: f64) {
        if value > 0.0 {
            self.state = STORE_DISCHARGING;
            self.pct_kw_out = value / self.kw_rating * 100.0;
        } else if value < 0.0 {
            self.state = STORE_CHARGING;
            self.pct_kw_in = value.abs() / self.kw_rating * 100.0;
        } else {
            self.state = STORE_IDLING;
        }
    }

    /// Pascal `SetNominalDEROutput` (ideal inverter).
    fn set_nominal(&mut self) {
        self.nominal_calls += 1;
        match self.state {
            STORE_DISCHARGING => {
                if self.kwh_stored > self.kwh_reserve {
                    self.kw_out = self.kw_rating * self.pct_kw_out / 100.0;
                } else {
                    self.state = STORE_IDLING;
                }
            }
            STORE_CHARGING => {
                if self.kwh_stored < self.kwh_rating {
                    self.kw_out = -self.kw_rating * self.pct_kw_in / 100.0;
                } else {
                    self.state = STORE_IDLING;
                }
            }
            _ => {}
        }
        if self.state == STORE_IDLING {
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
    errors: Vec<String>,
    pushes: Vec<i32>,
    release_inhibit_pushes: usize,
    loads_need_updating: bool,
    time_of_day: f64,
    dyna_h: f64,
    dbl_hour: f64,
    mode: SolveMode,
}

impl MockEnv {
    fn new(power_kw: f64, fleet: Vec<MockStorage>) -> Self {
        Self {
            monitored_power: Complex64::new(power_kw * 1000.0, 0.0),
            monitored_current: 0.0,
            fleet,
            events: Vec::new(),
            errors: Vec::new(),
            pushes: Vec::new(),
            release_inhibit_pushes: 0,
            loads_need_updating: false,
            time_of_day: 0.0,
            dyna_h: 3600.0,
            dbl_hour: 0.0,
            mode: SolveMode::Daily,
        }
    }
    fn idx(r: ElemRef) -> usize {
        r.idx
    }
}

impl StorageDispatchEnv for MockEnv {
    fn control_power(&mut self, _mon_phase: i32, _fnphases: usize) -> Complex64 {
        self.monitored_power
    }
    fn control_current(&mut self, _mon_phase: i32, _fnphases: usize) -> f64 {
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
                    FleetFind::Found(ElemRef { cls: 0, idx: i })
                } else {
                    FleetFind::Disabled
                }
            }
        }
    }
    fn all_fleet_storage(&self) -> Vec<(String, ElemRef)> {
        self.fleet
            .iter()
            .enumerate()
            .filter(|(_, s)| s.enabled && s.dispatch_mode != STORE_EXTERNALMODE)
            .map(|(i, s)| (s.name.clone(), ElemRef { cls: 0, idx: i }))
            .collect()
    }
    fn push_error(&mut self, msg: String) {
        self.errors.push(msg);
    }
    fn snap(&self, r: ElemRef) -> StorageSnap {
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
    fn set_state(&mut self, r: ElemRef, state: i32) {
        self.fleet[Self::idx(r)].set_storage_state(state);
    }
    fn set_kw(&mut self, r: ElemRef, kw: f64) {
        self.fleet[Self::idx(r)].set_kw(kw);
    }
    fn set_pct_kw_out(&mut self, r: ElemRef, pct: f64) {
        self.fleet[Self::idx(r)].pct_kw_out = pct;
    }
    fn set_pct_kw_in(&mut self, r: ElemRef, pct: f64) {
        self.fleet[Self::idx(r)].pct_kw_in = pct;
    }
    fn set_pct_reserve(&mut self, r: ElemRef, pct: f64) {
        self.fleet[Self::idx(r)].pct_reserve = pct;
    }
    fn set_state_desired(&mut self, r: ElemRef, state: i32) {
        self.fleet[Self::idx(r)].state_desired = state;
    }
    fn set_dispatch_external(&mut self, r: ElemRef) {
        self.fleet[Self::idx(r)].dispatch_mode = STORE_EXTERNALMODE;
    }
    fn set_nominal(&mut self, r: ElemRef) {
        self.fleet[Self::idx(r)].set_nominal();
    }
    fn present_kw(&self, r: ElemRef) -> f64 {
        self.fleet[Self::idx(r)].present_kw
    }
    fn storage_full_name(&self, r: ElemRef) -> String {
        format!("Storage.{}", self.fleet[Self::idx(r)].name)
    }
    fn push_immediate(&mut self, code: i32) {
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
    fn solve_mode(&self) -> SolveMode {
        self.mode
    }
}

/// A PeakShave controller with the given target, watching a single storage.
fn peakshave_controller(target_kw: f64) -> StorageController {
    let mut sc = StorageController::new("sc1");
    sc.discharge_mode = MODE_PEAKSHAVE;
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
    assert_eq!(env.fleet[0].dispatch_mode, STORE_EXTERNALMODE);
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
    assert_eq!(env.fleet[0].state, STORE_DISCHARGING);
    assert!(
        (env.fleet[0].present_kw - 1000.0).abs() < 1e-9,
        "present_kw = {}",
        env.fleet[0].present_kw
    );
    assert!(sc.fleet_state == STORE_DISCHARGING);
    assert!(env.loads_need_updating);
}

#[test]
fn sample_peakshave_in_band_does_not_dispatch() {
    // Monitored 10_050 kW vs target 10_000: |PDiff| = 50 < half-band 100 → no
    // discharge; the storage stays idle and charging becomes allowed.
    let mut sc = peakshave_controller(10_000.0);
    let mut env = MockEnv::new(10_050.0, vec![MockStorage::new("a", 2000.0, 500.0, 0.7)]);
    sc.sample(&mut env);
    assert_eq!(env.fleet[0].state, STORE_IDLING);
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
    assert_eq!(env.fleet[0].state, STORE_IDLING);
}

#[test]
fn sample_time_mode_discharges_at_trigger() {
    // Time mode, trigger time == time-of-day: the fleet is set to discharge at
    // pctkWRate (= 20%): kW = 2000 * 0.20 = 400.
    let mut sc = StorageController::new("sc1");
    sc.discharge_mode = MODE_TIME;
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
    assert_eq!(env.fleet[0].state, STORE_DISCHARGING);
    assert_eq!(env.fleet[0].pct_kw_out, sc.pct_kw_rate);
}

#[test]
fn sample_peakshavelow_charges_below_target() {
    // Discharge mode PeakShave with monitored below target → in-band, charging
    // allowed. Charge mode PeakShaveLow with monitored below kWTargetLow →
    // charge the fleet.
    let mut sc = StorageController::new("sc1");
    sc.discharge_mode = MODE_PEAKSHAVE;
    sc.charge_mode = MODE_PEAKSHAVELOW;
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
    assert_eq!(env.fleet[0].state, STORE_CHARGING);
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
    assert!(
        env.errors.iter().any(|e| e.contains("\"ghost\" not found")),
        "{:?}",
        env.errors
    );
    // The rebuild stays pending (FleetListChanged left set).
    assert!(sc.fleet_list_changed);
}

#[test]
fn reset_idles_the_fleet() {
    let mut sc = peakshave_controller(10_000.0);
    let mut store = MockStorage::new("a", 2000.0, 500.0, 0.7);
    store.state = STORE_DISCHARGING;
    let mut env = MockEnv::new(11_000.0, vec![store]);
    sc.reset(&mut env);
    assert_eq!(env.fleet[0].state, STORE_IDLING);
    assert_eq!(sc.fleet_state, STORE_IDLING);
}

use super::*;
use crate::obj::base::DssObject;
use crate::obj::props::PropType;

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
fn make_fleet_list_default_empty_errors_37201() {
    // No ElementList → default branch finds no Storage → 37201.
    let mut sc = StorageController::new("sc1");
    assert!(!sc.make_fleet_list());
}

#[test]
fn default_recalc_clears_fleet_flag_no_repeat_37201() {
    // Pascal's MakeFleetList default branch clears FleetListChanged, so a
    // *second* RecalcElementData (a re-Edit) must not re-run the fleet build
    // nor re-emit 37201.
    let mut sc = StorageController::new("sc1");
    sc.mon_snap = Some(RefSnapshot {
        full_name: "line.l1".into(),
        nphases: 3,
        nterms: 2,
        buses: vec!["b1".into(), "b2".into()],
    });
    sc.recalc();
    let first = sc.ccd.cd.obj.take_errors();
    assert_eq!(
        first
            .iter()
            .filter(|e| e.contains("No unassigned Storage"))
            .count(),
        1,
        "{first:?}"
    );
    assert!(
        !sc.fleet_list_changed,
        "default branch must clear the rebuild flag"
    );
    // Second recalc: flag cleared → no rebuild → no fresh 37201.
    sc.recalc();
    let second = sc.ccd.cd.obj.take_errors();
    assert!(
        second.iter().all(|e| !e.contains("No unassigned Storage")),
        "second recalc re-emitted 37201: {second:?}"
    );
}

#[test]
fn named_missing_recalc_keeps_flag_pending() {
    // The named branch Exits *before* clearing FleetListChanged (Pascal
    // l.1889), so the rebuild stays pending and a re-Edit re-emits 14403.
    let mut sc = StorageController::new("sc1");
    sc.set_string_list(prop::ELEMENT_LIST, vec!["sa".into()]);
    sc.side_effects(prop::ELEMENT_LIST, 0);
    assert!(!sc.make_fleet_list());
    assert!(
        sc.fleet_list_changed,
        "missing-name path must leave the rebuild flag set"
    );
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
    // A subsequent MonPhase=2 now validates against the synced nphases (1)
    // and is rejected, instead of silently passing against the default 3.
    sc.set_i32(prop::MON_PHASE, 2);
    sc.side_effects(prop::MON_PHASE, 0);
    assert_eq!(sc.f_mon_phase, 1);
}

#[test]
fn make_fleet_list_named_missing_errors_14403() {
    let mut sc = StorageController::new("sc1");
    sc.set_string_list(prop::ELEMENT_LIST, vec!["sa".into(), "sb".into()]);
    sc.side_effects(prop::ELEMENT_LIST, 0);
    assert!(!sc.make_fleet_list());
    let errs = sc.ccd.cd.obj.take_errors();
    assert!(errs.iter().any(|e| e.contains("\"sa\" not found")));
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

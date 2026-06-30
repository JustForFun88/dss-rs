use super::*;
use crate::obj::base::DssObject;

#[test]
fn default_shape_and_defaults() {
    let ec = EspvlControl::new("e1");
    assert_eq!(ec.ccd.cd.nphases, 3);
    assert_eq!(ec.ccd.cd.nconds, 3);
    assert_eq!(ec.ccd.cd.nterms, 1);
    assert_eq!(ec.ccd.element_terminal, 1);
    assert_eq!(ec.f_type, 0); // dumps '' (no Type set)
    assert_eq!(ec.f_kw_limit, 8000.0); // hardcoded, unsettable
    assert_eq!(ec.f_kw_band, 100.0);
    assert_eq!(ec.half_kw_band, 50.0);
    assert_eq!(ec.f_kvar_limit, 4000.0); // FkWLimit / 2
    assert_eq!(ec.local_control_list_size, 0);
    assert!(ec.ccd.cd.yprim.is_none());
}

#[test]
fn local_control_list_side_effect_levelizes_weights() {
    let mut ec = EspvlControl::new("e1");
    ec.set_string_list(
        prop::LOCAL_CONTROL_LIST,
        vec!["a".into(), "b".into(), "c".into()],
    );
    ec.side_effects(prop::LOCAL_CONTROL_LIST, 0);
    assert_eq!(ec.local_control_list_size, 3);
    assert_eq!(ec.local_control_weights, vec![1.0, 1.0, 1.0]);
    assert_eq!(ec.array_size(prop::LOCAL_CONTROL_WEIGHTS), 3);
}

#[test]
fn pv_storage_lists_track_size_only() {
    let mut ec = EspvlControl::new("e1");
    ec.set_string_list(prop::PV_SYSTEM_LIST, vec!["p1".into(), "p2".into()]);
    ec.side_effects(prop::PV_SYSTEM_LIST, 0);
    assert_eq!(ec.pv_system_list_size, 2);
    // Weights stay NIL (empty) until explicitly set — they dump '' (probed).
    assert!(ec.pv_system_weights.is_empty());
    assert_eq!(ec.array_size(prop::PV_SYSTEM_WEIGHTS), 2);

    ec.set_string_list(prop::STORAGE_LIST, vec!["s1".into()]);
    ec.side_effects(prop::STORAGE_LIST, 0);
    assert_eq!(ec.storage_list_size, 1);
    assert!(ec.storage_weights.is_empty());
}

/// A mock environment: a fixed monitored power plus a tiny ESPVLControl "fleet"
/// keyed by name, with each entry carrying a phantom kW base. Lets the redispatch
/// arithmetic (onto the phantom field) be tested in isolation.
struct MockEnv {
    power: Complex64,
    names: Vec<String>,
    phantom: Vec<f64>,
}
impl MockEnv {
    fn new(power: Complex64, fleet: &[(&str, f64)]) -> Self {
        Self {
            power,
            names: fleet.iter().map(|(n, _)| (*n).to_string()).collect(),
            phantom: fleet.iter().map(|(_, k)| *k).collect(),
        }
    }
}
impl EspvlDispatchEnv for MockEnv {
    fn monitored_power(&mut self) -> Complex64 {
        self.power
    }
    fn find_enabled_espvl(&self, name: &str) -> Option<ElemRef> {
        self.names
            .iter()
            .position(|n| n.eq_ignore_ascii_case(name))
            .map(|i| ElemRef { cls: 0, idx: i })
    }
    fn all_enabled_espvls(&self) -> Vec<ElemRef> {
        (0..self.names.len())
            .map(|i| ElemRef { cls: 0, idx: i })
            .collect()
    }
    fn local_kw_base(&self, r: ElemRef) -> f64 {
        self.phantom[r.idx]
    }
    fn set_local_kw_base(&mut self, r: ElemRef, value: f64) {
        self.phantom[r.idx] = value;
    }
}

fn sys_controller_with_list(names: &[&str], weights: &[f64]) -> EspvlControl {
    let mut ec = EspvlControl::new("e1");
    ec.set_i32(prop::TYP, 1); // SystemController
    ec.set_string_list(
        prop::LOCAL_CONTROL_LIST,
        names.iter().map(|s| s.to_string()).collect(),
    );
    ec.side_effects(prop::LOCAL_CONTROL_LIST, 0);
    ec.local_control_weights = weights.to_vec();
    ec
}

#[test]
fn local_controller_sample_is_noop() {
    // A Local controller (Ftype=2) never builds a list: Sample does nothing.
    let mut ec = EspvlControl::new("e1");
    ec.set_i32(prop::TYP, 2);
    let mut env = MockEnv::new(Complex64::new(4_000_000.0, 0.0), &[("e1", 7.0)]);
    assert!(!ec.sample(&mut env));
    assert_eq!(env.phantom[0], 7.0); // untouched
    assert_eq!(ec.local_control_list_size, 0);
}

#[test]
fn default_type_sample_is_noop() {
    // Ftype=0 (unset): MakeLocalControlList returns false, Sample does nothing.
    let mut ec = EspvlControl::new("e1");
    let mut env = MockEnv::new(Complex64::new(40_000_000.0, 0.0), &[("e1", 0.0)]);
    assert!(!ec.sample(&mut env));
    assert!(ec.local_control_pointer_list.is_empty());
}

#[test]
fn system_controller_scan_all_redispatches_phantom() {
    // Ftype=1, no LocalControlList → scan all enabled ESPVLControls (uniform
    // weights). Monitored 4000 kW vs FkWLimit 8000 → PDiff = -4000, |PDiff| > 50.
    // Two entries, weight 1 each (total 2): each phantom kWBase += -2000, floored
    // at 1.0 by Max(1.0, …). NOTE: this writes only the phantom field — no
    // generator, no power flow, nothing observable changes.
    let mut ec = EspvlControl::new("e1");
    ec.set_i32(prop::TYP, 1);
    let mut env = MockEnv::new(
        Complex64::new(4_000_000.0, 0.0),
        &[("e1", 5000.0), ("e2", 5000.0)],
    );
    let changed = ec.sample(&mut env);
    assert!(changed);
    // 5000 + (-4000)*(1/2) = 3000 (both, above the 1.0 floor).
    assert!((env.phantom[0] - 3000.0).abs() < 1e-9);
    assert!((env.phantom[1] - 3000.0).abs() < 1e-9);
    assert_eq!(ec.local_control_list_size, 2);
    assert_eq!(ec.total_weight, 2.0);
    assert_eq!(ec.local_control_pointer_list.len(), 2);
}

#[test]
fn system_controller_named_list_respects_weights() {
    // PDiff = +4000 (12000 kW vs 8000) over weights [3, 1] (total 4): +3000 / +1000.
    let mut ec = sys_controller_with_list(&["e1", "e2"], &[3.0, 1.0]);
    let mut env = MockEnv::new(
        Complex64::new(12_000_000.0, 0.0),
        &[("e1", 100.0), ("e2", 100.0)],
    );
    assert!(ec.sample(&mut env));
    assert!((env.phantom[0] - 3100.0).abs() < 1e-9);
    assert!((env.phantom[1] - 1100.0).abs() < 1e-9);
}

#[test]
fn system_controller_floors_at_one() {
    // A large negative PDiff drives the phantom base negative; Max(1.0, …) floors it.
    let mut ec = sys_controller_with_list(&["e1"], &[1.0]);
    let mut env = MockEnv::new(Complex64::new(0.0, 0.0), &[("e1", 100.0)]);
    assert!(ec.sample(&mut env));
    assert_eq!(env.phantom[0], 1.0);
}

#[test]
fn system_controller_in_band_does_nothing() {
    // Monitored 8040 kW vs FkWLimit 8000, half-band 50 → |PDiff| = 40 < 50.
    let mut ec = sys_controller_with_list(&["e1"], &[1.0]);
    let mut env = MockEnv::new(Complex64::new(8_040_000.0, 0.0), &[("e1", 100.0)]);
    assert!(!ec.sample(&mut env));
    assert_eq!(env.phantom[0], 100.0);
}

#[test]
fn sample_skips_unresolved_without_crash() {
    // A named list with an entry that doesn't resolve: Pascal walks off the end of
    // the pointer list into a NIL deref; this port iterates the resolved subset.
    // FListSize/TotalWeight still reflect the full list (3).
    let mut ec = sys_controller_with_list(&["e1", "missing", "e2"], &[1.0, 1.0, 1.0]);
    let mut env = MockEnv::new(
        Complex64::new(4_000_000.0, 0.0),
        &[("e1", 5000.0), ("e2", 5000.0)],
    );
    assert!(ec.sample(&mut env));
    assert_eq!(ec.local_control_list_size, 3);
    assert_eq!(ec.total_weight, 3.0);
    assert_eq!(ec.local_control_pointer_list.len(), 2); // only the two resolved
    // PDiff = -4000, share = -4000 * (1/3) ≈ -1333.33 for each resolved entry.
    assert!((env.phantom[0] - (5000.0 - 4000.0 / 3.0)).abs() < 1e-9);
    assert!((env.phantom[1] - (5000.0 - 4000.0 / 3.0)).abs() < 1e-9);
}

#[test]
fn recalc_missing_element_errors_372() {
    let mut ec = EspvlControl::new("e1");
    ec.recalc();
    let errs = ec.ccd.cd.obj.take_errors();
    assert_eq!(errs.len(), 1);
    assert!(errs[0].contains("is not set"));
}

#[test]
fn make_like_copies_only_terminal_and_monitored() {
    let mut src = EspvlControl::new("src");
    src.f_type = 1;
    src.f_kw_band = 250.0;
    src.f_kvar_limit = 1500.0;
    src.ccd.element_terminal = 2;
    src.monitored_full_name = "Line.l1".into();
    src.ccd.monitored_element = Some(ElemRef { cls: 1, idx: 3 });

    let mut dst = EspvlControl::new("dst");
    dst.make_like(&src);
    assert_eq!(dst.ccd.element_terminal, 2);
    assert_eq!(dst.monitored_full_name, "Line.l1");
    assert_eq!(dst.ccd.monitored_element, Some(ElemRef { cls: 1, idx: 3 }));
    // … but Type/bands keep the ctor defaults (Pascal quirk).
    assert_eq!(dst.f_type, 0);
    assert_eq!(dst.f_kw_band, 100.0);
    assert_eq!(dst.f_kvar_limit, 4000.0);
}

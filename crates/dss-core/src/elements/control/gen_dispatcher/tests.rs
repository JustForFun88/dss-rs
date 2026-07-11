use super::*;
use crate::obj::base::DssObject;

#[test]
fn default_shape_and_defaults() {
    let gd = GenDispatcher::new("gd1");
    assert_eq!(gd.ccd.cd.nphases, 3);
    assert_eq!(gd.ccd.cd.nconds, 3);
    assert_eq!(gd.ccd.cd.nterms, 1);
    assert_eq!(gd.ccd.element_terminal, 1);
    assert_eq!(gd.f_kw_limit, 8000.0);
    assert_eq!(gd.f_kw_band, 100.0);
    assert_eq!(gd.half_kw_band, 50.0);
    assert_eq!(gd.f_kvar_limit, 4000.0);
    assert_eq!(gd.list_size, 0);
    assert!(gd.ccd.cd.yprim.is_none());
}

#[test]
fn genlist_side_effect_levelizes_weights() {
    let mut gd = GenDispatcher::new("gd1");
    gd.set_string_list(prop::GENLIST, vec!["g1".into(), "g2".into(), "g3".into()]);
    gd.side_effects(prop::GENLIST, 0);
    assert_eq!(gd.list_size, 3);
    assert_eq!(gd.weights, vec![1.0, 1.0, 1.0]);
    // Weights array is sized by the GenList length.
    assert_eq!(gd.array_size(prop::WEIGHTS), 3);
}

#[test]
fn kwband_side_effect_sets_half_band() {
    let mut gd = GenDispatcher::new("gd1");
    gd.f_kw_band = 250.0;
    gd.side_effects(prop::KWBAND, 0);
    assert_eq!(gd.half_kw_band, 125.0);
}

/// A mock environment: a fixed monitored power plus a tiny generator table
/// keyed by name, so the redispatch arithmetic is testable in isolation.
struct MockEnv {
    power: Complex64,
    names: Vec<String>,
    kw: Vec<f64>,
    kvar: Vec<f64>,
}
impl MockEnv {
    fn new(power: Complex64, gens: &[(&str, f64, f64)]) -> Self {
        Self {
            power,
            names: gens.iter().map(|(n, _, _)| (*n).to_string()).collect(),
            kw: gens.iter().map(|(_, p, _)| *p).collect(),
            kvar: gens.iter().map(|(_, _, q)| *q).collect(),
        }
    }
    fn idx(&self, g: ElemRef) -> usize {
        g.idx
    }
}
impl GenDispatchEnv for MockEnv {
    fn monitored_power(&mut self) -> Complex64 {
        self.power
    }
    fn find_enabled_gen(&self, name: &str) -> Option<ElemRef> {
        self.names
            .iter()
            .position(|n| n.eq_ignore_ascii_case(name))
            .map(|i| ElemRef { cls: 0, idx: i })
    }
    fn all_enabled_gens(&self) -> Vec<ElemRef> {
        (0..self.names.len())
            .map(|i| ElemRef { cls: 0, idx: i })
            .collect()
    }
    fn gen_kw_base(&self, g: ElemRef) -> f64 {
        self.kw[self.idx(g)]
    }
    fn set_gen_kw_base(&mut self, g: ElemRef, value: f64) {
        let i = self.idx(g);
        self.kw[i] = value;
    }
    fn gen_kvar_base(&self, g: ElemRef) -> f64 {
        self.kvar[self.idx(g)]
    }
    fn set_gen_kvar_base(&mut self, g: ElemRef, value: f64) {
        let i = self.idx(g);
        self.kvar[i] = value;
    }
}

fn dispatcher_with_list(names: &[&str], weights: &[f64]) -> GenDispatcher {
    let mut gd = GenDispatcher::new("gd1");
    gd.set_string_list(prop::GENLIST, names.iter().map(|s| s.to_string()).collect());
    gd.side_effects(prop::GENLIST, 0);
    gd.weights = weights.to_vec();
    gd
}

#[test]
fn sample_redispatches_overage_by_equal_weights() {
    // Monitored 2300 kW vs limit 2000 (band 100 → half 50): PDiff = +300,
    // split evenly across two gens (weight 1 each, total 2) → +150 each.
    let mut gd = dispatcher_with_list(&["g1", "g2"], &[1.0, 1.0]);
    gd.f_kw_limit = 2000.0;
    gd.f_kw_band = 100.0;
    gd.half_kw_band = 50.0;
    gd.f_kvar_limit = 1.0e9; // keep QDiff inside the band
    let mut env = MockEnv::new(
        Complex64::new(2_300_000.0, 0.0),
        &[("g1", 1000.0, 0.0), ("g2", 1000.0, 0.0)],
    );
    let changed = gd.sample(&mut env);
    assert!(changed);
    assert!((env.kw[0] - 1150.0).abs() < 1e-9);
    assert!((env.kw[1] - 1150.0).abs() < 1e-9);
    // The pointer list was resolved and cached.
    assert_eq!(gd.gen_pointer_list.len(), 2);
    assert_eq!(gd.total_weight, 2.0);
}

#[test]
fn sample_respects_weights() {
    // PDiff = +400 over weights [3, 1] (total 4): +300 / +100.
    let mut gd = dispatcher_with_list(&["g1", "g2"], &[3.0, 1.0]);
    gd.f_kw_limit = 2000.0;
    gd.half_kw_band = 50.0;
    gd.f_kvar_limit = 1.0e9;
    let mut env = MockEnv::new(
        Complex64::new(2_400_000.0, 0.0),
        &[("g1", 1000.0, 0.0), ("g2", 1000.0, 0.0)],
    );
    assert!(gd.sample(&mut env));
    assert!((env.kw[0] - 1300.0).abs() < 1e-9);
    assert!((env.kw[1] - 1100.0).abs() < 1e-9);
}

#[test]
fn sample_in_band_does_nothing() {
    // Monitored 2040 kW vs limit 2000, half-band 50 → |PDiff|=40 < 50.
    let mut gd = dispatcher_with_list(&["g1"], &[1.0]);
    gd.f_kw_limit = 2000.0;
    gd.half_kw_band = 50.0;
    gd.f_kvar_limit = 1.0e9;
    let mut env = MockEnv::new(Complex64::new(2_040_000.0, 0.0), &[("g1", 1000.0, 0.0)]);
    assert!(!gd.sample(&mut env));
    assert_eq!(env.kw[0], 1000.0);
}

#[test]
fn sample_floors_kw_at_one() {
    // A large negative PDiff would drive the base negative; Max(1.0, …) floors it.
    let mut gd = dispatcher_with_list(&["g1"], &[1.0]);
    gd.f_kw_limit = 5000.0;
    gd.half_kw_band = 50.0;
    gd.f_kvar_limit = 1.0e9;
    let mut env = MockEnv::new(Complex64::new(0.0, 0.0), &[("g1", 1000.0, 0.0)]);
    assert!(gd.sample(&mut env));
    assert_eq!(env.kw[0], 1.0);
}

#[test]
fn sample_redispatches_kvar_overage() {
    // Monitored 800 kvar vs kvarLimit 500 (half-band 50): QDiff = +300, split
    // evenly across two gens (weight 1 each, total 2) → +150 each. The kW
    // limit is set so PDiff stays inside the band (kW branch must not fire).
    let mut gd = dispatcher_with_list(&["g1", "g2"], &[1.0, 1.0]);
    gd.f_kw_limit = 0.0; // monitored kW = 0 → PDiff = 0, in band
    gd.f_kvar_limit = 500.0;
    gd.half_kw_band = 50.0;
    let mut env = MockEnv::new(
        Complex64::new(0.0, 800_000.0),
        &[("g1", 0.0, 200.0), ("g2", 0.0, 200.0)],
    );
    let changed = gd.sample(&mut env);
    assert!(changed);
    // kvar redispatched, kW untouched (PDiff in band).
    assert!((env.kvar[0] - 350.0).abs() < 1e-9);
    assert!((env.kvar[1] - 350.0).abs() < 1e-9);
    assert_eq!(env.kw[0], 0.0);
    assert_eq!(env.kw[1], 0.0);
}

#[test]
fn sample_respects_weights_for_kvar() {
    // QDiff = +400 over weights [3, 1] (total 4): +300 / +100.
    let mut gd = dispatcher_with_list(&["g1", "g2"], &[3.0, 1.0]);
    gd.f_kw_limit = 0.0;
    gd.f_kvar_limit = 500.0;
    gd.half_kw_band = 50.0;
    let mut env = MockEnv::new(
        Complex64::new(0.0, 900_000.0),
        &[("g1", 0.0, 200.0), ("g2", 0.0, 200.0)],
    );
    assert!(gd.sample(&mut env));
    assert!((env.kvar[0] - 500.0).abs() < 1e-9);
    assert!((env.kvar[1] - 300.0).abs() < 1e-9);
}

#[test]
fn sample_floors_kvar_at_zero() {
    // A large negative QDiff would drive the base negative; Max(0.0, …) floors it.
    let mut gd = dispatcher_with_list(&["g1"], &[1.0]);
    gd.f_kw_limit = 0.0;
    gd.f_kvar_limit = 5000.0;
    gd.half_kw_band = 50.0;
    let mut env = MockEnv::new(Complex64::new(0.0, 0.0), &[("g1", 0.0, 200.0)]);
    assert!(gd.sample(&mut env));
    assert_eq!(env.kvar[0], 0.0);
}

#[test]
fn sample_skips_unresolved_gens_without_crash() {
    // A named list with a generator that doesn't resolve: Pascal walks off
    // the end of FGenPointerList into a NIL deref; this port iterates the
    // resolved subset. FListSize/TotalWeight still reflect the full list, so
    // the resolved gens dispatch with sequential weights over total 3.
    let mut gd = dispatcher_with_list(&["g1", "missing", "g2"], &[1.0, 1.0, 1.0]);
    gd.f_kw_limit = 2000.0;
    gd.half_kw_band = 50.0;
    gd.f_kvar_limit = 1.0e9;
    let mut env = MockEnv::new(
        Complex64::new(2_300_000.0, 0.0),
        &[("g1", 1000.0, 0.0), ("g2", 1000.0, 0.0)],
    );
    assert!(gd.sample(&mut env));
    assert_eq!(gd.list_size, 3);
    assert_eq!(gd.total_weight, 3.0);
    assert_eq!(gd.gen_pointer_list.len(), 2); // only the two that resolved
    // PDiff = +300, share = 300 * (1/3) = 100 for each resolved gen.
    assert!((env.kw[0] - 1100.0).abs() < 1e-9);
    assert!((env.kw[1] - 1100.0).abs() < 1e-9);
}

#[test]
fn sample_empty_list_scans_all_enabled() {
    // No GenList → list_size 0 → MakeGenList scans every enabled generator
    // and allocates uniform weights.
    let mut gd = GenDispatcher::new("gd1");
    gd.f_kw_limit = 2000.0;
    gd.half_kw_band = 50.0;
    gd.f_kvar_limit = 1.0e9;
    let mut env = MockEnv::new(
        Complex64::new(2_200_000.0, 0.0),
        &[("g1", 1000.0, 0.0), ("g2", 1000.0, 0.0)],
    );
    assert!(gd.sample(&mut env));
    assert_eq!(gd.list_size, 2);
    assert_eq!(gd.weights, vec![1.0, 1.0]);
    assert!((env.kw[0] - 1100.0).abs() < 1e-9);
    assert!((env.kw[1] - 1100.0).abs() < 1e-9);
}

#[test]
fn recalc_missing_element_errors_372() {
    let mut gd = GenDispatcher::new("gd1");
    gd.recalc();
    let errs = gd.ccd.cd.obj.take_errors();
    assert_eq!(errs.len(), 1);
    assert!(errs[0].contains("is not set"));
}

#[test]
fn make_like_copies_only_terminal_and_monitored() {
    let mut src = GenDispatcher::new("src");
    src.f_kw_limit = 1234.0;
    src.f_kw_band = 250.0;
    src.ccd.element_terminal = 2;
    src.monitored_full_name = "Line.l1".into();
    src.ccd.monitored_element = Some(ElemRef { cls: 1, idx: 3 });

    let mut dst = GenDispatcher::new("dst");
    dst.make_like(&src);
    // Terminal + monitored element are copied …
    assert_eq!(dst.ccd.element_terminal, 2);
    assert_eq!(dst.monitored_full_name, "Line.l1");
    assert_eq!(dst.ccd.monitored_element, Some(ElemRef { cls: 1, idx: 3 }));
    // … but the dispatch settings keep the ctor defaults (Pascal quirk).
    assert_eq!(dst.f_kw_limit, 8000.0);
    assert_eq!(dst.f_kw_band, 100.0);
}

#[cfg(test)]
mod make_pos_seq_tests {
    use super::super::*;
    use crate::elements::pos_seq::{PosSeqCtx, PosSeqElemInfo};
    use crate::elements::traits::{CktElement, ElemRef};

    /// Pascal `TGenDispatcherObj.MakePosSequence` (GenDispatcher.pas:263) is a
    /// NIL-deref hazard: `element=` set (MonitoredElement <> NIL) makes it deref
    /// the always-NIL `ControlledElement` (Access violation #303, probe `3a`).
    /// CLAUDE.md forbids reproducing UB → safe-skip: no mutation, no panic.
    #[test]
    fn crash_config_element_set_is_safe_skip() {
        let mut gd = GenDispatcher::new("gd1");
        gd.ccd.monitored_element = Some(ElemRef { cls: 1, idx: 0 }); // element= set
        // ControlledElement is always NIL for a fleet control → ctx.controlled None.
        let (np, nc) = (gd.ccd.cd.nphases, gd.ccd.cd.nconds);
        let bus = gd.ccd.cd.get_bus(1).to_string();
        let ctx = PosSeqCtx {
            monitored: Some(PosSeqElemInfo {
                nphases: 1,
                nconds: 1,
                bus_names: vec!["b1".into()],
                ..Default::default()
            }),
            controlled: None,
            ..Default::default()
        };
        let plan = gd.make_pos_sequence(&ctx); // must not panic
        assert_eq!((gd.ccd.cd.nphases, gd.ccd.cd.nconds), (np, nc)); // untouched
        assert_eq!(gd.ccd.cd.get_bus(1), bus); // Setbus safe-skipped
        assert!(plan.run_base); // inherited still runs
        assert_eq!(gd.monitored_element_ref(), Some(ElemRef { cls: 1, idx: 0 }));
    }
}

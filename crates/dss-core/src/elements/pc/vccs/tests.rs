//! Unit tests for the ring-buffer index helpers (`map_idx`, `offset_idx`), the
//! rated-quantity recalculation and the seed of a second dynamics entry. The
//! full dynamics trajectory is gated against the oracle in `exec/tests/vccs.rs`.

use super::*;
use crate::elements::ckt::DynamicsSeed;
use crate::solution::SolveMode;
use crate::support::dynamics::IterationFlag;

#[test]
fn map_idx_matches_pascal_1based_wraparound() {
    // Pascal `MapIdx(idx, len)`: wrap into 1..=len (a 0 result maps to 1).
    let len = 5;
    assert_eq!(map_idx(1, len), 1);
    assert_eq!(map_idx(5, len), 5);
    assert_eq!(map_idx(6, len), 1); // 6 mod 6 = 0 -> 1
    assert_eq!(map_idx(0, len), 5); // 0 -> +5 -> 5 mod 6 = 5
    assert_eq!(map_idx(-1, len), 4); // -1 -> 4 -> 4
    assert_eq!(map_idx(-5, len), 5); // -5 -> 0 -> +5 -> 5
}

#[test]
fn offset_idx_steps_and_wraps() {
    let len = 5;
    assert_eq!(offset_idx(0, 1, len), 1); // first use after init (sIdxU = 0)
    assert_eq!(offset_idx(1, 1, len), 2);
    assert_eq!(offset_idx(5, 1, len), 1); // wrap len -> 1
    // The history lookup `MapIdx(iu - k + 1, len)` for iu = 1, k = 1..len.
    let iu = 1;
    let hist: Vec<usize> = (1..=len).map(|k| map_idx(iu - k + 1, len)).collect();
    assert_eq!(hist, vec![1, 5, 4, 3, 2]);
}

#[test]
fn ringbuf_tap_reproduces_pascal_map_idx_order() {
    // Fill a len-5 ring so slot k holds value k (slot 0 dead).
    let len = 5usize;
    let mut rb = RingBuf::alloc(len);
    for k in 1..=len {
        rb[k] = k as f64;
    }
    // The filter taps `iu - k + 1` for k = 1..=len at head iu = 1 must read the
    // slots in the exact Pascal `MapIdx` order [1, 5, 4, 3, 2] — the same order
    // `offset_idx_steps_and_wraps` pins for the raw index helper.
    let iu = 1i64;
    let taps: Vec<f64> = (1..=len).map(|k| rb.tap(iu - k as i64 + 1)).collect();
    assert_eq!(taps, vec![1.0, 5.0, 4.0, 3.0, 2.0]);
    // The wraparound contract, checked independently of `tap`'s body: for every
    // raw index the filter can emit (negative `iu-k+1`, past-end), `tap` must
    // fold into a live slot `1..=len` — never the dead slot 0, never out of
    // bounds. With `buf[k] == k` the returned value equals the physical slot, so
    // this pins the fold range without restating `self[map_idx(..)]`.
    for idx in -(len as i64)..=(2 * len as i64) {
        let v = rb.tap(idx);
        assert!(
            (1.0..=len as f64).contains(&v),
            "tap({idx}) folded to slot {v}, outside 1..={len}"
        );
    }
}

#[test]
fn recalc_sets_rated_quantities() {
    // 1-phase: Irated = Prated/Vrated/1, BaseVolt = Vrated, BaseCurr = Ppct%·Irated.
    let mut v = Vccs::new("v1");
    v.prated = 3000.0;
    v.vrated = 208.0;
    v.ppct = 100.0;
    v.recalc();
    assert!((v.irated - 3000.0 / 208.0).abs() < 1e-9);
    assert!((v.base_volt - 208.0).abs() < 1e-9);
    assert!((v.base_curr - 3000.0 / 208.0).abs() < 1e-9);

    // 3-phase: Irated *= sqrt(3), BaseVolt /= sqrt(3).
    v.cd.nphases = 3;
    v.cd.set_nconds(3);
    v.prated = 3000.0;
    v.vrated = 360.0;
    v.ppct = 50.0;
    v.recalc();
    let sqrt3 = 3.0_f64.sqrt();
    let irated = 3000.0 / 360.0 / 3.0 * sqrt3;
    assert!((v.irated - irated).abs() < 1e-9);
    assert!((v.base_volt - 360.0 / sqrt3).abs() < 1e-9);
    assert!((v.base_curr - 0.5 * irated).abs() < 1e-9);
}

/// What a waveform seed writes and a step reads, the hunt start of each
/// curve's lookup included.
#[derive(Debug, PartialEq)]
struct WaveformState {
    s: [f64; 6],
    s_v1: Complex64,
    vlast: Complex64,
    whist: Vec<f64>,
    wlast: Vec<f64>,
    z: Vec<f64>,
    zlast: Vec<f64>,
    y2: Vec<f64>,
    idx: (i64, i64),
    hunts: (usize, usize),
}

fn waveform_state(v: &Vccs) -> WaveformState {
    let hunt = |c: &Option<XyCurveObj>| c.as_ref().map_or(usize::MAX, XyCurveObj::hunt_start);
    WaveformState {
        s: [v.s1, v.s2, v.s3, v.s4, v.s5, v.s6],
        s_v1: v.s_v1,
        vlast: v.vlast,
        whist: v.whist.buf.clone(),
        wlast: v.wlast.clone(),
        z: v.z.buf.clone(),
        zlast: v.zlast.clone(),
        y2: v.y2.clone(),
        idx: (v.s_idx_u, v.s_idx_y),
        hunts: (hunt(&v.fbp1), hunt(&v.fbp2)),
    }
}

/// A second entry seeds a waveform VCCS as its first did, whatever the run
/// between left in the hunt of each curve's lookup. The seed's `bp1` input sits
/// just above a curve point, where a lookup that hunts from the first point
/// returns that point's ordinate and one that starts where the run left it
/// interpolates the next segment.
#[test]
fn a_second_entry_seeds_each_curve_lookup_from_the_first_point() {
    let mut v = Vccs::new("v1");
    v.fbp1 = Some(XyCurveObj::from_points(
        "bp1",
        &[-1.0, 0.0, 0.999995, 2.0],
        &[-1.0, 0.0, 0.8, 1.5],
    ));
    v.fbp2 = Some(XyCurveObj::from_points(
        "bp2",
        &[-2.0, 0.0, 2.0],
        &[-1.0, 0.0, 1.0],
    ));
    v.ffilter = Some(XyCurveObj::from_points("filter", &[1.0], &[1.0]));
    v.fsample_freq = 6000.0;
    v.recalc();
    v.cd.terminals[0].set_bus(0);
    v.cd.node_ref = vec![1];
    v.cd.dynamics_seed = DynamicsSeed::Seeded(1);
    let node_v = [Complex64::ZERO, Complex64::new(v.base_volt, 0.0)];
    // The entry reads the terminal current of the last power flow.
    let power_flow = SysCtx::parse_default();
    // One step of one cycle whose last sample falls at w·t = 2π, so the run
    // leaves the `bp1` hunt at 0.999995 and the `bp2` hunt past its first point.
    let step = |iteration_flag| SysCtx {
        is_dynamic_model: true,
        mode: SolveMode::Dynamic,
        solution_count: 1,
        dyna_h: 1.0 / 60.0,
        dyna_t: 1.0 / 60.0,
        iteration_flag,
        ..SysCtx::parse_default()
    };
    let run = |v: &mut Vccs| {
        v.init_state_vars_impl(&power_flow, &node_v);
        let seeded = waveform_state(v);
        v.integrate_states_impl(&step(IterationFlag::NewTimeStep), &node_v);
        v.integrate_states_impl(&step(IterationFlag::SameTimeStep), &node_v);
        (seeded, waveform_state(v), v.y2sum)
    };

    let first = run(&mut v);
    assert_eq!(
        first.0.whist[1], 0.8,
        "the seed reads the ordinate at 0.999995"
    );
    assert_eq!(
        first.1.hunts.0, 2,
        "the run leaves the bp1 hunt at 0.999995"
    );
    assert_ne!(first.1.hunts.1, 0, "the run moves the bp2 hunt");
    let mut left = v.fbp1.clone().expect("bp1 is set");
    assert_ne!(
        left.get_y_value(1.0),
        0.8,
        "from the hunt the run left, bp1 interpolates the next segment"
    );

    let second = run(&mut v);
    assert_eq!(second, first);
}

#[cfg(test)]
mod pos_seq_tests {
    use super::*;
    use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx};
    use crate::elements::traits::CktElement;

    /// VCCS, multi-phase → bare `Phases := 1` edit + run_base
    /// (Pascal `TVCCSObj.MakePosSequence`, vccs.pas:495-500).
    #[test]
    fn makeposseq_vccs_multiphase_sets_phases_1() {
        let mut v = Vccs::new("v1");
        v.cd.nphases = 3;
        v.cd.set_nconds(3);
        let plan = v.make_pos_sequence(&PosSeqCtx::default());
        assert!(plan.run_base);
        assert_eq!(plan.actions, vec![PosSeqAction::SetI32(prop::PHASES, 1)]);
    }

    /// VCCS, already single phase (default) → base-only, still run_base.
    #[test]
    fn makeposseq_vccs_single_phase_is_base_only() {
        let mut v = Vccs::new("v1");
        assert_eq!(v.cd.nphases, 1);
        let plan = v.make_pos_sequence(&PosSeqCtx::default());
        assert!(plan.run_base);
        assert!(plan.actions.is_empty());
    }
}

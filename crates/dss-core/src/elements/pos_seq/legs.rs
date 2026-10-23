//! The legs of an element for the MakePosSequence analysis: the builders the
//! classes call from [`CktElement::pos_seq_legs`], the rated leg voltage and
//! the phase patterns of [`CktElement::pos_seq_pattern`].
//!
//! Conductor indices run over all terminals in order, 0-based: terminal 2's
//! conductor `k` is `nconds + k`. Pinned by `tests::the_legs_of_every_connection`.
//!
//! [`CktElement::pos_seq_legs`]: crate::elements::traits::CktElement::pos_seq_legs
//! [`CktElement::pos_seq_pattern`]: crate::elements::traits::CktElement::pos_seq_pattern

use std::f64::consts::PI;

use num_complex::Complex64;

use super::{PosSeqLeg, PosSeqLegKind};
use crate::util::sqrt3;

/// Leg `phase` from conductor `plus` to `minus` (`None` is earth).
fn leg(
    kind: PosSeqLegKind,
    phase: usize,
    plus: usize,
    minus: Option<usize>,
    v_leg: f64,
    s_leg: Complex64,
) -> PosSeqLeg {
    PosSeqLeg {
        kind,
        winding: 0,
        phase,
        plus,
        minus,
        v_leg,
        s_leg,
    }
}

/// Wye on one terminal: leg `k` from conductor `k` to the neutral conductor
/// `n`, for `k < n`.
pub fn wye(n: usize, kind: PosSeqLegKind, v_leg: f64, s_leg: Complex64) -> Vec<PosSeqLeg> {
    (0..n)
        .map(|k| leg(kind, k, k, Some(n), v_leg, s_leg))
        .collect()
}

/// Delta on one terminal: leg `k` from conductor `k` to conductor `k + 1`.
/// Three or more phases close the last leg on conductor 0. With one or two
/// phases the last leg ends on the extra conductor `n`.
pub fn delta(n: usize, kind: PosSeqLegKind, v_leg: f64, s_leg: Complex64) -> Vec<PosSeqLeg> {
    (0..n)
        .map(|k| {
            let minus = if n >= 3 { (k + 1) % n } else { k + 1 };
            leg(kind, k, k, Some(minus), v_leg, s_leg)
        })
        .collect()
}

/// [`delta`] when `delta_connected`, else [`wye`].
pub fn connected(
    n: usize,
    delta_connected: bool,
    kind: PosSeqLegKind,
    v_leg: f64,
    s_leg: Complex64,
) -> Vec<PosSeqLeg> {
    if delta_connected {
        delta(n, kind, v_leg, s_leg)
    } else {
        wye(n, kind, v_leg, s_leg)
    }
}

/// Per conductor to earth: leg `k` from conductor `k` to earth, for `k < n`.
pub fn to_earth(n: usize, kind: PosSeqLegKind, v_leg: f64, s_leg: Complex64) -> Vec<PosSeqLeg> {
    (0..n)
        .map(|k| leg(kind, k, k, None, v_leg, s_leg))
        .collect()
}

/// A branch between two terminals: leg `k` from conductor `from + k` to
/// conductor `to + k`, for `k < n`. `from` and `to` are the first conductor of
/// each terminal over all terminals in order.
pub fn branch(
    n: usize,
    from: usize,
    to: usize,
    kind: PosSeqLegKind,
    v_leg: f64,
    s_leg: Complex64,
) -> Vec<PosSeqLeg> {
    (0..n)
        .map(|k| leg(kind, k, from + k, Some(to + k), v_leg, s_leg))
        .collect()
}

/// The conductor pairs of a series element from terminal 1 to terminal 2:
/// [`branch`] of [`PosSeqLegKind::Through`] legs with no rated voltage and no
/// power.
pub fn through(n: usize, nconds: usize) -> Vec<PosSeqLeg> {
    branch(n, 0, nconds, PosSeqLegKind::Through, 0.0, Complex64::ZERO)
}

/// The rated leg voltage in volts of an element rated `kv`: `kv/√3` for a wye
/// element of two or three phases, which is rated line to line, and `kv` for
/// a delta leg and for any other phase count.
pub fn v_leg_of(kv: f64, nphases: usize, delta_connected: bool) -> f64 {
    if !delta_connected && matches!(nphases, 2 | 3) {
        kv * 1000.0 / sqrt3()
    } else {
        kv * 1000.0
    }
}

/// The power in volt-amperes a leg of admittance `y` siemens draws at
/// `v_leg` volts: `v_leg² · conj(y)`.
pub fn s_leg_of(v_leg: f64, y: Complex64) -> Complex64 {
    y.conj() * (v_leg * v_leg)
}

/// Whether the terminal written `bus` reaches a node other than earth on its
/// `nconds` conductors. The dotted node numbers give the conductors in order,
/// and a conductor past them takes its default: node `k + 1` for a phase
/// conductor `k < nphases`, earth for the others. Only an explicit `0` is
/// earth among the written numbers.
pub fn reaches_a_live_node(bus: &str, nphases: usize, nconds: usize) -> bool {
    let mut written = bus.split('.').skip(1);
    (0..nconds).any(|k| match written.next() {
        Some(node) => !matches!(node.trim().parse::<i64>(), Ok(0)),
        None => k < nphases,
    })
}

/// The balanced positive-sequence phasors of three phases in conductor order,
/// `(1, a², a)` with `a = exp(j2π/3)`.
pub fn abc() -> [Complex64; 3] {
    [
        Complex64::new(1.0, 0.0),
        Complex64::from_polar(1.0, -2.0 * PI / 3.0),
        Complex64::from_polar(1.0, 2.0 * PI / 3.0),
    ]
}

/// The phasors `exp(-j2πk/n)` for `k < n`.
pub fn rotation(n: usize) -> Vec<Complex64> {
    (0..n)
        .map(|k| Complex64::from_polar(1.0, -2.0 * PI * k as f64 / n as f64))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::pc::pvsystem::PVSystem;
    use crate::elements::pc::storage::{Storage, StorageState};
    use crate::elements::pd::capacitor::Capacitor;
    use crate::elements::pos_seq::balance;
    use crate::elements::traits::CktElement;
    use crate::exec::Dss;
    use PosSeqLegKind::{Lumped, Through};

    const ZERO: Complex64 = Complex64::ZERO;

    fn pairs(legs: &[PosSeqLeg]) -> Vec<(usize, Option<usize>)> {
        legs.iter().map(|l| (l.plus, l.minus)).collect()
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-12 * b.abs().max(1.0)
    }

    fn close_c(a: Complex64, b: Complex64) -> bool {
        (a - b).norm() <= 1e-12 * b.norm().max(1.0)
    }

    /// The circuit element `class.name` of `dss`.
    fn elem<'a>(dss: &'a Dss, class: &str, name: &str) -> &'a dyn CktElement {
        let cls = dss
            .registered_classes()
            .iter()
            .find(|c| c.props.class_name().eq_ignore_ascii_case(class))
            .unwrap_or_else(|| panic!("no class {class}"));
        let idx = cls.name_to_idx[&name.to_ascii_lowercase()];
        cls.arena
            .try_ckt_elem(idx)
            .unwrap_or_else(|| panic!("{class}.{name} is not a circuit element"))
    }

    fn legs_of(dss: &Dss, class: &str, name: &str) -> Vec<PosSeqLeg> {
        elem(dss, class, name)
            .pos_seq_legs()
            .unwrap_or_else(|| panic!("{class}.{name} gives no legs"))
    }

    /// Every leg of `legs` has `kind`, winding 0, its index as phase, `v_leg`
    /// and `s_leg` at T1.
    fn assert_legs(
        legs: &[PosSeqLeg],
        kind: PosSeqLegKind,
        v_leg: f64,
        s_leg: Complex64,
        what: &str,
    ) {
        for (k, l) in legs.iter().enumerate() {
            assert_eq!((l.kind, l.winding, l.phase), (kind, 0, k), "{what} leg {k}");
            assert!(
                close(l.v_leg, v_leg),
                "{what} leg {k}: v_leg {} vs {v_leg}",
                l.v_leg
            );
            assert!(
                close_c(l.s_leg, s_leg),
                "{what} leg {k}: s_leg {} vs {s_leg}",
                l.s_leg
            );
        }
    }

    /// The legs the builders give and the legs real elements give. Each row
    /// asserts the conductor pairs, the leg kind, winding and phase, the rated
    /// leg voltage and the power a leg draws in its nominal admittance as
    /// [`PosSeqLeg`] states it:
    ///
    /// - the builders: the pairs of wye, delta, earth and branch legs, and
    ///   which bus names reach a live node;
    /// - a Load of one, two or three phases, wye or delta: its rated power
    ///   shared over its legs, at the voltage across each;
    /// - an IndMach012 of each connection and phase count, a VCCS and a
    ///   VSConverter: 0, since their Y stamps hold no nominal power, at the
    ///   rated phase or line voltage (`kVac` for the converter);
    /// - a Reactor given by `kvar`, wye or delta: its rated power, and one
    ///   given by `r` and `x`, by matrices or by `z1` and `z0`: 0;
    /// - a Capacitor given by `kvar`, wye or delta: its rated power, a bank of
    ///   steps the kvar of every step whatever its states, and the bank
    ///   given by `cuf`: 0;
    /// - a Fault: 0 at no rated voltage, and a UPFC, a GICLine, a GICSource,
    ///   an Isource and a Vsource off earth: legs through their two
    ///   terminals with 0 at no rated voltage, while a source to earth gives
    ///   no legs;
    /// - a Generator and a WindGen, wye or delta: their rated output, a third
    ///   on each leg;
    /// - a Storage and a PVSystem: the power their present stamp holds, 0
    ///   idling or grid-forming, the rating charging or discharging;
    /// - a GICTransformer of each type: 0 at no rated voltage;
    /// - the phase patterns of the elements that impose one, and none for the
    ///   others.
    #[test]
    fn the_legs_of_every_connection() {
        let wye3 = [(0, Some(3)), (1, Some(3)), (2, Some(3))];
        let delta3 = [(0, Some(1)), (1, Some(2)), (2, Some(0))];
        let branch3 = [(0, Some(3)), (1, Some(4)), (2, Some(5))];
        let earth3 = [(0, None), (1, None), (2, None)];
        assert_eq!(pairs(&wye(3, Lumped, 0.0, ZERO)), wye3);
        assert_eq!(pairs(&wye(1, Lumped, 0.0, ZERO)), [(0, Some(1))]);
        assert_eq!(pairs(&delta(3, Lumped, 0.0, ZERO)), delta3);
        assert_eq!(pairs(&delta(1, Lumped, 0.0, ZERO)), [(0, Some(1))]);
        assert_eq!(
            pairs(&delta(2, Lumped, 0.0, ZERO)),
            [(0, Some(1)), (1, Some(2))]
        );
        assert_eq!(pairs(&to_earth(3, Lumped, 0.0, ZERO)), earth3);
        assert_eq!(pairs(&branch(3, 0, 3, Through, 0.0, ZERO)), branch3);
        assert!(!reaches_a_live_node("b.0.0.0", 3, 3));
        assert!(reaches_a_live_node("b", 3, 3));
        assert!(reaches_a_live_node("b.0", 3, 3));
        assert!(reaches_a_live_node("c.3.2.1", 3, 3));

        let mut dss = balance::stiff("legs");
        for cmd in [
            "new load.w bus1=b phases=3 kv=12.47 kw=300 pf=0.9",
            "new load.w1 bus1=b.1 phases=1 kv=7.2 kw=10",
            "new load.d bus1=b phases=3 conn=delta kv=12.47 kw=300 pf=0.9",
            "new load.d1 bus1=b.1.2 phases=1 conn=delta kv=12.47 kw=10",
            "new load.d2 bus1=b.1.2.3 phases=2 conn=delta kv=12.47 kw=10",
            "new vccs.cs bus1=b phases=3 prated=1000 vrated=12470",
            "new reactor.r bus1=b phases=3 kvar=300 kv=12.47",
            "new capacitor.c bus1=b phases=3 kvar=300 kv=12.47",
            "new generator.g bus1=b phases=3 kv=12.47 kw=300 pf=0.9",
            "new storage.st bus1=b phases=3 kv=12.47 kwrated=100 kwhrated=400",
            "new storage.stc bus1=b phases=3 kv=12.47 kwrated=100 kwhrated=400 %stored=50 \
             state=charging",
            "new storage.stg bus1=b phases=3 kv=12.47 kwrated=100 kwhrated=400 %stored=50 \
             state=charging controlmode=gfm",
            "new storage.std bus1=b phases=3 kv=12.47 kwrated=100 kwhrated=400 \
             state=discharging controlmode=gfm",
            "new vsconverter.vc bus1=b phases=4 ndc=1 kvac=7.2",
            "new vsconverter.v2 bus1=b phases=3 ndc=1 kvac=7.2",
            "new isource.i2 bus1=b.1.2 phases=2 amps=1",
            "new vsource.v2 bus1=b bus2=c phases=3 basekv=12.47",
            "new gictransformer.ty type=yy busH=b busNH=nh busX=x busNX=nx phases=3",
            "new indmach012.m3 bus1=b phases=3 kv=12.47 kw=100",
            "new indmach012.m3w bus1=b phases=3 conn=wye kv=12.47 kw=100",
            "new indmach012.m1d bus1=b.1.2 phases=1 conn=delta kv=12.47 kw=100",
            "new indmach012.m2d bus1=b.1.2.3 phases=2 conn=delta kv=12.47 kw=100",
            "new indmach012.m1w bus1=b.1 phases=1 conn=wye kv=7.2 kw=100",
            "new indmach012.m2w bus1=b.1.2 phases=2 conn=wye kv=12.47 kw=100",
            "new indmach012.mx bus1=b.1.2 conn=wye phases=2 conn=delta kv=12.47 kw=100",
            "new load.w2 bus1=b.1.2 phases=2 kv=12.47 kw=10",
            "new reactor.rd bus1=b phases=3 conn=delta kvar=300 kv=12.47",
            "new capacitor.cd bus1=b phases=3 conn=delta kvar=300 kv=12.47",
            "new capacitor.c2 bus1=b phases=3 numsteps=2 kvar=[100 200] kv=12.47",
            "new capacitor.c2o bus1=b phases=3 numsteps=2 kvar=[100 200] kv=12.47 states=[1 0]",
            "new capacitor.cu bus1=b phases=3 cuf=5 kv=12.47",
            "new fault.f bus1=b bus2=c phases=3 r=1",
            "new upfc.u bus1=b bus2=c phases=3",
            "new gicline.gl bus1=b bus2=c phases=3 volts=1",
            "new line.gs bus1=b bus2=c phases=3 r1=1 x1=1 length=1",
            "new gicsource.gs phases=3 volts=1",
            "new isource.it bus1=b bus2=c phases=3 amps=1",
            "new isource.i3 bus1=b phases=3 amps=1",
            "new isource.i1 bus1=b.1 phases=1 amps=1",
            "new vsource.vp2 bus1=b.1.2 phases=2 basekv=0.24",
            "new vsource.vp1 bus1=b.1 phases=1 basekv=7.2",
            "new storage.sd bus1=b phases=3 kv=12.47 kwrated=100 kwhrated=400 state=discharging",
            "new storage.sdd bus1=b phases=3 conn=delta kv=12.47 kwrated=100 kwhrated=400 \
             state=discharging",
            "new storage.sb bus1=b phases=3 kv=12.47 kwrated=100 kwhrated=400 balanced=yes",
            "new storage.sb1 bus1=b.1 phases=1 kv=7.2 kwrated=10 kwhrated=40 balanced=yes",
            "new pvsystem.pvb bus1=b phases=3 kv=12.47 kva=500 pmpp=400 balanced=yes",
            "new pvsystem.pvu bus1=b phases=3 kv=12.47 kva=500 pmpp=400",
            "new pvsystem.pvb1 bus1=b.1 phases=1 kv=7.2 kva=50 pmpp=40 balanced=yes",
            "new generator.g7 bus1=b phases=3 kv=12.47 kw=300 model=7 balanced=yes",
            "new generator.g7u bus1=b phases=3 kv=12.47 kw=300 model=7",
            "new generator.g1b bus1=b phases=3 kv=12.47 kw=300 balanced=yes",
            "new generator.g7b2 bus1=b.1.2 phases=2 kv=12.47 kw=300 model=7 balanced=yes",
            "new gictransformer.tg type=gsu busH=b busNH=nh2 phases=3",
            "new gictransformer.ta type=auto busH=b busNH=nh3 busX=x2 busNX=nx2 phases=3",
            "new generator.gd bus1=b phases=3 conn=delta kv=12.47 kw=300 pf=0.9",
            "new windgen.wg bus1=b phases=3 kv=12.47 kw=300 pf=0.9",
            "new windgen.wgd bus1=b phases=3 conn=delta kv=12.47 kw=300 pf=0.9",
            "new reactor.rz bus1=b phases=3 r=1 x=10 kv=12.47",
            "new reactor.rm bus1=b phases=3 rmatrix=[1|0 1|0 0 1] xmatrix=[10|0 10|0 0 10] \
             kv=12.47",
            "new reactor.rsc bus1=b phases=3 z1=[1 10] z0=[3 30] kv=12.47",
        ] {
            dss.command(cmd);
        }
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());

        let v_wye = 12470.0 / 3.0_f64.sqrt();
        assert!(close(v_wye, 7199.557856794634), "{v_wye}");
        let q = 300.0 * (1.0 / 0.81 - 1.0_f64).sqrt();
        let s_load = Complex64::new(300.0, q) * (1000.0 / 3.0);
        let w = legs_of(&dss, "load", "w");
        assert_eq!(pairs(&w), wye3);
        assert_legs(&w, Lumped, v_wye, s_load, "load.w");
        let d = legs_of(&dss, "load", "d");
        assert_eq!(pairs(&d), delta3);
        assert_legs(&d, Lumped, 12470.0, s_load, "load.d");
        // The 10 kW loads at the default power factor 0.88: one or two legs,
        // each rated at the voltage across it.
        let q10 = 10.0 * (1.0 / (0.88 * 0.88) - 1.0_f64).sqrt();
        let s10 = Complex64::new(10.0, q10) * 1000.0;
        for (name, want, v, s) in [
            ("w1", vec![(0, Some(1))], 7200.0, s10),
            ("w2", vec![(0, Some(2)), (1, Some(2))], v_wye, s10 / 2.0),
            ("d1", vec![(0, Some(1))], 12470.0, s10),
            ("d2", vec![(0, Some(1)), (1, Some(2))], 12470.0, s10 / 2.0),
        ] {
            let what = format!("load.{name}");
            let l = legs_of(&dss, "load", name);
            assert_eq!(pairs(&l), want, "{what}");
            assert_legs(&l, Lumped, v, s, &what);
        }

        // An induction machine's legs run over its phases as its stamp joins
        // its conductors: delta legs, line-to-line pairs for a three-phase wye
        // machine, whose star floats, and legs to earth for a one- or
        // two-phase wye machine. The conductor count `mx` took while it was
        // wye has no extra conductor, so its last delta leg wraps to conductor
        // 0, as its stamp does.
        let open2 = [(0, Some(1)), (1, Some(2))];
        for (name, nconds, want, v) in [
            ("m3", 3, delta3.to_vec(), 12470.0),
            ("m3w", 3, delta3.to_vec(), 12470.0),
            ("m1d", 2, vec![(0, Some(1))], 12470.0),
            ("m2d", 3, open2.to_vec(), 12470.0),
            ("m1w", 2, vec![(0, None)], 7200.0),
            ("m2w", 3, vec![(0, None), (1, None)], v_wye),
            ("mx", 2, vec![(0, Some(1)), (1, Some(0))], 12470.0),
        ] {
            let what = format!("indmach012.{name}");
            assert_eq!(elem(&dss, "indmach012", name).cd().nconds, nconds, "{what}");
            let m = legs_of(&dss, "indmach012", name);
            assert_eq!(pairs(&m), want, "{what}");
            assert_legs(&m, Lumped, v, ZERO, &what);
        }

        let cs = legs_of(&dss, "vccs", "cs");
        assert_eq!(pairs(&cs), earth3);
        assert_legs(&cs, Lumped, v_wye, ZERO, "vccs.cs");
        let r = legs_of(&dss, "reactor", "r");
        assert_eq!(pairs(&r), branch3);
        assert_legs(
            &r,
            Lumped,
            v_wye,
            Complex64::new(0.0, 100_000.0),
            "reactor.r",
        );
        let c = legs_of(&dss, "capacitor", "c");
        assert_eq!(pairs(&c), branch3);
        assert_legs(
            &c,
            Lumped,
            v_wye,
            Complex64::new(0.0, -100_000.0),
            "capacitor.c",
        );
        // A branch given by impedance, matrices or sequence impedances holds
        // no nominal power.
        for name in ["rz", "rm", "rsc"] {
            let what = format!("reactor.{name}");
            let l = legs_of(&dss, "reactor", name);
            assert_eq!(pairs(&l), branch3, "{what}");
            assert_legs(&l, Lumped, v_wye, ZERO, &what);
        }
        let rd = legs_of(&dss, "reactor", "rd");
        assert_eq!(pairs(&rd), delta3);
        assert_legs(
            &rd,
            Lumped,
            12470.0,
            Complex64::new(0.0, 100_000.0),
            "reactor.rd",
        );
        let cdl = legs_of(&dss, "capacitor", "cd");
        assert_eq!(pairs(&cdl), delta3);
        assert_legs(
            &cdl,
            Lumped,
            12470.0,
            Complex64::new(0.0, -100_000.0),
            "capacitor.cd",
        );
        // A bank of steps draws the kvar of every step whatever its states,
        // and a bank given by capacitance draws nothing.
        let capacitors = dss
            .registered_classes()
            .iter()
            .find(|c| c.props.class_name().eq_ignore_ascii_case("capacitor"))
            .unwrap();
        let steps = Complex64::new(0.0, -100_000.0);
        for (name, states, s) in [
            ("c2", vec![1, 1], steps),
            ("c2o", vec![1, 0], steps),
            ("cu", vec![1], ZERO),
        ] {
            let what = format!("capacitor.{name}");
            let bank = capacitors
                .arena
                .get::<Capacitor>(capacitors.name_to_idx[name])
                .unwrap();
            assert_eq!(bank.states(), states, "{what}");
            let legs = legs_of(&dss, "capacitor", name);
            assert_eq!(pairs(&legs), branch3, "{what}");
            assert_legs(&legs, Lumped, v_wye, s, &what);
        }
        let f = legs_of(&dss, "fault", "f");
        assert_eq!(pairs(&f), branch3);
        assert_legs(&f, Lumped, 0.0, ZERO, "fault.f");
        // The series elements carry their conductors from terminal 1 to
        // terminal 2, and so does a source whose terminal 2 is off earth.
        for (class, name) in [
            ("upfc", "u"),
            ("gicline", "gl"),
            ("gicsource", "gs"),
            ("isource", "it"),
        ] {
            let what = format!("{class}.{name}");
            let t = legs_of(&dss, class, name);
            assert_eq!(pairs(&t), branch3, "{what}");
            assert_legs(&t, Through, 0.0, ZERO, &what);
        }
        for (class, name) in [("isource", "i3"), ("vsource", "vp2"), ("vsource", "vp1")] {
            assert_eq!(
                elem(&dss, class, name).pos_seq_legs(),
                None,
                "{class}.{name}"
            );
        }
        // The legs of a Generator and a WindGen carry the rated output, a third
        // of it on each leg, wye or delta.
        for (class, name, want, v) in [
            ("generator", "g", wye3, v_wye),
            ("generator", "gd", delta3, 12470.0),
            ("windgen", "wg", wye3, v_wye),
            ("windgen", "wgd", delta3, 12470.0),
        ] {
            let what = format!("{class}.{name}");
            let l = legs_of(&dss, class, name);
            assert_eq!(pairs(&l), want, "{what}");
            assert_legs(&l, Lumped, v, -s_load, &what);
        }
        // A full unit does not charge, so the charging units start half full.
        let storage = dss
            .registered_classes()
            .iter()
            .find(|c| c.props.class_name().eq_ignore_ascii_case("storage"))
            .unwrap();
        for (name, state, gfm) in [
            ("st", StorageState::Idling, false),
            ("stc", StorageState::Charging, false),
            ("stg", StorageState::Charging, true),
            ("std", StorageState::Discharging, true),
        ] {
            let unit = storage
                .arena
                .get::<Storage>(storage.name_to_idx[name])
                .unwrap();
            assert_eq!(
                (unit.f_state, unit.base.gfm_mode),
                (state, gfm),
                "storage.{name}"
            );
        }
        let st = legs_of(&dss, "storage", "st");
        assert_eq!(pairs(&st), wye3);
        assert_legs(&st, Lumped, v_wye, ZERO, "storage.st idling");
        let stc = legs_of(&dss, "storage", "stc");
        assert_eq!(pairs(&stc), wye3);
        let charging = Complex64::new(100_000.0 / 3.0, 0.0);
        assert_legs(&stc, Lumped, v_wye, charging, "storage.stc charging");
        // A grid-forming unit stamps nothing charging and a source impedance
        // discharging, neither of them a nominal power.
        for name in ["stg", "std"] {
            let gfm = legs_of(&dss, "storage", name);
            assert_eq!(pairs(&gfm), wye3, "storage.{name}");
            assert_legs(&gfm, Lumped, v_wye, ZERO, &format!("storage.{name} GFM"));
        }
        // Discharging, a unit generates its rating, a third of it on each
        // leg, wye or delta.
        let discharging = -charging;
        let sd = legs_of(&dss, "storage", "sd");
        assert_eq!(pairs(&sd), wye3);
        assert_legs(&sd, Lumped, v_wye, discharging, "storage.sd discharging");
        let sdd = legs_of(&dss, "storage", "sdd");
        assert_eq!(pairs(&sdd), delta3);
        assert_legs(
            &sdd,
            Lumped,
            12470.0,
            discharging,
            "storage.sdd discharging",
        );

        // A VSConverter's AC conductors are sources to earth rated at the
        // line-to-neutral `kVac`, whatever their number.
        let vc = legs_of(&dss, "vsconverter", "vc");
        assert_eq!(pairs(&vc), earth3);
        assert_legs(&vc, Lumped, 7200.0, ZERO, "vsconverter.vc");
        let v2c = legs_of(&dss, "vsconverter", "v2");
        assert_eq!(pairs(&v2c), [(0, None), (1, None)]);
        assert_legs(&v2c, Lumped, 7200.0, ZERO, "vsconverter.v2");

        assert_eq!(elem(&dss, "vsource", "source").pos_seq_legs(), None);
        let v2 = legs_of(&dss, "vsource", "v2");
        assert_eq!(pairs(&v2), branch3);
        assert_legs(&v2, Through, 0.0, ZERO, "vsource.v2");

        let ty = legs_of(&dss, "gictransformer", "ty");
        let both = [
            branch3.to_vec(),
            vec![(6, Some(9)), (7, Some(10)), (8, Some(11))],
        ]
        .concat();
        assert_eq!(pairs(&ty), both);
        assert_legs(&ty, Lumped, 0.0, ZERO, "gictransformer.ty");
        let ta = legs_of(&dss, "gictransformer", "ta");
        assert_eq!(pairs(&ta), both);
        assert_legs(&ta, Lumped, 0.0, ZERO, "gictransformer.ta");
        let tg = legs_of(&dss, "gictransformer", "tg");
        assert_eq!(pairs(&tg), branch3);
        assert_legs(&tg, Lumped, 0.0, ZERO, "gictransformer.tg");

        let half = 3.0_f64.sqrt() / 2.0;
        let one = Complex64::new(1.0, 0.0);
        let a2 = Complex64::new(-0.5, -half);
        let a = Complex64::new(-0.5, half);
        let same = |got: Option<Vec<Complex64>>, want: &[Complex64], what: &str| {
            let got = got.unwrap_or_else(|| panic!("{what}: no pattern"));
            assert_eq!(got.len(), want.len(), "{what}");
            for (k, (g, w)) in got.iter().zip(want).enumerate() {
                let e = g - w;
                assert!(
                    e.re.abs() <= 1e-12 && e.im.abs() <= 1e-12,
                    "{what} {k}: {g} vs {w}"
                );
            }
        };
        let source = elem(&dss, "vsource", "source").pos_seq_pattern();
        same(source, &[one, a2, a], "vsource.source");
        same(
            elem(&dss, "isource", "i2").pos_seq_pattern(),
            &[one, a2],
            "isource.i2",
        );
        assert_eq!(elem(&dss, "load", "w").pos_seq_pattern(), None);
        same(
            elem(&dss, "vsconverter", "vc").pos_seq_pattern(),
            &[one, a2, a],
            "vsconverter.vc",
        );
        same(
            elem(&dss, "vsconverter", "v2").pos_seq_pattern(),
            &[one, -one],
            "vsconverter.v2",
        );
        for (class, name, want) in [
            ("vsource", "vp2", vec![one, -one]),
            ("vsource", "vp1", vec![one]),
            ("isource", "i3", vec![one, a2, a]),
            ("isource", "i1", vec![one]),
            ("indmach012", "m3", vec![one, a2, a]),
            ("storage", "sb", vec![one, a2, a]),
            ("pvsystem", "pvb", vec![one, a2, a]),
            ("generator", "g7", vec![one, a2, a]),
        ] {
            let what = format!("{class}.{name}");
            same(elem(&dss, class, name).pos_seq_pattern(), &want, &what);
        }
        // Only a three-phase machine, and a three-phase unit balanced by its
        // model, imposes the positive order.
        for (class, name) in [
            ("indmach012", "m1w"),
            ("storage", "st"),
            ("storage", "sb1"),
            ("pvsystem", "pvu"),
            ("pvsystem", "pvb1"),
            ("generator", "g"),
            ("generator", "g7u"),
            ("generator", "g1b"),
            ("generator", "g7b2"),
        ] {
            assert_eq!(
                elem(&dss, class, name).pos_seq_pattern(),
                None,
                "{class}.{name}"
            );
        }

        let mut pv = balance::stiff("legspv");
        pv.command("new pvsystem.pv bus1=b phases=3 kv=12.47 kva=500 pmpp=400 irradiance=1");
        pv.command(
            "new pvsystem.pd bus1=b phases=3 conn=delta kv=12.47 kva=500 pmpp=400 irradiance=1",
        );
        balance::solve_clean(&mut pv, "the PVSystem circuit");
        let cls = pv
            .registered_classes()
            .iter()
            .find(|c| c.props.class_name().eq_ignore_ascii_case("pvsystem"))
            .unwrap();
        let sys = cls.arena.get::<PVSystem>(cls.name_to_idx["pv"]).unwrap();
        let from_stamp = -sys.base.yeq.conj() * (v_wye * v_wye);
        let nominal = -Complex64::new(sys.base.p_nominal_per_phase, sys.base.q_nominal_per_phase);
        let pmpp = Complex64::new(-400_000.0 / 3.0, 0.0);
        assert!(close_c(from_stamp, pmpp), "{from_stamp} vs {pmpp}");
        assert!(close_c(from_stamp, nominal), "{from_stamp} vs {nominal}");
        let p = legs_of(&pv, "pvsystem", "pv");
        assert_eq!(pairs(&p), wye3);
        assert_legs(&p, Lumped, v_wye, from_stamp, "pvsystem.pv");
        // Delta, the system stamps `-Yeq/3` on each leg at the line-to-line
        // voltage, and each leg generates a third of its rating as on wye.
        let sys_d = cls.arena.get::<PVSystem>(cls.name_to_idx["pd"]).unwrap();
        let from_delta_stamp = -(sys_d.base.yeq / 3.0).conj() * (12470.0 * 12470.0);
        assert!(
            close_c(from_delta_stamp, pmpp),
            "{from_delta_stamp} vs {pmpp}"
        );
        let pd = legs_of(&pv, "pvsystem", "pd");
        assert_eq!(pairs(&pd), delta3);
        assert_legs(&pd, Lumped, 12470.0, from_delta_stamp, "pvsystem.pd");

        // Grid-forming, the system stamps a source impedance in place of
        // `-Yeq`, which holds no nominal power.
        pv.command("edit pvsystem.pv controlmode=gfm");
        assert!(pv.errors().is_empty(), "{:?}", pv.errors());
        let p = legs_of(&pv, "pvsystem", "pv");
        assert_eq!(pairs(&p), wye3);
        assert_legs(&p, Lumped, v_wye, ZERO, "pvsystem.pv GFM");
    }
}

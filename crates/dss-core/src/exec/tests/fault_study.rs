//! WP7.9 FaultStudy targeted gate. A self-contained radial feeder (a source
//! behind a known `R1/X1/R0/X0`, two series lines with no shunt) where every
//! bus short-circuit impedance is analytically the series sum of the upstream
//! branch impedances — so the oracle's `Zsc1`/`Zsc0` are exact closed forms and
//! validate the whole FaultStudy path end-to-end: the per-node unit-injection
//! re-solve off the factored Y (each column of `Zsc` = a column of `Y⁻¹`), the
//! `Ysc = Zsc⁻¹` inversion, the symmetrical-component reduction, and
//! `Isc = Ysc · Voc`. All pins are dss-python 0.15.7 (`Bus.Zsc1`/`Zsc0`/`Isc`).

use crate::exec::*;

fn fault_study_dss() -> Dss {
    let mut dss = Dss::new();
    dss.command("Set DefaultBaseFrequency=60");
    dss.command(
        "New Circuit.fs basekv=12.47 phases=3 bus1=sourcebus pu=1.0 \
         R1=0.5 X1=2.0 R0=1.0 X0=4.0",
    );
    dss.command(
        "New Line.l1 bus1=sourcebus bus2=b2 phases=3 \
         r1=0.1 x1=0.3 r0=0.3 x0=0.9 c1=0 c0=0 length=2",
    );
    dss.command(
        "New Line.l2 bus1=b2 bus2=b3 phases=3 \
         r1=0.1 x1=0.3 r0=0.3 x0=0.9 c1=0 c0=0 length=1",
    );
    dss.command("Set voltagebases=[12.47]");
    dss.command("calcv");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "snap solve: {:?}", dss.errors());
    dss.command("solve mode=faultstudy");
    assert!(dss.errors().is_empty(), "faultstudy: {:?}", dss.errors());
    dss
}

#[test]
fn fault_study_zsc_and_isc_match_oracle() {
    let dss = fault_study_dss();
    {
        let sol = &dss.circuit().unwrap().solution;
        assert_eq!(sol.mode as i32, 9, "mode should be FaultStudy(9)");
    }

    // (bus, Zsc1.re, Zsc1.im, Zsc0.re, Zsc0.im): the analytic series impedances,
    // which the oracle reproduces exactly. b2 = source + l1 (len 2), b3 = + l2.
    let z1z0: [(&str, f64, f64, f64, f64); 3] = [
        ("sourcebus", 0.5, 2.0, 1.0, 4.0),
        ("b2", 0.7, 2.6, 1.6, 5.8),
        ("b3", 0.8, 2.9, 1.9, 6.7),
    ];
    for (bus, z1r, z1i, z0r, z0i) in z1z0 {
        let v = dss
            .bus_short_circuit(bus)
            .unwrap_or_else(|| panic!("bus {bus} not found"));
        let mag1 = (z1r * z1r + z1i * z1i).sqrt();
        assert!(
            (v.zsc1.re - z1r).hypot(v.zsc1.im - z1i) < 1e-9 * mag1,
            "{bus} Zsc1 = {:?} vs oracle ({z1r}, {z1i})",
            v.zsc1
        );
        let mag0 = (z0r * z0r + z0i * z0i).sqrt();
        assert!(
            (v.zsc0.re - z0r).hypot(v.zsc0.im - z0i) < 1e-9 * mag0,
            "{bus} Zsc0 = {:?} vs oracle ({z0r}, {z0i})",
            v.zsc0
        );
    }

    // sourcebus Isc, full complex per node (dss-python 0.15.7).
    let src = dss.bus_short_circuit("sourcebus").unwrap();
    let want: [(f64, f64); 3] = [
        (847.006808, -3388.027226),
        (-3357.621051, 960.484201),
        (2510.614243, 2427.543025),
    ];
    assert_eq!(src.isc.len(), 3, "sourcebus has 3 nodes");
    for (k, &(re, im)) in want.iter().enumerate() {
        let mag = (re * re + im * im).sqrt();
        assert!(
            (src.isc[k].re - re).hypot(src.isc[k].im - im) < 1e-5 * mag,
            "sourcebus Isc[{k}] = {:?} vs oracle ({re}, {im})",
            src.isc[k]
        );
    }

    // Balanced fault: equal |Isc| across the three nodes; oracle magnitudes.
    for (bus, want_mag) in [("b2", 2673.848662_f64), ("b3", 2393.214010)] {
        let v = dss.bus_short_circuit(bus).unwrap();
        for k in 0..3 {
            let mag = v.isc[k].norm();
            assert!(
                (mag - want_mag).abs() < 1e-5 * want_mag,
                "{bus} |Isc[{k}]| = {mag} vs oracle {want_mag}"
            );
        }
    }
}

/// Extended topology coverage (the paths the balanced anchor above can't reach):
/// a **single-phase** bus (the `n=1` `avg_off_diagonal` branch + a 1×1 invert), an
/// **asymmetric** bus behind a full-matrix line (a non-circulant `Ysc` → distinct
/// per-node `Isc`, so `Isc = Ysc·VBus` and the `Zsc[j,i]` indexing are genuinely
/// exercised), and a **delta-isolated** bus (a near-singular zero-sequence `Zsc`,
/// the deliberately-ignored `invert()` failure path). All pins are dss-python
/// 0.15.7 (`Bus.Zsc1`/`Zsc0`/`Isc`).
fn fault_study_topologies_dss() -> Dss {
    let mut dss = Dss::new();
    dss.command("Set DefaultBaseFrequency=60");
    dss.command(
        "New Circuit.fx basekv=12.47 phases=3 bus1=sourcebus pu=1.0 \
         R1=0.5 X1=2.0 R0=1.0 X0=4.0",
    );
    // Asymmetric full-matrix line breaks phase symmetry (distinct per-node Isc).
    dss.command(
        "New Line.la bus1=sourcebus bus2=ba phases=3 \
         rmatrix=(0.1 | 0.02 0.12 | 0.03 0.025 0.11) \
         xmatrix=(0.3 | 0.05 0.32 | 0.06 0.055 0.31) \
         cmatrix=(0 | 0 0 | 0 0 0) length=1",
    );
    // Single-phase lateral -> a 1-node bus.
    dss.command("New Line.ls bus1=ba.1 bus2=bsingle.1 phases=1 r1=0.2 x1=0.5 c1=0 length=1");
    // Delta-delta transformer -> a zero-sequence-isolated bus.
    dss.command(
        "New Transformer.tx phases=3 windings=2 xhl=5 buses=[ba, bdelta] \
         conns=[delta delta] kvs=[12.47 4.16] kvas=[1000 1000] %r=1",
    );
    dss.command("Set voltagebases=[12.47, 4.16]");
    dss.command("calcv");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "snap solve: {:?}", dss.errors());
    dss.command("solve mode=faultstudy");
    assert!(dss.errors().is_empty(), "faultstudy: {:?}", dss.errors());
    dss
}

#[test]
fn fault_study_extended_topologies_match_oracle() {
    let dss = fault_study_topologies_dss();

    // Single-phase bus: n=1, so avg_off_diagonal=0 and Zsc1 == Zsc0 == Zsc[0,0].
    let s = dss.bus_short_circuit("bsingle").unwrap();
    assert_eq!(s.nodes.len(), 1, "bsingle is single-phase");
    assert!(
        (s.zsc1.re - 0.96666665).hypot(s.zsc1.im - 3.46666664) < 1e-6 * 3.6,
        "bsingle Zsc1 = {:?}",
        s.zsc1
    );
    assert!(
        (s.zsc1 - s.zsc0).norm() < 1e-12,
        "bsingle Zsc1 must equal Zsc0 (n=1 branch): {:?} vs {:?}",
        s.zsc1,
        s.zsc0
    );
    assert!(
        (s.isc[0].re - 537.326529).hypot(s.isc[0].im + 1926.964097) < 1e-5 * 2000.48,
        "bsingle Isc = {:?}",
        s.isc[0]
    );

    // Asymmetric bus: distinct per-node Isc (the circulant case forces them equal).
    let a = dss.bus_short_circuit("ba").unwrap();
    assert!(
        (a.zsc1.re - 0.58499999).hypot(a.zsc1.im - 2.25499998) < 1e-6 * 2.33
            && (a.zsc0.re - 1.15999998).hypot(a.zsc0.im - 4.41999996) < 1e-6 * 4.57,
        "ba Zsc1={:?} Zsc0={:?}",
        a.zsc1,
        a.zsc0
    );
    let want_a: [f64; 3] = [3094.515540, 3074.371487, 3102.639937];
    // Genuinely distinct magnitudes (would be impossible for a circulant Zsc).
    assert!(
        (want_a[0] - want_a[1]).abs() > 1.0 && (want_a[2] - want_a[1]).abs() > 1.0,
        "sanity: oracle ba |Isc| are distinct"
    );
    for (k, &m) in want_a.iter().enumerate() {
        assert!(
            (a.isc[k].norm() - m).abs() < 1e-5 * m,
            "ba |Isc[{k}]| = {} vs oracle {m}",
            a.isc[k].norm()
        );
    }

    // Delta-isolated bus: the zero-sequence is open, so Zsc is near-singular and
    // Zsc0 blows up (oracle ~5.2e7j). The exact value is a conditioning floor of the
    // singular inversion (faer-vs-KLU last ulp amplified) — pin only the robust
    // facts: Zsc1 (the well-conditioned positive sequence) matches tightly, and
    // Zsc0 is huge. The fault Isc (dominated by the +seq path) still matches.
    let dd = dss.bus_short_circuit("bdelta").unwrap();
    assert!(
        (dd.zsc1.re - 0.27277148).hypot(dd.zsc1.im - 1.11623755) < 1e-5 * 1.15,
        "bdelta Zsc1 = {:?}",
        dd.zsc1
    );
    assert!(
        dd.zsc0.im.abs() > 1.0e6,
        "bdelta Zsc0 must blow up (delta-isolated zero sequence): {:?}",
        dd.zsc0
    );
    let want_dd: [f64; 3] = [2090.109843, 2087.724736, 2092.682053];
    for (k, &m) in want_dd.iter().enumerate() {
        assert!(
            (dd.isc[k].norm() - m).abs() < 1e-4 * m,
            "bdelta |Isc[{k}]| = {} vs oracle {m}",
            dd.isc[k].norm()
        );
    }
}

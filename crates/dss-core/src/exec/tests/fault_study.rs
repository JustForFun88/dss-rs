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

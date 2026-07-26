//! UPFC + UPFCControl targeted gate.
//!
//! The only corpus deck (`Version8/Distrib/Examples/UPFC_Test/UPFC_test_3.dss`)
//! ends in `show`/`plot` (Phase 8), so it cannot enter `solvable_now`. This is a
//! transcribed oracle gate: the **snapshot** portion of that deck (the circuit +
//! the mode-1 voltage-regulator UPFC + UPFCControl + the interface transformers +
//! loads + loss curve, up to the first `solve`; the daily/show/plot tail dropped)
//! run on the Rust engine and pinned against the pinned **dss-python 0.15.7**
//! oracle.
//!
//! The UPFC regulates its output-bus voltage `Vbout` to `RefkV` (0.242 kV) within
//! `Tol1` via 2 control iterations: `UPFCControl.Sample` polls the UPFC's
//! `CheckStatus`, pushes a control action, and `DoPendingAction` → `UploadCurrents`
//! recomputes the series-injection `OutCurr` / input `InCurr`, latching the `Sr0`/
//! `Sr1` shift registers.
//!
//! **Convergence-tolerance note (proven, not fudged):** the 14 mode-3 state
//! variables `Re/Im{Vbin}`/`Re/Im{Vbout}` are *mid-iteration* snapshots captured
//! one Newton iterate before convergence (`GetInjCurrents` runs before
//! `SolveSystem`). At the default 1e-4 convergence tolerance the two engines stop
//! at slightly different points within the convergence band, so those four vars
//! differ by ~4e-5 rel (and their small `Im` components by ~2e-3 rel) even though
//! the converged solution — currents, powers, and the latched `Sr0`/`Sr1` — match
//! to f64. Tightening the tolerance to 1e-12 (this deck) collapses the gap: both
//! engines then converge to the **identical fixpoint** (`Vbin = 236.41620285`, to
//! 12 digits) in the **same 17 iterations** (probed) — the CLAUDE.md proof that
//! the engines share the fixpoint, so the default-tol gap is a convergence-band
//! snapshot artifact, not a port bug. The gate therefore pins the tight-tolerance
//! fixpoint, where every variable is deterministic to f64.
//!
//! The mode-3 monitor records nothing in a snapshot solve (monitors only sample on
//! a time-series step; the oracle's `State` monitor returns `SampleCount=0` /
//! empty channels here — probed), so the 14 state variables are pinned against the
//! oracle's live f64 `ActiveCktElement.AllVariableValues` (`get_all_variables`) —
//! the authoritative read the f32 monitor channel hides (CLAUDE.md). The `State`
//! monitor is kept only to exercise the mode-3 admission path for a UPFC (its
//! 14-name header is asserted).

use crate::exec::*;
use num_complex::Complex64;

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(1.0)
}

/// The deck up to (but not including) the first `solve`.
fn upfc_dss_pre() -> Dss {
    let mut dss = Dss::new();
    dss.command("Set DefaultBaseFrequency=60");
    dss.command(
        "New circuit.UPFC3-1 bus1=SOURCE_BUS.1.0 phases=1 BasekV=7.2 pu=1 angle=0 \
         mvasc3=2000000 mvasc=20000000",
    );
    dss.command("New XYCurve.Losses npts=3 xarray=[0.9 1 1.1] yarray=[1.0143 1.008 1.0143]");
    dss.command(
        "New XfmrCode.1-ph50kVA-2 phases=1 Windings=2 ppm=0 Xhl=2.04 %noloadloss=.02 \
         kVs=[7.2 0.24] kVAs=[50 50] %Rs=[0.9 0.9] conns=[wye wye]",
    );
    dss.command(
        "New XfmrCode.UPFCInterface phases=1 Windings=3 ppm=0 Xhl=.0204 Xht=.0204 Xlt=.0136 \
         %noloadloss=.01 kVs=[0.24 0.12 0.12] kVAs=[50 50 50] %Rs=[0.006 .012 .012] \
         conns=[wye wye wye]",
    );
    dss.command(
        "New Transformer.Service50kVA Xfmrcode=1-ph50kVA-2 Buses=[Source_Bus.1.0 UPFC_Input.1]",
    );
    dss.command(
        "New upfc.TEST phases=1 bus1=UPFC_Input.1 bus2=UPFC_Output.1 refkV=0.242 mode=1 \
         losscurve=Losses TOL1=0.001 Xs=0.02",
    );
    dss.command("New UPFCControl.myUPFCCtrl");
    dss.command(
        "New Transformer.TUPFCout XfmrCode=UPFCInterface \
         Buses=[UPFC_output.1.0 LOAD_BUS.1.0 LOAD_BUS.0.2]",
    );
    dss.command("New load.LOAD120A phases=1 model=1 bus1=LOAD_BUS.1.0 kv=0.12 kw=14.98 kvar=10.08");
    dss.command("New load.LOAD120B phases=1 model=1 bus1=LOAD_BUS.2.0 kv=0.12 kw=12.38 kvar=1.71");
    dss.command("Set voltagebases=[12.47 .415 0.208]");
    dss.command("Calcv");
    dss.command("New monitor.State UPFC.Test 1 mode=3");
    // Tight tolerance pins the shared converged fixpoint (see the module note).
    dss.command("Set tolerance=1e-12");
    dss.command("Set maxiterations=100");
    dss
}

fn upfc_dss() -> Dss {
    let mut dss = upfc_dss_pre();
    dss.command("solve");
    assert!(dss.errors().is_empty(), "solve: {:?}", dss.errors());
    dss
}

#[test]
fn upfc_voltage_regulator_matches_oracle() {
    let mut dss = upfc_dss();

    // Same converged iteration counts as the oracle (dss-python 0.15.7, tol 1e-12).
    let sol = &dss.circuit().unwrap().solution;
    assert_eq!(sol.iteration, 17, "power-flow iteration count");
    assert_eq!(sol.control_iteration, 2, "control iteration count");

    // The 14 Monitor-mode-3 state variables, pinned against the oracle's live f64
    // `AllVariableValues` (the regulated Vbin/Vbout, the loss factor, and the
    // latched Sr0/Sr1 shift registers).
    //   ModeUPFC, |IUPFC|, Re/Im{Vbin}, Re/Im{Vbout}, Losses, P_UPFC, Q_UPFC,
    //   Qideal, Re/Im{Sr0[1]}, Re/Im{Sr1[1]}.
    let want_vars: [f64; 14] = [
        1.0,
        0.0,
        236.41620285,
        -1.71645501,
        242.08077307,
        -1.70106123,
        1.00946148,
        0.0,
        0.0,
        0.0,
        113.47197584,
        -332.73567446,
        -117.33900982,
        332.73567446,
    ];
    let vars = dss
        .element_variables("UPFC.TEST")
        .expect("UPFC.TEST variables");
    assert_eq!(vars.len(), 14, "UPFC exposes 14 mode-3 variables");
    let var_names = [
        "ModeUPFC",
        "IUPFC",
        "Re{Vbin}",
        "Im{Vbin}",
        "Re{Vbout}",
        "Im{Vbout}",
        "Losses",
        "P_UPFC",
        "Q_UPFC",
        "Qideal",
        "Re{Sr0}",
        "Im{Sr0}",
        "Re{Sr1}",
        "Im{Sr1}",
    ];
    for (i, (&got, &exp)) in vars.iter().zip(want_vars.iter()).enumerate() {
        assert!(
            rel(got, exp) < 1e-6,
            "UPFC var {} ({}) = {got} vs oracle {exp}",
            i + 1,
            var_names[i]
        );
    }

    // The output bus voltage is regulated to RefkV·1000 = 242 V within Tol1·242 V.
    let got_vbout = (vars[4] * vars[4] + vars[5] * vars[5]).sqrt();
    assert!(
        (1.0 - got_vbout / 242.0).abs() < 0.001,
        "Vbout regulated to RefkV within Tol1: |Vbout| = {got_vbout}, RefkV·1000 = 242"
    );

    let snaps = dss.snapshot_elements();
    let get = |name: &str| {
        snaps
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
            .unwrap_or_else(|| panic!("element {name} not found"))
    };

    // UPFC terminal currents (A, re/im per terminal) — dss-python 0.15.7.
    let upfc = get("UPFC.TEST");
    let want_curr = [
        Complex64::new(116.569321, -49.507163),
        Complex64::new(-112.702287, 49.507163),
    ];
    for (k, &c) in want_curr.iter().enumerate() {
        let got = upfc.currents[k];
        assert!(
            rel(got.re, c.re) < 1e-6 && rel(got.im, c.im) < 1e-6,
            "UPFC current[{k}] = {got} vs oracle {c}"
        );
    }
    // UPFC terminal powers (kW/kvar per terminal) — dss-python 0.15.7.
    let want_pow = [
        Complex64::new(27.643853, 11.50421),
        Complex64::new(-27.367272, -11.793019),
    ];
    for (k, &p) in want_pow.iter().enumerate() {
        let got = upfc.powers[k];
        assert!(
            rel(got.re, p.re) < 1e-6 && rel(got.im, p.im) < 1e-6,
            "UPFC power[{k}] = {got} vs oracle {p}"
        );
    }

    // The controlled-path transformer powers (kW/kvar at terminal 1) pin the
    // UPFC's effect on the surrounding network. dss-python 0.15.7.
    let svc = get("Transformer.Service50kVA");
    assert!(
        rel(svc.powers[0].re, 27.986349) < 1e-6,
        "Service50kVA P = {}",
        svc.powers[0].re
    );
    assert!(
        rel(svc.powers[0].im, 11.881373) < 1e-6,
        "Service50kVA Q = {}",
        svc.powers[0].im
    );
    let tout = get("Transformer.TUPFCout");
    assert!(
        rel(tout.powers[0].re, 27.367272) < 1e-6,
        "TUPFCout P = {}",
        tout.powers[0].re
    );
    assert!(
        rel(tout.powers[0].im, 11.793019) < 1e-6,
        "TUPFCout Q = {}",
        tout.powers[0].im
    );

    // Mode-3 monitor admission: the State monitor accepts the UPFC and exposes the
    // 14 variable names in its header (the channels record nothing in a snapshot
    // solve, matching the oracle).
    let m = dss.monitor_view("State").expect("State monitor");
    assert_eq!(
        &m.header[2..],
        [
            "ModeUPFC",
            "IUPFC",
            "Re{Vbin}",
            "Im{Vbin}",
            "Re{Vbout}",
            "Im{Vbout}",
            "Losses",
            "P_UPFC",
            "Q_UPFC",
            "Qideal",
            "Re{Sr0^[1]}",
            "Im{Sr0^[1]}",
            "Re{Sr1^[1]}",
            "Im{Sr1^[1]}",
        ],
        "mode-3 header tail = the 14 UPFC variable names"
    );
}

//! VCCS (voltage-controlled current source / HW inverter model) gates.
//!
//! The snapshot power-flow injection (`BaseCurr` at the terminal-voltage angle)
//! is gated live against the oracle by the corpus deck
//! `Version8/Distrib/Examples/VCCS/HWtest.dss` (now in `solvable_now.json`). These
//! tests pin the **dynamics** mode-3 monitor trajectory against the pinned
//! dss-python 0.15.7 oracle: the z-domain ring-buffer filter — both the
//! time-domain waveform path (`HWDyn`, a microinverter through a temporary fault)
//! and the RMS/phasor PLL path (`HWPLL`/`HWPLL3`, `RmsMode=true`). The decks are
//! transcribed verbatim from the corpus (the library curves inlined from
//! `HW_Inverters.txt`); the monitor channels are f32 on both engines, pinned at
//! the standard `1e-6` (TOLERANCE_NOTES.md).

use crate::exec::*;

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(1.0)
}

// ---------------------------------------------------------------------------
// Waveform dynamics (HWDyn): a 190 W microinverter on SourceBus.1 behind a
// temporary 1-phase fault (ontime 0.1 s), 125 steps at h = 2 ms. The mode-3
// monitor `invst` records the 6 waveform state variables (Vwave / Iwave / Irms /
// Ipeak / BP1out / Hout).
// ---------------------------------------------------------------------------

fn hwdyn_dss() -> Dss {
    let mut dss = Dss::new();
    dss.command("Set DefaultBaseFrequency=60");
    dss.command(
        "new circuit.HWDynamic basekv=0.360 pu=1.0 angle=0 phases=3 bus1=SourceBus \
         r1=0.029 x1=0.058 r0=0.014 x0=0.029",
    );
    // Microinverter library curves (HW_Inverters.txt, inlined).
    dss.command(
        "New XYcurve.bp1_micro npts=3 xarray=[-0.882023 0.0 0.882023] yarray=[-0.19 0.0 0.19]",
    );
    dss.command(
        "New XYcurve.bp2_micro npts=5 xarray=[-0.085467 -0.0202 0.0 0.0202 0.085467] \
         yarray=[4.59572 1.0 0.0 -1.0 -4.59572]",
    );
    dss.command(
        "New XYcurve.z_micro npts=14 \
         xarray=[1.0000000000000 -1.3802100241988 -0.9341004900495 1.1196620891546 \
         0.1499829369937 1.2123723040884 -0.7654392492287 -0.9113338678997 0.3694835076144 \
         0.1034830928158 0.1549604084080 -0.2209879303901 0.1373924816843 -0.0352249177928] \
         yarray=[0.0000000000000 -0.2328126380366 0.9436331671129 -0.5290906112039 \
         -1.2586072419992 0.6136072759695 0.5705618604200 1.0000000000000 -0.5053547399078 \
         -1.1387058093285 -0.0677012182563 0.5067831545610 0.4003099336092 -0.3026288423351]",
    );
    dss.command(
        "New vccs.pv Phases=1 Bus1=SourceBus.1 Prated=190 Vrated=208 Ppct=89.5 \
         bp1='bp1_micro' bp2='bp2_micro' filter='z_micro' fsample=10000",
    );
    dss.command("new fault.flt bus1=Sourcebus.1 phases=1 r=0.001 temporary=yes ontime=0.1");
    dss.command("new monitor.invst element=vccs.pv terminal=1 mode=3");
    dss.command("Set Voltagebases=[0.360]");
    dss.command("set maxiterations=100");
    dss.command("calcv");
    dss.command("set mode=snap");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "hwdyn snapshot: {:?}",
        dss.errors()
    );
    dss
}

#[test]
fn vccs_waveform_dynamics_mode3_matches_oracle() {
    let mut dss = hwdyn_dss();
    dss.command("set mode=dynamic");
    dss.command("set stepsize=0.002");
    dss.command("set number=125");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "hwdyn dynamic: {:?}", dss.errors());

    let m = dss.monitor_view("invst").expect("invst monitor");
    // Mode-3 header tail = the 6 waveform state-variable names.
    assert_eq!(
        &m.header[2..],
        ["Vwave", "Iwave", "Irms", "Ipeak", "BP1out", "Hout"],
        "mode-3 header tail = the 6 VCCS waveform variable names"
    );
    assert_eq!(m.sample_count, 125);
    assert_eq!(m.channels.len(), 6);
    let at = |ch: usize, s: usize| m.channels[ch][s] as f64;

    // Oracle (dss-python 0.15.7) `invst` channels, pinned at the standard monitor
    // `1e-6` rel (small/near-zero channels fall to the 1e-6 absolute floor via the
    // `max(|b|,1)` denominator). The fault turns on at t = 0.1 s (step 50): the
    // pre-fault rows (0, 49) sit at the steady inverter output, then Vwave collapses
    // and Ipeak (a latched running max) jumps to 1.665 and holds.
    let want: [(usize, [f64; 6]); 5] = [
        (
            0,
            [0.7286281, 0.7328382, 1.0002263, 1.0, 0.1569566, -0.0148033],
        ),
        (
            49,
            [
                0.9993688, 1.0559313, 1.1538068, 1.1675874, 0.2152779, -0.0212152,
            ],
        ),
        (
            50,
            [
                -0.0050166, 0.8153786, 1.1924487, 1.6650211, -0.0010806, -0.0164706,
            ],
        ),
        (
            60,
            [
                -0.0183520, -0.3169772, 0.4190959, 1.6650211, -0.0039533, 0.0064029,
            ],
        ),
        (
            124,
            [
                0.0084366, 0.0131103, 0.0132377, 1.6650211, 0.0018174, -0.0002648,
            ],
        ),
    ];
    for (s, row) in want {
        for (ch, &v) in row.iter().enumerate() {
            assert!(
                rel(at(ch, s), v) < 1e-6,
                "HWDyn invst[ch{ch}, sample {s}] = {} vs oracle {v}",
                at(ch, s)
            );
        }
    }

    // Ipeak is a running max: monotone non-decreasing across the whole run.
    for s in 1..125 {
        assert!(
            at(3, s) >= at(3, s - 1) - 1e-7,
            "Ipeak must be non-decreasing; dropped at sample {s}: {} -> {}",
            at(3, s - 1),
            at(3, s)
        );
    }
}

// ---------------------------------------------------------------------------
// RMS/phasor (PLL) dynamics: HWPLL (1-phase) and HWPLL3 (3-phase), RmsMode=true.
// The mode-3 monitor `invst` records the phasor state variables (Vrms / Ipwr /
// Hout / Irms / NA / NA). A temporary 1-phase fault at t = 0.1 s collapses Vrms,
// so the power-controlled current `Ipwr` rails at IMaxpu = 1.15.
// ---------------------------------------------------------------------------

fn z_pll_curve(dss: &mut Dss, xa: &str, ya: &str) {
    dss.command(&format!(
        "New XYcurve.z_pll npts=3 xarray=[{xa}] yarray=[{ya}]"
    ));
}

fn hwpll_common(dss: &mut Dss, vccs: &str) {
    dss.command("New fault.flt bus1=Sourcebus.1 phases=1 r=0.001 temporary=yes ontime=0.1");
    dss.command("new monitor.invst element=vccs.pv terminal=1 mode=3");
    dss.command("Set Voltagebases=[4.16]");
    dss.command("set maxiterations=100");
    dss.command("calcv");
    dss.command("set mode=snap");
    dss.command("solve");
    let _ = vccs;
}

fn hwpll1_dss() -> Dss {
    let mut dss = Dss::new();
    dss.command("Set DefaultBaseFrequency=60");
    dss.command(
        "new circuit.HWPLL basekv=4.16 pu=1.0 angle=0 phases=3 bus1=SourceBus \
         r1=0.029 x1=0.058 r0=0.014 x0=0.029",
    );
    z_pll_curve(&mut dss, "1.0000 -1.9852 0.9853", "0.0000 0.0148 -0.0147");
    dss.command(
        "New vccs.pv Phases=1 Bus1=SourceBus.1 Prated=125e3 Vrated=2400 Ppct=100 \
         filter='z_pll' fsample=10000 rmsmode=true imaxpu=1.15 vrmstau=0.01 irmstau=0.05",
    );
    hwpll_common(&mut dss, "pv");
    assert!(
        dss.errors().is_empty(),
        "hwpll snapshot: {:?}",
        dss.errors()
    );
    dss
}

fn hwpll3_dss() -> Dss {
    let mut dss = Dss::new();
    dss.command("Set DefaultBaseFrequency=60");
    dss.command(
        "new circuit.HWPLL3 basekv=4.16 pu=1.0 angle=0 phases=3 bus1=SourceBus \
         r1=0.029 x1=0.058 r0=0.014 x0=0.029",
    );
    z_pll_curve(
        &mut dss,
        "1.0000 -1.98515 0.98531",
        "0.0000 0.01485 -0.01469",
    );
    dss.command(
        "New vccs.pv Phases=3 Bus1=SourceBus Prated=375e3 Vrated=4160 Ppct=100 \
         filter='z_pll' fsample=10000 rmsmode=true imaxpu=1.15 vrmstau=0.01 irmstau=0.05",
    );
    hwpll_common(&mut dss, "pv");
    assert!(
        dss.errors().is_empty(),
        "hwpll3 snapshot: {:?}",
        dss.errors()
    );
    dss
}

#[test]
fn vccs_rmsmode_dynamics_mode3_matches_oracle_1phase() {
    let mut dss = hwpll1_dss();
    dss.command("set mode=dynamic");
    dss.command("set stepsize=0.0001");
    dss.command("set number=2500");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "hwpll dynamic: {:?}", dss.errors());

    let m = dss.monitor_view("invst").expect("invst monitor");
    assert_eq!(
        &m.header[2..],
        ["Vrms", "Ipwr", "Hout", "Irms", "NA", "NA"],
        "mode-3 header tail = the 6 VCCS RMS-mode variable names"
    );
    assert_eq!(m.sample_count, 2500);
    let at = |ch: usize, s: usize| m.channels[ch][s] as f64;

    // Oracle (dss-python 0.15.7) `invst` channels (RmsMode PLL). Pre-fault the
    // inverter holds rated power (Vrms≈1, Ipwr≈1); after the t=0.1 s fault Vrms
    // collapses and Ipwr rails at IMaxpu=1.15.
    let want: [(usize, [f64; 4]); 4] = [
        (0, [1.0012608, 0.9987409, 1.0000001, 1.0]),
        (1, [1.0012608, 0.9987409, 0.9999631, 0.9999999]),
        (1250, [0.0249049, 1.15, 1.1790863, 1.0926709]),
        (2499, [0.0184153, 1.15, 1.1500001, 1.1497283]),
    ];
    for (s, row) in want {
        for (ch, &v) in row.iter().enumerate() {
            assert!(
                rel(at(ch, s), v) < 1e-6,
                "HWPLL invst[ch{ch}, sample {s}] = {} vs oracle {v}",
                at(ch, s)
            );
        }
    }
    // Channels 5/6 (NA) are identically 0 across the run.
    for s in 0..2500 {
        assert!(
            at(4, s) == 0.0 && at(5, s) == 0.0,
            "NA channels nonzero at {s}"
        );
    }
}

#[test]
fn vccs_rmsmode_dynamics_mode3_matches_oracle_3phase() {
    let mut dss = hwpll3_dss();
    dss.command("set mode=dynamic");
    dss.command("set stepsize=0.001");
    dss.command("set number=250");
    dss.command("Solve");
    assert!(
        dss.errors().is_empty(),
        "hwpll3 dynamic: {:?}",
        dss.errors()
    );

    let m = dss.monitor_view("invst").expect("invst monitor");
    assert_eq!(m.sample_count, 250);
    let at = |ch: usize, s: usize| m.channels[ch][s] as f64;

    // Oracle (dss-python 0.15.7) `invst` channels. The 3-phase positive-sequence
    // PLL: Vrms collapses less than the 1-phase case (only one phase faulted), so
    // the post-fault Vrms settles near 0.60 while Ipwr rails at 1.15.
    let want: [(usize, [f64; 4]); 4] = [
        (0, [1.0006276, 0.9993728, 0.9998404, 0.9999937]),
        (1, [1.0006276, 0.9993728, 0.9996797, 0.9999814]),
        (125, [0.6054478, 1.15, 1.1781981, 1.1022637]),
        (249, [0.6032816, 1.15, 1.1500027, 1.1496766]),
    ];
    for (s, row) in want {
        for (ch, &v) in row.iter().enumerate() {
            assert!(
                rel(at(ch, s), v) < 1e-6,
                "HWPLL3 invst[ch{ch}, sample {s}] = {} vs oracle {v}",
                at(ch, s)
            );
        }
    }
}

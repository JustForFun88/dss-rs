//! Exec-layer integration tests for the PVSystem element (WP7.3 step 2):
//! the power-flow solve path and the deferred-behavior guards.

use crate::exec::Dss;

/// A minimal source -> line -> PVSystem circuit solves cleanly (the power-flow
/// injection path end to end).
#[test]
fn pvsystem_snapshot_solves_clean() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("New circuit.t basekv=12.47 phases=3 bus1=src basefreq=60");
    dss.command("New Line.l1 bus1=src bus2=b phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km");
    dss.command("New PVSystem.pv bus1=b phases=3 kV=12.47 kVA=500 Pmpp=500 pf=1.0 irradiance=1.0");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "unexpected errors: {:?}",
        dss.errors()
    );
    assert!(dss.circuit().expect("circuit").is_solved);
}

/// Grid-forming mode (`ControlMode=GFM`, WPG.13): the PVSystem becomes an
/// internal balanced voltage source (`CalcGFMVoltage` at `BaseV`) behind its
/// `CalcGFMYprim` short-circuit impedance, injecting with the sources. With no
/// local load it delivers ~0 kW at 1.0 pu and solves cleanly (no NOT_PORTED
/// error now that GFM is ported).
#[test]
fn pvsystem_gfm_mode_solves() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("New circuit.t basekv=12.47 phases=3 bus1=src basefreq=60");
    dss.command("New Line.l1 bus1=src bus2=b phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km");
    dss.command(
        "New PVSystem.pv bus1=b phases=3 kV=12.47 kVA=500 Pmpp=500 pf=1.0 \
         irradiance=1.0 ControlMode=GFM",
    );
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "GFM solve must not error now that it is ported, got: {:?}",
        dss.errors()
    );
    assert!(dss.circuit().expect("circuit").is_solved);
}

/// A mode-3 (state-variable) monitor on a PVSystem must attach and solve cleanly:
/// PVSystem is a `TPCElement`, so the monitor's metered-kind check classifies it
/// as `PcElement`. Regression guard for the fix that removed the
/// "must be a power conversion element (Load or Generator)!" error + the singular
/// Y it left behind (the `Test/PVSystemTest.dss` failure; that case stays in
/// `skipped_unsupported` only because of its later Export/Plot commands, so it
/// does not guard this fix) — audit-tests follow-up.
#[test]
fn pvsystem_accepts_mode3_monitor() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("New circuit.t basekv=12.47 phases=3 bus1=src basefreq=60");
    dss.command("New Line.l1 bus1=src bus2=b phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km");
    dss.command("New PVSystem.pv bus1=b phases=3 kV=12.47 kVA=500 Pmpp=500 pf=1.0 irradiance=1.0");
    dss.command("New Monitor.mvars element=PVSystem.pv terminal=1 mode=3");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "unexpected errors: {:?}",
        dss.errors()
    );
    assert!(dss.circuit().expect("circuit").is_solved);
}

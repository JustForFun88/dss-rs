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

/// Grid-forming mode (`ControlMode=GFM`) is a settable property (it round-trips)
/// but its solve behavior (`DoGFM_Mode`/`CalcGFMYprim`) is WP7.7. It must surface
/// an explicit "not ported" error at solve time, never silently run the regular
/// PQ model — the `NOT_PORTED`-deferral-is-never-a-silent-fallback convention
/// (audit-code follow-up).
#[test]
fn pvsystem_gfm_mode_errors_not_silent() {
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
        dss.errors()
            .iter()
            .any(|e| e.contains("grid-forming") && e.contains("WP7.7")),
        "GFM solve must emit the not-ported error, got: {:?}",
        dss.errors()
    );
}

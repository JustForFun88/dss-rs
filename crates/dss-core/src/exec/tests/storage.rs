//! Exec-layer integration tests for the Storage element (WP7.4 step 1):
//! the power-flow solve path, the SOC integration over a daily run, and the
//! deferred-behavior guards.

use crate::exec::Dss;

/// A minimal source -> line -> Storage circuit solves cleanly (the power-flow
/// injection path end to end), discharging into the feeder.
#[test]
fn storage_snapshot_solves_clean() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("New circuit.t basekv=12.47 phases=3 bus1=src basefreq=60");
    dss.command("New Line.l1 bus1=src bus2=b phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km");
    dss.command(
        "New Storage.s1 bus1=b phases=3 kV=12.47 kWrated=500 kWhrated=1000 \
         state=discharging %discharge=50",
    );
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

/// A daily run advances the integrated state of charge (`UpdateStorage` runs in
/// the time-step cleanup): a 100 kW / 200 kWh battery discharging at 50 kW with
/// a 10% reserve depletes to the reserve and flips to Idling. Guards that the
/// SOC hook is actually wired into the daily loop (not just the snapshot path).
#[test]
fn storage_daily_run_depletes_soc() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("New circuit.t basekv=12.47 phases=3 bus1=src basefreq=60");
    dss.command("New Line.l1 bus1=src bus2=b phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km");
    dss.command(
        "New Storage.s1 bus1=b phases=3 kV=12.47 kWrated=100 kWhrated=200 \
         state=discharging %discharge=50 %reserve=10 %IdlingkW=1 \
         %EffDischarge=90 %EffCharge=90 pf=1.0",
    );
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("set mode=daily number=6 stepsize=1h");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "unexpected errors: {:?}",
        dss.errors()
    );
    // Depleted to the 20 kWh reserve and turned off.
    dss.command("? Storage.s1.kWhStored");
    assert_eq!(dss.result(), "20");
    dss.command("? Storage.s1.State");
    assert_eq!(dss.result(), "Idling");
}

/// Grid-forming mode (`ControlMode=GFM`, WPG.13): the Storage becomes an internal
/// balanced voltage source (`CalcGFMVoltage` at `BaseV`) behind its `CalcGFMYprim`
/// short-circuit impedance, injecting with the sources. With no local load its
/// internal voltage matches the grid, so it delivers ~0 kW and the bus sits at
/// 1.0 pu — bit-matching the pinned oracle (converges in 2 iterations).
#[test]
fn storage_gfm_mode_solves() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("New circuit.t basekv=12.47 phases=3 bus1=src basefreq=60");
    dss.command("New Line.l1 bus1=src bus2=b phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km");
    dss.command(
        "New Storage.s1 bus1=b phases=3 kV=12.47 kWrated=500 kWhrated=1000 \
         state=discharging ControlMode=GFM",
    );
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "GFM solve must not error now that it is ported, got: {:?}",
        dss.errors()
    );
    let ckt = dss.circuit().expect("circuit exists");
    assert!(ckt.is_solved, "GFM circuit did not converge");
}

/// The **dynamics-mode** GFM branch (`DoDynamicMode`/`IntegrateStates` GFM) is
/// still NOT_PORTED (WPG.13). A `set mode=dynamics` solve over a grid-forming
/// Storage must be refused with an explicit abort — never silently inject a stale
/// current (the deferral-is-never-a-silent-fallback convention; the power-flow
/// GFM above is ported and solves).
#[test]
fn storage_gfm_dynamics_aborts_loudly() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("New circuit.t basekv=4.16 phases=3 bus1=src basefreq=60");
    dss.command("New Line.l1 bus1=src bus2=b phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km");
    dss.command(
        "New Storage.s1 bus1=b phases=3 conn=delta kV=4.16 kva=800 kWrated=800 \
         kWhrated=6000 state=discharging %R=50 %X=50 kP=0.3 KVDC=0.7 PITol=0.1 \
         ControlMode=GFM",
    );
    dss.command("set voltagebases=[4.16]");
    dss.command("calcvoltagebases");
    dss.command("solve"); // snapshot GFM: ported, solves
    assert!(
        dss.errors().is_empty(),
        "snapshot GFM must solve: {:?}",
        dss.errors()
    );
    dss.command("set mode=dynamics stepsize=0.001 number=1");
    dss.command("solve"); // dynamics GFM: NOT_PORTED, must abort loudly
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("grid-forming") && e.to_lowercase().contains("dynamics")),
        "dynamics GFM must abort with an explicit not-ported error, got: {:?}",
        dss.errors()
    );
}

/// A mode-3 (state-variable) monitor on a Storage must attach and solve cleanly:
/// Pascal mode 3 validates `BASECLASSMASK = PC_ELEMENT` and Storage is a
/// `TPCElement`, so the mode-3 check accepts `MeteredKind::Storage` alongside
/// `PcElement` (regression guard for the metered-kind branch).
#[test]
fn storage_accepts_mode3_monitor() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("New circuit.t basekv=12.47 phases=3 bus1=src basefreq=60");
    dss.command("New Line.l1 bus1=src bus2=b phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km");
    dss.command("New Storage.s1 bus1=b phases=3 kV=12.47 kWrated=500 kWhrated=1000");
    dss.command("New Monitor.mvars element=Storage.s1 terminal=1 mode=3");
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

/// A mode-7 (Storage-state) monitor must accept a Storage element: Pascal
/// validates `CLASSMASK = STORAGE_ELEMENT` (Monitor.pas `RecalcElementData`
/// case 7). The port misclassified Storage as plain `PcElement`, so every
/// valid `mode=7 element=Storage.*` monitor errored "is not a storage
/// device!" (found re-probing the StoCtrl_* corpus decks; mode-7 *sampling*
/// stays deferred, this pins only the Pascal-faithful validation).
#[test]
fn storage_accepts_mode7_monitor() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("New circuit.t basekv=12.47 phases=3 bus1=src basefreq=60");
    dss.command("New Line.l1 bus1=src bus2=b phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km");
    dss.command("New Storage.s1 bus1=b phases=3 kV=12.47 kWrated=500 kWhrated=1000");
    dss.command("New Monitor.msto element=Storage.s1 terminal=1 mode=7");
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

/// The converse of [`storage_accepts_mode7_monitor`]: a mode-7 monitor on a
/// non-Storage PC element must still raise Pascal's "is not a storage
/// device!" (`CLASSMASK` check, msg 2016002).
#[test]
fn mode7_monitor_rejects_non_storage() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("New circuit.t basekv=12.47 phases=3 bus1=src basefreq=60");
    dss.command("New Line.l1 bus1=src bus2=b phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km");
    dss.command("New Load.l1 bus1=b phases=3 kV=12.47 kW=100 pf=0.95");
    dss.command("New Monitor.msto element=Load.l1 terminal=1 mode=7");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("is not a storage device!")),
        "expected the mode-7 class error, got: {:?}",
        dss.errors()
    );
}

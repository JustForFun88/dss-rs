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
/// ported (WPG.17). A discharging grid-forming Storage black-STARTS an islanded
/// section in `set mode=dynamics`: the droop lifts the island from 0 V and the
/// inverter forms the grid, delivering ~400 kW into the island load. The oracle
/// pins (dss-python 0.15.7, 60x 1ms steps) are islbus ~0.977 pu and the Storage
/// delivering ~-400.7 kW. (SafeVoltage=0 is mandatory — the default 100 blocks the
/// black start from 0 V.) The full monitor trajectory is bit-checked against the
/// oracle by the live-gate deck `tests/corpus/controls/gfm_dynamics.dss`.
#[test]
fn storage_gfm_dynamics_matches_oracle() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("New Circuit.gfm_dyn basekv=4.16 phases=3 bus1=sourcebus");
    dss.command(
        "New Line.feeder bus1=sourcebus bus2=mainbus phases=3 r1=0.3 x1=0.6 c1=0 \
         length=1 units=km",
    );
    dss.command("New Line.sw1 bus1=mainbus bus2=islbus phases=3 switch=yes");
    dss.command(
        "New Transformer.tsto phases=3 windings=2 buses=(stobus islbus) \
         conns=(delta wye) kvs=(0.48 4.16) kvas=(1000 1000) XHL=0.5",
    );
    dss.command(
        "New Storage.batt phases=3 conn=delta bus1=stobus kV=0.48 kva=800 \
         kWrated=800 kWhrated=6000 %stored=100 %reserve=20 %IdlingkW=1 %R=50 %X=50 \
         State=DISCHARGING kP=0.3 KVDC=0.700 PITol=0.1 SafeVoltage=0 ControlMode=GFM",
    );
    dss.command("New Load.isl phases=3 bus1=islbus kV=4.16 kW=400 kvar=80 model=1");
    dss.command("New Monitor.msv element=Storage.batt terminal=1 mode=3");
    dss.command("Set voltagebases=[4.16 0.48]");
    dss.command("Calcvoltagebases");
    dss.command("open line.sw1 terminal=1");
    dss.command("solve"); // steady snapshot (islanded GFM) — converges
    assert!(
        dss.errors().is_empty(),
        "islanded GFM snapshot: {:?}",
        dss.errors()
    );
    dss.command("Set mode=dynamics stepsize=0.001 number=60 maxiterations=30");
    dss.command("solve"); // dynamics GFM black start — ported, must NOT abort
    assert!(
        dss.errors().is_empty(),
        "dynamics GFM must solve now that it is ported: {:?}",
        dss.errors()
    );

    let ckt = dss.circuit().expect("circuit");
    assert!(ckt.is_solved, "dynamics GFM island did not converge");

    // The black start energised the island: islbus lifts to ~0.977 pu (a dead GFL
    // island would sit at ~0). `kv_base` is L-N kV (SetVoltageBases).
    let bidx = ckt.bus_list.find("islbus").expect("islbus exists");
    let bus = &ckt.buses[bidx];
    let vpu = ckt.solution.node_v[bus.get_ref(0)].norm() / (bus.kv_base * 1000.0);
    assert!(
        (vpu - 0.97682).abs() < 2e-3,
        "islbus energised to oracle ~0.977 pu, got {vpu}"
    );

    // The Storage forms the grid and delivers ~400 kW (mode-3 var 3 = kWOut, the
    // abs delivered real power; a dead island would report 0).
    let m = dss.monitor_view("msv").expect("msv monitor");
    let kw_out = *m.channels[2].last().expect("kWOut samples") as f64;
    assert!(
        (kw_out - 400.7).abs() < 2.0,
        "Storage delivers oracle ~400.7 kW, got {kw_out}"
    );
}

/// FaultStudy also sets `is_dynamic_model` (Pascal `Set_Mode`, Solution.pas
/// l.2092-2094), so it reaches the (now ported, WPG.17) GFM `DoDynamicMode`. A
/// GFM DER in FaultStudy converges cleanly on the oracle — the `it[0]=0` state
/// init makes `BaseV=0` so the inverter injects nothing (kW=0), leaving its
/// `CalcGFMYprim` shunt in the short-circuit network. No abort.
#[test]
fn storage_gfm_faultstudy_converges() {
    let mut dss = gfm_der_circuit();
    dss.command("set mode=faultstudy");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "FaultStudy GFM must converge, got: {:?}",
        dss.errors()
    );
    assert!(dss.circuit().expect("circuit").is_solved);
}

/// MonteFault also sets `is_dynamic_model`, and with a Fault object present a GFM
/// DER converges cleanly (matches the oracle). NOTE: MonteFault over a circuit
/// with an **empty** Faults list is an upstream NIL-deref Access Violation
/// (`PickAFault`/`ActiveFaultObj.Randomize`, SolutionAlgs.pas l.701/725) — this is
/// independent of GFM (it reproduces with a plain GFL DER too) and is upstream UB,
/// which the port does not reproduce: `pick_a_fault` returns `None` and the direct
/// solve proceeds fault-free. Here a Fault is present, so both engines run the
/// real fault case.
#[test]
fn storage_gfm_montefault_with_fault_converges() {
    let mut dss = gfm_der_circuit();
    dss.command("New Fault.fx bus1=b phases=3 r=1 enabled=no");
    dss.command("set mode=MF");
    dss.command("set number=1");
    dss.command("set random=none"); // deterministic (no resistance jitter)
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "MonteFault GFM (fault present) must converge, got: {:?}",
        dss.errors()
    );
    assert!(dss.circuit().expect("circuit").is_solved);
}

/// Shared source → line → grid-forming Storage, solved to a converged snapshot and
/// ready to enter a dynamic mode (FaultStudy / MonteFault).
fn gfm_der_circuit() -> Dss {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("New circuit.t basekv=4.16 phases=3 bus1=src basefreq=60");
    dss.command("New Line.l1 bus1=src bus2=b phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km");
    dss.command(
        "New Storage.s1 bus1=b phases=3 conn=delta kV=4.16 kva=800 kWrated=800 \
         kWhrated=6000 state=discharging %R=50 %X=50 kP=0.3 KVDC=0.7 PITol=0.1 \
         SafeVoltage=0 ControlMode=GFM",
    );
    dss.command("set voltagebases=[4.16]");
    dss.command("calcvoltagebases");
    dss.command("solve"); // snapshot GFM: ported, solves
    assert!(
        dss.errors().is_empty(),
        "snapshot GFM must solve: {:?}",
        dss.errors()
    );
    dss
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

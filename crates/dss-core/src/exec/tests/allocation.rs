use crate::exec::*;

// ---- WP6.7: Sensor + load allocation -------------------------------------

/// A radial feeder with an EnergyMeter at the head and two ConnectedkVA-spec
/// loads. The meter's `SensorCurrent` defaults to 400 A, so `allocateloads`
/// scales the zone loads to push the metered current toward that peak.
fn allocation_feeder() -> Dss {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.a basekv=12.47 bus1=src phases=3");
    dss.command("new line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("new line.l2 bus1=b1 bus2=b2 length=1 r1=0.3 x1=0.6");
    dss.command("new load.ld1 bus1=b1 phases=3 kv=12.47 xfkva=500 allocationfactor=0.5 pf=0.9");
    dss.command("new load.ld2 bus1=b2 phases=3 kv=12.47 xfkva=800 allocationfactor=0.5 pf=0.9");
    dss.command("new energymeter.m1 element=line.l1 terminal=1");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve mode=snap");
    assert!(dss.errors().is_empty(), "build errors: {:?}", dss.errors());
    dss
}

fn close_rel(a: f64, e: f64) -> bool {
    (a - e).abs() <= 1e-3 + 1e-4 * e.abs()
}

/// `allocateloads` with the default `MaxAllocationIterations = 2`. Values
/// transcribed from the pinned oracle (`Loads.kW` / `AllocationFactor`).
#[test]
fn allocateloads_meter_drives_zone() {
    let mut dss = allocation_feeder();
    dss.command("allocateloads");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let (kw1, f1) = dss.load_alloc("ld1").unwrap();
    let (kw2, f2) = dss.load_alloc("ld2").unwrap();
    assert!(close_rel(kw1, 2867.625566), "ld1 kW {kw1}");
    assert!(close_rel(kw2, 4588.200906), "ld2 kW {kw2}");
    assert!(close_rel(f1, 6.372501), "ld1 factor {f1}");
    assert!(close_rel(f2, 6.372501), "ld2 factor {f2}");
}

/// WP-U1.5 D9 (dss_capi `fb728364`, SVN r4115): a DISABLED EnergyMeter is
/// ignored by load allocation — `TEnergyMeterObj.AllocateLoad` and
/// `TMeterElement.CalcAllocationFactors` each open with `if not Enabled then
/// Exit`. Here the meter is enabled at zone-build time (so its zone/load list is
/// populated), then disabled before `allocateloads`; the guard makes the
/// allocation a clean no-op, leaving both loads' factors at their initial `0.5`.
/// Feature-sensitive: without the guard the populated zone would be walked and
/// the factors driven to `6.3725` (the enabled-meter result of
/// `allocateloads_meter_drives_zone`). Probed 2026-07-16: capi015 keeps `0.5`
/// (0.14.5 hit an Access Violation walking the disabled meter — UB, not
/// reproduced).
#[test]
fn allocateloads_ignores_disabled_meter() {
    let mut dss = allocation_feeder();
    dss.command("edit energymeter.m1 enabled=no");
    dss.command("allocateloads");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let (_, f1) = dss.load_alloc("ld1").unwrap();
    let (_, f2) = dss.load_alloc("ld2").unwrap();
    assert!(
        close_rel(f1, 0.5),
        "ld1 factor {f1} (disabled meter: expected 0.5)"
    );
    assert!(
        close_rel(f2, 0.5),
        "ld2 factor {f2} (disabled meter: expected 0.5)"
    );
}

/// `Set NumAllocIterations=4` runs two more allocation passes, converging
/// the loads slightly (oracle-pinned).
#[test]
fn allocateloads_honors_numallociterations() {
    let mut dss = allocation_feeder();
    dss.command("set numallociterations=4");
    dss.command("allocateloads");
    let (kw1, f1) = dss.load_alloc("ld1").unwrap();
    let (kw2, _) = dss.load_alloc("ld2").unwrap();
    assert!(close_rel(kw1, 2863.277886), "ld1 kW {kw1}");
    assert!(close_rel(kw2, 4581.244618), "ld2 kW {kw2}");
    assert!(close_rel(f1, 6.36284), "ld1 factor {f1}");
}

/// `Set AllocationFactors=X` sets every load's kVA allocation factor; for a
/// ConnectedkVA-spec load `kWbase = xfkVA · factor · |pf|`.
#[test]
fn set_allocation_factors_scales_all_loads() {
    let mut dss = allocation_feeder();
    dss.command("set allocationfactors=0.8");
    let (kw1, f1) = dss.load_alloc("ld1").unwrap();
    let (kw2, f2) = dss.load_alloc("ld2").unwrap();
    assert!((kw1 - 360.0).abs() < 1e-9, "ld1 {kw1}"); // 500·0.8·0.9
    assert!((kw2 - 576.0).abs() < 1e-9, "ld2 {kw2}"); // 800·0.8·0.9
    assert!((f1 - 0.8).abs() < 1e-12);
    assert!((f2 - 0.8).abs() < 1e-12);
    // A non-positive factor is rejected (Pascal error 271).
    dss.command("set allocationfactors=0");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("Allocation Factor must be greater than zero")),
        "{:?}",
        dss.errors()
    );
}

/// A Sensor on the mid-feeder line (measured `currents` set in a *separate*
/// edit so they survive `RecalcElementData`'s `ZeroSensorArrays`) gives its
/// downstream load its own allocation target; the meter still drives the
/// upstream load. Oracle-pinned.
#[test]
fn allocateloads_with_sensor() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.a basekv=12.47 bus1=src phases=3");
    dss.command("new line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("new line.l2 bus1=b1 bus2=b2 length=1 r1=0.3 x1=0.6");
    dss.command("new load.ld1 bus1=b1 phases=3 kv=12.47 xfkva=500 allocationfactor=0.5 pf=0.9");
    dss.command("new load.ld2 bus1=b2 phases=3 kv=12.47 xfkva=800 allocationfactor=0.5 pf=0.9");
    dss.command("new energymeter.m1 element=line.l1 terminal=1");
    dss.command("new sensor.s1 element=line.l2 terminal=1");
    dss.command("edit sensor.s1 currents=[20,20,20]");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.command("allocateloads");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let (kw1, _) = dss.load_alloc("ld1").unwrap();
    let (kw2, _) = dss.load_alloc("ld2").unwrap();
    assert!(close_rel(kw1, 6780.125059), "ld1 kW {kw1}");
    assert!(close_rel(kw2, 382.584034), "ld2 kW {kw2}");
}

/// A bare Sensor (no `element=`) records the Pascal 666 error; defining the
/// element makes it valid and adopts the line's phase count.
#[test]
fn sensor_requires_element() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.a basekv=12.47 bus1=src phases=3");
    dss.command("new line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("new sensor.s1 terminal=1 kvbase=12.47");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("Circuit Element is not set")),
        "bare sensor must error, got {:?}",
        dss.errors()
    );
}

// The replay scenarios below (kWh-spec, single-phase per-phase, P/Q-sensor)
// are also covered by the data-driven gate `tests/golden_allocation.rs`;
// they are kept here as well for clearer per-case failure messages.

/// `allocateloads` over **kWh/Cfactor-spec** loads (`LoadSpec::KwhPf`): the
/// allocation factor feeds `Set_AllocationFactor`'s `c_factor` branch, not
/// `kva_allocation_factor`. Oracle-pinned `Loads.kW`/`AllocationFactor`.
#[test]
fn allocateloads_kwh_spec_loads() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.a basekv=12.47 bus1=src phases=3");
    dss.command("new line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("new line.l2 bus1=b1 bus2=b2 length=1 r1=0.3 x1=0.6");
    dss.command("new load.ld1 bus1=b1 phases=3 kv=12.47 kwh=200000 cfactor=0.3 pf=0.9");
    dss.command("new load.ld2 bus1=b2 phases=3 kv=12.47 kwh=350000 cfactor=0.3 pf=0.9");
    dss.command("new energymeter.m1 element=line.l1 terminal=1");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve mode=snap");
    dss.command("allocateloads");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let (kw1, f1) = dss.load_alloc("ld1").unwrap();
    let (kw2, f2) = dss.load_alloc("ld2").unwrap();
    assert!(close_rel(kw1, 2710.478969), "ld1 kW {kw1}");
    assert!(close_rel(kw2, 4743.338196), "ld2 kW {kw2}");
    assert!(close_rel(f1, 9.757724), "ld1 cfactor {f1}");
    assert!(close_rel(f2, 9.757724), "ld2 cfactor {f2}");
}

/// `allocateloads` over **single-phase** loads on distinct phases: each load
/// is scaled by its connected phase's `PhsAllocationFactor[ConnectedPhase]`
/// (the meter is the sensor). Unbalanced xfkVA → distinct per-phase factors
/// (an off-by-one in the phase index would cross-wire them). Oracle-pinned.
#[test]
fn allocateloads_single_phase_per_phase_factor() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.a basekv=12.47 bus1=src phases=3");
    dss.command("new line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("new load.ld1 bus1=b1.1 phases=1 kv=7.2 xfkva=200 allocationfactor=0.5 pf=0.9");
    dss.command("new load.ld2 bus1=b1.2 phases=1 kv=7.2 xfkva=400 allocationfactor=0.5 pf=0.9");
    dss.command("new load.ld3 bus1=b1.3 phases=1 kv=7.2 xfkva=600 allocationfactor=0.5 pf=0.9");
    dss.command("new energymeter.m1 element=line.l1 terminal=1");
    dss.command("set voltagebases=[12.47,7.2]");
    dss.command("calcvoltagebases");
    dss.command("solve mode=snap");
    dss.command("allocateloads");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let (kw1, f1) = dss.load_alloc("ld1").unwrap();
    let (kw2, f2) = dss.load_alloc("ld2").unwrap();
    let (kw3, f3) = dss.load_alloc("ld3").unwrap();
    // The factors are phase-distinct (13.9 / 6.95 / 4.63) — this is the part
    // that pins the connected-phase indexing.
    assert!(close_rel(f1, 13.902785), "ld1 factor {f1}");
    assert!(close_rel(f2, 6.951545), "ld2 factor {f2}");
    assert!(close_rel(f3, 4.634581), "ld3 factor {f3}");
    assert!(close_rel(kw1, 2502.501239), "ld1 kW {kw1}");
    assert!(close_rel(kw2, 2502.556163), "ld2 kW {kw2}");
    assert!(close_rel(kw3, 2502.673607), "ld3 kW {kw3}");
}

/// `allocateloads` driven by a **P/Q (kWs/kvars) Sensor**: the sensor's
/// `UpdateCurrentVector` converts |S|/Vbase to a per-phase current target
/// that then drives its downstream load, while the meter drives the
/// upstream load. Oracle-pinned.
#[test]
fn allocateloads_pq_sensor() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.a basekv=12.47 bus1=src phases=3");
    dss.command("new line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("new line.l2 bus1=b1 bus2=b2 length=1 r1=0.3 x1=0.6");
    dss.command("new load.ld1 bus1=b1 phases=3 kv=12.47 xfkva=500 allocationfactor=0.5 pf=0.9");
    dss.command("new load.ld2 bus1=b2 phases=3 kv=12.47 xfkva=800 allocationfactor=0.5 pf=0.9");
    dss.command("new energymeter.m1 element=line.l1 terminal=1");
    dss.command("new sensor.s1 element=line.l2 terminal=1 kvbase=12.47");
    // P/Q in a separate edit so they survive RecalcElementData's zeroing.
    dss.command("edit sensor.s1 kWs=[400,400,400] kvars=[200,200,200]");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve mode=snap");
    dss.command("allocateloads");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let (kw1, _) = dss.load_alloc("ld1").unwrap();
    let (kw2, _) = dss.load_alloc("ld2").unwrap();
    assert!(close_rel(kw1, 5431.462098), "ld1 kW {kw1}");
    assert!(close_rel(kw2, 1179.744649), "ld2 kW {kw2}");
}

/// Feeder shared by the `TakeSample` tests: a Sensor on line `l2` term 1
/// (bus `b1`), one 3-phase load downstream, solved.
fn sample_feeder(conn: &str) -> Dss {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.a basekv=12.47 bus1=src phases=3");
    dss.command("new line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("new line.l2 bus1=b1 bus2=b2 length=1 r1=0.3 x1=0.6");
    dss.command("new load.ld2 bus1=b2 phases=3 kv=12.47 kw=1000 pf=0.9");
    dss.command(&format!(
        "new sensor.s1 element=line.l2 terminal=1 kvbase=12.47 conn={conn}"
    ));
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve mode=snap");
    assert!(dss.errors().is_empty(), "build: {:?}", dss.errors());
    dss
}

fn cclose(a: num_complex::Complex64, re: f64, im: f64) -> bool {
    (a.re - re).abs() <= 1e-3 + 1e-4 * re.abs() && (a.im - im).abs() <= 1e-3 + 1e-4 * im.abs()
}

/// `TakeSample` (wye): `CalculatedCurrent` = the metered element's terminal-1
/// currents; `CalculatedVoltage` = the terminal node voltages. Oracle-pinned.
#[test]
fn sensor_take_sample_wye() {
    let mut dss = sample_feeder("wye");
    let (curr, volt) = dss.sensor_sample("s1").unwrap();
    assert!(cclose(curr[0], 46.530078, -22.890880), "I0 {:?}", curr[0]);
    assert!(cclose(curr[1], -43.089122, -28.850790), "I1 {:?}", curr[1]);
    assert!(cclose(curr[2], -3.440956, 51.741669), "I2 {:?}", curr[2]);
    assert!(cclose(volt[0], 7169.263686, -24.130402), "V0 {:?}", volt[0]);
    assert!(
        cclose(volt[1], -3605.529385, -6196.699277),
        "V1 {:?}",
        volt[1]
    );
    assert!(
        cclose(volt[2], -3563.734300, 6220.829680),
        "V2 {:?}",
        volt[2]
    );
}

/// `TakeSample` (delta): `CalculatedVoltage[i] = VTerminal[i] -
/// VTerminal[RotatePhases(i)]` (DeltaDirection +1 → L-L differences).
/// Oracle-pinned (computed from the same node voltages).
#[test]
fn sensor_take_sample_delta() {
    let mut dss = sample_feeder("delta");
    let (_curr, volt) = dss.sensor_sample("s1").unwrap();
    assert!(
        cclose(volt[0], 10774.793071, 6172.568875),
        "V0 {:?}",
        volt[0]
    );
    assert!(
        cclose(volt[1], -41.795084, -12417.528957),
        "V1 {:?}",
        volt[1]
    );
    assert!(
        cclose(volt[2], -10732.997986, 6244.960082),
        "V2 {:?}",
        volt[2]
    );
}

/// W3 settler: `Estimate`'s two tail legs are engine-**internal** nested
/// `ParseCommand`s (`ExecHelper.pas:4224-4225` → `Executive.pas:225` →
/// `ProcessCommand`, `ExecCommands.pas:214`), which reset only
/// `CmdResult`/`ErrorNumber`/`GlobalResult`. `SolutionAbort := FALSE  // Reset
/// for commands entered from outside` occurs 32× upstream and **only** under
/// `src/CAPI/*` (`CAPI_Text.pas:35`), i.e. in this port's [`Dss::command`]
/// wrapper (the four engine-side resets are off the command path) — so a
/// solution aborted by the allocation leg must survive the `Set showexport=yes`
/// / `Export Estimation` tail. Feature-sensitive: routing those two legs
/// through `command` instead of `process_command` clears the flag and fails the
/// last assertion (that is exactly what this test caught).
#[test]
fn estimate_tail_legs_use_the_nested_seam_and_keep_a_solution_abort() {
    // Keep the exported CSV out of the crate directory.
    let scratch = std::env::temp_dir().join(format!("dss_estimate_seam_{}", std::process::id()));
    std::fs::create_dir_all(&scratch).ok();

    let mut dss = allocation_feeder();
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));

    // The two seams, side by side: the outside-entry wrapper clears a pending
    // abort, the executive's own `ProcessCommand` leaves it alone.
    dss.circuit_mut().unwrap().solution.solution_abort = true;
    dss.process_command("Get hour");
    assert!(
        dss.circuit().unwrap().solution.solution_abort,
        "ProcessCommand must not clear SolutionAbort (that is CAPI-only)"
    );
    dss.command("Get hour");
    assert!(
        !dss.circuit().unwrap().solution.solution_abort,
        "Text_Set_Command clears SolutionAbort for outside commands"
    );

    // `Estimate` with the solution already aborted (as an allocation leg that
    // failed to build Y would leave it): the allocation's three solves refuse to
    // run ("Solution aborted.") and the tail legs must leave the flag standing.
    dss.circuit_mut().unwrap().solution.solution_abort = true;
    dss.process_command("estimate");
    assert!(
        dss.errors()
            .iter()
            .all(|e| e.message == "Solution aborted."),
        "{:?}",
        dss.errors()
    );
    assert!(
        dss.circuit().unwrap().solution.solution_abort,
        "the Estimate tail legs must not clear SolutionAbort"
    );
    // The export leg still ran (the allocation report was written).
    assert!(
        std::fs::read_dir(&scratch)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().contains("EXP_ESTIMATION")),
        "Export Estimation must still write its file"
    );
    std::fs::remove_dir_all(&scratch).ok();
}

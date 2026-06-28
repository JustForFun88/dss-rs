//! Dynamics solve-mode integration tests: the `Solve mode=dynamic`
//! predictor/corrector driver (`SolveDynamic`, WP7.7 step 1) and the Generator
//! per-element dynamics state machinery (`InitStateVars`/`IntegrateStates`/
//! `DoDynamicMode`, WP7.7 step 2a). The static-circuit tests pin the driver (no
//! machine present → the base `IntegrateStates` is a no-op, so the loop must hold
//! the snapshot fixpoint); the Kundur test pins the real Generator state vars
//! against the oracle (mode-3 monitor trajectory).

use crate::exec::*;

/// A static circuit (VSource + line + constant-PQ load, no dynamics machines)
/// solved at fundamental, ready to enter dynamics mode.
fn static_dyn_dss() -> Dss {
    let mut dss = Dss::new();
    dss.command("New circuit.dyn basekv=12.47 pu=1.0 phases=3 mvasc3=20000 mvasc1=21000");
    dss.command("New Line.l1 bus1=sourcebus bus2=loadbus length=1 units=km r1=0.1 x1=0.3");
    dss.command("New Load.ld1 bus1=loadbus phases=3 kv=12.47 kw=1000 pf=0.95 model=1");
    dss.command("New Monitor.m element=Line.l1 terminal=1 mode=0");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "fundamental: {:?}", dss.errors());
    dss
}

/// The lowest node-voltage magnitude (V) — on this VSource→line→load circuit the
/// loadbus phases, so it tracks the load operating point.
fn min_node_vmag(dss: &Dss) -> f64 {
    let ckt = dss.circuit().unwrap();
    (1..=ckt.num_nodes)
        .map(|i| ckt.solution.node_v[i].norm())
        .fold(f64::INFINITY, f64::min)
}

#[test]
fn dynamic_mode_holds_snapshot_and_samples_each_step() {
    let mut dss = static_dyn_dss();
    // Capture the converged snapshot node voltages.
    let v_snap: Vec<num_complex::Complex64> = dss.circuit().unwrap().solution.node_v.clone();

    // h defaults to 1 ms in dynamics mode; pin it (and the step count) explicitly.
    dss.command("Set mode=dynamic stepsize=0.001 number=5");
    assert!(
        dss.errors().is_empty(),
        "set mode=dynamic: {:?}",
        dss.errors()
    );
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "dynamic solve: {:?}", dss.errors());

    let ckt = dss.circuit().unwrap();
    assert!(ckt.is_solved);
    // IncrementTime runs before each step: 5 steps × 1 ms advances the clock 5 ms
    // into the hour.
    assert_eq!(ckt.solution.int_hour, 0);
    assert!(
        (ckt.solution.t - 0.005).abs() < 1e-12,
        "t = {}",
        ckt.solution.t
    );

    // With no dynamics machines, IntegratePCStates is a no-op and the
    // predictor/corrector power flow reconverges to the snapshot fixpoint every
    // step: node voltages are unchanged to solver precision (1 ppm).
    assert_eq!(ckt.solution.node_v.len(), v_snap.len());
    for (i, (a, b)) in ckt.solution.node_v.iter().zip(&v_snap).enumerate() {
        let denom = b.norm().max(1.0);
        assert!(
            (*a - *b).norm() / denom < 1e-6,
            "node {i} drifted in dynamics mode: {a} vs {b}"
        );
    }

    // Pascal `MonitorClass.SampleAll` runs once per dynamics step (the mode change
    // first reset the buffer in the `Set_Mode` tail).
    assert_eq!(
        dss.monitor_view("m").expect("m").sample_count,
        5,
        "one monitor sample per dynamics step"
    );
}

#[test]
fn dynamic_mode_load_multiplier_moves_operating_point() {
    // The new Load `GENERALTIME`/`DYNAMICMODE` `SetNominalLoad` arm scales the load
    // by `LoadMultiplier` (unless Exempt). Doubling it in dynamics mode must drive
    // the predictor/corrector loop to a *heavier* operating point — the loadbus
    // voltage drops materially below the snapshot. This (a) exercises the new
    // multiplier arm (the other tests run at loadmult = 1, where the multiply is a
    // no-op) and (b) proves the driver actually re-solves the power flow each step:
    // a driver that skipped `SolveSnap` would leave the snapshot voltages frozen.
    // The exact magnitude is oracle-pinned in step 4.
    let mut dss = static_dyn_dss();
    let v_snap = min_node_vmag(&dss);

    dss.command("Set mode=dynamic stepsize=0.001 number=3");
    dss.command("Set loadmult=2");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "dynamic solve: {:?}", dss.errors());
    assert!(dss.circuit().unwrap().is_solved);

    let v_heavy = min_node_vmag(&dss);
    assert!(
        v_heavy < v_snap - 1.0,
        "doubling loadmult must drop the loadbus voltage (the loop re-solved): \
         snapshot {v_snap} V vs heavy {v_heavy} V"
    );
}

#[test]
fn dynamic_mode_requires_a_solved_circuit() {
    // Pascal `OK_for_Dynamics` (Solution.pas l.2188): entering a dynamics mode
    // from an unsolved circuit is refused (error 486) — the machine-state init
    // would have nothing valid to capture. (No `Solve`/`CalcVoltageBases`, so
    // `IsSolved` stays false.)
    let mut dss = Dss::new();
    dss.command("New circuit.dyn basekv=12.47 pu=1.0 phases=3 mvasc3=20000");
    dss.command("New Line.l1 bus1=sourcebus bus2=b2 length=1 units=km r1=0.1 x1=0.3");
    dss.command("Set mode=dynamic");
    let errs = dss.errors().join("\n");
    assert!(
        errs.contains("must be solved in a non-dynamic mode"),
        "expected the OK_for_Dynamics guard error, got: {errs:?}"
    );
    // The refused entry leaves the mode unchanged (snapshot) — the dynamics state
    // init never ran.
    assert!(!dss.circuit().unwrap().solution.is_dynamic_model);
}

/// The canonical Kundur Example 13.1 single-machine swing case (corpus
/// `Test/Dynamic_Kundur.dss`, transcribed): a Generator behind Xd' on a step-up
/// transformer feeding two parallel lines to a quasi-ideal source, solved to the
/// steady state and ready to enter dynamics. A mode-3 monitor `g1vars` records the
/// six GenVars state variables (Frequency / Theta / Vd / PShaft / dSpeed / dTheta).
fn kundur_dss() -> Dss {
    let mut dss = Dss::new();
    dss.command("Set DefaultBaseFrequency=60");
    // @Zbase = 53.615 inlined into the RPN (the `Var` executive verb is Phase 8 /
    // unported); the resulting line reactances are identical to the deck's.
    dss.command(
        "New Circuit.SimpleDemo BasekV=345 pu=0.90081 phases=3 \
         Angle=0.0 Model=ideal puZideal=[1.0e-7, 0.00001] BaseMVA=2220",
    );
    dss.command(
        "New Line.Source_HT_1 Bus1=SourceBus Bus2=HT R1=0 X1=(0.5 53.615 *) \
         R0=0 X0=(0.5 53.615 *) C1=0 C0=0 length=1 Units=mi",
    );
    dss.command(
        "New Line.Source_HT_2 Bus1=SourceBus Bus2=HT R1=0 X1=(0.93 53.615 *) \
         R0=0 X0=(0.93 53.615 *) C1=0 C0=0 length=1 Units=mi",
    );
    dss.command(
        "New Transformer.Step_Up Phases=3 Windings=2 XHL=15 ppm=0 \
         buses=(HT LT) conns='wye wye' kvs=\"345 24\" kvas=\"2220000 2220000\" %Loadloss=0",
    );
    dss.command(
        "New Generator.G1 Bus1=LT kV=24 kW=(2220000 0.9 *) kvar=(2220000 0.436 *) \
         Model=1 vminpu=0.80 Vmaxpu=1.4 MVA=2220 XRdp=1e12 Xdp=0.3 Xdpp=0.25 H=3.5 D=0",
    );
    dss.command("set voltagebases=[345, 24]");
    dss.command("calcv");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "kundur steady solve: {:?}",
        dss.errors()
    );
    dss.command("New Monitor.g1vars Generator.G1 Term=1 mode=3");
    dss
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(1.0)
}

/// WP7.7 step-2a focused oracle gate: the Generator dynamics state variables on
/// the undisturbed Kundur run match the pinned dss-python 0.15.7 oracle. The deck
/// enters dynamics (`h=0.001`, 1 step) then runs 1000 more steps with no
/// disturbance; the swing equation sits at its fixpoint, so every mode-3 sample
/// holds the operating point constant. Values are the oracle's `g1vars` monitor
/// channels (captured with the pinned engine; channels are f32 on both sides).
#[test]
fn generator_dynamics_mode3_holds_operating_point_vs_oracle() {
    let mut dss = kundur_dss();
    dss.command("solve mode=dynamic h=0.001 number=1");
    assert!(
        dss.errors().is_empty(),
        "enter dynamics: {:?}",
        dss.errors()
    );
    dss.command("Solve number=1000");
    assert!(dss.errors().is_empty(), "dynamic run: {:?}", dss.errors());

    let m = dss.monitor_view("g1vars").expect("g1vars monitor");
    // The offline `MonitorView` header keeps the two leading time columns
    // (`hour`/`t(sec)`) that the C-API `Monitors_Get_Header` strips (WP7.6 step 3);
    // the 6 GenVars names follow. `channel(i)` already skips the time columns, so
    // `channels[0..6]` line up with the oracle's `Channel(1..6)`.
    assert_eq!(
        &m.header[2..],
        [
            "Frequency",
            "Theta (Deg)",
            "Vd",
            "PShaft",
            "dSpeed (Deg/sec)",
            "dTheta (Deg)"
        ],
        "mode-3 header tail = the 6 GenVars variable names"
    );
    // 1 step (enter) + 1000 steps = 1001 samples (the monitor was reset at the
    // `Set mode=dynamic` tail, then sampled once per dynamics step).
    assert_eq!(m.sample_count, 1001);
    assert_eq!(m.channels.len(), 6);

    // Oracle (dss-python 0.15.7) — the undisturbed segment is the swing-equation
    // fixpoint: the last sample equals the init operating point.
    let last = |ch: usize| *m.channels[ch].last().expect("samples") as f64;
    assert!(
        (last(0) - 60.0).abs() < 1e-4,
        "Frequency (Hz) = {}",
        last(0)
    );
    assert!(rel(last(1), 41.77272) < 1e-5, "Theta (deg) = {}", last(1));
    assert!(rel(last(2), 1.1625859) < 1e-5, "Vd (pu) = {}", last(2));
    assert!(rel(last(3), 1.9979999e9) < 1e-5, "PShaft (W) = {}", last(3));
    // dSpeed / dTheta sit at numerical-noise zero on the undisturbed run.
    assert!(last(4).abs() < 1e-3, "dSpeed (deg/s) = {}", last(4));
    assert!(last(5).abs() < 1e-4, "dTheta (deg) = {}", last(5));

    // The fixpoint holds for the whole run, not just the last sample.
    for (s, &v) in m.channels[1].iter().enumerate() {
        assert!(
            rel(v as f64, 41.77272) < 1e-5,
            "Theta drifted at sample {s}: {v}"
        );
    }
}

/// The fault response: a 3-phase bolted fault at HT drops the generator's
/// electrical power, so the rotor accelerates and the angle Theta climbs. This
/// drives `IntegrateStates` with a genuine disturbance (not the fixpoint hold of
/// the steady test) and matches the oracle elementwise through the whole fault.
#[test]
fn generator_dynamics_fault_response_matches_oracle_kundur() {
    let mut dss = kundur_dss();
    dss.command("solve mode=dynamic h=0.001 number=1");
    dss.command("Solve number=1000");
    // Apply the 3-phase fault at HT and integrate 70 ms.
    dss.command("New fault.F1 phases=3 Bus1=HT");
    dss.command("Solve number=70");
    assert!(dss.errors().is_empty(), "fault run: {:?}", dss.errors());

    let m = dss.monitor_view("g1vars").expect("g1vars monitor");
    assert_eq!(m.sample_count, 1 + 1000 + 70);
    let theta = &m.channels[1];
    let freq = &m.channels[0];

    // Pre-fault (sample 1000) the rotor sits at the steady angle; the fault then
    // accelerates it monotonically. Oracle (dss-python 0.15.7): Theta rises
    // 41.77272 -> 48.47866 deg over the 70 fault steps; Frequency reaches 60.536 Hz.
    assert!(rel(theta[1000] as f64, 41.77272) < 1e-5, "pre-fault Theta");
    let last = theta[1070] as f64;
    assert!(
        rel(last, 48.47866) < 1e-4,
        "end-of-fault Theta (deg) = {last}"
    );
    assert!(
        rel(freq[1070] as f64, 60.536129) < 1e-4,
        "end-of-fault Frequency (Hz) = {}",
        freq[1070]
    );
    // The rotor accelerates throughout the fault (Theta non-decreasing).
    for s in 1001..=1070 {
        assert!(
            theta[s] >= theta[s - 1] - 1.0e-4,
            "Theta must rise under fault; dropped at sample {s}: {} -> {}",
            theta[s - 1],
            theta[s]
        );
    }
}

/// The full Kundur Example 13.1 transient-stability swing: the 3-phase fault is
/// cleared after 70 ms by **opening the weaker parallel line**, then the machine
/// rides out a 10 s undamped (D=0) swing. The rotor angle Theta oscillates between
/// the oracle's trough and first peak — the end-to-end `IntegrateStates` +
/// network-reconfiguration test.
///
/// Clearing by `Open Line.Source_HT_2` is the case that pins the `Open` exec verb
/// with an omitted `term=`: it must open the active terminal (terminal 1), not
/// no-op (`command.rs` `do_open_close_cmd`, Pascal `DoOpenCmd`/`Set_ActiveTerminal`).
/// With the line left connected the generator over-delivers and the swing inverts;
/// the oracle-pinned min/max below is the regression guard. (`Disable Fault.F1` is
/// an unported verb, so the fault is cleared with the equivalent `enabled=no`.)
#[test]
fn generator_dynamics_swing_matches_oracle_kundur() {
    let mut dss = kundur_dss();
    dss.command("solve mode=dynamic h=0.001 number=1");
    dss.command("Solve number=1000");
    dss.command("New fault.F1 phases=3 Bus1=HT");
    dss.command("Solve number=70");
    dss.command("Edit Fault.F1 enabled=no");
    dss.command("Open Line.Source_HT_2");
    dss.command("Solve Number=10000");
    assert!(dss.errors().is_empty(), "swing run: {:?}", dss.errors());

    let m = dss.monitor_view("g1vars").expect("g1vars monitor");
    assert_eq!(m.sample_count, 1 + 1000 + 70 + 10000);
    let theta = &m.channels[1];
    let tmin = theta.iter().fold(f64::INFINITY, |a, &b| a.min(b as f64));
    let tmax = theta
        .iter()
        .fold(f64::NEG_INFINITY, |a, &b| a.max(b as f64));
    // Oracle (dss-python 0.15.7): the undamped rotor swing reaches min 24.18569 deg
    // (trough) and max 98.131889 deg (first peak) over the 10 s ride-out.
    assert!(rel(tmin, 24.18569) < 1e-4, "Theta swing min (deg) = {tmin}");
    assert!(
        rel(tmax, 98.131889) < 1e-4,
        "Theta swing max (deg) = {tmax}"
    );
}

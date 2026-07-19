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
    let errs = dss.error_texts().join("\n");
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
/// disturbance; the macro operating point (Frequency / Theta / Vd / PShaft) holds
/// constant, while the derivative channels (dSpeed / dTheta) slowly ring around the
/// fixpoint — they are a catastrophic-cancellation residual, see the dSpeed pin
/// below. Values are the oracle's `g1vars` monitor channels (captured with the
/// pinned engine; channels are f32 on both sides).
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
    // All channels pinned at the standard monitor `1e-6` (TOLERANCE_NOTES.md). The
    // macro channels (Frequency / Theta / Vd / PShaft) match the oracle to the f32
    // floor; the cancellation-residual channels (dSpeed / dTheta) match to ~6e-8
    // f64 / exactly 1 f32-ulp (≈9e-8) — see the dSpeed note.
    assert!(rel(last(0), 60.0) < 1e-6, "Frequency (Hz) = {}", last(0));
    assert!(rel(last(1), 41.77272) < 1e-6, "Theta (deg) = {}", last(1));
    assert!(rel(last(2), 1.1625859) < 1e-6, "Vd (pu) = {}", last(2));
    assert!(rel(last(3), 1.9979999e9) < 1e-6, "PShaft (W) = {}", last(3));
    // dSpeed / dTheta are NOT zero and NOT constant: they are the slow ring of the
    // trapezoidal integrator around the fixpoint, i.e. `dSpeed = (Pshaft +
    // TracePower.re) / Mmass` where `Pshaft` is frozen at init (`Get_Power`) and
    // `TracePower.re` is recomputed every step (`TerminalPowerIn`). Those two
    // summands are ≈±2e9 W and nearly cancel: the residual is ≈31 W = 1.5e-8 rel.
    // Both summands match the oracle to the f64 solver floor (PShaft 1 ulp,
    // TracePower 7 ulp on 2e9), so their ≈31 W difference matches to ~8 ulp and
    // dSpeed = difference / Mmass inherits a ~6e-8-rel f64 floor — which f32 monitor
    // storage then quantizes to exactly 1 ulp (≈9e-8). That is the cancellation
    // floor (amplified faer-vs-KLU rounding), NOT engine error: it cannot be
    // tightened without KLU-bit-identical arithmetic, and "fixing" the residual to 0
    // would diverge from the oracle. Pinned against the oracle's actual value (one
    // point on the slow ring at sample 1000), not against 0.
    assert!(
        rel(last(4), -4.3093074e-5) < 1e-6,
        "dSpeed (deg/s) = {}",
        last(4)
    );
    assert!(
        rel(last(5), -1.9431876e-7) < 1e-6,
        "dTheta (deg) = {}",
        last(5)
    );

    // The fixpoint holds for the whole run, not just the last sample.
    for (s, &v) in m.channels[1].iter().enumerate() {
        assert!(
            rel(v as f64, 41.77272) < 1e-6,
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
    assert!(rel(theta[1000] as f64, 41.77272) < 1e-6, "pre-fault Theta");
    let last = theta[1070] as f64;
    assert!(
        rel(last, 48.47866) < 1e-6,
        "end-of-fault Theta (deg) = {last}"
    );
    assert!(
        rel(freq[1070] as f64, 60.536129) < 1e-6,
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
    // (trough) and max 98.131889 deg (first peak) over the 10 s ride-out. Pinned at
    // the standard monitor `1e-6` (measured match 3.0e-9 / 3.5e-9).
    assert!(rel(tmin, 24.18569) < 1e-6, "Theta swing min (deg) = {tmin}");
    assert!(
        rel(tmax, 98.131889) < 1e-6,
        "Theta swing max (deg) = {tmax}"
    );
}

// ===========================================================================
// WP7.7 step 3b — DynEqPCE: the Generator driven by a user DynamicExp instead of
// its built-in shaft model. The Kundur Example 13.1 deck (corpus
// `Dynamic_Expressions/Dynamic_KundurDynExp.dss`) replaces `H/D` with a 6-variable
// DynamicExp whose swing equation `Speed dt = -(Pterm + Damp*Speed - Pshaft)/Mass;
// theta dt = Speed` is mathematically identical to the built-in model — so the
// DynExp trajectory matches the classic Kundur gate (above) *and* the oracle's
// DynExp mode-3 monitor. The mode-3 monitor now records the 12 DynamicExp memory
// slots (6 vars × [value, derivative]) instead of the 6 GenVars.
// ===========================================================================

/// The Kundur deck with the shaft model replaced by a `DynamicExp`. `DynamicEq=`
/// must precede the inline state-variable initializers (`PShaft=P0 …`) and `DynOut`
/// in the `New` so the equation object is resolved/sized first (the inline
/// initializers fall through to `ParseDynVar`). @Zbase=53.615 inlined as in
/// `kundur_dss`.
fn kundur_dynexp_dss() -> Dss {
    let mut dss = Dss::new();
    dss.command("Set DefaultBaseFrequency=60");
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
        "New DynamicExp.myDiffEq nvariables=6 varnames=[Speed Mass PShaft Pterm Damp theta] \
         expression=[Speed dt = -1 Mass / ( Pterm Damp Speed * + Pshaft - ) *; theta dt = Speed]",
    );
    dss.command(
        "New Generator.G1 Bus1=LT kV=24 kW=(2220000 0.9 *) kvar=(2220000 0.436 *) \
         Model=1 vminpu=0.80 Vmaxpu=1.4 DynamicEq=myDiffEq MVA=2220 XRdp=1e12 Xdp=0.3 Xdpp=0.25 \
         Damp=0 PShaft=P0 Pterm=P Speed=0 theta=Edp Mass=(3.5 2 * 2220000000 376.99112 / *) \
         DynOut=[Speed theta]",
    );
    dss.command("set voltagebases=[345, 24]");
    dss.command("calcv");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "kundur dynexp steady solve: {:?}",
        dss.errors()
    );
    dss.command("New Monitor.g1vars Generator.G1 Term=1 mode=3");
    dss
}

/// The DynamicExp generator's mode-3 monitor records the 12 memory slots, named
/// from the `DynamicExp` variables (value + derivative per variable), undisturbed
/// at the swing-equation fixpoint. Oracle (dss-python 0.15.7) on the corpus deck.
#[test]
fn generator_dynexp_dynamics_mode3_holds_operating_point_vs_oracle() {
    let mut dss = kundur_dynexp_dss();
    dss.command("solve mode=dynamic h=0.001 number=1");
    assert!(
        dss.errors().is_empty(),
        "enter dynamics: {:?}",
        dss.errors()
    );
    dss.command("Solve number=1000");
    assert!(dss.errors().is_empty(), "dynamic run: {:?}", dss.errors());

    let m = dss.monitor_view("g1vars").expect("g1vars monitor");
    // The mode-3 header tail is the DynamicExp memory-slot names (lowercased on the
    // DynamicExp varnames); the two leading time columns are kept offline (WP7.6).
    assert_eq!(
        &m.header[2..],
        [
            "speed", "dspeed", "mass", "dmass", "pshaft", "dpshaft", "pterm", "dpterm", "damp",
            "ddamp", "theta", "dtheta"
        ],
        "mode-3 header tail = the 12 DynamicExp memory slots"
    );
    assert_eq!(m.sample_count, 1001);
    assert_eq!(m.channels.len(), 12);

    let last = |ch: usize| *m.channels[ch].last().expect("samples") as f64;
    // Oracle DynExp steady values, all pinned at the standard monitor-channel
    // `1e-6` (TOLERANCE_NOTES.md). The "small" channels (speed / dspeed / dtheta)
    // are NOT zero and NOT constant — they are the slow ring of the trapezoidal
    // integrator around the fixpoint: `Pshaft`/`Mass` are frozen at init but the
    // electrical power `Pterm` is recomputed each step, so `dspeed` is the residual
    // of two ≈±2e9 W summands (≈31 W = 1.5e-8 rel). The big channels (mass / pshaft
    // / pterm) match the oracle to the f64 solver floor; the cancellation residuals
    // (speed / dspeed / dtheta) therefore match only to ~6e-8 f64 / 1 f32-ulp — the
    // cancellation floor, not engine error (cf. the classic-Kundur dSpeed note).
    // Pinned against the oracle's actual value, not against 0. `damp` (deck-set 0,
    // no equation) is exactly 0.
    assert!(rel(last(0), -1.9431351e-7) < 1e-6, "speed = {}", last(0));
    assert!(rel(last(1), -7.521161e-7) < 1e-6, "dspeed = {}", last(1));
    assert!(rel(last(2), 41221132.0) < 1e-6, "mass = {}", last(2)); // 2HS/w0
    assert!(rel(last(4), 1.9979999e9) < 1e-6, "pshaft = {}", last(4));
    assert!(rel(last(6), 1.998e9) < 1e-6, "pterm = {}", last(6));
    assert!(last(8).abs() < 1e-9, "damp = {}", last(8)); // exactly 0
    assert!(
        rel(last(10), 0.7290715) < 1e-6,
        "theta (rad) = {}",
        last(10)
    );
    assert!(rel(last(11), -1.9431876e-7) < 1e-6, "dtheta = {}", last(11));

    // theta (rad) here equals the classic gate's Theta (41.77272 deg) — the DynExp
    // reproduces the built-in shaft model exactly. (Sanity cross-check against the
    // classic f32 pin, so deg-loose; the binding pin is the rad value above.)
    assert!(
        (last(10) * 180.0 / std::f64::consts::PI - 41.77272).abs() < 1e-3,
        "DynExp theta must equal the classic Theta in degrees"
    );

    // The fixpoint holds for the whole run (measured ≤7.9e-8 rel drift = sub-ULP).
    for (s, &v) in m.channels[10].iter().enumerate() {
        assert!(
            rel(v as f64, 0.7290715) < 1e-6,
            "theta drifted at sample {s}: {v}"
        );
    }
}

/// The DynamicExp fault response (parity with the classic `fault_response` gate):
/// a 3-phase bolted fault at HT drops electrical power, so the rotor accelerates
/// and the `theta` slot climbs monotonically. Exercises the `SolveEq` integration
/// under a genuine disturbance (not the fixpoint hold of the steady test) and pins
/// the fault-end `theta`/`speed` against the oracle.
#[test]
fn generator_dynexp_dynamics_fault_response_matches_oracle() {
    let mut dss = kundur_dynexp_dss();
    dss.command("solve mode=dynamic h=0.001 number=1");
    dss.command("Solve number=1000");
    dss.command("New fault.F1 phases=3 Bus1=HT");
    dss.command("Solve number=70");
    assert!(dss.errors().is_empty(), "fault run: {:?}", dss.errors());

    let m = dss.monitor_view("g1vars").expect("g1vars monitor");
    assert_eq!(m.sample_count, 1 + 1000 + 70);
    let theta = &m.channels[10]; // radians
    let speed = &m.channels[0]; // rad/s relative to synchronous

    // Pre-fault the rotor sits at the steady angle; the fault accelerates it.
    // Oracle (dss-python 0.15.7): theta 0.7290715 -> 0.84611225 rad
    // (= 41.77272 -> 48.478657 deg, the classic gate's 48.47866); speed -> 3.368602.
    assert!(rel(theta[1000] as f64, 0.7290715) < 1e-6, "pre-fault theta");
    assert!(
        rel(theta[1070] as f64, 0.84611225) < 1e-6,
        "end-of-fault theta (rad) = {}",
        theta[1070]
    );
    assert!(
        rel(speed[1070] as f64, 3.368602) < 1e-6,
        "end-of-fault speed = {}",
        speed[1070]
    );
    // The rotor accelerates throughout the fault (theta non-decreasing).
    for s in 1001..=1070 {
        assert!(
            theta[s] >= theta[s - 1] - 1.0e-7,
            "theta must rise under fault; dropped at sample {s}: {} -> {}",
            theta[s - 1],
            theta[s]
        );
    }
}

/// The full Kundur transient swing, DynamicExp-driven: 3-phase fault at HT,
/// cleared after 70 ms by opening the weaker line, then a 10 s undamped swing.
/// The rotor-angle `theta` (state-variable slot, in radians) oscillates between
/// the oracle's trough and first peak — matching the classic gate's swing scaled
/// by π/180 (the classic reports degrees, the DynExp slot is the raw radian state).
#[test]
fn generator_dynexp_dynamics_swing_matches_oracle_kundur() {
    let mut dss = kundur_dynexp_dss();
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
    let theta = &m.channels[10]; // theta state slot, radians
    let tmin = theta.iter().fold(f64::INFINITY, |a, &b| a.min(b as f64));
    let tmax = theta
        .iter()
        .fold(f64::NEG_INFINITY, |a, &b| a.max(b as f64));
    // Oracle (dss-python 0.15.7): theta swings between 0.42211992 and 1.7127246 rad
    // (= 24.18569 / 98.131889 deg, the classic gate's values). Pinned at the
    // standard monitor-channel `1e-6` rel (measured match 1.1e-8 / 2.0e-8).
    assert!(
        rel(tmin, 0.42211992) < 1e-6,
        "theta swing min (rad) = {tmin}"
    );
    assert!(
        rel(tmax, 1.7127246) < 1e-6,
        "theta swing max (rad) = {tmax}"
    );
    assert!(
        (tmin * 180.0 / std::f64::consts::PI - 24.18569).abs() < 1e-2
            && (tmax * 180.0 / std::f64::consts::PI - 98.131889).abs() < 1e-2,
        "DynExp swing must equal the classic Kundur swing in degrees"
    );
}

// ===========================================================================
// WP7.7 step 2b — PVSystem / Storage grid-following inverter dynamics
// (InvDynamics.TInvDynamicVars). The classic-+inverter state-variable interface
// (PVSystem 22 vars = 13 classic + 9 InvDyn; Storage 34 = 25 + 9) is recorded by
// a mode-3 monitor and pinned against the dss-python 0.15.7 oracle. The decks are
// transcribed verbatim from `tools/golden`'s oracle probe (scratchpad
// probe_inv_dyn.py); GFM / DynamicEq / UserModel paths are NOT exercised (deferred).
// ===========================================================================

/// PVSystem grid-following dynamics deck: a quasi-ideal source → short line → a
/// 3-phase PVSystem, solved to steady state and ready to enter dynamics. A mode-3
/// monitor `pvvars` records the 22 PVSystem state variables.
fn pv_dyn_dss() -> Dss {
    let mut dss = Dss::new();
    dss.command("Set DefaultBaseFrequency=60");
    dss.command(
        "New Circuit.pvdyn basekv=12.47 pu=1.0 phases=3 bus1=sourcebus \
         mvasc3=20000 mvasc1=21000",
    );
    dss.command("New Line.l1 bus1=sourcebus bus2=pvbus length=0.5 units=km r1=0.1 x1=0.3");
    dss.command(
        "New PVSystem.pv1 phases=3 bus1=pvbus kv=12.47 kVA=600 Pmpp=500 \
         irradiance=1 %cutin=0.1 %cutout=0.1 kvar=0",
    );
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve");
    assert!(
        dss.errors().is_empty(),
        "pv steady solve: {:?}",
        dss.errors()
    );
    dss.command("New Monitor.pvvars PVSystem.pv1 Term=1 mode=3");
    dss
}

/// Storage grid-following dynamics deck: a discharging 3-phase Storage on a short
/// feeder, solved to steady and ready to enter dynamics. A mode-3 monitor
/// `stovars` records the 34 Storage state variables (incl. the SOC `kWh`, which
/// must integrate during the dynamics run — Pascal `UpdateStorage` exits early
/// only for `IsDynamicModel AND IsUserModel`, and user models are NOT_PORTED).
fn sto_dyn_dss() -> Dss {
    let mut dss = Dss::new();
    dss.command("Set DefaultBaseFrequency=60");
    dss.command(
        "New Circuit.stodyn basekv=12.47 pu=1.0 phases=3 bus1=sourcebus \
         mvasc3=20000 mvasc1=21000",
    );
    dss.command("New Line.l1 bus1=sourcebus bus2=stobus length=0.5 units=km r1=0.1 x1=0.3");
    dss.command(
        "New Storage.st1 phases=3 bus1=stobus kv=12.47 kWrated=500 kWhrated=1000 \
         kWhstored=1000 %stored=100 state=discharging %discharge=100 \
         %cutin=0.1 %cutout=0.1",
    );
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve");
    assert!(
        dss.errors().is_empty(),
        "sto steady solve: {:?}",
        dss.errors()
    );
    dss.command("New Monitor.stovars Storage.st1 Term=1 mode=3");
    dss
}

const PV_VAR_NAMES: [&str; 22] = [
    "Irradiance",
    "PanelkW",
    "P_TFactor",
    "Efficiency",
    "Vreg",
    "Vavg (DRC)",
    "volt-var",
    "volt-watt",
    "DRC",
    "VV_DRC",
    "watt-pf",
    "watt-var",
    "kW_out_desired",
    "Grid voltage",
    "di/dt",
    "it",
    "it History",
    "Rated VDC",
    "Avg duty cycle",
    "Target (Amps)",
    "Series L",
    "Max. Amps (phase)",
];

const STO_VAR_NAMES: [&str; 34] = [
    "kWh",
    "State",
    "kWOut",
    "kWIn",
    "kvarOut",
    "DCkW",
    "kWTotalLosses",
    "kWInvLosses",
    "kWIdlingLosses",
    "kWChDchLosses",
    "kWh Chng",
    "InvEff",
    "InverterON",
    "Vref",
    "Vavg (DRC)",
    "VV Oper",
    "VW Oper",
    "DRC Oper",
    "VV_DRC Oper",
    "WP Oper",
    "WV Oper",
    "kWDesired",
    "kW VW Limit",
    "Limit kWOut Function",
    "kVA Exceeded",
    "Grid voltage",
    "di/dt",
    "it",
    "it History",
    "Rated VDC",
    "Avg duty cycle",
    "Target (Amps)",
    "Series L",
    "Max. Amps (phase)",
];

/// PVSystem dynamics, undisturbed: the grid-following inverter integrates its
/// filter current to the steady operating point. The mode-3 monitor trajectory
/// (every InvDyn + classic variable) matches the oracle. The duty cycle saturates
/// at 1, so `it` relaxes from its init value (20.55) to the rail-limited steady
/// value (6.17) while `di/dt` decays to 0 — a genuine integration transient.
#[test]
fn pvsystem_dynamics_mode3_matches_oracle() {
    let mut dss = pv_dyn_dss();
    dss.command("solve mode=dynamic h=0.001 number=1");
    assert!(
        dss.errors().is_empty(),
        "enter dynamics: {:?}",
        dss.errors()
    );
    dss.command("Solve number=200");
    assert!(dss.errors().is_empty(), "dynamic run: {:?}", dss.errors());

    let m = dss.monitor_view("pvvars").expect("pvvars monitor");
    assert_eq!(
        &m.header[2..],
        PV_VAR_NAMES,
        "mode-3 header = 22 PV var names"
    );
    assert_eq!(m.sample_count, 201);
    assert_eq!(m.channels.len(), 22);
    let at = |ch: usize, s: usize| m.channels[ch][s] as f64;

    // All channels pinned at the standard monitor `1e-6` (TOLERANCE_NOTES.md;
    // measured Rust↔oracle match ~1e-8 on every channel).
    // Classic vars 1..13 are constant over the undisturbed run (no InvControl).
    for (ch, want) in [
        (0, 1.0),     // Irradiance
        (1, 500.0),   // PanelkW
        (2, 1.0),     // P_TFactor
        (3, 1.0),     // Efficiency
        (4, 9999.0),  // Vreg
        (12, 500.0),  // kW_out_desired
        (17, 8000.0), // Rated VDC
        // WP-U1.2 D7: iMaxPPhase base PanelkW(500)→FkVArating(600), ×1.2 =
        // 27.779484; matches capi015 (`? Monitor` ch, /tmp/probe_pvdyn.py). The
        // limit is non-binding (it settles at 6.17 ≪ 27.78) so the trajectory
        // channels 13-20 are unchanged.
        (21, 27.779484), // Max. Amps (phase)
    ] {
        assert!(rel(at(ch, 200), want) < 1e-6, "PV ch{ch} = {}", at(ch, 200));
    }
    // The InvControl op-flag vars (5..11) sit at the 9999 default.
    for ch in 5..=11 {
        assert!(
            rel(at(ch, 200), 9999.0) < 1e-6,
            "PV op ch{ch} = {}",
            at(ch, 200)
        );
    }

    // Inverter-dynamics channels at the start of the run (sample 0).
    assert!(
        rel(at(13, 0), 7200.5923) < 1e-6,
        "PV Vgrid[0] = {}",
        at(13, 0)
    );
    assert!(
        rel(at(14, 0), -5193.585) < 1e-6,
        "PV di/dt[0] = {}",
        at(14, 0)
    );
    assert!(rel(at(15, 0), 20.548918) < 1e-6, "PV it[0] = {}", at(15, 0));
    assert!(
        rel(at(16, 0), 23.145710) < 1e-6,
        "PV itHist[0] = {}",
        at(16, 0)
    );
    assert!(
        rel(at(19, 0), 23.146244) < 1e-6,
        "PV ISP[0] = {}",
        at(19, 0)
    );
    // ...and at the settled end (sample 200).
    assert!(
        rel(at(13, 200), 7199.8784) < 1e-6,
        "PV Vgrid = {}",
        at(13, 200)
    );
    // di/dt → exactly 0 at the railed steady state (the PI loop converged m so the
    // physical current derivative is 0).
    assert!(
        at(14, 200).abs() < 1e-6,
        "PV di/dt settled = {}",
        at(14, 200)
    );
    assert!(
        rel(at(15, 200), 6.1745348) < 1e-6,
        "PV it = {}",
        at(15, 200)
    );
    assert!(
        rel(at(16, 200), 6.1745348) < 1e-6,
        "PV itHist = {}",
        at(16, 200)
    );
    assert!(
        rel(at(18, 200), 1.0) < 1e-6,
        "PV duty (rail) = {}",
        at(18, 200)
    );
    assert!(
        rel(at(19, 200), 23.148539) < 1e-6,
        "PV ISP = {}",
        at(19, 200)
    );
    assert!(
        rel(at(20, 200), 0.34373245) < 1e-6,
        "PV Series L = {}",
        at(20, 200)
    );
}

/// PVSystem dynamics under a bolted 3-phase fault at the PV bus: the terminal
/// voltage collapses below MinVS, so the inverter enters safe mode — `it`/`di/dt`
/// drive to 0, the duty cycle drops to 0, and the current target is forced to the
/// 0.01 A "off" value. Pins the safe-mode branch of `SolveModulation`.
#[test]
fn pvsystem_dynamics_safe_mode_under_fault_matches_oracle() {
    let mut dss = pv_dyn_dss();
    dss.command("solve mode=dynamic h=0.001 number=1");
    dss.command("Solve number=200");
    dss.command("New Fault.F1 phases=3 Bus1=pvbus");
    dss.command("Solve number=100");
    assert!(dss.errors().is_empty(), "pv fault run: {:?}", dss.errors());

    let m = dss.monitor_view("pvvars").expect("pvvars monitor");
    assert_eq!(m.sample_count, 301);
    let last = |ch: usize| *m.channels[ch].last().expect("samples") as f64;
    // Pinned at the standard monitor `1e-6` (measured ~1e-8). The safe-mode branch
    // forces it/di/dt/itHist/duty to *exactly* 0, and ISP to the 0.01 A "off" value.
    assert!(
        rel(last(13), 4.3397388) < 1e-6,
        "PV Vgrid (faulted) = {}",
        last(13)
    );
    assert!(last(14).abs() < 1e-6, "PV di/dt (safe) = {}", last(14));
    assert!(last(15).abs() < 1e-6, "PV it (safe) = {}", last(15));
    assert!(last(16).abs() < 1e-6, "PV itHist (safe) = {}", last(16));
    assert!(last(18).abs() < 1e-6, "PV duty (safe) = {}", last(18));
    assert!(rel(last(19), 0.01) < 1e-6, "PV ISP (off) = {}", last(19));
}

/// Storage grid-following dynamics, undisturbed: a discharging Storage ramps its
/// inverter current to deliver its rated 500 kW, the duty cycle is PI-controlled
/// (not railed), and the state of charge `kWh` integrates downward. Every mode-3
/// channel — incl. the discharge/idle/total/inverter loss breakdown and the SOC
/// trajectory — matches the oracle.
#[test]
fn storage_dynamics_mode3_matches_oracle() {
    let mut dss = sto_dyn_dss();
    dss.command("solve mode=dynamic h=0.001 number=1");
    assert!(
        dss.errors().is_empty(),
        "enter dynamics: {:?}",
        dss.errors()
    );
    dss.command("Solve number=200");
    assert!(dss.errors().is_empty(), "dynamic run: {:?}", dss.errors());

    let m = dss.monitor_view("stovars").expect("stovars monitor");
    assert_eq!(
        &m.header[2..],
        STO_VAR_NAMES,
        "mode-3 header = 34 Storage var names"
    );
    assert_eq!(m.sample_count, 201);
    assert_eq!(m.channels.len(), 34);
    let at = |ch: usize, s: usize| m.channels[ch][s] as f64;

    // SOC integrates during the dynamics run (the WP7.4 `is_dynamic_model`
    // early-return in `update_storage` was a latent simplification corrected here):
    // kWh starts at the rated 1000 and drops monotonically as the unit discharges.
    // All value channels pinned at the standard monitor `1e-6` (measured ~1e-8).
    assert!(rel(at(0, 0), 1000.0) < 1e-9, "SOC[0] = {}", at(0, 0));
    assert!(
        rel(1000.0 - at(0, 100), 0.004699707) < 1e-6,
        "SOC drop@100 = {}",
        1000.0 - at(0, 100)
    );
    assert!(
        rel(1000.0 - at(0, 200), 0.020080566) < 1e-6,
        "SOC drop@200 = {}",
        1000.0 - at(0, 200)
    );

    // Last-sample state vector (steady discharge at rated power).
    for (ch, want) in [
        (1usize, 1.0),    // State = DISCHARGING
        (2, 500.08337),   // kWOut
        (5, 500.08337),   // DCkW
        (6, 61.120377),   // kWTotalLosses
        (8, 5.0),         // kWIdlingLosses
        (9, 56.120377),   // kWChDchLosses
        (11, 1.0),        // InvEff
        (12, 1.0),        // InverterON
        (21, 500.0),      // kWDesired
        (24, 1.0),        // kVA Exceeded
        (25, 7200.7583),  // Grid voltage
        (26, 111.64898),  // di/dt
        (27, 44.981205),  // it
        (28, 44.925381),  // it History
        (29, 8000.0),     // Rated VDC
        (30, 0.90585142), // Avg duty cycle
        (31, 23.145710),  // Target (Amps)
        (32, 0.41247895), // Series L
    ] {
        assert!(
            rel(at(ch, 200), want) < 1e-6,
            "STO ch{ch} = {}",
            at(ch, 200)
        );
    }
    // kWInvLosses is exactly 0 (ideal inverter — the loss decomposition
    // 61.120377 = 56.120377 + 5.0 + 0 pins it). kWIn (discharging) and kvarOut
    // (no kvar dispatched) are near-zero kW/kvar at the monitor abs floor (1e-4,
    // TOLERANCE_NOTES.md — a flat kW floor for a value that is physically 0).
    assert!(at(7, 200).abs() < 1e-6, "STO kWInvLosses = {}", at(7, 200));
    assert!(at(3, 200).abs() < 1e-4, "STO kWIn = {}", at(3, 200));
    // Full-interface parity (audit-tests follow-up): pin the remaining classic
    // channels by value, not only by header name — the InvControl op-flag defaults
    // (9999), the VW kW limit, and the static caps. A regression corrupting any of
    // these on Storage would otherwise slip past on the name check alone.
    for ch in [13usize, 14, 15, 16, 17, 18, 19, 20, 22] {
        assert!(
            rel(at(ch, 200), 9999.0) < 1e-6,
            "STO default ch{ch} = {}",
            at(ch, 200)
        );
    }
    assert!(
        rel(at(23, 200), 500.0) < 1e-6,
        "STO Limit kWOut = {}",
        at(23, 200)
    );
    assert!(
        rel(at(33, 200), 23.149570) < 1e-6,
        "STO Max. Amps = {}",
        at(33, 200)
    );
    assert!(at(4, 200).abs() < 1e-4, "STO kvarOut = {}", at(4, 200)); // ~0 (monitor floor)
    assert!(
        at(10, 200) < 0.0 && at(10, 200).abs() < 1e-3,
        "STO kWh Chng = {}",
        at(10, 200)
    );

    // The current ramp from rest (sample 0) is PI-controlled, not railed. (it[0]
    // is sub-unity, so the `rel` denominator floors to 1 → a 1e-6 absolute band,
    // which still distinguishes it from 0/frozen — audit-tests.)
    assert!(
        rel(at(26, 0), 2.9092627) < 1e-6,
        "STO di/dt[0] = {}",
        at(26, 0)
    );
    assert!(
        rel(at(27, 0), 0.0014546313) < 1e-6,
        "STO it[0] = {}",
        at(27, 0)
    );
    assert!(at(28, 0).abs() < 1e-6, "STO itHist[0] = {}", at(28, 0)); // exactly 0 at init
    assert!(
        rel(at(30, 0), 0.90009481) < 1e-6,
        "STO duty[0] = {}",
        at(30, 0)
    );
    // Mid-run the inverter is still ramping (kWOut < rated).
    assert!(
        rel(at(2, 100), 415.29892) < 1e-6,
        "STO kWOut@100 = {}",
        at(2, 100)
    );
    assert!(
        rel(at(27, 100), 19.225319) < 1e-6,
        "STO it@100 = {}",
        at(27, 100)
    );
}

/// Storage dynamics under a bolted 3-phase fault: the terminal voltage collapses,
/// so the discharging inverter trips to IDLING (State 1 → 0), output goes to 0,
/// and the SOC stops discharging. Pins the `IntegrateStates` MinVS/MaxVS trip
/// branch and the state-flip path.
#[test]
fn storage_dynamics_trips_to_idle_under_fault_matches_oracle() {
    let mut dss = sto_dyn_dss();
    dss.command("solve mode=dynamic h=0.001 number=1");
    dss.command("Solve number=200");
    dss.command("New Fault.F1 phases=3 Bus1=stobus");
    dss.command("Solve number=100");
    assert!(dss.errors().is_empty(), "sto fault run: {:?}", dss.errors());

    let m = dss.monitor_view("stovars").expect("stovars monitor");
    assert_eq!(m.sample_count, 301);
    let last = |ch: usize| *m.channels[ch].last().expect("samples") as f64;
    // The idle state forces State/kWOut/kWDesired to exactly 0; SOC is frozen.
    // Pinned at the standard monitor `1e-6` (measured ~1e-8).
    assert!(
        (last(1) - 0.0).abs() < 1e-6,
        "STO State (idle) = {}",
        last(1)
    );
    assert!(last(2).abs() < 1e-6, "STO kWOut (idle) = {}", last(2));
    assert!(last(21).abs() < 1e-6, "STO kWDesired (idle) = {}", last(21));
    // SOC stopped discharging near where it was when the fault hit.
    assert!(
        rel(last(0), 999.9797) < 1e-6,
        "STO SOC (frozen) = {}",
        last(0)
    );
}

// ===========================================================================
// WPG.17 — grid-forming (GFM) inverter dynamics black start.
// A discharging Storage / a PVSystem in ControlMode=GFM forms the voltage of an
// ISLANDED section from 0 V (the VDelta droop ramps ISPDelta, FixPhaseAngle locks
// the phase), delivering the island load. Pinned against dss-python 0.15.7.
// SafeVoltage=0 is mandatory (the default 100 MinVS blocks the lift-off from 0 V).
// ===========================================================================

/// PVSystem grid-forming dynamics black start (the PVSystem GFM arms: `it := 0`
/// init, the `IMaxPPhase`-overwrite ramp/clamp, `FixPhaseAngle`, and the
/// `DoDynamicMode` internal-voltage-source injection). An islanded PVSystem forms
/// the grid and supplies a 400 kW / 80 kvar island load; the mode-3 trajectory
/// matches the oracle. (The Storage counterpart is
/// `exec::tests::storage::storage_gfm_dynamics_matches_oracle` + the live-gate
/// deck `tests/corpus/controls/gfm/gfm_dynamics.dss`.)
#[test]
fn pvsystem_gfm_dynamics_mode3_matches_oracle() {
    let mut dss = Dss::new();
    dss.command("New Circuit.pvgfm basekv=4.16 phases=3 bus1=sourcebus");
    dss.command(
        "New Line.feeder bus1=sourcebus bus2=mainbus phases=3 r1=0.3 x1=0.6 c1=0 \
         length=1 units=km",
    );
    dss.command("New Line.sw1 bus1=mainbus bus2=islbus phases=3 switch=yes");
    dss.command(
        "New Transformer.tpv phases=3 windings=2 buses=(pvbus islbus) \
         conns=(delta wye) kvs=(0.48 4.16) kvas=(1000 1000) XHL=0.5",
    );
    dss.command(
        "New PVSystem.pv phases=3 conn=delta bus1=pvbus kV=0.48 kva=800 pmpp=800 \
         irradiance=1 %R=50 %X=50 kP=0.3 KVDC=0.700 PITol=0.1 SafeVoltage=0 \
         ControlMode=GFM",
    );
    dss.command("New Load.isl phases=3 bus1=islbus kV=4.16 kW=400 kvar=80 model=1");
    dss.command("New Monitor.psv element=PVSystem.pv terminal=1 mode=3");
    dss.command("Set voltagebases=[4.16 0.48]");
    dss.command("Calcvoltagebases");
    dss.command("open line.sw1 terminal=1");
    dss.command("solve"); // islanded GFM snapshot
    assert!(
        dss.errors().is_empty(),
        "pv gfm snapshot: {:?}",
        dss.errors()
    );
    dss.command("Set mode=dynamics stepsize=0.001 number=60 maxiterations=30");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "pv gfm dynamics: {:?}",
        dss.errors()
    );

    // Island energised to ~0.977 pu.
    let ckt = dss.circuit().expect("circuit");
    assert!(ckt.is_solved);
    let bidx = ckt.bus_list.find("islbus").expect("islbus");
    let bus = &ckt.buses[bidx];
    let vpu = ckt.solution.node_v[bus.get_ref(0)].norm() / (bus.kv_base * 1000.0);
    assert!(
        (vpu - 0.97682).abs() < 2e-3,
        "islbus pu (oracle ~0.977) = {vpu}"
    );

    // Mode-3 (22 PV vars) last sample vs oracle (dss-python 0.15.7). Monitor is
    // f32; pinned at the standard monitor `1e-5` rel band (TOLERANCE_NOTES.md; the
    // black-start ramp end value is well-conditioned).
    let m = dss.monitor_view("psv").expect("psv monitor");
    assert_eq!(&m.header[2..], PV_VAR_NAMES, "mode-3 header = 22 PV vars");
    assert_eq!(m.channels.len(), 22);
    let last = |ch: usize| *m.channels[ch].last().expect("samples") as f64;
    // PanelkW (ch1), it (ch15), Target Amps (ch19), Max Amps (ch21), Grid V (ch13).
    assert!(rel(last(1), 800.0) < 1e-5, "PV PanelkW = {}", last(1));
    assert!(rel(last(15), 942.13049) < 1e-5, "PV it = {}", last(15));
    assert!(
        rel(last(19), 944.85358) < 1e-5,
        "PV Target(A) = {}",
        last(19)
    );
    assert!(
        rel(last(21), 962.25043) < 1e-5,
        "PV Max.Amps = {}",
        last(21)
    );
    assert!(rel(last(13), 271.30814) < 1e-5, "PV Grid V = {}", last(13));
}

// ===========================================================================
// WP7.7 step 3a — IndMach012 (induction machine) dynamics.
// ===========================================================================

/// The shared IndMach012 deck (self-contained reproduction of the corpus
/// `Version8/.../InductionMachine` example: a 12.47 kV source → 1500 kVA step-down
/// transformer → a 600 kvar shunt cap and a 1200 kW delta induction motor), built
/// through `calcv` but **not** solved — the caller picks the tolerance and solves.
/// Shared by the default-tolerance snapshot gate and the tight-tolerance dynamics
/// gate so the two cannot silently drift apart.
fn indmach_deck() -> Dss {
    let mut dss = Dss::new();
    dss.command("Set DefaultBaseFrequency=60");
    dss.command(
        "New Circuit.indtest basekv=12.47 pu=1.0 phases=3 bus1=src \
         mvasc3=20000 mvasc1=21000",
    );
    dss.command(
        "New Transformer.tg phases=3 windings=2 buses=(src, mbus) \
         conns=(delta,wye) kvs=(12.47,0.48) kvas=(1500,1500) xhl=5",
    );
    dss.command("New Capacitor.cg conn=wye bus1=mbus phases=3 kvar=600 kv=0.48");
    dss.command(
        "New IndMach012.m1 bus1=mbus kV=0.48 kW=1200 conn=delta kVA=1500 H=6 \
         puRs=0.048 puXs=0.075 puRr=0.018 puXr=0.12 puXm=3.8 slip=0.02 \
         SlipOption=variableslip",
    );
    dss.command("set voltagebases=[12.47, 0.48]");
    dss.command("calcv");
    dss
}

/// WP7.7 step-3a regression: the snapshot power flow of the induction motor at
/// the **default** node-voltage tolerance (1e-4) matches the pinned dss-python
/// 0.15.7 oracle exactly. The slip-Newton uses a fixed `dSdP` slope, so it lags
/// the node-voltage convergence and the snapshot stops at a not-fully-settled
/// operating point (slip 0.0159858, P 1200.687 kW — *not* the 1200 kW target).
/// Both engines must stop at that *same* point. Before the fix Rust took one
/// extra slip step at power-read time because `do_indmach_model` set
/// `iterminal_updated` without the Pascal `set_ITerminalUpdated` stamp of
/// `IterminalSolutionCount`; the post-solve `ComputeIterminal` then re-ran the
/// stateful `CalcPFlow` (a 5th slip step), landing ~5e-4 past the oracle in P.
/// The oracle values below are the pinned engine's `Powers`/`Currents` at 1e-4.
#[test]
fn indmach012_snapshot_default_tol_matches_oracle() {
    let mut dss = indmach_deck();
    // Default tolerance (1e-4) — the conditions that exposed the extra-slip-step bug.
    dss.command("Set tolerance=1e-4 maxiterations=100");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "snapshot: {:?}", dss.errors());

    // The oracle stops at iteration 4 (one control iteration, no controls).
    assert_eq!(
        dss.circuit().unwrap().solution.iteration,
        4,
        "iteration count"
    );

    let snaps = dss.snapshot_elements();
    let m = snaps
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case("IndMach012.m1"))
        .expect("motor element");
    // Terminal-1 three-phase power and the phase-a terminal current magnitude
    // (oracle dss-python 0.15.7 `Powers`/`Currents`, default tol 1e-4). The 5th
    // slip step would land P at 1200.1275 kW / |Ia| at 1593.2895 A — ~4.7e-4 off.
    let p1 = m.powers[0] + m.powers[2] + m.powers[4];
    let q1 = m.powers[1] + m.powers[3] + m.powers[5];
    let i1a = (m.currents[0].powi(2) + m.currents[1].powi(2)).sqrt();
    assert!(rel(p1, 1200.686713) < 1e-6, "P1 (kW) = {p1}");
    assert!(rel(q1, 552.830142) < 1e-6, "Q1 (kvar) = {q1}");
    assert!(rel(i1a, 1594.017119) < 1e-6, "|I1a| (A) = {i1a}");
}

/// IndMach012 dynamics deck: the shared [`indmach_deck`] solved to steady state
/// (tight tolerance, see below) with a mode-3 monitor on the 22 IndMach012 state
/// variables.
fn indmach_dyn_dss() -> Dss {
    let mut dss = indmach_deck();
    // The IndMach012 slip-Newton (fixed `dSdP` slope) converges slower than the
    // node-voltage tolerance: at the default 1e-4 the network test trips at iter 4
    // while the slip is still ~1e-4 from its root, so the snapshot stops short of
    // the true electromechanical equilibrium (P = the 1200 kW target, dSpeed ≈ 0).
    // Rust now lands on the *same* iter-4 point as the oracle (the spurious extra
    // slip step at power-read time is fixed — see
    // `indmach012_snapshot_default_tol_matches_oracle`), but to start dynamics from
    // the clean fixpoint we tighten the tolerance so both engines fully settle.
    dss.command("Set tolerance=1e-8 maxiterations=100");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "indmach steady: {:?}",
        dss.errors()
    );
    dss.command("New Monitor.mvars IndMach012.m1 Term=1 mode=3");
    dss
}

/// WP7.7 step-3a focused oracle gate: the IndMach012 dynamics state variables on
/// the undisturbed run match the pinned dss-python 0.15.7 oracle. The motor sits at
/// its slipping equilibrium, so the electrical state (slip, currents, losses,
/// power) holds constant while the rotor angle Theta drifts at the slip rate. The
/// deck is solved to a tight tolerance (`1e-8`) so the slip-Newton (which lags the
/// node-voltage convergence) reaches the true electromechanical fixpoint before
/// dynamics start — `1e-8` is a clean-start choice, *not* a divergence workaround:
/// at the default tolerance Rust and the oracle now stop at the *same* iter-4 point
/// (pinned by `indmach012_snapshot_default_tol_matches_oracle`; the earlier
/// "different iter-4 points / conditioning" reading was the extra-slip-step bug,
/// since fixed). Values are the oracle's `mvars` mode-3 monitor channels (f32 on
/// both sides).
#[test]
fn indmach012_dynamics_mode3_holds_operating_point_vs_oracle() {
    let mut dss = indmach_dyn_dss();
    dss.command("solve mode=dynamic h=0.001 number=1");
    assert!(
        dss.errors().is_empty(),
        "enter dynamics: {:?}",
        dss.errors()
    );
    dss.command("Solve number=49");
    assert!(dss.errors().is_empty(), "dynamic run: {:?}", dss.errors());

    let m = dss.monitor_view("mvars").expect("mvars monitor");
    // The mode-3 header tail is the 22 IndMach012 state-variable names.
    assert_eq!(
        &m.header[2..],
        [
            "Frequency",
            "Theta (deg)",
            "E1",
            "Pshaft",
            "dSpeed (deg/sec)",
            "dTheta (deg)",
            "Slip",
            "puRs",
            "puXs",
            "puRr",
            "puXr",
            "puXm",
            "Maxslip",
            "Is1",
            "Is2",
            "Ir1",
            "Ir2",
            "Stator Losses",
            "Rotor Losses",
            "Shaft Power (hp)",
            "Power Factor",
            "Efficiency (%)"
        ],
        "mode-3 header tail = the 22 IndMach012 variable names"
    );
    // 1 (enter) + 49 = 50 samples.
    assert_eq!(m.sample_count, 50);
    assert_eq!(m.channels.len(), 22);
    let at = |ch: usize, s: usize| m.channels[ch][s] as f64;
    let last = |ch: usize| at(ch, m.channels[ch].len() - 1);

    // The slipping equilibrium: the electrical state is constant across the run and
    // matches the oracle, pinned at the standard monitor `1e-6` (measured ~1e-8).
    // (Oracle dss-python 0.15.7, tol 1e-8.)
    assert!(rel(last(0), 59.041553) < 1e-6, "Frequency = {}", last(0));
    assert!(rel(last(2), 0.8986012) < 1e-6, "E1 (pu) = {}", last(2));
    assert!(rel(last(3), 1200000.1) < 1e-6, "Pshaft (W) = {}", last(3));
    assert!(rel(last(6), 0.015974108) < 1e-6, "Slip = {}", last(6));
    assert!(rel(last(13), 1593.1091) < 1e-6, "Is1 (A) = {}", last(13));
    assert!(rel(last(15), 1531.1659) < 1e-6, "Ir1 (A) = {}", last(15));
    assert!(
        rel(last(17), 56136.43) < 1e-6,
        "Stator Losses = {}",
        last(17)
    );
    assert!(
        rel(last(18), 19445.965) < 1e-6,
        "Rotor Losses = {}",
        last(18)
    );
    assert!(rel(last(19), 1605.7596) < 1e-6, "Shaft hp = {}", last(19));
    assert!(
        rel(last(20), 0.90832734) < 1e-6,
        "Power Factor = {}",
        last(20)
    );
    assert!(
        rel(last(21), 93.70147) < 1e-6,
        "Efficiency % = {}",
        last(21)
    );
    assert!(rel(last(5), -6.022097) < 1e-6, "dTheta (deg) = {}", last(5));
    // dSpeed is the slipping-equilibrium residual, not zero — the oracle reproduces
    // it (1.742747e-5 deg/s; Rust matches to ~2.5e-9), so it is pinned against the
    // oracle's actual value (cf. the Kundur dSpeed residual).
    assert!(
        rel(last(4), 1.742747e-5) < 1e-6,
        "dSpeed (deg/s) = {}",
        last(4)
    );
    // The induction-machine rotor slips, so Theta drifts linearly: pin both ends.
    assert!(
        rel(at(1, 0), -41.16823) < 1e-6,
        "Theta@0 (deg) = {}",
        at(1, 0)
    );
    assert!(
        rel(last(1), -58.075226) < 1e-6,
        "Theta@49 (deg) = {}",
        last(1)
    );

    // The balanced source keeps the negative sequence quiescent across the run.
    // This is a "stays small" upper bound (the oracle holds Is2/Ir2 at ~4.79e-7,
    // so the 1e-5 threshold is ~20× over it) — not an oracle value pin, so it stays
    // at 1e-5 (a 1e-6 threshold would be only ~2× over the quiescent level).
    for s in 0..m.channels[14].len() {
        assert!(at(14, s) < 1e-5, "Is2 grew at sample {s}: {}", at(14, s));
        assert!(at(16, s) < 1e-5, "Ir2 grew at sample {s}: {}", at(16, s));
    }
    // The electrical equilibrium holds for the whole run, not just the endpoints.
    for s in 0..m.channels[6].len() {
        assert!(
            rel(at(6, s), 0.015974108) < 1e-6,
            "Slip drifted at sample {s}: {}",
            at(6, s)
        );
    }
}

/// The fault response: a 3-phase bolted fault at the motor bus collapses the
/// terminal voltage, so the motor loses electrical torque, decelerates (the slip
/// rises and the rotor frequency falls), and draws a large inrush. This drives
/// `IntegrateStates` with a genuine disturbance and matches the oracle elementwise
/// through the fault. (Oracle dss-python 0.15.7, tol 1e-8.)
#[test]
fn indmach012_dynamics_fault_response_matches_oracle() {
    let mut dss = indmach_dyn_dss();
    dss.command("solve mode=dynamic h=0.001 number=1");
    dss.command("Solve number=49"); // 50 pre-fault samples (indices 0..=49)
    dss.command("New Fault.f1 phases=3 bus1=mbus");
    dss.command("Solve number=50"); // 50 fault samples
    assert!(dss.errors().is_empty(), "fault run: {:?}", dss.errors());

    let m = dss.monitor_view("mvars").expect("mvars monitor");
    assert_eq!(m.sample_count, 100);
    let at = |ch: usize, s: usize| m.channels[ch][s] as f64;

    // Pinned at the standard monitor `1e-6` (measured ~1e-8; the literals are the
    // exact oracle f32 values).
    // Pre-fault (sample 49) the motor sits at the slipping equilibrium.
    assert!(rel(at(6, 49), 0.015974108) < 1e-6, "pre-fault Slip");
    // First fault step (sample 50): voltage collapse → large inrush.
    assert!(
        rel(at(13, 50), 8044.474) < 1e-6,
        "fault Is1 = {}",
        at(13, 50)
    );
    assert!(
        rel(at(6, 50), 0.016010445) < 1e-6,
        "fault Slip = {}",
        at(6, 50)
    );
    // End of fault (sample 99): slip risen, rotor frequency fallen, decelerating.
    assert!(
        rel(at(6, 99), 0.019402187) < 1e-6,
        "end Slip = {}",
        at(6, 99)
    );
    assert!(
        rel(at(0, 99), 58.83587) < 1e-6,
        "end Frequency (Hz) = {}",
        at(0, 99)
    );
    assert!(
        rel(at(1, 99), -77.1814) < 1e-6,
        "end Theta (deg) = {}",
        at(1, 99)
    );
    // The rotor decelerates throughout the fault (slip non-decreasing).
    for s in 51..100 {
        assert!(
            at(6, s) >= at(6, s - 1) - 1.0e-6,
            "slip must rise under fault; dropped at sample {s}: {} -> {}",
            at(6, s - 1),
            at(6, s)
        );
    }
}

// ===========================================================================
// WP7.7 step 3b cont. — PVSystem / Storage driven by a user `DynamicExp` instead
// of the built-in GFL inverter `SolveDynamicStep`. The mode-3 monitor now records
// the DynamicExp memory slots (NVariables × [value, derivative]) instead of the
// 22/34 classic vars. The equation `it dt = (1/L)·(modul·vdc − R·it − vac)`
// (transcribed verbatim from the corpus GFL_IEEE123 DynExp deck's myDiffEq /
// myDiffEq2) reproduces the inverter filter ODE with the user's own L/R, so the
// per-phase current `it` (DynOut[0]) integrates to the same ISP current setpoint
// the classic gate reaches — but through the user equation, not the built-in step.
// Oracle: dss-python 0.15.7.
// ===========================================================================

/// PVSystem grid-following DynExp deck: the `pv_dyn_dss` micro feeder with the
/// PV's inverter model replaced by `DynamicExp.myDiffEq`. `DynamicEq=` must precede
/// the inline calc-value bindings (`it=imag vdc=kvdc …`) and `DynOut` in the `New`
/// so the equation resolves/sizes first (the bindings fall through to `ParseDynVar`).
fn pv_dynexp_dss() -> Dss {
    let mut dss = Dss::new();
    dss.command("Set DefaultBaseFrequency=60");
    dss.command(
        "New Circuit.pvdynexp basekv=12.47 pu=1.0 phases=3 bus1=sourcebus \
         mvasc3=20000 mvasc1=21000",
    );
    dss.command("New Line.l1 bus1=sourcebus bus2=pvbus length=0.5 units=km r1=0.1 x1=0.3");
    dss.command(
        "New DynamicExp.myDiffEq nvariables=4 varnames=[it vdc modul vac] \
         expression=[it dt = 1 0.61059E-3 / ( -0.230187 it * modul vdc * + vac - ) *]",
    );
    dss.command(
        "New PVSystem.pv1 phases=3 bus1=pvbus kv=12.47 kVA=600 Pmpp=500 \
         irradiance=1 %cutin=0.1 %cutout=0.1 kvar=0 DynamicEq=myDiffEq \
         it=imag vdc=kvdc modul=mod vac=vmag DynOut=[it]",
    );
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve");
    assert!(
        dss.errors().is_empty(),
        "pv dynexp steady solve: {:?}",
        dss.errors()
    );
    dss.command("New Monitor.pvvars PVSystem.pv1 Term=1 mode=3");
    dss
}

/// Storage grid-following DynExp deck: the `sto_dyn_dss` micro feeder with the
/// discharging Storage driven by `DynamicExp.myDiffEq2` (the corpus Storage filter
/// L/R).
fn sto_dynexp_dss() -> Dss {
    let mut dss = Dss::new();
    dss.command("Set DefaultBaseFrequency=60");
    dss.command(
        "New Circuit.stodynexp basekv=12.47 pu=1.0 phases=3 bus1=sourcebus \
         mvasc3=20000 mvasc1=21000",
    );
    dss.command("New Line.l1 bus1=sourcebus bus2=stobus length=0.5 units=km r1=0.1 x1=0.3");
    dss.command(
        "New DynamicExp.myDiffEq2 nvariables=4 varnames=[it vdc modul vac] \
         expression=[it dt = 1 0.50882E-3 / ( -0.1918225 it * modul vdc * + vac - ) *]",
    );
    dss.command(
        "New Storage.st1 phases=3 bus1=stobus kv=12.47 kWrated=500 kWhrated=1000 \
         kWhstored=1000 %stored=100 state=discharging %discharge=100 \
         %cutin=0.1 %cutout=0.1 DynamicEq=myDiffEq2 \
         it=imag vdc=kvdc modul=mod vac=vmag DynOut=[it]",
    );
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve");
    assert!(
        dss.errors().is_empty(),
        "sto dynexp steady solve: {:?}",
        dss.errors()
    );
    dss.command("New Monitor.stovars Storage.st1 Term=1 mode=3");
    dss
}

/// The 8 DynamicExp memory-slot names (`it vdc modul vac`, each value+derivative)
/// the mode-3 monitor records for both DynExp inverter decks.
const DYNEXP_INV_SLOTS: [&str; 8] = ["it", "dit", "vdc", "dvdc", "modul", "dmodul", "vac", "dvac"];

/// PVSystem DynExp dynamics, undisturbed: the user equation integrates the filter
/// current `it` to the same ISP setpoint the built-in model reaches. The mode-3
/// monitor records the 8 DynamicExp slots (not the 22 classic PV vars). The PV's
/// tiny filter L (0.61 mH) makes the start-up stiff (a fast transient in the first
/// ~50 ms), so the binding pins are the *settled* slots (sample 100+) where Rust
/// and the oracle share the algebraic fixpoint; the header + count prove the
/// variable interface switched to the DynamicExp memory. Oracle: dss-python 0.15.7.
#[test]
fn pvsystem_dynexp_dynamics_mode3_matches_oracle() {
    let mut dss = pv_dynexp_dss();
    dss.command("solve mode=dynamic h=0.001 number=1");
    assert!(
        dss.errors().is_empty(),
        "enter dynamics: {:?}",
        dss.errors()
    );
    dss.command("Solve number=200");
    assert!(dss.errors().is_empty(), "dynamic run: {:?}", dss.errors());

    let m = dss.monitor_view("pvvars").expect("pvvars monitor");
    // The mode-3 header tail is the DynamicExp memory-slot names (NVariables=4 →
    // 8 slots), replacing the 22 classic PV variable names.
    assert_eq!(
        &m.header[2..],
        DYNEXP_INV_SLOTS,
        "mode-3 header tail = the 8 DynamicExp memory slots"
    );
    assert_eq!(m.sample_count, 201);
    assert_eq!(m.channels.len(), 8);
    let at = |ch: usize, s: usize| m.channels[ch][s] as f64;

    // The deterministic first dynamics step (sample 0): a single trapezoidal
    // half-step from the classic GFL init seed through the user equation — `it`/`dit`
    // are large because the seed is far from the equation's fixpoint and the filter L
    // is tiny, `modul` is still the init duty (1.0). (Samples 1..99 ring too hard to
    // pin at f32 — the stiff start-up — but step 0 is one deterministic step.)
    // WP-U1.2 D7: this deck's `isp = (500000/vgmag)/3 ≈ 23.1457` sat right at the
    // OLD `iMaxPPhase` clamp boundary (PanelkW/base = 23.14571); the new base
    // FkVArating(600)/... = 27.78 RELEASES the clamp (isp is now uncapped ~23.146,
    // matching capi015's clamp logic), so the single-step derivative shifts a
    // deterministic ~0.035%: `dit@0` 1055150.8 → 1054757.2. `it@0` (the predictor
    // half-step) is unchanged. The startup transient itself is 0.14.5-shaped and
    // NOT capi015's (capi015 seeds the DynExp at the fixpoint — a separate,
    // out-of-scope 0.15.x init change); the BINDING pins are the settled slots
    // (sample 100+, `it`=23.14571) which match BOTH engines.
    assert!(rel(at(0, 0), 673.266) < 1e-6, "PV it@0 = {}", at(0, 0));
    assert!(rel(at(1, 0), 1054757.2) < 1e-6, "PV dit@0 = {}", at(1, 0));
    assert!(rel(at(4, 0), 1.0) < 1e-6, "PV modul@0 = {}", at(4, 0));

    // Settled state (sample 100 = past the stiff start-up; sample 200 = end). All
    // pinned at the standard monitor `1e-6` (TOLERANCE_NOTES.md; channels are f32).
    // `it` (DynOut[0]) relaxes to the ISP current setpoint; `dit` → exactly 0; the
    // duty cycle `modul` settles; `vdc` is the constant Rated VDC; `vac` is the
    // grid voltage. The user L/R differ from the built-in, so `it`'s 23.14571 is
    // distinct from the classic gate's 23.148539 — i.e. the *equation* drove it.
    assert!(
        rel(at(0, 100), 23.145702) < 1e-6,
        "PV it@100 = {}",
        at(0, 100)
    );
    assert!(
        rel(at(0, 200), 23.14571) < 1e-6,
        "PV it@200 = {}",
        at(0, 200)
    );
    assert!(at(1, 200).abs() < 1e-6, "PV dit (settled) = {}", at(1, 200));
    assert!(rel(at(2, 0), 8000.0) < 1e-6, "PV vdc@0 = {}", at(2, 0));
    assert!(
        rel(at(2, 200), 8000.0) < 1e-6,
        "PV vdc@200 = {}",
        at(2, 200)
    );
    assert!(
        rel(at(4, 100), 0.90076077) < 1e-6,
        "PV modul@100 = {}",
        at(4, 100)
    );
    assert!(
        rel(at(4, 200), 0.90076077) < 1e-6,
        "PV modul@200 = {}",
        at(4, 200)
    );
    assert!(
        rel(at(6, 200), 7200.7583) < 1e-6,
        "PV vac@200 = {}",
        at(6, 200)
    );

    // The fixpoint holds across the settled tail (samples 100..=200): `it` stays
    // at the ISP setpoint (oracle span 23.145702..23.145716, all within 1e-6).
    for s in 100..=200 {
        assert!(
            rel(at(0, s), 23.14571) < 1e-6,
            "PV it drifted at sample {s}: {}",
            at(0, s)
        );
    }
}

/// Storage DynExp dynamics, undisturbed: a discharging Storage seeds `it = 0`, so
/// the user equation ramps the current up smoothly (no stiff overshoot) to the ISP
/// setpoint. Pins both the ramp (sample 0: `it = 0`, the first derivative `dit`,
/// the seed duty cycle) and the settled state. Oracle: dss-python 0.15.7.
#[test]
fn storage_dynexp_dynamics_mode3_matches_oracle() {
    let mut dss = sto_dynexp_dss();
    dss.command("solve mode=dynamic h=0.001 number=1");
    assert!(
        dss.errors().is_empty(),
        "enter dynamics: {:?}",
        dss.errors()
    );
    dss.command("Solve number=200");
    assert!(dss.errors().is_empty(), "dynamic run: {:?}", dss.errors());

    let m = dss.monitor_view("stovars").expect("stovars monitor");
    assert_eq!(
        &m.header[2..],
        DYNEXP_INV_SLOTS,
        "mode-3 header tail = the 8 DynamicExp memory slots"
    );
    assert_eq!(m.sample_count, 201);
    assert_eq!(m.channels.len(), 8);
    let at = |ch: usize, s: usize| m.channels[ch][s] as f64;

    // The ramp from rest (sample 0): `it` starts at exactly 0 (the discharging
    // seed), `dit` is the first equation derivative (deterministic from the init
    // seed: (1/L)·(modul·vdc − R·0 − vac)), and `modul`/`vac` are the seed values.
    assert!(at(0, 0).abs() < 1e-6, "STO it@0 = {}", at(0, 0));
    assert!(rel(at(1, 0), 2358.4167) < 1e-6, "STO dit@0 = {}", at(1, 0));
    assert!(
        rel(at(4, 0), 0.9000948) < 1e-6,
        "STO modul@0 = {}",
        at(4, 0)
    );
    assert!(rel(at(6, 0), 7199.558) < 1e-6, "STO vac@0 = {}", at(6, 0));

    // Settled state (samples 100/200): `it` reaches the ISP setpoint, `dit` → 0.
    assert!(
        rel(at(0, 100), 23.14571) < 1e-6,
        "STO it@100 = {}",
        at(0, 100)
    );
    assert!(
        rel(at(0, 200), 23.14571) < 1e-6,
        "STO it@200 = {}",
        at(0, 200)
    );
    assert!(
        at(1, 200).abs() < 1e-6,
        "STO dit (settled) = {}",
        at(1, 200)
    );
    assert!(
        rel(at(2, 200), 8000.0) < 1e-6,
        "STO vdc@200 = {}",
        at(2, 200)
    );
    assert!(
        rel(at(4, 200), 0.9006498) < 1e-6,
        "STO modul@200 = {}",
        at(4, 200)
    );
    assert!(
        rel(at(6, 200), 7200.7583) < 1e-6,
        "STO vac@200 = {}",
        at(6, 200)
    );

    // The fixpoint holds across the settled tail.
    for s in 100..=200 {
        assert!(
            rel(at(0, s), 23.14571) < 1e-6,
            "STO it drifted at sample {s}: {}",
            at(0, s)
        );
    }
}

/// PVSystem DynExp dynamics under a bolted 3-phase fault at the PV bus: the grid
/// voltage `vac` collapses below MinVS, so the inverter enters safe mode — the
/// `SolveModulation` safe-mode branch (reached through the equation's `mod` calc
/// value) drives the duty cycle `modul` to 0, and the user equation settles `it` at
/// its safe-mode limit. The post-fault state is settled (flat over the last fault
/// steps), so it is value-pinned against the oracle. This exercises the
/// `DynamicEqObj <> NIL` integration branch under a genuine disturbance (parity with
/// the classic `pvsystem_dynamics_safe_mode_under_fault` gate). Oracle: dss-python
/// 0.15.7.
#[test]
fn pvsystem_dynexp_dynamics_safe_mode_under_fault_matches_oracle() {
    let mut dss = pv_dynexp_dss();
    dss.command("solve mode=dynamic h=0.001 number=1");
    dss.command("Solve number=200");
    dss.command("New Fault.F1 phases=3 Bus1=pvbus");
    dss.command("Solve number=100");
    assert!(dss.errors().is_empty(), "pv fault run: {:?}", dss.errors());

    let m = dss.monitor_view("pvvars").expect("pvvars monitor");
    assert_eq!(m.sample_count, 301);
    let last = |ch: usize| *m.channels[ch].last().expect("samples") as f64;
    // Pinned at the standard monitor `1e-6` (the post-fault state is settled).
    // `vac` collapses (the bolted fault crushes the bus voltage), `modul` → exactly 0
    // (safe mode zeroed the duty), and the equation holds `it`/`dit` at the safe-mode
    // limit (distinct from the classic safe-mode `it → 0` because the user equation,
    // not `SolveDynamicStep`, governs `dit`).
    assert!(
        rel(last(6), 4.3389945) < 1e-6,
        "PV vac (faulted) = {}",
        last(6)
    );
    assert!(last(4).abs() < 1e-6, "PV modul (safe) = {}", last(4));
    assert!(
        rel(last(0), -7.4471335) < 1e-6,
        "PV it (safe) = {}",
        last(0)
    );
    assert!(
        rel(last(1), -4298.7295) < 1e-6,
        "PV dit (safe) = {}",
        last(1)
    );
}

/// Storage DynExp dynamics under a bolted 3-phase fault at the Storage bus: the
/// grid voltage collapses, so the discharging unit trips to IDLING. In idle the
/// `IntegrateStates` else-branch runs (not the DynExp path), so the equation memory
/// stops updating and the `it` slot freezes at its last discharging value, while the
/// `modul` slot drops to 0 (safe). Pins the trip path (parity with the classic
/// `storage_dynamics_trips_to_idle_under_fault` gate). Oracle: dss-python 0.15.7.
#[test]
fn storage_dynexp_dynamics_trips_under_fault_matches_oracle() {
    let mut dss = sto_dynexp_dss();
    dss.command("solve mode=dynamic h=0.001 number=1");
    dss.command("Solve number=200");
    dss.command("New Fault.F1 phases=3 Bus1=stobus");
    dss.command("Solve number=100");
    assert!(dss.errors().is_empty(), "sto fault run: {:?}", dss.errors());

    let m = dss.monitor_view("stovars").expect("stovars monitor");
    assert_eq!(m.sample_count, 301);
    let last = |ch: usize| *m.channels[ch].last().expect("samples") as f64;
    // `vac` collapses; `modul` → exactly 0 (safe). The DynExp `it` slot freezes at the
    // pre-trip discharging value (23.14571) because the idle else-branch stops feeding
    // the equation memory; `dit` holds the last discharging derivative.
    assert!(
        rel(last(6), 4.340462) < 1e-6,
        "STO vac (faulted) = {}",
        last(6)
    );
    assert!(last(4).abs() < 1e-6, "STO modul (idle) = {}", last(4));
    assert!(
        rel(last(0), 23.14571) < 1e-6,
        "STO it (frozen) = {}",
        last(0)
    );
    assert!(rel(last(1), -17256.26) < 1e-6, "STO dit = {}", last(1));
}

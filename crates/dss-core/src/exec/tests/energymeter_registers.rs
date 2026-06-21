use crate::exec::*;

/// Helper: fetch a meter register value by name.
fn meter_reg(dss: &Dss, meter: &str, reg_name: &str) -> f64 {
    dss.meter_registers(meter)
        .unwrap_or_else(|| panic!("meter {meter} not found"))
        .into_iter()
        .find(|(n, _)| n == reg_name)
        .unwrap_or_else(|| panic!("register {reg_name} not found"))
        .1
}

/// Build the 2-bus daily case shared by the register tests. Loadshape ramps
/// 1→2→3 over 3 one-hour steps; the meter is on the source line.
fn daily_meter_case(trapezoidal: bool) -> Dss {
    let mut dss = Dss::new();
    dss.command("New circuit.t basekv=12.47 bus1=src");
    dss.command("New loadshape.ls npts=3 interval=1 mult=(1.0 2.0 3.0)");
    dss.command("New line.l1 bus1=src bus2=b2 length=1 r1=0.1 x1=0.1");
    dss.command("New load.ld1 bus1=b2 kV=12.47 kW=1000 pf=1 model=1 daily=ls");
    dss.command("New energymeter.m1 element=line.l1 terminal=1");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    // `Set mode=` resets the trapezoidal flag, so set it afterwards.
    dss.command("Set mode=daily number=3 stepsize=1h time=(0,0)");
    dss.command(if trapezoidal {
        "Set trapezoidal=yes"
    } else {
        "Set trapezoidal=no"
    });
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

/// Plain Euler integration: `kWh = Σ interval·P`. Oracle (dss-python
/// 0.15.7) register values for the 1→2→3 daily ramp.
#[test]
fn energymeter_daily_registers_euler() {
    let dss = daily_meter_case(false);
    let approx = |got: f64, want: f64| {
        assert!(
            (got - want).abs() <= 1e-6 * want.abs().max(1.0),
            "got {got}, want {want}"
        );
    };
    approx(meter_reg(&dss, "m1", "kWh"), 6009.009963043285);
    approx(meter_reg(&dss, "m1", "kvarh"), 8.436633138535129);
    approx(meter_reg(&dss, "m1", "Zone kWh"), 5999.979170340599);
    approx(meter_reg(&dss, "m1", "Max kW"), 3005.7947873123644);
    approx(meter_reg(&dss, "m1", "Line Losses"), 9.038717950944731);
    approx(
        meter_reg(&dss, "m1", "Zone Max kW Losses"),
        5.814433198962477,
    );
    // The single voltage base bucket carries the same line losses.
    approx(
        meter_reg(&dss, "m1", "12.5 kV Line Loss"),
        9.038717950944731,
    );
}

/// Trapezoidal integration: the first sample after reset is skipped, then
/// `kWh += 0.5·interval·(P + P_prev)`. Same circuit, oracle values.
#[test]
fn energymeter_daily_registers_trapezoidal() {
    let dss = daily_meter_case(true);
    let approx = |got: f64, want: f64| {
        assert!(
            (got - want).abs() <= 1e-6 * want.abs().max(1.0),
            "got {got}, want {want}"
        );
    };
    approx(meter_reg(&dss, "m1", "kWh"), 4005.7905368432225);
    approx(meter_reg(&dss, "m1", "Zone kWh"), 3999.9865469246124);
    // Drag-hand maxima are independent of the integration rule.
    approx(meter_reg(&dss, "m1", "Max kW"), 3005.7947873123644);
    approx(
        meter_reg(&dss, "m1", "Zone Max kW Losses"),
        5.814433198962477,
    );
}

/// `Reset Meters` zeroes the registers and re-primes the drag-hand maxima to
/// the large-negative sentinel.
#[test]
fn energymeter_reset_registers() {
    let mut dss = daily_meter_case(false);
    assert!(meter_reg(&dss, "m1", "kWh") > 1.0, "registers accumulated");
    dss.command("Reset Meters");
    assert_eq!(meter_reg(&dss, "m1", "kWh"), 0.0);
    assert_eq!(meter_reg(&dss, "m1", "Zone kWh"), 0.0);
    // Drag-hand registers reset to -1e50.
    assert_eq!(meter_reg(&dss, "m1", "Max kW"), -1.0e50);
    assert_eq!(meter_reg(&dss, "m1", "Zone Max kW Losses"), -1.0e50);
}

/// Helper used by the new register tests: build a 3-step daily case from a
/// list of `New ...` commands, run it, and return the solved `Dss`. The
/// loadshape `ls` (1→2→3) and the daily-mode/trapezoidal-off boilerplate are
/// shared; callers pass the topology + `voltagebases`.
fn meter_case(decls: &[&str], voltagebases: &str, extra_set: &[&str]) -> Dss {
    let mut dss = Dss::new();
    dss.command("New circuit.t basekv=12.47 bus1=src");
    dss.command("New loadshape.ls npts=3 interval=1 mult=(1.0 2.0 3.0)");
    for d in decls {
        dss.command(d);
    }
    dss.command(&format!("Set voltagebases=[{voltagebases}]"));
    dss.command("CalcVoltageBases");
    for s in extra_set {
        dss.command(s);
    }
    dss.command("Set mode=daily number=3 stepsize=1h time=(0,0)");
    dss.command("Set trapezoidal=no");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

fn approx_meter(dss: &Dss, reg_name: &str, want: f64) {
    let got = meter_reg(dss, "m1", reg_name);
    assert!(
        (got - want).abs() <= 1e-6 * want.abs().max(1.0),
        "{reg_name}: got {got}, want {want}"
    );
}

/// A generator in the zone accumulates the Gen registers (`Accumulate_Gen`:
/// `−Power[1]·0.001` into the gen totals, *not* the zone-load totals). Oracle
/// values for a 500 kW gen on the same 1→2→3 daily ramp.
#[test]
fn energymeter_generator_registers() {
    let dss = meter_case(
        &[
            "New line.l1 bus1=src bus2=b2 length=1 r1=0.1 x1=0.1",
            "New load.ld1 bus1=b2 kV=12.47 kW=1000 pf=1 model=1 daily=ls",
            "New generator.g1 bus1=b2 kV=12.47 kW=500 pf=1 model=1 daily=ls",
            "New energymeter.m1 element=line.l1 terminal=1",
        ],
        "12.47",
        &[],
    );
    approx_meter(&dss, "Gen kWh", 2999.9974045506274);
    approx_meter(&dss, "Gen kvarh", -0.0010792012877156054);
    approx_meter(&dss, "Gen Max kW", 1499.9981626190265);
    approx_meter(&dss, "Gen Max kVA", 1499.9981626191664);
    // Zone load is unaffected by the generator (gen has its own totals).
    approx_meter(&dss, "Zone kWh", 5999.994809101255);
}

/// 3-phase line sequence-mode loss split (`GetSeqLosses`, 3-phase only):
/// balanced line ⇒ all loss in the positive/line mode, ~0 zero-mode.
#[test]
fn energymeter_sequence_mode_losses() {
    let dss = meter_case(
        &[
            "New line.l1 bus1=src bus2=b2 length=1 r1=0.1 x1=0.1",
            "New load.ld1 bus1=b2 kV=12.47 kW=1000 pf=1 model=1 daily=ls",
            "New energymeter.m1 element=line.l1 terminal=1",
        ],
        "12.47",
        &[],
    );
    approx_meter(&dss, "Line Mode Line Losses", 9.038717950944614);
    approx_meter(&dss, "3-phase Line Losses", 9.038717950944731);
    approx_meter(&dss, "1- and 2-phase Line Losses", 0.0);
    // Balanced ⇒ zero-sequence loss is numerically ~0 (1e-20).
    assert!(
        meter_reg(&dss, "m1", "Zero Mode Line Losses").abs() < 1e-9,
        "zero-mode loss should be ~0 for a balanced line"
    );
}

/// A transformer in the zone exercises the load/no-load loss split
/// (`GetLosses` override) and the second voltage-base bucket (the 4.16 kV
/// secondary, reached via line `l2`). Oracle values.
#[test]
fn energymeter_transformer_loss_split_and_vbase() {
    let dss = meter_case(
        &[
            "New line.l1 bus1=src bus2=b2 length=1 r1=0.1 x1=0.1",
            "New transformer.t1 windings=2 buses=(b2 b3) conns=(wye wye) \
                 kvs=(12.47 4.16) kvas=(2000 2000) xhl=5 %loadloss=1 %noloadloss=0.2",
            "New line.l2 bus1=b3 bus2=b4 length=0.5 r1=0.05 x1=0.05",
            "New load.ld1 bus1=b4 kV=4.16 kW=1000 pf=1 model=1 daily=ls",
            "New energymeter.m1 element=line.l1 terminal=1",
        ],
        "12.47 4.16",
        &[],
    );
    approx_meter(&dss, "Transformer Losses", 85.06511357026721);
    approx_meter(&dss, "Load Losses kWh", 103.96306991988196);
    approx_meter(&dss, "No Load Losses kWh", 11.675484334236636);
    approx_meter(&dss, "Line Losses", 30.57344068385137);
    // First voltage-base bucket (12.5 kV primary side): transformer split.
    approx_meter(&dss, "12.5 kV Load Loss", 73.389629);
    approx_meter(&dss, "12.5 kV No Load Loss", 11.675484);
    // Second voltage-base bucket (4.16 kV secondary): line l2 losses +
    // the load energy bucketed by its parent branch's voltage base.
    approx_meter(&dss, "4.16 kV Line Loss", 21.134364);
    approx_meter(&dss, "4.16 kV Load Energy", 5999.665065);
}

/// An under-rated line drives the overload registers and the *radial*
/// EEN/UE marking (`ExcesskVANorm/Emerg` set `Overload_EEN/UE`, loads marked
/// by the degree of overload). Oracle values.
#[test]
fn energymeter_overload_and_radial_een_ue() {
    let dss = meter_case(
        &[
            "New line.l1 bus1=src bus2=b2 length=1 r1=0.1 x1=0.1 normamps=50 emergamps=70",
            "New load.ld1 bus1=b2 kV=12.47 kW=1000 pf=1 model=1 daily=ls",
            "New energymeter.m1 element=line.l1 terminal=1",
        ],
        "12.47",
        &[],
    );
    approx_meter(&dss, "Overload kWh Normal", 2849.1631405361386);
    approx_meter(&dss, "Overload kWh Emerg", 1985.482037568384);
    approx_meter(&dss, "Load EEN", 7062.605747589624);
    approx_meter(&dss, "Load UE", 3616.152913382251);
}

/// A high-impedance line with ample current rating: no line overload, so the
/// EEN/UE come from the load's *voltage* criterion (`ExceedsNormal`/
/// `Unserved`, `VminNormal`/`VminEmerg` defaults). Oracle values.
#[test]
fn energymeter_voltage_een_ue() {
    let dss = meter_case(
        &[
            "New line.l1 bus1=src bus2=b2 length=1 r1=2 x1=2 normamps=2000 emergamps=3000",
            "New load.ld1 bus1=b2 kV=12.47 kW=4000 pf=1 model=1 daily=ls",
            "New energymeter.m1 element=line.l1 terminal=1",
        ],
        "12.47",
        &["Set normvminpu=0.95 emergvminpu=0.90"],
    );
    // No line overload ⇒ overload-energy registers stay 0.
    approx_meter(&dss, "Overload kWh Normal", 0.0);
    approx_meter(&dss, "Overload kWh Emerg", 0.0);
    // EEN/UE come purely from the voltage criterion.
    approx_meter(&dss, "Load EEN", 28188.694199630165);
    approx_meter(&dss, "Load UE", 11344.628473647135);
}

/// `Reset` (no argument) must reset controls too (Pascal `DoResetControls`):
/// a CapControl that opened its bank during the solve has the bank driven
/// back to its `InitialState` (closed) by the reset.
#[test]
fn reset_command_resets_controls() {
    let mut dss = Dss::new();
    dss.command("New circuit.t basekv=12.47 bus1=src");
    dss.command("New line.l1 bus1=src bus2=b2 length=1 r1=0.5 x1=1.0");
    dss.command("New load.ld1 bus1=b2 kV=12.47 kW=50 pf=0.99 model=1");
    dss.command("New capacitor.c1 bus1=b2 kV=12.47 kvar=600 numsteps=1");
    // kvar control opens the bank when the sensed kvar is below `offsetting`;
    // the tiny load keeps it below, so the solve switches the bank OUT.
    dss.command(
        "New capcontrol.cc1 element=line.l1 terminal=1 capacitor=c1 \
             type=kvar ptratio=1 onsetting=200 offsetting=100",
    );
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    // The control opened the bank during the solve.
    assert_eq!(
        dss.capacitor_closed("c1"),
        Some(false),
        "control should have opened the bank"
    );
    // No-arg Reset must run DoResetControls → bank back to InitialState.
    dss.command("Reset");
    assert_eq!(
        dss.capacitor_closed("c1"),
        Some(true),
        "Reset must reset controls (close the bank to InitialState)"
    );
}

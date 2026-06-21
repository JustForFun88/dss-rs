use crate::exec::*;

/// Build the 2-bus regulator micro-circuit the WP5.7 oracle probes used.
fn reg_two_bus(dss: &mut Dss, reg_props: &str) {
    dss.command("New circuit.ctl basekv=12.47 pu=1.0 phases=3 mvasc3=2000");
    dss.command(
        "New transformer.t1 phases=3 windings=2 buses=(sourcebus, b2) \
             conns=(delta wye) kvs=(12.47 4.16) kvas=(5000 5000) xhl=8",
    );
    dss.command(&format!("New regcontrol.r1 transformer=t1 {reg_props}"));
    dss.command("New load.l1 bus1=b2 phases=3 kv=4.16 kw=300 pf=0.95");
    dss.command("Set voltagebases=[12.47, 4.16]");
    dss.command("CalcVoltageBases");
}

/// WP5.7: the live control loop drives the regulator to the oracle's tap.
/// Oracle probe (pinned dss-python): iterations=6, winding-2 tap=1.01875.
#[test]
fn control_loop_regulates_two_bus_to_oracle_tap() {
    let mut dss = Dss::new();
    reg_two_bus(&mut dss, "winding=2 vreg=122 band=0.0001 ptratio=20");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let ckt = dss.circuit().unwrap();
    assert!(ckt.is_solved);
    assert_eq!(ckt.solution.iteration, 6);
    let taps = dss.transformer_taps();
    assert_eq!(taps[0].0, "t1");
    assert!(
        (taps[0].1[1] - 1.01875).abs() < 1e-12,
        "winding-2 tap = {}",
        taps[0].1[1]
    );
}

/// WP7.1 step 4: a *direct* `Transformer.X.Taps=` edit moves the winding tap
/// without going through the regulator, so the control's parse-time tap snapshot
/// goes stale. Pascal `Get_TapNum` reads the live transformer
/// `PresentTap[TapWinding]`; the executive `regcontrol_tap_numbers` view must
/// too — else the reported tap number stays 0. Reproduces the IEEE13 geometry
/// scripts' manual-tap + `controlmode=off` epilogue (oracle reports +10, not 0).
#[test]
fn regcontrol_tap_number_reads_live_transformer_after_manual_tap() {
    let mut dss = Dss::new();
    reg_two_bus(&mut dss, "winding=2 vreg=122 band=2 ptratio=20");
    // Position winding-2 at +10 taps (1.0625 = 1.0 + 10·0.00625) and freeze
    // controls so nothing moves it back.
    dss.command("Transformer.t1.Taps=[1.0 1.0625]");
    dss.command("Set controlmode=OFF");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(
        dss.regcontrol_tap_numbers(),
        vec![("r1".to_string(), 10)],
        "TapNumber must reflect the live winding tap, not the stale snapshot",
    );
    let taps = dss.transformer_taps();
    assert!(
        (taps[0].1[1] - 1.0625).abs() < 1e-12,
        "tap={}",
        taps[0].1[1]
    );
}

/// WP5.7 step 5: a control that cannot settle within `maxcontroliter`
/// stops with the 485 warning and aborts the solution. Oracle probe:
/// `maxcontroliter=2` + `maxtapchange=1` → iterations=4, tap=1.00625,
/// error 485; the next *external* command resets the abort flag
/// (CAPI `Text_Set_Command`).
#[test]
fn max_control_iterations_exceeded_warns_and_aborts() {
    let mut dss = Dss::new();
    reg_two_bus(
        &mut dss,
        "winding=2 vreg=122 band=2 ptratio=20 maxtapchange=1",
    );
    dss.command("Set maxcontroliter=2");
    dss.command("Solve");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.starts_with("Warning Max Control Iterations Exceeded.")),
        "{:?}",
        dss.errors()
    );
    let ckt = dss.circuit().unwrap();
    assert_eq!(ckt.solution.iteration, 4);
    assert!(ckt.solution.solution_abort);
    let taps = dss.transformer_taps();
    assert!(
        (taps[0].1[1] - 1.00625).abs() < 1e-12,
        "winding-2 tap = {}",
        taps[0].1[1]
    );
    // External commands reset the abort (the oracle solves again and
    // exceeds again rather than reporting "Solution aborted.").
    dss.command("Get hour");
    assert!(!dss.circuit().unwrap().solution.solution_abort);
}

/// Build the 2-bus + line + load + two-generator micro-circuit the
/// GenDispatcher oracle probes used.
fn gen_disp_two_bus(dss: &mut Dss, gd_props: &str) {
    dss.command("New circuit.a basekv=12.47 bus1=src phases=3");
    dss.command("New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("New load.ld1 bus1=b1 phases=3 kv=12.47 kw=5000 pf=0.95");
    dss.command("New generator.g1 bus1=b1 phases=3 kv=12.47 kw=1000 pf=1.0 model=1");
    dss.command("New generator.g2 bus1=b1 phases=3 kv=12.47 kw=1000 pf=1.0 model=1");
    dss.command(&format!(
        "New gendispatcher.gd1 element=line.l1 terminal=1 {gd_props}"
    ));
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
}

/// WP6.8: the control loop's GenDispatcher redispatches its generators so
/// the monitored line power approaches `kWLimit`. Oracle probe (pinned
/// dss-python): equal weights → g1 = g2 = 1511.569498763734 kW.
#[test]
fn gendispatcher_redispatches_to_oracle() {
    let mut dss = Dss::new();
    gen_disp_two_bus(
        &mut dss,
        "kwlimit=2000 kwband=100 genlist=[g1,g2] weights=[1,1]",
    );
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    for g in ["g1", "g2"] {
        let (kw, _kvar) = dss.generator_kw_kvar(g).unwrap();
        assert!((kw - 1511.569498763734).abs() < 1e-6, "{g} kW = {kw}");
    }
}

/// Weighted redispatch [3, 1]: g1 = 1767.3542481456006, g2 = 1255.7847493818672.
#[test]
fn gendispatcher_respects_weights_oracle() {
    let mut dss = Dss::new();
    gen_disp_two_bus(
        &mut dss,
        "kwlimit=2000 kwband=100 genlist=[g1,g2] weights=[3,1]",
    );
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let (kw1, _) = dss.generator_kw_kvar("g1").unwrap();
    let (kw2, _) = dss.generator_kw_kvar("g2").unwrap();
    assert!((kw1 - 1767.3542481456006).abs() < 1e-6, "g1 kW = {kw1}");
    assert!((kw2 - 1255.7847493818672).abs() < 1e-6, "g2 kW = {kw2}");
}

/// No GenList → dispatch every enabled generator (uniform weights); same
/// result as the explicit equal-weight list.
#[test]
fn gendispatcher_no_list_dispatches_all_gens() {
    let mut dss = Dss::new();
    gen_disp_two_bus(&mut dss, "kwlimit=2000 kwband=100");
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    for g in ["g1", "g2"] {
        let (kw, _) = dss.generator_kw_kvar(g).unwrap();
        assert!((kw - 1511.569498763734).abs() < 1e-6, "{g} kW = {kw}");
    }
}

/// WP6.8: the QDiff (kvar) redispatch path, exercised end-to-end. The gens
/// run at `pf=0.95` so they carry a dispatchable `kvarBase`, and both
/// `kWLimit` and `kvarLimit` bind. Oracle probe (pinned dss-python): equal
/// weights → g1 = g2 = (1509.8126154343354 kW, 591.2600618943429 kvar).
#[test]
fn gendispatcher_redispatches_kvar_to_oracle() {
    let mut dss = Dss::new();
    dss.command("New circuit.a basekv=12.47 bus1=src phases=3");
    dss.command("New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("New load.ld1 bus1=b1 phases=3 kv=12.47 kw=5000 pf=0.95");
    dss.command("New generator.g1 bus1=b1 phases=3 kv=12.47 kw=1000 pf=0.95 model=1");
    dss.command("New generator.g2 bus1=b1 phases=3 kv=12.47 kw=1000 pf=0.95 model=1");
    dss.command(
        "New gendispatcher.gd1 element=line.l1 terminal=1 \
             kwlimit=2000 kwband=100 kvarlimit=500 genlist=[g1,g2] weights=[1,1]",
    );
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    for g in ["g1", "g2"] {
        let (kw, kvar) = dss.generator_kw_kvar(g).unwrap();
        assert!((kw - 1509.8126154343354).abs() < 1e-6, "{g} kW = {kw}");
        assert!((kvar - 591.2600618943429).abs() < 1e-6, "{g} kvar = {kvar}");
    }
}

/// WP6.8: the monitored *terminal* is honored (not hard-wired to 1). A later
/// `terminal=2` overrides the helper's `terminal=1`; terminal 2 of the line
/// sits at the load/gen bus, so the measured power drives `PDiff` strongly
/// negative and both gens floor at `Max(1.0, …)` — a result distinct from
/// terminal 1's 1511.57 kW, which pins that the terminal index is read.
/// Oracle probe (pinned dss-python): g1 = g2 = 1.0 kW.
#[test]
fn gendispatcher_honors_monitored_terminal() {
    let mut dss = Dss::new();
    gen_disp_two_bus(
        &mut dss,
        "kwlimit=2000 kwband=100 terminal=2 genlist=[g1,g2] weights=[1,1]",
    );
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    for g in ["g1", "g2"] {
        let (kw, _) = dss.generator_kw_kvar(g).unwrap();
        assert!((kw - 1.0).abs() < 1e-6, "{g} kW = {kw}");
    }
}

/// WP6.8 StorageController skeleton: a circuit carrying a StorageController
/// (whose fleet is always empty in Phase 6) must still solve — the control
/// sweep treats it as an inert no-op. The only logged error is the faithful
/// 37201 ("No unassigned Storage Elements found") emitted at parse-time
/// RecalcElementData, exactly as the oracle reports on a Storage-less circuit.
#[test]
fn storagecontroller_skeleton_solves_as_noop() {
    let mut dss = Dss::new();
    dss.command("new circuit.a basekv=12.47 bus1=src phases=3");
    dss.command("new line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("new load.ld1 bus1=b1 phases=3 kv=12.47 kw=3000 pf=0.95");
    dss.command("new storagecontroller.sc1 element=line.l1 terminal=1");
    // The 37201 is logged during the New command; everything after solves.
    let errs: Vec<String> = dss.errors().to_vec();
    assert_eq!(errs.len(), 1, "{errs:?}");
    assert!(errs[0].contains("No unassigned Storage Elements found"));

    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve mode=snap");
    // No *new* errors from the control loop; the circuit converged.
    assert_eq!(dss.errors().len(), 1, "{:?}", dss.errors());
    assert!(dss.circuit().unwrap().solution.converged_flag);
}

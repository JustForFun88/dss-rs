use crate::exec::*;

/// AutoAdd option object defaults (`TAutoAdd.Init` + Circuit loss/UE
/// defaults) echoed back through `Get`.
#[test]
fn autoadd_options_defaults_via_get() {
    let mut dss = Dss::new();
    dss.command("New circuit.c1");
    dss.command("Get genkw genpf capkvar addtype ueweight lossweight ueregs lossregs");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(dss.result(), "1000, 1, 600, generator, 1, 1, [10], [13]");
}

/// `Set` the AutoAdd options, then verify both the circuit state and the
/// `Get` echo (AddType maps to the lowercase device word).
#[test]
fn autoadd_options_set_then_get() {
    let mut dss = Dss::new();
    dss.command("New circuit.c1");
    dss.command(
        "Set genkw=500 genpf=0.95 capkvar=1200 addtype=capacitor \
             ueweight=2 lossweight=3 ueregs=[1,2,3] lossregs=[13,14]",
    );
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    {
        let ckt = dss.circuit().unwrap();
        assert_eq!(ckt.auto_add_obj.gen_kw, 500.0);
        assert_eq!(ckt.auto_add_obj.gen_pf, 0.95);
        assert_eq!(ckt.auto_add_obj.cap_kvar, 1200.0);
        assert_eq!(ckt.auto_add_obj.add_type, crate::circuit::CAPADD);
        assert_eq!(ckt.ue_weight, 2.0);
        assert_eq!(ckt.loss_weight, 3.0);
        assert_eq!(ckt.ue_regs, vec![1, 2, 3]);
        assert_eq!(ckt.loss_regs, vec![13, 14]);
    }
    dss.command("Get genkw genpf capkvar addtype ueweight lossweight ueregs lossregs");
    assert_eq!(
        dss.result(),
        "500, 0.95, 1200, capacitor, 2, 3, [1, 2, 3], [13, 14]"
    );
}

/// `Set UEregs=` with a non-numeric token reproduces the Pascal
/// `MakeInteger` *raise*: the parser error is logged and the fill stops at
/// the bad token, leaving the already-sized array zero-filled from there on
/// (`[10, 0, 0]`, not a silent `[10, 0, 13]`). A roundable decimal still
/// rounds (`13.7 -> 14`) via the double fallback.
#[test]
fn ueregs_nonnumeric_token_logs_error_and_truncates() {
    let mut dss = Dss::new();
    dss.command("New circuit.c1");
    dss.command("Set ueregs=(10 abc 13)");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("Integer number conversion error")),
        "expected a logged conversion error, got {:?}",
        dss.errors()
    );
    assert_eq!(dss.circuit().unwrap().ue_regs, vec![10, 0, 0]);

    // The roundable-decimal path is unaffected (fresh circuit so the
    // error log above doesn't bleed into this assertion).
    let mut dss2 = Dss::new();
    dss2.command("New circuit.c2");
    dss2.command("Set lossregs=(13.7 14)");
    assert!(dss2.errors().is_empty(), "{:?}", dss2.errors());
    assert_eq!(dss2.circuit().unwrap().loss_regs, vec![14, 14]);
}

/// `Set addtype=` with an unrecognized value resolves to the enum default
/// (CAPADD) with **no** error — Pascal `StringToOrdinal` returns the default
/// rather than raising.
#[test]
fn addtype_unknown_falls_back_to_default_no_error() {
    let mut dss = Dss::new();
    dss.command("New circuit.c1");
    dss.command("Set addtype=foo");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(
        dss.circuit().unwrap().auto_add_obj.add_type,
        crate::circuit::CAPADD
    );
    dss.command("Get addtype");
    assert_eq!(dss.result(), "capacitor");
}

/// `Set AutoBusList=` parses an inline bus-name list (`DoAutoAddBusList`),
/// stored insertion-ordered and echoed comma-separated by `Get`.
#[test]
fn autoadd_bus_list_inline_round_trips() {
    let mut dss = Dss::new();
    dss.command("New circuit.c1");
    dss.command("Set autobuslist=[b1, b2, b3]");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(
        dss.circuit().unwrap().auto_add_bus_list,
        vec!["b1".to_string(), "b2".to_string(), "b3".to_string()]
    );
    dss.command("Get autobuslist");
    assert_eq!(dss.result(), "b1, b2, b3");
}

/// The AutoAdd *solve mode* (WPG.5) runs the capacity search and adds the
/// winning generator through the normal command path. This is the structural
/// unit test (a clean run + a winner instantiated + `GlobalResult` set); the
/// numeric winner/log/voltages are pinned by the live `autoadd.dss` case in
/// `corpus_live.rs` against the oracle.
#[test]
fn autoadd_solve_mode_adds_winner() {
    // Route the AutoAddLog / AutoAddedGenerators side files into a scratch dir
    // so the test doesn't litter the crate directory.
    let scratch = std::env::temp_dir().join(format!("dss_autoadd_{}", std::process::id()));
    std::fs::create_dir_all(&scratch).ok();

    let mut dss = Dss::new();
    dss.command("New circuit.c1 basekv=12.47 phases=3 bus1=sourcebus");
    dss.command("New Line.l1 bus1=sourcebus bus2=b1 phases=3 r1=0.3 x1=0.9 length=2 units=km");
    dss.command("New Line.l2 bus1=b1 bus2=b2 phases=3 r1=0.35 x1=0.95 length=1.5 units=km");
    dss.command("New Load.ld1 bus1=b2 phases=3 kv=12.47 kw=700 pf=0.95 model=1");
    dss.command("New EnergyMeter.em element=Line.l1 terminal=1");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("Set addtype=generator genkw=300 genpf=1.0");
    dss.command("Set autobuslist=(b1, b2)");
    dss.command("solve mode=autoadd");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    // A winner was instantiated as `Generator.Gadd1`.
    dss.command("? generator.gadd1.kw");
    assert_eq!(dss.result(), "300");

    // GlobalResult is `<bus>, <improvement>` (GENADD form). A second AutoAdd
    // re-searches with gadd1 already present; this is a STRUCTURAL check that the
    // second solve's GlobalResult is well-formed (`<bus>, <numeric figure>`). The
    // exact numeric winner / improvement / log / voltages are pinned against the
    // pinned oracle by `autoadd.dss` in `corpus_live.rs`, not here.
    dss.command("solve mode=autoadd");
    let last = dss.result().to_string();
    let (bus, figure) = last
        .split_once(", ")
        .unwrap_or_else(|| panic!("GlobalResult not `<bus>, <figure>`: {last:?}"));
    assert!(!bus.is_empty(), "winner bus empty in {last:?}");
    assert!(
        figure.parse::<f64>().is_ok(),
        "improvement figure not numeric in {last:?}"
    );

    std::fs::remove_dir_all(&scratch).ok();
}

//! An input that would make a rating or an element's stamp not a finite number,
//! or a rated voltage or kVA base that is not one, is refused, and what it
//! would have changed keeps what it held.

use super::common::query;
use crate::elements::pos_seq::balance::REFUSED_SOLVE;
use crate::exec::*;

/// A `Set %Normal` percentage that takes any Line's normal rating past a finite
/// number refuses the whole command, naming the first such Line. The factor and
/// every Line's ratings keep what they held.
#[test]
fn set_pct_normal_that_overflows_a_line_rating_is_refused() {
    let mut dss = Dss::new();
    for c in [
        "new circuit.c basekv=12.47 bus1=src pu=1.0",
        "new line.l0 bus1=src bus2=a length=1 units=kft normamps=120 emergamps=150",
        "new line.l1 bus1=src bus2=b length=1 units=kft emergamps=1e308",
        "new line.l2 bus1=src bus2=c length=1 units=kft emergamps=1e308",
        "set %normal=80",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    let readings = |dss: &mut Dss| -> Vec<String> {
        ["l0", "l1", "l2"]
            .iter()
            .flat_map(|l| [format!("line.{l}.normamps"), format!("line.{l}.emergamps")])
            .map(|what| query(dss, &what))
            .collect()
    };
    let before = readings(&mut dss);
    assert_eq!(before, ["120", "150", "8E307", "1E308", "8E307", "1E308"]);

    dss.command("set %normal=1e10");
    assert_eq!(
        dss.error_texts(),
        ["Set %Normal: 10000000000 % of Line.l1.EmergAmps 1E308 is not a finite rating."],
        "the whole message, naming the first Line past a finite rating"
    );
    dss.command("get %normal");
    assert_eq!(dss.result(), "80", "the factor keeps what it held");
    assert_eq!(readings(&mut dss), before, "every Line keeps its ratings");
}

/// A stiff 12.47 kV source, a 1 kft line to bus `b` and a 500 kW load there:
/// the circuit each refusal leg adds one element to.
fn refusal_circuit(element: &str) -> Dss {
    let mut dss = Dss::new();
    for c in [
        "new circuit.c basekv=12.47 bus1=src pu=1.0",
        "new line.ln bus1=src bus2=b length=1 units=kft",
        "new load.ld bus1=b kv=12.47 kw=500 kvar=100",
        element,
    ] {
        dss.command(c);
    }
    dss
}

/// Solve and return the node voltages of every bus, in bus order.
fn solved_voltages(dss: &mut Dss) -> Vec<num_complex::Complex64> {
    dss.command("set voltagebases=[12.47 0.48 4.16]");
    dss.command("calcv");
    dss.command("solve");
    node_voltages(dss)
}

/// The node voltages of every bus, in bus order.
fn node_voltages(dss: &mut Dss) -> Vec<num_complex::Complex64> {
    dss.all_bus_voltages()
        .into_iter()
        .flat_map(|b| b.node_v)
        .collect()
}

/// The readings `?` gives for `props` of `element`, as text.
fn readings(dss: &mut Dss, element: &str, props: &[&str]) -> Vec<String> {
    props
        .iter()
        .map(|p| query(dss, &format!("{element}.{p}")))
        .collect()
}

/// An input that makes a stamp quantity or a derived rating of a Capacitor,
/// Reactor, transformer, Load, generator, Storage, PVSystem, induction machine
/// or VCCS not a finite number is refused at the element's recalc, naming the
/// input, every solve refusing on the same message and `Save` writing the input
/// so that the circuit compiles back to the same refusal. The valid value typed
/// next builds the element again as the twin defined with that value.
#[test]
fn an_input_that_makes_a_stamp_non_finite_is_refused_at_the_recalc() {
    let cap = "new capacitor.c1 bus1=b phases=3 kvar=100 kv=12.47";
    let cap_cuf = "new capacitor.c1 bus1=b phases=3 cuf=10 kv=12.47";
    let cap_cuf_n = "new capacitor.c1 bus1=b phases=3 cuf=10 kv=12.47 normamps=100";
    let cap_m = "new capacitor.c1 bus1=b phases=1 cmatrix=[10]";
    let cap_m3 = "new capacitor.c1 bus1=b phases=3 cmatrix=[10 |-2 11 |-1 -3 12]";
    let reac = "new reactor.r1 bus1=b phases=3 kvar=100 kv=12.47";
    let reac_r = "new reactor.r1 bus1=b phases=3 kvar=100 kv=12.47 r=5";
    let reac_rn = "new reactor.r1 bus1=b phases=3 kvar=100 kv=12.47 r=5 normamps=100";
    let reac_rx = "new reactor.r1 bus1=b phases=3 r=1 x=10";
    let reac_m = "new reactor.r1 bus1=b phases=1 rmatrix=[1] xmatrix=[10]";
    let reac_m3 = "new reactor.r1 bus1=b phases=3 rmatrix=[10 |1 20 |2 3 30] \
                   xmatrix=[100 |10 100 |10 10 100]";
    let reac_sym = "new reactor.r1 bus1=b phases=3 z1=[1 1] z0=[1 1] z2=[1 1]";
    let reac_sym1 = "new reactor.r1 bus1=b phases=1 z1=[1 1]";
    let reac_sym_s = "new reactor.r1 bus1=b bus2=r phases=3 z1=[1 1] z0=[1 1] z2=[1 1]";
    let reac_mp = "new reactor.r1 bus1=b phases=3 parallel=yes                    rmatrix=[100 |0 100 |0 0 100] xmatrix=[10 |0 10 |0 0 10]";
    let xf = "new transformer.t1 phases=3 windings=2 buses=[b t] conns=[wye wye] \
              kvs=[12.47 0.48] kvas=[500 500] xhl=6";
    let xf3 = "new transformer.t1 phases=3 windings=3 buses=[b t u] conns=[wye wye wye] \
               kvs=[12.47 0.48 0.48] kvas=[500 500 500] xhl=6 xht=6 xlt=6";
    let xf_e = "new transformer.t1 phases=3 windings=2 buses=[b t] conns=[wye wye] \
                kvs=[12.47 0.48] kvas=[500 500] xhl=6 emerghkva=1e305";
    let xf_s = "new transformer.t1 phases=3 windings=2 buses=[b t] conns=[wye wye] \
                kvs=[12.47 0.48] kvas=[500 500] xhl=6 seasons=2 ratings=[500 1e306]";
    let at = "new autotrans.a1 phases=3 windings=2 buses=[b c] conns=[s w] \
              kvs=[12.47 4.16] kvas=[2000 2000] xhx=5";
    let ld = "new load.l2 bus1=b kv=12.47 kw=100 kvar=20";
    let ld_pf = "new load.l2 bus1=b kv=12.47 kw=100 pf=0.9";
    let ld_open = "new load.l2 bus1=b kv=12.47 kw=100 pf=0.9 rneut=-1 xneut=(1 0 /)";
    let ld_delta = "new load.l2 bus1=b kv=12.47 kw=100 pf=0.9 conn=delta rneut=(0 0 /)";
    let ld_n = "new load.l2 bus1=b kv=12.47 kw=10 pf=0.9";
    let ld_kwh = "new load.l2 bus1=b kv=12.47 kwh=1000 kwhdays=30 pf=0.9";
    let ld_xf = "new load.l2 bus1=b kv=12.47 xfkva=100 allocationfactor=0.5 pf=0.9";
    let gn = "new generator.g1 bus1=b kv=12.47 kw=100 pf=0.9";
    let gn6 = "new generator.g1 bus1=b kv=12.47 kw=100 pf=0.9 model=6";
    let gn3 = "new generator.g1 bus1=b kv=12.47 kw=100 pf=0.9 model=3";
    let wg = "new windgen.w1 bus1=b kv=12.47 kw=100 pf=0.9";
    let sto = "new storage.s1 bus1=b phases=3 kv=12.47 kwrated=100 kwhrated=400 kva=120";
    let pv = "new pvsystem.p1 bus1=b phases=3 kv=12.47 pmpp=100 kva=120 irradiance=1";
    let im = "new indmach012.m1 bus1=b kv=12.47 kw=100 kva=150";
    let vc = "new vccs.v1 bus1=b phases=3 prated=100 vrated=12470";
    let amps: &[&str] = &["normamps", "emergamps"];
    let kva: &[&str] = &["normamps", "emergamps", "normhkva", "emerghkva"];
    let load_all: &[&str] = &["kw", "kvar", "pf", "kva"];
    let powers: &[&str] = &["kw", "kvar", "kva"];
    let msg = |head: &str, quantity: &str| {
        format!("{head} makes its {quantity} not a finite number, so it is not built.")
    };
    // The definition, the element, the refused edit, the valid edit, the
    // readings kept and the message.
    type Leg<'a> = (&'a str, &'a str, &'a str, &'a str, &'a [&'a str], String);
    let legs: Vec<Leg> = vec![
        (
            cap,
            "capacitor.c1",
            "kv=0",
            "kv=12.47",
            amps,
            msg("Capacitor.c1: kV=0", "capacitance"),
        ),
        (
            cap_cuf,
            "capacitor.c1",
            "kv=0",
            "kv=12.47",
            amps,
            msg("Capacitor.c1: kV=0", "normal current rating"),
        ),
        (
            cap_cuf_n,
            "capacitor.c1",
            "kv=0",
            "kv=12.47",
            amps,
            msg("Capacitor.c1: kV=0", "emergency current rating"),
        ),
        (
            cap_cuf,
            "capacitor.c1",
            "cuf=[(1 0 /)]",
            "cuf=[10]",
            amps,
            msg("Capacitor.c1: Cuf=[ +Inf]", "capacitance"),
        ),
        (
            cap_m,
            "capacitor.c1",
            "cmatrix=[(1 0 /)]",
            "cmatrix=[10]",
            amps,
            msg("Capacitor.c1: CMatrix=(+Inf )", "capacitance"),
        ),
        (
            cap_m3,
            "capacitor.c1",
            "cmatrix=[10 |(1 0 /) 11 |-1 -3 12]",
            "cmatrix=[10 |-2 11 |-1 -3 12]",
            amps,
            msg(
                "Capacitor.c1: CMatrix=(10 |+Inf 11 |-1 -3 12 )",
                "capacitance",
            ),
        ),
        (
            cap,
            "capacitor.c1",
            "kvar=[(0 0 /)]",
            "kvar=[100]",
            amps,
            msg("Capacitor.c1: kvar=[ Nan]", "capacitance"),
        ),
        (
            cap,
            "capacitor.c1",
            "harm=5 r=[(1 0 /)]",
            "harm=5 r=[1]",
            amps,
            msg("Capacitor.c1: R=[ +Inf]", "filter impedance"),
        ),
        (
            cap,
            "capacitor.c1",
            "xl=[(1 0 /)]",
            "xl=[5]",
            amps,
            msg("Capacitor.c1: XL=[ +Inf]", "filter impedance"),
        ),
        (
            reac,
            "reactor.r1",
            "kv=0",
            "kv=12.47",
            amps,
            msg("Reactor.r1: kV=0", "admittance"),
        ),
        (
            reac,
            "reactor.r1",
            "kvar=(1 0 /)",
            "kvar=100",
            amps,
            msg("Reactor.r1: kvar=+Inf", "admittance"),
        ),
        (
            reac_r,
            "reactor.r1",
            "kv=0",
            "kv=12.47",
            amps,
            msg("Reactor.r1: kV=0", "normal current rating"),
        ),
        (
            reac_r,
            "reactor.r1",
            "kvar=(1 0 /)",
            "kvar=100",
            amps,
            msg("Reactor.r1: kvar=+Inf", "normal current rating"),
        ),
        (
            reac_rn,
            "reactor.r1",
            "kv=0",
            "kv=12.47",
            amps,
            msg("Reactor.r1: kV=0", "emergency current rating"),
        ),
        (
            reac_rx,
            "reactor.r1",
            "r=0 x=0",
            "r=1 x=10",
            amps,
            msg("Reactor.r1: R=0 X=0", "admittance"),
        ),
        (
            reac_rx,
            "reactor.r1",
            "x=(1 0 /)",
            "x=10",
            amps,
            msg("Reactor.r1: X=+Inf", "reactance"),
        ),
        (
            reac_rx,
            "reactor.r1",
            "r=0 lmh=0",
            "r=1 lmh=10",
            amps,
            msg("Reactor.r1: R=0 LmH=0", "admittance"),
        ),
        (
            reac_rx,
            "reactor.r1",
            "lmh=(1 0 /)",
            "lmh=10",
            amps,
            msg("Reactor.r1: LmH=+Inf", "reactance"),
        ),
        (
            reac_sym,
            "reactor.r1",
            "z1=[0 0] z0=[0 0] z2=[0 0]",
            "z1=[1 1] z0=[1 1] z2=[1 1]",
            amps,
            msg("Reactor.r1: Z1=[0, 0] Z2=[0, 0] Z0=[0, 0]", "admittance"),
        ),
        (
            reac_sym,
            "reactor.r1",
            "z1=[(0 0 /) 1]",
            "z1=[1 1]",
            amps,
            msg("Reactor.r1: Z1=[----, 1]", "admittance"),
        ),
        (
            reac_sym,
            "reactor.r1",
            "z0=[1 (1 0 /)]",
            "z0=[1 1]",
            amps,
            msg("Reactor.r1: Z0=[1, +Inf]", "admittance"),
        ),
        (
            reac_sym1,
            "reactor.r1",
            "z1=[0 0]",
            "z1=[1 1]",
            amps,
            msg("Reactor.r1: Z1=[0, 0]", "admittance"),
        ),
        (
            reac_sym1,
            "reactor.r1",
            "z1=[1e-320 0]",
            "z1=[1 1]",
            amps,
            msg("Reactor.r1: Z1=[9.99988867182683E-321, 0]", "admittance"),
        ),
        (
            reac_sym,
            "reactor.r1",
            "z1=[0 0]",
            "z1=[1 1]",
            amps,
            msg("Reactor.r1: Z1=[0, 0]", "admittance"),
        ),
        (
            reac_sym,
            "reactor.r1",
            "z0=[0 0]",
            "z0=[1 1]",
            amps,
            msg("Reactor.r1: Z0=[0, 0]", "admittance"),
        ),
        (
            reac_sym,
            "reactor.r1",
            "z2=[0 0]",
            "z2=[1 1]",
            amps,
            msg("Reactor.r1: Z2=[0, 0]", "admittance"),
        ),
        (
            reac_sym_s,
            "reactor.r1",
            "z1=[0 0]",
            "z1=[1 1]",
            amps,
            msg("Reactor.r1: Z1=[0, 0]", "admittance"),
        ),
        (
            reac_sym_s,
            "reactor.r1",
            "z0=[0 0]",
            "z0=[1 1]",
            amps,
            msg("Reactor.r1: Z0=[0, 0]", "admittance"),
        ),
        (
            reac_sym_s,
            "reactor.r1",
            "z2=[0 0]",
            "z2=[1 1]",
            amps,
            msg("Reactor.r1: Z2=[0, 0]", "admittance"),
        ),
        (
            reac_sym,
            "reactor.r1",
            "z1=[0 0] z2=[0 0]",
            "z1=[1 1] z2=[1 1]",
            amps,
            msg("Reactor.r1: Z1=[0, 0] Z2=[0, 0]", "admittance"),
        ),
        (
            reac_rx,
            "reactor.r1",
            "r=(0 0 /)",
            "r=1",
            amps,
            msg("Reactor.r1: R=----", "admittance"),
        ),
        (
            reac,
            "reactor.r1",
            "parallel=yes rp=(0 0 /)",
            "parallel=yes rp=1000",
            amps,
            msg("Reactor.r1: Rp=----", "admittance"),
        ),
        (
            reac_m,
            "reactor.r1",
            "rmatrix=[(0 0 /)]",
            "rmatrix=[1]",
            amps,
            msg("Reactor.r1: RMatrix=(---- )", "admittance"),
        ),
        (
            reac_m3,
            "reactor.r1",
            "rmatrix=[10 |1 20 |2 (1 0 /) 30]",
            "rmatrix=[10 |1 20 |2 3 30]",
            amps,
            msg("Reactor.r1: RMatrix=(10 |1 20 |2 +Inf 30 )", "admittance"),
        ),
        (
            reac_m,
            "reactor.r1",
            "xmatrix=[(0 0 /)]",
            "xmatrix=[10]",
            amps,
            msg("Reactor.r1: XMatrix=(---- )", "admittance"),
        ),
        (
            reac_m,
            "reactor.r1",
            "rmatrix=[0] xmatrix=[0]",
            "rmatrix=[1] xmatrix=[10]",
            amps,
            msg("Reactor.r1: RMatrix=(0 ) XMatrix=(0 )", "admittance"),
        ),
        (
            reac_m3,
            "reactor.r1",
            "rmatrix=[0 |0 0 |0 0 0] xmatrix=[0 |0 0 |0 0 0]",
            "rmatrix=[10 |1 20 |2 3 30] xmatrix=[100 |10 100 |10 10 100]",
            amps,
            msg(
                "Reactor.r1: RMatrix=(0 |0 0 |0 0 0 ) XMatrix=(0 |0 0 |0 0 0 )",
                "admittance",
            ),
        ),
        (
            reac_mp,
            "reactor.r1",
            "rmatrix=[0 |0 0 |0 0 0]",
            "rmatrix=[100 |0 100 |0 0 100]",
            amps,
            msg("Reactor.r1: RMatrix=(0 |0 0 |0 0 0 )", "admittance"),
        ),
        (
            xf,
            "transformer.t1",
            "kvs=[0 0.48]",
            "kvs=[12.47 0.48]",
            amps,
            msg("Transformer.t1: kV=0", "normal current rating"),
        ),
        (
            xf,
            "transformer.t1",
            "%loadloss=(1 0 /)",
            "%loadloss=1",
            amps,
            msg("Transformer.t1: %LoadLoss=+Inf", "admittance"),
        ),
        (
            xf,
            "transformer.t1",
            "kvs=[12.47 0]",
            "kvs=[12.47 0.48]",
            amps,
            msg("Transformer.t1: wdg=2 kV=0", "admittance"),
        ),
        (
            xf,
            "transformer.t1",
            "kvas=[0 0]",
            "kvas=[500 500]",
            amps,
            msg("Transformer.t1: kVA=0", "admittance"),
        ),
        (
            xf,
            "transformer.t1",
            "kvas=[1.2e308 1.2e308]",
            "kvas=[500 500]",
            kva,
            msg("Transformer.t1: kVA=1.2E308", "emergency kVA rating"),
        ),
        (
            xf,
            "transformer.t1",
            "kvas=[1.7e308 1.7e308]",
            "kvas=[500 500]",
            kva,
            msg("Transformer.t1: kVA=1.7E308", "normal kVA rating"),
        ),
        (
            xf,
            "transformer.t1",
            "kvas=[1.7e308 1.7e308] normhkva=550",
            "kvas=[500 500]",
            kva,
            msg("Transformer.t1: kVA=1.7E308", "emergency kVA rating"),
        ),
        (
            xf,
            "transformer.t1",
            "kvas=[1.2e308 1.2e308] emerghkva=750",
            "kvas=[500 500]",
            kva,
            msg("Transformer.t1: kVA=1.2E308", "admittance"),
        ),
        (
            xf,
            "transformer.t1",
            "kvas=[1e300 1e300]",
            "kvas=[500 500]",
            amps,
            msg("Transformer.t1: kVA=1E300", "admittance"),
        ),
        (
            xf,
            "transformer.t1",
            "kvs=[1e-310 0.48]",
            "kvs=[12.47 0.48]",
            amps,
            msg(
                "Transformer.t1: kV=9.99999999999997E-311",
                "normal current rating",
            ),
        ),
        (
            xf_e,
            "transformer.t1",
            "wdg=1 kv=1e-5",
            "wdg=1 kv=12.47",
            kva,
            msg("Transformer.t1: kV=0.00001", "emergency current rating"),
        ),
        (
            xf_s,
            "transformer.t1",
            "wdg=1 kv=1e-5",
            "wdg=1 kv=12.47",
            kva,
            msg("Transformer.t1: kV=0.00001", "seasonal current ratings"),
        ),
        (
            xf,
            "transformer.t1",
            "xhl=(0 0 /)",
            "xhl=6",
            amps,
            msg("Transformer.t1: XHL=----", "admittance"),
        ),
        (
            xf,
            "transformer.t1",
            "xscarray=[(0 0 /)]",
            "xscarray=[6]",
            amps,
            msg("Transformer.t1: XSCArray=[ Nan]", "admittance"),
        ),
        (
            xf3,
            "transformer.t1",
            "xht=(0 0 /)",
            "xht=6",
            amps,
            msg("Transformer.t1: XHT=----", "admittance"),
        ),
        (
            xf3,
            "transformer.t1",
            "xlt=(0 0 /)",
            "xlt=6",
            amps,
            msg("Transformer.t1: XLT=----", "admittance"),
        ),
        (
            xf,
            "transformer.t1",
            "%rs=[(0 0 /) 1]",
            "%rs=[1 1]",
            amps,
            msg("Transformer.t1: %R=----", "admittance"),
        ),
        (
            xf,
            "transformer.t1",
            "wdg=2 tap=(0 0 /)",
            "wdg=2 tap=1",
            amps,
            msg("Transformer.t1: wdg=2 Tap=----", "admittance"),
        ),
        (
            xf,
            "transformer.t1",
            "taps=[1 (0 0 /)]",
            "taps=[1 1]",
            amps,
            msg("Transformer.t1: wdg=2 Tap=----", "admittance"),
        ),
        (
            xf,
            "transformer.t1",
            "wdg=1 rdcohms=(0 0 /)",
            "wdg=1 rdcohms=0.1",
            amps,
            msg("Transformer.t1: RDCOhms=----", "dc resistance"),
        ),
        (
            at,
            "autotrans.a1",
            "kvs=[12.47 0]",
            "kvs=[12.47 4.16]",
            amps,
            msg("AutoTrans.a1: wdg=2 kV=0", "admittance"),
        ),
        (
            at,
            "autotrans.a1",
            "%loadloss=(1 0 /)",
            "%loadloss=1",
            amps,
            msg("AutoTrans.a1: %LoadLoss=+Inf", "admittance"),
        ),
        (
            at,
            "autotrans.a1",
            "kvas=[0 0]",
            "kvas=[2000 2000]",
            amps,
            msg("AutoTrans.a1: kVA=0", "admittance"),
        ),
        (
            at,
            "autotrans.a1",
            "kvas=[1.2e308 1.2e308]",
            "kvas=[2000 2000]",
            kva,
            msg("AutoTrans.a1: kVA=1.2E308", "emergency kVA rating"),
        ),
        (
            at,
            "autotrans.a1",
            "kvas=[1.7e308 1.7e308]",
            "kvas=[2000 2000]",
            kva,
            msg("AutoTrans.a1: kVA=1.7E308", "normal kVA rating"),
        ),
        (
            at,
            "autotrans.a1",
            "kvas=[1.7e308 1.7e308] normhkva=2200",
            "kvas=[2000 2000]",
            kva,
            msg("AutoTrans.a1: kVA=1.7E308", "emergency kVA rating"),
        ),
        (
            at,
            "autotrans.a1",
            "kvas=[1.2e308 1.2e308] emerghkva=3000",
            "kvas=[2000 2000]",
            kva,
            msg("AutoTrans.a1: kVA=1.2E308", "admittance"),
        ),
        (
            at,
            "autotrans.a1",
            "kvas=[1e300 1e300]",
            "kvas=[2000 2000]",
            amps,
            msg("AutoTrans.a1: kVA=1E300", "admittance"),
        ),
        (
            at,
            "autotrans.a1",
            "xhx=(0 0 /)",
            "xhx=5",
            amps,
            msg("AutoTrans.a1: XHX=----", "admittance"),
        ),
        (
            at,
            "autotrans.a1",
            "wdg=2 tap=(0 0 /)",
            "wdg=2 tap=1",
            amps,
            msg("AutoTrans.a1: wdg=2 Tap=----", "admittance"),
        ),
        (
            ld,
            "load.l2",
            "kv=0",
            "kv=12.47",
            load_all,
            msg("Load.l2: kV=0", "admittance"),
        ),
        (
            ld_pf,
            "load.l2",
            "pf=0",
            "pf=0.9",
            powers,
            msg("Load.l2: PF=0", "admittance"),
        ),
        (
            ld_open,
            "load.l2",
            "kv=0",
            "kv=12.47",
            load_all,
            msg("Load.l2: kV=0", "admittance"),
        ),
        (
            ld_delta,
            "load.l2",
            "kv=0",
            "kv=12.47",
            load_all,
            msg("Load.l2: kV=0", "admittance"),
        ),
        (
            ld_n,
            "load.l2",
            "rneut=(1 0 /)",
            "rneut=-1",
            load_all,
            msg("Load.l2: Rneut=+Inf", "admittance"),
        ),
        (
            ld_n,
            "load.l2",
            "rneut=(0 0 /)",
            "rneut=-1",
            load_all,
            msg("Load.l2: Rneut=----", "admittance"),
        ),
        (
            ld_kwh,
            "load.l2",
            "kwhdays=0",
            "kwhdays=30",
            load_all,
            msg("Load.l2: kWhDays=0", "admittance"),
        ),
        (
            ld,
            "load.l2",
            "kw=(0 0 /)",
            "kw=100",
            &["kvar", "pf", "kva"],
            msg("Load.l2: kW=----", "admittance"),
        ),
        (
            ld,
            "load.l2",
            "kvar=(1 0 /)",
            "kvar=20",
            &["kw", "pf", "kva"],
            msg("Load.l2: kvar=+Inf", "admittance"),
        ),
        (
            ld_pf,
            "load.l2",
            "pf=(0 0 /)",
            "pf=0.9",
            powers,
            msg("Load.l2: PF=----", "admittance"),
        ),
        (
            ld_kwh,
            "load.l2",
            "kwh=(1 0 /)",
            "kwh=1000",
            load_all,
            msg("Load.l2: kWh=+Inf", "admittance"),
        ),
        (
            ld_kwh,
            "load.l2",
            "cfactor=(1 0 /)",
            "cfactor=4",
            load_all,
            msg("Load.l2: CFactor=+Inf", "admittance"),
        ),
        (
            ld_xf,
            "load.l2",
            "allocationfactor=(1 0 /)",
            "allocationfactor=0.5",
            load_all,
            msg("Load.l2: AllocationFactor=+Inf", "admittance"),
        ),
        (
            gn,
            "generator.g1",
            "kv=0",
            "kv=12.47",
            powers,
            msg("Generator.g1: kV=0", "admittance"),
        ),
        (
            gn,
            "generator.g1",
            "kvar=(1 0 /)",
            "kvar=50",
            &["kw", "pf", "kva"],
            msg("Generator.g1: kvar=+Inf", "admittance"),
        ),
        (
            gn,
            "generator.g1",
            "pf=(0 0 /)",
            "pf=0.9",
            powers,
            msg("Generator.g1: PF=----", "admittance"),
        ),
        (
            gn,
            "generator.g1",
            "kw=(0 0 /)",
            "kw=100",
            &["kvar", "pf", "kva"],
            msg("Generator.g1: kW=----", "admittance"),
        ),
        (
            gn,
            "generator.g1",
            "kvar=(0 0 /)",
            "kvar=50",
            &["kw", "pf", "kva"],
            msg("Generator.g1: kvar=----", "admittance"),
        ),
        (
            gn,
            "generator.g1",
            "pf=1e-310",
            "pf=0.9",
            &["kw", "kvar", "kva", "maxkvar", "minkvar"],
            msg("Generator.g1: PF=9.99999999999997E-311", "admittance"),
        ),
        (
            gn6,
            "generator.g1",
            "xd=0",
            "xd=1",
            powers,
            msg("Generator.g1: Xd=0", "admittance"),
        ),
        (
            gn3,
            "generator.g1",
            "vpu=(0 0 /)",
            "vpu=1",
            powers,
            msg("Generator.g1: Vpu=----", "model current"),
        ),
        (
            gn6,
            "generator.g1",
            "mva=0",
            "mva=0.12",
            &["kw", "kvar"],
            msg("Generator.g1: kVA=0", "admittance"),
        ),
        (
            wg,
            "windgen.w1",
            "kv=0",
            "kv=12.47",
            powers,
            msg("WindGen.w1: kV=0", "admittance"),
        ),
        (
            wg,
            "windgen.w1",
            "pf=0",
            "pf=0.9",
            powers,
            msg("WindGen.w1: PF=0", "kVA rating"),
        ),
        (
            wg,
            "windgen.w1",
            "kvar=(1 0 /)",
            "kvar=50",
            &["kw", "pf", "kva"],
            msg("WindGen.w1: kvar=+Inf", "admittance"),
        ),
        (
            wg,
            "windgen.w1",
            "kw=(0 0 /)",
            "kw=100",
            &["kvar", "pf", "kva"],
            msg("WindGen.w1: kW=----", "admittance"),
        ),
        (
            wg,
            "windgen.w1",
            "kvar=(0 0 /)",
            "kvar=50",
            &["kw", "pf", "kva"],
            msg("WindGen.w1: kvar=----", "admittance"),
        ),
        (
            wg,
            "windgen.w1",
            "pf=1e-310",
            "pf=0.9",
            powers,
            msg("WindGen.w1: PF=9.99999999999997E-311", "admittance"),
        ),
        (
            sto,
            "storage.s1",
            "kv=0",
            "kv=12.47",
            &["kw", "kvar", "kva"],
            msg("Storage.s1: kV=0", "admittance"),
        ),
        (
            sto,
            "storage.s1",
            "kva=0",
            "kva=120",
            &["kw", "kvar"],
            msg("Storage.s1: kVA=1E-8", "inverter output"),
        ),
        (
            sto,
            "storage.s1",
            "kva=0.5",
            "kva=120",
            &["kw", "kvar"],
            msg("Storage.s1: kVA=0.5", "inverter output"),
        ),
        (
            pv,
            "pvsystem.p1",
            "kv=0",
            "kv=12.47",
            &["kva", "pmpp"],
            msg("PVSystem.p1: kV=0", "admittance"),
        ),
        (
            im,
            "indmach012.m1",
            "kv=0",
            "kv=12.47",
            &["kw", "kva"],
            msg("IndMach012.m1: kV=0", "admittance"),
        ),
        (
            im,
            "indmach012.m1",
            "kva=0",
            "kva=150",
            &["kw"],
            msg("IndMach012.m1: kVA=0", "impedance"),
        ),
        (
            im,
            "indmach012.m1",
            "slip=(0 0 /)",
            "slip=0.007",
            &["kw", "kva"],
            msg("IndMach012.m1: Slip=----", "model current"),
        ),
        (
            im,
            "indmach012.m1",
            "kw=(0 0 /)",
            "kw=100",
            &["kva"],
            msg("IndMach012.m1: kW=----", "model current"),
        ),
        (
            ld,
            "load.l2",
            "model=8 zipv=[(0 0 /) 0 0 1 0 0 0.8]",
            "model=8 zipv=[1 0 0 1 0 0 0.8]",
            load_all,
            msg("Load.l2: ZIPV=[ Nan 0 0 1 0 0 0.8]", "model current"),
        ),
        (
            ld,
            "load.l2",
            "model=8 zipv=[1 0 0 (1 0 /) 0 0 0.8]",
            "model=8 zipv=[1 0 0 1 0 0 0.8]",
            load_all,
            msg("Load.l2: ZIPV=[ 1 0 0 +Inf 0 0 0.8]", "model current"),
        ),
        (
            vc,
            "vccs.v1",
            "vrated=1e-310",
            "vrated=12470",
            &["prated", "ppct"],
            msg("VCCS.v1: VRated=9.99999999999997E-311", "rated current"),
        ),
    ];
    let mut failures = Vec::new();
    for (i, (def, element, edit, valid, props, message)) in legs.iter().enumerate() {
        let leg = format!("{element} {edit}");
        let mut dss = refusal_circuit(def);
        assert!(dss.errors().is_empty(), "{leg}: {:?}", dss.error_texts());
        let before = readings(&mut dss, element, props);
        dss.command(&format!("edit {element} {edit}"));
        if dss.error_texts() != [message.as_str()] {
            failures.push(format!("{leg}: logged {:?}", dss.error_texts()));
            continue;
        }
        let after = readings(&mut dss, element, props);
        if after != before {
            failures.push(format!(
                "{leg}: the derived readings {after:?} left their pre-edit {before:?}"
            ));
            continue;
        }
        // `Solve` with no `calcv` before it builds the Y matrix itself.
        let refused = [message.as_str(), REFUSED_SOLVE];
        let solves: Vec<Vec<String>> = (0..2)
            .map(|_| {
                let n = dss.errors().len();
                dss.command("solve");
                dss.error_texts()[n..].to_vec()
            })
            .collect();
        if solves.iter().any(|logged| *logged != refused) {
            failures.push(format!("{leg}: the two solves logged {solves:?}"));
            continue;
        }
        if dss.circuit().unwrap().is_solved {
            failures.push(format!("{leg}: solved"));
            continue;
        }
        let voltages = node_voltages(&mut dss);
        if !voltages
            .iter()
            .all(|v| v.re.is_finite() && v.im.is_finite())
        {
            failures.push(format!(
                "{leg}: a node voltage that is not finite: {voltages:?}"
            ));
            continue;
        }
        // Save writes the refused input, so the saved circuit compiles back
        // to the same refusal (at the `New` line and at the Y build of the
        // saved `CalcVoltageBases`) and its solve builds nothing.
        let mut back = save_and_compile(&mut dss, &format!("recalc_{i}"));
        let reloaded = [message.as_str(), message.as_str(), REFUSED_CALCV];
        if back.error_texts() != reloaded {
            failures.push(format!("{leg}: the reload logged {:?}", back.error_texts()));
            continue;
        }
        let n = back.errors().len();
        back.command("solve");
        if back.error_texts()[n..] != refused || back.circuit().unwrap().is_solved {
            failures.push(format!(
                "{leg}: the reload's solve logged {:?}",
                &back.error_texts()[n..]
            ));
            continue;
        }

        // The valid edit builds the element again: it logs nothing, and the
        // solve gives what the twin typed with that edit alone gives.
        let mut twin = refusal_circuit(def);
        twin.command(&format!("edit {element} {valid}"));
        assert!(
            twin.errors().is_empty(),
            "{leg}: the twin's {valid}: {:?}",
            twin.error_texts()
        );
        let expected = solved_voltages(&mut twin);
        assert!(twin.circuit().unwrap().is_solved, "{leg}: the twin solves");
        let n = dss.errors().len();
        dss.command(&format!("edit {element} {valid}"));
        let voltages = solved_voltages(&mut dss);
        if dss.error_texts()[n..] != *twin.error_texts() {
            failures.push(format!(
                "{leg}: {valid} logged {:?}, the twin {:?}",
                &dss.error_texts()[n..],
                twin.error_texts()
            ));
            continue;
        }
        if !dss.circuit().unwrap().is_solved || voltages != expected {
            failures.push(format!(
                "{leg}: {valid} solves to {voltages:?}, the twin to {expected:?}"
            ));
            continue;
        }
        let (got, want) = (
            readings(&mut dss, element, props),
            readings(&mut twin, element, props),
        );
        if got != want {
            failures.push(format!("{leg}: {valid} reads {got:?}, the twin {want:?}"));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// A refused element is built again by the edit that makes its input valid, a
/// Capacitor by a valid `kv` or filter `XL` and a Transformer or AutoTrans by
/// an `xfmrcode=` with the data it had, and solves as the twin defined that
/// way. A `like=` copy of a refused element is refused for the same input and
/// reads what its source reads.
#[test]
fn a_refused_element_is_built_again_once_its_input_is_valid() {
    let mut twin = refusal_circuit("new capacitor.c1 bus1=b phases=3 kvar=100 kv=12.47");
    let expected = solved_voltages(&mut twin);
    assert!(twin.circuit().unwrap().is_solved);

    let mut dss = refusal_circuit("new capacitor.c1 bus1=b phases=3 kvar=100 kv=0");
    assert_eq!(dss.error_texts().len(), 1, "{:?}", dss.error_texts());
    let n = dss.errors().len();
    dss.command("edit capacitor.c1 kv=12.47");
    assert!(dss.error_texts()[n..].is_empty(), "{:?}", dss.error_texts());
    let voltages = solved_voltages(&mut dss);
    assert!(dss.error_texts()[n..].is_empty(), "{:?}", dss.error_texts());
    assert!(dss.circuit().unwrap().is_solved);
    assert_eq!(voltages, expected, "the twin's voltages");
    assert_eq!(
        readings(&mut dss, "capacitor.c1", &["normamps", "emergamps"]),
        readings(&mut twin, "capacitor.c1", &["normamps", "emergamps"]),
    );

    let filter = &["r", "xl", "normamps", "emergamps"];
    let mut twin = refusal_circuit("new capacitor.c1 bus1=b phases=3 kvar=100 kv=12.47 xl=[5]");
    let expected = solved_voltages(&mut twin);
    assert!(twin.circuit().unwrap().is_solved);
    let mut dss = refusal_circuit("new capacitor.c1 bus1=b phases=3 kvar=100 kv=12.47");
    dss.command("edit capacitor.c1 xl=[(1 0 /)]");
    assert_eq!(
        dss.error_texts(),
        [
            "Capacitor.c1: XL=[ +Inf] makes its filter impedance not a finite number, \
             so it is not built."
        ],
    );
    assert_eq!(query(&mut dss, "capacitor.c1.r"), "[ 0]", "no R derived");
    dss.errors.clear();
    dss.command("edit capacitor.c1 xl=[5]");
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    let voltages = solved_voltages(&mut dss);
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    assert!(dss.circuit().unwrap().is_solved);
    assert_eq!(voltages, expected, "the xl=[5] twin's voltages");
    let twin_readings = readings(&mut twin, "capacitor.c1", filter);
    assert_eq!(twin_readings[0], "[ 0.005]", "R derived from XL");
    assert_eq!(readings(&mut dss, "capacitor.c1", filter), twin_readings);
    let mut back = save_and_reload(&mut dss, "xl");
    assert_eq!(solved_voltages(&mut back), expected, "the saved bank");
    assert_eq!(readings(&mut back, "capacitor.c1", filter), twin_readings);

    let xf_props = &["kvas", "normhkva", "emerghkva", "normamps", "emergamps"];
    let overflow = "kVA=1.2E308 makes its emergency kVA rating not a finite number, \
                    so it is not built.";
    for (object, full, def, code, fetch, copy, copy_object, copy_full) in [
        (
            "transformer.t1",
            "Transformer.t1",
            "new transformer.t1 phases=3 windings=2 buses=[b t] conns=[wye wye] \
             kvs=[12.47 0.48] kvas=[500 500] xhl=6",
            "new xfmrcode.xc phases=3 windings=2 conns=[wye wye] kvs=[12.47 0.48] \
             kvas=[500 500] xhl=6",
            "xfmrcode=xc",
            "new transformer.t2 like=t1 buses=[b t2]",
            "transformer.t2",
            "Transformer.t2",
        ),
        (
            "autotrans.a1",
            "AutoTrans.a1",
            "new autotrans.a1 phases=3 windings=2 buses=[b c] conns=[s w] \
             kvs=[12.47 4.16] kvas=[2000 2000] xhx=5",
            "new xfmrcode.ac phases=3 windings=2 kvs=[12.47 4.16] kvas=[2000 2000] xhl=5",
            "xfmrcode=ac",
            "new autotrans.a2 like=a1 buses=[b c2]",
            "autotrans.a2",
            "AutoTrans.a2",
        ),
    ] {
        let mut twin = refusal_circuit(def);
        twin.command(code);
        twin.command(&format!("edit {object} {fetch}"));
        assert!(
            twin.errors().is_empty(),
            "{object}: {:?}",
            twin.error_texts()
        );
        let expected = solved_voltages(&mut twin);
        assert!(
            twin.circuit().unwrap().is_solved,
            "{object}: the twin solves"
        );
        let twin_readings = readings(&mut twin, object, xf_props);

        let mut dss = refusal_circuit(def);
        dss.command(code);
        dss.command(&format!("edit {object} kvas=[1.2e308 1.2e308]"));
        assert_eq!(dss.error_texts(), [format!("{full}: {overflow}")]);
        dss.errors.clear();
        dss.command(&format!("edit {object} {fetch}"));
        assert!(dss.errors().is_empty(), "{object}: {:?}", dss.error_texts());
        let voltages = solved_voltages(&mut dss);
        assert!(dss.errors().is_empty(), "{object}: {:?}", dss.error_texts());
        assert!(dss.circuit().unwrap().is_solved, "{object}: solved");
        assert_eq!(voltages, expected, "{object}: the {fetch} twin's voltages");
        assert_eq!(readings(&mut dss, object, xf_props), twin_readings);

        let mut dss = refusal_circuit(def);
        dss.command(&format!("edit {object} kvas=[1.2e308 1.2e308]"));
        dss.errors.clear();
        dss.command(copy);
        assert_eq!(
            dss.error_texts(),
            [format!("{copy_full}: {overflow}")],
            "{object}: the copy is refused for the rating of its source"
        );
        assert_eq!(
            readings(&mut dss, copy_object, xf_props),
            readings(&mut dss, object, xf_props),
            "{object}: the copy reads what its refused source reads"
        );
    }
}

/// A refused Generator or WindGen keeps the powers its valid tokens derive from
/// finite inputs, while a power derived from an input that is not finite keeps
/// what it held. The edit that makes the input valid logs nothing and gives the
/// voltages and readings of the twin defined with that input.
#[test]
fn a_refused_edit_keeps_the_powers_its_valid_tokens_derive() {
    let gn = "new generator.g1 bus1=b kv=12.47 kw=100 pf=0.9";
    let wg = "new windgen.w1 bus1=b kv=12.47 kw=100 pf=0.9";
    let shape = "new loadshape.s npts=2 interval=1 mult=[1 2] qmult=[0.5 1] useactual=yes";
    let nan_shape = "new loadshape.sn npts=2 interval=1 mult=[(0 0 /) 2] qmult=[0.5 1] \
                     useactual=yes";
    let plain_shape = "new loadshape.p npts=2 interval=1 mult=[1 0.5]";
    let msg = |head: &str, quantity: &str| {
        format!("{head} makes its {quantity} not a finite number, so it is not built.")
    };
    let adm = "admittance";
    // The definition, the refused commands with what each logs, the readings
    // after them, the valid edit and the twin's definition.
    type Leg<'a> = (
        Vec<&'a str>,
        Vec<(&'a str, String)>,
        Vec<(&'a str, &'a str)>,
        &'a str,
        Vec<&'a str>,
    );
    let legs: Vec<Leg> = vec![
        (
            vec![],
            vec![(
                "new generator.n bus1=b kw=200 pf=0.8 kv=0",
                msg("Generator.n: kV=0", adm),
            )],
            vec![("generator.n.kw", "200"), ("generator.n.kvar", "150")],
            "edit generator.n kv=12.47",
            vec!["new generator.n bus1=b kw=200 pf=0.8 kv=12.47"],
        ),
        (
            vec![gn],
            vec![(
                "edit generator.g1 pf=0.8 kv=0",
                msg("Generator.g1: kV=0", adm),
            )],
            vec![
                ("generator.g1.kvar", "75"),
                ("generator.g1.maxkvar", "150"),
                ("generator.g1.minkvar", "-150"),
            ],
            "edit generator.g1 kv=12.47",
            vec!["new generator.g1 bus1=b kv=12.47 kw=100 pf=0.8"],
        ),
        (
            vec![gn],
            vec![(
                "edit generator.g1 kw=200 kv=0",
                msg("Generator.g1: kV=0", adm),
            )],
            vec![
                ("generator.g1.kvar", "96.8644209675705"),
                ("generator.g1.kva", "240"),
                ("generator.g1.maxkvar", "193.728841935141"),
                ("generator.g1.minkvar", "-193.728841935141"),
            ],
            "edit generator.g1 kv=12.47",
            vec!["new generator.g1 bus1=b kv=12.47 kw=200 pf=0.9"],
        ),
        (
            vec![gn],
            vec![(
                "edit generator.g1 kw=200 kvar=(1 0 /)",
                msg("Generator.g1: kvar=+Inf", adm),
            )],
            vec![
                ("generator.g1.kw", "200"),
                ("generator.g1.pf", "0.9"),
                ("generator.g1.kva", "240"),
                ("generator.g1.maxkvar", "96.8644209675705"),
            ],
            "edit generator.g1 kvar=20",
            vec!["new generator.g1 bus1=b kv=12.47 kw=200 pf=0.9 kvar=20"],
        ),
        (
            vec![shape, gn],
            vec![(
                "edit generator.g1 daily=s kv=0",
                msg("Generator.g1: kV=0", adm),
            )],
            vec![("generator.g1.kw", "2"), ("generator.g1.kvar", "1")],
            "edit generator.g1 kv=12.47",
            vec![
                shape,
                "new generator.g1 bus1=b kv=12.47 kw=100 pf=0.9 daily=s",
            ],
        ),
        (
            vec![shape, nan_shape, gn],
            vec![(
                "edit generator.g1 daily=sn",
                msg("Generator.g1: Daily=sn", adm),
            )],
            vec![
                ("generator.g1.kw", "100"),
                ("generator.g1.kvar", "48.4322104837853"),
                ("generator.g1.pf", "0.9"),
                ("generator.g1.maxkvar", "96.8644209675705"),
            ],
            "edit generator.g1 daily=s",
            vec![
                shape,
                nan_shape,
                "new generator.g1 bus1=b kv=12.47 kw=100 pf=0.9 daily=s",
            ],
        ),
        (
            vec![gn],
            vec![
                (
                    "edit generator.g1 pf=(0 0 /)",
                    msg("Generator.g1: PF=----", adm),
                ),
                (
                    "edit generator.g1 kv=12.4",
                    msg("Generator.g1: PF=----", adm),
                ),
            ],
            vec![
                ("generator.g1.pf", "----"),
                ("generator.g1.kvar", "48.4322104837853"),
                ("generator.g1.maxkvar", "96.8644209675705"),
            ],
            "edit generator.g1 pf=0.9",
            vec!["new generator.g1 bus1=b kv=12.4 kw=100 pf=0.9"],
        ),
        (
            vec![gn],
            vec![
                (
                    "edit generator.g1 pf=1e-310",
                    msg("Generator.g1: PF=9.99999999999997E-311", adm),
                ),
                (
                    "edit generator.g1 kv=12.4",
                    msg("Generator.g1: PF=9.99999999999997E-311", adm),
                ),
            ],
            vec![("generator.g1.kvar", "48.4322104837853")],
            "edit generator.g1 pf=0.9",
            vec!["new generator.g1 bus1=b kv=12.4 kw=100 pf=0.9"],
        ),
        (
            vec![plain_shape, gn],
            vec![
                (
                    "edit generator.g1 kw=(0 0 /)",
                    msg("Generator.g1: kW=----", adm),
                ),
                (
                    "edit generator.g1 daily=p",
                    msg("Generator.g1: kW=----", adm),
                ),
            ],
            vec![
                ("generator.g1.kw", "----"),
                ("generator.g1.kvar", "48.4322104837853"),
            ],
            "edit generator.g1 kw=100",
            vec![
                plain_shape,
                "new generator.g1 bus1=b kv=12.47 kw=100 pf=0.9 daily=p",
            ],
        ),
        (
            vec![gn],
            vec![(
                "new generator.g2 like=g1 kw=(0 0 /)",
                msg("Generator.g2: kW=----", adm),
            )],
            vec![
                ("generator.g2.kvar", "48.4322104837853"),
                ("generator.g2.kva", "120"),
                ("generator.g2.pf", "0.9"),
            ],
            "edit generator.g2 kw=100",
            vec![gn, "new generator.g2 like=g1 kw=100"],
        ),
        (
            vec![],
            vec![(
                "new windgen.n bus1=b kw=200 pf=0.8 kv=0",
                msg("WindGen.n: kV=0", adm),
            )],
            vec![("windgen.n.kw", "200"), ("windgen.n.kvar", "150")],
            "edit windgen.n kv=12.47",
            vec!["new windgen.n bus1=b kw=200 pf=0.8 kv=12.47"],
        ),
        (
            vec![wg],
            vec![("edit windgen.w1 pf=0.8 kv=0", msg("WindGen.w1: kV=0", adm))],
            vec![("windgen.w1.kvar", "75")],
            "edit windgen.w1 kv=12.47",
            vec!["new windgen.w1 bus1=b kv=12.47 kw=100 pf=0.8"],
        ),
        (
            vec![wg],
            vec![("edit windgen.w1 kw=200 kv=0", msg("WindGen.w1: kV=0", adm))],
            vec![("windgen.w1.kvar", "96.8644209675705")],
            "edit windgen.w1 kv=12.47",
            vec!["new windgen.w1 bus1=b kv=12.47 kw=200 pf=0.9"],
        ),
        (
            vec![wg],
            vec![(
                "new windgen.w2 like=w1 kw=(0 0 /)",
                msg("WindGen.w2: kW=----", adm),
            )],
            vec![
                ("windgen.w2.kvar", "48.4322104837853"),
                ("windgen.w2.kva", "111.111111111111"),
            ],
            "edit windgen.w2 kw=100",
            vec![wg, "new windgen.w2 like=w1 kw=100"],
        ),
        (
            vec![wg],
            vec![
                (
                    "edit windgen.w1 pf=(0 0 /)",
                    msg("WindGen.w1: PF=----", adm),
                ),
                (
                    "edit windgen.w1 kv=12.4",
                    msg("WindGen.w1: PF=----", "kVA rating"),
                ),
            ],
            vec![("windgen.w1.kvar", "48.4322104837853")],
            "edit windgen.w1 pf=0.9",
            vec!["new windgen.w1 bus1=b kv=12.4 kw=100 pf=0.9"],
        ),
    ];
    let circuit = |lines: &[&str]| {
        let mut dss = refusal_circuit("");
        for c in lines {
            dss.command(c);
        }
        dss
    };
    let mut failures = Vec::new();
    for (base, refused, kept, valid, twin_lines) in &legs {
        let leg = refused[0].0;
        let mut dss = circuit(base);
        assert!(dss.errors().is_empty(), "{leg}: {:?}", dss.error_texts());
        let logged: Vec<Vec<String>> = refused
            .iter()
            .map(|(c, _)| {
                let n = dss.errors().len();
                dss.command(c);
                dss.error_texts()[n..].to_vec()
            })
            .collect();
        let want: Vec<Vec<String>> = refused.iter().map(|(_, m)| vec![m.clone()]).collect();
        if logged != want {
            failures.push(format!("{leg}: logged {logged:?}"));
            continue;
        }
        let read: Vec<(&str, String)> =
            kept.iter().map(|&(p, _)| (p, query(&mut dss, p))).collect();
        let expected: Vec<(&str, String)> = kept.iter().map(|&(p, v)| (p, v.to_string())).collect();
        if read != expected {
            failures.push(format!("{leg}: reads {read:?}"));
            continue;
        }
        let element = valid.split_whitespace().nth(1).unwrap();
        let mut twin = circuit(twin_lines);
        let expected = solved_voltages(&mut twin);
        assert!(twin.circuit().unwrap().is_solved, "{leg}: the twin solves");
        let n = dss.errors().len();
        dss.command(valid);
        let voltages = solved_voltages(&mut dss);
        if dss.error_texts()[n..] != *twin.error_texts() {
            failures.push(format!(
                "{leg}: {valid} logged {:?}, the twin {:?}",
                &dss.error_texts()[n..],
                twin.error_texts()
            ));
            continue;
        }
        if !dss.circuit().unwrap().is_solved || voltages != expected {
            failures.push(format!(
                "{leg}: {valid} solves to {voltages:?}, the twin to {expected:?}"
            ));
            continue;
        }
        let props = ["kw", "kvar", "pf", "kva"];
        let (got, want) = (
            readings(&mut dss, element, &props),
            readings(&mut twin, element, &props),
        );
        if got != want {
            failures.push(format!("{leg}: {valid} reads {got:?}, the twin {want:?}"));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// A Transformer or AutoTrans refused for a first-winding kVA whose emergency
/// kVA rating overflows keeps both kVA ratings it held. A valid kVA typed next
/// with the second winding active rates the first winding and derives both kVA
/// ratings from it, so the element builds and solves as the twin that types it
/// there directly.
#[test]
fn a_kva_typed_at_winding_2_rebuilds_a_transformer_refused_for_its_kva_rating() {
    let props = &["kvas", "normhkva", "emerghkva", "normamps", "emergamps"];
    for (object, full, def, twin) in [
        (
            "transformer.t1",
            "Transformer.t1",
            "new transformer.t1 phases=3 windings=2 wdg=1 kva=1.3e308 buses=[b t] \
             kvs=[12.47 0.48] xhl=6",
            "new transformer.t1 phases=3 windings=2 buses=[b t] kvs=[12.47 0.48] xhl=6 \
             kva=500",
        ),
        (
            "autotrans.a1",
            "AutoTrans.a1",
            "new autotrans.a1 phases=3 windings=2 wdg=1 kva=1.3e308 buses=[b c] conns=[s w] \
             kvs=[12.47 4.16] xhx=5",
            "new autotrans.a1 phases=3 windings=2 buses=[b c] conns=[s w] kvs=[12.47 4.16] \
             xhx=5 kva=500",
        ),
    ] {
        let mut twin = refusal_circuit(twin);
        assert!(
            twin.errors().is_empty(),
            "{object}: {:?}",
            twin.error_texts()
        );
        let expected = solved_voltages(&mut twin);
        assert!(
            twin.circuit().unwrap().is_solved,
            "{object}: the twin solves"
        );
        let twin_readings = readings(&mut twin, object, props);

        let mut dss = refusal_circuit(def);
        assert_eq!(
            dss.error_texts(),
            [format!(
                "{full}: kVA=1.3E308 makes its emergency kVA rating not a finite number, \
                 so it is not built."
            )],
        );
        assert_eq!(
            readings(&mut dss, object, &["normhkva", "emerghkva"]),
            ["1100", "1500"],
            "{object}: both kVA ratings keep what they held"
        );
        dss.errors.clear();
        dss.command(&format!("edit {object} kva=500"));
        assert!(dss.errors().is_empty(), "{object}: {:?}", dss.error_texts());
        let voltages = solved_voltages(&mut dss);
        assert!(dss.errors().is_empty(), "{object}: {:?}", dss.error_texts());
        assert!(dss.circuit().unwrap().is_solved, "{object}: solved");
        assert_eq!(voltages, expected, "{object}: the twin's voltages");
        assert_eq!(readings(&mut dss, object, props), twin_readings);
        assert_eq!(
            readings(&mut dss, object, &["normhkva", "emerghkva"]),
            [
                crate::util::float_to_str(1.1 * 500.0),
                crate::util::float_to_str(1.5 * 500.0)
            ],
            "{object}: the kVA ratings follow the kVA of winding 1"
        );
    }
}

/// A refused element set aside by `enabled=no` is out of the model: its recalc
/// still logs the refusal at the edit, and the solve logs nothing and converges
/// to the voltages of the circuit without it. That holds on each refusing path,
/// a Capacitor stamp, a Generator recalc, transformer winding data and an input
/// only the dynamics solve reads.
#[test]
fn a_disabled_refused_element_is_out_of_the_model() {
    let dynamics = "solve mode=dynamics number=2 stepsize=0.001";
    let legs = [
        (
            "new capacitor.c1 bus1=b phases=3 kvar=100 kv=0 enabled=no",
            None,
            Some(
                "Capacitor.c1: kV=0 makes its capacitance not a finite number, so it is not built.",
            ),
            None,
        ),
        (
            "new generator.g1 bus1=b kv=0 kw=100 pf=0.9 enabled=no",
            None,
            Some(
                "Generator.g1: kV=0 makes its admittance not a finite number, so it is not built.",
            ),
            None,
        ),
        (
            "new transformer.t1 phases=3 windings=2 buses=[b t] conns=[wye wye] \
             kvs=[0 0.48] kvas=[500 500] xhl=6 enabled=no",
            None,
            Some(
                "Transformer.t1: kV=0 makes its normal current rating not a finite number, so it \
                 is not built.",
            ),
            None,
        ),
        (
            "new generator.g1 bus1=b kv=12.47 kw=100 pf=0.9 enabled=no",
            Some("edit generator.g1 xdp=0"),
            None,
            Some(dynamics),
        ),
    ];
    let mut failures = Vec::new();
    for (def, edit, message, mode) in legs {
        let leg = format!("{def} {}", edit.unwrap_or(""));
        let mut without = refusal_circuit("");
        let mut expected = solved_voltages(&mut without);
        let mut dss = refusal_circuit(def);
        if let Some(edit) = edit {
            dss.command(edit);
        }
        let logged = dss.error_texts();
        if logged != Vec::from_iter(message.map(String::from)) {
            failures.push(format!("{leg}: the edit logged {logged:?}"));
            continue;
        }
        let n = dss.errors().len();
        let mut voltages = solved_voltages(&mut dss);
        if let Some(mode) = mode {
            without.command(mode);
            expected = node_voltages(&mut without);
            dss.command(mode);
            voltages = node_voltages(&mut dss);
        }
        if !dss.error_texts()[n..].is_empty() {
            failures.push(format!(
                "{leg}: the solve logged {:?}",
                &dss.error_texts()[n..]
            ));
        } else if !dss.circuit().unwrap().is_solved {
            failures.push(format!("{leg}: not solved"));
        } else if voltages != expected {
            failures.push(format!(
                "{leg}: solves to {voltages:?}, the circuit without it to {expected:?}"
            ));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// A Capacitor or Reactor of no size is an open device: a Reactor of no kvar,
/// alone, with a resistance or in the parallel form, and a tuned Capacitor of
/// no kvar stamp nothing, log no message, read their derived ratings `none` and
/// solve to the voltages of the circuit without them.
#[test]
fn a_zero_kvar_reactor_is_an_open_device() {
    let mut without = refusal_circuit("");
    let expected = solved_voltages(&mut without);
    for (element, def) in [
        (
            "reactor.r1",
            "new reactor.r1 bus1=b phases=3 kv=12.47 kvar=0",
        ),
        (
            "reactor.r1",
            "new reactor.r1 bus1=b phases=3 kv=12.47 kvar=0 r=5",
        ),
        (
            "reactor.r1",
            "new reactor.r1 bus1=b phases=3 kv=12.47 kvar=0 parallel=yes",
        ),
        (
            "capacitor.c1",
            "new capacitor.c1 bus1=b phases=3 kv=12.47 kvar=0 harm=5",
        ),
    ] {
        let mut dss = refusal_circuit(def);
        assert!(dss.errors().is_empty(), "{def}: {:?}", dss.error_texts());
        assert_eq!(
            readings(&mut dss, element, &["normamps", "emergamps"]),
            ["none", "none"],
            "{def}"
        );
        let voltages = solved_voltages(&mut dss);
        assert!(dss.errors().is_empty(), "{def}: {:?}", dss.error_texts());
        assert!(dss.circuit().unwrap().is_solved, "{def}");
        assert_eq!(voltages, expected, "{def}: the voltages without it");
    }
}

/// A Reactor of two or four phases given by its sequence impedances stamps
/// their mean on its diagonal, so one zero impedance builds and solves as
/// the `R + jX` of that mean, while a zero mean is refused with the whole
/// message naming every impedance, at its edit and at the solve, until the
/// valid impedances typed next build it again.
#[test]
fn a_sequence_impedance_reactor_of_two_or_four_phases_stamps_their_mean() {
    let one_zero = "z1=[0 0] z2=[1.5 1.5] z0=[1.5 1.5]";
    for def in [
        "new reactor.r1 bus1=b phases=2",
        "new reactor.r1 bus1=b.1.2.3.1 phases=4",
    ] {
        let mut twin = refusal_circuit(&format!("{def} r=1 x=1"));
        let expected = solved_voltages(&mut twin);
        assert!(twin.errors().is_empty(), "{def}: {:?}", twin.error_texts());
        assert!(twin.circuit().unwrap().is_solved, "{def}: the twin solves");
        let mut dss = refusal_circuit(&format!("{def} {one_zero}"));
        let voltages = solved_voltages(&mut dss);
        assert!(dss.errors().is_empty(), "{def}: {:?}", dss.error_texts());
        assert!(dss.circuit().unwrap().is_solved, "{def}: not solved");
        assert_eq!(
            voltages, expected,
            "{def} {one_zero}: the voltages of the mean"
        );
        for (edit, inputs) in [
            (
                "z1=[0 0] z2=[0 0] z0=[0 0]",
                "Z1=[0, 0] Z2=[0, 0] Z0=[0, 0]",
            ),
            (
                "z1=[1 1] z2=[1 1] z0=[-2 -2]",
                "Z1=[1, 1] Z2=[1, 1] Z0=[-2, -2]",
            ),
        ] {
            let leg = format!("{def} edited to {edit}");
            let message = format!(
                "Reactor.r1: {inputs} makes its admittance not a finite number, so it is not built."
            );
            let n = dss.errors().len();
            dss.command(&format!("edit reactor.r1 {edit}"));
            assert_eq!(
                dss.error_texts()[n..],
                [message.as_str()],
                "{leg}: the edit"
            );
            let n = dss.errors().len();
            dss.command("solve");
            assert_eq!(
                dss.error_texts()[n..],
                [message.as_str(), REFUSED_SOLVE],
                "{leg}: the solve"
            );
            assert!(!dss.circuit().unwrap().is_solved, "{leg}: solved");
            let voltages = node_voltages(&mut dss);
            assert!(
                voltages
                    .iter()
                    .all(|v| v.re.is_finite() && v.im.is_finite()),
                "{leg}: a node voltage that is not finite: {voltages:?}"
            );
            // The twin solves as often, so both start from the same state.
            let expected = solved_voltages(&mut twin);
            let n = dss.errors().len();
            dss.command(&format!("edit reactor.r1 {one_zero}"));
            let voltages = solved_voltages(&mut dss);
            assert!(
                dss.error_texts()[n..].is_empty(),
                "{leg}: {:?}",
                dss.error_texts()
            );
            assert!(dss.circuit().unwrap().is_solved, "{leg}: not solved again");
            assert_eq!(voltages, expected, "{leg}: built again");
        }
    }
}

/// An input that only the dynamics or the harmonics model reads is checked
/// where that mode computes its admittance, so the snapshot solves with no
/// message and only the mode's solve fails, naming the input and leaving no
/// voltage that is not finite. The legs cover the Generator reactances and kVA,
/// a Load motor branch, the PVSystem and Storage Thevenin impedance and an
/// induction machine whose rotor and magnetizing reactances resonate.
#[test]
fn a_mode_specific_input_is_refused_only_in_its_mode() {
    let gn = "new generator.g1 bus1=b kv=12.47 kw=100 pf=0.9";
    let ld = "new load.l2 bus1=b kv=12.47 kw=100 kvar=20";
    let pv = "new pvsystem.p1 bus1=b phases=3 kv=12.47 pmpp=100 kva=120 irradiance=1";
    let sto = "new storage.s1 bus1=b phases=3 kv=12.47 kwrated=100 kwhrated=400 kva=120";
    let im = "new indmach012.m1 bus1=b kv=12.47 kw=100 kva=150";
    let dynamics = "solve mode=dynamics number=2 stepsize=0.001";
    let harmonics = "solve mode=harmonics";
    let legs = [
        (
            gn,
            "edit generator.g1 xdp=0",
            dynamics,
            "Generator.g1: Xdp=0 makes its dynamic admittance not a finite number, so it is not built.",
        ),
        (
            gn,
            "edit generator.g1 mva=0",
            dynamics,
            "Generator.g1: kVA=0 makes its dynamic admittance not a finite number, so it is not built.",
        ),
        (
            gn,
            "edit generator.g1 xdpp=0",
            harmonics,
            "Generator.g1: Xdpp=0 makes its harmonic admittance not a finite number, so it is not built.",
        ),
        (
            gn,
            "edit generator.g1 mva=0",
            harmonics,
            "Generator.g1: kVA=0 makes its harmonic admittance not a finite number, so it is not built.",
        ),
        (
            ld,
            "edit load.l2 puxharm=0.1 xrharm=0",
            harmonics,
            "Load.l2: XRHarm=0 makes its harmonic admittance not a finite number, so it is not built.",
        ),
        (
            gn,
            "edit generator.g1 xdpp=0",
            dynamics,
            "Generator.g1: Xdpp=0 makes its dynamic admittance not a finite number, so it is not built.",
        ),
        (
            pv,
            "edit pvsystem.p1 %r=0 %x=0",
            harmonics,
            "PVSystem.p1: %R=0 %X=0 makes its harmonic admittance not a finite number, so it is not built.",
        ),
        (
            sto,
            "edit storage.s1 %r=0 %x=0",
            harmonics,
            "Storage.s1: %R=0 %X=0 makes its harmonic admittance not a finite number, so it is not built.",
        ),
        (
            im,
            "edit indmach012.m1 puxr=0.5 puxm=-0.5",
            dynamics,
            "IndMach012.m1: puXr=0.5 puXm=-0.5 makes its transient reactance not a finite number, so it is not built.",
        ),
    ];
    for (def, edit, mode, message) in legs {
        let leg = format!("{edit} at {mode}");
        let mut dss = refusal_circuit(def);
        dss.command(edit);
        assert!(dss.errors().is_empty(), "{leg}: {:?}", dss.error_texts());
        let snapshot = solved_voltages(&mut dss);
        assert!(dss.errors().is_empty(), "{leg}: {:?}", dss.error_texts());
        assert!(
            dss.circuit().unwrap().is_solved,
            "{leg}: the snapshot solves"
        );
        assert!(
            snapshot
                .iter()
                .all(|v| v.re.is_finite() && v.im.is_finite())
        );
        dss.command(mode);
        let errors = dss.error_texts();
        assert!(
            errors.iter().any(|e| e == message),
            "{leg}: the mode refuses with the message: {errors:?}"
        );
        assert!(!dss.circuit().unwrap().is_solved, "{leg}: solved");
        let voltages: Vec<_> = dss
            .all_bus_voltages()
            .into_iter()
            .flat_map(|b| b.node_v)
            .collect();
        assert!(
            voltages
                .iter()
                .all(|v| v.re.is_finite() && v.im.is_finite()),
            "{leg}: a node voltage that is not finite: {voltages:?}"
        );
    }
}

/// A Load voltage region the solve never enters is not refused, nor is an input
/// whose snapshot stamp is finite (a Generator of no kVA under the default
/// model, an induction machine with resonant or lossless rotor data, a
/// VSConverter of no DC voltage and a UPFC of no power factor), each building
/// and solving with no message. The dynamics solve refuses the Generator and
/// the induction machine (`a_mode_specific_input_is_refused_only_in_its_mode`).
#[test]
fn a_region_the_solve_never_enters_is_not_refused() {
    let ld = "new load.l2 bus1=b kv=12.47 kw=100 kvar=20";
    let legs = [
        (ld, "edit load.l2 vminpu=0.5 vlowpu=0.5"),
        (ld, "edit load.l2 vminpu=0"),
        (ld, "edit load.l2 vmaxpu=0"),
        (
            "new generator.g1 bus1=b kv=12.47 kw=100 pf=0.9",
            "edit generator.g1 mva=0",
        ),
        (
            "new indmach012.m1 bus1=b kv=12.47 kw=100 kva=150",
            "edit indmach012.m1 puxr=0.5 puxm=-0.5",
        ),
        (
            "new indmach012.m1 bus1=b kv=12.47 kw=100 kva=150",
            "edit indmach012.m1 purr=0",
        ),
        (
            "new vsconverter.c1 bus1=b phases=3 kvac=12.47 kvdc=20 kw=100",
            "edit vsconverter.c1 kvdc=0",
        ),
        (
            "new upfc.u1 bus1=b bus2=u refkv=12.47 pf=0.9 xs=0.7 mode=1",
            "edit upfc.u1 pf=0",
        ),
    ];
    for (def, edit) in legs {
        let mut dss = refusal_circuit(def);
        dss.command(edit);
        let voltages = solved_voltages(&mut dss);
        assert!(dss.errors().is_empty(), "{edit}: {:?}", dss.error_texts());
        assert!(dss.circuit().unwrap().is_solved, "{edit}: not solved");
        assert!(
            voltages
                .iter()
                .all(|v| v.re.is_finite() && v.im.is_finite()),
            "{edit}: a node voltage that is not finite: {voltages:?}"
        );
    }
}

/// An input the class clamps or defaults to a finite value is not refused, a
/// zero kVA stored as a finite base, a Generator user model with no kVA and a
/// VSConverter at its default impedance among them, while a VCCS of no rated
/// voltage is refused at its token. A Reactor whose `rp` is zero has no
/// parallel resistor, the `Rp` the documentation assumes infinite when not
/// given, and solves as its twin without `rp`.
#[test]
fn a_clamped_or_defaulted_input_is_not_refused() {
    let user_model = "Generator.g1 model designated to use user-written model, \
                      but user-written model is not defined.";
    let legs = [
        (
            "new load.l2 bus1=b kv=12.47 kw=100 kvar=20",
            "edit load.l2 kva=0",
            "load.l2.kva",
        ),
        (
            "new windgen.w1 bus1=b kv=12.47 kw=100 pf=0.9",
            "edit windgen.w1 kva=0",
            "windgen.w1.kva",
        ),
        (
            "new pvsystem.p1 bus1=b phases=3 kv=12.47 pmpp=100 kva=120 irradiance=1",
            "edit pvsystem.p1 kva=0",
            "pvsystem.p1.kva",
        ),
        (
            "new storage.s1 bus1=b phases=3 kv=12.47 kwrated=100 kwhrated=400 kva=120 \
             state=discharging %discharge=50",
            "edit storage.s1 kva=0",
            "storage.s1.kva",
        ),
        (
            "new generator.g1 bus1=b kv=12.47 kw=100 pf=0.9 model=6",
            "edit generator.g1 kva=0",
            "generator.g1.kva",
        ),
    ];
    for (def, edit, kva) in legs {
        let mut dss = refusal_circuit(def);
        dss.command(edit);
        assert!(dss.errors().is_empty(), "{edit}: {:?}", dss.error_texts());
        assert_eq!(query(&mut dss, kva), "1E-8", "{edit}: the clamped kVA");
        let voltages = solved_voltages(&mut dss);
        assert!(
            dss.error_texts().iter().all(|e| e == user_model),
            "{edit}: {:?}",
            dss.error_texts()
        );
        assert!(
            voltages
                .iter()
                .all(|v| v.re.is_finite() && v.im.is_finite()),
            "{edit}: a node voltage that is not finite: {voltages:?}"
        );
    }

    let mut dss = refusal_circuit("new vsconverter.c1 bus1=b phases=3");
    assert_eq!(query(&mut dss, "vsconverter.c1.rac"), "1E-12");
    let voltages = solved_voltages(&mut dss);
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    assert!(
        dss.circuit().unwrap().is_solved,
        "the default converter solves"
    );
    assert!(
        voltages
            .iter()
            .all(|v| v.re.is_finite() && v.im.is_finite())
    );

    let mut dss = refusal_circuit("new vccs.v1 bus1=b phases=3 prated=100 vrated=12470");
    dss.command("edit vccs.v1 vrated=0");
    assert_eq!(
        dss.error_texts(),
        ["VCCS.v1.VRated: Value (0) cannot be zero."],
        "the token message, whole"
    );
    assert_eq!(query(&mut dss, "vccs.v1.vrated"), "12470");

    for def in [
        "new reactor.r1 bus1=b phases=3 kvar=100 kv=12.47",
        "new reactor.r1 bus1=b phases=3 r=10 x=100",
    ] {
        let mut twin = refusal_circuit(def);
        assert_eq!(query(&mut twin, "reactor.r1.rp"), "0", "{def}: no rp=");
        let expected = solved_voltages(&mut twin);
        assert!(twin.circuit().unwrap().is_solved, "{def}: the twin solves");
        let mut dss = refusal_circuit(&format!("{def} rp=0"));
        assert_eq!(query(&mut dss, "reactor.r1.rp"), "0", "{def} rp=0");
        let voltages = solved_voltages(&mut dss);
        assert!(
            dss.errors().is_empty(),
            "{def} rp=0: {:?}",
            dss.error_texts()
        );
        assert!(dss.circuit().unwrap().is_solved, "{def} rp=0: not solved");
        assert_eq!(voltages, expected, "{def} rp=0: the voltages of its twin");
    }
}

/// The value the command parser hands a property: an inline `(…)` or `[…]`
/// without its outer brackets.
fn handed(value: &str) -> &str {
    value
        .strip_prefix(['(', '['])
        .and_then(|v| v.strip_suffix([')', ']']))
        .unwrap_or(value)
}

/// `Save circuit` into a scratch folder, compiled back into a fresh engine
/// that logs no error.
fn save_and_reload(dss: &mut Dss, tag: &str) -> Dss {
    let dir = std::env::temp_dir().join(format!("dss_refusal_{tag}_{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    let fwd = |p: &std::path::Path| p.to_string_lossy().replace('\\', "/");
    dss.command(&format!("save circuit dir=\"{}\"", fwd(&dir)));
    assert!(
        dss.errors().is_empty(),
        "{tag}: save: {:?}",
        dss.error_texts()
    );
    let mut back = Dss::new();
    back.command(&format!("compile \"{}\"", fwd(&dir.join("Master.dss"))));
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        back.errors().is_empty(),
        "{tag}: reload: {:?}",
        back.error_texts()
    );
    back
}

/// The error of a `CalcVoltageBases` whose Y build refuses an element.
const REFUSED_CALCV: &str =
    "Error Encountered in CalcVoltageBases: an element has no defined primitive admittance";

/// `Save circuit` into a scratch folder, which logs nothing, compiled back
/// into a fresh engine whatever the reload logs.
fn save_and_compile(dss: &mut Dss, tag: &str) -> Dss {
    let dir = std::env::temp_dir().join(format!("dss_refusal_{tag}_{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    let fwd = |p: &std::path::Path| p.to_string_lossy().replace('\\', "/");
    let n = dss.errors().len();
    dss.command(&format!("save circuit dir=\"{}\"", fwd(&dir)));
    assert!(
        dss.error_texts()[n..].is_empty(),
        "{tag}: save: {:?}",
        &dss.error_texts()[n..]
    );
    let mut back = Dss::new();
    back.command(&format!("compile \"{}\"", fwd(&dir.join("Master.dss"))));
    std::fs::remove_dir_all(&dir).ok();
    back
}

/// The three values that are not finite numbers each token refusal is tried
/// with: an infinity of either sign and a NaN.
const NOT_FINITE: [&str; 3] = ["(1 0 /)", "(-1 0 /)", "(0 0 /)"];

/// A rating leg: the definition, the object, its name in the message, the
/// property as the class spells it, the readings that keep their values and
/// an array value around the one entry that is not finite.
type RatingLeg<'a> = (
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a [&'a str],
    Option<&'a str>,
);

/// A rated voltage or kVA base leg: the definition, the object, its name in
/// the message, the property as the class spells it, an array value around
/// the one entry that is not finite and what runs after the edits.
type BaseLeg<'a> = (
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    Option<&'a str>,
    Option<&'a str>,
);

/// A kVA rating or a protective device's rating that is not a finite number is
/// refused at its token, and so is an XfmrCode first-winding kVA whose derived
/// kVA rating overflows, the property and the current rating derived from it
/// keeping what they held. A kVA typed at the second winding of a two-winding
/// Transformer, AutoTrans or XfmrCode rates the first winding and derives both
/// kVA ratings, while on three windings, or as a later `kVAs` entry, it derives
/// none.
#[test]
fn a_typed_non_finite_kva_or_device_rating_is_refused() {
    let xf = "new transformer.t1 phases=3 windings=2 buses=[b t] conns=[wye wye] \
              kvs=[12.47 0.48] kvas=[500 500] xhl=6 seasons=2 ratings=[500 600]";
    let at = "new autotrans.a1 phases=3 windings=2 buses=[b c] conns=[s w] \
              kvs=[12.47 4.16] kvas=[2000 2000] xhx=5";
    let xc = "new xfmrcode.xc phases=3 windings=2 kvs=[12.47 0.48] kvas=[500 500] xhl=6 \
              seasons=2 ratings=[500 600]";
    let fu = "new fuse.f1 monitoredobj=line.ln ratedcurrent=100 interruptingrating=1000";
    let rc = "new recloser.rc1 monitoredobj=line.ln ratedcurrent=100 interruptingrating=1000";
    let rl = "new relay.rl1 monitoredobj=line.ln ratedcurrent=100 interruptingrating=1000";
    let sw = "new swtcontrol.s1 switchedobj=line.ln ratedcurrent=100";
    let amps: &[&str] = &["normhkva", "emerghkva", "normamps", "emergamps"];
    let hk: &[&str] = &["normhkva", "emerghkva"];
    let seasonal: &[&str] = &["ratings", "seasons"];
    let pd: &[&str] = &["ratedcurrent", "interruptingrating"];
    let legs: Vec<RatingLeg> = vec![
        (
            xf,
            "transformer.t1",
            "Transformer.t1",
            "NormHkVA",
            amps,
            None,
        ),
        (
            xf,
            "transformer.t1",
            "Transformer.t1",
            "EmergHkVA",
            amps,
            None,
        ),
        (
            xf,
            "transformer.t1",
            "Transformer.t1",
            "Ratings",
            seasonal,
            Some("[500 {}]"),
        ),
        (at, "autotrans.a1", "AutoTrans.a1", "NormHkVA", amps, None),
        (at, "autotrans.a1", "AutoTrans.a1", "EmergHkVA", amps, None),
        (xc, "xfmrcode.xc", "XfmrCode.xc", "NormHkVA", hk, None),
        (xc, "xfmrcode.xc", "XfmrCode.xc", "EmergHkVA", hk, None),
        (
            xc,
            "xfmrcode.xc",
            "XfmrCode.xc",
            "Ratings",
            seasonal,
            Some("[{} 600]"),
        ),
        (fu, "fuse.f1", "Fuse.f1", "RatedCurrent", pd, None),
        (fu, "fuse.f1", "Fuse.f1", "InterruptingRating", pd, None),
        (rc, "recloser.rc1", "Recloser.rc1", "RatedCurrent", pd, None),
        (
            rc,
            "recloser.rc1",
            "Recloser.rc1",
            "InterruptingRating",
            pd,
            None,
        ),
        (rl, "relay.rl1", "Relay.rl1", "RatedCurrent", pd, None),
        (rl, "relay.rl1", "Relay.rl1", "InterruptingRating", pd, None),
        (
            sw,
            "swtcontrol.s1",
            "SwtControl.s1",
            "RatedCurrent",
            &["ratedcurrent"],
            None,
        ),
    ];
    for (def, object, full, prop, props, array) in &legs {
        let leg = format!("{object}.{prop}");
        let mut dss = refusal_circuit(def);
        assert!(dss.errors().is_empty(), "{leg}: {:?}", dss.error_texts());
        let before = readings(&mut dss, object, props);
        for bad in NOT_FINITE {
            let value = array.map_or(bad.to_string(), |a| a.replace("{}", bad));
            dss.command(&format!(
                "edit {object} {}={value}",
                prop.to_ascii_lowercase()
            ));
            assert_eq!(
                dss.error_texts(),
                [format!(
                    "{full}.{prop}: the rating {} is not a finite number.",
                    handed(bad)
                )],
                "{leg}={value}: the whole message"
            );
            dss.errors.clear();
            assert_eq!(
                readings(&mut dss, object, props),
                before,
                "{leg}={value}: the readings keep their values"
            );
        }
        let tag = format!("{}_{}", full.replace('.', "_"), prop).to_ascii_lowercase();
        let mut back = save_and_reload(&mut dss, &tag);
        assert_eq!(
            readings(&mut back, object, props),
            before,
            "{leg}: the saved circuit reads the same"
        );
    }

    let dir = std::env::temp_dir().join(format!("dss_refusal_kva_file_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let file = dir.join("ratings.txt");
    let path = file.to_string_lossy().replace('\\', "/");
    for bad in NOT_FINITE {
        std::fs::write(&file, format!("500\n{bad}\n")).expect("ratings file");
        for (def, object, full) in [
            (xf, "transformer.t1", "Transformer.t1"),
            (xc, "xfmrcode.xc", "XfmrCode.xc"),
        ] {
            let mut dss = refusal_circuit(def);
            let before = readings(&mut dss, object, seasonal);
            dss.command(&format!("edit {object} ratings=(file=\"{path}\")"));
            assert_eq!(
                dss.error_texts(),
                [format!(
                    "{full}.Ratings: a rating in file \"{path}\" is not a finite number."
                )],
                "{object} {bad}: a kVA rating read from a file"
            );
            dss.errors.clear();
            assert_eq!(
                readings(&mut dss, object, seasonal),
                before,
                "{object} {bad}: the file leaves the ratings as they were"
            );
        }
    }
    std::fs::remove_dir_all(&dir).ok();

    let xc_props: &[&str] = &["kvas", "normhkva", "emerghkva"];
    for (edit, message) in [
        (
            "wdg=1 kva=1.2e308",
            "XfmrCode.xc.kVA: the value 1.2e308 derives an emergency kVA rating that is not a \
             finite number.",
        ),
        (
            "wdg=1 kva=1.7e308",
            "XfmrCode.xc.kVA: the value 1.7e308 derives a normal kVA rating that is not a \
             finite number.",
        ),
        (
            "kvas=[1.2e308 1.2e308]",
            "XfmrCode.xc.kVAs: the value 1.2e308 derives an emergency kVA rating that is not a \
             finite number.",
        ),
        (
            "wdg=2 kva=1.2e308",
            "XfmrCode.xc.kVA: the value 1.2e308 derives an emergency kVA rating that is not a \
             finite number.",
        ),
    ] {
        let mut dss = refusal_circuit(xc);
        let before = readings(&mut dss, "xfmrcode.xc", xc_props);
        dss.command(&format!("edit xfmrcode.xc {edit}"));
        assert_eq!(dss.error_texts(), [message], "{edit}: the whole message");
        dss.errors.clear();
        assert_eq!(
            readings(&mut dss, "xfmrcode.xc", xc_props),
            before,
            "{edit}: the code keeps every kVA it held"
        );
        dss.command("new transformer.tx buses=[b x] xfmrcode=xc");
        assert!(dss.errors().is_empty(), "{edit}: {:?}", dss.error_texts());
        assert_eq!(
            readings(&mut dss, "transformer.tx", hk),
            ["550", "750"],
            "{edit}: a Transformer fetching the code is rated as before"
        );
    }
    let mut dss = refusal_circuit(xc);
    dss.command("edit xfmrcode.xc wdg=1 kva=1e308");
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    assert_eq!(
        readings(&mut dss, "xfmrcode.xc", hk),
        ["1.1E308", "1.5E308"],
        "a kVA whose derived ratings stay finite is accepted"
    );
    // A kVA typed at winding 2 of a two-winding unit rates winding 1 and
    // derives both kVA ratings, as a kVA typed at winding 1 does.
    let xf_amps: &[&str] = &["kvas", "normhkva", "emerghkva", "normamps", "emergamps"];
    let derived = [
        crate::util::float_to_str(1.1 * 1000.0),
        crate::util::float_to_str(1.5 * 1000.0),
    ];
    for (def, object, props) in [
        (xf, "transformer.t1", xf_amps),
        (at, "autotrans.a1", xf_amps),
        (xc, "xfmrcode.xc", xc_props),
    ] {
        let at_winding = |wdg: u32| {
            let mut dss = refusal_circuit(def);
            dss.command(&format!("edit {object} wdg={wdg} kva=1000"));
            assert!(
                dss.errors().is_empty(),
                "{object} wdg={wdg}: {:?}",
                dss.error_texts()
            );
            if object == "xfmrcode.xc" {
                dss.command("new transformer.tx buses=[b x] xfmrcode=xc");
                assert!(dss.errors().is_empty(), "{object}: {:?}", dss.error_texts());
            }
            dss
        };
        let (mut second, mut first) = (at_winding(2), at_winding(1));
        let got = readings(&mut second, object, props);
        assert_eq!(
            got[0], "[1000, 1000, ]",
            "{object}: winding 1 takes the kVA"
        );
        assert_eq!(got[1..3], derived, "{object}: both kVA ratings follow it");
        assert_eq!(
            got,
            readings(&mut first, object, props),
            "{object}: as at winding 1"
        );
        if object == "xfmrcode.xc" {
            assert_eq!(
                readings(&mut second, "transformer.tx", xf_amps),
                readings(&mut first, "transformer.tx", xf_amps),
                "a Transformer fetching the code is rated as at winding 1"
            );
        }
        let (a, b) = (solved_voltages(&mut second), solved_voltages(&mut first));
        assert!(
            second.errors().is_empty(),
            "{object}: {:?}",
            second.error_texts()
        );
        assert_eq!(a, b, "{object}: the twin's voltages");
    }

    // On three windings the H winding carries the other two, so a kVA typed
    // at winding 2 rates winding 2 alone.
    for (def, object) in [
        (
            "new transformer.t3 phases=3 windings=3 buses=[b t u] kvs=[12.47 0.48 0.48] \
             kvas=[500 500 500] xhl=6 xht=6 xlt=6",
            "transformer.t3",
        ),
        (
            "new autotrans.a3 phases=3 windings=3 buses=[b c d] conns=[s w d] \
             kvs=[12.47 4.16 0.48] kvas=[500 500 500] xhx=6 xht=6 xxt=6",
            "autotrans.a3",
        ),
    ] {
        let mut dss = refusal_circuit(def);
        dss.command(&format!("edit {object} wdg=2 kva=1000"));
        assert!(dss.errors().is_empty(), "{object}: {:?}", dss.error_texts());
        assert_eq!(
            readings(&mut dss, object, &["kvas", "normhkva", "emerghkva"]),
            ["[500, 1000, 500, ]", "550", "750"],
            "{object}: the H ratings stay"
        );
    }

    // A `kVAs` entry after winding 1 derives no kVA rating, and the fetch
    // builds.
    let mut dss = refusal_circuit(xc);
    dss.command("edit xfmrcode.xc kvas=[500 1.2e308]");
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    assert_eq!(
        readings(&mut dss, "xfmrcode.xc", xc_props),
        ["[500, 1.2E308, ]", "550", "750"],
        "a kVAs entry after winding 1 derives no kVA rating"
    );
    dss.command("new transformer.tx buses=[b x] xfmrcode=xc");
    let voltages = solved_voltages(&mut dss);
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    assert!(dss.circuit().unwrap().is_solved, "not solved");
    assert!(
        voltages
            .iter()
            .all(|v| v.re.is_finite() && v.im.is_finite()),
        "a node voltage that is not finite"
    );
}

/// A rated voltage or kVA base that is not a finite number is refused at its
/// token on every class that has one, an array naming the entry, and the
/// property keeps what it held, the element being built from it. A Transformer
/// fetching a refused XfmrCode is built from what the code held.
#[test]
fn a_typed_non_finite_rated_voltage_or_kva_base_is_refused_at_its_token() {
    let cap = "new capacitor.c1 bus1=b phases=3 kvar=100 kv=12.47";
    let reac = "new reactor.r1 bus1=b phases=3 kvar=100 kv=12.47";
    let ld = "new load.l2 bus1=b kv=12.47 kw=100 kvar=20";
    let gn = "new generator.g1 bus1=b kv=12.47 kw=100 pf=0.9";
    let wg = "new windgen.w1 bus1=b kv=12.47 kw=100 pf=0.9";
    let sto = "new storage.s1 bus1=b phases=3 kv=12.47 kwrated=100 kwhrated=400 kva=120";
    let pv = "new pvsystem.p1 bus1=b phases=3 kv=12.47 pmpp=100 kva=120 irradiance=1";
    let im = "new indmach012.m1 bus1=b kv=12.47 kw=100 kva=150";
    let vc = "new vccs.v1 bus1=b phases=3 prated=100 vrated=12470";
    let vsc = "new vsconverter.c1 bus1=b phases=3 kvac=12.47 kvdc=20 kw=100";
    let xf = "new transformer.t1 phases=3 windings=2 buses=[b t] conns=[wye wye] \
              kvs=[12.47 0.48] kvas=[500 500] xhl=6";
    let at = "new autotrans.a1 phases=3 windings=2 buses=[b c] conns=[s w] \
              kvs=[12.47 4.16] kvas=[2000 2000] xhx=5";
    let xc = "new xfmrcode.xc phases=3 windings=2 kvs=[12.47 0.48] kvas=[500 500] xhl=6";
    let tx = "new transformer.tx buses=[b x] xfmrcode=xc";
    let legs: Vec<BaseLeg> = vec![
        (cap, "capacitor.c1", "Capacitor.c1", "kV", None, None),
        (reac, "reactor.r1", "Reactor.r1", "kV", None, None),
        (ld, "load.l2", "Load.l2", "kV", None, None),
        (ld, "load.l2", "Load.l2", "kVA", None, None),
        (ld, "load.l2", "Load.l2", "XfkVA", None, None),
        (gn, "generator.g1", "Generator.g1", "kV", None, None),
        (gn, "generator.g1", "Generator.g1", "kVA", None, None),
        (gn, "generator.g1", "Generator.g1", "MVA", None, None),
        (wg, "windgen.w1", "WindGen.w1", "kV", None, None),
        (wg, "windgen.w1", "WindGen.w1", "kVA", None, None),
        (wg, "windgen.w1", "WindGen.w1", "MVA", None, None),
        (sto, "storage.s1", "Storage.s1", "kV", None, None),
        (sto, "storage.s1", "Storage.s1", "kVA", None, None),
        (sto, "storage.s1", "Storage.s1", "kVDC", None, None),
        (pv, "pvsystem.p1", "PVSystem.p1", "kV", None, None),
        (pv, "pvsystem.p1", "PVSystem.p1", "kVA", None, None),
        (pv, "pvsystem.p1", "PVSystem.p1", "kVDC", None, None),
        (im, "indmach012.m1", "IndMach012.m1", "kV", None, None),
        (im, "indmach012.m1", "IndMach012.m1", "kVA", None, None),
        (vc, "vccs.v1", "VCCS.v1", "VRated", None, None),
        (vsc, "vsconverter.c1", "VSConverter.c1", "kVAC", None, None),
        (vsc, "vsconverter.c1", "VSConverter.c1", "kVDC", None, None),
        (xf, "transformer.t1", "Transformer.t1", "kV", None, None),
        (xf, "transformer.t1", "Transformer.t1", "kVA", None, None),
        (
            xf,
            "transformer.t1",
            "Transformer.t1",
            "kVs",
            Some("[{} 0.48]"),
            None,
        ),
        (
            xf,
            "transformer.t1",
            "Transformer.t1",
            "kVAs",
            Some("[500 {}]"),
            None,
        ),
        (at, "autotrans.a1", "AutoTrans.a1", "kV", None, None),
        (at, "autotrans.a1", "AutoTrans.a1", "kVA", None, None),
        (
            at,
            "autotrans.a1",
            "AutoTrans.a1",
            "kVs",
            Some("[12.47 {}]"),
            None,
        ),
        (
            at,
            "autotrans.a1",
            "AutoTrans.a1",
            "kVAs",
            Some("[{} 2000]"),
            None,
        ),
        (xc, "xfmrcode.xc", "XfmrCode.xc", "kV", None, Some(tx)),
        (xc, "xfmrcode.xc", "XfmrCode.xc", "kVA", None, Some(tx)),
        (
            xc,
            "xfmrcode.xc",
            "XfmrCode.xc",
            "kVs",
            Some("[{} 0.48]"),
            Some(tx),
        ),
        (
            xc,
            "xfmrcode.xc",
            "XfmrCode.xc",
            "kVAs",
            Some("[500 {}]"),
            Some(tx),
        ),
    ];
    for (def, object, full, prop, array, after) in &legs {
        let leg = format!("{object}.{prop}");
        let lower = prop.to_ascii_lowercase();
        let mut twin = refusal_circuit(def);
        if let Some(after) = after {
            twin.command(after);
        }
        assert!(twin.errors().is_empty(), "{leg}: {:?}", twin.error_texts());
        let twin_voltages = solved_voltages(&mut twin);
        assert!(twin.circuit().unwrap().is_solved, "{leg}: the twin solves");

        let mut dss = refusal_circuit(def);
        let before = readings(&mut dss, object, &[&lower]);
        for bad in NOT_FINITE {
            let value = array.map_or(bad.to_string(), |a| a.replace("{}", bad));
            dss.command(&format!("edit {object} {lower}={value}"));
            assert_eq!(
                dss.error_texts(),
                [format!(
                    "{full}.{prop}: the value {} is not a finite number.",
                    handed(bad)
                )],
                "{leg}={value}: the whole message"
            );
            dss.errors.clear();
            assert_eq!(
                readings(&mut dss, object, &[&lower]),
                before,
                "{leg}={value}: the property keeps what it held"
            );
        }
        if let Some(after) = after {
            dss.command(after);
        }
        let voltages = solved_voltages(&mut dss);
        assert!(dss.errors().is_empty(), "{leg}: {:?}", dss.error_texts());
        assert!(dss.circuit().unwrap().is_solved, "{leg}: built and solved");
        assert_eq!(
            voltages, twin_voltages,
            "{leg}: the element is built from what it held"
        );
    }
}

/// A finite value that is not finite in the unit the element keeps it in, an
/// `MVA` in kVA or a `kVDC` in volts, is refused at its token, while a value
/// that stays finite there is taken and saved. A WindGen whose kVA typed
/// through `MVA` makes its admittance not finite is refused at its recalc,
/// naming the kVA rating.
#[test]
fn a_value_not_finite_in_its_stored_unit_is_refused_at_its_token() {
    let gn = "new generator.g1 bus1=b kv=12.47 kw=100 pf=0.9";
    let wg = "new windgen.w1 bus1=b kv=12.47 kw=100 pf=0.9";
    let sto = "new storage.s1 bus1=b phases=3 kv=12.47 kwrated=100 kwhrated=400 kva=120";
    let pv = "new pvsystem.p1 bus1=b phases=3 kv=12.47 pmpp=100 kva=120 irradiance=1";
    for (def, object, full, prop, unit, file) in [
        (
            gn,
            "generator.g1",
            "Generator.g1",
            "MVA",
            "kVA",
            "Generator.dss",
        ),
        (wg, "windgen.w1", "WindGen.w1", "MVA", "kVA", "WindGen.dss"),
        (
            pv,
            "pvsystem.p1",
            "PVSystem.p1",
            "kVDC",
            "VDC",
            "PVSystem.dss",
        ),
        (
            sto,
            "storage.s1",
            "Storage.s1",
            "kVDC",
            "VDC",
            "Storage.dss",
        ),
    ] {
        let leg = format!("{object}.{prop}");
        let lower = prop.to_ascii_lowercase();
        let mut twin = refusal_circuit(def);
        let twin_voltages = solved_voltages(&mut twin);
        assert!(twin.errors().is_empty(), "{leg}: {:?}", twin.error_texts());

        let mut dss = refusal_circuit(def);
        let before = readings(&mut dss, object, &[&lower, "kva"]);
        dss.command(&format!("edit {object} {lower}=1e306"));
        assert_eq!(
            dss.error_texts(),
            [format!(
                "{full}.{prop}: the value 1e306 is not a finite number in {unit}."
            )],
            "{leg}: the whole message"
        );
        dss.errors.clear();
        assert_eq!(
            readings(&mut dss, object, &[&lower, "kva"]),
            before,
            "{leg}: the property keeps what it held"
        );
        let voltages = solved_voltages(&mut dss);
        assert!(dss.errors().is_empty(), "{leg}: {:?}", dss.error_texts());
        assert_eq!(
            voltages, twin_voltages,
            "{leg}: the element is built from what it held"
        );
        let (texts, mut back) = saved_files(&mut dss, &format!("stored_{lower}_{object}"), &[file]);
        assert!(
            !texts.concat().contains("Inf"),
            "{leg}: Save writes the value it held: {}",
            texts.concat()
        );
        assert_eq!(
            readings(&mut back, object, &[&lower, "kva"]),
            before,
            "{leg}: the saved deck compiles back to the same reading"
        );
    }
    for (def, object, lower, taken, file) in [
        (
            gn,
            "generator.g1",
            "mva",
            ["1E305", "1E308"],
            "Generator.dss",
        ),
        (pv, "pvsystem.p1", "kvdc", ["1E305", "120"], "PVSystem.dss"),
        (sto, "storage.s1", "kvdc", ["1E305", "120"], "Storage.dss"),
    ] {
        let edit = format!("{lower}=1e305");
        let mut dss = refusal_circuit(def);
        dss.command(&format!("edit {object} {edit}"));
        assert!(
            dss.errors().is_empty(),
            "{object} {edit}: {:?}",
            dss.error_texts()
        );
        assert_eq!(
            readings(&mut dss, object, &[lower, "kva"]),
            taken,
            "{object} {edit}"
        );
        let voltages = solved_voltages(&mut dss);
        assert!(
            dss.errors().is_empty(),
            "{object} {edit}: {:?}",
            dss.error_texts()
        );
        assert!(dss.circuit().unwrap().is_solved, "{object} {edit}: solved");
        let mut twin = refusal_circuit(&format!("{def} {edit}"));
        assert_eq!(
            voltages,
            solved_voltages(&mut twin),
            "{object} {edit}: the twin defined with it"
        );
        let (_, mut back) = saved_files(&mut dss, &format!("boundary_{lower}_{object}"), &[file]);
        assert_eq!(
            readings(&mut back, object, &[lower, "kva"]),
            taken,
            "{object} {edit}: the saved deck compiles back to the same readings"
        );
    }
    let mut dss = refusal_circuit(wg);
    dss.command("edit windgen.w1 mva=1e305");
    assert_eq!(
        dss.error_texts(),
        ["WindGen.w1: kVA=1E308 makes its admittance not a finite number, so it is not built."],
        "the WindGen kVA its power is derived from"
    );
}

/// A PVSystem, Storage or WindGen input edited after a dynamics or a harmonics
/// solve is checked against the power-flow output it gives, though either mode
/// keeps the output the element held. The edit logs the refusal and the next
/// snapshot solve fails on it with no voltage that is not finite.
#[test]
fn a_refusal_holds_after_a_dynamics_or_harmonics_solve() {
    let pv = "new pvsystem.p1 bus1=b phases=3 kv=12.47 pmpp=100 kva=120 irradiance=1";
    let sto = "new storage.s1 bus1=b phases=3 kv=12.47 kwrated=100 kwhrated=400 kva=120";
    let wg = "new windgen.w1 bus1=b kv=12.47 kw=100 pf=0.9";
    let pv_msg = "PVSystem.p1: kV=0 makes its admittance not a finite number, so it is not built.";
    let sto_msg =
        "Storage.s1: kVA=1E-8 makes its inverter output not a finite number, so it is not built.";
    let wg_msg =
        "WindGen.w1: Cp=---- makes its admittance not a finite number, so it is not built.";
    let dynamics = "solve mode=dynamics number=2 stepsize=0.001";
    let both: &[&str] = &[dynamics, "solve mode=harmonics"];
    for (def, edit, message, modes) in [
        (pv, "edit pvsystem.p1 kv=0", pv_msg, both),
        (sto, "edit storage.s1 kva=0", sto_msg, both),
        (wg, "edit windgen.w1 cp=(0 0 /)", wg_msg, &[dynamics][..]),
    ] {
        for &mode in modes {
            let leg = format!("{edit} after {mode}");
            let mut dss = refusal_circuit(def);
            solved_voltages(&mut dss);
            dss.command(mode);
            assert!(dss.errors().is_empty(), "{leg}: {:?}", dss.error_texts());
            dss.command(edit);
            assert_eq!(
                dss.error_texts(),
                [message.to_string()],
                "{leg}: the whole message at the edit"
            );
            dss.errors.clear();
            dss.command("set mode=snapshot");
            let voltages = solved_voltages(&mut dss);
            assert!(
                dss.error_texts().iter().any(|e| e == message),
                "{leg}: the Y build error carries the message: {:?}",
                dss.error_texts()
            );
            assert!(!dss.circuit().unwrap().is_solved, "{leg}: solved");
            assert!(
                voltages
                    .iter()
                    .all(|v| v.re.is_finite() && v.im.is_finite()),
                "{leg}: a node voltage that is not finite: {voltages:?}"
            );
        }
    }
}

/// The text of each of `files` that `Save circuit` writes, then the circuit
/// compiled back into a fresh engine that logs no error.
fn saved_files(dss: &mut Dss, tag: &str, files: &[&str]) -> (Vec<String>, Dss) {
    let dir = std::env::temp_dir().join(format!("dss_saved_{tag}_{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    let fwd = |p: &std::path::Path| p.to_string_lossy().replace('\\', "/");
    dss.command(&format!("save circuit dir=\"{}\"", fwd(&dir)));
    assert!(
        dss.errors().is_empty(),
        "{tag}: save: {:?}",
        dss.error_texts()
    );
    let texts = files
        .iter()
        .map(|f| std::fs::read_to_string(dir.join(f)).unwrap_or_else(|e| panic!("{tag}: {f}: {e}")))
        .collect();
    let mut back = Dss::new();
    back.command(&format!("compile \"{}\"", fwd(&dir.join("Master.dss"))));
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        back.errors().is_empty(),
        "{tag}: reload: {:?}",
        back.error_texts()
    );
    (texts, back)
}

/// A conductor normal rating whose derived emergency rating is not a finite
/// number is refused when the edit ends, on WireData, CNData and TSData, and
/// the ratings and `Save` keep what they held, a `like=` earlier in the edit
/// standing. With the emergency rating typed nothing is derived, and a normal
/// rating whose derived emergency rating stays finite is taken.
#[test]
fn a_conductor_rating_whose_emergency_rating_overflows_is_refused() {
    const WIRE: &str = "gmrac=0.0244 diam=0.721 rac=0.306 runits=mi radunits=in gmrunits=ft";
    let ratings = |dss: &mut Dss, obj: &str| -> Vec<String> {
        ["normamps", "emergamps"]
            .iter()
            .map(|p| query(dss, &format!("{obj}.{p}")))
            .collect()
    };
    for (cls, class) in [
        ("wiredata", "WireData"),
        ("cndata", "CNData"),
        ("tsdata", "TSData"),
    ] {
        let obj = format!("{cls}.c1");
        let class_file = format!("{class}.dss");
        let saved = ["Master.dss", class_file.as_str()];
        let message = format!(
            "{class}.c1.NormAmps: the rating 1.7E308 derives an emergency rating that is not a finite number."
        );

        let mut dss = Dss::new();
        dss.command("new circuit.c basekv=12.47 bus1=src pu=1.0");
        dss.command(&format!("new {obj} {WIRE} normamps=1.7e308"));
        assert_eq!(
            dss.error_texts(),
            std::slice::from_ref(&message),
            "{cls}: a fresh conductor"
        );
        dss.errors.clear();
        assert_eq!(
            ratings(&mut dss, &obj),
            ["none", "none"],
            "{cls}: a fresh conductor"
        );
        let (files, mut back) = saved_files(&mut dss, &format!("{cls}_fresh"), &saved);
        let text = files.concat();
        assert!(
            text.contains(&format!("{class}.c1")) && !text.contains("Amps="),
            "{cls}: Save writes no rating: {text}"
        );
        assert_eq!(
            ratings(&mut back, &obj),
            ["none", "none"],
            "{cls}: the reload"
        );
        if cls == "wiredata" {
            dss.command("new linespacing.sp nconds=3 nphases=3 units=ft x=[-4 0 4] h=[28 28 28]");
            dss.command("new line.l bus1=src bus2=b spacing=sp wires=[c1 c1 c1] length=1 units=mi");
            assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
            assert_eq!(
                ratings(&mut dss, "line.l"),
                ["none", "none"],
                "a Line on it"
            );
        }

        let mut dss = Dss::new();
        dss.command("new circuit.c basekv=12.47 bus1=src pu=1.0");
        dss.command(&format!("new {obj} normamps=530 {WIRE}"));
        assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
        let (before, _) = saved_files(&mut dss, &format!("{cls}_before"), &saved);
        dss.command(&format!("edit {obj} normamps=1.7e308"));
        assert_eq!(
            dss.error_texts(),
            std::slice::from_ref(&message),
            "{cls}: a rated conductor"
        );
        dss.errors.clear();
        assert_eq!(
            ratings(&mut dss, &obj),
            ["530", "795"],
            "{cls}: a rated conductor"
        );
        let (after, mut back) = saved_files(&mut dss, &format!("{cls}_after"), &saved);
        assert_eq!(
            after, before,
            "{cls}: Save writes what it wrote before the edit"
        );
        assert_eq!(
            ratings(&mut back, &obj),
            ["530", "795"],
            "{cls}: the reload"
        );

        // A `like=` before the refused rating in the same edit copies a
        // conductor whose emergency rating nobody typed: the copy stands, in
        // an `Edit` of a conductor holding the typed pair `1.7E308`/`800` and
        // in a `New`.
        let mut dss = Dss::new();
        dss.command("new circuit.c basekv=12.47 bus1=src pu=1.0");
        dss.command(&format!("new {cls}.c0 {WIRE} normamps=530"));
        dss.command(&format!("new {obj} {WIRE} emergamps=800 normamps=1.7e308"));
        assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
        dss.command(&format!("edit {obj} like=c0 normamps=1.7e308"));
        dss.command(&format!("new {cls}.c2 like=c0 normamps=1.7e308"));
        assert_eq!(
            dss.error_texts(),
            [message.clone(), message.replace(".c1.", ".c2.")],
            "{cls}: like= in the edit"
        );
        dss.errors.clear();
        for o in [obj.clone(), format!("{cls}.c2")] {
            assert_eq!(
                ratings(&mut dss, &o),
                ["530", "795"],
                "{o}: like= in the edit"
            );
        }
        let (_, mut back) = saved_files(&mut dss, &format!("{cls}_like"), &saved);
        for o in [obj.clone(), format!("{cls}.c2")] {
            assert_eq!(ratings(&mut back, &o), ["530", "795"], "{o}: like= reload");
        }

        for (tag, lines) in [
            (
                "normal first",
                vec![format!("new {obj} {WIRE} normamps=1.7e308 emergamps=800")],
            ),
            (
                "emergency first",
                vec![format!("new {obj} {WIRE} emergamps=800 normamps=1.7e308")],
            ),
            (
                "emergency earlier",
                vec![
                    format!("new {obj} {WIRE} emergamps=800"),
                    format!("edit {obj} normamps=1.7e308"),
                ],
            ),
        ] {
            let mut dss = Dss::new();
            dss.command("new circuit.c basekv=12.47 bus1=src pu=1.0");
            for l in &lines {
                dss.command(l);
            }
            assert!(
                dss.errors().is_empty(),
                "{cls} {tag}: {:?}",
                dss.error_texts()
            );
            assert_eq!(ratings(&mut dss, &obj), ["1.7E308", "800"], "{cls} {tag}");
        }

        // A normal rating near the overflow whose 1.5 times stays finite.
        let mut dss = Dss::new();
        dss.command("new circuit.c basekv=12.47 bus1=src pu=1.0");
        dss.command(&format!("new {obj} {WIRE} normamps=1e308"));
        assert!(
            dss.errors().is_empty(),
            "{cls} 1e308: {:?}",
            dss.error_texts()
        );
        assert_eq!(ratings(&mut dss, &obj), ["1E308", "1.5E308"], "{cls} 1e308");
        let (_, mut back) = saved_files(&mut dss, &format!("{cls}_edge"), &saved);
        assert_eq!(
            ratings(&mut back, &obj),
            ["1E308", "1.5E308"],
            "{cls} 1e308: the reload"
        );
    }
}

/// The refusal of a PVSystem, Storage or WindGen names the first input in
/// property order that is not a finite number among those the refused quantity
/// computes with, else the zero it divides by, else the divisor or the
/// multiplied input of an overflow, never a reading of the output. Each leg
/// logs the whole message at the edit, keeps the power readings and fails the
/// solve on the same message.
#[test]
fn a_refusal_names_the_input_that_is_not_finite_or_overflows() {
    let pv = "new pvsystem.p1 bus1=b phases=3 kv=12.47 pmpp=100 kva=120 irradiance=1";
    let pv_t = "new xycurve.pt npts=2 xarray=[0 100] yarray=[1 0.5]\n\
                new pvsystem.p1 bus1=b phases=3 kv=12.47 pmpp=100 kva=120 irradiance=1 \
                p-tcurve=pt";
    let pv_q = "new pvsystem.p1 bus1=b phases=3 kv=12.47 pmpp=100 kva=120 irradiance=1 \
                pf=0.9 kvarmaxabs=10";
    let pv_w = "new pvsystem.p1 bus1=b phases=3 kv=12.47 pmpp=100 kva=120 irradiance=1 \
                wattpriority=yes";
    let sto = "new storage.s1 bus1=b phases=3 kv=12.47 kwrated=100 kwhrated=400 kva=120";
    let sto_d = "new storage.s1 bus1=b phases=3 kv=12.47 kwrated=100 kwhrated=400 kva=120 \
                 %stored=50 state=discharging";
    let sto_r = "new storage.s1 bus1=b phases=3 kv=12.47 kwrated=100 kwhrated=400";
    let pv_cap = "new pvsystem.p1 bus1=b phases=3 kv=12.47 pmpp=100 kva=120 irradiance=1                   kvarmaxabs=(1 0 /)";
    let sto_cap = "new storage.s1 bus1=b phases=3 kv=12.47 kwrated=100 kwhrated=400 kva=120                    kvarmaxabs=(1 0 /)";
    let pv_p = "new pvsystem.p1 bus1=b phases=3 kv=12.47 pmpp=100 kva=120 irradiance=1 \
                pfpriority=yes";
    let sto_p = "new storage.s1 bus1=b phases=3 kv=12.47 kwrated=100 kwhrated=400 kva=120 \
                 %stored=50 state=discharging pfpriority=yes";
    let pv_n = "new pvsystem.p1 bus1=b phases=3 kv=12.47 pmpp=100 kva=120 irradiance=1 \
                %pminnovars=(0 0 /)";
    let sto_n = "new storage.s1 bus1=b phases=3 kv=12.47 kwrated=100 kwhrated=400 kva=120 \
                 %pminnovars=(0 0 /)";
    let pv_ce = "new xycurve.eff npts=2 xarray=[0 1] yarray=[(0 0 /) 0.9]
                 new pvsystem.p1 bus1=b phases=3 kv=12.47 pmpp=100 kva=120 irradiance=1";
    let pv_ct = "new xycurve.ptn npts=2 xarray=[0 100] yarray=[(0 0 /) 1]
                 new pvsystem.p1 bus1=b phases=3 kv=12.47 pmpp=100 kva=120 irradiance=1";
    let sto_ce = "new xycurve.eff npts=2 xarray=[0 1] yarray=[(0 0 /) 0.9]
                  new storage.s1 bus1=b phases=3 kv=12.47 kwrated=100 kwhrated=400 kva=120";
    let wg = "new windgen.w1 bus1=b kv=12.47 kw=100 pf=0.9";
    let wg_q = "new windgen.w1 bus1=b kv=12.47 kw=100 pf=0.9 kva=120 qmode=1";
    let wg_z = "new windgen.w1 bus1=b kv=12.47 kw=100 pf=0 kva=120";
    let out = "inverter output";
    let adm = "admittance";
    let legs: &[(&str, &str, &str, &str, &str)] = &[
        (
            pv,
            "pvsystem.p1",
            "pmpp=(0 0 /)",
            "PVSystem.p1: Pmpp=----",
            out,
        ),
        (
            pv,
            "pvsystem.p1",
            "irradiance=(0 0 /)",
            "PVSystem.p1: Irradiance=----",
            out,
        ),
        (
            pv,
            "pvsystem.p1",
            "kvar=(0 0 /)",
            "PVSystem.p1: kvar=----",
            out,
        ),
        (
            pv_t,
            "pvsystem.p1",
            "temperature=(0 0 /)",
            "PVSystem.p1: Temperature=----",
            out,
        ),
        (
            pv_q,
            "pvsystem.p1",
            "kvarmax=(0 0 /)",
            "PVSystem.p1: kvarMax=----",
            out,
        ),
        (
            pv_w,
            "pvsystem.p1",
            "%pmpp=(-1 0 /)",
            "PVSystem.p1: %Pmpp=-Inf",
            out,
        ),
        (
            pv_ce,
            "pvsystem.p1",
            "effcurve=eff",
            "PVSystem.p1: EffCurve=eff",
            out,
        ),
        (
            pv_ct,
            "pvsystem.p1",
            "p-tcurve=ptn",
            "PVSystem.p1: P-TCurve=ptn",
            out,
        ),
        (
            sto_ce,
            "storage.s1",
            "effcurve=eff",
            "Storage.s1: EffCurve=eff",
            out,
        ),
        (
            sto,
            "storage.s1",
            "kvar=(0 0 /)",
            "Storage.s1: kvar=----",
            out,
        ),
        (
            sto,
            "storage.s1",
            "kwrated=(0 0 /)",
            "Storage.s1: kWRated=----",
            out,
        ),
        (
            sto,
            "storage.s1",
            "%idlingkw=(0 0 /)",
            "Storage.s1: %IdlingkW=----",
            out,
        ),
        (sto, "storage.s1", "kv=1e-160", "Storage.s1: kV=1E-160", adm),
        (
            sto_d,
            "storage.s1",
            "%discharge=(0 0 /)",
            "Storage.s1: %Discharge=----",
            out,
        ),
        (
            sto_d,
            "storage.s1",
            "kwrated=1e306",
            "Storage.s1: kWRated=1E306",
            adm,
        ),
        (sto, "storage.s1", "kwrated=1e5", "Storage.s1: kVA=120", out),
        (
            sto,
            "storage.s1",
            "kwrated=1e306",
            "Storage.s1: kVA=120",
            out,
        ),
        (
            sto_r,
            "storage.s1",
            "kwrated=1e306",
            "Storage.s1: kWRated=1E306",
            out,
        ),
        (
            wg,
            "windgen.w1",
            "vwind=(0 0 /)",
            "WindGen.w1: VWind=----",
            adm,
        ),
        (wg, "windgen.w1", "cp=(0 0 /)", "WindGen.w1: Cp=----", adm),
        (wg, "windgen.w1", "pd=(1 0 /)", "WindGen.w1: pd=+Inf", adm),
        (wg, "windgen.w1", "rad=(0 0 /)", "WindGen.w1: Rad=----", adm),
        (wg, "windgen.w1", "kv=1e-160", "WindGen.w1: kV=1E-160", adm),
        (wg, "windgen.w1", "kva=1e200", "WindGen.w1: kVA=1E200", adm),
        (wg, "windgen.w1", "pf=(0 0 /)", "WindGen.w1: PF=----", adm),
        (pv, "pvsystem.p1", "pf=0", "PVSystem.p1: PF=0", out),
        (pv, "pvsystem.p1", "pf=(0 0 /)", "PVSystem.p1: PF=----", out),
        (sto, "storage.s1", "pf=0", "Storage.s1: PF=0", out),
        (sto, "storage.s1", "pf=(0 0 /)", "Storage.s1: PF=----", out),
        (pv_cap, "pvsystem.p1", "pf=0", "PVSystem.p1: PF=0", out),
        (sto_cap, "storage.s1", "pf=0", "Storage.s1: PF=0", out),
        (
            pv_p,
            "pvsystem.p1",
            "kva=1e306 pmpp=1e306",
            "PVSystem.p1: kVA=1E306",
            adm,
        ),
        (
            sto_p,
            "storage.s1",
            "kva=1e306 kwrated=1e306",
            "Storage.s1: kVA=1E306",
            adm,
        ),
        (
            pv_n,
            "pvsystem.p1",
            "kv=1e-160",
            "PVSystem.p1: kV=1E-160",
            adm,
        ),
        (
            sto_n,
            "storage.s1",
            "kv=1e-160",
            "Storage.s1: kV=1E-160",
            adm,
        ),
        (wg_q, "windgen.w1", "pf=0", "WindGen.w1: PF=0", adm),
        (
            wg_z,
            "windgen.w1",
            "kv=1e-160",
            "WindGen.w1: kV=1E-160",
            adm,
        ),
    ];
    let mut failures = Vec::new();
    for (def, element, edit, head, quantity) in legs {
        let leg = format!("{element} {edit}");
        let message =
            format!("{head} makes its {quantity} not a finite number, so it is not built.");
        let mut dss = Dss::new();
        for c in [
            "new circuit.c basekv=12.47 bus1=src pu=1.0",
            "new line.ln bus1=src bus2=b length=1 units=kft",
            "new load.ld bus1=b kv=12.47 kw=500 kvar=100",
        ]
        .into_iter()
        .chain(def.lines())
        {
            dss.command(c);
        }
        assert!(dss.errors().is_empty(), "{leg}: {:?}", dss.error_texts());
        let before = readings(&mut dss, element, &["kw", "kvar"]);
        dss.command(&format!("edit {element} {edit}"));
        if dss.error_texts() != [message.as_str()] {
            failures.push(format!("{leg}: logged {:?}", dss.error_texts()));
            continue;
        }
        let after = readings(&mut dss, element, &["kw", "kvar"]);
        if after != before {
            failures.push(format!(
                "{leg}: the readings {after:?} left their pre-edit {before:?}"
            ));
            continue;
        }
        let n = dss.errors().len();
        let voltages = solved_voltages(&mut dss);
        assert!(
            dss.error_texts()[n..].contains(&message),
            "{leg}: the Y build error carries the message"
        );
        assert!(!dss.circuit().unwrap().is_solved, "{leg}: solved");
        assert!(
            voltages
                .iter()
                .all(|v| v.re.is_finite() && v.im.is_finite()),
            "{leg}: a node voltage that is not finite"
        );
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// A refusal whose inputs are all finite and none zero names the cause of the
/// overflow, the multiplied input of a product or the divisor of a quotient, in
/// every Generator, WindGen, induction machine, Load, Reactor, Capacitor,
/// Storage, Transformer and AutoTrans leg, the legs where only an intermediate
/// product overflows pinning that boundary as it stands. Each logs the whole
/// message once at its definition, and the solve fails on the same message with
/// no voltage that is not finite.
#[test]
fn an_overflow_of_finite_inputs_names_its_cause() {
    let adm = "admittance";
    let legs = [
        (
            "new generator.g1 bus1=b kv=12.47 kw=100 kvar=1e306 model=1",
            "Generator.g1: kvar=1E306",
            adm,
        ),
        (
            "new generator.g1 bus1=b kv=12.47 kw=1e306",
            "Generator.g1: kW=1E306",
            adm,
        ),
        (
            "new generator.g1 bus1=b kv=1e-10 kw=1e300",
            "Generator.g1: kV=1E-10",
            adm,
        ),
        (
            "new generator.g1 bus1=b kv=1e-170 kw=100",
            "Generator.g1: kV=1E-170",
            adm,
        ),
        (
            "new transformer.t1 phases=3 windings=2 buses=[b t] kvs=[12.47 1e-310] \
             kvas=[500 500] xhl=6",
            "Transformer.t1: wdg=2 kV=9.99999999999997E-311",
            adm,
        ),
        (
            "new indmach012.m1 bus1=b kv=1e-200 kw=100 kva=150",
            "IndMach012.m1: kV=1E-200",
            adm,
        ),
        (
            "new indmach012.m1 bus1=b kv=12.47 kw=100 kva=150 purs=1e306",
            "IndMach012.m1: puRs=1E306",
            "impedance",
        ),
        (
            "new indmach012.m1 bus1=b kv=1e-160 kw=100 kva=150",
            "IndMach012.m1: kV=1E-160",
            adm,
        ),
        (
            "new indmach012.m1 bus1=b kv=1e200 kw=100 kva=150",
            "IndMach012.m1: kV=1E200",
            "impedance",
        ),
        (
            "new generator.g1 bus1=b kv=1e-170 kw=100 model=6",
            "Generator.g1: kV=1E-170",
            adm,
        ),
        (
            "new generator.g1 bus1=b kv=1e-160 kw=100 pf=0.9 model=6",
            "Generator.g1: kV=1E-160",
            adm,
        ),
        (
            "new generator.g1 bus1=b kv=1e200 kw=100 pf=0.9 model=6",
            "Generator.g1: kV=1E200",
            adm,
        ),
        (
            "new generator.g1 bus1=b kv=12.47 kw=100 pf=0.9 model=6 xd=1e-310",
            "Generator.g1: Xd=9.99999999999997E-311",
            adm,
        ),
        (
            "new generator.g1 bus1=b kv=12.47 kw=1e300 pf=0.9 model=6",
            "Generator.g1: kW=1E300",
            adm,
        ),
        (
            "new generator.g1 bus1=b kv=12.47 kw=100 pf=0.9 model=6 kva=1e300",
            "Generator.g1: kVA=1E300",
            adm,
        ),
        (
            "new storage.s1 bus1=b phases=3 kv=12.47 kwrated=1e300 kwhrated=400",
            "Storage.s1: kWRated=1E300",
            "inverter output",
        ),
        (
            "new transformer.t1 phases=3 windings=2 buses=[b t] kvs=[12.47 4.16] \
             kvas=[1e306 1e306]",
            "Transformer.t1: kVA=1E306",
            adm,
        ),
        (
            "new autotrans.a1 phases=3 windings=2 buses=[b c] conns=[s w] \
             kvs=[12.47 4.16] kvas=[1e306 1e306]",
            "AutoTrans.a1: kVA=1E306",
            adm,
        ),
        (
            "new transformer.t1 phases=3 windings=2 buses=[b t] kvs=[12.47 4.16] \
             kva=1.2e308",
            "Transformer.t1: kVA=1.2E308",
            "emergency kVA rating",
        ),
        (
            "new autotrans.a1 phases=3 windings=2 buses=[b c] conns=[s w] \
             kvs=[12.47 4.16] kva=1.2e308",
            "AutoTrans.a1: kVA=1.2E308",
            "emergency kVA rating",
        ),
        (
            "new transformer.t1 phases=3 windings=2 buses=[b t] kvs=[12.47 0.48] \
             kvas=[1e-310 1e-310] xhl=6",
            "Transformer.t1: kVA=9.99999999999997E-311",
            "dc resistance",
        ),
        (
            "new autotrans.a1 phases=3 windings=2 buses=[b c] conns=[s w] kvs=[12.47 4.16] \
             kvas=[1e-310 1e-310] xhx=5",
            "AutoTrans.a1: kVA=9.99999999999997E-311",
            "dc resistance",
        ),
        (
            "new transformer.t1 phases=3 windings=2 buses=[b t] kvs=[12.47 0.48] \
             kvas=[500 500] xhl=6 wdg=2 tap=1e-320",
            "Transformer.t1: wdg=2 Tap=9.99988867182683E-321",
            adm,
        ),
        (
            "new transformer.t1 phases=3 windings=2 buses=[b t] kvs=[1e160 0.48]              kvas=[500 500] xhl=6",
            "Transformer.t1: kV=1E160",
            "dc resistance",
        ),
        (
            "new transformer.t1 phases=3 windings=2 buses=[b t] kvs=[12.47 0.48]              kvas=[500 500] xhl=6 %rs=[1e306 0.5]",
            "Transformer.t1: %R=1E306",
            "dc resistance",
        ),
        (
            "new autotrans.a1 phases=3 windings=2 buses=[b c] conns=[s w]              kvs=[12.47 1e-310] kvas=[2000 2000] xhx=5",
            "AutoTrans.a1: wdg=2 kV=9.99999999999997E-311",
            adm,
        ),
        (
            "new windgen.w1 bus1=b phases=3 kv=12.47 kw=500 kvar=1e306",
            "WindGen.w1: kvar=1E306",
            adm,
        ),
        (
            "new windgen.w1 bus1=b phases=3 kv=1e-160 kw=500 kvar=1e306",
            "WindGen.w1: kvar=1E306",
            adm,
        ),
        (
            "new capacitor.c1 bus1=b phases=3 kv=12.47 numsteps=2 kvar=[1e308 1e308]",
            "Capacitor.c1: kvar=[ 1E308 1E308]",
            "total kvar",
        ),
        (
            "new capacitor.c1 bus1=b phases=3 kv=12.47 numsteps=2 kvar=[1e308 1e308] \
             normamps=100 emergamps=150",
            "Capacitor.c1: kvar=[ 1E308 1E308]",
            "total kvar",
        ),
        (
            "new capacitor.c1 bus1=b phases=3 kv=1000 numsteps=2 cuf=[1e308 1e308]",
            "Capacitor.c1: Cuf=[ 1E308 1E308]",
            "total kvar",
        ),
        (
            "new capacitor.c1 bus1=b phases=3 kv=10000 cuf=[1e308]",
            "Capacitor.c1: Cuf=[ 1E308]",
            "total kvar",
        ),
        (
            "new capacitor.c1 bus1=b phases=3 kv=1e155 cuf=[10]",
            "Capacitor.c1: kV=1E155",
            "total kvar",
        ),
        (
            "new capacitor.c1 bus1=b phases=3 kv=12.47 cuf=[1e-310] harm=[5]",
            "Capacitor.c1: Cuf=[ 9.99999983659714E-311]",
            "filter impedance",
        ),
        (
            "new capacitor.c1 bus1=b phases=3 kv=12.47 cuf=[1e-300] harm=[0.001]",
            "Capacitor.c1: Harm=[ 0.001]",
            "filter impedance",
        ),
        (
            "new load.l2 bus1=b kv=1e-156 kw=10 pf=0.9",
            "Load.l2: kV=1E-156",
            adm,
        ),
        (
            "new load.l2 bus1=b kv=1e-310 kw=10 pf=0.9",
            "Load.l2: kV=9.99999999999997E-311",
            adm,
        ),
        (
            "new load.l2 bus1=b kv=1e-310 kw=10 kvar=1e306",
            "Load.l2: kV=9.99999999999997E-311",
            adm,
        ),
        (
            "new load.l2 bus1=b kv=12.47 kw=10 pf=0.9 rneut=1e-320",
            "Load.l2: Rneut=9.99988867182683E-321 Xneut=0",
            adm,
        ),
        (
            "new load.l2 bus1=b kv=12.47 kw=10 pf=0.9 xneut=1e-320 rneut=0",
            "Load.l2: Rneut=0 Xneut=9.99988867182683E-321",
            adm,
        ),
        (
            "new load.l2 bus1=b kv=12.47 kw=10 pf=1e-160",
            "Load.l2: PF=1E-160",
            adm,
        ),
        (
            "new load.l2 bus1=b kv=12.47 kwh=1000 kwhdays=1e-310 pf=0.9",
            "Load.l2: kWhDays=9.99999999999997E-311",
            adm,
        ),
        (
            "new load.l2 bus1=b kv=12.47 kw=10 kvar=1e306",
            "Load.l2: kvar=1E306",
            adm,
        ),
        (
            "new load.l2 bus1=b kv=12.47 xfkva=100 pf=0.9 allocationfactor=1e308",
            "Load.l2: AllocationFactor=1E308",
            adm,
        ),
        (
            "new reactor.r5 bus1=b phases=1 kv=12.47 kvar=1e-320",
            "Reactor.r5: kvar=9.99988867182683E-321",
            "reactance",
        ),
    ];
    let mut failures = Vec::new();
    for (def, head, quantity) in legs {
        let message =
            format!("{head} makes its {quantity} not a finite number, so it is not built.");
        let mut dss = refusal_circuit(def);
        if dss.error_texts() != [message.as_str()] {
            failures.push(format!("{def}: logged {:?}", dss.error_texts()));
            continue;
        }
        let n = dss.errors().len();
        let voltages = solved_voltages(&mut dss);
        assert!(
            dss.error_texts()[n..].contains(&message),
            "{def}: the Y build error carries the message"
        );
        assert!(!dss.circuit().unwrap().is_solved, "{def}: solved");
        assert!(
            voltages
                .iter()
                .all(|v| v.re.is_finite() && v.im.is_finite()),
            "{def}: a node voltage that is not finite"
        );
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// A Load's kVA base is the magnitude of its kW and kvar, finite wherever both
/// are though their squares overflow, so such a Load builds with no message and
/// reads its kVA base and power factor from those powers. `Save` writes the
/// typed powers, and the saved circuit compiles back to the same readings.
#[test]
fn a_finite_power_whose_square_overflows_is_built() {
    let text = crate::util::float_to_str;
    let props: &[&str] = &["kw", "kvar", "kva", "pf"];
    for (edit, kvar, kva, pf) in [
        ("kw=1e200 kvar=0", "0", text(1e200), "1".to_string()),
        (
            "kw=1e200 kvar=1e200",
            "1E200",
            text(std::f64::consts::SQRT_2 * 1e200),
            text(1e200 / (std::f64::consts::SQRT_2 * 1e200)),
        ),
    ] {
        let mut dss = refusal_circuit("new load.l2 bus1=b kv=12.47 kw=10 pf=0.9");
        dss.command(&format!("edit load.l2 {edit}"));
        assert!(dss.errors().is_empty(), "{edit}: {:?}", dss.error_texts());
        let read = readings(&mut dss, "load.l2", props);
        assert_eq!(read, ["1E200", kvar, kva.as_str(), pf.as_str()], "{edit}");
        let voltages = solved_voltages(&mut dss);
        assert!(dss.errors().is_empty(), "{edit}: {:?}", dss.error_texts());
        assert!(dss.circuit().unwrap().is_solved, "{edit}: solved");
        let magnitudes: Vec<String> = voltages
            .iter()
            .map(|v| format!("{:.6}", v.norm()))
            .collect();
        assert_eq!(
            magnitudes,
            [
                "4575.760321",
                "4575.760321",
                "4575.760321",
                "0.000000",
                "0.000000",
                "0.000000"
            ],
            "{edit}: the voltages"
        );
        let mut back = save_and_reload(&mut dss, &format!("kva_base_{}", kvar.len()));
        assert_eq!(
            readings(&mut back, "load.l2", props),
            read,
            "{edit}: the saved circuit"
        );
    }
}

/// A refusal is decided on results, so an input of the model current that
/// leaves it finite is not refused: a Load cut-off voltage that is not a
/// number, the `kW` of an induction machine at a fixed slip and an infinite
/// power that drives a floating slip to its limit. A delta Load stamps no
/// neutral admittance, so a neutral resistance whose reciprocal overflows
/// builds as the same load with an open neutral.
#[test]
fn an_input_that_leaves_the_model_current_finite_is_not_refused() {
    let ld = "new load.l2 bus1=b kv=12.47 kw=100 kvar=20 model=8";
    let im = "new indmach012.m1 bus1=b kv=12.47 kw=100 kva=150";
    let ld_delta = "new load.l2 bus1=b kv=12.47 kw=10 pf=0.9 conn=delta";
    let legs = [
        (
            ld,
            "edit load.l2 zipv=[1 0 0 1 0 0 (0 0 /)]",
            Some("edit load.l2 zipv=[1 0 0 1 0 0 0]"),
        ),
        (
            im,
            "edit indmach012.m1 slipoption=fixedslip kw=(0 0 /)",
            Some("edit indmach012.m1 slipoption=fixedslip"),
        ),
        (im, "edit indmach012.m1 kw=1e306", None),
        (
            ld_delta,
            "edit load.l2 rneut=1e-320",
            Some("edit load.l2 rneut=-1"),
        ),
    ];
    for (def, edit, twin_edit) in legs {
        let mut dss = refusal_circuit(def);
        dss.command(edit);
        assert!(dss.errors().is_empty(), "{edit}: {:?}", dss.error_texts());
        let voltages = solved_voltages(&mut dss);
        assert!(dss.errors().is_empty(), "{edit}: {:?}", dss.error_texts());
        assert!(dss.circuit().unwrap().is_solved, "{edit}: solved");
        assert!(
            voltages
                .iter()
                .all(|v| v.re.is_finite() && v.im.is_finite()),
            "{edit}: a node voltage that is not finite: {voltages:?}"
        );
        if let Some(twin_edit) = twin_edit {
            let mut twin = refusal_circuit(def);
            twin.command(twin_edit);
            assert_eq!(solved_voltages(&mut twin), voltages, "{edit}: the twin");
        }
    }
}

/// A Transformer or AutoTrans that fetches an XfmrCode inside an edit logs the
/// refusal of the state the edit leaves once per edit, at the `New` and again
/// at an `Edit` that fetches the code. An edit that fetches a refused code and
/// corrects it logs nothing and solves as the twin built from the corrected
/// code.
#[test]
fn a_refusal_through_an_xfmrcode_is_logged_once_per_edit() {
    let code = "new xfmrcode.xc0 phases=3 windings=2 kvs=[0 4.16] kvas=[500 500]";
    let refused = "Transformer.t: kV=0 makes its normal current rating not a finite number, so \
                   it is not built.";
    let mut dss = refusal_circuit(code);
    dss.command("new transformer.t buses=[b t] xfmrcode=xc0");
    assert_eq!(dss.error_texts(), [refused], "the New");
    dss.command("edit transformer.t xfmrcode=xc0");
    assert_eq!(dss.error_texts(), [refused, refused], "the Edit");

    let mut auto = refusal_circuit(
        "new xfmrcode.xb phases=3 windings=2 kvs=[12.47 4.16] kvas=[500 500] %rs=[(0 0 /) 1]",
    );
    let refused = "AutoTrans.a: %R=---- makes its admittance not a finite number, so it is not \
                   built.";
    auto.command("new autotrans.a buses=[b c] conns=[s w] xfmrcode=xb");
    assert_eq!(auto.error_texts(), [refused], "the AutoTrans New");
    auto.command("edit autotrans.a xfmrcode=xb");
    assert_eq!(auto.error_texts(), [refused, refused], "the AutoTrans Edit");

    let mut fixed = refusal_circuit(code);
    fixed.command("new transformer.t buses=[b t] xfmrcode=xc0 kvs=[12.47 4.16]");
    assert!(fixed.errors().is_empty(), "{:?}", fixed.error_texts());
    let voltages = solved_voltages(&mut fixed);
    assert!(fixed.circuit().unwrap().is_solved, "solved");
    let mut twin =
        refusal_circuit("new xfmrcode.xc1 phases=3 windings=2 kvs=[12.47 4.16] kvas=[500 500]");
    twin.command("new transformer.t buses=[b t] xfmrcode=xc1");
    assert!(twin.errors().is_empty(), "{:?}", twin.error_texts());
    assert_eq!(
        voltages,
        solved_voltages(&mut twin),
        "the twin of a valid code"
    );
}

/// `Set AllocationFactors` and `AllocateLoads` refuse a factor that is not a
/// finite number, or that would leave a load unbuilt where the edit
/// `allocationfactor=` refuses it, with the whole message naming the factor
/// and the first such load. The refused command changes no load and keeps the
/// solved flag, the next solve gives the circuit never set, a load refused as
/// it stands keeps its factor with a message and does not block a valid one,
/// and a valid factor after a refusal solves as its twin.
#[test]
fn an_allocation_factor_set_outside_an_edit_is_refused_where_its_load_would_not_build() {
    let deck = |loads: &[&str], meter: Option<&str>| {
        let mut dss = Dss::new();
        for c in [
            "new circuit.c basekv=12.47 bus1=src pu=1.0",
            "new line.ln bus1=src bus2=b",
        ]
        .into_iter()
        .chain(loads.iter().copied())
        .chain(meter)
        {
            dss.command(c);
        }
        dss
    };
    let half = "new load.l1 bus1=b kv=12.47 xfkva=100 pf=0.9 allocationfactor=0.5";
    let large = "new load.l2 bus1=b kv=12.47 xfkva=1e10 pf=0.9 allocationfactor=0.5";
    let plain = "new load.ld bus1=b kv=12.47 kw=500 kvar=100";
    let props: &[&str] = &["kw", "kvar", "allocationfactor"];
    let kept = |dss: &mut Dss, loads: &[&str]| -> Vec<String> {
        loads
            .iter()
            .flat_map(|l| readings(dss, &format!("load.{l}"), props))
            .collect()
    };
    // The kvar of a kW at the power factor 0.9, `kW tan(acos 0.9)`.
    let kvar_of = |kw: f64| crate::util::float_to_str(kw * (1.0 / 0.9_f64.powi(2) - 1.0).sqrt());
    let breaks = |command: &str, factor: &str, load: &str, effect: &str| {
        format!(
            "{command}: AllocationFactor={factor} makes the admittance of Load.{load} not a \
             finite number, so {effect}."
        )
    };
    let not_finite = |command: &str, factor: &str, of: &str, effect: &str| {
        format!("{command}: AllocationFactor={factor}{of} is not a finite number, so {effect}.")
    };
    let set = "Set AllocationFactors";
    let none_takes = "no load takes it";

    // `Set AllocationFactors` on solved circuits: a factor whose kW base
    // overflows, two whose base is finite and whose admittance is not, a
    // factor that breaks the second load only, and factors that are not
    // finite numbers, with and without a connected-kVA load.
    let one: &[&str] = &[half];
    let two: &[&str] = &[half, large];
    let ld: &[&str] = &[plain];
    let cases: Vec<(&[&str], &[&str], &str, String)> = vec![
        (
            one,
            &["l1"],
            "1e308",
            breaks(set, "1E308", "l1", none_takes),
        ),
        (
            one,
            &["l1"],
            "1e305",
            breaks(set, "1E305", "l1", none_takes),
        ),
        (
            one,
            &["l1"],
            "1e304",
            breaks(set, "1E304", "l1", none_takes),
        ),
        (
            two,
            &["l1", "l2"],
            "1e300",
            breaks(set, "1E300", "l2", none_takes),
        ),
        (
            ld,
            &["ld"],
            "(0 0 /)",
            not_finite(set, "----", "", none_takes),
        ),
        (
            ld,
            &["ld"],
            "(1 0 /)",
            not_finite(set, "+Inf", "", none_takes),
        ),
        (
            one,
            &["l1"],
            "(0 0 /)",
            not_finite(set, "----", "", none_takes),
        ),
    ];
    for (loads, names, factor, message) in &cases {
        let mut twin = deck(loads, None);
        solved_voltages(&mut twin);
        let never_set = solved_voltages(&mut twin);
        let mut dss = deck(loads, None);
        assert!(dss.errors().is_empty(), "{factor}: {:?}", dss.error_texts());
        solved_voltages(&mut dss);
        let before = kept(&mut dss, names);
        dss.command(&format!("set allocationfactors={factor}"));
        assert_eq!(
            dss.error_texts(),
            [message.as_str()],
            "{factor}: the whole message"
        );
        dss.errors.clear();
        assert!(
            dss.circuit().unwrap().is_solved,
            "{factor}: the solved flag is kept"
        );
        assert_eq!(
            kept(&mut dss, names),
            before,
            "{factor}: every load keeps its factor and base"
        );
        assert_eq!(
            solved_voltages(&mut dss),
            never_set,
            "{factor}: the circuit never set"
        );
        assert!(dss.errors().is_empty(), "{factor}: {:?}", dss.error_texts());
    }

    // A valid factor after a refusal, and beside a load refused as it stands,
    // which keeps its factor and is logged.
    let refused = "new load.lz bus1=b kv=0 kw=10 pf=0.9";
    let skip = |command: &str, factor: &str| {
        format!(
            "{command}: Load.lz is refused as it stands, so it keeps AllocationFactor={factor}."
        )
    };
    for (loads, tag) in [
        (one, "after a refusal"),
        (&[half, refused][..], "beside a refused load"),
    ] {
        let mut dss = deck(loads, None);
        dss.errors.clear();
        dss.command("set allocationfactors=1e308");
        dss.errors.clear();
        let lz = (loads.len() == 2).then(|| readings(&mut dss, "load.lz", props));
        dss.command("set allocationfactors=0.6");
        match &lz {
            Some(before) => {
                assert_eq!(dss.error_texts(), [skip(set, "0.5")], "{tag}");
                dss.errors.clear();
                assert_eq!(
                    &readings(&mut dss, "load.lz", props),
                    before,
                    "{tag}: the refused load keeps its factor"
                );
            }
            None => assert!(dss.errors().is_empty(), "{tag}: {:?}", dss.error_texts()),
        }
        assert_eq!(
            readings(&mut dss, "load.l1", props),
            ["54".to_string(), kvar_of(54.0), "0.6".to_string()],
            "{tag}: a factor whose load builds is taken"
        );
        if loads.len() == 1 {
            let voltages = solved_voltages(&mut dss);
            assert!(dss.circuit().unwrap().is_solved, "{tag}: solved");
            let twin = "new load.l1 bus1=b kv=12.47 xfkva=100 pf=0.9 allocationfactor=0.6";
            assert_eq!(
                voltages,
                solved_voltages(&mut deck(&[twin], None)),
                "{tag}: the twin"
            );
        }
    }

    // A factor that would break the skipped load is not stored on it: the
    // edit that makes the load valid builds it with the factor it kept.
    let huge = "new load.lz bus1=b kv=0 xfkva=1e10 pf=0.9";
    let mut dss = deck(&[half, huge], None);
    dss.errors.clear();
    let before = readings(&mut dss, "load.lz", props);
    dss.command("set allocationfactors=1e300");
    assert_eq!(dss.error_texts(), [skip(set, "0.5")], "the skip");
    dss.errors.clear();
    assert_eq!(
        readings(&mut dss, "load.lz", props),
        before,
        "the skipped load stores nothing"
    );
    assert_eq!(query(&mut dss, "load.l1.allocationfactor"), "1E300");
    dss.command("edit load.lz kv=12.47");
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    assert_eq!(query(&mut dss, "load.lz.allocationfactor"), "0.5");
    // `AllocateLoads` skips such a load in its zone the same way, logged once
    // however many passes run.
    let zone = "new load.lz bus1=b kv=12.47 xfkva=100 pf=0.9";
    let mut dss = deck(
        &[half, zone],
        Some("new energymeter.m1 element=line.ln terminal=1 peakcurrent=[40 40 40]"),
    );
    solved_voltages(&mut dss);
    dss.command("edit load.lz kv=0");
    dss.errors.clear();
    let before = readings(&mut dss, "load.lz", props);
    dss.command("allocateloads");
    let skip = skip("AllocateLoads", "0.5");
    let texts = dss.error_texts();
    assert_eq!(
        texts.iter().filter(|t| **t == skip).count(),
        1,
        "the skip, once: {texts:?}"
    );
    dss.errors.clear();
    assert_eq!(
        readings(&mut dss, "load.lz", props),
        before,
        "the skipped load keeps its factor"
    );

    // `AllocateLoads`: the factor of a pass is refused before it changes a
    // load, and the command stops after its guess solve.
    let meter = |peak: &str| {
        format!("new energymeter.m1 element=line.ln terminal=1 peakcurrent=[{peak} {peak} {peak}]")
    };
    let tiny = "new load.l1 bus1=b kv=12.47 xfkva=1e-298 pf=0.9 allocationfactor=1e300";
    let allocate = "AllocateLoads";
    let stops = "the allocation stops";
    // (loads, the loads read back, the meter peak, the load the message names)
    type Leg<'a> = (&'a [&'a str], &'a [&'a str], &'a str, Option<&'a str>);
    let legs: [Leg; 4] = [
        (one, &["l1"], "1e308", Some("l1")),
        (one, &["l1"], "1e306", Some("l1")),
        (two, &["l1", "l2"], "1e308", Some("l2")),
        (&[tiny], &["l1"], "1e10", None),
    ];
    for (loads, names, peak, culprit) in legs {
        let meter = meter(peak);
        // The twin solves as often as the allocation does: its guess solve
        // stands for the pass refused before the first re-solve.
        let mut twin = deck(loads, Some(&meter));
        assert!(twin.errors().is_empty(), "{peak}: {:?}", twin.error_texts());
        solved_voltages(&mut twin);
        twin.command("solve");
        let twin_voltages = solved_voltages(&mut twin);
        let mut dss = deck(loads, Some(&meter));
        solved_voltages(&mut dss);
        let before = kept(&mut dss, names);
        dss.command("allocateloads");
        let texts = dss.error_texts();
        assert_eq!(
            texts.len(),
            1,
            "{peak}: one message, the command stops: {texts:?}"
        );
        match culprit {
            Some(load) => {
                let head = "AllocateLoads: AllocationFactor=";
                let value = texts[0]
                    .strip_prefix(head)
                    .and_then(|rest| rest.split(' ').next())
                    .unwrap_or_else(|| panic!("{peak}: {texts:?}"));
                let v: f64 = value.parse().unwrap_or_else(|_| panic!("{peak}: {value}"));
                assert!(v.is_finite(), "{peak}: a finite factor: {v}");
                if peak == "1e306" {
                    // Its kW base is finite and its per-phase power is not.
                    let kw = 100.0 * v * 0.9;
                    assert!(kw.is_finite() && !(1000.0 * kw / 3.0).is_finite(), "{v}");
                }
                assert_eq!(
                    texts[0],
                    breaks(allocate, value, load, stops),
                    "{peak}: the whole message"
                );
            }
            None => assert_eq!(
                texts[0],
                not_finite(allocate, "+Inf", " of Load.l1", stops),
                "{peak}: the whole message"
            ),
        }
        dss.errors.clear();
        assert!(
            dss.circuit().unwrap().is_solved,
            "{peak}: the solved flag is kept"
        );
        assert_eq!(
            kept(&mut dss, names),
            before,
            "{peak}: every load keeps its factor and base"
        );
        let voltages = solved_voltages(&mut dss);
        assert!(dss.errors().is_empty(), "{peak}: {:?}", dss.error_texts());
        assert!(dss.circuit().unwrap().is_solved, "{peak}: solved");
        assert_eq!(
            voltages, twin_voltages,
            "{peak}: the circuit never allocated"
        );
    }
}

/// A curve the element holds with a point, `X` or `Y`, that is not a finite
/// number refuses the element at its recalc, naming the curve with the whole
/// message whatever mode the element is in, and at the Y build of a dynamics
/// solve for a VCCS outside its RMS mode. Each refused solve logs the same
/// message, solves nothing and leaves no node voltage that is not finite,
/// while the twin with every point valid solves.
#[test]
fn a_curve_point_that_is_not_a_number_refuses_its_element() {
    let finite = |dss: &mut Dss| {
        node_voltages(dss)
            .iter()
            .all(|v| v.re.is_finite() && v.im.is_finite())
    };
    let daily = ["set mode=daily number=2 stepsize=1h", "solve"];
    let snapshot = ["solve"];
    let out = "inverter output";
    let adm = "admittance";
    let pv = "new tshape.ts npts=2 interval=1 temp=[25 40]\n\
              new pvsystem.p1 bus1=b phases=3 kv=12.47 pmpp=100 kva=120 irradiance=1 \
              p-tcurve=pt tdaily=ts";
    let st = "new loadshape.disp npts=2 interval=1 mult=[0 1]\n\
              new storage.s1 bus1=b phases=3 kv=12.47 kwrated=100 kwhrated=400 kva=120 \
              %stored=50 dispmode=follow daily=disp effcurve=eff";
    let upfc = |mode: u8| {
        format!(
            "new upfc.u1 bus1=b bus2=c refkv=12.47 mode={mode} losscurve=lc\n\
             new load.lc bus1=c kv=12.47 kw=100 kvar=10"
        )
    };
    let pt_valid = "new xycurve.pt npts=3 xarray=[0 25 40] yarray=[1 1 0.9]";
    let vv_valid = "new xycurve.vv npts=4 xarray=[0.5 0.9 1.1 1.5] yarray=[1 1 -1 -1]";
    let lc_valid = "new xycurve.lc npts=3 xarray=[0.9 1 1.1] yarray=[1.0143 1.008 1.0143]";
    let lc_broken = "new xycurve.lc npts=3 xarray=[0.9 1 1.1] yarray=[1.0143 (0 0 /) 1.0143]";
    // (the curve, its twin with every point valid, the element reading it,
    // the solve, the message)
    let legs: Vec<(&str, &str, String, &[&str], String)> = vec![
        (
            "new xycurve.pt npts=3 xarray=[0 25 40] yarray=[1 1 (0 0 /)]",
            pt_valid,
            pv.to_string(),
            &daily,
            message("PVSystem.p1", "P-TCurve=pt", out),
        ),
        (
            "new xycurve.pt npts=3 xarray=[0 25 40] yarray=[1 1 (1 0 /)]",
            pt_valid,
            pv.to_string(),
            &daily,
            message("PVSystem.p1", "P-TCurve=pt", out),
        ),
        (
            "new xycurve.pt npts=3 xarray=[0 25 (0 0 /)] yarray=[1 1 0.9]",
            pt_valid,
            pv.to_string(),
            &daily,
            message("PVSystem.p1", "P-TCurve=pt", out),
        ),
        (
            "new xycurve.eff npts=5 xarray=[0 0.3 0.4 0.5 1] yarray=[0.9 0.9 (0 0 /) 0.9 0.9]",
            "new xycurve.eff npts=5 xarray=[0 0.3 0.4 0.5 1] yarray=[0.9 0.9 0.9 0.9 0.9]",
            "new loadshape.irr npts=2 interval=1 mult=[1 0.5]\n\
             new pvsystem.p1 bus1=b phases=3 kv=12.47 pmpp=100 kva=120 irradiance=1 \
             effcurve=eff daily=irr"
                .to_string(),
            &daily,
            message("PVSystem.p1", "EffCurve=eff", out),
        ),
        (
            "new xycurve.eff npts=4 xarray=[0 0.1 0.5 1] yarray=[0.9 0.9 0.9 (0 0 /)]",
            "new xycurve.eff npts=4 xarray=[0 0.1 0.5 1] yarray=[0.9 0.9 0.9 0.9]",
            st.to_string(),
            &daily,
            message("Storage.s1", "EffCurve=eff", out),
        ),
        (
            "new xycurve.rc npts=3 xarray=[60 180 300] yarray=[1 (0 0 /) 1]",
            "new xycurve.rc npts=3 xarray=[60 180 300] yarray=[1 1 1]",
            "new reactor.r1 bus1=b phases=3 r=1 x=10 rcurve=rc".to_string(),
            &snapshot,
            message("Reactor.r1", "RCurve=rc", adm),
        ),
        (
            "new xycurve.lc npts=3 xarray=[60 180 300] yarray=[1 (0 0 /) 1]",
            "new xycurve.lc npts=3 xarray=[60 180 300] yarray=[1 1 1]",
            "new reactor.r1 bus1=b phases=3 r=1 x=10 lcurve=lc".to_string(),
            &snapshot,
            message("Reactor.r1", "LCurve=lc", adm),
        ),
        (
            "new xycurve.pl npts=4 xarray=[0 10 20 25] yarray=[1 1 1 (0 0 /)]",
            "new xycurve.pl npts=4 xarray=[0 10 20 25] yarray=[1 1 1 1]",
            "new windgen.w1 bus1=b kv=12.47 kw=100 pf=0.9 ploss=pl".to_string(),
            &snapshot,
            message("WindGen.w1", "PLoss=pl", adm),
        ),
        (
            "new xycurve.vv npts=4 xarray=[0.5 0.9 1.1 1.5] yarray=[1 1 -1 (0 0 /)]",
            vv_valid,
            "new windgen.w1 bus1=b kv=12.47 kw=100 pf=0.9 qmode=2 vv_curve=vv".to_string(),
            &snapshot,
            message("WindGen.w1", "VV_Curve=vv", adm),
        ),
        // A volt-var curve outside the volt-var `QMode`, and a loss curve in
        // a UPFC mode that reads no losses.
        (
            "new xycurve.vv npts=4 xarray=[0.5 0.9 1.1 1.5] yarray=[1 1 -1 (0 0 /)]",
            vv_valid,
            "new windgen.w1 bus1=b kv=12.47 kw=100 pf=0.9 vv_curve=vv".to_string(),
            &snapshot,
            message("WindGen.w1", "VV_Curve=vv", adm),
        ),
        (
            lc_broken,
            lc_valid,
            upfc(1),
            &snapshot,
            message("UPFC.u1", "LossCurve=lc", "input current"),
        ),
        (
            lc_broken,
            lc_valid,
            upfc(2),
            &snapshot,
            message("UPFC.u1", "LossCurve=lc", "input current"),
        ),
    ];
    let setup = ["set voltagebases=[12.47]", "calcv"];
    for (curve, valid, element, solve, message) in &legs {
        let mut dss = curve_circuit(&format!("{curve}\n{element}"));
        assert_eq!(
            dss.error_texts(),
            [message.as_str()],
            "{curve} {element}: the definition"
        );
        dss.errors.clear();
        for c in setup {
            dss.command(c);
        }
        assert_eq!(
            dss.error_texts(),
            [message.as_str(), REFUSED_CALCV],
            "{curve} {element}: calcv"
        );
        dss.errors.clear();
        for c in *solve {
            dss.command(c);
        }
        assert_eq!(
            dss.error_texts(),
            [message.as_str(), REFUSED_SOLVE],
            "{curve} {element}: the solve"
        );
        dss.errors.clear();
        assert!(!dss.circuit().unwrap().is_solved, "{element}: solved");
        assert!(finite(&mut dss), "{element}: a voltage that is not finite");

        let mut twin = curve_circuit(&format!("{valid}\n{element}"));
        for c in setup.iter().chain(solve.iter()) {
            twin.command(c);
        }
        assert!(
            twin.errors().is_empty(),
            "{valid} {element}: the twin {:?}",
            twin.error_texts()
        );
        assert!(
            twin.circuit().unwrap().is_solved,
            "{element}: the twin solves"
        );
        assert!(finite(&mut twin), "{element}: the twin");
    }

    // A zero `PF` beside a broken curve: the curve is named, and the zero
    // `PF` beside the valid one.
    for (curve, valid, element, full, input) in [
        (
            "new xycurve.pt npts=3 xarray=[0 25 40] yarray=[1 1 (0 0 /)]",
            pt_valid,
            pv,
            "PVSystem.p1",
            "P-TCurve=pt",
        ),
        (
            "new xycurve.eff npts=4 xarray=[0 0.1 0.5 1] yarray=[0.9 0.9 0.9 (0 0 /)]",
            "new xycurve.eff npts=4 xarray=[0 0.1 0.5 1] yarray=[0.9 0.9 0.9 0.9]",
            st,
            "Storage.s1",
            "EffCurve=eff",
        ),
    ] {
        let dss = curve_circuit(&format!("{curve}\n{element} pf=0"));
        assert_eq!(
            dss.error_texts(),
            [message(full, input, out)],
            "{full}: the curve"
        );
        let twin = curve_circuit(&format!("{valid}\n{element} pf=0"));
        assert_eq!(
            twin.error_texts(),
            [message(full, "PF=0", out)],
            "{full}: the PF"
        );
    }

    // A VCCS reads its curves only in the waveform model of a dynamics solve,
    // so the snapshot before it, and a dynamics solve in RMS mode, read none.
    let deck = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/asymmetric/vccs/vccs_asym.dss");
    assert!(deck.is_file(), "vendored corpus deck missing: {deck:?}");
    let dynamics = [
        "set mode=dynamic",
        "set stepsize=0.002",
        "set number=5",
        "solve",
    ];
    let vccs = |edit: &str| {
        let mut dss = Dss::new();
        dss.command(&format!("compile \"{}\"", deck.display()));
        dss.command(
            "new xycurve.bad npts=3 xarray=[-0.820 0.0 0.820] yarray=[-0.788 (0 0 /) 0.788]",
        );
        dss.command(&format!("edit vccs.pv1 {edit}"));
        dss.command("set mode=snap");
        dss.command("solve");
        assert!(dss.errors().is_empty(), "{edit}: {:?}", dss.error_texts());
        assert!(
            dss.circuit().unwrap().is_solved,
            "{edit}: the snapshot solves"
        );
        for c in dynamics {
            dss.command(c);
        }
        dss
    };
    for (edit, input) in [
        ("bp1=bad", "BP1=bad"),
        ("bp2=bad", "BP2=bad"),
        ("filter=bad", "Filter=bad"),
    ] {
        let mut dss = vccs(edit);
        let message = message("VCCS.pv1", input, "dynamic current");
        assert_eq!(
            dss.error_texts(),
            [message.as_str(), REFUSED_SOLVE],
            "{edit}: the dynamics solve"
        );
        assert!(
            !dss.circuit().unwrap().is_solved,
            "{edit}: the dynamics solve solves nothing"
        );
        assert!(finite(&mut dss), "{edit}: the refused dynamics solve");
    }
    for edit in ["bp1=bp1_1phase", "bp1=bad rmsmode=yes"] {
        let mut twin = vccs(edit);
        assert!(twin.errors().is_empty(), "{edit}: {:?}", twin.error_texts());
        assert!(twin.circuit().unwrap().is_solved, "{edit}: solved");
        assert!(finite(&mut twin), "{edit}");
    }
}

/// The message that refuses the element `full` for its `input`.
fn message(full: &str, input: &str, quantity: &str) -> String {
    format!("{full}: {input} makes its {quantity} not a finite number, so it is not built.")
}

/// The refusal circuit with the lines of `text` added, one command each.
fn curve_circuit(text: &str) -> Dss {
    let mut dss = Dss::new();
    for c in [
        "new circuit.c basekv=12.47 bus1=src pu=1.0",
        "new line.ln bus1=src bus2=b length=1 units=kft",
        "new load.ld bus1=b kv=12.47 kw=500 kvar=100",
    ] {
        dss.command(c);
    }
    for c in text.lines() {
        dss.command(c.trim());
    }
    dss
}

/// A built element edited to a curve with a point that is not a number is
/// refused at that edit and at every solve, Save writes a circuit that
/// compiles back to the refusal, and the valid curve typed next builds the
/// element as the twin that holds it from the start. `Set StateVar` that
/// switches on the mode reading a broken curve leaves the solve refused.
#[test]
fn a_curve_edited_broken_refuses_its_element_until_a_valid_one_is_typed() {
    let setup = ["set voltagebases=[12.47]", "calcv", "solve"];
    // (the valid and the broken curve, the element, the edits to each, the
    // message)
    let legs: [(&str, &str, &str, &str, &str, String); 5] = [
        (
            "new xycurve.ok npts=3 xarray=[0 25 40] yarray=[1 1 0.9]",
            "new xycurve.bad npts=3 xarray=[0 25 40] yarray=[1 1 (0 0 /)]",
            "new pvsystem.p1 bus1=b phases=3 kv=12.47 pmpp=100 kva=120 irradiance=1 p-tcurve=ok",
            "edit pvsystem.p1 p-tcurve=bad",
            "edit pvsystem.p1 p-tcurve=ok",
            message("PVSystem.p1", "P-TCurve=bad", "inverter output"),
        ),
        (
            "new xycurve.ok npts=2 xarray=[0 1] yarray=[0.9 0.9]",
            "new xycurve.bad npts=2 xarray=[0 1] yarray=[0.9 (0 0 /)]",
            "new storage.s1 bus1=b phases=3 kv=12.47 kwrated=100 kwhrated=400 kva=120 \
             %stored=50 state=discharging %discharge=50 effcurve=ok",
            "edit storage.s1 effcurve=bad",
            "edit storage.s1 effcurve=ok",
            message("Storage.s1", "EffCurve=bad", "inverter output"),
        ),
        (
            "new xycurve.ok npts=3 xarray=[60 180 300] yarray=[1 1 1]",
            "new xycurve.bad npts=3 xarray=[60 180 300] yarray=[1 (0 0 /) 1]",
            "new reactor.r1 bus1=b phases=3 r=1 x=10 rcurve=ok",
            "edit reactor.r1 rcurve=bad",
            "edit reactor.r1 rcurve=ok",
            message("Reactor.r1", "RCurve=bad", "admittance"),
        ),
        (
            "new xycurve.ok npts=4 xarray=[0 10 20 25] yarray=[1 1 1 1]",
            "new xycurve.bad npts=4 xarray=[0 10 20 25] yarray=[1 1 1 (0 0 /)]",
            "new windgen.w1 bus1=b kv=12.47 kw=100 pf=0.9 ploss=ok",
            "edit windgen.w1 ploss=bad",
            "edit windgen.w1 ploss=ok",
            message("WindGen.w1", "PLoss=bad", "admittance"),
        ),
        (
            "new xycurve.ok npts=3 xarray=[0.9 1 1.1] yarray=[1.0143 1.008 1.0143]",
            "new xycurve.bad npts=3 xarray=[0.9 1 1.1] yarray=[1.0143 (0 0 /) 1.0143]",
            "new upfc.u1 bus1=b bus2=c refkv=12.47 mode=1 losscurve=ok\n\
             new load.lc bus1=c kv=12.47 kw=100 kvar=10",
            "edit upfc.u1 losscurve=bad",
            "edit upfc.u1 losscurve=ok",
            message("UPFC.u1", "LossCurve=bad", "input current"),
        ),
    ];
    for (ok, bad, element, broken, valid, message) in &legs {
        let mut dss = curve_circuit(&format!("{ok}\n{bad}\n{element}"));
        for c in setup {
            dss.command(c);
        }
        assert!(
            dss.errors().is_empty(),
            "{element}: {:?}",
            dss.error_texts()
        );
        let solved = node_voltages(&mut dss);
        dss.command(broken);
        assert_eq!(dss.error_texts(), [message.as_str()], "{broken}");
        dss.errors.clear();
        for _ in 0..2 {
            dss.command("solve");
            assert_eq!(
                dss.error_texts(),
                [message.as_str(), REFUSED_SOLVE],
                "{broken}: the solve"
            );
            dss.errors.clear();
            assert!(!dss.circuit().unwrap().is_solved, "{broken}: solved");
            assert_eq!(
                node_voltages(&mut dss),
                solved,
                "{broken}: the voltages kept"
            );
        }
        let mut back = save_and_compile(&mut dss, "curve_edit");
        assert!(
            back.error_texts().contains(&message.clone()),
            "{broken}: the reload {:?}",
            back.error_texts()
        );
        back.errors.clear();
        back.command("solve");
        assert_eq!(
            back.error_texts(),
            [message.as_str(), REFUSED_SOLVE],
            "{broken}: the reloaded solve"
        );
        dss.command(valid);
        assert!(dss.errors().is_empty(), "{valid}: {:?}", dss.error_texts());
        dss.command("solve");
        assert!(dss.errors().is_empty(), "{valid}: {:?}", dss.error_texts());
        assert!(dss.circuit().unwrap().is_solved, "{valid}: solved");
        let mut twin = curve_circuit(&format!("{ok}\n{bad}\n{element}"));
        for c in setup {
            twin.command(c);
        }
        twin.command("solve");
        assert_eq!(
            node_voltages(&mut dss),
            node_voltages(&mut twin),
            "{valid}: the twin"
        );
    }

    // `Set StateVar` writes the mode with no recalc: the curve was refused at
    // the definition already, and the twin with a valid curve solves.
    for (curve, valid, element, switch, message) in [
        (
            "new xycurve.vv npts=3 xarray=[0.5 1 1.5] yarray=[1 (0 0 /) 1]",
            "new xycurve.vv npts=3 xarray=[0.5 1 1.5] yarray=[1 0 -1]",
            "new windgen.w1 bus1=b kv=12.47 kw=100 pf=0.9 qmode=0 vv_curve=vv",
            "set statevar=x windgen.w1 QMode 2",
            message("WindGen.w1", "VV_Curve=vv", "admittance"),
        ),
        (
            "new xycurve.lc npts=3 xarray=[0.9 1 1.1] yarray=[1.0143 (0 0 /) 1.0143]",
            "new xycurve.lc npts=3 xarray=[0.9 1 1.1] yarray=[1.0143 1.008 1.0143]",
            "new upfc.u1 bus1=b bus2=c refkv=12.47 mode=2 losscurve=lc\n\
             new load.lc bus1=c kv=12.47 kw=100 kvar=10",
            "set statevar=x upfc.u1 ModeUPFC 1",
            message("UPFC.u1", "LossCurve=lc", "input current"),
        ),
    ] {
        let mut dss = curve_circuit(&format!("{curve}\n{element}"));
        assert_eq!(dss.error_texts(), [message.as_str()], "{element}");
        dss.errors.clear();
        dss.command(switch);
        assert!(dss.errors().is_empty(), "{switch}: {:?}", dss.error_texts());
        dss.command("set voltagebases=[12.47]");
        dss.command("calcv");
        dss.errors.clear();
        dss.command("solve");
        assert_eq!(
            dss.error_texts(),
            [message.as_str(), REFUSED_SOLVE],
            "{switch}: the solve"
        );
        assert!(!dss.circuit().unwrap().is_solved, "{switch}: solved");
        assert!(
            node_voltages(&mut dss)
                .iter()
                .all(|v| v.re.is_finite() && v.im.is_finite()),
            "{switch}: a voltage that is not finite"
        );
        let mut twin = curve_circuit(&format!("{valid}\n{element}"));
        for c in [
            "set voltagebases=[12.47]",
            "calcv",
            "solve",
            switch,
            "solve",
        ] {
            twin.command(c);
        }
        assert!(
            twin.errors().is_empty(),
            "{switch}: {:?}",
            twin.error_texts()
        );
        assert!(
            twin.circuit().unwrap().is_solved,
            "{switch}: the twin solves"
        );
        assert!(
            node_voltages(&mut twin)
                .iter()
                .all(|v| v.re.is_finite() && v.im.is_finite()),
            "{switch}: the twin"
        );
    }
}

/// A typed impedance matrix whose rows are dependent within the rounding of
/// its entries has no admittance, nor a series impedance whose resistance is
/// so at dc: a Reactor's series `RMatrix + j XMatrix` and each of its parallel
/// matrices are refused at the recalc, and a Line's or LineCode's series
/// impedance and a Reactor's at another frequency at every Y build, each with
/// the whole message until a valid input is typed. A refused solve solves
/// nothing and keeps the voltages it found, while the matrices held a little
/// away from singular, or with a resistance, build and solve.
#[test]
fn a_typed_impedance_matrix_singular_within_its_rounding_is_refused() {
    let rank_one = "[0.01 |0.07 0.49 |0.03 0.21 0.09]";
    let held = "[0.010000001 |0.07 0.490000001 |0.03 0.21 0.090000001]";
    let text = "(0.01 |0.07 0.49 |0.03 0.21 0.09 )";
    let diagonal = "[10 |0 10 |0 0 10]";
    let finite =
        |v: &[num_complex::Complex64]| v.iter().all(|v| v.re.is_finite() && v.im.is_finite());
    let reactor = |rm: &str, xm: &str, parallel: bool| {
        let form = if parallel { "parallel=yes " } else { "" };
        format!("new reactor.m1 bus1=b phases=3 {form}rmatrix={rm} xmatrix={xm}")
    };
    for (singular, valid, input) in [
        (
            reactor(rank_one, rank_one, false),
            reactor(held, held, false),
            format!("RMatrix={text} XMatrix={text}"),
        ),
        (
            reactor(rank_one, diagonal, true),
            reactor(held, diagonal, true),
            format!("RMatrix={text}"),
        ),
        (
            reactor(diagonal, rank_one, true),
            reactor(diagonal, held, true),
            format!("XMatrix={text}"),
        ),
    ] {
        let message = message("Reactor.m1", &input, "admittance");
        let mut dss = refusal_circuit(&singular);
        assert_eq!(
            dss.error_texts(),
            [message.as_str()],
            "{singular}: the edit"
        );
        dss.errors.clear();
        let voltages = solved_voltages(&mut dss);
        assert_eq!(
            dss.error_texts(),
            [
                message.as_str(),
                REFUSED_CALCV,
                message.as_str(),
                REFUSED_SOLVE
            ],
            "{singular}: calcv and the solve"
        );
        assert!(!dss.circuit().unwrap().is_solved, "{singular}: solved");
        assert!(
            finite(&voltages),
            "{singular}: a voltage that is not finite"
        );

        let mut twin = refusal_circuit(&valid);
        let voltages = solved_voltages(&mut twin);
        assert!(
            twin.errors().is_empty(),
            "{valid}: {:?}",
            twin.error_texts()
        );
        assert!(twin.circuit().unwrap().is_solved, "{valid}: solved");
        assert!(finite(&voltages), "{valid}");
    }

    // A solved circuit whose Line is edited to a singular matrix, typed or
    // fetched from a LineCode: every solve is refused until a valid edit.
    let line_message = "Matrix Inversion Error for Line \"l2\". Invalid impedance specified. \
                        Aborting solution.";
    let load = "new load.lc bus1=c kv=12.47 kw=100 kvar=10";
    let line = "new line.l2 bus1=b bus2=c phases=3 length=1 units=kft";
    let matrices = |m: &str| format!("rmatrix={m} xmatrix={m}");
    let codes = format!(
        "new linecode.ok nphases=3 units=kft {}\n\
         new linecode.rank_one nphases=3 units=kft {}\n\
         new linecode.exact nphases=3 units=kft {}",
        matrices(held),
        matrices(rank_one),
        matrices("[1 |2 4 |3 6 9]")
    );
    for (defined, broken, valid) in [
        (
            format!("{line} {}", matrices(held)),
            format!("edit line.l2 {}", matrices(rank_one)),
            format!("edit line.l2 {}", matrices(held)),
        ),
        (
            format!("{line} {}", matrices(held)),
            format!("edit line.l2 {}", matrices("[1 |2 4 |3 6 9]")),
            format!("edit line.l2 {}", matrices(held)),
        ),
        (
            format!("{codes}\n{line} linecode=ok"),
            "edit line.l2 linecode=rank_one".to_string(),
            "edit line.l2 linecode=ok".to_string(),
        ),
        (
            format!("{codes}\n{line} linecode=ok"),
            "edit line.l2 linecode=exact".to_string(),
            "edit line.l2 linecode=ok".to_string(),
        ),
    ] {
        let mut dss = refusal_circuit(load);
        for c in defined.lines() {
            dss.command(c.trim());
        }
        let solved = solved_voltages(&mut dss);
        assert!(
            dss.errors().is_empty(),
            "{defined}: {:?}",
            dss.error_texts()
        );
        assert!(dss.circuit().unwrap().is_solved, "{defined}: solved");
        dss.command(&broken);
        assert!(dss.errors().is_empty(), "{broken}: {:?}", dss.error_texts());
        for _ in 0..2 {
            dss.command("solve");
            assert_eq!(
                dss.error_texts(),
                [line_message, REFUSED_SOLVE],
                "{broken}: the solve"
            );
            dss.errors.clear();
            assert!(!dss.circuit().unwrap().is_solved, "{broken}: solved");
            assert_eq!(
                node_voltages(&mut dss),
                solved,
                "{broken}: the voltages kept"
            );
        }
        dss.command(&valid);
        dss.command("solve");
        assert!(dss.errors().is_empty(), "{valid}: {:?}", dss.error_texts());
        assert!(dss.circuit().unwrap().is_solved, "{valid}: solved");
        let mut twin = refusal_circuit(load);
        for c in defined.lines() {
            twin.command(c.trim());
        }
        solved_voltages(&mut twin);
        twin.command("solve");
        assert_eq!(
            node_voltages(&mut dss),
            node_voltages(&mut twin),
            "{valid}: the twin"
        );
    }

    // The dc solve of a series impedance with no resistance, a Line or a
    // lossless Reactor matrix or sequence form: a short the stamp cannot
    // carry, while the same element with a resistance solves there.
    let reactor_dc = |input: &str| message("Reactor.m1", input, "admittance");
    let dc_legs: [(String, Option<String>); 7] = [
        (
            format!(
                "{load}\nnew line.l2 bus1=b bus2=c phases=3 length=1 units=none r1=0 x1=0.1 r0=0 x0=0.3 c1=0 c0=0"
            ),
            Some(line_message.to_string()),
        ),
        (
            format!(
                "{load}\nnew line.l2 bus1=b bus2=c phases=3 length=1 units=none r1=0.01 x1=0.1 r0=0.01 x0=0.3 c1=0 c0=0"
            ),
            None,
        ),
        (
            "new reactor.m1 bus1=b phases=3 rmatrix=[0 |0 0 |0 0 0] xmatrix=[10 |0 20 |0 0 30]"
                .into(),
            Some(reactor_dc(
                "RMatrix=(0 |0 0 |0 0 0 ) XMatrix=(10 |0 20 |0 0 30 )",
            )),
        ),
        (
            "new reactor.m1 bus1=b phases=3 rmatrix=[1 |0 1 |0 0 1] xmatrix=[10 |0 20 |0 0 30]"
                .into(),
            None,
        ),
        (
            format!("new reactor.m1 bus1=b phases=3 rmatrix={rank_one} xmatrix={diagonal}"),
            Some(reactor_dc(&format!(
                "RMatrix={text} XMatrix=(10 |0 10 |0 0 10 )"
            ))),
        ),
        (
            "new reactor.m1 bus1=b phases=3 z1=[0 9] z0=[0 12]".into(),
            Some(reactor_dc("Z1=[0, 9] Z2=[0, 9] Z0=[0, 12]")),
        ),
        (
            "new reactor.m1 bus1=b phases=3 z1=[1 9] z0=[1 12]".into(),
            None,
        ),
    ];
    for (element, refused) in &dc_legs {
        let mut dss = curve_circuit(element);
        let solved = solved_voltages(&mut dss);
        assert!(
            dss.errors().is_empty(),
            "{element}: {:?}",
            dss.error_texts()
        );
        dss.command("set frequency=0.1");
        for _ in 0..2 {
            dss.command("solve");
            match refused {
                Some(msg) => {
                    assert_eq!(
                        dss.error_texts(),
                        [msg.as_str(), REFUSED_SOLVE],
                        "{element}"
                    );
                    dss.errors.clear();
                    assert!(!dss.circuit().unwrap().is_solved, "{element}: solved");
                    assert_eq!(
                        node_voltages(&mut dss),
                        solved,
                        "{element}: the voltages kept"
                    );
                }
                None => {
                    assert!(
                        dss.errors().is_empty(),
                        "{element}: {:?}",
                        dss.error_texts()
                    );
                    assert!(dss.circuit().unwrap().is_solved, "{element}: solved");
                    assert!(finite(&node_voltages(&mut dss)), "{element}");
                }
            }
        }
    }
}

/// A matrix that spans many decades but whose rows are independent builds,
/// solving as the one-phase elements of its diagonal do in the Reactor's series
/// and parallel forms and as a Line, and so does a reactance with an inductive
/// and a capacitive mode beside a tiny resistance. The series test reads `R +
/// jX`, so a lossless series matrix builds as its lossless one-phase twin.
#[test]
fn a_matrix_spanning_many_decades_builds_as_its_one_phase_twin() {
    let magnitudes = |dss: &mut Dss| -> Vec<String> {
        node_voltages(dss)
            .iter()
            .map(|v| format!("{:.6}", v.norm()))
            .collect()
    };
    let one_phase = |kind: &str, phases: &[(&str, &str)], x: &str| -> String {
        phases
            .iter()
            .enumerate()
            .map(|(k, (node, r))| match kind {
                "series" => format!("new reactor.d{k} bus1=b.{node} phases=1 r={r} x=0"),
                "parallel" => format!(
                    "new reactor.d{k} bus1=b.{node} phases=1 parallel=yes rmatrix=[{r}] \
                     xmatrix=[{x}]"
                ),
                _ => format!(
                    "new line.l{k} bus1=b.{node} bus2=c.{node} phases=1 length=1 units=none \
                     rmatrix=[{r}] xmatrix=[0] cmatrix=[0]"
                ),
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let two = [("1", "1e-8"), ("2", "1e8")];
    let three = |a: &'static str, c: &'static str| [("1", a), ("2", "1"), ("3", c)];
    // (the matrix form, its one-phase twin, the twin's voltages past `src`)
    let legs: Vec<(String, String, Vec<&str>)> = vec![
        (
            "new reactor.d bus1=b.1.2 phases=2 rmatrix=[1e-8 |0 1e8] xmatrix=[0 |0 0]".into(),
            one_phase("series", &two, ""),
            vec!["0.000232", "8528.202395", "8644.780155"],
        ),
        (
            "new reactor.d bus1=b.1.2 phases=2 parallel=yes rmatrix=[1e-8 |0 1e8] \
             xmatrix=[10 |0 10]"
                .into(),
            one_phase("parallel", &two, "10"),
            vec!["0.000235", "8313.391739", "8625.959215"],
        ),
        (
            "new line.l bus1=b.1.2 bus2=c.1.2 phases=2 length=1 units=none \
             rmatrix=[1e-8 |0 1e8] xmatrix=[0 |0 0] cmatrix=[0 |0 0]\n\
             new load.lc bus1=c.1.2 phases=2 kv=12.47 kw=100 kvar=10"
                .into(),
            format!(
                "{}\nnew load.lc bus1=c.1.2 phases=2 kv=12.47 kw=100 kvar=10",
                one_phase("line", &two, "")
            ),
            vec![
                "7195.850512",
                "7197.565087",
                "7196.523313",
                "7195.850512",
                "0.074244",
            ],
        ),
        (
            "new reactor.d bus1=b phases=3 rmatrix=[1e-4 |0 1 |0 0 1e12] \
             xmatrix=[0 |0 0 |0 0 0]"
                .into(),
            one_phase("series", &three("1e-4", "1e12"), ""),
            vec!["2.299367", "7523.942316", "9072.016938"],
        ),
        (
            "new reactor.d bus1=b phases=3 parallel=yes rmatrix=[1e-4 |0 1 |0 0 1e12] \
             xmatrix=[10 |0 10 |0 0 10]"
                .into(),
            one_phase("parallel", &three("1e-4", "1e12"), "10"),
            vec!["2.324288", "7399.313147", "8816.862413"],
        ),
        (
            "new line.l bus1=b bus2=c phases=3 length=1 units=none \
             rmatrix=[1e-4 |0 1 |0 0 1e12] xmatrix=[0 |0 0 |0 0 0] cmatrix=[0 |0 0 |0 0 0]\n\
             new load.lc bus1=c phases=3 kv=12.47 kw=100 kvar=10"
                .into(),
            format!(
                "{}\nnew load.lc bus1=c phases=3 kv=12.47 kw=100 kvar=10",
                one_phase("line", &three("1e-4", "1e12"), "")
            ),
            vec![
                "7195.959053",
                "7196.653483",
                "7197.102430",
                "7195.958590",
                "7192.018701",
                "0.000011",
            ],
        ),
        (
            "new reactor.d bus1=b phases=3 rmatrix=[1e-9 |0 1 |0 0 1e9] xmatrix=[0 |0 0 |0 0 0]"
                .into(),
            one_phase("series", &three("1e-9", "1e9"), ""),
            vec!["0.000023", "7523.721499", "9072.695078"],
        ),
        (
            "new reactor.d bus1=b phases=3 parallel=yes rmatrix=[1e-9 |0 1 |0 0 1e9] \
             xmatrix=[10 |0 10 |0 0 10]"
                .into(),
            one_phase("parallel", &three("1e-9", "1e9"), "10"),
            vec!["0.000023", "7399.085694", "8817.527171"],
        ),
        (
            "new line.l bus1=b bus2=c phases=3 length=1 units=none \
             rmatrix=[1e-9 |0 1 |0 0 1e9] xmatrix=[0 |0 0 |0 0 0] cmatrix=[0 |0 0 |0 0 0]\n\
             new load.lc bus1=c phases=3 kv=12.47 kw=100 kvar=10"
                .into(),
            format!(
                "{}\nnew load.lc bus1=c phases=3 kv=12.47 kw=100 kvar=10",
                one_phase("line", &three("1e-9", "1e9"), "")
            ),
            vec![
                "7195.959104",
                "7196.653442",
                "7197.102454",
                "7195.959104",
                "7192.018659",
                "0.011136",
            ],
        ),
    ];
    for (matrix, twin, expected) in &legs {
        let mut volts = Vec::new();
        for deck in [matrix, twin] {
            let mut dss = curve_circuit(deck);
            dss.command("set voltagebases=[12.47]");
            dss.command("calcv");
            dss.command("solve");
            assert!(dss.errors().is_empty(), "{deck}: {:?}", dss.error_texts());
            assert!(dss.circuit().unwrap().is_solved, "{deck}: solved");
            volts.push(magnitudes(&mut dss));
        }

        assert_eq!(volts[0][3..], expected[..], "{matrix}");
        assert_eq!(volts[1], volts[0], "{matrix}: the twin");
    }

    // A lossless matrix: `R` alone is singular, `R + jX` is not.
    for (matrix, twin) in [
        (
            "new reactor.m1 bus1=b phases=3 rmatrix=[0 |0 0 |0 0 0] xmatrix=[10 |0 20 |0 0 30]",
            "new reactor.x1 bus1=b.1 phases=1 r=0 x=10\n\
             new reactor.x2 bus1=b.2 phases=1 r=0 x=20\n\
             new reactor.x3 bus1=b.3 phases=1 r=0 x=30",
        ),
        (
            "new line.l2 bus1=b bus2=c phases=3 length=1 units=none rmatrix=[0 |0 0 |0 0 0] \
             xmatrix=[10 |0 20 |0 0 30] cmatrix=[0 |0 0 |0 0 0]\n\
             new load.lc bus1=c kv=12.47 kw=100 kvar=10",
            "new line.x1 bus1=b.1 bus2=c.1 phases=1 length=1 units=none rmatrix=[0] \
             xmatrix=[10] cmatrix=[0]\n\
             new line.x2 bus1=b.2 bus2=c.2 phases=1 length=1 units=none rmatrix=[0] \
             xmatrix=[20] cmatrix=[0]\n\
             new line.x3 bus1=b.3 bus2=c.3 phases=1 length=1 units=none rmatrix=[0] \
             xmatrix=[30] cmatrix=[0]\n\
             new load.lc bus1=c kv=12.47 kw=100 kvar=10",
        ),
    ] {
        let mut volts = Vec::new();
        for deck in [matrix, twin] {
            let mut dss = curve_circuit(deck);
            dss.command("set voltagebases=[12.47]");
            dss.command("calcv");
            dss.command("solve");
            assert!(dss.errors().is_empty(), "{deck}: {:?}", dss.error_texts());
            assert!(dss.circuit().unwrap().is_solved, "{deck}: solved");
            volts.push(magnitudes(&mut dss));
        }
        assert_eq!(volts[0], volts[1], "{matrix}: the twin");
    }

    // A reactance with an inductive and a capacitive mode beside a tiny
    // resistance solves as the same matrix with a larger one.
    let modes = |r: &str| {
        format!(
            "new reactor.a1 bus1=b bus2=c phases=3 rmatrix=[{r} |0 {r} |0 0 1] \
             xmatrix=[0 |1 0 |0 0 1]\n\
             new load.lc bus1=c kv=12.47 kw=100 kvar=10"
        )
    };
    let mut volts = Vec::new();
    for r in ["1e-20", "1e-16", "1e-12"] {
        let mut dss = curve_circuit(&modes(r));
        for c in ["set voltagebases=[12.47]", "calcv", "solve"] {
            dss.command(c);
        }
        assert!(dss.errors().is_empty(), "{r}: {:?}", dss.error_texts());
        assert!(dss.circuit().unwrap().is_solved, "{r}: solved");
        volts.push(magnitudes(&mut dss));
    }
    assert_eq!(volts[0][6..], ["7192.645282", "7200.668335", "7191.320841"]);
    assert_eq!(volts[0], volts[2], "the larger resistance");
    assert_eq!(volts[1], volts[2], "the larger resistance");
}

/// A transformer or auto whose short-circuit impedance is singular, as three
/// windings whose leakage reactances leave each winding no impedance to the
/// other two shorted, is refused at its recalc, at every Y build and in a
/// `like=` copy with the whole message, and so is one whose inverse is not
/// finite at the solve frequency. The valid reactance typed next builds it as
/// the twin that has it from the start.
#[test]
fn a_singular_short_circuit_impedance_refuses_its_transformer() {
    for (element, input, valid, twin, setup) in [
        (
            "new transformer.t1 phases=3 windings=3 buses=[b t u] conns=[wye wye wye] \
             kvs=[12.47 0.48 0.48] kvas=[500 500 500] xhl=6 xht=6 xlt=24 %rs=[0 0 0]\n\
             new load.lt bus1=t kv=0.48 kw=10 kvar=1\n\
             new load.lu bus1=u kv=0.48 kw=10 kvar=1",
            "Transformer.t1: XHL=6 XHT=6 XLT=24",
            "edit transformer.t1 xlt=20",
            "new transformer.t1 phases=3 windings=3 buses=[b t u] conns=[wye wye wye] \
             kvs=[12.47 0.48 0.48] kvas=[500 500 500] xhl=6 xht=6 xlt=20 %rs=[0 0 0]\n\
             new load.lt bus1=t kv=0.48 kw=10 kvar=1\n\
             new load.lu bus1=u kv=0.48 kw=10 kvar=1",
            "set voltagebases=[12.47 0.48]",
        ),
        (
            "new autotrans.a1 phases=3 windings=3 buses=[b c d] conns=[s w d] \
             kvs=[8.32 4.16 0.48] kvas=[2000 2000 500] xhx=25 xht=100 xxt=225 %rs=[0 0 0]\n\
             new load.lc bus1=c kv=4.16 kw=100 kvar=10",
            "AutoTrans.a1: XHX=25 XHT=100 XXT=225",
            "edit autotrans.a1 xxt=200",
            "new autotrans.a1 phases=3 windings=3 buses=[b c d] conns=[s w d] \
             kvs=[8.32 4.16 0.48] kvas=[2000 2000 500] xhx=25 xht=100 xxt=200 %rs=[0 0 0]\n\
             new load.lc bus1=c kv=4.16 kw=100 kvar=10",
            "set voltagebases=[12.47 4.16 0.48]",
        ),
    ] {
        let message =
            format!("{input} makes its admittance not a finite number, so it is not built.");
        let mut dss = curve_circuit(element);
        assert_eq!(dss.error_texts(), [message.as_str()], "{input}: the edit");
        dss.errors.clear();
        dss.command(setup);
        dss.command("calcv");
        assert_eq!(
            dss.error_texts(),
            [message.as_str(), REFUSED_CALCV],
            "{input}: calcv"
        );
        dss.errors.clear();
        for _ in 0..2 {
            dss.command("solve");
            assert_eq!(
                dss.error_texts(),
                [message.as_str(), REFUSED_SOLVE],
                "{input}: the solve"
            );
            dss.errors.clear();
            assert!(!dss.circuit().unwrap().is_solved, "{input}: solved");
            assert!(
                node_voltages(&mut dss)
                    .iter()
                    .all(|v| v.re.is_finite() && v.im.is_finite()),
                "{input}: a voltage that is not finite"
            );
        }
        dss.command(valid);
        assert!(dss.errors().is_empty(), "{valid}: {:?}", dss.error_texts());
        dss.command("calcv");
        dss.command("solve");
        assert!(dss.errors().is_empty(), "{valid}: {:?}", dss.error_texts());
        assert!(dss.circuit().unwrap().is_solved, "{valid}: solved");
        let mut twin = curve_circuit(twin);
        twin.command(setup);
        twin.command("calcv");
        twin.command("solve");
        assert!(
            twin.errors().is_empty(),
            "{valid}: the twin {:?}",
            twin.error_texts()
        );
        assert_eq!(
            node_voltages(&mut dss),
            node_voltages(&mut twin),
            "{valid}: the twin"
        );
    }
    // A `like=` copy of the refused unit, typed so or edited so from a built
    // one, is refused under its own name and reads the reactances it copied.
    let tx = |xlt: &str| {
        format!(
            "new transformer.t1 phases=3 windings=3 buses=[b t u] conns=[wye wye wye] \
             kvs=[12.47 0.48 0.48] kvas=[500 500 500] xhl=6 xht=6 xlt={xlt} %rs=[0 0 0]"
        )
    };
    let auto = |xxt: &str| {
        format!(
            "new autotrans.a1 phases=3 windings=3 buses=[b c d] conns=[s w d] \
             kvs=[8.32 4.16 0.48] kvas=[2000 2000 500] xhx=25 xht=100 xxt={xxt} %rs=[0 0 0]"
        )
    };
    let tx_loads = "disable transformer.t1\n\
                    new load.l2 bus1=t2 kv=0.48 kw=10 kvar=1\n\
                    new load.u2 bus1=u2 kv=0.48 kw=10 kvar=1";
    let auto_loads = "disable autotrans.a1\nnew load.l2 bus1=c2 kv=4.16 kw=100 kvar=10";
    let tx_copy = (
        "new transformer.t2 like=t1 buses=[b t2 u2]",
        "XHL=6 XHT=6 XLT=24",
        ("transformer.t2.xlt", "24"),
        tx_loads,
        "set voltagebases=[12.47 0.48]",
    );
    let auto_copy = (
        "new autotrans.a2 like=a1 buses=[b c2 d2]",
        "XHX=25 XHT=100 XXT=225",
        ("autotrans.a2.xxt", "225"),
        auto_loads,
        "set voltagebases=[12.47 4.16 0.48]",
    );
    for (defined, edit, (copy, input, read, loads, setup), (donor, copied)) in [
        (
            tx("24"),
            None,
            tx_copy,
            ("Transformer.t1", "Transformer.t2"),
        ),
        (
            tx("20"),
            Some("edit transformer.t1 xlt=24"),
            tx_copy,
            ("Transformer.t1", "Transformer.t2"),
        ),
        (
            auto("225"),
            None,
            auto_copy,
            ("AutoTrans.a1", "AutoTrans.a2"),
        ),
        (
            auto("200"),
            Some("edit autotrans.a1 xxt=225"),
            auto_copy,
            ("AutoTrans.a1", "AutoTrans.a2"),
        ),
    ] {
        let (donor, copied) = (
            message(donor, input, "admittance"),
            message(copied, input, "admittance"),
        );
        let mut dss = curve_circuit(&defined);
        if let Some(edit) = edit {
            assert!(
                dss.errors().is_empty(),
                "{defined}: {:?}",
                dss.error_texts()
            );
            dss.command(edit);
        }
        assert_eq!(dss.error_texts(), [donor.as_str()], "{copy}: the source");
        dss.errors.clear();
        dss.command(copy);
        assert_eq!(dss.error_texts(), [copied.as_str()], "{copy}");
        assert_eq!(query(&mut dss, read.0), read.1, "{copy}: {}", read.0);
        for c in loads.lines() {
            dss.command(c.trim());
        }
        dss.errors.clear();
        dss.command(setup);
        dss.command("calcv");
        assert_eq!(
            dss.error_texts(),
            [copied.as_str(), REFUSED_CALCV],
            "{copy}: calcv"
        );
        dss.errors.clear();
        dss.command("solve");
        assert_eq!(
            dss.error_texts(),
            [copied.as_str(), REFUSED_SOLVE],
            "{copy}: the solve"
        );
        assert!(!dss.circuit().unwrap().is_solved, "{copy}: solved");
    }
    // An impedance whose inverse is finite at the base frequency but not at a
    // lower one is refused at that frequency, the voltages kept.
    let mut dss = curve_circuit(
        "new transformer.t1 phases=3 windings=2 buses=[b t] conns=[wye wye] \
         kvs=[12.47 0.48] kvas=[500 500] xhl=5e-155 %rs=[0 0]\n\
         new load.lt bus1=t kv=0.48 kw=10 kvar=1",
    );
    for c in [
        "set voltagebases=[12.47 0.48]",
        "calcv",
        "solve",
        "set frequency=30",
    ] {
        dss.command(c);
    }
    dss.errors.clear();
    let kept = node_voltages(&mut dss);
    let message = message("Transformer.t1", "XHL=5E-155", "admittance");
    for _ in 0..2 {
        dss.command("solve");
        assert_eq!(
            dss.error_texts(),
            [message.as_str(), REFUSED_SOLVE],
            "30 Hz"
        );
        dss.errors.clear();
        assert!(!dss.circuit().unwrap().is_solved, "30 Hz: solved");
        assert_eq!(node_voltages(&mut dss), kept, "30 Hz: the voltages kept");
    }
}

/// A Capacitor whose series filter has no resistance and resonates at the
/// solve frequency shorts its bus, so it is refused at every Y build with the
/// whole message naming the filter. A resistance typed next builds it as the
/// twin defined with it.
#[test]
fn a_series_filter_at_its_resonance_refuses_its_capacitor() {
    let bank = "new capacitor.c1 bus1=b phases=3 cuf=[10] xl=[265.25823848649225]";
    let message = message(
        "Capacitor.c1",
        "R=[ 0] XL=[ 265.258238486492]",
        "admittance",
    );
    let mut dss = curve_circuit(&format!("{bank} r=[0]"));
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    for c in ["set voltagebases=[12.47]", "calcv", "solve"] {
        dss.command(c);
    }
    assert_eq!(
        dss.error_texts(),
        [
            message.as_str(),
            REFUSED_CALCV,
            message.as_str(),
            REFUSED_SOLVE
        ]
    );
    dss.errors.clear();
    assert!(!dss.circuit().unwrap().is_solved, "solved");
    dss.command("edit capacitor.c1 r=[0.1]");
    dss.command("calcv");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    assert!(dss.circuit().unwrap().is_solved, "the resistance: solved");
    let mut twin = curve_circuit(&format!("{bank} r=[0.1]"));
    for c in ["set voltagebases=[12.47]", "calcv", "solve"] {
        twin.command(c);
    }
    assert_eq!(
        magnitudes_past_source(&mut dss),
        magnitudes_past_source(&mut twin),
        "the twin"
    );
}

/// The magnitudes of the node voltages past `src`, to six decimals.
fn magnitudes_past_source(dss: &mut Dss) -> Vec<String> {
    node_voltages(dss)[3..]
        .iter()
        .map(|v| format!("{:.6}", v.norm()))
        .collect()
}

/// A Line whose series impedance inverts to an admittance that is not a
/// finite number is refused at every solve with the whole message naming its
/// impedance inputs and length, defined so or edited so after a solve, in the
/// matrix and sequence forms, through a LineCode, by its length and in the dc
/// stamp alone. The valid impedance typed next solves as the twin defined
/// with it.
#[test]
fn a_line_whose_admittance_overflows_is_refused_at_every_solve() {
    let setup = ["set voltagebases=[12.47]", "calcv", "solve"];
    let two = "new load.lc bus1=c.1.2 phases=2 kv=12.47 kw=100 kvar=10
               new linecode.ok nphases=2 units=none rmatrix=[1 |0 1] xmatrix=[1 |0 1]                cmatrix=[0 |0 0]
               new linecode.tiny nphases=2 units=none rmatrix=[1e-310 |0 1] xmatrix=[0 |0 0]                cmatrix=[0 |0 0]
               new line.l2 bus1=b.1.2 bus2=c.1.2 phases=2 length=1 units=none";
    let one = "new load.lc bus1=c.1 phases=1 kv=7.2 kw=100 kvar=10
               new line.l2 bus1=b.1 bus2=c.1 phases=1 length=1 units=none";
    let matrix2 = "rmatrix=[1 |0 1] xmatrix=[1 |0 1] cmatrix=[0 |0 0]";
    let tiny = "9.99999999999997E-311";
    let twin2 = vec![
        "7195.501115",
        "7196.543655",
        "7197.219086",
        "7187.846586",
        "7188.890238",
    ];
    let twin1 = vec!["7194.819104", "7198.263296", "7196.177414", "7179.486754"];
    // (the circuit, the edit that breaks the Line, the input the message
    // names, the valid edit, the twin's voltages past `src`)
    let legs: [(String, &str, String, &str, Vec<&str>); 6] = [
        (
            format!("{two} {matrix2}"),
            "edit line.l2 rmatrix=[1e-310 |0 1] xmatrix=[0 |0 0]",
            format!("RMatrix=[{tiny} |0 1 ] XMatrix=[0 |0 0 ] Length=1"),
            "edit line.l2 rmatrix=[1 |0 1] xmatrix=[1 |0 1]",
            twin2.clone(),
        ),
        (
            format!("{two} {matrix2}"),
            "edit line.l2 rmatrix=[1e-300 |0 1e10] xmatrix=[0 |0 0]",
            "RMatrix=[1E-300 |0 10000000000 ] XMatrix=[0 |0 0 ] Length=1".into(),
            "edit line.l2 rmatrix=[1 |0 1] xmatrix=[1 |0 1]",
            twin2.clone(),
        ),
        (
            format!("{two} {matrix2}"),
            "edit line.l2 rmatrix=[1e-300 |0 1] xmatrix=[1e-300 |0 1] length=1e-12",
            "RMatrix=[1E-300 |0 1 ] XMatrix=[1E-300 |0 1 ] Length=1E-12".into(),
            "edit line.l2 rmatrix=[1 |0 1] xmatrix=[1 |0 1] length=1",
            twin2.clone(),
        ),
        (
            format!("{two} linecode=ok"),
            "edit line.l2 linecode=tiny",
            "LineCode=tiny Length=1".into(),
            "edit line.l2 linecode=ok",
            twin2,
        ),
        (
            format!("{one} rmatrix=[1] xmatrix=[1] cmatrix=[0]"),
            "edit line.l2 rmatrix=[1e-310] xmatrix=[0]",
            format!("RMatrix=[{tiny} ] XMatrix=[0 ] Length=1"),
            "edit line.l2 rmatrix=[1] xmatrix=[1]",
            twin1.clone(),
        ),
        (
            format!("{one} r1=1 x1=1 c1=0 c0=0"),
            "edit line.l2 r1=1e-310 x1=0",
            format!("R1={tiny} X1=0 Length=1"),
            "edit line.l2 r1=1 x1=1",
            twin1,
        ),
    ];
    for (circuit, broken, input, valid, expected) in &legs {
        let message = message("Line.l2", input, "admittance");
        // Defined broken: refused at calcv and at the solve.
        let mut dss = curve_circuit(&format!(
            "{circuit}
{broken}"
        ));
        assert!(dss.errors().is_empty(), "{broken}: {:?}", dss.error_texts());
        for c in setup {
            dss.command(c);
        }
        assert_eq!(
            dss.error_texts(),
            [
                message.as_str(),
                REFUSED_CALCV,
                message.as_str(),
                REFUSED_SOLVE
            ],
            "{broken}: defined so"
        );
        assert!(!dss.circuit().unwrap().is_solved, "{broken}: solved");
        // Edited so after a solve: every solve refused, the voltages kept.
        let mut dss = curve_circuit(circuit);
        for c in setup {
            dss.command(c);
        }
        assert!(
            dss.errors().is_empty(),
            "{circuit}: {:?}",
            dss.error_texts()
        );
        let solved = node_voltages(&mut dss);
        dss.command(broken);
        assert!(dss.errors().is_empty(), "{broken}: {:?}", dss.error_texts());
        for _ in 0..2 {
            dss.command("solve");
            assert_eq!(
                dss.error_texts(),
                [message.as_str(), REFUSED_SOLVE],
                "{broken}: the solve"
            );
            dss.errors.clear();
            assert!(!dss.circuit().unwrap().is_solved, "{broken}: solved");
            assert_eq!(
                node_voltages(&mut dss),
                solved,
                "{broken}: the voltages kept"
            );
        }
        dss.command(valid);
        dss.command("solve");
        assert!(dss.errors().is_empty(), "{valid}: {:?}", dss.error_texts());
        assert!(dss.circuit().unwrap().is_solved, "{valid}: solved");
        let mut twin = curve_circuit(circuit);
        for c in setup {
            twin.command(c);
        }
        twin.command("solve");
        assert_eq!(
            magnitudes_past_source(&mut dss),
            magnitudes_past_source(&mut twin),
            "{valid}: the twin"
        );
        assert_eq!(
            magnitudes_past_source(&mut twin),
            expected[..],
            "{valid}: the twin's voltages"
        );
    }
    // A resistance whose dc stamp overflows while the impedance inverts: the
    // dc solve is refused, the voltages kept.
    let mut dss = curve_circuit(&format!("{one} r1=1e-310 x1=1 c1=0 c0=0 rg=0 xg=0"));
    for c in setup {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    assert!(dss.circuit().unwrap().is_solved, "60 Hz: solved");
    let kept = node_voltages(&mut dss);
    dss.command("set frequency=0.1");
    let message = message("Line.l2", &format!("R1={tiny} X1=1 Length=1"), "admittance");
    for _ in 0..2 {
        dss.command("solve");
        assert_eq!(dss.error_texts(), [message.as_str(), REFUSED_SOLVE], "dc");
        dss.errors.clear();
        assert!(!dss.circuit().unwrap().is_solved, "dc: solved");
        assert_eq!(node_voltages(&mut dss), kept, "dc: the voltages kept");
    }
}

/// A Reactor in the R + jX form whose `LCurve` or `RCurve` zeroes its
/// impedance at the solve frequency has no admittance there, so every solve at
/// that frequency is refused with the whole message naming its resistance,
/// reactance and curve, the voltages kept. The solve back at the base
/// frequency builds it again as before.
#[test]
fn a_reactor_curve_that_zeroes_its_impedance_refuses_it_at_that_frequency() {
    for (curve, reactor, frequency, input, expected) in [
        (
            "new xycurve.lc npts=3 xarray=[0 60 120] yarray=[1 1 0]",
            "new reactor.r1 bus1=b bus2=c phases=3 r=0 x=10 lcurve=lc",
            "set frequency=120",
            "R=0 X=10 LCurve=lc",
            ["7196.416644", "7191.632362"],
        ),
        (
            "new xycurve.rc npts=2 xarray=[0 60] yarray=[0 1]",
            "new reactor.r1 bus1=b bus2=c phases=3 r=1 x=0 rcurve=rc",
            "set frequency=0.1",
            "R=1 X=0 RCurve=rc",
            ["7196.422324", "7191.787392"],
        ),
    ] {
        let message = message("Reactor.r1", input, "admittance");
        let mut dss = curve_circuit(&format!(
            "{curve}\n{reactor}\nnew load.lc bus1=c kv=12.47 kw=100 kvar=10"
        ));
        for c in ["set voltagebases=[12.47]", "calcv", "solve"] {
            dss.command(c);
        }
        assert!(dss.errors().is_empty(), "{input}: {:?}", dss.error_texts());
        assert!(dss.circuit().unwrap().is_solved, "{input}: solved");
        let base = magnitudes_past_source(&mut dss);
        let expected: Vec<&str> = [expected[0]; 3]
            .into_iter()
            .chain([expected[1]; 3])
            .collect();
        assert_eq!(base, expected, "{input}: the base frequency");
        let solved = node_voltages(&mut dss);
        dss.command(frequency);
        for _ in 0..2 {
            dss.command("solve");
            assert_eq!(
                dss.error_texts(),
                [message.as_str(), REFUSED_SOLVE],
                "{input}: {frequency}"
            );
            dss.errors.clear();
            assert!(!dss.circuit().unwrap().is_solved, "{input}: solved");
            assert_eq!(
                node_voltages(&mut dss),
                solved,
                "{input}: the voltages kept"
            );
        }
        dss.command("set frequency=60");
        dss.command("solve");
        assert!(dss.errors().is_empty(), "{input}: {:?}", dss.error_texts());
        assert!(dss.circuit().unwrap().is_solved, "{input}: solved again");
        assert_eq!(
            magnitudes_past_source(&mut dss),
            base,
            "{input}: back at base"
        );
    }
}

/// A Transformer or AutoTrans winding with no resistance is a short at dc, so
/// every dc solve is refused with the whole message naming its `%R`, the
/// voltages kept, also after an edit of its reactances in that study, while a
/// winding with a resistance solves there. The solve back at the base
/// frequency builds it again, or names the reactances an edit at dc made
/// singular.
#[test]
fn a_winding_with_no_resistance_refuses_its_transformer_at_dc() {
    let three = "new transformer.t1 phases=3 windings=3 buses=[b t u] conns=[wye wye wye] \
                 kvs=[12.47 0.48 0.48] kvas=[500 500 500] xhl=6 xht=6 xlt=20 %rs=[0 0 0]\n\
                 new load.lt bus1=t kv=0.48 kw=10 kvar=1\n\
                 new load.lu bus1=u kv=0.48 kw=10 kvar=1";
    let two = |rs: &str| {
        format!(
            "new transformer.t1 phases=3 windings=2 buses=[b t] conns=[wye wye] \
             kvs=[12.47 0.48] kvas=[500 500] xhl=6 %rs={rs}\n\
             new load.lt bus1=t kv=0.48 kw=10 kvar=1"
        )
    };
    let auto = "new autotrans.a1 phases=3 windings=2 buses=[b c] conns=[series wye] \
                kvs=[12.47 4.16] kvas=[2000 2000] xhx=10 %rs=[0 0]\n\
                new load.lc bus1=c kv=4.16 kw=100 kvar=10";
    let tx = "set voltagebases=[12.47 0.48]";
    for (element, setup, refused) in [
        (three.to_string(), tx, Some("Transformer.t1: %R=0")),
        (two("[0 0]"), tx, Some("Transformer.t1: %R=0")),
        (
            auto.to_string(),
            "set voltagebases=[12.47 4.16]",
            Some("AutoTrans.a1: %R=0"),
        ),
        (two("[0.5 0.5]"), tx, None),
    ] {
        let mut dss = curve_circuit(&element);
        for c in [setup, "calcv", "solve"] {
            dss.command(c);
        }
        assert!(
            dss.errors().is_empty(),
            "{element}: {:?}",
            dss.error_texts()
        );
        let base = node_voltages(&mut dss);
        let magnitudes = magnitudes_past_source(&mut dss);
        dss.command("set frequency=0.1");
        let Some(input) = refused else {
            dss.command("solve");
            assert!(
                dss.errors().is_empty(),
                "{element}: {:?}",
                dss.error_texts()
            );
            assert!(dss.circuit().unwrap().is_solved, "{element}: solved at dc");
            continue;
        };
        let message =
            format!("{input} makes its admittance not a finite number, so it is not built.");
        for _ in 0..2 {
            dss.command("solve");
            assert_eq!(
                dss.error_texts(),
                [message.as_str(), REFUSED_SOLVE],
                "{input}"
            );
            dss.errors.clear();
            assert!(!dss.circuit().unwrap().is_solved, "{input}: solved");
            assert_eq!(node_voltages(&mut dss), base, "{input}: the voltages kept");
        }
        dss.command("set frequency=60");
        dss.command("solve");
        assert!(dss.errors().is_empty(), "{input}: {:?}", dss.error_texts());
        assert!(dss.circuit().unwrap().is_solved, "{input}: solved again");
        assert_eq!(
            magnitudes_past_source(&mut dss),
            magnitudes,
            "{input}: back at base"
        );
    }
    // An edit at dc that makes the reactances singular: the dc solve names
    // the `%R`, the solve back at the base frequency the reactances.
    let mut dss = curve_circuit(three);
    for c in [
        tx,
        "calcv",
        "solve",
        "set frequency=0.1",
        "edit transformer.t1 xlt=24",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    for (frequency, input) in [
        ("set frequency=0.1", "%R=0"),
        ("set frequency=60", "XHL=6 XHT=6 XLT=24"),
    ] {
        dss.command(frequency);
        dss.command("solve");
        let message = message("Transformer.t1", input, "admittance");
        assert_eq!(
            dss.error_texts(),
            [message.as_str(), REFUSED_SOLVE],
            "{frequency}"
        );
        dss.errors.clear();
        assert!(!dss.circuit().unwrap().is_solved, "{frequency}: solved");
    }
}

/// The DC power that updates a Storage's state of charge solves its efficiency
/// curve on the segment that holds the solution, discharging and charging, a
/// curve of one point being its constant. A curve that cannot carry the AC
/// power at any DC power refuses the element at the time step that reads it,
/// naming the curve, its state of charge keeps what it held, every later solve
/// refuses it and a refusal before the last hour leaves the run neither solved
/// nor converged.
#[test]
fn the_dc_power_of_an_efficiency_curve_is_solved_on_the_segment_that_holds_it() {
    let storage = |mult: &str, curve: &str, kwh: &str| {
        let mut dss = curve_circuit(&format!(
            "new loadshape.disp npts=2 interval=1 mult=[0 {mult}]\n\
             {curve}\n\
             new storage.s1 bus1=b phases=3 kv=12.47 kwrated=100 kwhrated={kwh} kva=120 \
             %stored=50 dispmode=follow daily=disp effcurve=eff"
        ));
        for c in [
            "set voltagebases=[12.47]",
            "calcv",
            "set mode=daily number=2 stepsize=1h",
            "solve",
        ] {
            dss.command(c);
        }
        dss
    };
    let kwh = |dss: &mut Dss| -> f64 {
        query(dss, "storage.s1.kwhstored")
            .parse()
            .expect("a number")
    };
    // The segment of the AC power and the one past it each send the solution
    // to the other, the steep one between them holds it.
    let cycling = "new xycurve.eff npts=4 xarray=[0 0.9 0.91 1.2] yarray=[0.89 0.9 0.96 0.97]";
    let flat = "new xycurve.eff npts=4 xarray=[0 0.9 0.91 1.2] yarray=[0.9 0.9 0.9 0.9]";
    for curve in [cycling, flat] {
        let mut dss = storage("1", curve, "400");
        assert!(dss.errors().is_empty(), "{curve}: {:?}", dss.error_texts());
        assert!(dss.circuit().unwrap().is_solved, "{curve}: solved");
        let b: Vec<String> = node_voltages(&mut dss)[3..]
            .iter()
            .map(|v| format!("{:.6}", v.norm()))
            .collect();
        assert_eq!(
            b, ["7197.226872"; 3],
            "{curve}: the voltage of the flat twin"
        );
    }
    // One discharging hour moves the stored energy by the DC power and the
    // idling losses over the discharge efficiency, and the flat twin, whose
    // DC power is the AC power over its efficiency, gives the AC power.
    let dc_of = |dss: &mut Dss| {
        let idle: f64 = query(dss, "storage.s1.%idlingkw").parse().unwrap();
        let eff: f64 = query(dss, "storage.s1.%effdischarge").parse().unwrap();
        (2000.0 - kwh(dss)) * 0.01 * eff - idle
    };
    let mut dss = storage("1", cycling, "4000");
    let mut twin = storage("1", flat, "4000");
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    assert!(twin.errors().is_empty(), "{:?}", twin.error_texts());
    let (dc, ac) = (dc_of(&mut dss), 0.9 * dc_of(&mut twin));
    let x = dc / 120.0;
    assert!((0.9..=0.91).contains(&x), "the steep segment: {x}");
    let eta = 0.9 + (0.96 - 0.9) * (x - 0.9) / 0.01;
    assert!(
        (eta * dc - ac).abs() <= 1e-9 * ac,
        "the AC power: {} for {ac}",
        eta * dc
    );
    // On a falling last segment the smaller root of its quadratic is the
    // one on the segment.
    let falling = "new xycurve.eff npts=3 xarray=[0 0.5 1] yarray=[0.96 0.96 0.86]";
    let level = "new xycurve.eff npts=3 xarray=[0 0.5 1] yarray=[0.96 0.96 0.96]";
    let mut dss = storage("1", falling, "4000");
    let mut twin = storage("1", level, "4000");
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    let (dc, ac) = (dc_of(&mut dss), 0.96 * dc_of(&mut twin));
    let x = dc / 120.0;
    assert!((0.5..=1.0).contains(&x), "the falling segment: {x}");
    let eta = 0.96 - 0.2 * (x - 0.5);
    assert!(
        (eta * dc - ac).abs() <= 1e-9 * ac,
        "the AC power: {} for {ac}",
        eta * dc
    );
    // Charging stores the efficiency of the DC power times the AC power: the
    // AC power reads the falling segment, the DC power the flat one.
    let fall = "new xycurve.eff npts=3 xarray=[0 0.8 1.2] yarray=[0.9 0.9 0.5]";
    let level = "new xycurve.eff npts=3 xarray=[0 0.8 1.2] yarray=[0.9 0.9 0.9]";
    let mut dss = storage("-1", fall, "4000");
    let mut twin = storage("-1", level, "4000");
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    assert!(twin.errors().is_empty(), "{:?}", twin.error_texts());
    assert!(kwh(&mut dss) > 2000.0, "charged");
    assert_eq!(kwh(&mut dss), kwh(&mut twin), "the flat twin");
    // On a sloped segment the DC power the hour stores holds `P = η(P / kVA) p`
    // at the AC power its flat twin reads, also on the third segment of a
    // curve that starts past zero.
    let charged_dc = |dss: &mut Dss| {
        let idle: f64 = query(dss, "storage.s1.%idlingkw").parse().unwrap();
        let eff: f64 = query(dss, "storage.s1.%effcharge").parse().unwrap();
        (kwh(dss) - 2000.0) / (0.01 * eff) + idle
    };
    for (npts, xs, ys, (lo, hi), (a, b)) in [
        (3, "[0 0.5 1.2]", "[0.8 0.9 0.97]", (0.5, 1.2), (0.1, 0.85)),
        (
            4,
            "[.1 .2 .4 1]",
            "[.86 .9 .93 .97]",
            (0.4, 1.0),
            (0.04 / 0.6, 0.93 - 0.04 / 1.5),
        ),
    ] {
        let sloped = format!("new xycurve.eff npts={npts} xarray={xs} yarray={ys}");
        let flat = "new xycurve.eff npts=2 xarray=[0 1] yarray=[0.9 0.9]";
        let mut dss = storage("-1", &sloped, "4000");
        let mut twin = storage("-1", flat, "4000");
        assert!(dss.errors().is_empty(), "{xs}: {:?}", dss.error_texts());
        let ac = charged_dc(&mut twin) / 0.9;
        let dc = charged_dc(&mut dss);
        assert!(
            (lo..=hi).contains(&(dc / 120.0)),
            "{xs}: the sloped segment {dc}"
        );
        let eta = a * dc / 120.0 + b;
        assert!(
            (dc - eta * ac).abs() <= 1e-9 * dc,
            "{xs}: {dc} for {}",
            eta * ac
        );
    }
    // A curve of one point is its efficiency at every power, as its flat twin.
    for mult in ["1", "-1"] {
        let mut one = storage(
            mult,
            "new xycurve.eff npts=1 xarray=[0.5] yarray=[0.9]",
            "4000",
        );
        let mut flat = storage(
            mult,
            "new xycurve.eff npts=2 xarray=[0 1] yarray=[0.9 0.9]",
            "4000",
        );
        assert!(one.errors().is_empty(), "{mult}: {:?}", one.error_texts());
        assert_eq!(kwh(&mut one), kwh(&mut flat), "{mult}: the flat twin");
    }
    let mut one = storage(
        "1",
        "new xycurve.eff npts=1 xarray=[0.5] yarray=[0.9]",
        "4000",
    );
    assert_eq!(kwh(&mut one), 1875.43820115188);
    // No DC power gives the AC power: the curve is refused when the hour
    // reads it, and the next solve with it, a snapshot too.
    let low = "new xycurve.eff npts=2 xarray=[0 1] yarray=[0.5 0.1]";
    let message = message("Storage.s1", "EffCurve=eff", "dc power");
    let mut dss = storage("1", low, "4000");
    assert_eq!(
        dss.error_texts(),
        [message.as_str()],
        "the hour that reads it"
    );
    dss.errors.clear();
    assert_eq!(kwh(&mut dss), 2000.0, "the stored energy is kept");
    dss.command("solve");
    assert_eq!(
        dss.error_texts(),
        [message.as_str(), REFUSED_SOLVE],
        "the next solve"
    );
    assert!(!dss.circuit().unwrap().is_solved);
    dss.errors.clear();
    dss.command("set mode=snapshot");
    dss.command("solve");
    assert_eq!(
        dss.error_texts(),
        [message.as_str(), REFUSED_SOLVE],
        "the snapshot after it"
    );
    assert!(!dss.circuit().unwrap().is_solved, "the snapshot: solved");
    // Refused at the first of two hours, the run is neither solved nor
    // converged.
    let mut dss = curve_circuit(&format!(
        "new loadshape.disp npts=2 interval=1 mult=[1 1]\n\
         {low}\n\
         new storage.s1 bus1=b phases=3 kv=12.47 kwrated=100 kwhrated=4000 kva=120 \
         %stored=50 dispmode=follow daily=disp effcurve=eff"
    ));
    for c in [
        "set voltagebases=[12.47]",
        "calcv",
        "set mode=daily number=2 stepsize=1h",
        "solve",
    ] {
        dss.command(c);
    }
    assert_eq!(
        dss.error_texts(),
        [message.as_str(), message.as_str(), REFUSED_SOLVE],
        "the hour that reads it, then the next hour"
    );
    let ckt = dss.circuit().unwrap();
    assert!(!ckt.is_solved, "the run: solved");
    assert!(!ckt.solution.converged_flag, "the run: converged");
    assert_eq!(kwh(&mut dss), 2000.0, "the stored energy is kept");
    // A snapshot reads no DC power, so a Monitor of the variables changes
    // nothing: the circuit solves alike with and without it.
    let mut volts = Vec::new();
    for monitor in ["", "new monitor.ms element=storage.s1 mode=3"] {
        let mut dss = curve_circuit(&format!(
            "{low}\n\
             new storage.s1 bus1=b phases=3 kv=12.47 kwrated=100 kwhrated=4000 kva=120 \
             %stored=50 state=discharging effcurve=eff\n\
             {monitor}"
        ));
        for c in [
            "set voltagebases=[12.47]",
            "calcv",
            "solve",
            "sample",
            "solve",
            "solve",
        ] {
            dss.command(c);
        }
        assert!(
            dss.errors().is_empty(),
            "{monitor}: {:?}",
            dss.error_texts()
        );
        assert!(dss.circuit().unwrap().is_solved, "{monitor}: solved");
        volts.push(node_voltages(&mut dss));
    }
    assert_eq!(volts[0], volts[1], "with and without the Monitor");
}

/// An AutoTrans has two windings at least, as a Transformer has, so a single
/// winding is refused at its token and the auto keeps its two windings, solving
/// with the second on its default bus.
#[test]
fn an_autotrans_of_one_winding_is_refused_at_its_token() {
    let mut dss = refusal_circuit("new autotrans.a4 windings=1 buses=[b]");
    assert_eq!(
        dss.error_texts(),
        ["AutoTrans.a4.Windings: Value (1) must be greater than one."]
    );
    assert_eq!(query(&mut dss, "autotrans.a4.windings"), "2");
    assert_eq!(query(&mut dss, "autotrans.a4.buses"), "[b, a4_2, ]");
    let voltages = solved_voltages(&mut dss);
    assert_eq!(dss.error_texts().len(), 1, "{:?}", dss.error_texts());
    assert!(dss.circuit().unwrap().is_solved);
    let magnitudes: Vec<String> = voltages
        .iter()
        .map(|v| format!("{:.2}", v.norm()))
        .collect();
    assert_eq!(
        magnitudes,
        [
            "7198.77", "7198.77", "7198.77", "7196.87", "7196.87", "7196.87", "780.39", "780.39",
            "780.39"
        ]
    );
}

/// The node voltages of the bus `name`.
fn bus_nodes(dss: &mut Dss, name: &str) -> Vec<num_complex::Complex64> {
    dss.all_bus_voltages()
        .into_iter()
        .find(|b| b.name == name)
        .unwrap_or_else(|| panic!("no bus {name}"))
        .node_v
}

/// A `like=` copy of a Transformer or AutoTrans refused for its reactances
/// that types an `XSCArray` in its own command, and the refused unit itself
/// given an `XSCArray`, build and solve as the unit written out with that
/// array. The saved circuit of the copy rebuilds the same solve.
#[test]
fn a_typed_xscarray_replaces_refused_reactances() {
    let tx_head = "phases=3 windings=3 conns=[wye wye wye] kvs=[12.47 0.48 0.48] \
                   kvas=[500 500 500] %rs=[0 0 0]";
    let auto_head = "phases=3 windings=3 conns=[s w d] kvs=[8.32 4.16 0.48] \
                     kvas=[2000 2000 500] %rs=[0 0 0]";
    for (
        defined,
        edit,
        refused,
        (copy, copied_buses, twin, twin_buses, written),
        (unit_array, unit_buses),
        loads,
        setup,
        expected,
    ) in [
        (
            format!("new transformer.t1 {tx_head} buses=[b t u] xhl=6 xht=6 xlt=20"),
            "edit transformer.t1 xlt=24",
            message("Transformer.t1", "XHL=6 XHT=6 XLT=24", "admittance"),
            (
                "new transformer.t2 like=t1 buses=[b t2 u2] xscarray=[6 6 20]\n\
                 disable transformer.t1",
                ["t2", "u2"],
                format!("new transformer.t2 {tx_head} buses=[b t2 u2] xscarray=[6 6 20]"),
                ["t2", "u2"],
                "transformer.t2",
            ),
            ("edit transformer.t1 xscarray=[6 6 20]", ["t", "u"]),
            "new load.l2 bus1={0} kv=0.48 kw=10 kvar=1\n\
             new load.l3 bus1={1} kv=0.48 kw=10 kvar=1",
            "set voltagebases=[12.47 0.48]",
            277.010108,
        ),
        (
            format!("new autotrans.a1 {auto_head} buses=[b c d] xhx=25 xht=100 xxt=200"),
            "edit autotrans.a1 xxt=225",
            message("AutoTrans.a1", "XHX=25 XHT=100 XXT=225", "admittance"),
            (
                "new autotrans.a2 like=a1 buses=[b c2 d2] xscarray=[25 100 200]\n\
                 disable autotrans.a1",
                ["c2", "d2"],
                format!("new autotrans.a2 {auto_head} buses=[b c2 d2] xscarray=[25 100 200]"),
                ["c2", "d2"],
                "autotrans.a2",
            ),
            ("edit autotrans.a1 xscarray=[25 100 200]", ["c", "d"]),
            "new load.l2 bus1={0} kv=4.16 kw=100 kvar=10\n\
             new load.l3 bus1={1} kv=0.48 kw=10 kvar=1",
            "set voltagebases=[12.47 4.16 0.48]",
            3594.162653,
        ),
    ] {
        let loads_at = |buses: [&str; 2]| loads.replace("{0}", buses[0]).replace("{1}", buses[1]);
        let solve = |dss: &mut Dss, what: &str| {
            dss.command(setup);
            dss.command("calcv");
            dss.command("solve");
            assert!(dss.errors().is_empty(), "{what}: {:?}", dss.error_texts());
            assert!(dss.circuit().unwrap().is_solved, "{what}: solved");
        };
        // The circuit written out with the array.
        let mut twin = curve_circuit(&format!("{twin}\n{}", loads_at(twin_buses)));
        solve(&mut twin, written);
        let expected_nodes: Vec<_> = twin_buses.iter().map(|b| bus_nodes(&mut twin, b)).collect();
        assert!(
            (expected_nodes[0][0].norm() - expected).abs() < 1e-6,
            "{written}: {}",
            expected_nodes[0][0].norm()
        );

        // The copy typing the array in its own command.
        let mut dss = curve_circuit(&defined);
        dss.command(edit);
        assert_eq!(dss.error_texts(), [refused.as_str()], "{copy}: the source");
        dss.errors.clear();
        let (copy_line, disable) = copy.split_once('\n').unwrap();
        dss.command(copy_line);
        assert!(
            dss.errors().is_empty(),
            "{copy_line}: {:?}",
            dss.error_texts()
        );
        // The disabled source logs its refusal again.
        dss.command(disable);
        dss.errors.clear();
        for c in loads_at(copied_buses).lines() {
            dss.command(c.trim());
        }
        solve(&mut dss, copy);
        for (b, nodes) in copied_buses.iter().zip(&expected_nodes) {
            assert_eq!(&bus_nodes(&mut dss, b), nodes, "{copy}: {b}");
        }
        // Save writes the copy with its array, so the saved circuit solves
        // the same.
        let mut back = save_and_compile(&mut dss, &format!("xsc_{}", copied_buses[0]));
        back.errors.clear();
        back.command("solve");
        assert!(
            back.errors().is_empty(),
            "{copy}: reload {:?}",
            back.error_texts()
        );
        for (b, nodes) in copied_buses.iter().zip(&expected_nodes) {
            let reloaded = bus_nodes(&mut back, b);
            assert!(
                reloaded.len() == nodes.len()
                    && reloaded
                        .iter()
                        .zip(nodes)
                        .all(|(r, e)| (r.norm() - e.norm()).abs() < 1e-6),
                "{copy}: reload {b}: {reloaded:?} against {nodes:?}"
            );
        }

        // The refused unit itself given the array.
        let mut dss = curve_circuit(&defined);
        dss.command(edit);
        assert_eq!(
            dss.error_texts(),
            [refused.as_str()],
            "{unit_array}: before"
        );
        dss.errors.clear();
        dss.command(unit_array);
        for c in loads_at(unit_buses).lines() {
            dss.command(c.trim());
        }
        solve(&mut dss, unit_array);
        for (b, nodes) in unit_buses.iter().zip(&expected_nodes) {
            assert_eq!(&bus_nodes(&mut dss, b), nodes, "{unit_array}: {b}");
        }
    }
}

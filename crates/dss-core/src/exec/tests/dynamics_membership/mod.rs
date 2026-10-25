//! Which elements take part in a dynamics run.
//!
//! A disabled element is out of the model: it is not integrated, not asked for
//! a refusal and not stamped, so the network solves exactly as if it were
//! absent. Every reader sees zero for its dynamic quantities (currents, powers,
//! state variables) and keeps its settings, the values its input data fixes.
//!
//! Common deck: a 0.69 kV source, `Line.l1` and a 300 kW + 100 kvar load at
//! `wbus`, solved, then `Set mode=dynamic stepsize=0.001`. Every command goes
//! through [`Dss::command`] one line at a time. Each expected value comes from
//! a rule evaluated here. The literals marked as cross-checks were measured on
//! 2026-10-17 on the r4133 DLL through `epri-worker` and on the pinned
//! dss-python with the same command lists.
//!
//! Both oracles integrate a disabled element from states nobody set (measured
//! as above, five 1 ms steps, `|V(WBUS.1)|` 395.3590962 V with nothing added,
//! which every disabled case leaves where they solve). A disabled Generator and
//! a disabled IndMach012 read `Frequency`, `Theta`, `dSpeed` and `dTheta` NaN
//! (capi) or non-finite (r4133), and the IndMach012 a 212.377 A `Ir1` and a
//! 304.665 hp shaft power. A Generator disabled after three steps, the Load at
//! `model=2`, keeps accelerating (60.0125, 60.0375 and 60.0625 Hz at steps 4 to 6
//! on r4133) while the node voltage equals the circuit without it (395.4045536 V
//! on all three).
//! A disabled PVSystem reads its eight InvControl operation values as 9999 and
//! `Max. Amps (phase)` infinite or non-finite, a disabled Storage reads `Series
//! L` and `Max. Amps (phase)` 0. A disabled VCCS with its curves reads NaN
//! (capi) or fails the `Solve` with `#335` (r4133), one without curves and a
//! disabled Generator linked to a `DynamicExp` with no memory fail it with 482
//! `Access violation` on both, and a disabled WindGen host of a `DynamicExp`
//! with variables fails it on r4133. A disabled VSConverter reads 4.2e244 and
//! NaN (capi) or `#327` (r4133). The engine reads zero for all of these, the
//! InvControl operation values included, since they are outputs of a controller
//! acting on a connected unit, and computes `Series L` and `Max. Amps (phase)`
//! from the rated data. The settings a class keeps are those its
//! `CktElement::setting_variables` writes, decided by the coordinator under the
//! user's delegation of 2026-10-10.

use num_complex::Complex64;

use super::common::{query, query_f64};
use crate::exec::Dss;

mod flat_start;
mod joining;
mod rebuild;

const H: [&str; 4] = [
    "Clear",
    "New Circuit.dp basekv=0.69 phases=3 bus1=srcbus",
    "New Line.l1 bus1=srcbus bus2=wbus phases=3 r1=0.005 x1=0.02 length=1",
    "New Load.ld bus1=wbus phases=3 kv=0.69 kW=300 kvar=100",
];
/// The same load declared constant impedance (`model=2`): with no machine in the
/// run the dynamics network is linear, so every step is one solve of the same Y
/// and the same source injection, whatever voltage the iteration starts from.
const H_MODEL2: [&str; 4] = [
    "Clear",
    "New Circuit.dp basekv=0.69 phases=3 bus1=srcbus",
    "New Line.l1 bus1=srcbus bus2=wbus phases=3 r1=0.005 x1=0.02 length=1",
    "New Load.ld bus1=wbus phases=3 kv=0.69 kW=300 kvar=100 model=2",
];
/// The common deck with the zero-sequence impedance of `Line.l1` equal to its
/// positive-sequence one. Its default zero-sequence data is so much weaker that
/// the dynamics step's network iteration diverges with the GFL PVSystem in it,
/// the same on all three engines, an open item. This deck holds it.
const H_Z0: [&str; 4] = [
    "Clear",
    "New Circuit.dp basekv=0.69 phases=3 bus1=srcbus",
    "New Line.l1 bus1=srcbus bus2=wbus phases=3 r1=0.005 x1=0.02 r0=0.005 x0=0.02 length=1",
    "New Load.ld bus1=wbus phases=3 kv=0.69 kW=300 kvar=100",
];
const PF: [&str; 3] = ["Set voltagebases=[0.69]", "Calcvoltagebases", "Solve"];

const GEN: &str = "New Generator.e1 bus1=wbus phases=3 kv=0.69 kW=1500 kva=1800 model=1";
const PV: &str = "New PVSystem.e1 bus1=wbus phases=3 kv=0.69 kVA=1800 Pmpp=1500 irradiance=1";
const STD: &str = "New Storage.e1 bus1=wbus phases=3 kv=0.69 kWrated=500 kWhrated=2000 kva=500 \
                   %stored=50 %discharge=60 state=discharging";
const STI: &str = "New Storage.e1 bus1=wbus phases=3 kv=0.69 kWrated=500 kWhrated=2000 kva=500 \
                   %stored=50 state=idling";
const WG: &str = "New WindGen.e1 bus1=wbus phases=3 kv=0.69 kW=1500 kva=1800 conn=wye model=1 \
                  vss=1 pss=1 qss=0 vwind=12";
const IM: &str = "New IndMach012.e1 bus1=wbus kW=200 conn=delta kVA=250 H=6 purs=0.048 \
                  puxs=0.075 purr=0.018 puxr=0.12 puxm=3.8 slip=0.02 SlipOption=variableslip kv=0.69";
const VCCS_NC: &str = "New VCCS.e1 phases=1 bus1=wbus.1 Prated=190 Vrated=398 Ppct=89.5";
/// The microinverter curves of the vendored `Examples/VCCS/HW_Inverters.txt`.
const CURVES_W: [&str; 3] = [
    "New XYcurve.bp1_micro npts=3 xarray=[-0.882023 0.0 0.882023] yarray=[-0.19 0.0 0.19]",
    "New XYcurve.bp2_micro npts=5 xarray=[-0.085467 -0.0202 0.0 0.0202 0.085467] \
     yarray=[4.59572 1.0 0.0 -1.0 -4.59572]",
    "New XYcurve.z_micro npts=14 xarray=[1.0000000000000 -1.3802100241988 -0.9341004900495 \
     1.1196620891546 0.1499829369937 1.2123723040884 -0.7654392492287 -0.9113338678997 \
     0.3694835076144 0.1034830928158 0.1549604084080 -0.2209879303901 0.1373924816843 \
     -0.0352249177928] yarray=[0.0000000000000 -0.2328126380366 0.9436331671129 \
     -0.5290906112039 -1.2586072419992 0.6136072759695 0.5705618604200 1.0000000000000 \
     -0.5053547399078 -1.1387058093285 -0.0677012182563 0.5067831545610 0.4003099336092 \
     -0.3026288423351]",
];
/// The PLL filter of the vendored `Examples/VCCS/HWPLL.dss`.
const CURVE_R: [&str; 1] =
    ["New XYcurve.z_pll npts=3 xarray=[1.0000 -1.9852 0.9853] yarray=[0.0000 0.0148 -0.0147]"];
const VCCS_W: &str = "New VCCS.e1 phases=1 bus1=wbus.1 Prated=190 Vrated=398 Ppct=89.5 \
                      bp1=bp1_micro bp2=bp2_micro filter=z_micro fsample=10000";
const VCCS_R: &str = "New VCCS.e1 phases=1 bus1=wbus.1 Prated=190 Vrated=398 Ppct=89.5 \
                      filter=z_pll fsample=10000 rmsmode=true imaxpu=1.15 vrmstau=0.01 irmstau=0.05";

/// The converter circuit of the vendored VSConverter example: a 1-phase source,
/// the converter and a DC line to a constant-impedance DC load, solved.
const VSC_HEAD: [&str; 2] = [
    "Clear",
    "New Circuit.vsc basekv=7.967 pu=1.004644 phases=1 bus1=SourceBus r1=0.02 r0=0.02 \
     x1=0.0 x0=0.0",
];
const VSC: &str = "New VSConverter.e1 Phases=2 Ndc=1 Bus1=SourceBus.1.2 kVac=7.967 kVdc=45 \
                   kW=800 Rac=0.02 Xac=0.05 VscMode=VdcVac Vacref=7967 Vdcref=45000 Idcmax=1.1";
const VSC_TAIL: [&str; 7] = [
    "New LineCode.DCcable nphases=1 rmatrix=(0.01) xmatrix=(0.0) cmatrix=(0.0)",
    "New Line.dc1 phases=1 Bus1=SourceBus.2 Bus2=DC2.1 linecode=DCCable length=0.5",
    "New Load.dc2 phases=1 bus1=dc2.1 Conn=Wye Model=2 kV=45.0 kW=800 pf=1.0",
    "Set Voltagebases=[13.8]",
    "set maxiterations=100",
    "Calcv",
    "Solve",
];

fn dyn_entry(n: usize) -> String {
    format!("Set mode=dynamic stepsize=0.001 number={n}")
}

fn mode3(class: &str) -> String {
    format!("New Monitor.ms element={class}.e1 terminal=1 mode=3")
}

/// A scratch directory for the reports one test writes.
fn scratch(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("dss_dyn_membership_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// Send `lines` one command at a time, each adding no diagnostic.
fn run(dss: &mut Dss, lines: &[String]) {
    for line in lines {
        dss.command(line);
        assert!(dss.errors().is_empty(), "{line}: {:?}", dss.errors());
    }
}

/// A fresh engine writing into `dir` that ran `lines`.
fn deck(dir: &std::path::Path, lines: &[String]) -> Dss {
    let mut dss = Dss::new();
    let mut all = vec![
        lines[0].clone(),
        format!("set datapath=\"{}\"", dir.display()),
    ];
    all.extend_from_slice(&lines[1..]);
    run(&mut dss, &all);
    dss
}

fn lines(parts: &[&[&str]]) -> Vec<String> {
    parts
        .iter()
        .flat_map(|p| p.iter().map(|s| s.to_string()))
        .collect()
}

/// Every node voltage by node name.
fn node_voltages(dss: &Dss) -> Vec<(String, Complex64)> {
    let ckt = dss.circuit().expect("circuit");
    let mut v: Vec<(String, Complex64)> = (1..=ckt.num_nodes)
        .map(|i| (ckt.node_name(i), ckt.solution.node_v[i]))
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

/// Both runs hold the same nodes with bit-equal voltages. `what` names the
/// reference in a failure, as in "against 1+2j {what}".
fn assert_same_voltages(case: &str, got: &Dss, reference: &Dss, what: &str) {
    let (a, b) = (node_voltages(got), node_voltages(reference));
    assert_eq!(
        a.iter().map(|x| &x.0).collect::<Vec<_>>(),
        b.iter().map(|x| &x.0).collect::<Vec<_>>(),
        "{case}: node lists"
    );
    for ((n, va), (_, vb)) in a.iter().zip(&b) {
        assert!(
            va.re.to_bits() == vb.re.to_bits() && va.im.to_bits() == vb.im.to_bits(),
            "{case}: {n} = {va} against {vb} {what}"
        );
    }
}

fn wbus1(dss: &Dss) -> f64 {
    node_voltages(dss)
        .into_iter()
        .find(|(n, _)| n == "WBUS.1")
        .expect("WBUS.1")
        .1
        .norm()
}

/// The terminal currents `CktElement::get_currents` answers in the present
/// mode, read with the circuit's own `SysCtx`. The buffer starts as NaN so an
/// override that writes nothing is seen.
fn element_currents(dss: &mut Dss, name: &str) -> Vec<Complex64> {
    let Dss {
        classes, circuit, ..
    } = dss;
    let ckt = circuit.as_ref().expect("circuit");
    let sys = crate::solution::solution::sys_ctx(ckt);
    let node_v = ckt.solution.node_v.clone();
    for class in classes.iter_mut() {
        let cn = class.props.class_name();
        for oi in 0..class.arena.len() {
            let full = format!("{}.{}", cn, class.arena.obj_mut(oi).data().name());
            if full.eq_ignore_ascii_case(name) {
                let elem = class.arena.try_ckt_elem_mut(oi).expect("a circuit element");
                let mut cur = vec![Complex64::new(f64::NAN, f64::NAN); elem.cd().yorder];
                elem.get_currents(&sys, &node_v, &mut cur);
                return cur;
            }
        }
    }
    panic!("{name} not found")
}

fn variables(dss: &mut Dss, name: &str) -> Vec<(String, f64)> {
    let names = dss.element_variable_names(name).expect("element");
    let values = dss.element_variables(name).expect("element");
    names.into_iter().zip(values).collect()
}

/// Write variable `i` (1-based) of `name` through its variable setter, the one
/// `Set StateVar` and the API's variable write reach.
fn set_state(dss: &mut Dss, name: &str, i: usize, value: f64) {
    let Dss {
        classes, circuit, ..
    } = dss;
    let ckt = circuit.as_ref().expect("circuit");
    let sys = crate::solution::solution::sys_ctx(ckt);
    for class in classes.iter_mut() {
        let cn = class.props.class_name();
        for oi in 0..class.arena.len() {
            let full = format!("{}.{}", cn, class.arena.obj_mut(oi).data().name());
            if full.eq_ignore_ascii_case(name) {
                let elem = class.arena.try_ckt_elem_mut(oi).expect("a circuit element");
                elem.set_variable(i, value, &sys);
                return;
            }
        }
    }
    panic!("{name} not found")
}

/// The WindGen settings that only its variable setter writes, each given a
/// value its default does not hold: (1-based variable, name, value).
const WG_INPUTS: [(usize, &str, f64); 4] = [
    (1, "userTrip", 1.0),
    (3, "Pcurtail", 0.85),
    (14, "dFrqPuTest", 0.03),
    (16, "Qref", 0.2),
];

/// The element line of a setting pin: the WindGen gets `QMode=1`, the PF
/// reactive mode, where the default is 0 (Q).
fn setting_line(class: &str, line: &str) -> String {
    if class == "WindGen" {
        format!("{line} QMode=1")
    } else {
        line.to_string()
    }
}

/// Give `{class}.e1` the setting inputs no property carries: the WindGen gets
/// [`WG_INPUTS`] through its variable setter, every other class nothing.
fn write_setting_inputs(dss: &mut Dss, class: &str) {
    if class == "WindGen" {
        for (i, _, value) in WG_INPUTS {
            set_state(dss, "WindGen.e1", i, value);
        }
    }
}

/// The inverter settings by variable name: `Series L` and `Max. Amps (phase)`
/// come from the rated data in the seed's operation order (`%X` 0 read as 50),
/// `Rated VDC` is `kVDC` in volts.
fn inverter_ratings(dss: &mut Dss, elem: &str, fundamental: f64) -> Vec<(&'static str, f64)> {
    let kv = query_f64(dss, &format!("{elem}.kv"));
    let kva = query_f64(dss, &format!("{elem}.kVA"));
    let pct_x = query_f64(dss, &format!("{elem}.%X"));
    let nphases = query_f64(dss, &format!("{elem}.phases"));
    let base_kv = if nphases > 1.0 {
        kv / 3.0_f64.sqrt()
    } else {
        kv
    };
    let base_zt = 0.01 * (kv * kv / kva) * 1000.0;
    let i_max_p_phase = (kva / base_kv) / nphases;
    let pct_x = if pct_x == 0.0 { 50.0 } else { pct_x };
    let ls = pct_x * base_zt / (2.0 * std::f64::consts::PI * fundamental);
    vec![
        (
            "Rated VDC",
            query_f64(dss, &format!("{elem}.kVDC")) * 1000.0,
        ),
        ("Series L", ls),
        ("Max. Amps (phase)", i_max_p_phase),
    ]
}

/// The settings a disabled element of each case keeps, each from its rule.
/// Property-read slots also agree with `snap`, the reading taken before the
/// entry or the reading under test. No expected value is taken from `snap`.
fn expected_settings(
    dss: &mut Dss,
    case: &str,
    elem: &str,
    snap: &[(String, f64)],
) -> Vec<(&'static str, f64)> {
    let snap_of = |n: &str| {
        snap.iter()
            .find(|(m, _)| m == n)
            .unwrap_or_else(|| panic!("{case}: {n} in the snap reading"))
            .1
    };
    match case {
        "PVSystem" => {
            // No LoadShapeClass, no P-TCurve, no EffCurve: the shape factor, the
            // temperature factor and the efficiency are 1.
            let (shape_factor, temp_factor, efficiency) = (1.0, 1.0, 1.0);
            let irradiance = query_f64(dss, &format!("{elem}.irradiance")) * shape_factor;
            let panel_kw = query_f64(dss, &format!("{elem}.Pmpp")) * irradiance * temp_factor;
            let mut s = vec![
                ("Irradiance", irradiance),
                ("PanelkW", panel_kw),
                ("P_TFactor", temp_factor),
                ("Efficiency", efficiency),
                ("kW_out_desired", panel_kw * efficiency),
            ];
            s.extend(inverter_ratings(dss, elem, 60.0));
            assert_eq!(snap_of("Rated VDC"), s[5].1, "{case}: snap Rated VDC");
            s
        }
        "Storage discharging" | "Storage idling" => {
            let kw_rated = query_f64(dss, &format!("{elem}.kWRated"));
            let state = query(dss, &format!("{elem}.State"));
            let (ordinal, kw_desired) = if state.eq_ignore_ascii_case("discharging") {
                (
                    1.0,
                    query_f64(dss, &format!("{elem}.%Discharge")) * kw_rated / 100.0,
                )
            } else {
                assert!(state.eq_ignore_ascii_case("idling"), "{case}: {state}");
                (0.0, 0.0)
            };
            let mut s = vec![
                ("kWh", query_f64(dss, &format!("{elem}.kWhStored"))),
                ("State", ordinal),
                ("kWDesired", kw_desired),
                (
                    "Limit kWOut Function",
                    query_f64(dss, &format!("{elem}.%kWRated")) * 0.01 * kw_rated,
                ),
            ];
            s.extend(inverter_ratings(dss, elem, 60.0));
            for n in ["kWh", "State", "Rated VDC"] {
                let want = s.iter().find(|x| x.0 == n).expect("listed").1;
                assert_eq!(snap_of(n), want, "{case}: snap {n}");
            }
            s
        }
        "WindGen" => {
            // The inputs the user writes: `vwind` and `QMode` as properties
            // (`setting_line`), the other four through the variable setter
            // (`write_setting_inputs`).
            let vwind = query_f64(dss, &format!("{elem}.vwind"));
            assert_eq!(snap_of("vwind"), vwind, "{case}: vwind");
            let q_mode = query(dss, &format!("{elem}.QMode"));
            let q_mode = match q_mode.to_ascii_lowercase().as_str() {
                "q" | "0" => 0.0,
                "pf" | "1" => 1.0,
                "voltvar" | "2" => 2.0,
                other => panic!("{case}: QMode {other}"),
            };
            assert_eq!(q_mode, 1.0, "{case}: the pin's WindGen is in PF mode");
            let mut s: Vec<(&'static str, f64)> =
                WG_INPUTS.iter().map(|&(_, n, v)| (n, v)).collect();
            s.push(("vwind", vwind));
            s.push(("QMode", q_mode));
            s
        }
        "IndMach012" => {
            let s: Vec<(&'static str, f64)> = [
                ("puRs", "puRs"),
                ("puXs", "puXs"),
                ("puRr", "puRr"),
                ("puXr", "puXr"),
                ("puXm", "puXm"),
                ("Maxslip", "MaxSlip"),
            ]
            .into_iter()
            .map(|(n, p)| (n, query_f64(dss, &format!("{elem}.{p}"))))
            .collect();
            for (n, v) in &s {
                assert_eq!(snap_of(n), *v, "{case}: snap {n}");
            }
            s
        }
        _ => Vec::new(),
    }
}

/// Every variable of `elem` is finite, each setting equals its rule and every
/// other variable reads 0.
fn assert_zero_but_settings(case: &str, vars: &[(String, f64)], settings: &[(&str, f64)]) {
    assert!(!vars.is_empty(), "{case}: no variables");
    for (n, v) in vars {
        assert!(v.is_finite(), "{case}: {n} = {v}");
        match settings.iter().find(|s| s.0 == n) {
            Some((_, want)) => assert_eq!(v, want, "{case}: setting {n}"),
            None => assert_eq!(*v, 0.0, "{case}: dynamic {n}"),
        }
    }
    for (n, _) in settings {
        assert!(
            vars.iter().any(|(m, _)| m == n),
            "{case}: setting {n} is a variable"
        );
    }
}

/// `Show Variables` lists only enabled elements and prints no non-finite value.
fn assert_show_variables_omits(dss: &mut Dss, case: &str, elem: &str) {
    dss.command("Show Variables");
    assert!(dss.errors().is_empty(), "{case}: {:?}", dss.errors());
    let text = std::fs::read_to_string(dss.last_show_file()).expect("Show Variables file");
    assert!(
        !text
            .lines()
            .any(|l| l.trim().eq_ignore_ascii_case(&format!("ELEMENT: {elem}"))),
        "{case}: Show Variables lists the disabled element\n{text}"
    );
    let lower = text.to_lowercase();
    assert!(
        !lower.contains("nan") && !lower.contains("inf"),
        "{case}: non-finite value in Show Variables\n{text}"
    );
}

/// A disabled element of every class with dynamic state takes no part in a
/// dynamics run: the network equals the one without it, `Show Variables` omits
/// it, and the API, `CktElement::get_currents` with the circuit's dynamics
/// `SysCtx` and a state-variable monitor read zero but its settings. The
/// WindGen has no monitor, since one fails its binding, and the settings carry
/// inputs their defaults do not hold ([`setting_line`],
/// [`write_setting_inputs`]), so each slot shows its own value.
#[test]
fn a_disabled_element_takes_no_part_in_a_dynamics_run() {
    let dir = scratch("disabled");
    // (case, class, lines the element needs, element line, mode-3 monitor)
    let cases: [(&str, &str, &[&str], &str, bool); 9] = [
        ("Generator", "Generator", &[], GEN, true),
        ("PVSystem", "PVSystem", &[], PV, true),
        ("Storage discharging", "Storage", &[], STD, true),
        ("Storage idling", "Storage", &[], STI, true),
        ("WindGen", "WindGen", &[], WG, false),
        ("IndMach012", "IndMach012", &[], IM, true),
        ("VCCS without curves", "VCCS", &[], VCCS_NC, true),
        ("VCCS with curves", "VCCS", &CURVES_W, VCCS_W, true),
        ("RMS VCCS", "VCCS", &CURVE_R, VCCS_R, true),
    ];
    for (case, class, pre, line, monitored) in cases {
        let elem = format!("{class}.e1");
        let disabled = format!("{} enabled=no", setting_line(class, line));
        let monitor = mode3(class);
        let mut before = lines(&[&H, pre, &[disabled.as_str()]]);
        if monitored {
            before.push(monitor);
        }
        before.extend(lines(&[&PF]));
        let mut dss = deck(&dir, &before);
        write_setting_inputs(&mut dss, class);
        let snap = variables(&mut dss, &elem);
        let settings = expected_settings(&mut dss, case, &elem, &snap);
        run(&mut dss, &[dyn_entry(5), "Solve".to_string()]);

        let reference = deck(
            &dir,
            &lines(&[&H, pre, &PF, &[dyn_entry(5).as_str(), "Solve"]]),
        );
        assert_same_voltages(case, &dss, &reference, "without the element");
        assert_eq!(format!("{:.6}", wbus1(&dss)), "395.359096", "{case}");

        assert_zero_but_settings(case, &variables(&mut dss, &elem), &settings);
        for c in element_currents(&mut dss, &elem) {
            assert_eq!(c, Complex64::ZERO, "{case}: current");
        }
        assert_show_variables_omits(&mut dss, case, &elem);
        if monitored {
            let m = dss.monitor_view("ms").expect("ms");
            assert_eq!(m.sample_count, 5, "{case}: samples");
            let names = dss.element_variable_names(&elem).expect("names");
            assert_eq!(m.channels.len(), names.len(), "{case}: channels");
            for (ch, n) in m.channels.iter().zip(&names) {
                let want = settings.iter().find(|s| s.0 == n).map_or(0.0, |s| s.1) as f32;
                assert_eq!(ch.len(), 5, "{case}: {n}");
                assert!(
                    ch.iter().all(|&x| x == want),
                    "{case}: monitor {n} = {ch:?}"
                );
            }
        }
    }

    // The VSConverter of the vendored converter example, disabled on `New`:
    // the network equals the one without it (both oracles: the DC bus at 0 V
    // with and without it). Its current reading is a monitor question.
    let vsc = format!("{VSC} enabled=no");
    let steps = [dyn_entry(5), "Solve".to_string()];
    let mut dss = deck(&dir, &lines(&[&VSC_HEAD, &[vsc.as_str()], &VSC_TAIL]));
    run(&mut dss, &steps);
    let mut reference = deck(&dir, &lines(&[&VSC_HEAD, &VSC_TAIL]));
    run(&mut reference, &steps);
    assert_same_voltages("VSConverter", &dss, &reference, "without the element");
    std::fs::remove_dir_all(&dir).ok();
}

/// The panel settings of a disabled PVSystem follow its present inputs, before
/// and during the run and after a daily run: `P_TFactor` is its `P-TCurve` at
/// the `Temperature` property, `Efficiency` its `EffCurve` at `PanelkW / kVA`,
/// and a reading leaves the unit as it was. A unit seeded straight after a
/// daily run starts from the temperature that run left, an open defect held so
/// that a seed from the present inputs turns it red, and one seeded after `Set
/// mode=snap` and `Solve` starts from its `Temperature`.
#[test]
fn a_disabled_pvsystem_reads_its_panel_settings_from_its_present_inputs() {
    let dir = scratch("pv_curves");
    let curves = [
        "New XYcurve.pt npts=4 xarray=[0 25 50 75] yarray=[1.2 1.0 0.8 0.6]",
        "New XYcurve.eff npts=4 xarray=[0.1 0.4 0.8 1.0] yarray=[0.86 0.94 0.95 0.96]",
    ];
    let pv = "New PVSystem.e1 bus1=wbus phases=3 kv=0.69 kVA=1500 Pmpp=1500 irradiance=1 \
              temperature=25 P-TCurve=pt EffCurve=eff";
    // No LoadShapeClass: the shape factor is 1.
    let (irradiance, shape_factor, pmpp, kva) = (1.0, 1.0, 1500.0, 1500.0);
    let check = |dss: &mut Dss, case: &str, p_tfactor: f64, efficiency: f64| {
        let vars = variables(dss, "PVSystem.e1");
        let read = |n: &str| vars.iter().find(|(m, _)| m == n).expect("variable").1;
        let panel_kw = irradiance * shape_factor * pmpp * p_tfactor;
        assert_eq!(read("Irradiance"), irradiance * shape_factor, "{case}");
        assert_eq!(read("P_TFactor"), p_tfactor, "{case}");
        assert_eq!(read("PanelkW"), panel_kw, "{case}");
        assert_eq!(read("Efficiency"), efficiency, "{case}");
        assert_eq!(read("kW_out_desired"), panel_kw * efficiency, "{case}");
    };
    assert_eq!(pmpp * 0.8 / kva, 0.8, "the EffCurve abscissa at 50 degrees");
    assert_eq!(pmpp * 1.0 / kva, 1.0, "the EffCurve abscissa at 25 degrees");

    let disabled = format!("{pv} enabled=no");
    let head = lines(&[
        &H,
        &curves,
        &[disabled.as_str()],
        &PF,
        &[
            "Edit PVSystem.e1 temperature=50",
            dyn_entry(3).as_str(),
            "Solve",
        ],
    ]);
    let tail = lines(&[&["Edit PVSystem.e1 temperature=25", "Set mode=snap"]]);
    let mut dss = deck(&dir, &head);
    check(&mut dss, "50 degrees before the entry", 0.8, 0.95);
    run(&mut dss, &tail[..1]);
    check(&mut dss, "25 degrees during the run", 1.0, 0.96);
    run(&mut dss, &tail[1..]);
    let mut unread = deck(&dir, &[head, tail].concat());
    let bits = |dss: &mut Dss| -> Vec<u64> {
        dss.element_variables("PVSystem.e1")
            .expect("PVSystem")
            .iter()
            .map(|v| v.to_bits())
            .collect()
    };
    assert_eq!(
        bits(&mut dss),
        bits(&mut unread),
        "the readings in the run changed the unit"
    );

    let tdaily = format!("{pv} Tdaily=tday");
    let daily = lines(&[
        &H,
        &curves,
        &[
            "New TShape.tday npts=2 interval=1 temp=[50 50]",
            tdaily.as_str(),
        ],
        &PF,
        &["Set mode=daily number=1", "Solve"],
    ]);
    let entry = dyn_entry(3);
    let disabled = lines(&[&["Disable PVSystem.e1", entry.as_str()]]);
    let mut dss = deck(&dir, &[daily.clone(), disabled].concat());
    check(&mut dss, "disabled after a daily run", 1.0, 0.96);

    let mut seeded = deck(&dir, &[daily.clone(), vec![entry.clone()]].concat());
    // The seed's stale temperature, an open defect (see the doc).
    check(
        &mut seeded,
        "seeded straight after the daily run",
        0.8,
        0.95,
    );
    run(&mut seeded, &["Solve".to_string()]);
    check(&mut seeded, "in the run after the daily run", 1.0, 0.95);
    let remedy = lines(&[&["Set mode=snap", "Solve", entry.as_str()]]);
    let mut seeded = deck(&dir, &[daily, remedy].concat());
    check(
        &mut seeded,
        "seeded after Set mode=snap and Solve",
        1.0,
        0.96,
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// A Storage outside the run reads the `kWDesired` its present `State` asks:
/// `%discharge` of the rated kW delivered, `%charge` of it drawn, nothing when
/// idling. A disabled unit follows a `State=` edit during the run, and a unit
/// created after the entry, disabled and solved or joining, reads its state's
/// demand though no power flow included it.
#[test]
fn a_storage_outside_the_run_reads_the_kw_its_state_asks() {
    let dir = scratch("storage_kw_desired");
    let st = "New Storage.e1 bus1=wbus phases=3 kv=0.69 kWrated=500 kWhrated=2000 kva=500 \
              %stored=50 %discharge=60 %charge=40 state=discharging";
    let (kw_rated, pct_discharge, pct_charge) = (500.0, 60.0, 40.0);
    let check = |dss: &mut Dss, elem: &str, case: &str, state: f64, kw_desired: f64| {
        let vars = variables(dss, elem);
        let read = |n: &str| vars.iter().find(|(m, _)| m == n).expect("variable").1;
        assert_eq!(read("State"), state, "{case}: State");
        assert_eq!(read("kWDesired"), kw_desired, "{case}: kWDesired");
    };
    let disabled = format!("{st} enabled=no");
    let mut dss = deck(
        &dir,
        &lines(&[
            &H,
            &[disabled.as_str()],
            &PF,
            &[dyn_entry(1).as_str(), "Solve"],
        ]),
    );
    let discharging = pct_discharge * kw_rated / 100.0;
    check(
        &mut dss,
        "Storage.e1",
        "disabled, discharging",
        1.0,
        discharging,
    );
    run(&mut dss, &["Edit Storage.e1 state=charging".to_string()]);
    let charging = -pct_charge * kw_rated / 100.0;
    check(&mut dss, "Storage.e1", "disabled, charging", -1.0, charging);
    run(&mut dss, &["Edit Storage.e1 state=idling".to_string()]);
    check(&mut dss, "Storage.e1", "disabled, idling", 0.0, 0.0);

    let late = format!("{} enabled=no", st.replace("Storage.e1", "Storage.e3"));
    run(&mut dss, &[late, "Solve".to_string()]);
    check(
        &mut dss,
        "Storage.e3",
        "disabled, created after the entry",
        1.0,
        discharging,
    );
    run(&mut dss, &["Edit Storage.e3 state=idling".to_string()]);
    check(
        &mut dss,
        "Storage.e3",
        "disabled, created after the entry, idling",
        0.0,
        0.0,
    );

    let joining = st.replace("Storage.e1", "Storage.e2");
    run(&mut dss, &[joining]);
    check(
        &mut dss,
        "Storage.e2",
        "created after the entry",
        1.0,
        discharging,
    );
    assert_eq!((discharging, charging), (300.0, -200.0));
    std::fs::remove_dir_all(&dir).ok();
}

/// `Disable` after the power flow takes the PVSystem out before the entry, so
/// the run equals the one that never had it and the PVSystem reads zero but its
/// settings. The Load is constant impedance, so the network is linear, each
/// step one solve of the same Y and injection, and bit-equality is the rule.
#[test]
fn the_disable_command_takes_a_solved_element_out_of_the_run() {
    let dir = scratch("disable_cmd");
    let mut dss = deck(
        &dir,
        &lines(&[
            &H_MODEL2,
            &[PV],
            &PF,
            &[
                "Disable PVSystem.e1",
                "Solve",
                dyn_entry(5).as_str(),
                "Solve",
            ],
        ]),
    );
    let reference = deck(
        &dir,
        &lines(&[&H_MODEL2, &PF, &[dyn_entry(5).as_str(), "Solve"]]),
    );
    assert_same_voltages("Disable", &dss, &reference, "without the element");
    assert_eq!(format!("{:.6}", wbus1(&dss)), "395.404554");
    let settings = [
        "Irradiance",
        "PanelkW",
        "P_TFactor",
        "Efficiency",
        "kW_out_desired",
        "Rated VDC",
        "Series L",
        "Max. Amps (phase)",
    ];
    for (n, v) in variables(&mut dss, "PVSystem.e1") {
        assert!(v.is_finite(), "{n} = {v}");
        if !settings.contains(&n.as_str()) {
            assert_eq!(v, 0.0, "dynamic {n}");
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// A Generator disabled during a run stops: its stored state is the one it had
/// when it left, read back in snap mode, every reader and its monitor see zero
/// while it is out, and the network equals the one without it.
#[test]
fn an_element_disabled_mid_run_keeps_its_state_and_reads_zero() {
    let dir = scratch("mid_run");
    let monitor = mode3("Generator");
    let start = lines(&[
        &H_MODEL2,
        &[GEN, monitor.as_str()],
        &PF,
        &[dyn_entry(3).as_str(), "Solve"],
    ]);
    let mut dss = deck(&dir, &start);
    let before = dss.element_variables("Generator.e1").expect("Generator");
    assert!(before.iter().any(|v| *v != 0.0), "a live state: {before:?}");
    run(
        &mut dss,
        &[
            "Edit Generator.e1 enabled=no".to_string(),
            "Solve".to_string(),
        ],
    );

    let during = dss.element_variables("Generator.e1").expect("Generator");
    assert!(
        during.iter().all(|v| *v == 0.0),
        "out of the run: {during:?}"
    );
    for c in element_currents(&mut dss, "Generator.e1") {
        assert_eq!(c, Complex64::ZERO, "current");
    }

    let mut live = deck(&dir, &start);
    run(&mut live, &["Solve".to_string()]);
    let (m, ml) = (
        dss.monitor_view("ms").expect("ms"),
        live.monitor_view("ms").expect("ms"),
    );
    assert_eq!((m.sample_count, ml.sample_count), (6, 6));
    for (ch, chl) in m.channels.iter().zip(&ml.channels) {
        assert_eq!(ch[..3], chl[..3], "samples while in the run");
        assert!(
            ch[3..].iter().all(|&x| x == 0.0),
            "samples while out: {ch:?}"
        );
    }

    let reference = deck(
        &dir,
        &lines(&[&H_MODEL2, &PF, &[dyn_entry(3).as_str(), "Solve", "Solve"]]),
    );
    assert_same_voltages("disabled mid-run", &dss, &reference, "without the element");

    run(&mut dss, &["Set mode=snap".to_string()]);
    let after = dss.element_variables("Generator.e1").expect("Generator");
    assert_eq!(
        before.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        after.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        "stored state {before:?} against {after:?}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// A disabled Generator linked to a `DynamicExp` without equations is not asked
/// about it: the run equals the one without the Generator.
#[test]
fn a_disabled_host_is_not_asked_about_its_dynamic_exp() {
    let dir = scratch("dyneq_host");
    let host = "New Generator.e2 bus1=wbus phases=3 kv=0.69 kW=100 kva=120 model=1 \
                DynamicEq=de enabled=no";
    let steps = [dyn_entry(3), "Solve".to_string()];
    let mut dss = deck(&dir, &lines(&[&H, &["New DynamicExp.de", host], &PF]));
    run(&mut dss, &steps);
    let mut reference = deck(&dir, &lines(&[&H, &["New DynamicExp.de"], &PF]));
    run(&mut reference, &steps);
    assert_same_voltages("disabled host", &dss, &reference, "without the element");
    std::fs::remove_dir_all(&dir).ok();
}

/// The committed `UserModel` fixture `name` of `tests/fixtures/wasm`, as an
/// absolute forward-slash path.
fn wasm_fixture(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/wasm")
        .join(name)
        .canonicalize()
        .expect("the committed fixture")
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('\\', "/")
}

/// A disabled PVSystem, Storage or WindGen that hosts a `DynamicExp` with
/// variables lists the equation's variables, and in the run every one reads
/// zero through the API and in a state-variable monitor, since none of the
/// host's settings is written into the equation's slots. A PVSystem that also
/// binds a `UserModel` lists the model's variables after them, which answer in
/// snap mode and read zero in the run, and the WindGen has no monitor because
/// one fails its binding.
#[test]
fn a_disabled_dynamic_exp_host_reads_zero_in_every_slot() {
    let dir = scratch("dyneq_host_slots");
    let de = "New DynamicExp.de nvariables=2 varnames=[a b] expression=[a dt = b]";
    let pv_model = format!(
        "{PV} UserModel={} UserData=(g=0.25 b=-0.125 tau=0.5)",
        wasm_fixture("wm4model.wasm")
    );
    // (case, class, element line, mode-3 monitor, UserModel)
    let cases = [
        ("PVSystem", "PVSystem", PV, true, false),
        (
            "PVSystem with a UserModel",
            "PVSystem",
            pv_model.as_str(),
            true,
            true,
        ),
        ("Storage", "Storage", STD, true, false),
        ("WindGen", "WindGen", WG, false, false),
    ];
    for (case, class, line, monitored, model) in cases {
        let elem = format!("{class}.e1");
        let host = format!("{line} DynamicEq=de enabled=no");
        let mut before = lines(&[&H, &[de, host.as_str()]]);
        if monitored {
            before.push(mode3(class));
        }
        before.extend(lines(&[&PF]));
        let mut dss = deck(&dir, &before);
        let snap = variables(&mut dss, &elem);
        run(&mut dss, &[dyn_entry(5), "Solve".to_string()]);
        let vars = variables(&mut dss, &elem);
        assert!(!vars.is_empty(), "{case}: the host lists variables");
        for (n, v) in &vars {
            assert_eq!(*v, 0.0, "{case}: {n}");
        }
        if model {
            let names: Vec<&str> = vars.iter().map(|(n, _)| n.as_str()).collect();
            assert!(
                names.ends_with(&["Iout1", "G", "B", "Tau"]),
                "{case}: {names:?}"
            );
            let read = |n: &str| snap.iter().find(|(m, _)| m == n).expect("variable").1;
            assert_eq!(
                (read("G"), read("B"), read("Tau")),
                (0.25, -0.125, 0.5),
                "{case}: the model answers in snap mode"
            );
        }
        if monitored {
            let m = dss.monitor_view("ms").expect("ms");
            assert_eq!(m.sample_count, 5, "{case}: samples");
            assert_eq!(m.channels.len(), vars.len(), "{case}: channels");
            for (ch, (n, _)) in m.channels.iter().zip(&vars) {
                assert_eq!(ch.len(), 5, "{case}: {n}");
                assert!(ch.iter().all(|&x| x == 0.0), "{case}: monitor {n} = {ch:?}");
            }
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// A UPFC solved in the power flow, then disabled and solved again, leaves the
/// model: in the dynamics run it reads its mode and zero and carries no
/// current. Its `UPFCControl` keeps acting on it in every solve and ends each
/// with the max-control-iterations warning, the one diagnostic the pin allows.
#[test]
fn a_disabled_upfc_reads_its_mode_and_zero() {
    let deck_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/controls/upfc/upfc_statcom.dss");
    assert!(
        deck_path.is_file(),
        "vendored corpus deck missing: {deck_path:?}"
    );
    let mut dss = Dss::new();
    let only_control_warning = |dss: &Dss, step: &str| {
        assert!(
            dss.errors().iter().all(|e| e
                .message
                .starts_with("Warning Max Control Iterations Exceeded.")),
            "{step}: {:?}",
            dss.errors()
        );
    };
    dss.command(&format!("compile \"{}\"", deck_path.display()));
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let live = dss.element_variables("UPFC.test").expect("UPFC");
    assert_eq!(live[0], 2.0, "ModeUPFC");
    assert!(live[1] > 0.0, "IUPFC of the live unit: {live:?}");

    dss.command("Disable UPFC.test");
    dss.command("Solve");
    only_control_warning(&dss, "disabled power flow");
    let left = dss.element_variables("UPFC.test").expect("UPFC");
    assert!(
        left[1..].iter().any(|v| *v != 0.0),
        "operating values: {left:?}"
    );

    dss.command(&dyn_entry(3));
    dss.command("Solve");
    only_control_warning(&dss, "dynamics run");
    let vars = dss.element_variables("UPFC.test").expect("UPFC");
    assert_eq!(vars.len(), 14);
    assert_eq!(vars[0], 2.0, "ModeUPFC is a setting");
    assert!(
        vars[1..].iter().all(|v| *v == 0.0),
        "out of the model: {vars:?}"
    );
    for c in element_currents(&mut dss, "UPFC.test") {
        assert_eq!(c, Complex64::ZERO, "current");
    }
}

/// `Get StateVar`, the `VARIABLES` block of `Dump` and a Generic relay read the
/// zero of a disabled Generator in the dynamics run, not the fundamental a
/// never-seeded Generator holds as its raw `Frequency`. The relay samples only
/// in the run and its `undertrip` lies below zero, so the zero reading leaves
/// the line closed and the network equals the one without the Generator, while
/// with the Generator in the run it opens `Line.l1`.
#[test]
fn readers_of_a_disabled_element_read_zero() {
    let dir = scratch("readers");
    let relay = "New Relay.r1 MonitoredObj=Generator.e1 MonitoredTerm=1 SwitchedObj=Line.l1 \
                 SwitchedTerm=1 type=generic Variable=Frequency overtrip=59 undertrip=-1 Delay=0 \
                 Breakertime=0";
    let disabled = format!("{GEN} enabled=no");
    let mut dss = deck(
        &dir,
        &lines(&[
            &H,
            &[disabled.as_str()],
            &PF,
            &[relay, dyn_entry(5).as_str(), "Solve"],
        ]),
    );
    let reference = deck(&dir, &lines(&[&H, &PF, &[dyn_entry(5).as_str(), "Solve"]]));
    assert_same_voltages(
        "relay on a disabled Generator",
        &dss,
        &reference,
        "without the element",
    );

    dss.command("Get StateVar Generator.e1 Frequency");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(dss.result(), "0");

    dss.command("Dump Generator.e1 debug");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let dump = std::fs::read_to_string(dss.last_result_file()).expect("Dump file");
    assert!(
        dump.lines().any(|l| l == "!  1: Frequency = 0"),
        "Dump VARIABLES:\n{dump}"
    );

    // Control: the relay samples in the run and operates on a reading of 60.
    let mut live = Dss::new();
    for line in lines(&[&H, &[GEN], &PF, &[relay, dyn_entry(5).as_str(), "Solve"]]) {
        live.command(&line);
    }
    let ckt = live.circuit().expect("circuit");
    let line_l1 = ckt
        .ckt_elements
        .iter()
        .find_map(|&r| {
            let class = &live.classes[r.class_ord()];
            let name = format!(
                "{}.{}",
                class.props.class_name(),
                class.arena[r.index()].data().name()
            );
            name.eq_ignore_ascii_case("Line.l1").then(|| {
                class
                    .arena
                    .try_ckt_elem(r.index())
                    .expect("a circuit element")
            })
        })
        .expect("Line.l1");
    assert!(
        !line_l1.cd().conductor_closed(1, 1),
        "the relay opens Line.l1 on the live Generator"
    );
    std::fs::remove_dir_all(&dir).ok();
}

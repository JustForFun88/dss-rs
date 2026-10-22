//! A `DynamicExp` whose memory cannot hold what its host integrates: no state
//! variables (`NVariables=0`), fewer rows than the equations use, or a
//! `DynOut` that names fewer outputs than the host integrates. Every host
//! refuses the dynamics solve with error 482 before the first step, and the
//! variable and `DynOut` reads of such an equation answer instead of failing.
//! A `DynOut` list longer than its two output slots, or with a name that is
//! not an output, is refused where it is written, with error 50009 or 50008,
//! and leaves `DynOut` as it was. A list that names one variable twice, or has
//! an empty element before a name, is refused at its token before either
//! check, and every string list refuses an empty or blank element before a
//! name, also in a JSON import, and the property form
//! `Class.name.Property=value` applies its value as `Edit` does.
//! `Save` writes `DynamicEq` before `DynOut`, so a saved host
//! reloads with its outputs. A re-link of `DynamicEq=` keeps `DynOut` only
//! when the new equation defines all its outputs, and carries the values and
//! the memory rows to the new equation by variable name: what the new equation
//! cannot hold waits for a later link, and a link to no equation keeps them
//! all for the next link. The last assignment of a variable is the one its
//! host applies, and a value that does not read as a number is refused. An
//! equation that omits `NVariables` holds 20 rows and integrates like the same
//! equation with its count written, and its host lists only the variables the
//! equation names.

use crate::elements::pc::dyneq_pce::DynInitValue;
use crate::exec::*;

use super::common::query;

/// The four `DynamicEq=` hosts, each a 1.5 MW-class unit on `wbus`.
const HOSTS: [(&str, &str); 4] = [
    (
        "WindGen",
        "New WindGen.e1 bus1=wbus phases=3 kv=0.69 kW=1500 kva=1800 conn=wye model=1 vwind=12",
    ),
    (
        "Generator",
        "New Generator.e1 bus1=wbus phases=3 kv=0.69 kW=1500 kva=1800 model=1",
    ),
    (
        "PVSystem",
        "New PVSystem.e1 bus1=wbus phases=3 kv=0.69 kVA=1800 Pmpp=1500 irradiance=1",
    ),
    (
        "Storage",
        "New Storage.e1 bus1=wbus phases=3 kv=0.69 kWrated=1500 kWhrated=6000 kva=1800 \
         kW=1000 %stored=50 state=discharging",
    ),
];

/// An equation with no state variables at all: `NVariables=0`, no `VarNames`.
const NO_STATE: &str = "New DynamicExp.de nvariables=0 expression=[]";

/// The bare equation: the 20 default rows, no `VarNames`, no expression.
const BARE: &str = "New DynamicExp.de";

/// The power-flow-solved deck: a stiff 0.69 kV source, a short line, the
/// `DynamicExp` lines `eq` (one per line), and the host `class` linked to
/// `DynamicExp.de` plus `edit` (`~` lines, one per line, or nothing). The
/// caller checks the errors the build recorded.
fn solved(class: &str, eq: &str, edit: Option<&str>) -> Dss {
    let host = HOSTS
        .iter()
        .find(|(c, _)| *c == class)
        .map(|(_, line)| *line)
        .expect("a DynamicEq host");
    let mut dss = Dss::new();
    for line in [
        "Clear",
        "New Circuit.dyneq basekv=0.69 phases=3 bus1=srcbus",
        "New Line.l1 bus1=srcbus bus2=wbus phases=3 r1=0.005 x1=0.02 length=1",
    ]
    .into_iter()
    .chain(eq.lines())
    {
        dss.command(line);
    }
    dss.command(&format!("{host} DynamicEq=de"));
    for line in edit.unwrap_or_default().lines() {
        dss.command(line);
    }
    for line in ["Set voltagebases=[0.69]", "Calcvoltagebases", "Solve"] {
        dss.command(line);
    }
    dss
}

/// Enter dynamics on a deck that solved cleanly, then `Solve` one step and
/// assert it was refused with exactly `refusal` (error 482, solution aborted,
/// the clock not advanced).
fn assert_dynamics_refused(mut dss: Dss, refusal: &str) {
    assert!(dss.errors().is_empty(), "power flow: {:?}", dss.errors());
    dss.command("Set mode=dynamic stepsize=0.001 number=1");
    assert!(
        dss.errors().is_empty(),
        "entering dynamics: {:?}",
        dss.errors()
    );
    let t0 = dss.circuit().expect("circuit").solution.t;
    dss.command("Solve");
    let errors = dss.errors();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].code, Some(482));
    assert_eq!(
        errors[0].message,
        format!("Error Encountered in Solve: {refusal}")
    );
    let sol = &dss.circuit().expect("circuit").solution;
    assert!(sol.solution_abort, "the refusal aborts the solution");
    assert_eq!(sol.t, t0, "no step ran");
}

fn no_state_refusal(class: &str) -> String {
    format!(
        "{class}.e1: DynamicExp.de declares no state variables (NVariables=0). Set \
         NVariables to the number of its VarNames."
    )
}

#[test]
fn windgen_dynamic_eq_without_state_variables_refuses_the_dynamics_solve() {
    assert_dynamics_refused(
        solved("WindGen", NO_STATE, None),
        &no_state_refusal("WindGen"),
    );
}

#[test]
fn generator_dynamic_eq_without_state_variables_refuses_the_dynamics_solve() {
    assert_dynamics_refused(
        solved("Generator", NO_STATE, None),
        &no_state_refusal("Generator"),
    );
}

#[test]
fn pvsystem_dynamic_eq_without_state_variables_refuses_the_dynamics_solve() {
    assert_dynamics_refused(
        solved("PVSystem", NO_STATE, None),
        &no_state_refusal("PVSystem"),
    );
}

#[test]
fn storage_dynamic_eq_without_state_variables_refuses_the_dynamics_solve() {
    assert_dynamics_refused(
        solved("Storage", NO_STATE, None),
        &no_state_refusal("Storage"),
    );
}

/// `NVariables` defaults to 20: an equation that never wrote it reads back 20,
/// with or without `VarNames`, and so does a `like=` copy, which copies
/// nothing. A written count reads back as written, 0 included. The first and
/// the third deck are the `props_roundtrip` scenarios `dynamicexp_default` and
/// `dynamicexp_makelike`, whose 0.14.5 capture prints 0.
#[test]
fn an_omitted_nvariables_reads_back_twenty() {
    let cases: [(&[&str], &str); 5] = [
        (&["New DynamicExp.de"], "20"),
        (
            &["New DynamicExp.de varnames=[a b] expression=[a dt = b]"],
            "20",
        ),
        (
            &[
                "New DynamicExp.base nvariables=2 varnames=[a b] expression=[a dt = b]",
                "New DynamicExp.de like=base",
            ],
            "20",
        ),
        (&["New DynamicExp.de nvariables=0"], "0"),
        (&["New DynamicExp.de nvariables=6"], "6"),
    ];
    for (lines, want) in cases {
        let mut dss = Dss::new();
        dss.command("clear");
        dss.command("new circuit.propsprobe");
        for line in lines {
            dss.command(line);
        }
        assert_eq!(
            query(&mut dss, "DynamicExp.de.NVariables"),
            want,
            "{lines:?}"
        );
    }
}

/// `Save` writes `NVariables` only when the deck wrote it, so each saved
/// equation reloads with the count it had: 20 when omitted, 0 when written.
#[test]
fn save_reloads_an_omitted_nvariables_as_the_default() {
    let dir = std::env::temp_dir().join(format!("dss_nvars_save_{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    let mut dss = Dss::new();
    for line in [
        "Clear",
        "New Circuit.nvsave basekv=0.69",
        "New DynamicExp.de varnames=[a b] expression=[a dt = b]",
        "New DynamicExp.dz nvariables=0 varnames=[a b] expression=[a dt = b]",
    ] {
        dss.command(line);
    }
    dss.command(&format!(
        "Save circuit dir=\"{}\"",
        dir.to_string_lossy().replace('\\', "/")
    ));
    let saved = std::fs::read_to_string(dir.join("DynamicExp.dss")).expect("saved class file");
    std::fs::remove_dir_all(&dir).ok();
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let line = |name: &str| {
        saved
            .lines()
            .find(|l| l.contains(&format!("\"DynamicExp.{name}\"")))
            .unwrap_or_else(|| panic!("no {name} in {saved}"))
            .to_ascii_lowercase()
    };
    assert!(!line("de").contains("nvariables"), "{saved}");
    assert!(line("dz").contains("nvariables=0"), "{saved}");

    let mut reloaded = Dss::new();
    reloaded.command("Clear");
    reloaded.command("New Circuit.nvreload basekv=0.69");
    for l in saved.lines() {
        reloaded.command(l);
    }
    assert!(reloaded.errors().is_empty(), "{:?}", reloaded.errors());
    assert_eq!(query(&mut reloaded, "DynamicExp.de.NVariables"), "20");
    assert_eq!(query(&mut reloaded, "DynamicExp.dz.NVariables"), "0");
}

/// The equation `linked` gives `class`, named `name`, with `count`
/// (`nvariables=N ` or nothing): a swing equation for a machine, the filter
/// current of a grid-following unit for an inverter.
fn equation(class: &str, name: &str, count: &str) -> String {
    if matches!(class, "Generator" | "WindGen") {
        format!(
            "New DynamicExp.{name} {count}varnames=[Speed Mass PShaft Pterm Damp theta] \
             expression=[Speed dt = -1 Mass / ( Pterm Damp Speed * + Pshaft - ) *; \
             theta dt = Speed]"
        )
    } else {
        format!(
            "New DynamicExp.{name} {count}varnames=[it vdc modul vac] \
             expression=[it dt = 1 0.61059E-3 / ( -0.230187 it * modul vdc * + vac - ) *]"
        )
    }
}

/// The constant initial values `linked` gives the machine equation.
const MACHINE_CONSTANTS: &str = "Damp=0 Speed=0.05 Mass=(3.5 2 * 1800000 376.99112 / *)";

/// The `~` line [`linked`] writes after the host line: a machine starts its
/// swing equation off its rest point (`Speed=0.05`), an inverter loads its
/// filter state from its model.
fn linked_edit(class: &str) -> &'static str {
    if matches!(class, "Generator" | "WindGen") {
        "~ Damp=0 PShaft=P0 Pterm=P Speed=0.05 theta=Edp \
         Mass=(3.5 2 * 1800000 376.99112 / *) DynOut=[Speed theta]"
    } else {
        "~ it=imag vdc=kvdc modul=mod vac=vmag DynOut=[it]"
    }
}

/// One host linked to the [`equation`] `de`, with `nvariables=` written
/// (`Some`) or omitted (`None`), written with [`linked_edit`], its power flow
/// solved.
fn linked(class: &str, nvariables: Option<usize>) -> Dss {
    let count = nvariables.map_or(String::new(), |n| format!("nvariables={n} "));
    linked_with(class, &equation(class, "de", &count), linked_edit(class))
}

/// One host linked to the equation `de` that the lines `eq` define, written
/// with the `~` lines `edit`, its power flow solved without an error. The
/// PVSystem runs on a 12.47 kV feeder, where its filter is stable.
fn linked_with(class: &str, eq: &str, edit: &str) -> Dss {
    let dss = if class == "PVSystem" {
        let mut dss = Dss::new();
        for line in [
            "Clear",
            "New Circuit.pvdyneq basekv=12.47 pu=1.0 phases=3 bus1=sourcebus \
             mvasc3=20000 mvasc1=21000",
            "New Line.l1 bus1=sourcebus bus2=pvbus length=0.5 units=km r1=0.1 x1=0.3",
        ]
        .into_iter()
        .chain(eq.lines())
        .chain([
            "New PVSystem.e1 phases=3 bus1=pvbus kv=12.47 kVA=600 Pmpp=500 irradiance=1 \
             %cutin=0.1 %cutout=0.1 kvar=0 DynamicEq=de",
        ])
        .chain(edit.lines())
        .chain(["Set voltagebases=[12.47]", "Calcvoltagebases", "Solve"])
        {
            dss.command(line);
        }
        dss
    } else {
        solved(class, eq, Some(edit))
    };
    assert!(dss.errors().is_empty(), "{class}: {:?}", dss.errors());
    dss
}

/// Enter dynamics and run 51 steps without an error.
fn run_dynamics(dss: &mut Dss, class: &str) {
    for line in [
        "Set mode=dynamic stepsize=0.001 number=1",
        "Solve",
        "Solve number=50",
    ] {
        dss.command(line);
    }
    assert!(dss.errors().is_empty(), "{class}: {:?}", dss.errors());
}

/// [`linked`] after 51 dynamics steps. A `refused` edit, a `DynOut=` list
/// whose one unresolved name is `zzz`, runs after the power flow and must
/// record exactly that error.
fn integrated(class: &str, nvariables: Option<usize>, refused: Option<&str>) -> Dss {
    let mut dss = linked(class, nvariables);
    if let Some(edit) = refused {
        dss.command(edit);
        let got: Vec<_> = dss
            .errors()
            .iter()
            .map(|e| (e.code, e.message.clone()))
            .collect();
        assert_eq!(got, [(Some(50008), unresolved("zzz"))], "{class} {edit}");
        dss.errors.clear();
    }
    run_dynamics(&mut dss, class);
    dss
}

/// An equation that names its variables and omits `NVariables` integrates on
/// every host, step for step like the same equation with the count written:
/// the same node voltages, host powers and currents, and the same variables,
/// by name and value.
#[test]
fn every_host_integrates_an_omitted_nvariables_like_the_written_count() {
    for (class, n) in [
        ("Generator", 6),
        ("WindGen", 6),
        ("PVSystem", 4),
        ("Storage", 4),
    ] {
        let mut written = integrated(class, Some(n), None);
        let mut omitted = integrated(class, None, None);
        let t = |dss: &Dss| dss.circuit().expect("circuit").solution.t;
        assert!((t(&omitted) - 0.051).abs() < 1e-12, "{class}: t");
        assert_eq!(t(&omitted), t(&written), "{class}: t");
        let node_v = |dss: &Dss| dss.circuit().expect("circuit").solution.node_v.clone();
        assert_eq!(node_v(&omitted), node_v(&written), "{class}: node voltages");

        let elem = format!("{class}.e1");
        let host = |dss: &mut Dss| {
            dss.snapshot_elements()
                .into_iter()
                .find(|e| e.name.eq_ignore_ascii_case(&elem))
                .map(|e| (e.powers, e.currents))
                .expect("host snapshot")
        };
        assert_eq!(host(&mut omitted), host(&mut written), "{class}: host");

        let w_names = written.element_variable_names(&elem).expect("names");
        let w_values = written.element_variables(&elem).expect("values");
        let o_names = omitted.element_variable_names(&elem).expect("names");
        let o_values = omitted.element_variables(&elem).expect("values");
        assert_eq!(w_names.len(), 2 * n, "{class}: {w_names:?}");
        assert_eq!(o_names, w_names, "{class}");
        assert_eq!(o_values, w_values, "{class}");

        // The comparison is not between two idle runs: the first state is
        // live, finite and nonzero, and a machine's speed has left its 0.05.
        let first = o_values[0];
        assert!(
            first.is_finite() && first != 0.0 && first != 0.05,
            "{class}: {first}"
        );
    }
}

/// The default holds 20 rows: an equation with 21 `VarNames` integrates while
/// its equations stay in the first 20, and is refused once one reads the 21st.
/// The reading equation comes last, since a bare operand in front of `;`
/// compiles to nothing.
#[test]
fn an_omitted_nvariables_holds_twenty_rows() {
    let names: Vec<String> = (1..=21).map(|i| format!("v{i}")).collect();
    let names = names.join(" ");
    let inside =
        format!("New DynamicExp.de varnames=[{names}] expression=[v2 dt = v1; v1 dt = v20]");
    let mut dss = solved("Generator", &inside, Some("~ DynOut=[v1 v2]"));
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "DynamicExp.de.NVariables"), "20");
    dss.command("Set mode=dynamic stepsize=0.001 number=1");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!((dss.circuit().expect("circuit").solution.t - 0.001).abs() < 1e-15);

    let past = format!("New DynamicExp.de varnames=[{names}] expression=[v2 dt = v1; v1 dt = v21]");
    assert_dynamics_refused(
        solved("Generator", &past, Some("~ DynOut=[v1 v2]")),
        "Generator.e1: DynamicExp.de holds the default NVariables=20 but its equations use \
         21 state variables. Set NVariables to the number of its VarNames.",
    );
}

/// The integration reads the outputs `DynOut` names, so an equation with
/// state but without `DynOut` cannot be integrated either.
#[test]
fn a_dynamic_eq_without_dynout_refuses_the_dynamics_solve() {
    let eq = "New DynamicExp.de nvariables=2 varnames=[Speed theta] \
              expression=[Speed dt = Speed; theta dt = Speed]";
    assert_dynamics_refused(
        solved("Generator", eq, None),
        "Generator.e1: DynOut is not set. Name the output variables of DynamicExp.de \
         with DynOut=[...].",
    );
}

/// An `NVariables` smaller than the variables the equations use leaves the
/// angle without a memory row.
#[test]
fn a_dynamic_eq_with_too_few_rows_refuses_the_dynamics_solve() {
    let eq = "New DynamicExp.de nvariables=1 varnames=[Speed theta] \
              expression=[Speed dt = Speed; theta dt = Speed]";
    assert_dynamics_refused(
        solved("Generator", eq, Some("~ DynOut=[Speed theta]")),
        "Generator.e1: DynamicExp.de declares NVariables=1 but its equations use 2 state \
         variables. Set NVariables to the number of its VarNames.",
    );
}

/// An initial value (`Edp`, `P0`) assigned to a variable past the memory has
/// no row to seed. Entering dynamics skips it, and the solve refuses the
/// equation before its first step.
fn assert_init_values_past_the_memory_refused(class: &str) {
    let eq = "New DynamicExp.de nvariables=1 varnames=[Speed theta pm] \
              expression=[Speed dt = Speed]";
    assert_dynamics_refused(
        solved(class, eq, Some("~ DynOut=[Speed] theta=Edp pm=P0")),
        &format!(
            "{class}.e1: DynamicExp.de declares NVariables=1 but its equations use 3 state \
             variables. Set NVariables to the number of its VarNames."
        ),
    );
}

#[test]
fn generator_init_values_without_a_memory_row_refuse_the_dynamics_solve() {
    assert_init_values_past_the_memory_refused("Generator");
}

#[test]
fn windgen_init_values_without_a_memory_row_refuse_the_dynamics_solve() {
    assert_init_values_past_the_memory_refused("WindGen");
}

fn two_rows_refusal(class: &str) -> String {
    format!(
        "{class}.e1: DynamicExp.de declares NVariables=1 but its equations use 2 state \
         variables. Set NVariables to the number of its VarNames."
    )
}

/// The equations may read a variable that `DynOut` does not name: `a dt = b`
/// reads `b` past a one-row memory, and every host refuses the solve.
#[test]
fn every_host_refuses_an_equation_that_reads_past_its_memory() {
    let eq = "New DynamicExp.de nvariables=1 varnames=[a b] expression=[a dt = b]";
    for (class, _) in HOSTS {
        assert_dynamics_refused(
            solved(class, eq, Some("~ DynOut=[a]")),
            &two_rows_refusal(class),
        );
    }
}

/// Each step loads `Vmag` into `vac`. With `vac` past a one-row memory the
/// value has no row, though the equation and `DynOut` stay inside it, and
/// every host refuses the solve.
#[test]
fn every_host_refuses_a_calculated_value_past_its_memory() {
    let eq = "New DynamicExp.de nvariables=1 varnames=[a vac] expression=[a dt = a]";
    for (class, _) in HOSTS {
        assert_dynamics_refused(
            solved(class, eq, Some("~ DynOut=[a] vac=Vmag")),
            &two_rows_refusal(class),
        );
    }
}

/// The same equation with a row per variable integrates: the refusal is about
/// the memory, not about linking an equation.
#[test]
fn a_dynamic_eq_with_its_memory_integrates() {
    let eq = "New DynamicExp.de nvariables=2 varnames=[Speed theta] \
              expression=[Speed dt = Speed; theta dt = Speed]";
    let mut dss = solved("Generator", eq, Some("~ DynOut=[Speed theta]"));
    dss.command("Set mode=dynamic stepsize=0.001 number=1");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!((dss.circuit().expect("circuit").solution.t - 0.001).abs() < 1e-15);
}

/// A WindGen linked to an equation without variables reports its 22 native
/// variables, by name and value, exactly as with no equation — the layout the
/// other three hosts already keep.
#[test]
fn windgen_dynamic_eq_without_variables_keeps_the_classic_layout() {
    let mut plain = Dss::new();
    let mut linked = solved("WindGen", NO_STATE, None);
    assert!(linked.errors().is_empty(), "{:?}", linked.errors());
    let host = HOSTS[0].1;
    for line in [
        "Clear",
        "New Circuit.dyneq basekv=0.69 phases=3 bus1=srcbus",
        "New Line.l1 bus1=srcbus bus2=wbus phases=3 r1=0.005 x1=0.02 length=1",
        host,
        "Set voltagebases=[0.69]",
        "Calcvoltagebases",
        "Solve",
    ] {
        plain.command(line);
    }
    assert!(plain.errors().is_empty(), "{:?}", plain.errors());

    let names = linked.element_variable_names("WindGen.e1").expect("names");
    let values = linked.element_variables("WindGen.e1").expect("values");
    assert_eq!(names.len(), 22);
    assert_eq!(names[0], "userTrip");
    assert_eq!(names[21], "s");
    assert_eq!(
        Some(names),
        plain.element_variable_names("WindGen.e1"),
        "names"
    );
    assert_eq!(
        Some(values),
        plain.element_variables("WindGen.e1"),
        "values"
    );
}

/// An `NVariables` larger than its `VarNames` leaves memory rows without a
/// name: every host reports them by the empty string, after its own names.
#[test]
fn every_host_names_an_unnamed_memory_row_by_the_empty_string() {
    for (class, _) in HOSTS {
        let mut dss = solved(class, "New DynamicExp.de nvariables=2 expression=[]", None);
        assert!(dss.errors().is_empty(), "{class}: {:?}", dss.errors());
        let elem = format!("{class}.e1");
        let names = dss.element_variable_names(&elem).expect("names");
        let values = dss.element_variables(&elem).expect("values");
        assert_eq!(names[..4], ["", "", "", ""], "{class}: {names:?}");
        assert_eq!(values[..4], [0.0; 4], "{class}: {values:?}");
    }
}

/// `DynOut` has two output slots, speed and angle, and an inverter reads the
/// first as its current. A longer list is refused whole with error 50009, even
/// when a name in it does not resolve, and the slots keep their outputs.
#[test]
fn every_host_refuses_a_dynout_longer_than_two_outputs() {
    let eq = "New DynamicExp.de nvariables=3 varnames=[a b c] \
              expression=[a dt = b; b dt = c; c dt = a]";
    for (class, _) in HOSTS {
        let mut dss = solved(class, eq, Some("~ DynOut=[c a]"));
        assert!(dss.errors().is_empty(), "{class}: {:?}", dss.errors());
        let elem = format!("{class}.e1");
        assert_eq!(
            query(&mut dss, &format!("{elem}.DynOut")),
            "[c, a]",
            "{class}"
        );
        for (list, given) in [("a b c", "[a, b, c]"), ("b c nosuch", "[b, c, nosuch]")] {
            let before = dss.errors().len();
            dss.command(&format!("Edit {elem} DynOut=[{list}]"));
            let errors = &dss.errors()[before..];
            assert_eq!(errors.len(), 1, "{class} {list}: {errors:?}");
            assert_eq!(errors[0].code, Some(50009), "{class} {list}");
            assert_eq!(
                errors[0].message,
                format!(
                    "DynOut takes at most 2 output variables, but 3 were given: {given}. \
                     DynOut is unchanged."
                ),
                "{class} {list}"
            );
            assert_eq!(
                query(&mut dss, &format!("{elem}.DynOut")),
                "[c, a]",
                "{class} {list}"
            );
        }
    }
}

/// `DynOut` names its outputs through `VarNames`, so an equation without names
/// reads back no outputs (the empty value) after the `DynOut=` that could not
/// resolve them, with `NVariables=0` and with the count omitted.
#[test]
fn every_host_reads_back_no_dynout_from_an_equation_without_var_names() {
    for (class, _) in HOSTS {
        for eq in [NO_STATE, BARE] {
            let mut dss = solved(class, eq, Some("~ DynOut=[Speed theta]"));
            let errors: Vec<_> = dss.errors().iter().map(|e| e.code).collect();
            assert_eq!(errors, [Some(50008), Some(50008)], "{class} {eq}");
            assert_eq!(
                query(&mut dss, &format!("{class}.e1.DynOut")),
                "",
                "{class} {eq}"
            );
        }
    }
}

/// The error a `DynOut=` list records for a name that is not an output of the
/// linked equation.
fn unresolved(name: &str) -> String {
    format!(
        "DynamicExp variable \"{name}\" not found or not defined as an output. DynOut is unchanged."
    )
}

/// A `DynOut=` list with a name that is not an output is refused whole, so a
/// host whose list never resolved has no output to integrate and refuses the
/// dynamics solve, whatever its memory holds: the bare equation, whose 20
/// default rows hold no named variable, a `like=` copy, which copies nothing,
/// and a written count whose one name is not an output.
#[test]
fn every_host_refuses_a_dynout_that_names_no_output_of_its_equation() {
    let decks: [(&str, &str, &[&str]); 3] = [
        (BARE, "~ DynOut=[Speed theta]", &["speed", "theta"]),
        (
            "New DynamicExp.base nvariables=2 varnames=[a b] expression=[a dt = b]\n\
             New DynamicExp.de like=base",
            "~ DynOut=[a b]",
            &["a", "b"],
        ),
        (
            "New DynamicExp.de nvariables=2 varnames=[a b] expression=[a dt = b]",
            "~ DynOut=[zzz]",
            &["zzz"],
        ),
    ];
    for (class, _) in HOSTS {
        for (eq, edit, names) in decks {
            let mut dss = solved(class, eq, Some(edit));
            let mut want: Vec<(Option<u32>, String)> = Vec::new();
            if eq.contains("like=") {
                want.push((None, "\"Like\" is not implemented for DynamicExp.".into()));
            }
            want.extend(names.iter().map(|n| (Some(50008), unresolved(n))));
            let got: Vec<_> = dss
                .errors()
                .iter()
                .map(|e| (e.code, e.message.clone()))
                .collect();
            assert_eq!(got, want, "{class} {edit}");
            assert_eq!(
                query(&mut dss, &format!("{class}.e1.DynOut")),
                "",
                "{class} {edit}"
            );
            dss.errors.clear();
            assert_dynamics_refused(
                dss,
                &format!(
                    "{class}.e1: DynOut is not set. Name the output variables of \
                     DynamicExp.de with DynOut=[...]."
                ),
            );
        }
    }
}

/// An empty `DynOut=[]` names no output either.
#[test]
fn every_host_refuses_an_empty_dynout() {
    for (class, _) in HOSTS {
        let mut dss = solved(class, BARE, Some("~ DynOut=[]"));
        assert_eq!(
            query(&mut dss, &format!("{class}.e1.DynOut")),
            "",
            "{class}"
        );
        assert_dynamics_refused(
            dss,
            &format!(
                "{class}.e1: DynOut is not set. Name the output variables of DynamicExp.de \
                 with DynOut=[...]."
            ),
        );
    }
}

/// A machine integrates two outputs, its speed and its angle, so a `DynOut`
/// that names one leaves it without an angle and the dynamics solve is
/// refused, with the count written or omitted. An inverter integrates one
/// output, so the same list drives it. The list reads back as written.
#[test]
fn a_machine_refuses_a_dynout_with_one_output() {
    for count in ["nvariables=2 ", ""] {
        let eq = format!(
            "New DynamicExp.de {count}varnames=[Speed theta] \
             expression=[Speed dt = theta; theta dt = Speed]"
        );
        for (class, _) in HOSTS {
            let mut dss = solved(class, &eq, Some("~ DynOut=[Speed]"));
            assert!(dss.errors().is_empty(), "{class}: {:?}", dss.errors());
            let elem = format!("{class}.e1");
            assert_eq!(
                query(&mut dss, &format!("{elem}.DynOut")),
                "[speed]",
                "{class} {count}"
            );
            if matches!(class, "Generator" | "WindGen") {
                assert_dynamics_refused(
                    dss,
                    &format!(
                        "{elem}: DynOut names one output, but a machine integrates two, its \
                         speed and its angle. Name both output variables of DynamicExp.de \
                         with DynOut=[...]."
                    ),
                );
            } else {
                dss.command("Set mode=dynamic stepsize=0.001 number=1");
                dss.command("Solve");
                assert!(
                    dss.errors().is_empty(),
                    "{class} {count}: {:?}",
                    dss.errors()
                );
                let t = dss.circuit().expect("circuit").solution.t;
                assert!((t - 0.001).abs() < 1e-15, "{class} {count}: t = {t}");
            }
        }
    }
}

/// A `DynOut=` list with an unresolved name leaves the outputs set before
/// untouched: the host reads them back and integrates them step for step as
/// if the refused list had never been written.
#[test]
fn every_host_keeps_its_dynout_through_a_list_with_an_unresolved_name() {
    for (class, n, kept, refused) in [
        ("Generator", 6, "[speed, theta]", "Speed zzz"),
        ("WindGen", 6, "[speed, theta]", "Speed zzz"),
        ("PVSystem", 4, "[it]", "zzz"),
        ("Storage", 4, "[it]", "zzz"),
    ] {
        let edit = format!("Edit {class}.e1 DynOut=[{refused}]");
        let mut plain = integrated(class, Some(n), None);
        let mut edited = integrated(class, Some(n), Some(&edit));
        let elem = format!("{class}.e1");
        assert_eq!(
            query(&mut edited, &format!("{elem}.DynOut")),
            kept,
            "{class}"
        );
        let host = |dss: &mut Dss| {
            dss.snapshot_elements()
                .into_iter()
                .find(|e| e.name.eq_ignore_ascii_case(&elem))
                .map(|e| (e.powers, e.currents))
                .expect("host snapshot")
        };
        assert_eq!(host(&mut edited), host(&mut plain), "{class}");
        assert_eq!(
            edited.element_variables(&elem),
            plain.element_variables(&elem),
            "{class}"
        );
    }
}

/// The outputs [`linked`] names, as `DynOut` reads them back.
fn linked_outputs(class: &str) -> &'static str {
    if matches!(class, "Generator" | "WindGen") {
        "[speed, theta]"
    } else {
        "[it]"
    }
}

/// A re-link of `DynamicEq=` resolves the outputs `DynOut` names against the
/// new equation. An equation without all of them, the bare one, a `like=`
/// copy, a written count without `VarNames`, the same names with no equation
/// for the last output, or a smaller equation with other names, leaves
/// `DynOut` empty, and the host refuses the dynamics solve. The values
/// [`linked`] loads from its model follow their variables by name, so the
/// smaller equation, which lacks them, pairs none, and one that keeps a
/// variable of them on its second row is refused for its one-row memory
/// first.
#[test]
fn every_host_refuses_a_relink_to_an_equation_without_its_outputs() {
    for (class, n, short) in [
        (
            "Generator",
            6,
            "varnames=[Speed Mass PShaft Pterm Damp theta] \
             expression=[Speed dt = -1 Mass / ( Pterm Damp Speed * + Pshaft - ) *]",
        ),
        (
            "WindGen",
            6,
            "varnames=[Speed Mass PShaft Pterm Damp theta] \
             expression=[Speed dt = -1 Mass / ( Pterm Damp Speed * + Pshaft - ) *]",
        ),
        (
            "PVSystem",
            4,
            "varnames=[it vdc modul vac] expression=[vdc dt = it]",
        ),
        (
            "Storage",
            4,
            "varnames=[it vdc modul vac] expression=[vdc dt = it]",
        ),
    ] {
        let elem = format!("{class}.e1");
        let not_set = format!(
            "{elem}: DynOut is not set. Name the output variables of DynamicExp.b with \
             DynOut=[...]."
        );
        let small = format!(
            "{elem}: DynamicExp.b declares NVariables=1 but its equations use 2 state \
             variables. Set NVariables to the number of its VarNames."
        );
        let kept = if matches!(class, "Generator" | "WindGen") {
            "pshaft"
        } else {
            "vac"
        };
        let decks = [
            ("New DynamicExp.b".to_string(), &not_set),
            ("New DynamicExp.b like=de".to_string(), &not_set),
            (format!("New DynamicExp.b nvariables={n}"), &not_set),
            (format!("New DynamicExp.b nvariables={n} {short}"), &not_set),
            (
                "New DynamicExp.b nvariables=1 varnames=[x] expression=[x dt = x]".to_string(),
                &not_set,
            ),
            (
                format!("New DynamicExp.b nvariables=1 varnames=[x {kept}] expression=[x dt = x]"),
                &small,
            ),
        ];
        for (eq, refusal) in decks {
            let mut dss = linked(class, Some(n));
            assert_eq!(
                query(&mut dss, &format!("{elem}.DynOut")),
                linked_outputs(class),
                "{class}"
            );
            dss.command(&eq);
            dss.command(&format!("Edit {elem} DynamicEq=b"));
            let mut want: Vec<(Option<u32>, String)> = Vec::new();
            if eq.contains("like=") {
                want.push((None, "\"Like\" is not implemented for DynamicExp.".into()));
            }
            let got: Vec<_> = dss
                .errors()
                .iter()
                .map(|e| (e.code, e.message.clone()))
                .collect();
            assert_eq!(got, want, "{class} {eq}");
            assert_eq!(
                query(&mut dss, &format!("{elem}.DynOut")),
                "",
                "{class} {eq}"
            );
            dss.errors.clear();
            assert_dynamics_refused(dss, refusal);
        }
    }
}

/// A re-link to an equation that defines the same outputs keeps `DynOut`: a
/// copy of the equation integrates step for step like the run that never
/// re-linked. Writing the constants again changes nothing. A `DynOut=` list
/// written after `DynamicEq=` on the re-link's line names the outputs of the
/// new equation.
#[test]
fn every_host_keeps_its_dynout_through_a_relink_to_an_equation_with_its_outputs() {
    for (class, n) in [
        ("Generator", 6),
        ("WindGen", 6),
        ("PVSystem", 4),
        ("Storage", 4),
    ] {
        let elem = format!("{class}.e1");
        let machine = matches!(class, "Generator" | "WindGen");
        let mut plain = integrated(class, Some(n), None);
        let mut relinked = linked(class, Some(n));
        relinked.command(&equation(class, "copy", &format!("nvariables={n} ")));
        relinked.command(&format!("Edit {elem} DynamicEq=copy"));
        assert_eq!(
            query(&mut relinked, &format!("{elem}.DynOut")),
            linked_outputs(class),
            "{class}"
        );
        if machine {
            relinked.command(&format!("Edit {elem} {MACHINE_CONSTANTS}"));
        }
        assert!(
            relinked.errors().is_empty(),
            "{class}: {:?}",
            relinked.errors()
        );
        run_dynamics(&mut relinked, class);
        let host = |dss: &mut Dss| {
            dss.snapshot_elements()
                .into_iter()
                .find(|e| e.name.eq_ignore_ascii_case(&elem))
                .map(|e| (e.powers, e.currents))
                .expect("host snapshot")
        };
        assert_eq!(host(&mut relinked), host(&mut plain), "{class}");
        let values = relinked.element_variables(&elem).expect("values");
        assert_eq!(
            Some(values.clone()),
            plain.element_variables(&elem),
            "{class}"
        );
        assert!(
            values[0].is_finite() && values[0] != 0.0,
            "{class}: {values:?}"
        );

        let (renamed, list, want) = if machine {
            (
                "New DynamicExp.renamed nvariables=2 varnames=[w th] \
                 expression=[w dt = th; th dt = w]",
                "[w th]",
                "[w, th]",
            )
        } else {
            (
                "New DynamicExp.renamed nvariables=1 varnames=[i] expression=[i dt = i]",
                "[i]",
                "[i]",
            )
        };
        let mut dss = linked(class, Some(n));
        dss.command(renamed);
        dss.command(&format!("Edit {elem} DynamicEq=renamed DynOut={list}"));
        assert!(dss.errors().is_empty(), "{class}: {:?}", dss.errors());
        assert_eq!(query(&mut dss, &format!("{elem}.DynOut")), want, "{class}");
    }
}

/// The four hosts with the variable count of their [`equation`].
const COUNTS: [(&str, usize); 4] = [
    ("Generator", 6),
    ("WindGen", 6),
    ("PVSystem", 4),
    ("Storage", 4),
];

/// The state variables of the [`equation`] of `class`, as the host stores
/// their names.
fn variables(class: &str) -> &'static [&'static str] {
    if matches!(class, "Generator" | "WindGen") {
        &["speed", "mass", "pshaft", "pterm", "damp", "theta"]
    } else {
        &["it", "vdc", "modul", "vac"]
    }
}

/// [`linked`] with `nvariables=n` written, where an inverter writes the
/// constant `vdc=8000`, the value its model loads for `kvdc`
/// ([`every_host_keeps_its_equation_memory_through_a_relink`] reads it). The
/// step loop reloads every calculated value, so this gives every host a row
/// that only its equation memory holds: a machine its mass, damping and
/// speed, an inverter its DC voltage.
fn with_constant(class: &str, n: usize) -> Dss {
    linked_with(
        class,
        &equation(class, "de", &format!("nvariables={n} ")),
        &linked_edit(class).replace("vdc=kvdc", "vdc=8000"),
    )
}

/// The [`equation`] of `class` named `name`, its count written and its
/// `VarNames` in another order, the expression unchanged.
fn reordered(class: &str, name: &str) -> String {
    let (n, names, other) = if matches!(class, "Generator" | "WindGen") {
        (
            6,
            "[Speed Mass PShaft Pterm Damp theta]",
            "[theta Speed Mass PShaft Pterm Damp]",
        )
    } else {
        (4, "[it vdc modul vac]", "[vac it vdc modul]")
    };
    let eq = equation(class, name, &format!("nvariables={n} "));
    assert!(eq.contains(names), "{eq}");
    eq.replace(names, other)
}

/// What a run leaves on `{class}.e1`: its powers and currents, its variable
/// names and their values.
type HostState = (
    Vec<num_complex::Complex64>,
    Vec<num_complex::Complex64>,
    Vec<String>,
    Vec<f64>,
);

fn host_state(dss: &mut Dss, class: &str) -> HostState {
    let elem = format!("{class}.e1");
    let (powers, currents) = dss
        .snapshot_elements()
        .into_iter()
        .find(|e| e.name.eq_ignore_ascii_case(&elem))
        .map(|e| (e.powers, e.currents))
        .expect("host snapshot");
    let names = dss.element_variable_names(&elem).expect("names");
    let values = dss.element_variables(&elem).expect("values");
    (powers, currents, names, values)
}

/// The value of the variable `var` in a [`HostState`].
fn variable(state: &HostState, var: &str) -> f64 {
    let i = state
        .2
        .iter()
        .position(|n| n == var)
        .unwrap_or_else(|| panic!("no {var} in {:?}", state.2));
    state.3[i]
}

/// The run is not idle: the first output state is finite and nonzero, and a
/// machine's speed has left its 0.05 start.
fn assert_live(state: &HostState, class: &str) {
    if matches!(class, "Generator" | "WindGen") {
        let speed = variable(state, "speed");
        assert!(
            speed.is_finite() && speed != 0.0 && speed != 0.05,
            "{class}: speed {speed}"
        );
    } else {
        let it = variable(state, "it");
        assert!(it.is_finite() && it != 0.0, "{class}: it {it}");
    }
}

/// The variable names of `{class}.e1` with no equation linked.
fn classic_names(class: &str) -> Vec<String> {
    let host = HOSTS
        .iter()
        .find(|(c, _)| *c == class)
        .map(|(_, line)| *line)
        .expect("a DynamicEq host");
    let mut dss = Dss::new();
    for line in [
        "Clear",
        "New Circuit.classic basekv=0.69 phases=3 bus1=srcbus",
        host,
    ] {
        dss.command(line);
    }
    assert!(dss.errors().is_empty(), "{class}: {:?}", dss.errors());
    dss.element_variable_names(&format!("{class}.e1"))
        .expect("names")
}

/// The entries of the `"DynInit"` member of an object's JSON export, in
/// their order, `None` without the member. A number is read back exactly,
/// whichever way the lane prints it.
fn dyn_init(json: &str) -> Option<Vec<(String, DynInitValue)>> {
    const KEY: &str = "\"DynInit\":{";
    let start = json.find(KEY)? + KEY.len();
    let body = &json[start..start + json[start..].find('}')?];
    let entry = |e: &str| {
        let (key, value) = e.split_once(':').expect("key:value");
        let value = match value.strip_prefix('"') {
            Some(text) => DynInitValue::Text(text.trim_end_matches('"').to_string()),
            None => DynInitValue::Number(value.parse().expect("a JSON number")),
        };
        (key.trim_matches('"').to_string(), value)
    };
    Some(body.split(',').map(entry).collect())
}

/// `Save circuit` of `dss` into a folder of its own: the line it writes for
/// `{class}.e1`, and a fresh engine that compiled the saved `Master.dss`.
fn save_and_reload(dss: &mut Dss, class: &str, tag: &str) -> (String, Dss) {
    save_and_reload_object(dss, class, "e1", tag)
}

/// [`save_and_reload`] for the object `{class}.{name}`.
fn save_and_reload_object(dss: &mut Dss, class: &str, name: &str, tag: &str) -> (String, Dss) {
    let dir = std::env::temp_dir().join(format!("dss_dyneq_{tag}_{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    let fwd = |p: &std::path::Path| p.to_string_lossy().replace('\\', "/");
    let before = dss.errors().len();
    dss.command(&format!("Save circuit dir=\"{}\"", fwd(&dir)));
    assert_eq!(dss.errors().len(), before, "{tag}: {:?}", dss.error_texts());
    let file = dir.join(format!("{class}.dss"));
    let text =
        std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{tag}: {}: {e}", file.display()));
    let needle = format!("\"{class}.{name}\"").to_ascii_lowercase();
    let line = text
        .lines()
        .find(|l| l.to_ascii_lowercase().contains(&needle))
        .unwrap_or_else(|| panic!("{tag}: no {class}.{name} in {text}"))
        .to_string();
    let mut back = Dss::new();
    back.command(&format!("compile \"{}\"", fwd(&dir.join("Master.dss"))));
    std::fs::remove_dir_all(&dir).ok();
    (line, back)
}

/// A re-link carries every value its host holds to the new equation by
/// variable name: a host linked to `de` and re-linked to `b`, which lists the
/// same variables in another order, integrates step for step like the host
/// linked to `b` from the start, because a value belongs to the variable it
/// was written to. Pairs kept by their index into the replaced equation would
/// load `P0` into the mass of a machine, `P` into its shaft power and the
/// angle of `Edp` into its damping.
#[test]
fn every_host_carries_its_calculated_values_by_name_through_a_relink() {
    for (class, n) in COUNTS {
        let elem = format!("{class}.e1");
        let mut relinked = linked(class, Some(n));
        relinked.command(&reordered(class, "b"));
        relinked.command(&format!("Edit {elem} DynamicEq=b"));
        assert!(
            relinked.errors().is_empty(),
            "{class}: {:?}",
            relinked.errors()
        );
        assert_eq!(
            query(&mut relinked, &format!("{elem}.DynOut")),
            linked_outputs(class),
            "{class}"
        );
        run_dynamics(&mut relinked, class);
        let mut from_start = linked_with(class, &reordered(class, "de"), linked_edit(class));
        run_dynamics(&mut from_start, class);

        let state = host_state(&mut relinked, class);
        assert_eq!(state, host_state(&mut from_start, class), "{class}");
        assert_eq!(state.2.len(), 2 * n, "{class}: {:?}", state.2);
        let first = if n == 6 { "theta" } else { "vac" };
        assert_eq!(state.2[0], first, "{class}: the reordered names");
        assert_live(&state, class);
    }
}

/// A re-link keeps the equation memory: a host re-linked to a copy of `de`,
/// or to `de` itself, without writing its constants again, integrates step
/// for step like the host that never re-linked. The copy defines the same
/// variables in the same order, so each memory row finds its variable again.
/// On [`with_constant`] every host holds a row only its memory keeps, so a
/// zeroed memory would leave a machine without its mass and an inverter
/// without its DC voltage.
#[test]
fn every_host_keeps_its_equation_memory_through_a_relink() {
    for (class, n) in COUNTS {
        let elem = format!("{class}.e1");
        if n == 4 {
            let mut loaded = integrated(class, Some(n), None);
            let vdc = variable(&host_state(&mut loaded, class), "vdc");
            assert_eq!(vdc, 8000.0, "{class}: the value kvdc loads");
        }
        let mut plain = with_constant(class, n);
        run_dynamics(&mut plain, class);
        let want = host_state(&mut plain, class);
        assert_live(&want, class);

        for target in ["copy", "de"] {
            let mut dss = with_constant(class, n);
            if target == "copy" {
                dss.command(&equation(class, "copy", &format!("nvariables={n} ")));
            }
            dss.command(&format!("Edit {elem} DynamicEq={target}"));
            assert!(dss.errors().is_empty(), "{class}: {:?}", dss.errors());
            run_dynamics(&mut dss, class);
            assert_eq!(
                host_state(&mut dss, class),
                want,
                "{class} DynamicEq={target}"
            );
        }
    }
}

/// A re-link lists and writes only the variables of the new equation: the
/// Generator re-linked to `w`, which names only its speed and its angle,
/// lists `Speed=0.05` and `theta=Edp` and none of its mass, damping and power
/// values, which it keeps for a later link
/// ([`every_host_keeps_what_an_equation_lacks_for_a_later_link`]), in its
/// variable list, in the initial values `Save` writes and in the JSON
/// `"DynInit"` tail. The saved deck reloads with its outputs.
#[test]
fn a_relink_lists_and_writes_only_the_variables_of_the_new_equation() {
    let elem = "Generator.e1";
    let mut dss = linked("Generator", Some(6));
    dss.command(
        "New DynamicExp.w nvariables=2 varnames=[Speed theta] \
         expression=[Speed dt = theta; theta dt = Speed]",
    );
    dss.command(&format!("Edit {elem} DynamicEq=w"));
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, &format!("{elem}.DynOut")), "[speed, theta]");
    let state = host_state(&mut dss, "Generator");
    assert_eq!(state.2, ["speed", "dspeed", "theta", "dtheta"]);
    assert_eq!(variable(&state, "speed"), 0.05);

    let json = dss.obj_to_json(elem, Default::default()).expect("json");
    assert_eq!(
        dyn_init(&json),
        Some(vec![
            ("speed".to_string(), DynInitValue::Number(0.05)),
            ("theta".to_string(), DynInitValue::Text("Edp".to_string())),
        ]),
        "{json}"
    );
    let (line, mut back) = save_and_reload(&mut dss, "Generator", "relink_w");
    assert!(line.ends_with(" speed=0.05 theta=Edp"), "{line}");
    let lower = line.to_ascii_lowercase();
    for kept in ["mass", "pshaft", "pterm", "damp"] {
        assert!(!lower.contains(&format!(" {kept}=")), "{line}");
    }
    assert!(back.errors().is_empty(), "{line}: {:?}", back.errors());
    assert_eq!(
        query(&mut back, &format!("{elem}.DynOut")),
        "[speed, theta]"
    );
}

/// The last assignment of a state variable is the one its host applies: a
/// constant written after a calculated value replaces it, and a calculated
/// value written after a constant replaces that. With 1 MW on the shaft
/// against a terminal power near 1.5 MW the swing equation decelerates the
/// machine, so its speed falls below its 0.05 start.
#[test]
fn a_constant_after_a_calculated_value_replaces_it() {
    let run = |class: &str, edit: &str| {
        let n = if class == "Generator" { 6 } else { 4 };
        let eq = equation(class, "de", &format!("nvariables={n} "));
        let mut dss = linked_with(class, &eq, edit);
        run_dynamics(&mut dss, class);
        host_state(&mut dss, class)
    };

    let edit = linked_edit("Generator");
    let constant = edit.replace("PShaft=P0", "PShaft=1000000");
    let written = run("Generator", &constant);
    let after = run("Generator", &format!("{edit}\n~ PShaft=1000000"));
    assert_eq!(after, written);
    assert_eq!(variable(&after, "pshaft"), 1_000_000.0);
    let speed = variable(&after, "speed");
    assert!(speed.is_finite() && speed < 0.05, "speed {speed}");
    let plain = run("Generator", edit);
    assert_ne!(variable(&plain, "pshaft"), 1_000_000.0);
    assert_eq!(run("Generator", &format!("{constant}\n~ PShaft=P0")), plain);

    let edit = linked_edit("Storage");
    let written = run("Storage", &edit.replace("vac=vmag", "vac=0.5"));
    let after = run("Storage", &format!("{edit}\n~ vac=0.5"));
    assert_eq!(after, written, "Storage");
    assert_eq!(variable(&after, "vac"), 0.5);
}

/// `Edit {class}.e1 DynamicEq=nosuch`, a link to no equation, which records
/// exactly the one error that the equation does not exist.
fn unlink(dss: &mut Dss, class: &str) {
    let elem = format!("{class}.e1");
    dss.command(&format!("Edit {elem} DynamicEq=nosuch"));
    let got: Vec<_> = dss
        .errors()
        .iter()
        .map(|e| (e.code, e.message.clone()))
        .collect();
    let want = format!("{elem}.DynamicEq: DynamicExp object \"nosuch\" not found.");
    assert_eq!(got, [(None, want)], "{class}");
    dss.errors.clear();
}

/// A link to no equation keeps the values and the memory for the next link:
/// a value belongs to the variable it was written to, and a failed link names
/// no variable that could replace it. Without an equation the host shows its
/// classic variables, `DynOut` reads back empty, the JSON export has no
/// `"DynInit"`, and `Save` writes no `DynamicEq`, no `DynOut` and no initial
/// value, so the saved deck reloads. Linked to `de` again the host needs its
/// `DynOut` again, and with it integrates step for step like the host that
/// was never unlinked.
#[test]
fn a_link_to_no_equation_keeps_the_values_for_the_next_link() {
    for (class, n) in COUNTS {
        let elem = format!("{class}.e1");
        let mut dss = with_constant(class, n);
        unlink(&mut dss, class);
        assert_eq!(query(&mut dss, &format!("{elem}.DynOut")), "", "{class}");
        assert_eq!(
            dss.element_variable_names(&elem).expect("names"),
            classic_names(class),
            "{class}"
        );
        let json = dss.obj_to_json(&elem, Default::default()).expect("json");
        assert_eq!(dyn_init(&json), None, "{class}: {json}");
        let (line, back) = save_and_reload(&mut dss, class, &format!("nosuch_{class}"));
        let lower = line.to_ascii_lowercase();
        assert!(
            !lower.contains("dynamiceq=") && !lower.contains("dynout="),
            "{line}"
        );
        for var in variables(class) {
            assert!(!lower.contains(&format!(" {var}=")), "{class}: {line}");
        }
        assert!(back.errors().is_empty(), "{class}: {:?}", back.errors());

        let mut relinked = with_constant(class, n);
        unlink(&mut relinked, class);
        relinked.command(&format!("Edit {elem} DynamicEq=de"));
        assert!(
            relinked.errors().is_empty(),
            "{class}: {:?}",
            relinked.errors()
        );
        assert_dynamics_refused(
            relinked,
            &format!(
                "{elem}: DynOut is not set. Name the output variables of DynamicExp.de \
                 with DynOut=[...]."
            ),
        );

        let outputs = if n == 6 { "[Speed theta]" } else { "[it]" };
        let mut relinked = with_constant(class, n);
        unlink(&mut relinked, class);
        relinked.command(&format!("Edit {elem} DynamicEq=de"));
        relinked.command(&format!("Edit {elem} DynOut={outputs}"));
        assert!(
            relinked.errors().is_empty(),
            "{class}: {:?}",
            relinked.errors()
        );
        run_dynamics(&mut relinked, class);
        let mut plain = with_constant(class, n);
        run_dynamics(&mut plain, class);
        let state = host_state(&mut relinked, class);
        assert_eq!(state, host_state(&mut plain, class), "{class}");
        assert_live(&state, class);
    }
}

/// The equations [`every_host_keeps_what_an_equation_lacks_for_a_later_link`]
/// passes a host through, each with the variables it names as the host
/// stores them: `lacking` names only the outputs of `class`, and `small`
/// names one more, past its memory. Both define the outputs, so `DynOut`
/// stays through the re-link.
fn passed_through(class: &str) -> [(&'static str, &'static str, &'static [&'static str]); 2] {
    if matches!(class, "Generator" | "WindGen") {
        [
            (
                "lacking",
                "New DynamicExp.lacking nvariables=2 varnames=[Speed theta] \
                 expression=[Speed dt = theta; theta dt = Speed]",
                &["speed", "theta"],
            ),
            (
                "small",
                "New DynamicExp.small nvariables=2 varnames=[theta Speed Mass] \
                 expression=[Speed dt = theta; theta dt = Speed]",
                &["theta", "speed", "mass"],
            ),
        ]
    } else {
        [
            (
                "lacking",
                "New DynamicExp.lacking nvariables=1 varnames=[it] expression=[it dt = it]",
                &["it"],
            ),
            (
                "small",
                "New DynamicExp.small nvariables=1 varnames=[it vdc] expression=[it dt = it]",
                &["it", "vdc"],
            ),
        ]
    }
}

/// A re-link keeps what the new equation cannot hold for a later link: a
/// machine's mass and an inverter's DC voltage do not change with the
/// equation the host passed through. On [`with_constant`] a host re-linked
/// to `lacking`, which lacks those constants, or to `small`, which holds one
/// of them past its memory, and linked to `de` again integrates step for step
/// like the host that never left `de`, and so does the host that passes from
/// either through a link to no equation before `de`, given its `DynOut`
/// again. `Save` while linked to either writes the values of that equation's
/// variables only, and the saved deck reloads without an error. Under
/// `small` that is the constant held past its memory too, so the reloaded
/// host linked to `de` starts it from the value written. A constant written
/// to `small` past its memory is the value the next link to `de` starts
/// from, as in the deck that wrote it to `de`.
#[test]
fn every_host_keeps_what_an_equation_lacks_for_a_later_link() {
    for (class, n) in COUNTS {
        let elem = format!("{class}.e1");
        let outputs = if n == 6 { "[Speed theta]" } else { "[it]" };
        let mut plain = with_constant(class, n);
        run_dynamics(&mut plain, class);
        let want = host_state(&mut plain, class);
        assert_live(&want, class);

        for (name, eq, named) in passed_through(class) {
            let mut dss = with_constant(class, n);
            dss.command(eq);
            dss.command(&format!("Edit {elem} DynamicEq={name}"));
            assert!(
                dss.errors().is_empty(),
                "{class} {name}: {:?}",
                dss.errors()
            );
            assert_eq!(
                query(&mut dss, &format!("{elem}.DynOut")),
                linked_outputs(class),
                "{class} {name}"
            );
            let (line, mut back) = save_and_reload(&mut dss, class, &format!("{name}_{class}"));
            let lower = line.to_ascii_lowercase();
            for var in variables(class) {
                assert_eq!(
                    lower.contains(&format!(" {var}=")),
                    named.contains(var),
                    "{class} {name} {var}: {line}"
                );
            }
            assert!(back.errors().is_empty(), "{line}: {:?}", back.errors());
            let (held, written) = if n == 6 {
                ("mass", 7.0 * (1_800_000.0 / 376.99112))
            } else {
                ("vdc", 8000.0)
            };
            if named.contains(&held) {
                back.command(&format!("Edit {elem} DynamicEq=de"));
                assert!(back.errors().is_empty(), "{line}: {:?}", back.errors());
                let names = back.element_variable_names(&elem).expect("names");
                let values = back.element_variables(&elem).expect("values");
                let at = names.iter().position(|v| v == held).expect(held);
                assert_eq!(values[at], written, "{class} {name}: {line}");
            }

            dss.command(&format!("Edit {elem} DynamicEq=de"));
            assert!(
                dss.errors().is_empty(),
                "{class} {name}: {:?}",
                dss.errors()
            );
            run_dynamics(&mut dss, class);
            assert_eq!(host_state(&mut dss, class), want, "{class} {name}");

            let mut dss = with_constant(class, n);
            dss.command(eq);
            dss.command(&format!("Edit {elem} DynamicEq={name}"));
            assert!(
                dss.errors().is_empty(),
                "{class} {name}: {:?}",
                dss.errors()
            );
            unlink(&mut dss, class);
            dss.command(&format!("Edit {elem} DynamicEq=de"));
            dss.command(&format!("Edit {elem} DynOut={outputs}"));
            assert!(
                dss.errors().is_empty(),
                "{class} {name} nosuch: {:?}",
                dss.errors()
            );
            run_dynamics(&mut dss, class);
            assert_eq!(
                host_state(&mut dss, class),
                want,
                "{class} {name} nosuch de"
            );
        }

        let (var, value) = if n == 6 {
            ("Mass", "40000")
        } else {
            ("vdc", "7000")
        };
        let edit = linked_edit(class)
            .replace("Mass=(3.5 2 * 1800000 376.99112 / *)", "Mass=40000")
            .replace("vdc=kvdc", "vdc=7000");
        let mut written = linked_with(
            class,
            &equation(class, "de", &format!("nvariables={n} ")),
            &edit,
        );
        run_dynamics(&mut written, class);
        let written = host_state(&mut written, class);
        assert_live(&written, class);
        assert_ne!(written, want, "{class}: {var}={value} moves the run");
        let [_, (small, eq, _)] = passed_through(class);
        let mut dss = linked(class, Some(n));
        for line in [
            eq.to_string(),
            format!("Edit {elem} DynamicEq={small}"),
            format!("~ {var}={value}"),
            format!("Edit {elem} DynamicEq=de"),
        ] {
            dss.command(&line);
        }
        assert!(dss.errors().is_empty(), "{class}: {:?}", dss.errors());
        run_dynamics(&mut dss, class);
        assert_eq!(
            host_state(&mut dss, class),
            written,
            "{class} {var}={value}"
        );
    }
}

/// `Save` writes only what the language can express: a host line linked to
/// an equation names only that equation's variables, so a deck saved while
/// the host is linked to `lacking` keeps neither the rows nor the
/// assignments that equation lacks. Reloaded and linked to `de`, the host
/// starts those rows at 0, a machine its mass and an inverter its DC
/// voltage, where the live host linked to `de` again carries the values it
/// was written. The variables of `lacking` keep their assignments, so a
/// machine's speed starts at its written 0.05.
#[test]
fn a_saved_host_starts_the_rows_its_equation_lacked_at_zero() {
    for (class, n) in COUNTS {
        let elem = format!("{class}.e1");
        let value_of = |dss: &mut Dss, var: &str| {
            let names = dss.element_variable_names(&elem).expect("names");
            let values = dss.element_variables(&elem).expect("values");
            let i = names
                .iter()
                .position(|name| name == var)
                .unwrap_or_else(|| panic!("{class}: no {var} in {names:?}"));
            values[i]
        };
        let (var, written) = if n == 6 {
            ("mass", 7.0 * (1_800_000.0 / 376.99112))
        } else {
            ("vdc", 8000.0)
        };
        let [(lacking, eq, _), _] = passed_through(class);

        let mut live = with_constant(class, n);
        live.command(eq);
        live.command(&format!("Edit {elem} DynamicEq={lacking}"));
        live.command(&format!("Edit {elem} DynamicEq=de"));
        assert!(live.errors().is_empty(), "{class}: {:?}", live.errors());
        assert_eq!(value_of(&mut live, var), written, "{class} live");

        let mut dss = with_constant(class, n);
        dss.command(eq);
        dss.command(&format!("Edit {elem} DynamicEq={lacking}"));
        assert!(dss.errors().is_empty(), "{class}: {:?}", dss.errors());
        let (line, mut back) = save_and_reload(&mut dss, class, &format!("zero_{class}"));
        assert!(back.errors().is_empty(), "{line}: {:?}", back.errors());
        back.command(&format!("Edit {elem} DynamicEq=de"));
        assert!(back.errors().is_empty(), "{line}: {:?}", back.errors());
        assert_eq!(value_of(&mut back, var), 0.0, "{class}: {line}");
        if n == 6 {
            assert_eq!(value_of(&mut back, "speed"), 0.05, "{class}: {line}");
        }
    }
}

/// A variable that `VarNames` names twice is the row its name resolves to,
/// the first. A value written to it follows it through a re-link and back,
/// in the variable list and in the value `Save` writes.
#[test]
fn a_relink_carries_the_row_a_repeated_variable_name_resolves_to() {
    let elem = "Generator.e1";
    let eq = equation("Generator", "d", "nvariables=7 ");
    let mut dss = linked("Generator", Some(6));
    for line in [
        eq.replace("Damp theta]", "Damp theta speed]"),
        format!("Edit {elem} DynamicEq=d"),
        "~ speed=0.07".to_string(),
        format!("Edit {elem} DynamicEq=de"),
    ] {
        dss.command(&line);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(variable(&host_state(&mut dss, "Generator"), "speed"), 0.07);

    dss.command(&format!("Edit {elem} DynamicEq=d"));
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let state = host_state(&mut dss, "Generator");
    assert_eq!(state.2.len(), 14, "{:?}", state.2);
    assert_eq!(variable(&state, "speed"), 0.07);
    let (line, back) = save_and_reload(&mut dss, "Generator", "repeated_name");
    assert!(line.contains(" speed=0.07"), "{line}");
    assert!(back.errors().is_empty(), "{line}: {:?}", back.errors());
}

/// A state-variable value that does not read as a number is refused at its
/// token, as the same text is on a numeric property, and records nothing:
/// the variable keeps its earlier assignment, a calculated value or a
/// constant, so the host integrates step for step like the deck without the
/// edit and `Save` writes the earlier value. The rest of the line applies, as
/// after a numeric property's refusal, which keeps the value the property
/// had: a property written behind the value takes effect, and the host runs
/// and saves like the line that writes that property alone.
#[test]
fn an_unreadable_state_variable_value_is_refused() {
    let refusal = |typo: &str| format!("Invalid inline math entry: \"{typo}\"");
    for (class, n) in COUNTS {
        let elem = format!("{class}.e1");
        let mut plain = integrated(class, Some(n), None);
        let want = host_state(&mut plain, class);
        let typos: [(&str, &str, &str); 2] = if n == 6 {
            [
                ("PShaft", "1e6x", "pshaft=P0"),
                ("Speed", "abc", "speed=0.05"),
            ]
        } else {
            [("vdc", "1e6x", "vdc=kvdc"), ("vac", "abc", "vac=vmag")]
        };
        for (var, typo, kept) in typos {
            let mut dss = linked(class, Some(n));
            let edit = format!("Edit {elem} {var}=");
            dss.command(&format!("{edit}{typo}"));
            let at = Some((edit.len(), typo.len()));
            assert_eq!(
                logged(&dss),
                [(None, refusal(typo), at)],
                "{class} {var}={typo}"
            );
            dss.errors.clear();
            run_dynamics(&mut dss, class);
            assert_eq!(host_state(&mut dss, class), want, "{class} {var}={typo}");
            let (line, _) = save_and_reload(&mut dss, class, &format!("typo_{class}_{var}"));
            assert!(line.contains(&format!(" {kept}")), "{class}: {line}");
        }

        let [(var, typo, _), _] = typos;
        let (prop, value) = ("Vminpu", "0.85");
        let mut alone = linked(class, Some(n));
        alone.command(&format!("Edit {elem} {prop}={value}"));
        assert!(alone.errors().is_empty(), "{class}: {:?}", alone.errors());
        run_dynamics(&mut alone, class);
        let applied = host_state(&mut alone, class);
        assert_live(&applied, class);
        let (applied_line, _) = save_and_reload(&mut alone, class, &format!("alone_{class}"));
        for refused in [var, "kva"] {
            let mut dss = linked(class, Some(n));
            let kva = query(&mut dss, &format!("{elem}.kva"));
            let edit = format!("Edit {elem} {refused}=");
            dss.command(&format!("{edit}{typo} {prop}={value}"));
            let at = Some((edit.len(), typo.len()));
            assert_eq!(
                logged(&dss),
                [(None, refusal(typo), at)],
                "{class} {refused}={typo} {prop}={value}"
            );
            dss.errors.clear();
            assert_eq!(
                query(&mut dss, &format!("{elem}.{prop}")),
                value,
                "{class} {refused}={typo} {prop}={value}"
            );
            assert_eq!(
                query(&mut dss, &format!("{elem}.kva")),
                kva,
                "{class} {refused}={typo}: kva"
            );
            if refused == var {
                run_dynamics(&mut dss, class);
                assert_eq!(host_state(&mut dss, class), applied, "{class} {var}={typo}");
                let (line, _) = save_and_reload(&mut dss, class, &format!("behind_{class}"));
                assert_eq!(line, applied_line, "{class} {var}={typo} {prop}={value}");
            }
        }
    }
}

/// The properties of `{class}.e1` in the order they were set, the chain
/// `Save` walks.
fn set_order(dss: &Dss, class: &str) -> Vec<usize> {
    let cls = &dss.classes[dss.class_by_name[&class.to_ascii_lowercase()]];
    let data = cls.arena.obj(cls.name_to_idx["e1"]).data();
    std::iter::successors(data.next_property_set(None), |&p| {
        data.next_property_set(Some(p))
    })
    .collect()
}

/// Whether an accepted list set `DynOut` of `{class}.e1`.
fn dynout_specified(dss: &Dss, class: &str) -> bool {
    let cls = &dss.classes[dss.class_by_name[&class.to_ascii_lowercase()]];
    let dynout = cls.props.property_index("DynOut").expect("DynOut");
    cls.arena
        .obj(cls.name_to_idx["e1"])
        .data()
        .prp_specified(dynout)
}

/// The refusal of a `DynOut` list, `typed` as written, that names `name`
/// more than once.
fn named_more_than_once(elem: &str, typed: &str, name: &str) -> String {
    format!(
        "{elem}.DynOut: [{typed}] names the variable \"{name}\" more than once. Each output \
         needs its own variable."
    )
}

/// The index of the property `name` of `class`.
fn property(dss: &Dss, class: &str, name: &str) -> usize {
    let cls = &dss.classes[dss.class_by_name[&class.to_ascii_lowercase()]];
    cls.props.property_index(name).expect(name)
}

/// One recorded error: its code, its message and the span of the value it
/// underlines.
type Logged = (Option<u32>, String, Option<(usize, usize)>);

/// The errors `dss` recorded.
fn logged(dss: &Dss) -> Vec<Logged> {
    dss.errors()
        .iter()
        .map(|e| {
            let span = e.span.as_ref().map(|s| (s.offset(), s.len()));
            (e.code, e.message.clone(), span)
        })
        .collect()
}

/// `Edit {class}.e1 DynOut=[{list}]` on [`linked`] after the host's `kV` is
/// written again, so `DynOut` is not the last property set and a refusal
/// that moved it would show in the set order. The line records exactly the
/// error `refusal`, with no code, at the list's token, `DynOut` and the set
/// order stay as they were, and the host integrates step for step `want`.
fn assert_list_refused(class: &str, n: usize, list: &str, refusal: &str, want: &HostState) {
    let elem = format!("{class}.e1");
    let kv = if class == "PVSystem" { "12.47" } else { "0.69" };
    let mut dss = linked(class, Some(n));
    dss.command(&format!("Edit {elem} kv={kv}"));
    assert!(dss.errors().is_empty(), "{class}: {:?}", dss.errors());
    let order = set_order(&dss, class);
    assert_eq!(
        order.last(),
        Some(&property(&dss, class, "kV")),
        "{class}: {order:?}"
    );
    dss.command(&format!("Edit {elem} DynOut=[{list}]"));
    let at = format!("Edit {elem} DynOut=[").len();
    assert_eq!(
        logged(&dss),
        [(None, refusal.to_string(), Some((at, list.len())))],
        "{class} {list}"
    );
    dss.errors.clear();
    assert_eq!(
        query(&mut dss, &format!("{elem}.DynOut")),
        linked_outputs(class),
        "{class} {list}"
    );
    assert_eq!(set_order(&dss, class), order, "{class} {list}");
    run_dynamics(&mut dss, class);
    assert_eq!(host_state(&mut dss, class), *want, "{class} {list}");
}

/// A `DynOut` gives each output its own variable in a fixed order, a
/// machine its speed and then its angle, an inverter its current, so one
/// variable cannot be two outputs. A list that names one variable twice,
/// compared without case, is refused at its token with no error code, ahead
/// of its length and its names, and the rest of the line is still read:
/// `DynOut` keeps the outputs it had, its place in the set order does not
/// move ([`assert_list_refused`]), and the host integrates step for step as
/// if the list had never been written. A host whose `DynOut` was never set
/// stays without it, unset and unsaved, and a host without an equation gets
/// the same refusal, not 50007.
#[test]
fn every_host_refuses_a_dynout_that_names_one_variable_twice() {
    for (class, n) in COUNTS {
        let elem = format!("{class}.e1");
        let lists: &[(&str, &str, &str)] = if n == 6 {
            &[
                ("Speed speed", "Speed, speed", "speed"),
                ("theta THETA", "theta, THETA", "theta"),
                ("Speed Speed Speed", "Speed, Speed, Speed", "speed"),
            ]
        } else {
            &[("it it", "it, it", "it"), ("it it it", "it, it, it", "it")]
        };
        let want = host_state(&mut integrated(class, Some(n), None), class);
        for &(list, typed, name) in lists {
            let refusal = named_more_than_once(&elem, typed, name);
            assert_list_refused(class, n, list, &refusal, &want);
        }

        let (list, typed, name) = lists[0];
        let eq = equation(class, "de", &format!("nvariables={n} "));
        let mut dss = solved(class, &eq, Some(&format!("Edit {elem} DynOut=[{list}]")));
        let got: Vec<_> = dss
            .errors()
            .iter()
            .map(|e| (e.code, e.message.clone()))
            .collect();
        assert_eq!(
            got,
            [(None, named_more_than_once(&elem, typed, name))],
            "{class}"
        );
        dss.errors.clear();
        assert!(!dynout_specified(&dss, class), "{class}");
        assert_eq!(query(&mut dss, &format!("{elem}.DynOut")), "", "{class}");
        let (line, _) = save_and_reload(&mut dss, class, &format!("twice_{class}"));
        assert!(!line.to_ascii_lowercase().contains("dynout="), "{line}");
    }

    let generator = HOSTS
        .iter()
        .find(|(c, _)| *c == "Generator")
        .map(|(_, line)| *line)
        .expect("a Generator");
    let mut dss = Dss::new();
    for line in [
        "Clear",
        "New Circuit.twice basekv=0.69 phases=3 bus1=srcbus",
        generator,
        "Edit Generator.e1 DynOut=[a a]",
    ] {
        dss.command(line);
    }
    let got: Vec<_> = dss
        .errors()
        .iter()
        .map(|e| (e.code, e.message.clone()))
        .collect();
    assert_eq!(
        got,
        [(None, named_more_than_once("Generator.e1", "a, a", "a"))]
    );

    let mut dss = linked("Generator", Some(6));
    dss.command("Edit Generator.e1 DynOut=[Speed speed] kW=1400");
    let refusal = named_more_than_once("Generator.e1", "Speed, speed", "speed");
    assert_eq!(logged(&dss), [(None, refusal, Some((26, 11)))]);
    dss.errors.clear();
    assert_eq!(query(&mut dss, "Generator.e1.kW"), "1400");
    assert_eq!(query(&mut dss, "Generator.e1.DynOut"), "[speed, theta]");
}

/// The refusal of the string list `typed` as written, with an empty element
/// before a name, on the property `prop` (`Class.name.Property`).
fn with_an_empty_element(prop: &str, typed: &str) -> String {
    format!(
        "{prop}: [{typed}] has an empty element before a name. Remove it so that each element \
         names one entry."
    )
}

/// A comma separates the elements of a list, so two commas in a row leave
/// an empty element between them, and in a `DynOut` an empty element before
/// a name takes a slot and names no output variable. Such a list is
/// refused at its token with no error code, ahead of a repeated name, its
/// length and its names, and changes nothing ([`assert_list_refused`]). A
/// list whose empty elements, commas or empty quoted tokens, all stand at
/// its end names its outputs and is read without them, and `["",""]` is the
/// empty list.
#[test]
fn every_host_refuses_a_dynout_with_an_empty_element() {
    for (class, n) in COUNTS {
        let elem = format!("{class}.e1");
        let lists: &[(&str, &str)] = if n == 6 {
            &[
                ("Speed,,theta", "Speed, , theta"),
                ("Speed,,Speed", "Speed, , Speed"),
                ("Speed theta,,Speed", "Speed, theta, , Speed"),
                ("Speed \"\" theta", "Speed, , theta"),
            ]
        } else {
            &[
                ("it,,it", "it, , it"),
                ("it,,vac", "it, , vac"),
                ("it () vac", "it, , vac"),
            ]
        };
        let want = host_state(&mut integrated(class, Some(n), None), class);
        for &(list, typed) in lists {
            let refusal = with_an_empty_element(&format!("{elem}.DynOut"), typed);
            assert_list_refused(class, n, list, &refusal, &want);
        }

        let ends: &[&str] = if n == 6 {
            &["[Speed theta,]", "[Speed theta,,\"\"]", "[Speed theta,,()]"]
        } else {
            &["[it,]", "[it,,\"\"]", "[it,,()]"]
        };
        for ends in ends {
            let mut dss = linked(class, Some(n));
            dss.command(&format!("Edit {elem} DynOut=[\"\",\"\"]"));
            assert_eq!(query(&mut dss, &format!("{elem}.DynOut")), "", "{class}");
            dss.command(&format!("Edit {elem} DynOut={ends}"));
            assert!(
                dss.errors().is_empty(),
                "{class} {ends}: {:?}",
                dss.errors()
            );
            assert_eq!(
                query(&mut dss, &format!("{elem}.DynOut")),
                linked_outputs(class),
                "{class} {ends}"
            );
            run_dynamics(&mut dss, class);
            assert_eq!(host_state(&mut dss, class), want, "{class} {ends}");
        }

        let mut dss = linked(class, Some(n));
        dss.command(&format!("Edit {elem} DynOut=[\"\",\"\"]"));
        assert!(dss.errors().is_empty(), "{class}: {:?}", dss.errors());
        assert_eq!(query(&mut dss, &format!("{elem}.DynOut")), "", "{class}");
        assert_dynamics_refused(
            dss,
            &format!(
                "{elem}: DynOut is not set. Name the output variables of DynamicExp.de \
                 with DynOut=[...]."
            ),
        );
    }
}

/// Every string-list property, as the engine declares them.
const STRING_LISTS: [(&str, &str); 18] = [
    ("DynamicExp", "VarNames"),
    ("Generator", "DynOut"),
    ("PVSystem", "DynOut"),
    ("Storage", "DynOut"),
    ("WindGen", "DynOut"),
    ("InvControl", "DERList"),
    ("InvControl", "MonBus"),
    ("InvControl", "PVSystemList"),
    ("ExpControl", "PVSystemList"),
    ("ExpControl", "DERList"),
    ("ESPVLControl", "LocalControlList"),
    ("ESPVLControl", "PVSystemList"),
    ("ESPVLControl", "StorageList"),
    ("GenDispatcher", "GenList"),
    ("StorageController", "ElementList"),
    ("UPFCControl", "UPFCList"),
    ("EnergyMeter", "Option"),
    ("EnergyMeter", "ZoneList"),
];

/// A circuit imported from a JSON document whose one `{class}` object `t`
/// carries the items `items` for `prop` (`None` leaves the property out),
/// with the properties the import requires of its class and an equation `x`
/// for a host to link: the errors the import recorded and `prop` read back.
fn imported(class: &str, prop: &str, items: Option<&str>) -> (Vec<String>, String) {
    let mut records = Vec::new();
    let mut fields = vec![r#""Name":"t""#.to_string()];
    let required = match class {
        "DynamicExp" => r#","Expression":"[a dt = a]""#,
        "Generator" => r#","Bus1":"srcbus","kV":0.69,"kW":100,"PF":1,"kvar":0"#,
        "PVSystem" => r#","Bus1":"srcbus","kV":0.69,"PF":1,"kvar":0"#,
        "Storage" => r#","Bus1":"srcbus","kV":0.69,"kvar":0,"PF":1,"kWRated":100"#,
        "ESPVLControl" | "GenDispatcher" | "StorageController" => r#","Element":"Vsource.source""#,
        _ => "",
    };
    fields[0].push_str(required);
    if matches!(class, "Generator" | "PVSystem" | "Storage" | "WindGen") {
        records.push(
            r#""DynamicExp":[{"Name":"x","NVariables":2,"VarNames":["a","b"],"Expression":"[a dt = b]"}]"#
                .to_string(),
        );
        fields.push(r#""DynamicEq":"x""#.to_string());
    }
    if let Some(items) = items {
        fields.push(format!(r#""{prop}":{items}"#));
    }
    records.push(format!(r#""{class}":[{{{}}}]"#, fields.join(",")));
    let mut dss = Dss::new();
    let json = format!(r#"{{"Name":"census",{}}}"#, records.join(","));
    dss.circuit_from_json(&json).expect("a JSON circuit");
    let errors = dss.error_texts();
    dss.errors.clear();
    let read = query(&mut dss, &format!("{class}.t.{prop}"));
    (errors, read)
}

/// Every string list refuses an empty element before a name, as `DynOut`
/// does: the element names no entry, and the names written behind it are
/// part of the list. The list is refused at its token with no code, the
/// property keeps the list it had and the rest of the line applies, as for
/// any refused value. `VarNames` keeps its variables, so the equation keeps
/// the `b` it reads, and an InvControl keeps the DER it controls. Empty
/// elements at the end of a list name nothing and are read without them.
/// An element that is only white space names nothing either, so the list
/// `Save` writes reloads. [`STRING_LISTS`] is the whole census, and each of
/// its properties refuses `[a,,b]` on a fresh object, ahead of any message
/// its class gives for the edit of an incomplete object. The JSON import
/// decides a list by its items the same way: an empty or blank item before
/// a name is refused with the same message and the property stays as it
/// was, one at the end is read without it. The import reads no
/// `PVSystemList` of an InvControl, whose list is that of `DERList`.
#[test]
fn every_string_list_refuses_an_empty_element_before_a_name() {
    let mut dss = Dss::new();
    for line in [
        "Clear",
        "New Circuit.lists basekv=0.69 phases=3 bus1=srcbus",
        "New DynamicExp.x nvariables=2 varnames=[a b] expression=[a dt = b]",
        "New PVSystem.p1 bus1=srcbus phases=3 kv=0.69 kVA=10 Pmpp=10",
        "New PVSystem.p2 bus1=srcbus phases=3 kv=0.69 kVA=10 Pmpp=10",
        "New InvControl.ic DERList=[PVSystem.p1]",
    ] {
        dss.command(line);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    dss.command("Edit DynamicExp.x varnames=[A,,B] nvariables=3");
    let refusal = with_an_empty_element("DynamicExp.x.VarNames", "A, , B");
    let at = "Edit DynamicExp.x varnames=[".len();
    assert_eq!(logged(&dss), [(None, refusal, Some((at, 4)))]);
    dss.errors.clear();
    assert_eq!(query(&mut dss, "DynamicExp.x.VarNames"), "[a, b]");
    assert_eq!(query(&mut dss, "DynamicExp.x.NVariables"), "3");

    for (list, typed) in [
        ("PVSystem.p1,,PVSystem.p2", "PVSystem.p1, , PVSystem.p2"),
        (",PVSystem.p2", ", PVSystem.p2"),
        ("PVSystem.p1 \"\" PVSystem.p2", "PVSystem.p1, , PVSystem.p2"),
    ] {
        dss.command(&format!(
            "Edit InvControl.ic DERList=[{list}] DeltaQ_Factor=0.25"
        ));
        let refusal = with_an_empty_element("InvControl.ic.DERList", typed);
        let at = "Edit InvControl.ic DERList=[".len();
        assert_eq!(
            logged(&dss),
            [(None, refusal, Some((at, list.len())))],
            "{list}"
        );
        dss.errors.clear();
        assert_eq!(query(&mut dss, "InvControl.ic.DERList"), "[PVSystem.p1]");
        assert_eq!(query(&mut dss, "InvControl.ic.DeltaQ_Factor"), "0.25");
        dss.command("Edit InvControl.ic DeltaQ_Factor=0.5");
    }
    for list in ["PVSystem.p1 PVSystem.p2,,", "PVSystem.p1 PVSystem.p2 \"\""] {
        dss.command("Edit InvControl.ic DERList=[PVSystem.p1]");
        dss.command(&format!("Edit InvControl.ic DERList=[{list}]"));
        assert!(dss.errors().is_empty(), "{list}: {:?}", dss.errors());
        assert_eq!(
            query(&mut dss, "InvControl.ic.DERList"),
            "[PVSystem.p1, PVSystem.p2]",
            "{list}"
        );
    }

    let blank = r#"PVSystem.p1 " " PVSystem.p2"#;
    dss.command("Edit InvControl.ic DERList=[PVSystem.p1]");
    dss.command(&format!("Edit InvControl.ic DERList=[{blank}]"));
    let refusal = with_an_empty_element("InvControl.ic.DERList", "PVSystem.p1,  , PVSystem.p2");
    let at = "Edit InvControl.ic DERList=[".len();
    assert_eq!(logged(&dss), [(None, refusal, Some((at, blank.len())))]);
    dss.errors.clear();
    assert_eq!(query(&mut dss, "InvControl.ic.DERList"), "[PVSystem.p1]");
    dss.command(r#"Edit InvControl.ic DERList=[PVSystem.p1 PVSystem.p2 " "]"#);
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let (line, mut back) = save_and_reload_object(&mut dss, "InvControl", "ic", "blank");
    assert!(
        line.contains("DERList=[PVSystem.p1, PVSystem.p2]"),
        "{line}"
    );
    assert!(back.errors().is_empty(), "{line}: {:?}", back.errors());
    assert_eq!(
        query(&mut back, "InvControl.ic.DERList"),
        "[PVSystem.p1, PVSystem.p2]"
    );

    let mut declared: Vec<(String, String)> = Vec::new();
    for cls in &dss.classes {
        for idx in 1..=cls.props.num_properties() {
            let pd = cls.props.prop(idx);
            if pd.ptype == crate::obj::props::PropType::StringList {
                let class = cls.props.class_name().to_ascii_lowercase();
                declared.push((class, pd.name.to_ascii_lowercase()));
            }
        }
    }
    let mut census: Vec<(String, String)> = STRING_LISTS
        .iter()
        .map(|(c, p)| (c.to_ascii_lowercase(), p.to_ascii_lowercase()))
        .collect();
    declared.sort();
    census.sort();
    assert_eq!(declared, census);

    for (class, prop) in STRING_LISTS {
        let mut dss = Dss::new();
        dss.command("Clear");
        dss.command("New Circuit.census basekv=0.69 phases=3 bus1=srcbus");
        dss.command(&format!("New {class}.t"));
        dss.errors.clear();
        let before = query(&mut dss, &format!("{class}.t.{prop}"));
        dss.command(&format!("Edit {class}.t {prop}=[a,,b]"));
        let refusal = with_an_empty_element(&format!("{class}.t.{prop}"), "a, , b");
        let at = format!("Edit {class}.t {prop}=[").len();
        let got = logged(&dss);
        assert_eq!(
            got.first(),
            Some(&(None, refusal, Some((at, 4)))),
            "{class}"
        );
        assert!(
            got[1..]
                .iter()
                .all(|(_, m, _)| !m.contains("empty element")),
            "{class}: {got:?}"
        );
        dss.errors.clear();
        assert_eq!(
            query(&mut dss, &format!("{class}.t.{prop}")),
            before,
            "{class}.{prop}"
        );

        let unwritten = imported(class, prop, None);
        for (items, typed) in [
            (r#"["a","","b"]"#, "a, , b"),
            (r#"["a"," ","b"]"#, "a,  , b"),
        ] {
            let got = imported(class, prop, Some(items));
            if (class, prop) == ("InvControl", "PVSystemList") {
                assert_eq!(got, unwritten, "{class}.{prop} {items}");
                continue;
            }
            let refusal = with_an_empty_element(&format!("{class}.t.{prop}"), typed);
            assert_eq!(got.0.first(), Some(&refusal), "{class}.{prop} {items}");
            assert!(
                got.0[1..].iter().all(|m| !m.contains("empty element")),
                "{class}.{prop} {items}: {:?}",
                got.0
            );
            assert_eq!(got.1, unwritten.1, "{class}.{prop} {items}");
        }
        let one = imported(class, prop, Some(r#"["a"]"#));
        for items in [r#"["a",""]"#, r#"["a"," "]"#] {
            assert_eq!(
                imported(class, prop, Some(items)),
                one,
                "{class}.{prop} {items}"
            );
        }
    }
    assert_eq!(
        imported("InvControl", "DERList", Some(r#"["a",""]"#)),
        (Vec::new(), "[a]".to_string())
    );
}

/// The property form `Class.name.Property=value` gives the property its
/// value as read, whatever quotes or brackets it holds, and then reads the
/// rest of the line, so a line applies as `Edit` applies it. On every
/// string list an inner `""` or `()` is refused with the same message and
/// leaves the object as `Edit` leaves it, and a list of quoted names and a
/// plain list apply alike. A zone list with an empty quoted element keeps
/// the meter's zone and `LocalOnly`, where cutting the value at its quote
/// would write the rest into `LocalOnly`, and a switch state written
/// without quotes opens every phase, where a value read as quoted would
/// open the first only.
#[test]
fn the_property_form_applies_a_value_as_edit_does() {
    let outcome = |class: &str, prop: &str, line: &str| {
        let mut dss = Dss::new();
        dss.command("Clear");
        dss.command("New Circuit.form basekv=0.69 phases=3 bus1=srcbus");
        dss.command("New DynamicExp.x nvariables=2 varnames=[a b] expression=[a dt = b]");
        dss.command(&format!("New {class}.t"));
        if matches!(class, "Generator" | "PVSystem" | "Storage" | "WindGen") {
            dss.command(&format!("Edit {class}.t DynamicEq=x"));
        }
        dss.errors.clear();
        dss.command(line);
        let errors: Vec<_> = dss
            .errors()
            .iter()
            .map(|e| (e.code, e.message.clone()))
            .collect();
        dss.errors.clear();
        let read = query(&mut dss, &format!("{class}.t.{prop}"));
        let json = dss
            .obj_to_json(&format!("{class}.t"), Default::default())
            .expect("json");
        (errors, read, json)
    };
    for (class, prop) in STRING_LISTS {
        let plain = outcome(class, prop, &format!("Edit {class}.t {prop}=[a b]"));
        for list in [r#"[a "" b]"#, "[a () b]", r#"["a" "b"]"#, "[a b]"] {
            let edit = outcome(class, prop, &format!("Edit {class}.t {prop}={list}"));
            let form = outcome(class, prop, &format!("{class}.t.{prop}={list}"));
            assert_eq!(form, edit, "{class}.{prop}={list}");
            let refused = edit.0.iter().any(|(_, m)| m.contains("empty element"));
            let inner = list.contains(r#""""#) || list.contains("()");
            assert_eq!(refused, inner, "{class}.{prop}={list}: {:?}", edit.0);
            if !inner {
                assert_eq!(form, plain, "{class}.{prop}={list}");
            }
        }
    }

    let zone = |line: &str| {
        let mut dss = Dss::new();
        for setup in [
            "Clear",
            "New Circuit.zone basekv=12.47 phases=3 bus1=srcbus",
            "New Line.l1 bus1=srcbus bus2=b2 length=1 units=km",
            "New Line.l2 bus1=b2 bus2=b3 length=1 units=km",
            "New EnergyMeter.m Element=Line.l1 Terminal=1 ZoneList=[Line.l2] LocalOnly=yes",
        ] {
            dss.command(setup);
        }
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        dss.command(line);
        let got = logged(&dss);
        dss.errors.clear();
        let zone = query(&mut dss, "EnergyMeter.m.ZoneList");
        (got, zone, query(&mut dss, "EnergyMeter.m.LocalOnly"))
    };
    let list = r#"Line.l1 "" Line.l2"#;
    let refusal = with_an_empty_element("EnergyMeter.m.ZoneList", "Line.l1, , Line.l2");
    for line in [
        format!("EnergyMeter.m.ZoneList=[{list}]"),
        format!("Edit EnergyMeter.m ZoneList=[{list}]"),
    ] {
        let at = line.find('[').expect("a list") + 1;
        assert_eq!(
            zone(&line),
            (
                vec![(None, refusal.clone(), Some((at, list.len())))],
                "[Line.l2]".to_string(),
                "Yes".to_string()
            ),
            "{line}"
        );
    }

    let switch = |line: &str| {
        let mut dss = Dss::new();
        for setup in [
            "Clear",
            "New Circuit.switch basekv=12.47 phases=3 bus1=srcbus",
            "New Line.sw bus1=srcbus bus2=b2 switch=yes",
            "New SwtControl.s SwitchedObj=Line.sw SwitchedTerm=1",
            line,
        ] {
            dss.command(setup);
        }
        assert!(dss.errors().is_empty(), "{line}: {:?}", dss.errors());
        query(&mut dss, "SwtControl.s.State")
    };
    for (value, state) in [
        ("open", "[open, open, open, ]"),
        ("(open)", "[open, closed, closed, ]"),
    ] {
        assert_eq!(switch(&format!("SwtControl.s.State={value}")), state);
        assert_eq!(switch(&format!("Edit SwtControl.s State={value}")), state);
    }
}

/// `Save` writes `DynamicEq` before `DynOut` whatever order the deck set
/// them in (a `DynOut` set first moves to directly behind `DynamicEq`) and
/// every other property in the order it was set, because `DynOut` names
/// outputs of the linked equation and a line that wrote it first would
/// reload without them. The plain deck, saved and reloaded, integrates step
/// for step like the deck it was saved from. A host re-linked to a copy of
/// its equation writes the plain line with the copy's name, and a host whose
/// `DynOut` and `DynamicEq` were written again on one line writes the plain
/// line. A `kV` written again between `DynOut` and the re-link stays ahead of
/// `DynamicEq`, one written after the re-link goes behind `DynOut`. Each
/// reloads without an error, reads its outputs back and integrates like the
/// plain deck. `Save` leaves the set order of the live host as it was.
#[test]
fn save_writes_dynamic_eq_before_dynout() {
    for (class, n) in COUNTS {
        let elem = format!("{class}.e1");
        let reloaded = |dss: &mut Dss, tag: &str, eq: &str| -> (String, HostState) {
            let order = set_order(dss, class);
            let (line, mut back) = save_and_reload(dss, class, &format!("{tag}_{class}"));
            assert_eq!(set_order(dss, class), order, "{class} {tag}");
            let lower = line.to_ascii_lowercase();
            let at = |token: &str| {
                lower
                    .find(token)
                    .unwrap_or_else(|| panic!("{class} {tag}: no {token} in {line}"))
            };
            assert!(
                at(&format!(" dynamiceq={eq} ")) < at(" dynout="),
                "{class} {tag}: {line}"
            );
            assert!(
                back.errors().is_empty(),
                "{class} {tag}: {line}: {:?}",
                back.errors()
            );
            assert_eq!(
                query(&mut back, &format!("{elem}.DynOut")),
                linked_outputs(class),
                "{class} {tag}"
            );
            back.command("Solve");
            run_dynamics(&mut back, class);
            (line, host_state(&mut back, class))
        };

        let (plain, want) = reloaded(&mut linked(class, Some(n)), "plain", "de");
        assert_live(&want, class);
        assert_eq!(
            want,
            host_state(&mut integrated(class, Some(n), None), class),
            "{class}: the saved deck integrates like the deck it was saved from"
        );

        let copy = equation(class, "copy", &format!("nvariables={n} "));
        let mut relinked = linked(class, Some(n));
        relinked.command(&copy);
        relinked.command(&format!("Edit {elem} DynamicEq=copy"));
        assert!(
            relinked.errors().is_empty(),
            "{class}: {:?}",
            relinked.errors()
        );
        let (line, got) = reloaded(&mut relinked, "copy", "copy");
        assert_eq!(got, want, "{class} DynamicEq=copy");
        assert_eq!(
            line,
            plain.replace("DynamicEq=de ", "DynamicEq=copy "),
            "{class} DynamicEq=copy"
        );

        let outputs = if n == 6 { "[Speed theta]" } else { "[it]" };
        let mut rewritten = linked(class, Some(n));
        rewritten.command(&format!("Edit {elem} DynOut={outputs} DynamicEq=de"));
        assert!(
            rewritten.errors().is_empty(),
            "{class}: {:?}",
            rewritten.errors()
        );
        let (line, got) = reloaded(&mut rewritten, "rewritten", "de");
        assert_eq!(got, want, "{class} DynOut and DynamicEq on one line");
        assert_eq!(line, plain, "{class} DynOut and DynamicEq on one line");

        if class != "Generator" {
            continue;
        }
        let unmoved = plain.replacen(" kV=0.69 ", " ", 1);
        for (tag, edits, line_want) in [
            (
                "kv_before",
                [
                    "Edit Generator.e1 kv=0.69",
                    "Edit Generator.e1 DynamicEq=copy",
                ],
                unmoved.replace("DynamicEq=de ", "kV=0.69 DynamicEq=copy "),
            ),
            (
                "kv_after",
                [
                    "Edit Generator.e1 DynamicEq=copy",
                    "Edit Generator.e1 kv=0.69",
                ],
                unmoved.replace(
                    "DynamicEq=de DynOut=[speed, theta] ",
                    "DynamicEq=copy DynOut=[speed, theta] kV=0.69 ",
                ),
            ),
        ] {
            let mut dss = linked(class, Some(n));
            dss.command(&copy);
            for edit in edits {
                dss.command(edit);
            }
            assert!(dss.errors().is_empty(), "{tag}: {:?}", dss.errors());
            let (line, got) = reloaded(&mut dss, tag, "copy");
            assert_ne!(
                line,
                plain.replace("DynamicEq=de ", "DynamicEq=copy "),
                "{tag}"
            );
            assert_eq!(line, line_want, "{tag}");
            assert_eq!(got, want, "{tag}");
        }
    }
}

/// The text of the file the report `command` (`Show ...`, `Export ...` or
/// `Dump ...`) writes for `dss`, its output routed to a folder of its own.
fn report_text(dss: &mut Dss, command: &str, tag: &str) -> String {
    let dir = std::env::temp_dir().join(format!("dss_dyneq_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{tag}: {}: {e}", dir.display()));
    let before = dss.errors().len();
    dss.command(&format!(
        "Set datapath=\"{}\"",
        dir.to_string_lossy().replace('\\', "/")
    ));
    dss.command(command);
    assert_eq!(dss.errors().len(), before, "{tag}: {:?}", dss.error_texts());
    let path = if command.to_ascii_lowercase().starts_with("show") {
        dss.last_show_file().to_string()
    } else {
        dss.last_result_file().to_string()
    };
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{tag}: {path}: {e}"));
    std::fs::remove_dir_all(&dir).ok();
    text
}

/// The block `Show Variables` writes for `{class}.e1`: its `ELEMENT` line,
/// its count and one line per variable.
fn shown_variables(dss: &mut Dss, class: &str, tag: &str) -> Vec<String> {
    let text = report_text(dss, "Show Variables", &format!("{tag}_{class}"));
    let head = format!("element: {class}.e1").to_ascii_lowercase();
    let lines: Vec<&str> = text.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.to_ascii_lowercase() == head)
        .unwrap_or_else(|| panic!("{class}: no {head} in {text}"));
    lines[start..]
        .iter()
        .take_while(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect()
}

/// A host whose equation omits `NVariables` lists only the variables the
/// equation names, although its memory holds 20 rows: its variable names
/// and values, its `Show Variables` block, its `Dump` `! VARIABLES` block, a
/// mode-3 monitor and that monitor's export read exactly like those of the
/// same equation with its
/// count written, 12 variables for a machine and 8 for an inverter. An
/// equation with 21 names lists the 20 rows its memory holds, all named,
/// and one without `VarNames` leaves its host the classic layout. A mode-3
/// monitor does not take a WindGen, so the WindGen is checked without one.
#[test]
fn an_omitted_nvariables_lists_only_the_named_variables() {
    for (class, n) in COUNTS {
        let elem = format!("{class}.e1");
        let mut written = linked(class, Some(n));
        let mut omitted = linked(class, None);
        let names = written.element_variable_names(&elem).expect("names");
        assert_eq!(names.len(), 2 * n, "{class}: {names:?}");
        assert_eq!(
            omitted.element_variable_names(&elem),
            Some(names.clone()),
            "{class}"
        );
        assert_eq!(
            omitted.element_variables(&elem),
            written.element_variables(&elem),
            "{class}"
        );

        let shown = shown_variables(&mut omitted, class, "show_omitted");
        let count = format!("No. of variables: {}", 2 * n);
        assert_eq!(shown.get(1), Some(&count), "{class}: {shown:?}");
        assert_eq!(shown.len(), 2 + 2 * n, "{class}: {shown:?}");
        assert_eq!(
            shown,
            shown_variables(&mut written, class, "show_written"),
            "{class}"
        );

        let dump = format!("dump {elem} debug");
        let dumped = report_text(&mut omitted, &dump, &format!("dump_omitted_{class}"));
        let listed: Vec<&str> = dumped
            .lines()
            .skip_while(|l| *l != "! VARIABLES")
            .skip(1)
            .map_while(|l| {
                let (row, rest) = l.strip_prefix("! ")?.split_once(": ")?;
                row.trim().parse::<usize>().ok()?;
                Some(rest.split_once(" = ")?.0)
            })
            .collect();
        assert_eq!(listed, names, "{class}: {dumped}");
        assert_eq!(
            dumped,
            report_text(&mut written, &dump, &format!("dump_written_{class}")),
            "{class}"
        );

        if class == "WindGen" {
            continue;
        }
        let monitored = |count: Option<usize>, tag: &str| {
            let count = count.map_or(String::new(), |n| format!("nvariables={n} "));
            let mut dss = linked_with(
                class,
                &equation(class, "de", &count),
                &format!(
                    "{}\nNew Monitor.m3 element={elem} terminal=1 mode=3",
                    linked_edit(class)
                ),
            );
            run_dynamics(&mut dss, class);
            let csv = report_text(&mut dss, "Export Monitors m3", &format!("{tag}_{class}"));
            (csv, dss.monitor_view("m3").expect("monitor m3"))
        };
        let (csv, view) = monitored(None, "m3_omitted");
        assert_eq!(view.header[2..], names[..], "{class}: {:?}", view.header);
        assert_eq!(view.channels.len(), 2 * n, "{class}");
        let samples = view.sample_count as usize;
        assert!(samples > 0, "{class}");
        assert!(view.channels.iter().all(|c| c.len() == samples), "{class}");
        let header: Vec<&str> = csv
            .lines()
            .next()
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .collect();
        assert_eq!(header[2..], names[..], "{class}: {header:?}");
        let (w_csv, w_view) = monitored(Some(n), "m3_written");
        assert_eq!(csv, w_csv, "{class}");
        assert_eq!(view.header, w_view.header, "{class}");
        assert_eq!(view.channels, w_view.channels, "{class}");
    }

    let vars: Vec<String> = (1..=21).map(|i| format!("v{i}")).collect();
    let eq = format!(
        "New DynamicExp.de varnames=[{}] expression=[v2 dt = v1; v1 dt = v20]",
        vars.join(" ")
    );
    let mut dss = solved("Generator", &eq, Some("~ DynOut=[v1 v2]"));
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let want: Vec<String> = vars[..20]
        .iter()
        .flat_map(|v| [v.clone(), format!("d{v}")])
        .collect();
    assert_eq!(dss.element_variable_names("Generator.e1"), Some(want));

    for (class, _) in HOSTS {
        let mut dss = solved(class, BARE, None);
        assert!(dss.errors().is_empty(), "{class}: {:?}", dss.errors());
        let names = dss
            .element_variable_names(&format!("{class}.e1"))
            .expect("names");
        assert_eq!(names, classic_names(class), "{class}");
    }
    assert_eq!(classic_names("Generator").len(), 6);
    assert_eq!(classic_names("WindGen").len(), 22);
}

//! A `DynamicExp` whose memory cannot hold what its host integrates: no state
//! variables (`NVariables=0`, the default), fewer rows than the equations use,
//! or no `DynOut`. Every host refuses the dynamics solve with error 482 before
//! the first step, and the variable and `DynOut` reads of such an equation
//! answer instead of failing. A `DynOut` longer than its two output slots is
//! refused where it is written, with error 50009.

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

/// An equation with no state variables at all: no `NVariables`, no `VarNames`.
const NO_STATE: &str = "New DynamicExp.de expression=[]";

/// The power-flow-solved deck: a stiff 0.69 kV source, a short line, the
/// `DynamicExp` line `eq`, and the host `class` linked to it plus `edit` (a
/// `~` line or nothing). The caller checks the errors the build recorded.
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
        eq,
        &format!("{host} DynamicEq=de"),
    ] {
        dss.command(line);
    }
    if let Some(edit) = edit {
        dss.command(edit);
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

/// `NVariables` sizes the memory and defaults to 0, so an equation that names
/// its variables but omits the count holds no state either. The refusal says
/// which property to set.
#[test]
fn an_omitted_nvariables_holds_no_state_variables() {
    let eq = "New DynamicExp.de varnames=[Speed theta] \
              expression=[Speed dt = Speed; theta dt = Speed]";
    let dss = solved("WindGen", eq, Some("~ DynOut=[Speed theta]"));
    assert_dynamics_refused(dss, &no_state_refusal("WindGen"));
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
/// resolve them.
#[test]
fn every_host_reads_back_no_dynout_from_an_equation_without_var_names() {
    for (class, _) in HOSTS {
        let mut dss = solved(class, NO_STATE, Some("~ DynOut=[Speed theta]"));
        let errors: Vec<_> = dss.errors().iter().map(|e| e.code).collect();
        assert_eq!(errors, [Some(50008), Some(50008)], "{class}");
        assert_eq!(
            query(&mut dss, &format!("{class}.e1.DynOut")),
            "",
            "{class}"
        );
    }
}

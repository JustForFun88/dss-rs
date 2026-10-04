//! A `DynamicExp` whose memory cannot hold what its host integrates: no state
//! variables (`NVariables=0`), fewer rows than the equations use, or a
//! `DynOut` that names fewer outputs than the host integrates. Every host
//! refuses the dynamics solve with error 482 before the first step, and the
//! variable and `DynOut` reads of such an equation answer instead of failing.
//! A `DynOut` list longer than its two output slots, or with a name that is
//! not an output, is refused where it is written, with error 50009 or 50008,
//! and leaves `DynOut` as it was. A re-link of `DynamicEq=` keeps `DynOut`
//! only when the new equation defines all its outputs. An equation that omits
//! `NVariables` holds 20 rows and integrates like the same equation with its
//! count written.

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
/// `DynamicExp.de` plus `edit` (a `~` line or nothing). The caller checks the
/// errors the build recorded.
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

/// One host linked to the [`equation`] `de`, with `nvariables=` written
/// (`Some`) or omitted (`None`), its power flow solved. A machine starts its
/// swing equation off its rest point (`Speed=0.05`), an inverter loads its
/// filter state from its model (the PVSystem on a 12.47 kV feeder, where its
/// filter is stable).
fn linked(class: &str, nvariables: Option<usize>) -> Dss {
    let count = nvariables.map_or(String::new(), |n| format!("nvariables={n} "));
    let eq = equation(class, "de", &count);
    let dss = if matches!(class, "Generator" | "WindGen") {
        let edit = "~ Damp=0 PShaft=P0 Pterm=P Speed=0.05 theta=Edp \
                    Mass=(3.5 2 * 1800000 376.99112 / *) DynOut=[Speed theta]";
        solved(class, &eq, Some(edit))
    } else {
        let edit = "~ it=imag vdc=kvdc modul=mod vac=vmag DynOut=[it]";
        if class == "PVSystem" {
            let mut dss = Dss::new();
            for line in [
                "Clear",
                "New Circuit.pvdyneq basekv=12.47 pu=1.0 phases=3 bus1=sourcebus \
                 mvasc3=20000 mvasc1=21000",
                "New Line.l1 bus1=sourcebus bus2=pvbus length=0.5 units=km r1=0.1 x1=0.3",
                &eq,
                "New PVSystem.e1 phases=3 bus1=pvbus kv=12.47 kVA=600 Pmpp=500 irradiance=1 \
                 %cutin=0.1 %cutout=0.1 kvar=0 DynamicEq=de",
                edit,
                "Set voltagebases=[12.47]",
                "Calcvoltagebases",
                "Solve",
            ] {
                dss.command(line);
            }
            dss
        } else {
            solved(class, &eq, Some(edit))
        }
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
/// the same node voltages, host powers and currents, and the same values in
/// its named memory rows. The 20 default rows read past the named ones as
/// unnamed zeros.
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
        assert_eq!(o_names.len(), 40, "{class}: {o_names:?}");
        assert_eq!(o_names[..2 * n], w_names[..], "{class}");
        assert_eq!(o_values[..2 * n], w_values[..], "{class}");
        assert!(o_names[2 * n..].iter().all(String::is_empty), "{class}");
        assert!(o_values[2 * n..].iter().all(|v| *v == 0.0), "{class}");

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
/// `DynOut` empty, and the host refuses the dynamics solve. The smaller one
/// is refused for its memory first: the values [`linked`] loads from its
/// model reach row `n`.
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
            "{elem}: DynamicExp.b declares NVariables=1 but its equations use {n} state \
             variables. Set NVariables to the number of its VarNames."
        );
        let decks = [
            ("New DynamicExp.b".to_string(), &not_set),
            ("New DynamicExp.b like=de".to_string(), &not_set),
            (format!("New DynamicExp.b nvariables={n}"), &not_set),
            (format!("New DynamicExp.b nvariables={n} {short}"), &not_set),
            (
                "New DynamicExp.b nvariables=1 varnames=[x] expression=[x dt = x]".to_string(),
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
/// re-linked. The re-link allocates a fresh memory, so a machine writes its
/// constant initial values again. A `DynOut=` list written after
/// `DynamicEq=` on the re-link's line names the outputs of the new equation.
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

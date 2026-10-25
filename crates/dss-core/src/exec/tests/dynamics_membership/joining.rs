//! Elements without a state in the present run, and the solves that refuse them.
//!
//! The entry into a dynamics-model mode seeds every enabled element the last
//! power flow held in its present shape and connection. An element with dynamic
//! state that it did not seed (created, enabled, re-phased or given another bus
//! after that power flow or after `Set mode=`) has no initial state: every
//! solve in that mode refuses it before it integrates or stamps anything, and
//! names the element and the remedy (`Set mode=snap`, `Solve`, `Set mode=` the
//! same mode). A VCCS without the curves of its model has no dynamics to
//! integrate and takes part in a fault study with its seeded current, and a
//! non-finite node voltage is never a solution.
//!
//! Deck and conventions as in the parent module. A refused command must leave
//! the solution as it found it, so its assertions compare bit-for-bit with the
//! state read just before it. The cross-check literals were measured on
//! 2026-10-17 on the r4133 DLL through `epri-worker` and on the pinned
//! dss-python with the same command lists.
//!
//! The documentation's `OpenDSS Dynamics Mode` page: "The first requirement is
//! to get a solution to the base power flow to initialize the dynamics
//! solution", and "the OpenDSS program goes into dynamics solution mode for
//! FaultStudy and MonteFault (MF) solution modes". Neither oracle refuses a
//! machine without an initial state (measured 2026-10-17 as above, dss_capi
//! 0.14.5 has no WindGen). Created or enabled after `Set mode=dynamic`, it fails
//! the `Solve` with `#11001` (a primitive Y it cannot build), 482 `Access
//! violation` (PVSystem, Storage) or `#335` (VCCS), or gives NaN node voltages
//! reported not converged. Created or enabled after the last `Solve`, it makes
//! the entry raise `#303` and the run goes on. A machine re-phased during the run
//! runs on with no message (a Storage from one phase to three at 405.3207575 V
//! on both, against 395.7439151 V for a three-phase unit of that rating), and
//! one enabled again on a bus the last solve dropped is read past the end of
//! the node voltages and reported converged. A Generator created after `Set
//! mode=faultstudy` is reported converged with 0 A on every bus of `Export
//! Faultstudy`, where the circuit gives 19101 A at `WBUS` with it disabled.
//! `Disable` and `Enable` with no step between continue the run on both, and
//! the engine refuses that form too, since the ruling names an element enabled
//! again after a disable.
//!
//! A VCCS without the curves of its model fails the dynamics `Solve` with 482
//! `Access violation` on both oracles (no curves, the RMS model without
//! `filter`, a filter without points), and without `bp1` fails `Set
//! mode=dynamic` with `#303`. A `bp1` or `bp2` without points reads 0 everywhere
//! there. A waveform VCCS whose `fsample` is below the fundamental fails the
//! `Solve` with 482 `Access violation` on both once a step takes a sample
//! (measured 2026-10-23), and with a step shorter than one sample period both
//! run on with the current seeded at the entry, where the engine refuses it at
//! any step. The engine also refuses a VCCS of either model whose step is not
//! a whole number of sample steps, which both accept. The fault study and
//! MonteFault solves of a VCCS without curves complete on both with that
//! seeded current (19101.21 A of three-phase fault current at `WBUS` on
//! dss_capi, and 27.8537865832 V at `WBUS.1` under `mf`), as on the engine. The
//! documentation's VCCS page gives `bp1` and `bp2` as "not used in RMS mode",
//! the filter as the phase-locked loop of RMS mode, and requires the time step
//! to be an integer multiple of the sample step.

use num_complex::Complex64;

use super::rebuild::elem_data;
use super::*;
use crate::solution::solution::SolveMode;

/// Every class with dynamic state: (class, lines the element needs, element).
const MACHINES: [(&str, &[&str], &str); 6] = [
    ("Generator", &[], GEN),
    ("PVSystem", &[], PV),
    ("Storage", &[], STD),
    ("WindGen", &[], WG),
    ("IndMach012", &[], IM),
    ("VCCS", &CURVES_W, VCCS_W),
];
const PV1: &str = "New PVSystem.e1 bus1=wbus.1 phases=1 kv=0.398 kVA=600 Pmpp=500 irradiance=1";
const STD1: &str = "New Storage.e1 bus1=wbus.1 phases=1 kv=0.398 kWrated=200 kWhrated=800 \
                    kva=200 %stored=50 %discharge=60 state=discharging";
/// A WindGen of one phase, which has no WTG3 dynamics, and the edit that gives it
/// the rating and the three phases of `WG`.
const WG1: &str = "New WindGen.e1 bus1=wbus.1 phases=1 kv=0.398 kW=500 kva=600 conn=wye model=1 \
                   vss=1 pss=1 qss=0 vwind=12";
const WG1_TO_3: &str = "Edit WindGen.e1 phases=3 bus1=wbus kv=0.69 kW=1500 kva=1800";
const FLT: &str = "New Fault.f1 bus1=wbus phases=1 r=0.01";
/// A monitor whose sample count shows whether a solve stepped.
const LINE_MONITOR: &str = "New Monitor.ml element=Line.l1 terminal=1 mode=0";

pub(super) fn join_msg(class: &str, mode: &str) -> String {
    format!(
        "{class}.e1 joined after Set mode={mode} and has no initial state. Run Set mode=snap, \
         Solve and Set mode={mode} to start a new run that includes it."
    )
}

pub(super) fn outside_msg(class: &str, mode: &str) -> String {
    format!(
        "{class}.e1 was created, enabled, re-phased or reconnected after the last Solve before \
         Set mode={mode} and has no initial state. Run Set mode=snap, Solve and Set mode={mode} \
         to start a new run that includes it."
    )
}

pub(super) fn renumbered_msg(class: &str, mode: &str) -> String {
    format!(
        "{class}.e1 was renumbered by a bus rebuild since the last power flow and has no \
         initial state. Run Set mode=snap, Solve and Set mode={mode} to start a new run that \
         includes it."
    )
}

fn phase_msg(class: &str) -> String {
    format!(
        "{class}.e1 changed its phase count after Set mode=dynamic and has no initial state for \
         it. Run Set mode=snap, Solve and Set mode=dynamic to start a new run with it."
    )
}

const VCCS_NO_MODEL: &str =
    "VCCS.e1 has no dynamics model: set bp1, bp2 and filter before entering dynamics.";

/// The `expected_settings` case of a class (the Storage line is discharging).
fn settings_case(class: &str) -> &str {
    if class == "Storage" {
        "Storage discharging"
    } else {
        class
    }
}

/// `line` adds exactly `Error Encountered in Solve: {refusal}` (code 482) and
/// aborts the solution. Every node voltage stays bit-equal to its value before
/// the command and finite, the clock stands still and each monitor in
/// `monitors` keeps its sample count. A refused `Solve` leaves the circuit not
/// solved, as every `Solve` starts by clearing that flag, and a refused
/// step-solution command leaves it as it was. The log is cleared afterwards.
pub(super) fn assert_refused(
    dss: &mut Dss,
    case: &str,
    line: &str,
    refusal: &str,
    monitors: &[&str],
) {
    assert!(dss.errors().is_empty(), "{case}: {:?}", dss.errors());
    let v0 = node_voltages(dss);
    let solved_before = dss.circuit().expect("circuit").is_solved;
    let sol = &dss.circuit().expect("circuit").solution;
    let clock = (sol.t.to_bits(), sol.dbl_hour.to_bits());
    let samples = |dss: &Dss| -> Vec<i32> {
        monitors
            .iter()
            .map(|m| dss.monitor_view(m).expect("monitor").sample_count)
            .collect()
    };
    let samples0 = samples(dss);

    dss.command(line);
    let errors = dss.errors();
    assert_eq!(errors.len(), 1, "{case}: {line}: {errors:?}");
    assert_eq!(errors[0].code, Some(482), "{case}: {line}");
    assert_eq!(
        errors[0].message,
        format!("Error Encountered in Solve: {refusal}"),
        "{case}: {line}"
    );
    let solved_after = dss.circuit().expect("circuit").is_solved;
    let want_solved = if line == "Solve" {
        false
    } else {
        solved_before
    };
    assert_eq!(
        solved_after, want_solved,
        "{case}: {line} and the solved flag"
    );
    let sol = &dss.circuit().expect("circuit").solution;
    assert!(
        sol.solution_abort,
        "{case}: the refusal aborts the solution"
    );
    assert_eq!(
        (sol.t.to_bits(), sol.dbl_hour.to_bits()),
        clock,
        "{case}: the clock moved"
    );
    let v1 = node_voltages(dss);
    assert_eq!(v1.len(), v0.len(), "{case}: node count");
    for ((n, a), (_, b)) in v1.iter().zip(&v0) {
        assert!(a.re.is_finite() && a.im.is_finite(), "{case}: {n} = {a}");
        assert!(
            a.re.to_bits() == b.re.to_bits() && a.im.to_bits() == b.im.to_bits(),
            "{case}: {n} = {a} against {b} before {line}"
        );
    }
    assert_eq!(samples(dss), samples0, "{case}: monitor samples");
    dss.errors.clear();
}

/// The variables of a class that are settings (the `expected_settings` names).
fn setting_names(class: &str) -> &'static [&'static str] {
    match class {
        "PVSystem" => &[
            "Irradiance",
            "PanelkW",
            "P_TFactor",
            "Efficiency",
            "kW_out_desired",
            "Rated VDC",
            "Series L",
            "Max. Amps (phase)",
        ],
        "Storage" => &[
            "kWh",
            "State",
            "kWDesired",
            "Limit kWOut Function",
            "Rated VDC",
            "Series L",
            "Max. Amps (phase)",
        ],
        "WindGen" => &[
            "userTrip",
            "Pcurtail",
            "vwind",
            "dFrqPuTest",
            "QMode",
            "Qref",
        ],
        "IndMach012" => &["puRs", "puXs", "puRr", "puXr", "puXm", "Maxslip"],
        _ => &[],
    }
}

/// `{class}.e1` is outside the run: it carries no current through
/// `CktElement::get_currents` and every variable that is not a setting reads 0,
/// whatever state it kept from an earlier seed.
fn assert_outside_the_run(dss: &mut Dss, case: &str, class: &str) {
    let elem = format!("{class}.e1");
    for c in element_currents(dss, &elem) {
        assert_eq!(c, Complex64::ZERO, "{case}: current");
    }
    for (n, v) in variables(dss, &elem) {
        assert!(v.is_finite(), "{case}: {n} = {v}");
        if !setting_names(class).contains(&n.as_str()) {
            assert_eq!(v, 0.0, "{case}: {n}");
        }
    }
}

/// The variables of `elem` are bit-equal in both runs.
fn assert_same_variables(case: &str, got: &mut Dss, reference: &mut Dss, elem: &str) {
    let bits = |dss: &mut Dss| -> Vec<u64> {
        dss.element_variables(elem)
            .expect("element")
            .iter()
            .map(|v| v.to_bits())
            .collect()
    };
    assert_eq!(bits(got), bits(reference), "{case}: variables of {elem}");
}

/// Every node voltage is finite.
fn assert_finite_voltages(case: &str, dss: &Dss) {
    for (n, v) in node_voltages(dss) {
        assert!(v.re.is_finite() && v.im.is_finite(), "{case}: {n} = {v}");
    }
}

/// The magnitude of the voltage at node `node` (`BUS.N`).
pub(super) fn node_magnitude(dss: &Dss, node: &str) -> f64 {
    node_voltages(dss)
        .into_iter()
        .find(|(n, _)| n == node)
        .unwrap_or_else(|| panic!("{node}"))
        .1
        .norm()
}

/// An element of every class with dynamic state that joins a running dynamics
/// run is refused at `Solve`, whether it is created after the entry before or
/// after a step, entered disabled and enabled after a step, or disabled and
/// enabled again with or without a step between. Before the refused `Solve` it
/// carries no current and reads zero but its settings.
#[test]
fn an_element_that_joins_a_running_dynamics_run_is_refused() {
    let dir = scratch("join");
    for (class, pre, line) in MACHINES {
        let off = format!("{line} enabled=no");
        let on = format!("Edit {class}.e1 enabled=yes");
        let disable = format!("Disable {class}.e1");
        let enable = format!("Enable {class}.e1");
        let (entry2, entry5) = (dyn_entry(2), dyn_entry(5));
        let forms: [(&str, Vec<String>); 5] = [
            (
                "created after the entry",
                lines(&[&H, pre, &[LINE_MONITOR], &PF, &[entry5.as_str(), line]]),
            ),
            (
                "created after two steps",
                lines(&[&H, pre, &[LINE_MONITOR], &PF, &[&entry2, "Solve", line]]),
            ),
            (
                "enabled after two steps",
                lines(&[
                    &H,
                    pre,
                    &[off.as_str(), LINE_MONITOR],
                    &PF,
                    &[&entry2, "Solve", &on],
                ]),
            ),
            (
                "disabled for one step",
                lines(&[
                    &H,
                    pre,
                    &[line, LINE_MONITOR],
                    &PF,
                    &[&entry2, "Solve", &disable, "Set number=1", "Solve", &enable],
                ]),
            ),
            (
                "disabled and enabled between two steps",
                lines(&[
                    &H,
                    pre,
                    &[line, LINE_MONITOR],
                    &PF,
                    &[&entry2, "Solve", &disable, &enable],
                ]),
            ),
        ];
        for (form, before) in forms {
            let case = format!("{class} {form}");
            let mut dss = deck(&dir, &before);
            assert_outside_the_run(&mut dss, &case, class);
            assert_refused(
                &mut dss,
                &case,
                "Solve",
                &join_msg(class, "dynamic"),
                &["ml"],
            );
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// A seeded element whose phase count changes during the run has no state for
/// its new phases: it carries no current, reads zero but its settings and its
/// `Solve` is refused. A WindGen of one phase is refused at the entry, since
/// its WTG3 model is three-phase, and the remedy after a re-phase to three
/// phases ends where the run that has the three-phase WindGen from the start
/// ends.
#[test]
fn an_element_whose_phase_count_changes_during_the_run_is_refused() {
    let dir = scratch("phases");
    // (class, lines the element needs, element, the edit, steps before it)
    let cases: [(&str, &[&str], &str, &str, bool); 5] = [
        (
            "PVSystem",
            &[],
            PV1,
            "Edit PVSystem.e1 phases=3 bus1=wbus kv=0.69 kVA=1800 Pmpp=1500",
            false,
        ),
        (
            "Storage",
            &[],
            STD1,
            "Edit Storage.e1 phases=3 bus1=wbus kv=0.69 kWrated=500 kva=500",
            true,
        ),
        (
            "Generator",
            &[],
            GEN,
            "Edit Generator.e1 phases=1 bus1=wbus.1 kv=0.398",
            true,
        ),
        (
            "VCCS",
            &CURVES_W,
            VCCS_W,
            "Edit VCCS.e1 phases=3 bus1=wbus",
            true,
        ),
        (
            "IndMach012",
            &[],
            IM,
            "Edit IndMach012.e1 phases=1 bus1=wbus.1",
            true,
        ),
    ];
    for (class, pre, line, edit, stepped) in cases {
        let entry = dyn_entry(2);
        let mut before = lines(&[&H, pre, &[line, LINE_MONITOR], &PF, &[entry.as_str()]]);
        if stepped {
            before.push("Solve".to_string());
        }
        before.push(edit.to_string());
        let mut dss = deck(&dir, &before);
        assert_outside_the_run(&mut dss, class, class);
        assert_refused(&mut dss, class, "Solve", &phase_msg(class), &["ml"]);
    }

    let one_phase_entry = |dss: &mut Dss, case: &str| {
        dss.command(&dyn_entry(2));
        let logged: Vec<&str> = dss.errors().iter().map(|e| e.message.as_str()).collect();
        assert_eq!(
            logged,
            [
                "Dynamics mode requires a 3-phase WindGen (the WTG3 model is 3-phase-only). \
                 WindGen.e1 has 1 phases."
            ],
            "{case}: the entry seeds no WindGen of one phase"
        );
        dss.errors.clear();
    };
    let mut dss = deck(
        &dir,
        &lines(&[
            &H,
            &[WG, LINE_MONITOR],
            &PF,
            &[
                dyn_entry(2).as_str(),
                "Solve",
                "Edit WindGen.e1 phases=1 bus1=wbus.1 kv=0.398 kW=500 kva=600",
            ],
        ]),
    );
    assert_outside_the_run(&mut dss, "WindGen to one phase", "WindGen");
    assert_refused(
        &mut dss,
        "WindGen to one phase",
        "Solve",
        &phase_msg("WindGen"),
        &["ml"],
    );
    run(&mut dss, &lines(&[&["Set mode=snap", "Solve"]]));
    one_phase_entry(&mut dss, "WindGen to one phase");

    let mut dss = deck(&dir, &lines(&[&H, &[WG1, LINE_MONITOR], &PF]));
    one_phase_entry(&mut dss, "WindGen of one phase");
    run(&mut dss, &[WG1_TO_3.to_string()]);
    assert_outside_the_run(&mut dss, "WindGen", "WindGen");
    assert_refused(&mut dss, "WindGen", "Solve", &phase_msg("WindGen"), &["ml"]);
    let five = [dyn_entry(5), "Solve".to_string()];
    run(&mut dss, &lines(&[&["Set mode=snap", "Solve"]]));
    run(&mut dss, &five);
    let mut fresh = deck(&dir, &lines(&[&H, &[WG1, WG1_TO_3, LINE_MONITOR], &PF]));
    run(&mut fresh, &five);
    assert_eq!(format!("{:.6}", wbus1(&fresh)), "403.000200");
    assert_same_run(
        "WindGen re-phased to three",
        &mut dss,
        &mut fresh,
        "WindGen.e1",
        None,
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// The remedy the refusal names starts a run that includes the element, and
/// that run equals its own command list without the refused `Solve` bit for
/// bit, since the refused `Solve` returns before any solve, clock step, monitor
/// sample or state write. A run entered again after `Set mode=snap` with a
/// PVSystem created in snap mode equals the run that had it from the start.
#[test]
fn the_refusal_names_a_remedy_that_works() {
    let dir = scratch("remedy");
    let head = lines(&[&H, &PF, &[dyn_entry(5).as_str(), PV]]);
    let tail = lines(&[&["Set mode=snap", "Solve", dyn_entry(5).as_str(), "Solve"]]);
    let mut dss = deck(&dir, &head);
    assert_refused(
        &mut dss,
        "PVSystem created after the entry",
        "Solve",
        &join_msg("PVSystem", "dynamic"),
        &[],
    );
    run(&mut dss, &tail);
    let mut reference = deck(&dir, &[head, tail].concat());
    assert_same_voltages(
        "remedy",
        &dss,
        &reference,
        "in its command list without the refused Solve",
    );
    assert_same_variables("remedy", &mut dss, &mut reference, "PVSystem.e1");
    assert_eq!(format!("{:.6}", wbus1(&dss)), "401.200719");

    let mut reentry = deck(
        &dir,
        &lines(&[
            &H,
            &PF,
            &[
                dyn_entry(3).as_str(),
                "Solve",
                "Set mode=snap",
                PV,
                "Solve",
                dyn_entry(5).as_str(),
                "Solve",
            ],
        ]),
    );
    let mut live = deck(
        &dir,
        &lines(&[&H, &[PV], &PF, &[dyn_entry(5).as_str(), "Solve"]]),
    );
    assert_same_voltages("re-entry", &reentry, &live, "in the run from the start");
    assert_same_variables("re-entry", &mut reentry, &mut live, "PVSystem.e1");
    assert_eq!(format!("{:.6}", wbus1(&reentry)), "401.200719");
    std::fs::remove_dir_all(&dir).ok();
}

/// The steps of each run the rerun pins compare.
const STEPS: usize = 5;

/// The circuit the rerun pin runs `class` on: the PVSystem on [`H_Z0`], where
/// its run from the start holds the power-flow point
/// (`flat_start::a_machine_at_its_equilibrium_holds_the_power_flow_point`),
/// every other class on [`H`].
fn circuit_of(class: &str) -> &'static [&'static str] {
    if class == "PVSystem" { &H_Z0 } else { &H }
}

/// A power flow converged far below the default 1e-4.
const TIGHT: [&str; 2] = ["Set tolerance=1e-12", "Set maxiterations=200"];
/// The input slip of `IM`, written again so that its power flow restarts from it.
const IM_SLIP: &str = "Edit IndMach012.e1 slip=0.02";

/// The power-flow iterations of the last solve.
fn pf_iterations(dss: &Dss) -> i32 {
    dss.circuit().expect("circuit").solution.iteration
}

/// The `kWh` a Storage holds, `None` for any other element.
fn stored_kwh(dss: &mut Dss, elem: &str) -> Option<f64> {
    elem.starts_with("Storage").then(|| {
        variables(dss, elem)
            .into_iter()
            .find(|(n, _)| n == "kWh")
            .expect("kWh")
            .1
    })
}

/// Run `before`, then a [`STEPS`]-step entry and its `Solve`, and return the
/// `kWh` of `elem` at the entry. Neither a power flow nor the entry takes time,
/// so the entry starts from the `kWh` held before `before`, bit for bit.
fn enter_and_run(dss: &mut Dss, case: &str, elem: &str, before: &[String]) -> Option<f64> {
    let held = stored_kwh(dss, elem);
    run(dss, before);
    run(dss, &[dyn_entry(STEPS)]);
    let entry = stored_kwh(dss, elem);
    assert_eq!(
        entry.map(f64::to_bits),
        held.map(f64::to_bits),
        "{case}: the stored energy moved before the steps, {entry:?} against {held:?}"
    );
    run(dss, &["Solve".to_string()]);
    entry
}

/// `got` ends where `fresh` ends: every node voltage and every variable of
/// `elem` bit-equal, but the Storage `kWh`. Each run draws its energy from the
/// `kWh` it held at its entry (`entry`, `got` then `fresh`), the same energy
/// in both: the two drops agree within the rounding of the [`STEPS`]
/// subtractions of each run, each at most half an ulp of a value no larger
/// than its entry. The drop itself is exact, both its values within a factor
/// of two. The Storage `kWh Chng`, the change of the last step, is compared
/// bit for bit: an engine reading, exact while the `kWh` of both runs lie in
/// one binade (here near 1000 and 400 kWh), where each run subtracts the same
/// per-step energy from a value of the same exponent.
fn assert_same_run(
    case: &str,
    got: &mut Dss,
    fresh: &mut Dss,
    elem: &str,
    entry: Option<(f64, f64)>,
) {
    assert_same_voltages(case, got, fresh, "in the run from the start");
    assert_eq!(
        entry.is_some(),
        elem.starts_with("Storage"),
        "{case}: the entry kWh of a Storage and only of a Storage"
    );
    let (a, b) = (variables(got, elem), variables(fresh, elem));
    assert_eq!(a.len(), b.len(), "{case}: variable count");
    for ((n, x), (_, y)) in a.iter().zip(&b) {
        match entry {
            Some((g0, f0)) if n == "kWh" => {
                let (drop, fresh_drop) = (g0 - x, f0 - y);
                let bound = STEPS as f64 * f64::EPSILON * g0.max(f0);
                assert!(
                    fresh_drop > 0.0 && (drop - fresh_drop).abs() <= bound,
                    "{case}: kWh {x} after a drop of {drop} from {g0}, against {fresh_drop} \
                     from {f0} in the run from the start"
                );
            }
            _ => assert_eq!(x.to_bits(), y.to_bits(), "{case}: {n} = {x} against {y}"),
        }
    }
}

/// Every node voltage of `got` is within `rel` of the one in `fresh`, relative
/// to its magnitude.
fn assert_voltages_within(case: &str, got: &Dss, fresh: &Dss, rel: f64) {
    let (a, b) = (node_voltages(got), node_voltages(fresh));
    assert_eq!(a.len(), b.len(), "{case}: node count");
    for ((n, va), (m, vb)) in a.iter().zip(&b) {
        assert_eq!(n, m, "{case}: node lists");
        let gap = (va - vb).norm() / vb.norm();
        assert!(
            gap <= rel,
            "{case}: {n} = {va} against {vb}, {gap:e} relative"
        );
    }
}

/// A second run equals a first run: the entry writes every state of a machine
/// that ran before as at a first entry, so a plain second run (`Set mode=snap`,
/// `Solve`, a new entry), the remedy of a machine disabled and enabled between
/// two steps and the remedy of a phase change each end where the run from the
/// start ends (`assert_same_run`), the PVSystem on [`H_Z0`] ([`circuit_of`]).
/// The open exceptions are a WindGen linked to a `DynamicExp`, whose WTG3 model
/// no entry seeds while an equation is linked, and an IndMach012, whose next
/// power flow starts warm from the run before it.
#[test]
fn a_machine_that_ran_before_starts_the_next_run_fresh() {
    let dir = scratch("rerun");
    let snap = lines(&[&["Set mode=snap", "Solve"]]);
    let im_snap = lines(&[&["Set mode=snap", IM_SLIP, "Solve"]]);
    // |V(WBUS.1)| of the run that has the machine from the start. The VCCS
    // reading leaves both oracles, which seed its waveform history mirrored in
    // time (`a_waveform_vccs_reads_one_current_on_every_phase_and_angle`).
    let from_start = [
        "414.450879",
        "401.200934",
        "396.017603",
        "403.000200",
        "392.863048",
        "395.387862",
    ];
    for ((class, pre, line), v) in MACHINES.into_iter().zip(from_start) {
        let elem = format!("{class}.e1");
        let snap = if class == "IndMach012" {
            &im_snap
        } else {
            &snap
        };
        let head = lines(&[circuit_of(class), pre, &[line], &PF]);
        let mut fresh = deck(&dir, &head);
        let f0 = enter_and_run(&mut fresh, class, &elem, &[]);
        assert_eq!(format!("{:.6}", wbus1(&fresh)), v, "{class} from the start");

        let case = format!("{class} second run");
        let mut second = deck(&dir, &head);
        enter_and_run(&mut second, &case, &elem, &[]);
        let g0 = enter_and_run(&mut second, &case, &elem, snap);
        assert_same_run(&case, &mut second, &mut fresh, &elem, g0.zip(f0));

        let (off, on) = (format!("Disable {elem}"), format!("Enable {elem}"));
        let rejoin_head = lines(&[&[dyn_entry(2).as_str(), "Solve", off.as_str(), on.as_str()]]);
        let mut rejoin = deck(&dir, &[head, rejoin_head].concat());
        let case = format!("{class} rejoining");
        assert_refused(
            &mut rejoin,
            &case,
            "Solve",
            &join_msg(class, "dynamic"),
            &[],
        );
        let g0 = enter_and_run(&mut rejoin, &case, &elem, snap);
        assert_same_run(&case, &mut rejoin, &mut fresh, &elem, g0.zip(f0));
    }

    // The IndMach012 power flow after a run without the `slip=` edit starts
    // warm. Its iteration counts and voltages are engine readings of that open
    // item, held so that a fix turns them red, and the bound at the tight
    // tolerance is a reading with a margin, not a derived bound.
    let elem = "IndMach012.e1";
    let rejoin_head = lines(&[&[
        dyn_entry(2).as_str(),
        "Solve",
        "Disable IndMach012.e1",
        "Enable IndMach012.e1",
    ]]);
    for (tol, first, after) in [(&[][..], 3, 4), (&TIGHT[..], 13, 15)] {
        let head = lines(&[&H, &[IM], tol, &PF]);
        let mut fresh = deck(&dir, &head);
        assert_eq!(
            pf_iterations(&fresh),
            first,
            "{tol:?}: the first power flow"
        );
        enter_and_run(&mut fresh, "IndMach012", elem, &[]);
        let mut second = deck(&dir, &head);
        enter_and_run(&mut second, "IndMach012 second run", elem, &[]);
        run(&mut second, &snap);
        assert_eq!(
            pf_iterations(&second),
            after,
            "{tol:?}: the power flow after a run"
        );
        enter_and_run(&mut second, "IndMach012 second run", elem, &[]);
        let mut rejoin = deck(&dir, &[head, rejoin_head.clone()].concat());
        assert_refused(
            &mut rejoin,
            "IndMach012 rejoining",
            "Solve",
            &join_msg("IndMach012", "dynamic"),
            &[],
        );
        enter_and_run(&mut rejoin, "IndMach012 rejoining", elem, &snap);
        if tol.is_empty() {
            let read = |dss: &Dss| format!("{:.6}", wbus1(dss));
            assert_eq!(
                [read(&fresh), read(&second), read(&rejoin)],
                ["392.863048", "392.861240", "392.863639"],
                "IndMach012 from the start, after a second run and after the rejoin remedy"
            );
        } else {
            assert_voltages_within("IndMach012 second run", &second, &fresh, 1e-12);
            assert_voltages_within("IndMach012 rejoining", &rejoin, &fresh, 1e-12);
        }
    }

    let phase_changes = [
        (
            "Storage",
            STD1,
            "Edit Storage.e1 phases=3 bus1=wbus kv=0.69 kWrated=500 kva=500",
            "396.017603",
        ),
        (
            "PVSystem",
            PV1,
            "Edit PVSystem.e1 phases=3 bus1=wbus kv=0.69 kVA=1800 Pmpp=1500",
            "401.200934",
        ),
        (
            "Generator",
            GEN,
            "Edit Generator.e1 phases=1 bus1=wbus.1 kv=0.398",
            "645.666965",
        ),
    ];
    for (class, line, edit, v) in phase_changes {
        let elem = format!("{class}.e1");
        let mut fresh = deck(&dir, &lines(&[circuit_of(class), &[line, edit], &PF]));
        let f0 = enter_and_run(&mut fresh, class, &elem, &[]);
        assert_eq!(
            format!("{:.6}", wbus1(&fresh)),
            v,
            "{class} re-phased from the start"
        );
        let mut changed = deck(
            &dir,
            &lines(&[
                circuit_of(class),
                &[line],
                &PF,
                &[dyn_entry(2).as_str(), "Solve", edit],
            ]),
        );
        let case = format!("{class} re-phased after two steps");
        assert_refused(&mut changed, &case, "Solve", &phase_msg(class), &[]);
        let g0 = enter_and_run(&mut changed, &case, &elem, &snap);
        assert_same_run(&case, &mut changed, &mut fresh, &elem, g0.zip(f0));
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// The reports read an element created after the entry, mapped by
/// `Calcvoltagebases` but never solved, as carrying no current and no state,
/// through the API and in the printed report alike. Its `Export Currents` row
/// is zero on every conductor, and `Show Variables` lists it with each setting
/// at its rule (`expected_settings`) and every other variable zero.
#[test]
fn reports_read_a_joining_element_as_zero() {
    let dir = scratch("join_reports");
    let mut all = lines(&[&H, &CURVES_W, &PF, &[dyn_entry(5).as_str()]]);
    all.extend(MACHINES.iter().map(|m| setting_line(m.0, m.2)));
    all.push("Calcvoltagebases".to_string());
    let mut dss = deck(&dir, &all);
    write_setting_inputs(&mut dss, "WindGen");

    dss.command("Export Currents");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let text = std::fs::read_to_string(dir.join("dp_EXP_CURRENTS.csv")).expect("Export Currents");
    for (class, _, _) in MACHINES {
        let elem = format!("{class}.e1");
        let row = text
            .lines()
            .find(|l| {
                l.split(',')
                    .next()
                    .is_some_and(|n| n.trim().eq_ignore_ascii_case(&elem))
            })
            .unwrap_or_else(|| panic!("{elem} has a row in\n{text}"));
        for field in row.split(',').skip(1) {
            let x: f64 = field.trim().parse().expect("a number");
            assert_eq!(x, 0.0, "{elem}: {row}");
        }
    }

    dss.command("Show Variables");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let show = std::fs::read_to_string(dss.last_show_file()).expect("Show Variables file");
    for (class, _, _) in MACHINES {
        let elem = format!("{class}.e1");
        let reading = variables(&mut dss, &elem);
        let settings = expected_settings(&mut dss, settings_case(class), &elem, &reading);
        assert_zero_but_settings(&elem, &reading, &settings);
        let block: Vec<&str> = show
            .lines()
            .skip_while(|l| !l.trim().eq_ignore_ascii_case(&format!("ELEMENT: {elem}")))
            .skip(2)
            .take(reading.len())
            .collect();
        assert_eq!(block.len(), reading.len(), "{elem} in\n{show}");
        for l in block {
            let (n, v) = l.trim().split_once(" = ").expect("name = value");
            let want = settings.iter().find(|s| s.0 == n).map_or(0.0, |s| s.1);
            assert_eq!(v, crate::report::format::g(want, 6), "{elem}: {l}");
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// An element without dynamic state joins a running dynamics run like a Load,
/// since its power-flow model is its model. A UPFC or a VSConverter created
/// after the entry gives the run that has it from the start, every node voltage
/// and the UPFC's variables bit for bit.
#[test]
fn an_algebraic_element_joins_a_dynamics_run() {
    let dir = scratch("algebraic");
    // The circuit of the vendored `controls/upfc/upfc_statcom.dss`.
    let head = [
        "Clear",
        "Set DefaultBaseFrequency=60",
        "New circuit.upfcstatcom bus1=source_bus.1.0 phases=1 basekv=7.2 pu=1 angle=0 \
         mvasc3=2000000 mvasc1=20000000",
        "New xycurve.losses npts=3 xarray=[0.9 1 1.1] yarray=[1.0143 1.008 1.0143]",
        "New xfmrcode.1-ph50kva-2 phases=1 windings=2 ppm=0 xhl=2.04 %noloadloss=.02 \
         kvs=[7.2 0.24] kvas=[50 50] %rs=[0.9 0.9] conns=[wye wye]",
        "New xfmrcode.upfcinterface phases=1 windings=3 ppm=0 xhl=.0204 xht=.0204 xlt=.0136 \
         %noloadloss=.01 kvs=[0.24 0.12 0.12] kvas=[50 50 50] %rs=[0.006 .012 .012] \
         conns=[wye wye wye]",
        "New transformer.service50kva xfmrcode=1-ph50kva-2 buses=[source_bus.1.0 upfc_input.1]",
    ];
    let upfc = [
        "New upfc.test phases=1 bus1=upfc_input.1 bus2=upfc_output.1 mode=2 losscurve=losses \
         tol1=0.005 xs=0.02 pf=0.95 kvarlim=20 element=Transformer.service50kva",
        "New upfccontrol.myupfcctrl",
    ];
    let tail = [
        "New transformer.tupfcout xfmrcode=upfcinterface \
         buses=[upfc_output.1.0 load_bus.1.0 load_bus.0.2]",
        "New load.load120a phases=1 model=1 bus1=load_bus.1.0 kv=0.12 kw=14.98 kvar=10.08",
        "New load.load120b phases=1 model=1 bus1=load_bus.2.0 kv=0.12 kw=12.38 kvar=1.71",
        "Set voltagebases=[12.47 .415 0.208]",
        "calcv",
        "Set tolerance=1e-12",
        "Set maxiterations=100",
        "Set maxcontroliter=30",
        "Solve",
    ];
    let mut dss = deck(
        &dir,
        &lines(&[&head, &tail, &[dyn_entry(3).as_str()], &upfc, &["Solve"]]),
    );
    assert!(dss.circuit().expect("circuit").is_solved, "UPFC run solved");
    assert_finite_voltages("UPFC", &dss);
    // The UPFC from the start, defined after the same elements, so both runs
    // hold the same bus and element lists.
    let mut reference = deck(
        &dir,
        &lines(&[
            &head,
            &tail[..3],
            &upfc,
            &tail[3..],
            &[dyn_entry(3).as_str(), "Solve"],
        ]),
    );
    assert_same_voltages("UPFC", &dss, &reference, "in the run from the start");
    assert_same_variables("UPFC", &mut dss, &mut reference, "UPFC.test");
    assert_eq!(
        format!("{:.6}", node_magnitude(&dss, "LOAD_BUS.1")),
        "117.830754"
    );

    let dss = deck(
        &dir,
        &lines(&[&VSC_HEAD, &VSC_TAIL, &[dyn_entry(5).as_str(), VSC, "Solve"]]),
    );
    assert!(
        dss.circuit().expect("circuit").is_solved,
        "VSConverter run solved"
    );
    assert_finite_voltages("VSConverter", &dss);
    let reference = deck(
        &dir,
        &lines(&[
            &VSC_HEAD,
            &VSC_TAIL[..3],
            &[VSC],
            &VSC_TAIL[3..],
            &[dyn_entry(5).as_str(), "Solve"],
        ]),
    );
    assert_same_voltages("VSConverter", &dss, &reference, "in the run from the start");
    assert_eq!(
        format!("{:.6}", node_magnitude(&dss, "DC2.1")),
        "49500.000001"
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// A machine's fault contribution is its internal voltage behind its transient
/// reactance from the entry, so one created after `Set mode=faultstudy` or `Set
/// mode=mf` is refused by the solve and `Show Fault` prints nothing non-finite.
/// A Generator already in the power flow solves both modes to finite values.
#[test]
fn a_machine_that_joins_a_fault_study_is_refused() {
    let dir = scratch("fault_join");
    let show_fault_is_finite = |dss: &mut Dss, case: &str| {
        dss.command("Show Fault");
        assert!(dss.errors().is_empty(), "{case}: {:?}", dss.errors());
        let text = std::fs::read_to_string(dss.last_show_file()).expect("Show Fault file");
        let lower = text.to_lowercase();
        assert!(
            !lower.contains("nan") && !lower.contains("inf"),
            "{case}: non-finite value in Show Fault\n{text}"
        );
    };
    for (class, line) in [("Generator", GEN), ("PVSystem", PV), ("IndMach012", IM)] {
        let mut dss = deck(&dir, &lines(&[&H, &PF, &["Set mode=faultstudy", line]]));
        assert_refused(
            &mut dss,
            class,
            "Solve",
            &join_msg(class, "faultstudy"),
            &[],
        );
        show_fault_is_finite(&mut dss, class);
    }
    let mut dss = deck(
        &dir,
        &lines(&[&H, &[FLT], &PF, &["Set mode=mf number=2", GEN]]),
    );
    assert_refused(&mut dss, "mf", "Solve", &join_msg("Generator", "mf"), &[]);

    let mut dss = deck(
        &dir,
        &lines(&[&H, &[GEN], &PF, &["Set mode=faultstudy", "Solve"]]),
    );
    show_fault_is_finite(&mut dss, "Generator in the power flow");
    assert_eq!(format!("{:.6}", wbus1(&dss)), "0.015567");
    let dss = deck(
        &dir,
        &lines(&[&H, &[FLT, GEN], &PF, &["Set mode=mf number=2", "Solve"]]),
    );
    assert_finite_voltages("mf control", &dss);
    assert_eq!(format!("{:.6}", wbus1(&dss)), "140.477892");
    std::fs::remove_dir_all(&dir).ok();
}

/// The machine `line` moved to `gbus` behind `Line.l2` (one-phase for the
/// VCCS), solved, then the line and the machine disabled, solved without them
/// (`gbus` leaves the bus list, so the node count falls below the machine's
/// node numbers) and enabled again.
pub(super) fn dropped_bus(class: &str, pre: &[&str], line: &str) -> Vec<String> {
    let l2 = if class == "VCCS" {
        "New Line.l2 bus1=wbus.1 bus2=gbus.1 phases=1 r1=0.005 x1=0.02 length=1"
    } else {
        "New Line.l2 bus1=wbus bus2=gbus phases=3 r1=0.005 x1=0.02 length=1"
    };
    let at_gbus = line.replace("bus1=wbus", "bus1=gbus");
    assert_ne!(at_gbus, line, "{class}: the machine moves to gbus");
    let off = format!("Disable {class}.e1");
    let on = format!("Enable {class}.e1");
    lines(&[
        &H,
        pre,
        &[l2, at_gbus.as_str()],
        &PF,
        &[
            "Disable Line.l2",
            off.as_str(),
            "Solve",
            "Enable Line.l2",
            on.as_str(),
        ],
    ])
}

/// An element no solved power flow included in its present shape (created, enabled, re-phased,
/// reconnected or given another bus after it) is seeded with nothing and refused at the first
/// solve with the remedy. The remedy run equals the same command list without the refused `Solve`.
#[test]
fn a_machine_the_last_solve_did_not_include_is_refused_at_the_first_solve() {
    let dir = scratch("not_included");
    let check = |case: &str, class: &str, before: &[String], mode: &str, rerun: &str| {
        let elem = format!("{class}.e1");
        let entry = format!("Set mode={mode}");
        let start = || {
            let mut dss = deck(&dir, &[before, std::slice::from_ref(&entry)].concat());
            write_setting_inputs(&mut dss, class);
            dss
        };
        let mut dss = start();
        let want_mode = if mode == "dynamic" {
            SolveMode::Dynamic
        } else {
            SolveMode::FaultStudy
        };
        assert_eq!(
            dss.circuit().expect("circuit").solution.mode,
            want_mode,
            "{case}"
        );
        let reading = variables(&mut dss, &elem);
        let settings = expected_settings(&mut dss, settings_case(class), &elem, &reading);
        assert_zero_but_settings(case, &reading, &settings);
        for c in element_currents(&mut dss, &elem) {
            assert_eq!(c, Complex64::ZERO, "{case}: current");
        }
        assert_refused(&mut dss, case, "Solve", &outside_msg(class, mode), &[]);

        let tail = ["Set mode=snap", "Solve", rerun, "Solve"].map(String::from);
        run(&mut dss, &tail);
        assert_finite_voltages(case, &dss);
        let mut reference = start();
        run(&mut reference, &tail);
        assert_same_voltages(
            case,
            &dss,
            &reference,
            "in its command list without the refused Solve",
        );
        assert_same_variables(case, &mut dss, &mut reference, &elem);
    };
    let entry5 = dyn_entry(5);
    for (class, pre, line) in MACHINES {
        let line = setting_line(class, line);
        let line = line.as_str();
        check(
            &format!("{class} created after the power flow"),
            class,
            &lines(&[&H, pre, &PF, &[line]]),
            "dynamic",
            &entry5,
        );
        let off = format!("{line} enabled=no");
        let on = format!("Enable {class}.e1");
        check(
            &format!("{class} enabled after the power flow"),
            class,
            &lines(&[&H, pre, &[off.as_str()], &PF, &[on.as_str()]]),
            "dynamic",
            &entry5,
        );
        check(
            &format!("{class} enabled again on a bus the last solve dropped"),
            class,
            &dropped_bus(class, pre, line),
            "dynamic",
            &entry5,
        );
    }
    check(
        "Generator enabled again on a dropped bus before Set mode=faultstudy",
        "Generator",
        &dropped_bus("Generator", &[], GEN),
        "faultstudy",
        "Set mode=faultstudy",
    );
    check(
        "PVSystem re-phased after the power flow",
        "PVSystem",
        &lines(&[
            &H,
            &[PV1],
            &PF,
            &["Edit PVSystem.e1 phases=3 bus1=wbus kv=0.69 kVA=1800 Pmpp=1500"],
        ]),
        "dynamic",
        &entry5,
    );
    check(
        "PVSystem reconnected after the power flow",
        "PVSystem",
        &lines(&[&H, &[PV], &PF, &["Edit PVSystem.e1 conn=delta"]]),
        "dynamic",
        &entry5,
    );
    check(
        "Generator created before Set mode=faultstudy",
        "Generator",
        &lines(&[&H, &PF, &[GEN]]),
        "faultstudy",
        "Set mode=faultstudy",
    );
    // The Generator defines `gbus` before `Line.l1` defines `wbus`, so the
    // solve without it numbers `wbus` first and `gbus` takes other nodes.
    let first_on_gbus = GEN.replace("bus1=wbus", "bus1=gbus");
    check(
        "Generator enabled again after a solve that renumbered its bus",
        "Generator",
        &lines(&[
            &[
                H[0],
                H[1],
                first_on_gbus.as_str(),
                H[2],
                H[3],
                "New Line.l2 bus1=wbus bus2=gbus phases=3 r1=0.005 x1=0.002 length=1",
                "New Load.lg bus1=gbus phases=3 kv=0.69 kW=10 kvar=0",
            ],
            &PF,
            &["Disable Generator.e1", "Solve", "Enable Generator.e1"],
        ]),
        "dynamic",
        &entry5,
    );
    let kept = lines(&[
        &H,
        &[GEN],
        &PF,
        &["Disable Generator.e1", "Solve", "Enable Generator.e1"],
    ]);
    check(
        "Generator enabled again on the nodes it had",
        "Generator",
        &kept,
        "dynamic",
        &entry5,
    );
    check(
        "Generator enabled again on the nodes it had before Set mode=faultstudy",
        "Generator",
        &kept,
        "faultstudy",
        "Set mode=faultstudy",
    );
    check(
        "PVSystem given another bus after the power flow",
        "PVSystem",
        &lines(&[&H, &[PV], &PF, &["Edit PVSystem.e1 bus1=srcbus"]]),
        "dynamic",
        &entry5,
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// A Generator disabled, left out of a solve that kept its nodes and enabled again keeps its node
/// map for the readers but is outside the last power flow, so the dynamics entry, FaultStudy,
/// MonteFault and the step commands refuse it with the outside message and change nothing.
/// A snap or direct `Solve` after the `Enable` takes it in, and the run then starts from that power flow.
#[test]
fn a_machine_enabled_again_on_its_nodes_is_outside_the_run() {
    let dir = scratch("kept_outside");
    const MG: &str = "New Monitor.mg element=Generator.e1 terminal=1 mode=0";
    let kept = |extra: &[&str], entry: &str| {
        lines(&[
            &H,
            extra,
            &[GEN, MG],
            &PF,
            &[
                "Disable Generator.e1",
                "Solve",
                "Enable Generator.e1",
                entry,
            ],
        ])
    };
    let readers = [
        "Export Powers",
        "Export Currents",
        "Export VoltagesElements",
        "Show Powers Elements",
        "Show Currents Elements",
        "Show Voltages LN Elements",
        "Sample",
        "Export Monitors mg",
    ]
    .map(String::from);
    let entry5 = dyn_entry(5);
    for (mode, extra, entry, cmd) in [
        ("dynamic", &[][..], entry5.as_str(), "Solve"),
        ("faultstudy", &[][..], "Set mode=faultstudy", "Solve"),
        ("mf", &[FLT][..], "Set mode=mf number=2", "Solve"),
        ("dynamic", &[][..], entry5.as_str(), "_SolvePFlow"),
    ] {
        let case = format!("{cmd} after Set mode={mode}");
        let mut dss = deck(&dir, &kept(extra, entry));
        let cd = super::rebuild::elem_data(&dss, "Generator.e1");
        assert!(cd.is_mapped(), "{case}: the kept node map");
        assert!(!cd.in_power_flow, "{case}: no power flow solved with it");
        assert_refused(
            &mut dss,
            &case,
            cmd,
            &outside_msg("Generator", mode),
            &["mg"],
        );
        run(&mut dss, &readers);
    }

    // Enabled again with no solve between counts as enabled after the last
    // power flow too.
    let mut dss = deck(
        &dir,
        &lines(&[
            &H,
            &[GEN, MG],
            &PF,
            &["Disable Generator.e1", "Enable Generator.e1", &entry5],
        ]),
    );
    assert_refused(
        &mut dss,
        "enabled again with no solve between",
        "Solve",
        &outside_msg("Generator", "dynamic"),
        &["mg"],
    );

    let mut dss = deck(&dir, &kept(&[], "Solve"));
    run(&mut dss, &[entry5.clone(), "Solve".to_string()]);
    assert_eq!(
        format!("{:.6}", wbus1(&dss)),
        "414.450704",
        "kept, then solved"
    );
    let fresh = deck(&dir, &lines(&[&H, &[GEN, MG], &PF, &[&entry5, "Solve"]]));
    assert_eq!(
        format!("{:.6}", wbus1(&fresh)),
        "414.450879",
        "from the start"
    );
    // A direct solve holds it too, as the admittance it stamps.
    let mut dss = deck(&dir, &kept(&[], "Set mode=direct"));
    run(
        &mut dss,
        &["Solve".to_string(), entry5.clone(), "Solve".to_string()],
    );
    assert_finite_voltages("kept, then a direct solve", &dss);
    std::fs::remove_dir_all(&dir).ok();
}

/// A machine given another phase count or connection after the last power flow,
/// through `phases=`, `conn=` or `like=`, is outside the run whether its
/// conductor count changed or stayed, also after a bus rebuild that maps it in
/// its new shape, so the first `Solve` refuses it and the remedy run equals its
/// own command list without the refused `Solve`. An edit that keeps both is no
/// reshape, and the entry seeds the machine.
#[test]
fn a_machine_reshaped_on_its_conductors_is_outside_the_run() {
    let dir = scratch("reshaped_outside");
    let g2d = "New Generator.e1 bus1=wbus phases=2 conn=delta kv=0.69 kW=1000 kva=1200 model=1";
    let g2w = "New Generator.e1 bus1=wbus phases=2 conn=wye kv=0.69 kW=1000 kva=1200 model=1";
    let g3 = "New Generator.g3 bus1=wbus phases=3 kv=0.69 kW=10 kva=12 model=1";
    let g1 = "New Generator.e1 bus1=wbus.1 phases=1 kv=0.398 kW=500 kva=600 model=1";
    let forms: [(&str, &str, Vec<String>); 5] = [
        (
            "phases=3 on a two-phase delta Generator",
            "Generator",
            lines(&[&H, &[g2d], &PF, &["Edit Generator.e1 phases=3"]]),
        ),
        (
            "conn=delta phases=3 on a two-phase wye Generator",
            "Generator",
            lines(&[&H, &[g2w], &PF, &["Edit Generator.e1 conn=delta phases=3"]]),
        ),
        (
            "like= of a three-phase Generator onto an enabled two-phase one",
            "Generator",
            lines(&[&H, &[g2w, g3], &PF, &["Edit Generator.e1 like=g3"]]),
        ),
        (
            "conn=delta on a one-phase Generator, then MakeBusList",
            "Generator",
            lines(&[
                &H,
                &[g1],
                &PF,
                &["Edit Generator.e1 conn=delta", "MakeBusList"],
            ]),
        ),
        (
            "PVSystem reconnected after the power flow, then MakeBusList",
            "PVSystem",
            lines(&[
                &H,
                &[PV],
                &PF,
                &["Edit PVSystem.e1 conn=delta", "MakeBusList"],
            ]),
        ),
    ];
    let entry5 = dyn_entry(5);
    for (case, class, before) in &forms {
        let elem = format!("{class}.e1");
        let start = || deck(&dir, &[before.clone(), vec![entry5.clone()]].concat());
        let mut dss = start();
        assert!(
            !elem_data(&dss, &elem).in_power_flow,
            "{case}: outside the last power flow"
        );
        assert_refused(&mut dss, case, "Solve", &outside_msg(class, "dynamic"), &[]);
        let tail = ["Set mode=snap", "Solve", entry5.as_str(), "Solve"].map(String::from);
        run(&mut dss, &tail);
        assert_finite_voltages(case, &dss);
        let mut reference = start();
        run(&mut reference, &tail);
        assert_same_voltages(
            case,
            &dss,
            &reference,
            "in its command list without the refused Solve",
        );
        assert_same_variables(case, &mut dss, &mut reference, &elem);
    }
    let g3d = "New Generator.e1 bus1=wbus phases=3 conn=delta kv=0.69 kW=1000 kva=1200 model=1";
    for (case, machine) in [
        ("phases=3 on a three-phase wye Generator", GEN),
        ("phases=3 on a three-phase delta Generator", g3d),
    ] {
        let mut dss = deck(
            &dir,
            &lines(&[&H, &[machine], &PF, &["Edit Generator.e1 phases=3"]]),
        );
        assert!(
            elem_data(&dss, "Generator.e1").in_power_flow,
            "{case}: in the last power flow"
        );
        run(&mut dss, &[entry5.clone(), "Solve".to_string()]);
        assert_eq!(
            elem_data(&dss, "Generator.e1").dynamics_seed,
            crate::elements::ckt::DynamicsSeed::Seeded(3),
            "{case}: seeded"
        );
        assert!(
            dss.circuit().expect("circuit").solution.converged_flag,
            "{case}: converged"
        );
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// A step-solution command that solves injects every element of the run, so in
/// a dynamics-model mode `_SolveNoControl`, `_SolvePFlow`, `_SolveDirect` and
/// `_DoControlActions` under the multi-rate control mode refuse an element
/// without a state in the run as `Solve` does. In snap mode with the PVSystem
/// in the power flow the three solve commands add no error.
#[test]
fn a_step_solution_command_refuses_a_joining_element() {
    let dir = scratch("step_cmds");
    let joined = lines(&[&H, &PF, &["Set mode=dynamic", PV]]);
    for cmd in ["_SolveNoControl", "_SolvePFlow", "_SolveDirect"] {
        let mut dss = deck(&dir, &joined);
        assert_refused(&mut dss, cmd, cmd, &join_msg("PVSystem", "dynamic"), &[]);
    }
    let mut dss = deck(
        &dir,
        &lines(&[
            &H,
            &PF,
            &["Set mode=dynamic", "Set controlmode=multirate", PV],
        ]),
    );
    assert_refused(
        &mut dss,
        "_DoControlActions",
        "_DoControlActions",
        &join_msg("PVSystem", "dynamic"),
        &[],
    );

    let mut dss = deck(&dir, &lines(&[&H, &[PV], &PF]));
    run(
        &mut dss,
        &["_SolveNoControl", "_SolvePFlow", "_SolveDirect"].map(String::from),
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// A step-solution command reports the error of its solve as `Solve` does and
/// leaves the solution aborted when a three-phase delta `cmatrix` capacitor
/// bank, which has no primitive admittance, makes the next Y build fail.
/// `_DoControlActions` under the multi-rate control mode reaches the build
/// through the re-solve after a switching that a `CapControl` created after the
/// power flow queues at `_SampleControls`.
#[test]
fn a_step_solution_command_reports_the_error_of_its_solve() {
    let dir = scratch("step_errors");
    let bank = "New Capacitor.cd bus1=wbus phases=3 conn=delta cmatrix=[10 | -2 10 | -2 -2 10]";
    let reported = |dss: &mut Dss, cmd: &str| -> Vec<(String, Option<u32>)> {
        run(dss, &[bank.to_string()]);
        dss.command(cmd);
        assert!(
            dss.circuit().expect("circuit").solution.solution_abort,
            "{cmd}: aborted"
        );
        let errors = dss
            .errors()
            .iter()
            .map(|e| (e.message.clone(), e.code))
            .collect();
        dss.errors.clear();
        errors
    };
    let solved = lines(&[&H, &PF]);
    let want = reported(&mut deck(&dir, &solved), "Solve");
    assert_eq!(
        want.last(),
        Some(&(
            "Error Encountered in Solve: an element has no defined primitive admittance"
                .to_string(),
            Some(482)
        )),
        "{want:?}"
    );
    for cmd in ["_SolveNoControl", "_SolvePFlow"] {
        assert_eq!(reported(&mut deck(&dir, &solved), cmd), want, "{cmd}");
    }
    let queued = lines(&[
        &H,
        &["New Capacitor.c1 bus1=wbus phases=3 kvar=100 kv=0.69"],
        &PF,
        &[
            "New CapControl.cc1 element=Line.l1 terminal=2 capacitor=c1 type=voltage ON=100 \
             OFF=300 PTratio=1 Delay=0.5 DelayOff=0.5",
            "Set controlmode=multirate",
            "Set stepsize=1",
            "_SampleControls",
        ],
    ]);
    assert_eq!(
        reported(&mut deck(&dir, &queued), "_DoControlActions"),
        want,
        "_DoControlActions"
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// The numbers of a report row, read from its tokens that parse as numbers
/// once the brackets around a per-unit value are dropped.
fn row_numbers(row: &str) -> Vec<f64> {
    row.split(|c: char| c == ',' || c.is_whitespace() || c == '(' || c == ')')
        .filter_map(|f| f.parse().ok())
        .collect()
}

/// The rows of `elem`'s block in a `Show ... Elements` report: the lines after
/// its `ELEMENT = "..."` line up to the first empty one.
fn element_block<'a>(report: &'a str, elem: &str) -> Vec<&'a str> {
    let head = format!("ELEMENT = \"{elem}\"").to_ascii_uppercase();
    report
        .lines()
        .skip_while(|l| l.trim().to_ascii_uppercase() != head)
        .skip(1)
        .take_while(|l| !l.trim().is_empty())
        .collect()
}

/// A machine enabled again on a bus the last solve dropped, or created after
/// the last bus rebuild, has no node map and carries no current, and each
/// export and show report here reads it at node 0 with zero current, power and
/// volts or leaves it out, in snap mode and in the dynamics-model modes. The
/// readers that leave such an element out as a disabled one are pinned by
/// `rebuild::readers_of_an_element_enabled_after_a_bus_rebuild`.
#[test]
fn readers_of_an_element_enabled_on_a_dropped_bus_read_zero() {
    let dir = scratch("dropped_bus_readers");
    let shows = [
        "Show Powers Elements",
        "Show Currents Elements",
        "Show Voltages LN Elements",
        "Show Voltages LL Elements",
    ];
    let exports = [
        ("Export Currents", "dp_EXP_CURRENTS.csv", 1),
        ("Export Powers", "dp_EXP_POWERS.csv", 2),
        ("Export Seqcurrents", "dp_EXP_SEQCURRENTS.csv", 2),
        ("Export Losses", "dp_EXP_LOSSES.csv", 0),
        ("Export Overloads", "dp_EXP_OVERLOADS.csv", 0),
    ];
    for (class, line) in [("Generator", GEN), ("PVSystem", PV)] {
        let gbus = line.replace("bus1=wbus", "bus1=gbus");
        let created = lines(&[
            &H,
            &PF,
            &[
                "New Line.l2 bus1=wbus bus2=gbus phases=3 r1=0.005 x1=0.02 length=1",
                &gbus,
            ],
        ]);
        for (form, before) in [
            ("dropped", dropped_bus(class, &[], line)),
            ("created", created),
        ] {
            for entry in [
                "",
                "Set mode=dynamic stepsize=0.001 number=5",
                "Set mode=faultstudy",
            ] {
                let case = format!("{class} {form} [{entry}]");
                let mut all = before.clone();
                all.extend((!entry.is_empty()).then(|| entry.to_string()));
                let mut dss = deck(&dir, &all);
                let elem = format!("{class}.e1");
                assert!(
                    dss.circuit().expect("circuit").solution.node_v.len() <= 7,
                    "{case}"
                );
                let cd = elem_data(&dss, &elem);
                assert!(cd.node_ref.is_empty(), "{case}: {:?}", cd.node_ref);
                let conductors = cd.nterms * cd.nconds;
                for c in element_currents(&mut dss, &elem) {
                    assert_eq!(c, Complex64::ZERO, "{case}: current");
                }
                let upper = elem.to_ascii_uppercase();
                for (cmd, file, skip) in exports {
                    run(&mut dss, &[cmd.to_string()]);
                    let text = std::fs::read_to_string(dir.join(file)).expect("export file");
                    let rows: Vec<&str> = text
                        .lines()
                        .filter(|l| {
                            let l = l.trim_start().trim_start_matches('"').to_ascii_uppercase();
                            l.strip_prefix(&upper)
                                .is_some_and(|r| r.starts_with('"') || r.starts_with(','))
                        })
                        .collect();
                    if skip == 0 {
                        assert!(rows.is_empty(), "{case}: {cmd} lists it: {rows:?}");
                        continue;
                    }
                    assert!(!rows.is_empty(), "{case}: no row in {cmd}\n{text}");
                    for row in rows {
                        let values: Vec<f64> = row
                            .split(',')
                            .skip(skip)
                            .map(|v| v.trim().parse().expect("a number"))
                            .collect();
                        assert!(
                            !values.is_empty() && values.iter().all(|&v| v == 0.0),
                            "{case}: {cmd}: {row}"
                        );
                    }
                }
                run(&mut dss, &["Export Summary", "Sample"].map(String::from));
                for cmd in shows {
                    run(&mut dss, &[cmd.to_string()]);
                    let text = std::fs::read_to_string(dss.last_show_file()).expect("show file");
                    let block = element_block(&text, &elem);
                    let powers = cmd.starts_with("Show Powers");
                    // The power report adds its terminal total.
                    let want_rows = conductors + usize::from(powers);
                    assert_eq!(block.len(), want_rows, "{case}: {cmd}\n{text}");
                    for row in block {
                        let mut values = row_numbers(row);
                        if powers {
                            assert_eq!(values.pop(), Some(1.0), "{case}: {cmd}: {row}");
                        }
                        assert!(
                            !values.is_empty() && values.iter().all(|&v| v == 0.0),
                            "{case}: {cmd}: {row}"
                        );
                    }
                }
            }
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// A non-finite state is no solution: a Generator in the run whose `Theta` is
/// made NaN through its variable setter drives the node voltages non-finite,
/// and the dynamics solve reports that it did not converge. In FaultStudy the
/// direct solution fails with the first non-finite node named and leaves the
/// circuit neither solved nor converged, through `Solve` and through
/// `_SolveDirect` alike.
#[test]
fn a_non_finite_state_is_never_a_solution() {
    let dir = scratch("non_finite");
    let mut dss = deck(&dir, &lines(&[&H, &[GEN], &PF, &[dyn_entry(1).as_str()]]));
    set_state(&mut dss, "Generator.e1", 2, f64::NAN);
    dss.command("Solve");
    let ckt = dss.circuit().expect("circuit");
    assert!(!ckt.solution.converged_flag, "dynamics: converged on NaN");
    assert!(!ckt.is_solved, "dynamics: solved on NaN");
    assert!(
        node_voltages(&dss)
            .iter()
            .any(|(_, v)| !v.re.is_finite() || !v.im.is_finite()),
        "the NaN state reaches the network"
    );

    // Each command in its own deck: the power flow left the circuit solved and
    // converged, so the failed direct solution itself clears both flags.
    for cmd in ["Solve", "_SolveDirect"] {
        let mut dss = deck(&dir, &lines(&[&H, &[GEN], &PF, &["Set mode=faultstudy"]]));
        let ckt = dss.circuit().expect("circuit");
        assert!(
            ckt.is_solved && ckt.solution.converged_flag,
            "{cmd}: the power flow solved"
        );
        set_state(&mut dss, "Generator.e1", 2, f64::NAN);
        dss.command(cmd);
        let ckt = dss.circuit().expect("circuit");
        assert!(!ckt.is_solved, "{cmd}: solved on NaN");
        assert!(!ckt.solution.converged_flag, "{cmd}: converged on NaN");
        let first = (1..=ckt.num_nodes)
            .find(|&i| {
                let v = ckt.solution.node_v[i];
                !v.re.is_finite() || !v.im.is_finite()
            })
            .map(|i| ckt.node_name(i))
            .expect("a non-finite node");
        let errors = dss.errors();
        assert_eq!(errors.len(), 1, "{cmd}: {errors:?}");
        assert_eq!(errors[0].code, Some(482), "{cmd}");
        assert_eq!(
            errors[0].message,
            format!(
                "Error Encountered in Solve: The direct solution has a non-finite voltage at \
                 node {first}. It is not a solution."
            ),
            "{cmd}"
        );
        dss.errors.clear();
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// A VCCS without the curves with points of its model (`bp1`, `bp2` and
/// `filter` for the waveform model, `filter` for the RMS model), a finite
/// `fsample` or, in the waveform model, a sample in its one-cycle window is
/// refused by the dynamics solve, naming what is missing, and its entry seeds
/// and keeps the variables of the same VCCS with its model. A waveform VCCS
/// sampled at the fundamental and an RMS VCCS sampled below it run, since the
/// window then holds one sample and the RMS model has no window.
#[test]
fn a_vccs_without_its_curves_refuses_the_dynamics_solve() {
    let dir = scratch("vccs_no_model");
    let bp1_missing = format!("{VCCS_NC} bp2=bp2_micro filter=z_micro fsample=10000");
    let no_filter = format!("{VCCS_NC} bp1=bp1_micro bp2=bp2_micro fsample=10000");
    let rms_bare = format!("{VCCS_NC} rmsmode=true");
    let with_empty: Vec<&str> = CURVES_W
        .iter()
        .copied()
        .chain(["New XYcurve.zz npts=0"])
        .collect();
    let empty_filter = format!("{VCCS_NC} bp1=bp1_micro bp2=bp2_micro filter=zz fsample=10000");
    let rms_empty_filter = format!(
        "{VCCS_NC} filter=zz fsample=10000 rmsmode=true imaxpu=1.15 vrmstau=0.01 irmstau=0.05"
    );
    let empty_bp1 = format!("{VCCS_NC} bp1=zz bp2=bp2_micro filter=z_micro fsample=10000");
    let empty_bp2_no_filter = format!("{VCCS_NC} bp1=bp1_micro bp2=zz fsample=10000");
    let empty_bp1_bp2 = format!("{VCCS_NC} bp1=zz bp2=zz filter=z_micro fsample=10000");
    let fs_nan = format!("{VCCS_NC} bp1=bp1_micro bp2=bp2_micro filter=z_micro fsample=NaN");
    let empty_filter_msg = "VCCS.e1 has no dynamics model: its filter XYcurve.zz has no points.";
    // A one-cycle window of trunc(fsample / 60) slots holds no sample below 60 Hz.
    let fs50 = format!("{VCCS_NC} bp1=bp1_micro bp2=bp2_micro filter=z_micro fsample=50");
    let fs59 = format!("{VCCS_NC} bp1=bp1_micro bp2=bp2_micro filter=z_micro fsample=59");
    let window_msg = |fs: u32| {
        format!(
            "VCCS.e1 has no dynamics model: its fsample of {fs} Hz is below the fundamental of 60 Hz, so its one-cycle RMS window holds no sample."
        )
    };
    let (fs50_msg, fs59_msg) = (window_msg(50), window_msg(59));
    let cases: [(&str, &[&str], &str, &str); 12] = [
        ("no curves", &[], VCCS_NC, VCCS_NO_MODEL),
        (
            "bp1 missing",
            &CURVES_W,
            &bp1_missing,
            "VCCS.e1 has no dynamics model: set bp1 before entering dynamics.",
        ),
        (
            "bp1 and bp2 without a filter",
            &CURVES_W,
            &no_filter,
            "VCCS.e1 has no dynamics model: set filter before entering dynamics.",
        ),
        (
            "RMS without its filter",
            &[],
            &rms_bare,
            "VCCS.e1 has no dynamics model: set filter before entering dynamics.",
        ),
        (
            "a filter without points",
            &with_empty,
            &empty_filter,
            empty_filter_msg,
        ),
        (
            "RMS with a filter without points",
            &with_empty,
            &rms_empty_filter,
            empty_filter_msg,
        ),
        (
            "a bp1 without points",
            &with_empty,
            &empty_bp1,
            "VCCS.e1 has no dynamics model: its bp1 XYcurve.zz has no points.",
        ),
        (
            "a bp2 without points and no filter",
            &with_empty,
            &empty_bp2_no_filter,
            "VCCS.e1 has no dynamics model: set filter before entering dynamics, and its bp2 \
             XYcurve.zz has no points.",
        ),
        ("fsample below the fundamental", &CURVES_W, &fs50, &fs50_msg),
        (
            "fsample just below the fundamental",
            &CURVES_W,
            &fs59,
            &fs59_msg,
        ),
        (
            "bp1 and bp2 without points",
            &with_empty,
            &empty_bp1_bp2,
            "VCCS.e1 has no dynamics model: its bp1 XYcurve.zz and its bp2 XYcurve.zz have no \
             points.",
        ),
        (
            "fsample not a number",
            &CURVES_W,
            &fs_nan,
            "VCCS.e1 has no dynamics model: its fsample of NaN Hz is not a finite number.",
        ),
    ];
    let bits = |v: Vec<f64>| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    let entry = dyn_entry(5);
    for (case, pre, line, refusal) in cases {
        let mut dss = deck(&dir, &lines(&[&H, pre, &[line], &PF, &[entry.as_str()]]));
        let model = if line.contains("rmsmode=true") {
            lines(&[&H, &CURVE_R, &[VCCS_R], &PF, &[entry.as_str()]])
        } else {
            lines(&[&H, &CURVES_W, &[VCCS_W], &PF, &[entry.as_str()]])
        };
        let mut model = deck(&dir, &model);
        let seeded = dss.element_variables("VCCS.e1").expect("VCCS");
        assert_eq!(
            bits(seeded.clone()),
            bits(model.element_variables("VCCS.e1").expect("VCCS")),
            "{case}: the entry seeds what a direct solve reads, {seeded:?}"
        );
        assert_refused(&mut dss, case, "Solve", refusal, &[]);
        let after = dss.element_variables("VCCS.e1").expect("VCCS");
        assert_eq!(
            bits(after.clone()),
            bits(seeded.clone()),
            "{case}: the refused solve changed the state, {after:?} against {seeded:?}"
        );
    }

    let at_fundamental = format!("{VCCS_NC} bp1=bp1_micro bp2=bp2_micro filter=z_micro fsample=60");
    let rms_below = format!(
        "{VCCS_NC} filter=z_pll fsample=50 rmsmode=true imaxpu=1.15 vrmstau=0.01 irmstau=0.05"
    );
    // Each step a whole number of samples: 3 at 60 Hz, 1 at 50 Hz.
    for (case, pre, line, h) in [
        (
            "fsample at the fundamental",
            &CURVES_W[..],
            &at_fundamental,
            "0.05",
        ),
        (
            "RMS below the fundamental",
            &CURVE_R[..],
            &rms_below,
            "0.02",
        ),
    ] {
        let entry = format!("Set mode=dynamic stepsize={h} number={STEPS}");
        let dss = deck(
            &dir,
            &lines(&[&H, pre, &[line.as_str()], &PF, &[entry.as_str(), "Solve"]]),
        );
        assert!(dss.circuit().expect("circuit").is_solved, "{case}: solved");
        assert_finite_voltages(case, &dss);
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// A VCCS without the curves or the window sample of its model takes part in
/// the FaultStudy and MonteFault solves at the current seeded from the power
/// flow, as a converter in a fault keeps the current it carried before. Every
/// bus voltage, fault current and the VCCS's own current are bit-equal to the
/// waveform VCCS's.
#[test]
fn a_vccs_without_its_curves_takes_part_in_a_fault_study() {
    let dir = scratch("vccs_fault_study");
    let with_empty: Vec<&str> = CURVES_W
        .iter()
        .copied()
        .chain(["New XYcurve.zz npts=0"])
        .collect();
    let forms: [(&str, &[&str], String); 6] = [
        ("with its curves", &CURVES_W, VCCS_W.to_string()),
        ("RMS with its filter", &CURVE_R, VCCS_R.to_string()),
        ("no curves", &[], VCCS_NC.to_string()),
        (
            "RMS without its filter",
            &[],
            format!("{VCCS_NC} rmsmode=true"),
        ),
        (
            "a filter without points",
            &with_empty,
            format!("{VCCS_NC} bp1=bp1_micro bp2=bp2_micro filter=zz fsample=10000"),
        ),
        (
            "fsample below the fundamental",
            &CURVES_W,
            format!("{VCCS_NC} bp1=bp1_micro bp2=bp2_micro filter=z_micro fsample=50"),
        ),
    ];
    // The open-circuit voltage and the fault current of every bus.
    let fault_study = |dss: &Dss| -> Vec<u64> {
        let buses = &dss.circuit().expect("circuit").buses;
        buses
            .iter()
            .flat_map(|b| b.vbus.iter().chain(&b.bus_current))
            .flat_map(|c| [c.re.to_bits(), c.im.to_bits()])
            .collect()
    };
    // The VCCS's own current: what `CktElement::get_currents` answers, bit for
    // bit, and its `Export Currents` row, each nonzero.
    let own_current = |dss: &mut Dss, case: &str| -> (Vec<u64>, String) {
        let cur = element_currents(dss, "VCCS.e1");
        assert!(
            cur.iter().any(|c| c.norm() > 0.0),
            "{case}: the VCCS carries no current: {cur:?}"
        );
        run(dss, &["Export Currents".to_string()]);
        let text = std::fs::read_to_string(dir.join("dp_EXP_CURRENTS.csv")).expect("export");
        let row = text
            .lines()
            .find(|l| {
                let l = l.trim_start().trim_start_matches('"').to_ascii_uppercase();
                l.strip_prefix("VCCS.E1")
                    .is_some_and(|r| r.starts_with('"') || r.starts_with(','))
            })
            .unwrap_or_else(|| panic!("{case}: no VCCS row\n{text}"))
            .to_string();
        let values: Vec<f64> = row
            .split(',')
            .skip(1)
            .map(|v| v.trim().parse().expect("a number"))
            .collect();
        assert!(values.iter().any(|&v| v != 0.0), "{case}: {row}");
        let bits = cur
            .iter()
            .flat_map(|c| [c.re.to_bits(), c.im.to_bits()])
            .collect();
        (bits, row)
    };
    let mut reference = None;
    let mut current_reference = None;
    for (form, pre, line) in &forms {
        let case = format!("{form} [faultstudy]");
        let study = ["Set mode=faultstudy", "Solve", "Export Faultstudy"];
        let mut dss = deck(
            &dir,
            &[lines(&[&H, pre]), vec![line.clone()], lines(&[&PF, &study])].concat(),
        );
        assert!(dss.circuit().expect("circuit").is_solved, "{case}: solved");
        assert_eq!(format!("{:.6}", wbus1(&dss)), "0.114697", "{case}");
        let export = std::fs::read_to_string(dir.join("dp_EXP_FAULTS.csv")).expect("export");
        assert_eq!(
            export.lines().find(|l| l.starts_with("WBUS")),
            Some("WBUS        ,   19101.21,    2943.43,   16532.01"),
            "{case}: {export}"
        );
        let got = fault_study(&dss);
        assert_eq!(got, *reference.get_or_insert_with(|| got.clone()), "{case}");
        let own = own_current(&mut dss, &case);
        assert_eq!(
            own,
            *current_reference.get_or_insert_with(|| own.clone()),
            "{case}: the VCCS current against the one with its curves"
        );
    }
    let mut reference: Option<Dss> = None;
    let mut current_reference = None;
    for (form, pre, line) in &forms {
        let case = format!("{form} [mf]");
        let all = [
            lines(&[&H, pre, &[FLT]]),
            vec![line.clone()],
            lines(&[&PF, &["Set mode=mf number=2", "Solve"]]),
        ]
        .concat();
        let mut dss = deck(&dir, &all);
        assert!(dss.circuit().expect("circuit").is_solved, "{case}: solved");
        assert_eq!(format!("{:.6}", wbus1(&dss)), "27.853787", "{case}");
        let own = own_current(&mut dss, &case);
        assert_eq!(
            own,
            *current_reference.get_or_insert_with(|| own.clone()),
            "{case}: the VCCS current against the one with its curves"
        );
        match &reference {
            None => reference = Some(dss),
            Some(r) => assert_same_voltages(&case, &dss, r, "with the curves"),
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// The boundary of the VCCS rule: the RMS model uses only its filter, so an RMS
/// VCCS with a filter and no `bp1`, `bp2` runs.
#[test]
fn an_rms_vccs_needs_only_its_filter() {
    let dir = scratch("vccs_rms");
    let dss = deck(
        &dir,
        &lines(&[
            &H,
            &CURVE_R,
            &[VCCS_R],
            &PF,
            &[dyn_entry(5).as_str(), "Solve"],
        ]),
    );
    assert!(dss.circuit().expect("circuit").is_solved, "solved");
    assert_finite_voltages("RMS VCCS", &dss);
    // Both oracles fill the filter input history with the voltage, where the
    // engine starts the filter at rest
    // (`a_vccs_holds_its_operating_point_at_any_sample_rate`).
    assert_eq!(format!("{:.6}", wbus1(&dss)), "395.387813");
    std::fs::remove_dir_all(&dir).ok();
}

/// A VCCS that enters dynamics without its curves is seeded only with what a
/// direct solve reads, so curves given after the entry leave it without the
/// filter history the integration reads and the dynamics solve refuses it,
/// naming the missing model at the entry as the cause. A fault study reads no
/// filter history, so the same VCCS given its curves after `Set
/// mode=faultstudy` gives the `Export Faultstudy` row of the VCCS with its
/// curves (`a_vccs_without_its_curves_takes_part_in_a_fault_study`).
#[test]
fn a_vccs_given_its_curves_after_the_entry_cannot_integrate() {
    let dir = scratch("vccs_late_curves");
    let mut dss = deck(
        &dir,
        &lines(&[
            &H,
            &CURVES_W,
            &[VCCS_NC],
            &PF,
            &[
                dyn_entry(5).as_str(),
                "Edit VCCS.e1 bp1=bp1_micro bp2=bp2_micro filter=z_micro fsample=10000",
            ],
        ]),
    );
    assert_refused(
        &mut dss,
        "curves after the entry",
        "Solve",
        "VCCS.e1 had no dynamics model at Set mode=dynamic and has no initial state. Run Set \
         mode=snap, Solve and Set mode=dynamic to start a new run that includes it.",
        &[],
    );

    let dss = deck(
        &dir,
        &lines(&[
            &H,
            &CURVES_W,
            &[VCCS_NC],
            &PF,
            &[
                "Set mode=faultstudy",
                "Edit VCCS.e1 bp1=bp1_micro bp2=bp2_micro filter=z_micro fsample=10000",
                "Solve",
                "Export Faultstudy",
            ],
        ]),
    );
    assert!(
        dss.circuit().expect("circuit").is_solved,
        "fault study solved"
    );
    let export = std::fs::read_to_string(dir.join("dp_EXP_FAULTS.csv")).expect("export");
    assert_eq!(
        export.lines().find(|l| l.starts_with("WBUS")),
        Some("WBUS        ,   19101.21,    2943.43,   16532.01"),
        "{export}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// A VCCS seeded at one entry that reaches a later entry without the curves of
/// its model is seeded at the later entry as a VCCS that never had them: each
/// entry writes the mark and the seed of every element anew, so nothing of the
/// earlier run carries over. Run as an RMS VCCS, then switched to the waveform
/// model, which lacks `bp1` and `bp2`, it reads at the later entry the
/// variables and the current of that waveform VCCS entered fresh, bit for
/// bit, and the dynamics solve refuses it naming both curves.
#[test]
fn a_vccs_that_lost_its_model_since_the_last_entry_starts_as_one_without_it() {
    let dir = scratch("vccs_two_entries");
    let mut dss = deck(
        &dir,
        &lines(&[
            &H,
            &CURVE_R,
            &[VCCS_R],
            &PF,
            &[dyn_entry(3).as_str(), "Solve", "Set mode=snap"],
        ]),
    );
    let ran = dss.element_variables("VCCS.e1").expect("VCCS");
    assert!(
        ran.iter().any(|v| *v != 0.0),
        "the first run left a state: {ran:?}"
    );
    run(
        &mut dss,
        &lines(&[&["Edit VCCS.e1 rmsmode=no", "Solve", dyn_entry(3).as_str()]]),
    );
    let mut fresh = deck(
        &dir,
        &lines(&[
            &H,
            &CURVE_R,
            &[VCCS_R, "Edit VCCS.e1 rmsmode=no"],
            &PF,
            &[dyn_entry(3).as_str()],
        ]),
    );
    assert_same_voltages("the later entry", &dss, &fresh, "entered fresh");
    assert_same_variables("the later entry", &mut dss, &mut fresh, "VCCS.e1");
    assert_ne!(
        dss.element_variables("VCCS.e1").expect("VCCS"),
        ran,
        "the later entry seeds anew"
    );
    assert_eq!(
        element_currents(&mut dss, "VCCS.e1"),
        element_currents(&mut fresh, "VCCS.e1"),
        "current"
    );
    assert_refused(
        &mut dss,
        "a later entry without bp1 and bp2",
        "Solve",
        "VCCS.e1 has no dynamics model: set bp1 and bp2 before entering dynamics.",
        &[],
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// A VCCS starts every run with the filter history a first run finds, in every
/// filter and window length, so a second run ends where the run from the start
/// ends, every node voltage and variable bit for bit, also where the filter is
/// longer than its window of a cycle.
#[test]
fn a_vccs_starts_every_run_with_the_filter_history_of_a_first_run() {
    let dir = scratch("vccs_history");
    let wave = format!("{VCCS_NC} bp1=bp1_micro bp2=bp2_micro filter=z_micro fsample=600");
    let rms = format!(
        "{VCCS_NC} filter=z_pll fsample=50 rmsmode=true imaxpu=1.15 vrmstau=0.01 irmstau=0.05"
    );
    let forms: [(&str, &[&str], &str, &str); 2] = [
        ("waveform", &CURVES_W, &wave, "0.005"),
        ("RMS", &CURVE_R, &rms, "0.02"),
    ];
    for (case, pre, line, h) in forms {
        let entry = format!("Set mode=dynamic stepsize={h} number={STEPS}");
        let head = lines(&[&H, pre, &[line], &PF, &[entry.as_str(), "Solve"]]);
        let mut fresh = deck(&dir, &head);
        assert!(
            fresh.circuit().expect("circuit").solution.converged_flag,
            "{case} from the start"
        );
        let rerun = lines(&[&["Set mode=snap", "Solve", entry.as_str(), "Solve"]]);
        let mut second = deck(&dir, &[head, rerun].concat());
        assert_same_run(case, &mut second, &mut fresh, "VCCS.e1", None);
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// The `xarray` and the `yarray` of an XYcurve line.
fn curve_arrays(line: &str) -> (Vec<f64>, Vec<f64>) {
    let array = |key: &str| -> Vec<f64> {
        let start = line.find(key).expect(key) + key.len();
        let end = start + line[start..].find(']').expect("a closing bracket");
        line[start..end]
            .split_whitespace()
            .map(|v| v.parse().expect("a number"))
            .collect()
    };
    (array("xarray=["), array("yarray=["))
}

/// The first sample at which the unit step response of the VCCS filter with
/// numerator `num` (its `yarray`) and denominator `den` (its `xarray`) leaves
/// [0, 1], the step arriving at sample 1.
fn first_step_response_exit(num: &[f64], den: &[f64]) -> usize {
    // `g[i]` is the output at sample `i + 1`. The input is one from sample 1 on,
    // and the output is zero before it.
    let mut g: Vec<f64> = Vec::new();
    for n in 1..=10_000_usize {
        let mut y: f64 = num.iter().take(n).sum();
        for (j, d) in den.iter().enumerate().skip(1).take(n - 1) {
            y -= d * g[n - 1 - j];
        }
        if !(0.0..=1.0).contains(&y) {
            return n;
        }
        g.push(y);
    }
    panic!("the step response stays within [0, 1]")
}

/// An RMS VCCS whose filter is longer than its window of a cycle starts with the
/// filter at rest at its power-flow current, so through a run with no
/// disturbance its RMS current moves from that current toward the one its power
/// target asks and passes neither before the filter's step response first
/// leaves its range. Pinned at sample rates whose window holds fewer samples
/// than the filter has points.
#[test]
fn a_vccs_holds_its_operating_point_at_any_sample_rate() {
    let dir = scratch("vccs_operating_point");
    // From rest at the power-flow current `Ipf` the filter input steps to the
    // current the power target asks, `Ipower`, so the output is
    // `Ipf + (Ipower - Ipf) g_n` with `g_n` the unit step response, which passes
    // zero frequency at unit gain. While `g_n` stays within [0, 1] the output,
    // and the RMS current that lags it, stay between `Ipf` and `Ipower`, on the
    // side of `Ipf` the target asks. Each step below takes one sample.
    let (den, num) = curve_arrays(CURVE_R[0]);
    let gain: f64 = num.iter().sum::<f64>() / den.iter().sum::<f64>();
    assert!(
        (gain - 1.0).abs() < 1e-9,
        "the gain at zero frequency is {gain}"
    );
    let steps = 10;
    let exit = first_step_response_exit(&num, &den);
    assert!(
        exit > steps,
        "the step response leaves [0, 1] at sample {exit}"
    );
    // The current base of `VCCS_NC`: `Ppct` of `Prated / Vrated`, one phase.
    let (prated, vrated, ppct) = (190.0, 398.0, 89.5);
    let i_base = 0.01 * ppct * prated / vrated;
    for (fsample, h) in [(50, "0.02"), (100, "0.01")] {
        let case = format!("fsample={fsample}");
        let line = format!(
            "{VCCS_NC} filter=z_pll fsample={fsample} rmsmode=true imaxpu=1.15 vrmstau=0.01 \
             irmstau=0.05"
        );
        let mut dss = deck(&dir, &lines(&[&H, &CURVE_R, &[line.as_str()], &PF]));
        let i_pf = element_currents(&mut dss, "VCCS.e1")[0].norm() / i_base;
        // The power target asks the rated power at the present voltage.
        let i_power = vrated / node_magnitude(&dss, "WBUS.1");
        let (lo, hi) = (i_pf.min(i_power), i_pf.max(i_power));
        // The rounding of a recursion at rest.
        let eps = 1e-12 * hi;
        run(
            &mut dss,
            &[format!("Set mode=dynamic stepsize={h} number=1")],
        );
        for step in 1..=steps {
            run(&mut dss, &["Solve".to_string()]);
            let irms = variables(&mut dss, "VCCS.e1")
                .into_iter()
                .find(|(n, _)| n == "Irms")
                .expect("Irms")
                .1;
            assert!(
                lo - eps <= irms && irms <= hi + eps,
                "{case}: step {step}: Irms {irms} outside [{lo}, {hi}]"
            );
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// A one-phase waveform VCCS reads the same RMS current on each phase of a
/// balanced bus and under a rotated source angle, the trajectory its filter and
/// window give from a history that continues the waveform the integration
/// samples. Pinned with a filter longer than its window.
#[test]
fn a_waveform_vccs_reads_one_current_on_every_phase_and_angle() {
    let dir = scratch("vccs_every_phase");
    // At 50 Hz and 10 kHz a cycle, the window of the RMS current, holds `w`
    // samples. The filter `z_n = (1 - a)/2 (w_n + w_{n-2w}) + a z_{n-2w}` passes
    // the fundamental at unit gain and both curves are the identity, so for the
    // first `2w` samples its output is the seeded sinusoid of amplitude
    // `r1 = (1 - a) s1 + a s3`, with `s1` and `s3` the seeded voltage and current
    // in per unit, at the phase of the terminal voltage.
    let (w, a) = (200_usize, 0.5_f64);
    let taps = 2 * w + 1;
    let (mut den, mut num) = (vec![0.0; taps], vec![0.0; taps]);
    (den[0], den[taps - 1]) = (1.0, -a);
    (num[0], num[taps - 1]) = ((1.0 - a) / 2.0, (1.0 - a) / 2.0);
    let text = |v: &[f64]| v.iter().map(f64::to_string).collect::<Vec<_>>().join(" ");
    let filter = format!(
        "New XYcurve.comb npts={taps} xarray=[{}] yarray=[{}]",
        text(&den),
        text(&num)
    );
    let curves = [
        "New XYcurve.bp1_lin npts=2 xarray=[-2 2] yarray=[-2 2]",
        "New XYcurve.bp2_lin npts=2 xarray=[-2 2] yarray=[-2 2]",
        filter.as_str(),
    ];
    let (prated, vrated, ppct) = (190.0, 398.0, 89.5);
    let i_base = 0.01 * ppct * prated / vrated;
    let wd = std::f64::consts::TAU * 50.0 / 10_000.0;
    // Within the first cycle the window holds `w - k` seeded samples and `k` new
    // ones, so `Irms² = s3² + (r1² - s3²)(k + Σ cos 2θ)/w`, the sum over the new
    // samples at most `1 / sin(wd)` in magnitude. From sample `w` on it holds a
    // whole cycle of the new sinusoid, so `Irms = r1`. The live input `w_n` reads
    // the terminal voltage the dynamics network solves, within twice the
    // power-flow tolerance of the one the seed read, which moves `r1` by at most
    // `delta`.
    let mut first: Option<(String, Vec<f64>)> = None;
    let mut delta_max: f64 = 0.0;
    for (phase, angle) in [(1, 0), (2, 0), (3, 0), (1, 90)] {
        let case = format!("phase {phase}, source angle {angle}");
        let source = format!("New Circuit.dp basekv=0.69 phases=3 bus1=srcbus angle={angle}");
        let vccs = format!(
            "New VCCS.e1 phases=1 bus1=wbus.{phase} Prated={prated} Vrated={vrated} \
             Ppct={ppct} bp1=bp1_lin bp2=bp2_lin filter=comb fsample=10000"
        );
        let head = [
            "Clear",
            "Set DefaultBaseFrequency=50",
            source.as_str(),
            H[2],
            H[3],
        ];
        let mut dss = deck(&dir, &lines(&[&head, &curves, &[vccs.as_str()], &PF]));
        let s1 = node_magnitude(&dss, &format!("WBUS.{phase}")) / vrated;
        let s3 = element_currents(&mut dss, "VCCS.e1")[0].norm() / i_base;
        let r1 = (1.0 - a) * s1 + a * s3;
        let tol = dss
            .circuit()
            .expect("circuit")
            .solution
            .convergence_tolerance;
        let delta = (1.0 - a) * tol * s1;
        delta_max = delta_max.max(delta);
        let band = (r1 * r1 - s3 * s3).abs() / (w as f64 * wd.sin()) + 2.0 * r1 * delta;
        run(
            &mut dss,
            &["Set mode=dynamic stepsize=0.0001 number=1".to_string()],
        );
        let mut readings = Vec::with_capacity(2 * w);
        for k in 1..=2 * w {
            run(&mut dss, &["Solve".to_string()]);
            let irms = variables(&mut dss, "VCCS.e1")
                .into_iter()
                .find(|(n, _)| n == "Irms")
                .expect("Irms")
                .1;
            if k < w {
                let mean = s3 * s3 + (r1 * r1 - s3 * s3) * k as f64 / w as f64;
                assert!(
                    (irms * irms - mean).abs() <= band,
                    "{case}: sample {k}: Irms² {} against {mean} within {band}",
                    irms * irms
                );
            } else {
                assert!(
                    (irms - r1).abs() <= delta,
                    "{case}: sample {k}: Irms {irms} against {r1}"
                );
            }
            readings.push(irms);
        }
        match &first {
            None => first = Some((case, readings)),
            Some((first_case, reference)) => {
                for k in w - 1..2 * w {
                    assert!(
                        (readings[k] - reference[k]).abs() <= 2.0 * delta_max,
                        "{case}: sample {}: Irms {} against {} on {first_case}",
                        k + 1,
                        readings[k],
                        reference[k]
                    );
                }
            }
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// The dynamics solve refuses a VCCS of either model whose step, set with the
/// entry or changed during the run, is not a whole number of its sample steps,
/// naming the step, `fsample`, the model and the remedy, as the VCCS
/// documentation requires. A step a rounding error away from a whole number
/// passes, within the margin the doc of `Vccs::step_refusal` states.
#[test]
fn a_vccs_steps_a_whole_number_of_samples() {
    let dir = scratch("vccs_sample_step");
    let wave =
        |fs: &str| format!("{VCCS_NC} bp1=bp1_micro bp2=bp2_micro filter=z_micro fsample={fs}");
    let rms = |fs: &str| {
        format!(
            "{VCCS_NC} filter=z_pll fsample={fs} rmsmode=true imaxpu=1.15 vrmstau=0.01 \
             irmstau=0.05"
        )
    };
    let refusal_of = |model: &str, h: &str, fs: &str, samples: &str| {
        format!(
            "VCCS.e1 cannot take a step of {h} s: its fsample of {fs} Hz gives {samples} \
             samples per step, and the {model} model needs a whole number of them. Set \
             stepsize to an integer multiple of 1/fsample."
        )
    };
    let refusal = |h: &str, fs: &str, samples: &str| refusal_of("waveform", h, fs, samples);
    let entry = |h: &str| format!("Set mode=dynamic stepsize={h} number={STEPS}");
    for (fs, h, samples) in [
        ("500", "0.001", "0.5"),
        ("1000", "0.0015", "1.5"),
        ("600", "0.002", "1.2"),
        ("1000.000000000001", "0.001", "1.000000000000001"),
    ] {
        let case = format!("fsample={fs} stepsize={h}");
        let mut dss = deck(
            &dir,
            &lines(&[
                &H,
                &CURVES_W,
                &[wave(fs).as_str()],
                &PF,
                &[entry(h).as_str()],
            ]),
        );
        assert_refused(&mut dss, &case, "Solve", &refusal(h, fs, samples), &[]);
    }
    for (fs, h) in [
        ("10000", "0.0001"),
        ("10000", "0.001"),
        ("10000", "0.002"),
        ("10000", "0.0003"),
        ("1000.0000000000005", "0.001"),
        ("600", "0.005"),
    ] {
        let case = format!("fsample={fs} stepsize={h}");
        let dss = deck(
            &dir,
            &lines(&[
                &H,
                &CURVES_W,
                &[wave(fs).as_str()],
                &PF,
                &[entry(h).as_str(), "Solve"],
            ]),
        );
        let sol = &dss.circuit().expect("circuit").solution;
        let h: f64 = h.parse().expect("a step");
        let stepped = (0..STEPS).fold(0.0, |t, _| t + h);
        assert_eq!(sol.t, stepped, "{case}: the clock");
        assert_finite_voltages(&case, &dss);
    }
    let mut dss = deck(
        &dir,
        &lines(&[
            &H,
            &CURVES_W,
            &[VCCS_W],
            &PF,
            &[entry("0.001").as_str(), "Solve", "Set stepsize=0.00015"],
        ]),
    );
    assert_refused(
        &mut dss,
        "a step changed during the run",
        "Solve",
        &refusal("0.00015", "10000", "1.4999999999999998"),
        &[],
    );
    for (h, samples) in [("0.00005", "0.5"), ("0.00015", "1.4999999999999998")] {
        let case = format!("RMS fsample=10000 stepsize={h}");
        let mut dss = deck(
            &dir,
            &lines(&[
                &H,
                &CURVE_R,
                &[rms("10000").as_str()],
                &PF,
                &[entry(h).as_str()],
            ]),
        );
        assert_refused(
            &mut dss,
            &case,
            "Solve",
            &refusal_of("RMS", h, "10000", samples),
            &[],
        );
    }
    for (fs, h) in [("10000", "0.0001"), ("10000", "0.001"), ("50", "0.02")] {
        let case = format!("RMS fsample={fs} stepsize={h}");
        let dss = deck(
            &dir,
            &lines(&[
                &H,
                &CURVE_R,
                &[rms(fs).as_str()],
                &PF,
                &[entry(h).as_str(), "Solve"],
            ]),
        );
        assert!(dss.circuit().expect("circuit").is_solved, "{case}: solved");
        assert_finite_voltages(&case, &dss);
    }
    std::fs::remove_dir_all(&dir).ok();
}

//! MakePosSequence refusals, the reduction order and the repeat guard: a
//! circuit the reduction cannot take is left exactly as it was, every control
//! and meter is reduced after every element, and a circuit already
//! positive-sequence is left as it is.

use super::{assert_refused, element_nphases};
use crate::diag::DssDiagnostic;
use crate::elements::pd::{ABORTING_SOLUTION, line, reactor};
use crate::elements::pos_seq::{PosSeqAction, PosSeqPlan, balance};
use crate::exec::make_pos_seq::refuse::PosSeqRefusal;
use crate::exec::*;
use num_complex::Complex64;

/// `<Class>.<name>` of the circuit element `r`.
fn full_name(dss: &Dss, r: ElemId) -> String {
    let class = &dss.classes[r.class_ord()];
    format!(
        "{}.{}",
        class.props.class_name(),
        class.arena.obj(r.index()).data().name()
    )
}

/// The circuit element named `full` (`<Class>.<name>`, any case).
fn elem_id(dss: &Dss, full: &str) -> ElemId {
    dss.circuit()
        .unwrap()
        .ckt_elements
        .iter()
        .copied()
        .find(|&r| full_name(dss, r).eq_ignore_ascii_case(full))
        .unwrap_or_else(|| panic!("{full} is not a circuit element"))
}

/// Replay the one write `SetText(idx, text)` on `full` through the applier.
fn set_text(dss: &mut Dss, full: &str, idx: usize, text: &str) -> Result<(), PosSeqRefusal> {
    let r = elem_id(dss, full);
    let plan = PosSeqPlan {
        actions: vec![PosSeqAction::SetText(idx, text.to_string())],
        run_base: false,
    };
    dss.apply_pos_seq_plan(r, &plan)
}

/// Every command of `lines` in order, none leaving an error.
fn run(dss: &mut Dss, lines: &[&str]) {
    for line in lines {
        dss.command(line);
        assert!(dss.errors().is_empty(), "`{line}`: {:?}", dss.errors());
    }
}

/// [`balance::stiff`] named `name` with a line from `b` to `c` and a
/// constant-impedance load on `c`.
fn feeder(name: &str) -> Dss {
    let mut dss = balance::stiff(name);
    run(
        &mut dss,
        &[
            "new line.l1 bus1=b bus2=c phases=3 r1=0.3 x1=0.6 c1=10 length=1 units=km",
            "new load.ld bus1=c phases=3 kv=12.47 kw=1000 pf=0.9 model=2",
        ],
    );
    dss
}

/// [`feeder`] with a capacitor on `c` when `capacitor` and the fault `fault`,
/// solved twice: the circuit and the source power of the second solve.
fn fault_deck(name: &str, capacitor: bool, fault: &str) -> (Dss, Complex64) {
    let mut dss = feeder(name);
    if capacitor {
        run(
            &mut dss,
            &["new capacitor.cb bus1=c phases=3 kvar=300 kv=12.47"],
        );
    }
    run(&mut dss, &[fault]);
    balance::solve_clean(&mut dss, "before makeposseq");
    solve_again(dss)
}

/// Solve again from the converged state and return the source power of that
/// solve.
fn solve_again(mut dss: Dss) -> (Dss, Complex64) {
    dss.command("solve");
    assert!(dss.errors().is_empty(), "solving again: {:?}", dss.errors());
    let before = balance::source_power(&mut dss);
    (dss, before)
}

const UNBALANCED_FAULT: &str =
    "A one- or two-phase fault has no positive-sequence form. Nothing was changed.";

/// A one-phase fault has no positive-sequence form (IEEE Std 551-2006 section
/// 3.6.12, p. 61: an unbalanced fault invalidates the single-phase
/// equivalent), so `makeposseq` refuses it and leaves the circuit unchanged.
#[test]
fn a_one_phase_fault_stops_makeposseq_and_changes_nothing() {
    let (mut dss, before) = fault_deck("rf1", true, "new fault.f1 phases=1 bus1=c.2 r=100");
    assert_refused(
        &mut dss,
        "rf1",
        &format!("MakePosSequence: Fault.f1 has 1 phase. {UNBALANCED_FAULT}"),
        Some(before),
    );
}

/// A two-phase fault has no positive-sequence form either (IEEE Std 551-2006
/// section 3.6.12, p. 61).
#[test]
fn a_two_phase_fault_stops_makeposseq_and_changes_nothing() {
    let (mut dss, before) = fault_deck("rf2", true, "new fault.f2 phases=2 bus1=c.1.2 r=50");
    assert_refused(
        &mut dss,
        "rf2",
        &format!("MakePosSequence: Fault.f2 has 2 phases. {UNBALANCED_FAULT}"),
        Some(before),
    );
}

/// A disabled element is analysed like an enabled one: enabling it after the
/// reduction would put an element the reduction refuses into the reduced
/// circuit.
#[test]
fn a_disabled_two_phase_fault_stops_makeposseq() {
    let (mut dss, before) = fault_deck(
        "rfd",
        true,
        "new fault.f2 phases=2 bus1=c.1.2 r=50 enabled=no",
    );
    assert_refused(
        &mut dss,
        "rfd",
        &format!("MakePosSequence: Fault.f2 has 2 phases. {UNBALANCED_FAULT}"),
        Some(before),
    );
}

/// The symmetric part of `[0.1|0.3 0.1|0 0 0.1]` has the eigenvalues 0.4,
/// 0.1 and -0.2, so the fault delivers power under some voltage and has no
/// passive positive-sequence form.
#[test]
fn a_non_passive_gmatrix_stops_makeposseq() {
    let (mut dss, before) = fault_deck(
        "rfg",
        false,
        "new fault.fg phases=3 bus1=c gmatrix=[0.1|0.3 0.1|0 0 0.1]",
    );
    assert_refused(
        &mut dss,
        "rfg",
        "MakePosSequence: the gmatrix of Fault.fg is not passive. Nothing was changed.",
        Some(before),
    );
}

/// `r=-10` is the conductance -0.1 S on each phase: a source, not a fault.
#[test]
fn a_negative_fault_resistance_stops_makeposseq() {
    let (mut dss, before) = fault_deck("rfn", false, "new fault.fn phases=3 bus1=c r=-10");
    assert_refused(
        &mut dss,
        "rfn",
        "MakePosSequence: the r of Fault.fn is not passive. Nothing was changed.",
        Some(before),
    );
}

/// `(0 0 /)` writes the conductance NaN between phases 1 and 2, which draws no
/// defined power under any voltage, so the fault has no passive form. No solve
/// runs, since a NaN conductance has no solved state.
#[test]
fn a_gmatrix_entry_that_is_not_a_number_stops_makeposseq() {
    let mut dss = feeder("rfq");
    run(
        &mut dss,
        &["new fault.fq phases=3 bus1=c gmatrix=[1|(0 0 /) 1|0 0 1]"],
    );
    assert_refused(
        &mut dss,
        "rfq",
        "MakePosSequence: the gmatrix of Fault.fq is not passive. Nothing was changed.",
        None,
    );
}

/// `r=(0 0 /)` is a resistance that is not a number, so the conductance the
/// fault stamps is no number either and draws no defined power, which no
/// passive form holds. No solve runs, since such a fault has no solved state.
#[test]
fn a_fault_resistance_that_is_not_a_number_stops_makeposseq() {
    let mut dss = feeder("rfr");
    run(&mut dss, &["new fault.fr phases=3 bus1=c r=(0 0 /)"]);
    assert_refused(
        &mut dss,
        "rfr",
        "MakePosSequence: the r of Fault.fr is not passive. Nothing was changed.",
        None,
    );
}

/// Within one element the phase count is read before passivity: the
/// two-phase fault with the non-passive `[0.1|0.3 0.1]` (eigenvalues 0.4 and
/// -0.2) is refused for its phase count, since passivity is a property of a
/// form that exists.
#[test]
fn a_two_phase_fault_with_a_non_passive_gmatrix_stops_on_its_phase_count() {
    let (mut dss, before) = fault_deck(
        "rfp",
        false,
        "new fault.fp phases=2 bus1=c.1.2 gmatrix=[0.1|0.3 0.1]",
    );
    assert_refused(
        &mut dss,
        "rfp",
        &format!("MakePosSequence: Fault.fp has 2 phases. {UNBALANCED_FAULT}"),
        Some(before),
    );
}

/// A Reactor whose matrices do not match its phase count is refused with the
/// reason its own solve refuses it. No solve runs, since the solve refuses
/// the reactor too.
#[test]
fn a_matrix_of_the_wrong_order_stops_makeposseq() {
    let mut dss = balance::stiff("rw");
    run(
        &mut dss,
        &[
            "new reactor.rw phases=3 bus1=b rmatrix=[1|0 1|0 0 1] xmatrix=[10|0 10|0 0 10]",
            "reactor.rw.phases=2",
        ],
    );
    assert_refused(
        &mut dss,
        "rw",
        "MakePosSequence: Reactor.rw has 2 phases but its rmatrix has 3 x 3 entries. \
         Specify rmatrix and xmatrix for 2 phases. Nothing was changed.",
        None,
    );
}

const SINGULAR_LINE: &str =
    "new line.ls phases=3 bus1=b bus2=c length=1 rmatrix=[0|0 0|0 0 0] xmatrix=[0|0 0|0 0 0]";

/// A Line whose impedance matrix is singular is refused with the reason its
/// own solve refuses it, the text the solve of the same deck pushes.
#[test]
fn a_line_whose_impedance_is_singular_stops_makeposseq() {
    let reason = "Matrix Inversion Error for Line \"ls\". Invalid impedance specified.";

    let mut solved = balance::stiff("ls");
    run(&mut solved, &[SINGULAR_LINE]);
    solved.command("solve");
    let solve_text = format!("{reason}{ABORTING_SOLUTION}");
    assert!(
        solved
            .errors()
            .iter()
            .any(|e| e.code == Some(183) && e.text() == solve_text),
        "the solve pushes the reason: {:?}",
        solved.errors()
    );

    let mut dss = balance::stiff("ls");
    run(&mut dss, &[SINGULAR_LINE]);
    assert_refused(
        &mut dss,
        "ls",
        &format!("MakePosSequence: {reason} Nothing was changed."),
        None,
    );
}

/// The stiff circuit `name` with the element of `lines`, which its own solve
/// refuses: the solve of one copy pushes `reason` with its
/// `Aborting solution.`, and MakePosSequence stops on another copy with that
/// reason. No solve runs before the command, since the solve refuses the
/// element.
fn assert_solve_refusal_stops_makeposseq(name: &str, lines: &[&str], reason: &str) {
    let deck = || {
        let mut dss = balance::stiff(name);
        for line in lines {
            dss.command(line);
        }
        dss
    };
    let mut solved = deck();
    solved.command("solve");
    let solve_text = format!("{reason}{ABORTING_SOLUTION}");
    assert!(
        solved
            .errors()
            .iter()
            .any(|e| e.text().contains(&solve_text)),
        "{name}: the solve pushes the reason: {:?}",
        solved.errors()
    );
    let mut dss = deck();
    assert_refused(
        &mut dss,
        name,
        &format!("MakePosSequence: {reason} Nothing was changed."),
        None,
    );
}

/// A Capacitor whose `cmatrix` does not match its phase count is refused with
/// the reason its own solve refuses it.
#[test]
fn a_capacitor_matrix_of_the_wrong_order_stops_makeposseq() {
    assert_solve_refusal_stops_makeposseq(
        "cp2",
        &[
            "new capacitor.cp2 bus1=b phases=3 cmatrix=[10 | -2 10 | -2 -2 10]",
            "edit capacitor.cp2 phases=2",
        ],
        "Capacitor.cp2 has 2 phases but its cmatrix has 3 x 3 entries. \
         Specify cmatrix for 2 phases.",
    );
}

/// A Fault whose `gmatrix` does not match its phase count is refused with the
/// reason its own solve refuses it, before its conductance is read for
/// passivity.
#[test]
fn a_fault_gmatrix_of_the_wrong_order_stops_makeposseq() {
    assert_solve_refusal_stops_makeposseq(
        "fshort",
        &[
            "new fault.fshort bus1=b phases=3 gmatrix=[0.1 | -0.02 0.1 | -0.02 -0.02 0.1]",
            "edit fault.fshort phases=4",
        ],
        "Fault.fshort has 4 phases but its gmatrix has 3 x 3 entries. \
         Specify gmatrix for 4 phases.",
    );
}

/// A Capacitor given by `cmatrix` and connected in delta is refused with the
/// reason its own solve refuses it.
#[test]
fn a_delta_cmatrix_capacitor_stops_makeposseq() {
    assert_solve_refusal_stops_makeposseq(
        "cd3",
        &["new capacitor.cd3 bus1=b phases=3 conn=delta cmatrix=[10 | -2 10 | -2 -2 10]"],
        "Capacitor.cd3: a cmatrix bank of 3 phases cannot be connected in delta. \
         Specify it with conn=wye.",
    );
}

/// A two-phase Fault whose `gmatrix` has 3 x 3 entries is refused with the
/// reason its own solve refuses it, which is read before its phase count.
#[test]
fn a_two_phase_fault_with_a_gmatrix_of_the_wrong_order_stops_on_its_solve_refusal() {
    assert_solve_refusal_stops_makeposseq(
        "flarge",
        &[
            "new fault.flarge bus1=b phases=3 gmatrix=[0.1 | -0.02 0.1 | -0.03 -0.04 0.1]",
            "edit fault.flarge phases=2 bus1=b.1.2",
        ],
        "Fault.flarge has 2 phases but its gmatrix has 3 x 3 entries. \
         Specify gmatrix for 2 phases.",
    );
}

/// The element checks run element by element in creation order and the first
/// refusal met is reported, whatever its case: a one-phase Fault created
/// before a Reactor whose matrices do not match its phase count is refused for
/// its phase count, and the same Reactor created first is refused with the
/// reason its own solve gives. No solve runs, since the solve refuses the
/// reactor.
#[test]
fn the_first_refused_element_in_creation_order_is_named() {
    let fault = "new fault.f1 phases=1 bus1=b.2 r=100";
    let reactor = [
        "new reactor.rw phases=3 bus1=b rmatrix=[1|0 1|0 0 1] xmatrix=[10|0 10|0 0 10]",
        "reactor.rw.phases=2",
    ];

    let mut dss = balance::stiff("ffirst");
    run(&mut dss, &[fault, reactor[0], reactor[1]]);
    assert_refused(
        &mut dss,
        "ffirst",
        &format!("MakePosSequence: Fault.f1 has 1 phase. {UNBALANCED_FAULT}"),
        None,
    );

    let mut dss = balance::stiff("rfirst");
    run(&mut dss, &[reactor[0], reactor[1], fault]);
    assert_refused(
        &mut dss,
        "rfirst",
        "MakePosSequence: Reactor.rw has 2 phases but its rmatrix has 3 x 3 entries. \
         Specify rmatrix and xmatrix for 2 phases. Nothing was changed.",
        None,
    );
}

const WIRE: &str = "new wiredata.w runits=m gmrunits=m radunits=m rac=0.0003 gmrac=0.005 \
                    radius=0.01 normamps=400";

const NO_SPACING: &str = "Line.l has no LineSpacing to build its impedance from.";

/// The stiff circuit `tag` with the Line `l` on the spacing `sp` and its
/// wires, and a load on `c`.
fn spacing_deck(tag: &str) -> Dss {
    let mut dss = balance::stiff(tag);
    run(
        &mut dss,
        &[
            WIRE,
            "new linespacing.sp nconds=3 nphases=3 x=[-1 0 1] h=[10 10 10] units=m",
            "new line.l bus1=b bus2=c phases=3 spacing=sp wires=[w w w] length=1 units=km",
            "new load.ld bus1=c phases=3 kv=12.47 kw=100",
        ],
    );
    dss
}

/// `edit line.l spacing=nonexistent` on `dss`, asserting that it logs its one
/// message naming the missing LineSpacing.
fn miss_the_spacing(dss: &mut Dss, tag: &str) {
    assert_eq!(
        errors_of(dss, "edit line.l spacing=nonexistent"),
        [(
            "Line.l.Spacing: LineSpacing object \"nonexistent\" not found.".to_string(),
            None
        )],
        "{tag}: the edit names a LineSpacing that does not exist"
    );
}

/// `cmd` on `dss` and the text and code of each error it adds.
fn errors_of(dss: &mut Dss, cmd: &str) -> Vec<(String, Option<u32>)> {
    let n = dss.errors().len();
    dss.command(cmd);
    dss.errors()[n..]
        .iter()
        .map(|e| (e.text().to_string(), e.code))
        .collect()
}

/// A Line whose `spacing=` names no LineSpacing that exists after its wires
/// were given holds no spacing to build its impedance from, whether or not a
/// solve built its impedance before the edit. On the Line enabled, or
/// disabled after the edit, each command on its own copy of the deck:
/// `CalcVoltageBases` refuses it with that reason (code `None`),
/// MakePosSequence stops on it and the solve after it refuses it with that
/// reason (code 482), and so does a solve run alone.
#[test]
fn a_line_left_without_its_spacing_stops_makeposseq() {
    let cvb = (
        format!("Error Encountered in CalcVoltageBases: {NO_SPACING}"),
        None,
    );
    let solve = (
        format!("Error Encountered in Solve: {NO_SPACING}"),
        Some(482),
    );
    for (disabled, solved) in [(false, false), (true, false), (false, true), (true, true)] {
        let tag = match (disabled, solved) {
            (false, false) => "lsp",
            (true, false) => "lspd",
            (false, true) => "lsps",
            (true, true) => "lspds",
        };
        let deck = || {
            let mut dss = spacing_deck(tag);
            if solved {
                balance::solve_clean(&mut dss, tag);
            }
            miss_the_spacing(&mut dss, tag);
            if disabled {
                let added = errors_of(&mut dss, "edit line.l enabled=no");
                assert!(added.is_empty(), "{tag}: {added:?}");
            }
            dss
        };

        let mut dss = deck();
        dss.command("set voltagebases=[12.47]");
        assert_eq!(
            errors_of(&mut dss, "calcvoltagebases"),
            std::slice::from_ref(&cvb),
            "{tag}: `CalcVoltageBases` pushes the reason alone"
        );

        let mut dss = deck();
        assert_refused(
            &mut dss,
            tag,
            &format!("MakePosSequence: {NO_SPACING} Nothing was changed."),
            None,
        );
        assert_eq!(
            errors_of(&mut dss, "solve"),
            std::slice::from_ref(&solve),
            "{tag}: the solve after MakePosSequence"
        );

        let mut dss = deck();
        assert_eq!(
            errors_of(&mut dss, "solve"),
            std::slice::from_ref(&solve),
            "{tag}: the solve"
        );
    }
}

/// A Line disabled before it loses its spacing, as it stands: with no solve
/// before, the next solve builds every YPrim and refuses the Line (code
/// 482). After a solve, the disabled Line asks for no Y build, so the next
/// solve keeps the Y matrix built before and converges with no error.
/// `CalcVoltageBases` (code `None`) and MakePosSequence refuse the Line in
/// both orders, and the solve after MakePosSequence reads as a solve alone.
/// Each command runs on its own copy of the deck.
#[test]
fn a_line_disabled_before_it_loses_its_spacing() {
    for solved in [false, true] {
        let tag = if solved { "lsdbs" } else { "lsdb" };
        let deck = || {
            let mut dss = spacing_deck(tag);
            run(&mut dss, &["edit line.l enabled=no"]);
            if solved {
                balance::solve_clean(&mut dss, tag);
            }
            miss_the_spacing(&mut dss, tag);
            dss
        };
        let assert_solve = |dss: &mut Dss, what: &str| {
            let added = errors_of(dss, "solve");
            if solved {
                assert!(added.is_empty(), "{tag}: {what}: {added:?}");
                assert!(
                    dss.circuit().unwrap().is_solved,
                    "{tag}: {what}: not solved"
                );
            } else {
                assert_eq!(
                    added,
                    [(
                        format!("Error Encountered in Solve: {NO_SPACING}"),
                        Some(482)
                    )],
                    "{tag}: {what}"
                );
            }
        };

        let mut dss = deck();
        assert_solve(&mut dss, "the solve");

        let mut dss = deck();
        dss.command("set voltagebases=[12.47]");
        assert_eq!(
            errors_of(&mut dss, "calcvoltagebases"),
            [(
                format!("Error Encountered in CalcVoltageBases: {NO_SPACING}"),
                None
            )],
            "{tag}: `CalcVoltageBases`"
        );

        let mut dss = deck();
        assert_refused(
            &mut dss,
            tag,
            &format!("MakePosSequence: {NO_SPACING} Nothing was changed."),
            None,
        );
        assert_solve(&mut dss, "the solve after MakePosSequence");
    }
}

/// A Line whose geometry cannot build its impedance is refused with the
/// geometry's message, led by the Line's name, since that message names the
/// geometry alone. The solve pushes the geometry's message as it stands.
#[test]
fn a_line_whose_geometry_cannot_build_its_impedance_stops_makeposseq() {
    let reason = "Error in LineGeometry.bad: Conductors 1 and 2 occupy the same space.";
    let mut dss = balance::stiff("lgb");
    run(
        &mut dss,
        &[
            WIRE,
            "new linegeometry.bad nconds=3 nphases=3 cond=1 wire=w x=0 h=10 units=m cond=2 \
             wire=w x=0 h=10 cond=3 wire=w x=2 h=10",
            "new line.l bus1=b bus2=c phases=3 geometry=bad length=1 units=km",
            "new load.ld bus1=c phases=3 kv=12.47 kw=100",
        ],
    );
    assert_refused(
        &mut dss,
        "lgb",
        &format!("MakePosSequence: Line.l: {reason} Nothing was changed."),
        None,
    );
    dss.command("solve");
    let solve_text = format!("Error Encountered in Solve: {reason}");
    assert!(
        dss.errors().iter().any(|e| e.text() == solve_text),
        "the solve pushes the geometry's message: {:?}",
        dss.errors()
    );
}

/// A Line whose spacing cannot build its impedance is refused with the
/// spacing's message, which names the Line. The solve pushes the same
/// message.
#[test]
fn a_line_whose_spacing_cannot_build_its_impedance_stops_makeposseq() {
    let reason = "Error in Line.l (spacing=bad): Conductors 1 and 2 occupy the same space.";
    let mut dss = balance::stiff("lsb");
    run(
        &mut dss,
        &[
            WIRE,
            "new linespacing.bad nconds=3 nphases=3 x=[0 0 2] h=[10 10 10] units=m",
            "new line.l bus1=b bus2=c phases=3 spacing=bad wires=[w w w] length=1 units=km",
            "new load.ld bus1=c phases=3 kv=12.47 kw=100",
        ],
    );
    assert_refused(
        &mut dss,
        "lsb",
        &format!("MakePosSequence: {reason} Nothing was changed."),
        None,
    );
    dss.command("solve");
    let solve_text = format!("Error Encountered in Solve: {reason}");
    assert!(
        dss.errors().iter().any(|e| e.text() == solve_text),
        "the solve pushes the spacing's message: {:?}",
        dss.errors()
    );
}

/// The late-refusal deck solved once: lines, constant-power loads, a
/// capacitor, a transformer with a load behind it and a generator, then the
/// one-phase fault `flast` created last.
fn late_deck() -> Dss {
    let mut dss = balance::stiff("late");
    run(
        &mut dss,
        &[
            "new line.l1 bus1=b bus2=c phases=3 r1=0.2 x1=0.5 length=1 units=km",
            "new line.l2 bus1=c bus2=d phases=3 r1=0.2 x1=0.5 length=1 units=km",
            "new load.l3 bus1=c phases=3 kv=12.47 kw=500 pf=0.9",
            "new load.l1p bus1=d.2 phases=1 kv=7.2 kw=100 pf=0.9",
            "new capacitor.cq bus1=c phases=3 kvar=300 kv=12.47",
            "new transformer.dy phases=3 windings=2 buses=[d x] conns=[delta wye] \
             kvs=[12.47 4.16] kvas=[1000 1000] xhl=6",
            "new load.lx bus1=x phases=3 kv=4.16 kw=300 pf=0.9",
            "new generator.g bus1=d phases=3 kv=12.47 kw=200 pf=1",
            "new fault.flast phases=1 bus1=c.3 r=10",
        ],
    );
    balance::solve_clean_at(&mut dss, &[12.47, 4.16], "before makeposseq");
    dss
}

/// A refusal raised by the last element leaves every element before it
/// unchanged: the analysis reads the whole circuit before the apply edits any
/// of it. One solve at the default tolerance leaves the convergence residual
/// of the constant-power loads and the generator, which the next solves take
/// away: in both lanes the second solve moves the source power by 1.5e-6
/// relative, the third by 1.0e-9 and the fourth by 6.7e-13 (2026-10-17). So
/// the reference is read after a third solve.
#[test]
fn a_refusal_late_in_the_element_list_leaves_every_earlier_element_unchanged() {
    let mut dss = late_deck();
    dss.command("solve");
    let (mut dss, before) = solve_again(dss);
    assert_refused(
        &mut dss,
        "late",
        &format!("MakePosSequence: Fault.flast has 1 phase. {UNBALANCED_FAULT}"),
        Some(before),
    );
}

/// The order deck: a line, a load, a disabled load and a capacitor, then a
/// CapControl, a Monitor, an EnergyMeter, a Fuse and a Sensor on the line,
/// then one more load.
fn order_deck() -> Dss {
    let mut dss = balance::stiff("order");
    run(
        &mut dss,
        &[
            "new line.l bus1=b bus2=c phases=3 r1=0.2 x1=0.5 length=1 units=km",
            "new load.ld bus1=c phases=3 kv=12.47 kw=400 pf=0.95",
            "new load.off bus1=c phases=3 kv=12.47 kw=50 pf=0.95 enabled=no",
            "new capacitor.c bus1=c phases=3 kvar=300 kv=12.47",
            "new capcontrol.cc element=line.l terminal=1 capacitor=c type=current on=60 off=30",
            "new monitor.m element=line.l terminal=1",
            "new energymeter.em element=line.l terminal=1",
            "new fuse.f monitoredobj=line.l",
            "new sensor.s element=line.l terminal=1",
            "new load.late bus1=c phases=3 kv=12.47 kw=100 pf=0.95",
        ],
    );
    dss
}

/// The reduction takes every source, PD and PC element in creation order,
/// disabled ones included, then every control and meter in creation order,
/// so a control reads its target reduced. `load.late`, created after the
/// meters, comes before them, and the Fuse and the Sensor count among the
/// controls.
#[test]
fn the_apply_order_puts_controls_and_meters_last() {
    let mut dss = order_deck();
    let reduction = dss
        .analyse_pos_seq()
        .unwrap_or_else(|e| panic!("{}", e.message()));
    let names: Vec<String> = reduction
        .order
        .iter()
        .map(|&r| full_name(&dss, r).to_ascii_lowercase())
        .collect();
    assert_eq!(
        names,
        [
            "vsource.source",
            "line.l",
            "load.ld",
            "load.off",
            "capacitor.c",
            "load.late",
            "capcontrol.cc",
            "monitor.m",
            "energymeter.em",
            "fuse.f",
            "sensor.s",
        ]
    );
}

/// A disabled element belongs to the circuit as built, so the reduction
/// reduces it like an enabled one and leaves it disabled. Enabled afterwards,
/// the reduced load draws the power the unreduced one draws.
#[test]
fn a_disabled_load_is_reduced_and_stays_disabled() {
    let deck = |name: &str, enabled: &str| {
        let mut dss = balance::stiff(name);
        run(
            &mut dss,
            &[
                "new line.l bus1=b bus2=c phases=3 r1=0.2 x1=0.5 length=1 units=km",
                "new load.ld bus1=c phases=3 kv=12.47 kw=400 pf=0.95 model=2",
                &format!(
                    "new load.off bus1=c phases=3 kv=12.47 kw=50 pf=0.95 model=2 \
                     enabled={enabled}"
                ),
            ],
        );
        dss
    };
    let mut full = deck("offon", "yes");
    balance::solve_clean(&mut full, "the load enabled");
    let want = balance::source_power(&mut full);

    let mut dss = deck("off", "no");
    balance::solve_clean(&mut dss, "before makeposseq");
    run(&mut dss, &["makeposseq"]);
    assert_eq!(element_nphases(&dss, "Load.off"), Some(1), "Load.off");
    let kw = balance::query_f64(&mut dss, "load.off.kw");
    balance::assert_rel(kw, 50.0 / 3.0, 1e-12, "load.off.kw");
    let r = elem_id(&dss, "Load.off");
    let off = dss.classes[r.class_ord()].arena.try_ckt_elem(r.index());
    assert!(!off.unwrap().cd().enabled, "Load.off stays disabled");

    run(&mut dss, &["edit load.off enabled=yes"]);
    balance::solve_clean(&mut dss, "the reduced load enabled");
    let got = balance::source_power(&mut dss);
    balance::assert_kept(want, got, "the reduced load enabled");
}

/// A Monitor and a CapControl created on a first line and moved by `edit` to
/// a line and a capacitor created after them read their targets reduced, so
/// both take one phase and the reduced circuit solves.
#[test]
fn a_control_moved_to_a_later_element_reads_it_reduced() {
    let mut dss = balance::stiff("moved");
    run(
        &mut dss,
        &[
            "new line.l bus1=b bus2=c phases=3 r1=0.2 x1=0.5 length=1 units=km",
            "new capacitor.c1 bus1=c phases=3 kvar=300 kv=12.47",
            "new monitor.m element=line.l terminal=1 mode=0",
            "new capcontrol.cc element=line.l terminal=1 capacitor=c1 type=current on=60 off=30",
            "new line.l2 bus1=c bus2=d phases=3 r1=0.2 x1=0.5 length=1 units=km",
            "new load.ld bus1=d phases=3 kv=12.47 kw=400 pf=0.95",
            "new capacitor.c2 bus1=d phases=3 kvar=600 kv=12.47",
            "edit monitor.m element=line.l2 terminal=1",
            "edit capcontrol.cc element=line.l2 terminal=1 capacitor=c2",
        ],
    );
    balance::solve_clean(&mut dss, "before makeposseq");
    run(&mut dss, &["makeposseq"]);
    assert_eq!(element_nphases(&dss, "Monitor.m"), Some(1), "Monitor.m");
    assert_eq!(
        element_nphases(&dss, "CapControl.cc"),
        Some(1),
        "CapControl.cc"
    );
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "after makeposseq: {:?}",
        dss.errors()
    );
    assert!(
        dss.circuit().unwrap().is_solved,
        "after makeposseq: not solved"
    );
}

/// A stiff circuit with the three-phase shunt Reactor `r` on `b`.
fn settext_deck() -> Dss {
    let mut dss = balance::stiff("settext");
    run(
        &mut dss,
        &["new reactor.r bus1=b phases=3 kvar=100 kv=12.47"],
    );
    dss
}

/// `SetText` runs the property edit path of `edit`, side effects included,
/// and the recalc of a single edit. `bus2=c` turns the shunt Reactor into a
/// series one, which neither the saved text nor the YPrim shows (a shunt and
/// a series reactor stamp the same order-6 YPrim), so the twins are also
/// compared on `is_shunt`. `kvar=200` on a solved circuit reaches the
/// reactance through the recalc alone, so the YPrim of the next solve holds
/// it only when the recalc ran.
#[test]
fn set_text_runs_the_property_edit_path() {
    for bus2 in ["b.0", "c"] {
        let mut edited = settext_deck();
        run(&mut edited, &[&format!("edit reactor.r bus2={bus2}")]);
        let mut replayed = settext_deck();
        set_text(&mut replayed, "Reactor.r", reactor::prop::BUS2, bus2)
            .unwrap_or_else(|e| panic!("bus2={bus2}: {}", e.message()));
        assert!(replayed.errors().is_empty(), "{:?}", replayed.errors());
        assert_eq!(
            balance::saved_circuit(&mut edited, "settext"),
            balance::saved_circuit(&mut replayed, "settext"),
            "bus2={bus2}: the saved text"
        );

        balance::solve_clean(&mut edited, "edited");
        balance::solve_clean(&mut replayed, "replayed");
        assert_eq!(
            balance::saved_circuit(&mut edited, "settext"),
            balance::saved_circuit(&mut replayed, "settext"),
            "bus2={bus2}: the saved text after the solve"
        );
        let shunt = |dss: &Dss| {
            dss.pd_elements()
                .into_iter()
                .find(|e| e.name.eq_ignore_ascii_case("Reactor.r"))
                .expect("Reactor.r is a PD element")
                .is_shunt
        };
        assert_eq!(shunt(&edited), shunt(&replayed), "bus2={bus2}: is_shunt");
        if bus2 == "c" {
            assert!(!shunt(&replayed), "bus2=c makes a series reactor");
        }
        let yprim = edited.element_yprim("Reactor.r");
        assert!(yprim.is_some(), "bus2={bus2}: the solve builds the YPrim");
        assert_eq!(
            yprim,
            replayed.element_yprim("Reactor.r"),
            "bus2={bus2}: YPrim"
        );
    }

    let mut edited = settext_deck();
    balance::solve_clean(&mut edited, "edited, first solve");
    let at_100 = edited.element_yprim("Reactor.r");
    run(&mut edited, &["edit reactor.r kvar=200"]);
    let mut replayed = settext_deck();
    balance::solve_clean(&mut replayed, "replayed, first solve");
    set_text(&mut replayed, "Reactor.r", reactor::prop::KVAR, "200")
        .unwrap_or_else(|e| panic!("kvar=200: {}", e.message()));
    assert!(replayed.errors().is_empty(), "{:?}", replayed.errors());
    balance::solve_clean(&mut edited, "edited");
    balance::solve_clean(&mut replayed, "replayed");
    let yprim = edited.element_yprim("Reactor.r");
    assert_ne!(yprim, at_100, "kvar=200 moves the YPrim");
    assert_eq!(
        yprim,
        replayed.element_yprim("Reactor.r"),
        "kvar=200: YPrim"
    );
}

/// Replay `actions` on `full` through the applier.
fn replay(dss: &mut Dss, full: &str, actions: Vec<PosSeqAction>) -> Result<(), PosSeqRefusal> {
    let r = elem_id(dss, full);
    dss.apply_pos_seq_plan(
        r,
        &PosSeqPlan {
            actions,
            run_base: false,
        },
    )
}

/// After a refused replay: the error log is empty, `? <what>` still reads
/// `kept`, so the write after the refused one did not land, and the edit
/// `drain` adds no error, so the refusal left no message queued.
fn assert_stopped(dss: &mut Dss, what: &str, kept: &str, drain: &str) {
    assert!(dss.errors().is_empty(), "{what}: {:?}", dss.errors());
    assert_eq!(
        crate::exec::tests::common::query(dss, what),
        kept,
        "{what}: the later write"
    );
    run(dss, &[drain]);
}

/// A write the class refuses stops the apply with the write named, the
/// property in its defined spelling, and the class's own message. The write
/// after it does not run.
#[test]
fn a_write_the_class_refuses_names_the_write() {
    let mut dss = settext_deck();
    let refusal = replay(
        &mut dss,
        "Reactor.r",
        vec![
            PosSeqAction::SetText(reactor::prop::PHASES, "x".to_string()),
            PosSeqAction::SetText(reactor::prop::KVAR, "200".to_string()),
        ],
    )
    .expect_err("phases=x is no integer");
    assert!(
        matches!(refusal, PosSeqRefusal::ClassRefusedWrite { .. }),
        "{refusal:?}"
    );
    assert_eq!(
        refusal.message(),
        "MakePosSequence: Reactor.r.Phases=x was refused by the class: \
         Invalid inline math entry: \"x\"."
    );
    assert_stopped(&mut dss, "reactor.r.kvar", "100", "edit reactor.r kv=12.47");
}

/// A value the edit path refuses with a message while keeping the old value
/// stops the apply as an `Err` does: a 2 x 2 `rmatrix` on a three-phase
/// Reactor, whose message the parser logs, and a phase count a matrix Line
/// does not take, whose message the Line queues on itself. The class's
/// message leaves the log and the queue for the framed text, so a later edit
/// drains nothing, and the write after the refused one does not run.
#[test]
fn a_write_the_class_refuses_with_a_message_names_the_write() {
    let mut dss = settext_deck();
    let rmatrix = crate::exec::tests::common::query(&mut dss, "reactor.r.rmatrix");
    let refusal = replay(
        &mut dss,
        "Reactor.r",
        vec![
            PosSeqAction::SetText(reactor::prop::RMATRIX, "5|0 5".to_string()),
            PosSeqAction::SetText(reactor::prop::KVAR, "200".to_string()),
        ],
    )
    .expect_err("a 2 x 2 rmatrix does not fit three phases");
    assert!(
        matches!(refusal, PosSeqRefusal::ClassRefusedWrite { .. }),
        "{refusal:?}"
    );
    assert_eq!(
        refusal.message(),
        "MakePosSequence: Reactor.r.RMatrix=5|0 5 was refused by the class: \
         Reactor.r.RMatrix: The matrix entered does not match with the expected order, \
         review the entered parameters and try again."
    );
    assert_eq!(
        crate::exec::tests::common::query(&mut dss, "reactor.r.rmatrix"),
        rmatrix,
        "the refused write kept the old rmatrix"
    );
    assert_stopped(&mut dss, "reactor.r.kvar", "100", "edit reactor.r kv=12.47");

    let mut dss = balance::stiff("settextq");
    run(
        &mut dss,
        &["new line.lm bus1=b bus2=c phases=3 length=1 \
           rmatrix=[1|0 1|0 0 1] xmatrix=[2|0 2|0 0 2]"],
    );
    let refusal = replay(
        &mut dss,
        "Line.lm",
        vec![
            PosSeqAction::SetText(line::prop::PHASES, "1".to_string()),
            PosSeqAction::SetText(line::prop::LENGTH, "3".to_string()),
        ],
    )
    .expect_err("a matrix Line keeps its phase count");
    assert_eq!(
        refusal.message(),
        "MakePosSequence: Line.lm.Phases=1 was refused by the class: \
         Illegal change of number of phases for \"Line.lm\"."
    );
    assert_eq!(
        crate::exec::tests::common::query(&mut dss, "line.lm.phases"),
        "3",
        "the refused write kept the old phase count"
    );
    assert_stopped(&mut dss, "line.lm.length", "1", "edit line.lm length=2");
}

/// A value the recalc of its edit refuses with a message stops the apply as
/// a refusal of the edit path does, here a singular `rmatrix` on a parallel
/// matrix Reactor. A single edit names its write. A block runs the recalc
/// once, after all its writes, as an `Edit` line does, so its refusal names
/// every write of the block in order, and a block whose last `rmatrix` is
/// regular is accepted. The message leaves the queue for the framed text,
/// and the write after the refused edit does not run.
#[test]
fn a_write_whose_recalc_the_class_refuses_names_the_write() {
    let deck = || {
        let mut dss = balance::stiff("settextr");
        run(
            &mut dss,
            &["new reactor.rp phases=2 bus1=b.1.2 parallel=yes \
               rmatrix=[1|0 1] xmatrix=[10|0 10]"],
        );
        dss
    };
    let text = |idx: usize, value: &str| PosSeqAction::SetText(idx, value.to_string());
    let singular = || text(reactor::prop::RMATRIX, "1|1 1");
    let reason = "Reactor.rp: RMatrix=(1 |1 1 ) makes its admittance not a finite number, so it \
                  is not built.";
    let refused = format!("was refused by the class: {reason}");
    let both = format!("were refused by the class: {reason}");
    for (what, actions, message, kept) in [
        (
            "single",
            vec![singular(), text(reactor::prop::XMATRIX, "20|0 20")],
            format!("MakePosSequence: Reactor.rp.RMatrix=1|1 1 {refused}"),
            "(10 |0 10 )",
        ),
        (
            "block",
            vec![
                PosSeqAction::BeginEdit,
                singular(),
                text(reactor::prop::XMATRIX, "20|0 20"),
                PosSeqAction::EndEdit,
                text(reactor::prop::XMATRIX, "30|0 30"),
            ],
            format!(
                "MakePosSequence: Reactor.rp.RMatrix=1|1 1 and Reactor.rp.XMatrix=20|0 20 {both}"
            ),
            "(20 |0 20 )",
        ),
    ] {
        let mut dss = deck();
        let refusal = replay(&mut dss, "Reactor.rp", actions)
            .expect_err("a singular rmatrix leaves the parallel reactor no conductance");
        assert!(
            matches!(refusal, PosSeqRefusal::ClassRefusedWrite { .. }),
            "{what}: {refusal:?}"
        );
        assert_eq!(refusal.message(), message, "{what}");
        assert_stopped(
            &mut dss,
            "reactor.rp.xmatrix",
            kept,
            "edit reactor.rp rmatrix=[1|0 1]",
        );
    }

    let mut dss = deck();
    replay(
        &mut dss,
        "Reactor.rp",
        vec![
            PosSeqAction::BeginEdit,
            singular(),
            text(reactor::prop::RMATRIX, "2|0 2"),
            PosSeqAction::EndEdit,
        ],
    )
    .unwrap_or_else(|e| panic!("a regular last rmatrix: {}", e.message()));
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(
        crate::exec::tests::common::query(&mut dss, "reactor.rp.rmatrix"),
        "(2 |0 2 )"
    );
}

/// A message queued on the element before a `SetText` write is no refusal
/// of the write and stays queued, so the edit tail of the apply logs it.
#[test]
fn a_message_queued_before_a_write_stays_queued() {
    let mut dss = settext_deck();
    let r = elem_id(&dss, "Reactor.r");
    dss.classes[r.class_ord()]
        .arena
        .obj_mut(r.index())
        .data_mut()
        .push_error(DssDiagnostic::msg("queued before", None));
    set_text(&mut dss, "Reactor.r", reactor::prop::KVAR, "200")
        .unwrap_or_else(|e| panic!("kvar=200: {}", e.message()));
    let texts: Vec<&str> = dss.errors().iter().map(|e| e.text()).collect();
    assert_eq!(texts, ["queued before"]);
}

/// The repeat deck: a line, a 400 kW load and a second source on `c`, solved.
fn repeat_deck(name: &str) -> Dss {
    let mut dss = balance::stiff(name);
    run(
        &mut dss,
        &[
            "new line.l1 bus1=b bus2=c phases=3 r1=0.2 x1=0.5 length=1 units=km",
            "new load.ld bus1=c phases=3 kv=12.47 kw=400 pf=0.95",
            "new vsource.v2 bus1=c basekv=12.47 phases=3 r1=1 x1=3",
        ],
    );
    balance::solve_clean(&mut dss, "before makeposseq");
    dss
}

/// Run `makeposseq` on a circuit flagged positive-sequence and assert that it
/// adds no error and leaves the saved circuit (saved under `tag`)
/// byte-identical.
fn assert_unchanged(dss: &mut Dss, tag: &str) {
    assert!(dss.circuit().unwrap().positive_sequence, "{tag}: the flag");
    let n = dss.errors().len();
    let saved = balance::saved_circuit(dss, tag);
    dss.command("makeposseq");
    assert_eq!(dss.errors().len(), n, "{tag}: {:?}", &dss.errors()[n..]);
    assert_eq!(
        balance::saved_circuit(dss, tag),
        saved,
        "{tag}: the saved circuit"
    );
    assert!(dss.circuit().unwrap().positive_sequence, "{tag}: the flag");
}

/// The positive-sequence equivalent of a reduced circuit is that circuit, so
/// a second `makeposseq` changes nothing: the load keeps a third of its
/// 400 kW, not a ninth, and the second source keeps `12.47/√3` kV, not
/// `12.47/3`.
#[test]
fn a_second_makeposseq_changes_nothing() {
    let mut dss = repeat_deck("rep");
    run(&mut dss, &["makeposseq"]);
    assert_unchanged(&mut dss, "rep");
    let kw = balance::query_f64(&mut dss, "load.ld.kw");
    balance::assert_rel(kw, 400.0 / 3.0, 1e-12, "load.ld.kw");
    let kv = balance::query_f64(&mut dss, "vsource.v2.basekv");
    balance::assert_rel(kv, 12.47 / 3.0_f64.sqrt(), 1e-12, "vsource.v2.basekv");
}

/// `set cktmodel=positive` declares the circuit positive-sequence, so
/// `makeposseq` reads it as reduced and changes nothing.
#[test]
fn makeposseq_on_a_circuit_declared_positive_sequence_changes_nothing() {
    let mut dss = repeat_deck("decl");
    run(&mut dss, &["set cktmodel=positive"]);
    assert_unchanged(&mut dss, "decl");
}

/// A Save of a reduced circuit writes `Set Cktmodel=Positive`, so the reload
/// is flagged positive-sequence and `makeposseq` changes nothing on it.
#[test]
fn makeposseq_after_a_save_and_reload_changes_nothing() {
    let mut dss = repeat_deck("reload");
    run(&mut dss, &["makeposseq"]);
    let dir = std::env::temp_dir()
        .join("dss_posseq_reload")
        .join(format!("reload_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.to_string_lossy().replace('\\', "/");
    run(
        &mut dss,
        &[
            &format!("save circuit dir=\"{path}\""),
            "clear",
            &format!("compile \"{path}/Master.dss\""),
        ],
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert_unchanged(&mut dss, "reload");
}

/// The corpus deck `rel` compiled, with no error.
fn compile_corpus_deck(rel: &str) -> Dss {
    let deck = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(rel);
    assert!(deck.is_file(), "vendored corpus deck missing: {deck:?}");
    let mut dss = Dss::new();
    dss.command(&format!("compile \"{}\"", deck.display()));
    assert!(dss.errors().is_empty(), "{rel}: {:?}", dss.errors());
    dss
}

/// The bits of a complex number, so two readings compare bit for bit.
fn bits(z: Complex64) -> (u64, u64) {
    (z.re.to_bits(), z.im.to_bits())
}

/// One bus: its name, its node numbers and the bits of its node voltages.
type BusBits = (String, Vec<i32>, Vec<(u64, u64)>);

/// One element: its name and the bits of its terminal powers, its terminal
/// currents and its losses.
type ElementBits = (String, Vec<(u64, u64)>, Vec<(u64, u64)>, (u64, u64));

/// The repeat deck's second `makeposseq` changes nothing, so the deck runs
/// solve, reduce, solve, solve on the circuit of `makeposseq_pc.dss` and
/// holds that deck's solution after one more solve, bit for bit: every node
/// voltage, the system Y, the node injection currents, every element's
/// terminal powers, terminal currents and losses, and the YPrim of every
/// load.
#[test]
fn the_repeat_deck_holds_the_solution_of_the_once_converted_deck() {
    let mut once = compile_corpus_deck("modes/makeposseq/makeposseq_pc.dss");
    once.command("solve");
    assert!(once.errors().is_empty(), "{:?}", once.errors());
    let mut repeat = compile_corpus_deck("modes/makeposseq/makeposseq_pc_repeat.dss");

    let nodes = |dss: &Dss| -> Vec<BusBits> {
        let mut buses: Vec<_> = dss
            .all_bus_voltages()
            .into_iter()
            .map(|b| (b.name, b.nodes, b.node_v.into_iter().map(bits).collect()))
            .collect();
        buses.sort();
        buses
    };
    let (n_once, n_repeat) = (nodes(&once), nodes(&repeat));
    assert_eq!(n_once.len(), 4, "the four buses of the deck");
    assert_eq!(n_once, n_repeat, "the node voltages");

    let system_y = |dss: &mut Dss| {
        let (n, entries) = dss.system_y_csc().expect("the solve builds the system Y");
        let entries: Vec<_> = entries
            .into_iter()
            .map(|(r, c, y)| (r, c, bits(y)))
            .collect();
        (n, entries)
    };
    let (y_once, y_repeat) = (system_y(&mut once), system_y(&mut repeat));
    assert!(!y_once.1.is_empty(), "the system Y holds entries");
    assert_eq!(y_once, y_repeat, "the system Y");
    let injection = |dss: &Dss| -> Vec<(u64, u64)> {
        dss.node_injection_currents()
            .into_iter()
            .map(bits)
            .collect()
    };
    assert_eq!(
        injection(&once),
        injection(&repeat),
        "the node injection currents"
    );

    let elements = |dss: &mut Dss| -> Vec<ElementBits> {
        let mut elements: Vec<_> = dss
            .snapshot_elements()
            .into_iter()
            .map(|e| {
                (
                    e.name.to_ascii_lowercase(),
                    e.powers.into_iter().map(bits).collect(),
                    e.currents.into_iter().map(bits).collect(),
                    (e.loss_w.0.to_bits(), e.loss_w.1.to_bits()),
                )
            })
            .collect();
        elements.sort();
        elements
    };
    let (e_once, e_repeat) = (elements(&mut once), elements(&mut repeat));
    assert_eq!(e_once.len(), 16, "the sixteen elements of the deck");
    assert_eq!(e_once, e_repeat, "the element powers, currents and losses");

    for load in ["ld_wye", "ld_delta", "ld_1ph", "ld3"] {
        let yprim = |dss: &Dss| {
            let (n, y) = dss
                .element_yprim(&format!("Load.{load}"))
                .unwrap_or_else(|| panic!("Load.{load} has a YPrim"));
            (n, y.into_iter().map(bits).collect::<Vec<_>>())
        };
        assert_eq!(yprim(&once), yprim(&repeat), "the YPrim of Load.{load}");
    }
}

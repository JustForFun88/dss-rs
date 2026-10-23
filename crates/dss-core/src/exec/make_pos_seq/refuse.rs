//! Why MakePosSequence refuses a circuit: one variant per case, each with the
//! message the command pushes. Every message of the analysis names the element
//! and ends with ` Nothing was changed.`, since a refusal leaves the circuit as
//! it was. The apply's own error names the writes the class refused.

use crate::elements::pd::ABORTING_SOLUTION;

/// The one series value of a class that cannot hold circuits of different
/// phase count.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "the stage of the coupled elements constructs it")
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::exec) enum PosSeqSeries {
    /// The series `R` and `XL` of a Capacitor.
    RAndXl,
    /// The winding resistance of a GICTransformer.
    WindingResistance,
}

/// A refusal of the reduction. `element` fields hold `<Class>.<name>`, node
/// fields `<bus>.<node>`, and angles are in degrees.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the map, projection, chain and source stages construct the other variants"
    )
)]
#[derive(Debug, Clone, PartialEq)]
pub(in crate::exec) enum PosSeqRefusal {
    /// Two different phasors reach one node.
    TwoPhasors {
        node: String,
        first_deg: f64,
        second_deg: f64,
        first: String,
        second: String,
    },
    /// A multi-phase terminal of an element whose model depends on the order
    /// does not see its own pattern on its port. `nodes` is the terminal's bus
    /// with its node list.
    PortOutOfOrder { element: String, nodes: String },
    /// A Vsource or Isource of more than three phases.
    SourceTooManyPhases { element: String, nphases: usize },
    /// Two elements group the phase nodes of one bus differently.
    GroupsDiffer {
        first: String,
        second: String,
        bus: String,
    },
    /// A terminal whose phases are not one balanced set.
    NotOneBalancedSet { element: String, nodes: String },
    /// A Line conductor joins a port node to a node of another phasor.
    ConductorToAnotherPhasor {
        conductor: usize,
        element: String,
        from: String,
        to: String,
    },
    /// A lumped element mixes series and shunt conductors.
    SeriesAndShunt { element: String },
    /// The element's own solve refuses it, with this message.
    SolveRefuses { message: String },
    /// A fault conductance that is not passive. `form` is `"gmatrix"` or `"r"`.
    NotPassive { element: String, form: &'static str },
    /// A one- or two-phase fault.
    UnbalancedFault { element: String, nphases: usize },
    /// A lumped conductor between ports of different phasors.
    PortsOfDifferentPhasors {
        element: String,
        first: String,
        second: String,
    },
    /// A circuit open on one conductor at a terminal and closed on the others.
    PartlyOpen {
        element: String,
        conductor: usize,
        terminal: usize,
    },
    /// Transformer windings that leave a node without a value.
    WindingsLeaveNode { element: String, node: String },
    /// A class whose one-phase form does not hold its balanced power.
    NoOnePhaseForm { element: String },
    /// A joint star whose carrying elements cannot stay passive.
    StarCannotBeWritten { node: String, elements: Vec<String> },
    /// A coupled element whose class holds one series value for circuits of
    /// different phase count.
    MixedPhaseCount {
        element: String,
        series: PosSeqSeries,
    },
    /// The property edit path refused a value the analysis produced, or the
    /// recalc of an edit refused its writes: `writes` holds each write as
    /// `(property, value)`, in order.
    ClassRefusedWrite {
        element: String,
        writes: Vec<(String, String)>,
        message: String,
    },
}

impl PosSeqRefusal {
    /// The message the command pushes, with code `None`.
    pub(in crate::exec) fn message(&self) -> String {
        let what = match self {
            Self::TwoPhasors {
                node,
                first_deg,
                second_deg,
                first,
                second,
            } => format!(
                "node {node} receives two phasors ({} and {} degrees) through {first} and \
                 {second}.",
                degrees(*first_deg),
                degrees(*second_deg)
            ),
            Self::PortOutOfOrder { element, nodes } => format!(
                "{element} has its phases on {nodes}, which is not one port in positive order."
            ),
            Self::SourceTooManyPhases { element, nphases } => format!(
                "{element} has {nphases} phases. A source of more than three phases has no \
                 positive-sequence pattern."
            ),
            Self::GroupsDiffer { first, second, bus } => {
                format!("{first} and {second} group the phase nodes of bus {bus} differently.")
            }
            Self::NotOneBalancedSet { element, nodes } => {
                format!("{element} has its phases on {nodes}, which is not one balanced set.")
            }
            Self::ConductorToAnotherPhasor {
                conductor,
                element,
                from,
                to,
            } => format!(
                "conductor {conductor} of {element} joins {from} to {to}, a node of another \
                 phasor."
            ),
            Self::SeriesAndShunt { element } => {
                format!("{element} joins some conductors between ports and others to earth.")
            }
            Self::SolveRefuses { message } => message
                .strip_suffix(ABORTING_SOLUTION)
                .unwrap_or(message)
                .to_string(),
            Self::NotPassive { element, form } => {
                format!("the {form} of {element} is not passive.")
            }
            Self::UnbalancedFault { element, nphases } => {
                let phases = if *nphases == 1 { "phase" } else { "phases" };
                format!(
                    "{element} has {nphases} {phases}. A one- or two-phase fault has no \
                     positive-sequence form."
                )
            }
            Self::PortsOfDifferentPhasors {
                element,
                first,
                second,
            } => format!("{element} joins {first} and {second}, ports of different phasors."),
            Self::PartlyOpen {
                element,
                conductor,
                terminal,
            } => format!(
                "{element} has conductor {conductor} open at terminal {terminal} while its \
                 circuit's other phases are closed."
            ),
            Self::WindingsLeaveNode { element, node } => {
                format!("the windings of {element} leave node {node} without a balanced voltage.")
            }
            Self::NoOnePhaseForm { element } => format!("{element} has no one-phase form."),
            Self::StarCannotBeWritten { node, elements } => format!(
                "the star at {node} cannot be written into {}.",
                elements.join(", ")
            ),
            Self::MixedPhaseCount { element, series } => {
                let series = match series {
                    PosSeqSeries::RAndXl => "series R and XL",
                    PosSeqSeries::WindingResistance => "winding resistance",
                };
                format!("{element} joins circuits of different phase count through one {series}.")
            }
            Self::ClassRefusedWrite {
                element,
                writes,
                message,
            } => {
                // The apply has edited the circuit by then, so this message
                // does not say that nothing was changed. The class's message
                // ends at the one full stop this text puts after it.
                let message = message.strip_suffix('.').unwrap_or(message);
                let named: Vec<String> = writes
                    .iter()
                    .map(|(property, value)| format!("{element}.{property}={value}"))
                    .collect();
                let (named, verb) = match named.split_last() {
                    Some((last, rest)) if !rest.is_empty() => {
                        (format!("{} and {last}", rest.join(", ")), "were")
                    }
                    _ => (named.concat(), "was"),
                };
                return format!("MakePosSequence: {named} {verb} refused by the class: {message}.");
            }
        };
        format!("MakePosSequence: {what} Nothing was changed.")
    }
}

/// An angle in degrees with one decimal, in `[0, 360)` after the rounding.
fn degrees(deg: f64) -> String {
    let tenths = (deg * 10.0).round().rem_euclid(3600.0) + 0.0;
    format!("{:.1}", tenths / 10.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(text: &str) -> String {
        text.to_string()
    }

    /// Every variant reads exactly the message of its case, with the example
    /// values of the refusal table. The case-6 input carries the solve's
    /// [`ABORTING_SOLUTION`], which the command drops. An angle prints in
    /// `[0, 360)`, and one that rounds to a negative zero prints `0.0`.
    #[test]
    fn every_refusal_message_reads_as_written() {
        use PosSeqRefusal::*;
        let cases = [
            (
                TwoPhasors {
                    node: s("c.1"),
                    first_deg: 0.0,
                    second_deg: 120.0,
                    first: s("Line.l1"),
                    second: s("Line.l2"),
                },
                "MakePosSequence: node c.1 receives two phasors (0.0 and 120.0 degrees) through \
                 Line.l1 and Line.l2. Nothing was changed.",
            ),
            (
                TwoPhasors {
                    node: s("c.1"),
                    first_deg: 359.96,
                    second_deg: -120.0,
                    first: s("Line.l1"),
                    second: s("Line.l2"),
                },
                "MakePosSequence: node c.1 receives two phasors (0.0 and 240.0 degrees) through \
                 Line.l1 and Line.l2. Nothing was changed.",
            ),
            (
                TwoPhasors {
                    node: s("c.1"),
                    first_deg: -0.04,
                    second_deg: 120.0,
                    first: s("Line.l1"),
                    second: s("Line.l2"),
                },
                "MakePosSequence: node c.1 receives two phasors (0.0 and 120.0 degrees) through \
                 Line.l1 and Line.l2. Nothing was changed.",
            ),
            (
                // The angle of the phasor 1 - j1e-17.
                TwoPhasors {
                    node: s("c.1"),
                    first_deg: 240.0,
                    second_deg: -5.729577951308233e-16,
                    first: s("Line.l1"),
                    second: s("Line.l2"),
                },
                "MakePosSequence: node c.1 receives two phasors (240.0 and 0.0 degrees) through \
                 Line.l1 and Line.l2. Nothing was changed.",
            ),
            (
                PortOutOfOrder {
                    element: s("IndMach012.mn"),
                    nodes: s("c.1.3.2"),
                },
                "MakePosSequence: IndMach012.mn has its phases on c.1.3.2, which is not one port \
                 in positive order. Nothing was changed.",
            ),
            (
                SourceTooManyPhases {
                    element: s("Vsource.v4"),
                    nphases: 4,
                },
                "MakePosSequence: Vsource.v4 has 4 phases. A source of more than three phases has \
                 no positive-sequence pattern. Nothing was changed.",
            ),
            (
                SourceTooManyPhases {
                    element: s("Isource.i4"),
                    nphases: 4,
                },
                "MakePosSequence: Isource.i4 has 4 phases. A source of more than three phases has \
                 no positive-sequence pattern. Nothing was changed.",
            ),
            (
                GroupsDiffer {
                    first: s("Vsource.source"),
                    second: s("Reactor.r14"),
                    bus: s("b"),
                },
                "MakePosSequence: Vsource.source and Reactor.r14 group the phase nodes of bus b \
                 differently. Nothing was changed.",
            ),
            (
                NotOneBalancedSet {
                    element: s("Load.l4"),
                    nodes: s("d.1.2.3.4"),
                },
                "MakePosSequence: Load.l4 has its phases on d.1.2.3.4, which is not one balanced \
                 set. Nothing was changed.",
            ),
            (
                ConductorToAnotherPhasor {
                    conductor: 3,
                    element: s("Line.l"),
                    from: s("b.3"),
                    to: s("c.0"),
                },
                "MakePosSequence: conductor 3 of Line.l joins b.3 to c.0, a node of another \
                 phasor. Nothing was changed.",
            ),
            (
                SeriesAndShunt {
                    element: s("Reactor.r"),
                },
                "MakePosSequence: Reactor.r joins some conductors between ports and others to \
                 earth. Nothing was changed.",
            ),
            (
                SolveRefuses {
                    message: format!(
                        "Reactor.rw has 2 phases but its rmatrix has 3 x 3 entries. Specify \
                         rmatrix and xmatrix for 2 phases.{ABORTING_SOLUTION}"
                    ),
                },
                "MakePosSequence: Reactor.rw has 2 phases but its rmatrix has 3 x 3 entries. \
                 Specify rmatrix and xmatrix for 2 phases. Nothing was changed.",
            ),
            (
                SolveRefuses {
                    message: format!(
                        "Matrix Inversion Error for Line \"ls\". Invalid impedance \
                         specified.{ABORTING_SOLUTION}"
                    ),
                },
                "MakePosSequence: Matrix Inversion Error for Line \"ls\". Invalid impedance \
                 specified. Nothing was changed.",
            ),
            (
                SolveRefuses {
                    message: s("Line.lg: the geometry gives no impedance."),
                },
                "MakePosSequence: Line.lg: the geometry gives no impedance. Nothing was changed.",
            ),
            (
                NotPassive {
                    element: s("Fault.f"),
                    form: "gmatrix",
                },
                "MakePosSequence: the gmatrix of Fault.f is not passive. Nothing was changed.",
            ),
            (
                NotPassive {
                    element: s("Fault.f"),
                    form: "r",
                },
                "MakePosSequence: the r of Fault.f is not passive. Nothing was changed.",
            ),
            (
                UnbalancedFault {
                    element: s("Fault.f"),
                    nphases: 2,
                },
                "MakePosSequence: Fault.f has 2 phases. A one- or two-phase fault has no \
                 positive-sequence form. Nothing was changed.",
            ),
            (
                UnbalancedFault {
                    element: s("Fault.f"),
                    nphases: 1,
                },
                "MakePosSequence: Fault.f has 1 phase. A one- or two-phase fault has no \
                 positive-sequence form. Nothing was changed.",
            ),
            (
                PortsOfDifferentPhasors {
                    element: s("Reactor.r"),
                    first: s("b.1"),
                    second: s("c.2"),
                },
                "MakePosSequence: Reactor.r joins b.1 and c.2, ports of different phasors. \
                 Nothing was changed.",
            ),
            (
                PartlyOpen {
                    element: s("Line.l"),
                    conductor: 2,
                    terminal: 2,
                },
                "MakePosSequence: Line.l has conductor 2 open at terminal 2 while its circuit's \
                 other phases are closed. Nothing was changed.",
            ),
            (
                WindingsLeaveNode {
                    element: s("Transformer.ty"),
                    node: s("b.5"),
                },
                "MakePosSequence: the windings of Transformer.ty leave node b.5 without a \
                 balanced voltage. Nothing was changed.",
            ),
            (
                NoOnePhaseForm {
                    element: s("UPFC.u"),
                },
                "MakePosSequence: UPFC.u has no one-phase form. Nothing was changed.",
            ),
            (
                StarCannotBeWritten {
                    node: s("b.4"),
                    elements: vec![s("Capacitor.c1"), s("Capacitor.c2")],
                },
                "MakePosSequence: the star at b.4 cannot be written into Capacitor.c1, \
                 Capacitor.c2. Nothing was changed.",
            ),
            (
                MixedPhaseCount {
                    element: s("Capacitor.c5"),
                    series: PosSeqSeries::RAndXl,
                },
                "MakePosSequence: Capacitor.c5 joins circuits of different phase count through \
                 one series R and XL. Nothing was changed.",
            ),
            (
                MixedPhaseCount {
                    element: s("GICTransformer.g"),
                    series: PosSeqSeries::WindingResistance,
                },
                "MakePosSequence: GICTransformer.g joins circuits of different phase count \
                 through one winding resistance. Nothing was changed.",
            ),
            (
                ClassRefusedWrite {
                    element: s("Reactor.r"),
                    writes: vec![(s("bus2"), s("x"))],
                    message: s("<message>"),
                },
                "MakePosSequence: Reactor.r.bus2=x was refused by the class: <message>.",
            ),
            (
                ClassRefusedWrite {
                    element: s("Reactor.r"),
                    writes: vec![(s("R"), s("1")), (s("X"), s("2"))],
                    message: s("<message>"),
                },
                "MakePosSequence: Reactor.r.R=1 and Reactor.r.X=2 were refused by the \
                 class: <message>.",
            ),
            (
                ClassRefusedWrite {
                    element: s("Reactor.r"),
                    writes: vec![(s("R"), s("1")), (s("X"), s("2")), (s("kV"), s("3"))],
                    message: s("<message>"),
                },
                "MakePosSequence: Reactor.r.R=1, Reactor.r.X=2 and Reactor.r.kV=3 were \
                 refused by the class: <message>.",
            ),
        ];
        for (refusal, want) in cases {
            assert_eq!(refusal.message(), want, "{refusal:?}");
        }
    }
}

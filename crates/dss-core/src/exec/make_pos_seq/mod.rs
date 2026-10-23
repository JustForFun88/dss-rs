//! `MakePosSeq`: reduce the circuit to its positive-sequence equivalent.
//!
//! On a circuit already positive-sequence the command changes nothing.
//! Otherwise it analyses the whole circuit first and edits nothing there
//! (`analyse`). An element the reduction cannot take stops the command with
//! the element named and the circuit unchanged (`refuse`). Once the analysis
//! accepted the circuit, the command flags it positive-sequence and reduces
//! every element in the analysis's order through the applier (`apply`): each
//! element converts its own fields and returns the property writes the
//! applier replays through the typed setters, with the post-edit signal tail
//! every property edit runs.

pub(in crate::exec) mod analyse;
mod apply;
pub(in crate::exec) mod refuse;

use super::*;

impl Dss {
    /// Reduce the circuit to its positive-sequence equivalent. A circuit
    /// already positive-sequence is left as it is, with no message: the guard
    /// reads `Circuit::positive_sequence`, which a first `MakePosSeq`,
    /// `Set CktModel=Positive` and the reload of a saved reduced circuit
    /// (`Set Cktmodel=Positive`) all set (pinned by
    /// `exec::tests::make_pos_seq::refusals::a_second_makeposseq_changes_nothing`).
    /// Otherwise the analysis runs first: a refusal pushes its message with
    /// code `None` and leaves the circuit as it was (pinned by
    /// `elements::pd::fault::tests::makeposseq_refuses_a_two_phase_gmatrix_fault_and_keeps_its_power`).
    /// On acceptance the circuit is flagged positive-sequence and every
    /// element of the analysis's order is reduced: every source, PD and PC
    /// element in creation order, then every control and meter. A write the
    /// class refuses during the apply is a defect of the analysis, so the
    /// command stops there with its message.
    pub(super) fn do_make_pos_seq(&mut self) {
        let Some(ckt) = self.circuit.as_ref() else {
            return;
        };
        if ckt.positive_sequence {
            return;
        }
        let reduction = match self.analyse_pos_seq() {
            Ok(reduction) => reduction,
            Err(refusal) => {
                self.errors
                    .push(crate::diag::DssDiagnostic::msg(refusal.message(), None));
                return;
            }
        };
        if let Some(ckt) = self.circuit.as_mut() {
            ckt.positive_sequence = true;
        }
        for r in reduction.order {
            let ctx = self.build_pos_seq_ctx(r);
            let plan = match self.classes[r.class_ord()]
                .arena
                .try_ckt_elem_mut(r.index())
            {
                Some(elem) => elem.make_pos_sequence(&ctx),
                None => continue, // every `ckt_elements` entry is a circuit element
            };
            if let Err(refusal) = self.apply_pos_seq_plan(r, &plan) {
                let message = refusal.message();
                self.errors
                    .push(crate::diag::DssDiagnostic::msg(message.clone(), None));
                debug_assert!(false, "{message}");
                return;
            }
        }
    }
}

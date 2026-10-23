//! The analysis: it reads the whole circuit, refuses what the reduction cannot
//! take and orders the elements the apply reduces. It edits nothing.

use crate::elements::pd::Fault;
use crate::elements::pos_seq::project::is_passive;
use crate::exec::make_pos_seq::refuse::PosSeqRefusal;
use crate::exec::*;

/// What the analysis hands the apply.
pub(in crate::exec) struct PosSeqReduction {
    /// The elements to reduce, in apply order: every source, PD and PC element
    /// in creation order, then every control and meter in creation order, so
    /// each control reads its target reduced. Disabled elements are ordered
    /// like enabled ones.
    pub(in crate::exec) order: Vec<ElemId>,
}

impl Dss {
    /// Analyse the circuit for MakePosSequence. Every circuit element is
    /// checked in creation order, disabled ones included: the reason its own
    /// solve refuses it, then for a Fault its phase count and the passivity of
    /// its conductance. The first refusal is returned.
    pub(in crate::exec) fn analyse_pos_seq(&mut self) -> Result<PosSeqReduction, PosSeqRefusal> {
        let Some(ckt) = self.circuit.as_ref() else {
            return Ok(PosSeqReduction { order: Vec::new() });
        };
        let mut order = Vec::with_capacity(ckt.ckt_elements.len());
        let mut controls = Vec::new();
        for &r in &ckt.ckt_elements {
            let class = &self.classes[r.class_ord()];
            let Some(elem) = class.arena.try_ckt_elem(r.index()) else {
                continue; // every `ckt_elements` entry is a circuit element
            };
            if let Some(message) = elem.pos_seq_refusal() {
                return Err(PosSeqRefusal::SolveRefuses { message });
            }
            if class.kind == Some(ElemKind::Fault)
                && let Some(fault) = class.arena.get::<Fault>(r.index())
            {
                let element = format!("{}.{}", class.props.class_name(), fault.cd.obj.name());
                if let Some(refusal) = fault_refusal(fault, element) {
                    return Err(refusal);
                }
            }
            match class.kind {
                Some(
                    ElemKind::Control | ElemKind::Meter | ElemKind::EnergyMeter | ElemKind::Sensor,
                ) => controls.push(r),
                _ => order.push(r),
            }
        }
        order.extend(controls);
        Ok(PosSeqReduction { order })
    }
}

/// A one- or two-phase fault has no positive-sequence form, and a fault whose
/// conductance is not passive has no passive one. The phase count is read
/// first, since passivity is a property of a form that exists.
fn fault_refusal(fault: &Fault, element: String) -> Option<PosSeqRefusal> {
    let nphases = fault.cd.nphases;
    if nphases <= 2 {
        return Some(PosSeqRefusal::UnbalancedFault { element, nphases });
    }
    let (g, form) = fault.pos_seq_conductance();
    (!is_passive(&g, nphases)).then_some(PosSeqRefusal::NotPassive { element, form })
}

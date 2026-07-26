//! The `impl CktElement` / `impl DssObject` property surface and the
//! `capture_metered` RefSnapshot helper.

use num_complex::Complex64;

use super::{EmSnapshot, EnergyMeter, NUM_EM_REGISTERS};
use crate::elements::ckt::CktElementData;
use crate::elements::pd::capacitor::Capacitor;
use crate::elements::pd::line::Line;
use crate::elements::pd::reactor::Reactor;
use crate::elements::pd::transformer::Transformer;
use crate::elements::pos_seq::{PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, ElemId, SysCtx};
use crate::obj::arena::ResolvedObj;
use crate::obj::base::{DssObjData, DssObject};

/// Capture the parse-relevant shape of the metered element (the RefSnapshot).
pub(crate) fn capture_metered(full_name: String, obj: &dyn DssObject) -> EmSnapshot {
    let elem = obj
        .as_ckt_element()
        .expect("element= resolves to a ckt elem");
    let cd = elem.cd();
    // Pascal checks `BASECLASSMASK = PD_ELEMENT` — AutoTrans qualifies like any
    // other PD element (no transformer special-casing in EnergyMeter:
    // `IsTransformerElement` matches XFMR_ELEMENT only, Utilities.pas:728).
    let is_pd = obj.as_any().downcast_ref::<Line>().is_some()
        || obj.as_any().downcast_ref::<Transformer>().is_some()
        || obj
            .as_any()
            .downcast_ref::<crate::elements::pd::auto_trans::AutoTrans>()
            .is_some()
        || obj.as_any().downcast_ref::<Capacitor>().is_some()
        || obj.as_any().downcast_ref::<Reactor>().is_some();
    EmSnapshot {
        full_name,
        is_pd,
        nphases: cd.nphases,
        nconds: cd.nconds,
        nterms: cd.nterms,
        buses: (1..=cd.nterms).map(|i| cd.get_bus(i).to_string()).collect(),
    }
}

impl CktElement for EnergyMeter {
    fn cd(&self) -> &CktElementData {
        &self.med.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.med.cd
    }

    fn recalc_element_data(&mut self, _sys: &SysCtx) {
        let mut errors = crate::diag::ErrorLog::new();
        self.recalc(&mut errors);
        for e in errors {
            self.med.cd.obj.push_error(e);
        }
    }

    /// `TEnergyMeterObj.CalcYPrim` is empty — a meter never stamps admittance.
    fn calc_yprim(&mut self, _sys: &SysCtx) {}

    /// `TEnergyMeterObj.GetCurrents` returns zeros.
    fn get_currents(&mut self, _sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
        curr.fill(Complex64::ZERO);
    }

    /// Pascal `TEnergyMeterObj.MakePosSequence` (`Meters/EnergyMeter.pas:1185`):
    /// when the metered element is set, resync the meter to its bus / phase /
    /// conductor counts, reallocate the sensor arrays, and drop the (now stale)
    /// zone branch tree, then run the base bus rename (`inherited`). Pascal
    /// NIL-guards `MeteredElement`; `ctx.monitored` is `None` in the same case.
    fn make_pos_sequence(&mut self, ctx: &PosSeqCtx) -> PosSeqPlan {
        if let Some(m) = &ctx.monitored {
            // Setbus(1, MeteredElement.GetBus(MeteredTerminal))
            let mt = self.med.metered_terminal as usize;
            let bus = mt
                .checked_sub(1)
                .and_then(|k| m.bus_names.get(k))
                .cloned()
                .unwrap_or_default();
            self.med.cd.set_bus(1, &bus);
            // FNphases := MeteredElement.NPhases; Nconds := MeteredElement.Nconds
            self.med.cd.nphases = m.nphases;
            self.med.cd.set_nconds(m.nconds);
            // AllocateSensorArrays (calc buffers sized to the metered Yorder,
            // sensor arrays to the new Fnphases).
            self.med.allocate_sensor_arrays(m.yorder);
            // if BranchList <> NIL then BranchList.Free; BranchList := NIL
            self.branch_list = None;
        }
        // inherited MakePosSequence -> base bus rename.
        PosSeqPlan::base()
    }

    /// Pascal `TMeterElement.MeteredElement` — resolved so the exec applier can
    /// build [`PosSeqCtx::monitored`] before calling [`Self::make_pos_sequence`].
    fn monitored_element_ref(&self) -> Option<ElemId> {
        self.med.metered_element
    }
}

impl EnergyMeter {
    pub(crate) fn make_like(&mut self, other: &Self) {
        let o = other;
        self.med.cd.make_like_base(&o.med.cd);
        self.med.cd.nphases = o.med.cd.nphases;
        self.med.cd.set_nconds(o.med.cd.nconds);
        self.med.metered_element = o.med.metered_element;
        self.med.metered_terminal = o.med.metered_terminal;
        self.metered_snap = o.metered_snap.clone();
        self.element_full_name = o.element_full_name.clone();
        self.excess_flag = o.excess_flag;
        self.max_zone_kva_norm = o.max_zone_kva_norm;
        self.max_zone_kva_emerg = o.max_zone_kva_emerg;
        self.source_num_interruptions = o.source_num_interruptions;
        self.source_int_duration = o.source_int_duration;
        self.defined_zone_list = o.defined_zone_list.clone();
        self.local_only = o.local_only;
        self.voltage_ue_only = o.voltage_ue_only;
        self.f_losses = o.f_losses;
        self.f_line_losses = o.f_line_losses;
        self.f_xfmr_losses = o.f_xfmr_losses;
        self.f_seq_losses = o.f_seq_losses;
        self.f_3phase_losses = o.f_3phase_losses;
        self.f_vbase_losses = o.f_vbase_losses;
        self.f_phase_voltage_report = o.f_phase_voltage_report;
    }
}

impl DssObject for EnergyMeter {
    fn data(&self) -> &DssObjData {
        &self.med.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.med.cd.obj
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn as_ckt_element(&self) -> Option<&dyn CktElement> {
        Some(self)
    }
    fn as_ckt_element_mut(&mut self) -> Option<&mut dyn CktElement> {
        Some(self)
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use super::prop::*;
        match idx {
            TERMINAL => self.med.metered_terminal,
            _ => unreachable!("EnergyMeter has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use super::prop::*;
        match idx {
            TERMINAL => self.med.metered_terminal = value,
            _ => unreachable!("EnergyMeter has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use super::prop::*;
        match idx {
            LOCAL_ONLY => self.local_only,
            LOSSES => self.f_losses,
            LINE_LOSSES => self.f_line_losses,
            XFMR_LOSSES => self.f_xfmr_losses,
            SEQ_LOSSES => self.f_seq_losses,
            THREE_PHASE_LOSSES => self.f_3phase_losses,
            VBASE_LOSSES => self.f_vbase_losses,
            PHASE_VOLTAGE_REPORT => self.f_phase_voltage_report,
            ENABLED => self.med.cd.enabled,
            _ => unreachable!("EnergyMeter has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use super::prop::*;
        match idx {
            LOCAL_ONLY => self.local_only = value,
            LOSSES => self.f_losses = value,
            LINE_LOSSES => self.f_line_losses = value,
            XFMR_LOSSES => self.f_xfmr_losses = value,
            SEQ_LOSSES => self.f_seq_losses = value,
            THREE_PHASE_LOSSES => self.f_3phase_losses = value,
            VBASE_LOSSES => self.f_vbase_losses = value,
            PHASE_VOLTAGE_REPORT => self.f_phase_voltage_report = value,
            ENABLED => self.med.cd.set_enabled(value),
            _ => unreachable!("EnergyMeter has no boolean property {idx}"),
        }
    }

    fn get_f64(&self, idx: usize) -> f64 {
        use super::prop::*;
        match idx {
            KVA_NORMAL => self.max_zone_kva_norm,
            KVA_EMERG => self.max_zone_kva_emerg,
            INT_RATE => self.source_num_interruptions,
            INT_DURATION => self.source_int_duration,
            SAIFI => self.saifi,
            SAIFI_KW => self.saifi_kw,
            SAIDI => self.saidi,
            CAIDI => self.caidi,
            CUST_INTERRUPTS => self.cust_interrupts,
            BASE_FREQ => self.med.cd.base_frequency,
            _ => unreachable!("EnergyMeter has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use super::prop::*;
        match idx {
            KVA_NORMAL => self.max_zone_kva_norm = value,
            KVA_EMERG => self.max_zone_kva_emerg = value,
            INT_RATE => self.source_num_interruptions = value,
            INT_DURATION => self.source_int_duration = value,
            // Read-only reliability props (Pascal SilentReadOnly): ignore writes.
            SAIFI | SAIFI_KW | SAIDI | CAIDI | CUST_INTERRUPTS => {}
            BASE_FREQ => self.med.cd.base_frequency = value,
            _ => unreachable!("EnergyMeter has no double property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        use super::prop::*;
        match idx {
            PEAK_CURRENT => Some(&self.med.sensor_current),
            MASK => Some(&self.totals_mask),
            _ => unreachable!("EnergyMeter has no double-array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        use super::prop::*;
        match idx {
            PEAK_CURRENT => self.med.sensor_current = value,
            MASK => {
                self.totals_mask = value;
                self.totals_mask.resize(NUM_EM_REGISTERS, 0.0);
            }
            _ => unreachable!("EnergyMeter has no double-array property {idx}"),
        }
    }
    fn array_size(&self, idx: usize) -> usize {
        use super::prop::*;
        match idx {
            PEAK_CURRENT => self.med.cd.nphases,
            _ => unreachable!("EnergyMeter has no function-sized array {idx}"),
        }
    }

    fn get_string_list(&self, idx: usize) -> Vec<String> {
        use super::prop::*;
        match idx {
            // Pascal `GetOptions`: E/T, R/M, V/C.
            OPTION => vec![
                if self.excess_flag { "E" } else { "T" }.to_string(),
                if self.zone_is_radial { "R" } else { "M" }.to_string(),
                if self.voltage_ue_only { "V" } else { "C" }.to_string(),
            ],
            ZONE_LIST => self.defined_zone_list.clone(),
            _ => unreachable!("EnergyMeter has no string-list property {idx}"),
        }
    }
    fn set_string_list(&mut self, idx: usize, value: Vec<String>) {
        use super::prop::*;
        match idx {
            // Pascal `SetOptions`: branch on the first character of each token.
            OPTION => {
                for s in value {
                    match s.chars().next().map(|c| c.to_ascii_lowercase()) {
                        Some('e') => self.excess_flag = true,
                        Some('t') => self.excess_flag = false,
                        Some('r') => self.zone_is_radial = true,
                        Some('m') => self.zone_is_radial = false,
                        Some('c') => self.voltage_ue_only = false,
                        Some('v') => self.voltage_ue_only = true,
                        _ => {}
                    }
                }
            }
            ZONE_LIST => self.defined_zone_list = value,
            _ => unreachable!("EnergyMeter has no string-list property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use super::prop::*;
        match idx {
            ELEMENT => self.element_full_name.clone(),
            _ => unreachable!("EnergyMeter has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        use super::prop::*;
        match idx {
            ELEMENT => self.element_full_name = value,
            _ => unreachable!("EnergyMeter has no string property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.med.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.med.cd.get_bus(terminal).to_string()
    }

    /// Resolve `element=` (any circuit class by full name) and snapshot it.
    fn set_object_ref(&mut self, idx: usize, name: String, resolved: Option<ResolvedObj<'_>>) {
        use super::prop::*;
        match idx {
            ELEMENT => {
                self.element_full_name = name.clone();
                match resolved {
                    Some(o) => {
                        self.med.metered_element = Some(o.id());
                        self.med.metered_element_changed = true;
                        self.metered_snap = Some(capture_metered(name, o.obj()));
                    }
                    None => {
                        self.med.metered_element = None;
                        self.metered_snap = None;
                    }
                }
            }
            _ => unreachable!("EnergyMeter has no object-ref property {idx}"),
        }
    }

    fn side_effects(&mut self, idx: usize, prev_int: i32) {
        use super::prop::*;
        match idx {
            ELEMENT | TERMINAL => {
                self.med.metered_element_changed = true;
                self.needs_recalc = true;
            }
            MASK => {
                // Pascal: the slots past the supplied values default to 1.0.
                let start = (prev_int.max(0) as usize).min(NUM_EM_REGISTERS);
                for v in &mut self.totals_mask[start..] {
                    *v = 1.0;
                }
            }
            _ => {}
        }
    }

    /// Pascal `DoAction`: Clear → `ResetRegisters`; the others
    /// (Allocate/Reduce/Save/TakeSample/ZoneDump) need the live circuit and are
    /// driven from the executive / later work packages, so they no-op here.
    fn do_action(&mut self, ordinal: i32, _errors: &mut crate::diag::ErrorLog) {
        if ordinal == 1 {
            self.reset_registers();
        }
    }

    fn end_edit(&mut self, _sys: &crate::elements::traits::SysCtx) {
        // Pascal `EndEdit`: only recalc when a basic datum (element/terminal)
        // changed, so editing e.g. `kVANormal` alone doesn't re-run validation.
        if !self.needs_recalc {
            return;
        }
        let mut errors = crate::diag::ErrorLog::new();
        self.recalc(&mut errors);
        for e in errors {
            self.med.cd.obj.push_error(e);
        }
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod make_pos_seq_tests {
    use super::*;
    use crate::circuit::ckt_tree::CktTree;
    use crate::elements::pos_seq::{PosSeqCtx, PosSeqElemInfo};

    /// Pascal `TEnergyMeterObj.MakePosSequence` (EnergyMeter.pas:1185): resync
    /// to the metered element (bus at `MeteredTerminal`, phases, conds), realloc
    /// the sensor arrays, and drop the zone branch tree; `inherited` runs the
    /// base rename. Cross-check: `makeposseq_ctrl.dss` metered line is 1-phase.
    #[test]
    fn resyncs_to_metered_element_and_drops_branch_list() {
        let mut em = EnergyMeter::new("m1");
        em.med.metered_element = Some(ElemId::new(1, 2));
        em.med.metered_terminal = 2; // GetBus(2)
        em.put_branch_list(CktTree::new());
        assert!(em.has_branch_list());

        let ctx = PosSeqCtx {
            monitored: Some(PosSeqElemInfo {
                bus_names: vec!["b1.1.2.3".into(), "b2.1".into()],
                nphases: 1,
                nconds: 1,
                yorder: 2,
                ..Default::default()
            }),
            ..Default::default()
        };
        let plan = em.make_pos_sequence(&ctx);

        assert_eq!(em.get_bus_name(1), "b2.1"); // Setbus(1, GetBus(2))
        assert_eq!(em.cd().nphases, 1);
        assert_eq!(em.cd().nconds, 1);
        // AllocateSensorArrays: calc buffers = metered Yorder (2); per-phase
        // arrays = the new Fnphases (1).
        assert_eq!(em.med.calculated_current.len(), 2);
        assert_eq!(em.med.calculated_voltage.len(), 2);
        assert_eq!(em.med.sensor_current.len(), 1);
        assert!(!em.has_branch_list()); // BranchList := NIL
        assert!(plan.run_base && plan.actions.is_empty()); // inherited only
        // monitored_element_ref resolves the metered element for the applier.
        assert_eq!(em.monitored_element_ref(), Some(ElemId::new(1, 2)));
    }

    /// Pascal NIL guard: no metered element ⇒ the body is skipped, only the base
    /// rename (`inherited`) runs.
    #[test]
    fn nil_metered_element_is_noop_but_runs_base() {
        let mut em = EnergyMeter::new("m1");
        let (np, nc) = (em.cd().nphases, em.cd().nconds);
        let plan = em.make_pos_sequence(&PosSeqCtx::default());
        assert_eq!((em.cd().nphases, em.cd().nconds), (np, nc));
        assert!(plan.run_base);
    }
}

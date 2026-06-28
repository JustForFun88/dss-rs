//! The `impl CktElement` / `impl DssObject` property surface and the
//! metered-element snapshot capture (`capture_metered`).

use num_complex::Complex64;

use super::Monitor;
use crate::elements::ckt::CktElementData;
use crate::elements::meter::meter_element::{MeteredKind, MeteredSnapshot};
use crate::elements::pd::capacitor::Capacitor;
use crate::elements::pd::transformer::Transformer;
use crate::elements::traits::{CktElement, ElemRef, SysCtx};
use crate::obj::base::{DssObjData, DssObject};

impl CktElement for Monitor {
    fn cd(&self) -> &CktElementData {
        &self.med.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.med.cd
    }

    fn recalc_element_data(&mut self, sys: &SysCtx) {
        let mut errors = Vec::new();
        self.recalc(&mut errors, sys.is_harmonic_model);
        for e in errors {
            self.med.cd.obj.push_error(e);
        }
    }

    /// `TMonitorObj.CalcYPrim` is empty — a monitor never stamps admittance.
    fn calc_yprim(&mut self, _sys: &SysCtx) {}

    /// `TMonitorObj.GetCurrents` returns zeros (12-7-99 fix: a monitor is a
    /// zero-current source so it does not perturb Newton iteration).
    fn get_currents(&mut self, _sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
        curr.fill(Complex64::ZERO);
    }
}

impl DssObject for Monitor {
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
            MODE => self.mode,
            _ => unreachable!("Monitor has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use super::prop::*;
        match idx {
            TERMINAL => self.med.metered_terminal = value,
            MODE => self.mode = value,
            _ => unreachable!("Monitor has no integer property {idx}"),
        }
    }

    fn get_f64(&self, idx: usize) -> f64 {
        match idx {
            super::prop::BASE_FREQ => self.med.cd.base_frequency,
            _ => unreachable!("Monitor has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        match idx {
            super::prop::BASE_FREQ => self.med.cd.base_frequency = value,
            _ => unreachable!("Monitor has no double property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use super::prop::*;
        match idx {
            RESIDUAL => self.include_residual,
            VIPOLAR => self.vi_polar,
            PPOLAR => self.pp_polar,
            ENABLED => self.med.cd.enabled,
            _ => unreachable!("Monitor has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use super::prop::*;
        match idx {
            RESIDUAL => self.include_residual = value,
            VIPOLAR => self.vi_polar = value,
            PPOLAR => self.pp_polar = value,
            ENABLED => self.med.cd.set_enabled(value),
            _ => unreachable!("Monitor has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        match idx {
            super::prop::ELEMENT => self.element_full_name.clone(),
            _ => unreachable!("Monitor has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        match idx {
            super::prop::ELEMENT => self.element_full_name = value,
            _ => unreachable!("Monitor has no string property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.med.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.med.cd.get_bus(terminal).to_string()
    }

    /// Resolve `element=` (any circuit class by full name): capture a snapshot
    /// of the metered element for `RecalcElementData`/`ClearMonitorStream`.
    fn set_object_ref(
        &mut self,
        idx: usize,
        name: String,
        resolved: Option<(ElemRef, &dyn DssObject)>,
    ) {
        match idx {
            super::prop::ELEMENT => {
                self.element_full_name = name.clone();
                match resolved {
                    Some((r, obj)) => {
                        self.med.metered_element = Some(r);
                        self.med.metered_element_changed = true;
                        self.med.metered_snap = Some(capture_metered(name, obj));
                    }
                    None => {
                        self.med.metered_element = None;
                        self.med.metered_snap = None;
                    }
                }
            }
            _ => unreachable!("Monitor has no object-ref property {idx}"),
        }
    }

    /// Pascal `DoAction`: Clear/Reset → `ResetIt`; Save/TakeSample/Process are
    /// meaningful only via the `Sample` command / solution loop (they need the
    /// live metered element + node voltages), so they no-op here.
    fn do_action(&mut self, ordinal: i32, _errors: &mut Vec<String>) {
        if ordinal == 0 {
            // `Action=Clear/Reset` runs at parse time with no solution context,
            // so the header is rebuilt with the fundamental `hour`/`t(sec)`
            // labels; the `Set mode=harmonics` reset relabels them (see
            // `end_edit` for the one residual harmonics-mode divergence).
            self.reset_it(false);
        }
    }

    fn end_edit(&mut self) {
        // Pascal `RecalcElementData` reads the live `IsHarmonicModel`, but the
        // `DssObject` edit surface carries no solution state, so the header is
        // built with the fundamental `hour`/`t(sec)` labels here. The harmonic
        // `Freq`/`Harmonic` labels are applied when `Set mode=harmonics` resets
        // every monitor (Pascal `Set_Mode` -> `ClearMonitorStream` with
        // `IsHarmonicModel=TRUE`) — the path that matters in practice, since
        // monitors are defined before the first solve. The one residual
        // divergence (editing/clearing a monitor *after* entering harmonics
        // mode, where Pascal would write `Freq`/`Harmonic`) relabels only on the
        // next `Set mode=`/`Reset Monitors`; it is unobservable through the
        // oracle (the C-API strips both time columns) and self-healing.
        let mut errors = Vec::new();
        self.recalc(&mut errors, false);
        for e in errors {
            self.med.cd.obj.push_error(e);
        }
    }

    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(o) = other.as_any().downcast_ref::<Monitor>() else {
            return;
        };
        self.med.cd.make_like_base(&o.med.cd);
        self.med.cd.nphases = o.med.cd.nphases;
        self.med.cd.set_nconds(o.med.cd.nconds);
        self.med.metered_element = o.med.metered_element;
        self.med.metered_terminal = o.med.metered_terminal;
        self.med.metered_snap = o.med.metered_snap.clone();
        self.element_full_name = o.element_full_name.clone();
        self.mode = o.mode;
        self.include_residual = o.include_residual;
        self.vi_polar = o.vi_polar;
        self.pp_polar = o.pp_polar;
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

/// Capture the parse-relevant shape + header dimensions of the metered element.
fn capture_metered(full_name: String, obj: &dyn DssObject) -> MeteredSnapshot {
    let elem = obj
        .as_ckt_element()
        .expect("element= resolves to a ckt elem");
    let cd = elem.cd();
    let num_variables = elem.num_variables();
    let (kind, num_windings, num_steps) =
        if let Some(t) = obj.as_any().downcast_ref::<Transformer>() {
            (
                MeteredKind::Transformer,
                t.num_windings().max(0) as usize,
                0,
            )
        } else if let Some(c) = obj.as_any().downcast_ref::<Capacitor>() {
            (MeteredKind::Capacitor, 0, c.states().len())
        } else if obj
            .as_any()
            .downcast_ref::<crate::elements::pc::load::Load>()
            .is_some()
            || obj
                .as_any()
                .downcast_ref::<crate::elements::pc::generator::Generator>()
                .is_some()
            || obj
                .as_any()
                .downcast_ref::<crate::elements::pc::pvsystem::PVSystem>()
                .is_some()
            || obj
                .as_any()
                .downcast_ref::<crate::elements::pc::storage::Storage>()
                .is_some()
            || obj
                .as_any()
                .downcast_ref::<crate::elements::pc::ind_mach012::IndMach012>()
                .is_some()
            || obj
                .as_any()
                .downcast_ref::<crate::elements::pc::vccs::Vccs>()
                .is_some()
            || obj
                .as_any()
                .downcast_ref::<crate::elements::pc::upfc::Upfc>()
                .is_some()
        {
            (MeteredKind::PcElement, 0, 0)
        } else {
            (MeteredKind::Other, 0, 0)
        };
    MeteredSnapshot {
        full_name,
        kind,
        nphases: cd.nphases,
        nconds: cd.nconds,
        nterms: cd.nterms,
        yorder: cd.yorder,
        buses: (1..=cd.nterms).map(|i| cd.get_bus(i).to_string()).collect(),
        num_windings,
        num_steps,
        num_variables,
        // Pascal `VariableName(i)` for i := 1..NumVariables (1-based).
        variable_names: (1..=num_variables).map(|i| elem.variable_name(i)).collect(),
    }
}

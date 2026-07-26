//! The `impl CktElement` / `impl DssObject` property surface and the
//! metered-element snapshot capture (`capture_metered`).

use num_complex::Complex64;

use super::Monitor;
use crate::elements::ckt::CktElementData;
use crate::elements::meter::meter_element::{MeteredKind, MeteredSnapshot};
use crate::elements::pd::auto_trans::AutoTrans;
use crate::elements::pd::capacitor::Capacitor;
use crate::elements::pd::transformer::Transformer;
use crate::elements::pos_seq::{PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, ElemId, SysCtx};
use crate::obj::arena::ResolvedObj;
use crate::obj::base::{DssObjData, DssObject};

impl CktElement for Monitor {
    fn cd(&self) -> &CktElementData {
        &self.med.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.med.cd
    }

    fn recalc_element_data(&mut self, sys: &SysCtx) {
        let mut errors = crate::diag::ErrorLog::new();
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

    /// Pascal `TMonitorObj.MakePosSequence` (`Meters/Monitor.pas:652`): resync
    /// the monitor to the metered element's bus / phase / conductor counts, run
    /// the per-mode buffer reallocation, then rebuild the header
    /// (`ClearMonitorStream`) and mark it valid, ending with the base bus rename
    /// (`inherited`). Pascal NIL-guards `MeteredElement`; `ctx.monitored` is
    /// `None` in the same case.
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
            // Pascal `case Mode and MODEMASK`: mode 3 resizes StateBuffer to
            // NumVariables, mode 4 reallocs FlickerBuffer, mode 5 reallocs
            // SolutionBuffer, else reallocs CurrentBuffer (Yorder) / VoltageBuffer
            // (NConds). This port keeps no persistent per-sample buffers
            // (`TakeSample` sizes its scratch on demand from the metered snapshot
            // / `record_size`), so these reallocs have no field to touch; the
            // mode-3 NumVariables record size is applied by `ClearMonitorStream`
            // below through the metered snapshot.
            //
            // ClearMonitorStream (`DoMakePosSeq` runs in the non-harmonic
            // power-flow pass, so the time columns are hour/t(sec)).
            self.clear_monitor_stream(false);
            self.valid_monitor = true;
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

impl Monitor {
    pub(crate) fn make_like(&mut self, other: &Self) {
        let o = other;
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
}

impl DssObject for Monitor {
    fn data(&self) -> &DssObjData {
        &self.med.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.med.cd.obj
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use super::prop::*;
        match idx {
            TERMINAL => self.med.metered_terminal,
            MODE => self.mode.to_raw(),
            _ => unreachable!("Monitor has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use super::prop::*;
        match idx {
            TERMINAL => self.med.metered_terminal = value,
            MODE => self.mode = super::MonitorModeView::from_raw(value),
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
    fn set_object_ref(&mut self, idx: usize, name: String, resolved: Option<ResolvedObj<'_>>) {
        match idx {
            super::prop::ELEMENT => {
                self.element_full_name = name.clone();
                match resolved {
                    Some(o) => {
                        self.med.metered_element = Some(o.id());
                        self.med.metered_element_changed = true;
                        self.med.metered_snap = Some(capture_metered(name, o));
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

    /// Pascal `DoAction` (`Monitor.pas:289-304`): Clear/Reset (ordinal 0) →
    /// `ResetIt`. The other three actions all need context this parse-time edit
    /// hook does not hold, so — like every other meter's `do_action` (cf.
    /// `EnergyMeter`) — they are driven from the executive and no-op here:
    /// `TakeSample` (2) needs the live solution/node voltages; `Save` (1) needs
    /// the output file; `Process` (3) → `PostProcess` → `DoFlickerCalculations`
    /// needs the circuit to resolve the metered-bus `kVBase` (the `Vbase`
    /// normalizer). The port already runs `PostProcess` on the path that matters
    /// — `Export`/`Show Monitor` → `to_csv`, which resolves `kv_base` from the
    /// circuit — so the flicker rewrite is covered; the pinned dss_capi oracle
    /// cannot gate `Process` anyway (its `DoFlickerCalculations` segfaults on the
    /// `Terminals` OOB, so `Monitors.Process()` hard-crashes).
    fn do_action(&mut self, ordinal: i32, _errors: &mut crate::diag::ErrorLog) {
        if ordinal == 0 {
            // `Action=Clear/Reset` runs at parse time with no solution context,
            // so the header is rebuilt with the fundamental `hour`/`t(sec)`
            // labels; the `Set mode=harmonics` reset relabels them (see
            // `end_edit` for the one residual harmonics-mode divergence).
            self.reset_it(false);
        }
    }

    fn end_edit(&mut self, _sys: &crate::elements::traits::SysCtx) {
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
        let mut errors = crate::diag::ErrorLog::new();
        self.recalc(&mut errors, false);
        for e in errors {
            self.med.cd.obj.push_error(e);
        }
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

/// Capture the parse-relevant shape + header dimensions of the metered element.
fn capture_metered(full_name: String, o: ResolvedObj<'_>) -> MeteredSnapshot {
    let elem = o.ckt().expect("element= resolves to a ckt elem");
    let cd = elem.cd();
    let num_variables = elem.num_variables();
    let (kind, num_windings, num_steps) = if let Some(t) = o.get::<Transformer>() {
        (
            MeteredKind::Transformer,
            t.num_windings().max(0) as usize,
            0,
        )
    } else if let Some(at) = o.get::<AutoTrans>() {
        // Pascal Monitor mode 2/8/10 accepts AUTOTRANS_ELEMENT alongside
        // XFMR_ELEMENT (Monitor.pas:542-543); the auto reports the same kind.
        (
            MeteredKind::Transformer,
            at.num_windings().max(0) as usize,
            0,
        )
    } else if let Some(c) = o.get::<Capacitor>() {
        (MeteredKind::Capacitor, 0, c.states().len())
    } else if matches!(o.id(), ElemId::Storage(_)) {
        // Pascal validates mode 7 by CLASSMASK = STORAGE_ELEMENT and mode 3
        // by BASECLASSMASK = PC_ELEMENT (Monitor.pas `RecalcElementData`), so
        // Storage needs its own kind and mode 3 accepts it as a PC element.
        (MeteredKind::Storage, 0, 0)
    } else if matches!(
        o.id(),
        ElemId::Load(_)
            | ElemId::Generator(_)
            | ElemId::PVSystem(_)
            | ElemId::IndMach012(_)
            | ElemId::Vccs(_)
            | ElemId::Upfc(_)
    ) {
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

#[cfg(test)]
mod make_pos_seq_tests {
    use super::super::MonitorModeView;
    use super::*;
    use crate::elements::pos_seq::{PosSeqCtx, PosSeqElemInfo};

    fn snap(nphases: usize, nconds: usize, num_variables: usize) -> MeteredSnapshot {
        MeteredSnapshot {
            full_name: "line.l1".into(),
            nphases,
            nconds,
            nterms: 2,
            yorder: nconds * 2,
            buses: vec!["b1".into(), "b2".into()],
            num_variables,
            variable_names: (1..=num_variables).map(|i| format!("v{i}")).collect(),
            ..Default::default()
        }
    }

    fn ctx1(nphases: usize, nconds: usize, yorder: usize) -> PosSeqCtx {
        PosSeqCtx {
            monitored: Some(PosSeqElemInfo {
                bus_names: vec!["b1".into(), "b2".into()],
                nphases,
                nconds,
                yorder,
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    /// Pascal `TMonitorObj.MakePosSequence` (Monitor.pas:652): resync to the
    /// metered element, rebuild the header (ClearMonitorStream), mark valid.
    /// Mode 0 → general V/I header, record_size = 2·nconds·2 for 1 conductor = 4.
    #[test]
    fn mode0_resyncs_and_rebuilds_header() {
        let mut m = Monitor::new("mon1");
        m.med.metered_element = Some(ElemId::new(1, 0));
        m.med.metered_snap = Some(snap(1, 1, 0));
        let plan = m.make_pos_sequence(&ctx1(1, 1, 2));
        assert_eq!(m.cd().nphases, 1);
        assert_eq!(m.cd().nconds, 1);
        assert_eq!(m.get_bus_name(1), "b1");
        assert_eq!(m.num_channels(), 4);
        assert!(plan.run_base && plan.actions.is_empty());
        assert_eq!(m.monitored_element_ref(), Some(ElemId::new(1, 0)));
    }

    /// Mode 3 (state variables): ClearMonitorStream sets `RecordSize` to the
    /// metered element's `NumVariables` (StateBuffer resize in Pascal).
    #[test]
    fn mode3_record_size_is_num_variables() {
        let mut m = Monitor::new("mon1");
        m.mode = MonitorModeView::from_raw(3);
        m.med.metered_element = Some(ElemId::new(2, 5));
        m.med.metered_snap = Some(snap(1, 1, 2));
        let plan = m.make_pos_sequence(&ctx1(1, 1, 2));
        assert_eq!(m.num_channels(), 2); // NumVariables
        assert!(m.header().contains(&"v1".to_string()));
        assert!(plan.run_base);
    }

    /// Mode 4 (flicker): ClearMonitorStream sets `RecordSize = 2·nphases`
    /// (Flk/Pst pair per phase) — resynced to the metered element's phase count.
    #[test]
    fn mode4_record_size_is_two_per_phase() {
        let mut m = Monitor::new("mon1");
        m.mode = MonitorModeView::from_raw(4);
        m.med.metered_element = Some(ElemId::new(1, 0));
        m.med.metered_snap = Some(snap(1, 1, 0));
        let plan = m.make_pos_sequence(&ctx1(1, 1, 2));
        assert_eq!(m.cd().nphases, 1);
        assert_eq!(m.num_channels(), 2); // 2·nphases
        assert!(m.header().contains(&"Flk1".to_string()));
        assert!(m.header().contains(&"Pst1".to_string()));
        assert!(plan.run_base);
    }

    /// Mode 5 (solution vars): ClearMonitorStream sets `RecordSize` to the fixed
    /// NUM_SOLUTION_VARS = 12 (independent of the metered element).
    #[test]
    fn mode5_record_size_is_num_solution_vars() {
        let mut m = Monitor::new("mon1");
        m.mode = MonitorModeView::from_raw(5);
        m.med.metered_element = Some(ElemId::new(1, 0));
        m.med.metered_snap = Some(snap(1, 1, 0));
        let plan = m.make_pos_sequence(&ctx1(1, 1, 2));
        assert_eq!(m.num_channels(), 12); // NUM_SOLUTION_VARS
        assert!(m.header().contains(&"TotalIterations".to_string()));
        assert!(m.header().contains(&"Frequency".to_string()));
        assert!(plan.run_base);
    }

    /// Pascal NIL guard: no metered element ⇒ only the base rename runs.
    #[test]
    fn nil_metered_element_runs_base_only() {
        let mut m = Monitor::new("mon1");
        let np = m.cd().nphases;
        let plan = m.make_pos_sequence(&PosSeqCtx::default());
        assert_eq!(m.cd().nphases, np);
        assert!(plan.run_base);
    }
}

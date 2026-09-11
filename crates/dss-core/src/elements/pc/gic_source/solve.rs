//! `RecalcElementData` (the Line splice), `CalcYPrim` (fixed series G),
//! `GetVterminalForSource` / injection, and the `impl CktElement` for
//! [`GicSource`].

use num_complex::Complex64;

use super::GicSource;
use crate::elements::ckt::CktElementData;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::pd::line::Line;
use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, InjComputeCtx, SysCtx};
use crate::obj::arena::ArenaClass;
use crate::obj::base::RefAction;
use crate::support::cmatrix::CMatrix;
use crate::support::complexutil::pdeg_to_complex;
use crate::util::EPSILON2;

/// Pascal `CompareTextShortest('GIC_', LineBus2) = 0`: the (case-insensitive)
/// leading-substring compare over the shorter of the two strings — true iff the
/// Line's Bus2 already begins with the `GIC_` bus prefix (splice already done).
fn line_bus2_is_gic(bus2: &str) -> bool {
    let n = 4.min(bus2.len());
    bus2[..n].eq_ignore_ascii_case(&"GIC_"[..n])
}

impl GicSource {
    /// Pascal `TGICSourceObj.RecalcElementData` (GICsource.pas:326): splice a
    /// `GIC_<name>` bus in front of the associated Line, then (unless specified)
    /// compute the induced `Volts`.
    pub(super) fn recalc(&mut self) {
        if self.line_ref.is_none() {
            // Pascal: `if pLineElem = NIL then ... DoSimpleMsg 333` (the executive
            // already tried to resolve the Line through the foreign view).
            if self.line_missing {
                self.cd.obj.push_error(format!(
                    "Line Object {} associated with GICsource.{} not found. \
                     Make sure you define it first.",
                    self.cd.obj.name(),
                    self.cd.obj.name()
                ));
            }
        } else {
            let line_bus2 = self.line_bus2.clone();
            // If LineBus2 already begins with GIC, don't insert the GIC bus.
            if !line_bus2_is_gic(&line_bus2) {
                // Define buses — inserting a new bus GIC_{Name}.
                let gic_bus = format!("gic_{}", self.cd.obj.name());
                self.cd.set_bus(1, &gic_bus);
                self.cd.set_bus(2, &line_bus2);
                // Redefine the Bus2 spec for the Line (through the property path;
                // its Bus2 side effect is a plain rename).
                if let Some(target) = self.line_ref {
                    self.pending_actions.push(RefAction::SetElementBus {
                        // The deferred action speaks the class-erased handle;
                        // widen the typed `Idx<Line>` back through the Line
                        // class's own `ArenaClass::id` (same class, same index).
                        target: Line::id(target.get()),
                        terminal: 2,
                        bus: gic_bus,
                    });
                }
            }
            self.bus2_defined = true;
            if !self.volts_specified {
                self.volts = self.compute_vline();
            }
        }
        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
    }

    /// Pascal `TGICSourceObj.GetVterminalForSource` (GICsource.pas:403): a
    /// zero-sequence source — every phase gets `pdegtocomplex(Vmag, Angle)`; the
    /// magnitude is `Volts` only when the solution frequency matches
    /// `SrcFrequency` (else the source is shorted).
    fn get_vterminal_for_source(&mut self, sys: &SysCtx) {
        let nphases = self.cd.nphases;
        // If the solution frequency isn't the source frequency, source shorted.
        let vmag = if (sys.frequency - self.src_frequency).abs() < EPSILON2 {
            self.volts
        } else {
            0.0
        };
        for i in 0..nphases {
            self.cd.vterminal[i] = pdeg_to_complex(vmag, self.angle); // all the same (zero seq)
            self.cd.vterminal[i + nphases] = Complex64::ZERO;
        }
    }

    /// Pascal `TGICSourceObj.GetInjCurrents` (GICsource.pas:457): fill
    /// `self.cd.inj_current` from `[Yprim]·[Vsource; 0]` (the solve path).
    fn get_inj_currents(&mut self, sys: &SysCtx) {
        self.cd.inj_current = self.compute_inj_currents(sys);
    }

    /// Pascal `GetInjCurrents`, **returning** the injection; leaves
    /// `self.cd.inj_current` untouched (the reporting `GetCurrents` uses the
    /// `ComplexBuffer` scratch).
    fn compute_inj_currents(&mut self, sys: &SysCtx) -> Vec<Complex64> {
        self.get_vterminal_for_source(sys);
        let mut inj = vec![Complex64::ZERO; self.cd.yorder];
        if let Some(yprim) = &self.cd.yprim {
            yprim.mv_mult(&mut inj, &self.cd.vterminal);
        }
        self.cd.iterminal_updated = false;
        inj
    }
}

impl CktElement for GicSource {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    /// Pascal `TGICSourceObj.MakePosSequence` (GICsource.pas:471-476): a
    /// multi-phase GICsource collapses to `Phases := 1` (a bare single edit),
    /// then `inherited` (the base bus rename).
    fn make_pos_sequence(&mut self, _ctx: &PosSeqCtx) -> PosSeqPlan {
        if self.cd.nphases > 1 {
            PosSeqPlan::with_actions(vec![PosSeqAction::SetI32(super::prop::PHASES, 1)])
        } else {
            PosSeqPlan::base()
        }
    }

    /// Pascal `TGICSourceObj.CalcYPrim` (GICsource.pas:361): a fixed 10000-mho
    /// (0.0001 Ω) series conductance block — the source's own tiny impedance.
    fn calc_yprim(&mut self, _sys: &SysCtx) {
        let nphases = self.cd.nphases;
        let yorder = self.cd.yorder;

        let value = Complex64::new(10000.0, 0.0); // Assume 0.0001 ohms resistance
        let neg = -value;
        let mut yp_series = CMatrix::new(yorder);
        for i in 0..nphases {
            let j = i + nphases;
            yp_series.set(i, i, value);
            yp_series.set(j, j, value);
            yp_series.set(i, j, neg);
            yp_series.set(j, i, neg);
        }

        let mut yprim = CMatrix::new(yorder);
        yprim.copy_from(&yp_series);
        self.cd.yprim_series = Some(yp_series);
        self.cd.yprim_shunt = None;
        self.cd.yprim = Some(yprim);

        // Account for open conductors.
        self.cd.apply_yprim_open_conductor_calcs();
        self.cd.yprim_invalid = false;
    }

    /// Pascal `TGICSourceObj.InjCurrents` + `TPCElement.InjCurrents` (M3b compute
    /// half; the caller scatters `cd.inj_current`).
    fn compute_inj_currents(
        &mut self,
        sys: &SysCtx,
        _node_v: &[Complex64],
        _ctx: &mut InjComputeCtx,
    ) -> bool {
        self.get_inj_currents(sys);
        false
    }

    fn harmonic_spectrum(&self) -> Option<&SpectrumObj> {
        self.spectrum_obj.as_ref()
    }
    fn harmonic_spectrum_name(&self) -> Option<&str> {
        Some(&self.spectrum)
    }
    fn set_harmonic_spectrum(&mut self, spectrum: Option<SpectrumObj>) {
        self.spectrum_obj = spectrum;
    }

    /// Pascal `GetSourceFrequency` (GICsource branch): the source's `SrcFrequency`.
    fn source_frequency(&self) -> Option<f64> {
        Some(self.src_frequency)
    }

    /// Pascal `TGICSourceObj.GetCurrents` (GICsource.pas:435): `Yprim·V(node)`
    /// minus a freshly recomputed injection (into a local scratch).
    #[allow(clippy::needless_range_loop)] // loop-for-loop Pascal port
    fn get_currents(&mut self, sys: &SysCtx, node_v: &[Complex64], curr: &mut [Complex64]) {
        // No node references yet: an element declared after the last
        // `SetNodeRef` sweep has an empty `node_ref` (the port's model of
        // r4133's `NodeRef := nil`, `Common/CktElement.pas:186`, allocated only
        // by `SetNodeRef` `:547-558`, which `ReProcessBusDefs` re-runs for
        // enabled elements at Y build). r4133 indexes the nil pointer right
        // below and lets the access violation surface out of this procedure's
        // own `TRY ... EXCEPT` as DSS error 335 -- `TGICSourceObj.GetCurrents`,
        // `PCElements/GICsource.pas:513-546`; the port answers
        // with the zero vector the base trait's `get_currents` default returns
        // for an unmapped element. The `Enabled` test the other PC classes
        // inherit from `TPCElement.GetCurrents` is deliberately NOT added here:
        // this override carries none, so a disabled element goes on reporting
        // `YPrim*Vterminal - Iinj` from its last mapping, as upstream does.
        if self.cd.node_ref.is_empty() {
            curr.fill(Complex64::ZERO);
            return;
        }
        let yorder = self.cd.yorder;
        for i in 0..yorder {
            self.cd.vterminal[i] = node_v[self.cd.node_ref[i]];
        }
        if let Some(yprim) = &self.cd.yprim {
            yprim.mv_mult(curr, &self.cd.vterminal);
        }
        let inj = self.compute_inj_currents(sys);
        for i in 0..yorder {
            curr[i] -= inj[i];
        }
    }
}

#[cfg(test)]
mod pos_seq_tests {
    use super::*;
    use crate::elements::pc::gic_source::prop;
    use crate::elements::pos_seq::PosSeqCtx;

    /// GICsource, multi-phase (default 3) → bare `Phases := 1` edit + run_base
    /// (Pascal `TGICSourceObj.MakePosSequence`, GICsource.pas:471-476).
    #[test]
    fn makeposseq_gicsource_multiphase_sets_phases_1() {
        let mut g = GicSource::new("g");
        assert!(g.cd.nphases > 1);
        let plan = g.make_pos_sequence(&PosSeqCtx::default());
        assert!(plan.run_base);
        assert_eq!(plan.actions, vec![PosSeqAction::SetI32(prop::PHASES, 1)]);
    }

    /// GICsource, already single phase → base-only (no actions), still run_base.
    #[test]
    fn makeposseq_gicsource_single_phase_is_base_only() {
        let mut g = GicSource::new("g");
        g.cd.nphases = 1;
        let plan = g.make_pos_sequence(&PosSeqCtx::default());
        assert!(plan.run_base);
        assert!(plan.actions.is_empty());
    }
}

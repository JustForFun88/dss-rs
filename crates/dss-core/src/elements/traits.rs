//! The circuit-element behavior trait — Pascal `TDSSCktElement`'s virtual
//! methods (`CalcYPrim`, `InjCurrents`, `GetCurrents`, `RecalcElementData`)
//! plus the context structs that replace "reach through `ActiveCircuit`"
//! global access (PORTING_PLAN.md §2.1).

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::solution::SolveMode;

/// Reference to a circuit element inside the executive's class registry:
/// `(class index, object index)`. The Pascal pointer lists (`CktElements`,
/// `Sources`, `Lines`, `Loads`, ...) become `Vec<ElemRef>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElemRef {
    pub cls: usize,
    pub idx: usize,
}

/// Element storage the solver walks — implemented by the executive's class
/// registry. Replaces Pascal's `TDSSPointerList` of `TDSSCktElement`.
pub trait ElemStore {
    fn ckt_elem(&self, r: ElemRef) -> &dyn CktElement;
    fn ckt_elem_mut(&mut self, r: ElemRef) -> &mut dyn CktElement;
}

/// Scalar state the elements read from the circuit/solution during
/// `CalcYPrim`/`InjCurrents` — the Pascal `ActiveCircuit.Solution.X`
/// accesses, snapshotted into one struct of plain values.
#[derive(Debug, Clone)]
pub struct SysCtx {
    /// `Solution.Frequency`.
    pub frequency: f64,
    /// `Circuit.Fundamental`.
    pub fundamental: f64,
    pub is_harmonic_model: bool,
    pub is_dynamic_model: bool,
    /// `Solution.LoadModel`: POWERFLOW (1) or ADMITTANCE (2).
    pub load_model: i32,
    pub mode: SolveMode,
    /// `Circuit.LoadMultiplier`.
    pub load_multiplier: f64,
    /// `Circuit.DefaultGrowthFactor`.
    pub default_growth_factor: f64,
    /// `Solution.Year`.
    pub year: i32,
    /// `Solution.DynaVars.dblHour`.
    pub dbl_hour: f64,
    pub solution_count: i32,
    pub loads_need_updating: bool,
    pub neglect_load_y: bool,
    pub long_line_correction: bool,
    pub positive_sequence: bool,
}

/// Mutable solve-state view for current injection: the node voltage vector
/// and the system injection-current accumulator (`Solution.NodeV` /
/// `Solution.Currents`, both with the slot-0 ground convention).
pub struct InjCtx<'a> {
    pub node_v: &'a [Complex64],
    pub currents: &'a mut [Complex64],
}

/// Pascal `TDSSCktElement` virtual surface (Phase 3 subset).
pub trait CktElement {
    fn cd(&self) -> &CktElementData;
    fn cd_mut(&mut self) -> &mut CktElementData;

    /// `RecalcElementData` (abstract in the base class).
    fn recalc_element_data(&mut self, sys: &SysCtx);

    /// `CalcYPrim` (abstract): rebuild the primitive Y matrices.
    fn calc_yprim(&mut self, sys: &SysCtx);

    /// `InjCurrents` (sources and PC elements): add this element's injection
    /// into `ctx.currents` through `NodeRef` (slot 0 absorbs ground).
    /// The base class raises error 753; PD elements never get called.
    fn inj_currents(&mut self, sys: &SysCtx, ctx: &mut InjCtx) {
        let _ = (sys, ctx);
        unreachable!(
            "Improper call to InjCurrents for Element: \"{}\"",
            self.cd().obj.name()
        );
    }

    /// `GetCurrents`: total currents into the element terminals. The default
    /// is the PD-element behavior `Iterminal = Yprim · Vterminal`; PC
    /// elements override (compensation form).
    fn get_currents(&mut self, sys: &SysCtx, node_v: &[Complex64], curr: &mut [Complex64]) {
        let _ = sys;
        let cd = self.cd_mut();
        if !cd.enabled || cd.node_ref.is_empty() {
            curr.fill(Complex64::ZERO);
            return;
        }
        cd.compute_vterminal(node_v);
        if let Some(yprim) = &cd.yprim {
            yprim.mv_mult(curr, &cd.vterminal);
        } else {
            curr.fill(Complex64::ZERO);
        }
    }

    /// `ComputeIterminal`: cache-aware terminal-current refresh.
    fn compute_iterminal(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        if self.cd().iterminal_solution_count != sys.solution_count {
            let mut curr = vec![Complex64::ZERO; self.cd().yorder];
            self.get_currents(sys, node_v, &mut curr);
            let cd = self.cd_mut();
            cd.iterminal.copy_from_slice(&curr);
            cd.iterminal_solution_count = sys.solution_count;
        }
    }

    /// `Get_Losses`: sum of `NodeV[ref] · conj(Iterminal)` over all
    /// conductors (zero refs skipped), ×3 under positive sequence.
    fn losses(&mut self, sys: &SysCtx, node_v: &[Complex64]) -> Complex64 {
        if !self.cd().enabled || self.cd().node_ref.is_empty() {
            return Complex64::ZERO;
        }
        self.compute_iterminal(sys, node_v);
        let cd = self.cd();
        let mut result = Complex64::ZERO;
        for k in 0..cd.yorder {
            let n = cd.node_ref[k];
            if n > 0 {
                result += node_v[n] * cd.iterminal[k].conj();
            }
        }
        if sys.positive_sequence {
            result *= 3.0;
        }
        result
    }
}

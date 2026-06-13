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

    /// Read view of any registered object (control dispatch peeks at a
    /// control's references before splitting the mutable borrows).
    fn obj(&self, r: ElemRef) -> &dyn crate::obj::base::DssObject;

    /// Pascal `TDSSCircuit.SetElementActive`: resolve a full element name
    /// (`Class.Name`, or a bare `Name` searched across all circuit-element
    /// classes) to its [`ElemRef`], or `None` if not found. Used by the
    /// EnergyMeter manual `ZoneList` zone build.
    fn find_ckt_element(&self, full_name: &str) -> Option<ElemRef>;

    /// Single mutable object view (for `as_any_mut` downcasts when only one
    /// element is touched, e.g. the model-3 generator DQDV sweep).
    fn obj_mut(&mut self, r: ElemRef) -> &mut dyn crate::obj::base::DssObject;

    /// Two distinct objects borrowed mutably at once — the Rust stand-in for
    /// Pascal's live cross-object pointers during `Sample`/`DoPendingAction`
    /// (PHASE5_PLAN §2.1: the control plus its controlled element). Panics if
    /// `a == b`.
    fn pair_mut(
        &mut self,
        a: ElemRef,
        b: ElemRef,
    ) -> (
        &mut dyn crate::obj::base::DssObject,
        &mut dyn crate::obj::base::DssObject,
    );

    /// Three pairwise-distinct objects borrowed mutably at once (CapControl:
    /// control + capacitor + monitored element). Panics on any aliasing.
    fn triple_mut(
        &mut self,
        a: ElemRef,
        b: ElemRef,
        c: ElemRef,
    ) -> (
        &mut dyn crate::obj::base::DssObject,
        &mut dyn crate::obj::base::DssObject,
        &mut dyn crate::obj::base::DssObject,
    );
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
    /// `Circuit.GenMultiplier`.
    pub gen_multiplier: f64,
    /// `Circuit.GeneratorDispatchReference` (set per solve by
    /// `SetGeneratorDispRef`).
    pub generator_dispatch_reference: f64,
    /// `Circuit.PriceSignal` ($/MWh).
    pub price_signal: f64,
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

    /// Pascal `TPDElement.IsShunt`: true for shunt-connected capacitors and
    /// reactors (`Circuit.Get_Losses` ignores shunt PD elements). The base
    /// class default is false.
    fn is_shunt(&self) -> bool {
        false
    }

    /// Pascal `TDSSCktElement.GetTermVoltages(iTerm, VBuffer)`: the node voltages
    /// at terminal `iterm` (1-based) into `vbuffer` (0-based, length ≥ nconds);
    /// zeros if the terminal number is out of range. Used by the controls to
    /// sense a monitored element's terminal voltages.
    fn get_term_voltages(&self, iterm: usize, node_v: &[Complex64], vbuffer: &mut [Complex64]) {
        let cd = self.cd();
        let ncond = cd.nconds;
        if iterm < 1 || iterm > cd.nterms || cd.node_ref.is_empty() {
            for v in vbuffer.iter_mut().take(ncond) {
                *v = Complex64::ZERO;
            }
            return;
        }
        let k = (iterm - 1) * ncond;
        for i in 0..ncond {
            vbuffer[i] = node_v[cd.node_ref[k + i]];
        }
    }

    /// Pascal `TDSSCktElement.Get_Power(idxTerm)`: total complex power (W, var)
    /// into terminal `idx_term` (1-based), summed over its conductors (zero refs
    /// skipped), ×3 under positive sequence.
    fn terminal_power(&mut self, sys: &SysCtx, node_v: &[Complex64], idx_term: usize) -> Complex64 {
        if !self.cd().enabled || self.cd().node_ref.is_empty() {
            return Complex64::ZERO;
        }
        self.compute_iterminal(sys, node_v);
        let cd = self.cd();
        let nconds = cd.nconds;
        let k = (idx_term - 1) * nconds;
        let mut result = Complex64::ZERO;
        for i in 0..nconds {
            let n = cd.node_ref[k + i];
            if n > 0 {
                result += node_v[n] * cd.iterminal[k + i].conj();
            }
        }
        if sys.positive_sequence {
            result *= 3.0;
        }
        result
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

//! The circuit-element behavior trait — Pascal `TDSSCktElement`'s virtual
//! methods (`CalcYPrim`, `InjCurrents`, `GetCurrents`, `RecalcElementData`)
//! plus the context structs that replace "reach through `ActiveCircuit`"
//! global access (PORTING_PLAN.md §2.1).

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::general::spectrum::SpectrumObj;
use crate::solution::SolveMode;
use crate::support::dynamics::IterationFlag;

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
    /// `Solution.TimeOfDay()` (no epsilon) — wrapped hour-of-day (Storage
    /// `CheckStateTriggerLevel` charge-time trigger).
    pub time_of_day: f64,
    /// `Solution.DynaVars.h` — the dynamics step size in seconds (Storage
    /// charge-time tolerance window).
    pub dyna_h: f64,
    /// `Solution.DynaVars.IterationFlag` — the predictor (`NewTimeStep`) /
    /// corrector (`SameTimeStep`) selector consumed by `IntegrateStates`.
    pub iteration_flag: IterationFlag,
}

/// Mutable solve-state view for current injection: the node voltage vector
/// and the system injection-current accumulator (`Solution.NodeV` /
/// `Solution.Currents`, both with the slot-0 ground convention).
pub struct InjCtx<'a> {
    pub node_v: &'a [Complex64],
    pub currents: &'a mut [Complex64],
    /// `Solution.SystemYChanged`. A PC element that re-derives its nominal here
    /// (Storage/PVSystem when `LoadsNeedUpdating`) can invalidate its own YPrim;
    /// Pascal's `CktElement.set_YprimInvalid` raises `SystemYChanged` as a side
    /// effect (CktElement.pas l.245), so the snapshot loop rebuilds Y right after
    /// `GetPCInjCurr` (Solution.pas l.895). Reproduce that side effect by letting
    /// the element raise this flag.
    pub system_y_changed: &'a mut bool,
}

/// Per-element reliability inputs returned by [`CktElement::reliability_data`]
/// for the EnergyMeter reliability sweep (Pascal `TPDElement` fields).
#[derive(Debug, Clone, Copy, Default)]
pub struct ReliabilityData {
    /// `BranchFltRate` = `CalcFltRate` result (faults/yr for this branch).
    pub branch_flt_rate: f64,
    /// `HrsToRepair`: average repair time (hours).
    pub hrs_to_repair: f64,
    /// `MilesThisLine`: branch length in miles (0 for non-line PD elements).
    pub miles_this_line: f64,
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

    /// Pascal `TPCElement.InitHarmonics`: capture the per-element harmonic base
    /// values (the fundamental-frequency reference magnitude/angle the spectrum
    /// is applied to) from the present fundamental solution. Run once over every
    /// enabled PC element when entering harmonics mode (`InitializeForHarmonics`).
    /// Default no-op — most elements (and the sources, whose harmonic injection
    /// is recomputed each step) carry no harmonic state.
    fn init_harmonics(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        let _ = (sys, node_v);
    }

    /// Pascal `TPCElement.InitStateVars` (`PCElement.pas` l.174): seed this
    /// machine's dynamic state variables from the present (power-flow) operating
    /// point. Run once over every enabled PC element when entering dynamics mode
    /// (`calcInitialMachineStates`, the `OK_for_Dynamics` success path). Default
    /// no-op — the base `TPCElement` and the elements without dynamic state
    /// (loads, sources) carry nothing to initialise.
    fn init_state_vars(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        let _ = (sys, node_v);
    }

    /// Pascal `TPCElement.IntegrateStates` (`PCElement.pas` l.179): advance this
    /// machine's dynamic states by one predictor or corrector half-step (the
    /// `Solution.iteration_flag` predictor/corrector selector is surfaced to
    /// `SysCtx` once a machine consumes it, WP7.7 step 2). Run over every PC
    /// element twice per dynamics time step (`IntegratePCStates`). Default no-op
    /// — only machines with dynamic state respond.
    fn integrate_states(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        let _ = (sys, node_v);
    }

    /// Pascal `TPCElement.NumVariables` (`PCElement.pas`): the number of dynamic
    /// state variables this element exposes (Monitor mode 3 reads them). Default
    /// 0 — non-machine elements carry no state variables.
    fn num_variables(&self) -> usize {
        0
    }

    /// Pascal `TPCElement.VariableName(i)` (`PCElement.pas`): the name of state
    /// variable `i` (1-based). Default empty — only machines name their states.
    fn variable_name(&self, i: usize) -> String {
        let _ = i;
        String::new()
    }

    /// Pascal `TPCElement.GetAllVariables(States)` (`PCElement.pas`): fill `states`
    /// (length ≥ [`Self::num_variables`]) with the present value of every dynamic
    /// state variable. Default no-op — non-machine elements write nothing. Run by
    /// Monitor mode 3 each dynamics sample.
    fn get_all_variables(&mut self, sys: &SysCtx, node_v: &[Complex64], states: &mut [f64]) {
        let _ = (sys, node_v, states);
    }

    /// Pascal `SpectrumObj`: the harmonic spectrum this element injects from, if
    /// one is resolved. Read by the harmonic frequency sweep
    /// (`CollectAllFrequencies`). Default None.
    fn harmonic_spectrum(&self) -> Option<&SpectrumObj> {
        None
    }

    /// The `spectrum=` name this element resolves its harmonic spectrum from
    /// (default or explicit), or None if it has no spectrum. The executive
    /// resolves it at edit-completion (Pascal `Set_Spectrum` / the constructor
    /// default) and hands the clone back through [`Self::set_harmonic_spectrum`].
    fn harmonic_spectrum_name(&self) -> Option<&str> {
        None
    }

    /// Store the resolved harmonic spectrum snapshot (Pascal `SpectrumObj`).
    fn set_harmonic_spectrum(&mut self, spectrum: Option<SpectrumObj>) {
        let _ = spectrum;
    }

    /// Pascal `GetSourceFrequency` (Vsource/Isource): the source's own base
    /// frequency, used by `CollectAllFrequencies` for the source pass. Non-source
    /// elements return None (the sweep uses the system fundamental for them).
    fn source_frequency(&self) -> Option<f64> {
        None
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

    /// Per-element reliability inputs for the EnergyMeter reliability sweep
    /// (Pascal `TPDElement.CalcFltRate` + the `HrsToRepair`/`MilesThisLine`
    /// fields). `CalcFltRate` is virtual: the base `TPDElement` formula is
    /// `FaultRate · pctperm · 0.01`, which `TLineObj` overrides by multiplying
    /// by `Len`. Non-PD elements return the zero default and never appear in a
    /// meter `SequenceList`.
    fn reliability_data(&self) -> ReliabilityData {
        ReliabilityData::default()
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

    /// `NormAmps` rating (PD elements override; 0 = no rating, like Pascal's
    /// base where `Get_ExcesskVANorm` short-circuits to 0).
    fn norm_amps(&self) -> f64 {
        0.0
    }

    /// `EmergAmps` rating (PD elements override).
    fn emerg_amps(&self) -> f64 {
        0.0
    }

    /// Pascal `TDSSCktElement.MaxTerminalOneIMag` (CktElement.pas l.552): the
    /// max phase-current magnitude on terminal 1. Forces `Iterminal`.
    fn max_terminal_one_imag(&mut self, sys: &SysCtx, node_v: &[Complex64]) -> f64 {
        if !self.cd().enabled || self.cd().node_ref.is_empty() {
            return 0.0;
        }
        self.compute_iterminal(sys, node_v);
        let cd = self.cd();
        let mut max_sq = 0.0_f64;
        for i in 0..cd.nphases {
            let c = cd.iterminal[i];
            max_sq = max_sq.max(c.re * c.re + c.im * c.im);
        }
        max_sq.sqrt()
    }

    /// Pascal `TPDElement.Get_ExcessKVANorm` (PDElement.pas l.230): excess kVA
    /// over the normal rating into `idx_term` (1-based), in kVA. Side effect:
    /// sets `overload_een` to the per-unit overload factor.
    fn excess_kva_norm(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        idx_term: usize,
    ) -> Complex64 {
        let norm_amps = self.norm_amps();
        if norm_amps == 0.0 || !self.cd().enabled {
            self.cd_mut().overload_een = 0.0;
            return Complex64::ZERO;
        }
        let kva = self.terminal_power(sys, node_v, idx_term) * 0.001; // forces Iterminal
        let imax = self.max_terminal_one_imag(sys, node_v);
        let factor = imax / norm_amps - 1.0;
        if factor > 0.0 {
            self.cd_mut().overload_een = factor;
            kva * (1.0 - 1.0 / (factor + 1.0))
        } else {
            self.cd_mut().overload_een = 0.0;
            Complex64::ZERO
        }
    }

    /// Pascal `TPDElement.Get_ExcessKVAEmerg` (PDElement.pas l.257). Side
    /// effect: sets `overload_ue`.
    fn excess_kva_emerg(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        idx_term: usize,
    ) -> Complex64 {
        let emerg_amps = self.emerg_amps();
        if emerg_amps == 0.0 || !self.cd().enabled {
            self.cd_mut().overload_ue = 0.0;
            return Complex64::ZERO;
        }
        let kva = self.terminal_power(sys, node_v, idx_term) * 0.001;
        let imax = self.max_terminal_one_imag(sys, node_v);
        let factor = imax / emerg_amps - 1.0;
        if factor > 0.0 {
            self.cd_mut().overload_ue = factor;
            kva * (1.0 - 1.0 / (factor + 1.0))
        } else {
            self.cd_mut().overload_ue = 0.0;
            Complex64::ZERO
        }
    }

    /// Pascal `TDSSCktElement.GetLosses` (CktElement.pas l.441): total, load and
    /// no-load losses (W, var). Base default returns `(total, total, 0)`;
    /// Transformer/Reactor override to split the no-load component.
    fn get_losses_split(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> (Complex64, Complex64, Complex64) {
        let total = self.losses(sys, node_v);
        (total, total, Complex64::ZERO)
    }

    /// Pascal `TDSSCktElement.GetSeqLosses` (base l.1092): sequence-mode losses.
    /// Base returns zeros; Line overrides for 3-phase branches.
    fn get_seq_losses(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> (Complex64, Complex64, Complex64) {
        let _ = (sys, node_v);
        (Complex64::ZERO, Complex64::ZERO, Complex64::ZERO)
    }
}

//! The circuit-element behavior trait — Pascal `TDSSCktElement`'s virtual
//! methods (`CalcYPrim`, `InjCurrents`, `GetCurrents`, `RecalcElementData`)
//! plus the context structs that replace "reach through `ActiveCircuit`"
//! global access (PORTING_PLAN.md §2.1).

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::pos_seq::{PosSeqCtx, PosSeqPlan};
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

    /// The [`ElemKind`] of the class the ref points at — the meter/sampling
    /// type-guards (`is_line`/`is_pd_element`/PD/PC checks) match on this
    /// instead of an `as_any` downcast probe. Panics if `r` names a non-circuit
    /// ("general") class, which those guards never pass.
    ///
    /// [`ElemKind`]: crate::circuit::ElemKind
    fn kind(&self, r: ElemRef) -> crate::circuit::ElemKind;

    /// Pascal `TDSSCircuit.SetElementActive`: resolve a full element name
    /// (`Class.Name`, or a bare `Name` searched across all circuit-element
    /// classes) to its [`ElemRef`], or `None` if not found. Used by the
    /// EnergyMeter manual `ZoneList` zone build.
    fn find_ckt_element(&self, full_name: &str) -> Option<ElemRef>;

    /// Pascal `<SomeClass>.Find(name)` reaching a *non-circuit* ("general",
    /// `DSS_OBJECT`) class registered via `DssClass::dss_object` — e.g.
    /// `XYcurve`. Unlike [`ElemStore::find_ckt_element`] this is **not**
    /// restricted to circuit-element classes. Used by
    /// `StorageController.Get_DynamicTarget`'s live, uncached
    /// `DSS.XYCurveClass.Find(DSS.SeasonSignal)` (the season signal is a bare
    /// `Set`-option string, not an object-ref property, so nothing can resolve
    /// and cache the `ElemRef` up front at edit time).
    fn find_general(&self, class_name: &str, obj_name: &str) -> Option<ElemRef>;

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
    /// `Circuit.ActiveLoadShapeClass` (`Set LoadShapeClass=`): the class the
    /// GENERALTIME / DYNAMICMODE nominal dispatch consults (`USENONE`=-1 /
    /// `USEDAILY`=0 / `USEYEARLY`=1 / `USEDUTY`=2).
    pub active_load_shape_class: i32,
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
    /// `Solution.DynaVars.t` — seconds from the top of the hour (the dynamics
    /// clock the VCCS waveform integrator samples `w·t` against).
    pub dyna_t: f64,
    /// `Solution.DynaVars.IterationFlag` — the predictor (`NewTimeStep`) /
    /// corrector (`SameTimeStep`) selector consumed by `IntegrateStates`.
    pub iteration_flag: IterationFlag,
    /// `Solution.LastSolutionWasDirect` — set at the end of `SolveDirect`
    /// (`Solution.pas` l.1282), cleared at the end of `DoPFLOWsolution`
    /// (l.1022). While set, `TPCElement.GetCurrents` reports terminal currents
    /// via the `CalcYPrimContribution` shortcut ("the model is entirely in the
    /// Y matrix") instead of the load-model compensation current.
    pub last_solution_was_direct: bool,
}

impl SysCtx {
    /// Pascal `TPCElement.GetCurrents` shortcut condition (`PCElement.pas`
    /// l.137): `LastSolutionWasDirect and not (IsDynamicModel or
    /// IsHarmonicModel)` — take terminal currents from `YPrim · Vterminal`
    /// only. Applies to the PC classes that inherit the base `GetCurrents`
    /// (Load, Generator, IndMach012) and, via `inherited` in
    /// `TInvBasedPCE.GetCurrents` (`InvBasedPCE.pas` l.218), to non-GFM
    /// PVSystem/Storage; the overrides that never call `inherited` (Vsource,
    /// Isource, GICLine, GICsource, VCCS, UPFC, VSConverter, and the GFM
    /// branch of InvBasedPCE) must NOT consult it.
    pub fn pc_direct_shortcut(&self) -> bool {
        self.last_solution_was_direct && !(self.is_dynamic_model || self.is_harmonic_model)
    }
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

    /// Pascal `TDSSCktElement.SetNodeRef` (virtual): copy one terminal's node
    /// refs into the flat array + terminal record. The base behavior is the
    /// `CktElementData` method; `TAutoTransObj` overrides it to alias the series
    /// winding's second node onto the common winding's first ("Magic happens
    /// here", `AutoTrans.pas:875`). The circuit build path calls this (not
    /// `cd_mut().set_node_ref`) so the override fires.
    fn set_node_ref(&mut self, iterm: usize, node_ref_array: &[usize]) {
        self.cd_mut().set_node_ref(iterm, node_ref_array);
    }

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

    /// Pascal `TPCElement.Set_Variable(i, value)` (`PCElement.pas`): write dynamic
    /// state variable `i` (1-based). The write side of the variable interface
    /// (`num_variables`/`variable_name`/`get_all_variables`), mirroring the
    /// `TPCElement` virtual. Default no-op — only machines with settable state
    /// respond. (No external caller yet — the Rust-native variable-set API that
    /// replaces the C-API `DSSElement_Set_*` is a later phase.)
    fn set_variable(&mut self, i: usize, value: f64) {
        let _ = (i, value);
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

    /// Force a fresh `Iterminal` from the present `NodeV`, bypassing the
    /// `SolutionCount` cache — the model of the CAPI `CktElement.Currents` read
    /// path (`CAPI_CktElement.pas` `elem.GetCurrents`), which always recomputes
    /// `Yprim·Vterminal (± inj)` rather than returning the solver's internal
    /// `ComputeIterminal` cache. The two agree after every fixed-point solve
    /// (the cache is invalid at read time, so `compute_iterminal` recomputes),
    /// but `DoNewtonSolution`'s final `SumAllCurrents` stamps `Iterminal` at the
    /// converged `SolutionCount` from the *pre-final* voltage guess `NodeV_{n-1}`
    /// (the update `NodeV -= dV` follows it), so a plain `compute_iterminal`
    /// would then return that one-step-stale current. Reporting reads use this
    /// to match the oracle's fresh `GetCurrents`.
    fn refresh_iterminal(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        let mut curr = vec![Complex64::ZERO; self.cd().yorder];
        self.get_currents(sys, node_v, &mut curr);
        let cd = self.cd_mut();
        cd.iterminal.copy_from_slice(&curr);
        cd.iterminal_solution_count = sys.solution_count;
    }

    /// Pascal `TPDElement.IsShunt`: true for shunt-connected capacitors and
    /// reactors (`Circuit.Get_Losses` ignores shunt PD elements). The base
    /// class default is false.
    fn is_shunt(&self) -> bool {
        false
    }

    /// Pascal `(pElem is TInvBasedPCE) and TInvBasedPCE(pElem).GFM_Mode` — an
    /// inverter-based PC element (PVSystem/Storage) currently in grid-forming
    /// mode. The solution splits its injection pass on this flag
    /// (`GetPCInjCurr(GFMOnly)`): a GFM PCE injects with the *sources*, not with
    /// the ordinary PC elements. Default false.
    fn is_gfm(&self) -> bool {
        false
    }

    /// Pascal `TControlElem.FControlledElement` (via `Set_ControlledElement`):
    /// the circuit element this control acts on, or `None` for a non-control
    /// element (and for the fleet controls that act on a *list* of elements
    /// rather than a single one). The reverse of Pascal's
    /// `ControlledElement.ControlElementList` — the reports that need the
    /// forward `PDElement → controls` mapping (`ShowControlledElements`,
    /// `ShowTopology`) derive it by scanning `Circuit.controls` and matching this.
    /// `Circuit.controls` is in creation order, so the derived per-element list
    /// reproduces the Pascal `ControlElementList` insertion order, and a control
    /// reassigned to a different target follows its *current* target — the same
    /// final state as Pascal's remove-then-add `Set_ControlledElement`. **Known
    /// narrow limitation:** when a control's element ref is *re-edited* after a
    /// second control already registered on the same target, Pascal's remove-then-
    /// add re-appends the re-edited control to the *end* of that target's list,
    /// whereas the creation-order derive keeps the original order — so the two
    /// disagree only for ≥2 controls on one element with a post-creation
    /// element-ref edit (probe-only; no corpus deck hits it — the fully-faithful
    /// fix would materialise the whole `ControlElementList`, disproportionate here).
    /// Default `None`; every control overrides it to return
    /// `self.ccd.controlled_element`.
    fn controlled_element(&self) -> Option<ElemRef> {
        None
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

    /// `NumAmpRatings` — the number of seasonal current ratings (`Seasons`).
    /// `1` for a base PDElement / non-PD element (no seasonal ratings).
    fn num_amp_ratings(&self) -> i32 {
        1
    }

    /// `AmpRatings` — the per-season current ratings array (PD elements with
    /// `Seasons > 1` override). Empty by default.
    fn amp_ratings(&self) -> &[f64] {
        &[]
    }

    /// Pascal `TPDElement.GetRatings` (dss_capi 0.15.x `55400a29`, PDElement.pas
    /// l.330): the (norm, emerg) current ratings, overridden by the seasonal
    /// rating `AmpRatings[seasonal_idx]` when the global season index is in range
    /// (`0 <= seasonal_idx < NumAmpRatings`) — applied to ANY PDElement (0.14.5's
    /// `DI_Overloads` path restricted this to lines). `55400a29` dropped the
    /// pre-refactor/r4133 `NumAmpRatings > 1` guard, so a single-season element
    /// (`NumAmpRatings == 1`) at idx 0 also takes `AmpRatings[0]`. Both norm and
    /// emerg take the same seasonal value.
    fn get_ratings(&self, seasonal_idx: i32) -> (f64, f64) {
        let norm = self.norm_amps();
        let emerg = self.emerg_amps();
        if seasonal_idx >= 0 && seasonal_idx < self.num_amp_ratings() {
            let r = self
                .amp_ratings()
                .get(seasonal_idx as usize)
                .copied()
                .unwrap_or(norm);
            (r, r)
        } else {
            (norm, emerg)
        }
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

    /// Pascal `TDSSCktElement.MakePosSequence` (virtual): convert this element
    /// to its positive-sequence equivalent (`TExecHelper.DoMakePosSeq` calls it
    /// on every circuit element, in creation order, after setting
    /// `PositiveSequence := TRUE`). The element mutates its own direct fields
    /// here and returns the ordered property-system mutations the exec applier
    /// must replay through the typed setter helpers (see [`PosSeqPlan`]).
    ///
    /// The default is the base behavior: no property sets, run the base bus
    /// rename ([`CktElementData::make_pos_sequence_base`]) — every element
    /// without a `MakePosSequence` override in Pascal inherits exactly this.
    ///
    /// [`PosSeqPlan`]: crate::elements::pos_seq::PosSeqPlan
    /// [`CktElementData::make_pos_sequence_base`]: crate::elements::ckt::CktElementData::make_pos_sequence_base
    fn make_pos_sequence(&mut self, ctx: &PosSeqCtx) -> PosSeqPlan {
        let _ = ctx;
        PosSeqPlan::default()
    }

    /// Pascal `TLineObj` length in kilometres (`Len · <units→km>`). `None` for
    /// every non-Line element — the EnergyMeter zone walk adds it to
    /// `DistFromMeter` only for lines (R0 Category B typed read, replacing an
    /// `as_any().downcast_ref::<Line>()` guard on `store.obj`).
    fn line_length_km(&self) -> Option<f64> {
        None
    }

    /// Pascal `TLoadObj.NumCustomers`. `None` for every non-Load element — the
    /// EnergyMeter zone walk counts customers and appends to the load list only
    /// for loads (R0 Category B typed read).
    fn load_num_customers(&self) -> Option<i32> {
        None
    }

    /// Pascal transformer / autotransformer `PresentTap[iWinding]` (Monitor
    /// mode 2, the tap monitor). `None` for every non-transformer element (the
    /// mode records `0.0`). `terminal` is the 1-based winding index.
    fn present_tap(&self, terminal: usize) -> Option<f64> {
        let _ = terminal;
        None
    }

    /// Pascal `TControlElem.MonitoredElement` / `TMeterElement.MeteredElement`:
    /// the element this control/meter senses, resolved to its [`ElemRef`]. The
    /// exec applier reads it to build the [`PosSeqCtx::monitored`] snapshot
    /// before calling [`Self::make_pos_sequence`]. Default `None` — a plain
    /// circuit element monitors nothing; controls/meters override it (in the
    /// later WTs of this round).
    ///
    /// [`PosSeqCtx::monitored`]: crate::elements::pos_seq::PosSeqCtx::monitored
    fn monitored_element_ref(&self) -> Option<ElemRef> {
        None
    }
}

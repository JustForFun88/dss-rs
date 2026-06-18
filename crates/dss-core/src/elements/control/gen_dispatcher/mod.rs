//! Port of `Controls/GenDispatcher.pas` — `TGenDispatcherObj`, a control element
//! that watches one terminal of a monitored circuit element and redispatches a
//! set of generators' `kWBase`/`kvarBase` to drive the monitored power back into
//! a `kWLimit ± kWBand/2` (and `kvarLimit ± kWBand/2`) band.
//!
//! Like every `TControlElem`, a GenDispatcher builds **no Yprim** and its
//! terminal currents are zero; its single terminal attaches to the monitored
//! element's terminal bus. `DoPendingAction`/`Reset` are no-ops in Pascal (the
//! redispatch happens entirely in `Sample`, which only pushes a present-time
//! action onto the queue to force a re-solve).
//!
//! The generator list is a *dynamic* set, so unlike RegControl/CapControl (which
//! pair/triple with a single fixed target) the dispatch needs the executive to
//! reach an arbitrary number of generators. That plumbing is abstracted behind
//! [`GenDispatchEnv`]; the control-loop implements it over the class registry
//! (`solution/controls.rs`), and the unit tests against a mock.
//!
//! **Deliberately not ported** (consistent with the rest of the controls):
//! - `TGenDispatcherObj.MakePosSequence` — positive-sequence reduction isn't
//!   supported yet, and the upstream body dereferences the always-NIL
//!   `ControlledElement`, so a faithful port would only reproduce a crash.
//! - the `Element` property's Pascal `Required` flag — not enforced anywhere in
//!   the port yet (same deferral as RegControl/CapControl/Reactor).
//!
//! **One deliberate divergence from Pascal:** when a named generator fails to
//! resolve (missing or disabled), `FGenPointerList.Count < FListSize` and the
//! Pascal `Sample` loop (`for i := 1 to FListSize`) walks past the resolved
//! entries into a NIL `Gen.kWBase` dereference — an upstream crash. This port
//! iterates the resolved subset instead, so it cannot crash; for the generators
//! that *do* resolve the dispatch is numerically identical (same sequential
//! weights, same `TotalWeight` summed over the full `FListSize`). Not pinned by
//! any golden. See `sample_skips_unresolved_gens_without_crash`.

#[cfg(test)]
mod tests;

use num_complex::Complex64;

use crate::elements::control::control_elem::{ControlElemData, RefSnapshot};
use crate::elements::traits::{CktElement, ElemRef, SysCtx};
use crate::obj::base::{DssObjData, DssObject};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

/// 1-based property ordinals (Pascal `TGenDispatcherProp` + the
/// `TCktElementClass` tail).
pub mod prop {
    pub const ELEMENT: usize = 1;
    pub const TERMINAL: usize = 2;
    pub const KWLIMIT: usize = 3;
    pub const KWBAND: usize = 4;
    pub const KVARLIMIT: usize = 5;
    pub const GENLIST: usize = 6;
    pub const WEIGHTS: usize = 7;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 8;
    pub const ENABLED: usize = 9;
    pub const NUM_PROPS: usize = 10; // incl. Like
}

/// `TGenDispatcher.DefineProperties`.
pub fn class_props(_enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    let defs = vec![
        // Pascal `PropertyOffset2 = 0` + WriteByFunction(SetMonitoredElement) +
        // Required: any circuit element by full name. (The Pascal `Required`
        // flag is inert here — not enforced in the port yet, same as the other
        // controls; a missing Element still surfaces via RecalcElementData 372.)
        PropDef::object_ref_any("Element"),
        PropDef::integer("Terminal"),
        PropDef::double("kWLimit"),
        PropDef::double("kWBand"),
        PropDef::double("kvarLimit"),
        PropDef::string_list("GenList"),
        // Pascal `DoubleArrayProperty` with IndirectCount over the GenList: the
        // element count is the generator-name-list length (see `array_size`).
        PropDef::double_v_array("Weights"),
        // TCktElementClass tail:
        PropDef::double("basefreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("enabled"),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);
    ClassProps::new("GenDispatcher", defs, true)
}

/// The executive surface `Sample` needs to reach the monitored element and the
/// dispatched generators (the Rust stand-in for Pascal's live object pointers).
/// `ElemRef`s returned by the lookups are passed back to the accessors.
pub(crate) trait GenDispatchEnv {
    /// Pascal `MonitoredElement.Power[ElementTerminal]` (complex VA: W + jVAr).
    fn monitored_power(&mut self) -> Complex64;
    /// Pascal `GenClass.Find(name)` restricted to *enabled* generators.
    fn find_enabled_gen(&self, name: &str) -> Option<ElemRef>;
    /// Pascal's "scan the whole circuit for enabled generators", creation order.
    fn all_enabled_gens(&self) -> Vec<ElemRef>;
    /// `Gen.kWBase` (the public published field; setting it has no side effect,
    /// which is why `Sample` sets `LoadsNeedUpdating` to force a recalc).
    fn gen_kw_base(&self, g: ElemRef) -> f64;
    fn set_gen_kw_base(&mut self, g: ElemRef, value: f64);
    /// `Gen.kvarBase`.
    fn gen_kvar_base(&self, g: ElemRef) -> f64;
    fn set_gen_kvar_base(&mut self, g: ElemRef, value: f64);
}

/// `TGenDispatcherObj`.
#[derive(Debug, Clone)]
pub struct GenDispatcher {
    pub ccd: ControlElemData,
    /// Dump name of the monitored element (Pascal renders `FullName`).
    monitored_full_name: String,
    /// Parse-time shape snapshot of the monitored reference.
    mon_snap: Option<RefSnapshot>,

    f_kw_limit: f64,
    f_kw_band: f64,
    half_kw_band: f64,
    f_kvar_limit: f64,
    total_weight: f64,
    /// `FListSize` — the dispatched-generator count (the GenList length once a
    /// list is given, else the enabled-generator count discovered on first
    /// `Sample`).
    list_size: i32,
    /// `FGeneratorNameList`.
    gen_name_list: Vec<String>,
    /// `FWeights` (one per list entry; default 1.0).
    weights: Vec<f64>,
    /// `FGenPointerList` — resolved lazily on the first `Sample` (empty until
    /// then), cached across samples exactly like Pascal.
    gen_pointer_list: Vec<ElemRef>,
}

impl GenDispatcher {
    /// Pascal `TGenDispatcherObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut ccd = ControlElemData::new(name, prop::NUM_PROPS);
        ccd.cd.nphases = 3; // directly set conds and phases
        ccd.cd.nconds = 3;
        ccd.cd.set_nterms(1); // forces allocation of terminals and conductors
        ccd.element_terminal = 1;

        let f_kw_limit = 8000.0;
        let f_kw_band = 100.0;
        Self {
            ccd,
            monitored_full_name: String::new(),
            mon_snap: None,
            f_kw_limit,
            f_kw_band,
            half_kw_band: f_kw_band / 2.0,
            f_kvar_limit: f_kw_limit / 2.0,
            total_weight: 1.0,
            list_size: 0,
            gen_name_list: Vec::new(),
            weights: Vec::new(),
            gen_pointer_list: Vec::new(),
        }
    }

    /// Pascal `TGenDispatcherObj.MakeGenList`: build (or rebuild) the resolved
    /// generator pointer list. A named list resolves each entry against the
    /// generator class (keeping only enabled ones, preserving their existing
    /// weights); an empty name list scans every enabled generator and allocates
    /// uniform weights. Returns whether the list ended up non-empty.
    fn make_gen_list(&mut self, env: &dyn GenDispatchEnv) -> bool {
        self.gen_pointer_list.clear();

        if self.list_size > 0 {
            // Name list is defined — use it.
            let mut refs = Vec::with_capacity(self.gen_name_list.len());
            for name in &self.gen_name_list {
                if let Some(g) = env.find_enabled_gen(name) {
                    refs.push(g);
                }
            }
            self.gen_pointer_list = refs;
        } else {
            // Search the entire circuit for enabled generators.
            self.gen_pointer_list = env.all_enabled_gens();
            // Allocate uniform weights.
            self.list_size = self.gen_pointer_list.len() as i32;
            self.weights = vec![1.0; self.list_size as usize];
        }

        // Add up total weights.
        self.total_weight = self
            .weights
            .iter()
            .take(self.list_size.max(0) as usize)
            .sum();

        !self.gen_pointer_list.is_empty()
    }

    /// Pascal `TGenDispatcherObj.Sample`: read the monitored power, and if it is
    /// more than `HalfkWBand` outside `kWLimit` (resp. `kvarLimit`), redispatch
    /// every generator by its weighted share of the deficit. Returns whether any
    /// generator's base changed (the caller then sets `LoadsNeedUpdating` and
    /// pushes a present-time control action, Pascal's `if … then` tail).
    pub(crate) fn sample(&mut self, env: &mut dyn GenDispatchEnv) -> bool {
        // If the list is not defined, make one from all generators in circuit.
        if self.gen_pointer_list.is_empty() {
            self.make_gen_list(env);
        }
        if self.list_size <= 0 {
            return false;
        }

        let s = env.monitored_power(); // power in the active terminal
        let p_diff = s.re * 0.001 - self.f_kw_limit;
        let q_diff = s.im * 0.001 - self.f_kvar_limit;

        let mut changed = false;

        if p_diff.abs() > self.half_kw_band {
            // PDiff is the kW needed to get back into band.
            for (i, &g) in self.gen_pointer_list.iter().enumerate() {
                let cur = env.gen_kw_base(g);
                let gen_kw = (cur + p_diff * (self.weights[i] / self.total_weight)).max(1.0);
                if gen_kw != cur {
                    env.set_gen_kw_base(g, gen_kw);
                    changed = true;
                }
            }
        }

        if q_diff.abs() > self.half_kw_band {
            // QDiff is the kvar needed to get back into band.
            for (i, &g) in self.gen_pointer_list.iter().enumerate() {
                let cur = env.gen_kvar_base(g);
                let gen_kvar = (cur + q_diff * (self.weights[i] / self.total_weight)).max(0.0);
                if gen_kvar != cur {
                    env.set_gen_kvar_base(g, gen_kvar);
                    changed = true;
                }
            }
        }

        changed
    }

    /// Pascal `TGenDispatcherObj.RecalcElementData` (parse-time subset): validate
    /// the monitored element and attach the control's single terminal to the
    /// monitored terminal's bus.
    fn recalc(&mut self) {
        let Some(mon) = self.mon_snap.clone() else {
            // Pascal `DoSimpleMsg('Monitored Element in %s is not set', 372)`.
            self.ccd.cd.obj.push_error(format!(
                "Monitored Element in GenDispatcher.{} is not set",
                self.ccd.cd.obj.name()
            ));
            return;
        };

        if self.ccd.element_terminal > mon.nterms as i32 {
            // Pascal `DoErrorMsg(... 'Terminal no. "%d" does not exist.' 371)`.
            self.ccd.cd.obj.push_error(format!(
                "GenDispatcher: \"{}\": Terminal no. \"{}\" does not exist. Re-specify terminal no.",
                self.ccd.cd.obj.name(),
                self.ccd.element_terminal
            ));
            return;
        }

        // Set the name of the control's 1st terminal's connected bus.
        let t = self.ccd.element_terminal;
        let bus = if t >= 1 && (t as usize) <= mon.buses.len() {
            mon.buses[(t - 1) as usize].clone()
        } else {
            String::new() // Pascal GetBus(i) out of range yields ''
        };
        self.ccd.cd.set_bus(1, &bus);
    }
}

impl CktElement for GenDispatcher {
    fn cd(&self) -> &crate::elements::ckt::CktElementData {
        &self.ccd.cd
    }
    fn cd_mut(&mut self) -> &mut crate::elements::ckt::CktElementData {
        &mut self.ccd.cd
    }

    fn recalc_element_data(&mut self, _sys: &SysCtx) {
        self.recalc();
    }

    /// Pascal `TControlElem.CalcYPrim`: leave YPrim NIL — `BuildYMatrix` skips it.
    fn calc_yprim(&mut self, _sys: &SysCtx) {}

    /// Pascal `TControlElem.GetCurrents`: always zero.
    fn get_currents(&mut self, _sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
        curr.fill(Complex64::ZERO);
    }
}

impl DssObject for GenDispatcher {
    fn data(&self) -> &DssObjData {
        &self.ccd.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.ccd.cd.obj
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

    fn get_f64(&self, idx: usize) -> f64 {
        use prop::*;
        match idx {
            KWLIMIT => self.f_kw_limit,
            KWBAND => self.f_kw_band,
            KVARLIMIT => self.f_kvar_limit,
            BASE_FREQ => self.ccd.cd.base_frequency,
            _ => unreachable!("GenDispatcher has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            KWLIMIT => self.f_kw_limit = value,
            KWBAND => self.f_kw_band = value,
            KVARLIMIT => self.f_kvar_limit = value,
            BASE_FREQ => self.ccd.cd.base_frequency = value,
            _ => unreachable!("GenDispatcher has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        match idx {
            prop::TERMINAL => self.ccd.element_terminal,
            _ => unreachable!("GenDispatcher has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        match idx {
            prop::TERMINAL => self.ccd.element_terminal = value,
            _ => unreachable!("GenDispatcher has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        match idx {
            prop::ENABLED => self.ccd.cd.enabled,
            _ => unreachable!("GenDispatcher has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        match idx {
            // GenDispatcher does not override Set_Enabled; for a control with no
            // Yprim, toggling the flag has no node-order effect either way.
            prop::ENABLED => self.ccd.cd.enabled = value,
            _ => unreachable!("GenDispatcher has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        match idx {
            prop::ELEMENT => self.monitored_full_name.clone(),
            _ => unreachable!("GenDispatcher has no string property {idx}"),
        }
    }

    fn get_string_list(&self, idx: usize) -> Vec<String> {
        match idx {
            prop::GENLIST => self.gen_name_list.clone(),
            _ => unreachable!("GenDispatcher has no string-list property {idx}"),
        }
    }
    fn set_string_list(&mut self, idx: usize, value: Vec<String>) {
        match idx {
            prop::GENLIST => self.gen_name_list = value,
            _ => unreachable!("GenDispatcher has no string-list property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        match idx {
            // Pascal `FWeights` is NIL until a GenList allocates it; a NIL array
            // dumps as "" (not "[]"), so report the empty list as absent.
            prop::WEIGHTS => (!self.weights.is_empty()).then_some(self.weights.as_slice()),
            _ => unreachable!("GenDispatcher has no double-array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        match idx {
            prop::WEIGHTS => self.weights = value,
            _ => unreachable!("GenDispatcher has no double-array property {idx}"),
        }
    }
    /// Pascal `Weights` IndirectCount: the element count comes from the GenList
    /// (`PropertyOffset3 = @FGeneratorNameList`), so the array is sized by the
    /// number of generator names.
    fn array_size(&self, idx: usize) -> usize {
        match idx {
            prop::WEIGHTS => self.gen_name_list.len(),
            _ => unreachable!("GenDispatcher has no function-sized array {idx}"),
        }
    }

    /// `element=` resolution (any circuit element by full name): keep the
    /// `ElemRef` plus a shape snapshot for `RecalcElementData`.
    fn set_object_ref(
        &mut self,
        idx: usize,
        name: String,
        resolved: Option<(ElemRef, &dyn DssObject)>,
    ) {
        match idx {
            prop::ELEMENT => {
                // `name` is the FullName ("Class.name") for the dump.
                self.monitored_full_name = name.clone();
                match resolved {
                    Some((r, obj)) => {
                        self.ccd.monitored_element = Some(r);
                        let elem = obj
                            .as_ckt_element()
                            .expect("element= resolves against circuit classes");
                        self.mon_snap = Some(RefSnapshot::capture(name, elem));
                    }
                    None => {
                        self.ccd.monitored_element = None;
                        self.mon_snap = None;
                    }
                }
            }
            _ => unreachable!("GenDispatcher has no object-ref property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.ccd.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.ccd.cd.get_bus(terminal).to_string()
    }

    /// Pascal `TGenDispatcherObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        match idx {
            prop::KWBAND => self.half_kw_band = self.f_kw_band / 2.0,
            prop::GENLIST => {
                // Levelize the list.
                self.gen_pointer_list.clear(); // reset on first sample
                self.list_size = self.gen_name_list.len() as i32;
                self.weights = vec![1.0; self.list_size.max(0) as usize];
            }
            _ => {}
        }
    }

    /// Pascal `TCktElementClass.EndEdit` default → `RecalcElementData`.
    fn end_edit(&mut self) {
        self.recalc();
    }

    /// Pascal `TGenDispatcherObj.MakeLike` — note it copies *only* the phase
    /// count, monitored element, and terminal (plus the base `PrpSequence`); the
    /// dispatch settings (`kWLimit`/`kWBand`/`kvarLimit`/`GenList`/`Weights`)
    /// are deliberately **not** copied, so a `like=` dispatcher keeps the ctor
    /// defaults for them. Reproduced verbatim.
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(other) = other.as_any().downcast_ref::<GenDispatcher>() else {
            return;
        };
        self.ccd.cd.make_like_base(&other.ccd.cd);
        self.ccd.cd.nphases = other.ccd.cd.nphases;
        let nc = other.ccd.cd.nconds;
        self.ccd.cd.set_nconds(nc); // Force Reallocation of terminal stuff
        self.ccd.monitored_element = other.ccd.monitored_element;
        self.monitored_full_name = other.monitored_full_name.clone();
        self.mon_snap = other.mon_snap.clone();
        self.ccd.element_terminal = other.ccd.element_terminal;
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

//! Port of `Controls/RegControl.pas` — `TRegControlObj`, the voltage-regulator
//! control attached to a Transformer winding. **Phase 4 ports the parse-time
//! surface only** (properties, reference resolution, `RecalcElementData`'s
//! bus/phase setup, `TapNum`, `MakeLike`); the `Sample`/`DoPendingAction`
//! tap-changing machinery is Phase 5 (PHASE4_PLAN §WP4.7) and is recorded as an
//! engine error if ever invoked.
//!
//! A RegControl is a `TControlElem`: a circuit element with **no Yprim** and
//! zero terminal currents, whose single terminal is attached to the controlled
//! transformer's winding bus so it participates in bus-list processing without
//! changing node order.

use num_complex::Complex64;

use crate::elements::control::control_elem::{ControlElemData, RefSnapshot};
use crate::elements::pd::transformer::Transformer;
use crate::elements::traits::{CktElement, ElemRef, SysCtx};
use crate::obj::base::{DssObjData, DssObject, RefAction};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

/// 1-based property ordinals (Pascal `TRegControlProp` + class tails).
pub mod prop {
    pub const TRANSFORMER: usize = 1;
    pub const WINDING: usize = 2;
    pub const VREG: usize = 3;
    pub const BAND: usize = 4;
    pub const PTRATIO: usize = 5;
    pub const CTPRIM: usize = 6;
    pub const R: usize = 7;
    pub const X: usize = 8;
    pub const BUS: usize = 9;
    pub const DELAY: usize = 10;
    pub const REVERSIBLE: usize = 11;
    pub const REVVREG: usize = 12;
    pub const REVBAND: usize = 13;
    pub const REVR: usize = 14;
    pub const REVX: usize = 15;
    pub const TAPDELAY: usize = 16;
    pub const DEBUGTRACE: usize = 17;
    pub const MAXTAPCHANGE: usize = 18;
    pub const INVERSETIME: usize = 19;
    pub const TAPWINDING: usize = 20;
    pub const VLIMIT: usize = 21;
    pub const PTPHASE: usize = 22;
    pub const REVTHRESHOLD: usize = 23;
    pub const REVDELAY: usize = 24;
    pub const REVNEUTRAL: usize = 25;
    pub const EVENTLOG: usize = 26;
    pub const REMOTEPTRATIO: usize = 27;
    pub const TAPNUM: usize = 28;
    pub const RESET: usize = 29;
    pub const LDC_Z: usize = 30;
    pub const REV_Z: usize = 31;
    pub const COGEN: usize = 32;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 33;
    pub const ENABLED: usize = 34;
    pub const NUM_PROPS: usize = 35; // incl. Like
}

/// `TRegControl.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    let defs = vec![
        // Pascal resolves against a Transformer/AutoTrans proxy; AutoTrans is
        // not ported (Phase 6+), so the reference is Transformer-only here.
        // Pascal also flags `CheckForVar` + `Required` (both inert here).
        PropDef::object_ref_class("Transformer", "Transformer"),
        PropDef::integer("Winding"),
        PropDef::double("VReg"),
        PropDef::double("Band"),
        PropDef::double("PTRatio"),
        PropDef::double("CTPrim"),
        PropDef::double("R"),
        PropDef::double("X"),
        PropDef::string("Bus"),
        PropDef::double("Delay"),
        PropDef::boolean("Reversible"),
        PropDef::double("RevVReg"),
        PropDef::double("RevBand"),
        PropDef::double("RevR"),
        PropDef::double("RevX"),
        PropDef::double("TapDelay"),
        PropDef::boolean("DebugTrace"),
        PropDef::integer("MaxTapChange"),
        PropDef::boolean("InverseTime"),
        PropDef::integer("TapWinding"),
        PropDef::double("VLimit"),
        PropDef::mapped_string_enum("PTPhase", enums.reg_control_phase),
        PropDef::double("RevThreshold"),
        PropDef::double("RevDelay"),
        PropDef::boolean("RevNeutral"),
        PropDef::boolean("EventLog"),
        PropDef::double("RemotePTRatio"),
        // Pascal: IntegerProperty with Read/WriteByFunction (Get_/Set_TapNum).
        PropDef::integer("TapNum"),
        // Pascal: BooleanActionProperty (DoReset); the getter is always 0.
        PropDef::boolean("Reset"),
        PropDef::double("LDC_Z"),
        PropDef::double("Rev_Z"),
        PropDef::boolean("Cogen"),
        // TCktElementClass tail:
        PropDef::double("basefreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("enabled"),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);
    ClassProps::new("RegControl", defs, true)
}

/// Per-winding tap snapshot: `(PresentTap, MaxTap, MinTap, TapIncrement)`,
/// captured from the live transformer when `transformer=` resolves (see
/// [`RefSnapshot`] for the staleness argument) and kept in sync by `TapNum`
/// writes (both sides apply the identical clamp).
type TapSnap = (f64, f64, f64, f64);

/// `TRegControlObj`.
#[derive(Debug, Clone)]
pub struct RegControl {
    pub ccd: ControlElemData,
    /// Dump name of the controlled transformer (Pascal renders `Name`).
    controlled_name: String,
    /// Parse-time shape snapshot of the controlled transformer.
    snapshot: Option<RefSnapshot>,
    /// Per-winding tap data of the controlled transformer (1-based winding i
    /// at `tap_snap[i-1]`).
    tap_snap: Vec<TapSnap>,
    /// Deferred `TapNum` writes for the executive to apply post-edit.
    pending_actions: Vec<RefAction>,

    vreg: f64,
    bandwidth: f64,
    pt_ratio: f64,
    remote_pt_ratio: f64,
    ct_rating: f64,
    r: f64,
    x: f64,
    ldc_z: f64,
    regulated_bus: String,
    using_regulated_bus: bool,
    ldc_active: bool,
    fpt_phase: i32,
    tap_delay: f64,
    tap_limit_per_change: i32,
    debug_trace: bool,
    inverse_time: bool,
    tap_winding: i32,
    vlimit: f64,
    // Reverse-power variables:
    is_reversible: bool,
    rev_vreg: f64,
    rev_bandwidth: f64,
    rev_r: f64,
    rev_x: f64,
    rev_ldc_z: f64,
    rev_delay: f64,
    rev_power_threshold: f64,
    kw_rev_power_threshold: f64,
    reverse_neutral: bool,
    cogen_enabled: bool,
    // Phase-5 runtime state (kept so `Reset` is faithful):
    pending_tap_change: f64,
    armed: bool,
}

impl RegControl {
    /// Pascal `TRegControlObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut ccd = ControlElemData::new(name, prop::NUM_PROPS);
        ccd.cd.nphases = 3;
        ccd.cd.nconds = 3;
        ccd.cd.set_nterms(1); // forces allocation of terminals and conductors
        ccd.element_terminal = 1;
        ccd.time_delay = 15.0;

        Self {
            ccd,
            controlled_name: String::new(),
            snapshot: None,
            tap_snap: Vec::new(),
            pending_actions: Vec::new(),
            vreg: 120.0,
            bandwidth: 3.0,
            pt_ratio: 60.0,
            remote_pt_ratio: 60.0, // RemotePTRatio := PTRatio
            ct_rating: 300.0,
            r: 0.0,
            x: 0.0,
            ldc_z: 0.0,
            regulated_bus: String::new(),
            using_regulated_bus: false,
            ldc_active: false,
            fpt_phase: 1,
            tap_delay: 2.0,
            tap_limit_per_change: 16,
            debug_trace: false,
            inverse_time: false,
            tap_winding: 1, // TapWinding := ElementTerminal
            vlimit: 0.0,
            is_reversible: false,
            rev_vreg: 120.0,
            rev_bandwidth: 3.0,
            rev_r: 0.0,
            rev_x: 0.0,
            rev_ldc_z: 0.0,
            rev_delay: 60.0,
            rev_power_threshold: 100_000.0, // 100 kW
            kw_rev_power_threshold: 100.0,
            reverse_neutral: false,
            cogen_enabled: false,
            pending_tap_change: 0.0,
            armed: false,
        }
    }

    /// Pascal `TRegControlObj.Reset` (the `Reset` action property).
    fn reset(&mut self) {
        self.pending_tap_change = 0.0;
        self.armed = false;
    }

    /// Pascal `Get_TapNum`: integer tap position relative to the mid-tap,
    /// computed from the controlled winding's tap data.
    fn get_tap_num(&self) -> i32 {
        if self.ccd.controlled_element.is_none() {
            return 0;
        }
        let w = self.tap_winding;
        if w < 1 || w as usize > self.tap_snap.len() {
            return 0;
        }
        let (tap, max_tap, min_tap, inc) = self.tap_snap[(w - 1) as usize];
        if inc == 0.0 {
            return 0; // NumTaps = 0 winding; Pascal would divide by zero
        }
        // TODO(compat): FPC `Round` is ties-to-even with an integer-indefinite
        // path for out-of-Int64 magnitudes; tap positions are tiny integers, so
        // plain ties-to-even matches. Wiped with the other compat shims.
        ((tap - (max_tap + min_tap) / 2.0) / inc).round_ties_even() as i32
    }

    /// Pascal `Set_TapNum`: position the controlled winding's tap at
    /// `mid-tap + value · increment`. The transformer write is deferred (see
    /// [`RefAction`]); the local tap snapshot applies the identical clamp so a
    /// subsequent dump reads the same value the oracle would.
    fn set_tap_num(&mut self, value: i32) {
        if self.ccd.controlled_element.is_none() {
            // Pascal: `if not Assigned(ControlledElement) then RecalcElementData()`
            // — which reports error 124 and leaves the reference NIL.
            self.recalc();
            return;
        }
        let target = self.ccd.controlled_element.expect("checked above");
        let w = self.tap_winding;
        if w < 1 || w as usize > self.tap_snap.len() {
            return;
        }
        let snap = &mut self.tap_snap[(w - 1) as usize];
        let (_, max_tap, min_tap, inc) = *snap;
        let new_tap = value as f64 * inc + (max_tap + min_tap) / 2.0;
        // Tap range checking is done in PresentTap (the target clamps too).
        snap.0 = new_tap.clamp(min_tap, max_tap);
        self.pending_actions.push(RefAction::SetTransformerTap {
            target,
            winding: w as usize,
            tap: new_tap,
        });
    }

    /// Pascal `TRegControlObj.RecalcElementData` (parse-time subset: LDC/bus
    /// flags, phase/conductor counts from the controlled transformer, terminal
    /// bus). The VBuffer/CBuffer sampling buffers are Phase 5.
    fn recalc(&mut self) {
        self.ldc_active = self.r != 0.0 || self.x != 0.0 || self.ldc_z > 0.0;
        self.using_regulated_bus = !self.regulated_bus.is_empty();

        if self.ccd.controlled_element.is_none() {
            // element not found or not set (DoErrorMsg 124)
            self.ccd.cd.obj.push_error(format!(
                "RegControl: \"{}\": Transformer Element is not set. Element must be defined previously. (Error 124)",
                self.ccd.cd.obj.name()
            ));
            return;
        }
        let snap = self.snapshot.clone().unwrap_or_default();

        if self.using_regulated_bus {
            self.ccd.cd.nphases = 1; // Only need one phase
            self.ccd.cd.set_nconds(2);
        } else {
            self.ccd.cd.nphases = snap.nphases;
            self.ccd.cd.set_nconds(snap.nphases);
            if self.fpt_phase > self.ccd.cd.nphases as i32 {
                self.fpt_phase = 1;
                self.ccd.cd.obj.set_as_next_seq(prop::PTPHASE);
            }
        }

        // The reference resolves against the Transformer class only, so the
        // Pascal "Controlled Regulator Element is not a transformer" branch
        // (error 123) is unreachable here.
        if self.ccd.element_terminal > snap.nterms as i32 {
            self.ccd.cd.obj.push_error(format!(
                "RegControl: \"{}\": Winding no. \"{}\" does not exist. Respecify Monitored Winding no. (Error 122)",
                self.ccd.cd.obj.name(),
                self.ccd.element_terminal
            ));
        } else {
            // Sets the name of the 1st terminal's connected bus; this value
            // seeds the NodeRef array at bus-list processing.
            let bus = if self.using_regulated_bus {
                self.regulated_bus.clone() // hopefully this will actually exist
            } else {
                let t = self.ccd.element_terminal;
                if t >= 1 && (t as usize) <= snap.buses.len() {
                    snap.buses[(t - 1) as usize].clone()
                } else {
                    String::new() // Pascal GetBus(i) out of range yields ''
                }
            };
            self.ccd.cd.set_bus(1, &bus);
            // VBuffer/CBuffer (regulator voltage/current sampling buffers) are
            // allocated here in Pascal — Phase 5 (Sample machinery).
        }
    }
}

impl CktElement for RegControl {
    fn cd(&self) -> &crate::elements::ckt::CktElementData {
        &self.ccd.cd
    }
    fn cd_mut(&mut self) -> &mut crate::elements::ckt::CktElementData {
        &mut self.ccd.cd
    }

    fn recalc_element_data(&mut self, _sys: &SysCtx) {
        self.recalc();
    }

    /// Pascal `TControlElem.CalcYPrim`: leave YPrim as NIL — `BuildYMatrix`
    /// skips elements with no primitive matrix.
    fn calc_yprim(&mut self, _sys: &SysCtx) {}

    /// Pascal `TControlElem.GetCurrents`: always zero.
    fn get_currents(&mut self, _sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
        curr.fill(Complex64::ZERO);
    }
}

impl DssObject for RegControl {
    fn data(&self) -> &DssObjData {
        &self.ccd.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.ccd.cd.obj
    }
    fn as_any(&self) -> &dyn std::any::Any {
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
            VREG => self.vreg,
            BAND => self.bandwidth,
            PTRATIO => self.pt_ratio,
            CTPRIM => self.ct_rating,
            R => self.r,
            X => self.x,
            DELAY => self.ccd.time_delay,
            REVVREG => self.rev_vreg,
            REVBAND => self.rev_bandwidth,
            REVR => self.rev_r,
            REVX => self.rev_x,
            TAPDELAY => self.tap_delay,
            VLIMIT => self.vlimit,
            REVTHRESHOLD => self.kw_rev_power_threshold,
            REVDELAY => self.rev_delay,
            REMOTEPTRATIO => self.remote_pt_ratio,
            LDC_Z => self.ldc_z,
            REV_Z => self.rev_ldc_z,
            BASE_FREQ => self.ccd.cd.base_frequency,
            _ => unreachable!("RegControl has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            VREG => self.vreg = value,
            BAND => self.bandwidth = value,
            PTRATIO => self.pt_ratio = value,
            CTPRIM => self.ct_rating = value,
            R => self.r = value,
            X => self.x = value,
            DELAY => self.ccd.time_delay = value,
            REVVREG => self.rev_vreg = value,
            REVBAND => self.rev_bandwidth = value,
            REVR => self.rev_r = value,
            REVX => self.rev_x = value,
            TAPDELAY => self.tap_delay = value,
            VLIMIT => self.vlimit = value,
            REVTHRESHOLD => self.kw_rev_power_threshold = value,
            REVDELAY => self.rev_delay = value,
            REMOTEPTRATIO => self.remote_pt_ratio = value,
            LDC_Z => self.ldc_z = value,
            REV_Z => self.rev_ldc_z = value,
            BASE_FREQ => self.ccd.cd.base_frequency = value,
            _ => unreachable!("RegControl has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use prop::*;
        match idx {
            WINDING => self.ccd.element_terminal,
            MAXTAPCHANGE => self.tap_limit_per_change,
            TAPWINDING => self.tap_winding,
            PTPHASE => self.fpt_phase,
            TAPNUM => self.get_tap_num(),
            _ => unreachable!("RegControl has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            WINDING => self.ccd.element_terminal = value,
            MAXTAPCHANGE => self.tap_limit_per_change = value,
            TAPWINDING => self.tap_winding = value,
            PTPHASE => self.fpt_phase = value,
            TAPNUM => self.set_tap_num(value),
            _ => unreachable!("RegControl has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use prop::*;
        match idx {
            REVERSIBLE => self.is_reversible,
            DEBUGTRACE => self.debug_trace,
            INVERSETIME => self.inverse_time,
            REVNEUTRAL => self.reverse_neutral,
            EVENTLOG => self.ccd.show_event_log,
            COGEN => self.cogen_enabled,
            RESET => false, // Pascal BooleanActionProperty getter: always 0
            ENABLED => self.ccd.cd.enabled,
            _ => unreachable!("RegControl has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use prop::*;
        match idx {
            REVERSIBLE => self.is_reversible = value,
            DEBUGTRACE => self.debug_trace = value,
            INVERSETIME => self.inverse_time = value,
            REVNEUTRAL => self.reverse_neutral = value,
            EVENTLOG => self.ccd.show_event_log = value,
            COGEN => self.cogen_enabled = value,
            RESET => {
                // Pascal BooleanActionProperty: the action fires on TRUE only.
                if value {
                    self.reset();
                }
            }
            // Pascal `TRegControlObj.Set_Enabled` override: only toggle the
            // flag — no BusNameRedefined side effect.
            ENABLED => self.ccd.cd.enabled = value,
            _ => unreachable!("RegControl has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use prop::*;
        match idx {
            TRANSFORMER => self.controlled_name.clone(),
            BUS => self.regulated_bus.clone(),
            _ => unreachable!("RegControl has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        match idx {
            prop::BUS => self.regulated_bus = value,
            _ => unreachable!("RegControl has no string property {idx}"),
        }
    }

    /// `transformer=` resolution: keep the `ElemRef` and snapshot the
    /// transformer's shape + per-winding tap data for `RecalcElementData` and
    /// `TapNum` (which run after the foreign view is gone).
    fn set_object_ref(
        &mut self,
        idx: usize,
        name: String,
        resolved: Option<(ElemRef, &dyn DssObject)>,
    ) {
        debug_assert_eq!(idx, prop::TRANSFORMER);
        self.controlled_name = name;
        match resolved {
            Some((r, obj)) => {
                self.ccd.controlled_element = Some(r);
                let elem = obj
                    .as_ckt_element()
                    .expect("Transformer is a circuit element");
                self.snapshot = Some(RefSnapshot::capture(
                    format!("Transformer.{}", obj.data().name()),
                    elem,
                ));
                let xf = obj
                    .as_any()
                    .downcast_ref::<Transformer>()
                    .expect("transformer= resolves against the Transformer class");
                self.tap_snap = (1..=xf.num_windings().max(0) as usize)
                    .map(|i| xf.winding_tap_data(i))
                    .collect();
            }
            None => {
                self.ccd.controlled_element = None;
                self.snapshot = None;
                self.tap_snap.clear();
            }
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.ccd.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.ccd.cd.get_bus(terminal).to_string()
    }

    /// Pascal `TRegControlObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        use prop::*;
        match idx {
            TRANSFORMER => {
                // MonitoredElement := ControlledElement (same for this
                // controller). Pascal also sets PrpSequence[transformer] :=
                // -10 so Save writes it first; Save is not ported (Phase 6+),
                // so the mark is omitted.
                self.ccd.monitored_element = self.ccd.controlled_element;
            }
            WINDING => {
                // Resets if property re-assigned.
                self.tap_winding = self.ccd.element_terminal;
            }
            PTRATIO => {
                // re-initialise RemotePTRatio whenever PTRatio is set
                self.remote_pt_ratio = self.pt_ratio;
            }
            DEBUGTRACE => {
                // Pascal opens/closes the REG_<name>.csv trace file here; the
                // tap-changing machinery that writes it is Phase 5, so the
                // flag is stored without the file.
            }
            MAXTAPCHANGE => {
                self.tap_limit_per_change = self.tap_limit_per_change.max(0);
            }
            REVTHRESHOLD => {
                self.rev_power_threshold = self.kw_rev_power_threshold * 1000.0;
            }
            _ => {}
        }
    }

    /// Pascal `TCktElementClass.EndEdit` default → `RecalcElementData`.
    fn end_edit(&mut self) {
        self.recalc();
    }

    fn take_ref_actions(&mut self) -> Vec<RefAction> {
        std::mem::take(&mut self.pending_actions)
    }

    /// Pascal `TRegControlObj.MakeLike`.
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(other) = other.as_any().downcast_ref::<RegControl>() else {
            return;
        };
        self.ccd.cd.make_like_base(&other.ccd.cd);
        self.ccd.cd.nphases = other.ccd.cd.nphases;
        let nc = other.ccd.cd.nconds;
        self.ccd.cd.set_nconds(nc); // Force reallocation of terminal stuff

        // ControlledElement := Other.ControlledElement (pointer copy; the
        // Pascal HasControl/ControlElementList bookkeeping only matters for
        // element deletion, which is not supported).
        self.ccd.controlled_element = other.ccd.controlled_element;
        self.ccd.monitored_element = other.ccd.monitored_element;
        self.controlled_name = other.controlled_name.clone();
        self.snapshot = other.snapshot.clone();
        self.tap_snap = other.tap_snap.clone();

        self.ccd.element_terminal = other.ccd.element_terminal;
        self.vreg = other.vreg;
        self.bandwidth = other.bandwidth;
        self.pt_ratio = other.pt_ratio;
        self.remote_pt_ratio = other.remote_pt_ratio;
        self.ct_rating = other.ct_rating;
        self.r = other.r;
        self.x = other.x;
        self.regulated_bus = other.regulated_bus.clone();
        self.ccd.time_delay = other.ccd.time_delay;
        self.is_reversible = other.is_reversible;
        self.rev_vreg = other.rev_vreg;
        self.rev_bandwidth = other.rev_bandwidth;
        self.rev_r = other.rev_r;
        self.rev_x = other.rev_x;
        self.tap_delay = other.tap_delay;
        self.tap_winding = other.tap_winding;
        self.inverse_time = other.inverse_time;
        self.tap_limit_per_change = other.tap_limit_per_change;
        self.kw_rev_power_threshold = other.kw_rev_power_threshold;
        self.rev_power_threshold = other.rev_power_threshold;
        self.rev_delay = other.rev_delay;
        self.reverse_neutral = other.reverse_neutral;
        self.ccd.show_event_log = other.ccd.show_event_log;
        // DebugTrace := Other.DebugTrace;  Always default to NO
        self.fpt_phase = other.fpt_phase;
        // TapNum := Other.TapNum — runs the property setter, repositioning the
        // (copied) controlled transformer's tap; with untouched taps this is a
        // no-op write of the mid-tap.
        self.set_tap_num(other.get_tap_num());
        self.cogen_enabled = other.cogen_enabled;
        self.ldc_z = other.ldc_z;
        self.rev_ldc_z = other.rev_ldc_z;
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solution::SolveMode;

    fn test_sys() -> SysCtx {
        SysCtx {
            frequency: 60.0,
            fundamental: 60.0,
            is_harmonic_model: false,
            is_dynamic_model: false,
            load_model: 1,
            mode: SolveMode::Snapshot,
            load_multiplier: 1.0,
            default_growth_factor: 1.0,
            year: 0,
            dbl_hour: 0.0,
            solution_count: 0,
            loads_need_updating: false,
            neglect_load_y: false,
            long_line_correction: false,
            positive_sequence: false,
        }
    }

    #[test]
    fn default_shape_is_3ph_1term_no_yprim() {
        let mut rc = RegControl::new("r1");
        assert_eq!(rc.ccd.cd.nphases, 3);
        assert_eq!(rc.ccd.cd.nconds, 3);
        assert_eq!(rc.ccd.cd.nterms, 1);
        assert_eq!(rc.vreg, 120.0);
        assert_eq!(rc.bandwidth, 3.0);
        assert_eq!(rc.pt_ratio, 60.0);
        assert_eq!(rc.remote_pt_ratio, 60.0);
        assert_eq!(rc.ct_rating, 300.0);
        assert_eq!(rc.ccd.time_delay, 15.0);
        assert_eq!(rc.tap_limit_per_change, 16);
        // CalcYPrim is a no-op: YPrim stays None so BuildYMatrix skips it.
        rc.calc_yprim(&test_sys());
        assert!(rc.ccd.cd.yprim.is_none());
        // GetCurrents is always zero.
        let mut curr = vec![Complex64::new(1.0, 1.0); 3];
        rc.get_currents(&test_sys(), &[], &mut curr);
        assert!(curr.iter().all(|c| *c == Complex64::ZERO));
    }

    #[test]
    fn tapnum_maps_tap_to_integer_and_back() {
        let mut rc = RegControl::new("r1");
        rc.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
        rc.tap_winding = 2;
        // (PresentTap, MaxTap, MinTap, TapIncrement) — the 32-tap default.
        rc.tap_snap = vec![(1.0, 1.1, 0.9, 0.00625), (1.0, 1.1, 0.9, 0.00625)];
        assert_eq!(rc.get_tap_num(), 0);

        rc.set_tap_num(5); // → 1.03125 (probed: oracle taps [1, 1.03125])
        assert_eq!(rc.get_tap_num(), 5);
        let actions = rc.take_ref_actions();
        assert_eq!(actions.len(), 1);
        let RefAction::SetTransformerTap { winding, tap, .. } = &actions[0];
        assert_eq!(*winding, 2);
        assert!((tap - 1.03125).abs() < 1e-12);

        rc.set_tap_num(-3); // → 0.98125 (probed)
        assert_eq!(rc.get_tap_num(), -3);
    }

    #[test]
    fn recalc_without_transformer_records_error_124() {
        let mut rc = RegControl::new("r1");
        rc.end_edit();
        let errs = rc.ccd.cd.obj.take_errors();
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("Transformer Element is not set"));
    }
}

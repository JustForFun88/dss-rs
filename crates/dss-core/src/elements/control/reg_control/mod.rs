//! Port of `Controls/RegControl.pas` — `TRegControlObj`, the voltage-regulator
//! control attached to a Transformer winding. Phase 4 ported the parse-time
//! surface (properties, reference resolution, `RecalcElementData`'s bus/phase
//! setup, `TapNum`, `MakeLike`); **WP5.5 adds the behavior** — `Sample` (sense
//! the regulated voltage, compute the pending tap change, arm the control
//! queue) and `DoPendingAction` (apply the tap, per control mode), plus the
//! `AtLeastOneTap`/`OneInDirectionOf`/`ComputeTimeDelay`/`GetControlVoltage`
//! helpers.
//!
//! A RegControl is a `TControlElem`: a circuit element with **no Yprim** and
//! zero terminal currents, whose single terminal is attached to the controlled
//! transformer's winding bus so it participates in bus-list processing without
//! changing node order.
//!
//! The class is split by concern: this file holds the property metadata, the
//! [`RegControl`] struct, its construction/reset lifecycle, the parse-time
//! [`RegControl::recalc`], and the tap-snapshot helpers (`get_tap_num`/
//! `set_tap_num`); [`control_loop`] holds the solve-time `Sample`/
//! `DoPendingAction` machinery and its decision helpers; [`accessors`] holds the
//! [`CktElement`](crate::elements::traits::CktElement)/[`DssObject`](crate::obj::base::DssObject)
//! trait impls.

#[cfg(test)]
mod tests;

mod accessors;
mod control_loop;

use crate::elements::control::control_elem::{ControlElemData, RefSnapshot};
use crate::obj::base::RefAction;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

/// `RegControl.pas` action codes (distinct from the `EControlAction` enum).
const ACTION_TAPCHANGE: i32 = 0;
const ACTION_REVERSE: i32 = 1;

/// `RegControl.pas` PTphase pseudo-phases (the hybrid enum's `max`/`min`).
const MAXPHASE: i32 = -2;
const MINPHASE: i32 = -3;

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
    // Runtime control state (Pascal `TRegControlObj` mutable fields):
    pending_tap_change: f64,
    armed: bool,
    last_change: i32,
    control_action_handle: i32,
    rev_handle: i32,
    rev_back_handle: i32,
    in_reverse_mode: bool,
    reverse_pending: bool,
    in_cogen_mode: bool,
    /// `ControlledPhase`, stored **0-based** (Pascal is 1-based) — set by
    /// `GetControlVoltage`, consumed by the LDC current pickup.
    controlled_phase: usize,
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
            last_change: 0,
            control_action_handle: 0,
            rev_handle: 0,
            rev_back_handle: 0,
            in_reverse_mode: false,
            reverse_pending: false,
            in_cogen_mode: false,
            controlled_phase: 0,
        }
    }

    /// Pascal `TRegControlObj.Reset` (the `Reset` action property and the
    /// `DoResetControls` path).
    pub(crate) fn reset(&mut self) {
        self.pending_tap_change = 0.0;
        self.armed = false;
    }

    /// Keep the parse-time tap snapshot in sync with the live transformer so
    /// the `TapNum` getter / property dump reads what Pascal's live pointer
    /// would after a control action moved the tap.
    fn sync_tap_snap(&mut self, w: usize, tap: f64) {
        if w >= 1 && w <= self.tap_snap.len() {
            self.tap_snap[w - 1].0 = tap;
        }
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

    /// The controlled transformer's [`ElemRef`](crate::elements::traits::ElemRef),
    /// if `transformer=` resolved — lets the executive view read the *live* tap.
    pub(crate) fn controlled_ref(&self) -> Option<crate::elements::traits::ElemRef> {
        self.ccd.controlled_element
    }

    /// Pascal `Get_TapNum` evaluated against the **live** controlled transformer
    /// rather than the parse-time `tap_snap`. A direct `Transformer.X.Taps=`
    /// edit moves the transformer winding tap without going through this control,
    /// so the cached snapshot (resynced only on a control action) goes stale;
    /// Pascal always reads `PresentTap[TapWinding]` live. The executive
    /// `regcontrol_tap_numbers` view uses this so `TapNumber` matches the oracle
    /// after a manual tap set (e.g. the IEEE13 geometry scripts).
    pub(crate) fn tap_num_live(
        &self,
        tr: &dyn crate::elements::pd::transformer::ControlledTransformer,
    ) -> i32 {
        let w = self.tap_winding;
        if self.ccd.controlled_element.is_none() || w < 1 {
            return 0;
        }
        let inc = tr.tap_increment(w as usize);
        if inc == 0.0 {
            return 0; // NumTaps = 0 winding; Pascal would divide by zero
        }
        let mid = (tr.max_tap(w as usize) + tr.min_tap(w as usize)) / 2.0;
        // TODO(compat): FPC `Round` ties-to-even (see `get_tap_num`).
        ((tr.present_tap(w as usize) - mid) / inc).round_ties_even() as i32
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
            // Pascal also (re)allocates the VBuffer/CBuffer sampling buffers
            // here; `sample` allocates them as locals instead.
        }
    }
}

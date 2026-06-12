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

use num_complex::Complex64;

use crate::elements::control::control_elem::{ControlElemData, CtrlCtx, RefSnapshot};
use crate::elements::pd::transformer::{ControlledTransformer, Transformer};
use crate::elements::traits::{CktElement, ElemRef, SysCtx};
use crate::obj::base::{DssObjData, DssObject, RefAction};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use crate::solution::{CTRLSTATIC, EVENTDRIVEN, MULTIRATE, TIMEDRIVEN};
use crate::util::EPSILON;

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

    /// Pascal `set_PendingTapChange`: store the pending change and mirror it to
    /// the debug-trace scratch.
    fn set_pending_tap_change(&mut self, value: f64) {
        self.pending_tap_change = value;
        self.ccd.dbl_trace_param = value;
    }

    /// Pascal `VLimitActive`.
    fn vlimit_active(&self) -> bool {
        self.vlimit > 0.0
    }

    /// Pascal `ComputeTimeDelay`: fixed `Delay`, or the inverse-time form
    /// (`Delay / min(10, 2·|Vreg − Vavg|/Band)`).
    fn compute_time_delay(&self, vavg: f64) -> f64 {
        if self.inverse_time {
            self.ccd.time_delay / 10.0_f64.min(2.0 * (self.vreg - vavg).abs() / self.bandwidth)
        } else {
            self.ccd.time_delay
        }
    }

    /// Pascal `AtLeastOneTap` (STATIC mode): change 70 % of the way but at least
    /// one tap, capped at `TapLimitPerChange`; records `LastChange`.
    fn at_least_one_tap(&mut self, proposed_change: f64, increment: f64) -> f64 {
        let mut num_taps = (0.7 * proposed_change.abs() / increment).trunc() as i32;
        if num_taps == 0 {
            num_taps = 1;
        }
        if num_taps > self.tap_limit_per_change {
            num_taps = self.tap_limit_per_change;
        }
        self.last_change = num_taps;
        if proposed_change > 0.0 {
            num_taps as f64 * increment
        } else {
            self.last_change = -num_taps;
            -(num_taps as f64) * increment
        }
    }

    /// Pascal `OneInDirectionOf`: one tap toward the pending change, decrementing
    /// `FPendingTapChange` directly (no debug-trace mirror, as in Pascal) and
    /// zeroing it once within 0.9 increments.
    fn one_in_direction_of(&mut self, increment: f64) -> f64 {
        self.last_change = 0;
        let result = if self.pending_tap_change > 0.0 {
            self.last_change = 1;
            self.pending_tap_change -= increment;
            increment
        } else {
            self.last_change = -1;
            self.pending_tap_change += increment;
            -increment
        };
        if self.pending_tap_change.abs() < 0.9 * increment {
            self.pending_tap_change = 0.0;
        }
        result
    }

    /// Pascal `GetControlVoltage`: pick the regulated phase per `PTphase`
    /// (specific / `max` / `min`) and divide by the PT ratio. `vbuffer` and the
    /// stored `controlled_phase` are 0-based (Pascal is 1-based).
    fn get_control_voltage(
        &mut self,
        vbuffer: &[Complex64],
        nphs: usize,
        pt_ratio: f64,
    ) -> Complex64 {
        match self.fpt_phase {
            MAXPHASE => {
                let mut cp = 0;
                let mut v = vbuffer[0].norm();
                for (i, val) in vbuffer.iter().enumerate().take(nphs).skip(1) {
                    if val.norm() > v {
                        v = val.norm();
                        cp = i;
                    }
                }
                self.controlled_phase = cp;
                vbuffer[cp] / pt_ratio
            }
            MINPHASE => {
                let mut cp = 0;
                let mut v = vbuffer[0].norm();
                for (i, val) in vbuffer.iter().enumerate().take(nphs).skip(1) {
                    if val.norm() < v {
                        v = val.norm();
                        cp = i;
                    }
                }
                self.controlled_phase = cp;
                vbuffer[cp] / pt_ratio
            }
            // Specific phase (most controls): FPTphase is a 1-based phase.
            _ => {
                let cp = (self.fpt_phase - 1).max(0) as usize;
                self.controlled_phase = cp;
                vbuffer[cp] / pt_ratio
            }
        }
    }

    /// Pascal `TRegControlObj.Sample` — sense the regulated voltage, optionally
    /// flip reverse/cogen mode, and (if out of band) compute `PendingTapChange`
    /// and arm an `ACTION_TAPCHANGE` on the control queue. Ported top-to-bottom.
    pub(crate) fn sample(&mut self, tr: &mut dyn ControlledTransformer, ctx: &mut CtrlCtx) {
        if self.tap_limit_per_change == 0 {
            self.set_pending_tap_change(0.0);
            return;
        }

        // Always looking forward in cogen mode.
        let looking_forward = (!self.in_reverse_mode) || self.in_cogen_mode;
        let element_terminal = self.ccd.element_terminal as usize;
        let tap_winding = self.tap_winding as usize;
        let nphases = self.ccd.cd.nphases;

        // 1) Reverse / cogen power-direction handling (not for regulated bus).
        if !self.using_regulated_bus && (self.is_reversible || self.cogen_enabled) {
            if looking_forward && !self.in_cogen_mode {
                let fwd_power = -tr.power_into_re(element_terminal, ctx.node_v, ctx.sys);
                if !self.reverse_pending && fwd_power < -self.rev_power_threshold {
                    self.reverse_pending = true;
                    self.rev_handle = ctx.queue.push_delay(
                        ctx.int_hour,
                        ctx.t,
                        self.rev_delay,
                        ACTION_REVERSE,
                        0,
                        ctx.self_ref,
                    );
                }
                if self.reverse_pending && fwd_power >= -self.rev_power_threshold {
                    self.reverse_pending = false; // Reset it if power goes back
                    if self.rev_handle > 0 {
                        ctx.queue.delete(self.rev_handle);
                        self.rev_handle = 0;
                    }
                }
            } else {
                // Looking the reverse direction or in cogen mode.
                let fwd_power = -tr.power_into_re(element_terminal, ctx.node_v, ctx.sys);
                if !self.reverse_pending && fwd_power > self.rev_power_threshold {
                    self.reverse_pending = true;
                    self.rev_back_handle = ctx.queue.push_delay(
                        ctx.int_hour,
                        ctx.t,
                        self.rev_delay,
                        ACTION_REVERSE,
                        0,
                        ctx.self_ref,
                    );
                }
                if self.reverse_pending && fwd_power <= self.rev_power_threshold {
                    self.reverse_pending = false;
                    if self.rev_back_handle > 0 {
                        ctx.queue.delete(self.rev_back_handle);
                        self.rev_back_handle = 0;
                    }
                }
                // Reverse-neutral special case: drive the tap to neutral (1.0).
                if self.reverse_neutral {
                    if !self.armed {
                        self.set_pending_tap_change(0.0);
                        let present = tr.present_tap(tap_winding);
                        if (present - 1.0).abs() > EPSILON {
                            let increment = tr.tap_increment(tap_winding);
                            // TODO(compat): FPC banker's `Round`.
                            let ptc = ((1.0 - present) / increment).round_ties_even() * increment;
                            self.set_pending_tap_change(ptc);
                            if self.pending_tap_change != 0.0 && !self.armed {
                                ctx.queue.push_delay(
                                    ctx.int_hour,
                                    ctx.t,
                                    self.tap_delay,
                                    ACTION_TAPCHANGE,
                                    0,
                                    ctx.self_ref,
                                );
                                self.armed = true;
                            }
                        }
                    }
                    return; // Done in any case if reverse-neutral specified.
                }
            }
        }

        // 2) Control voltage.
        let mut vbuffer = vec![Complex64::ZERO; nphases];
        let mut vcontrol = if self.using_regulated_bus {
            let conn = tr.wdg_connection(element_terminal);
            self.ccd.cd.compute_vterminal(ctx.node_v); // voltage at the regulated bus
            for (i, vb) in vbuffer.iter_mut().enumerate().take(nphases) {
                match conn {
                    0 => *vb = self.ccd.cd.vterminal[i], // Wye
                    1 => {
                        // Delta: next phase in sequence.
                        let ii = tr.rotate_phases(i + 1) - 1;
                        *vb = self.ccd.cd.vterminal[i] - self.ccd.cd.vterminal[ii];
                    }
                    _ => ctx.errors.push(format!(
                        "{}: Series connection used in \"Transformer.{}\" has not been implemented or tested!",
                        self.ccd.cd.obj.name(),
                        tr.name()
                    )),
                }
            }
            self.get_control_voltage(&vbuffer, nphases, self.remote_pt_ratio)
        } else {
            tr.winding_voltages(element_terminal, ctx.node_v, &mut vbuffer);
            self.get_control_voltage(&vbuffer, nphases, self.pt_ratio)
        };

        // 3) Vlimit local-bus voltage.
        let vlimit_active = self.vlimit_active();
        let vlocalbus = if vlimit_active {
            if self.using_regulated_bus {
                tr.winding_voltages(element_terminal, ctx.node_v, &mut vbuffer);
                (vbuffer[0] / self.pt_ratio).norm()
            } else {
                vcontrol.norm()
            }
        } else {
            0.0
        };

        // 4) Line-drop compensation.
        if !self.using_regulated_bus && self.ldc_active {
            let nconds = tr.n_conds();
            let mut cbuffer = vec![Complex64::ZERO; tr.y_order()];
            tr.terminal_currents(ctx.node_v, ctx.sys, &mut cbuffer);
            let ildc =
                cbuffer[nconds * (element_terminal - 1) + self.controlled_phase] / self.ct_rating;
            if self.ldc_z == 0.0 {
                // Standard R + jX LDC; ILDC is INTO the terminal → Vterm − (R+jX)·I.
                let vldc = if self.in_reverse_mode || self.in_cogen_mode {
                    Complex64::new(self.rev_r, self.rev_x) * ildc
                } else {
                    Complex64::new(self.r, self.x) * ildc
                };
                vcontrol += vldc;
            } else {
                // Beckwith LDC_Z mode: magnitudes only.
                let z = if self.in_reverse_mode || self.in_cogen_mode {
                    self.rev_ldc_z
                } else {
                    self.ldc_z
                };
                vcontrol = Complex64::new(vcontrol.norm() - ildc.norm() * z, 0.0);
            }
        }

        let mut vactual = vcontrol.norm(); // assumes looking forward; adjusted below

        // 5) Out-of-band test.
        let (vreg_test, band_test) = if self.in_reverse_mode {
            vactual /= tr.present_tap(tap_winding);
            (self.rev_vreg, self.rev_bandwidth)
        } else if self.in_cogen_mode {
            (self.rev_vreg, self.rev_bandwidth)
        } else {
            (self.vreg, self.bandwidth)
        };
        let mut tap_change_needed = (vreg_test - vactual).abs() > band_test / 2.0;
        if vlimit_active && vlocalbus > self.vlimit {
            tap_change_needed = true;
        }

        if tap_change_needed {
            let mut vboost = vreg_test - vactual;
            if vlimit_active && vlocalbus > self.vlimit {
                vboost = self.vlimit - vlocalbus;
            }
            // Per-unit winding boost needed.
            let boost_needed = vboost * self.pt_ratio / tr.base_voltage(element_terminal);
            let increment = tr.tap_increment(tap_winding);
            // TODO(compat): FPC banker's `Round` — this single line decides
            // tap-position equality; `round_ties_even` reproduces it.
            let mut ptc = (boost_needed / increment).round_ties_even() * increment;
            // A tap on another winding or in REVERSE moves the opposite way.
            if (self.tap_winding != self.ccd.element_terminal) || self.in_reverse_mode {
                ptc = -ptc;
            }
            self.set_pending_tap_change(ptc);

            if self.pending_tap_change != 0.0 && !self.armed {
                let present = tr.present_tap(tap_winding);
                // Only arm if a tap change is actually possible in that direction.
                let possible = if self.pending_tap_change > 0.0 {
                    present < tr.max_tap(tap_winding)
                } else {
                    present > tr.min_tap(tap_winding)
                };
                if possible {
                    let delay = self.compute_time_delay(vactual);
                    self.control_action_handle = ctx.queue.push_delay(
                        ctx.int_hour,
                        ctx.t,
                        delay,
                        ACTION_TAPCHANGE,
                        0,
                        ctx.self_ref,
                    );
                    self.armed = true; // Armed to change taps
                }
            }
        } else {
            // Back in band: reset.
            self.set_pending_tap_change(0.0);
            if self.armed {
                ctx.queue.delete(self.control_action_handle);
                self.armed = false;
                self.control_action_handle = 0;
            }
        }
    }

    /// Pascal `TRegControlObj.DoPendingAction` — apply the armed action when its
    /// queue time arrives. `ACTION_TAPCHANGE` applies the pending tap (per
    /// control mode); `ACTION_REVERSE` toggles reverse/cogen mode.
    pub(crate) fn do_pending_action(
        &mut self,
        code: i32,
        tr: &mut dyn ControlledTransformer,
        ctx: &mut CtrlCtx,
    ) {
        match code {
            ACTION_TAPCHANGE => {
                if self.pending_tap_change == 0.0 {
                    // Control has reset since the action was queued.
                    self.armed = false;
                    return;
                }
                let tap_winding = self.tap_winding as usize;
                let increment = tr.tap_increment(tap_winding);
                if ctx.control_mode == CTRLSTATIC {
                    let change = self.at_least_one_tap(self.pending_tap_change, increment);
                    let new_tap = tr.present_tap(tap_winding) + change;
                    if tr.set_present_tap(tap_winding, new_tap) {
                        *ctx.system_y_changed = true;
                    }
                    self.sync_tap_snap(tap_winding, tr.present_tap(tap_winding));
                    if self.ccd.show_event_log {
                        ctx.events.append(
                            &format!("Regulator.{}", tr.name()),
                            &format!(
                                " Changed {} taps to {}.",
                                self.last_change,
                                crate::util::fmt_g(tr.present_tap(tap_winding), 6)
                            ),
                            ctx.int_hour,
                            ctx.t,
                            ctx.control_iter,
                        );
                    }
                    self.set_pending_tap_change(0.0); // program re-determines need
                    self.armed = false;
                } else if matches!(ctx.control_mode, EVENTDRIVEN | TIMEDRIVEN | MULTIRATE) {
                    let change = self.one_in_direction_of(increment);
                    let new_tap = tr.present_tap(tap_winding) + change;
                    if tr.set_present_tap(tap_winding, new_tap) {
                        *ctx.system_y_changed = true;
                    }
                    self.sync_tap_snap(tap_winding, tr.present_tap(tap_winding));
                    if self.ccd.show_event_log {
                        ctx.events.append(
                            &format!("Regulator.{}", tr.name()),
                            &format!(
                                " Changed {} tap to {}.",
                                self.last_change,
                                crate::util::fmt_g(tr.present_tap(tap_winding), 6)
                            ),
                            ctx.int_hour,
                            ctx.t,
                            ctx.control_iter,
                        );
                    }
                    if self.pending_tap_change != 0.0 {
                        ctx.queue.push_delay(
                            ctx.int_hour,
                            ctx.t,
                            self.tap_delay,
                            ACTION_TAPCHANGE,
                            0,
                            ctx.self_ref,
                        );
                    } else {
                        self.armed = false;
                    }
                }
            }
            // Toggle reverse mode or cogen mode flag (only if still pending).
            ACTION_REVERSE if self.reverse_pending => {
                if self.cogen_enabled {
                    // Cogen mode takes precedence if present.
                    self.in_cogen_mode = !self.in_cogen_mode;
                } else {
                    self.in_reverse_mode = !self.in_reverse_mode;
                }
                self.reverse_pending = false;
            }
            _ => {}
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

    // --- Sample / DoPendingAction (WP5.5) ---

    use crate::elements::pd::transformer::ControlledTransformer;
    use crate::solution::{CTRLSTATIC, ControlQueue, EventLog};

    /// A lightweight `ControlledTransformer` returning canned winding voltages
    /// and per-winding tap data, so the regulator decision logic is testable
    /// without a node-wired transformer.
    struct MockTransformer {
        name: String,
        nphases: usize,
        nconds: usize,
        yorder: usize,
        present: Vec<f64>,
        maxt: Vec<f64>,
        mint: Vec<f64>,
        inc: Vec<f64>,
        base_v: Vec<f64>,
        conn: Vec<i32>,
        wv: Vec<Complex64>,
    }

    impl MockTransformer {
        /// A 2-winding wye regulator with the canonical 32-tap range and a
        /// single regulated phase voltage `vph` (volts, secondary base).
        fn wye_2wdg(vph: f64) -> Self {
            Self {
                name: "reg1".into(),
                nphases: 1,
                nconds: 2,
                yorder: 4,
                present: vec![1.0, 1.0],
                maxt: vec![1.1, 1.1],
                mint: vec![0.9, 0.9],
                inc: vec![0.00625, 0.00625],
                base_v: vec![100.0, 100.0],
                conn: vec![0, 0],
                wv: vec![Complex64::new(vph, 0.0)],
            }
        }
    }

    impl ControlledTransformer for MockTransformer {
        fn name(&self) -> &str {
            &self.name
        }
        fn n_phases(&self) -> usize {
            self.nphases
        }
        fn n_conds(&self) -> usize {
            self.nconds
        }
        fn y_order(&self) -> usize {
            self.yorder
        }
        fn wdg_connection(&self, term: usize) -> i32 {
            self.conn[term - 1]
        }
        fn rotate_phases(&self, iphs: usize) -> usize {
            iphs
        }
        fn base_voltage(&self, term: usize) -> f64 {
            self.base_v[term - 1]
        }
        fn present_tap(&self, w: usize) -> f64 {
            self.present[w - 1]
        }
        fn min_tap(&self, w: usize) -> f64 {
            self.mint[w - 1]
        }
        fn max_tap(&self, w: usize) -> f64 {
            self.maxt[w - 1]
        }
        fn tap_increment(&self, w: usize) -> f64 {
            self.inc[w - 1]
        }
        fn set_present_tap(&mut self, w: usize, value: f64) -> bool {
            let v = value.clamp(self.mint[w - 1], self.maxt[w - 1]);
            if v != self.present[w - 1] {
                self.present[w - 1] = v;
                true
            } else {
                false
            }
        }
        fn power_into_re(&mut self, _term: usize, _node_v: &[Complex64], _sys: &SysCtx) -> f64 {
            0.0
        }
        fn winding_voltages(
            &mut self,
            _term: usize,
            _node_v: &[Complex64],
            vbuffer: &mut [Complex64],
        ) {
            for (i, v) in vbuffer.iter_mut().take(self.nphases).enumerate() {
                *v = self.wv[i];
            }
        }
        fn terminal_currents(
            &mut self,
            _node_v: &[Complex64],
            _sys: &SysCtx,
            cbuffer: &mut [Complex64],
        ) {
            cbuffer.fill(Complex64::ZERO);
        }
    }

    /// Build a `CtrlCtx` over freshly-owned queue/event/error/flag scratch.
    struct Scratch {
        queue: ControlQueue,
        events: EventLog,
        errors: Vec<String>,
        y_changed: bool,
        sys: SysCtx,
    }
    impl Scratch {
        fn new() -> Self {
            Self {
                queue: ControlQueue::new(),
                events: EventLog::new(),
                errors: Vec::new(),
                y_changed: false,
                sys: test_sys(),
            }
        }
        fn ctx(&mut self, control_mode: i32) -> CtrlCtx<'_> {
            CtrlCtx {
                node_v: &[],
                sys: &self.sys,
                queue: &mut self.queue,
                events: &mut self.events,
                errors: &mut self.errors,
                system_y_changed: &mut self.y_changed,
                control_mode,
                int_hour: 0,
                t: 0.0,
                dbl_hour: 0.0,
                control_iter: 1,
                self_ref: ElemRef { cls: 0, idx: 0 },
            }
        }
    }

    #[test]
    fn sample_out_of_band_high_arms_a_downward_tap() {
        // Vactual 125 V vs Vreg 120 ± 1.5 → out of band high → boost negative.
        let mut rc = RegControl::new("r1");
        rc.pt_ratio = 1.0;
        rc.ccd.cd.nphases = 1; // regulator senses one phase
        let mut tr = MockTransformer::wye_2wdg(125.0);
        let mut sc = Scratch::new();
        rc.sample(&mut tr, &mut sc.ctx(CTRLSTATIC));

        // boost_needed = (120-125)*1/100 = -0.05; /0.00625 = -8 → -0.05 pu.
        assert!((rc.pending_tap_change - (-0.05)).abs() < 1e-12);
        assert!(rc.armed);
        assert_eq!(sc.queue.queue_size(), 1); // armed ACTION_TAPCHANGE
    }

    #[test]
    fn sample_in_band_disarms_and_clears() {
        let mut rc = RegControl::new("r1");
        rc.pt_ratio = 1.0;
        rc.ccd.cd.nphases = 1;
        // Pre-arm the control as if a prior sample queued a change.
        rc.armed = true;
        rc.control_action_handle = 999;
        let mut tr = MockTransformer::wye_2wdg(120.5); // within ±1.5 of 120
        let mut sc = Scratch::new();
        rc.sample(&mut tr, &mut sc.ctx(CTRLSTATIC));
        assert_eq!(rc.pending_tap_change, 0.0);
        assert!(!rc.armed);
    }

    #[test]
    fn ctrlstatic_action_applies_at_least_one_tap_and_marks_y() {
        let mut rc = RegControl::new("r1");
        rc.pt_ratio = 1.0;
        rc.ccd.cd.nphases = 1;
        let mut tr = MockTransformer::wye_2wdg(125.0);
        let mut sc = Scratch::new();
        rc.sample(&mut tr, &mut sc.ctx(CTRLSTATIC));
        assert!((rc.pending_tap_change - (-0.05)).abs() < 1e-12);

        // CTRLSTATIC moves 70% of the pending change, at least one tap:
        // trunc(0.7*0.05/0.00625) = trunc(5.6) = 5 taps down → −0.03125.
        rc.do_pending_action(ACTION_TAPCHANGE, &mut tr, &mut sc.ctx(CTRLSTATIC));
        assert_eq!(rc.last_change, -5);
        assert!((tr.present_tap(1) - 0.96875).abs() < 1e-12);
        assert!(sc.y_changed);
        assert_eq!(rc.pending_tap_change, 0.0);
        assert!(!rc.armed);
    }

    #[test]
    fn eventdriven_action_moves_one_tap_and_repushes() {
        let mut rc = RegControl::new("r1");
        rc.pt_ratio = 1.0;
        rc.ccd.cd.nphases = 1;
        let mut tr = MockTransformer::wye_2wdg(125.0);
        let mut sc = Scratch::new();
        // Pretend two taps are pending downward.
        rc.set_pending_tap_change(-2.0 * 0.00625);
        rc.do_pending_action(ACTION_TAPCHANGE, &mut tr, &mut sc.ctx(EVENTDRIVEN));
        assert_eq!(rc.last_change, -1); // one tap toward the change
        assert!((tr.present_tap(1) - (1.0 - 0.00625)).abs() < 1e-12);
        assert!((rc.pending_tap_change - (-0.00625)).abs() < 1e-12); // remainder
        assert_eq!(sc.queue.queue_size(), 1); // re-pushed for the next tap
    }

    #[test]
    fn event_log_records_tap_change_when_enabled() {
        let mut rc = RegControl::new("r1");
        rc.pt_ratio = 1.0;
        rc.ccd.cd.nphases = 1;
        rc.ccd.show_event_log = true;
        rc.set_pending_tap_change(-0.05);
        let mut tr = MockTransformer::wye_2wdg(125.0);
        let mut sc = Scratch::new();
        rc.do_pending_action(ACTION_TAPCHANGE, &mut tr, &mut sc.ctx(CTRLSTATIC));
        assert_eq!(sc.events.len(), 1);
        let line = &sc.events.entries()[0];
        assert!(line.contains("Element=Regulator.reg1"));
        assert!(line.contains("CHANGED -5 TAPS TO"));
    }

    #[test]
    fn compute_time_delay_fixed_vs_inverse() {
        let mut rc = RegControl::new("r1");
        rc.ccd.time_delay = 15.0;
        assert_eq!(rc.compute_time_delay(118.0), 15.0); // fixed by default
        rc.inverse_time = true;
        rc.vreg = 120.0;
        rc.bandwidth = 3.0;
        // 2*|120-118|/3 = 1.333 < 10 → 15 / 1.333 = 11.25.
        assert!((rc.compute_time_delay(118.0) - 11.25).abs() < 1e-9);
    }

    #[test]
    fn maxtapchange_zero_zeroes_pending_and_exits() {
        let mut rc = RegControl::new("r1");
        rc.tap_limit_per_change = 0;
        rc.set_pending_tap_change(0.5);
        let mut tr = MockTransformer::wye_2wdg(150.0);
        let mut sc = Scratch::new();
        rc.sample(&mut tr, &mut sc.ctx(CTRLSTATIC));
        assert_eq!(rc.pending_tap_change, 0.0);
        assert!(sc.queue.is_empty());
    }
}

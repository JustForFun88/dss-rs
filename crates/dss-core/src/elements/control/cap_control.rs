//! Port of `Controls/CapControl.pas` — `TCapControlObj`, the capacitor-bank
//! switching control. **Phase 4 ports the parse-time surface only**
//! (properties, `capacitor=`/`element=` resolution, `RecalcElementData`'s
//! bus/phase setup, `MakeLike`); the `Sample`/`DoPendingAction` switching
//! machinery is Phase 5 (PHASE4_PLAN §WP4.7).
//!
//! Like every `TControlElem`, a CapControl builds **no Yprim** and its terminal
//! currents are zero; its single terminal attaches to the monitored element's
//! terminal bus (or the capacitor's, for Time/Follow control types).

use num_complex::Complex64;

use crate::elements::control::control_elem::{
    CTRL_CLOSE, CTRL_NONE, CTRL_OPEN, ControlElemData, CtrlCtx, RefSnapshot,
};
use crate::elements::pd::capacitor::ControlledCapacitor;
use crate::elements::traits::{CktElement, ElemRef, SysCtx};
use crate::obj::base::{DssObjData, DssObject};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

/// `ECapControlType` ordinals.
mod ctrl_type {
    pub const CURRENT: i32 = 0;
    pub const VOLTAGE: i32 = 1;
    pub const KVAR: i32 = 2;
    pub const TIME: i32 = 3;
    pub const PF: i32 = 4;
    pub const FOLLOW: i32 = 5;
}

/// `CapControl.pas` monitored-phase pseudo-phases (the `mon_phase` hybrid enum's
/// avg/max/min, mirrored in `RegControl`).
const AVGPHASES: i32 = -1;
const MAXPHASE: i32 = -2;
const MINPHASE: i32 = -3;

/// 1-based property ordinals (Pascal `TCapControlProp` + class tails).
pub mod prop {
    pub const ELEMENT: usize = 1;
    pub const TERMINAL: usize = 2;
    pub const CAPACITOR: usize = 3;
    pub const TYPE: usize = 4;
    pub const PTRATIO: usize = 5;
    pub const CTRATIO: usize = 6;
    pub const ONSETTING: usize = 7;
    pub const OFFSETTING: usize = 8;
    pub const DELAY: usize = 9;
    pub const VOLTOVERRIDE: usize = 10;
    pub const VMAX: usize = 11;
    pub const VMIN: usize = 12;
    pub const DELAYOFF: usize = 13;
    pub const DEADTIME: usize = 14;
    pub const CTPHASE: usize = 15;
    pub const PTPHASE: usize = 16;
    pub const VBUS: usize = 17;
    pub const EVENTLOG: usize = 18;
    pub const USERMODEL: usize = 19;
    pub const USERDATA: usize = 20;
    pub const PCTMINKVAR: usize = 21;
    pub const RESET: usize = 22;
    pub const CONTROLSIGNAL: usize = 23;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 24;
    pub const ENABLED: usize = 25;
    pub const NUM_PROPS: usize = 26; // incl. Like
}

/// `TCapControl.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    let defs = vec![
        // Pascal `PropertyOffset2 = 0`: any circuit element by full name.
        PropDef::object_ref_any("Element"),
        PropDef::integer("Terminal"),
        // Pascal flags CheckForVar + Required (both inert here).
        PropDef::object_ref_class("Capacitor", "Capacitor"),
        PropDef::mapped_string_enum("Type", enums.cap_control_type),
        PropDef::double("PTRatio"),
        PropDef::double("CTRatio"),
        PropDef::double("OnSetting"),
        PropDef::double("OffSetting"),
        PropDef::double("Delay"),
        PropDef::boolean("VoltOverride"),
        PropDef::double("VMax"),
        PropDef::double("VMin"),
        PropDef::double("DelayOff"),
        PropDef::double("DeadTime"),
        PropDef::mapped_string_enum("CTPhase", enums.mon_phase),
        PropDef::mapped_string_enum("PTPhase", enums.mon_phase),
        PropDef::string("VBus"),
        PropDef::boolean("EventLog"),
        // User-written control DLLs are never ported (no DLL loading in safe
        // Rust — PHASE4_PLAN §5); setting them is a hard error.
        PropDef::string("UserModel").flags(PropFlags::NOT_PORTED | PropFlags::IS_FILENAME),
        PropDef::string("UserData").flags(PropFlags::NOT_PORTED),
        PropDef::double("pctMinkvar"),
        // Pascal: BooleanActionProperty (DoReset); the getter is always 0.
        PropDef::boolean("Reset"),
        // LoadShape references arrive in Phase 5 (WP5.1); `Follow` mode needs it.
        PropDef::object_ref("ControlSignal").flags(PropFlags::NOT_PORTED),
        // TCktElementClass tail:
        PropDef::double("basefreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("enabled"),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);
    ClassProps::new("CapControl", defs, true)
}

/// `TCapControlObj` (+ the parse-relevant `TCapControlVars` fields).
#[derive(Debug, Clone)]
pub struct CapControl {
    pub ccd: ControlElemData,
    /// Dump name of the controlled capacitor (Pascal renders `Name`).
    controlled_name: String,
    /// Dump name of the monitored element (Pascal renders `FullName`).
    monitored_full_name: String,
    /// Parse-time shape snapshots of the two references.
    ctrl_snap: Option<RefSnapshot>,
    mon_snap: Option<RefSnapshot>,

    /// `ECapControlType` ordinal (0=Current ... 5=Follow).
    control_type: i32,
    fct_phase: i32,
    fpt_phase: i32,
    pt_ratio: f64,
    ct_ratio: f64,
    on_value: f64,
    off_value: f64,
    pfon_value: f64,
    pfoff_value: f64,
    on_delay: f64,
    off_delay: f64,
    dead_time: f64,
    last_open_time: f64,
    voverride: bool,
    voverride_bus_specified: bool,
    voverride_bus_name: String,
    vmax: f64,
    vmin: f64,
    fpct_minkvar: f64,
    // Runtime switching state (`TCapControlVars`), driven by `Sample`/
    // `DoPendingAction` (wired into the control loop in WP5.7):
    /// `FPendingChange` (CTRL_NONE/OPEN/CLOSE).
    pending_change: i32,
    /// `ShouldSwitch`: an action is pending.
    should_switch: bool,
    /// `Armed`: a queue action is outstanding (deleted on disarm).
    armed: bool,
    /// `PresentState`/`InitialState` (CTRL_OPEN/CTRL_CLOSE).
    present_state: i32,
    initial_state: i32,
    /// `VoverrideEvent`.
    voverride_event: bool,
    /// `ControlActionHandle` (the queue handle to delete when disarming).
    control_action_handle: i32,
}

impl CapControl {
    /// Pascal `TCapControlObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut ccd = ControlElemData::new(name, prop::NUM_PROPS);
        ccd.cd.nphases = 3;
        ccd.cd.nconds = 3;
        ccd.cd.set_nterms(1); // forces allocation of terminals and conductors
        ccd.element_terminal = 1;

        let dead_time = 300.0;
        Self {
            ccd,
            controlled_name: String::new(),
            monitored_full_name: String::new(),
            ctrl_snap: None,
            mon_snap: None,
            control_type: ctrl_type::CURRENT,
            fct_phase: 1,
            fpt_phase: 1,
            pt_ratio: 60.0,
            ct_ratio: 60.0,
            on_value: 300.0,
            off_value: 200.0,
            pfon_value: 0.95,
            pfoff_value: 1.05,
            on_delay: 15.0,
            off_delay: 15.0,
            dead_time,
            last_open_time: -dead_time,
            voverride: false,
            voverride_bus_specified: false,
            voverride_bus_name: String::new(),
            vmax: 126.0,
            vmin: 115.0,
            fpct_minkvar: 50.0,
            pending_change: CTRL_NONE,
            should_switch: false,
            armed: false,
            present_state: CTRL_CLOSE,
            initial_state: CTRL_CLOSE,
            voverride_event: false,
            control_action_handle: 0,
        }
    }

    /// Pascal `TCapControlObj.Reset` (the `Reset` action property). The
    /// `ControlledElement.Closed[0] := InitialState` restore needs the
    /// controlled capacitor, which the property setter cannot reach; it is
    /// applied by the control-loop reset path ([`Self::reset_with`]) — here we
    /// restore the control's own switching state.
    fn reset(&mut self) {
        self.set_pending_change(CTRL_NONE);
        self.should_switch = false;
        self.armed = false;
        self.last_open_time = -self.dead_time;
        self.present_state = self.initial_state;
    }

    /// The full Pascal `Reset` (the `DoResetControls` path): restore the
    /// control state *and* drive the bank back to `InitialState`. Returns
    /// whether the bank's switch state changed (the caller raises
    /// `SystemYChanged`, Pascal's `Set_ConductorClosed` side effect).
    pub(crate) fn reset_with(&mut self, cap: &mut dyn ControlledCapacitor) -> bool {
        let was_closed = cap.is_closed();
        let want_closed = match self.initial_state {
            CTRL_OPEN => Some(false),
            CTRL_CLOSE => Some(true),
            _ => None,
        };
        if let Some(want) = want_closed {
            cap.set_closed(want);
        }
        self.reset();
        want_closed.is_some_and(|want| want != was_closed)
    }

    /// Pascal `Set_PendingChange` (also mirrors to `DblTraceParameter`).
    fn set_pending_change(&mut self, value: i32) {
        self.pending_change = value;
        self.ccd.dbl_trace_param = value as f64;
    }

    /// Pascal `TSolutionObj.TimeOfDay(useEpsilon = true)`: normalize the
    /// simulation time to a 0:00⁺…24:00 time-of-day, wrapping past 24 h.
    fn time_of_day_eps(int_hour: i32, t: f64) -> f64 {
        let h = int_hour;
        let hour_of_day = if h > 24 { h - ((h - 1) / 24) * 24 } else { h };
        let result = hour_of_day as f64 + t / 3600.0;
        if result - 24.0 > crate::util::EPSILON {
            result - 24.0
        } else {
            result
        }
    }

    /// Pascal `GetControlCurrent`: the control current from `cbuffer` (the
    /// monitored element's terminal currents) per `FCTphase`, divided by the CT
    /// ratio. `cond_offset` is the 0-based start of the monitored terminal's
    /// conductors.
    fn get_control_current(&self, cbuffer: &[Complex64], cond_offset: usize) -> f64 {
        let nph = self.ccd.cd.nphases; // Fnphases
        match self.fct_phase {
            AVGPHASES => {
                let mut c = 0.0;
                for i in 0..nph {
                    c += cbuffer[cond_offset + i].norm();
                }
                c / nph as f64 / self.ct_ratio
            }
            MAXPHASE => {
                let mut c = 0.0_f64;
                for i in 0..nph {
                    c = c.max(cbuffer[cond_offset + i].norm());
                }
                c / self.ct_ratio
            }
            MINPHASE => {
                let mut c = 1.0e50_f64;
                for i in 0..nph {
                    c = c.min(cbuffer[cond_offset + i].norm());
                }
                c / self.ct_ratio
            }
            // Just one phase (the monitored phase) — note: Pascal uses no
            // CondOffset on this default branch.
            _ => cbuffer[(self.fct_phase as usize) - 1].norm() / self.ct_ratio,
        }
    }

    /// Pascal `GetControlVoltage`: the control voltage from `cbuffer` (the
    /// monitored element's terminal voltages) per `FPTphase`, divided by the PT
    /// ratio. The specific-phase branch uses the controlled capacitor's
    /// connection (delta ⇒ line-line difference).
    fn get_control_voltage(&self, cbuffer: &[Complex64], mon_nphases: usize, cap_conn: i32) -> f64 {
        match self.fpt_phase {
            AVGPHASES => {
                let mut v = 0.0;
                for vb in cbuffer.iter().take(mon_nphases) {
                    v += vb.norm();
                }
                v / mon_nphases as f64 / self.pt_ratio
            }
            MAXPHASE => {
                let mut v = 0.0_f64;
                for vb in cbuffer.iter().take(mon_nphases) {
                    v = v.max(vb.norm());
                }
                v / self.pt_ratio
            }
            MINPHASE => {
                let mut v = 1.0e50_f64;
                for vb in cbuffer.iter().take(mon_nphases) {
                    v = v.min(vb.norm());
                }
                v / self.pt_ratio
            }
            // Just one phase; L-L if the capacitor is delta-connected.
            _ => {
                let p = self.fpt_phase as usize; // 1-based
                if cap_conn == 1 {
                    // NextDeltaPhase uses the control's own Fnphases.
                    let mut next = p + 1;
                    if next > self.ccd.cd.nphases {
                        next = 1;
                    }
                    (cbuffer[p - 1] - cbuffer[next - 1]).norm() / self.pt_ratio
                } else {
                    cbuffer[p - 1].norm() / self.pt_ratio
                }
            }
        }
    }

    /// Pascal `TCapControlObj.Sample` — sense the monitored quantity for the
    /// control type, decide whether the bank should switch, and arm/disarm the
    /// control queue. `cap` is the controlled capacitor; `mon` the monitored
    /// element (they differ except for Time/Follow control, where `mon` is the
    /// capacitor and is not read). Ported top-to-bottom.
    pub(crate) fn sample(
        &mut self,
        cap: &mut dyn ControlledCapacitor,
        mon: &mut dyn CktElement,
        ctx: &mut CtrlCtx,
    ) {
        // ControlledElement.ActiveTerminalIdx := 1 (terminal 1 is implicit).
        self.present_state = if cap.is_closed() {
            CTRL_CLOSE
        } else {
            CTRL_OPEN
        };
        self.should_switch = false;

        let now = ctx.t + ctx.int_hour as f64 * 3600.0;
        let element_terminal = self.ccd.element_terminal as usize;
        let mon_nconds = mon.cd().nconds;
        let mon_nphases = mon.cd().nphases;
        let cond_offset = (element_terminal - 1) * mon_nconds;
        let mut cbuffer = vec![Complex64::ZERO; mon.cd().yorder.max(1)];

        // First, voltage override (skipped for VOLTAGECONTROL).
        if self.voverride && self.control_type != ctrl_type::VOLTAGE {
            // VoverrideBusSpecified is always reverted at parse (the bus list
            // does not yet exist — PHASE4 §WP4.7), so the GetBusVoltages variant
            // is unreachable here; sense the monitored terminal instead.
            mon.get_term_voltages(element_terminal, ctx.node_v, &mut cbuffer);
            let vtest = self.get_control_voltage(&cbuffer, mon_nphases, cap.connection());

            // Faithful to Pascal's `case PresentState of CTRL_x: if … then`;
            // a match guard would obscure the ported `case`/`if` structure.
            #[allow(clippy::collapsible_match)]
            match self.present_state {
                CTRL_OPEN => {
                    if vtest < self.vmin {
                        self.set_pending_change(CTRL_CLOSE);
                        self.should_switch = true;
                        self.voverride_event = true;
                        if self.ccd.show_event_log {
                            ctx.events.append(
                                &cap.full_name(),
                                &format!(
                                    "Low Voltage Override: {} V",
                                    crate::util::fmt_g(vtest, 8)
                                ),
                                ctx.int_hour,
                                ctx.t,
                                ctx.control_iter,
                            );
                        }
                    }
                }
                CTRL_CLOSE => {
                    if vtest > self.vmax {
                        self.set_pending_change(CTRL_OPEN);
                        self.should_switch = true;
                        self.voverride_event = true;
                        if self.ccd.show_event_log {
                            ctx.events.append(
                                &cap.full_name(),
                                &format!(
                                    "High Voltage Override: {} V",
                                    crate::util::fmt_g(vtest, 8)
                                ),
                                ctx.int_hour,
                                ctx.t,
                                ctx.control_iter,
                            );
                        }
                    }
                }
                _ => {}
            }
        }

        if !self.should_switch {
            match self.control_type {
                ctrl_type::CURRENT => {
                    mon.get_currents(ctx.sys, ctx.node_v, &mut cbuffer);
                    let curr_test = self.get_control_current(&cbuffer, cond_offset);
                    match self.present_state {
                        CTRL_OPEN => {
                            if curr_test > self.on_value {
                                self.set_pending_change(CTRL_CLOSE);
                                self.should_switch = true;
                            } else {
                                self.set_pending_change(CTRL_NONE);
                            }
                        }
                        CTRL_CLOSE => {
                            if curr_test < self.off_value {
                                self.set_pending_change(CTRL_OPEN);
                                self.should_switch = true;
                            } else if cap.available_steps() > 0 {
                                if curr_test > self.on_value {
                                    self.set_pending_change(CTRL_CLOSE);
                                    self.should_switch = true;
                                }
                            } else {
                                self.set_pending_change(CTRL_NONE);
                            }
                        }
                        _ => {}
                    }
                }
                ctrl_type::VOLTAGE => {
                    mon.get_term_voltages(element_terminal, ctx.node_v, &mut cbuffer);
                    let vtest = self.get_control_voltage(&cbuffer, mon_nphases, cap.connection());
                    match self.present_state {
                        CTRL_OPEN => {
                            if vtest < self.on_value {
                                self.set_pending_change(CTRL_CLOSE);
                                self.should_switch = true;
                            } else {
                                self.set_pending_change(CTRL_NONE);
                            }
                        }
                        CTRL_CLOSE => {
                            self.set_pending_change(CTRL_NONE);
                            if vtest > self.off_value {
                                self.set_pending_change(CTRL_OPEN);
                                self.should_switch = true;
                            } else if cap.available_steps() > 0 && vtest < self.on_value {
                                self.set_pending_change(CTRL_CLOSE);
                                self.should_switch = true;
                            }
                        }
                        _ => {}
                    }
                }
                ctrl_type::KVAR => {
                    let s = mon.terminal_power(ctx.sys, ctx.node_v, element_terminal);
                    let q = s.im * 0.001; // kvar
                    match self.present_state {
                        CTRL_OPEN => {
                            if q > self.on_value {
                                self.set_pending_change(CTRL_CLOSE);
                                self.should_switch = true;
                            } else {
                                self.set_pending_change(CTRL_NONE);
                            }
                        }
                        CTRL_CLOSE => {
                            if q < self.off_value {
                                self.set_pending_change(CTRL_OPEN);
                                self.should_switch = true;
                            } else if cap.available_steps() > 0 {
                                if q > self.on_value {
                                    self.set_pending_change(CTRL_CLOSE);
                                    self.should_switch = true;
                                }
                            } else {
                                self.set_pending_change(CTRL_NONE);
                            }
                        }
                        _ => {}
                    }
                }
                ctrl_type::TIME => {
                    let normalized_time = Self::time_of_day_eps(ctx.int_hour, ctx.t);
                    self.sample_time_control(normalized_time, cap.available_steps());
                }
                ctrl_type::PF => {
                    let s = mon.terminal_power(ctx.sys, ctx.node_v, element_terminal);
                    let pf = pf_1to2(s);
                    match self.present_state {
                        CTRL_OPEN => {
                            // Make sure we don't go too far leading.
                            if pf < self.pfon_value
                                && s.im * 0.001 > cap.total_kvar() * self.fpct_minkvar * 0.01
                            {
                                self.set_pending_change(CTRL_CLOSE);
                                self.should_switch = true;
                            } else {
                                self.set_pending_change(CTRL_NONE);
                            }
                        }
                        CTRL_CLOSE => {
                            if pf > self.pfoff_value {
                                self.set_pending_change(CTRL_OPEN);
                                self.should_switch = true;
                            } else if cap.available_steps() > 0 {
                                if pf < self.pfon_value
                                    && s.im * 0.001
                                        > cap.total_kvar() / cap.num_steps() as f64 * 0.5
                                {
                                    self.set_pending_change(CTRL_CLOSE);
                                    self.should_switch = true;
                                }
                            } else {
                                self.set_pending_change(CTRL_NONE);
                            }
                        }
                        _ => {}
                    }
                }
                ctrl_type::FOLLOW => {
                    // FOLLOWCONTROL needs ControlSignal (LoadShape), which is
                    // NOT_PORTED (PHASE4 §WP4.7); Pascal aborts the solution when
                    // it is unset, which is always the case here.
                    ctx.errors.push(format!(
                        "CapControl.{}: Type is set to \"Follow\", but no \"ControlSignal\" was provided. Aborting solution.",
                        self.ccd.cd.obj.name()
                    ));
                }
                _ => {}
            }
        }

        // Arm / disarm the control queue.
        if self.should_switch && !self.armed {
            let time_delay = if self.pending_change == CTRL_CLOSE {
                if (now - self.last_open_time) < self.dead_time {
                    // Delay the close until the dead time has elapsed.
                    self.on_delay
                        .max((self.dead_time + self.on_delay) - (now - self.last_open_time))
                } else {
                    self.on_delay
                }
            } else {
                self.off_delay
            };
            self.ccd.time_delay = time_delay;
            self.control_action_handle = ctx.queue.push_delay(
                ctx.int_hour,
                ctx.t,
                time_delay,
                self.pending_change,
                0,
                ctx.self_ref,
            );
            self.armed = true;
            if self.ccd.show_event_log {
                ctx.events.append(
                    &cap.full_name(),
                    &format!(
                        "**Armed**, Delay= {} sec",
                        crate::util::fmt_g(time_delay, 5)
                    ),
                    ctx.int_hour,
                    ctx.t,
                    ctx.control_iter,
                );
            }
        }

        if self.armed && self.pending_change == CTRL_NONE {
            ctx.queue.delete(self.control_action_handle);
            self.armed = false;
            if self.ccd.show_event_log {
                ctx.events.append(
                    &cap.full_name(),
                    "**Reset**",
                    ctx.int_hour,
                    ctx.t,
                    ctx.control_iter,
                );
            }
        }
    }

    /// Pascal `Sample`'s `TIMECONTROL` branch (factored out for readability):
    /// compare the time-of-day against the on/off window.
    fn sample_time_control(&mut self, normalized_time: f64, available_steps: i32) {
        match self.present_state {
            CTRL_OPEN => {
                let close = if self.off_value > self.on_value {
                    normalized_time >= self.on_value && normalized_time < self.off_value
                } else {
                    // OFF time is next day.
                    normalized_time >= self.on_value && normalized_time < 24.0
                };
                if close {
                    self.set_pending_change(CTRL_CLOSE);
                    self.should_switch = true;
                } else {
                    self.set_pending_change(CTRL_NONE);
                }
            }
            CTRL_CLOSE => {
                if self.off_value > self.on_value {
                    if normalized_time >= self.off_value || normalized_time < self.on_value {
                        self.set_pending_change(CTRL_OPEN);
                        self.should_switch = true;
                    } else if available_steps > 0
                        && normalized_time >= self.on_value
                        && normalized_time < self.off_value
                    {
                        self.set_pending_change(CTRL_CLOSE);
                        self.should_switch = true;
                    }
                    // (no else-reset branch in the OFF>ON close path)
                } else {
                    // OFF time is next day.
                    if normalized_time >= self.off_value && normalized_time < self.on_value {
                        self.set_pending_change(CTRL_OPEN);
                        self.should_switch = true;
                    } else if available_steps > 0
                        && normalized_time >= self.on_value
                        && normalized_time < 24.0
                    {
                        self.set_pending_change(CTRL_CLOSE);
                        self.should_switch = true;
                    } else {
                        self.set_pending_change(CTRL_NONE);
                    }
                }
            }
            _ => {}
        }
    }

    /// Pascal `TCapControlObj.DoPendingAction` — switch the controlled bank when
    /// the queued action's time arrives (open/close or step up/down), then
    /// disarm. Marks `system_y_changed` whenever the bank's admittance changes.
    pub(crate) fn do_pending_action(
        &mut self,
        cap: &mut dyn ControlledCapacitor,
        ctx: &mut CtrlCtx,
    ) {
        // ControlledElement.ActiveTerminalIdx := 1 (terminal 1 is implicit).
        match self.pending_change {
            CTRL_OPEN => {
                if cap.num_steps() == 1 {
                    if self.present_state == CTRL_CLOSE {
                        cap.set_closed(false); // open all phases
                        cap.subtract_step();
                        *ctx.system_y_changed = true;
                        if self.ccd.show_event_log {
                            ctx.events.append(
                                &cap.full_name(),
                                "**Opened**",
                                ctx.int_hour,
                                ctx.t,
                                ctx.control_iter,
                            );
                        }
                        self.present_state = CTRL_OPEN;
                        self.last_open_time = ctx.t + 3600.0 * ctx.int_hour as f64;
                    }
                } else if self.present_state == CTRL_CLOSE {
                    // Multi-step: step down (do this only if at least one closed).
                    if !cap.subtract_step() {
                        self.present_state = CTRL_OPEN;
                        cap.set_closed(false); // open all phases
                        if self.ccd.show_event_log {
                            ctx.events.append(
                                &cap.full_name(),
                                "**Opened**",
                                ctx.int_hour,
                                ctx.t,
                                ctx.control_iter,
                            );
                        }
                    } else if self.ccd.show_event_log {
                        ctx.events.append(
                            &cap.full_name(),
                            "**Step Down**",
                            ctx.int_hour,
                            ctx.t,
                            ctx.control_iter,
                        );
                    }
                    *ctx.system_y_changed = true;
                }
            }
            CTRL_CLOSE => {
                if self.present_state == CTRL_OPEN {
                    cap.set_closed(true); // close all phases
                    if self.ccd.show_event_log {
                        ctx.events.append(
                            &cap.full_name(),
                            "**Closed**",
                            ctx.int_hour,
                            ctx.t,
                            ctx.control_iter,
                        );
                    }
                    self.present_state = CTRL_CLOSE;
                    cap.add_step();
                    *ctx.system_y_changed = true;
                } else if cap.add_step() {
                    *ctx.system_y_changed = true;
                    if self.ccd.show_event_log {
                        ctx.events.append(
                            &cap.full_name(),
                            "**Step Up**",
                            ctx.int_hour,
                            ctx.t,
                            ctx.control_iter,
                        );
                    }
                }
            }
            _ => {}
        }

        self.voverride_event = false;
        self.should_switch = false;
        self.armed = false;
    }

    /// Pascal `TCapControlObj.RecalcElementData` (parse-time subset).
    fn recalc(&mut self) {
        // Check for existence of capacitor.
        if self.ccd.controlled_element.is_none() {
            // Pascal raises here (surfaces as command error 303 in the oracle).
            self.ccd.cd.obj.push_error(format!(
                "\"CapControl.{}\": Capacitor is not set, aborting.",
                self.ccd.cd.obj.name()
            ));
            return;
        }
        let ctrl = self.ctrl_snap.clone().unwrap_or_default();

        // Force number of phases to be same as the controlled capacitor.
        self.ccd.cd.nphases = ctrl.nphases;
        self.ccd.cd.set_nconds(ctrl.nphases);
        // Pascal syncs `ControlledElement.Closed[0]` with AvailableSteps and
        // derives PresentState/InitialState here — control actions are Phase 5
        // and capacitor terminals start (and stay) closed during parse.

        let eff = if self.control_type != ctrl_type::TIME && self.control_type != ctrl_type::FOLLOW
        {
            if self.mon_snap.is_none() {
                self.ccd.cd.obj.push_error(format!(
                    "CapControl.{}: Element is not set, aborting.",
                    self.ccd.cd.obj.name()
                ));
                return;
            }
            self.mon_snap.clone().unwrap_or_default()
        } else {
            // force terminal to 1 if no monitored element is provided
            self.ccd.element_terminal = 1;
            ctrl
        };

        if self.ccd.element_terminal > eff.nterms as i32 {
            // DoErrorMsg 362.
            self.ccd.cd.obj.push_error(format!(
                "CapControl.{}: Terminal number {} does not exist in \"{}\". Re-specify terminal number. (Error 362)",
                self.ccd.cd.obj.name(),
                self.ccd.element_terminal,
                eff.full_name
            ));
            return;
        }

        // Sets the name of the 1st terminal's connected bus.
        let t = self.ccd.element_terminal;
        let bus = if t >= 1 && (t as usize) <= eff.buses.len() {
            eff.buses[(t - 1) as usize].clone()
        } else {
            String::new() // Pascal GetBus(i) out of range yields ''
        };
        self.ccd.cd.set_bus(1, &bus);
        // cBuffer/CondOffset (sampling buffers) are Phase 5.

        // Alternative override bus: the circuit bus list is only built at
        // solve time, so during parsing the lookup always misses — exactly as
        // in Pascal, which warns and reverts to the default.
        if self.voverride_bus_specified {
            self.ccd.cd.obj.push_error(format!(
                "CapControl.{}: Voltage override Bus \"{}\" not found. Did you wait until buses were defined? Reverting to default.",
                self.ccd.cd.obj.name(),
                self.voverride_bus_name
            ));
            self.voverride_bus_specified = false;
        }
    }
}

impl CktElement for CapControl {
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

impl DssObject for CapControl {
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
            PTRATIO => self.pt_ratio,
            CTRATIO => self.ct_ratio,
            ONSETTING => self.on_value,
            OFFSETTING => self.off_value,
            DELAY => self.on_delay,
            VMAX => self.vmax,
            VMIN => self.vmin,
            DELAYOFF => self.off_delay,
            DEADTIME => self.dead_time,
            PCTMINKVAR => self.fpct_minkvar,
            BASE_FREQ => self.ccd.cd.base_frequency,
            _ => unreachable!("CapControl has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            PTRATIO => self.pt_ratio = value,
            CTRATIO => self.ct_ratio = value,
            ONSETTING => self.on_value = value,
            OFFSETTING => self.off_value = value,
            DELAY => self.on_delay = value,
            VMAX => self.vmax = value,
            VMIN => self.vmin = value,
            DELAYOFF => self.off_delay = value,
            DEADTIME => self.dead_time = value,
            PCTMINKVAR => self.fpct_minkvar = value,
            BASE_FREQ => self.ccd.cd.base_frequency = value,
            _ => unreachable!("CapControl has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use prop::*;
        match idx {
            TERMINAL => self.ccd.element_terminal,
            TYPE => self.control_type,
            CTPHASE => self.fct_phase,
            PTPHASE => self.fpt_phase,
            _ => unreachable!("CapControl has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            TERMINAL => self.ccd.element_terminal = value,
            TYPE => self.control_type = value,
            CTPHASE => self.fct_phase = value,
            PTPHASE => self.fpt_phase = value,
            _ => unreachable!("CapControl has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use prop::*;
        match idx {
            VOLTOVERRIDE => self.voverride,
            EVENTLOG => self.ccd.show_event_log,
            RESET => false, // Pascal BooleanActionProperty getter: always 0
            ENABLED => self.ccd.cd.enabled,
            _ => unreachable!("CapControl has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use prop::*;
        match idx {
            VOLTOVERRIDE => self.voverride = value,
            EVENTLOG => self.ccd.show_event_log = value,
            RESET => {
                // Pascal BooleanActionProperty: the action fires on TRUE only.
                if value {
                    self.reset();
                }
            }
            // Pascal `TCapControlObj.Set_Enabled` override: only toggle the
            // flag — no BusNameRedefined side effect.
            ENABLED => self.ccd.cd.enabled = value,
            _ => unreachable!("CapControl has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use prop::*;
        match idx {
            ELEMENT => self.monitored_full_name.clone(),
            CAPACITOR => self.controlled_name.clone(),
            VBUS => self.voverride_bus_name.clone(),
            // NOT_PORTED user-model / control-signal slots dump as empty (NIL).
            USERMODEL | USERDATA | CONTROLSIGNAL => String::new(),
            _ => unreachable!("CapControl has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        match idx {
            prop::VBUS => self.voverride_bus_name = value,
            _ => unreachable!("CapControl has no string property {idx}"),
        }
    }

    /// `capacitor=` / `element=` resolution: keep the `ElemRef`s plus shape
    /// snapshots for `RecalcElementData` (which runs after the foreign view is
    /// gone).
    fn set_object_ref(
        &mut self,
        idx: usize,
        name: String,
        resolved: Option<(ElemRef, &dyn DssObject)>,
    ) {
        use prop::*;
        match idx {
            CAPACITOR => {
                self.controlled_name = name;
                match resolved {
                    Some((r, obj)) => {
                        self.ccd.controlled_element = Some(r);
                        let elem = obj
                            .as_ckt_element()
                            .expect("Capacitor is a circuit element");
                        self.ctrl_snap = Some(RefSnapshot::capture(
                            format!("Capacitor.{}", obj.data().name()),
                            elem,
                        ));
                    }
                    None => {
                        self.ccd.controlled_element = None;
                        self.ctrl_snap = None;
                    }
                }
            }
            ELEMENT => {
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
            _ => unreachable!("CapControl has no object-ref property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.ccd.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.ccd.cd.get_bus(terminal).to_string()
    }

    /// Pascal `TCapControlObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        use prop::*;
        // PF Controller changes (the type has already been written when the
        // `typ` side effect runs, so this covers "switched to PF" too).
        if self.control_type == ctrl_type::PF {
            match idx {
                TYPE => {
                    self.pfon_value = 0.95; // defaults
                    self.pfoff_value = 1.05;
                }
                ONSETTING => {
                    if (-1.0..=1.0).contains(&self.on_value) {
                        self.pfon_value = if self.on_value < 0.0 {
                            2.0 + self.on_value
                        } else {
                            self.on_value
                        };
                    } else {
                        self.ccd.cd.obj.push_error(format!(
                            "Invalid PF ON value for \"CapControl.{}\"",
                            self.ccd.cd.obj.name()
                        ));
                    }
                }
                OFFSETTING => {
                    if (-1.0..=1.0).contains(&self.off_value) {
                        self.pfoff_value = if self.off_value < 0.0 {
                            2.0 + self.off_value
                        } else {
                            self.off_value
                        };
                    } else {
                        self.ccd.cd.obj.push_error(format!(
                            "Invalid PF OFF value for \"CapControl.{}\"",
                            self.ccd.cd.obj.name()
                        ));
                    }
                }
                _ => {}
            }
        }

        match idx {
            CTPHASE => {
                if self.fct_phase > self.ccd.cd.nphases as i32 {
                    self.ccd.cd.obj.push_error(format!(
                        "Error: Monitored phase ({}) must be less than or equal to number of phases ({}). ",
                        self.fct_phase, self.ccd.cd.nphases
                    ));
                    self.fct_phase = 1;
                }
            }
            PTPHASE => {
                if self.fpt_phase > self.ccd.cd.nphases as i32 {
                    self.ccd.cd.obj.push_error(format!(
                        "Error: Monitored phase ({}) must be less than or equal to number of phases ({}). ",
                        self.fpt_phase, self.ccd.cd.nphases
                    ));
                    self.fpt_phase = 1;
                }
            }
            // CAPACITOR: Pascal stores ControlVars.CapacitorName :=
            // ControlledElement.FullName for Save; Save is not ported.
            VBUS => {
                self.voverride_bus_name = self.voverride_bus_name.to_lowercase();
                self.voverride_bus_specified = true;
            }
            // USERMODEL/USERDATA are NOT_PORTED (hard parse error upstream of
            // this hook), so the user-model wiring never runs.
            _ => {}
        }
    }

    /// Pascal `TCktElementClass.EndEdit` default → `RecalcElementData`.
    fn end_edit(&mut self) {
        self.recalc();
    }

    /// Pascal `TCapControlObj.MakeLike`.
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(other) = other.as_any().downcast_ref::<CapControl>() else {
            return;
        };
        self.ccd.cd.make_like_base(&other.ccd.cd);
        self.ccd.cd.nphases = other.ccd.cd.nphases;
        let nc = other.ccd.cd.nconds;
        self.ccd.cd.set_nconds(nc); // Force Reallocation of terminal stuff

        self.ccd.controlled_element = other.ccd.controlled_element;
        self.ccd.monitored_element = other.ccd.monitored_element;
        self.controlled_name = other.controlled_name.clone();
        self.monitored_full_name = other.monitored_full_name.clone();
        self.ctrl_snap = other.ctrl_snap.clone();
        self.mon_snap = other.mon_snap.clone();

        self.ccd.element_terminal = other.ccd.element_terminal;
        self.pt_ratio = other.pt_ratio;
        self.ct_ratio = other.ct_ratio;
        self.control_type = other.control_type;
        self.present_state = other.present_state;
        self.should_switch = other.should_switch;
        self.on_value = other.on_value;
        self.off_value = other.off_value;
        self.pfon_value = other.pfon_value;
        self.pfoff_value = other.pfoff_value;
        self.fct_phase = other.fct_phase;
        self.fpt_phase = other.fpt_phase;
        self.voverride = other.voverride;
        self.voverride_bus_specified = other.voverride_bus_specified;
        self.voverride_bus_name = other.voverride_bus_name.clone();
        self.fpct_minkvar = other.fpct_minkvar;
        self.ccd.show_event_log = other.ccd.show_event_log;
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

/// Pascal `Sample`'s local `PF1to2`: power factor mapped onto `0..2` with the
/// leading range `1..2` (`im < 0` ⇒ `2 − PF`); unity when the apparent power is
/// zero.
fn pf_1to2(s: Complex64) -> f64 {
    let sabs = s.norm();
    let mut result = if sabs != 0.0 { s.re.abs() / sabs } else { 1.0 };
    if s.im < 0.0 {
        result = 2.0 - result;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_shape_is_3ph_1term() {
        let cc = CapControl::new("cc1");
        assert_eq!(cc.ccd.cd.nphases, 3);
        assert_eq!(cc.ccd.cd.nconds, 3);
        assert_eq!(cc.ccd.cd.nterms, 1);
        assert_eq!(cc.control_type, 0); // Current
        assert_eq!(cc.pt_ratio, 60.0);
        assert_eq!(cc.ct_ratio, 60.0);
        assert_eq!(cc.on_value, 300.0);
        assert_eq!(cc.off_value, 200.0);
        assert_eq!(cc.on_delay, 15.0);
        assert_eq!(cc.off_delay, 15.0);
        assert_eq!(cc.dead_time, 300.0);
        assert_eq!(cc.vmax, 126.0);
        assert_eq!(cc.vmin, 115.0);
        assert_eq!(cc.fpct_minkvar, 50.0);
        assert!(cc.ccd.cd.yprim.is_none());
    }

    #[test]
    fn time_control_forces_terminal_1() {
        // Probed: `type=time terminal=2` dumps Terminal = 1.
        let mut cc = CapControl::new("cc1");
        cc.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
        cc.ctrl_snap = Some(RefSnapshot {
            full_name: "Capacitor.cap1".into(),
            nphases: 3,
            nterms: 2,
            buses: vec!["b2.1.2.3".into(), "b2.0.0.0".into()],
        });
        cc.control_type = ctrl_type::TIME;
        cc.ccd.element_terminal = 2;
        cc.recalc();
        assert_eq!(cc.ccd.element_terminal, 1);
        assert_eq!(cc.ccd.cd.get_bus(1), "b2.1.2.3");
        assert!(cc.ccd.cd.obj.take_errors().is_empty());
    }

    #[test]
    fn pf_mode_translates_on_off_settings() {
        // Probed: type=pf onsetting=0.97 offsetting=-0.99 keeps the raw dump
        // values; internally PFON=0.97, PFOFF=2-0.99=1.01.
        let mut cc = CapControl::new("cc1");
        cc.control_type = ctrl_type::PF;
        cc.side_effects(prop::TYPE, 0);
        assert_eq!(cc.pfon_value, 0.95);
        assert_eq!(cc.pfoff_value, 1.05);
        cc.on_value = 0.97;
        cc.side_effects(prop::ONSETTING, 0);
        assert!((cc.pfon_value - 0.97).abs() < 1e-12);
        cc.off_value = -0.99;
        cc.side_effects(prop::OFFSETTING, 0);
        assert!((cc.pfoff_value - 1.01).abs() < 1e-12);
        assert_eq!(cc.on_value, 0.97);
        assert_eq!(cc.off_value, -0.99);
    }

    // --- Sample / DoPendingAction (WP5.6) ---

    use crate::elements::ckt::CktElementData;
    use crate::solution::{ControlQueue, EventLog, SolveMode};

    fn test_sys() -> SysCtx {
        SysCtx {
            frequency: 60.0,
            fundamental: 60.0,
            is_harmonic_model: false,
            is_dynamic_model: false,
            load_model: 1,
            mode: SolveMode::Snapshot,
            load_multiplier: 1.0,
            gen_multiplier: 1.0,
            generator_dispatch_reference: 0.0,
            price_signal: 25.0,
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

    /// A controlled capacitor stub: holds the bank's step/closed state and the
    /// scalars CapControl reads, with the Pascal `AddStep`/`SubtractStep`
    /// semantics.
    struct MockCap {
        name: String,
        num_steps: i32,
        last_step: i32,
        total_kvar: f64,
        conn: i32,
        closed: bool,
    }
    impl MockCap {
        fn one_step(closed: bool) -> Self {
            Self {
                name: "cc".into(),
                num_steps: 1,
                last_step: if closed { 1 } else { 0 },
                total_kvar: 600.0,
                conn: 0,
                closed,
            }
        }
    }
    impl ControlledCapacitor for MockCap {
        fn full_name(&self) -> String {
            format!("Capacitor.{}", self.name)
        }
        fn num_steps(&self) -> i32 {
            self.num_steps
        }
        fn available_steps(&self) -> i32 {
            self.num_steps - self.last_step
        }
        fn total_kvar(&self) -> f64 {
            self.total_kvar
        }
        fn connection(&self) -> i32 {
            self.conn
        }
        fn is_closed(&self) -> bool {
            self.closed
        }
        fn set_closed(&mut self, value: bool) {
            self.closed = value;
        }
        fn add_step(&mut self) -> bool {
            if self.last_step == self.num_steps {
                false
            } else {
                self.last_step += 1;
                true
            }
        }
        fn subtract_step(&mut self) -> bool {
            if self.last_step == 0 {
                false
            } else {
                self.last_step -= 1;
                self.last_step != 0
            }
        }
    }

    /// A monitored element stub returning canned measurements, so the CapControl
    /// decision logic is testable without a node-wired circuit.
    struct MockMon {
        cd: CktElementData,
        power: Complex64,
        currents: Vec<Complex64>,
        voltages: Vec<Complex64>,
    }
    impl MockMon {
        fn new(nphases: usize) -> Self {
            let mut cd = CktElementData::new("mon", 1);
            cd.nphases = nphases;
            cd.nconds = nphases;
            cd.set_nterms(1);
            cd.yorder = nphases;
            Self {
                cd,
                power: Complex64::ZERO,
                currents: Vec::new(),
                voltages: Vec::new(),
            }
        }
    }
    impl CktElement for MockMon {
        fn cd(&self) -> &CktElementData {
            &self.cd
        }
        fn cd_mut(&mut self) -> &mut CktElementData {
            &mut self.cd
        }
        fn recalc_element_data(&mut self, _sys: &SysCtx) {}
        fn calc_yprim(&mut self, _sys: &SysCtx) {}
        fn get_currents(&mut self, _sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
            for (i, c) in curr.iter_mut().enumerate() {
                *c = self.currents.get(i).copied().unwrap_or(Complex64::ZERO);
            }
        }
        fn terminal_power(
            &mut self,
            _sys: &SysCtx,
            _node_v: &[Complex64],
            _idx_term: usize,
        ) -> Complex64 {
            self.power
        }
        fn get_term_voltages(
            &self,
            _iterm: usize,
            _node_v: &[Complex64],
            vbuffer: &mut [Complex64],
        ) {
            for (i, v) in vbuffer.iter_mut().enumerate() {
                *v = self.voltages.get(i).copied().unwrap_or(Complex64::ZERO);
            }
        }
    }

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
        fn ctx(&mut self, control_mode: i32, int_hour: i32, t: f64) -> CtrlCtx<'_> {
            CtrlCtx {
                node_v: &[],
                sys: &self.sys,
                queue: &mut self.queue,
                events: &mut self.events,
                errors: &mut self.errors,
                system_y_changed: &mut self.y_changed,
                control_mode,
                int_hour,
                t,
                dbl_hour: int_hour as f64 + t / 3600.0,
                control_iter: 1,
                self_ref: ElemRef { cls: 0, idx: 0 },
            }
        }
    }

    #[test]
    fn kvar_open_arms_close_above_onsetting() {
        let mut cc = CapControl::new("cc");
        cc.control_type = ctrl_type::KVAR;
        cc.on_value = 150.0;
        cc.off_value = -225.0;
        let mut cap = MockCap::one_step(false); // bank open → PresentState OPEN
        let mut mon = MockMon::new(3);
        mon.power = Complex64::new(0.0, 200_000.0); // 200 kvar inductive
        let mut sc = Scratch::new();
        cc.sample(&mut cap, &mut mon, &mut sc.ctx(0, 0, 0.0));
        assert_eq!(cc.pending_change, CTRL_CLOSE);
        assert!(cc.should_switch);
        assert!(cc.armed);
        assert_eq!(sc.queue.queue_size(), 1);
        // pending CLOSE; dead time already elapsed → ONDelay.
        assert_eq!(cc.ccd.time_delay, 15.0);
    }

    #[test]
    fn kvar_closed_arms_open_below_offsetting() {
        let mut cc = CapControl::new("cc");
        cc.control_type = ctrl_type::KVAR;
        cc.on_value = 150.0;
        cc.off_value = -225.0;
        let mut cap = MockCap::one_step(true); // bank closed → PresentState CLOSE
        let mut mon = MockMon::new(3);
        mon.power = Complex64::new(0.0, -300_000.0); // -300 kvar (too leading)
        let mut sc = Scratch::new();
        cc.sample(&mut cap, &mut mon, &mut sc.ctx(0, 0, 0.0));
        assert_eq!(cc.pending_change, CTRL_OPEN);
        assert!(cc.armed);
        assert_eq!(cc.ccd.time_delay, 15.0); // OFFDelay
    }

    #[test]
    fn kvar_in_band_does_not_switch() {
        let mut cc = CapControl::new("cc");
        cc.control_type = ctrl_type::KVAR;
        cc.on_value = 150.0;
        cc.off_value = -225.0;
        let mut cap = MockCap::one_step(true);
        let mut mon = MockMon::new(3);
        mon.power = Complex64::new(0.0, -50_000.0); // -50 kvar: between off and on
        let mut sc = Scratch::new();
        cc.sample(&mut cap, &mut mon, &mut sc.ctx(0, 0, 0.0));
        assert_eq!(cc.pending_change, CTRL_NONE);
        assert!(!cc.armed);
        assert!(sc.queue.is_empty());
    }

    #[test]
    fn do_pending_open_single_step_opens_bank() {
        let mut cc = CapControl::new("cc");
        cc.present_state = CTRL_CLOSE;
        cc.set_pending_change(CTRL_OPEN);
        cc.armed = true;
        let mut cap = MockCap::one_step(true);
        let mut sc = Scratch::new();
        cc.do_pending_action(&mut cap, &mut sc.ctx(0, 0, 0.0));
        assert!(!cap.closed);
        assert_eq!(cap.last_step, 0);
        assert_eq!(cc.present_state, CTRL_OPEN);
        assert!(sc.y_changed);
        assert!(!cc.armed);
    }

    #[test]
    fn do_pending_close_single_step_closes_bank() {
        let mut cc = CapControl::new("cc");
        cc.present_state = CTRL_OPEN;
        cc.set_pending_change(CTRL_CLOSE);
        cc.armed = true;
        let mut cap = MockCap::one_step(false);
        let mut sc = Scratch::new();
        cc.do_pending_action(&mut cap, &mut sc.ctx(0, 0, 0.0));
        assert!(cap.closed);
        assert_eq!(cap.last_step, 1);
        assert_eq!(cc.present_state, CTRL_CLOSE);
        assert!(sc.y_changed);
    }

    #[test]
    fn do_pending_open_multistep_steps_down() {
        let mut cc = CapControl::new("cc");
        cc.present_state = CTRL_CLOSE;
        cc.set_pending_change(CTRL_OPEN);
        let mut cap = MockCap {
            name: "cc".into(),
            num_steps: 4,
            last_step: 4,
            total_kvar: 1200.0,
            conn: 0,
            closed: true,
        };
        let mut sc = Scratch::new();
        cc.do_pending_action(&mut cap, &mut sc.ctx(0, 0, 0.0));
        // One step down: still partly closed, bank stays Closed.
        assert_eq!(cap.last_step, 3);
        assert!(cap.closed);
        assert_eq!(cc.present_state, CTRL_CLOSE);
        assert!(sc.y_changed);
    }

    #[test]
    fn time_control_closes_inside_window() {
        let mut cc = CapControl::new("cc");
        cc.control_type = ctrl_type::TIME;
        cc.on_value = 6.0;
        cc.off_value = 21.0;
        let mut cap = MockCap::one_step(false); // open at start
        let mut mon = MockMon::new(3);
        let mut sc = Scratch::new();
        // 12:00 is inside [6, 21) → close.
        cc.sample(&mut cap, &mut mon, &mut sc.ctx(0, 12, 0.0));
        assert_eq!(cc.pending_change, CTRL_CLOSE);
        assert!(cc.armed);
    }

    #[test]
    fn pf_control_closes_when_leading_room_remains() {
        let mut cc = CapControl::new("cc");
        cc.control_type = ctrl_type::PF;
        cc.pfon_value = 0.95;
        cc.fpct_minkvar = 50.0;
        let mut cap = MockCap::one_step(false); // open
        cap.total_kvar = 50.0;
        let mut mon = MockMon::new(3);
        // 100 kW + 50 kvar → PF1to2 = 0.894 < 0.95; 50 kvar > 50·50·0.01 = 25.
        mon.power = Complex64::new(100_000.0, 50_000.0);
        let mut sc = Scratch::new();
        cc.sample(&mut cap, &mut mon, &mut sc.ctx(0, 0, 0.0));
        assert_eq!(cc.pending_change, CTRL_CLOSE);
        assert!(cc.armed);
    }

    #[test]
    fn event_log_records_close_when_enabled() {
        let mut cc = CapControl::new("cc");
        cc.ccd.show_event_log = true;
        cc.present_state = CTRL_OPEN;
        cc.set_pending_change(CTRL_CLOSE);
        let mut cap = MockCap::one_step(false);
        let mut sc = Scratch::new();
        cc.do_pending_action(&mut cap, &mut sc.ctx(0, 0, 0.0));
        assert_eq!(sc.events.len(), 1);
        let line = &sc.events.entries()[0];
        assert!(line.contains("Element=Capacitor.cc"));
        assert!(line.contains("**CLOSED**"));
    }

    #[test]
    fn pf_1to2_maps_leading_above_one() {
        // Lagging (im>0): PF in [0,1]. Leading (im<0): PF in [1,2].
        assert!((pf_1to2(Complex64::new(100.0, 0.0)) - 1.0).abs() < 1e-12);
        assert!((pf_1to2(Complex64::ZERO) - 1.0).abs() < 1e-12);
        let lag = pf_1to2(Complex64::new(80.0, 60.0)); // 0.8 lagging
        assert!((lag - 0.8).abs() < 1e-12);
        let lead = pf_1to2(Complex64::new(80.0, -60.0)); // 0.8 leading → 1.2
        assert!((lead - 1.2).abs() < 1e-12);
    }
}

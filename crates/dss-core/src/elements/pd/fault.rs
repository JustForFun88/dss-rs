//! Port of `PDElements/Fault.pas` — `TFaultObj`, the Fault object: one or more
//! faults placed across any two buses. Like the Capacitor/Reactor it follows the
//! shunt connection rule — Bus2 defaults to the grounded-zero node of Bus1 (a
//! wye-grounded fault); a Bus2 with non-matching nodes makes it a series branch.
//!
//! Electrically it is an uncoupled multi-phase **conductance** branch: either a
//! single per-phase `G = 1/r` (`SpecType = 1`, the `r` property is stored
//! inverted) or a full nodal-conductance `Gmatrix` (`SpecType = 2`). The YPrim is
//! frequency-independent (pure real G), stamped diagonally (SpecType 1) or as the
//! supplied matrix (SpecType 2) into the two-terminal primitive.
//!
//! Its class type is `FAULTOBJECT or NON_PCPD_ELEM` (DSSClassDefs.pas): a
//! `TPDElement` subclass with a YPrim that participates in the system Y, but
//! deliberately **excluded** from the `PDElements`/`PCElements` enumerations — it
//! lives only on the circuit's `Faults` list (Circuit.pas `AddCktElement`).
//!
//! Time-varying behavior (`CheckStatus`/`Reset`) runs from the control-iteration
//! loop via `Check_Fault_Status` (Solution.pas l.1149): the fault is enabled once
//! solution time passes `ONtime`, and a `temporary` fault self-clears when its
//! terminal current drops below `MinAmps`. In a snapshot solve the control mode
//! is `CTRLSTATIC`, so `CheckStatus` is a no-op and the fault (default `ONtime=0`,
//! `Is_ON=true`) simply stamps its conductance.
//!
//! `Randomize` + the `RandomMult` jitter only act in `MONTEFAULT` solve mode,
//! whose solve loop is deferred to WP7.9; until then `RandomMult` stays `1.0`
//! (`CalcYPrim` forces it for every non-MonteFault mode) and the field is inert.

#[cfg(test)]
mod tests;

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::traits::{CktElement, ReliabilityData, SysCtx};
use crate::obj::base::{DssObjData, DssObject};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use crate::solution::event_log::EventLog;
use crate::solution::solution::{EVENTDRIVEN, MULTIRATE, SolveMode, TIMEDRIVEN};
use crate::support::cmatrix::CMatrix;

/// 1-based property ordinals (Pascal `TFaultProp` + the TPDClass/TCktElementClass
/// tails appended by `inherited DefineProperties`).
pub mod prop {
    pub const BUS1: usize = 1;
    pub const BUS2: usize = 2;
    pub const PHASES: usize = 3;
    pub const R: usize = 4;
    pub const PCTSTDDEV: usize = 5;
    pub const GMATRIX: usize = 6;
    pub const ONTIME: usize = 7;
    pub const TEMPORARY: usize = 8;
    pub const MINAMPS: usize = 9;
    // TPDClass tail:
    pub const NORMAMPS: usize = 10;
    pub const EMERGAMPS: usize = 11;
    pub const FAULTRATE: usize = 12;
    pub const PCTPERM: usize = 13;
    pub const REPAIR: usize = 14;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 15;
    pub const ENABLED: usize = 16;
    pub const NUM_PROPS: usize = 17; // incl. Like
}

/// `TFault.DefineProperties`.
pub fn class_props(_enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    let defs = vec![
        // Pascal flags bus1 `Required` (inert here — not enforced).
        PropDef::bus("Bus1", 1),
        PropDef::bus("Bus2", 2),
        PropDef::integer("Phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        // `r` stores its inverse `G` (Pascal `InverseValue`); the spec-set flags
        // are inert metadata in this port.
        PropDef::double("R").flags(PropFlags::INVERSE_VALUE | PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::double("%StdDev").scale(0.01),
        PropDef::double_sym_matrix("GMatrix", PHASES).flags(PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::double("OnTime"),
        PropDef::boolean("Temporary"),
        PropDef::double("MinAmps"),
        // TPDClass tail (Pascal suppresses normamps/emergamps from JSON — inert
        // here; the text dump still carries them).
        PropDef::double("normamps"),
        PropDef::double("emergamps"),
        PropDef::double("faultrate"),
        PropDef::double("pctperm"),
        PropDef::double("repair"),
        // TCktElementClass tail:
        PropDef::double("basefreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("enabled"),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);
    ClassProps::new("Fault", defs, true)
}

/// What `Check_Fault_Status` passes to each fault's [`Fault::check_status`]: the
/// solution's control mode and event-log time coordinates, plus the solved state
/// `FaultStillGoing` reads. Mirrors the controls' `CtrlCtx` borrow split.
pub struct FaultStatusCtx<'a> {
    pub control_mode: i32,
    pub int_hour: i32,
    pub t: f64,
    pub control_iter: i32,
    pub sys: &'a SysCtx,
    pub node_v: &'a [Complex64],
}

impl FaultStatusCtx<'_> {
    /// Pascal `PresentTimeInSec` = `DynaVars.t + DynaVars.intHour * 3600`.
    fn present_time_sec(&self) -> f64 {
        self.t + self.int_hour as f64 * 3600.0
    }
}

/// `TFaultObj`.
#[derive(Debug, Clone)]
pub struct Fault {
    pub cd: CktElementData,
    /// Single conductance per phase (`G = 1/r`) used when `Gmatrix` is unset.
    g: f64,
    /// Nodal-conductance matrix (row-major `nphases²`); `Some` overrides `g`
    /// (`SpecType = 2`).
    gmatrix: Option<Vec<f64>>,
    /// Per-unit standard deviation (`%StdDev` scaled by 0.01) — MonteFault only.
    stddev: f64,
    /// 1 = `r` (single G), 2 = `Gmatrix`.
    spec_type: i32,
    min_amps: f64,
    is_temporary: bool,
    /// Set once a temporary fault self-clears; blocks re-application.
    cleared: bool,
    /// Whether the fault conductance is currently stamped.
    is_on: bool,
    bus2_defined: bool,
    /// Enable time (s); a fault with `ONtime > 0` starts `Is_ON = false`.
    on_time: f64,
    /// MonteFault resistance jitter; always `1.0` until WP7.9 wires MonteFault.
    random_mult: f64,
    is_shunt: bool,
    // PD-element common (Fault's own defaults — all reliability fields zeroed,
    // pctperm 100). Fault computes no default Norm/Emerg amps (Pascal leaves them
    // 0), so there is no `*_specified` tracking.
    norm_amps: f64,
    emerg_amps: f64,
    fault_rate: f64,
    pct_perm: f64,
    hrs_to_repair: f64,
}

impl Fault {
    /// Pascal `TFaultObj.Create`: default to a 1-phase (SLG) grounded shunt fault
    /// with a low `1/10000 Ω` resistance.
    pub fn new(name: &str) -> Self {
        let mut cd = CktElementData::new(name, prop::NUM_PROPS);
        cd.nphases = 1;
        cd.nconds = 1;
        cd.set_nterms(2); // forces allocation of terminals/conductors + buses

        // Default to grounded (Bus2 = Bus1.0); the Bus1 side effect later widens
        // it to `.0.0.0` once Bus1 is parsed.
        let bus1 = cd.get_bus(1).to_string();
        cd.set_bus(2, &format!("{bus1}.0"));

        let mut f = Self {
            cd,
            g: 10000.0,
            gmatrix: None,
            stddev: 0.0,
            spec_type: 1, // r (G); 2 = Gmatrix
            min_amps: 5.0,
            is_temporary: false,
            cleared: false,
            is_on: true,
            bus2_defined: false,
            on_time: 0.0,
            random_mult: 1.0,
            is_shunt: true,
            norm_amps: 0.0,
            emerg_amps: 0.0,
            fault_rate: 0.0,
            pct_perm: 100.0,
            hrs_to_repair: 0.0,
        };
        // Mark `r` as filled (Pascal `SetAsNextSeq(ord(TProp.r))`).
        f.cd.obj.set_as_next_seq(prop::R);
        f.cd.yorder = f.cd.nterms * f.cd.nconds;
        f
    }

    /// Pascal `TFaultObj.FaultStillGoing`: any terminal current above `MinAmps`.
    fn fault_still_going(&mut self, sys: &SysCtx, node_v: &[Complex64]) -> bool {
        self.compute_iterminal(sys, node_v);
        let nphases = self.cd.nphases;
        (0..nphases).any(|i| self.cd.iterminal[i].norm() > self.min_amps)
    }

    /// Pascal `TFaultObj.CheckStatus`: in an event/time-driven control mode,
    /// enable the fault once solution time passes `ONtime`, and self-clear a
    /// temporary fault once its current drops below `MinAmps`. `CTRLSTATIC`
    /// (snapshot) leaves the fault as configured. Returns `true` if `Is_ON`
    /// changed (the caller invalidates Y).
    pub fn check_status(&mut self, ctx: &FaultStatusCtx, events: &mut EventLog) -> bool {
        match ctx.control_mode {
            EVENTDRIVEN | MULTIRATE | TIMEDRIVEN => {}
            // CTRLSTATIC (and any other mode) leaves it however it is defined.
            _ => return false,
        }
        let full = format!("Fault.{}", self.cd.obj.name());
        if !self.is_on {
            // Turn it on unless it has been previously cleared.
            if ctx.present_time_sec() > self.on_time && !self.cleared {
                self.is_on = true;
                self.cd.yprim_invalid = true;
                events.append(&full, "**APPLIED**", ctx.int_hour, ctx.t, ctx.control_iter);
                return true;
            }
            false
        } else if self.is_temporary && !self.fault_still_going(ctx.sys, ctx.node_v) {
            self.is_on = false;
            self.cleared = true;
            self.cd.yprim_invalid = true;
            events.append(&full, "**CLEARED**", ctx.int_hour, ctx.t, ctx.control_iter);
            true
        } else {
            false
        }
    }

    /// Pascal `TFaultObj.Reset`: clear the self-cleared latch (`DoResetFaults` at
    /// the start of a solution and the `Reset Faults` command).
    pub fn reset(&mut self) {
        self.cleared = false;
    }
}

impl CktElement for Fault {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    /// Pascal `RecalcElementData`: nothing to do (the YPrim is built directly
    /// from `G`/`Gmatrix`).
    fn recalc_element_data(&mut self, _sys: &SysCtx) {}

    /// Pascal `TPDElement.CalcFltRate` (base): `Faultrate · pctperm · 0.01`.
    /// Fault's own `FaultRate` defaults to 0, so this is 0 — a fault never
    /// contributes a branch fault rate.
    fn reliability_data(&self) -> ReliabilityData {
        ReliabilityData {
            branch_flt_rate: self.fault_rate * self.pct_perm * 0.01,
            hrs_to_repair: self.hrs_to_repair,
            miles_this_line: 0.0,
        }
    }

    fn norm_amps(&self) -> f64 {
        self.norm_amps
    }
    fn emerg_amps(&self) -> f64 {
        self.emerg_amps
    }

    /// Pascal `TPDElement.IsShunt` (set by the Bus1/Bus2 side effects).
    fn is_shunt(&self) -> bool {
        self.is_shunt
    }

    /// Pascal `TFaultObj.CalcYPrim`: stamp the (uncoupled) conductance into the
    /// shunt or series primitive. `Is_ON = false` stamps zero conductance.
    fn calc_yprim(&mut self, sys: &SysCtx) {
        let yorder = self.cd.yorder;
        let nphases = self.cd.nphases;

        // RandomMult is 1.0 outside MonteFault (whose solve loop is WP7.9); guard
        // a zero to a tiny value as Pascal does.
        let mut random_mult = if sys.mode == SolveMode::MonteFault {
            self.random_mult
        } else {
            1.0
        };
        if random_mult == 0.0 {
            random_mult = 0.000001;
        }

        let mut work = CMatrix::new(yorder);
        match self.spec_type {
            2 => {
                // Gmatrix specified.
                let gm = self.gmatrix.as_ref().expect("SpecType 2 has Gmatrix");
                for i in 0..nphases {
                    let ioffset = i * nphases;
                    for j in 0..nphases {
                        let value = if self.is_on {
                            Complex64::new(gm[ioffset + j] / random_mult, 0.0)
                        } else {
                            Complex64::ZERO
                        };
                        work.set(i, j, value);
                        work.set(i + nphases, j + nphases, value);
                        work.set(i, j + nphases, -value);
                        work.set(j + nphases, i, -value);
                    }
                }
            }
            _ => {
                // Single G per phase (SpecType 1): diagonal only.
                let value = if self.is_on {
                    Complex64::new(self.g / random_mult, 0.0)
                } else {
                    Complex64::ZERO
                };
                for i in 0..nphases {
                    work.set(i, i, value);
                    work.set(i + nphases, i + nphases, value);
                    work.set(i, i + nphases, -value);
                    work.set(i + nphases, i, -value);
                }
            }
        }

        // YPrimTemp is the shunt or series matrix; the other stays zero and YPrim
        // mirrors the filled one (Pascal `YPrim.CopyFrom(YPrimTemp)`).
        let mut yp_series = CMatrix::new(yorder);
        let mut yp_shunt = CMatrix::new(yorder);
        if self.is_shunt {
            yp_shunt.copy_from(&work);
        } else {
            yp_series.copy_from(&work);
        }
        let mut yprim = CMatrix::new(yorder);
        yprim.copy_from(&work);

        self.cd.yprim_freq = sys.frequency;
        self.cd.yprim_series = Some(yp_series);
        self.cd.yprim_shunt = Some(yp_shunt);
        self.cd.yprim = Some(yprim);

        self.cd.apply_yprim_open_conductor_calcs();
        self.cd.yprim_invalid = false;
    }
}

impl DssObject for Fault {
    fn data(&self) -> &DssObjData {
        &self.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.cd.obj
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
            // `r` stores its inverse `G`; the InverseValue getter re-inverts.
            R => self.g,
            PCTSTDDEV => self.stddev,
            ONTIME => self.on_time,
            MINAMPS => self.min_amps,
            NORMAMPS => self.norm_amps,
            EMERGAMPS => self.emerg_amps,
            FAULTRATE => self.fault_rate,
            PCTPERM => self.pct_perm,
            REPAIR => self.hrs_to_repair,
            BASE_FREQ => self.cd.base_frequency,
            _ => unreachable!("Fault has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            R => self.g = value,
            PCTSTDDEV => self.stddev = value,
            ONTIME => self.on_time = value,
            MINAMPS => self.min_amps = value,
            NORMAMPS => self.norm_amps = value,
            EMERGAMPS => self.emerg_amps = value,
            FAULTRATE => self.fault_rate = value,
            PCTPERM => self.pct_perm = value,
            REPAIR => self.hrs_to_repair = value,
            BASE_FREQ => self.cd.base_frequency = value,
            _ => unreachable!("Fault has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases as i32,
            _ => unreachable!("Fault has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases = value.max(0) as usize,
            _ => unreachable!("Fault has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use prop::*;
        match idx {
            TEMPORARY => self.is_temporary,
            ENABLED => self.cd.enabled,
            _ => unreachable!("Fault has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use prop::*;
        match idx {
            TEMPORARY => self.is_temporary = value,
            ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("Fault has no boolean property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        use prop::*;
        match idx {
            GMATRIX => self.gmatrix.as_deref(),
            _ => unreachable!("Fault has no double-array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        use prop::*;
        match idx {
            GMATRIX => self.gmatrix = Some(value),
            _ => unreachable!("Fault has no double-array property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.cd.get_bus(terminal).to_string()
    }

    /// Pascal `TFaultObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, prev_int: i32) {
        use prop::*;
        match idx {
            PHASES => {
                if self.cd.nphases as i32 != prev_int {
                    // Force reallocation of terminal info (NConds := Fnphases).
                    let np = self.cd.nphases;
                    self.cd.set_nconds(np);
                    self.cd.yorder = self.cd.nterms * self.cd.nconds;
                }
            }
            BUS1 => {
                if !self.bus2_defined {
                    // Default Bus2 to the grounded-zero node of Bus1 (wye
                    // grounded), up to 3 phases (`.0.0.0`).
                    let s = self.cd.get_bus(1).to_string();
                    let base = match s.find('.') {
                        Some(p) => s[..p].to_string(),
                        None => s,
                    };
                    self.cd.set_bus(2, &format!("{base}.0.0.0"));
                    self.is_shunt = true;
                    self.cd.obj.set_as_next_seq(BUS2);
                }
            }
            BUS2 => {
                if !strip_extension(self.cd.get_bus(1))
                    .eq_ignore_ascii_case(&strip_extension(self.cd.get_bus(2)))
                {
                    self.is_shunt = false;
                    self.bus2_defined = true;
                }
            }
            R => {
                self.spec_type = 1;
                if self.g == 0.0 {
                    self.g = 10000.0; // default to a low resistance
                }
                self.cd.obj.clear_seq(GMATRIX);
            }
            GMATRIX => {
                self.spec_type = 2;
                self.cd.obj.clear_seq(R);
            }
            ONTIME => {
                if self.on_time > 0.0 {
                    self.is_on = false; // assume the fault will be on later
                }
            }
            _ => {}
        }

        // YPrim invalidation on anything that changes impedance values
        // (phases / r / Gmatrix — Pascal `case Idx of 3, 4, 6`).
        if matches!(idx, PHASES | R | GMATRIX) {
            self.cd.yprim_invalid = true;
        }
    }

    /// Pascal base `EndEdit` → `RecalcElementData` (Fault's is a no-op).
    fn end_edit(&mut self) {}

    /// Pascal `TFaultObj.MakeLike`.
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(other) = other.as_any().downcast_ref::<Fault>() else {
            return;
        };
        self.cd.make_like_base(&other.cd);
        if self.cd.nphases != other.cd.nphases {
            self.cd.nphases = other.cd.nphases;
            let n = other.cd.nphases;
            self.cd.set_nconds(n); // force reallocation of terminals/conductors
            self.cd.yorder = self.cd.nconds * self.cd.nterms;
            self.cd.yprim_invalid = true;
        }
        self.g = other.g;
        self.spec_type = other.spec_type;
        self.min_amps = other.min_amps;
        self.is_temporary = other.is_temporary;
        self.cleared = other.cleared;
        self.is_on = other.is_on;
        self.on_time = other.on_time;
        self.gmatrix = other.gmatrix.clone();

        // TPDElement.MakeLike copies the rating fields.
        self.norm_amps = other.norm_amps;
        self.emerg_amps = other.emerg_amps;
        self.fault_rate = other.fault_rate;
        self.pct_perm = other.pct_perm;
        self.hrs_to_repair = other.hrs_to_repair;
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

/// Pascal `StripExtension`: the bus name with its `.node.node…` suffix removed.
fn strip_extension(s: &str) -> String {
    match s.find('.') {
        Some(p) => s[..p].to_string(),
        None => s.to_string(),
    }
}

//! The trait plumbing for `TSwtControlObj`: the [`CktElement`] hooks (a control
//! element builds no Yprim and carries zero current) and the [`DssObject`]
//! property accessors (`get_*`/`set_*`, `set_object_ref`, `PropertySideEffects`,
//! `EndEdit`, `MakeLike`, `take_ref_actions`).
//!
//! RP3.7: the state model is per-phase (`super::SwtControl::present_state`/
//! `normal_state`) and so is the property surface — `Normal`/`State` are
//! `MappedStringEnumArray`s whose [`DssObject::array_size`] is the LIVE
//! controlled-element phase count and whose writes run r4133's
//! `InterpretSwitchState` through [`DssObject::set_enum_array_raw`]. The lock
//! rule is r4133's name-based guard (a locked `normal=` applies; locked
//! `state=`/`action=` do not), with the Edit supplemental running either way
//! (`SwtControl.pas:219-228`, measured — `tmp/rp37/out_a2a2.txt`).

use num_complex::Complex64;

use crate::elements::control::control_elem::{ControlAction, ControlElemData};
use crate::elements::pos_seq::{PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, SysCtx};
use crate::obj::arena::ResolvedObj;
use crate::obj::base::{DssObjData, DssObject, RefAction};

use super::{ARR, SW_MAX, SwtControl};

impl CktElement for SwtControl {
    fn cd(&self) -> &crate::elements::ckt::CktElementData {
        &self.ccd.cd
    }
    fn cd_mut(&mut self) -> &mut crate::elements::ckt::CktElementData {
        &mut self.ccd.cd
    }

    /// Pascal `TControlElem.FControlledElement` - the element this control
    /// acts on (`None` when it drives a list rather than a single element).
    fn controlled_element(&self) -> Option<crate::elements::traits::ElemId> {
        self.ccd.controlled_element
    }

    /// Pascal `TControlElem.CalcYPrim`: leave YPrim as NIL.
    fn calc_yprim(&mut self, _sys: &SysCtx) {}

    /// Pascal `TControlElem.GetCurrents`: always zero.
    fn get_currents(&mut self, _sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
        curr.fill(Complex64::ZERO);
    }

    /// Pascal `TSwtControlObj.MakePosSequence`
    /// (`Version8/Source/Controls/SwtControl.pas:367-375`): adopt the controlled
    /// (switched) element's phase / conductor counts and attach terminal 1 to
    /// its bus, then run the base bus rename (`inherited`). r4133 does NOT
    /// touch the state arrays here — the render resync the probe measured after
    /// `makeposseq` (`tmp/rp37/probe.md` §6: `'[closed, closed, closed, ]'` →
    /// `'[closed, ]'`) is purely the getter's live `ControlledElement.NPhases`
    /// loop. The port's stand-in for that live read is the controlled-element
    /// snapshot, so this method refreshes the snapshot's shape too — otherwise
    /// [`SwtControl::state_size`] would keep answering the FROZEN parse-time
    /// count and a later `edit` (whose `recalc` re-reads the snapshot) would
    /// resurrect the pre-pos-seq bound. Only reads `ControlledElement`, so no
    /// `monitored_element_ref` override is needed.
    fn make_pos_sequence(&mut self, ctx: &PosSeqCtx) -> PosSeqPlan {
        if let Some(c) = &ctx.controlled {
            // Nphases := ControlledElement.NPhases; Nconds := FNphases
            self.ccd.cd.nphases = c.nphases;
            self.ccd.cd.set_nconds(c.nphases);
            // Setbus(1, ControlledElement.GetBus(ElementTerminal))
            let t = self.ccd.element_terminal as usize;
            let bus = t
                .checked_sub(1)
                .and_then(|k| c.bus_names.get(k))
                .cloned()
                .unwrap_or_default();
            self.ccd.cd.set_bus(1, &bus);
            // The live controlled-element shape the render/drive bound reads.
            if let Some(snap) = self.ctrl_snap.as_mut() {
                snap.nphases = c.nphases;
                snap.nterms = c.bus_names.len(); // `BusNames[1..NTerms]`
                snap.buses = c.bus_names.clone();
            }
        }
        // inherited MakePosSequence -> base bus rename.
        PosSeqPlan::base()
    }
}

impl SwtControl {
    /// Pascal `TSwtControlObj.MakeLike` (`SwtControl.pas:239-273`). The state
    /// arrays copy in full (r4133 loops `1..Min(SWTCONTROLMAXDIM,
    /// ControlledElement.Nphases)` at `:261-264`; the port's render/drive
    /// bounds clip to the controlled element's phase count, so the extra slots
    /// are unobservable). `NormalStateSet` copies with them: r4133 `MakeLike`
    /// omits the flag (`:248-268`) — an upstream oversight, since it copies
    /// every other piece of state (`Locked`, `RatedCurrent`, the arrays,
    /// `PropertyValue[]`); the observable consequence would be the clone's
    /// first `State`/`Action` write re-latching Normal from Present and
    /// silently discarding the base's declared normal. The pinned oracle
    /// (capi015, whose scalar model has no flag and copies `NormalState`)
    /// renders the carried normal — the `swtcontrol_makelike` props golden —
    /// and no corpus deck exercises `like=` followed by a state write, so the
    /// port keeps the consistent reading; the divergence point is recorded in
    /// the RP3.7 A1 handoff (`tmp/rp37/impl_a1.md`).
    pub(crate) fn make_like(&mut self, other: &Self) {
        self.ccd.cd.make_like_base(&other.ccd.cd);
        self.ccd.cd.nphases = other.ccd.cd.nphases;
        let nc = other.ccd.cd.nconds;
        self.ccd.cd.set_nconds(nc); // Force Reallocation of terminal stuff

        self.ccd.element_terminal = other.ccd.element_terminal;
        self.ccd.controlled_element = other.ccd.controlled_element;
        self.switched_full_name = other.switched_full_name.clone();
        self.ctrl_snap = other.ctrl_snap.clone();

        self.ccd.time_delay = other.ccd.time_delay;
        self.locked = other.locked;
        self.present_state = other.present_state;
        self.normal_state = other.normal_state;
        self.normal_state_set = other.normal_state_set;
        self.current_action = other.current_action;
        // r4133 `MakeLike`: `RatedCurrent := OtherSwtControl.RatedCurrent`.
        self.rated_current = other.rated_current;
    }
}

impl DssObject for SwtControl {
    fn data(&self) -> &DssObjData {
        &self.ccd.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.ccd.cd.obj
    }

    fn as_control(&self) -> Option<&dyn crate::elements::control::control_elem::ControlElem> {
        Some(self)
    }
    fn as_control_mut(
        &mut self,
    ) -> Option<&mut dyn crate::elements::control::control_elem::ControlElem> {
        Some(self)
    }

    fn get_f64(&self, idx: usize) -> f64 {
        use super::prop::*;
        match idx {
            DELAY => self.ccd.time_delay,
            RATED_CURRENT => self.rated_current,
            BASE_FREQ => self.ccd.cd.base_frequency,
            _ => unreachable!("SwtControl has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use super::prop::*;
        match idx {
            DELAY => self.ccd.time_delay = value,
            // r4133 `RatedCurrent := Parser.DblValue` — informational store.
            RATED_CURRENT => self.rated_current = value,
            BASE_FREQ => self.ccd.cd.base_frequency = value,
            _ => unreachable!("SwtControl has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use super::prop::*;
        match idx {
            SWITCHED_TERM => self.ccd.element_terminal,
            // `Action` is the only scalar state seam left (r4133 has NO getter
            // arm for index 3 — the class-props doc owns that divergence; the
            // port keeps the 0.14.5 `CurrentAction` readback the capi015 props
            // golden pins). `Normal`/`State` are `MappedStringEnumArray`s and
            // never reach this accessor — see [`Self::get_enum_array`].
            ACTION => self.current_action.ordinal(),
            _ => unreachable!("SwtControl has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use super::prop::*;
        match idx {
            SWITCHED_TERM => self.ccd.element_terminal = value,
            // r4133 Edit arm 3 (`SwtControl.pas:201-204`) hands `Action` to
            // `InterpretSwitchState` like the other two, so the port routes it
            // there as well — one write mechanics, one lock guard (`:416-417`:
            // a locked `action=` is refused because the property name starts
            // with `a`). The scalar seam has already decoded the token to an
            // ordinal through the `SwtControl: Action` enum, whose
            // `max_chars = 1` + `allow_longer` + `Keep` default reproduce
            // r4133's `case LowerCase(param)[1]` with no else arm exactly; the
            // canonical first character feeds that decision back in without
            // re-running a match on a token this seam no longer has.
            ACTION => {
                let token = match ControlAction::from_ordinal(value) {
                    ControlAction::Open => "o",
                    ControlAction::Close => "c",
                    _ => "", // `Keep` (no match) — the Pascal `case` falls through
                };
                self.interpret_switch_state(super::SwtStateProp::Action, token, false);
            }
            _ => unreachable!("SwtControl has no integer property {idx}"),
        }
    }

    /// `Normal`/`State`: one token per LIVE controlled-element phase
    /// (`GetPropertyValue` `:589-610`), `0` with no controlled element — which
    /// renders the bare `'[]'` r4133 answers on an orphan control
    /// (`tmp/rp37/out_nil_controlled.txt`).
    fn array_size(&self, idx: usize) -> usize {
        use super::prop::*;
        match idx {
            NORMAL | STATE => self.state_size(),
            _ => unreachable!("SwtControl has no function-sized array property {idx}"),
        }
    }

    /// `Normal`/`State` render ordinals (1-based Pascal slots, exposed 0-based).
    ///
    /// r4133's getters are a two-armed `case`: `CTRL_OPEN` prints `open`, the
    /// `else` prints `closed` for **every** other ordinal (`:594-598` Normal,
    /// `:605-609` State) — so a slot holding anything but `CTRL_OPEN` (a
    /// `Keep`/`None` sentinel included) renders `closed`, and the fold happens
    /// here rather than in the enum table.
    fn get_enum_array(&self, idx: usize) -> Vec<i32> {
        use super::prop::*;
        let n = self.state_size();
        let arr = match idx {
            NORMAL => &self.normal_state,
            STATE => &self.present_state,
            _ => unreachable!("SwtControl has no enum-array property {idx}"),
        };
        arr[1..=n]
            .iter()
            .map(|s| {
                if *s == ControlAction::Open {
                    ControlAction::Open.ordinal()
                } else {
                    ControlAction::Close.ordinal()
                }
            })
            .collect()
    }

    /// The RAW `Normal`/`State` write — r4133 `InterpretSwitchState`
    /// (`SwtControl.pas:410-482`) in full: the name-based lock guard, the
    /// ganged-vs-per-phase split on `Parser.WasQuoted`, the first-character
    /// token match and the five-token per-phase cap. Always consumes the value
    /// (returns `true`), so the generic ordinal tokenizer in `parse_into` never
    /// runs for these two properties.
    ///
    /// `was_quoted` is the outer parser's flag; seams that have no outer parser
    /// pass `false` and the quoted case is reconstructed from the value itself
    /// ([`SwtControl::value_implies_quoted`]).
    fn set_enum_array_raw(&mut self, idx: usize, value: &str, was_quoted: bool) -> bool {
        use super::prop::*;
        let prop = match idx {
            NORMAL => super::SwtStateProp::Normal,
            STATE => super::SwtStateProp::State,
            _ => return false,
        };
        let quoted = was_quoted || SwtControl::value_implies_quoted(value);
        self.interpret_switch_state(prop, value, quoted);
        true
    }

    /// The generic ordinal-array twin of [`Self::set_enum_array_raw`], for a
    /// caller that has already tokenized to enum ordinals. Production never
    /// reaches it — `parse_into` consults the raw hook first and the JSON
    /// importer re-renders its array as a value string through the same
    /// `edit_property` path — but the semantics are the interpreter's so the
    /// two can never drift: a list writes phase by phase honoring the five-slot
    /// cap (`:461` `i < SWTCONTROLMAXDIM`), a [`ControlAction::Keep`] ordinal
    /// (r4133's no-else arm) leaves its slot unchanged, and the lock guard
    /// refuses `State` while letting `Normal` through (`:416-417`).
    fn set_enum_array(&mut self, idx: usize, values: &[i32]) {
        use super::prop::*;
        if idx == STATE && self.locked {
            return; // `:416-417` — 's'tate is refused while locked
        }
        let arr = match idx {
            NORMAL => &mut self.normal_state,
            STATE => &mut self.present_state,
            _ => unreachable!("SwtControl has no enum-array property {idx}"),
        };
        for (k, &v) in values.iter().take(SW_MAX - 1).enumerate() {
            let state = ControlAction::from_ordinal(v);
            if state != ControlAction::Keep {
                arr[k + 1] = state;
            }
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use super::prop::*;
        match idx {
            LOCK => self.locked,
            RESET => false, // Pascal BooleanActionProperty getter: always 0
            ENABLED => self.ccd.cd.enabled,
            _ => unreachable!("SwtControl has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use super::prop::*;
        match idx {
            LOCK => self.locked = value,
            RESET => {
                // Pascal BooleanActionProperty (DoReset): fires on TRUE only.
                if value {
                    self.do_reset_action();
                }
            }
            // Pascal `TSwtControlObj.Set_Enabled` override: only toggle the flag
            // — no BusNameRedefined side effect.
            ENABLED => self.ccd.cd.enabled = value,
            _ => unreachable!("SwtControl has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use super::prop::*;
        match idx {
            SWITCHED_OBJ => self.switched_full_name.clone(),
            _ => unreachable!("SwtControl has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, _value: String) {
        unreachable!("SwtControl has no settable string property {idx}");
    }

    /// `switchedobj=` resolution (Pascal `SetControlledElement`): keep the
    /// `ElemId` plus a shape snapshot for `RecalcElementData`.
    fn set_object_ref(&mut self, idx: usize, name: String, resolved: Option<ResolvedObj<'_>>) {
        use super::prop::*;
        match idx {
            SWITCHED_OBJ => {
                // `name` is the FullName ("Class.name") for the dump.
                self.switched_full_name = name.clone();
                match resolved {
                    Some(o) => {
                        self.ccd.controlled_element = Some(o.id());
                        let elem = o
                            .ckt()
                            .expect("switchedobj resolves against circuit classes");
                        self.ctrl_snap = Some(super::RefSnapshot::capture(name, elem));
                    }
                    None => {
                        self.ccd.controlled_element = None;
                        self.ctrl_snap = None;
                    }
                }
            }
            _ => unreachable!("SwtControl has no object-ref property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.ccd.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.ccd.cd.get_bus(terminal).to_string()
    }

    /// Pascal `TSwtControlObj.PropertySideEffects`. The slot writes themselves
    /// land in `interpret_switch_state` (through
    /// [`Self::set_enum_array_raw`] for `Normal`/`State`, through
    /// [`Self::set_i32`] for `Action`); these are the r4133 Edit-arm tails.
    ///
    /// **Neither tail is guarded by `Locked`, and that is the source's own
    /// shape.** The lock guard lives *inside* `InterpretSwitchState`
    /// (`SwtControl.pas:416-417`), so it stops the state write and nothing
    /// else: arm 6's `if not NormalStateSet then NormalStateSet := TRUE`
    /// (`:203`) is inside the arm but after the call, and the
    /// `{Supplemental Actions} case ParamPointer of 3, 7:` block
    /// (`:219-228`) sits outside the arm entirely, running after **every**
    /// `Action`/`State` edit. Measured on the r4133 DLL
    /// (`tmp/rp37/out_a2a.txt` A2a(3), `tmp/rp37/out_a2a2.txt` A2a(4) vs
    /// A2a(5)): on a fresh control a LOCKED `state=`/`action=` write — refused,
    /// the switch never moves — still latches `NormalStateSet`, so a LATER
    /// unlocked `state=open` leaves `Normal` at `[closed, closed, closed, ]`
    /// where the same sequence without the lock carries it to
    /// `[open, open, open, ]`.
    ///
    /// The 0.14.5 `CurrentAction` glue follows the arrays so the `Sample` queue
    /// branch and the capi015 `Action` readback stay pinned. The
    /// controlled-element force is NOT queued here — r4133 drives it at
    /// end-of-Edit `RecalcElementData` (`:234` → `:347-355`), which the port's
    /// `recalc` reproduces.
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        use super::prop::*;
        match idx {
            NORMAL => {
                // r4133 Edit arm 6 tail (`:203`): any Normal write latches
                // `NormalStateSet` — and a locked `normal=` write IS a write
                // (the guard lets `n`ormal through), so no lock gate here.
                self.normal_state_set = true;
                // 0.14.5/capi015 glue: `Normal` mapped onto `CurrentAction`
                // (the props golden's `Action` readback follows `Normal=`).
                self.current_action = Self::ganged_view(&self.normal_state);
            }
            ACTION | STATE => {
                // The `{Supplemental Actions}` block (`:219-228`), shared by
                // arms 3 and 7 and unreachable by the lock guard: Normal
                // defaults to Present, per phase, on the first such edit.
                self.normal_defaults_to_present();
                self.current_action = Self::ganged_view(&self.present_state);
            }
            LOCK => {
                self.lock_command = if self.locked {
                    ControlAction::Lock
                } else {
                    ControlAction::Unlock
                };
            }
            _ => {}
        }
    }

    /// Pascal `TCktElementClass.EndEdit` default → `RecalcElementData`.
    fn end_edit(&mut self, _sys: &crate::elements::traits::SysCtx) {
        self.recalc();
    }

    fn take_ref_actions(&mut self) -> Vec<RefAction> {
        std::mem::take(&mut self.pending_ref_actions)
    }
}

impl crate::elements::control::control_elem::ControlElem for SwtControl {
    fn ccd(&self) -> &ControlElemData {
        &self.ccd
    }
    fn ccd_mut(&mut self) -> &mut ControlElemData {
        &mut self.ccd
    }
    fn control_kind(&self) -> crate::elements::control::control_elem::ControlClass {
        crate::elements::control::control_elem::ControlClass::Swt
    }
    fn reset_control_side(&mut self) {
        SwtControl::reset_control_side(self);
    }
}

/// Keep the array slot count honest if `SW_MAX` ever moves: the 1-based
/// `present_state[i]` indexing relies on `ARR = SW_MAX + 1`.
const _: () = assert!(ARR == SW_MAX + 1);

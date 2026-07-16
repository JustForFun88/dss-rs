//! The trait plumbing for `TSwtControlObj`: the [`CktElement`] hooks (a control
//! element builds no Yprim and carries zero current) and the [`DssObject`]
//! property accessors (`get_*`/`set_*`, `set_object_ref`, `PropertySideEffects`,
//! `EndEdit`, `MakeLike`, `take_ref_actions`).

use num_complex::Complex64;

use crate::elements::control::control_elem::{CTRL_CLOSE, CTRL_LOCK, CTRL_NONE, CTRL_UNLOCK};
use crate::elements::pos_seq::{PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, ElemRef, SysCtx};
use crate::obj::base::{DssObjData, DssObject, RefAction};

use super::SwtControl;

impl CktElement for SwtControl {
    fn cd(&self) -> &crate::elements::ckt::CktElementData {
        &self.ccd.cd
    }
    fn cd_mut(&mut self) -> &mut crate::elements::ckt::CktElementData {
        &mut self.ccd.cd
    }

    /// Pascal `TControlElem.FControlledElement` - the element this control
    /// acts on (`None` when it drives a list rather than a single element).
    fn controlled_element(&self) -> Option<crate::elements::traits::ElemRef> {
        self.ccd.controlled_element
    }

    fn recalc_element_data(&mut self, _sys: &SysCtx) {
        self.recalc();
    }

    /// Pascal `TControlElem.CalcYPrim`: leave YPrim as NIL.
    fn calc_yprim(&mut self, _sys: &SysCtx) {}

    /// Pascal `TControlElem.GetCurrents`: always zero.
    fn get_currents(&mut self, _sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
        curr.fill(Complex64::ZERO);
    }

    /// Pascal `TSwtControlObj.MakePosSequence` (`Controls/SwtControl.pas:306`):
    /// adopt the controlled (switched) element's phase / conductor counts and
    /// attach terminal 1 to its bus, then run the base bus rename (`inherited`).
    /// Only reads `ControlledElement`, so no `monitored_element_ref` override is
    /// needed.
    fn make_pos_sequence(&mut self, ctx: &PosSeqCtx) -> PosSeqPlan {
        if let Some(c) = &ctx.controlled {
            // FNphases := ControlledElement.NPhases; Nconds := FNphases
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
        }
        // inherited MakePosSequence -> base bus rename.
        PosSeqPlan::base()
    }
}

impl DssObject for SwtControl {
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
        use super::prop::*;
        match idx {
            DELAY => self.ccd.time_delay,
            BASE_FREQ => self.ccd.cd.base_frequency,
            _ => unreachable!("SwtControl has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use super::prop::*;
        match idx {
            DELAY => self.ccd.time_delay = value,
            BASE_FREQ => self.ccd.cd.base_frequency = value,
            _ => unreachable!("SwtControl has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use super::prop::*;
        match idx {
            SWITCHED_TERM => self.ccd.element_terminal,
            // D12 (WP-U1.6, `bb9c9785`): `Normal`/`State` map to their own fields
            // (`NormalState`/`PresentState`), NOT the shared `CurrentAction` the
            // 0.14.5 port used for all three. `Action` keeps `CurrentAction`.
            // (Pascal's `State` read-function `GetState` reports the *live*
            // controlled-element closed state; the port has no live view in this
            // accessor, so it returns the tracked `present_state`, which follows
            // the element after every `State`/action write.)
            ACTION => self.current_action,
            NORMAL => self.normal_state,
            STATE => self.present_state,
            _ => unreachable!("SwtControl has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use super::prop::*;
        match idx {
            SWITCHED_TERM => self.ccd.element_terminal = value,
            // ConditionalReadOnly on `Locked`: a write while locked is ignored.
            // D12: `Action`→`CurrentAction`, `Normal`→`NormalState`,
            // `State`→`PresentState` (the property offsets, now distinct).
            ACTION => {
                if !self.locked {
                    self.current_action = value;
                }
            }
            NORMAL => {
                if !self.locked {
                    self.normal_state = value;
                }
            }
            STATE => {
                if !self.locked {
                    self.present_state = value;
                }
            }
            _ => unreachable!("SwtControl has no integer property {idx}"),
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
    /// `ElemRef` plus a shape snapshot for `RecalcElementData`.
    fn set_object_ref(
        &mut self,
        idx: usize,
        name: String,
        resolved: Option<(ElemRef, &dyn DssObject)>,
    ) {
        use super::prop::*;
        match idx {
            SWITCHED_OBJ => {
                // `name` is the FullName ("Class.name") for the dump.
                self.switched_full_name = name.clone();
                match resolved {
                    Some((r, obj)) => {
                        self.ccd.controlled_element = Some(r);
                        let elem = obj
                            .as_ckt_element()
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

    /// Pascal `TSwtControlObj.PropertySideEffects`. Action/Normal/State are
    /// `ConditionalReadOnly` on `Locked`, so their side effect is skipped while
    /// locked (mirroring the ignored write in `set_i32`).
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        use super::prop::*;
        match idx {
            // D12 (WP-U1.6, `bb9c9785`): `Normal` writes `NormalState` (offset),
            // then the side effect syncs `CurrentAction := NormalState` (was the
            // reverse `NormalState := CurrentAction`).
            NORMAL => {
                if self.locked {
                    return;
                }
                self.current_action = self.normal_state;
            }
            ACTION => {
                if self.locked {
                    return;
                }
                // Default to the first action specified for legacy scripts.
                if self.normal_state == CTRL_NONE {
                    self.normal_state = self.current_action;
                }
            }
            LOCK => {
                self.lock_command = if self.locked { CTRL_LOCK } else { CTRL_UNLOCK };
            }
            // D12: `State` writes `PresentState` (offset), then the side effect
            // syncs `CurrentAction := PresentState` (was `PresentState :=
            // CurrentAction`) and forces the controlled element to that state.
            STATE => {
                if self.locked {
                    return;
                }
                self.current_action = self.present_state;
                if self.normal_state == CTRL_NONE {
                    self.normal_state = self.present_state;
                }
                // Force the controlled element to the new state (Pascal
                // `ControlledElement.Closed[0] := …`), deferred as a RefAction
                // since the property engine holds no mutable view of the target.
                if let Some(target) = self.ccd.controlled_element {
                    self.pending_ref_actions.push(RefAction::SetSwitchClosed {
                        target,
                        terminal: self.ccd.element_terminal as usize,
                        closed: self.present_state == CTRL_CLOSE,
                    });
                }
            }
            _ => {}
        }
    }

    /// Pascal `TCktElementClass.EndEdit` default → `RecalcElementData`.
    fn end_edit(&mut self) {
        self.recalc();
    }

    fn take_ref_actions(&mut self) -> Vec<RefAction> {
        std::mem::take(&mut self.pending_ref_actions)
    }

    /// Pascal `TSwtControlObj.MakeLike`.
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(other) = other.as_any().downcast_ref::<SwtControl>() else {
            return;
        };
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
        self.current_action = other.current_action;
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

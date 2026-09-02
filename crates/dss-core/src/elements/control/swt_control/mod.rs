//! Port of `Controls/SwtControl.pas` — `TSwtControlObj`, a manual/automatic
//! **switch** control: it opens or closes the phase conductors of a controlled
//! element's terminal and can be *locked* against further operation. Unlike the
//! sensing controls (CapControl/RegControl), SwtControl reads no monitored
//! quantity — it acts on an explicit `Action`/`State` command.
//!
//! Like every `TControlElem` it builds no Yprim and carries zero terminal
//! current; its single terminal attaches to the switched element's terminal bus
//! (`RecalcElementData`).
//!
//! **Per-phase state model (RP3.7, EPRI r4133 authority).** r4133 keeps the
//! switch state per phase — `FPresentState`/`FNormalState : pStateArray`
//! (`Version8/Source/Controls/SwtControl.pas:37-38`, `StateArray =
//! Array[1..SWTCONTROLMAXDIM=6] of EControlAction`, `:14-19`), settable
//! phase-by-phase from a quoted list or ganged from a bare token
//! (`InterpretSwitchState`, `:410-482`), each phase driving its own conductor
//! (`set_States` `:532-549`, `RecalcElementData` `:347-355`), and rendered one
//! token per controlled-element phase (`:573-623`). The port held one scalar
//! per field until RP3.7 A1 replaced it with these arrays; A2 flipped the
//! property surface to the r4133 per-phase render (`Normal`/`State` are
//! `MappedStringEnumArray`s looping the LIVE controlled-element phase count)
//! and to the r4133 name-based lock rule (a locked `normal=` write applies,
//! locked `state=`/`action=` do not — probe transcript `tmp/rp37/probe.md`
//! §10 is the implementation input).
//!
//! **The write seam.** `Normal`/`State` take the RAW value through
//! [`DssObject::set_enum_array_raw`](crate::obj::base::DssObject::set_enum_array_raw)
//! — the one hook that can carry the two things r4133's writer keys on and the
//! generic ordinal tokenizer cannot: `Parser.WasQuoted` (a quoted value goes
//! phase-by-phase, a bare token ganged, `:433-480`) and the FIRST-CHARACTER
//! token match with no else arm (`:438-441/:464-467`). `Action` keeps the
//! scalar `MappedStringEnum` seam and routes its decoded ordinal back through
//! the same [`SwtControl::interpret_switch_state`], so the interpreter is the
//! single write mechanics for all three properties. Seams with no outer parser
//! (JSON import, the `MakePosSequence` applier, direct unit-test calls) leave
//! `was_quoted` false and the class reconstructs the quoted case from the value
//! itself ([`SwtControl::value_implies_quoted`]).
//!
//! **Bound decision (probe §10):** r4133's `Create` allocates `FNPhases` (=3)
//! entries (`:299-305`) yet reads/writes up to 6 — a latent upstream heap OOB
//! absorbed by FastMM (measured live on a 4-phase controlled element, probe
//! P2(iv-b)). The port keeps all six slots in-bounds and initialized all-CLOSED
//! like `Create`'s initialized slots: observables (render token count = the
//! controlled element's `NPhases`, per-phase drive of each conductor, the
//! 5-token per-phase parse cap, ganged drive) stay r4133-exact, the OOB is not
//! reproduced.
//!
//! **Parse-time drive model.** r4133's `set_States` drives
//! `ControlledElement.Closed[Idx]` immediately mid-parse (`:532-549`) and
//! `RecalcElementData` re-drives every phase from `FPresentState` at the end of
//! Edit (`:347-355`, called at `:234`). The port's property engine holds no
//! mutable view of the target mid-parse, so the drive is deferred as a
//! [`RefAction::SetConductorsClosed`] queued by [`SwtControl::recalc`] (the
//! `EndEdit` → `RecalcElementData` seam) and applied by the executive right
//! after the edit — equivalent because nothing reads the target in between and
//! the end-of-Edit drive is the last one in both engines.
//!
//! That equivalence has **one exception**, deliberately not reproduced (RP3.7
//! FIX-A1, verify-A1 finding F6): a mid-edit `switchedobj=` re-point. r4133's
//! `set_States` drives whatever `ControlledElement` currently is, and
//! `RecalcElementData` re-points it only at the END of `Edit` (`:234`), so
//! `edit swtcontrol.x switchedobj=line.b state=open` opens line.**a** (the
//! element it no longer controls, left open) *and* line.b. The port resolves
//! `switchedobj=` first and drives only the new target. Operating an element
//! you have just stopped controlling is an upstream bug, not a semantic — no
//! corpus deck writes both properties in one edit.
//!
//! Concern split mirrors the other controls: this file holds the property
//! metadata, the [`SwtControl`] struct, construction/`recalc`, the r4133
//! `InterpretSwitchState` mechanics, and the `Sample`/`DoPendingAction`/`Reset`
//! behavior; [`accessors`] holds the
//! [`CktElement`](crate::elements::traits::CktElement)/[`DssObject`](crate::obj::base::DssObject)
//! trait impls.

#[cfg(test)]
mod tests;

mod accessors;

use crate::elements::control::control_elem::{
    ControlAction, ControlElemData, CtrlCtx, RefSnapshot,
};
use crate::elements::traits::CktElement;
use crate::obj::base::RefAction;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use dss_parser::{Parser, ParserVars};

/// 1-based property ordinals (Pascal `TSwtControlProp` + the `TCktElementClass`
/// tail).
pub mod prop {
    pub const SWITCHED_OBJ: usize = 1;
    pub const SWITCHED_TERM: usize = 2;
    pub const ACTION: usize = 3;
    pub const LOCK: usize = 4;
    pub const DELAY: usize = 5;
    pub const NORMAL: usize = 6;
    pub const STATE: usize = 7;
    pub const RESET: usize = 8;
    /// r4133 informational continuous rating (WP-U2.4). Not used in power flow or
    /// reporting; hidden from the 0.14.5-pinned full-enumeration surfaces.
    pub const RATED_CURRENT: usize = 9;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 10;
    pub const ENABLED: usize = 11;
    pub const NUM_PROPS: usize = 12; // incl. Like
}

/// `SWTCONTROLMAXDIM` — the per-phase state-array bound
/// (`Version8/Source/Controls/SwtControl.pas:14`).
pub(crate) const SW_MAX: usize = 6;

/// `StateArray = Array[1..SWTCONTROLMAXDIM] of EControlAction`
/// (`SwtControl.pas:18-19`), stored 1-based like the Relay twin
/// (`relay::ARR`): slot 0 is unused so `arr[i]` lines up with the Pascal index.
const ARR: usize = SW_MAX + 1;

/// Which state property an [`SwtControl::interpret_switch_state`] write targets
/// — the r4133 guard and ganged/per-phase split key on the property NAME's
/// first char (`SwtControl.pas:416-419`: `a`ction / `s`tate / `n`ormal).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SwtStateProp {
    Action,
    State,
    Normal,
}

/// `TSwtControl.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    let defs = vec![
        // Pascal `DSSObjectReferenceProperty` (offset2 = 0): any circuit element
        // by full name; the dump renders `Class.name`.
        PropDef::object_ref_any("SwitchedObj"),
        PropDef::integer("SwitchedTerm"),
        // Action: the deprecated ganged alias of State. r4133 has NO getter arm
        // for index 3 — `?`/dump answer the raw parse store (`SwtControl.pas`
        // InitPropertyValues `:652`, Edit `:192-193`; the `PROPS_ECHO_R4133`
        // `EchoParse` row owns that divergence on the r4133 channel); the port
        // keeps the 0.14.5 scalar `CurrentAction` readback (the capi channel's
        // render). Writes run the r4133 mechanics: ganged, first-char match,
        // name-based lock guard (`InterpretSwitchState` `:419-429/:416-417`).
        PropDef::mapped_string_enum("Action", enums.swt_control_action).flags(PropFlags::REDUNDANT),
        PropDef::boolean("Lock"),
        PropDef::double("Delay").flags(PropFlags::UNITS_S),
        // Normal/State: the r4133 per-phase state arrays (RP3.7). The render
        // loops the LIVE controlled-element phase count exactly as r4133
        // (`GetPropertyValue` `:589-599/:600-610`); the write takes the raw
        // value through `set_enum_array_raw` → `interpret_switch_state`
        // (ganged-vs-per-phase keyed on WasQuoted, first-char token match,
        // 5-token per-phase cap — `:410-482`).
        PropDef::mapped_string_enum_array("Normal", enums.swt_control_state)
            .flags(PropFlags::DYNAMIC_DEFAULT),
        PropDef::mapped_string_enum_array("State", enums.swt_control_state)
            .flags(PropFlags::NO_DEFAULT),
        // Pascal BooleanActionProperty (DoReset); the getter is always 0.
        PropDef::boolean("Reset"),
        // r4133 `RatedCurrent` (SwtControl.pas props 8->9): informational
        // continuous rating, default 0.0, "Not used internally for either power
        // flow or reporting." WP-U2.5 brought the SwtControl `Dump commands` block
        // to its full r4133 9-prop shape (self-referential golden), so it no longer
        // hides from the full-enum surface; `compare_all_properties` still excludes
        // it from the 0.14.5 property-table walk via the name-based PROPS_015X row.
        PropDef::double("RatedCurrent"),
        // TCktElementClass tail:
        PropDef::double("BaseFreq").flags(
            PropFlags::DYNAMIC_DEFAULT
                | PropFlags::NON_NEGATIVE
                | PropFlags::NON_ZERO
                | PropFlags::UNITS_HZ,
        ),
        PropDef::enabled("Enabled"),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);
    ClassProps::new("SwtControl", defs, true)
}

/// r4133 `case LowerCase(DataStr2)[1] of 'o': CTRL_OPEN; 'c': CTRL_CLOSE`
/// (`SwtControl.pas:438-441` ganged, `:464-467` per-phase) — tokens match on
/// the FIRST CHARACTER ONLY, case-insensitively, and any other first char
/// leaves the slot unchanged (the Pascal `case` has no else arm).
fn match_state_token(token: &str) -> Option<ControlAction> {
    match token.as_bytes().first()?.to_ascii_lowercase() {
        b'o' => Some(ControlAction::Open),
        b'c' => Some(ControlAction::Close),
        _ => None,
    }
}

/// `TSwtControlObj`.
#[derive(Debug, Clone)]
pub struct SwtControl {
    pub ccd: ControlElemData,
    /// Dump name of the switched (controlled) element (Pascal `FullName`).
    switched_full_name: String,
    /// Parse-time shape snapshot of the controlled element.
    ctrl_snap: Option<RefSnapshot>,

    /// `FPresentState : pStateArray` (`SwtControl.pas:37`) — the switch's live
    /// position per phase (slots `1..=SW_MAX`; r4133 allocates 3 entries at
    /// `Create` and reads/writes up to 6 out of bounds — the port keeps all six
    /// in-bounds and initialized, see the module doc).
    present_state: [ControlAction; ARR],
    /// `FNormalState : pStateArray` (`SwtControl.pas:38`) — the reset target
    /// per phase. r4133 `Create` initializes it all-CLOSED with
    /// `NormalStateSet = FALSE` (`:299-307`); the scalar-era `None`-until-first
    /// readback survives RP3.7 A1 through [`SwtControl::normal_view`] gating on
    /// [`Self::normal_state_set`] (A2's array render shows the r4133
    /// all-closed array directly).
    normal_state: [ControlAction; ARR],
    /// `NormalStateSet` (`SwtControl.pas:42`): FALSE until the first
    /// `Normal`/`State`/`Action` write; the Edit supplemental copies Present
    /// into Normal once, on the first `State`/`Action` write (`:221-228`).
    normal_state_set: bool,

    /// `CurrentAction` — the 0.14.5 scalar action field the port's
    /// `Sample`/`DoPendingAction` control-queue machinery (pinned by the
    /// capi-lane `swtcontrol_lock.dss`) still reads. r4133 has no such field
    /// (its Sample body is commented out, `:484-507`); the port syncs it as the
    /// *ganged view* of the per-phase arrays so the queue branch stays inert
    /// after `State`/`Action` writes (the D6 invariant the corpus decks pin).
    current_action: ControlAction,
    /// `LockCommand` (CTRL_NONE / CTRL_LOCK / CTRL_UNLOCK) — queued on `Sample`.
    lock_command: ControlAction,
    /// `Locked` (`FLocked`, `SwtControl.pas:40`).
    locked: bool,
    /// `Armed` — a queue action is outstanding.
    armed: bool,

    /// `RatedCurrent` (r4133) — switch continuous rated current in Amps.
    /// Informational only; not used in power flow or reporting.
    rated_current: f64,

    /// Deferred parse-time element forces (the `State=`/`Reset` side effects).
    pending_ref_actions: Vec<RefAction>,
}

impl SwtControl {
    /// Pascal `TSwtControlObj.Create` (`SwtControl.pas:279-315`): both state
    /// arrays all-CLOSED (`:302-305`), `NormalStateSet = FALSE` (`:307`).
    pub fn new(name: &str) -> Self {
        let mut ccd = ControlElemData::new(name, prop::NUM_PROPS);
        ccd.cd.nphases = 3; // directly set conds and phases
        ccd.cd.nconds = 3;
        ccd.cd.set_nterms(1); // forces allocation of terminals and conductors
        ccd.element_terminal = 1;
        ccd.time_delay = 120.0; // 2 minutes

        Self {
            ccd,
            switched_full_name: String::new(),
            ctrl_snap: None,
            // r4133 initializes slots 1..Min(6, FNPhases=3) to CTRL_CLOSE; the
            // port initializes all six (the module-doc bound decision).
            present_state: [ControlAction::Close; ARR],
            normal_state: [ControlAction::Close; ARR],
            normal_state_set: false,
            current_action: ControlAction::Close,
            lock_command: ControlAction::None,
            locked: false,
            armed: false,
            rated_current: 0.0, // r4133 default
            pending_ref_actions: Vec::new(),
        }
    }

    /// The control's own `FullName` (`SwtControl.<name>`), for the event log.
    fn full_name(&self) -> String {
        format!("SwtControl.{}", self.ccd.cd.obj.name())
    }

    /// The ganged (scalar) view of a per-phase state array: slot 1. Every
    /// corpus deck writes ganged (all slots equal), so the view is exact there;
    /// it feeds the 0.14.5 queue machinery (`sample`) and the scalar `Action`
    /// readback glue only.
    fn ganged_view(arr: &[ControlAction; ARR]) -> ControlAction {
        arr[1]
    }

    /// Ganged write of one state array, r4133 `for i := 1 to SWTCONTROLMAXDIM`
    /// (`SwtControl.pas:421` action, `:435` state/normal): every slot, not just
    /// the controlled element's phases.
    fn set_all(arr: &mut [ControlAction; ARR], state: ControlAction) {
        for slot in arr.iter_mut().take(SW_MAX + 1).skip(1) {
            *slot = state;
        }
    }

    /// The render/drive bound: the **live** controlled-element phase count.
    ///
    /// r4133 renders one token per `ControlledElement.NPhases`
    /// (`GetPropertyValue` `:591` Normal / `:602` State) and drives / resets
    /// `1 .. Min(SWTCONTROLMAXDIM, ControlledElement.Nphases)`
    /// (`RecalcElementData` `:346`, `Reset` `:633`); with `ControlledElement =
    /// NIL` both getters return the bare `'[]'` (`:589`/`:600`, measured on the
    /// orphan control — `tmp/rp37/out_nil_controlled.txt`). The port reads the
    /// count off the controlled-element snapshot, which
    /// [`SwtControl::recalc`] refreshes at every `EndEdit` and
    /// `make_pos_sequence` refreshes from the live `PosSeqCtx` — so after
    /// `makeposseq` the render follows the now-1-phase element exactly as
    /// r4133's live loop does (`tmp/rp37/probe.md` §6/§11.4).
    ///
    /// The `min(SW_MAX)` clip is the module-doc bound decision: r4133 loops the
    /// element's raw `NPhases` past its 3-entry allocation (probe P2(iv-b)
    /// renders four tokens off a 4-phase element); the port matches every
    /// measured observable while staying inside its own six initialized slots.
    pub(crate) fn state_size(&self) -> usize {
        if self.ccd.controlled_element.is_none() {
            return 0; // `ControlledElement = NIL` → `'[]'`
        }
        self.ctrl_snap
            .as_ref()
            .map_or(self.ccd.cd.nphases, |s| s.nphases)
            .min(SW_MAX)
    }

    /// Reconstruct Pascal `Parser.WasQuoted` for a value that reached the class
    /// without an outer parser (`PropEngine::was_quoted == false`: JSON import,
    /// the `MakePosSequence` applier, direct unit-test seams).
    ///
    /// The executive strips the quote pair before the property arm runs, so a
    /// quoted value arrives as its bare content — but the two shapes r4133
    /// distinguishes are still visible: a value that still carries its opening
    /// bracket/quote (nothing strips it on these seams), or one that holds more
    /// than one token. A single bare token is the only genuinely ambiguous
    /// case, and it is resolved as *ganged* — the r4133 spelling every corpus
    /// deck writes (`normal=closed`, `action=o`, probe §7 sweep). The
    /// distinction that ambiguity loses (`state=(open)` writes phase 1 only —
    /// measured on the r4133 DLL, `tmp/rp37/out_a2a.txt` A2a(1)) is carried
    /// faithfully wherever the outer parser runs, i.e. every DSS script.
    fn value_implies_quoted(value: &str) -> bool {
        let v = value.trim_start();
        if matches!(
            v.as_bytes().first(),
            Some(b'(' | b'[' | b'{' | b'"' | b'\'')
        ) {
            return true;
        }
        value
            .split([' ', '\t', '\r', '\n', ','])
            .filter(|t| !t.is_empty())
            .count()
            > 1
    }

    /// Pascal `TSwtControlObj.InterpretSwitchState`
    /// (`Version8/Source/Controls/SwtControl.pas:410-482`), the r4133 write
    /// mechanics for all three state properties:
    ///
    /// - lock guard (`:416-417`): while `Locked`, a write whose property name
    ///   starts with `a` (Action) or `s` (State) exits without touching
    ///   anything; `Normal` (starts with `n`) still applies — "Only allowed to
    ///   change normal state if locked" (probe P3 confirmed it verbatim on the
    ///   r4133 DLL; RP3.7 A2 made this the observable property rule in BOTH
    ///   lanes — 0.14.5's `ConditionalReadOnly` refuses all three, a deliberate
    ///   lane divergence with zero corpus exposure, probe §7 sweep).
    /// - `Action` is ALWAYS ganged (`:419-429`), quoted or not, matching the
    ///   whole param's first character.
    /// - `State`/`Normal`: ganged when the value was NOT quoted (`:433-451`,
    ///   slots 1..6); quoted values go phase-by-phase through the AuxParser
    ///   (`:453-480`) — at most FIVE tokens honored (loop bound
    ///   `i < SWTCONTROLMAXDIM`, `:461`), unlisted slots unchanged.
    /// - tokens match on the first character only via [`match_state_token`];
    ///   a non-matching token leaves its slot unchanged (no else arm).
    ///
    /// **Two r4133 defects are deliberately NOT reproduced** (2026-08-02
    /// policy; both recorded by RP3.7 FIX-A1, verify-A1 findings F4/F5):
    ///
    /// 1. *The `Else`-without-`Begin` fall-through* (`:452-455`). Only
    ///    `AuxParser.CmdString := param` is under the `Else // process phase by
    ///    phase`; the `DataStr` reads and the per-phase `While` loop
    ///    (`:457-480`) sit in the enclosing `Begin` and therefore run on BOTH
    ///    paths. After an UNQUOTED ganged fill r4133 re-reads whatever the
    ///    GLOBAL `AuxParser` still holds and applies it per phase from slot 1 —
    ///    usually inert (a drained AuxParser yields an empty token), but
    ///    reachable: the quoted branch stops at five tokens (`:461`), so a
    ///    7-token quoted list leaves residue that the NEXT unquoted `state=`
    ///    write consumes as per-phase states. `Relay.pas:1277-1306` carries the
    ///    identical shape; `Fuse.pas:569-597` has the `Else Begin` the other
    ///    two are missing, which is what makes this an upstream slip rather
    ///    than a design. The port scopes the per-phase loop to the quoted
    ///    branch and keeps a FRESH parser per call, so no state can leak
    ///    between writes.
    /// 2. *The empty-`ParamName` guard read* (`:417`). The guard keys on
    ///    `LowerCase(property_name[1])` where `property_name = ParamName`,
    ///    which `Edit` leaves EMPTY for a POSITIONAL token
    ///    (`:183-185`: `IF Length(ParamName) = 0 THEN Inc(ParamPointer)`);
    ///    indexing `[1]` of an empty Delphi string is undefined, so a locked
    ///    positional `state=` write is unguarded or faults. The port keys on
    ///    the property identity, so the lock rule holds for positional and
    ///    named writes alike. Zero corpus exposure — no deck writes a
    ///    SwtControl state positionally.
    ///
    /// The slot writes land in the arrays only; the controlled element is
    /// driven by [`SwtControl::recalc`]'s deferred per-phase force (the module
    /// doc's parse-time drive model), and the "normal defaults to present"
    /// supplemental (`:221-228`) lives in the property side effects where the
    /// executive's Edit sequence runs it.
    pub(crate) fn interpret_switch_state(
        &mut self,
        prop: SwtStateProp,
        param: &str,
        was_quoted: bool,
    ) {
        // `:416-417` — the guard keys on the property NAME's first char.
        if self.locked && !matches!(prop, SwtStateProp::Normal) {
            return;
        }
        match prop {
            // `:419-429`: action is ganged regardless of quoting.
            SwtStateProp::Action => {
                if let Some(state) = match_state_token(param) {
                    Self::set_all(&mut self.present_state, state);
                }
            }
            SwtStateProp::State | SwtStateProp::Normal => {
                if !was_quoted {
                    // `:433-451`: ganged specification.
                    if let Some(state) = match_state_token(param) {
                        let arr = if prop == SwtStateProp::State {
                            &mut self.present_state
                        } else {
                            &mut self.normal_state
                        };
                        Self::set_all(arr, state);
                    }
                } else {
                    // `:453-480`: phase by phase through the AuxParser.
                    let mut parser = Parser::new();
                    let vars = ParserVars::new();
                    parser.set_auto_increment(false);
                    parser.set_cmd_string(param);
                    parser.next_param(&vars); // name slot — ignored, Pascal `:457`
                    let mut token = parser.make_string(&vars);
                    let mut i = 1usize;
                    // `:461` `While (Length(DataStr2)>0) and (i<SWTCONTROLMAXDIM)`
                    // — a 6th token is silently dropped (probe §11.2).
                    while !token.is_empty() && i < SW_MAX {
                        if let Some(state) = match_state_token(&token) {
                            if prop == SwtStateProp::State {
                                self.present_state[i] = state;
                            } else {
                                self.normal_state[i] = state;
                            }
                        }
                        parser.next_param(&vars);
                        token = parser.make_string(&vars);
                        i += 1;
                    }
                }
            }
        }
    }

    /// Pascal `Edit` supplemental (`SwtControl.pas:221-228`), run by the
    /// property side effects after an `Action`/`State` write: on the FIRST such
    /// write (`not NormalStateSet`), copy Present into Normal per phase over
    /// `i := 1 to FNPhases` (the control's own phase count at edit time), then
    /// latch `NormalStateSet`. The port bounds the loop at `SW_MAX` (r4133's
    /// `FNPhases`-unbounded loop reads/writes past the 3-entry allocation —
    /// module-doc bound decision).
    fn normal_defaults_to_present(&mut self) {
        if !self.normal_state_set {
            let n = self.ccd.cd.nphases.clamp(1, SW_MAX);
            self.normal_state[1..=n].copy_from_slice(&self.present_state[1..=n]);
            self.normal_state_set = true;
        }
    }

    /// Queue the per-phase `RecalcElementData` Closed[i] drive
    /// (`SwtControl.pas:347-355`): one entry per controlled-element phase,
    /// `closed = FPresentState^[i] <> CTRL_OPEN` (the Pascal `else` branch
    /// closes for every non-OPEN ordinal). Deferred as a [`RefAction`] — the
    /// module-doc parse-time drive model. The bound is
    /// [`SwtControl::state_size`], the same live count the `Normal`/`State`
    /// render loops, so what the property surface shows and what the element
    /// carries can never disagree.
    fn queue_phase_drive(&mut self) {
        let n = self.state_size();
        if let Some(target) = self.ccd.controlled_element {
            let closed: Vec<bool> = (1..=n)
                .map(|i| self.present_state[i] != ControlAction::Open)
                .collect();
            self.pending_ref_actions
                .push(RefAction::SetConductorsClosed {
                    target,
                    terminal: self.ccd.element_terminal.max(1) as usize,
                    closed,
                });
        }
    }

    /// Pascal (FPC 0.14.5) `TSwtControlObj.Sample`: push the pending lock command
    /// (if any) and the pending switch action onto the control queue at the current
    /// time delay. Reads only the control's own state — no monitored quantity.
    ///
    /// NOTE (r4133-fidelity gap, out of the r4088→r4133 delta): the Delphi engine
    /// (r4088 *and* r4133) comments out this ENTIRE body — "Removing because action
    /// (redirects to state) and lock are instantaneous" — so on r4133 `Sample`
    /// queues nothing. This port keeps the FPC 0.14.5 body because it is pinned by
    /// the default (capi015 = FPC 0.14.5) oracle: `swtcontrol_lock.dss`
    /// (`compare_ctrlqueue`) verifies the spurious `CTRL_LOCK` push and so cannot
    /// flip to `oracle: "r4133"`. The D6 fix makes the action path inert on r4133's
    /// action/state decks (`current_action` tracks the per-phase arrays' ganged
    /// view after an immediate force ⇒ the action-queue branch is false), but the
    /// LOCK branch still queues a `CTRL_LOCK` that r4133 does not — a latent gap
    /// blocking future r4133 lock-path coverage, to be closed when this Sample body
    /// is retired.
    ///
    /// **A second, LIVE channel (RP3.7 FIX-A1, verify-A1 finding F2 — measured,
    /// not latent):** 0.14.5 maps all three state properties onto the single
    /// `CurrentAction` field, so the retained glue
    /// (`side_effects(NORMAL)` → `current_action = ganged_view(normal_state)`)
    /// leaves `current_action = Open` while `present_state` is still `Close`
    /// after a plain `normal=open` — exactly this method's arming condition.
    /// The port then queues the action and **opens a switch r4133 never
    /// touches**: on the `swtcontrol_lock`-shaped deck with `lock=no`,
    /// `normal=open` + duty steps opens it three steps later with an
    /// `Action=OPENED` event (`tmp/rp37/out_port_probe10.txt`), where the
    /// r4133 DLL keeps `[closed, closed, closed, ]` for every step and logs
    /// nothing (`tmp/rp37/out_fixa1.txt` §F2). So the "queue branch stays
    /// inert" invariant holds for `State`/`Action` writes only. Corpus exposure
    /// is zero (every corpus `normal=` is a ganged `normal=closed` over an
    /// all-closed present state — probe §7 sweep), which is why this is
    /// recorded rather than fixed here: retiring the body requires re-gating
    /// `swtcontrol_lock.dss` off the capi channel (a manifest/ledger change),
    /// and the `Action` readback the capi015 props golden pins rides on the
    /// same glue. Tripwire:
    /// `tests::sample_arms_on_a_normal_write_the_retained_capi_channel`.
    pub(crate) fn sample(&mut self, ctx: &mut CtrlCtx) {
        if self.lock_command != ControlAction::None {
            ctx.queue.push_delay(
                ctx.int_hour,
                ctx.t,
                self.ccd.time_delay,
                self.lock_command.ordinal(),
                0,
                ctx.self_ref,
            );
            self.lock_command = ControlAction::None; // reset for next time
        }

        // 0.14.5 compares the scalar `CurrentAction <> PresentState`; the
        // per-phase port compares the ganged view (slot 1) — exact for the
        // homogeneous corpus decks (the only place this branch is pinned).
        if self.current_action != Self::ganged_view(&self.present_state) && !self.armed {
            // we need to operate this switch
            ctx.queue.push_delay(
                ctx.int_hour,
                ctx.t,
                self.ccd.time_delay,
                self.current_action.ordinal(),
                0,
                ctx.self_ref,
            );
            self.armed = true;
        }
    }

    /// Pascal `TSwtControlObj.DoPendingAction`: execute a popped queue action —
    /// lock/unlock, or (when not locked) open/close all phases of the switched
    /// terminal, logging the operation. `ctrl` is the switched element.
    ///
    /// This is the 0.14.5 control-queue machinery (r4133 comments the body out,
    /// `SwtControl.pas:396-408`), scalar by construction (`Closed[0]` =
    /// all conductors of the terminal): the per-phase arrays follow ganged —
    /// every slot takes the operated state.
    pub(crate) fn do_pending_action(
        &mut self,
        code: i32,
        ctrl: &mut dyn CktElement,
        ctx: &mut CtrlCtx,
    ) {
        let term = self.ccd.element_terminal.max(1) as usize;
        // Pascal sets `ControlledElement.ActiveTerminalIdx := ElementTerminal`
        // before the `case` — for *every* code, incl. LOCK/UNLOCK.
        if term <= ctrl.cd().nterms {
            ctrl.cd_mut().active_terminal = term - 1;
        }
        let action = ControlAction::from_ordinal(code);
        match action {
            ControlAction::Lock => self.locked = true,
            ControlAction::Unlock => self.locked = false,
            _ => {
                if !self.locked {
                    let view = Self::ganged_view(&self.present_state);
                    if action == ControlAction::Open && view == ControlAction::Close {
                        ctrl.cd_mut().set_terminal_closed(term, false); // open all phases
                        Self::set_all(&mut self.present_state, ControlAction::Open);
                        ctx.events.append(
                            &self.full_name(),
                            "Opened",
                            ctx.int_hour,
                            ctx.t,
                            ctx.control_iter,
                        );
                        // Pascal `Closed[]` sets YprimInvalid -> SystemYChanged.
                        *ctx.system_y_changed = true;
                    }
                    if action == ControlAction::Close && view == ControlAction::Open {
                        ctrl.cd_mut().set_terminal_closed(term, true); // close all phases
                        Self::set_all(&mut self.present_state, ControlAction::Close);
                        ctx.events.append(
                            &self.full_name(),
                            "Closed",
                            ctx.int_hour,
                            ctx.t,
                            ctx.control_iter,
                        );
                        *ctx.system_y_changed = true;
                    }
                    self.armed = false; // reset the switch
                }
            }
        }
    }

    /// Pascal `TSwtControlObj.Reset` control-side (`SwtControl.pas:629-634`):
    /// per-phase `FPresentState[i] := FNormalState[i]`, guarded by `not
    /// Locked`. `true` when the reset ran (not locked) — the caller applies the
    /// element force. The 0.14.5 `CurrentAction := PresentState` glue follows
    /// the ganged view.
    pub(crate) fn reset_control_side(&mut self) -> bool {
        if self.locked {
            return false;
        }
        self.present_state[1..=SW_MAX].copy_from_slice(&self.normal_state[1..=SW_MAX]);
        self.current_action = Self::ganged_view(&self.normal_state);
        self.armed = false;
        true
    }

    /// Pascal `TSwtControlObj.Reset` (full, `SwtControl.pas:625-646`): restore
    /// the control state per phase and force the switched element's conductors
    /// to `FNormalState` (`case FNormalState[i] of CTRL_OPEN: FALSE else TRUE`
    /// — closed for every non-OPEN ordinal), `1..Min(6,
    /// ControlledElement.Nphases)`. Returns whether a force was applied (the
    /// caller raises `SystemYChanged`).
    ///
    /// The force is **unconditional** when not locked — Pascal's `Closed[i] := …`
    /// raises `SystemYChanged` every time, and gating on an all-or-nothing change
    /// check (`terminal_all_phases_closed`) would miss a real change on a
    /// *partially*-open terminal (one phase closed, the rest open → "all closed"
    /// reads false, so `was == want == open` and the flip of the closed phase
    /// slips past the Y rebuild, leaving a stale system Y). Reset is rare, so the
    /// occasional redundant rebuild is negligible.
    pub(crate) fn reset_with(&mut self, ctrl: &mut dyn CktElement) -> bool {
        if !self.reset_control_side() {
            return false; // locked → no-op
        }
        let term = self.ccd.element_terminal.max(1) as usize;
        let n = ctrl.cd().nphases.min(SW_MAX);
        // The executive's `SetConductorsClosed` application idiom
        // (`exec/command.rs`): enumerate + 1-based conductor.
        let closed: Vec<bool> = (1..=n)
            .map(|i| self.normal_state[i] != ControlAction::Open)
            .collect();
        for (i, &c) in closed.iter().enumerate() {
            ctrl.cd_mut().set_conductor_closed(term, i + 1, c);
        }
        true
    }

    /// Pascal `DoReset` (the `Reset=` boolean-action property,
    /// `SwtControl.pas:208-212`): force an unlock, then run `Reset`. At parse
    /// time the controlled-element force is deferred as a per-phase
    /// [`RefAction::SetConductorsClosed`] over [`SwtControl::state_size`]
    /// (r4133 drives `1..Min(6, ControlledElement.Nphases)`, `:633`).
    fn do_reset_action(&mut self) {
        self.locked = false;
        if !self.reset_control_side() {
            return;
        }
        let n = self.state_size();
        let Some(target) = self.ccd.controlled_element else {
            return;
        };
        let closed: Vec<bool> = (1..=n)
            .map(|i| self.normal_state[i] != ControlAction::Open)
            .collect();
        self.pending_ref_actions
            .push(RefAction::SetConductorsClosed {
                target,
                terminal: self.ccd.element_terminal.max(1) as usize,
                closed,
            });
    }

    /// Pascal `TSwtControlObj.RecalcElementData` (`SwtControl.pas:326-365`):
    /// take the controlled element's phase count and attach the control's
    /// terminal to the switched bus, then drive the controlled element's
    /// conductors per phase from `FPresentState` (`:347-355`). With no
    /// `SwitchedObj`, Pascal raises error 387 and exits.
    fn recalc(&mut self) {
        let Some(ctrl) = self.ctrl_snap.clone() else {
            self.ccd.cd.obj.push_error(format!(
                "SwtControl: \"{}\": SwitchedObj is not set. Element must be defined previously. (Error 387)",
                self.ccd.cd.obj.name()
            ));
            return;
        };
        self.ccd.cd.nphases = ctrl.nphases;
        self.ccd.cd.set_nconds(ctrl.nphases); // Nconds := FNphases
        // attach controller bus to the switch bus (terminal `ElementTerminal`).
        let t = self.ccd.element_terminal;
        let bus = if t >= 1 && (t as usize) <= ctrl.buses.len() {
            ctrl.buses[(t - 1) as usize].clone()
        } else {
            String::new() // Pascal GetBus(i) out of range yields ''
        };
        self.ccd.cd.set_bus(1, &bus);

        // `:347-355` — the per-phase Open/Closed drive of the controlled
        // element, deferred (module-doc parse-time drive model).
        self.queue_phase_drive();
    }
}

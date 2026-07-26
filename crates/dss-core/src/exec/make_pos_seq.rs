//! `MakePosSeq` — the positive-sequence conversion driver (Pascal
//! `TExecHelper.DoMakePosSeq`, `ExecHelper.pas:3035-3047`).
//!
//! `DoMakePosSeq` sets `ActiveCircuit.PositiveSequence := TRUE`, then walks
//! **every** `CktElement` in creation order and calls its `MakePosSequence()`.
//! Each per-class `MakePosSequence` mutates its own direct fields (bus/phase
//! resyncs, buffer reallocs) and drives a run of the typed property setters
//! (`SetInteger`/`SetDouble`/…), each of which auto-wraps `BeginEdit(True) …
//! EndEdit(1)` when not already editing.
//!
//! In this port the per-class override splits into two halves (frozen at
//! WPG.21): the direct self-mutations run inside `make_pos_sequence`, and the
//! typed-setter run is returned as an ordered [`PosSeqPlan`]. This module is the
//! *applier*: it resolves the read-only [`PosSeqCtx`] the override needs, calls
//! the override, then replays the plan through the same typed setters
//! (`ClassProps::set_prop_*`) under the editing-active bracketing VM that the
//! Pascal `SetInteger`/`SetDouble` helpers embed (`DSSObjectHelper.pas:3060`),
//! runs the base bus rename when the override ended with `inherited`, and drains
//! the shared post-edit signal tail (`command::apply_edit_signal_tail`).

use super::command::apply_edit_signal_tail;
use super::*;
use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx, PosSeqElemInfo, PosSeqPlan};

impl Dss {
    /// Pascal `TExecHelper.DoMakePosSeq` (`ExecHelper.pas:3035`): flip the
    /// circuit to positive-sequence and convert every circuit element to its
    /// equivalent positive-sequence single-phase form, **in creation order** —
    /// so a control created after its DER sees the already-converted DER (the
    /// upstream assumption the decks encode).
    pub(super) fn do_make_pos_seq(&mut self) {
        let Some(ckt) = self.circuit.as_mut() else {
            return; // Pascal touches `ActiveCircuit`; no circuit ⇒ nothing.
        };
        // `ActiveCircuit.PositiveSequence := TRUE` (the `Set Cktmodel=` flag).
        ckt.positive_sequence = true;
        // Snapshot the creation-order handle list so the per-element mutations
        // below can borrow `self` freely (`CktElements` is never grown here).
        let refs: Vec<ElemId> = ckt.ckt_elements.clone();

        for r in refs {
            let ctx = self.build_pos_seq_ctx(r);
            let plan = match self.classes[r.class_ord()]
                .arena
                .try_ckt_elem_mut(r.index())
            {
                Some(elem) => elem.make_pos_sequence(&ctx),
                None => continue, // every `CktElements` entry is a circuit element
            };
            self.apply_pos_seq_plan(r, &plan);
        }
    }

    /// Resolve the read-only [`PosSeqCtx`] Pascal's `MakePosSequence` reads live:
    /// this element's per-terminal parsed node numbers (via the AuxParser, Pascal
    /// `AuxParser.ParseAsBusName`), and the `MonitoredElement`/`ControlledElement`
    /// snapshots a control/meter dereferences mid-conversion.
    fn build_pos_seq_ctx(&mut self, r: ElemId) -> PosSeqCtx {
        // Clone this element's terminal bus names + its monitored/controlled
        // refs up front, so the AuxParser (and the foreign-element reads) do
        // not alias the element borrow.
        let (bus_strs, mon_ref, ctrl_ref) = {
            let elem = self.classes[r.class_ord()]
                .arena
                .try_ckt_elem(r.index())
                .expect("CktElements entry is a circuit element");
            let cd = elem.cd();
            let buses: Vec<String> = (1..=cd.nterms).map(|i| cd.get_bus(i).to_string()).collect();
            (
                buses,
                elem.monitored_element_ref(),
                elem.controlled_element(),
            )
        };

        // `AuxParser.ParseAsBusName(GetBus(i))` per terminal → its node list.
        // Slot i (0-based) = terminal i+1; a bare bus (no dots) → `[]`.
        let mut terminal_nodes: Vec<Vec<i32>> = Vec::with_capacity(bus_strs.len());
        for b in &bus_strs {
            let nodes = self
                .aux_parser
                .parse_as_bus_name(b, &self.vars)
                .map(|(_, nodes)| nodes)
                .unwrap_or_default();
            terminal_nodes.push(nodes);
        }

        let monitored = mon_ref.and_then(|mr| self.resolve_pos_seq_info(mr));
        let controlled = ctrl_ref.and_then(|cr| self.resolve_pos_seq_info(cr));

        PosSeqCtx {
            terminal_nodes,
            monitored,
            controlled,
        }
    }

    /// Snapshot the fields Pascal reads off a live `MonitoredElement` /
    /// `ControlledElement` (`NPhases`, `NConds`, `Yorder`, `NumStateVars`,
    /// `Enabled`, `BusNames[1..NTerms]`). `None` where Pascal would see NIL (an
    /// unresolved reference / a non-circuit target).
    fn resolve_pos_seq_info(&self, r: ElemId) -> Option<PosSeqElemInfo> {
        let arena = &self.classes.get(r.class_ord())?.arena;
        if r.index() >= arena.len() {
            return None;
        }
        let elem = arena.try_ckt_elem(r.index())?;
        let cd = elem.cd();
        Some(PosSeqElemInfo {
            bus_names: (1..=cd.nterms).map(|i| cd.get_bus(i).to_string()).collect(),
            nphases: cd.nphases,
            nconds: cd.nconds,
            yorder: cd.yorder,
            num_variables: elem.num_variables(),
            enabled: cd.enabled,
        })
    }

    /// Replay one element's [`PosSeqPlan`]: the typed-setter run (under the
    /// editing-active VM), the base bus rename when the override ended with
    /// `inherited MakePosSequence`, then the shared post-edit signal tail.
    fn apply_pos_seq_plan(&mut self, r: ElemId, plan: &PosSeqPlan) {
        self.apply_pos_seq_actions(r, &plan.actions);

        // `inherited MakePosSequence` → the base `TDSSCktElement.MakePosSequence`
        // bus rename (strip node extensions; keep a ground bus's `.0`).
        if plan.run_base
            && let Some(elem) = self.classes[r.class_ord()]
                .arena
                .try_ckt_elem_mut(r.index())
        {
            elem.cd_mut().make_pos_sequence_base();
        }

        // The identical tail every property edit runs (see the shared helper):
        // deferred errors/abort, circuit signal-flag propagation, ref-actions.
        apply_edit_signal_tail(
            &mut self.classes,
            &mut self.circuit,
            &mut self.errors,
            r.class_ord(),
            r.index(),
        );
    }

    /// The editing-active bracketing VM (Pascal `SetInteger`/`SetDouble`, whose
    /// body is `singleEdit := not EditingActive; if singleEdit then
    /// BeginEdit(True); <set>; if singleEdit then EndEdit(1)`):
    ///
    /// - `BeginEdit` action → open an explicit multi-set block (`editing_active`
    ///   on); no per-object side effect (Rust does not track the flag).
    /// - `Set*` action while NOT editing → a **single edit**: apply the typed
    ///   setter, then `end_edit()` (its own `RecalcElementData`).
    /// - `Set*` action while editing → apply the setter only (the block's
    ///   trailing `EndEdit` runs the single recalc).
    /// - `EndEdit` action → `end_edit()` + `editing_active` off. Storage's
    ///   trailing bare `EndEdit` (no matching `BeginEdit`) still runs one extra
    ///   `end_edit()`, exactly like the upstream `EndEdit(changes)`.
    /// - `Disable` action → `Enabled := FALSE` through the (default) `Set_Enabled`
    ///   path: flag off + `BusNameRedefined` (no recalc, no `inherited`).
    fn apply_pos_seq_actions(&mut self, r: ElemId, actions: &[PosSeqAction]) {
        // Pascal `RecalcElementData` (run by the replayed `EndEdit`) reads the
        // live `ActiveCircuit.Solution` globals; snapshot them once before the
        // split borrow. MakePosSequence always runs with a circuit present.
        let live_sys = self
            .circuit
            .as_ref()
            .map(crate::solution::solution::sys_ctx)
            .unwrap_or_else(crate::elements::traits::SysCtx::parse_default);
        let Dss {
            classes,
            aux_parser,
            vars,
            enums,
            errors,
            ..
        } = self;
        let DssClass { props, arena, .. } = &mut classes[r.class_ord()];
        // None: no MakePosSequence typed setter resolves an object-reference
        // property, so the foreign class view is never consulted here.
        let mut eng = PropEngine {
            parser: aux_parser,
            vars,
            enums,
            errors,
            foreign: None,
        };

        let mut editing_active = false;
        for action in actions {
            let obj: &mut dyn DssObject = arena.obj_mut(r.index());
            match action {
                PosSeqAction::BeginEdit => editing_active = true,
                PosSeqAction::EndEdit => {
                    obj.end_edit(&live_sys);
                    editing_active = false;
                }
                PosSeqAction::SetF64(idx, v) => {
                    props.set_prop_f64(obj, *idx, *v, &mut eng);
                    if !editing_active {
                        obj.end_edit(&live_sys);
                    }
                }
                PosSeqAction::SetI32(idx, v) => {
                    props.set_prop_i32(obj, *idx, *v, &mut eng);
                    if !editing_active {
                        obj.end_edit(&live_sys);
                    }
                }
                PosSeqAction::SetStructF64s(idx, vals) => {
                    props.set_prop_struct_f64s(obj, *idx, vals, &mut eng);
                    if !editing_active {
                        obj.end_edit(&live_sys);
                    }
                }
                PosSeqAction::SetStructI32s(idx, vals) => {
                    props.set_prop_struct_i32s(obj, *idx, vals, &mut eng);
                    if !editing_active {
                        obj.end_edit(&live_sys);
                    }
                }
                PosSeqAction::SetStructBuses(names) => {
                    props.set_prop_struct_buses(obj, names, &mut eng);
                    if !editing_active {
                        obj.end_edit(&live_sys);
                    }
                }
                PosSeqAction::Disable => {
                    if let Some(elem) = arena.try_ckt_elem_mut(r.index()) {
                        elem.cd_mut().set_enabled(false);
                    }
                }
            }
        }
    }
}

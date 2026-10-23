//! The applier: the read-only [`PosSeqCtx`] an element's conversion reads,
//! and the replay of the [`PosSeqPlan`] it returns through the typed setters.

use crate::diag::DssDiagnostic;
use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx, PosSeqElemInfo, PosSeqPlan};
use crate::elements::traits::SysCtx;
use crate::exec::command::apply_edit_signal_tail;
use crate::exec::make_pos_seq::refuse::PosSeqRefusal;
use crate::exec::*;

impl Dss {
    /// Resolve the read-only [`PosSeqCtx`] an element's conversion reads: this
    /// element's node numbers per terminal, parsed from its bus names, and the
    /// snapshots of the monitored and controlled elements a control or meter
    /// reads while it converts.
    pub(in crate::exec) fn build_pos_seq_ctx(&mut self, r: ElemId) -> PosSeqCtx {
        // Clone this element's terminal bus names and its monitored and
        // controlled refs up front, so the bus-name parser and the reads of
        // other elements do not alias the element borrow.
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

        // The node list of each terminal's bus name: slot i (0-based) is
        // terminal i+1, and a bare bus (no dots) gives `[]`.
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

    /// Snapshot the fields a conversion reads off a monitored or controlled
    /// element: its phase and conductor counts, Y order, state variable
    /// count, enabled flag and bus names. `None` for an unresolved reference
    /// or a target that is no circuit element.
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
    /// editing-active VM), the base bus rename when the plan asks for it, then
    /// the shared post-edit signal tail. A write the class refuses stops the
    /// replay with [`PosSeqRefusal::ClassRefusedWrite`].
    pub(in crate::exec) fn apply_pos_seq_plan(
        &mut self,
        r: ElemId,
        plan: &PosSeqPlan,
    ) -> Result<(), PosSeqRefusal> {
        self.apply_pos_seq_actions(r, &plan.actions)?;

        // The base bus rename: strip the node extensions, keep a ground bus's
        // `.0`.
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
        Ok(())
    }

    /// The editing-active bracketing VM. A write outside a block is a single
    /// edit, which runs the element's recalc after it. A block runs the recalc
    /// once, after all its writes, as an `Edit` line does:
    ///
    /// - `BeginEdit` action → open a block (`editing_active` on), no
    ///   side effect on the element.
    /// - `Set*` action outside a block → a single edit: apply the typed
    ///   setter, then `end_edit()`, the recalc.
    /// - `Set*` action in a block → apply the setter only.
    /// - `EndEdit` action → `end_edit()` + `editing_active` off. Storage's
    ///   trailing `EndEdit` with no `BeginEdit` runs one more `end_edit()`.
    /// - `Disable` action → the element disabled through `set_enabled`: the
    ///   flag off and the bus list marked for a rebuild, no recalc and no bus
    ///   rename.
    /// - `SetText` action → the property edit path of a DSS `edit`, side
    ///   effects included, then the single edit's `end_edit()` like a `Set*`.
    ///   A value that path refuses, by an `Err` or by a message it logs or
    ///   queues on the element, stops the replay with that write named. A
    ///   message the `end_edit()` of a single edit logs or queues stops it
    ///   with the write named, and one of a block's `end_edit()` with every
    ///   `SetText` write of the block named in order. Messages queued on the
    ///   element before stay queued.
    pub(in crate::exec) fn apply_pos_seq_actions(
        &mut self,
        r: ElemId,
        actions: &[PosSeqAction],
    ) -> Result<(), PosSeqRefusal> {
        // The recalc an `end_edit()` runs reads the live solution state, read
        // once here before the split borrow. MakePosSequence always runs with a
        // circuit present.
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
        // None: no MakePosSequence write resolves an object-reference
        // property, so the foreign class view is never consulted here.
        let mut eng = PropEngine {
            parser: aux_parser,
            vars,
            enums,
            errors,
            foreign: None,
            // Typed setters carry no string tokens — the quote state is moot.
            was_quoted: false,
        };

        let mut editing_active = false;
        // The `SetText` writes of the open block, whose `EndEdit` runs their
        // recalc.
        let mut block_writes: Option<TextWrites> = None;
        for action in actions {
            let obj: &mut dyn DssObject = arena.obj_mut(r.index());
            match action {
                PosSeqAction::BeginEdit => editing_active = true,
                PosSeqAction::EndEdit => {
                    match block_writes.take() {
                        Some(writes) => end_edit_of(writes, obj, &mut eng, &live_sys)?,
                        None => obj.end_edit(&live_sys),
                    }
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
                PosSeqAction::SetRating(idx, v) => {
                    props.set_prop_rating(obj, *idx, *v);
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
                PosSeqAction::SetText(idx, text) => {
                    // The edit path refuses a value by an `Err` or by a message
                    // it logs or queues on the object while keeping the old
                    // value. Either stops the replay.
                    let write = (props.property_name(*idx).to_string(), text.clone());
                    let element = format!("{}.{}", props.class_name(), obj.data().name());
                    let (written, added) = with_messages(obj, &mut eng, |obj, eng| {
                        props.edit_property(obj, *idx, text, eng)
                    });
                    let refused = match written {
                        Err(e) => Some(e.message().to_string()),
                        Ok(()) => added.into_iter().next().map(|d| d.message),
                    };
                    if let Some(message) = refused {
                        let refused = TextWrites {
                            element,
                            writes: vec![write],
                        };
                        return Err(refused.refused(message));
                    }
                    if editing_active {
                        block_writes
                            .get_or_insert_with(|| TextWrites {
                                element,
                                writes: Vec::new(),
                            })
                            .writes
                            .push(write);
                    } else {
                        let single = TextWrites {
                            element,
                            writes: vec![write],
                        };
                        end_edit_of(single, obj, &mut eng, &live_sys)?;
                    }
                }
                PosSeqAction::Disable => {
                    if let Some(elem) = arena.try_ckt_elem_mut(r.index()) {
                        elem.cd_mut().set_enabled(false);
                    }
                }
            }
        }
        Ok(())
    }
}

/// The `SetText` writes of one edit as a refusal names them: the element,
/// and each property in its defined spelling with its value, in order.
struct TextWrites {
    element: String,
    writes: Vec<(String, String)>,
}

impl TextWrites {
    fn refused(self, message: String) -> PosSeqRefusal {
        PosSeqRefusal::ClassRefusedWrite {
            element: self.element,
            writes: self.writes,
            message,
        }
    }
}

/// Runs `step` and returns its result with the messages it logged or queued
/// on `obj`, taken off the log and the queue. The messages queued on `obj`
/// before `step` stay queued.
fn with_messages<'a, T>(
    obj: &mut dyn DssObject,
    eng: &mut PropEngine<'a>,
    step: impl FnOnce(&mut dyn DssObject, &mut PropEngine<'a>) -> T,
) -> (T, Vec<DssDiagnostic>) {
    let logged = eng.errors.len();
    let earlier = obj.data_mut().take_errors();
    let out = step(obj, eng);
    let mut added = eng.errors.split_off(logged);
    added.extend(obj.data_mut().take_errors());
    for d in earlier {
        obj.data_mut().push_error(d);
    }
    (out, added)
}

/// The `end_edit()` recalc of a single edit or a block that holds the
/// `SetText` writes `writes`: a message it logs or queues refuses them.
fn end_edit_of(
    writes: TextWrites,
    obj: &mut dyn DssObject,
    eng: &mut PropEngine<'_>,
    sys: &SysCtx,
) -> Result<(), PosSeqRefusal> {
    let ((), added) = with_messages(obj, eng, |obj, _| obj.end_edit(sys));
    match added.into_iter().next() {
        Some(d) => Err(writes.refused(d.message)),
        None => Ok(()),
    }
}

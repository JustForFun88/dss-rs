//! Circuit reduction — a port of `Meters/ReduceAlgs.pas` (the eight strategy
//! procedures + `IsShortLine`), `PDElements/Line.pas` `TLineObj.MergeWith`
//! (series/parallel line merge), `Meters/EnergyMeter.pas` `ReduceZone`, and the
//! `Executive/ExecHelper.pas` `DoRemoveCmd`/`DoKeeperBusList` drivers.
//!
//! Unlike the read-only `solution::meters` walks, reduction *mutates* the model
//! through the executive edit machinery (`edit_active`: bus rewiring, impedance
//! re-definition, shunt reconnection, equivalent-load creation), so it lives at
//! the [`Dss`] level. The meter's branch tree (`CktTree`) is taken out of the
//! meter, walked, and — because the reprocessing strategies rebuild every zone —
//! either dropped (so `DoResetMeterZones` rebuilds it) or restored.
//!
//! Binding fidelity notes (PHASE8_PLAN WP8.7):
//! - only `DoReduceShortLines`/`DoRemoveAll_1ph_Laterals`/`DoRemoveBranches`
//!   reprocess bus defs themselves; the others rely on the bus1/bus2 edits and
//!   element disables raising `BusNameRedefined` → the next solve reprocesses.
//! - the head branch is always kept (`First` then `GoForward`).
//! - ratings are NOT recombined by `MergeWith` (upstream never does).
//! - the matrix-parallel merge reproduces the upstream `Len/2` "assume equal"
//!   TODO.

use super::*;
use crate::circuit::ReductionStrategy;
use crate::circuit::ckt_tree::CktTree;
use crate::elements::ckt::ElemFlags;
use crate::elements::control::control_elem::ControlElemData;
use crate::report::format::strip_extension;
use crate::support::cmatrix::CMatrix;
use crate::support::line_units::LineUnits;
use num_complex::Complex64;

/// Pascal `GetNodeString(BusName)` (`Common/Utilities.pas`): the `.n.n…` node
/// suffix of a bus reference (everything from the first `.`), or `""`.
fn get_node_string(bus_name: &str) -> &str {
    match bus_name.find('.') {
        Some(p) => &bus_name[p..],
        None => "",
    }
}

/// The subset of a `TLineObj`'s state `MergeWith` reads from either line.
struct LineSnap {
    name: String,
    len: f64,
    units_convert: f64,
    length_units: LineUnits,
    r1: f64,
    x1: f64,
    r0: f64,
    x0: f64,
    c1: f64,
    c0: f64,
    is_switch: bool,
    sym_components_model: bool,
    nphases: usize,
    /// Terminal 1/2 bus indices (`NodeRef[…]→BusRef`).
    bus_refs: [usize; 2],
    /// Terminal 1/2 bus name strings (`GetBus(1)`/`GetBus(2)`).
    bus_names: [String; 2],
    z: Option<CMatrix>,
    yc: Option<CMatrix>,
    /// `(LineGeometryObj <> NIL) or SpacingSpecified`.
    geom_or_spacing: bool,
}

impl Dss {
    // ------------------------------------------------------------------
    // Small element predicates/reads over the class registry.
    // ------------------------------------------------------------------

    fn red_as_line(&self, r: ElemId) -> Option<&line::Line> {
        self.classes[r.class_ord()]
            .arena
            .get::<line::Line>(r.index())
    }

    /// Pascal `IsLineElement`.
    fn red_is_line(&self, r: ElemId) -> bool {
        self.red_as_line(r).is_some()
    }

    fn red_enabled(&self, r: ElemId) -> bool {
        self.classes[r.class_ord()]
            .arena
            .try_ckt_elem(r.index())
            .map(|e| e.cd().enabled)
            .unwrap_or(false)
    }

    fn red_is_switch(&self, r: ElemId) -> bool {
        self.red_as_line(r).is_some_and(|l| l.is_switch)
    }

    fn red_flag(&self, r: ElemId, f: ElemFlags) -> bool {
        self.classes[r.class_ord()]
            .arena
            .try_ckt_elem(r.index())
            .map(|e| e.cd().flags.contains(f))
            .unwrap_or(false)
    }

    /// `bus = None` (no such bus recorded / cursor exhausted) is "not kept",
    /// exactly as an out-of-range index was.
    fn red_bus_keep(&self, bus: Option<usize>) -> bool {
        self.circuit
            .as_ref()
            .and_then(|c| bus.and_then(|b| c.buses.get(b)))
            .is_some_and(|b| b.keep)
    }

    /// Pascal `ShuntElement.DSSObjType and CLASSMASK in {CAP,REACTOR}`.
    fn red_is_cap_or_reactor(&self, r: ElemId) -> bool {
        let arena = &self.classes[r.class_ord()].arena;
        arena.get::<capacitor::Capacitor>(r.index()).is_some()
            || arena.get::<reactor::Reactor>(r.index()).is_some()
    }

    /// Pascal `elem.Enabled := FALSE`: disable and propagate the
    /// `BusNameRedefined` signal (`Set_Enabled` wrote the circuit global
    /// immediately upstream).
    fn red_disable(&mut self, r: ElemId) {
        if let Some(e) = self.classes[r.class_ord()]
            .arena
            .try_ckt_elem_mut(r.index())
        {
            e.cd_mut().set_enabled(false);
        }
        self.red_drain_signal(r);
    }

    /// Drain one element's `signal_bus_name_redefined` / yprim-invalid signals
    /// into the circuit (the `edit_active` tail, applied for direct mutations
    /// that bypass the edit loop).
    fn red_drain_signal(&mut self, r: ElemId) {
        let Dss {
            classes, circuit, ..
        } = self;
        let Some(ckt) = circuit.as_mut() else {
            return;
        };
        let Some(elem) = classes[r.class_ord()].arena.try_ckt_elem_mut(r.index()) else {
            return;
        };
        let cd = elem.cd_mut();
        if cd.signal_bus_name_redefined {
            cd.signal_bus_name_redefined = false;
            ckt.set_bus_name_redefined(true);
        }
        if cd.yprim_invalid && cd.enabled {
            ckt.solution.system_y_changed = true;
        }
    }

    /// Drain every circuit element's pending signals (called at each strategy's
    /// tail so the disables/merges raise `BusNameRedefined`/`SystemYChanged`).
    fn red_drain_all(&mut self) {
        let Some(ckt) = self.circuit.as_ref() else {
            return;
        };
        for r in ckt.ckt_elements.clone() {
            self.red_drain_signal(r);
        }
    }

    /// Edit the element at `r` with a property command string, reusing the
    /// executive edit path (Pascal `Parser.CmdString := …; Elem.Edit(Parser)` /
    /// `SetDouble`/`SetInteger` via `ParsePropertyValue`). The `edit_active`
    /// tail drains the element's `BusNameRedefined`/Yprim signals.
    fn red_edit_elem(&mut self, r: ElemId, cmd: &str) {
        self.active_class = Some(r.class_ord());
        self.classes[r.class_ord()].active = Some(r.index());
        self.parser.set_cmd_string(cmd);
        self.edit_active();
    }

    // ------------------------------------------------------------------
    // TLineObj.MergeWith (r4133 `Version8/Source/PDElements/Line.pas:1604`)
    // ------------------------------------------------------------------

    fn red_line_snap(&self, r: ElemId) -> LineSnap {
        let l = self.red_as_line(r).expect("red_line_snap on a non-line");
        let terms = &l.cd.terminals;
        let bus_refs = [
            terms.first().and_then(|t| t.bus_ref).unwrap_or(usize::MAX),
            terms.get(1).and_then(|t| t.bus_ref).unwrap_or(usize::MAX),
        ];
        let bus_names = [l.cd.get_bus(1).to_string(), l.cd.get_bus(2).to_string()];
        LineSnap {
            name: l.cd.obj.name().to_string(),
            len: l.len,
            units_convert: l.units_convert,
            length_units: l.length_units,
            r1: l.r1,
            x1: l.x1,
            r0: l.r0,
            x0: l.x0,
            c1: l.c1,
            c0: l.c0,
            is_switch: l.is_switch,
            sym_components_model: l.sym_components_model,
            nphases: l.cd.nphases,
            bus_refs,
            bus_names,
            z: l.z.clone(),
            yc: l.yc.clone(),
            geom_or_spacing: l.geometry_obj.is_some() || l.line_spacing_obj.is_some(),
        }
    }

    /// Pascal `TLineObj.MergeWith(Other, Series)` — r4133
    /// `Version8/Source/PDElements/Line.pas:1604`: merge `self` with `other` and
    /// disable `other`. Returns false if the merge is impossible (nil is caught
    /// by the caller here; phase mismatch; no common bus on a series merge).
    ///
    /// **Citation note (RP3.5).** The Pascal line numbers in the impedance
    /// section below name **r4133**, the behavioral authority (CLAUDE.md
    /// 2026-08-02); dss_capi 0.14.5 is named in full wherever it is cited. A bare
    /// `Line.pas:NNNN` elsewhere in this function still carries the older capi
    /// numbering (~30-60 lines off r4133's) and is re-pointed as each site is
    /// touched.
    fn red_merge(&mut self, self_ref: ElemId, other_ref: ElemId, series: bool) -> bool {
        use line::prop::*;

        let this = self.red_line_snap(self_ref);
        let other = self.red_line_snap(other_ref);

        if this.nphases != other.nphases {
            return false; // Can't merge
        }

        // Pascal sets `YPrimInvalid := TRUE` unconditionally right after the
        // phase guard (Line.pas:1653) — before any impedance work, so even a
        // failed merge (no common bus) or the parallel-matrix no-op branch
        // leaves self invalidated. That write goes through `Set_YprimInvalid`
        // (`CktElement.pas:240`): enabled element ⇒ `SystemYChanged := TRUE`.
        let enabled = {
            let ce = self.classes[self_ref.class_ord()]
                .arena
                .try_ckt_elem_mut(self_ref.index());
            match ce {
                Some(ce) => {
                    ce.cd_mut().yprim_invalid = true;
                    ce.cd().enabled
                }
                None => false,
            }
        };
        if enabled && let Some(ckt) = self.circuit.as_mut() {
            ckt.solution.system_y_changed = true;
        }

        let len_units_saved = this.length_units;

        // TotalLen (Line.pas:1658).
        let total_len = if series {
            this.len
                + other.len
                    * crate::support::line_units::convert_line_units(
                        other.length_units,
                        this.length_units,
                    )
        } else {
            1.0
        };

        // --- Series bus rewiring (Line.pas:1663) ------------------------
        if series {
            let mut common1 = 0usize;
            let mut common2 = 0usize;
            'outer: for i in 1..=2usize {
                let test_bus = this.bus_refs[i - 1];
                for j in 1..=2usize {
                    if other.bus_refs[j - 1] == test_bus {
                        common1 = i;
                        common2 = j;
                        break 'outer;
                    }
                }
            }
            if common1 == 0 {
                return false; // didn't find anything in common
            }
            // Redefine the bus connection, eliminating the common bus (Line.pas:1690):
            // point self's common terminal at Other's OTHER bus.
            let (self_term, new_bus) = match (common1, common2) {
                (1, 1) => (1, other.bus_names[1].clone()),
                (1, 2) => (1, other.bus_names[0].clone()),
                (2, 1) => (2, other.bus_names[1].clone()),
                (2, 2) => (2, other.bus_names[0].clone()),
                _ => unreachable!(),
            };
            self.red_set_bus(self_ref, self_term, &new_bus);
        }

        // --- Naming + control repointing (Line.pas:1708) ----------------
        let new_name = if series {
            format!("{}~{}", other.name, this.name)
        } else {
            format!(
                "{}||{}",
                strip_extension(&this.bus_names[0]),
                strip_extension(&this.bus_names[1])
            )
        };
        self.red_update_control_elements(self_ref, other_ref);
        self.red_rename_line(self_ref, &new_name);
        if series && let Some(l) = self.red_as_line_mut(self_ref) {
            l.is_switch = false; // not allowed on series merge
        }

        // --- Impedances -------------------------------------------------
        // LenSelf/LenOther in units of the R X data (Line.pas:1723).
        let len_self0 = this.len / this.units_convert;
        let len_other = other.len / other.units_convert;

        if this.sym_components_model && other.sym_components_model && this.nphases == 3 {
            // Symmetrical-component model (r4133 Line.pas:1695-1730).
            //
            // r4133 assembles ONE edit string `S` (`:1699-1719`) and edits it
            // once (`:1721-1722`). Only two of the four arms carry impedances:
            // the parallel self-is-switch arm leaves `S` EMPTY (`:1708`, "leave
            // as is if switch; just dummy z anyway") and the parallel
            // other-is-switch arm makes it `' switch=yes'` (`:1709`, "this will
            // take care of setting Z's"). The `Length=`/`Units=` re-apply
            // (`:1724-1726`) and `RecalcElementData` (`:1730`) then run
            // UNCONDITIONALLY, outside every arm.
            let mut rxc: Option<[f64; 6]> = None;
            let mut make_switch = false;
            if series {
                rxc = Some([
                    (this.r1 * len_self0 + other.r1 * len_other) / total_len,
                    (this.x1 * len_self0 + other.x1 * len_other) / total_len,
                    (this.r0 * len_self0 + other.r0 * len_other) / total_len,
                    (this.x0 * len_self0 + other.x0 * len_other) / total_len,
                    (this.c1 * len_self0 + other.c1 * len_other) / total_len * 1.0e9,
                    (this.c0 * len_self0 + other.c0 * len_other) / total_len * 1.0e9,
                ]);
            } else if this.is_switch {
                // Leave as is if switch; just dummy z anyway.
            } else if other.is_switch {
                // r4133 `:1709` emits the TEXT `' switch=yes'`. `Switch=1` — a
                // transliteration of dss_capi 0.14.5's typed
                // `SetInteger(ord(TProp.Switch), 1, [])` (`src/PDElements/
                // Line.pas:1736`) — is REJECTED by `InterpretYesNo` on
                // both engines (probed: `edit line.a Switch=1` leaves
                // `switch='False'` and `r1='0.301'` on the r4133 DLL and
                // `switch="No"` here), so routing it through the text parser
                // made this arm a silent no-op: the merged line kept the
                // partner's real impedance where both oracles give it dummy z.
                make_switch = true;
            } else {
                let z1 = crate::support::mathutil::parallel_z(
                    Complex64::new(this.r1 * this.len, this.x1 * this.len),
                    Complex64::new(other.r1 * other.len, other.x1 * other.len),
                );
                let z0 = crate::support::mathutil::parallel_z(
                    Complex64::new(this.r0 * this.len, this.x0 * this.len),
                    Complex64::new(other.r0 * other.len, other.x0 * other.len),
                );
                rxc = Some([
                    z1.re,
                    z1.im,
                    z0.re,
                    z0.im,
                    (this.c1 * this.len + other.c1 * other.len) / total_len * 1.0e9,
                    (this.c0 * this.len + other.c0 * other.len) / total_len * 1.0e9,
                ]);
            }
            // The `S` edit (r4133 `:1721-1722`), in the Pascal property order.
            // An empty `S` is a no-op edit upstream, so the self-is-switch arm
            // emits nothing here.
            if let Some(v) = rxc {
                let cmd = format!(
                    "R1={} X1={} R0={} X0={} C1={} C0={}",
                    v[0], v[1], v[2], v[3], v[4], v[5]
                );
                self.red_edit_elem(self_ref, &cmd);
            } else if make_switch {
                self.red_edit_elem(self_ref, "switch=yes");
            }
            // The `Length=`/`Units=` re-apply (r4133 `:1724-1726`) is a SEPARATE
            // edit and runs unconditionally — it is what restores the length the
            // `switch=yes` side effect flattened to 0.001 and the units every
            // impedance side effect reset. Nesting it inside the impedance arm
            // left the two switch arms with a pre-merge `Len` (0.001 or the
            // partner's length) where both oracles write `TotalLen` — dss_capi
            // 0.14.5 runs the same two setters outside `if UseRXC`
            // (`src/PDElements/Line.pas:1764-1768`). No vendored corpus deck
            // reaches either switch arm, so nothing on a gated case moves; both
            // halves are pinned by `exec::tests::reduce::
            // parallel_merge_with_a_switch_restores_length_and_dummy_z`.
            self.red_edit_elem(self_ref, &format!("Length={total_len}"));
            self.red_set_units(self_ref, len_units_saved);
            // RecalcElementData (`:1730`) is deferred to CalcYPrim
            // (SymComponentsChanged).
        } else if !series {
            // Matrix model, parallel: upstream "assume equal" TODO.
            // `TLineObj.MergeWith` (r4133 Line.pas:1734) sets `TotalLen := Len/2` here
            // — an admitted upstream approximation ("We'll assume lines are
            // equal for now"). It writes only the *local* `TotalLen`, which this
            // branch never reads back (no property is updated), so the merge is
            // a no-op on the impedance and the approximation has **no observable
            // effect in either engine**. No compat marker: there is no
            // divergence to reproduce and no clean fix to make — the binding
            // below exists to keep the ported control flow visible.
            let _total_len_matrix_parallel = this.len / 2.0;
        } else {
            // Matrix model, series (r4133 Line.pas:1735, the `Else` of the branch
            // above).
            let (Some(mut zvals), Some(mut ycvals)) = (this.z.clone(), this.yc.clone()) else {
                return false;
            };
            let (Some(oz), Some(oyc)) = (other.z.as_ref(), other.yc.as_ref()) else {
                return false;
            };
            let order = zvals.order();
            if oz.order() != order || oyc.order() != order || ycvals.order() != order {
                return false; // lines not same size
            }
            // If geometry/spacing specified, length already baked into the matrices.
            let len_self = if this.geom_or_spacing { 1.0 } else { len_self0 };
            let len_other = if other.geom_or_spacing {
                1.0
            } else {
                len_other
            };

            for i in 0..order {
                for j in 0..order {
                    let z = (zvals.get(i, j) * len_self + oz.get(i, j) * len_other) / total_len;
                    zvals.set(i, j, z);
                    let y = (ycvals.get(i, j) * len_self + oyc.get(i, j) * len_other) / total_len;
                    ycvals.set(i, j, y);
                }
            }

            if let Some(l) = self.red_as_line_mut(self_ref) {
                l.z = Some(zvals);
                l.yc = Some(ycvals);
                l.len = total_len;
                // Mark the properties the two matrix Edits set (r4133
                // `:1778-1779` `Rmatrix=[…] Xmatrix=[…]`, `:1791-1792`
                // `Cmatrix=[…]`) plus the `Length` of the re-apply below.
                for p in [RMATRIX, XMATRIX, CMATRIX, LENGTH] {
                    l.cd.obj.set_as_next_seq(p);
                }
                // PropertySideEffects for the matrix props: `12..14` clears the
                // linecode flag and the sym model and calls `ResetLengthUnits`
                // (r4133 `:691-693`).
                l.side_effects(RMATRIX, 0);
                l.side_effects(XMATRIX, 0);
                l.side_effects(CMATRIX, 0);
            }
            // r4133 `:1794-1796`: `Length=%-g  Units=%s` is a SEPARATE Edit that
            // runs AFTER the matrix Edits, precisely so the `12..14`
            // `ResetLengthUnits` cannot wipe it — the same construction
            // `MakePosSequence` uses at `:1595-1596` ("Repeat the Length Units to
            // compensate for unexpected reset"). Writing the field first and
            // running the side effects afterwards — dss_capi 0.14.5's order
            // (`src/PDElements/Line.pas:1806-1817`: `LengthUnits :=
            // LenUnitsSaved` then `PropertySideEffects(rmatrix/xmatrix/cmatrix)`)
            // — leaves the merged line at `UNITS_NONE`. Upstream bugs are never
            // reproduced (CLAUDE.md 2026-08-02), so the re-apply happens here.
            //
            // `red_set_units` (a typed `SetInteger` + `PropertySideEffects`) is
            // the whole of r4133's two-parameter Edit that is still outstanding:
            // `Len` already holds `TotalLen`, and the `UNITS` side effect
            // recomputes `units_convert` and `miles_this_line` off it, so the
            // one call lands the same four effects (`FUnitsConvert`,
            // `LengthUnits`, `FUserLengthUnits`, `MilesThisLine`) the Edit does.
            //
            // The three merged lines of `modes:reduce/midi_reduce.dss` therefore
            // render `kft` where the pinned 0.14.5 oracle renders `none`; that is
            // ledgered on the capi channel
            // (`tests/corpus/ledger.json` `reduce-merge-units-restored-midi-capi-props`)
            // and the correct value pinned by `exec::tests::reduce::
            // merged_matrix_line_keeps_the_surviving_lines_length_units`. Nothing
            // else moves: `convert_line_units` returns 1.0 whenever either side
            // is `None`, so `units_convert` — and with it YPrim, Y, V, I, S and
            // the losses — is identical either way.
            self.red_set_units(self_ref, len_units_saved);
        }

        // Disable the Other line (r4133 Line.pas:1800).
        self.red_disable(other_ref);
        true
    }

    fn red_as_line_mut(&mut self, r: ElemId) -> Option<&mut line::Line> {
        self.classes[r.class_ord()]
            .arena
            .get_mut::<line::Line>(r.index())
    }

    /// Pascal `ParsePropertyValue(Bus1/Bus2, name)`: set a line terminal's bus
    /// (direct, like `SetBus` — records the set order, raises `BusNameRedefined`).
    fn red_set_bus(&mut self, r: ElemId, terminal: usize, name: &str) {
        if let Some(l) = self.red_as_line_mut(r) {
            l.cd.set_bus(terminal, name);
            let idx = if terminal == 1 {
                line::prop::BUS1
            } else {
                line::prop::BUS2
            };
            l.cd.obj.set_as_next_seq(idx);
        }
        self.red_drain_signal(r);
    }

    /// Pascal `SetInteger(Units, code)`: typed set + `PropertySideEffects` with
    /// the previous units code.
    fn red_set_units(&mut self, r: ElemId, units: LineUnits) {
        if let Some(l) = self.red_as_line_mut(r) {
            let prev = l.get_i32(line::prop::UNITS);
            l.set_i32(line::prop::UNITS, units.code());
            l.cd.obj.set_as_next_seq(line::prop::UNITS);
            l.side_effects(line::prop::UNITS, prev);
        }
        self.red_drain_signal(r);
    }

    /// Pascal `Set_Name`: rename a line + keep the class name→index map in sync.
    fn red_rename_line(&mut self, r: ElemId, new_name: &str) {
        let lower = new_name.to_ascii_lowercase();
        let cls = &mut self.classes[r.class_ord()];
        let old = cls.arena[r.index()].data().name().to_string();
        cls.arena[r.index()].data_mut().set_name(lower.clone());
        cls.name_to_idx.remove(&old);
        cls.name_to_idx.insert(lower, r.index());
    }

    /// Pascal `TLineObj.UpdateControlElements(NewLine, OldLine)` (Line.pas:1842):
    /// re-point every control monitoring `old_ref` onto `new_ref`. The
    /// `monitored_element` is a stable [`ElemId`] here, so the repoint is
    /// order-independent of the rename (unlike the Pascal name-based re-edit).
    fn red_update_control_elements(&mut self, new_ref: ElemId, old_ref: ElemId) {
        let Some(ckt) = self.circuit.as_ref() else {
            return;
        };
        // Pascal replays a full `element=<NewLine.FullName>` property edit
        // (`ParsePropertyValue`, Line.pas:1849) — not a bare pointer swap — so
        // the control's stored ElementName string (what Save/Dump/`?` render)
        // updates along with the monitored-element reference and any property
        // side effects fire exactly like a user edit.
        let new_full = self.red_full_name(new_ref);
        for cr in ckt.controls.clone() {
            let monitored = control_data_mut(&mut self.classes[cr.class_ord()].arena, cr.index())
                .and_then(|ccd| ccd.monitored_element);
            if monitored == Some(old_ref) {
                self.red_edit_elem(cr, &format!("element={new_full}"));
            }
        }
    }

    // ------------------------------------------------------------------
    // ReduceAlgs.pas strategy procedures
    // ------------------------------------------------------------------

    /// Pascal `DoMergeParallelLines` (ReduceAlgs.pas:47).
    fn red_merge_parallel_lines(&mut self, tree: &mut CktTree) {
        tree.first();
        let mut elem = tree.go_forward(); // Always keep the first element
        while let Some(r) = elem {
            let present = tree.present.expect("present set after go_forward");
            if tree.node(present).is_parallel
                && let Some(loop_elem) = tree.node(present).loop_elem
                && self.red_enabled(r)
            {
                self.red_merge(r, loop_elem, false); // PARALLELMERGE
            }
            elem = tree.go_forward();
        }
    }

    /// Pascal `DoBreakLoops` (ReduceAlgs.pas:69).
    fn red_break_loops(&mut self, tree: &mut CktTree) {
        tree.first();
        let mut elem = tree.go_forward();
        while let Some(r) = elem {
            let present = tree.present.expect("present set after go_forward");
            if tree.node(present).is_looped
                && let Some(loop_elem) = tree.node(present).loop_elem
                && self.red_enabled(r)
            {
                self.red_disable(loop_elem); // Disable the other
            }
            elem = tree.go_forward();
        }
    }

    /// Pascal `DoReduceDangling` (ReduceAlgs.pas:91).
    fn red_reduce_dangling(&mut self, tree: &mut CktTree) {
        tree.first();
        let mut elem = tree.go_forward();
        while let Some(r) = elem {
            if self.red_is_line(r) {
                let present = tree.present.expect("present set after go_forward");
                if tree.node(present).is_dangling {
                    // Only access ToBusReference once (Pascal comment); the
                    // `if ToBusRef > 0` guard (`:111`) rejects an invalid
                    // to-bus — mirrored by requiring a REAL bus index (an
                    // unset/out-of-range entry must not fall through to the
                    // keep-check, whose miss would disable the line).
                    // `.flatten()`: an exhausted cursor and a recorded no-bus
                    // terminal are both "nothing to check here".
                    let to_bus = tree.node_mut(present).next_to_bus_reference().flatten();
                    if let Some(bus) = to_bus
                        && self
                            .circuit
                            .as_ref()
                            .is_some_and(|c| c.buses.get(bus).is_some())
                        && !self.red_bus_keep(to_bus)
                    {
                        self.red_disable(r);
                    }
                }
            }
            elem = tree.go_forward();
        }
    }

    /// Pascal `IsShortLine` (ReduceAlgs.pas:119).
    fn red_is_short_line(&self, r: ElemId) -> bool {
        let l = self.red_as_line(r).expect("IsShortLine on a non-line");
        let ztest = if l.sym_components_model {
            Complex64::new(l.r1, l.x1).norm() * l.len
        } else if l.cd.nphases > 1 {
            let z = l.z.as_ref();
            let d = z
                .map(|m| m.get(0, 0) - m.get(0, 1))
                .unwrap_or(Complex64::ZERO);
            d.norm() * l.len
        } else {
            let z11 = l.z.as_ref().map(|m| m.get(0, 0)).unwrap_or(Complex64::ZERO);
            z11.norm() * l.len
        };
        let zmag = self
            .circuit
            .as_ref()
            .map(|c| c.reduction_zmag)
            .unwrap_or(0.0);
        ztest <= zmag
    }

    /// Pascal `DoReduceShortLines` (ReduceAlgs.pas:142).
    fn red_reduce_short_lines(&mut self, tree: &mut CktTree) {
        // Pass 1: flag all short lines.
        tree.first();
        let mut elem = tree.go_forward();
        while let Some(r) = elem {
            if self.red_is_line(r) {
                let short = self.red_is_short_line(r);
                if let Some(e) = self.classes[r.class_ord()]
                    .arena
                    .try_ckt_elem_mut(r.index())
                {
                    if short {
                        e.cd_mut().flags.include(ElemFlags::FLAG);
                    } else {
                        e.cd_mut().flags.exclude(ElemFlags::FLAG);
                    }
                }
            }
            elem = tree.go_forward();
        }

        // Pass 2: merge flagged lines out.
        tree.first();
        let mut elem = tree.go_forward();
        while let Some(r) = elem {
            let go_extra = self.red_short_line_step(tree, r);
            if go_extra {
                // Skip to next branch since we eliminated a bus (Line.pas:282).
                elem = tree.go_forward();
                if elem.is_none() {
                    break;
                }
            }
            elem = tree.go_forward();
        }

        self.red_reprocess();
    }

    /// One iteration of the `DoReduceShortLines` pass-2 body. Returns whether an
    /// extra `GoForward` is due (the child-merge case, ReduceAlgs.pas:282).
    fn red_short_line_step(&mut self, tree: &mut CktTree, r: ElemId) -> bool {
        if !self.red_enabled(r) {
            return false;
        }
        if self.red_flag(r, ElemFlags::HAS_CONTROL) || self.red_flag(r, ElemFlags::IS_MONITORED) {
            return false;
        }
        if !self.red_flag(r, ElemFlags::FLAG) {
            return false; // not too short
        }

        let present = tree.present.expect("present set after go_forward");
        let num_children = tree.node(present).num_child_branches();
        let num_shunts = tree.node(present).num_shunt_objects();
        let to_bus = tree.node_mut(present).next_to_bus_reference().flatten();
        let to_keep = self.red_bus_keep(to_bus);

        if num_children == 0 && num_shunts == 0 && !to_keep {
            self.red_disable(r); // just discard it
        } else if num_children == 0 {
            // Merge with parent, move shunts to the TO node on the parent branch.
            let Some(parent) = tree.node(present).parent() else {
                return false;
            };
            if tree.node(parent).num_child_branches() != 1 {
                return false; // only works for in-line
            }
            if to_keep {
                return false; // check keeplist
            }
            // Skip if the parent carries a capacitor/reactor shunt — ALL of
            // them are scanned, in both lanes.
            //
            // Upstream inspects exactly one: `DoReduceShortLines` opens the
            // scan on `ParentNode.FirstShuntObject()` and advances it with
            // `PresentBranch.NextShuntObject()` (`ReduceAlgs.pas:200`/`:209`;
            // r4133 `Version8/Source/Meters/ReduceAlgs.pas:199`/`:206` is the
            // same pair), a cross-node cursor mix. The present branch's
            // `TDSSPointerList` cursor still sits at its last item from tree
            // construction (`Add` ends with `ActiveItem := Result`, the new
            // count — `DSSPointerList.pas:88`), so the first `Next` overflows and
            // returns `NIL` (`:113-131`), ending the loop after one element —
            // a capacitor at position ≥ 2 fails to block the merge and is
            // silently moved to another bus. That the merge-with-child branch
            // of the same procedure (`:246-258`) spells the identical loop
            // with a single cursor is what makes the parent branch a slip
            // rather than a rule, so neither lane reproduces it
            // (`GOLDEN_REBASE_PLAN.md` G2.1d; `issue-31`).
            let parent_shunts = tree.node(parent).shunts.clone();
            if parent_shunts.iter().any(|&s| self.red_is_cap_or_reactor(s)) {
                return false;
            }
            let line_elem2 = tree.node(parent).elem;
            if self.red_enabled(line_elem2)
                && self.red_is_line(line_elem2)
                && self.red_merge(line_elem2, r, true)
            {
                // Move any loads to the ToBus reference of the parent branch.
                let to_name = self.red_bus_name(to_bus);
                for s in parent_shunts {
                    self.red_move_shunt(s, &to_name);
                }
            }
        } else if num_children == 1 {
            // Merge with child.
            if to_keep {
                return false;
            }
            let present_shunts = tree.node(present).shunts.clone();
            if present_shunts
                .iter()
                .any(|&s| self.red_is_cap_or_reactor(s))
            {
                return false;
            }
            let child = tree.node(present).children()[0];
            let line_elem2 = tree.node(child).elem;
            if self.red_enabled(line_elem2)
                && self.red_is_line(line_elem2)
                && self.red_merge(line_elem2, r, true)
            {
                // Redefine bus connection to the upline (FROM) bus.
                let from_bus = tree.node(present).from_bus;
                let from_name = self.red_bus_name(from_bus);
                for s in present_shunts {
                    self.red_move_shunt(s, &from_name);
                }
                return true; // extra GoForward
            }
        }
        false
    }

    /// Pascal `DoReduceSwitches` (ReduceAlgs.pas:297).
    fn red_reduce_switches(&mut self, tree: &mut CktTree) {
        tree.first();
        let mut elem = tree.go_forward();
        while let Some(r) = elem {
            if self.red_enabled(r) && self.red_is_line(r) && self.red_is_switch(r) {
                let present = tree.present.expect("present set after go_forward");
                match tree.node(present).num_child_branches() {
                    0 => {
                        // Throw away if dangling (no shunts).
                        if tree.node(present).num_shunt_objects() == 0 {
                            self.red_disable(r);
                        }
                    }
                    1 => {
                        if tree.node(present).num_shunt_objects() == 0 {
                            let to_bus = tree.node_mut(present).next_to_bus_reference().flatten();
                            if !self.red_bus_keep(to_bus) {
                                let child = tree.node(present).children()[0];
                                let line_elem2 = tree.node(child).elem;
                                if self.red_is_line(line_elem2) && !self.red_is_switch(line_elem2) {
                                    self.red_merge(line_elem2, r, true); // Series merge
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            elem = tree.go_forward();
        }
    }

    /// Pascal `DoReduceDefault` (ReduceAlgs.pas:335).
    fn red_reduce_default(&mut self, tree: &mut CktTree) {
        tree.first();
        let mut elem = tree.go_forward();
        while let Some(r) = elem {
            if self.red_is_line(r)
                && !self.red_is_switch(r)
                && !self.red_flag(r, ElemFlags::HAS_CONTROL)
                && !self.red_flag(r, ElemFlags::IS_MONITORED)
                && self.red_enabled(r)
            {
                let present = tree.present.expect("present set after go_forward");
                if tree.node(present).num_child_branches() == 1
                    && tree.node(present).num_shunt_objects() == 0
                {
                    let to_bus = tree.node_mut(present).next_to_bus_reference().flatten();
                    if !self.red_bus_keep(to_bus) {
                        let child = tree.node(present).children()[0];
                        let line_elem2 = tree.node(child).elem;
                        if self.red_is_line(line_elem2) && !self.red_is_switch(line_elem2) {
                            self.red_merge(line_elem2, r, true); // Series merge
                        }
                    }
                }
            }
            elem = tree.go_forward();
        }
    }

    /// Pascal `DoRemoveAll_1ph_Laterals` (ReduceAlgs.pas:449).
    fn red_remove_all_1ph_laterals(&mut self, tree: &mut CktTree) {
        let keep_load = self
            .circuit
            .as_ref()
            .is_some_and(|c| c.reduce_laterals_keep_load);

        tree.first();
        let mut elem = tree.active();
        while let Some(r) = elem {
            let nphases = self.classes[r.class_ord()]
                .arena
                .try_ckt_elem(r.index())
                .map(|e| e.cd().nphases)
                .unwrap_or(0);
            if nphases == 1 {
                let present = tree.present.expect("present set");
                let to_bus = tree.node_mut(present).next_to_bus_reference().flatten();
                let one_node = to_bus
                    .and_then(|b| self.circuit.as_ref().and_then(|c| c.buses.get(b)))
                    .map(|b| b.num_nodes_this_bus() == 1)
                    .unwrap_or(false);
                if one_node {
                    // Eliminate the lateral starting with this branch.
                    let from_bus = tree.node(present).from_bus;
                    // Pascal computes `BusName`/`HeadBasekV` only under
                    // `ReduceLateralsKeepLoad` (`:478-495`); without it they
                    // keep their defaults — the empty string and 1.0 — and the
                    // shunt re-bus loop below still runs (`:505-513` is OUTSIDE
                    // the KeepLoad block), editing every shunt to `Bus1= kV=1`.
                    let mut head_base_kv = 1.0f64;
                    let mut bus_name = String::new();
                    if keep_load {
                        bus_name = self.red_elem_bus(r, tree.node(present).from_terminal);
                        // Ensure a node reference (default .1).
                        if !bus_name.contains('.') {
                            bus_name.push_str(".1");
                        }
                        head_base_kv = self.red_head_base_kv(from_bus);
                    }

                    let start_level = tree.level();
                    // Disable all PD elements down the lateral; move shunts to head.
                    loop {
                        let p = tree.present.expect("present set");
                        let cur = tree.node(p).elem;
                        let shunts = tree.node(p).shunts.clone();
                        for s in shunts {
                            let cmd = format!("Bus1={bus_name} kV={} ", fmt6(head_base_kv));
                            self.red_edit_elem(s, &cmd);
                        }
                        self.red_disable(cur);
                        elem = tree.go_forward();
                        if elem.is_none() {
                            break;
                        }
                        if tree.level() <= start_level {
                            break;
                        }
                    }
                    continue;
                } else {
                    elem = tree.go_forward();
                }
            } else {
                elem = tree.go_forward();
            }
        }

        self.red_reprocess();
    }

    /// Pascal `DoRemoveBranches` (ReduceAlgs.pas:374).
    fn red_remove_branches(
        &mut self,
        tree: &mut CktTree,
        first_pd: ElemId,
        keep_load: bool,
        edit_str: &str,
    ) {
        // Position BranchList at "FirstPDElement".
        let mut pd = tree.first();
        while pd.is_some() && pd != Some(first_pd) {
            pd = tree.go_forward();
        }
        let start_level = tree.level();

        if pd != Some(first_pd) {
            self.errors.push(format!(
                "{} not found (Remove Command).",
                self.red_full_name(first_pd)
            ));
            return;
        }

        // If KeepLoad, create a new Load at the upstream (from) bus.
        if keep_load {
            let present = tree.present.expect("present set");
            let from_terminal = tree.node(present).from_terminal;
            let from_bus = tree.node(present).from_bus;
            let bus_name = self.red_elem_bus(first_pd, from_terminal);
            let total_kva = self.red_terminal_power(first_pd, from_terminal) / 1000.0;
            let name = self.red_elem_name(first_pd);
            let new_load_name = format!("Eq_{}_{}", name, strip_extension(&bus_name));
            let nphases = self.classes[first_pd.class_ord()]
                .arena
                .try_ckt_elem(first_pd.index())
                .map(|e| e.cd().nphases)
                .unwrap_or(1);
            let load_base_kv = self.red_load_base_kv(from_bus, nphases);
            let cmd = format!(
                " phases={} Bus1={} kW={} kvar={} kV={} {}",
                nphases,
                bus_name,
                fmt_g(total_kva.re),
                fmt_g(total_kva.im),
                fmt_g(load_base_kv),
                edit_str
            );
            self.parser.set_cmd_string(&cmd);
            self.add_object("Load", &new_load_name);
        }

        // Disable all elements downline from the start element.
        loop {
            let p = tree.present.expect("present set");
            let cur = tree.node(p).elem;
            let shunts = tree.node(p).shunts.clone();
            for s in shunts {
                self.red_disable(s);
            }
            self.red_disable(cur);
            let nxt = tree.go_forward();
            if nxt.is_none() || tree.level() <= start_level {
                break;
            }
        }

        self.red_reprocess();
    }

    // ------------------------------------------------------------------
    // Shared helpers for the strategies
    // ------------------------------------------------------------------

    fn red_bus_name(&self, bus: Option<usize>) -> String {
        self.circuit
            .as_ref()
            .and_then(|c| bus.and_then(|b| c.buses.get(b)))
            .map(|b| b.name.clone())
            .unwrap_or_default()
    }

    /// Pascal shunt reconnection: `bus1="<newbus><node suffix>"` re-edit
    /// (ReduceAlgs.pas:226/:277).
    fn red_move_shunt(&mut self, shunt: ElemId, new_bus: &str) {
        let cur = self.classes[shunt.class_ord()]
            .arena
            .try_ckt_elem(shunt.index())
            .map(|e| e.cd().get_bus(1).to_string())
            .unwrap_or_default();
        let cmd = format!("bus1=\"{}{}\"", new_bus, get_node_string(&cur));
        self.red_edit_elem(shunt, &cmd);
    }

    fn red_elem_bus(&self, r: ElemId, terminal: usize) -> String {
        self.classes[r.class_ord()]
            .arena
            .try_ckt_elem(r.index())
            .map(|e| e.cd().get_bus(terminal).to_string())
            .unwrap_or_default()
    }

    fn red_elem_name(&self, r: ElemId) -> String {
        self.classes[r.class_ord()].arena[r.index()]
            .data()
            .name()
            .to_string()
    }

    fn red_full_name(&self, r: ElemId) -> String {
        format!(
            "{}.{}",
            self.classes[r.class_ord()].props.class_name(),
            self.classes[r.class_ord()].arena[r.index()].data().name()
        )
    }

    /// Pascal head-bus kV base for the lateral removal (ReduceAlgs.pas:487):
    /// the defined `kVBase`, else `Solution.UpdateVBus` +
    /// `Cabs(Bus.VBus[1])·0.001`. `UpdateVBus` (`Solution.pas:2377`) copies the
    /// live `NodeV[RefNo[j]]` into `VBus` — but only for buses whose `VBus` is
    /// allocated (fault study / `AllocateBusQuantities`); with `VBus = NIL`
    /// (any plain power-flow run) the upstream `VBus[1]` read is a NIL
    /// dereference — nondeterministic upstream UB, NOT reproduced (CLAUDE.md
    /// rule). The port reads the live `NodeV[RefNo[1]]` directly, which is
    /// exactly the refreshed-`VBus` value in the well-defined case.
    fn red_head_base_kv(&mut self, from_bus: Option<usize>) -> f64 {
        let Some(ckt) = self.circuit.as_ref() else {
            return 1.0;
        };
        let Some(bus) = from_bus.and_then(|b| ckt.buses.get(b)) else {
            return 1.0;
        };
        if bus.kv_base > 0.0 {
            return bus.kv_base;
        }
        bus.ref_no
            .first()
            .and_then(|&n| ckt.solution.node_v.get(n))
            .map(|v| v.norm() * 0.001)
            .unwrap_or(1.0)
    }

    /// Pascal load kV base for the branch removal (ReduceAlgs.pas:407): the
    /// from-bus `kVBase` (or the `|VBus[1]|·0.001` fallback), ×√3 when NPhases>1.
    fn red_load_base_kv(&mut self, from_bus: Option<usize>, nphases: usize) -> f64 {
        let base = self.red_head_base_kv(from_bus);
        if nphases > 1 {
            base * 3f64.sqrt()
        } else {
            base
        }
    }

    /// Pascal `PDelem.Power[FromTerminal]` (complex, watts+vars): compute over
    /// the current node voltages.
    fn red_terminal_power(&mut self, r: ElemId, terminal: usize) -> Complex64 {
        let Dss {
            classes, circuit, ..
        } = self;
        let Some(ckt) = circuit.as_mut() else {
            return Complex64::ZERO;
        };
        let sys = crate::solution::sys_ctx(ckt);
        let node_v = ckt.solution.node_v.clone();
        let Some(elem) = classes[r.class_ord()].arena.try_ckt_elem_mut(r.index()) else {
            return Complex64::ZERO;
        };
        elem.terminal_power(&sys, &node_v, terminal)
    }

    // ------------------------------------------------------------------
    // ReduceZone (EnergyMeter.pas:2257) + the executive drivers
    // ------------------------------------------------------------------

    /// Pascal `ReprocessBusDefs` + `Solution.SystemYChanged := TRUE`, the tail of
    /// the reprocessing strategies. Drains pending signals, rebuilds bus defs and
    /// meter zones, forces a Y rebuild.
    fn red_reprocess(&mut self) {
        self.red_drain_all();
        let Dss {
            classes,
            circuit,
            aux_parser,
            vars,
            errors,
            ..
        } = self;
        let Some(ckt) = circuit.as_mut() else {
            return;
        };
        let mut store = ClassStore { classes };
        ckt.reprocess_bus_defs(&mut store, aux_parser, vars, errors);
        crate::solution::meters::do_reset_meter_zones(ckt, &mut store);
        ckt.solution.system_y_changed = true;
    }

    /// Pascal `TEnergyMeterObj.ReduceZone` (EnergyMeter.pas:2257): build the zone
    /// list if needed, then dispatch the circuit's `ReductionStrategy`.
    pub(super) fn reduce_zone(&mut self, meter_ref: ElemId) {
        // Make sure the zone list is built (Pascal `MakeMeterZoneLists`).
        let has_tree = {
            let m = energymeter_ref(&self.classes, meter_ref);
            m.map(|m| m.has_branch_list()).unwrap_or(false)
        };
        if !has_tree {
            let Dss {
                classes, circuit, ..
            } = self;
            if let Some(ckt) = circuit.as_mut() {
                let mut store = ClassStore { classes };
                crate::solution::meters::do_reset_meter_zones(ckt, &mut store);
            }
        }

        let Some(mut tree) = self.take_meter_tree(meter_ref) else {
            return;
        };

        let strategy = self
            .circuit
            .as_ref()
            .map(|c| c.reduction_strategy)
            .unwrap_or(ReductionStrategy::Default);

        let reprocessing = matches!(
            strategy,
            ReductionStrategy::ShortLines | ReductionStrategy::Laterals
        );

        match strategy {
            ReductionStrategy::ShortLines => self.red_reduce_short_lines(&mut tree),
            ReductionStrategy::MergeParallel => self.red_merge_parallel_lines(&mut tree),
            ReductionStrategy::Dangling => self.red_reduce_dangling(&mut tree),
            ReductionStrategy::BreakLoop => self.red_break_loops(&mut tree),
            ReductionStrategy::Switches => self.red_reduce_switches(&mut tree),
            ReductionStrategy::Laterals => self.red_remove_all_1ph_laterals(&mut tree),
            ReductionStrategy::Default => self.red_reduce_default(&mut tree),
        }

        if reprocessing {
            // The reprocess rebuilt this meter's zone; leave the fresh tree.
        } else {
            // Non-reprocessing strategies: raise the signals so the next solve
            // reprocesses; restore the (now-stale) tree.
            self.red_drain_all();
            self.put_meter_tree(meter_ref, tree);
        }
    }

    fn take_meter_tree(&mut self, meter_ref: ElemId) -> Option<CktTree> {
        energymeter_mut(&mut self.classes, meter_ref).and_then(|m| m.take_branch_list())
    }

    fn put_meter_tree(&mut self, meter_ref: ElemId, tree: CktTree) {
        if let Some(m) = energymeter_mut(&mut self.classes, meter_ref) {
            m.put_branch_list(tree);
        }
    }

    /// Pascal `TExecHelper.DoRemoveCmd` (ExecHelper.pas:4939): the `Remove`
    /// command driver.
    pub(super) fn do_remove_cmd(&mut self) {
        if self.circuit.is_none() {
            self.errors
                .push("Error: There is no active circuit!".to_string());
            return;
        }

        // Parse via RemoveCommands ['ElementName','KeepLoad','Editstring'].
        let mut element_name = String::new();
        let mut edit_string = String::new();
        let mut keep_load = true;
        let mut param_pointer = 0i64;
        let mut param_name = self.parser.next_param(&self.vars);
        let mut param = self.parser.make_string(&self.vars);
        while !param.is_empty() {
            if param_name.is_empty() {
                param_pointer += 1;
            } else {
                param_pointer = remove_commands_index(&param_name);
            }
            match param_pointer {
                1 => element_name = param.clone(),
                2 => keep_load = interpret_yes_no(&param),
                3 => edit_string = param.clone(),
                _ => {}
            }
            param_name = self.parser.next_param(&self.vars);
            param = self.parser.make_string(&self.vars);
        }

        // Resolve the element.
        let elem_ref = {
            let store = ClassStore {
                classes: &mut self.classes,
            };
            store.find_ckt_element(&element_name)
        };
        let Some(elem_ref) = elem_ref else {
            self.errors.push(format!(
                "Error: Element {element_name} does not exist in this circuit."
            ));
            return;
        };

        // Refuse if the element IS a meter's metered element.
        let ckt = self.circuit.as_ref().expect("checked above");
        let tied = ckt.energy_meters.iter().any(|&mr| {
            energymeter_ref(&self.classes, mr)
                .and_then(|m| m.metered_element())
                .map(|me| me == elem_ref)
                .unwrap_or(false)
        });
        if tied {
            self.errors.push(format!(
                "Error: Element {element_name} is tied to an Energy Meter."
            ));
            return;
        }

        // Must be a PD element (Pascal `ActiveCktElement is TPDElement`).
        let is_pd = self
            .circuit
            .as_ref()
            .is_some_and(|c| c.pd_elements.contains(&elem_ref));
        if !is_pd {
            self.errors.push(format!(
                "Error: Element \"{element_name}\" is not a power delivery element (PDElement)"
            ));
            return;
        }

        // Must be in a meter zone (SensorObj set by the zone build).
        let sensor = self.classes[elem_ref.class_ord()]
            .arena
            .try_ckt_elem(elem_ref.index())
            .and_then(|e| e.cd().sensor_obj);
        let Some(meter_ref) = sensor else {
            self.errors.push(format!(
                "Element \"{}\" is not in a meter zone! Add an Energymeter. ",
                self.red_full_name(elem_ref)
            ));
            return;
        };

        // The sensor must be an EnergyMeter.
        if energymeter_ref(&self.classes, meter_ref).is_none() {
            self.errors.push(format!(
                "Error: The Sensor Object for \"{element_name}\" is not an EnergyMeter object"
            ));
            return;
        }

        let Some(mut tree) = self.take_meter_tree(meter_ref) else {
            return;
        };
        self.red_remove_branches(&mut tree, elem_ref, keep_load, &edit_string);
        // red_remove_branches reprocessed → the meter zone was rebuilt.
    }
}

/// Pascal `RemoveCommands.GetCommand` (positional-or-named): the 1-based index
/// of a `Remove` parameter name, 0 if unmatched.
fn remove_commands_index(name: &str) -> i64 {
    const NAMES: [&str; 3] = ["elementname", "keepload", "editstring"];
    let lower = name.to_ascii_lowercase();
    for (i, n) in NAMES.iter().enumerate() {
        if n.starts_with(&lower) {
            return (i + 1) as i64;
        }
    }
    0
}

/// Downcast a registry slot to `&EnergyMeter`.
fn energymeter_ref(
    classes: &[DssClass],
    r: ElemId,
) -> Option<&crate::elements::meter::energymeter::EnergyMeter> {
    classes[r.class_ord()]
        .arena
        .get::<crate::elements::meter::energymeter::EnergyMeter>(r.index())
}

fn energymeter_mut(
    classes: &mut [DssClass],
    r: ElemId,
) -> Option<&mut crate::elements::meter::energymeter::EnergyMeter> {
    classes[r.class_ord()]
        .arena
        .get_mut::<crate::elements::meter::energymeter::EnergyMeter>(r.index())
}

/// Access a control element's [`ControlElemData`] for the `UpdateControlElements`
/// repoint. Covers every control class that carries a `MonitoredElement`.
fn control_data_mut(
    arena: &mut crate::obj::arena::ClassArena,
    idx: usize,
) -> Option<&mut ControlElemData> {
    use crate::elements::control::*;
    // The shared probe borrows immutably (ends immediately); the mutable
    // `get_mut` only happens on the returning branch, so the borrows never
    // overlap. Each arm is a static `ArenaClass` match on the arena variant, so
    // at most one can hit.
    macro_rules! try_ccd {
        ($($ty:path),* $(,)?) => {$(
            if arena.get::<$ty>(idx).is_some() {
                return arena.get_mut::<$ty>(idx).map(|c| &mut c.ccd);
            }
        )*};
    }
    try_ccd!(
        cap_control::CapControl,
        reg_control::RegControl,
        recloser::Recloser,
        relay::Relay,
        gen_dispatcher::GenDispatcher,
        storage_controller::StorageController,
        swt_control::SwtControl,
        upfc_control::UpfcControl,
        espvl_control::EspvlControl,
        exp_control::ExpControl,
        inv_control::InvControl,
    );
    None
}

/// Pascal `Format('%.6g', v)` for the lateral head kV.
fn fmt6(v: f64) -> String {
    crate::report::format::g(v, 6)
}

/// Pascal `Format('%g', v)` for the equivalent-load kW/kvar/kV (FPC `%g`
/// default precision = 15 significant digits — the `distribute` convention).
fn fmt_g(v: f64) -> String {
    crate::report::format::g(v, 15)
}

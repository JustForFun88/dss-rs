//! `Save circuit` — Pascal `TDSSCircuit.Save` and its helpers
//! (`Common/Circuit.pas:2409-2988`: `Save`, `SaveDSSObjects`, `SaveVoltageBases`,
//! `SaveMasterFile`, `SaveFeeders`, `SaveBusCoords`), plus
//! `WriteVsourceClassFile`/`WriteClassFile` (`Common/Utilities.pas:1088-1210`)
//! and `TEnergyMeterObj.SaveZone` (`Meters/EnergyMeter.pas:2585-2807`).
//!
//! Re-emits the whole circuit as a re-compilable DSS script tree
//! (`Master.dss` + one file per class + one subdir per meter zone). The
//! serializer used for each object is the WP8.5-step-4 `Save` pair
//! ([`write_dss_object`]/[`save_write`] via [`class_file_text`]) — **only the
//! explicitly-set properties**, not the `Dump` all-property form. Faithfulness
//! is verified by *round-trip* (recompile + re-solve on our own engine), never
//! by byte-matching the oracle's Save output (PHASE8_PLAN §2.4).
//!
//! **Flags.** `Circuit.Save` is parameterized by a `DSSSaveFlags` set. The
//! executive `Save circuit` command (`DoSaveCmd`) always calls it with an
//! **empty** set, so every flag-gated branch below
//! (`SingleFile`/`KeepOrder`/`IncludeOptions`/`SetVoltageBases`/`IsOpen`/
//! `IncludeDisabled`/`ExcludeDefault`/`ExcludeMeterZones`/`CalcVoltageBases`) is
//! DORMANT. The [`SaveFlags`] set is defined faithfully for C-API parity, but
//! only the reachable empty-set path is exercised/gated.

use super::*;
use crate::report::save::save::{SaveCtx, class_file_text, write_dss_object};

/// Pascal `DSSSaveFlag` (`Common/DSSClass.pas:58-69`), the per-`Save` option
/// bits. The command path passes the empty set, so these are all off there.
#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // C-API parity: only the empty-set path is reachable here.
pub(crate) enum DssSaveFlag {
    CalcVoltageBases = 0,
    SetVoltageBases = 1,
    IncludeOptions = 2,
    IncludeDisabled = 3,
    ExcludeDefault = 4,
    SingleFile = 5,
    KeepOrder = 6,
    ExcludeMeterZones = 7,
    IsOpen = 8,
    ToString = 9,
}

/// Pascal `DSSSaveFlags = set of DSSSaveFlag`. The command path builds
/// [`SaveFlags::empty`]; the `contains` predicate mirrors Pascal's `in`.
#[derive(Clone, Copy, Default)]
pub(crate) struct SaveFlags(u16);

impl SaveFlags {
    /// The empty set — the only value reachable from `Save circuit`.
    pub(crate) fn empty() -> Self {
        SaveFlags(0)
    }

    /// Pascal `flag in saveFlags`.
    pub(crate) fn contains(self, f: DssSaveFlag) -> bool {
        (self.0 & (1u16 << (f as u16))) != 0
    }
}

/// Pascal `TDSSCircuit.SaveMasterFile`'s per-class ordering for the library
/// classes written *before* the vsource / feeders / general objects
/// (`Circuit.pas:2554-2582`). Written in this exact order so lines/loads/etc.
/// find their referenced library objects on re-compile. Names are matched
/// case-insensitively against the registered class list.
const LIBRARY_CLASSES: [&str; 15] = [
    "wiredata",
    "cndata",
    "tsdata",
    "linegeometry",
    "linespacing",
    "linecode",
    "xfmrcode",
    "loadshape",
    "tshape",
    "priceshape",
    "growthshape",
    "xycurve",
    "tcc_curve",
    "spectrum",
    "dynamicexp",
];

/// Write one object (`New`/`Edit "Class.name" …`) into `out` via the `Save`
/// serializer, split-borrowing its class's prop table + object list. Sets the
/// object's `HasBeenSaved` flag (Pascal `WriteDSSObject`).
fn write_object(
    classes: &mut [DssClass],
    enums: &EnumRegistry,
    r: ElemId,
    out: &mut String,
    new_or_edit: &str,
) {
    let DssClass { props, arena, .. } = &mut classes[r.class_ord()];
    let cx = SaveCtx { cls: props, enums };
    write_dss_object(out, &cx, arena, r.index(), new_or_edit);
}

impl Dss {
    /// Pascal `TDSSCircuit.Save(Dir)` on the executive `Save circuit` path
    /// (`ExecHelper.pas:808-813` → `Circuit.pas:2409`, `saveFlags = []`).
    ///
    /// `dir` is the `dir=` parameter (default `OutputDirectory` — the executive
    /// substitutes it before calling, so it is always non-empty here). The
    /// classic fresh-`<Name>000..999`-subdir loop (`:2457-2478`, `Dir = ''`) is
    /// UNREACHABLE from the executive and not reproduced; the reachable branch
    /// normalizes/creates `dir`, points `current_dir` (Pascal `CurrentDSSDir`)
    /// at it for the duration, and restores it at the end (`:2652`).
    pub(crate) fn do_save_circuit(&mut self, dir: &str) {
        // Pascal `SaveDir := DSS.CurrentDSSDir` — remember where to return to.
        let saved_dir = self.current_dir.clone();

        // Resolve `dir` relative to the current DSS dir when not absolute
        // (Pascal `Dir := SaveDir + Dir` in the non-`ALLOW_CHANGE_DIR` path —
        // the C-API-portable choice that keeps the process cwd fixed).
        // Deliberate narrowing (recorded by audit): Pascal `:2481-2508`
        // additionally drive-prefixes a bare-root `\foo`/`/foo` with
        // `ExtractFileDrive(SaveDir)` and keeps a drive-relative `C:foo`
        // literal; `Path::is_absolute()` treats all three as relative and
        // joins them to `current_dir`. Only those unusual `dir=` spellings
        // differ; the command default and every gate deck pass an absolute
        // or plainly-relative dir.
        let target: PathBuf = {
            let p = Path::new(dir);
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                self.current_dir.join(p)
            }
        };

        // `if not DirectoryExists(Dir) then CreateDir` else overwrite in place;
        // err 432 on create failure (Pascal `:2510-2530`).
        if !target.is_dir()
            && let Err(e) = std::fs::create_dir_all(&target)
        {
            self.errors.push(format!(
                "Could not create a folder \"{}\" for saving the circuit. {e}",
                target.display()
            ));
            return;
        }
        // Pascal `DSS.SetCurrentDSSDir(CurrDir)`.
        self.current_dir = target.clone();

        let flags = SaveFlags::empty();

        // Pascal chains every sub-step through `Success` and, on any failure,
        // reports err 434 with GlobalResult = the error text instead of the
        // "saved" string (`Circuit.pas:2590-2648`). The sub-writers here push
        // into `self.errors` on a write failure, so "Success" = no new errors
        // appeared during the body.
        let errors_before = self.errors.len();

        // `DSS.SavedFileList.Clear` — tracks every file saved, in write order.
        let mut saved_files: Vec<PathBuf> = Vec::new();
        // Per-class `Saved` flag (Pascal `TDssClass.Saved`) so `SaveDSSObjects`
        // does not re-write (and thereby delete) an already-written class file.
        let mut class_saved = vec![false; self.classes.len()];

        // `Exclude(Flg.HasBeenSaved)` on every object (Pascal clears both the
        // `DSSObjs` and `CktElements` lists — every object here).
        for cls in &mut self.classes {
            for obj in cls.arena.objs_mut() {
                obj.data_mut().set_has_been_saved(false);
            }
        }

        // Write library files first (default filename = class name), in the
        // verbatim Pascal order; empty classes write nothing.
        for name in LIBRARY_CLASSES {
            if let Some(&ci) = self.class_by_name.get(name) {
                self.write_class_file_circuit(ci, false, &mut class_saved, &mut saved_files);
            }
        }

        // Define voltage sources first (first vsource as `Edit`, rest `New`).
        self.write_vsource_class_file(&mut class_saved, &mut saved_files);

        // Save feeders (one subdir per enabled EnergyMeter). Dormant when the
        // circuit has no meters (all three round-trip masters).
        self.save_feeders(&target, flags, &mut saved_files);

        // Save the rest of the objects (every remaining populated class).
        self.save_dss_objects(flags, &mut class_saved, &mut saved_files);

        // BusVoltageBases.dss, BusCoords.dss, then the Master.dss header+footer.
        self.save_voltage_bases(&target, flags);
        self.save_bus_coords(&target);
        self.save_master_file(&target, flags, &saved_files);

        // Return to the original directory (Pascal `:2652`).
        self.current_dir = saved_dir;

        if self.errors.len() > errors_before {
            // Pascal `if Success then … else DoSimpleMsg('Error attempting to
            // save circuit …', 434)` — GlobalResult carries the failure, not
            // the "saved" string.
            self.last_result = self.errors[errors_before].message.clone();
            return;
        }
        // Pascal `GlobalResult := 'Circuit saved in directory: "<CurrentDSSDir>"'`
        // — `CurrentDSSDir` carries a trailing path delimiter upstream.
        let sep = std::path::MAIN_SEPARATOR;
        self.last_result = format!("Circuit saved in directory: \"{}{sep}\"", target.display());
    }

    /// Pascal `WriteClassFile` (`Utilities.pas:1134-1210`) with `circF = NIL`,
    /// default filename (`<ClassName>.dss`): create the file, write every
    /// not-yet-saved object (skipping disabled circuit elements when
    /// `is_ckt_element`, `saveFlags = []`), **delete a 0-record file**, and
    /// record a non-empty file in `saved_files` + mark the class `Saved`.
    fn write_class_file_circuit(
        &mut self,
        ci: usize,
        is_ckt_element: bool,
        class_saved: &mut [bool],
        saved_files: &mut Vec<PathBuf>,
    ) {
        // `if DSS_Class.ElementCount() = 0 then Exit` (no file, Saved untouched).
        if self.classes[ci].arena.is_empty() {
            return;
        }
        let filename = format!("{}.dss", self.classes[ci].props.class_name());
        let path = self.current_dir.join(&filename);
        let Dss { classes, enums, .. } = self;
        let (text, nrecords) = class_file_text(&mut classes[ci], enums, is_ckt_element);
        // Pascal `Saved := TRUE` (set inside the `try`, reached for a non-empty
        // class regardless of how many records survive the skips).
        class_saved[ci] = true;
        if let Err(e) = std::fs::write(&path, &text) {
            self.errors.push(format!("WriteClassFile Error: {e}"));
            return;
        }
        if nrecords > 0 {
            saved_files.push(path);
        } else {
            let _ = std::fs::remove_file(&path);
        }
    }

    /// Pascal `WriteVsourceClassFile` (`Utilities.pas:1088-1132`): the first
    /// Vsource is emitted as `Edit "Vsource.source"` (so the re-`New Circuit`
    /// does not duplicate it), every other Vsource as `New`. Marks the class
    /// `Saved` and each written object `HasBeenSaved`.
    fn write_vsource_class_file(
        &mut self,
        class_saved: &mut [bool],
        saved_files: &mut Vec<PathBuf>,
    ) {
        let Some(&ci) = self.class_by_name.get("vsource") else {
            return;
        };
        if self.classes[ci].arena.is_empty() {
            return; // `if DSS_Class.ElementCount() = 0 then Exit` (no Saved set).
        }
        let filename = format!("{}.dss", self.classes[ci].props.class_name());
        let path = self.current_dir.join(&filename);

        let mut out = String::new();
        let n = self.classes[ci].arena.len();
        let Dss { classes, enums, .. } = self;
        for i in 0..n {
            let r = ElemId::new(ci, i);
            // First vsource → Edit; the rest skip disabled / already-saved.
            if i == 0 {
                write_object(classes, enums, r, &mut out, "Edit");
                continue;
            }
            if classes[ci].arena[i].data().has_been_saved() {
                continue;
            }
            if classes[ci]
                .arena
                .try_ckt_elem(i)
                .is_some_and(|e| !e.cd().enabled)
            {
                continue; // `not includeDisabled and not Enabled`
            }
            write_object(classes, enums, r, &mut out, "New");
        }
        class_saved[ci] = true;
        match std::fs::write(&path, &out) {
            Ok(()) => saved_files.push(path),
            Err(e) => self
                .errors
                .push(format!("WriteVsourceClassFile Error: {e}")),
        }
    }

    /// Pascal `TDSSCircuit.SaveDSSObjects` (`Circuit.pas:2657-2706`,
    /// `saveFlags = []`, `KeepOrder` off): write every populated, not-yet-`Saved`
    /// class via [`Self::write_class_file_circuit`], in DSSClassList order,
    /// passing `IsCktElement = (cls is TCktElementClass)`.
    fn save_dss_objects(
        &mut self,
        _flags: SaveFlags,
        class_saved: &mut [bool],
        saved_files: &mut Vec<PathBuf>,
    ) {
        for ci in 0..self.classes.len() {
            if class_saved[ci] {
                continue;
            }
            let is_ckt = self.classes[ci].requires_circuit;
            self.write_class_file_circuit(ci, is_ckt, class_saved, saved_files);
        }
    }

    /// Pascal `TDSSCircuit.SaveFeeders` (`Circuit.pas:2817-2856`): one subdir per
    /// **enabled** EnergyMeter (named after it), into which its zone is written
    /// by [`Self::save_zone`]. The zone files join `saved_files` (relative
    /// Redirects). Disabled meters are skipped; err 436 on a subdir failure.
    fn save_feeders(&mut self, base: &Path, flags: SaveFlags, saved_files: &mut Vec<PathBuf>) {
        let meters: Vec<ElemId> = match &self.circuit {
            Some(ckt) => ckt.energy_meters.clone(),
            None => return,
        };
        for mr in meters {
            // Only active meters (Pascal `if not Meter.Enabled then continue`).
            let (enabled, name) = {
                let obj = &self.classes[mr.class_ord()].arena[mr.index()];
                let en = self.classes[mr.class_ord()]
                    .arena
                    .get::<crate::elements::meter::EnergyMeter>(mr.index())
                    .is_some_and(|m| m.enabled());
                (en, obj.data().name().to_string())
            };
            if !enabled {
                continue;
            }
            let meter_dir = base.join(&name);
            if !meter_dir.is_dir()
                && let Err(e) = std::fs::create_dir_all(&meter_dir)
            {
                self.errors.push(format!(
                    "Cannot create directory: \"{}\". {e}",
                    meter_dir.display()
                ));
                return;
            }
            self.save_zone(mr, &meter_dir, flags, saved_files);
        }
    }

    /// Pascal `TEnergyMeterObj.SaveZone` (`EnergyMeter.pas:2585-2807`): walk the
    /// meter's `BranchList` (the `First`/`GoForward` order our zone build
    /// persisted as `sequence_list`) and split each branch + its shunt objects
    /// (`FirstObject`/`NextObject`, the tree node's `shunts`) into
    /// `Branches.dss` / `Transformers.dss` / `Shunts.dss` / `Loads.dss` /
    /// `Generators.dss` / `Capacitors.dss`. Any element carrying controls
    /// (`Flg.HasControl`) has each control written right after it, into the same
    /// file. A 0-record file is deleted and not listed (`:2783-2806`).
    ///
    /// **Control derivation.** The port materialises no per-element
    /// `ControlElementList`; the controls acting on an element are derived by
    /// scanning `ckt.controls` (creation order) for `controlled_element() == r`,
    /// exactly like `report/show/controlled.rs`. This reproduces Pascal's list
    /// order for single-element controls (RegControl/Recloser/Relay/SwtControl);
    /// **fleet controls** (InvControl/ExpControl — `controlled_element = None`)
    /// are therefore NOT written in-zone and fall through to `SaveDSSObjects`
    /// instead. No corpus deck places a fleet control inside a meter zone, so the
    /// divergence is unobservable (mirrors the same project-wide limitation).
    fn save_zone(
        &mut self,
        meter: ElemId,
        dir: &Path,
        _flags: SaveFlags,
        saved_files: &mut Vec<PathBuf>,
    ) {
        // Snapshot the branch walk (branch ref + its shunt refs) so the meter's
        // immutable tree borrow is released before we mutate objects/classes.
        let branches: Vec<(ElemId, Vec<ElemId>)> = {
            let m = match self.classes[meter.class_ord()]
                .arena
                .get::<crate::elements::meter::EnergyMeter>(meter.index())
            {
                Some(m) => m,
                None => return,
            };
            let Some(tree) = m.branch_list() else {
                return; // `if BranchList = NIL then Exit`
            };
            m.sequence_list()
                .iter()
                .enumerate()
                .map(|(i, &br)| (br, tree.node(m.sequence_nodes()[i]).shunts.clone()))
                .collect()
        };

        // Six zone files (Pascal opens them all up front). `skipDisabled` = not
        // IncludeDisabled = TRUE on the command path.
        let mut branches_txt = String::new();
        let mut xfmrs_txt = String::new();
        let mut shunts_txt = String::new();
        let mut loads_txt = String::new();
        let mut gens_txt = String::new();
        let mut caps_txt = String::new();
        let (mut n_branches, mut n_xfmrs, mut n_shunts, mut n_loads, mut n_gens, mut n_caps) =
            (0usize, 0, 0, 0, 0, 0);

        // Control snapshot (creation order) for the in-zone `HasControl` writes.
        let controls: Vec<ElemId> = self
            .circuit
            .as_ref()
            .map(|c| c.controls.clone())
            .unwrap_or_default();

        for (branch, shunts) in branches {
            // `if skipDisabled and not Enabled then continue` (whole branch).
            if !self.elem_enabled(branch) {
                continue;
            }
            // Branch → Transformers.dss (XFMR_ELEMENT) else Branches.dss.
            let is_xfmr = self.classes[branch.class_ord()]
                .arena
                .get::<crate::elements::pd::transformer::Transformer>(branch.index())
                .is_some();
            let (buf, count) = if is_xfmr {
                (&mut xfmrs_txt, &mut n_xfmrs)
            } else {
                (&mut branches_txt, &mut n_branches)
            };
            *count += 1;
            let Dss { classes, enums, .. } = self;
            write_object(classes, enums, branch, buf, "New");
            self.write_element_controls(branch, &controls, buf);

            // Shunt objects at this branch (`FirstObject`/`NextObject`).
            for shunt in shunts {
                self.write_zone_shunt(
                    shunt,
                    &controls,
                    &mut loads_txt,
                    &mut n_loads,
                    &mut gens_txt,
                    &mut n_gens,
                    &mut caps_txt,
                    &mut n_caps,
                    &mut shunts_txt,
                    &mut n_shunts,
                );
            }
        }

        // Write each non-empty file; a 0-record file is deleted, not listed
        // (Pascal `:2783-2806`, in this exact order).
        for (name, text, n) in [
            ("Branches.dss", &branches_txt, n_branches),
            ("Transformers.dss", &xfmrs_txt, n_xfmrs),
            ("Shunts.dss", &shunts_txt, n_shunts),
            ("Loads.dss", &loads_txt, n_loads),
            ("Generators.dss", &gens_txt, n_gens),
            ("Capacitors.dss", &caps_txt, n_caps),
        ] {
            let path = dir.join(name);
            if n > 0 {
                if let Err(e) = std::fs::write(&path, text) {
                    self.errors
                        .push(format!("Error creating {name} for Energymeter: {e}"));
                    continue;
                }
                saved_files.push(path);
            }
            // (Pascal `DeleteFile` for the 0-record case is a no-op here — we
            // never created the file.)
        }
    }

    /// Classify + write one zone shunt object (Pascal `SaveZone`'s inner
    /// `FirstObject`/`NextObject` loop, `:2721-2768`): LOAD → Loads.dss (with the
    /// allocation-factor side effect if allocated), GEN → Generators.dss, CAP →
    /// Capacitors.dss, anything else → Shunts.dss; each control-carrying element
    /// (except loads, per Pascal) also writes its controls into the same file.
    #[allow(clippy::too_many_arguments)]
    fn write_zone_shunt(
        &mut self,
        shunt: ElemId,
        controls: &[ElemId],
        loads_txt: &mut String,
        n_loads: &mut usize,
        gens_txt: &mut String,
        n_gens: &mut usize,
        caps_txt: &mut String,
        n_caps: &mut usize,
        shunts_txt: &mut String,
        n_shunts: &mut usize,
    ) {
        let arena = &self.classes[shunt.class_ord()].arena;
        let is_load = arena
            .get::<crate::elements::pc::load::Load>(shunt.index())
            .is_some();
        let is_gen = arena
            .get::<crate::elements::pc::generator::Generator>(shunt.index())
            .is_some();
        let is_cap = arena
            .get::<crate::elements::pd::capacitor::Capacitor>(shunt.index())
            .is_some();

        if is_load {
            // Pascal: if the load was allocated, force the allocationfactor
            // property to render (`PropertySideEffects` + `SetAsNextSeq`).
            let allocated = self.classes[shunt.class_ord()]
                .arena
                .get::<crate::elements::pc::load::Load>(shunt.index())
                .is_some_and(|l| l.has_been_allocated);
            if allocated {
                use crate::elements::pc::load::prop::ALLOCATIONFACTOR;
                let obj = self.classes[shunt.class_ord()].arena.obj_mut(shunt.index());
                obj.side_effects(ALLOCATIONFACTOR, 0);
                obj.data_mut().set_as_next_seq(ALLOCATIONFACTOR);
            }
            *n_loads += 1;
            let Dss { classes, enums, .. } = self;
            write_object(classes, enums, shunt, loads_txt, "New");
            // (Pascal writes NO controls for loads.)
        } else if is_gen {
            *n_gens += 1;
            let Dss { classes, enums, .. } = self;
            write_object(classes, enums, shunt, gens_txt, "New");
            self.write_element_controls(shunt, controls, gens_txt);
        } else if is_cap {
            *n_caps += 1;
            let Dss { classes, enums, .. } = self;
            write_object(classes, enums, shunt, caps_txt, "New");
            self.write_element_controls(shunt, controls, caps_txt);
        } else {
            *n_shunts += 1;
            let Dss { classes, enums, .. } = self;
            write_object(classes, enums, shunt, shunts_txt, "New");
        }
    }

    /// Write every control acting on `elem` (Pascal `for pControlElem in
    /// elem.ControlElementList`), derived from `controls` (creation order).
    fn write_element_controls(&mut self, elem: ElemId, controls: &[ElemId], out: &mut String) {
        let acting: Vec<ElemId> = controls
            .iter()
            .copied()
            .filter(|&cr| {
                self.classes[cr.class_ord()]
                    .arena
                    .try_ckt_elem(cr.index())
                    .and_then(|ce| ce.controlled_element())
                    == Some(elem)
            })
            .collect();
        let Dss { classes, enums, .. } = self;
        for cr in acting {
            write_object(classes, enums, cr, out, "New");
        }
    }

    /// Whether the circuit element at `r` is enabled.
    fn elem_enabled(&self, r: ElemId) -> bool {
        self.classes[r.class_ord()]
            .arena
            .try_ckt_elem(r.index())
            .is_some_and(|e| e.cd().enabled)
    }

    /// Pascal `TDSSCircuit.SaveVoltageBases` (`Circuit.pas:2708-2747`,
    /// `circF = NIL`): `BusVoltageBases.dss` = `Set VoltageBases=<get
    /// voltagebases>` then, since the `CalcVoltageBases` save flag is off on the
    /// command path, the commented `! CalcVoltageBases` (probe-proven — the
    /// pinned oracle emits the comment, and the base-kV list only affects
    /// per-unit reporting, not the absolute-volt re-solve). The `SetVoltageBases`
    /// per-bus `SetkVBase` block is flag-gated → dormant.
    fn save_voltage_bases(&mut self, dir: &Path, flags: SaveFlags) {
        let Some(ckt) = self.circuit.as_ref() else {
            return;
        };
        // Reuse the ported `get voltagebases` renderer (Pascal `ParseCommand
        // ('get voltagebases')` → `GlobalResult`).
        let vbases = super::get_cmd::voltage_bases_result(ckt);
        let mut out = format!("Set VoltageBases={vbases}\n");
        if flags.contains(DssSaveFlag::CalcVoltageBases) {
            out.push_str("CalcVoltageBases\n");
        } else {
            out.push_str("! CalcVoltageBases\n");
        }
        let path = dir.join("BusVoltageBases.dss");
        if let Err(e) = std::fs::write(&path, out) {
            self.errors
                .push(format!("Error Saving BusVoltageBases File: {e}"));
        }
    }

    /// Pascal `TDSSCircuit.SaveBusCoords` (`Circuit.pas:2942-2988`, `circF =
    /// NIL`, CSV form): `BusCoords.dss` is ALWAYS created (empty when no bus has
    /// coordinates); one `<name>, %-g, %-g` row per coord-defined bus (`%-g` =
    /// FPC default 15-sig `%g`, probe-proven).
    fn save_bus_coords(&mut self, dir: &Path) {
        let mut out = String::new();
        if let Some(ckt) = self.circuit.as_ref() {
            for i in 0..ckt.buses.len() {
                let bus = &ckt.buses[i];
                if bus.coord_defined {
                    let name = ckt.bus_list.name(i).unwrap_or("");
                    out.push_str(&crate::util::check_for_blanks(name));
                    out.push_str(&format!(
                        ", {}, {}\n",
                        crate::report::format::g(bus.x, 15),
                        crate::report::format::g(bus.y, 15),
                    ));
                }
            }
        }
        let path = dir.join("BusCoords.dss");
        if let Err(e) = std::fs::write(&path, out) {
            self.errors
                .push(format!("Error creating BusCoords.dss. {e}"));
        }
    }

    /// Pascal `TDSSCircuit.SaveMasterFile` (`Circuit.pas:2749-2815`, `circF =
    /// NIL`, header+footer both, `saveFlags = []`): `Master.dss` = a re-buildable
    /// header (`Clear`, `Set DefaultBaseFreq`, `New Circuit.<name>`, conditional
    /// `Set Cktmodel`/`AllowDuplicates`/`LongLineCorrection`, `Set EarthModel`)
    /// then a footer of `Redirect <relative>` per saved file, `MakeBusList`,
    /// `Redirect BusVoltageBases.dss  ! set voltage bases`, and `BusCoords
    /// BusCoords.dss` (the file is always created above). The `IncludeOptions`
    /// solution dump and `IsOpen` `SaveOpenTerminals` are flag-gated → dormant.
    /// The `! Last saved by …` stamp is written as the port's own analogous
    /// comment (ignored on re-compile; round-trip, not byte-match).
    fn save_master_file(&mut self, dir: &Path, _flags: SaveFlags, saved_files: &[PathBuf]) {
        let mut out = String::new();
        // Port's own stamp (Pascal `! Last saved by AltDSS/…`); a comment, so
        // the round-trip ignores it.
        out.push_str("! Saved by dss-rs (Pascal DSS C-API 1:1 port)\n");
        out.push_str("Clear\n");
        out.push_str(&format!(
            "Set DefaultBaseFreq={}\n",
            crate::util::float_to_str(self.default_base_freq)
        ));

        // Pascal writes `'New Circuit.' + Name` where `TNamedObject.Get_Name`
        // returns the lowercase-normalized `LocalName` (`Circuit.pas:386`) —
        // NOT `CaseName`; the oracle emits e.g. `New Circuit.ieee13nodeckt`.
        let (name, positive_sequence, duplicates, long_line) = match self.circuit.as_ref() {
            Some(ckt) => (
                ckt.name.clone(),
                ckt.positive_sequence,
                ckt.duplicates_allowed,
                ckt.long_line_correction,
            ),
            None => (String::new(), false, false, false),
        };
        out.push_str(&format!("New Circuit.{name}\n"));
        out.push('\n');
        if positive_sequence {
            // Pascal `OrdinalToString(Integer(PositiveSequence))`
            // (`Circuit.pas:2768`) — the same `LongBool` -1 as the JSON
            // `PreCommands` writer, routed through the one Stage F row
            // (`compat::CKT_MODEL_RENDERED_ORDINAL`). This site had been ported
            // as a bare `1`, i.e. the *fixed* form in both lanes with no marker;
            // the parity lane now emits upstream's value-less `Set Cktmodel=`.
            out.push_str(&format!(
                "Set Cktmodel={}\n",
                self.enums
                    .get(self.enums.ckt_model)
                    .ordinal_to_string(crate::compat::CKT_MODEL_RENDERED_ORDINAL)
            ));
        }
        if duplicates {
            out.push_str("set AllowDuplicates=yes\n");
        }
        if long_line {
            out.push_str("Set LongLineCorrection=True\n");
        }
        out.push_str(&format!(
            "Set EarthModel={}\n",
            self.enums
                .get(self.enums.earth_model)
                .ordinal_to_string(self.default_earth_model)
        ));
        out.push('\n');

        // Footer: one Redirect per saved file, as a path relative to `dir`.
        for p in saved_files {
            let rel = p.strip_prefix(dir).unwrap_or(p.as_path());
            out.push_str(&format!("Redirect {}\n", rel.display()));
        }
        out.push_str("MakeBusList\n");
        out.push_str("Redirect BusVoltageBases.dss  ! set voltage bases\n");
        // BusCoords.dss is always created by `save_bus_coords`, so the command
        // is always emitted (Pascal `if FileExists('BusCoords.dss')`).
        out.push_str("BusCoords BusCoords.dss\n");

        let path = dir.join("Master.dss");
        if let Err(e) = std::fs::write(&path, out) {
            self.errors.push(format!("Error Saving Master File: {e}"));
        }
    }
}

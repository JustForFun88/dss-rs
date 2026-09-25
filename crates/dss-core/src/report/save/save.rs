//! The `Save` script serializer — Pascal `WriteDSSObject` / `TDSSObject.
//! SaveWrite` / `CheckForBlanks` (`Common/Utilities.pas:1221-1235` /
//! `General/DSSObject.pas:145-165` / `Common/Utilities.pas:1212-1219`) plus the
//! `WriteClassFile` object loop (`Common/Utilities.pas:1134-1210`).
//!
//! This is a **different serializer from Dump** ([`super::dump`]): Dump prints
//! *every* property as a `~ name=value` line; Save emits a single re-compilable
//! `New "Class.name" <name>=<value> …` line per object carrying **only the
//! explicitly-set properties, in the order they were set** (Pascal
//! `GetNextPropertySet` over `PrpSequence`; Rust [`DssObjData::
//! next_property_set`]). Shared by `Save <class>` (WP8.5 step 4) and
//! `Save circuit` (`Circuit.Save`, WP8.5 step 5).
//!
//! # What `Save` prints, and why (R4133_PROPS RP3.11, 2026-09-03)
//!
//! Pascal's loop body reads `PropertyValue[iProp]`, which is **not** a parse
//! store: it is `Get_PropertyValue` → the **virtual** `GetPropertyValue`
//! (r4133 `General/DSSObject.pas:45`, `:117-120`, *"This is virtual function
//! that may call routine"*; the base body at `:112-115` returns
//! `FPropertyValue`, which dss_capi 0.14.5 deleted outright). **49** r4133
//! units override that getter to answer **live** on a hand-picked index set, so
//! there is no coherent "authored circuit" semantics to match — only a
//! per-index accident: `TGeneratorObj.GetPropertyValue`
//! (`PCElements/generator.pas:3007-3038`) has arms for `kv`/`kW`/`pf`/`kvar`/
//! `maxkvar`/`minkvar`/`kVA` but **none for `model`**, while `TStorageObj`'s
//! list carries `propMODEL` (`PCElements/Storage.pas:1525-1596`).
//!
//! The port's policy, decided once for every class:
//!
//! * **values** — the live field, through the one [`ClassProps::get_value`]
//!   that also serves `Dump`, `?`, the property API and batchedit (as both
//!   Pascals route all of theirs through their one virtual getter). Never a
//!   value the engine knows to be superseded: matching r4133's `model=3` on
//!   `modes:ncim/ncim_pv_pq.dss` would mean printing `3` while `gen_model == 4`
//!   in the same process, which CLAUDE.md's 2026-08-02 policy forbids. The
//!   surviving divergence from r4133's serializer is *pinned*, both bytes
//!   quoted, by [`crate::exec::tests::report`]`::
//!   save_renders_the_live_model_after_ncim_pv2pq` and its `dump_…` twin.
//! * **membership + order** — the explicitly-set chain (`GetNextPropertySet`
//!   over `PrpSequence` / [`DssObjData::next_property_set`]), *including*
//!   0.14.5's property-tracking seeds, which r4133 has no counterpart for
//!   (`SetAsNextSeq` does not exist there) and which the AltDSS JSON export —
//!   a surface with no r4133 counterpart either — is captured with. That is why
//!   a Generator line starts `PF=0.88` where r4133 prints no `pf` at all, and
//!   why r4133's `tapwinding` stamp is absent; both directions are pinned by
//!   `save_membership_follows_property_tracking_not_prpsequence` and
//!   `save_omits_the_tapwinding_that_r4133_stamps`, and the two literal tests
//!   that carry the seeded spelling —
//!   `golden_reports.rs::save_class_disabled_load_writes_enabled_no` (`PF=0.88`
//!   on two never-typed loads, the oracle's own bytes) and the RP3.6 `Save` leg
//!   in `exec::tests::line_fetch` (`R1=…` on a `switch=yes` line) — are its
//!   *sequence* witnesses, not render ones.
//! * **structure and ordering guards** — the **union** of both upstreams'
//!   `SaveWrite` overrides (see [`write_dss_object`]), because every one of them
//!   exists to make the emitted deck re-compile; plus this port's own
//!   **sizing-property hoist** ([`save_order`]), same purpose. A property whose
//!   text parse is sized by the live value of another property of the same
//!   object ([`parse_sizer`]: curve `NPts`/`NumHarm`, Capacitor
//!   `NumSteps`/`Phases`, every `Seasons` → `Ratings`, LineSpacing `NConds`,
//!   Line/LineCode phases → matrices, Reactor/Fault phases → matrices, the
//!   transformer `Windings`, the controllers' element lists → weights) is
//!   written after that sizer. The relation is read off the class property
//!   tables, not listed by class, and the hoist re-orders the set-order chain
//!   itself for the duration of the write, so the generic walk and every
//!   override see it. r4133 guards only `LoadShape`
//!   (`General/DSSObject.pas:144-172`) and `XYcurve` (`XYcurve.pas:978-1003`);
//!   on every other class a sizer re-set after its arrays is written last and
//!   the line reloads truncated — a 600 kvar four-step capacitor comes back as
//!   `4 × 37.5` kvar on r4133 (measured). That shared upstream defect is not
//!   reproduced; the divergence is pinned by
//!   `exec::tests::report::save_hoists_numsteps_so_a_capacitor_bank_reloads_whole`
//!   and the all-classes `save_writes_every_sizing_property_ahead_of_its_arrays`.
//!
//! [`DssObjData::next_property_set`]: crate::obj::base::DssObjData::next_property_set
//! [`ClassProps::get_value`]: crate::obj::props::ClassProps::get_value

use crate::exec::registry::DssClass;
use crate::obj::base::{DssObjData, DssObject};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags, PropType};

/// Context for one class's serialization: its prop table (names + the
/// `get_value` renderer) and the enum registry `get_value` needs.
pub struct SaveCtx<'a> {
    pub cls: &'a ClassProps,
    pub enums: &'a EnumRegistry,
}

/// The Pascal `SaveWrite` loop **body** — one ` <Name>=<CheckForBlanks(value)>`
/// token for property `iprop`, skipping an empty value and the `----`
/// conditional-value sentinel (`CompareText = 0`, so case-insensitive —
/// faithfully mirrored though the sentinel is emitted literally).
///
/// Shared with the class overrides that re-order the walk but keep this body
/// ([`crate::elements::general::xy_curve`], [`crate::elements::control::reg_control`]).
pub(crate) fn save_write_token(out: &mut String, cx: &SaveCtx, obj: &dyn DssObject, iprop: usize) {
    // Pascal `str := trim(PropertyValue[iProp])`.
    let val = cx.cls.get_value(obj, iprop, cx.enums);
    let mut s = val.trim();
    if s.eq_ignore_ascii_case("----") {
        s = ""; // set to ignore this property
    }
    if !s.is_empty() {
        out.push(' ');
        out.push_str(cx.cls.property_name(iprop));
        out.push('=');
        // Pascal `CheckForBlanks` (`Utilities.pas:1212`): quote a value
        // containing spaces unless it starts with a quote/bracket char.
        out.push_str(&crate::util::check_for_blanks(s));
    }
}

/// Pascal `TDSSObject.SaveWrite` (r4133
/// `Version8/Source/General/DSSObject.pas:130-173`): append
/// ` <Name>=<CheckForBlanks(value)>` for **only the explicitly-set properties,
/// in the order they were actually set** (`GetNextPropertySet`), through
/// [`save_write_token`].
///
/// The walk reads the chain as [`write_dss_object`] leaves it — already
/// re-ordered by the sizing-property hoist ([`save_order`]) when a sizer was
/// set after one of its arrays — so the only class rule left here is r4133's
/// own.
///
/// **`LoadShape` — the npts-first branch** (RP3.11 P2,
/// `R4133:General/DSSObject.pas:139-150` + `:163-172`, *"created to guarantee
/// that the npts property will be the first to be declared when saving
/// LoadShapes"*): the walk starts at `NPts` **even when the deck never set it**,
/// restarts the chain from the beginning and ignores that index when it comes
/// up again (Pascal `LShpFlag`/`NptsRdy`). dss_capi 0.14.5 reaches the same
/// output by a different route — `TLoadShapeObj.SaveWrite` stamps
/// `PrpSequence[npts] := -999` and calls `inherited`
/// (`CAPI:General/LoadShape.pas:2376-2380`) — so on this class both upstreams
/// agree and the port matches both. The hoist cannot give this rule, because it
/// moves only a sizer the deck set. The five other curve classes (`TCC_Curve`,
/// `GrowthShape`, `PriceShape`, `TShape`, `Spectrum`), which the RP3.11
/// settlement P7 guarded here by name, are now covered by the table-derived
/// hoist; their pinned lines are unchanged
/// (`exec::tests::report::save_puts_the_sizing_property_first_for_every_curve_class`).
///
/// **One deliberate deviation from the letter of the Pascal, measured against the
/// live r4133 DLL.** Pascal tests `NptsRdy` only on the non-restart advance, so a
/// chain whose **head** is the sizing property prints it twice; r4133 really does
/// hit that case, and the port does not reproduce it:
///
/// ```text
/// r4133: New "LoadShape.ls3" npts=5 npts=5          port: New "LoadShape.ls3" NPts=5
/// r4133: New "LoadShape.ls4" npts=4 npts=4 interval=2   port: New "LoadShape.ls4" NPts=4 Interval=2
/// ```
///
/// (epri-worker, `OpenDSSDirect.dll` 11.0.0.1 rev r4133, RP3.11 settlement.) The
/// double token appears exactly when no array property was parsed, because
/// r4133's *"Keep Properties in order for save command"* re-stamp —
/// `Set_NumPoints` re-stamping `PropertyValue[npts]` **and then** the array
/// property (`R4133:General/LoadShape.pas:631-636`, `PriceShape.pas:303` +
/// `:910-916`, `TempShape.pas:302`, `XYcurve.pas:317`) — never fires, so `npts`
/// stays at the head of the chain. This port has no such re-stamp (RP3.11 §4
/// adds and removes no sequence site; STATUS open item (g)), so it applies the
/// `NptsRdy` skip to the restart as well: one `npts`, always first. That is
/// 0.14.5's rule (`PrpSequence[npts] := -999` puts it first and leaves it there)
/// expressed through r4133's structure, and it reproduces r4133 byte for byte on
/// every deck that types an array — `New "LoadShape.ls1" npts=3 interval=1
/// mult=[ 1 2 3]`, for a deck typing `npts` first *and* for one re-setting it
/// last. Both serializations are pinned by
/// `exec::tests::report::save_write_puts_npts_first_for_loadshape`.
pub fn save_write(out: &mut String, cx: &SaveCtx, obj: &dyn DssObject) {
    // Pascal `if ParentClass.Name = 'LoadShape' then iProp := 1` — a
    // case-sensitive compare on the registered class name; the ordinal comes
    // from the class's own property table.
    let npts = (cx.cls.class_name() == "LoadShape")
        .then_some(crate::elements::general::load_shape::prop::NPTS);
    let mut iprop = match npts {
        Some(_) => npts, // Pascal `iProp := 1`
        None => obj.data().next_property_set(None),
    };
    // Pascal `LShpFlag` / `NptsRdy`.
    let mut lshp_flag = npts.is_some();
    let mut npts_ready = false;
    while let Some(i) = iprop {
        save_write_token(out, cx, obj, i);
        if lshp_flag {
            // Pascal: start the chain over, `npts` is done.
            lshp_flag = false;
            npts_ready = true;
            iprop = obj.data().next_property_set(None);
        } else {
            iprop = obj.data().next_property_set(Some(i));
        }
        if npts_ready && iprop == npts {
            iprop = obj.data().next_property_set(npts);
        }
    }
}

/// The property whose **live** value sizes the text parse of property `p` of
/// the same object, if any — Pascal `PropertySizingPropertyIndex`
/// (`getSizePropertyIndex`) restricted to what the port's parse reads
/// (`obj/props/class_props/parse.rs`): the element count of a counted array,
/// the order of a symmetric matrix, the struct count of a per-winding value.
/// 1-based, like every property ordinal.
///
/// * a counted array or matrix (`DoubleArray`, `IntegerArray`, the three
///   symmetric-matrix kinds, the `…OnStruct` arrays, a `DoubleVArray` that is
///   not `ARRAY_MAX_SIZE`) and a `GLOBAL_COUNT` shape file property → its
///   `size_prop`; a `StringList` sizer counts its entries (`get_i32` of a list
///   is its length);
/// * a per-winding cursor value (`BusOnStruct`, `ON_ARRAY`, the
///   `INTEGER_STRUCT_INDEX` `Wdg`) and a singular with an
///   [`array_alternative`](PropDef::array_alternative) (`kV` → `kVs`) → the
///   class's struct count (`Windings`), which bounds the active winding;
/// * everything else → `None`: a `Bus` (its `size_prop` is a terminal), a
///   `DoubleFArray` (a fixed count), an `ARRAY_MAX_SIZE` array (a maximum) and
///   the function-sized arrays (`size_prop == 0`).
pub(crate) fn parse_sizer(cls: &ClassProps, p: usize) -> Option<usize> {
    let pd = cls.prop(p);
    let s = match own_sizer(cls, pd) {
        0 if pd.array_alternative != 0 => own_sizer(cls, cls.prop(pd.array_alternative)),
        s => s,
    };
    (1..=cls.num_properties()).contains(&s).then_some(s)
}

/// [`parse_sizer`] without the one-level `array_alternative` indirection; `0`
/// = not sized by a property.
fn own_sizer(cls: &ClassProps, pd: &PropDef) -> usize {
    match pd.ptype {
        PropType::DoubleArray
        | PropType::IntegerArray
        | PropType::DoubleSymMatrix
        | PropType::SymMatrixReal
        | PropType::SymMatrixImag
        | PropType::DoubleArrayOnStruct
        | PropType::EnumArrayOnStruct
        | PropType::BusesOnStruct => pd.size_prop,
        PropType::DoubleVArray if !pd.flags.contains(PropFlags::ARRAY_MAX_SIZE) => pd.size_prop,
        PropType::BusOnStruct => struct_count_prop(cls),
        _ if pd.flags.contains(PropFlags::GLOBAL_COUNT) => pd.size_prop,
        _ if pd.flags.contains(PropFlags::ON_ARRAY)
            || pd.flags.contains(PropFlags::INTEGER_STRUCT_INDEX) =>
        {
            struct_count_prop(cls)
        }
        _ => 0,
    }
}

/// The struct-array count property (Transformer / AutoTrans / XfmrCode
/// `Windings`) — Pascal `PropertyStructArrayCountOffset`, which the port carries
/// as the `size_prop` of every plural on-struct array. `0` for a class without
/// one.
fn struct_count_prop(cls: &ClassProps) -> usize {
    (1..=cls.num_properties())
        .map(|i| cls.prop(i))
        .find(|pd| {
            matches!(
                pd.ptype,
                PropType::DoubleArrayOnStruct
                    | PropType::EnumArrayOnStruct
                    | PropType::BusesOnStruct
            )
        })
        .map_or(0, |pd| pd.size_prop)
}

/// The **sizing-property hoist** (RF-D01-04): the order [`write_dss_object`]
/// walks an object's set properties in, or `None` when that is the set-order
/// chain itself (the common case — nothing of the line moves).
///
/// Every property is parsed against the **live** value of its sizer
/// ([`parse_sizer`]), so a line that writes an array before its sizer reloads
/// it against the default count — truncated, zero-filled, or rejected as a
/// matrix of the wrong order. A set sizer `S` is hoisted when a property it
/// sizes precedes it in the chain (a sizer re-set after its arrays):
///
/// 1. `S` moves to just behind its **anchor** — the last chain member ahead of
///    it that is an object reference (`ObjectRef` / `ObjectRefArray`) or
///    another sizing property of the class — or to the head of the line when
///    there is none. A reference re-sets sizers on reload (r4133
///    `PDElements/Line.pas:420` + `:436` `FetchLineCode`, `:2145` + `:2150`
///    `FetchGeometryCode`, `:2000-2004` `FetchWireList`'s ratings,
///    `General/LineGeometry.pas:383` the `wire` arm's ratings,
///    `PDElements/Transformer.pas:2334` `FetchXfmrCode`'s `SetNumWindings`),
///    so writing `S` ahead of it would let the reference clobber it; sizers
///    never pass one another, so they keep their chain order.
/// 2. Every property sized by `S` that is still ahead of it moves to directly
///    behind `S`, keeping its chain order — which keeps the per-winding cursor
///    tokens (`Wdg`, `Bus`, `kV`, …) in their relative order.
///
/// Nothing else moves, and no token is added or dropped. Values are rendered
/// live, so writing an array after a reference or a sizer it used to precede
/// reloads the value the object holds now.
fn save_order(cls: &ClassProps, data: &DssObjData) -> Option<Vec<usize>> {
    let chain: Vec<usize> = std::iter::successors(data.next_property_set(None), |&p| {
        data.next_property_set(Some(p))
    })
    .collect();
    // Fast path: most lines hold no sized property at all.
    if chain.iter().all(|&p| parse_sizer(cls, p).is_none()) {
        return None;
    }
    let n = cls.num_properties();
    let sizer_of: Vec<Option<usize>> = (0..=n)
        .map(|p| if p == 0 { None } else { parse_sizer(cls, p) })
        .collect();
    let mut is_anchor: Vec<bool> = (0..=n)
        .map(|p| {
            p != 0
                && matches!(
                    cls.prop(p).ptype,
                    PropType::ObjectRef | PropType::ObjectRefArray
                )
        })
        .collect();
    for &s in sizer_of.iter().flatten() {
        is_anchor[s] = true;
    }
    let mut order = chain.clone();
    for &s in &chain {
        let pos = order.iter().position(|&p| p == s)?;
        if !order[..pos].iter().any(|&p| sizer_of[p] == Some(s)) {
            continue;
        }
        order.remove(pos);
        let at = order[..pos]
            .iter()
            .rposition(|&p| is_anchor[p])
            .map_or(0, |a| a + 1);
        let ahead: Vec<usize> = order[..at]
            .iter()
            .copied()
            .filter(|&p| sizer_of[p] == Some(s))
            .collect();
        order.retain(|p| !ahead.contains(p));
        let at = at - ahead.len();
        order.insert(at, s);
        order.splice(at + 1..at + 1, ahead);
    }
    (order != chain).then_some(order)
}

/// Pascal `WriteDSSObject` (`Utilities.pas:1221-1235`): one script line —
/// `<New|Edit> "Class.name"` (the full name always double-quoted) +
/// [`save_write`] + ` ENABLED=NO` when the object is a **disabled** circuit
/// element (`DSSObjType and ClassMask <> DSS_Object`), then mark the object
/// `HasBeenSaved`.
///
/// **The sizing-property hoist** ([`save_order`]) is applied here, once, for the
/// generic walk and every override alike: when it re-orders the line, the
/// object's set-order chain is re-stamped in that order
/// ([`DssObjData::set_as_next_seq`], each member once) for the duration of the
/// `SaveWrite` dispatch and then restored exactly
/// ([`DssObjData::copy_prp_sequence_from`] the snapshot, the counter slot
/// included). It is dss_capi 0.14.5's own device — `TLoadShapeObj.SaveWrite`
/// stamps `PrpSequence[npts] := -999` before calling `inherited`
/// (`CAPI:General/LoadShape.pas:2376-2380`) — generalized to every sizer and
/// made non-persistent: the chain a later `Save`, the JSON export or an
/// `EndEdit` boundary test reads is the one the deck built.
///
/// [`DssObjData::set_as_next_seq`]: crate::obj::base::DssObjData::set_as_next_seq
/// [`DssObjData::copy_prp_sequence_from`]: crate::obj::base::DssObjData::copy_prp_sequence_from
pub fn write_dss_object(
    out: &mut String,
    cx: &SaveCtx,
    arena: &mut crate::obj::arena::ClassArena,
    idx: usize,
    new_or_edit: &str,
) {
    out.push_str(new_or_edit);
    out.push_str(" \"");
    out.push_str(cx.cls.class_name());
    out.push('.');
    out.push_str(arena.obj(idx).data().name());
    out.push('"');
    // The sizing-property hoist: re-stamp the chain in save order; restored
    // right after the dispatch below.
    let chain_snapshot = save_order(cx.cls, arena.obj(idx).data()).map(|order| {
        let data = arena.obj_mut(idx).data_mut();
        let snapshot = data.clone();
        for p in order {
            data.set_as_next_seq(p);
        }
        snapshot
    });
    // Pascal models `SaveWrite` as a virtual method; `TTransfObj` overrides it
    // (the per-winding structure needs the array-property rewrite — see
    // `elements/pd/transformer/save.rs`). Every other class uses the generic
    // form. (More overrides are added here if/when a class needs one.)
    //
    // RP3.11 §1.1: the override set is the **union** of both upstreams', because
    // every one of them exists to keep the emitted deck re-compilable. It is the
    // complete union, class for class, and it was completed by the RP3.11
    // settlement: the 0.14.5-derived Transformer / AutoTrans / LineGeometry /
    // Line four, r4133's XYcurve (`XYcurve.pas:978-1003`) and RegControl
    // (`RegControl.pas:1399-1421`) allocation/ordering guards, and 0.14.5's
    // XfmrCode (`CAPI:General/XfmrCode.pas:667-745`, the winding rewrite —
    // r4133 has none and drops every winding but the active one) plus DynEqPCE
    // (`CAPI:PCElements/DynEqPCE.pas:252-273`, the `UserDynInit` tail, appended
    // below because Pascal calls `inherited SaveWrite` first). 0.14.5's ninth,
    // `TLoadShapeObj.SaveWrite`, is the `LoadShape` rule of [`save_write`].
    if let Some(xf) = arena.get::<crate::elements::pd::transformer::Transformer>(idx) {
        xf.save_write_body(out, cx);
    } else if let Some(at) = arena.get::<crate::elements::pd::auto_trans::AutoTrans>(idx) {
        at.save_write_body(out, cx);
    } else if let Some(lg) =
        arena.get::<crate::elements::general::line_geometry::LineGeometryObj>(idx)
    {
        lg.save_write_body(out, cx);
    } else if let Some(ln) = arena.get::<crate::elements::pd::line::Line>(idx) {
        ln.save_write_body(out, cx);
    } else if let Some(xy) = arena.get::<crate::elements::general::xy_curve::XyCurveObj>(idx) {
        xy.save_write_body(out, cx);
    } else if let Some(rc) = arena.get::<crate::elements::control::reg_control::RegControl>(idx) {
        rc.save_write_body(out, cx);
    } else if let Some(xc) = arena.get::<crate::elements::general::xfmr_code::XfmrCodeObj>(idx) {
        xc.save_write_body(out, cx);
    } else {
        save_write(out, cx, arena.obj(idx));
    }
    if let Some(snapshot) = chain_snapshot {
        arena
            .obj_mut(idx)
            .data_mut()
            .copy_prp_sequence_from(&snapshot);
    }
    // Pascal `TDynEqPCE.SaveWrite` (`CAPI:PCElements/DynEqPCE.pas:252-273`):
    // `inherited SaveWrite(F)` and then every `UserDynInit` assignment — the
    // `DynamicEq` state-variable initializers, which are not class properties
    // and so are in no `PrpSequence` chain. Without the tail a Generator /
    // PVSystem / Storage driven by a `DynamicEq` re-compiles with every
    // initializer lost. A `TJSONNumber` prints through `FloatToStr`, a
    // `TJSONString` through `CheckForBlanks` (the same split the AltDSS JSON
    // `"DynInit"` tail makes, `report/export/json/build.rs`).
    if let Some(dyneq) = arena.obj(idx).as_dyneq() {
        for (var, val) in &dyneq.user_dyn_init {
            out.push(' ');
            out.push_str(var);
            out.push('=');
            match val {
                crate::elements::pc::dyneq_pce::DynInitValue::Number(n) => {
                    out.push_str(&crate::util::float_to_str(*n));
                }
                crate::elements::pc::dyneq_pce::DynInitValue::Text(t) => {
                    out.push_str(&crate::util::check_for_blanks(t));
                }
            }
        }
    }
    if arena
        .try_ckt_elem(idx)
        .is_some_and(|elem| !elem.cd().enabled)
    {
        out.push_str(" ENABLED=NO");
    }
    out.push('\n');
    arena.obj_mut(idx).data_mut().set_has_been_saved(true);
}

/// The `WriteClassFile` object loop (`Utilities.pas:1170-1189`) with
/// `saveFlags = []` (the only reachable set on both the `Save <class>` and the
/// command-path `Save circuit` routes — PHASE8_PLAN §WP8.5 step 5): build the
/// class file text and count the records actually written. The caller decides
/// the filename, creates the file, and deletes it again when `nrecords == 0`
/// (`:1191-1198`).
///
/// Skips ported 1:1: a disabled CktElement when `is_ckt_element` (Pascal
/// `IsCktElement and (not includeDisabled) and (not Enabled)` — `DoSaveCmd`
/// passes FALSE, so `Save <class>` writes disabled elements with `ENABLED=NO`),
/// and `Flg.HasBeenSaved`. The `excludeDefault`/`DefaultAndUnedited` skip is
/// dormant (`ExcludeDefault ∉ []`); the `isLoadShape and not Enabled` skip is
/// unreachable — `TLoadShapeObj.Enabled` is initialized TRUE and no script
/// path clears it (API-only), so the Rust LoadShape carries no such field.
pub(crate) fn class_file_text(
    cls: &mut DssClass,
    enums: &EnumRegistry,
    is_ckt_element: bool,
) -> (String, usize) {
    let DssClass { props, arena, .. } = cls;
    let cx = SaveCtx { cls: props, enums };
    let mut out = String::new();
    let mut nrecords = 0usize;
    for i in 0..arena.len() {
        if is_ckt_element && arena.try_ckt_elem(i).is_some_and(|e| !e.cd().enabled) {
            continue;
        }
        if arena.obj(i).data().has_been_saved() {
            continue;
        }
        write_dss_object(&mut out, &cx, arena, i, "New");
        nrecords += 1;
    }
    (out, nrecords)
}

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
//!   exists to make the emitted deck re-compile.
//!
//! [`DssObjData::next_property_set`]: crate::obj::base::DssObjData::next_property_set
//! [`ClassProps::get_value`]: crate::obj::props::ClassProps::get_value

use crate::exec::registry::DssClass;
use crate::obj::base::DssObject;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::ClassProps;

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
/// **The LoadShape `npts`-first branch** (RP3.11 P2, `:139-150` + `:163-172`,
/// r4133-only — neither 0.14.5 nor this port had it). When the parent class is
/// `LoadShape` the walk starts at property 1 (`npts`) instead of at the head of
/// the chain, then restarts the chain from the beginning and ignores index 1
/// when it comes up again (`LShpFlag`/`NptsRdy`): *"created to guarantee that
/// the npts property will be the first to be declared when saving LoadShapes"*.
/// A LoadShape that re-emits `Mult=` before `NPts=` reloads with the wrong
/// point count, so this is a re-compilability guard, not a spelling.
///
/// One deliberate deviation from the letter of the Pascal, forced by a
/// *sequence* difference and measured against the live r4133 DLL: Pascal tests
/// `NptsRdy` only on the non-restart advance, so a chain whose **head** is index
/// 1 would print `npts` twice. r4133 never hits that case, because
/// `TLoadShapeObj.Set_NumPoints` re-stamps `PropertyValue[1]` (`LoadShape.pas:
/// 1665-1677`) every time an array property is parsed (`:631-636`, *"Keep
/// Properties in order for save command"*) — which pushes `npts` to the *back*
/// of its chain. This port has no such re-stamp (RP3.11 §4 adds and removes no
/// sequence site), so its chain does head with `npts`; applying the `NptsRdy`
/// skip to the restart as well is what reproduces r4133's measured bytes —
/// `New "LoadShape.ls1" npts=3 interval=1 mult=[ 1 2 3]`, one `npts`, for a deck
/// typing `npts` first *and* for one re-setting it last (epri-worker probe,
/// OpenDSSDirect.dll 11.0.0.1 r4133, RP3.11 I1).
pub fn save_write(out: &mut String, cx: &SaveCtx, obj: &dyn DssObject) {
    /// Pascal property 1 = LoadShape's `npts` (`LoadShape.pas:225`;
    /// [`crate::elements::general::load_shape`] `prop::NPTS`).
    const NPTS: usize = 1;

    // Pascal `if ParentClass.Name = 'LoadShape'` — a case-sensitive compare on
    // the registered class name, so only LoadShape (not PriceShape/TempShape).
    let load_shape = cx.cls.class_name() == "LoadShape";
    let mut iprop = if load_shape {
        Some(NPTS) // Pascal `iProp := 1`
    } else {
        obj.data().next_property_set(None)
    };
    let mut lshp_flag = load_shape;
    let mut npts_ready = false;
    while let Some(i) = iprop {
        save_write_token(out, cx, obj, i);
        if lshp_flag {
            // Pascal: start the chain over, `npts` is already processed.
            lshp_flag = false;
            npts_ready = true;
            iprop = obj.data().next_property_set(None);
        } else {
            iprop = obj.data().next_property_set(Some(i));
        }
        if npts_ready && iprop == Some(NPTS) {
            iprop = obj.data().next_property_set(Some(NPTS));
        }
    }
}

/// Pascal `WriteDSSObject` (`Utilities.pas:1221-1235`): one script line —
/// `<New|Edit> "Class.name"` (the full name always double-quoted) +
/// [`save_write`] + ` ENABLED=NO` when the object is a **disabled** circuit
/// element (`DSSObjType and ClassMask <> DSS_Object`), then mark the object
/// `HasBeenSaved`.
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
    // Pascal models `SaveWrite` as a virtual method; `TTransfObj` overrides it
    // (the per-winding structure needs the array-property rewrite — see
    // `elements/pd/transformer/save.rs`). Every other class uses the generic
    // form. (More overrides are added here if/when a class needs one.)
    //
    // RP3.11 §1.1: the override set is the **union** of both upstreams', because
    // every one of them exists to keep the emitted deck re-compilable — the
    // 0.14.5-derived Transformer/AutoTrans/LineGeometry/Line four, plus r4133's
    // XYcurve (`XYcurve.pas:978-1003`) and RegControl
    // (`RegControl.pas:1399-1421`) allocation/ordering guards.
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
    } else {
        save_write(out, cx, arena.obj(idx));
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

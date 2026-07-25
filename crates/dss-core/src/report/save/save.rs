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
//! [`DssObjData::next_property_set`]: crate::obj::base::DssObjData::next_property_set

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

/// Pascal `TDSSObject.SaveWrite` (`DSSObject.pas:145-165`): append
/// ` <Name>=<CheckForBlanks(value)>` for **only the explicitly-set properties,
/// in the order they were actually set** (`GetNextPropertySet`), skipping empty
/// values and the `----` conditional-value sentinel (`CompareText = 0`, so
/// case-insensitive — faithfully mirrored though the sentinel is emitted
/// literally).
pub fn save_write(out: &mut String, cx: &SaveCtx, obj: &dyn DssObject) {
    let mut iprop = obj.data().next_property_set(None);
    while let Some(i) = iprop {
        // Pascal `str := trim(PropertyValue[iProp])`.
        let val = cx.cls.get_value(obj, i, cx.enums);
        let mut s = val.trim();
        if s.eq_ignore_ascii_case("----") {
            s = ""; // set to ignore this property
        }
        if !s.is_empty() {
            out.push(' ');
            out.push_str(cx.cls.property_name(i));
            out.push('=');
            // Pascal `CheckForBlanks` (`Utilities.pas:1212`): quote a value
            // containing spaces unless it starts with a quote/bracket char.
            out.push_str(&crate::util::check_for_blanks(s));
        }
        iprop = obj.data().next_property_set(Some(i));
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
    obj: &mut dyn DssObject,
    new_or_edit: &str,
) {
    out.push_str(new_or_edit);
    out.push_str(" \"");
    out.push_str(cx.cls.class_name());
    out.push('.');
    out.push_str(obj.data().name());
    out.push('"');
    // Pascal models `SaveWrite` as a virtual method; `TTransfObj` overrides it
    // (the per-winding structure needs the array-property rewrite — see
    // `elements/pd/transformer/save.rs`). Every other class uses the generic
    // form. (More overrides are added here if/when a class needs one.)
    if let Some(xf) = obj
        .as_any()
        .downcast_ref::<crate::elements::pd::transformer::Transformer>()
    {
        xf.save_write_body(out, cx);
    } else if let Some(at) = obj
        .as_any()
        .downcast_ref::<crate::elements::pd::auto_trans::AutoTrans>()
    {
        at.save_write_body(out, cx);
    } else if let Some(lg) = obj
        .as_any()
        .downcast_ref::<crate::elements::general::line_geometry::LineGeometryObj>()
    {
        lg.save_write_body(out, cx);
    } else if let Some(ln) = obj
        .as_any()
        .downcast_ref::<crate::elements::pd::line::Line>()
    {
        ln.save_write_body(out, cx);
    } else {
        save_write(out, cx, &*obj);
    }
    if obj.as_ckt_element().is_some_and(|elem| !elem.cd().enabled) {
        out.push_str(" ENABLED=NO");
    }
    out.push('\n');
    obj.data_mut().set_has_been_saved(true);
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
    for obj in arena.objs_mut() {
        if is_ckt_element && obj.as_ckt_element().is_some_and(|e| !e.cd().enabled) {
            continue;
        }
        if obj.data().has_been_saved() {
            continue;
        }
        write_dss_object(&mut out, &cx, obj, "New");
        nrecords += 1;
    }
    (out, nrecords)
}

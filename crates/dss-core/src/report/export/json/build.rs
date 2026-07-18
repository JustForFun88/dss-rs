//! `obj_to_json_data` / `batch_to_json` — the object-level assembly that drives
//! the per-property renderer: loop-for-loop ports of `Obj_ToJSONData`
//! (`.inputs/dss_capi/src/CAPI/CAPI_Obj.pas:626-760`) and `Batch_ToJSON`
//! (`:1201-1254`). The header (`Name`/`DSSClass`), the default set-order sweep
//! with the redundant/array-alternative `iPropNext2` deferral, the `Full` sweep,
//! and the skip flags all live here; the value of each surviving property comes
//! from [`ClassProps::get_json_value`](crate::obj::props::class_props).

use crate::elements::pc::dyneq_pce::DynInitValue;
use crate::obj::base::DssObject;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropFlags, PropType};

use super::{Json, JsonOpts};

/// Pascal `PropertyType[iProp] in [ ... ]` — the array-related types eligible
/// for the redundant-property deferral (`CAPI_Obj.pas:702-715`). `DoubleProperty`
/// is included (Vsource `R1`→`Z1`, LineCode `B1`→`C1`).
fn is_deferral_type(t: PropType) -> bool {
    matches!(
        t,
        PropType::BusesOnStruct
            | PropType::EnumArrayOnStruct
            | PropType::MappedStringEnumArray
            | PropType::DoubleArrayOnStruct
            | PropType::ObjectRefArray
            | PropType::IntegerArray
            | PropType::DoubleFArray
            | PropType::DoubleVArray
            | PropType::DoublePoints
            | PropType::DoubleArray
            | PropType::Double
    )
}

/// Pascal `Obj_ToJSONData`: build the ordered JSON object for `obj`.
pub fn obj_to_json_data(
    cls: &ClassProps,
    obj: &dyn DssObject,
    enums: &EnumRegistry,
    opts: JsonOpts,
) -> Json {
    let lowercase = opts.contains(JsonOpts::LOWERCASE_KEYS);
    let mut members: Vec<(String, Json)> = Vec::new();

    // Header — `{"DSSClass":cls,"Name":name}` under IncludeDSSClass (lowercase
    // key `dssclass` under LowercaseKeys), else `{"Name":name}`.
    if opts.contains(JsonOpts::INCLUDE_DSS_CLASS) {
        let key = if lowercase { "dssclass" } else { "DSSClass" };
        members.push((key.to_string(), Json::Str(cls.class_name().to_string())));
    }
    members.push(("Name".to_string(), Json::Str(obj.data().name().to_string())));

    let n = cls.num_properties();

    if !opts.contains(JsonOpts::FULL) {
        // Default: only *filled* properties, in set-order, with the
        // redundant/array-alternative deferral (`CAPI_Obj.pas:665-733`).
        let mut done = vec![false; n + 1];
        let mut i_prop_next = obj.data().next_property_set(None).unwrap_or(0);
        let mut i_prop_next2 = 0usize;
        while i_prop_next > 0 {
            let i_prop = i_prop_next;
            if i_prop_next2 != 0 {
                // keep the previous ordering after a deferral
                i_prop_next = i_prop_next2;
                i_prop_next2 = 0;
            } else {
                i_prop_next = obj.data().next_property_set(Some(i_prop)).unwrap_or(0);
            }
            if done[i_prop] {
                continue;
            }
            done[i_prop] = true;

            let pd = cls.prop(i_prop);
            // If redundant and array-related, prefer the singular original
            // (may chain multiple levels via iPropNext2).
            if pd.flags.contains(PropFlags::REDUNDANT)
                && pd.redundant_with != 0
                && (pd.flags.contains(PropFlags::ON_ARRAY) || is_deferral_type(pd.ptype))
            {
                i_prop_next2 = i_prop_next;
                i_prop_next = pd.redundant_with;
                continue;
            }

            // Skip Like, substructure index, suppressed, or 0.15.x/r4133-deferred
            // props.
            if pd.ptype == PropType::MakeLike
                || (pd.flags.suppresses_json_output() && !pd.flags.contains(PropFlags::REDUNDANT))
                || pd.flags.hidden_from_full_enum()
                || pd.flags.contains(PropFlags::ALT_INDEX)
                || pd.flags.contains(PropFlags::INTEGER_STRUCT_INDEX)
            {
                continue;
            }

            if let Some(v) = cls.get_json_value(obj, i_prop, enums, opts, true) {
                members.push((pd.json_key(lowercase), v));
            }
        }
    } else {
        // Full: every property (`CAPI_Obj.pas:735-751`).
        for i_prop in 1..=n {
            let pd = cls.prop(i_prop);
            if opts.contains(JsonOpts::SKIP_REDUNDANT) && pd.flags.contains(PropFlags::REDUNDANT) {
                continue;
            }
            if pd.flags.suppresses_json_output()
                || pd.flags.hidden_from_full_enum()
                || pd.flags.contains(PropFlags::ALT_INDEX)
                || pd.flags.contains(PropFlags::INTEGER_STRUCT_INDEX)
            {
                continue;
            }
            if let Some(v) = cls.get_json_value(obj, i_prop, enums, opts, true) {
                members.push((pd.json_key(lowercase), v));
            }
        }
    }

    // The `TDynEqPCE` `"DynInit"` tail (`CAPI_Obj.pas:752-759`): for an object
    // that is a `TDynEqPCE` (Generator/PVSystem/Storage) whose `UserDynInit`
    // is non-NIL — i.e. at least one `DynamicEq` state-variable assignment was
    // parsed — append a literal `"DynInit"` object of its var→value init
    // assignments. The `"DynInit"` key is emitted verbatim (Pascal `resObj.Add`
    // with a string literal — never lowercased, even under LowercaseKeys). The
    // inner keys are the already-lowercased variable names; each value is a JSON
    // number (a plain constant) or a JSON string (a calc-value operand or an
    // RPN constant). Appended in both default and Full sweeps, always last.
    if let Some(dyneq) = obj.as_dyneq()
        && !dyneq.user_dyn_init.is_empty()
    {
        let dyn_members: Vec<(String, Json)> = dyneq
            .user_dyn_init
            .iter()
            .map(|(k, v)| {
                let jv = match v {
                    DynInitValue::Number(n) => Json::Float(*n),
                    DynInitValue::Text(s) => Json::Str(s.clone()),
                };
                (k.clone(), jv)
            })
            .collect();
        members.push(("DynInit".to_string(), Json::Obj(dyn_members)));
    }

    Json::Obj(members)
}

/// Pascal `Batch_ToJSON`: a JSON array of `obj_to_json_data` over a class's
/// objects. `ExcludeDisabled` drops disabled circuit elements. The Pascal
/// `IncludeDefaultObjs`/`Flg.DefaultAndUnedited` gating is a no-op here: the Rust
/// registry carries no `DefaultAndUnedited` flag (that flag and the auto-created
/// default objects are a Stage-B dependency), so every object is treated as
/// edited and always included.
pub fn batch_to_json(
    cls: &ClassProps,
    objects: &[Box<dyn DssObject>],
    enums: &EnumRegistry,
    opts: JsonOpts,
) -> Json {
    let exclude_disabled = opts.contains(JsonOpts::EXCLUDE_DISABLED);
    // Pascal branches on whether the FIRST element is a TDSSCktElement.
    let is_ckt = objects
        .first()
        .map(|o| o.as_ckt_element().is_some())
        .unwrap_or(false);

    let mut arr = Vec::with_capacity(objects.len());
    if !exclude_disabled || !is_ckt {
        for o in objects {
            arr.push(obj_to_json_data(cls, o.as_ref(), enums, opts));
        }
    } else {
        for o in objects {
            if o.as_ckt_element().map(|e| e.cd().enabled).unwrap_or(false) {
                arr.push(obj_to_json_data(cls, o.as_ref(), enums, opts));
            }
        }
    }
    Json::Arr(arr)
}

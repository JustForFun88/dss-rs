//! Synthetic per-`PropType` arm tests for [`ClassProps::get_json_value`]
//! (JSON_EXPORT_PLAN §3.1). The golden byte tests (`golden_json.rs`) pin the
//! arms that real classes exercise against the oracle; these cover the remaining
//! arms and the value-shaping edge cases (NaN→null, integer `value_offset`,
//! `MappedIntEnum`, `AllowNone`→null, `IntegerArray`/`StringList` shape,
//! ObjectRef Name-vs-FullName) on a hand-built object where the exact input is
//! controlled.

use crate::obj::base::{DssObjData, DssObject};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use crate::report::export::json::{JsonOpts, serialize};

/// A minimal object returning fixed values for every accessor an arm may call.
#[derive(Clone)]
struct Mock {
    data: DssObjData,
    f64s: Vec<f64>,
    i32s: Vec<i32>,
    strings: Vec<String>,
    f64_arr: Option<Vec<f64>>,
    i32_arr: Option<Vec<i32>>,
    str_list: Vec<String>,
    matrix: Option<(Vec<f64>, usize)>,
}

impl Mock {
    fn new(n: usize) -> Self {
        Self {
            data: DssObjData::new("m", n),
            f64s: vec![0.0; n + 1],
            i32s: vec![0; n + 1],
            strings: vec![String::new(); n + 1],
            f64_arr: None,
            i32_arr: None,
            str_list: Vec::new(),
            matrix: None,
        }
    }
}

impl DssObject for Mock {
    fn data(&self) -> &DssObjData {
        &self.data
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.data
    }
    fn get_f64(&self, idx: usize) -> f64 {
        self.f64s[idx]
    }
    fn get_i32(&self, idx: usize) -> i32 {
        self.i32s[idx]
    }
    fn get_bool(&self, idx: usize) -> bool {
        self.i32s[idx] != 0
    }
    fn get_string(&self, idx: usize) -> String {
        self.strings[idx].clone()
    }
    fn get_complex(&self, _idx: usize) -> (f64, f64) {
        (1.5, -2.5)
    }
    fn get_f64_array(&self, _idx: usize) -> Option<&[f64]> {
        self.f64_arr.as_deref()
    }
    fn get_i32_array(&self, _idx: usize) -> Option<&[i32]> {
        self.i32_arr.as_deref()
    }
    fn get_string_list(&self, _idx: usize) -> Vec<String> {
        self.str_list.clone()
    }
    fn array_size(&self, _idx: usize) -> usize {
        self.f64_arr.as_ref().map(|v| v.len()).unwrap_or(0)
    }
    fn get_matrix_part(&self, _idx: usize, _real: bool) -> Option<(Vec<f64>, usize)> {
        self.matrix.clone()
    }
    fn get_object_ref_names(&self, _idx: usize) -> Vec<String> {
        self.str_list.clone()
    }
    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

/// Build a `ClassProps` from raw prop rows (index i+1 = defs[i]).
fn props(defs: Vec<PropDef>) -> ClassProps {
    ClassProps::new("Mock", defs, true)
}

fn render(cls: &ClassProps, obj: &Mock, idx: usize, opts: JsonOpts) -> Option<String> {
    let enums = EnumRegistry::default();
    cls.get_json_value(obj, idx, &enums, opts, true)
        .map(|j| serialize(&j, opts))
}

#[test]
fn double_finite_and_nan_null() {
    let cls = props(vec![PropDef::double("A"), PropDef::double("B")]);
    let mut obj = Mock::new(cls.num_properties());
    obj.f64s[1] = 12.47;
    obj.f64s[2] = f64::NAN;
    assert_eq!(
        render(&cls, &obj, 1, JsonOpts::NONE).unwrap(),
        "1.2470000000000001E+001"
    );
    // NaN → null (never reaches the float formatter).
    assert_eq!(render(&cls, &obj, 2, JsonOpts::NONE).unwrap(), "null");
    // +Inf also → null.
    obj.f64s[2] = f64::INFINITY;
    assert_eq!(render(&cls, &obj, 2, JsonOpts::NONE).unwrap(), "null");
}

#[test]
fn integer_value_offset_and_mapped_int_enum() {
    let cls = props(vec![
        PropDef::integer("Shots")
            .flags(PropFlags::VALUE_OFFSET)
            .value_offset(-1.0),
        // MappedIntEnum renders the bare ordinal, not the enum name.
        PropDef::mapped_int_enum("K", EnumRegistry::default().connection),
    ]);
    let mut obj = Mock::new(cls.num_properties());
    obj.i32s[1] = 4; // stored NumReclose = Shots-1 = 4 → dumps 4-(-1)=5
    obj.i32s[2] = 2;
    assert_eq!(render(&cls, &obj, 1, JsonOpts::NONE).unwrap(), "5");
    assert_eq!(render(&cls, &obj, 2, JsonOpts::NONE).unwrap(), "2");
}

#[test]
fn boolean_and_complex() {
    let cls = props(vec![PropDef::boolean("On"), PropDef::complex("Z")]);
    let mut obj = Mock::new(cls.num_properties());
    obj.i32s[1] = 1;
    assert_eq!(render(&cls, &obj, 1, JsonOpts::NONE).unwrap(), "true");
    assert_eq!(
        render(&cls, &obj, 2, JsonOpts::NONE).unwrap(),
        "[1.5000000000000000E+000,-2.5000000000000000E+000]"
    );
}

#[test]
fn mapped_string_enum_string_vs_enum_as_int() {
    let enums = EnumRegistry::default();
    let cls = props(vec![PropDef::mapped_string_enum("Conn", enums.connection)]);
    let mut obj = Mock::new(cls.num_properties());
    obj.i32s[1] = 1; // delta
    let s = render(&cls, &obj, 1, JsonOpts::NONE).unwrap();
    assert!(s.starts_with('"'), "string form: {s}");
    // EnumAsInt → bare ordinal.
    assert_eq!(render(&cls, &obj, 1, JsonOpts::ENUM_AS_INT).unwrap(), "1");
}

#[test]
fn double_array_null_and_values() {
    // size_prop = 2 (the count integer).
    let cls = props(vec![PropDef::integer("N"), PropDef::double_array("A", 1)]);
    let mut obj = Mock::new(cls.num_properties());
    obj.i32s[1] = 3;
    // NIL pointer → null.
    assert_eq!(render(&cls, &obj, 2, JsonOpts::NONE).unwrap(), "null");
    obj.f64_arr = Some(vec![1.0, 2.0, 3.0]);
    assert_eq!(
        render(&cls, &obj, 2, JsonOpts::NONE).unwrap(),
        "[1.0000000000000000E+000,2.0000000000000000E+000,3.0000000000000000E+000]"
    );
}

#[test]
fn double_v_array_allow_none_null() {
    let cls = props(vec![
        PropDef::double_v_array("A").flags(PropFlags::ALLOW_NONE),
    ]);
    let obj = Mock::new(cls.num_properties()); // f64_arr None → array_size 0
    // AllowNone + count 0 → null.
    assert_eq!(render(&cls, &obj, 1, JsonOpts::NONE).unwrap(), "null");
}

#[test]
fn integer_array_and_string_list() {
    let cls = props(vec![
        PropDef::integer("N"),
        PropDef::int_array("S", 1),
        PropDef::string_list("Opt"),
    ]);
    let mut obj = Mock::new(cls.num_properties());
    obj.i32s[1] = 2;
    obj.i32_arr = Some(vec![7, 8]);
    obj.str_list = vec!["a".into(), "b".into()];
    assert_eq!(render(&cls, &obj, 2, JsonOpts::NONE).unwrap(), "[7,8]");
    assert_eq!(
        render(&cls, &obj, 3, JsonOpts::NONE).unwrap(),
        r#"["a","b"]"#
    );
    // Empty string list → `[]` (unlike the text dump's `""`).
    obj.str_list.clear();
    assert_eq!(render(&cls, &obj, 3, JsonOpts::NONE).unwrap(), "[]");
}

#[test]
fn sym_matrix_nested_and_null() {
    let cls = props(vec![PropDef::sym_matrix_real("M", 1)]);
    let mut obj = Mock::new(cls.num_properties());
    // Not allocated → null.
    assert_eq!(render(&cls, &obj, 1, JsonOpts::NONE).unwrap(), "null");
    // 2x2 column-major [a=1 (0,0), c=2 (1,0), b=3 (0,1), d=4 (1,1)] → rows
    // [[1,3],[2,4]] (element(i,j)=vals[j*order+i]).
    obj.matrix = Some((vec![1.0, 2.0, 3.0, 4.0], 2));
    assert_eq!(
        render(&cls, &obj, 1, JsonOpts::NONE).unwrap(),
        "[[1.0000000000000000E+000,3.0000000000000000E+000],\
         [2.0000000000000000E+000,4.0000000000000000E+000]]"
    );
}

#[test]
fn object_ref_null_name_and_full_name() {
    let cls = props(vec![
        PropDef::object_ref_class("LineCode", "LC"),
        PropDef::object_ref("Bare"),
    ]);
    let mut obj = Mock::new(cls.num_properties());
    // Empty stored name → null.
    assert_eq!(render(&cls, &obj, 1, JsonOpts::NONE).unwrap(), "null");
    obj.strings[1] = "code1".into();
    // Default: the plain name.
    assert_eq!(render(&cls, &obj, 1, JsonOpts::NONE).unwrap(), r#""code1""#);
    // FullNames: `Class.Name` prefixed with the resolving class.
    assert_eq!(
        render(&cls, &obj, 1, JsonOpts::FULL_NAMES).unwrap(),
        r#""LineCode.code1""#
    );
}

#[test]
fn object_ref_array_full_name_as_json_array() {
    let cls = props(vec![
        PropDef::object_ref_array("WireData", "W").flags(PropFlags::FULL_NAME_AS_JSON_ARRAY),
    ]);
    let mut obj = Mock::new(cls.num_properties());
    // Empty → [].
    assert_eq!(render(&cls, &obj, 1, JsonOpts::NONE).unwrap(), "[]");
    obj.str_list = vec!["w1".into(), "w2".into()];
    // FullNameAsJSONArray → FullName each, regardless of FullNames.
    assert_eq!(
        render(&cls, &obj, 1, JsonOpts::NONE).unwrap(),
        r#"["WireData.w1","WireData.w2"]"#
    );
}

#[test]
fn object_ref_on_array_branch() {
    // DSSObjectReferenceProperty + the `OnArray` flag renders an ARRAY of the
    // referenced names (DSSObjectHelper.pas:1169-1194), not the scalar form.
    let cls = props(vec![
        PropDef::object_ref_class("LineCode", "Codes").flags(PropFlags::ON_ARRAY),
    ]);
    let mut obj = Mock::new(cls.num_properties());
    // Empty → [].
    assert_eq!(render(&cls, &obj, 1, JsonOpts::NONE).unwrap(), "[]");
    obj.str_list = vec!["c1".into(), "c2".into()];
    // Default: plain names.
    assert_eq!(
        render(&cls, &obj, 1, JsonOpts::NONE).unwrap(),
        r#"["c1","c2"]"#
    );
    // FullNames → `Class.Name` each.
    assert_eq!(
        render(&cls, &obj, 1, JsonOpts::FULL_NAMES).unwrap(),
        r#"["LineCode.c1","LineCode.c2"]"#
    );
    // FULL_NAME_AS_ARRAY forces FullName regardless of the FullNames option.
    let cls2 = props(vec![
        PropDef::object_ref_class("LineCode", "Codes")
            .flags(PropFlags::ON_ARRAY | PropFlags::FULL_NAME_AS_ARRAY),
    ]);
    assert_eq!(
        render(&cls2, &obj, 1, JsonOpts::NONE).unwrap(),
        r#"["LineCode.c1","LineCode.c2"]"#
    );
}

#[test]
fn double_array_allow_none_null() {
    // AllowNone applies to the DoubleArray arm too (not only DoubleVArray) —
    // Pascal shares the `AllowNone and Norder=0 → null` block across
    // DoubleArray/DoubleDArray/DoubleVArray (DSSObjectHelper.pas:1218).
    let cls = props(vec![
        PropDef::integer("N"),
        PropDef::double_array("A", 1).flags(PropFlags::ALLOW_NONE),
    ]);
    let mut obj = Mock::new(cls.num_properties());
    obj.i32s[1] = 0; // count 0 + AllowNone → null (not `[]`).
    assert_eq!(render(&cls, &obj, 2, JsonOpts::NONE).unwrap(), "null");
    // count > 0 → the array as usual.
    obj.i32s[1] = 2;
    obj.f64_arr = Some(vec![1.0, 2.0]);
    assert_eq!(
        render(&cls, &obj, 2, JsonOpts::NONE).unwrap(),
        "[1.0000000000000000E+000,2.0000000000000000E+000]"
    );
}

#[test]
fn not_ported_and_make_like_arms() {
    // `ClassProps::new` appends the `Like` (MakeLike) prop automatically, so
    // prop 1 = X, prop 2 = Like.
    let cls = props(vec![PropDef::double("X").flags(PropFlags::NOT_PORTED)]);
    let obj = Mock::new(cls.num_properties());
    // NOT_PORTED (Pascal PropertyOffset = -1) → omitted (None).
    assert!(render(&cls, &obj, 1, JsonOpts::NONE).is_none());
    // MakeLike renders `""`.
    let like = cls.num_properties();
    assert_eq!(render(&cls, &obj, like, JsonOpts::NONE).unwrap(), r#""""#);
}

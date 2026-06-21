use super::common::*;
use crate::exec::*;

#[test]
fn new_and_query_defaults() {
    let mut dss = dss_with_circuit();
    dss.command("New TCC_Curve.test");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "TCC_Curve.test.NPts"), "0");
    assert_eq!(query(&mut dss, "TCC_Curve.test.C_Array"), "");
    assert_eq!(query(&mut dss, "TCC_Curve.test.T_Array"), "");
    assert_eq!(query(&mut dss, "TCC_Curve.test.Like"), "");
}

#[test]
fn new_with_inline_edits() {
    let mut dss = dss_with_circuit();
    dss.command("New TCC_Curve.t npts=3 C_array=(1 2 3) T_array=(0.1 0.2 0.3)");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "tcc_curve.t.npts"), "3");
    assert_eq!(query(&mut dss, "tcc_curve.t.c_array"), "[ 1 2 3]");
    assert_eq!(query(&mut dss, "tcc_curve.t.t_array"), "[ 0.1 0.2 0.3]");
}

#[test]
fn edit_and_more_continue_the_object() {
    let mut dss = dss_with_circuit();
    dss.command("New TCC_Curve.t npts=2");
    dss.command("Edit TCC_Curve.t C_array=(5 6)");
    dss.command("~ T_array=(9 8)");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "tcc_curve.t.c_array"), "[ 5 6]");
    assert_eq!(query(&mut dss, "tcc_curve.t.t_array"), "[ 9 8]");
}

#[test]
fn make_like_copies_state() {
    let mut dss = dss_with_circuit();
    dss.command("New TCC_Curve.a npts=2 C_array=(1 2) T_array=(3 4)");
    dss.command("New TCC_Curve.b like=a");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "tcc_curve.b.npts"), "2");
    assert_eq!(query(&mut dss, "tcc_curve.b.c_array"), "[ 1 2]");
    assert_eq!(query(&mut dss, "tcc_curve.b.t_array"), "[ 3 4]");
}

#[test]
fn make_like_copies_prp_sequence() {
    // Pascal `TDSSObject.MakeLike` copies the source's PrpSequence, then
    // the Edit loop stamps the Like property itself — so a Save-order walk
    // of the target yields NPts, C_Array, T_Array, Like.
    let mut dss = Dss::new();
    dss.command("New TCC_Curve.a npts=2 C_array=(1 2) T_array=(3 4)");
    dss.command("New TCC_Curve.b like=a");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let cls = &dss.classes[0];
    let oi = cls.name_to_idx["b"];
    let data = cls.objects[oi].data();
    assert_eq!(data.next_property_set(None), Some(1)); // NPts
    assert_eq!(data.next_property_set(Some(1)), Some(2)); // C_Array
    assert_eq!(data.next_property_set(Some(2)), Some(3)); // T_Array
    assert_eq!(data.next_property_set(Some(3)), Some(4)); // Like
    assert_eq!(data.next_property_set(Some(4)), None);
}

#[test]
fn set_and_get_require_a_circuit() {
    let mut dss = Dss::new();
    dss.command("Set mode=snap");
    dss.command("Get mode");
    assert_eq!(dss.errors().len(), 2, "{:?}", dss.errors());
    assert!(
        dss.errors()
            .iter()
            .all(|e| e.contains("You must create a new circuit object first")),
        "{:?}",
        dss.errors()
    );
}

#[test]
fn solve_requires_a_circuit() {
    let mut dss = Dss::new();
    dss.command("Solve");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("You must create a new circuit object first")),
        "{:?}",
        dss.errors()
    );
}

#[test]
fn duplicate_new_edits_existing() {
    let mut dss = dss_with_circuit();
    dss.command("New TCC_Curve.t npts=2 C_array=(1 2)");
    // A second "New" of the same name becomes an edit (DSS_OBJECT, no dups).
    dss.command("New TCC_Curve.t C_array=(7 8)");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "tcc_curve.t.c_array"), "[ 7 8]");
}

#[test]
fn clear_drops_objects() {
    let mut dss = dss_with_circuit();
    dss.command("New TCC_Curve.t npts=2");
    dss.command("Clear"); // drops the circuit and all objects
    assert!(dss.circuit().is_none());
    // `?` is circuit-gated again after Clear, like the Pascal.
    dss.command("? TCC_Curve.t.npts");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("You must create a new circuit object first")),
        "{:?}",
        dss.errors()
    );
    // After recreating a circuit, the old object is really gone.
    let mut dss = dss_with_circuit();
    dss.command("New circuit.again"); // second circuit is rejected
    assert!(!dss.errors().is_empty());
    assert_eq!(query(&mut dss, "TCC_Curve.t.npts"), "Property Unknown");
}

#[test]
fn unknown_parameter_is_reported() {
    let mut dss = Dss::new();
    dss.command("New TCC_Curve.t bogus=3");
    assert!(dss.errors().iter().any(|e| e.contains("Unknown parameter")));
}

#[test]
fn parse_obj_name_splits_on_last_class_dot() {
    assert_eq!(
        parse_obj_name("TCC_Curve.test.npts"),
        ("TCC_Curve.test".to_string(), "npts".to_string())
    );
    assert_eq!(
        parse_obj_name("test.npts"),
        ("test".to_string(), "npts".to_string())
    );
    assert_eq!(parse_obj_name("npts"), (String::new(), "npts".to_string()));
}

#[test]
fn new_circuit_creates_default_source() {
    let mut dss = Dss::new();
    dss.command("New circuit.test basekv=115 pu=1.0001");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let ckt = dss.circuit().unwrap();
    assert_eq!(ckt.name, "test");
    assert_eq!(ckt.sources.len(), 1);
    assert_eq!(query(&mut dss, "vsource.source.basekv"), "115");
    assert_eq!(query(&mut dss, "vsource.source.pu"), "1.0001");
    assert_eq!(query(&mut dss, "vsource.source.bus1"), "sourcebus");
}

#[test]
fn get_returns_set_values() {
    let mut dss = Dss::new();
    dss.command("New circuit.c1");
    dss.command("Set mode=daily tolerance=0.001 maxiterations=25");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.command("Get mode tolerance maxiterations");
    assert_eq!(dss.result(), "Daily, 0.001, 25");
}

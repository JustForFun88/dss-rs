use super::*;

#[test]
fn set_as_next_seq_tracks_order() {
    let mut d = DssObjData::new("t", 4);
    assert!(!d.prp_specified(2));
    d.set_as_next_seq(3);
    d.set_as_next_seq(1);
    d.set_as_next_seq(3); // re-setting bumps it to the latest order
    assert!(d.prp_specified(3));
    assert!(d.prp_specified(1));
    assert!(!d.prp_specified(2));
    // SaveWrite walk: order is 1 (seq 2) then 3 (seq 3); 3's first set
    // (seq 1) is superseded.
    assert_eq!(d.next_property_set(None), Some(1));
    assert_eq!(d.next_property_set(Some(1)), Some(3));
    assert_eq!(d.next_property_set(Some(3)), None);
}

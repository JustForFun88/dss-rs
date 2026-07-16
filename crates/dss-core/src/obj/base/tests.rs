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

#[test]
fn make_like_copies_prp_slots_but_not_the_edit_boundary() {
    // Pascal `TDSSObject.MakeLike` (DSSObject.pas:133) copies
    // `SizeOf(Integer)*(NumProperties+1)` bytes = the counter slot + every
    // property slot, but NOT the per-edit boundary at index NumProperties+1
    // (our `edit_seq_boundary`). Copying the boundary would mis-fire
    // RegControl's `EndEdit` signed-threshold legacy fallback under `like=`.
    let mut parent = DssObjData::new("parent", 4);
    parent.set_as_next_seq(2);
    parent.set_as_next_seq(3);
    parent.set_as_next_seq(2); // parent counter now at 3
    parent.begin_edit_boundary(); // 2nd edit: boundary := 3
    parent.set_as_next_seq(4); // counter 4, set after the boundary

    // Fresh child (a `New … like=parent`) starts with boundary 0.
    let mut child = DssObjData::new("child", 4);
    assert_eq!(child.edit_seq_boundary, 0);
    child.copy_prp_sequence_from(&parent);

    // Property slots (incl. the counter) are copied verbatim...
    assert_eq!(child.prp_sequence, parent.prp_sequence);
    // ...but the boundary stays the child's own (0), not the parent's 3.
    assert_eq!(child.edit_seq_boundary, 0);
    // With boundary 0, an inherited slot set at counter 4 reads as "edited
    // since boundary"; had we copied the parent's boundary (3) prop 4 (seq 4)
    // would still read edited, but prop 2/3 (seq <=3) would flip to unedited —
    // exactly the mis-fire the fix prevents.
    assert!(child.prop_edited_since_boundary(2));
    assert!(child.prop_edited_since_boundary(3));
    assert!(child.prop_edited_since_boundary(4));
}

use super::*;

fn er(idx: usize) -> ElemId {
    ElemId::new(0, idx)
}

/// `GoForward` is a LIFO sweep: at each level the **last**-added child is
/// visited first (Pascal pushes children in insertion order onto a stack).
#[test]
fn go_forward_is_lifo_over_children() {
    let mut t = CktTree::new();
    t.add(er(0)); // root
    t.add_new_child(er(1), NO_BUS, 1);
    t.add_new_child(er(2), NO_BUS, 1);
    t.add_new_child(er(3), NO_BUS, 1);
    // Traversal from the root: 3, 2, 1.
    let mut order = Vec::new();
    while let Some(e) = t.go_forward() {
        order.push(e.index());
    }
    assert_eq!(order, vec![3, 2, 1]);
    assert!(t.present.is_none());
}

/// The zone-build pattern: children added to the node currently being
/// visited are picked up by the same sweep (`ChildAdded` re-push).
#[test]
fn children_added_mid_sweep_are_visited() {
    let mut t = CktTree::new();
    t.add(er(0));
    t.add_new_child(er(1), NO_BUS, 1);
    let mut order = Vec::new();
    let mut visited = t.go_forward(); // -> 1
    while let Some(e) = visited {
        order.push(e.index());
        if e.index() == 1 {
            // grow the tree at the present branch mid-sweep
            t.add_new_child(er(2), NO_BUS, 1);
            t.add_new_child(er(3), NO_BUS, 1);
        }
        visited = t.go_forward();
    }
    assert_eq!(order, vec![1, 3, 2]);
}

/// Depth-first shape: a deep chain is followed before siblings
/// (children of the popped node are pushed on top of the stack).
#[test]
fn go_forward_descends_before_siblings() {
    let mut t = CktTree::new();
    let root = t.add(er(0));
    t.add_new_child(er(1), NO_BUS, 1);
    let c2 = t.add_new_child(er(2), NO_BUS, 1);
    // grandchildren under node 2
    t.present = Some(c2);
    t.add_new_child(er(21), NO_BUS, 1);
    t.add_new_child(er(22), NO_BUS, 1);
    t.present = Some(root);
    let mut order = Vec::new();
    while let Some(e) = t.go_forward() {
        order.push(e.index());
    }
    // Stack after root: [1, 2] -> pop 2, push [21, 22] -> pop 22, 21, then 1.
    assert_eq!(order, vec![2, 22, 21, 1]);
}

#[test]
fn first_resets_traversal_and_levels_track_depth() {
    let mut t = CktTree::new();
    t.add(er(0));
    let c1 = t.add_new_child(er(1), 7, 2);
    t.present = Some(c1);
    t.add_new_child(er(2), 8, 1);
    assert_eq!(t.node(c1).from_bus, 7);
    assert_eq!(t.node(c1).from_terminal, 2);

    let f = t.first().unwrap();
    assert_eq!(f.index(), 0);
    assert_eq!(t.level(), 0);
    assert_eq!(t.go_forward().unwrap().index(), 1);
    assert_eq!(t.level(), 1);
    assert_eq!(t.go_forward().unwrap().index(), 2);
    assert_eq!(t.level(), 2);
    assert_eq!(t.parent().unwrap().index(), 1);
    assert_eq!(t.go_backward().unwrap().index(), 1);
}

/// `Get_ToBusReference` sequential-access semantics: single entry always
/// returned; multiple entries iterate then yield `None` once and reset.
#[test]
fn to_bus_reference_cursor_semantics() {
    let mut n = TreeNode::new(er(0), None, 0);
    n.add_to_bus_reference(5);
    assert_eq!(n.next_to_bus_reference(), Some(5));
    assert_eq!(n.next_to_bus_reference(), Some(5)); // single: no cursor

    n.add_to_bus_reference(9);
    assert_eq!(n.next_to_bus_reference(), Some(5));
    assert_eq!(n.next_to_bus_reference(), Some(9));
    assert_eq!(n.next_to_bus_reference(), None); // exhausted, resets
    assert_eq!(n.next_to_bus_reference(), Some(5)); // fresh sweep
}

#[test]
fn zone_ends_accumulate() {
    let mut t = CktTree::new();
    let root = t.add(er(0));
    t.zone_ends.add(root, 42);
    assert_eq!(t.zone_ends.num_ends(), 1);
    assert_eq!(t.zone_ends.ends[0], (root, 42));
}

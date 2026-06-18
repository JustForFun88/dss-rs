use super::*;

#[test]
fn add_returns_sequential_indices() {
    let mut h = HashList::new();
    assert_eq!(h.add("SourceBus"), 0);
    assert_eq!(h.add("Bus1"), 1);
    assert_eq!(h.add("bus2"), 2);
    assert_eq!(h.len(), 3);
}

#[test]
fn find_is_case_insensitive() {
    let mut h = HashList::new();
    h.add("SourceBus");
    h.add("650");
    assert_eq!(h.find("sourcebus"), Some(0));
    assert_eq!(h.find("SOURCEBUS"), Some(0));
    assert_eq!(h.find("650"), Some(1));
    assert_eq!(h.find("651"), None);
}

#[test]
fn names_are_stored_lowercased() {
    let mut h = HashList::new();
    h.add("MyBus");
    assert_eq!(h.name(0), Some("mybus"));
    assert_eq!(h.name(1), None);
}

#[test]
fn duplicates_are_kept_and_enumerable() {
    // Pascal allowed duplicate names and exposed them via Find/FindNext.
    let mut h = HashList::new();
    h.add("load1");
    h.add("other");
    h.add("Load1");
    h.add("LOAD1");
    assert_eq!(h.find("load1"), Some(0));
    let all: Vec<usize> = h.indices_of("loAD1").collect();
    assert_eq!(all, vec![0, 2, 3]);
}

#[test]
fn clear_empties_the_list() {
    let mut h = HashList::with_capacity(4);
    h.add("a");
    h.add("b");
    h.clear();
    assert!(h.is_empty());
    assert_eq!(h.find("a"), None);
    assert_eq!(h.name(0), None);
    // reusable after clear
    assert_eq!(h.add("c"), 0);
    assert_eq!(h.find("C"), Some(0));
}

#[test]
fn iter_preserves_insertion_order() {
    let mut h = HashList::new();
    h.add("One");
    h.add("Two");
    h.add("Three");
    let names: Vec<&str> = h.iter().collect();
    assert_eq!(names, vec!["one", "two", "three"]);
}

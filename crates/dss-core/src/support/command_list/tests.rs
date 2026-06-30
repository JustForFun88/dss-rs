use super::*;

#[test]
fn exact_and_prefix_match() {
    let cl = CommandList::new(["New", "Edit", "More"]);
    assert_eq!(cl.get_command("new"), Some(0));
    assert_eq!(cl.get_command("NEW"), Some(0));
    assert_eq!(cl.get_command("n"), Some(0));
    assert_eq!(cl.get_command("ed"), Some(1));
    assert_eq!(cl.get_command("mor"), Some(2));
    assert_eq!(cl.get_command("zzz"), None);
}

#[test]
fn first_registered_wins_ambiguous_prefix() {
    // Both start with "s": the earlier command owns the shared prefixes.
    let cl = CommandList::new(["Select", "Save", "Set"]);
    assert_eq!(cl.get_command("s"), Some(0));
    assert_eq!(cl.get_command("se"), Some(0));
    assert_eq!(cl.get_command("sel"), Some(0));
    assert_eq!(cl.get_command("sa"), Some(1));
    assert_eq!(cl.get_command("set"), Some(2)); // full name beats prefix
}

#[test]
fn full_name_always_matches_even_as_prefix_of_another() {
    // "M" is itself a command and a prefix of "More" — registered names
    // go in first, so "m" must resolve to the command "M".
    let cl = CommandList::new(["More", "M"]);
    assert_eq!(cl.get_command("m"), Some(1));
    assert_eq!(cl.get_command("mo"), Some(0));
}

#[test]
fn abbrev_disabled_requires_full_name() {
    let mut cl = CommandList::new(["npts", "year", "mult"]);
    cl.abbrev_allowed = false; // GrowthShape does this (GrowthShape.pas)
    assert_eq!(cl.get_command("npts"), Some(0));
    assert_eq!(cl.get_command("np"), None);
    assert_eq!(cl.get_command("YEAR"), Some(1));
}

#[test]
fn get_returns_original_spelling() {
    let cl = CommandList::new(["TCC_Curve", "%Mag"]);
    assert_eq!(cl.get(0), Some("TCC_Curve"));
    assert_eq!(cl.get(1), Some("%Mag"));
    assert_eq!(cl.get_command("%m"), Some(1));
    assert_eq!(cl.get(2), None);
    assert_eq!(cl.len(), 2);
}

use super::*;

fn units() -> DssEnum {
    let reg = EnumRegistry::new();
    reg.get(reg.units).clone()
}

#[test]
fn units_exact_and_abbreviated() {
    let u = units();
    assert_eq!(u.string_to_ordinal("none").unwrap(), 0);
    assert_eq!(u.string_to_ordinal("mi").unwrap(), 1);
    assert_eq!(u.string_to_ordinal("kft").unwrap(), 2);
    assert_eq!(u.string_to_ordinal("km").unwrap(), 3);
    assert_eq!(u.string_to_ordinal("m").unwrap(), 4);
    assert_eq!(u.string_to_ordinal("meter").unwrap(), 4); // alias
    assert_eq!(u.string_to_ordinal("miles").unwrap(), 1); // alias
    assert_eq!(u.string_to_ordinal("ft").unwrap(), 5);
    assert_eq!(u.string_to_ordinal("in").unwrap(), 6);
    assert_eq!(u.string_to_ordinal("cm").unwrap(), 7);
    assert_eq!(u.string_to_ordinal("mm").unwrap(), 8);
}

#[test]
fn unmatched_falls_back_to_default_or_error() {
    let u = units();
    // UnitsEnum has DefaultValue = 0
    assert_eq!(u.string_to_ordinal("zz").unwrap(), 0);

    let mut nodefault = DssEnum::new("X", true, 1, 1, &["a", "b"], &[1, 2]);
    assert!(nodefault.string_to_ordinal("q").is_err());
    nodefault.default_value = 2;
    assert_eq!(nodefault.string_to_ordinal("q").unwrap(), 2);
}

#[test]
fn ordinal_to_string_roundtrip() {
    let u = units();
    assert_eq!(u.ordinal_to_string(0), "none");
    assert_eq!(u.ordinal_to_string(4), "m");
    assert_eq!(u.ordinal_to_string(8), "mm");
    assert_eq!(u.ordinal_to_string(99), ""); // out of range, not hybrid
}

#[test]
fn hybrid_parses_numbers() {
    let mut e = DssEnum::new(
        "Monitored Phase",
        true,
        1,
        2,
        &["min", "max", "avg"],
        &[-3, -2, -1],
    );
    e.hybrid = true;
    assert_eq!(e.string_to_ordinal("max").unwrap(), -2);
    assert_eq!(e.string_to_ordinal("2").unwrap(), 2);
    // hybrid_min clamps from below (Pascal Max(HybridMin, Result))
    assert_eq!(e.string_to_ordinal("0").unwrap(), 1);
    assert!(e.string_to_ordinal("xy").is_err());
    assert!(e.is_ordinal_valid(7));
    assert!(e.is_ordinal_valid(-3));
    assert!(!e.is_ordinal_valid(-7));
    assert_eq!(e.ordinal_to_string(12), "12");
}

#[test]
fn shorter_names_are_skipped_without_allow_longer() {
    // value longer than a name can never match it unless AllowLonger
    let u = units();
    assert_eq!(u.string_to_ordinal("kilometer").unwrap(), 0); // default, no match
}

#[test]
fn joined_lists_names() {
    let e = DssEnum::new("X", true, 1, 1, &["a", "b"], &[1, 2]);
    assert_eq!(e.joined(), "[a,b]");
}

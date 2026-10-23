use super::*;

#[test]
fn from_word_reads_a_listed_word_whole() {
    let codes = [
        ("none", LineUnits::None),
        ("mi", LineUnits::Miles),
        ("kft", LineUnits::Kft),
        ("km", LineUnits::Km),
        ("m", LineUnits::Meter),
        ("me", LineUnits::Meter),
        ("ft", LineUnits::Ft),
        ("in", LineUnits::Inch),
        ("cm", LineUnits::Cm),
        ("mm", LineUnits::Mm),
    ];
    for (word, unit) in codes {
        assert_eq!(LineUnits::from_word(word, CODE_WORDS), Some(unit), "{word}");
    }
    assert_eq!(
        LineUnits::from_word("MI", CODE_WORDS),
        Some(LineUnits::Miles)
    );
    assert_eq!(
        LineUnits::from_word("Ft", POSITION_WORDS),
        Some(LineUnits::Ft)
    );
    assert_eq!(
        LineUnits::from_word(" km ", LENGTH_WORDS),
        Some(LineUnits::Km)
    );
    for word in [
        "MILES", "meters", "meter", "fe", "feet", "", "zz", "k", "mils",
    ] {
        assert_eq!(LineUnits::from_word(word, CODE_WORDS), None, "{word}");
    }
    assert_eq!(LineUnits::from_word("me", LENGTH_WORDS), None);
    assert_eq!(LineUnits::from_word("me", POSITION_WORDS), None);
    assert_eq!(LineUnits::from_word("none", POSITION_WORDS), None);
    assert_eq!(
        LineUnits::from_word("none", LENGTH_WORDS),
        Some(LineUnits::None)
    );
}

#[test]
fn the_word_lists_hold_the_documented_words() {
    assert_eq!(POSITION_WORD_LIST, POSITION_WORDS.join(", "));
    assert_eq!(LENGTH_WORDS[1..], *POSITION_WORDS);
    assert_eq!(
        CODE_WORDS.iter().filter(|w| **w != "me").count(),
        LENGTH_WORDS.len()
    );
    for words in [POSITION_WORDS, LENGTH_WORDS, CODE_WORDS] {
        for word in words {
            assert!(LineUnits::from_word(word, words).is_some(), "{word}");
        }
    }
}

#[test]
fn codes_round_trip() {
    for code in 0..=8 {
        assert_eq!(LineUnits::from_code(code).code(), code);
    }
    assert_eq!(LineUnits::from_code(99), LineUnits::None);
    assert_eq!(LineUnits::from_code(-1), LineUnits::None);
}

#[test]
fn meters_conversions() {
    assert_eq!(LineUnits::Miles.to_meters(), 1609.344);
    assert_eq!(LineUnits::Kft.to_meters(), 304.8);
    assert_eq!(LineUnits::Ft.to_meters(), 0.3048);
    assert_eq!(LineUnits::None.to_meters(), 1.0);
    assert_eq!(LineUnits::Km.from_meters(), 1.0 / 1000.0);
}

#[test]
fn convert_between_units() {
    // ohms-per-mile → ohms-per-kft: 1609.344 m/mi ÷ 304.8 m/kft
    let f = convert_line_units(LineUnits::Miles, LineUnits::Kft);
    assert!((f - 1609.344 / 304.8).abs() < 1e-12);
    // None on either side: no conversion
    assert_eq!(convert_line_units(LineUnits::None, LineUnits::Km), 1.0);
    assert_eq!(convert_line_units(LineUnits::Km, LineUnits::None), 1.0);
}

#[test]
fn display_strings() {
    assert_eq!(LineUnits::Miles.to_string(), "mi");
    assert_eq!(LineUnits::None.to_string(), "none");
    assert_eq!(LineUnits::Kft.to_string(), "kft");
}

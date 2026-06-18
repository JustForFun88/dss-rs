use super::*;

#[test]
fn parse_all_unit_spellings() {
    assert_eq!(LineUnits::parse("none"), LineUnits::None);
    assert_eq!(LineUnits::parse("mi"), LineUnits::Miles);
    assert_eq!(LineUnits::parse("MILES"), LineUnits::Miles);
    assert_eq!(LineUnits::parse("kft"), LineUnits::Kft);
    assert_eq!(LineUnits::parse("km"), LineUnits::Km);
    assert_eq!(LineUnits::parse("m"), LineUnits::Meter);
    assert_eq!(LineUnits::parse("meters"), LineUnits::Meter);
    assert_eq!(LineUnits::parse("ft"), LineUnits::Ft);
    assert_eq!(LineUnits::parse("feet"), LineUnits::None); // "fe" unknown, like Pascal
    assert_eq!(LineUnits::parse("in"), LineUnits::Inch);
    assert_eq!(LineUnits::parse("cm"), LineUnits::Cm);
    assert_eq!(LineUnits::parse("mm"), LineUnits::Mm);
    assert_eq!(LineUnits::parse(""), LineUnits::None);
    assert_eq!(LineUnits::parse("zz"), LineUnits::None);
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

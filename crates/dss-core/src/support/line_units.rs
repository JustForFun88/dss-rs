//! Length-unit codes for line data, port of `Shared/LineUnits.pas`.

/// Line length units (Pascal `UNITS_*` constants). The discriminants match
/// the Pascal codes, which appear in property values and saved circuits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum LineUnits {
    #[default]
    None = 0,
    Miles = 1,
    Kft = 2,
    Km = 3,
    Meter = 4,
    Ft = 5,
    Inch = 6,
    Cm = 7,
    Mm = 8,
}

/// `UNITS_MAXNUM` in Pascal.
pub const UNITS_MAX_NUM: i32 = 9;

impl LineUnits {
    /// Numeric code as stored in DSS properties.
    pub fn code(self) -> i32 {
        self as i32
    }

    /// Unit from its numeric code; out-of-range codes give `None`
    /// (matching how the Pascal case statements default).
    pub fn from_code(code: i32) -> LineUnits {
        match code {
            1 => LineUnits::Miles,
            2 => LineUnits::Kft,
            3 => LineUnits::Km,
            4 => LineUnits::Meter,
            5 => LineUnits::Ft,
            6 => LineUnits::Inch,
            7 => LineUnits::Cm,
            8 => LineUnits::Mm,
            _ => LineUnits::None,
        }
    }

    /// Parse a unit string the way `GetUnitsCode` does: only the first two
    /// characters are examined, case-insensitively, and unknown strings give
    /// `None`.
    pub fn parse(s: &str) -> LineUnits {
        let stest: String = s.chars().take(2).collect::<String>().to_lowercase();
        match stest.as_str() {
            "no" => LineUnits::None,
            "mi" => LineUnits::Miles,
            "kf" => LineUnits::Kft,
            "km" => LineUnits::Km,
            "m" | "me" => LineUnits::Meter,
            "ft" => LineUnits::Ft,
            "in" => LineUnits::Inch,
            "cm" => LineUnits::Cm,
            "mm" => LineUnits::Mm,
            _ => LineUnits::None,
        }
    }

    /// Canonical short name (Pascal `LineUnitsStr`).
    pub fn as_str(self) -> &'static str {
        match self {
            LineUnits::None => "none",
            LineUnits::Miles => "mi",
            LineUnits::Kft => "kft",
            LineUnits::Km => "km",
            LineUnits::Meter => "m",
            LineUnits::Ft => "ft",
            LineUnits::Inch => "in",
            LineUnits::Cm => "cm",
            LineUnits::Mm => "mm",
        }
    }

    /// Meters per one of this unit (Pascal `To_Meters`); 1.0 for `None`.
    pub fn to_meters(self) -> f64 {
        match self {
            LineUnits::Miles => 1609.344,
            LineUnits::Kft => 304.8,
            LineUnits::Km => 1000.0,
            LineUnits::Meter => 1.0,
            LineUnits::Ft => 0.3048,
            LineUnits::Inch => 0.0254,
            LineUnits::Cm => 0.01,
            LineUnits::Mm => 0.001,
            LineUnits::None => 1.0,
        }
    }

    /// Pascal `To_per_Meter`.
    pub fn to_per_meter(self) -> f64 {
        1.0 / self.to_meters()
    }

    /// Pascal `From_per_Meter`.
    pub fn from_per_meter(self) -> f64 {
        self.to_meters()
    }

    /// Pascal `From_Meters`.
    pub fn from_meters(self) -> f64 {
        1.0 / self.to_meters()
    }
}

impl std::fmt::Display for LineUnits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Multiplier converting a quantity *per* `from` units into *per* `to` units
/// (Pascal `ConvertLineUnits`); 1.0 when either side is `None`.
pub fn convert_line_units(from: LineUnits, to: LineUnits) -> f64 {
    if from == LineUnits::None || to == LineUnits::None {
        1.0
    } else {
        to.from_meters() * from.to_meters()
    }
}

#[cfg(test)]
mod tests {
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
}

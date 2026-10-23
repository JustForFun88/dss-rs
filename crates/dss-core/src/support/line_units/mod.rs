//! Length-unit codes for line data, port of `Shared/LineUnits.pas`.

#[cfg(test)]
mod tests;

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

/// The words of a position or height unit: LineGeometry and LineSpacing
/// `units=`, Line `HeightUnit`.
pub const POSITION_WORDS: &[&str] = &["mi", "kft", "km", "m", "ft", "in", "cm", "mm"];

/// [`POSITION_WORDS`] as a refusal lists them.
pub const POSITION_WORD_LIST: &str = "mi, kft, km, m, ft, in, cm, mm";

/// The words of a length unit that may be `none`: Line `units=` and the
/// `RUnits`, `GMRUnits` and `RadUnits` of WireData, CNData and TSData.
pub const LENGTH_WORDS: &[&str] = &["none", "mi", "kft", "km", "m", "ft", "in", "cm", "mm"];

/// The words of LineCode `units=` and of `Show LineConstants`, where `me`
/// names metres too.
pub const CODE_WORDS: &[&str] = &["none", "mi", "kft", "km", "m", "me", "ft", "in", "cm", "mm"];

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

    /// The unit `text` names when it is one of `words`, written whole in any
    /// case (surrounding blanks ignored). `me` reads as metres. Any other text
    /// names no unit.
    pub fn from_word(text: &str, words: &[&str]) -> Option<LineUnits> {
        let text = text.trim();
        let word = words.iter().find(|w| w.eq_ignore_ascii_case(text))?;
        if word.eq_ignore_ascii_case("me") {
            return Some(LineUnits::Meter);
        }
        (0..UNITS_MAX_NUM)
            .map(LineUnits::from_code)
            .find(|u| u.as_str().eq_ignore_ascii_case(word))
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

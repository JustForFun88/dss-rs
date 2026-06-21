//! `DssEnum`, the single mapped-string enumeration — port of Pascal `TDSSEnum`
//! (DSSClass.pas): the string↔ordinal matching with the min/max-char
//! disambiguation window and the hybrid integer fallback.

use dss_parser::{ParserError, val_i32};

/// Pascal `DefaultValue = -9999999` means "no default" (raise instead).
const NO_DEFAULT: i32 = -9999999;

#[derive(Debug, Clone)]
pub struct DssEnum {
    pub name: &'static str,
    /// Are the main ordinals (without aliases) sequential/contiguous?
    pub sequential: bool,
    pub min_ordinal: i32,
    pub max_ordinal: i32,
    /// Minimum/maximum number of chars required to disambiguate strings.
    pub min_chars: usize,
    pub max_chars: usize,
    pub names: Vec<&'static str>,
    lower_names: Vec<String>,
    pub ordinals: Vec<i32>,
    pub default_value: i32,
    pub use_first_found: bool,
    pub allow_longer: bool,
    pub try_exact_first: bool,
    pub hybrid: bool,
    pub hybrid_min: i32,
}

impl DssEnum {
    pub fn new(
        name: &'static str,
        sequential: bool,
        min_chars: usize,
        max_chars: usize,
        names: &[&'static str],
        ordinals: &[i32],
    ) -> Self {
        assert_eq!(
            names.len(),
            ordinals.len(),
            "Could not initialize enum (\"{name}\")."
        );
        let lower_names = names.iter().map(|n| n.to_lowercase()).collect();
        let min_ordinal = ordinals.iter().copied().min().unwrap_or(9999999);
        let max_ordinal = ordinals.iter().copied().max().unwrap_or(-9999999);
        Self {
            name,
            sequential,
            min_ordinal,
            max_ordinal,
            min_chars,
            max_chars,
            names: names.to_vec(),
            lower_names,
            ordinals: ordinals.to_vec(),
            default_value: NO_DEFAULT,
            use_first_found: false,
            allow_longer: false,
            try_exact_first: false,
            hybrid: false,
            hybrid_min: 1,
        }
    }

    /// Pascal `OrdinalToString`: out-of-range and unmapped ordinals render
    /// as the number itself for hybrid enums, empty otherwise.
    pub fn ordinal_to_string(&self, value: i32) -> String {
        if value < self.min_ordinal || value > self.max_ordinal {
            return if self.hybrid {
                value.to_string()
            } else {
                String::new()
            };
        }
        if self.sequential {
            return self
                .names
                .get((value - self.min_ordinal) as usize)
                .map(|s| s.to_string())
                .unwrap_or_default();
        }
        for (i, &ord) in self.ordinals.iter().enumerate() {
            if ord == value {
                return self.names[i].to_string();
            }
        }
        if self.hybrid {
            value.to_string()
        } else {
            String::new()
        }
    }

    /// Pascal `IsOrdinalValid`.
    pub fn is_ordinal_valid(&self, value: i32) -> bool {
        (self.hybrid && value >= self.hybrid_min) || self.ordinals.contains(&value)
    }

    /// Pascal `Joined`: `[name1,name2,...]`.
    pub fn joined(&self) -> String {
        format!("[{}]", self.names.join(","))
    }

    /// Pascal `StringToOrdinal`. Match `value` (callers lowercase it where
    /// the Pascal engine did) against the names using prefix windows from
    /// `min_chars` to `max_chars`; ambiguous prefixes keep widening until a
    /// unique match is found. Errors map to the exceptions the original
    /// raised (caught by `ProcessCommand`).
    pub fn string_to_ordinal(&self, value: &str) -> Result<i32, ParserError> {
        let vbytes = value.as_bytes();

        if self.min_chars != 0 && self.min_chars > vbytes.len() {
            if self.hybrid {
                return self.hybrid_value(value);
            }
            if self.try_exact_first {
                // case insensitive (Pascal AnsiIndexText)
                for (i, name) in self.names.iter().enumerate() {
                    if name.eq_ignore_ascii_case(value) {
                        return Ok(self.ordinals[i]);
                    }
                }
            }
            if self.default_value == NO_DEFAULT {
                return Err(self.no_match_error(value));
            }
            return Ok(self.default_value);
        }

        let minch = self.min_chars.max(1);
        let mut result = 0;
        for nch in minch..=vbytes.len().min(self.max_chars) {
            let mut found = 0u32;
            let s = &vbytes[..nch];
            for (i, lower) in self.lower_names.iter().enumerate() {
                if !self.allow_longer && lower.len() < vbytes.len() {
                    continue;
                }
                if nch == minch && value == lower {
                    return Ok(self.ordinals[i]);
                }
                let lbytes = lower.as_bytes();
                let n = s.len().min(lbytes.len());
                if s[..n].eq_ignore_ascii_case(&lbytes[..n]) {
                    result = self.ordinals[i];
                    if nch == vbytes.len() && self.use_first_found {
                        return Ok(result);
                    }
                    found += 1;
                    if found > 1 {
                        break;
                    }
                }
            }
            if found == 1 {
                return Ok(result); // found the match, can exit safely
            }
        }

        if self.hybrid {
            return self.hybrid_value(value);
        }
        if self.default_value == NO_DEFAULT {
            return Err(self.no_match_error(value));
        }
        Ok(self.default_value)
    }

    /// The hybrid fallback: FPC `Val` as integer, clamped to `hybrid_min`
    /// from below (Pascal raises `EParserProblem` on conversion failure).
    fn hybrid_value(&self, value: &str) -> Result<i32, ParserError> {
        match val_i32(value) {
            Some(v) => Ok(v.max(self.hybrid_min)),
            None => Err(ParserError::new(format!(
                "Integer number conversion error for string: \"{value}\""
            ))),
        }
    }

    fn no_match_error(&self, value: &str) -> ParserError {
        ParserError::new(format!(
            "Could not match enum (\"{}\") value \"{}\"",
            self.name, value
        ))
    }
}

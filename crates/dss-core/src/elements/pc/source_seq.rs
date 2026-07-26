//! The two shared source phase-rotation selectors — `ScanTypeEnum` and
//! `SequenceEnum` (`DSSClass.pas:1087-1091`).
//!
//! Three source classes carried their own private copy of the same pair of
//! bare `i32` fields: VSource (`Vsource.pas:132-133`), Isource
//! (`Isource.pas:89-90`) and GICLine (`GICLine.pas:109-110`). VSource and
//! Isource expose them as `ScanType=` / `Sequence=` `MappedStringEnum`
//! properties pointing at the **same two registry entries**
//! (`Vsource.pas:252-257`, `Isource.pas:173-178`); GICLine keeps them
//! property-less (its `Create` pins both to zero-sequence, `GICLine.pas:390-391`
//! — "Always 0 for GIC") but runs the identical `case` in `GetVterminalForSource`.
//!
//! Both registry lists are closed 3-value sets with no `DefaultValue`, so an
//! unmatched token raises rather than writing: `from_ordinal(v).unwrap_or(field)`
//! at the property seam keeps the previous value on a path the parser cannot
//! reach (the wave's established setter shape).

/// Pascal `DSS.ScanTypeEnum` (`DSSClass.pas:1087`) — how a harmonic source
/// rotates each phase after the first (`ScanType=`).
///
/// Registry `'Scan Type'`, names `['None', 'Zero', 'Positive']`, values
/// `[-1, 0, 1]`. The Pascal `case ScanType of` (`Vsource.pas:1058-1064`,
/// `Isource.pas:479-487`, `GICLine.pas:546`) names `1` and `0` and routes
/// [`Self::None`] through the `else` arm — the "normal" rotation at the source
/// harmonic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanType {
    /// `-1` — no sequence maintained: rotate at the source harmonic (the
    /// Pascal `else` arm).
    None = -1,
    /// `0` — zero sequence: all phases identical, no rotation.
    Zero = 0,
    /// `1` — positive sequence maintained (rotate at harmonic 1).
    Positive = 1,
}

impl ScanType {
    /// The `Scan Type` `DssEnum` ordinal (the `ScanType=` property value).
    pub fn ordinal(self) -> i32 {
        self as i32
    }

    /// From a property/registry ordinal; `None` outside `[-1, 0, 1]`.
    pub fn from_ordinal(value: i32) -> Option<Self> {
        match value {
            -1 => Some(Self::None),
            0 => Some(Self::Zero),
            1 => Some(Self::Positive),
            _ => Option::None,
        }
    }
}

/// Pascal `DSS.SequenceEnum` (`DSSClass.pas:1090`) — the sequence a source
/// synthesizes at the fundamental (`Sequence=`).
///
/// Registry `'Sequence Type'`, names `['Negative', 'Zero', 'Positive']`, values
/// `[-1, 0, 1]`. The Pascal `case Sequencetype of` (`Vsource.pas:1076-1083`,
/// `Isource.pas:489-497`, `GICLine.pas:563`) names `-1` and `0` and routes
/// [`Self::Positive`] through the `else` arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceType {
    /// `-1` — negative sequence (phase angle *advances* with the phase index).
    Negative = -1,
    /// `0` — zero sequence: the same angle on every phase.
    Zero = 0,
    /// `1` — positive sequence (the Pascal `else` arm).
    Positive = 1,
}

impl SequenceType {
    /// The `Sequence Type` `DssEnum` ordinal (the `Sequence=` property value).
    pub fn ordinal(self) -> i32 {
        self as i32
    }

    /// From a property/registry ordinal; `None` outside `[-1, 0, 1]`.
    pub fn from_ordinal(value: i32) -> Option<Self> {
        match value {
            -1 => Some(Self::Negative),
            0 => Some(Self::Zero),
            1 => Some(Self::Positive),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ScanType, SequenceType};

    /// `DSSClass.pas:1087` / `:1090` — the two value lists, plus the `Create`
    /// seeds of the three classes that speak them (`Vsource.pas:646-647` = 1/1,
    /// `Isource.pas:321-322` = 1/1, `GICLine.pas:390-391` = 0/0).
    #[test]
    fn scan_and_sequence_type_pin_pascal_ordinals() {
        assert_eq!(ScanType::None.ordinal(), -1);
        assert_eq!(ScanType::Zero.ordinal(), 0);
        assert_eq!(ScanType::Positive.ordinal(), 1);
        assert_eq!(SequenceType::Negative.ordinal(), -1);
        assert_eq!(SequenceType::Zero.ordinal(), 0);
        assert_eq!(SequenceType::Positive.ordinal(), 1);

        for s in [ScanType::None, ScanType::Zero, ScanType::Positive] {
            assert_eq!(ScanType::from_ordinal(s.ordinal()), Some(s));
        }
        for s in [
            SequenceType::Negative,
            SequenceType::Zero,
            SequenceType::Positive,
        ] {
            assert_eq!(SequenceType::from_ordinal(s.ordinal()), Some(s));
        }
        // Both registry entries are closed and carry NO default value, so an
        // out-of-set ordinal can never be written (the setters keep the field).
        for v in [i32::MIN, -3, -2, 2, 3, i32::MAX] {
            assert_eq!(ScanType::from_ordinal(v), None, "scan ordinal {v}");
            assert_eq!(SequenceType::from_ordinal(v), None, "seq ordinal {v}");
        }
    }
}

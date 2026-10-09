//! [`Rating`]: a current or power rating, or none.
//!
//! A rating that was never given is its own state, not a number. No loading,
//! overload, capacity or report computation reads a value from it: every reader
//! matches [`Rating::NotSet`] first and treats the element as unrated.

/// A current or power rating, or none.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rating {
    /// No rating: nothing is measured against it.
    NotSet,
    /// A rating in the property's own unit (amperes or kVA).
    Set(f64),
}

impl Rating {
    /// The text a rating that is not set reads back as, and the input token
    /// that sets it (ASCII case-insensitive).
    pub const NONE_TOKEN: &'static str = "none";

    /// A rating from a number, typed, read from a file or derived: `-1` is the
    /// input token of a rating that is not set, so it means not set, and every
    /// other number is that rating. The one rating built from a number without
    /// it is the current rating a Transformer or an AutoTrans derives from its
    /// kVA rating, which has no not-set state: that rating is always set.
    pub fn from_number(v: f64) -> Self {
        if v == -1.0 {
            Rating::NotSet
        } else {
            Rating::Set(v)
        }
    }

    /// Whether `token`, without the whitespace around it, is the
    /// [`Self::NONE_TOKEN`] spelling, as a number may carry that whitespace too.
    pub fn is_none_token(token: &str) -> bool {
        token.trim().eq_ignore_ascii_case(Self::NONE_TOKEN)
    }

    /// The rating value, `None` when not set.
    pub fn if_set(self) -> Option<f64> {
        match self {
            Rating::Set(v) => Some(v),
            Rating::NotSet => None,
        }
    }

    /// Whether a rating is set.
    pub fn is_set(self) -> bool {
        matches!(self, Rating::Set(_))
    }

    /// A rating derived from this one through [`Self::from_number`]: not set
    /// stays not set.
    pub fn map(self, f: impl FnOnce(f64) -> f64) -> Self {
        match self {
            Rating::Set(v) => Rating::from_number(f(v)),
            Rating::NotSet => Rating::NotSet,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Rating;

    #[test]
    fn minus_one_is_not_set_and_every_other_number_is_a_rating() {
        assert_eq!(Rating::from_number(-1.0), Rating::NotSet);
        assert_eq!(Rating::from_number(-2.0), Rating::Set(-2.0));
        assert_eq!(Rating::from_number(0.0), Rating::Set(0.0));
        assert_eq!(Rating::from_number(530.0), Rating::Set(530.0));
        assert!(Rating::is_none_token("NONE"));
        assert!(Rating::is_none_token(" none\t"));
        assert!(!Rating::is_none_token("-1"));
        assert!(!Rating::is_none_token("no ne"));
        assert_eq!(Rating::NotSet.map(|v| v * 1.5), Rating::NotSet);
        assert_eq!(Rating::Set(2.0).map(|v| v * 1.5), Rating::Set(3.0));
        assert_eq!(
            Rating::Set(-1.25).map(|v| 0.8 * v),
            Rating::NotSet,
            "a derivation that lands on -1 is not set"
        );
        assert_eq!(Rating::Set(-2.0).map(|v| 0.8 * v), Rating::Set(-1.6));
        assert_eq!(Rating::NotSet.if_set(), None);
        assert!(!Rating::NotSet.is_set());
    }
}

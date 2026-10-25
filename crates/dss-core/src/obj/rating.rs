//! [`Rating`]: a current or power rating, or none.
//!
//! A rating that was never given is its own state, not a number. No loading,
//! overload, capacity or report computation reads a value from it: every reader
//! matches [`Rating::NotSet`] first and treats the element as unrated.
//!
//! A typed rating is set only when it is positive: a typed `0` or negative
//! number is no rating and reads not set. A negative set rating comes only from
//! a derivation (a Capacitor of negative kvar, a Transformer below its other
//! winding) and a loading measures against its magnitude
//! ([`Rating::magnitude`]).

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

    /// A rating from a typed number, a number read from a file or a JSON
    /// import, or the `Set %Normal` result: a positive number is that rating,
    /// and `0`, `-1` and every other negative number name no rating, so they
    /// read not set. Each route refuses a number that is not finite before it
    /// gets here.
    pub fn from_number(v: f64) -> Self {
        debug_assert!(v.is_finite(), "a rating number is finite: {v}");
        if v > 0.0 {
            Rating::Set(v)
        } else {
            Rating::NotSet
        }
    }

    /// A rating an element derives from its other data: `0` (a device of no
    /// size) and `-1` (the input token of a rating that is not set) read not
    /// set, and every other finite number is that rating, a negative one
    /// included, as the current of a device of negative size flows all the
    /// same. A number that is not finite is no rating: the caller refuses the
    /// input that gave it. The one derived rating built without it is the
    /// current rating a Transformer or an AutoTrans derives from its kVA
    /// rating, which is always set until the kVA ratings have a not-set state.
    pub fn derived(v: f64) -> Result<Self, NotFinite> {
        if !v.is_finite() {
            Err(NotFinite)
        } else if v == 0.0 || v == -1.0 {
            Ok(Rating::NotSet)
        } else {
            Ok(Rating::Set(v))
        }
    }

    /// The rating a loading is measured against: a set rating's magnitude,
    /// and not set stays not set.
    pub fn magnitude(self) -> Self {
        match self {
            Rating::Set(v) => Rating::Set(v.abs()),
            Rating::NotSet => Rating::NotSet,
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

    /// A rating derived from this one through [`Self::derived`]: not set stays
    /// not set.
    pub fn map(self, f: impl FnOnce(f64) -> f64) -> Result<Self, NotFinite> {
        match self {
            Rating::Set(v) => Rating::derived(f(v)),
            Rating::NotSet => Ok(Rating::NotSet),
        }
    }
}

/// A derived rating that is not a finite number (`NaN` or an infinity).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotFinite;

/// A rating property as the user typed it, which `Save` writes back. `none`
/// and `-1` are both kept as `-1`, and so is an array entry typed `none` or
/// left empty. The typed number is kept whatever it reads as, `0` and a
/// negative number included.
#[derive(Debug, Clone, PartialEq)]
pub enum TypedRating {
    /// A [`PropType::Rating`](crate::obj::props::PropType::Rating) property.
    Scalar(f64),
    /// A [`PropType::RatingArray`](crate::obj::props::PropType::RatingArray)
    /// property, one entry per season read.
    Array(Vec<f64>),
}

impl TypedRating {
    /// The number a typed `none` is kept as.
    pub const NONE: f64 = -1.0;
}

#[cfg(test)]
mod tests {
    use super::{NotFinite, Rating};

    #[test]
    fn only_a_positive_typed_number_is_a_rating() {
        assert_eq!(Rating::from_number(-1.0), Rating::NotSet);
        assert_eq!(Rating::from_number(-2.0), Rating::NotSet);
        assert_eq!(Rating::from_number(0.0), Rating::NotSet);
        assert_eq!(Rating::from_number(-0.0), Rating::NotSet);
        assert_eq!(Rating::from_number(530.0), Rating::Set(530.0));
        assert_eq!(Rating::from_number(1e-300), Rating::Set(1e-300));
        assert!(Rating::is_none_token("NONE"));
        assert!(Rating::is_none_token(" none\t"));
        assert!(!Rating::is_none_token("-1"));
        assert!(!Rating::is_none_token("no ne"));
        assert_eq!(Rating::NotSet.if_set(), None);
        assert!(!Rating::NotSet.is_set());
    }

    #[test]
    fn a_derived_rating_is_not_set_only_at_zero_and_minus_one() {
        assert_eq!(Rating::derived(0.0), Ok(Rating::NotSet));
        assert_eq!(Rating::derived(-0.0), Ok(Rating::NotSet));
        assert_eq!(Rating::derived(-1.0), Ok(Rating::NotSet));
        assert_eq!(Rating::derived(-2.0), Ok(Rating::Set(-2.0)));
        assert_eq!(Rating::derived(530.0), Ok(Rating::Set(530.0)));
        assert_eq!(Rating::Set(-2.0).magnitude(), Rating::Set(2.0));
        assert_eq!(
            Rating::Set(0.0).magnitude(),
            Rating::Set(0.0),
            "a magnitude is built directly, not derived"
        );
        assert_eq!(Rating::NotSet.magnitude(), Rating::NotSet);
        assert_eq!(Rating::NotSet.map(|v| v * 1.5), Ok(Rating::NotSet));
        assert_eq!(Rating::Set(2.0).map(|v| v * 1.5), Ok(Rating::Set(3.0)));
        assert_eq!(
            Rating::Set(-1.25).map(|v| 0.8 * v),
            Ok(Rating::NotSet),
            "a derivation that lands on -1 is not set"
        );
        assert_eq!(
            Rating::Set(2.0).map(|v| v * 0.0),
            Ok(Rating::NotSet),
            "a derivation that lands on 0 is not set"
        );
        assert_eq!(Rating::Set(-2.0).map(|v| 0.8 * v), Ok(Rating::Set(-1.6)));
    }

    /// A derived number that is not finite is no rating: `Rating::derived` of
    /// `NaN`, `+Inf` and `-Inf` is an `Err` (so no caller can keep it), as is a
    /// [`Rating::map`] that overflows, while not set stays not set. The typed
    /// routes refuse such a number before [`Rating::from_number`], whose debug
    /// assertion fires on each.
    #[test]
    fn a_derived_number_that_is_not_finite_is_refused() {
        for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(Rating::derived(v), Err(NotFinite), "{v}");
            assert_eq!(Rating::Set(1.0).map(|_| v), Err(NotFinite), "{v}");
            assert_eq!(Rating::NotSet.map(|_| v), Ok(Rating::NotSet), "{v}");
            if cfg!(debug_assertions) {
                let fired = std::panic::catch_unwind(|| Rating::from_number(v)).is_err();
                assert!(fired, "the debug assertion of from_number fires on {v}");
            }
        }
        assert_eq!(Rating::Set(1.7e308).map(|v| 1.5 * v), Err(NotFinite));
        assert_eq!(Rating::derived(f64::MAX), Ok(Rating::Set(f64::MAX)));
    }
}

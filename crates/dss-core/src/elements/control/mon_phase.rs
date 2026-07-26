//! The shared monitored-phase selector (`MonPhaseEnum`, `DSSClass.pas:1184`).
//!
//! Four control classes carried their own private copy of the same sentinel
//! triple — CapControl (`FCTphase`/`FPTphase`, `CapControl.pas:230-232`),
//! RegControl (`FPTphase`, min/max only), InvControl (`FMonBusesPhase`) and
//! StorageController (`FMonPhase`, `StorageController.pas:38-40`). All four
//! resolve to the *same* `MonPhaseEnum` in the property registry
//! (`obj/dss_enum/registry/control.rs`, `hybrid = true`, values `[-3, -2, -1]`
//! for `min`/`max`/`avg`), and everything outside that set is a 1-based phase
//! number — which is exactly what `hybrid` means: an unmatched token is parsed
//! as an integer and stored verbatim.
//!
//! [`MonPhase`] therefore is a *hybrid* enum: three named sentinels plus a
//! [`MonPhase::Phase`] payload carrying the raw ordinal. `ordinal()` /
//! [`MonPhase::from_ordinal`] are total and mutually inverse over the whole
//! `i32` range, so the property boundary round-trips byte-for-byte exactly as
//! the pre-enum bare `i32` field did.

/// Pascal `MonPhaseEnum` (`DSSClass.pas:1184`) — the monitored-phase selector
/// shared by CapControl / RegControl / InvControl / StorageController.
///
/// Sentinels are pinned to the Pascal constants (`CapControl.pas:230-232`
/// `AVGPHASES = -1`, `MAXPHASE = -2`, `MINPHASE = -3`; the identical triple at
/// `StorageController.pas:38-40`) and to the registry's `[-3, -2, -1]` values.
/// Any other ordinal is a 1-based phase number ([`MonPhase::Phase`]) — the
/// registry entry is `hybrid`, so the parse falls back to an integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonPhase {
    /// `AVGPHASES = -1` — average over the monitored phases.
    Avg,
    /// `MAXPHASE = -2` — the maximum-magnitude phase.
    Max,
    /// `MINPHASE = -3` — the minimum-magnitude phase.
    Min,
    /// Any non-sentinel ordinal: a 1-based phase number, stored verbatim (the
    /// hybrid-enum integer fallback). Includes ordinals no parse can produce
    /// (`0`, `-4`, …) so the round-trip stays total.
    Phase(i32),
}

impl MonPhase {
    /// The `MonPhaseEnum` ordinal — the value the property `get_i32`/dump
    /// boundary reports (identical to the pre-enum raw field).
    pub fn ordinal(self) -> i32 {
        match self {
            Self::Avg => -1,
            Self::Max => -2,
            Self::Min => -3,
            Self::Phase(n) => n,
        }
    }

    /// From a property/registry ordinal. Total: every non-sentinel value lands
    /// in [`MonPhase::Phase`], so `from_ordinal(x).ordinal() == x` for all `x`.
    pub fn from_ordinal(value: i32) -> Self {
        match value {
            -1 => Self::Avg,
            -2 => Self::Max,
            -3 => Self::Min,
            n => Self::Phase(n),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MonPhase;

    /// `CapControl.pas:230-232` / `StorageController.pas:38-40` + the
    /// `Monitored Phase` `DssEnum` (`registry/control.rs`, values `[-3,-2,-1]`).
    #[test]
    fn mon_phase_pins_enum_ordinals() {
        assert_eq!(MonPhase::Avg.ordinal(), -1);
        assert_eq!(MonPhase::Max.ordinal(), -2);
        assert_eq!(MonPhase::Min.ordinal(), -3);
        for m in [MonPhase::Avg, MonPhase::Max, MonPhase::Min] {
            assert_eq!(MonPhase::from_ordinal(m.ordinal()), m);
        }
    }

    /// The hybrid fallback must be a *total* round-trip: no ordinal is lost or
    /// remapped, exactly like the pre-enum bare `i32` field.
    #[test]
    fn mon_phase_round_trips_every_non_sentinel_ordinal() {
        for v in [-1000, -5, -4, 0, 1, 2, 3, 4, 12, 1000] {
            let m = MonPhase::from_ordinal(v);
            assert_eq!(m, MonPhase::Phase(v), "{v} must be a phase number");
            assert_eq!(m.ordinal(), v);
        }
        // …and the sentinels are the only three that are *not* `Phase`.
        for v in -3..=-1 {
            assert!(!matches!(MonPhase::from_ordinal(v), MonPhase::Phase(_)));
        }
    }
}

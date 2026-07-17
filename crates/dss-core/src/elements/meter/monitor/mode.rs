//! Typed decode of the monitor `mode` bitfield (Pascal `TMonitorObj.Mode`).
//!
//! The `mode` integer packs a **multi-bit** base-mode subfield in the low four
//! bits (`Mode and MODEMASK`, values 0..=12) plus three independent modifier
//! flags (`SEQUENCEMASK=16`, `MAGNITUDEMASK=32`, `POSSEQONLYMASK=64` —
//! `Meters/Monitor.pas:257-260`). The raw `i32` lives only at the property
//! parse/report boundary (`mode=` in via [`MonitorModeView::from_raw`], the
//! ordinal out via [`MonitorModeView::to_raw`]); everywhere else the engine
//! matches on this decoded view. `bitflags` is not a fit — the low four bits
//! are a subfield, not independent flags.

/// Pascal `Mode and MODEMASK` — the base recording mode (`Meters/Monitor.pas`
/// `TakeSample`/`ClearMonitorStream` `case` selectors).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MonitorBaseMode {
    /// 0 — terminal voltages and currents.
    VoltageAndCurrent = 0,
    /// 1 — complex powers (kW/kvar).
    Power = 1,
    /// 2 — transformer tap.
    Tap = 2,
    /// 3 — PC-element state variables.
    StateVars = 3,
    /// 4 — flicker / Pst.
    Flicker = 4,
    /// 5 — solution variables.
    SolutionVars = 5,
    /// 6 — capacitor switching state.
    CapacitorSteps = 6,
    /// 7 — Storage device state.
    Storage = 7,
    /// 8 — transformer winding currents.
    TransformerWindingCurrents = 8,
    /// 9 — losses.
    Losses = 9,
    /// 10 — transformer winding voltages.
    TransformerWindingVoltages = 10,
    /// 11 — all terminal voltages and currents.
    AllTerminalVI = 11,
    /// 12 — line-to-line voltages.
    LineToLineVoltages = 12,
    /// 13/14/15 — undefined base subfield. No ported mode uses them; Pascal's
    /// `case (Mode and MODEMASK) of … else` routes them through the general
    /// (mode-0-style) header in `ClearMonitorStream` but through the bare
    /// `else Exit` in `TakeSample` — i.e. a *timestamp-only* row, NOT a mode-0
    /// V/I record. This distinct variant makes `sample.rs`'s catch-all arm route
    /// them to that timestamp-only `Exit` (mapping them onto `VoltageAndCurrent`
    /// would wrongly write a full V/I sample). The exact ordinal is preserved by
    /// [`MonitorModeView::raw`], not here.
    Undefined,
}

impl MonitorBaseMode {
    /// Decode the four-bit base subfield. Values 13/14/15 map to
    /// [`Self::Undefined`] (see its doc); the header path treats them like the
    /// general V/I header (Pascal `else`) while the sample path emits only the
    /// timestamp.
    fn from_bits(base: i32) -> Self {
        use MonitorBaseMode::*;
        match base {
            0 => VoltageAndCurrent,
            1 => Power,
            2 => Tap,
            3 => StateVars,
            4 => Flicker,
            5 => SolutionVars,
            6 => CapacitorSteps,
            7 => Storage,
            8 => TransformerWindingCurrents,
            9 => Losses,
            10 => TransformerWindingVoltages,
            11 => AllTerminalVI,
            12 => LineToLineVoltages,
            _ => Undefined,
        }
    }
}

/// Decoded `TMonitorObj.Mode`: the base mode plus the three modifier flags,
/// alongside the verbatim ordinal it was decoded from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MonitorModeView {
    /// The raw `mode=` ordinal, kept verbatim (Pascal stores `Mode: Integer`).
    /// The engine matches on the decoded subfields below, but the property
    /// read-back (`Get_Mode`) and the `SampleAllMode5` full-ordinal `Mode = 5`
    /// split read the *whole* integer — so the decode must be lossless. Bits the
    /// subfield decode drops (junk ≥128, and the exact undefined base ordinal
    /// 13/14/15) survive here, matching Pascal 1:1.
    raw: i32,
    pub base: MonitorBaseMode,
    /// `Mode and SEQUENCEMASK` — record symmetrical components.
    pub sequence: bool,
    /// `Mode and MAGNITUDEMASK` — record magnitudes only (drop the angle).
    pub magnitude: bool,
    /// `Mode and POSSEQONLYMASK` — record the positive-sequence value only.
    pub posseq_only: bool,
}

// The bit masks, pinned here (and nowhere else) per DE_PASCALIZE P2.
const MODEMASK: i32 = 15;
const SEQUENCEMASK: i32 = 16;
const MAGNITUDEMASK: i32 = 32;
const POSSEQONLYMASK: i32 = 64;

impl MonitorModeView {
    /// Decode a raw `mode=` integer (Pascal `Mode and <MASK>`), retaining the
    /// ordinal verbatim for the property/report boundary.
    pub fn from_raw(mode: i32) -> Self {
        Self {
            raw: mode,
            base: MonitorBaseMode::from_bits(mode & MODEMASK),
            sequence: (mode & SEQUENCEMASK) != 0,
            magnitude: (mode & MAGNITUDEMASK) != 0,
            posseq_only: (mode & POSSEQONLYMASK) != 0,
        }
    }

    /// The verbatim `mode=` ordinal for the property report boundary
    /// (Pascal `Get_Mode` returns the whole integer).
    pub fn to_raw(self) -> i32 {
        self.raw
    }
}

impl Default for MonitorModeView {
    /// Pascal `TMonitorObj.Create`: `Mode := 0` (voltage and current).
    fn default() -> Self {
        Self::from_raw(0)
    }
}

#[cfg(test)]
mod tests {
    use super::{MonitorBaseMode, MonitorModeView};

    /// `from_raw(to_raw(v)) == v` over every defined base (0..=12) crossed with
    /// all eight modifier-bit combinations — including the magic `base == 1`
    /// (Power) sites and the modifier codes seen in the corpus (32/48/65/96/112).
    #[test]
    fn round_trips_defined_encodings() {
        for base in 0..=12 {
            for mods in 0..8 {
                let raw = base
                    | if mods & 1 != 0 { 16 } else { 0 }
                    | if mods & 2 != 0 { 32 } else { 0 }
                    | if mods & 4 != 0 { 64 } else { 0 };
                let v = MonitorModeView::from_raw(raw);
                assert_eq!(v.to_raw(), raw, "round-trip failed for mode={raw}");
                assert_eq!(MonitorModeView::from_raw(v.to_raw()), v);
            }
        }
    }

    /// The base subfield decodes to the matching enum and the `Power` (base 1)
    /// discriminator survives the modifier bits.
    #[test]
    fn decodes_base_and_flags() {
        let v = MonitorModeView::from_raw(1); // plain power
        assert_eq!(v.base, MonitorBaseMode::Power);
        assert!(!v.sequence && !v.magnitude && !v.posseq_only);

        let v = MonitorModeView::from_raw(65); // 1 + POSSEQONLY
        assert_eq!(v.base, MonitorBaseMode::Power);
        assert!(v.posseq_only && !v.sequence && !v.magnitude);

        let v = MonitorModeView::from_raw(112); // 0 + SEQ + MAG + POSSEQ
        assert_eq!(v.base, MonitorBaseMode::VoltageAndCurrent);
        assert!(v.sequence && v.magnitude && v.posseq_only);
    }

    /// Undefined base values (13/14/15) decode to the distinct `Undefined`
    /// variant (routed to the timestamp-only `Exit` in the sample path) yet the
    /// raw ordinal survives verbatim through `to_raw` — Pascal keeps the whole
    /// `Mode: Integer` and its `TakeSample` `else Exit` writes no data row.
    #[test]
    fn undefined_base_preserves_raw_ordinal() {
        for raw in [13, 14, 15] {
            let v = MonitorModeView::from_raw(raw);
            assert_eq!(v.base, MonitorBaseMode::Undefined);
            assert_eq!(v.to_raw(), raw, "undefined base must round-trip");
        }
        // Modifier bits over an undefined base still survive verbatim.
        assert_eq!(MonitorModeView::from_raw(13 | 16).to_raw(), 13 | 16);
    }

    /// Junk bits above the defined mask (≥128) are NOT dropped: Pascal's
    /// `Get_Mode`/`Mode = 5` split read the whole ordinal, so `to_raw` must be
    /// lossless even though the base subfield decodes to a defined mode.
    #[test]
    fn high_junk_bits_survive_round_trip() {
        let v = MonitorModeView::from_raw(133); // 128 + base 5 (SolutionVars)
        assert_eq!(v.base, MonitorBaseMode::SolutionVars);
        assert_eq!(v.to_raw(), 133);
        // The mode-5 split keys off the full ordinal (Pascal `Mon.Mode = 5`),
        // so 133 must NOT collapse to 5.
        assert_ne!(v.to_raw(), 5);
    }
}

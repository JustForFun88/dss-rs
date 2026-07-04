//! Per-report formatters for `Show` (Pascal `Common/ShowResults.pas`). Each takes
//! a read view of the (solved) circuit and returns the report text; no formatter
//! mutates electrical state (PHASE8_PLAN §2.1). Coverage grows per WP8.4 step.
//!
//! Unlike the `Export` CSV formatters, `Show` emits Pascal's fixed-width text
//! tables (`Pad`/`PadDots` columns, `Format('%W.Df', …)` widths). The targeted
//! text golden (`golden_phase8.rs`) diffs them after tokenizing on whitespace +
//! commas (PHASE8_PLAN §2.3), so the exact padding is not gate-load-bearing — but
//! the field structure and the number formats are ported faithfully.

mod buses;
mod currents;
mod elements;
mod losses;
mod powers;
mod taps;
mod voltages;

pub(crate) use buses::show_buses;
pub(crate) use currents::{show_currents, show_currents_elements};
pub(crate) use elements::show_elements;
pub(crate) use losses::show_losses;
pub(crate) use powers::show_powers;
pub(crate) use taps::show_taps;
pub(crate) use voltages::{show_voltages, show_voltages_elements, show_voltages_nodes};

use crate::circuit::Circuit;
use crate::exec::registry::DssClass;

/// Pascal `SetMaxBusNameLength` (`ShowResults.pas:101`): the longest bus name.
/// Walks `BusList.NameOfIndex` (the lowercased stored names). Uses **byte**
/// length (`str::len`), matching Pascal's `Length(AnsiString)` (byte-1:1;
/// identical to char count for the ASCII corpus names).
///
/// TODO(compat): the floor is **12**, not the source's 4. The vendored Pascal
/// `SetMaxBusNameLength` resets `MaxBusNameLength := 4` before the max-loop, but
/// the pinned dss_capi 0.14.5 backend floors it at the unit-init constant
/// `MaxBusNameLength := 12` (`ShowResults.pas:3979`) — a backend-vs-source
/// divergence probe-proven `max(12, longest_bus_name)`: a 20-char bus name widens
/// to 20, a 5-char one floors at 12 (not 5), and IEEE13 (longest `sourcebus` = 9)
/// pads to 12. Reproduced 1:1 (settled empirically per CLAUDE.md — the oracle is
/// the spec), same class as [`max_device_name_length`]'s `= 0`. Matters wherever
/// the width feeds a **dot-padded** (`PadDots`) column — `WriteBusVoltages` splits
/// the bus name and its dot run into two whitespace tokens, so a wrong floor drops
/// the dots token on names ≤ 12 (the space-padded reports are token-invariant to
/// it). Clean fix in the post-1:1 pass: floor at 4 (and regenerate goldens).
pub(crate) fn max_bus_name_length(ckt: &Circuit) -> usize {
    let mut m = 12;
    for i in 0..ckt.buses.len() {
        if let Some(n) = ckt.bus_list.name(i) {
            m = m.max(n.len());
        }
    }
    m
}

/// Pascal `SetMaxDeviceNameLength` (`ShowResults.pas:111`): nominally the longest
/// `len(Name) + len(ParentClass.Name) + 1` over the `CktElements` master list.
///
/// TODO(compat): the **pinned dss_capi 0.14.5 backend** empirically returns **0**
/// here regardless of the element names — the device-name column in every
/// `Show Currents`/`Powers`/`Losses`/… report is left **unpadded** (the
/// `Paddots`/`Pad(…, MaxDeviceNameLength+2)` never fires, since a 2-char width is
/// below every `EncloseQuotes(name)`). Proven by probe: adding a 25-char-named
/// line to a deck does not widen the column, and the per-row terminal number sits
/// immediately after each (variable-length) name, not on a fixed column. The
/// vendored Pascal *source* would compute e.g. 16 on IEEE13, so this is a
/// backend-vs-source divergence reproduced 1:1 to match the oracle (settled
/// empirically per CLAUDE.md — "the oracle is the spec"). Matters only for the
/// dot-padded (`Paddots`) reports, where a nonzero width would split the name into
/// two whitespace tokens; the space-padded (`Pad`) reports are token-invariant to
/// it. Clean fix in the post-1:1 pass: compute the real max (and regenerate the
/// goldens against a fixed upstream).
pub(crate) fn max_device_name_length(_classes: &[DssClass], _ckt: &Circuit) -> usize {
    0
}

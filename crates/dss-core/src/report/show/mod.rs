//! Per-report formatters for `Show` (Pascal `Common/ShowResults.pas`). Each takes
//! a read view of the (solved) circuit and returns the report text; no formatter
//! mutates electrical state (PHASE8_PLAN §2.1). Coverage grows per WP8.4 step.
//!
//! Unlike the `Export` CSV formatters, `Show` emits Pascal's fixed-width text
//! tables (`Pad`/`PadDots` columns, `Format('%W.Df', …)` widths). The targeted
//! text golden (`golden_reports.rs`) diffs them after tokenizing on whitespace +
//! commas (PHASE8_PLAN §2.3), so the exact padding is not gate-load-bearing — but
//! the field structure and the number formats are ported faithfully.

mod bus_powers;
mod buses;
mod controlled;
mod currents;
mod delta_v;
mod diagnostics;
mod elements;
mod fault_study;
mod isolated;
mod line_constants;
mod losses;
mod matrix;
mod meter_zone;
mod meters;
mod overloads;
mod powers;
mod taps;
mod topology;
mod unserved;
mod voltages;
mod yprim;

pub(crate) use bus_powers::show_bus_powers;
pub(crate) use buses::show_buses;
pub(crate) use controlled::show_controlled;
pub(crate) use currents::{show_currents, show_currents_elements};
pub(crate) use delta_v::show_delta_v;
pub(crate) use diagnostics::{
    show_control_queue, show_convergence, show_event_log, show_kvbase_mismatch, show_mismatch,
    show_ratings, show_result, show_variables,
};
pub(crate) use elements::show_elements;
pub(crate) use fault_study::show_fault_study;
pub(crate) use isolated::show_isolated;
pub(crate) use line_constants::show_line_constants;
pub(crate) use losses::show_losses;
pub(crate) use matrix::show_y;
pub(crate) use meter_zone::{show_loops, show_meter_zone};
pub(crate) use meters::{show_gen_meters, show_meters};
pub(crate) use overloads::show_overloads;
pub(crate) use powers::{show_powers, show_powers_elements};
pub(crate) use taps::show_taps;
pub(crate) use topology::show_topology;
pub(crate) use unserved::show_unserved;
pub(crate) use voltages::{show_voltages, show_voltages_elements, show_voltages_nodes};
pub(crate) use yprim::show_yprim;

use crate::circuit::Circuit;
use crate::exec::registry::DssClass;

/// Pascal `SetMaxBusNameLength` (`ShowResults.pas:101`): the longest bus name,
/// floored at 4. Walks `BusList.NameOfIndex` (the lowercased stored names). Uses
/// **byte** length (`str::len`), matching Pascal's `Length(AnsiString)` (byte-1:1;
/// identical to char count for the ASCII corpus names).
///
/// This is the clean source value (`max(4, longest_bus_name)`). The pinned dss_capi
/// 0.14.5 backend's *effective* `MaxBusNameLength` is an **inconsistent per-report
/// quirk** (probe-proven: `ShowVoltages`/`WriteBusVoltages` floors it at 12,
/// `ShowPowers` at ~5 — even each run in isolation, so it is not the SetMax… loop
/// producing them and not reproducible with any single value). It only ever feeds
/// name-column *padding* — space pads (invisible to the whitespace-tokenizing
/// golden) or `PadDots` runs (the golden's `split_fields` drops pure dot-runs, so
/// the quirk cannot affect a token). SETTLED at the WP8.8 exit sweep: the quirk is
/// nondeterministic (no single value reproduces it), so per the CLAUDE.md UB rule
/// it is NOT reproduced — the honest source value stays, the padding widths are
/// masked cosmetics under the tokenizing comparator (same class as
/// [`max_device_name_length`]'s `= 0`, whose clean fix in the post-1:1 pass
/// covers both).
pub(crate) fn max_bus_name_length(ckt: &Circuit) -> usize {
    let mut m = 4;
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

/// Pascal `(CLASSMASK and DSSObjType) = AUTOTRANS_ELEMENT`, tested on the report
/// walks' `Class.name` label (the registry class name, so a prefix test is exact).
/// Drives the `Ntimes = Nphases` special cases in `WriteTerminalCurrents`
/// (`ShowResults.pas:604`), `ShowPowers` case 1 (`:1190`) and `ShowNodeCurrentSum`
/// (`:3636`).
pub(crate) fn is_autotrans(full_name: &str) -> bool {
    full_name
        .split('.')
        .next()
        .is_some_and(|c| c.eq_ignore_ascii_case("autotrans"))
}

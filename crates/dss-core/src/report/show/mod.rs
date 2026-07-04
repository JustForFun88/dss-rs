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
mod losses;
mod taps;
mod voltages;

pub(crate) use buses::show_buses;
pub(crate) use losses::show_losses;
pub(crate) use taps::show_taps;
pub(crate) use voltages::show_voltages;

use crate::circuit::Circuit;
use crate::exec::registry::DssClass;

/// Pascal `SetMaxBusNameLength` (`ShowResults.pas:101`): the longest bus name,
/// floored at 4. Walks `BusList.NameOfIndex` (the lowercased stored names).
/// Uses **byte** length (`str::len`), matching Pascal's `Length(AnsiString)`
/// (byte-1:1; identical to char count for the ASCII corpus names).
pub(crate) fn max_bus_name_length(ckt: &Circuit) -> usize {
    let mut m = 4;
    for i in 0..ckt.buses.len() {
        if let Some(n) = ckt.bus_list.name(i) {
            m = m.max(n.len());
        }
    }
    m
}

/// Pascal `SetMaxDeviceNameLength` (`ShowResults.pas:111`): the longest
/// `len(Name) + len(ParentClass.Name) + 1` over every circuit element (the
/// `CktElements` master list, creation order), floored at 0. **Byte** length
/// (`str::len`), matching Pascal's `Length(AnsiString)`.
pub(crate) fn max_device_name_length(classes: &[DssClass], ckt: &Circuit) -> usize {
    let mut m = 0;
    for &r in &ckt.ckt_elements {
        let class_len = classes[r.cls].props.class_name().len();
        let name_len = classes[r.cls].objects[r.idx].data().name().len();
        m = m.max(name_len + class_len + 1);
    }
    m
}

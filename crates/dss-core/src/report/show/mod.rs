//! Per-report formatters for `Show` (Pascal `Common/ShowResults.pas`). Each takes
//! a read view of the (solved) circuit and returns the report text; no formatter
//! mutates electrical state (PHASE8_PLAN §2.1). Coverage grows per WP8.4 step.
//!
//! Unlike the `Export` CSV formatters, `Show` emits Pascal's fixed-width text
//! tables (`Pad`/`PadDots` columns, `Format('%W.Df', …)` widths). The targeted
//! text golden (`golden_reports.rs`) diffs them after tokenizing on whitespace +
//! commas (PHASE8_PLAN §2.3), so the exact padding is not gate-load-bearing — but
//! the field structure and the number formats are ported faithfully.
//!
//! **Stage F (F-FMT step 2, `DE_PASCALIZE_PLAN.md` Part IV.2).** Those tables are
//! built as **row data** ([`crate::report::table`]) and rendered by the lane's
//! kernel: the parity lane replays Pascal's `Pad`/`PadDots`/`Format` in order
//! (byte for byte), the default lane hands each run to the table crate, which
//! sizes every column from its own content. Only the padding moves — the module
//! keeps every field, in order, in both lanes (`Cell::sep` enforces it
//! structurally). Section *headers* stay free text wherever a Pascal label spans
//! several data columns (`Show Voltages`' `Mag:`, `Show Currents`' `(Real)`/
//! `(Imag)` pair, `Show Buses`' two-line `Coord` banner); a v2 re-layout of them
//! is plan §F-FMT step 4, a separate decision.

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
mod pv2pq;
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
pub(crate) use pv2pq::show_pv2pq_gen;
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
/// masked cosmetics under the tokenizing comparator (the same class as the
/// device-name column's backend `= 0` — see [`device_name_width`], whose
/// reproduction `GOLDEN_REBASE_PLAN.md` G2.6 tore down; this one never had a
/// reproduction to tear down, because no single value reproduces the quirk in
/// the first place).
pub(crate) fn max_bus_name_length(ckt: &Circuit) -> usize {
    let mut m = 4;
    for i in 0..ckt.buses.len() {
        if let Some(n) = ckt.bus_list.name(i) {
            m = m.max(n.len());
        }
    }
    m
}

/// Pascal `SetMaxDeviceNameLength` (`ShowResults.pas:111-123`): the longest
/// `Length(element.Name) + Length(element.ParentClass.Name) + 1` over the
/// `CktElements` master list — i.e. the longest `Class.Name` full name, 0 for an
/// empty circuit.
///
/// Byte length, like [`max_bus_name_length`] and Pascal's `Length(AnsiString)`.
///
/// # The width every `Show` table uses, in both lanes
///
/// Until `GOLDEN_REBASE_PLAN.md` G2.6 the parity lane discarded this number and
/// padded to **0** instead (`compat::max_device_name_length`), because that is
/// what the pinned dss_capi 0.14.5 backend does. That was a **defect**, not a
/// rendering convention, and since the 2026-08-02 policy no upstream defect is
/// reproduced in any lane:
///
/// * dss_capi's `SetMaxDeviceNameLength` zeroes the *unit* variable
///   (`.inputs/dss_capi/src/Common/ShowResults.pas:116`, declared `:82`) and
///   then accumulates the maximum **inside `with DSS.ActiveCircuit do`**
///   (`:117-121`), where the identifier resolves to the shadowing `TDSSCircuit`
///   field (`src/Common/Circuit.pas:100`, initialized to 30 at `:379`). So the
///   loop fills a field nothing reads, the unit variable keeps the 0 of `:116`,
///   and every consumer that is *not* inside such a `with` block — the
///   `Show BusFlow` power rows at `:1375` are the observable one — formats
///   against width 0.
/// * r4133 does not share it: there `MaxDeviceNameLength` exists only as a unit
///   variable (`Version8/Source/Common/ShowResults.pas:66`), `TDSSCircuit`
///   declares no such field, and the same loop (`:79-90`) therefore leaves the
///   honest width behind. Its `WriteTerminalPowerSeq` also writes the terminal
///   as `j:3` (`:1160`) rather than `IntToStr(j)`, so it could not glue even at
///   width 0.
///
/// The glue is the only tokenizable consequence: `Pad(EncloseQuotes(FullName),
/// width + 2) + IntToStr(j)` (`ShowResults.pas:1375`) has no width of its own,
/// so at width 0 the terminal number lands on the closing quote
/// (`"Capacitor.cap1"1  …`) and at the honest width it is a column. Every other
/// consumer pads with spaces or `PadDots` runs, which the goldens' tokenizer
/// drops. The three `show_busflow*` goldens are therefore compared — in **both**
/// lanes since G2.6 — through an enumerated expectation that splits that one
/// token (`golden_reports.rs::busflow_expected`); no golden byte was
/// re-baselined. Pinned by
/// `exec::tests::compat_quirks::device_name_column_is_sized_from_its_content`.
pub(crate) fn device_name_width(classes: &[DssClass], ckt: &Circuit) -> usize {
    ckt.ckt_elements
        .iter()
        .map(|r| {
            let cls = &classes[r.class_ord()];
            cls.props.class_name().len() + 1 + cls.arena.obj(r.index()).data().name().len()
        })
        .max()
        .unwrap_or(0)
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

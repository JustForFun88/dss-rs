//! `Export Summary` (Pascal `ExportResults.pas` `ExportSummary`): a one-row
//! status/summary line (solve mode, device/bus/node counts, iteration counts,
//! and — when solved — the pu-voltage extremes, total MW/Mvar, and losses).
//!
//! Pascal **appends** to an existing summary file (header only on create), so a
//! sequence of `Export Summary` calls logs one row per call. The append/create
//! decision + the wall-clock `DateTimeToStr(Now)` timestamp are the caller's
//! (they touch the filesystem/clock); this formatter is a pure function of the
//! gathered fields and `include_header`.

use crate::circuit::Circuit;
use crate::report::format;

/// Pascal `GetMaxPUVoltage` (`Utilities.pas:1255`): the maximum node
/// `|V|/kVBase` over buses with a defined base, `×0.001` (pu). `-1` if none.
pub fn max_pu_voltage(ckt: &Circuit) -> f64 {
    let node_v = &ckt.solution.node_v;
    let mut result = -1.0f64;
    for bus in &ckt.buses {
        if bus.kv_base <= 0.0 {
            continue;
        }
        for j in 0..bus.num_nodes_this_bus() {
            let nref = bus.get_ref(j);
            if nref > 0 {
                result = result.max(node_v[nref].norm() / bus.kv_base);
            }
        }
    }
    result * 0.001
}

/// Pascal `GetMinPUVoltage` (`Utilities.pas:1280`): the minimum node `|V|/kVBase`
/// over buses with a defined base, `×0.001` (pu). With `ignore_neutrals`, only
/// nodes above 0.1 pu count. `-1` if none found.
pub fn min_pu_voltage(ckt: &Circuit, ignore_neutrals: bool) -> f64 {
    let node_v = &ckt.solution.node_v;
    let mut result = 1.0e50f64;
    let mut found = false;
    for bus in &ckt.buses {
        if bus.kv_base <= 0.0 {
            continue;
        }
        for j in 0..bus.num_nodes_this_bus() {
            let nref = bus.get_ref(j);
            if nref == 0 {
                continue;
            }
            let vmagpu = node_v[nref].norm() / bus.kv_base;
            if ignore_neutrals {
                if vmagpu > 100.0 {
                    result = result.min(vmagpu);
                    found = true;
                }
            } else {
                result = result.min(vmagpu);
                found = true;
            }
        }
    }
    if found { result * 0.001 } else { -1.0 }
}

/// The gathered fields one `Export Summary` row needs. The caller reads these
/// off the solved circuit/solution (Pascal reads them inline in `ExportSummary`).
pub struct SummaryFields {
    /// `DateTimeToStr(Now)` — the wall-clock timestamp (masked in the golden;
    /// non-deterministic). Written verbatim inside quotes.
    pub datetime: String,
    /// `CaseName` (`"NONE"` when no circuit — unreachable here, the router only
    /// dispatches post-circuit).
    pub case_name: String,
    pub is_solved: bool,
    pub bus_name_redefined: bool,
    /// `SolveModeEnum.OrdinalToString(mode)` (e.g. `Snap`).
    pub mode: String,
    /// `Solution.NumberofTimes`.
    pub number: i32,
    pub load_mult: f64,
    pub num_devices: i32,
    pub num_buses: i32,
    pub num_nodes: i32,
    /// `Solution.Iteration`.
    pub iterations: i32,
    /// `ControlModeEnum.OrdinalToString(ControlMode)` (e.g. `Static`).
    pub control_mode: String,
    pub control_iterations: i32,
    pub most_iterations_done: i32,
    // --- extended columns, present only when `is_solved && !bus_name_redefined`.
    pub year: i32,
    pub hour: i32,
    pub max_pu_voltage: f64,
    pub min_pu_voltage: f64,
    /// `GetTotalPowerFromSources * 1e-6` (MVA), re/im.
    pub total_mw: f64,
    pub total_mvar: f64,
    /// `Circuit.Losses * 1e-6` (MVA), re/im.
    pub mw_losses: f64,
    pub mvar_losses: f64,
}

/// Build the `Export Summary` output (Pascal `ExportSummary`): the header block
/// (only when `include_header`) plus one data row. The extended columns
/// (Year…Frequency) appear iff the circuit is solved and bus names are current.
pub fn export_summary(f: &SummaryFields, frequency: f64, include_header: bool) -> String {
    let extended = f.is_solved && !f.bus_name_redefined;
    let mut s = String::new();

    if include_header {
        s.push_str("DateTime, CaseName, ");
        s.push_str("Status, Mode, Number, LoadMult, NumDevices, NumBuses, NumNodes");
        s.push_str(", Iterations, ControlMode, ControlIterations");
        s.push_str(", MostIterationsDone");
        if extended {
            s.push_str(", Year, Hour, MaxPuVoltage, MinPuVoltage, TotalMW, TotalMvar");
            s.push_str(", MWLosses, pctLosses, MvarLosses, Frequency");
        }
        s.push('\n');
    }

    // Base columns (Pascal writes CaseName even with no circuit as "NONE, ").
    s.push_str(&format!("\"{}\", ", f.datetime));
    s.push_str(&format!("{}, ", f.case_name));
    s.push_str(if f.is_solved { "SOLVED" } else { "UnSolved" });
    s.push_str(&format!(", {}", f.mode));
    s.push_str(&format!(", {}", f.number));
    s.push_str(&format!(", {}", format::fixed(f.load_mult, 3)));
    s.push_str(&format!(", {}", f.num_devices));
    s.push_str(&format!(", {}", f.num_buses));
    s.push_str(&format!(", {}", f.num_nodes));
    s.push_str(&format!(", {}", f.iterations));
    s.push_str(&format!(", {}", f.control_mode));
    s.push_str(&format!(", {}", f.control_iterations));
    s.push_str(&format!(", {}", f.most_iterations_done));

    if extended {
        s.push_str(&format!(", {}", f.year));
        s.push_str(&format!(", {}", f.hour));
        s.push_str(&format!(", {}", format::g(f.max_pu_voltage, 5)));
        s.push_str(&format!(", {}", format::g(f.min_pu_voltage, 5)));
        s.push_str(&format!(", {}", format::g(f.total_mw, 6)));
        s.push_str(&format!(", {}", format::g(f.total_mvar, 6)));
        // MWLosses + pctLosses share one FSWrite gated on total_mw != 0; the
        // '****' fallback prints when the source real power is exactly zero.
        if f.total_mw != 0.0 {
            let pct = f.mw_losses / f.total_mw * 100.0;
            s.push_str(&format!(
                ", {}, {}",
                format::g(f.mw_losses, 6),
                format::g(pct, 4)
            ));
        } else {
            s.push_str(", Total Active Losses:   ****** MW, (**** %)");
        }
        s.push_str(&format!(", {}", format::g(f.mvar_losses, 6)));
        s.push_str(&format!(", {}", format::g(frequency, 6)));
    }

    s.push('\n');
    s
}

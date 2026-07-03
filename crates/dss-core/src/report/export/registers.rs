//! The register-dump exports (Pascal `ExportMeters`/`ExportGenMeters`/
//! `ExportPVSystemMeters`/`ExportStorageMeters`, `ExportResults.pas`). Each dumps
//! one row per enabled element — `Year, LDCurve, Hour, <Name>` then every register
//! (`Format('%10.0f')`) — under a header naming the registers. Unlike the
//! solution exports these **append** to an existing file (a running log across
//! runs), so the file-IO half (append vs create, the `/m` multi-file split) lives
//! in the `exec::report` dispatcher; this module owns the line formatting + the
//! class register-name tables.
//!
//! The EnergyMeter register names are per-object (they encode the zone's voltage
//! bases, `AssignVoltBaseRegisterNames`), so the meter export reads
//! `EnergyMeter::register_names`; the Generator/PVSystem/Storage register names
//! are class-fixed (below).

/// Pascal `TGenerator.RegisterNames` (`Generator.pas:462`).
pub(crate) const GEN_REGISTER_NAMES: [&str; 6] =
    ["kWh", "kvarh", "Max kW", "Max kVA", "Hours", "$"];
/// Pascal `TPVsystem.RegisterNames` (`PVsystem.pas:392`).
pub(crate) const PVSYSTEM_REGISTER_NAMES: [&str; 6] =
    ["kWh", "kvarh", "Max kW", "Max kVA", "Hours", "Price($)"];
/// Pascal `TStorage.RegisterNames` (`Storage.pas:492`).
pub(crate) const STORAGE_REGISTER_NAMES: [&str; 6] =
    ["kWh", "kvarh", "Max kW", "Max kVA", "Hours", "Price($)"];

/// One enabled element's register row payload (gathered by the dispatcher, which
/// owns the downcast to the concrete element type).
pub(crate) struct RegRow {
    /// Original-case element name (uppercased at row-format time, Pascal
    /// `AnsiUpperCase`).
    pub name: String,
    /// This element's own register names — used only by the `/m` multi-file path,
    /// where each file carries its element's header (meter names differ per zone).
    pub register_names: Vec<String>,
    pub registers: Vec<f64>,
}

/// The header line (Pascal `FSWrite(F, 'Year, LDCurve, Hour, <label>')` then
/// `Separator + '"' + regName + '"'` per register). Compared **verbatim** by the
/// golden, so the `, ` separators + the `"…"` quoting are byte-exact.
pub(crate) fn register_header(label: &str, reg_names: &[String]) -> String {
    let mut s = format!("Year, LDCurve, Hour, {label}");
    for rn in reg_names {
        s.push_str(", \"");
        s.push_str(rn);
        s.push('"');
    }
    s
}

/// One data row (Pascal: `Year`/`LDCurve`/`Hour`/`Pad('"'+UPPER(name)+'"',14)`
/// then `Separator + Format('%10.0f', [Register])` per register). The golden
/// parses numbers out of the (trimmed) fields, so the `Pad`/`%10.0f` widths are
/// cosmetic — reproduced for faithful, readable output. `ldcurve` is
/// `NameIfNotNil(LoadDurCurveObj)` (always `''` here — the LoadDuration mode /
/// `Set LoadDurCurve=` is not modeled).
pub(crate) fn register_row(
    year: i32,
    ldcurve: &str,
    hour: i32,
    name: &str,
    registers: &[f64],
) -> String {
    // Pascal `Pad(S, 14)` right-pads with blanks to width 14 (no truncation when
    // already longer), matching Rust's `{:<14}`.
    let name_field = format!("\"{}\"", name.to_uppercase());
    let mut s = format!("{year}, {ldcurve}, {hour}, {name_field:<14}");
    for &v in registers {
        // Pascal `Format('%10.0f', [v])`: right-justified, 0 decimals, width 10.
        s.push_str(&format!(", {v:10.0}"));
    }
    s
}

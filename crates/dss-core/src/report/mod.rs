//! Phase 8 reporting / output layer: `Export`, `Show`, `Save`, `Dump`.
//!
//! Pascal home: `Executive/ExportOptions.pas` (`TExportOption` + `DoExportCmd`),
//! `Executive/ShowOptions.pas` (`TShowOption` + `DoShowCmd`),
//! `Common/ExportResults.pas`, `Common/ShowResults.pas`, plus the `Save`/`Dump`
//! machinery in `Common/Circuit.pas` / `Common/Utilities.pas` /
//! `Executive/ExecHelper.pas`.
//!
//! This module owns the **report content**: the option name tables (here), the
//! number/string formatters, the output-path machinery, and the per-report
//! formatters as they land per WP (PHASE8_PLAN §2.1). The thin command
//! **routers** (`Dss::do_{export,show,save,dump}_cmd`) live in `exec/report.rs`
//! as `impl Dss`, like every other command router (`command.rs`/`set_cmd.rs`),
//! because they drive the (private) parser/circuit/error state; they delegate
//! all formatting here.
//!
//! Reports are read-only over the solved circuit — no formatter mutates shared
//! electrical state (PHASE8_PLAN §2.1).

pub mod export;
pub mod format;
pub mod output;
pub mod show;

/// Pascal `TExportOption` names in ordinal order (`ExportOptions.pas`
/// `DefineOptions` → `GetEnumName`), the **non-`DSS_CAPI_ADIAKOPTICS`** build
/// (`High(TExportOption) = Laplacian`, so 57 options; `ZLL`/`ZCC`/`Contours`/
/// `Y4` are ADIAKOPTICS-only and not registered — Phase 9, PHASE8_PLAN §4).
/// Index `i` is ordinal `i + 1`, matching the Pascal `case ParamPointer of`.
/// Spelling is verbatim from the enum (case-insensitive at match time via
/// [`CommandList`](crate::support::command_list::CommandList), so the
/// abbreviation ownership matches the oracle's `ExportCommands`).
pub(crate) const EXPORT_OPTIONS: &[&str] = &[
    "Voltages",          // 1
    "SeqVoltages",       // 2
    "Currents",          // 3
    "SeqCurrents",       // 4
    "Estimation",        // 5
    "Capacity",          // 6
    "Overloads",         // 7
    "Unserved",          // 8
    "Powers",            // 9
    "SeqPowers",         // 10
    "Faultstudy",        // 11
    "Generators",        // 12
    "Loads",             // 13
    "Meters",            // 14
    "Monitors",          // 15
    "Yprims",            // 16
    "Y",                 // 17
    "seqz",              // 18
    "P_byphase",         // 19
    "CIM100Fragments",   // 20
    "CIM100",            // 21
    "CDPSMAsset",        // 22
    "Buscoords",         // 23
    "Losses",            // 24
    "Uuids",             // 25
    "Counts",            // 26
    "Summary",           // 27
    "CDPSMElec",         // 28
    "CDPSMGeo",          // 29
    "CDPSMTopo",         // 30
    "CDPSMStateVar",     // 31
    "Profile",           // 32
    "EventLog",          // 33
    "AllocationFactors", // 34
    "VoltagesElements",  // 35
    "GICMvars",          // 36
    "BusReliability",    // 37
    "BranchReliability", // 38
    "NodeNames",         // 39
    "Taps",              // 40
    "NodeOrder",         // 41
    "ElemCurrents",      // 42
    "ElemVoltages",      // 43
    "ElemPowers",        // 44
    "Result",            // 45
    "YNodeList",         // 46
    "YVoltages",         // 47
    "YCurrents",         // 48
    "PVSystem_Meters",   // 49
    "Storage_Meters",    // 50
    "Sections",          // 51
    "ErrorLog",          // 52
    "IncMatrix",         // 53
    "IncMatrixRows",     // 54
    "IncMatrixCols",     // 55
    "BusLevels",         // 56
    "Laplacian",         // 57
];

/// Pascal `TShowOption` names in ordinal order (`ShowOptions.pas`
/// `DefineOptions` → `GetEnumName`). Index `i` is ordinal `i + 1`. No build
/// guards. Spelling verbatim from the enum.
pub(crate) const SHOW_OPTIONS: &[&str] = &[
    "autoadded",      // 1
    "buses",          // 2
    "currents",       // 3
    "convergence",    // 4
    "elements",       // 5
    "faults",         // 6
    "isolated",       // 7
    "generators",     // 8
    "meters",         // 9
    "monitor",        // 10
    "panel",          // 11
    "powers",         // 12
    "voltages",       // 13
    "zone",           // 14
    "taps",           // 15
    "overloads",      // 16
    "unserved",       // 17
    "eventlog",       // 18
    "variables",      // 19
    "ratings",        // 20
    "loops",          // 21
    "losses",         // 22
    "busflow",        // 23
    "lineconstants",  // 24
    "yprim",          // 25
    "y",              // 26
    "controlqueue",   // 27
    "topology",       // 28
    "mismatch",       // 29
    "kvbasemismatch", // 30
    "deltaV",         // 31
    "QueryLog",       // 32
    "Controlled",     // 33
    "Result",         // 34
];

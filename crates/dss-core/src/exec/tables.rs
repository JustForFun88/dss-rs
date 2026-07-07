//! The command/option name tables and their dispatch ordinals, split out of
//! `exec/mod.rs` (no behavioral change). [`EXEC_COMMANDS`]/[`EXEC_OPTIONS`] are
//! the Pascal name lists (so abbreviation ownership matches the oracle), and
//! [`cmd`]/[`opt`] are the ordinals the executive actually dispatches on.

/// Pascal `TExecCommand` names in ordinal order (`DefineCommands`), with the
/// non-identifier spellings replaced exactly as the Pascal does (`vr`→`var`,
/// `tilde`→`~`, `DoubleSlash`→`//`, `questionmark`→`?`, `SetOpt`→`Set`).
/// Index `i` is ordinal `i + 1`. The `DSS_CAPI_PM`-only commands (NewActor,
/// Wait, SolveAll) are appended because the oracle build defines that flag;
/// `Abort`/`Clone` close the list (125 names, oracle order, pinned byte-exact
/// by the `dump commands` golden). Unmatched ordinals dispatch to
/// `not_ported_command`.
pub(crate) const EXEC_COMMANDS: &[&str] = &[
    "New",
    "Edit",
    "More",
    "M",
    "~",
    "Select",
    "Save",
    "Show",
    "Solve",
    "Enable",
    "Disable",
    "Plot",
    "Reset",
    "Compile",
    "Set",
    "Dump",
    "Open",
    "Close",
    "//",
    "Redirect",
    "Help",
    "Quit",
    "?",
    "Next",
    "Panel",
    "Sample",
    "Clear",
    "About",
    "Calcvoltagebases",
    "SetkVBase",
    "BuildY",
    "Get",
    "Init",
    "Export",
    "Fileedit",
    "Voltages",
    "Currents",
    "Powers",
    "Seqvoltages",
    "Seqcurrents",
    "Seqpowers",
    "Losses",
    "Phaselosses",
    "Cktlosses",
    "Allocateloads",
    "Formedit",
    "Totals",
    "Capacity",
    "Classes",
    "Userclasses",
    "Zsc",
    "Zsc10",
    "ZscRefresh",
    "Ysc",
    "puvoltages",
    "VarValues",
    "Varnames",
    "Buscoords",
    "MakeBusList",
    "MakePosSeq",
    "Reduce",
    "Interpolate",
    "AlignFile",
    "TOP",
    "Rotate",
    "Vdiff",
    "Summary",
    "Distribute",
    "DI_plot",
    "Comparecases",
    "YearlyCurves",
    "CD",
    "Visualize",
    "CloseDI",
    "DOScmd",
    "Estimate",
    "Reconductor",
    "_InitSnap",
    "_SolveNoControl",
    "_SampleControls",
    "_DoControlActions",
    "_ShowControlQueue",
    "_SolveDirect",
    "_SolvePFlow",
    "AddBusMarker",
    "Uuids",
    "SetLoadAndGenKV",
    "CvrtLoadshapes",
    "NodeDiff",
    "Rephase",
    "SetBusXY",
    "UpdateStorage",
    "Obfuscate",
    "LatLongCoords",
    "BatchEdit",
    "Pstcalc",
    "Variable",
    "ReprocessBuses",
    "ClearBusMarkers",
    "RelCalc",
    "var",
    "Cleanup",
    "FinishTimeStep",
    "NodeList",
    "Connect",
    "Disconnect",
    "Remove",
    "CalcIncMatrix",
    "CalcIncMatrix_O",
    "Refine_BusLevels",
    "CalcLaplacian",
    "ExportOverloads",
    "ExportVViolations",
    "Zsc012",
    "AllPCEatBus",
    "AllPDEatBus",
    "TotalPowers",
    "GISCoords",
    "ClearAll",
    "COMHelp",
    "NewActor",
    "Wait",
    "SolveAll",
    "Abort",
    "Clone",
];

/// `TExecCommand` ordinals the executive dispatches on.
pub(crate) mod cmd {
    pub const NEW: usize = 1;
    pub const EDIT: usize = 2;
    pub const MORE: usize = 3;
    pub const M: usize = 4;
    pub const TILDE: usize = 5;
    pub const SELECT: usize = 6;
    pub const SAVE: usize = 7;
    pub const SHOW: usize = 8;
    pub const SOLVE: usize = 9;
    pub const PLOT: usize = 12;
    pub const DUMP: usize = 16;
    pub const OPEN: usize = 17;
    pub const CLOSE: usize = 18;
    pub const RESET: usize = 13;
    pub const SAMPLE: usize = 26;
    pub const COMPILE: usize = 14;
    pub const SET: usize = 15;
    pub const COMMENT: usize = 19; // "//"
    pub const REDIRECT: usize = 20;
    pub const HELP: usize = 21;
    pub const QUIT: usize = 22;
    pub const QUERY: usize = 23; // "?"
    pub const PANEL: usize = 25;
    pub const CLEAR: usize = 27;
    pub const ABOUT: usize = 28;
    pub const CALC_VOLTAGE_BASES: usize = 29;
    pub const BUILD_Y: usize = 31;
    pub const GET: usize = 32;
    pub const INIT: usize = 33;
    pub const EXPORT: usize = 34;
    pub const FILEEDIT: usize = 35;
    pub const ALLOCATE_LOADS: usize = 45;
    pub const CLASSES: usize = 49;
    pub const USERCLASSES: usize = 50;
    pub const BUSCOORDS: usize = 58;
    pub const MAKE_BUS_LIST: usize = 59;
    pub const INTERPOLATE: usize = 62;
    pub const ALIGN_FILE: usize = 63;
    pub const DI_PLOT: usize = 69;
    pub const COMPARE_CASES: usize = 70;
    pub const YEARLY_CURVES: usize = 71;
    pub const CD: usize = 72;
    pub const DISTRIBUTE: usize = 68;
    pub const UUIDS: usize = 86;
    pub const VISUALIZE: usize = 73;
    pub const CLOSE_DI: usize = 74;
    pub const DOSCMD: usize = 75;
    pub const CVRT_LOADSHAPES: usize = 88;
    pub const REDUCE: usize = 61;
    pub const REMOVE: usize = 107;
    pub const SET_BUS_XY: usize = 91;
    pub const BATCH_EDIT: usize = 95;
    pub const RELCALC: usize = 100;
    pub const VAR: usize = 101;
    pub const GIS_COORDS: usize = 118;
    pub const CLEAR_ALL: usize = 119;
    pub const COMHELP: usize = 120;
}

/// Pascal `TExecOption` names in ordinal order (`DefineOptions`), with the
/// same replacements the Pascal applies (`pct`→`%`, `cls`→`class`,
/// `typ`→`type`, `obj`→`object`). Index `i` is ordinal `i + 1`.
pub(crate) const EXEC_OPTIONS: &[&str] = &[
    "type",
    "element",
    "hour",
    "sec",
    "year",
    "frequency",
    "stepsize",
    "mode",
    "random",
    "number",
    "time",
    "class",
    "object",
    "circuit",
    "editor",
    "tolerance",
    "maxiterations",
    "h",
    "Loadmodel",
    "Loadmult",
    "normvminpu",
    "normvmaxpu",
    "emergvminpu",
    "emergvmaxpu",
    "%mean",
    "%stddev",
    "LDCurve",
    "%growth",
    "Genkw",
    "Genpf",
    "CapkVAR",
    "Addtype",
    "Allowduplicates",
    "Zonelock",
    "UEweight",
    "Lossweight",
    "UEregs",
    "Lossregs",
    "Voltagebases",
    "Algorithm",
    "Trapezoidal",
    "Autobuslist",
    "Controlmode",
    "Tracecontrol",
    "Genmult",
    "Defaultdaily",
    "Defaultyearly",
    "Allocationfactors",
    "Cktmodel",
    "Pricesignal",
    "Pricecurve",
    "Terminal",
    "Basefrequency",
    "Harmonics",
    "Maxcontroliter",
    "Bus",
    "Datapath",
    "KeepList",
    "ReduceOption",
    "DemandInterval",
    "%Normal",
    "DIVerbose",
    "Casename",
    "Markercode",
    "Nodewidth",
    "Log",
    "Recorder",
    "Overloadreport",
    "Voltexceptionreport",
    "Cfactors",
    "ShowExport",
    "Numallociterations",
    "DefaultBaseFrequency",
    "Markswitches",
    "Switchmarkercode",
    "Daisysize",
    "Marktransformers",
    "TransMarkerCode",
    "TransMarkerSize",
    "LoadShapeClass",
    "EarthModel",
    "QueryLog",
    "MarkCapacitors",
    "MarkRegulators",
    "MarkPVSystems",
    "MarkStorage",
    "CapMarkerCode",
    "RegMarkerCode",
    "PVMarkerCode",
    "StoreMarkerCode",
    "CapMarkerSize",
    "RegMarkerSize",
    "PVMarkerSize",
    "StoreMarkerSize",
    "NeglectLoadY",
    "MarkFuses",
    "FuseMarkerCode",
    "FuseMarkerSize",
    "MarkReclosers",
    "RecloserMarkerCode",
    "RecloserMarkerSize",
    "RegistryUpdate",
    "MarkRelays",
    "RelayMarkerCode",
    "RelayMarkerSize",
    "ProcessTime",
    "TotalTime",
    "StepTime",
    "SampleEnergyMeters",
    "MinIterations",
    "DSSVisualizationTool",
    "KeepLoad",
    "Zmag",
    "SeasonRating",
    "SeasonSignal",
    "LineTypes",
    "EventLogDefault",
    "LongLineCorrection",
    "ShowReports",
    // The `DSS_CAPI_PM` parallel-machine options (the oracle build defines that
    // flag, so its name table — and therefore `Dump commands` — includes them).
    // Unmatched ordinals fall to the "not ported yet" arms of `Set`/`Get`.
    "NumCPUs",
    "NumCores",
    "NumActors",
    "ActiveActor",
    "CPU",
    "ActorProgress",
    "Parallel",
    "ConcatenateReports",
    "NUMANodes",
];

/// `TExecOption` ordinals the executive implements.
pub(crate) mod opt {
    pub const HOUR: usize = 3;
    pub const SEC: usize = 4;
    pub const YEAR: usize = 5;
    pub const FREQUENCY: usize = 6;
    pub const STEPSIZE: usize = 7;
    pub const MODE: usize = 8;
    pub const RANDOM: usize = 9;
    pub const NUMBER: usize = 10;
    pub const TIME: usize = 11;
    pub const TOLERANCE: usize = 16;
    pub const MAXITERATIONS: usize = 17;
    /// `h` is an alias of `stepsize` (Pascal `7, 18:`).
    pub const H: usize = 18;
    pub const LOADMODEL: usize = 19;
    pub const LOADMULT: usize = 20;
    pub const NORMVMINPU: usize = 21;
    pub const NORMVMAXPU: usize = 22;
    pub const EMERGVMINPU: usize = 23;
    pub const EMERGVMAXPU: usize = 24;
    pub const LDCURVE: usize = 27;
    pub const PCT_GROWTH: usize = 28;
    pub const GEN_KW: usize = 29;
    pub const GEN_PF: usize = 30;
    pub const CAP_KVAR: usize = 31;
    pub const ADD_TYPE: usize = 32;
    pub const ALLOW_DUPLICATES: usize = 33;
    pub const ZONE_LOCK: usize = 34;
    pub const UE_WEIGHT: usize = 35;
    pub const LOSS_WEIGHT: usize = 36;
    pub const UE_REGS: usize = 37;
    pub const LOSS_REGS: usize = 38;
    pub const VOLTAGE_BASES: usize = 39;
    pub const ALGORITHM: usize = 40;
    pub const TRAPEZOIDAL: usize = 41;
    pub const AUTO_BUS_LIST: usize = 42;
    pub const CONTROL_MODE: usize = 43;
    pub const DEFAULT_DAILY: usize = 46;
    pub const DEFAULT_YEARLY: usize = 47;
    pub const CKT_MODEL: usize = 49;
    pub const PRICE_SIGNAL: usize = 50;
    pub const PRICE_CURVE: usize = 51;
    pub const BASE_FREQUENCY: usize = 53;
    pub const HARMONICS: usize = 54;
    pub const MAX_CONTROL_ITER: usize = 55;
    pub const ALLOCATION_FACTORS: usize = 48;
    pub const DEMAND_INTERVAL: usize = 60;
    pub const DI_VERBOSE: usize = 62;
    pub const CASE_NAME: usize = 63;
    pub const MARKER_CODE: usize = 64;
    pub const NODE_WIDTH: usize = 65;
    pub const LOG: usize = 66;
    pub const OVERLOAD_REPORT: usize = 68;
    pub const VOLT_EXCEPTION_REPORT: usize = 69;
    pub const SAMPLE_ENERGY_METERS: usize = 109;
    pub const LOAD_SHAPE_CLASS: usize = 80;
    pub const EARTH_MODEL: usize = 81;
    pub const NUM_ALLOC_ITERATIONS: usize = 72;
    pub const DEFAULT_BASE_FREQUENCY: usize = 73;
    pub const NEGLECT_LOAD_Y: usize = 95;
    pub const MIN_ITERATIONS: usize = 110;
    pub const KEEP_LIST: usize = 58;
    pub const REDUCE_OPTION: usize = 59;
    pub const KEEP_LOAD: usize = 112;
    pub const ZMAG: usize = 113;
    pub const DATA_PATH: usize = 57;
}

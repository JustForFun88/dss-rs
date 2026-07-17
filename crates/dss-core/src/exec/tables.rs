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
    pub const ENABLE: usize = 10;
    pub const DISABLE: usize = 11;
    pub const SET_KV_BASE: usize = 30;
    pub const LOSSES: usize = 42;
    pub const SUMMARY: usize = 67;
    pub const RECONDUCTOR: usize = 77;
    pub const INIT_SNAP: usize = 78;
    pub const SOLVE_NO_CONTROL: usize = 79;
    pub const SAMPLE_CONTROLS: usize = 80;
    pub const DO_CONTROL_ACTIONS: usize = 81;
    pub const SHOW_CONTROL_QUEUE: usize = 82;
    pub const SOLVE_DIRECT: usize = 83;
    pub const SOLVE_PFLOW: usize = 84;
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
    pub const LATLONGCOORDS: usize = 94;
    pub const MAKE_BUS_LIST: usize = 59;
    /// `MakePosSeq` (`ExecCommands.pas`): the 60th `TExecCommand`, dispatched to
    /// `TExecHelper.DoMakePosSeq`. (The array position is 60 — index 59 in
    /// `EXEC_COMMANDS`, right after `MakeBusList`.)
    pub const MAKE_POS_SEQ: usize = 60;
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
    pub const PSTCALC: usize = 96;
    pub const RELCALC: usize = 100;
    pub const VAR: usize = 101;
    pub const ADD_BUS_MARKER: usize = 85;
    pub const CLEAR_BUS_MARKER: usize = 99;
    pub const GIS_COORDS: usize = 118;
    pub const CALC_INC_MATRIX: usize = 108;
    pub const CALC_INC_MATRIX_O: usize = 109;
    /// `Refine_BusLevels` (position 110 in `EXEC_COMMANDS`, between
    /// `CalcIncMatrix_O` and `CalcLaplacian`): official `ExecCommands.pas` cmd 114
    /// → `Get_paths_4_Coverage` (WP-AD.5).
    pub const REFINE_BUSLEVELS: usize = 110;
    pub const CALC_LAPLACIAN: usize = 111;
    pub const WAIT: usize = 122;
    /// `SolveAll` (ordinal 123, `DSS_CAPI_PM`-only, appended to the oracle
    /// build's command list). Single-actor semantics = plain `Solve`
    /// (`ExecCommands.pas:346` iterates `DoSetCmd(child, 1)` over the one
    /// actor; `IsSolveAll` only steers the parallel/A-Diakoptics path we do
    /// not have).
    pub const SOLVE_ALL: usize = 123;
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
    // dss_capi 0.15.0b4 (`e936d210`) appended these after the PM block (the
    // `DSS_CAPI_ADIAKOPTICS` block is compiled out of the oracle build, so it
    // does not shift the ordinals). `IgnoreGenQLimits`/`NCIMQGain` are NCIM
    // (WP-U1.7); `PyPath` is NOT_PORTED (loud, §0); `AllowForms`/
    // `AllowProgressBar` are GUI no-ops. `StateVar`/`IterNumber`/
    // `CtrlIterNumber`/`InjCurrent`/`ITerminal`/`YPrim`/`IntegrationFlag` are
    // the WP-U1.9 PCE force hooks. `AllowForms`/`AllowProgressBar` are accepted
    // headless no-ops (`Set` stores the flag, `Get` reads it back — capi015
    // silently accepts them; erroring would diverge). Names must equal FPC
    // `GetEnumName`.
    "IgnoreGenQLimits",
    "NCIMQGain",
    "StateVar",
    "PyPath",
    "IterNumber",
    "CtrlIterNumber",
    "InjCurrent",
    "ITerminal",
    "YPrim",
    "IntegrationFlag",
    "AllowForms",
    "AllowProgressBar",
];

/// Pascal `TPlotOption` names in ordinal order (`PlotOptions.DefineOptions`),
/// with the `__` stripped and the two renames `typ`→`type`, `obj`→`object`.
/// Index `i` is `ParamPointer` `i + 1` (matched abbreviation-wise by the
/// `plot_commands` `CommandList`). See `PlotOptions.pas:19-44/152-170`.
pub(crate) const PLOT_OPTIONS: &[&str] = &[
    "type",
    "quantity",
    "max",
    "dots",
    "labels",
    "object",
    "showloops",
    "r3",
    "r2",
    "c1",
    "c2",
    "c3",
    "channels",
    "bases",
    "subs",
    "thickness",
    "buslist",
    "min",
    "3phLinestyle",
    "1phLinestyle",
    "phases",
    "profilescale",
    "PlotID",
];

/// `TExecOption` ordinals the executive implements.
pub(crate) mod opt {
    /// `Set Daisysize=` (ExecOptions.pas Daisysize=76): the DSS-context
    /// `DaisySize` written into the plot payload.
    pub const DAISY_SIZE: usize = 76;

    // The GUI plot-marker style options (ExecOptions.pas Set arms :615-682 /
    // Get arms :978-1041): headless-inert Circuit fields emitted into the
    // plot-callback `Markers` object (WPG.17 Plot audit settlement).
    pub const MARK_SWITCHES: usize = 74;
    pub const SWITCH_MARKER_CODE: usize = 75;
    pub const MARK_TRANSFORMERS: usize = 77;
    pub const TRANS_MARKER_CODE: usize = 78;
    pub const TRANS_MARKER_SIZE: usize = 79;
    pub const MARK_CAPACITORS: usize = 83;
    pub const MARK_REGULATORS: usize = 84;
    pub const MARK_PVSYSTEMS: usize = 85;
    pub const MARK_STORAGE: usize = 86;
    pub const CAP_MARKER_CODE: usize = 87;
    pub const REG_MARKER_CODE: usize = 88;
    pub const PV_MARKER_CODE: usize = 89;
    pub const STORE_MARKER_CODE: usize = 90;
    pub const CAP_MARKER_SIZE: usize = 91;
    pub const REG_MARKER_SIZE: usize = 92;
    pub const PV_MARKER_SIZE: usize = 93;
    pub const STORE_MARKER_SIZE: usize = 94;
    pub const MARK_FUSES: usize = 96;
    pub const FUSE_MARKER_CODE: usize = 97;
    pub const FUSE_MARKER_SIZE: usize = 98;
    pub const MARK_RECLOSERS: usize = 99;
    pub const RECLOSER_MARKER_CODE: usize = 100;
    pub const RECLOSER_MARKER_SIZE: usize = 101;
    pub const MARK_RELAYS: usize = 103;
    pub const RELAY_MARKER_CODE: usize = 104;
    pub const RELAY_MARKER_SIZE: usize = 105;

    /// `Set Type=`/`Set Class=` both activate a class (Pascal `1, 12:
    /// SetObjectClass`). `Set Element=`/`Set Object=` both select an object
    /// (Pascal `2, 13: SetObject`). WP-U1.1 item 5.
    pub const TYPE: usize = 1;
    pub const ELEMENT: usize = 2;
    pub const HOUR: usize = 3;
    pub const SEC: usize = 4;
    pub const YEAR: usize = 5;
    pub const FREQUENCY: usize = 6;
    pub const STEPSIZE: usize = 7;
    pub const MODE: usize = 8;
    pub const RANDOM: usize = 9;
    pub const NUMBER: usize = 10;
    pub const TIME: usize = 11;
    pub const CLASS: usize = 12;
    pub const OBJECT: usize = 13;
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
    pub const PCT_MEAN: usize = 25;
    pub const PCT_STDDEV: usize = 26;
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
    pub const GEN_MULT: usize = 45;
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
    pub const SHOW_EXPORT: usize = 71;
    /// `ProcessTime` (Get-only → `Solve_Time_Elapsed`) / `TotalTime`
    /// (Set+Get → `Total_Time_Elapsed`) / `StepTime` (Get-only →
    /// `Step_Time_Elapsed`), `ExecOptions.pas:106-108`.
    pub const PROCESS_TIME: usize = 106;
    pub const TOTAL_TIME: usize = 107;
    pub const STEP_TIME: usize = 108;
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
    pub const SEASON_RATING: usize = 114;
    pub const SEASON_SIGNAL: usize = 115;
    pub const DATA_PATH: usize = 57;
    /// `Set/Get LongLineCorrection=` (`ExecOptions.pas:138`, ordinal 118): the
    /// circuit's long-line (Kron) impedance correction flag. In the oracle's
    /// `DSS_CAPI_PM` build (`ExecOptions.pas:737/1095`).
    pub const LONG_LINE_CORRECTION: usize = 118;
    /// `Set/Get IgnoreGenQLimits=` (`ExecOptions.pas:156`, ordinal 129 in the
    /// capi build — the `DSS_CAPI_ADIAKOPTICS` block is ifdef'd out of the
    /// oracle, so the NCIM options follow `NUMANodes=128` directly): NCIM's
    /// `Solution.NCIM_IgnoreQLimit`.
    pub const IGNORE_GEN_Q_LIMITS: usize = 129;
    /// `Set/Get NCIMQGain=` (`ExecOptions.pas:157`, ordinal 130): NCIM's global
    /// reactive-power injection gain `Solution.NCIM_GenGain`.
    pub const NCIM_Q_GAIN: usize = 130;

    // WP-U1.9 PCE force-hook options, appended in dss_capi 0.15.0b4
    // (`ExecOptions.pas` @ `e936d210`) after the PM block (NUMANodes = ordinal
    // 128 in the oracle/runtime `EXEC_OPTIONS`). `IgnoreGenQLimits` (129) /
    // `NCIMQGain` (130) are NCIM (WP-U1.7). Ordinals verified against the runtime
    // `option_list.get_command` (an in-array comment made a naive source count
    // read one high).
    /// `Set StateVar <pce> <name> <value>` / `Get StateVar <pce> <name>`:
    /// write/read a PC element's dynamic state variable (A5-r3723 option).
    pub const STATE_VAR: usize = 131;
    /// `Set PyPath=` — pyControl co-simulation server launch; NOT_PORTED (§0).
    pub const PY_PATH: usize = 132;
    /// `Get IterNumber` (read-only): `Solution.Iteration`.
    pub const ITER_NUMBER: usize = 133;
    /// `Get CtrlIterNumber` (read-only): `Solution.ControlIteration`.
    pub const CTRL_ITER_NUMBER: usize = 134;
    /// `Set/Get InjCurrent`: force/read the active PCE's injection currents.
    pub const INJ_CURRENT: usize = 135;
    /// `Set/Get ITerminal`: force/read the active PCE's terminal currents.
    pub const ITERMINAL: usize = 136;
    /// `Set/Get YPrim`: force/read the active PCE's primitive Y matrix.
    pub const YPRIM: usize = 137;
    /// `Get IntegrationFlag` (read-only): `Solution.DynaVars.IterationFlag`.
    pub const INTEGRATION_FLAG: usize = 138;
    /// `Set/Get AllowForms` (`ExecOptions.pas:777`, `NoFormsAllowed`): a
    /// console-form gate with no meaning in a headless engine — stored for
    /// `Set`/`Get` round-trip parity, nothing reads it (cf. `SHOW_EXPORT`).
    pub const ALLOW_FORMS: usize = 139;
    /// `Set/Get AllowProgressBar` (`ExecOptions.pas:779`,
    /// `NoProgressBarFormAllowed`): headless no-op, stored for parity.
    pub const ALLOW_PROGRESS_BAR: usize = 140;
}

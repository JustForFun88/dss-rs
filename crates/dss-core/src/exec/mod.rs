//! The command executive: a port of `Executive.pas` / `ExecCommands.pas` /
//! `ExecOptions.pas` / `ExecHelper.pas`, Phase 3 subset. The full Pascal
//! command and option name lists are registered (so abbreviation ownership
//! matches the oracle), but only the ★-slice verbs do real work — `New`
//! (including `New circuit.`), `Edit`, `~`/`More`/`M`, `Set`/`Get`, `Solve`,
//! `CalcVoltageBases`, `Redirect`/`Compile`, `Clear`, `BuildY`, `Init`, and
//! `?`. Everything else records a clear not-ported message.
//!
//! [`Dss`] is the Rust form of `TDSSContext`: it owns the class registry,
//! the active circuit, the parsers, the enum table, and the error log.

#[cfg(test)]
mod tests;

pub(crate) use std::collections::HashMap;
pub(crate) use std::path::{Path, PathBuf};

pub(crate) use dss_parser::{Parser, ParserVars};

pub(crate) use crate::circuit::{Circuit, ElemKind};
pub(crate) use crate::elements::control::{
    cap_control, gen_dispatcher, reg_control, storage_controller,
};
pub(crate) use crate::elements::general::{
    conductor_data, growth_shape, line_code, line_geometry, line_spacing, load_shape, price_shape,
    spectrum, tcc_curve, temp_shape, xfmr_code, xy_curve,
};
pub(crate) use crate::elements::meter::energymeter;
pub(crate) use crate::elements::meter::monitor;
pub(crate) use crate::elements::meter::sensor;
pub(crate) use crate::elements::pc::{generator, load, vsource};
pub(crate) use crate::elements::pd::{capacitor, line, reactor, transformer};
pub(crate) use crate::elements::traits::{CktElement, ElemRef, ElemStore};
pub(crate) use crate::obj::base::DssObject;
pub(crate) use crate::obj::dss_enum::{EnumId, EnumRegistry};
pub(crate) use crate::obj::props::{ClassProps, ForeignClassesView, PropEngine, PropType};
pub(crate) use crate::solution::{SolveEnv, SolveMode, set_voltage_bases, solve};
pub(crate) use crate::support::command_list::CommandList;
pub(crate) use crate::util::{float_to_str, interpret_yes_no, parse_object_class_and_name};

mod command;
mod helpers;
mod registry;
mod set_get;
mod solve;
mod view;

pub(crate) use helpers::*;
pub(crate) use registry::{ClassStore, DssClass, ForeignClasses};
pub use view::{ElementSnapshot, MeterZoneView, MonitorView, SystemYCsc};

/// Pascal `TExecCommand` names in ordinal order (`DefineCommands`), with the
/// non-identifier spellings replaced exactly as the Pascal does (`vr`→`var`,
/// `tilde`→`~`, `DoubleSlash`→`//`, `questionmark`→`?`, `SetOpt`→`Set`).
/// Index `i` is ordinal `i + 1`. The `DSS_CAPI_PM`-only commands (NewActor,
/// Wait, SolveAll) are appended because the oracle build defines that flag.
const EXEC_COMMANDS: &[&str] = &[
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
];

/// `TExecCommand` ordinals the executive dispatches on.
mod cmd {
    pub const NEW: usize = 1;
    pub const EDIT: usize = 2;
    pub const MORE: usize = 3;
    pub const M: usize = 4;
    pub const TILDE: usize = 5;
    pub const SOLVE: usize = 9;
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
    pub const FILEEDIT: usize = 35;
    pub const ALLOCATE_LOADS: usize = 45;
    pub const CLASSES: usize = 49;
    pub const USERCLASSES: usize = 50;
    pub const BUSCOORDS: usize = 58;
    pub const ALIGN_FILE: usize = 63;
    pub const DI_PLOT: usize = 69;
    pub const COMPARE_CASES: usize = 70;
    pub const YEARLY_CURVES: usize = 71;
    pub const CD: usize = 72;
    pub const DOSCMD: usize = 75;
    pub const CVRT_LOADSHAPES: usize = 88;
    pub const REDUCE: usize = 61;
    pub const RELCALC: usize = 100;
    pub const VAR: usize = 101;
    pub const CLEAR_ALL: usize = 119;
    pub const COMHELP: usize = 120;
}

/// Pascal `TExecOption` names in ordinal order (`DefineOptions`), with the
/// same replacements the Pascal applies (`pct`→`%`, `cls`→`class`,
/// `typ`→`type`, `obj`→`object`). Index `i` is ordinal `i + 1`.
const EXEC_OPTIONS: &[&str] = &[
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
];

/// `TExecOption` ordinals the executive implements.
mod opt {
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
    pub const MAX_CONTROL_ITER: usize = 55;
    pub const ALLOCATION_FACTORS: usize = 48;
    pub const CASE_NAME: usize = 63;
    pub const LOG: usize = 66;
    pub const NUM_ALLOC_ITERATIONS: usize = 72;
    pub const DEFAULT_BASE_FREQUENCY: usize = 73;
    pub const NEGLECT_LOAD_Y: usize = 95;
    pub const MIN_ITERATIONS: usize = 110;
    pub const REDUCE_OPTION: usize = 59;
    pub const KEEP_LOAD: usize = 112;
    pub const ZMAG: usize = 113;
}

/// The DSS engine context (`TDSSContext`).
pub struct Dss {
    classes: Vec<DssClass>,
    /// Lowercased class name → index (Pascal `ClassNames`).
    class_by_name: HashMap<String, usize>,
    commands: CommandList,
    option_list: CommandList,
    /// Main parser driving the command/edit loop (`DSS.Parser`).
    parser: Parser,
    /// Scratch parser for property values (`DSS.AuxParser`/`PropParser`).
    aux_parser: Parser,
    vars: ParserVars,
    enums: EnumRegistry,
    /// Accumulated `DoSimpleMsg` log (record-and-continue errors).
    errors: Vec<String>,
    active_class: Option<usize>,
    /// `DSS.GlobalResult`: the value returned by `?` queries and `Get`.
    last_result: String,
    /// The active circuit (`DSS.ActiveCircuit`; `MaxCircuits = 1`).
    circuit: Option<Circuit>,
    /// `DSS.DefaultBaseFreq` (`Set DefaultBaseFrequency=`).
    default_base_freq: f64,
    /// `DSS.MaxAllocationIterations` (`Set NumAllocIterations=`); default 2.
    max_allocation_iterations: i32,
    /// `DSS.CurrentDSSDir`: base for resolving relative script paths.
    current_dir: PathBuf,
    /// `DSS.In_Redirect` / `DSS.Redirect_Abort`.
    in_redirect: bool,
    redirect_abort: bool,
}

impl Dss {
    pub fn new() -> Self {
        let enums = EnumRegistry::new();
        let commands = CommandList::new(EXEC_COMMANDS.iter().copied());
        let option_list = CommandList::new(EXEC_OPTIONS.iter().copied());

        // Class registry. More classes are registered here as they are ported.
        let classes = vec![
            DssClass::dss_object(tcc_curve::class_props(&enums), |name| {
                Box::new(tcc_curve::TccCurveObj::new(name))
            }),
            DssClass::dss_object(spectrum::class_props(&enums), |name| {
                Box::new(spectrum::SpectrumObj::new(name))
            }),
            DssClass::dss_object(line_code::class_props(&enums), |name| {
                Box::new(line_code::LineCodeObj::new(name))
            }),
            DssClass::dss_object(growth_shape::class_props(), |name| {
                Box::new(growth_shape::GrowthShapeObj::new(name))
            }),
            DssClass::dss_object(xfmr_code::class_props(&enums), |name| {
                Box::new(xfmr_code::XfmrCodeObj::new(name))
            }),
            DssClass::dss_object(xy_curve::class_props(&enums), |name| {
                Box::new(xy_curve::XyCurveObj::new(name))
            }),
            DssClass::dss_object(load_shape::class_props(&enums), |name| {
                Box::new(load_shape::LoadShapeObj::new(name))
            }),
            DssClass::dss_object(temp_shape::class_props(&enums), |name| {
                Box::new(temp_shape::TShapeObj::new(name))
            }),
            DssClass::dss_object(price_shape::class_props(&enums), |name| {
                Box::new(price_shape::PriceShapeObj::new(name))
            }),
            // Conductor catalog (Pascal DSSClassDefs.pas: WireData, CNData,
            // TSData register after Spectrum, before LineGeometry).
            DssClass::dss_object(conductor_data::wire_data::class_props(&enums), |name| {
                Box::new(conductor_data::WireDataObj::new(name))
            }),
            DssClass::dss_object(conductor_data::cn_data::class_props(&enums), |name| {
                Box::new(conductor_data::CnDataObj::new(name))
            }),
            DssClass::dss_object(conductor_data::ts_data::class_props(&enums), |name| {
                Box::new(conductor_data::TsDataObj::new(name))
            }),
            // LineSpacing registers after TSData, before LineGeometry
            // (Pascal DSSClassDefs.pas).
            DssClass::dss_object(line_spacing::class_props(&enums), |name| {
                Box::new(line_spacing::LineSpacingObj::new(name))
            }),
            // LineGeometry registers after LineSpacing (Pascal DSSClassDefs.pas).
            DssClass::dss_object(line_geometry::class_props(&enums), |name| {
                Box::new(line_geometry::LineGeometryObj::new(name))
            }),
            DssClass::ckt_class(
                vsource::class_props(&enums),
                |name| Box::new(vsource::VSource::new(name)),
                ElemKind::Source,
            ),
            DssClass::ckt_class(
                line::class_props(&enums),
                |name| Box::new(line::Line::new(name)),
                ElemKind::Line,
            ),
            DssClass::ckt_class(
                load::class_props(&enums),
                |name| Box::new(load::Load::new(name)),
                ElemKind::Load,
            ),
            DssClass::ckt_class(
                transformer::class_props(&enums),
                |name| Box::new(transformer::Transformer::new(name)),
                ElemKind::Transformer,
            ),
            DssClass::ckt_class(
                capacitor::class_props(&enums),
                |name| Box::new(capacitor::Capacitor::new(name)),
                ElemKind::Capacitor,
            ),
            DssClass::ckt_class(
                reactor::class_props(&enums),
                |name| Box::new(reactor::Reactor::new(name)),
                ElemKind::Reactor,
            ),
            DssClass::ckt_class(
                reg_control::class_props(&enums),
                |name| Box::new(reg_control::RegControl::new(name)),
                ElemKind::Control,
            ),
            DssClass::ckt_class(
                cap_control::class_props(&enums),
                |name| Box::new(cap_control::CapControl::new(name)),
                ElemKind::Control,
            ),
            DssClass::ckt_class(
                generator::class_props(&enums),
                |name| Box::new(generator::Generator::new(name)),
                ElemKind::Generator,
            ),
            // GenDispatcher is registered right after Generator
            // (Pascal DSSClassDefs.pas:231).
            DssClass::ckt_class(
                gen_dispatcher::class_props(&enums),
                |name| Box::new(gen_dispatcher::GenDispatcher::new(name)),
                ElemKind::Control,
            ),
            // StorageController follows GenDispatcher (Pascal DSSClassDefs.pas:237;
            // the Storage element at :234 is Phase 7, so it is skipped here).
            DssClass::ckt_class(
                storage_controller::class_props(&enums),
                |name| Box::new(storage_controller::StorageController::new(name)),
                ElemKind::Control,
            ),
            // Monitor is registered after Generator (Pascal DSSClassDefs.pas:288).
            DssClass::ckt_class(
                monitor::class_props(&enums),
                |name| Box::new(monitor::Monitor::new(name)),
                ElemKind::Meter,
            ),
            DssClass::ckt_class(
                energymeter::class_props(&enums),
                |name| Box::new(energymeter::EnergyMeter::new(name)),
                ElemKind::EnergyMeter,
            ),
            // Sensor is registered after EnergyMeter (Pascal DSSClassDefs.pas:294).
            DssClass::ckt_class(
                sensor::class_props(&enums),
                |name| Box::new(sensor::Sensor::new(name)),
                ElemKind::Sensor,
            ),
        ];
        let class_by_name = classes
            .iter()
            .enumerate()
            .map(|(i, c)| (c.props.class_name().to_lowercase(), i))
            .collect();

        let mut dss = Self {
            classes,
            class_by_name,
            commands,
            option_list,
            parser: Parser::new(),
            aux_parser: Parser::new(),
            vars: ParserVars::new(),
            enums,
            errors: Vec::new(),
            active_class: None,
            last_result: String::new(),
            circuit: None,
            default_base_freq: 60.0,
            max_allocation_iterations: 2,
            current_dir: std::env::current_dir().unwrap_or_default(),
            in_redirect: false,
            redirect_abort: false,
        };
        dss.create_default_dss_items();
        dss
    }

    /// Pascal `TExecutive.CreateDefaultDSSItems`: the default loadshapes,
    /// growthshapes, spectra and TCC curves every context starts with (created
    /// at context creation and again after `Clear`). The command strings are
    /// verbatim from `Executive.pas`.
    fn create_default_dss_items(&mut self) {
        const DEFAULT_ITEMS: &[&str] = &[
            // this load shape used for generator dispatching, etc. Loads may refer to it, also.
            "new loadshape.default npts=24 1.0 mult=(.677 .6256 .6087 .5833 .58028 .6025 .657 .7477 .832 .88 .94 .989 .985 .98 .9898 .999 1 .958 .936 .913 .876 .876 .828 .756)",
            "new growthshape.default 2 year=\"1 20\" mult=(1.025 1.025)", // 20 years at 2.5%
            "new spectrum.default 7  Harmonic=(1 3 5 7 9 11 13)  %mag=(100 33 20 14 11 9 7) Angle=(0 0 0 0 0 0 0)",
            "new spectrum.defaultload 7  Harmonic=(1 3 5 7 9 11 13)  %mag=(100 1.5 20 14 1 9 7) Angle=(0 180 180 180 180 180 180)",
            "new spectrum.defaultgen 7  Harmonic=(1 3 5 7 9 11 13)  %mag=(100 5 3 1.5 1 .7 .5) Angle=(0 0 0 0 0 0 0)",
            "new spectrum.defaultvsource 1  Harmonic=(1 )  %mag=(100 ) Angle=(0 ) ",
            "new spectrum.linear 1  Harmonic=(1 )  %mag=(100 ) Angle=(0 ) ",
            "new spectrum.pwm6 13  Harmonic=(1 3 5 7 9 11 13 15 17 19 21 23 25) %mag=(100 4.4 76.5 62.7 2.9 24.8 12.7 0.5 7.1 8.4 0.9 4.4 3.3) Angle=(-103 -5 28 -180 -33 -59 79 36 -253 -124 3 -30 86)",
            "new spectrum.dc6 10  Harmonic=(1 3 5 7 9 11 13 15 17 19)  %mag=(100 1.2 33.6 1.6 0.4 8.7  1.2  0.3  4.5 1.3) Angle=(-75 28 156 29 -91 49 54 148 -57 -46)",
            "New TCC_Curve.A 5 c_array=(1, 2.5, 4.5, 8.0, 14.)  t_array=(0.15 0.07 .05 .045 .045) ",
            "New TCC_Curve.D 5 c_array=(1, 2.5, 4.5, 8.0, 14.)  t_array=(6 0.7 .2 .06 .02)",
            "New TCC_Curve.TLink 7 c_array=(2 2.1 3 4 6 22 50)  t_array=(300 100 10.1 4.0 1.4 0.1  0.02)",
            "New TCC_Curve.KLink 6 c_array=(2 2.2 3 4 6 30)    t_array=(300 20 4 1.3 0.41 0.02)",
            "New \"TCC_Curve.uv1547\" npts=2 C_array=(0.5, 0.9, ) T_array=(0.166, 2, )",
            "New \"TCC_Curve.ov1547\" npts=2 C_array=(1.1, 1.2, ) T_array=(2, 0.166, )",
            "New \"TCC_Curve.mod_inv\" npts=15 C_array=(1.1, 1.3, 1.5, 2, 3, 4, 5, 6, 7, 8, 9, 10, 20, 50, 100, ) T_array=(27.1053, 9.9029, 6.439, 3.8032, 2.4322, 1.9458, 1.6883, 1.5255, 1.4117, 1.3267, 1.2604, 1.2068, 0.9481, 0.7468, 0.6478, )",
            "New \"TCC_Curve.very_inv\" npts=15 C_array=(1.1, 1.3, 1.5, 2, 3, 4, 5, 6, 7, 8, 9, 10, 20, 50, 100, ) T_array=(93.872, 28.9113, 16.179, 7.0277, 2.9423, 1.7983, 1.3081, 1.0513, 0.8995, 0.8023, 0.7361, 0.6891, 0.5401, 0.4988, 0.493, )",
            "New \"TCC_Curve.ext_inv\" npts=15 C_array=(1.1, 1.3, 1.5, 2, 3, 4, 5, 6, 7, 8, 9, 10, 20, 50, 100, ) T_array=(134.4074, 40.9913, 22.6817, 9.5217, 3.6467, 2.0017, 1.2967, 0.9274, 0.7092, 0.5693, 0.4742, 0.4065, 0.1924, 0.133, 0.1245, )",
            "New \"TCC_Curve.definite\" npts=3 C_array=(1, 1.001, 100, ) T_array=(300, 1, 1, )",
        ];
        for cmd in DEFAULT_ITEMS {
            self.command(cmd);
        }
        debug_assert!(
            self.errors.is_empty(),
            "default DSS items must parse cleanly: {:?}",
            self.errors
        );
    }

    /// Accumulated error messages (`DoSimpleMsg` log).
    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    /// The most recent query/`Get` result (`DSS.GlobalResult`).
    pub fn result(&self) -> &str {
        &self.last_result
    }

    /// The active circuit, if `New circuit.` has run.
    pub fn circuit(&self) -> Option<&Circuit> {
        self.circuit.as_ref()
    }

    pub fn circuit_mut(&mut self) -> Option<&mut Circuit> {
        self.circuit.as_mut()
    }
}

impl Default for Dss {
    fn default() -> Self {
        Self::new()
    }
}

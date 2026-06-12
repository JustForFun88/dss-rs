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

use std::collections::HashMap;
use std::path::PathBuf;

use dss_parser::{Parser, ParserVars};

use crate::circuit::{Circuit, ElemKind};
use crate::elements::control::{cap_control, reg_control};
use crate::elements::general::{growth_shape, line_code, spectrum, tcc_curve, xfmr_code, xy_curve};
use crate::elements::pc::{load, vsource};
use crate::elements::pd::{capacitor, line, reactor, transformer};
use crate::elements::traits::{CktElement, ElemRef, ElemStore};
use crate::obj::base::DssObject;
use crate::obj::dss_enum::{EnumId, EnumRegistry};
use crate::obj::props::{ClassProps, ForeignClassesView, PropEngine, PropType};
use crate::solution::{SolveEnv, SolveMode, set_voltage_bases, solve};
use crate::support::command_list::CommandList;
use crate::util::{float_to_str, interpret_yes_no, parse_object_class_and_name};

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
    pub const CLASSES: usize = 49;
    pub const USERCLASSES: usize = 50;
    pub const ALIGN_FILE: usize = 63;
    pub const DI_PLOT: usize = 69;
    pub const COMPARE_CASES: usize = 70;
    pub const YEARLY_CURVES: usize = 71;
    pub const CD: usize = 72;
    pub const DOSCMD: usize = 75;
    pub const CVRT_LOADSHAPES: usize = 88;
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
    pub const YEAR: usize = 5;
    pub const FREQUENCY: usize = 6;
    pub const MODE: usize = 8;
    pub const RANDOM: usize = 9;
    pub const NUMBER: usize = 10;
    pub const TOLERANCE: usize = 16;
    pub const MAXITERATIONS: usize = 17;
    pub const LOADMODEL: usize = 19;
    pub const LOADMULT: usize = 20;
    pub const NORMVMINPU: usize = 21;
    pub const NORMVMAXPU: usize = 22;
    pub const EMERGVMINPU: usize = 23;
    pub const EMERGVMAXPU: usize = 24;
    pub const PCT_GROWTH: usize = 28;
    pub const ALLOW_DUPLICATES: usize = 33;
    pub const ZONE_LOCK: usize = 34;
    pub const VOLTAGE_BASES: usize = 39;
    pub const ALGORITHM: usize = 40;
    pub const CONTROL_MODE: usize = 43;
    pub const CKT_MODEL: usize = 49;
    pub const BASE_FREQUENCY: usize = 53;
    pub const MAX_CONTROL_ITER: usize = 55;
    pub const CASE_NAME: usize = 63;
    pub const LOG: usize = 66;
    pub const DEFAULT_BASE_FREQUENCY: usize = 73;
    pub const NEGLECT_LOAD_Y: usize = 95;
    pub const MIN_ITERATIONS: usize = 110;
}

/// A class constructor: build a fresh, all-default object of the class.
type NewObjectFn = fn(&str) -> Box<dyn DssObject>;

/// One registered class plus its live objects — the Rust stand-in for a
/// `TDSSClass` together with its `ElementList`/`ElementNameList`.
struct DssClass {
    props: ClassProps,
    new_object: NewObjectFn,
    objects: Vec<Box<dyn DssObject>>,
    /// Lowercased object name → index (Pascal `ElementNameList`, THashList).
    name_to_idx: HashMap<String, usize>,
    /// Active object index (`ActiveElement`).
    active: Option<usize>,
    /// Pascal `TDSSClass.RequiresCircuit` (circuit-element classes).
    requires_circuit: bool,
    /// Which circuit list the elements join (`DSSObjType` class mask).
    kind: Option<ElemKind>,
}

impl DssClass {
    fn dss_object(props: ClassProps, new_object: NewObjectFn) -> Self {
        Self {
            props,
            new_object,
            objects: Vec::new(),
            name_to_idx: HashMap::new(),
            active: None,
            requires_circuit: false,
            kind: None,
        }
    }

    fn ckt_class(props: ClassProps, new_object: NewObjectFn, kind: ElemKind) -> Self {
        Self {
            props,
            new_object,
            objects: Vec::new(),
            name_to_idx: HashMap::new(),
            active: None,
            requires_circuit: true,
            kind: Some(kind),
        }
    }

    /// Pascal `SetActive`: make the named object active; returns whether it
    /// existed.
    fn set_active(&mut self, name: &str) -> bool {
        match self.name_to_idx.get(&name.to_lowercase()) {
            Some(&idx) => {
                self.active = Some(idx);
                true
            }
            None => false,
        }
    }
}

/// [`ElemStore`] view over the class registry — the bridge the solution
/// machinery walks instead of Pascal's pointer lists.
struct ClassStore<'a> {
    classes: &'a mut [DssClass],
}

impl ElemStore for ClassStore<'_> {
    fn ckt_elem(&self, r: ElemRef) -> &dyn CktElement {
        self.classes[r.cls].objects[r.idx]
            .as_ckt_element()
            .expect("ElemRef must point at a circuit element")
    }
    fn ckt_elem_mut(&mut self, r: ElemRef) -> &mut dyn CktElement {
        self.classes[r.cls].objects[r.idx]
            .as_ckt_element_mut()
            .expect("ElemRef must point at a circuit element")
    }
}

/// A read view of every class *except* the one being edited (the active class
/// is the excluded middle element), the [`ForeignClassesView`] the property
/// engine uses to resolve `ObjectRef` values mid-edit (PHASE4_PLAN §3.1).
struct ForeignClasses<'a> {
    /// `classes[..ci]` — global class index == slice index.
    left: &'a [DssClass],
    /// `classes[ci + 1..]` — global class index == `split + 1 + slice index`.
    right: &'a [DssClass],
    /// `ci`, the active class's global index.
    split: usize,
}

impl<'a> ForeignClasses<'a> {
    /// Resolve a (class name, object name) pair to its global [`ElemRef`] plus
    /// the live object, scanning both halves. A class match with no object
    /// match short-circuits to `None`, like `cls.Find` returning NIL.
    fn lookup(&self, class: &str, name_l: &str) -> Option<(ElemRef, &'a dyn DssObject)> {
        let find_in = |c: &'a DssClass, cls: usize| {
            c.name_to_idx
                .get(name_l)
                .map(|&idx| (ElemRef { cls, idx }, c.objects[idx].as_ref()))
        };
        let left = self.left;
        for (k, c) in left.iter().enumerate() {
            if c.props.class_name().eq_ignore_ascii_case(class) {
                return find_in(c, k);
            }
        }
        let right = self.right;
        for (k, c) in right.iter().enumerate() {
            if c.props.class_name().eq_ignore_ascii_case(class) {
                return find_in(c, self.split + 1 + k);
            }
        }
        None
    }
}

impl<'a> ForeignClassesView<'a> for ForeignClasses<'a> {
    fn find(&self, class: &str, name: &str) -> Option<(ElemRef, &'a dyn DssObject)> {
        self.lookup(class, &name.to_lowercase())
    }

    /// Pascal `GetCktElementIndex`: resolve a full `Class.Name` reference (the
    /// `PropertyOffset2 = 0` object-ref case, e.g. CapControl `element=`). The
    /// returned `String` is the canonical `FullName` for dumps.
    fn find_full(&self, full_name: &str) -> Option<(ElemRef, &'a dyn DssObject, String)> {
        let dot = full_name.find('.')?;
        let (class, name) = (&full_name[..dot], &full_name[dot + 1..]);
        // Reuse the per-class lookup, then rebuild the canonical FullName.
        let (r, obj) = self.lookup(class, &name.to_lowercase())?;
        let cls = if r.cls < self.split {
            &self.left[r.cls]
        } else {
            &self.right[r.cls - self.split - 1]
        };
        Some((
            r,
            obj,
            format!("{}.{}", cls.props.class_name(), obj.data().name()),
        ))
    }
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
        ];
        let class_by_name = classes
            .iter()
            .enumerate()
            .map(|(i, c)| (c.props.class_name().to_lowercase(), i))
            .collect();

        Self {
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
            current_dir: std::env::current_dir().unwrap_or_default(),
            in_redirect: false,
            redirect_abort: false,
        }
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

    /// Process one command line (Pascal `ProcessCommand`). Errors are recorded
    /// in [`Dss::errors`] (record-and-continue); query results land in
    /// [`Dss::result`].
    pub fn command(&mut self, cmd_line: &str) {
        self.last_result.clear(); // DSS.GlobalResult := ''
        self.parser.set_auto_increment(false);
        self.parser.set_cmd_string(cmd_line);
        let param_name = self.parser.next_param(&self.vars);
        let param = self.parser.make_string(&self.vars);
        if param.is_empty() {
            return; // Skip blank line
        }

        // Commands do not have equal signs, so ParamName must be empty.
        let pointer = if param_name.is_empty() {
            self.commands
                .get_command(&param)
                .map(|i| i + 1)
                .unwrap_or(0)
        } else {
            0
        };

        // Check first for Compile or Redirect and get outta here.
        if pointer == cmd::COMPILE || pointer == cmd::REDIRECT {
            self.do_redirect(pointer == cmd::COMPILE);
            return;
        }

        // Things that are OK to do before a circuit is defined.
        match pointer {
            cmd::NEW => {
                self.do_new_cmd();
                return;
            }
            cmd::SET if self.circuit.is_none() => {
                self.do_set_cmd_no_circuit();
                return;
            }
            cmd::GET if self.circuit.is_none() => {
                self.do_get_cmd_no_circuit();
                return;
            }
            cmd::COMMENT | cmd::HELP | cmd::QUIT | cmd::PANEL | cmd::ABOUT | cmd::COMHELP => {
                return; // no-ops (comment / GUI-only commands)
            }
            cmd::CLEAR | cmd::CLEAR_ALL => {
                self.do_clear_cmd();
                return;
            }
            cmd::FILEEDIT
            | cmd::CLASSES
            | cmd::USERCLASSES
            | cmd::ALIGN_FILE
            | cmd::DI_PLOT
            | cmd::COMPARE_CASES
            | cmd::YEARLY_CURVES
            | cmd::CD
            | cmd::DOSCMD
            | cmd::CVRT_LOADSHAPES
            | cmd::VAR => {
                self.not_ported_command(pointer);
                return;
            }
            0 => {} // possibly a property reference; checked below
            _ => {
                if self.circuit.is_none() {
                    self.errors.push(
                        "You must create a new circuit object first: \"new circuit.mycktname\" to execute this command."
                            .to_string(),
                    );
                    return;
                }
            }
        }

        // Not a command: it could be a property of the active circuit element.
        if pointer == 0 {
            if param_name.is_empty() || param_name.eq_ignore_ascii_case("command") {
                self.errors.push(format!("Unknown Command: \"{param}\""));
            } else {
                let (obj_name, prop_name) = parse_obj_name(&param_name);
                if !obj_name.is_empty() && !self.set_object(&obj_name) {
                    return;
                }
                // Rebuild the command line and pass it to the editor; quotes
                // ensure the first parameter is interpreted OK after rebuild.
                let remainder = self.parser.remainder().to_string();
                self.parser
                    .set_cmd_string(&format!("{prop_name}=\"{param}\" {remainder}"));
                self.edit_active();
            }
            return;
        }

        // Process the rest of the commands (circuit exists at this point).
        match pointer {
            cmd::EDIT => self.do_edit_cmd(),
            cmd::MORE | cmd::M | cmd::TILDE => self.edit_active(),
            cmd::SOLVE => self.do_set_cmd(1), // Solve = Set + DoSolveCmd
            cmd::SET => self.do_set_cmd(0),
            cmd::QUERY => self.do_query_cmd(),
            cmd::CALC_VOLTAGE_BASES => self.do_calc_voltage_bases(),
            cmd::BUILD_Y => self.do_build_y(),
            cmd::GET => self.do_get_cmd(),
            cmd::INIT => {
                if let Some(ckt) = self.circuit.as_mut() {
                    ckt.solution.solution_initialized = false;
                }
            }
            _ => self.not_ported_command(pointer),
        }
    }

    fn not_ported_command(&mut self, pointer: usize) {
        let name = EXEC_COMMANDS.get(pointer - 1).copied().unwrap_or("?");
        self.errors
            .push(format!("Command \"{name}\" is not ported in Phase 3."));
    }

    /// Pascal `GetObjClassAndName`: read the `class.name` token (optionally
    /// prefixed `object=`) from the main parser.
    fn get_obj_class_and_name(&mut self) -> (String, String) {
        let param_name = self.parser.next_param(&self.vars).to_lowercase();
        let param = self.parser.make_string(&self.vars);
        if !param_name.is_empty() && !crate::util::compare_text_shortest_eq(&param_name, "object") {
            self.errors
                .push("object=Class.Name expected as first parameter in command.".to_string());
            return (String::new(), String::new());
        }
        parse_object_class_and_name(&mut self.parser, &self.vars, &param)
    }

    /// Pascal `DSSGlobals.SetObject`: set the active object by `class.name`
    /// (or bare `name` against the active class).
    fn set_object(&mut self, param: &str) -> bool {
        let (class_part, name_part) = match param.find('.') {
            Some(p) => (param[..p].to_string(), param[p + 1..].to_string()),
            None => (String::new(), param.to_string()),
        };
        let ci = if class_part.is_empty() {
            self.active_class
        } else {
            self.class_by_name.get(&class_part.to_lowercase()).copied()
        };
        let Some(ci) = ci else {
            self.errors
                .push(format!("Error! Object \"{param}\" not found."));
            return false;
        };
        self.active_class = Some(ci);
        if !self.classes[ci].set_active(&name_part) {
            self.errors
                .push(format!("Error! Object \"{param}\" not found."));
            return false;
        }
        true
    }

    /// Pascal `DoNewCmd` → `MakeNewCircuit` / `AddObject`.
    fn do_new_cmd(&mut self) {
        let (obj_class, obj_name) = self.get_obj_class_and_name();
        if obj_class.eq_ignore_ascii_case("solution") {
            self.errors.push(
                "You cannot create new Solution objects through the command interface.".to_string(),
            );
            return;
        }
        if obj_class.eq_ignore_ascii_case("circuit") {
            self.make_new_circuit(&obj_name);
            return;
        }
        self.add_object(&obj_class, &obj_name);
    }

    /// Pascal `DSSGlobals.MakeNewCircuit`: create the circuit, then feed the
    /// remainder of the line to the default source —
    /// `New object=vsource.source Bus1=SourceBus <remainder>`.
    fn make_new_circuit(&mut self, name: &str) {
        if self.circuit.is_some() {
            // Pascal error 906; MaxCircuits is 1 in this build.
            self.errors.push(
                "MakeNewCircuit: Cannot create new circuit. Max. Circuits Exceeded. (Max no. of circuits=1)"
                    .to_string(),
            );
            return;
        }
        self.circuit = Some(Circuit::new(name, self.default_base_freq));
        let s = self.parser.remainder().to_string();
        self.command(&format!("New object=vsource.source Bus1=SourceBus {s}"));
    }

    /// Pascal `DoEditCmd` → `EditObject`.
    fn do_edit_cmd(&mut self) {
        let (obj_class, obj_name) = self.get_obj_class_and_name();
        if obj_class.eq_ignore_ascii_case("circuit") {
            return; // Do nothing if editing Circuit
        }
        let Some(&ci) = self.class_by_name.get(&obj_class.to_lowercase()) else {
            self.errors.push(format!(
                "Edit Command: Object Type \"{obj_class}\" not found."
            ));
            return;
        };
        self.active_class = Some(ci);
        if self.classes[ci].set_active(&obj_name) {
            self.edit_active();
        }
    }

    /// Pascal `AddObject`: create the object (or make the existing one
    /// active for `DSS_OBJECT` classes), register circuit elements with the
    /// circuit, and edit the rest of the line.
    fn add_object(&mut self, obj_class: &str, name: &str) {
        let Some(&ci) = self.class_by_name.get(&obj_class.to_lowercase()) else {
            self.errors.push(format!(
                "New Command: Object Type \"{obj_class}\" not found."
            ));
            return;
        };
        self.active_class = Some(ci);

        if name.is_empty() {
            self.errors.push("Object Name Missing".to_string());
            return;
        }

        if self.classes[ci].requires_circuit && self.circuit.is_none() {
            self.errors
                .push("You Must Create a circuit first: \"new circuit.yourcktname\"".to_string());
            return;
        }

        if !self.classes[ci].requires_circuit {
            // DSS_OBJECT path: duplicates become edits.
            if !self.classes[ci].set_active(name) {
                let cls = &mut self.classes[ci];
                let obj = (cls.new_object)(name);
                let idx = cls.objects.len();
                cls.name_to_idx.insert(obj.data().name().to_string(), idx);
                cls.objects.push(obj);
                cls.active = Some(idx);
            }
            self.edit_active();
            return;
        }

        // Circuit-element path. Duplicate names: warn and bail (the Pascal
        // exits without editing when DuplicatesAllowed is off).
        let duplicates_allowed = self.circuit.as_ref().is_some_and(|c| c.duplicates_allowed);
        if !duplicates_allowed && self.classes[ci].set_active(name) {
            self.errors.push(format!(
                "Warning: Duplicate new element definition: \"{}.{}\". Element being redefined.",
                self.classes[ci].props.class_name(),
                name
            ));
            return;
        }

        let cls = &mut self.classes[ci];
        let obj = (cls.new_object)(name);
        let idx = cls.objects.len();
        cls.name_to_idx.insert(obj.data().name().to_string(), idx);
        cls.objects.push(obj);
        cls.active = Some(idx);

        let kind = cls.kind.expect("circuit element class has a kind");
        let ckt = self.circuit.as_mut().expect("checked above");
        let elem = self.classes[ci].objects[idx]
            .as_ckt_element_mut()
            .expect("circuit element class builds circuit elements");
        ckt.add_ckt_element(ElemRef { cls: ci, idx }, kind, elem);

        self.edit_active();
    }

    /// Pascal `DoClearCmd`: drop the circuit and every object.
    fn do_clear_cmd(&mut self) {
        for cls in &mut self.classes {
            cls.objects.clear();
            cls.name_to_idx.clear();
            cls.active = None;
        }
        self.active_class = None;
        self.circuit = None;
        self.errors.clear();
    }

    /// Pascal `DoQueryCmd`: `? class.obj.prop` → store the property value in
    /// [`Dss::last_result`].
    fn do_query_cmd(&mut self) {
        self.parser.next_param(&self.vars);
        let full = self.parser.make_string(&self.vars);
        let (obj_name, prop_name) = parse_obj_name(&full);

        let (class_name, name) = {
            // Reuse the class.name splitter (no @var work needed on a query).
            let mut p = Parser::new();
            parse_object_class_and_name(&mut p, &self.vars, &obj_name)
        };

        self.last_result = "Property Unknown".to_string();
        let Some(&ci) = self.class_by_name.get(&class_name.to_lowercase()) else {
            self.errors
                .push(format!("Error! Object \"{obj_name}\" not found."));
            return;
        };
        self.active_class = Some(ci);
        if !self.classes[ci].set_active(&name) {
            self.errors
                .push(format!("Error! Object \"{obj_name}\" not found."));
            return;
        }
        let cls = &self.classes[ci];
        let oi = cls.active.expect("just set active");
        if let Some(idx) = cls.props.property_index(&prop_name) {
            self.last_result = cls
                .props
                .get_value(cls.objects[oi].as_ref(), idx, &self.enums);
        }
    }

    /// The body of Pascal `TDSSClass.Edit`: iterate `name=value` parameters on
    /// the main parser against the active object, then `EndEdit`, then
    /// propagate the element's signal flags to the circuit (the Pascal set
    /// `ActiveCircuit.BusNameRedefined`/`Solution.SystemYChanged` directly
    /// from the property setters; nothing reads them mid-edit, so polling
    /// after the edit is equivalent).
    fn edit_active(&mut self) {
        let Some(ci) = self.active_class else {
            self.errors
                .push("There is no active element to edit.".to_string());
            return;
        };
        let Dss {
            classes,
            circuit,
            parser,
            aux_parser,
            vars,
            enums,
            errors,
            ..
        } = self;
        // Split the registry so the active class is borrowed mutably for the
        // edit while every *other* class is a read view for ObjectRef
        // resolution (PHASE4_PLAN §3.1). `split_at_mut` + `split_first_mut`
        // keep the three regions provably disjoint with no unsafe.
        let (left, rest) = classes.split_at_mut(ci);
        let (active_class, right) = rest.split_first_mut().expect("ci is in range");
        let foreign = ForeignClasses {
            left,
            right,
            split: ci,
        };
        let DssClass {
            props,
            objects,
            name_to_idx,
            active,
            ..
        } = active_class;
        let Some(oi) = *active else {
            errors.push("There is no active element to edit.".to_string());
            return;
        };

        let mut param_pointer: i64 = 0;
        let mut param_name = parser.next_param(vars);
        let mut param = parser.make_string(vars);
        while !param.is_empty() {
            if param_name.is_empty() {
                param_pointer += 1;
            } else {
                param_pointer = props
                    .property_index(&param_name)
                    .map(|i| i as i64)
                    .unwrap_or(0);
            }

            if param_pointer <= 0 || param_pointer as usize > props.num_properties() {
                if param_name.is_empty() {
                    errors.push(format!(
                        "Unknown parameter for value \"{param}\" in object \"{}.{}\"",
                        props.class_name(),
                        objects[oi].data().name()
                    ));
                } else {
                    errors.push(format!(
                        "Unknown parameter \"{param_name}\" (value \"{param}\") for object \"{}.{}\"",
                        props.class_name(),
                        objects[oi].data().name()
                    ));
                }
            } else {
                let idx = param_pointer as usize;
                if props.prop(idx).ptype == PropType::MakeLike {
                    make_like(objects, name_to_idx, oi, &param, errors, props.class_name());
                    objects[oi].data_mut().set_as_next_seq(idx);
                    objects[oi].side_effects(idx, 0);
                } else {
                    let mut eng = PropEngine {
                        parser: aux_parser,
                        vars,
                        enums,
                        errors,
                        foreign: Some(&foreign),
                    };
                    if let Err(e) = props.edit_property(objects[oi].as_mut(), idx, &param, &mut eng)
                    {
                        errors.push(e.message().to_string());
                    }
                }
            }

            param_name = parser.next_param(vars);
            param = parser.make_string(vars);
        }

        objects[oi].end_edit();

        // Drain any `DoSimpleMsg`/`DoErrorMsg` queued by the property hooks
        // (e.g. `LineCode.Kron` on a 1-phase code) into the engine error log.
        let deferred = objects[oi].data_mut().take_errors();
        errors.extend(deferred);

        // Deferred cross-element writes (Pascal pokes the target through a
        // live pointer mid-parse, e.g. RegControl `TapNum` → the transformer's
        // PresentTap; nothing reads the target in between, so applying after
        // the edit is equivalent). Collected before the flag propagation so
        // the active-class borrows can end before `classes` is re-borrowed.
        let ref_actions = objects[oi].take_ref_actions();

        // Signal-flag propagation (Pascal `Set_Bus`/`Set_Enabled` write the
        // circuit globals immediately; `Set_YprimInvalid` raises
        // `SystemYChanged` for enabled elements).
        if let Some(ckt) = circuit.as_mut()
            && let Some(elem) = objects[oi].as_ckt_element_mut()
        {
            let cd = elem.cd_mut();
            if cd.signal_bus_name_redefined {
                cd.signal_bus_name_redefined = false;
                ckt.set_bus_name_redefined(true);
            }
            if cd.yprim_invalid && cd.enabled {
                ckt.solution.system_y_changed = true;
            }
        }

        for action in &ref_actions {
            let target = action.target();
            let tgt = &mut classes[target.cls].objects[target.idx];
            tgt.apply_ref_action(action);
            // Propagate the target's flags too (a tap change invalidates the
            // transformer's Yprim exactly like a direct `Tap=` edit).
            if let Some(ckt) = circuit.as_mut()
                && let Some(elem) = tgt.as_ckt_element_mut()
            {
                let cd = elem.cd_mut();
                if cd.signal_bus_name_redefined {
                    cd.signal_bus_name_redefined = false;
                    ckt.set_bus_name_redefined(true);
                }
                if cd.yprim_invalid && cd.enabled {
                    ckt.solution.system_y_changed = true;
                }
            }
        }
    }

    /// Pascal `DoSetCmd(SolveOption)`: parse `option=value` pairs, then run
    /// the solve when called from the `Solve` command.
    fn do_set_cmd(&mut self, solve_option: i32) {
        if self.circuit.is_none() {
            self.do_set_cmd_no_circuit();
            return;
        }
        {
            let Dss {
                circuit,
                option_list,
                parser,
                vars,
                enums,
                errors,
                default_base_freq,
                ..
            } = self;
            let ckt = circuit.as_mut().expect("checked above");

            let mut pointer: usize = 0;
            let mut param_name = parser.next_param(vars);
            let mut param = parser.make_string(vars);
            while !param.is_empty() {
                if param_name.is_empty() {
                    pointer += 1;
                } else {
                    pointer = option_list
                        .get_command(&param_name)
                        .map(|i| i + 1)
                        .unwrap_or(0);
                }

                match pointer {
                    0 => errors.push(format!(
                        "Unknown parameter \"{param_name}\" for Set Command"
                    )),
                    opt::YEAR => {
                        if let Some(v) = get_int(parser, vars, errors) {
                            ckt.solution.year = v;
                            ckt.default_growth_factor =
                                ckt.default_growth_rate.powi(ckt.solution.year - 1);
                        }
                    }
                    opt::FREQUENCY => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.solution.set_frequency(v);
                        }
                    }
                    opt::MODE => {
                        if let Some(v) = enum_ord(enums, enums.solve_mode, &param, errors) {
                            ckt.solution.set_mode(SolveMode::from_ordinal(v));
                        }
                    }
                    opt::RANDOM => {
                        if let Some(v) = enum_ord(enums, enums.random_mode, &param, errors) {
                            ckt.solution.random_type = v;
                        }
                    }
                    opt::NUMBER => {
                        if let Some(v) = get_int(parser, vars, errors) {
                            ckt.solution.number_of_times = v;
                        }
                    }
                    opt::TOLERANCE => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.solution.convergence_tolerance = v;
                        }
                    }
                    opt::MAXITERATIONS => {
                        if let Some(v) = get_int(parser, vars, errors) {
                            ckt.solution.max_iterations = v;
                        }
                    }
                    opt::LOADMODEL => {
                        if let Some(v) = enum_ord(enums, enums.default_load_model, &param, errors) {
                            ckt.solution.default_load_model = v;
                            ckt.solution.load_model = v;
                        }
                    }
                    opt::LOADMULT => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.load_multiplier = v;
                            ckt.solution.system_y_changed = true;
                        }
                    }
                    opt::NORMVMINPU => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.normal_min_volts = v;
                        }
                    }
                    opt::NORMVMAXPU => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.normal_max_volts = v;
                        }
                    }
                    opt::EMERGVMINPU => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.emerg_min_volts = v;
                        }
                    }
                    opt::EMERGVMAXPU => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.emerg_max_volts = v;
                        }
                    }
                    opt::PCT_GROWTH => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.default_growth_rate = 1.0 + v / 100.0;
                            ckt.default_growth_factor =
                                ckt.default_growth_rate.powi(ckt.solution.year - 1);
                        }
                    }
                    opt::ALLOW_DUPLICATES => ckt.duplicates_allowed = interpret_yes_no(&param),
                    opt::ZONE_LOCK => ckt.zones_locked = interpret_yes_no(&param),
                    opt::VOLTAGE_BASES => {
                        // Pascal `DoLegalVoltageBases` (1000-slot buffer).
                        let mut buf = vec![0.0; 1000];
                        match parser.parse_as_vector(vars, &mut buf, false) {
                            Ok(n) => {
                                buf.truncate(n.min(1000));
                                ckt.legal_voltage_bases = buf;
                            }
                            Err(e) => errors.push(e.message().to_string()),
                        }
                    }
                    opt::ALGORITHM => {
                        if let Some(v) = enum_ord(enums, enums.solve_alg, &param, errors) {
                            ckt.solution.algorithm = v;
                        }
                    }
                    opt::CONTROL_MODE => {
                        if let Some(v) = enum_ord(enums, enums.control_mode, &param, errors) {
                            ckt.solution.control_mode = v;
                            // always revert to last one specified in a script
                            ckt.solution.default_control_mode = v;
                        }
                    }
                    opt::CKT_MODEL => {
                        if let Some(v) = enum_ord(enums, enums.ckt_model, &param, errors) {
                            ckt.positive_sequence = v != 0;
                        }
                    }
                    opt::BASE_FREQUENCY => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.fundamental = v; // Set Base Frequency for system
                            ckt.solution.set_frequency(v);
                        }
                    }
                    opt::MAX_CONTROL_ITER => {
                        if let Some(v) = get_int(parser, vars, errors) {
                            ckt.solution.max_control_iterations = v;
                        }
                    }
                    opt::CASE_NAME => ckt.case_name = param.clone(),
                    opt::LOG => ckt.log_events = interpret_yes_no(&param),
                    opt::DEFAULT_BASE_FREQUENCY => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            *default_base_freq = v;
                            ckt.fundamental = v;
                            ckt.solution.set_frequency(v);
                        }
                    }
                    opt::NEGLECT_LOAD_Y => ckt.neglect_load_y = interpret_yes_no(&param),
                    opt::MIN_ITERATIONS => {
                        if let Some(v) = get_int(parser, vars, errors) {
                            ckt.solution.min_iterations = v;
                        }
                    }
                    _ => {
                        let name = EXEC_OPTIONS.get(pointer - 1).copied().unwrap_or("?");
                        errors.push(format!("Set option \"{name}\" is not ported in Phase 3."));
                    }
                }

                param_name = parser.next_param(vars);
                param = parser.make_string(vars);
            }
        }

        if solve_option == 1 {
            self.do_solve_cmd();
        }
    }

    /// Pascal `DoSetCmd_NoCircuit`: the few global options legal without a
    /// circuit; anything else is the error-301 message.
    fn do_set_cmd_no_circuit(&mut self) {
        let Dss {
            option_list,
            parser,
            vars,
            errors,
            default_base_freq,
            ..
        } = self;
        let mut pointer: usize = 0;
        let mut param_name = parser.next_param(vars);
        let mut param = parser.make_string(vars);
        while !param.is_empty() {
            if param_name.is_empty() {
                pointer += 1;
            } else {
                pointer = option_list
                    .get_command(&param_name)
                    .map(|i| i + 1)
                    .unwrap_or(0);
            }
            match pointer {
                0 => errors.push(format!(
                    "Unknown parameter \"{param_name}\" for Set Command"
                )),
                opt::DEFAULT_BASE_FREQUENCY => {
                    if let Some(v) = get_dbl(parser, vars, errors) {
                        *default_base_freq = v;
                    }
                }
                _ => {
                    errors.push(
                        "You must create a new circuit object first: \"new circuit.mycktname\" to execute this Set command."
                            .to_string(),
                    );
                    return;
                }
            }
            param_name = parser.next_param(vars);
            param = parser.make_string(vars);
        }
    }

    /// Pascal `DoGetCmd`: append the requested option values to
    /// `GlobalResult`, comma-separated.
    fn do_get_cmd(&mut self) {
        let Dss {
            circuit,
            option_list,
            parser,
            vars,
            enums,
            errors,
            default_base_freq,
            last_result,
            ..
        } = self;
        let ckt = circuit.as_mut().expect("gated in command()");
        let mut result = String::new();

        loop {
            let param_name = parser.next_param(vars);
            let param = parser.make_string(vars);
            if param.is_empty() {
                break;
            }
            // Params are themselves the option names to return.
            let pointer = option_list.get_command(&param).map(|i| i + 1).unwrap_or(0);
            match pointer {
                0 => errors.push(format!(
                    "Unknown parameter \"{param_name}\" for Get Command"
                )),
                opt::YEAR => append_result(&mut result, &ckt.solution.year.to_string()),
                opt::FREQUENCY => append_result(&mut result, &float_to_str(ckt.solution.frequency)),
                opt::MODE => append_result(
                    &mut result,
                    &enums
                        .get(enums.solve_mode)
                        .ordinal_to_string(ckt.solution.mode.ordinal()),
                ),
                opt::RANDOM => append_result(
                    &mut result,
                    &enums
                        .get(enums.random_mode)
                        .ordinal_to_string(ckt.solution.random_type),
                ),
                opt::NUMBER => {
                    append_result(&mut result, &ckt.solution.number_of_times.to_string())
                }
                opt::TOLERANCE => append_result(
                    &mut result,
                    &float_to_str(ckt.solution.convergence_tolerance),
                ),
                opt::MAXITERATIONS => {
                    append_result(&mut result, &ckt.solution.max_iterations.to_string())
                }
                opt::LOADMODEL => append_result(
                    &mut result,
                    &enums
                        .get(enums.default_load_model)
                        .ordinal_to_string(ckt.solution.load_model),
                ),
                opt::LOADMULT => append_result(&mut result, &float_to_str(ckt.load_multiplier)),
                opt::NORMVMINPU => append_result(&mut result, &float_to_str(ckt.normal_min_volts)),
                opt::NORMVMAXPU => append_result(&mut result, &float_to_str(ckt.normal_max_volts)),
                opt::EMERGVMINPU => append_result(&mut result, &float_to_str(ckt.emerg_min_volts)),
                opt::EMERGVMAXPU => append_result(&mut result, &float_to_str(ckt.emerg_max_volts)),
                opt::PCT_GROWTH => append_result(
                    &mut result,
                    &float_to_str((ckt.default_growth_rate - 1.0) * 100.0),
                ),
                opt::ALLOW_DUPLICATES => append_result(&mut result, yes_no(ckt.duplicates_allowed)),
                opt::ZONE_LOCK => append_result(&mut result, yes_no(ckt.zones_locked)),
                opt::VOLTAGE_BASES => {
                    // Pascal builds `(b1, b2, ... , )` replacing GlobalResult.
                    result = "(".to_string();
                    for v in &ckt.legal_voltage_bases {
                        result.push_str(&format!("{}, ", float_to_str(*v)));
                    }
                    result.push(')');
                }
                opt::ALGORITHM => append_result(
                    &mut result,
                    &enums
                        .get(enums.solve_alg)
                        .ordinal_to_string(ckt.solution.algorithm),
                ),
                opt::CONTROL_MODE => append_result(
                    &mut result,
                    &enums
                        .get(enums.control_mode)
                        .ordinal_to_string(ckt.solution.control_mode),
                ),
                opt::CKT_MODEL => append_result(
                    &mut result,
                    &enums
                        .get(enums.ckt_model)
                        .ordinal_to_string(ckt.positive_sequence as i32),
                ),
                opt::BASE_FREQUENCY => append_result(&mut result, &float_to_str(ckt.fundamental)),
                opt::MAX_CONTROL_ITER => append_result(
                    &mut result,
                    &ckt.solution.max_control_iterations.to_string(),
                ),
                opt::CASE_NAME => append_result(&mut result, &ckt.case_name),
                opt::LOG => append_result(&mut result, yes_no(ckt.log_events)),
                opt::DEFAULT_BASE_FREQUENCY => {
                    append_result(&mut result, &(default_base_freq.round() as i64).to_string())
                }
                opt::NEGLECT_LOAD_Y => append_result(&mut result, yes_no(ckt.neglect_load_y)),
                opt::MIN_ITERATIONS => {
                    append_result(&mut result, &ckt.solution.min_iterations.to_string())
                }
                _ => {
                    let name = EXEC_OPTIONS.get(pointer - 1).copied().unwrap_or("?");
                    errors.push(format!("Get option \"{name}\" is not ported in Phase 3."));
                }
            }
        }
        *last_result = result;
    }

    /// Pascal `DoGetCmd_NoCircuit`.
    fn do_get_cmd_no_circuit(&mut self) {
        let Dss {
            option_list,
            parser,
            vars,
            errors,
            default_base_freq,
            last_result,
            ..
        } = self;
        let mut result = String::new();
        loop {
            parser.next_param(vars);
            let param = parser.make_string(vars);
            if param.is_empty() {
                break;
            }
            let pointer = option_list.get_command(&param).map(|i| i + 1).unwrap_or(0);
            match pointer {
                opt::DEFAULT_BASE_FREQUENCY => {
                    append_result(&mut result, &(default_base_freq.round() as i64).to_string())
                }
                _ => {
                    errors.push(
                        "You must create a new circuit object first: \"new circuit.mycktname\" to execute this Get command."
                            .to_string(),
                    );
                    return;
                }
            }
        }
        *last_result = result;
    }

    /// Pascal `DoSolveCmd`: `ActiveCircuit.Solution.Solve()`.
    fn do_solve_cmd(&mut self) {
        let Dss {
            classes,
            circuit,
            aux_parser,
            vars,
            errors,
            ..
        } = self;
        let ckt = circuit.as_mut().expect("gated in command()");
        let mut store = ClassStore { classes };
        let mut env = SolveEnv {
            store: &mut store,
            parser: aux_parser,
            vars,
            errors,
        };
        let _ = solve(ckt, &mut env); // hard errors are recorded by solve()
    }

    /// Pascal `DoSetVoltageBases` (the `CalcVoltageBases` command).
    fn do_calc_voltage_bases(&mut self) {
        let Dss {
            classes,
            circuit,
            aux_parser,
            vars,
            errors,
            ..
        } = self;
        let ckt = circuit.as_mut().expect("gated in command()");
        let mut store = ClassStore { classes };
        let mut env = SolveEnv {
            store: &mut store,
            parser: aux_parser,
            vars,
            errors,
        };
        if let Err(e) = set_voltage_bases(ckt, &mut env) {
            env.errors
                .push(format!("Error Encountered in CalcVoltageBases: {e}"));
        }
    }

    /// Pascal `InvalidateAllPCElements` (the `BuildY` command).
    fn do_build_y(&mut self) {
        let Dss {
            classes, circuit, ..
        } = self;
        let ckt = circuit.as_mut().expect("gated in command()");
        for &r in &ckt.pc_elements {
            if let Some(elem) = classes[r.cls].objects[r.idx].as_ckt_element_mut() {
                let cd = elem.cd_mut();
                cd.yprim_invalid = true;
                if cd.enabled {
                    ckt.solution.system_y_changed = true;
                }
            }
        }
    }

    /// Pascal `DoRedirect` (`Redirect`/`Compile`): run a script file line by
    /// line, handling `/* ... */` block comments exactly like the original
    /// (`/*` only recognized at the start of a line; `*/` anywhere in one).
    fn do_redirect(&mut self, is_compile: bool) {
        self.parser.next_param(&self.vars);
        let fname = self.parser.make_string(&self.vars);
        if fname.is_empty() {
            return; // ignore altogether if null filename
        }

        // Expand relative to the current DSS directory.
        let mut path = self.current_dir.join(&fname);
        if !path.is_file() {
            // Try appending '.dss' if there is no extension yet.
            if !fname.contains('.') {
                path = self.current_dir.join(format!("{fname}.dss"));
            }
            if !path.is_file() {
                self.errors
                    .push(format!("Redirect file not found: \"{fname}\""));
                if let Some(ckt) = self.circuit.as_mut() {
                    ckt.solution.solution_abort = true;
                }
                return;
            }
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                self.errors
                    .push(format!("Redirect File \"{fname}\" could not be read: {e}"));
                if let Some(ckt) = self.circuit.as_mut() {
                    ckt.solution.solution_abort = true;
                }
                return;
            }
        };

        // Change directory to the file's path in case it loads more files.
        let save_dir = self.current_dir.clone();
        if let Some(parent) = path.parent() {
            self.current_dir = parent.to_path_buf();
        }

        self.redirect_abort = false;
        self.in_redirect = true;

        let mut in_block_comment = false;
        let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
        for input_line in &lines {
            if self.redirect_abort {
                break;
            }
            if input_line.is_empty() {
                continue;
            }
            if !in_block_comment && input_line.starts_with("/*") {
                in_block_comment = true;
            }
            if !in_block_comment {
                let solution_abort = self
                    .circuit
                    .as_ref()
                    .is_some_and(|c| c.solution.solution_abort);
                if !solution_abort {
                    self.command(input_line);
                } else {
                    self.redirect_abort = true; // Abort file if solution was aborted
                }
            }
            if in_block_comment && input_line.contains("*/") {
                in_block_comment = false;
            }
        }

        self.in_redirect = false;
        let path_str = path.to_string_lossy().to_string();
        self.vars.add("@lastfile", &path_str);
        if is_compile {
            // Compile keeps the script directory as the data path.
            self.vars.add("@lastcompilefile", &path_str);
        } else {
            self.current_dir = save_dir; // Redirect returns to where we were
            self.vars.add("@lastredirectfile", &path_str);
        }
    }
}

/// Per-element snapshot for the golden feeder gate: mirrors dss-python's
/// `CktElement.Powers`/`Currents` over the oracle's `First/Next` iteration
/// (= creation) order.
#[derive(Debug, Clone)]
pub struct ElementSnapshot {
    /// `FullName` (`Class.name`).
    pub name: String,
    /// kW/kvar interleaved per conductor and terminal (CAPI
    /// `Alt_CE_Get_Powers`: `GetPhasePower · 0.001`).
    pub powers: Vec<f64>,
    /// Amps, re/im interleaved per conductor and terminal (`Iterminal`).
    pub currents: Vec<f64>,
}

impl Dss {
    /// Snapshot every circuit element's terminal powers and currents in
    /// creation order (the oracle's `First/Next` order). Pascal
    /// `TDSSCktElement.GetPhasePower` / `ComputeIterminal`.
    pub fn snapshot_elements(&mut self) -> Vec<ElementSnapshot> {
        let Dss {
            classes, circuit, ..
        } = self;
        let ckt = circuit.as_ref().expect("snapshot needs a circuit");
        let sys = crate::solution::solution::sys_ctx(ckt);
        let node_v = ckt.solution.node_v.clone();
        let positive_seq = ckt.positive_sequence;
        let mut out = Vec::with_capacity(ckt.ckt_elements.len());
        for &r in &ckt.ckt_elements {
            let class_name = classes[r.cls].props.class_name();
            let obj = &mut classes[r.cls].objects[r.idx];
            let name = format!("{}.{}", class_name, obj.data().name());
            let elem = obj
                .as_ckt_element_mut()
                .expect("ckt_elements refs are circuit elements");
            let yorder = elem.cd().yorder;
            let mut currents = vec![0.0; 2 * yorder];
            let mut powers = vec![0.0; 2 * yorder];
            if elem.cd().enabled && !elem.cd().node_ref.is_empty() {
                elem.compute_iterminal(&sys, &node_v);
                let cd = elem.cd();
                for k in 0..yorder {
                    let i = cd.iterminal[k];
                    currents[2 * k] = i.re;
                    currents[2 * k + 1] = i.im;
                    let n = cd.node_ref[k];
                    if n > 0 {
                        let mut s = node_v[n] * i.conj();
                        if positive_seq {
                            s *= 3.0;
                        }
                        powers[2 * k] = s.re * 0.001;
                        powers[2 * k + 1] = s.im * 0.001;
                    }
                }
            }
            out.push(ElementSnapshot {
                name,
                powers,
                currents,
            });
        }
        out
    }

    /// CAPI `Circuit_Get_TotalPower`: the sum of every source's terminal-1
    /// power, in kW/kvar (negative of the power delivered to the circuit).
    pub fn total_power(&mut self) -> (f64, f64) {
        use num_complex::Complex64;
        let Dss {
            classes, circuit, ..
        } = self;
        let ckt = circuit.as_ref().expect("total_power needs a circuit");
        let sys = crate::solution::solution::sys_ctx(ckt);
        let node_v = ckt.solution.node_v.clone();
        let positive_seq = ckt.positive_sequence;
        let mut total = Complex64::ZERO;
        for &r in &ckt.sources {
            let elem = classes[r.cls].objects[r.idx]
                .as_ckt_element_mut()
                .expect("sources are circuit elements");
            if !elem.cd().enabled || elem.cd().node_ref.is_empty() {
                continue;
            }
            elem.compute_iterminal(&sys, &node_v);
            let cd = elem.cd();
            // Pascal Get_Power(1): sum over terminal-1 conductors.
            let mut s = Complex64::ZERO;
            for i in 0..cd.nconds {
                let n = cd.node_ref[i];
                if n > 0 {
                    s += node_v[n] * cd.iterminal[i].conj();
                }
            }
            if positive_seq {
                s *= 3.0;
            }
            total += s;
        }
        (total.re * 0.001, total.im * 0.001)
    }

    /// CAPI `Circuit_Get_Losses`: total circuit losses in W/var (sum over
    /// enabled PD elements).
    pub fn losses(&mut self) -> (f64, f64) {
        let Dss {
            classes, circuit, ..
        } = self;
        let ckt = circuit.as_mut().expect("losses needs a circuit");
        let sys = crate::solution::solution::sys_ctx(ckt);
        let mut store = ClassStore { classes };
        let total = ckt.losses(&mut store, &sys);
        (total.re, total.im)
    }
}

impl Default for Dss {
    fn default() -> Self {
        Self::new()
    }
}

/// Pascal `Parser.DblValue` on the current token, record-and-continue.
fn get_dbl(parser: &mut Parser, vars: &ParserVars, errors: &mut Vec<String>) -> Option<f64> {
    match parser.make_double(vars) {
        Ok(v) => Some(v),
        Err(e) => {
            errors.push(e.message().to_string());
            None
        }
    }
}

/// Pascal `Parser.IntValue` on the current token, record-and-continue.
fn get_int(parser: &mut Parser, vars: &ParserVars, errors: &mut Vec<String>) -> Option<i32> {
    match parser.make_integer(vars) {
        Ok(v) => Some(v),
        Err(e) => {
            errors.push(e.message().to_string());
            None
        }
    }
}

/// `StringToOrdinal` against a registry enum, record-and-continue (the Pascal
/// exception is caught by `ProcessCommand` and logged).
fn enum_ord(
    enums: &EnumRegistry,
    id: EnumId,
    value: &str,
    errors: &mut Vec<String>,
) -> Option<i32> {
    match enums.get(id).string_to_ordinal(&value.to_lowercase()) {
        Ok(v) => Some(v),
        Err(e) => {
            errors.push(e.message().to_string());
            None
        }
    }
}

/// Pascal `AppendGlobalResult`: comma-separated accumulation.
fn append_result(result: &mut String, s: &str) {
    if result.is_empty() {
        result.push_str(s);
    } else {
        result.push_str(", ");
        result.push_str(s);
    }
}

fn yes_no(b: bool) -> &'static str {
    if b { "Yes" } else { "No" }
}

/// Pascal `MakeLikeProperty` set path: find the source object by name in the
/// same class, clone it, and copy its state onto the target.
fn make_like(
    objects: &mut [Box<dyn DssObject>],
    name_to_idx: &HashMap<String, usize>,
    target: usize,
    source_name: &str,
    errors: &mut Vec<String>,
    class_name: &str,
) {
    match name_to_idx.get(&source_name.to_lowercase()) {
        Some(&si) => {
            let src = objects[si].clone_box();
            objects[target].make_like(src.as_ref());
        }
        None => {
            errors.push(format!(
                "Error in {class_name} MakeLike: \"{source_name}\" not found."
            ));
        }
    }
}

/// Pascal `ParseObjName`: split `Class.Object.Property` into `(Class.Object,
/// Property)`. With no dot, the whole string is the property name.
fn parse_obj_name(fullname: &str) -> (String, String) {
    match fullname.find('.') {
        None => (String::new(), fullname.to_string()),
        Some(dot1) => {
            let rest = &fullname[dot1 + 1..];
            match rest.find('.') {
                None => (fullname[..dot1].to_string(), rest.to_string()),
                Some(dot2) => (
                    fullname[..dot1 + 1 + dot2].to_string(),
                    rest[dot2 + 1..].to_string(),
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(dss: &mut Dss, what: &str) -> String {
        dss.command(&format!("? {what}"));
        dss.result().to_string()
    }

    /// `Edit`/`~`/`?` are circuit-gated in `ProcessCommand` (error 301), so
    /// even DSS_OBJECT tests need a circuit.
    fn dss_with_circuit() -> Dss {
        let mut dss = Dss::new();
        dss.command("New circuit.testckt");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        dss
    }

    #[test]
    fn new_and_query_defaults() {
        let mut dss = dss_with_circuit();
        dss.command("New TCC_Curve.test");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        assert_eq!(query(&mut dss, "TCC_Curve.test.NPts"), "0");
        assert_eq!(query(&mut dss, "TCC_Curve.test.C_Array"), "");
        assert_eq!(query(&mut dss, "TCC_Curve.test.T_Array"), "");
        assert_eq!(query(&mut dss, "TCC_Curve.test.Like"), "");
    }

    #[test]
    fn new_with_inline_edits() {
        let mut dss = dss_with_circuit();
        dss.command("New TCC_Curve.t npts=3 C_array=(1 2 3) T_array=(0.1 0.2 0.3)");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        assert_eq!(query(&mut dss, "tcc_curve.t.npts"), "3");
        assert_eq!(query(&mut dss, "tcc_curve.t.c_array"), "[ 1 2 3]");
        assert_eq!(query(&mut dss, "tcc_curve.t.t_array"), "[ 0.1 0.2 0.3]");
    }

    #[test]
    fn edit_and_more_continue_the_object() {
        let mut dss = dss_with_circuit();
        dss.command("New TCC_Curve.t npts=2");
        dss.command("Edit TCC_Curve.t C_array=(5 6)");
        dss.command("~ T_array=(9 8)");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        assert_eq!(query(&mut dss, "tcc_curve.t.c_array"), "[ 5 6]");
        assert_eq!(query(&mut dss, "tcc_curve.t.t_array"), "[ 9 8]");
    }

    #[test]
    fn make_like_copies_state() {
        let mut dss = dss_with_circuit();
        dss.command("New TCC_Curve.a npts=2 C_array=(1 2) T_array=(3 4)");
        dss.command("New TCC_Curve.b like=a");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        assert_eq!(query(&mut dss, "tcc_curve.b.npts"), "2");
        assert_eq!(query(&mut dss, "tcc_curve.b.c_array"), "[ 1 2]");
        assert_eq!(query(&mut dss, "tcc_curve.b.t_array"), "[ 3 4]");
    }

    #[test]
    fn make_like_copies_prp_sequence() {
        // Pascal `TDSSObject.MakeLike` copies the source's PrpSequence, then
        // the Edit loop stamps the Like property itself — so a Save-order walk
        // of the target yields NPts, C_Array, T_Array, Like.
        let mut dss = Dss::new();
        dss.command("New TCC_Curve.a npts=2 C_array=(1 2) T_array=(3 4)");
        dss.command("New TCC_Curve.b like=a");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());

        let cls = &dss.classes[0];
        let oi = cls.name_to_idx["b"];
        let data = cls.objects[oi].data();
        assert_eq!(data.next_property_set(None), Some(1)); // NPts
        assert_eq!(data.next_property_set(Some(1)), Some(2)); // C_Array
        assert_eq!(data.next_property_set(Some(2)), Some(3)); // T_Array
        assert_eq!(data.next_property_set(Some(3)), Some(4)); // Like
        assert_eq!(data.next_property_set(Some(4)), None);
    }

    #[test]
    fn set_and_get_require_a_circuit() {
        let mut dss = Dss::new();
        dss.command("Set mode=snap");
        dss.command("Get mode");
        assert_eq!(dss.errors().len(), 2, "{:?}", dss.errors());
        assert!(
            dss.errors()
                .iter()
                .all(|e| e.contains("You must create a new circuit object first")),
            "{:?}",
            dss.errors()
        );
    }

    #[test]
    fn solve_requires_a_circuit() {
        let mut dss = Dss::new();
        dss.command("Solve");
        assert!(
            dss.errors()
                .iter()
                .any(|e| e.contains("You must create a new circuit object first")),
            "{:?}",
            dss.errors()
        );
    }

    #[test]
    fn duplicate_new_edits_existing() {
        let mut dss = dss_with_circuit();
        dss.command("New TCC_Curve.t npts=2 C_array=(1 2)");
        // A second "New" of the same name becomes an edit (DSS_OBJECT, no dups).
        dss.command("New TCC_Curve.t C_array=(7 8)");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        assert_eq!(query(&mut dss, "tcc_curve.t.c_array"), "[ 7 8]");
    }

    #[test]
    fn clear_drops_objects() {
        let mut dss = dss_with_circuit();
        dss.command("New TCC_Curve.t npts=2");
        dss.command("Clear"); // drops the circuit and all objects
        assert!(dss.circuit().is_none());
        // `?` is circuit-gated again after Clear, like the Pascal.
        dss.command("? TCC_Curve.t.npts");
        assert!(
            dss.errors()
                .iter()
                .any(|e| e.contains("You must create a new circuit object first")),
            "{:?}",
            dss.errors()
        );
        // After recreating a circuit, the old object is really gone.
        let mut dss = dss_with_circuit();
        dss.command("New circuit.again"); // second circuit is rejected
        assert!(!dss.errors().is_empty());
        assert_eq!(query(&mut dss, "TCC_Curve.t.npts"), "Property Unknown");
    }

    #[test]
    fn unknown_parameter_is_reported() {
        let mut dss = Dss::new();
        dss.command("New TCC_Curve.t bogus=3");
        assert!(dss.errors().iter().any(|e| e.contains("Unknown parameter")));
    }

    #[test]
    fn parse_obj_name_splits_on_last_class_dot() {
        assert_eq!(
            parse_obj_name("TCC_Curve.test.npts"),
            ("TCC_Curve.test".to_string(), "npts".to_string())
        );
        assert_eq!(
            parse_obj_name("test.npts"),
            ("test".to_string(), "npts".to_string())
        );
        assert_eq!(parse_obj_name("npts"), (String::new(), "npts".to_string()));
    }

    #[test]
    fn new_circuit_creates_default_source() {
        let mut dss = Dss::new();
        dss.command("New circuit.test basekv=115 pu=1.0001");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        let ckt = dss.circuit().unwrap();
        assert_eq!(ckt.name, "test");
        assert_eq!(ckt.sources.len(), 1);
        assert_eq!(query(&mut dss, "vsource.source.basekv"), "115");
        assert_eq!(query(&mut dss, "vsource.source.pu"), "1.0001");
        assert_eq!(query(&mut dss, "vsource.source.bus1"), "sourcebus");
    }

    #[test]
    fn two_bus_snapshot_solves() {
        let mut dss = Dss::new();
        dss.command("New circuit.twobus basekv=12.47 pu=1.0 phases=3 mvasc3=2000");
        dss.command("New Line.l1 bus1=sourcebus bus2=loadbus length=1 units=km");
        dss.command("New Load.ld1 bus1=loadbus phases=3 kv=12.47 kw=600 pf=0.95");
        dss.command("Set voltagebases=[12.47]");
        dss.command("CalcVoltageBases");
        dss.command("Solve");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        let ckt = dss.circuit().unwrap();
        assert!(ckt.is_solved);
        assert_eq!(ckt.num_nodes, 6);
        let vbase = 12.47e3 / crate::util::sqrt3();
        for i in 1..=ckt.num_nodes {
            let vm = ckt.solution.node_v[i].norm();
            assert!(
                (vm / vbase - 1.0).abs() < 0.1,
                "node {i} voltage {vm} not near {vbase}"
            );
        }
        // Iteration count reported like the oracle's Solution.Iterations.
        assert!(ckt.solution.iteration >= 2);
    }

    /// Parse the single number a `?` scalar query returns.
    fn query_f64(dss: &mut Dss, what: &str) -> f64 {
        query(dss, what).parse().expect("numeric query result")
    }

    #[test]
    fn line_fetches_sym_linecode() {
        // Oracle (dss-python 0.15.7): linecode in mi, line length 2000 ft.
        let mut dss = Dss::new();
        dss.command("New circuit.p");
        dss.command(
            "New linecode.mtx601 nphases=3 r1=0.1 x1=0.2 r0=0.3 x0=0.6 c1=3 c0=1 \
             units=mi normamps=500 emergamps=700",
        );
        dss.command("New line.l1 bus1=a bus2=b linecode=mtx601 length=2000 units=ft");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        assert_eq!(query(&mut dss, "line.l1.linecode"), "mtx601");
        assert_eq!(query(&mut dss, "line.l1.normamps"), "500");
        assert_eq!(query(&mut dss, "line.l1.emergamps"), "700");
        assert_eq!(query(&mut dss, "line.l1.units"), "ft");
        // r1 getter divides by FUnitsConvert = ConvertLineUnits(mi, ft) = 5280.
        assert!((query_f64(&mut dss, "line.l1.r1") - 0.1 / 5280.0).abs() < 1e-12);
        // Unported scalar/array refs render like the oracle.
        assert_eq!(query(&mut dss, "line.l1.geometry"), "");
        assert_eq!(query(&mut dss, "line.l1.wires"), "[]");
    }

    #[test]
    fn line_fetches_matrix_linecode() {
        let mut dss = Dss::new();
        dss.command("New circuit.p");
        dss.command(
            "New linecode.mx nphases=2 rmatrix=[0.1 | 0.05 0.1] \
             xmatrix=[0.2 | 0.07 0.2] cmatrix=[3 | -1 3] units=mi",
        );
        dss.command("New line.l3 bus1=a.1.2 bus2=b.1.2 linecode=mx length=1 units=mi");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        assert_eq!(query(&mut dss, "line.l3.phases"), "2");
        assert_eq!(query(&mut dss, "line.l3.rmatrix"), "[0.1 |0.05 0.1 ]");
        // Matrix model hides the sym scalars (CONDITIONAL_VALUE).
        assert_eq!(query(&mut dss, "line.l3.r1"), "----");
    }

    #[test]
    fn line_linecode_then_r1_override_keeps_fetched_matrix() {
        // Oracle: r1=0.5 overrides the scalar, but the dumped rmatrix still
        // reflects the code's Z (recalc is deferred to CalcYPrim).
        let mut dss = Dss::new();
        dss.command("New circuit.p");
        dss.command("New linecode.mtx601 nphases=3 r1=0.1 x1=0.2 r0=0.3 x0=0.6 units=mi");
        dss.command("New line.l4 bus1=a bus2=b linecode=mtx601 r1=0.5 length=1 units=mi");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        assert_eq!(query(&mut dss, "line.l4.r1"), "0.5");
        // Zs.re = (2*0.1 + 0.3)/3 = 0.5/3, units_convert reset to 1 by r1.
        let rm = query(&mut dss, "line.l4.rmatrix");
        let first: f64 = rm
            .trim_start_matches('[')
            .split_whitespace()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!((first - 0.5 / 3.0).abs() < 1e-9, "{rm}");
    }

    #[test]
    fn line_unknown_linecode_errors_and_continues() {
        let mut dss = Dss::new();
        dss.command("New circuit.p");
        dss.command("New line.l5 bus1=a bus2=b linecode=nosuch r1=0.1 length=1");
        assert!(
            dss.errors()
                .iter()
                .any(|e| e == "Line.l5.LineCode: LineCode object \"nosuch\" not found."),
            "{:?}",
            dss.errors()
        );
        // The edit continued: r1=0.1 was applied, phases stayed default.
        assert_eq!(query(&mut dss, "line.l5.r1"), "0.1");
        assert_eq!(query(&mut dss, "line.l5.phases"), "3");
    }

    /// WP4.7 step 6 (the "silent killer" check): control elements attach to
    /// existing buses, so adding a RegControl must not change `YNodeOrder`,
    /// and the Y build must skip their `yprim: None` (no stamping, solvable).
    #[test]
    fn reg_control_does_not_change_node_order() {
        let build = |with_control: bool| -> (Vec<String>, bool, i32) {
            let mut dss = Dss::new();
            dss.command("New circuit.ctl basekv=12.47 pu=1.0 phases=3 mvasc3=2000");
            dss.command(
                "New transformer.t1 phases=3 windings=2 buses=(sourcebus, b2) \
                 conns=(delta wye) kvs=(12.47 4.16) kvas=(5000 5000) xhl=8",
            );
            if with_control {
                dss.command(
                    "New regcontrol.r1 transformer=t1 winding=2 vreg=122 band=2 ptratio=20",
                );
            }
            dss.command("New load.l1 bus1=b2 phases=3 kv=4.16 kw=300 pf=0.95");
            dss.command("Set voltagebases=[12.47, 4.16]");
            dss.command("CalcVoltageBases");
            dss.command("Solve");
            assert!(dss.errors().is_empty(), "{:?}", dss.errors());
            let ckt = dss.circuit().unwrap();
            if with_control {
                assert_eq!(ckt.controls.len(), 1);
                // The control sits on the transformer's winding-2 bus.
                let r = ckt.controls[0];
                let elem = dss.classes[r.cls].objects[r.idx].as_ckt_element().unwrap();
                assert_eq!(elem.cd().get_bus(1), "b2");
                assert!(elem.cd().yprim.is_none());
            }
            let names = (1..=ckt.num_nodes).map(|i| ckt.node_name(i)).collect();
            (names, ckt.is_solved, ckt.solution.iteration)
        };
        let (with, solved_w, iter_w) = build(true);
        let (without, solved_wo, iter_wo) = build(false);
        assert!(solved_w && solved_wo);
        assert_eq!(with, without, "RegControl changed the node order");
        assert_eq!(iter_w, iter_wo, "RegControl changed the iteration count");
    }

    #[test]
    fn get_returns_set_values() {
        let mut dss = Dss::new();
        dss.command("New circuit.c1");
        dss.command("Set mode=daily tolerance=0.001 maxiterations=25");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        dss.command("Get mode tolerance maxiterations");
        assert_eq!(dss.result(), "Daily, 0.001, 25");
    }
}

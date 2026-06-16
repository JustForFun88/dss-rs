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
use std::path::{Path, PathBuf};

use dss_parser::{Parser, ParserVars};

use crate::circuit::{Circuit, ElemKind};
use crate::elements::control::{cap_control, gen_dispatcher, reg_control, storage_controller};
use crate::elements::general::{
    conductor_data, growth_shape, line_code, line_geometry, line_spacing, load_shape, price_shape,
    spectrum, tcc_curve, temp_shape, xfmr_code, xy_curve,
};
use crate::elements::meter::energymeter;
use crate::elements::meter::monitor;
use crate::elements::meter::sensor;
use crate::elements::pc::{generator, load, vsource};
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

    fn obj(&self, r: ElemRef) -> &dyn DssObject {
        self.classes[r.cls].objects[r.idx].as_ref()
    }

    fn find_ckt_element(&self, full_name: &str) -> Option<ElemRef> {
        let lower = full_name.to_lowercase();
        let (cls_name, obj_name) = match lower.split_once('.') {
            Some((c, n)) => (Some(c), n),
            None => (None, lower.as_str()),
        };
        for (ci, class) in self.classes.iter().enumerate() {
            // Only circuit-element classes are eligible (Pascal DeviceList).
            if class.kind.is_none() {
                continue;
            }
            if let Some(cn) = cls_name
                && !class.props.class_name().eq_ignore_ascii_case(cn)
            {
                continue;
            }
            if let Some(&oi) = class.name_to_idx.get(obj_name) {
                return Some(ElemRef { cls: ci, idx: oi });
            }
        }
        None
    }

    fn obj_mut(&mut self, r: ElemRef) -> &mut dyn DssObject {
        self.classes[r.cls].objects[r.idx].as_mut()
    }

    fn pair_mut(&mut self, a: ElemRef, b: ElemRef) -> (&mut dyn DssObject, &mut dyn DssObject) {
        assert_ne!((a.cls, a.idx), (b.cls, b.idx), "pair_mut: aliasing refs");
        if a.cls == b.cls {
            let objs = &mut self.classes[a.cls].objects;
            let [oa, ob] = objs
                .get_disjoint_mut([a.idx, b.idx])
                .expect("pair_mut: object index out of range");
            (oa.as_mut(), ob.as_mut())
        } else {
            let [ca, cb] = self
                .classes
                .get_disjoint_mut([a.cls, b.cls])
                .expect("pair_mut: class index out of range");
            (ca.objects[a.idx].as_mut(), cb.objects[b.idx].as_mut())
        }
    }

    fn triple_mut(
        &mut self,
        a: ElemRef,
        b: ElemRef,
        c: ElemRef,
    ) -> (&mut dyn DssObject, &mut dyn DssObject, &mut dyn DssObject) {
        let key = |r: ElemRef| (r.cls, r.idx);
        assert!(
            key(a) != key(b) && key(a) != key(c) && key(b) != key(c),
            "triple_mut: aliasing refs"
        );
        // Split per distinct class first, then per object inside a shared class.
        if a.cls == b.cls && b.cls == c.cls {
            let objs = &mut self.classes[a.cls].objects;
            let [oa, ob, oc] = objs
                .get_disjoint_mut([a.idx, b.idx, c.idx])
                .expect("triple_mut: object index out of range");
            (oa.as_mut(), ob.as_mut(), oc.as_mut())
        } else if a.cls == b.cls {
            let [cab, cc] = self
                .classes
                .get_disjoint_mut([a.cls, c.cls])
                .expect("triple_mut: class index out of range");
            let [oa, ob] = cab
                .objects
                .get_disjoint_mut([a.idx, b.idx])
                .expect("triple_mut: object index out of range");
            (oa.as_mut(), ob.as_mut(), cc.objects[c.idx].as_mut())
        } else if a.cls == c.cls {
            let [cac, cb] = self
                .classes
                .get_disjoint_mut([a.cls, b.cls])
                .expect("triple_mut: class index out of range");
            let [oa, oc] = cac
                .objects
                .get_disjoint_mut([a.idx, c.idx])
                .expect("triple_mut: object index out of range");
            (oa.as_mut(), cb.objects[b.idx].as_mut(), oc.as_mut())
        } else if b.cls == c.cls {
            let [ca, cbc] = self
                .classes
                .get_disjoint_mut([a.cls, b.cls])
                .expect("triple_mut: class index out of range");
            let [ob, oc] = cbc
                .objects
                .get_disjoint_mut([b.idx, c.idx])
                .expect("triple_mut: object index out of range");
            (ca.objects[a.idx].as_mut(), ob.as_mut(), oc.as_mut())
        } else {
            let [ca, cb, cc] = self
                .classes
                .get_disjoint_mut([a.cls, b.cls, c.cls])
                .expect("triple_mut: class index out of range");
            (
                ca.objects[a.idx].as_mut(),
                cb.objects[b.idx].as_mut(),
                cc.objects[c.idx].as_mut(),
            )
        }
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

    /// Process one command line (Pascal `ProcessCommand`). Errors are recorded
    /// in [`Dss::errors`] (record-and-continue); query results land in
    /// [`Dss::result`].
    pub fn command(&mut self, cmd_line: &str) {
        if !self.in_redirect {
            // CAPI `Text_Set_Command`: "Reset for commands entered from
            // outside" — a previous abort doesn't poison the next external
            // command, while a Redirect/Compile still aborts the rest of its
            // file (`do_redirect` checks the live flag between lines).
            if let Some(ckt) = self.circuit.as_mut() {
                ckt.solution.solution_abort = false;
            }
        }
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
            cmd::SAMPLE => self.do_sample_cmd(),
            cmd::RESET => self.do_reset_cmd(),
            cmd::ALLOCATE_LOADS => self.do_allocate_loads_cmd(),
            cmd::RELCALC => self.do_relcalc_cmd(),
            cmd::REDUCE => self.do_reduce_cmd(),
            cmd::BUSCOORDS => self.do_bus_coords_cmd(false),
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
            .push(format!("Command \"{name}\" is not ported yet."));
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
        let mut ckt = Circuit::new(name, self.default_base_freq);
        // Pascal `TDSSCircuit.Create`: both default shape refs resolve to the
        // built-in `loadshape.default` (created with the default DSS items).
        ckt.default_daily_shape_obj = find_load_shape(&self.classes, "default");
        ckt.default_yearly_shape_obj = find_load_shape(&self.classes, "default");
        self.circuit = Some(ckt);
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
        // Pascal `DoClearCmd` → `ClearAll` → recreate the default items.
        self.create_default_dss_items();
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
            current_dir,
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

        // Deferred file loads (Pascal runs `DoCSVFile` etc. in the property
        // hook, which has the DSS context; our hook cannot reach the filesystem
        // or the current directory, so it queues the request and we resolve it
        // here — before `end_edit`, so derived state like `SetMaxPandQ` sees the
        // loaded data). Paths resolve relative to `current_dir`, like Redirect.
        let file_loads = objects[oi].take_file_loads();
        for fl in &file_loads {
            let path = current_dir.join(&fl.filename);
            match std::fs::read_to_string(&path) {
                Ok(content) => objects[oi].apply_file_load(fl, &content, errors),
                // Pascal error 613.
                Err(_) => errors.push(format!("Error opening file: \"{}\"", fl.filename)),
            }
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
            if cd.signal_reset_solution_initialized {
                cd.signal_reset_solution_initialized = false;
                ckt.solution.solution_initialized = false;
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
                classes,
                circuit,
                option_list,
                parser,
                aux_parser,
                vars,
                enums,
                errors,
                default_base_freq,
                max_allocation_iterations,
                current_dir,
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
                    opt::HOUR => {
                        if let Some(v) = get_int(parser, vars, errors) {
                            ckt.solution.int_hour = v;
                        }
                    }
                    opt::SEC => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.solution.t = v;
                        }
                    }
                    opt::STEPSIZE | opt::H => {
                        ckt.solution.h = interpret_time_step_size(&param, ckt.solution.h, errors);
                        ckt.solution.interval_hrs = ckt.solution.h / 3600.0;
                    }
                    opt::TIME => {
                        // Pascal `Set_Time`: parse `[hour, sec]` as a 2-vector.
                        let mut buf = vec![0.0; 2];
                        match parser.parse_as_vector(vars, &mut buf, false) {
                            Ok(_) => {
                                // TODO(compat): FPC banker's `Round` on the hour.
                                ckt.solution.int_hour = buf[0].round_ties_even() as i32;
                                ckt.solution.t = buf[1];
                                ckt.solution.update_dbl_hour();
                            }
                            Err(e) => errors.push(e.message().to_string()),
                        }
                    }
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
                        if let Some(v) = enum_ord(enums, enums.solve_mode, &param, errors)
                            && crate::solution::set_mode(ckt, SolveMode::from_ordinal(v), errors)
                        {
                            // Pascal `Set_Mode` tail: monitor/meter resets are
                            // Phase 6 no-ops, there are no Fault elements yet
                            // (Phase 7), and `DoResetControls` runs here.
                            let mut store = ClassStore { classes };
                            let mut env = SolveEnv {
                                store: &mut store,
                                parser: aux_parser,
                                vars,
                                errors,
                            };
                            if let Err(e) =
                                crate::solution::controls::reset_all_controls(ckt, &mut env)
                            {
                                env.errors.push(format!("Error resetting controls: {e}"));
                            }
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
                    // Pascal `Set GenkW/GenPF/Capkvar/AddType=`: the auto-add
                    // option object (`Circuit.AutoAddObj`). The auto-add solve
                    // itself is NOT_PORTED (see circuit/auto_add.rs).
                    opt::GEN_KW => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.auto_add_obj.gen_kw = v;
                        }
                    }
                    opt::GEN_PF => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.auto_add_obj.gen_pf = v;
                        }
                    }
                    opt::CAP_KVAR => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.auto_add_obj.cap_kvar = v;
                        }
                    }
                    opt::ADD_TYPE => {
                        if let Some(v) = enum_ord(enums, enums.add_type, &param, errors) {
                            ckt.auto_add_obj.add_type = v;
                        }
                    }
                    opt::ALLOW_DUPLICATES => ckt.duplicates_allowed = interpret_yes_no(&param),
                    opt::ZONE_LOCK => ckt.zones_locked = interpret_yes_no(&param),
                    opt::UE_WEIGHT => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.ue_weight = v;
                        }
                    }
                    opt::LOSS_WEIGHT => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.loss_weight = v;
                        }
                    }
                    // Pascal `parseIntArray` (ExecOptions.pas l.350) via AuxParser.
                    opt::UE_REGS => ckt.ue_regs = parse_int_array(aux_parser, vars, &param, errors),
                    opt::LOSS_REGS => {
                        ckt.loss_regs = parse_int_array(aux_parser, vars, &param, errors)
                    }
                    // Pascal `Set Trapezoidal=`: the meter integration rule
                    // (reset to false by `Set mode=`).
                    opt::TRAPEZOIDAL => ckt.trapezoidal_integration = interpret_yes_no(&param),
                    // Pascal `DoAutoAddBusList` (ExecHelper.pas l.1986).
                    opt::AUTO_BUS_LIST => do_auto_add_bus_list(
                        aux_parser,
                        vars,
                        current_dir,
                        &param,
                        &mut ckt.auto_add_bus_list,
                        errors,
                    ),
                    // Pascal `DoSetReduceStrategy` (ExecHelper.pas l.3049). The
                    // strategy is stored; the reduction itself is NOT_PORTED.
                    opt::REDUCE_OPTION => set_reduce_strategy(ckt, &param, errors),
                    opt::KEEP_LOAD => ckt.reduce_laterals_keep_load = interpret_yes_no(&param),
                    opt::ZMAG => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.reduction_zmag = v;
                        }
                    }
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
                    opt::DEFAULT_DAILY => {
                        // Pascal: only replace when the shape is found.
                        if let Some(shape) = find_load_shape(classes, &param) {
                            ckt.default_daily_shape_obj = Some(shape);
                        }
                    }
                    opt::DEFAULT_YEARLY => {
                        if let Some(shape) = find_load_shape(classes, &param) {
                            ckt.default_yearly_shape_obj = Some(shape);
                        }
                    }
                    opt::CKT_MODEL => {
                        if let Some(v) = enum_ord(enums, enums.ckt_model, &param, errors) {
                            ckt.positive_sequence = v != 0;
                        }
                    }
                    opt::PRICE_SIGNAL => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.price_signal = v;
                        }
                    }
                    opt::PRICE_CURVE => {
                        // Pascal assigns Find()'s result (NIL on miss) first.
                        ckt.price_curve_obj = find_price_shape(classes, &param);
                        if ckt.price_curve_obj.is_none() {
                            errors.push(format!("Priceshape.{param} not found."));
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
                    // Pascal `DoSetAllocationFactors` (ExecHelper.pas l.2651):
                    // set every load's kVA allocation factor (ConnectedkVA spec).
                    opt::ALLOCATION_FACTORS => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            if v <= 0.0 {
                                errors.push(
                                    "Allocation Factor must be greater than zero.".to_string(),
                                );
                            } else {
                                let mut store = ClassStore {
                                    classes: &mut classes[..],
                                };
                                for &lr in &ckt.loads {
                                    if let Some(load) =
                                        store.obj_mut(lr).as_any_mut().downcast_mut::<load::Load>()
                                    {
                                        load.set_kva_allocation_factor(v);
                                    }
                                }
                            }
                        }
                    }
                    opt::NUM_ALLOC_ITERATIONS => {
                        if let Some(v) = get_int(parser, vars, errors) {
                            *max_allocation_iterations = v;
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
                        errors.push(format!("Set option \"{name}\" is not ported yet."));
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
                opt::HOUR => append_result(&mut result, &ckt.solution.int_hour.to_string()),
                opt::SEC => append_result(&mut result, &float_to_str(ckt.solution.t)),
                opt::STEPSIZE | opt::H => append_result(&mut result, &float_to_str(ckt.solution.h)),
                opt::TIME => append_result(
                    &mut result,
                    &format!(
                        "[ {}, {} ] !... {} (hours)",
                        ckt.solution.int_hour,
                        float_to_str(ckt.solution.t),
                        float_to_str(ckt.solution.dbl_hour)
                    ),
                ),
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
                opt::GEN_KW => append_result(&mut result, &float_to_str(ckt.auto_add_obj.gen_kw)),
                opt::GEN_PF => append_result(&mut result, &float_to_str(ckt.auto_add_obj.gen_pf)),
                opt::CAP_KVAR => {
                    append_result(&mut result, &float_to_str(ckt.auto_add_obj.cap_kvar))
                }
                opt::ADD_TYPE => append_result(
                    &mut result,
                    // Pascal echoes the lowercase device word, not the enum name.
                    match ckt.auto_add_obj.add_type {
                        crate::circuit::CAPADD => "capacitor",
                        _ => "generator",
                    },
                ),
                opt::ALLOW_DUPLICATES => append_result(&mut result, yes_no(ckt.duplicates_allowed)),
                opt::ZONE_LOCK => append_result(&mut result, yes_no(ckt.zones_locked)),
                opt::UE_WEIGHT => append_result(&mut result, &float_to_str(ckt.ue_weight)),
                opt::LOSS_WEIGHT => append_result(&mut result, &float_to_str(ckt.loss_weight)),
                opt::UE_REGS => append_result(&mut result, &int_array_to_string(&ckt.ue_regs)),
                opt::LOSS_REGS => append_result(&mut result, &int_array_to_string(&ckt.loss_regs)),
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
                opt::AUTO_BUS_LIST => {
                    for name in &ckt.auto_add_bus_list {
                        append_result(&mut result, name);
                    }
                }
                opt::REDUCE_OPTION => append_result(&mut result, &ckt.reduction_strategy_string),
                opt::KEEP_LOAD => append_result(&mut result, yes_no(ckt.reduce_laterals_keep_load)),
                opt::ZMAG => append_result(&mut result, &float_to_str(ckt.reduction_zmag)),
                opt::CONTROL_MODE => append_result(
                    &mut result,
                    &enums
                        .get(enums.control_mode)
                        .ordinal_to_string(ckt.solution.control_mode),
                ),
                opt::DEFAULT_DAILY => append_result(
                    &mut result,
                    &ckt.default_daily_shape_obj
                        .as_ref()
                        .map(|s| s.data().name().to_string())
                        .unwrap_or_default(),
                ),
                opt::DEFAULT_YEARLY => append_result(
                    &mut result,
                    &ckt.default_yearly_shape_obj
                        .as_ref()
                        .map(|s| s.data().name().to_string())
                        .unwrap_or_default(),
                ),
                opt::CKT_MODEL => append_result(
                    &mut result,
                    &enums
                        .get(enums.ckt_model)
                        .ordinal_to_string(ckt.positive_sequence as i32),
                ),
                opt::PRICE_SIGNAL => append_result(&mut result, &float_to_str(ckt.price_signal)),
                opt::PRICE_CURVE => append_result(
                    &mut result,
                    &ckt.price_curve_obj
                        .as_ref()
                        .map(|s| s.data().name().to_string())
                        .unwrap_or_default(),
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
                    errors.push(format!("Get option \"{name}\" is not ported yet."));
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

    /// Pascal `DoSampleCmd` (`ExecHelper.pas` l.1036): `MonitorClass.SampleAll`
    /// — force every enabled monitor (mode ≠ 5) to take a sample.
    fn do_sample_cmd(&mut self) {
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
        let sys = crate::solution::solution::sys_ctx(ckt);
        crate::solution::monitors::sample_all_monitors(ckt, &mut env, false);
        // Pascal `DoSampleCmd` l.1037: `EnergyMeterClass.SampleAll` (gets
        // generators too — the generator register sweep is WP6.8).
        crate::solution::meters::take_sample_all(ckt, env.store, &sys);
    }

    /// Pascal `TExecHelper.DoAllocateLoadsCmd` (`ExecHelper.pas` l.2605): adjust
    /// loads defined by connected kVA or kWh billing to match the EnergyMeter /
    /// Sensor measured peaks. Solves a snapshot guess, then iterates
    /// `MaxAllocationIterations` times: recompute each meter/sensor allocation
    /// factor, run each meter's zone allocation, and re-solve.
    fn do_allocate_loads_cmd(&mut self) {
        let max_iters = self.max_allocation_iterations.max(0) as usize;
        let Dss {
            classes,
            circuit,
            aux_parser,
            vars,
            errors,
            ..
        } = self;
        let ckt = circuit.as_mut().expect("gated in command()");
        ckt.load_multiplier = 1.0;
        // Pascal `DoAllocateLoadsCmd` (ExecHelper.pas l.2617): force SNAPSHOT
        // before the guess solve — `if Mode <> SNAPSHOT then Mode := SNAPSHOT`,
        // whose `Set_Mode` side effect re-inits the solution and clears the
        // meter-sampling state that a prior yearly/daily run may have left set.
        if ckt.solution.mode != SolveMode::Snapshot {
            crate::solution::set_mode(ckt, SolveMode::Snapshot, errors);
        }
        let mut store = ClassStore { classes };
        let mut env = SolveEnv {
            store: &mut store,
            parser: aux_parser,
            vars,
            errors,
        };
        crate::solution::meters::allocate_loads(ckt, &mut env, max_iters);
    }

    /// Pascal `DoResetCmd` (`ExecHelper.pas` l.1527): with no argument, reset
    /// monitors, meters, controls and clear the event/error logs; otherwise the
    /// first letter selects the target (`MOnitors`/`MEters`/`Controls`/
    /// `Eventlog`). Faults (`F`) and the topology `KeepList` (`K`) have no class
    /// in this port yet, so those selectors are accepted as no-ops.
    fn do_reset_cmd(&mut self) {
        self.parser.next_param(&self.vars);
        let param = self.parser.make_string(&self.vars).to_uppercase();
        let b = param.as_bytes();
        // Decode the Pascal `case Param[1] of` dispatch into a set of targets.
        let (do_monitors, do_meters, do_controls, do_eventlog) = if param.is_empty() {
            (true, true, true, true)
        } else {
            match b.first() {
                Some(&b'M') => (
                    b.get(1) == Some(&b'O'),
                    b.get(1) == Some(&b'E'),
                    false,
                    false,
                ),
                Some(&b'C') => (false, false, true, false),
                Some(&b'E') => (false, false, false, true),
                // `F` (faults) / `K` (keep list) are later-phase classes; accept
                // the selector without erroring so scripts don't abort.
                Some(&b'F') | Some(&b'K') => (false, false, false, false),
                _ => {
                    self.errors
                        .push(format!("Unknown argument to Reset Command: \"{param}\""));
                    return;
                }
            }
        };
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
        if do_monitors {
            crate::solution::monitors::reset_all_monitors(ckt, &mut env);
        }
        if do_meters {
            crate::solution::meters::reset_all_meters(ckt, env.store);
        }
        if do_controls {
            // Pascal `DoResetControls`: `Reset()` on every enabled control.
            let _ = crate::solution::controls::reset_all_controls(ckt, &mut env);
        }
        if do_eventlog {
            ckt.solution.event_log.clear();
        }
    }

    /// Pascal `TExecHelper.DoLambdaCalcs` (the `RelCalc` command): fault-rate and
    /// bus-interruption reliability calc over every EnergyMeter zone. The single
    /// positional parameter (any name) is the `AssumeRestoration` yes/no flag.
    fn do_relcalc_cmd(&mut self) {
        // EnergyMeter objects required (Pascal error 28724).
        if self
            .circuit
            .as_ref()
            .expect("gated in command()")
            .energy_meters
            .is_empty()
        {
            self.errors.push(
                "No EnergyMeter Objects Defined. EnergyMeter objects required for this function."
                    .to_string(),
            );
            return;
        }
        self.parser.next_param(&self.vars);
        let param = self.parser.make_string(&self.vars);
        let assume_restoration = !param.is_empty() && interpret_yes_no(&param);

        let Dss {
            classes,
            circuit,
            errors,
            ..
        } = self;
        let ckt = circuit.as_mut().expect("gated in command()");
        let mut store = ClassStore { classes };
        let errs = crate::solution::meters::calc_all_reliability_indices(
            ckt,
            &mut store,
            assume_restoration,
        );
        errors.extend(errs);
    }

    /// Pascal `TExecHelper.MarkCapandReactorBuses` (ExecHelper.pas l.1573): mark
    /// every bus carrying an *enabled, shunt-connected* capacitor or reactor as
    /// a "keeper" (`Bus.Keep := TRUE`) so a later circuit reduction won't
    /// eliminate it. Runs as a side-effect of the `Reduce` command regardless of
    /// whether the reduction itself proceeds.
    fn mark_cap_and_reactor_buses(&mut self) {
        let Dss {
            classes, circuit, ..
        } = self;
        let ckt = circuit.as_mut().expect("gated in command()");
        // `ElemRef` is `Copy`; snapshot the refs so the bus write below doesn't
        // alias the element-list borrow (the store borrows `classes`, disjoint
        // from `ckt`).
        let refs: Vec<ElemRef> = ckt
            .shunt_capacitors
            .iter()
            .chain(ckt.reactors.iter())
            .copied()
            .collect();
        let store = ClassStore { classes };
        for r in refs {
            let elem = store.ckt_elem(r);
            if elem.is_shunt() && elem.cd().enabled {
                let bus = elem.cd().terminals[0].bus_ref;
                if let Some(b) = ckt.buses.get_mut(bus) {
                    b.keep = true;
                }
            }
        }
    }

    /// Pascal `DoReduceCmd` (ExecHelper.pas l.1614): the `Reduce` command. The
    /// observable surface is reproduced faithfully — the cap/reactor bus marking
    /// ([`Self::mark_cap_and_reactor_buses`]), the error-1890 no-meter
    /// precondition, the `'A'`(ll)-vs-named-meter dispatch, and the error-262
    /// "EnergyMeter not found". The reduction *work itself* —
    /// `EnergyMeter.ReduceZone` dispatching `ReduceAlgs.pas`
    /// (`DoReduceDefault`/`DoReduceShortLines`/…) → `TLineObj.MergeWith` — is
    /// NOT_PORTED (the 210-line line merge is unported), so a resolved meter
    /// records a deferral instead of reducing its zone.
    fn do_reduce_cmd(&mut self) {
        // Pascal reads the next parm and uppercases it (`AnsiUpperCase`).
        self.parser.next_param(&self.vars);
        let mut param = self.parser.make_string(&self.vars).to_uppercase();

        // Pascal marks cap/reactor buses Keep *before* the meter-count check.
        self.mark_cap_and_reactor_buses();

        let no_meters = self
            .circuit
            .as_ref()
            .expect("gated in command()")
            .energy_meters
            .is_empty();
        if no_meters {
            // Pascal error 1890.
            self.errors.push(
                "An energy meter is required to use this feature. Please check \
                 https://sourceforge.net/p/electricdss/code/HEAD/tree/trunk/Version8/Doc/Circuit%20Reduction%20for%20Version8.docx \
                 for examples."
                    .to_string(),
            );
            return;
        }

        // Pascal: empty arg defaults to 'A' (all meters).
        if param.is_empty() {
            param = "A".to_string();
        }

        if param.starts_with('A') {
            // All meters → ReduceZone on each (NOT_PORTED).
            self.errors.push(Self::reduce_deferred_msg());
            return;
        }

        // Named meter: resolve it (Pascal `MeterClass.SetActive(Param)`); a
        // miss is error 262, a hit would `ReduceZone` (NOT_PORTED → deferral).
        let found = {
            let Dss {
                classes, circuit, ..
            } = self;
            let ckt = circuit.as_ref().expect("gated in command()");
            let store = ClassStore { classes };
            ckt.energy_meters.iter().any(|&r| {
                store
                    .ckt_elem(r)
                    .cd()
                    .obj
                    .name()
                    .eq_ignore_ascii_case(&param)
            })
        };
        if found {
            self.errors.push(Self::reduce_deferred_msg());
        } else {
            // Pascal error 262 (echoes the uppercased name).
            self.errors
                .push(format!("EnergyMeter \"{param}\" not found."));
        }
    }

    /// The NOT_PORTED deferral logged when a `Reduce` would otherwise call
    /// `EnergyMeter.ReduceZone` (see [`Self::do_reduce_cmd`]).
    fn reduce_deferred_msg() -> String {
        "Reduce: circuit reduction is not ported yet (the zone line-merge \
         requires Line.MergeWith — deferred to a later phase)."
            .to_string()
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

    /// Pascal `DoBusCoordsCmd` (`ExecHelper.pas` l.2955): read a `bus, x, y`
    /// file (one bus per line, aux-parser delimiters) and set the coordinates
    /// on buses that exist; buses not in the circuit are silently ignored.
    /// `swap_xy` is the `LatLongCoords` variant (unported command).
    fn do_bus_coords_cmd(&mut self, swap_xy: bool) {
        self.parser.next_param(&self.vars);
        let param = self.parser.make_string(&self.vars);
        let path = self.current_dir.join(&param);
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                self.errors.push(format!(
                    "Bus Coordinate file \"{param}\" could not be read: {e}"
                ));
                return;
            }
        };
        let Dss {
            circuit,
            aux_parser,
            vars,
            errors,
            ..
        } = self;
        let ckt = circuit.as_mut().expect("gated in command()");
        for (lineno, line) in content.lines().enumerate() {
            aux_parser.set_cmd_string(line);
            aux_parser.next_param(vars);
            let bus_name = aux_parser.make_string(vars);
            let Some(ib) = ckt.bus_list.find(&bus_name) else {
                continue; // just ignore a bus that's not in the circuit
            };
            // Pascal reads both coordinates with DblValue; a malformed number
            // raises and aborts the whole file with error 275.
            aux_parser.next_param(vars);
            let first = aux_parser.make_double(vars);
            aux_parser.next_param(vars);
            let second = aux_parser.make_double(vars);
            let (Ok(first), Ok(second)) = (first, second) else {
                errors.push(format!(
                    "Bus Coordinate file: Error Reading Line {}",
                    lineno + 1
                ));
                return;
            };
            let bus = &mut ckt.buses[ib];
            if swap_xy {
                bus.y = first;
                bus.x = second;
            } else {
                bus.x = first;
                bus.y = second;
            }
            bus.coord_defined = true;
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
        let save_in_redirect = self.in_redirect; // nested Redirects stay "inside"
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

        self.in_redirect = save_in_redirect;
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

/// A monitor's recorded buffer for the golden/test harness (dss-python
/// `Monitors.Header` / `SampleCount` / `Channel(i)` / `dblHour`).
#[derive(Debug, Clone)]
pub struct MonitorView {
    pub header: Vec<String>,
    pub sample_count: i32,
    /// Per-sample hour values (record slot 0).
    pub dbl_hour: Vec<f64>,
    /// `channels[i]` = the (i+1)-th channel across all samples (f32).
    pub channels: Vec<Vec<f32>>,
}

/// Raw `ElemRef` lists copied out of an [`energymeter::EnergyMeter`] before
/// resolving full names (avoids a long tuple type in [`Dss::meter_zone`]).
struct MeterZoneRefs {
    branches: Vec<ElemRef>,
    ends: Vec<ElemRef>,
    pce: Vec<ElemRef>,
    register_names: Vec<String>,
}

/// An EnergyMeter's zone topology for the test/golden harness (dss-python
/// `Meters.AllBranchesInZone` / `AllEndElements` / `ZonePCE`).
#[derive(Debug, Clone)]
pub struct MeterZoneView {
    /// `AllBranchesInZone`: the zone branches in `SequenceList` order (FullNames).
    pub all_branches_in_zone: Vec<String>,
    /// `AllEndElements`: the feeder-end branches (FullNames).
    pub all_end_elements: Vec<String>,
    /// `ZonePCE`: the zone PC elements (loads/generators), FullNames.
    pub zone_pce: Vec<String>,
    /// `RegisterNames` (length `NumEMRegisters`).
    pub register_names: Vec<String>,
}

/// Per-element snapshot for the golden feeder gate: mirrors dss-python's
/// `CktElement.Powers`/`Currents` over the oracle's `First/Next` iteration
/// (= creation) order.
#[derive(Debug, Clone)]
pub struct ElementSnapshot {
    /// `FullName` (`Class.name`).
    pub name: String,
    /// `Enabled`.
    pub enabled: bool,
    /// `BusNames`: the stored bus spec per terminal (`GetBus(i)`).
    pub bus_names: Vec<String>,
    /// kW/kvar interleaved per conductor and terminal (CAPI
    /// `Alt_CE_Get_Powers`: `GetPhasePower · 0.001`).
    pub powers: Vec<f64>,
    /// Amps, re/im interleaved per conductor and terminal (`Iterminal`).
    pub currents: Vec<f64>,
}

/// `(n, [(row, col, value)])` — the assembled, unfactored system Y as 0-based
/// coordinates, returned by [`Dss::system_y_csc`].
pub type SystemYCsc = (usize, Vec<(usize, usize, num_complex::Complex64)>);

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
            let cd = elem.cd();
            let bus_names = (1..=cd.nterms).map(|i| cd.get_bus(i).to_string()).collect();
            out.push(ElementSnapshot {
                name,
                enabled: cd.enabled,
                bus_names,
                powers,
                currents,
            });
        }
        out
    }

    /// Read a monitor's recorded data — the dss-python `Monitors.Header` /
    /// `SampleCount` / `Channel(i)` / `dblHour` surface (tests/goldens).
    /// `name` may be `"m1"` or `"Monitor.m1"` (case-insensitive). `None` if no
    /// such monitor exists.
    pub fn monitor_view(&self, name: &str) -> Option<MonitorView> {
        let bare = name
            .strip_prefix("Monitor.")
            .or_else(|| name.strip_prefix("monitor."))
            .unwrap_or(name);
        for class in &self.classes {
            for obj in &class.objects {
                if let Some(m) = obj.as_any().downcast_ref::<monitor::Monitor>()
                    && m.med.cd.obj.name().eq_ignore_ascii_case(bare)
                {
                    let nch = m.num_channels();
                    return Some(MonitorView {
                        header: m.header().to_vec(),
                        sample_count: m.sample_count(),
                        dbl_hour: m.dbl_hour(),
                        channels: (1..=nch).map(|i| m.channel(i)).collect(),
                    });
                }
            }
        }
        None
    }

    /// An EnergyMeter's zone topology — the dss-python `Meters.AllBranchesInZone`
    /// / `AllEndElements` / `ZonePCE` / `CountBranches` surface (tests/goldens).
    /// `name` may be `"m1"` or `"EnergyMeter.m1"` (case-insensitive). `None` if
    /// no such meter exists.
    pub fn meter_zone(&self, name: &str) -> Option<MeterZoneView> {
        let bare = name
            .strip_prefix("EnergyMeter.")
            .or_else(|| name.strip_prefix("energymeter."))
            .unwrap_or(name);
        // Locate the meter, copy out its ElemRef lists, then resolve full names.
        let mut lists: Option<MeterZoneRefs> = None;
        for class in &self.classes {
            for obj in &class.objects {
                if let Some(em) = obj.as_any().downcast_ref::<energymeter::EnergyMeter>()
                    && em.data().name().eq_ignore_ascii_case(bare)
                {
                    lists = Some(MeterZoneRefs {
                        branches: em.sequence_list().to_vec(),
                        ends: em.zone_end_elements(),
                        pce: em.zone_pce().to_vec(),
                        register_names: em.register_names().to_vec(),
                    });
                }
            }
        }
        let lists = lists?;
        let full_name = |r: ElemRef| -> String {
            let cn = self.classes[r.cls].props.class_name();
            format!(
                "{}.{}",
                cn,
                self.classes[r.cls].objects[r.idx].data().name()
            )
        };
        Some(MeterZoneView {
            all_branches_in_zone: lists.branches.iter().map(|&r| full_name(r)).collect(),
            all_end_elements: lists.ends.iter().map(|&r| full_name(r)).collect(),
            zone_pce: lists.pce.iter().map(|&r| full_name(r)).collect(),
            register_names: lists.register_names,
        })
    }

    /// An EnergyMeter's register values paired with names — the dss-python
    /// `Meters.RegisterValues` / `RegisterNames` surface (tests/goldens).
    /// `name` may be `"m1"` or `"EnergyMeter.m1"` (case-insensitive).
    pub fn meter_registers(&self, name: &str) -> Option<Vec<(String, f64)>> {
        let bare = name
            .strip_prefix("EnergyMeter.")
            .or_else(|| name.strip_prefix("energymeter."))
            .unwrap_or(name);
        for class in &self.classes {
            for obj in &class.objects {
                if let Some(em) = obj.as_any().downcast_ref::<energymeter::EnergyMeter>()
                    && em.data().name().eq_ignore_ascii_case(bare)
                {
                    return Some(
                        em.register_names()
                            .iter()
                            .cloned()
                            .zip(em.registers().iter().copied())
                            .collect(),
                    );
                }
            }
        }
        None
    }

    /// A load's `(kWbase, FAllocationFactor)` by name — the oracle's
    /// `Loads.kW` / `Loads.AllocationFactor` (test API for `allocateloads`).
    pub fn load_alloc(&self, name: &str) -> Option<(f64, f64)> {
        for class in &self.classes {
            for obj in &class.objects {
                if let Some(ld) = obj.as_any().downcast_ref::<load::Load>()
                    && ld.data().name().eq_ignore_ascii_case(name)
                {
                    return Some((ld.kw_base, ld.allocation_factor()));
                }
            }
        }
        None
    }

    /// A generator's `(kWbase, kvarBase)` by name — the oracle's
    /// `Generators.kW` / `Generators.kvar` (test API for GenDispatcher
    /// redispatch).
    pub fn generator_kw_kvar(&self, name: &str) -> Option<(f64, f64)> {
        for class in &self.classes {
            for obj in &class.objects {
                if let Some(g) = obj.as_any().downcast_ref::<generator::Generator>()
                    && g.data().name().eq_ignore_ascii_case(name)
                {
                    return Some((g.kw_base, g.kvar_base));
                }
            }
        }
        None
    }

    /// Test API for Pascal `TSensorObj.TakeSample`: drive the named sensor
    /// against the solved circuit and return its `(CalculatedCurrent,
    /// CalculatedVoltage)` per phase. (`TakeSample` is otherwise dead in the
    /// snapshot path — `SensorClass.SampleAll` is only invoked by the
    /// state-estimation API, which is a later phase — so this is the only gate
    /// that exercises the offset/`RotatePhases` math.)
    pub fn sensor_sample(
        &mut self,
        name: &str,
    ) -> Option<(Vec<num_complex::Complex64>, Vec<num_complex::Complex64>)> {
        let Dss {
            classes, circuit, ..
        } = self;
        let ckt = circuit.as_ref()?;
        let sensor_ref = ckt.sensors.iter().copied().find(|r| {
            classes[r.cls].objects[r.idx]
                .data()
                .name()
                .eq_ignore_ascii_case(name)
        })?;
        let metered = classes[sensor_ref.cls].objects[sensor_ref.idx]
            .as_any()
            .downcast_ref::<sensor::Sensor>()?
            .metered_element()?;
        let sys = crate::solution::solution::sys_ctx(ckt);
        let node_v = ckt.solution.node_v.clone();
        let mut store = ClassStore { classes };
        let (s_obj, m_obj) = store.pair_mut(sensor_ref, metered);
        let m_ce = m_obj
            .as_ckt_element_mut()
            .expect("metered element is a circuit element");
        let s = s_obj
            .as_any_mut()
            .downcast_mut::<sensor::Sensor>()
            .expect("sensors holds Sensor objects");
        s.take_sample(m_ce, &sys, &node_v);
        let nph = s.med.cd.nphases;
        Some((
            s.med.calculated_current[..nph].to_vec(),
            s.med.calculated_voltage[..nph].to_vec(),
        ))
    }

    /// Per-transformer winding taps in creation order, keyed by name —
    /// mirrors the oracle's `Transformers` loop (`tr.Wdg = i; tr.Tap`).
    pub fn transformer_taps(&self) -> Vec<(String, Vec<f64>)> {
        let Some(ckt) = self.circuit.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(ckt.transformers.len());
        for &r in &ckt.transformers {
            let obj = &self.classes[r.cls].objects[r.idx];
            let tr = obj
                .as_any()
                .downcast_ref::<transformer::Transformer>()
                .expect("transformers list holds Transformers");
            let n = tr.num_windings() as usize;
            let taps = (1..=n).map(|w| tr.present_tap(w)).collect();
            out.push((obj.data().name().to_string(), taps));
        }
        out
    }

    /// Per-RegControl `TapNumber` in creation order (the oracle's
    /// `RegControls` API).
    pub fn regcontrol_tap_numbers(&self) -> Vec<(String, i32)> {
        let Some(ckt) = self.circuit.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for &r in &ckt.controls {
            let obj = &self.classes[r.cls].objects[r.idx];
            if obj
                .as_any()
                .downcast_ref::<reg_control::RegControl>()
                .is_some()
            {
                out.push((
                    obj.data().name().to_string(),
                    obj.get_i32(reg_control::prop::TAPNUM),
                ));
            }
        }
        out
    }

    /// Per-capacitor `States` array in creation order (the oracle's
    /// `Capacitors` API — every Capacitor object, not just the shunt list).
    pub fn capacitor_states(&self) -> Vec<(String, Vec<i32>)> {
        let Some(cls) = self
            .classes
            .iter()
            .find(|c| c.props.class_name().eq_ignore_ascii_case("Capacitor"))
        else {
            return Vec::new();
        };
        cls.objects
            .iter()
            .map(|obj| {
                let states = obj
                    .get_i32_array(capacitor::prop::STATES)
                    .map(|s| s.to_vec())
                    .unwrap_or_default();
                (obj.data().name().to_string(), states)
            })
            .collect()
    }

    /// Terminal-1 closed flag of a capacitor by name (test API for the `Reset`
    /// controls path: `CapControl.Reset` drives the bank back to `InitialState`
    /// via `ControlledElement.Closed[0]`).
    pub fn capacitor_closed(&self, name: &str) -> Option<bool> {
        let cls = self
            .classes
            .iter()
            .find(|c| c.props.class_name().eq_ignore_ascii_case("Capacitor"))?;
        cls.objects
            .iter()
            .find(|o| o.data().name().eq_ignore_ascii_case(name))
            .and_then(|o| o.as_ckt_element())
            .map(|e| e.cd().all_conductors_closed())
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

    /// `DSS.ActiveCircuit.Solution.EventLog`: the accumulated event-log lines
    /// (empty when no circuit exists). The control loop (WP5.7) and the
    /// controls' `AppendToEventLog` (WP5.5/5.6) populate it.
    pub fn event_log(&self) -> &[String] {
        match &self.circuit {
            Some(ckt) => ckt.solution.event_log.entries(),
            None => &[],
        }
    }

    /// Coordinate dump of the **assembled, unfactored** system Y matrix:
    /// `(n, [(row, col, value)])`, 0-based, where row `i` corresponds to node
    /// `i + 1` (so `node_name(row + 1)` names the row). The values are
    /// pre-equilibration — the matrix exactly as stamped from element YPrims —
    /// so they line up with the oracle's `YMatrix.getYSparse(factor=False)`.
    /// `None` if no system Y has been built. Test/golden API (the assembled-model
    /// checkpoint of `golden_checkpoints.rs`).
    pub fn system_y_csc(&mut self) -> Option<SystemYCsc> {
        let ckt = self.circuit.as_mut()?;
        let y = ckt.solution.y_system.as_mut()?;
        let n = y.size();
        let (rows, cols, vals) = y.coo_entries().ok()?;
        let coords = rows
            .into_iter()
            .zip(cols)
            .zip(vals)
            .map(|((r, c), v)| (r, c, v))
            .collect();
        Some((n, coords))
    }

    /// An element's primitive admittance matrix `Yprim` as a **column-major**
    /// `yorder × yorder` flat array (`out[col * yorder + row]`) — the exact
    /// layout of the oracle's `CktElement.Yprim` (Pascal `TcMatrix`, column-major)
    /// and of `CMatrix`'s own storage, so the two compare without any transpose.
    /// `name` is a full `Class.name` (e.g. `"Transformer.reg1"`) when it
    /// contains a dot, else a bare object name matched across all classes
    /// (case-insensitive). `None` if no such element exists or it has no Yprim.
    /// Test/golden API (the selected-element checkpoint of `golden_checkpoints.rs`).
    pub fn element_yprim(&self, name: &str) -> Option<(usize, Vec<num_complex::Complex64>)> {
        let want_full = name.contains('.');
        for class in &self.classes {
            let cn = class.props.class_name();
            for obj in &class.objects {
                let matches = if want_full {
                    format!("{}.{}", cn, obj.data().name()).eq_ignore_ascii_case(name)
                } else {
                    obj.data().name().eq_ignore_ascii_case(name)
                };
                if !matches {
                    continue;
                }
                let Some(ce) = obj.as_ckt_element() else {
                    continue;
                };
                let cd = ce.cd();
                let yorder = cd.yorder;
                let yprim = cd.yprim.as_ref()?;
                let mut out = Vec::with_capacity(yorder * yorder);
                for col in 0..yorder {
                    for row in 0..yorder {
                        out.push(yprim.get(row, col));
                    }
                }
                return Some((yorder, out));
            }
        }
        None
    }

    /// The node injection-current vector the solver last used (`Solution.Currents`,
    /// the RHS of `Y·V = I`), length `num_nodes + 1` with slot 0 = ground —
    /// the oracle's `YMatrix.getI()` surface. Empty when no circuit exists.
    /// Test/golden API.
    pub fn node_injection_currents(&self) -> Vec<num_complex::Complex64> {
        match &self.circuit {
            Some(ckt) => ckt.solution.currents.clone(),
            None => Vec::new(),
        }
    }
}

impl Default for Dss {
    fn default() -> Self {
        Self::new()
    }
}

/// Pascal `interpretTimeStepSize` (`ExecOptions.pas` l.315): plain number =
/// seconds; otherwise a single-char `h`/`m`/`s` suffix. On error the step size
/// is left unchanged.
fn interpret_time_step_size(s: &str, current_h: f64, errors: &mut Vec<String>) -> f64 {
    if let Ok(v) = s.parse::<f64>() {
        return v; // only a number was specified, so must be seconds
    }
    // Error occurred, so must have a units specifier (the last character).
    let Some(ch) = s.chars().last() else {
        errors.push(format!("Error in specification of StepSize: {s}"));
        return current_h;
    };
    let s2 = &s[..s.len() - ch.len_utf8()];
    let Ok(v) = s2.parse::<f64>() else {
        errors.push(format!("Error in specification of StepSize: {s}"));
        return current_h;
    };
    match ch {
        'h' => v * 3600.0,
        'm' => v * 60.0,
        's' => v,
        _ => {
            errors.push(format!(
                "Error in specification of StepSize: \"{s}\". Units can only be h, m, or s (single char only)"
            ));
            current_h
        }
    }
}

/// `DSS.LoadShapeClass.Find(name)`, snapshot-cloned for the circuit defaults
/// (same staleness semantics as the Load/VSource shape refs — STATUS §1c).
fn find_load_shape(classes: &[DssClass], name: &str) -> Option<load_shape::LoadShapeObj> {
    let cls = classes
        .iter()
        .find(|c| c.props.class_name().eq_ignore_ascii_case("LoadShape"))?;
    let &idx = cls.name_to_idx.get(&name.to_lowercase())?;
    cls.objects[idx]
        .as_any()
        .downcast_ref::<load_shape::LoadShapeObj>()
        .cloned()
}

/// `DSS.PriceShapeClass.Find(name)`, snapshot-cloned (`Set pricecurve=`).
fn find_price_shape(classes: &[DssClass], name: &str) -> Option<price_shape::PriceShapeObj> {
    let cls = classes
        .iter()
        .find(|c| c.props.class_name().eq_ignore_ascii_case("PriceShape"))?;
    let &idx = cls.name_to_idx.get(&name.to_lowercase())?;
    cls.objects[idx]
        .as_any()
        .downcast_ref::<price_shape::PriceShapeObj>()
        .cloned()
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

/// Pascal `parseIntArray` (ExecOptions.pas l.350): reparse `s` on the AuxParser
/// into an integer array. Pascal runs two passes — pass 1 counts the tokens and
/// `SetLength`s the array (zero-filling), pass 2 reads each token via `IntValue`
/// (`MakeInteger`). A token that is neither an integer nor a roundable decimal
/// makes `MakeInteger` *raise* `EParserProblem`, which the executive logs and
/// which aborts the fill — leaving the already-sized array zero-filled from the
/// bad token onward. We reproduce that exactly: the error is recorded and the
/// remaining slots stay 0 (a roundable decimal like `13.7` still rounds to 14,
/// matching the `MakeInteger` double-fallback path).
fn parse_int_array(
    aux_parser: &mut Parser,
    vars: &ParserVars,
    s: &str,
    errors: &mut Vec<String>,
) -> Vec<i32> {
    // Pass 1: count the tokens (StrValue never raises).
    aux_parser.set_cmd_string(s);
    let mut count = 0usize;
    loop {
        aux_parser.next_param(vars);
        if aux_parser.make_string(vars).is_empty() {
            break;
        }
        count += 1;
    }

    // Pascal `SetLength(iarray, count)` — new slots are zero-filled.
    let mut out = vec![0i32; count];

    // Pass 2: read each token as an integer, stopping at the first conversion
    // error (Pascal raises and unwinds), leaving the remaining slots at 0.
    aux_parser.set_cmd_string(s);
    for slot in &mut out {
        aux_parser.next_param(vars);
        match aux_parser.make_integer(vars) {
            Ok(v) => *slot = v,
            Err(e) => {
                errors.push(e.message().to_string());
                break;
            }
        }
    }
    out
}

/// Pascal `TExecHelper.DoAutoAddBusList` (ExecHelper.pas l.1986): parse the
/// `Set AutoBusList=` argument — either an inline bus-name list or the
/// `File=name` form (one bus name per line, resolved against the data path).
fn do_auto_add_bus_list(
    aux_parser: &mut Parser,
    vars: &ParserVars,
    current_dir: &Path,
    s: &str,
    out: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    out.clear();
    aux_parser.set_cmd_string(s);
    let parm_name = aux_parser.next_param(vars);
    let mut param = aux_parser.make_string(vars);

    if parm_name.eq_ignore_ascii_case("file") {
        // Load the list from a file (one bus name per line).
        let path = current_dir.join(&param);
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                for line in content.lines() {
                    aux_parser.set_cmd_string(line);
                    aux_parser.next_param(vars);
                    let p = aux_parser.make_string(vars);
                    if !p.is_empty() {
                        out.push(p);
                    }
                }
            }
            // Pascal `DoSimpleMsg('Error trying to read bus list file: %s',
            // [E.message], 268)`.
            Err(e) => errors.push(format!("Error trying to read bus list file: {e}")),
        }
    } else {
        // Parse bus names off the inline array list.
        while !param.is_empty() {
            out.push(param.clone());
            aux_parser.next_param(vars);
            param = aux_parser.make_string(vars);
        }
    }
}

/// Pascal `DoSetReduceStrategy` (ExecHelper.pas l.3049): parse the
/// `Set ReduceOption=` value into a [`crate::circuit::ReductionStrategy`]. The
/// first character (case-insensitive) selects the mode; an `S` is
/// disambiguated Switch-vs-Shortlines by `CompareTextShortest(S, 'SWITCH')`.
/// The strategy is only stored — the reduction (`ReduceAlgs.pas`) is
/// `NOT_PORTED`.
fn set_reduce_strategy(ckt: &mut Circuit, s: &str, errors: &mut Vec<String>) {
    use crate::circuit::ReductionStrategy as Rs;
    ckt.reduction_strategy_string = s.to_string();
    ckt.reduction_strategy = Rs::Default;
    let Some(first) = s.bytes().next() else {
        return; // No option given
    };
    ckt.reduction_strategy = match first.to_ascii_uppercase() {
        b'B' => Rs::BreakLoop,
        b'D' => Rs::Default,
        b'E' => Rs::Dangling, // Ends
        b'L' => Rs::Laterals,
        b'M' => Rs::MergeParallel,
        b'S' => {
            if crate::util::compare_text_shortest_eq(s, "SWITCH") {
                Rs::Switches
            } else {
                Rs::ShortLines
            }
        }
        _ => {
            errors.push(format!("Unknown Reduction Strategy: \"{s}\"."));
            return; // leaves rsDefault, matching Pascal
        }
    };
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

/// Pascal `IntArrayToString` (Utilities.pas): `[NULL]` when empty, else
/// `[a, b, c]`.
fn int_array_to_string(arr: &[i32]) -> String {
    if arr.is_empty() {
        return "[NULL]".to_string();
    }
    let mut s = String::from("[");
    for (i, v) in arr.iter().enumerate() {
        if i != 0 {
            s.push_str(", ");
        }
        s.push_str(&v.to_string());
    }
    s.push(']');
    s
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

    /// Generator model 1 (constant PQ) injects negative load: a 100 kW / pf
    /// 0.95 generator delivers −33.333 kW, −10.956 kvar per phase (oracle
    /// dss-python 0.15.7, stiff source + short line).
    #[test]
    fn generator_model1_pq_snapshot() {
        let mut dss = Dss::new();
        dss.command(
            "New circuit.t1 basekv=12.47 bus1=sourcebus pu=1.0 \
             r1=0 x1=0.0001 r0=0 x0=0.0001",
        );
        dss.command(
            "New Line.l1 bus1=sourcebus bus2=genbus length=1 \
             r1=0.01 x1=0.01 r0=0.01 x0=0.01 c1=0 c0=0",
        );
        dss.command("New Generator.g1 bus1=genbus kV=12.47 kW=100 PF=0.95 model=1 conn=wye");
        dss.command("Set controlmode=off");
        dss.command("Solve");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        assert!(dss.circuit().unwrap().is_solved);

        let snap = dss.snapshot_elements();
        let g = snap
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case("Generator.g1"))
            .expect("generator snapshot");
        for ph in 0..3 {
            assert!(
                (g.powers[2 * ph] - (-33.333333)).abs() < 1e-3,
                "phase {ph} P {}",
                g.powers[2 * ph]
            );
            assert!(
                (g.powers[2 * ph + 1] - (-10.956137)).abs() < 1e-3,
                "phase {ph} Q {}",
                g.powers[2 * ph + 1]
            );
        }
    }

    /// Generator model 3 (constant P, |V|) exercises the DQDV var-control
    /// machinery (`SetGeneratordQdV`): a 300 kW PV generator holds |V| ≈ 1 pu
    /// and absorbs/produces vars to do it, landing at −100.003 kW, −64.728
    /// kvar per phase (oracle dss-python 0.15.7).
    #[test]
    fn generator_model3_pv_snapshot() {
        let mut dss = Dss::new();
        dss.command(
            "New circuit.t1 basekv=12.47 bus1=sourcebus pu=1.0 \
             r1=0 x1=0.0001 r0=0 x0=0.0001",
        );
        dss.command(
            "New Line.l1 bus1=sourcebus bus2=genbus length=1 \
             r1=0.05 x1=0.10 r0=0.05 x0=0.10 c1=0 c0=0",
        );
        dss.command("New Load.ld1 bus1=genbus kV=12.47 kW=500 PF=0.9 conn=wye model=1");
        dss.command(
            "New Generator.g1 bus1=genbus kV=12.47 kW=300 model=3 conn=wye \
             Vpu=1.0 maxkvar=200 minkvar=-200",
        );
        dss.command("Set controlmode=off");
        dss.command("Solve");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        assert!(dss.circuit().unwrap().is_solved);

        let snap = dss.snapshot_elements();
        let g = snap
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case("Generator.g1"))
            .expect("generator snapshot");
        for ph in 0..3 {
            assert!(
                (g.powers[2 * ph] - (-100.00275)).abs() < 1e-2,
                "phase {ph} P {}",
                g.powers[2 * ph]
            );
            assert!(
                (g.powers[2 * ph + 1] - (-64.7282)).abs() < 1e-2,
                "phase {ph} Q {}",
                g.powers[2 * ph + 1]
            );
        }
    }

    /// Parse the single number a `?` scalar query returns.
    fn query_f64(dss: &mut Dss, what: &str) -> f64 {
        query(dss, what).parse().expect("numeric query result")
    }

    /// A mode-0 (V&I) and mode-1 (powers) monitor on a 2-bus line sampled by a
    /// single daily step. Channel values transcribed from the oracle
    /// (dss-python 0.15.7): at hour 1 the flat default shape gives mult=1, so
    /// the sample equals the snapshot solution.
    #[test]
    fn monitor_mode0_mode1_daily() {
        let mut dss = Dss::new();
        dss.command("New circuit.test basekv=12.47 pu=1.0");
        dss.command("New line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.1 length=1");
        dss.command("New load.ld1 bus1=b2 phases=3 kv=12.47 kw=100 pf=0.95");
        dss.command("New monitor.m0 element=line.l1 terminal=1 mode=0");
        dss.command("New monitor.m1 element=line.l1 terminal=1 mode=1");
        dss.command("Set mode=daily number=1 stepsize=1h");
        dss.command("Solve");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());

        let m0 = dss.monitor_view("m0").expect("m0");
        assert_eq!(m0.sample_count, 1);
        assert_eq!(
            m0.header,
            vec![
                "hour", "t(sec)", "V1", "VAngle1", "V2", "VAngle2", "V3", "VAngle3", "I1",
                "IAngle1", "I2", "IAngle2", "I3", "IAngle3"
            ]
        );
        assert_eq!(m0.dbl_hour, vec![1.0]);
        // V1, VAngle1, I1, IAngle1 (channels 1,2,7,8 — 0-based 0,1,6,7).
        assert!(
            (m0.channels[0][0] - 7199.3564).abs() < 1e-2,
            "V1 {}",
            m0.channels[0][0]
        );
        assert!(
            (m0.channels[1][0] - (-0.0025524646)).abs() < 1e-4,
            "VAng1 {}",
            m0.channels[1][0]
        );
        assert!(
            (m0.channels[6][0] - 4.8712726).abs() < 1e-4,
            "I1 {}",
            m0.channels[6][0]
        );
        assert!(
            (m0.channels[7][0] - (-18.096796)).abs() < 1e-3,
            "IAng1 {}",
            m0.channels[7][0]
        );

        let m1 = dss.monitor_view("m1").expect("m1");
        assert_eq!(
            m1.header,
            vec![
                "hour", "t(sec)", "S1 (kVA)", "Ang1", "S2 (kVA)", "Ang2", "S3 (kVA)", "Ang3"
            ]
        );
        assert!(
            (m1.channels[0][0] - 35.070026).abs() < 1e-3,
            "S1 {}",
            m1.channels[0][0]
        );
        assert!(
            (m1.channels[1][0] - 18.094244).abs() < 1e-3,
            "Ang1 {}",
            m1.channels[1][0]
        );
    }

    /// A mode-5 (solution variables) monitor records the per-step solution
    /// state. Channels 11/12 are wall-clock timings (non-reproducible), so only
    /// the deterministic 1..10 are checked (oracle dss-python 0.15.7).
    #[test]
    fn monitor_mode5_solution_vars() {
        let mut dss = Dss::new();
        dss.command("New circuit.test basekv=12.47 pu=1.0");
        dss.command("New line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.1 length=1");
        dss.command("New load.ld1 bus1=b2 phases=3 kv=12.47 kw=100 pf=0.95");
        dss.command("New monitor.m5 element=line.l1 terminal=1 mode=5");
        dss.command("Set mode=daily number=1 stepsize=1h");
        dss.command("Solve");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());

        let m5 = dss.monitor_view("m5").expect("m5");
        assert_eq!(m5.sample_count, 1);
        let v = |i: usize| m5.channels[i][0];
        assert_eq!(v(2), 15.0); // MaxIterations
        assert_eq!(v(3), 10.0); // MaxControlIterations
        assert_eq!(v(4), 1.0); // Converged
        assert_eq!(v(5), 1.0); // IntervalHrs
        assert_eq!(v(6), 1.0); // SolutionCount
        assert_eq!(v(7), 1.0); // Mode = daily (ordinal 1)
        assert_eq!(v(8), 60.0); // Frequency
        assert_eq!(v(9), 0.0); // Year
    }

    /// The header-string modifier paths (±16 sequence / ±32 magnitude / ±64
    /// pos-seq, residual, VIpolar/Ppolar) match the oracle (dss-python 0.15.7).
    #[test]
    fn monitor_header_modifiers() {
        let mut dss = Dss::new();
        dss.command("New circuit.test basekv=12.47");
        dss.command("New line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.1");
        let hdr = |dss: &mut Dss, decl: &str| -> Vec<String> {
            dss.command(decl);
            dss.monitor_view("m").expect("m").header
        };
        assert_eq!(
            hdr(
                &mut dss,
                "New monitor.m element=line.l1 mode=0 residual=yes"
            ),
            vec![
                "hour", "t(sec)", "V1", "VAngle1", "V2", "VAngle2", "V3", "VAngle3", "VN",
                "VNAngle", "I1", "IAngle1", "I2", "IAngle2", "I3", "IAngle3", "IN", "INAngle"
            ]
        );
        assert_eq!(
            hdr(&mut dss, "Edit monitor.m mode=0 residual=no VIPolar=no"),
            vec![
                "hour", "t(sec)", "V1.re", "V1.im", "V2.re", "V2.im", "V3.re", "V3.im", "I1.re",
                "I1.im", "I2.re", "I2.im", "I3.re", "I3.im"
            ]
        );
        assert_eq!(
            hdr(&mut dss, "Edit monitor.m mode=16 VIPolar=yes"),
            vec![
                "hour", "t(sec)", "V0", "VAngle0", "V1", "VAngle1", "V2", "VAngle2", "I0",
                "IAngle0", "I1", "IAngle1", "I2", "IAngle2"
            ]
        );
        assert_eq!(
            hdr(&mut dss, "Edit monitor.m mode=32"),
            vec![
                "hour",
                "t(sec)",
                "|V|1 (volts)",
                "|V|2 (volts)",
                "|V|3 (volts)",
                "|I|1 (amps)",
                "|I|2 (amps)",
                "|I|3 (amps)"
            ]
        );
        assert_eq!(
            hdr(&mut dss, "Edit monitor.m mode=64"),
            vec!["hour", "t(sec)", "V1", "V1ang", "I1", "I1ang"]
        );
        assert_eq!(
            hdr(&mut dss, "Edit monitor.m mode=96"),
            vec!["hour", "t(sec)", "V", "I"]
        );
        assert_eq!(
            hdr(&mut dss, "Edit monitor.m mode=1 PPolar=no"),
            vec![
                "hour",
                "t(sec)",
                "P1 (kW)",
                "Q1 (kvar)",
                "P2 (kW)",
                "Q2 (kvar)",
                "P3 (kW)",
                "Q3 (kvar)"
            ]
        );
    }

    /// A mode-2 monitor records a transformer tap; a wrong element class errors.
    #[test]
    fn monitor_mode2_tap_and_class_check() {
        let mut dss = Dss::new();
        dss.command("New circuit.test basekv=12.47 pu=1.0");
        dss.command("New line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.1 length=1");
        dss.command(
            "New transformer.t1 phases=3 windings=2 buses=[b2 b3] conns=[wye wye] \
             kvs=[12.47 4.16] kvas=[1000 1000] xhl=5 tap=1.05",
        );
        dss.command("New load.ld1 bus1=b3 phases=3 kv=4.16 kw=100 pf=0.95");
        dss.command("New monitor.mt element=transformer.t1 terminal=2 mode=2");
        dss.command("Set mode=daily number=1 stepsize=1h");
        dss.command("Solve");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        let mt = dss.monitor_view("mt").expect("mt");
        assert_eq!(mt.header, vec!["hour", "t(sec)", "Tap (pu)"]);
        assert!(
            (mt.channels[0][0] - 1.05).abs() < 1e-5,
            "tap {}",
            mt.channels[0][0]
        );

        // Mode 2 on a line is rejected (Pascal 663).
        let mut bad = Dss::new();
        bad.command("New circuit.t basekv=12.47");
        bad.command("New line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.1");
        bad.command("New monitor.bad element=line.l1 mode=2");
        assert!(
            bad.errors()
                .iter()
                .any(|e| e.contains("is not a transformer")),
            "{:?}",
            bad.errors()
        );
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
    fn load_and_vsource_resolve_shape_refs() {
        // WP5.3: the shape refs became resolved `object_ref_class` props. The
        // ObjectRef getter renders the resolved object's name, and an unset
        // `yearly` is seeded from `daily` (Pascal `YearlyShapeObj := DailyShapeObj`).
        let mut dss = Dss::new();
        dss.command("New circuit.t basekv=12.47 bus1=src");
        dss.command("New loadshape.d1 npts=2 interval=1 mult=(0.4 0.8)");
        dss.command("New growthshape.g1 npts=2 year=(1 2) mult=(1.02 1.05)");
        dss.command("New load.la bus1=src phases=3 kv=12.47 kw=100 pf=1 daily=d1 growth=g1");
        dss.command("New vsource.v2 bus1=src basekv=12.47 daily=d1");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        assert_eq!(query(&mut dss, "load.la.daily"), "d1");
        assert_eq!(query(&mut dss, "load.la.yearly"), "d1"); // seeded from daily
        assert_eq!(query(&mut dss, "load.la.growth"), "g1");
        assert_eq!(query(&mut dss, "vsource.v2.daily"), "d1");
        assert_eq!(query(&mut dss, "vsource.v2.yearly"), "d1");

        // A missing shape is the Pascal 401 ("object not found") and leaves the
        // reference empty — the edit continues.
        dss.command("New load.lb bus1=src daily=nope");
        assert!(
            dss.errors().iter().any(|e| e.contains("not found")),
            "expected a not-found error, got {:?}",
            dss.errors()
        );
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

    #[test]
    fn line_geometry_undefined_wire_in_array_aborts() {
        // Pascal `DSSObjectReferenceArrayProperty` Exits on the first unresolved
        // token: the "not found" is logged and the write function (SetWires)
        // never runs, so nothing is stored and no spurious "Unexpected number"
        // count error fires.
        let mut dss = Dss::new();
        dss.command("New circuit.p");
        dss.command("New WireData.acsr Rdc=0.0526 GMRac=0.0244 GMRunits=ft radius=0.0306 radunits=ft normamps=530 Runits=ft");
        dss.command("New LineGeometry.g1 nconds=3 nphases=3 wires=[acsr bad acsr]");
        let errs = dss.errors();
        assert!(
            errs.iter().any(|e| e.contains("object \"bad\" not found")),
            "{errs:?}"
        );
        assert!(
            !errs.iter().any(|e| e.contains("Unexpected number")),
            "{errs:?}"
        );
        // Exit before the write function: no conductors were stored.
        assert_eq!(query(&mut dss, "LineGeometry.g1.wires"), "[, , ]");
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
            // Controls off: this test isolates the *structural* invariants
            // (node order, no Yprim stamping). With controls active the
            // RegControl legitimately adds control iterations (WP5.7).
            dss.command("Set controlmode=off");
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

    /// WP6.1: `BuildActiveBusAdjacencyLists` (CktTree.pas l.678) — non-shunt
    /// PD branches are listed at *every* terminal's bus; PC elements and
    /// shunt capacitors land on the terminal-1 PC list; sources (NON_PCPD)
    /// appear in neither.
    #[test]
    fn bus_adjacency_lists_bucket_elements() {
        use crate::circuit::ckt_tree::build_active_bus_adjacency_lists;

        let mut dss = Dss::new();
        dss.command("New circuit.adj basekv=12.47 pu=1.0 phases=3 mvasc3=2000");
        dss.command(
            "New line.l1 bus1=sourcebus bus2=b2 length=1 units=km \
             r1=0.1 x1=0.2 r0=0.3 x0=0.6 c1=0 c0=0",
        );
        dss.command("New capacitor.cap1 bus1=b2 kv=12.47 kvar=300");
        dss.command("New load.ld1 bus1=b2 phases=3 kv=12.47 kw=100 pf=0.95");
        dss.command("Solve");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());

        let Dss {
            classes, circuit, ..
        } = &mut dss;
        let ckt = circuit.as_ref().unwrap();
        let store = ClassStore { classes };
        let adj = build_active_bus_adjacency_lists(ckt, &store);

        let sb = ckt.bus_list.find("sourcebus").unwrap();
        let b2 = ckt.bus_list.find("b2").unwrap();
        let names = |refs: &[ElemRef]| -> Vec<String> {
            refs.iter()
                .map(|&r| store.ckt_elem(r).cd().obj.name().to_string())
                .collect()
        };

        // The line (non-shunt PD) shows up at both of its terminal buses.
        assert_eq!(names(&adj.pd[sb]), vec!["l1"]);
        assert_eq!(names(&adj.pd[b2]), vec!["l1"]);
        // PC list at b2: the load plus the shunt capacitor (PD element on
        // the PC list, in pc_elements-then-pd_elements build order), and
        // no source anywhere.
        assert_eq!(names(&adj.pc[b2]), vec!["ld1", "cap1"]);
        assert!(adj.pc[sb].is_empty(), "sources are NON_PCPD");
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

    /// AutoAdd option object defaults (`TAutoAdd.Init` + Circuit loss/UE
    /// defaults) echoed back through `Get`.
    #[test]
    fn autoadd_options_defaults_via_get() {
        let mut dss = Dss::new();
        dss.command("New circuit.c1");
        dss.command("Get genkw genpf capkvar addtype ueweight lossweight ueregs lossregs");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        assert_eq!(dss.result(), "1000, 1, 600, generator, 1, 1, [10], [13]");
    }

    /// `Set` the AutoAdd options, then verify both the circuit state and the
    /// `Get` echo (AddType maps to the lowercase device word).
    #[test]
    fn autoadd_options_set_then_get() {
        let mut dss = Dss::new();
        dss.command("New circuit.c1");
        dss.command(
            "Set genkw=500 genpf=0.95 capkvar=1200 addtype=capacitor \
             ueweight=2 lossweight=3 ueregs=[1,2,3] lossregs=[13,14]",
        );
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        {
            let ckt = dss.circuit().unwrap();
            assert_eq!(ckt.auto_add_obj.gen_kw, 500.0);
            assert_eq!(ckt.auto_add_obj.gen_pf, 0.95);
            assert_eq!(ckt.auto_add_obj.cap_kvar, 1200.0);
            assert_eq!(ckt.auto_add_obj.add_type, crate::circuit::CAPADD);
            assert_eq!(ckt.ue_weight, 2.0);
            assert_eq!(ckt.loss_weight, 3.0);
            assert_eq!(ckt.ue_regs, vec![1, 2, 3]);
            assert_eq!(ckt.loss_regs, vec![13, 14]);
        }
        dss.command("Get genkw genpf capkvar addtype ueweight lossweight ueregs lossregs");
        assert_eq!(
            dss.result(),
            "500, 0.95, 1200, capacitor, 2, 3, [1, 2, 3], [13, 14]"
        );
    }

    /// `Set UEregs=` with a non-numeric token reproduces the Pascal
    /// `MakeInteger` *raise*: the parser error is logged and the fill stops at
    /// the bad token, leaving the already-sized array zero-filled from there on
    /// (`[10, 0, 0]`, not a silent `[10, 0, 13]`). A roundable decimal still
    /// rounds (`13.7 -> 14`) via the double fallback.
    #[test]
    fn ueregs_nonnumeric_token_logs_error_and_truncates() {
        let mut dss = Dss::new();
        dss.command("New circuit.c1");
        dss.command("Set ueregs=(10 abc 13)");
        assert!(
            dss.errors()
                .iter()
                .any(|e| e.contains("Integer number conversion error")),
            "expected a logged conversion error, got {:?}",
            dss.errors()
        );
        assert_eq!(dss.circuit().unwrap().ue_regs, vec![10, 0, 0]);

        // The roundable-decimal path is unaffected (fresh circuit so the
        // error log above doesn't bleed into this assertion).
        let mut dss2 = Dss::new();
        dss2.command("New circuit.c2");
        dss2.command("Set lossregs=(13.7 14)");
        assert!(dss2.errors().is_empty(), "{:?}", dss2.errors());
        assert_eq!(dss2.circuit().unwrap().loss_regs, vec![14, 14]);
    }

    /// `Set addtype=` with an unrecognized value resolves to the enum default
    /// (CAPADD) with **no** error — Pascal `StringToOrdinal` returns the default
    /// rather than raising.
    #[test]
    fn addtype_unknown_falls_back_to_default_no_error() {
        let mut dss = Dss::new();
        dss.command("New circuit.c1");
        dss.command("Set addtype=foo");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        assert_eq!(
            dss.circuit().unwrap().auto_add_obj.add_type,
            crate::circuit::CAPADD
        );
        dss.command("Get addtype");
        assert_eq!(dss.result(), "capacitor");
    }

    /// `Set AutoBusList=` parses an inline bus-name list (`DoAutoAddBusList`),
    /// stored insertion-ordered and echoed comma-separated by `Get`.
    #[test]
    fn autoadd_bus_list_inline_round_trips() {
        let mut dss = Dss::new();
        dss.command("New circuit.c1");
        dss.command("Set autobuslist=[b1, b2, b3]");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        assert_eq!(
            dss.circuit().unwrap().auto_add_bus_list,
            vec!["b1".to_string(), "b2".to_string(), "b3".to_string()]
        );
        dss.command("Get autobuslist");
        assert_eq!(dss.result(), "b1, b2, b3");
    }

    /// The AutoAdd *solve mode* is `NOT_PORTED` (the capacity search needs
    /// aux-current injection + meter sampling). `Solve mode=autoadd` therefore
    /// still reports the unknown-mode error — the documented deferral.
    #[test]
    fn autoadd_solve_mode_still_deferred() {
        let mut dss = Dss::new();
        dss.command("New circuit.c1 basekv=12.47 bus1=src phases=3");
        dss.command("set voltagebases=[12.47]");
        dss.command("calcvoltagebases");
        dss.command("solve mode=autoadd");
        assert!(
            dss.errors()
                .iter()
                .any(|e| e.contains("Unknown solution mode")),
            "expected AutoAdd solve to remain deferred, got {:?}",
            dss.errors()
        );
    }

    /// `Set ReduceOption/Zmag/KeepLoad=` defaults + round-trip through `Get`.
    /// (ReduceOption's default string is empty, so `Get` elides it — exactly
    /// like Pascal `AppendGlobalResult` on a zero-length string.)
    #[test]
    fn reduce_options_defaults_and_round_trip() {
        let mut dss = Dss::new();
        dss.command("New circuit.c1");
        dss.command("Get zmag keepload");
        assert_eq!(dss.result(), "0.02, Yes");
        dss.command("Get reduceoption");
        assert_eq!(dss.result(), "");

        dss.command("Set reduceoption=shortlines zmag=0.05 keepload=no");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        {
            let ckt = dss.circuit().unwrap();
            assert_eq!(
                ckt.reduction_strategy,
                crate::circuit::ReductionStrategy::ShortLines
            );
            assert_eq!(ckt.reduction_strategy_string, "shortlines");
            assert_eq!(ckt.reduction_zmag, 0.05);
            assert!(!ckt.reduce_laterals_keep_load);
        }
        dss.command("Get reduceoption zmag keepload");
        assert_eq!(dss.result(), "shortlines, 0.05, No");
    }

    /// `DoSetReduceStrategy` dispatches on the first character; `S` resolves to
    /// Switch via `CompareTextShortest(S,'SWITCH')`, else ShortLines.
    #[test]
    fn reduce_strategy_first_char_dispatch() {
        use crate::circuit::ReductionStrategy as Rs;
        let mut dss = Dss::new();
        dss.command("New circuit.c1");
        let cases = [
            ("break", Rs::BreakLoop),
            ("default", Rs::Default),
            ("ends", Rs::Dangling),
            ("laterals", Rs::Laterals),
            ("merge", Rs::MergeParallel),
            ("switch", Rs::Switches),
            ("shortlines", Rs::ShortLines),
            ("s", Rs::Switches), // CompareTextShortest("s","SWITCH")=0 -> Switch
        ];
        for (opt, want) in cases {
            dss.command(&format!("Set reduceoption={opt}"));
            assert_eq!(dss.circuit().unwrap().reduction_strategy, want, "opt={opt}");
        }
        // Unknown strategy: error logged, strategy falls back to Default.
        dss.command("Set reduceoption=zzz");
        assert!(
            dss.errors()
                .iter()
                .any(|e| e.contains("Unknown Reduction Strategy")),
            "{:?}",
            dss.errors()
        );
        assert_eq!(dss.circuit().unwrap().reduction_strategy, Rs::Default);
    }

    /// `Reduce` with no energy meters reproduces Pascal error 1890, including
    /// the full documentation URL (pinned so an edit can't silently drift it).
    #[test]
    fn reduce_command_requires_energy_meter() {
        let mut dss = Dss::new();
        dss.command("New circuit.c1 basekv=12.47 bus1=src phases=3");
        dss.command("reduce");
        assert!(
            dss.errors().iter().any(|e| e
                == "An energy meter is required to use this feature. Please check \
                    https://sourceforge.net/p/electricdss/code/HEAD/tree/trunk/Version8/Doc/Circuit%20Reduction%20for%20Version8.docx \
                    for examples."),
            "{:?}",
            dss.errors()
        );
    }

    /// `Reduce <name>` with a meter present but no such meter reproduces Pascal
    /// error 262 (echoing the *uppercased* name), not the generic deferral.
    #[test]
    fn reduce_named_meter_not_found_is_262() {
        let mut dss = Dss::new();
        dss.command("New circuit.c1 basekv=12.47 bus1=src phases=3");
        dss.command("New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
        dss.command("New energymeter.m1 element=line.l1 terminal=1");
        dss.command("reduce nope");
        assert!(
            dss.errors()
                .iter()
                .any(|e| e == "EnergyMeter \"NOPE\" not found."),
            "{:?}",
            dss.errors()
        );
        // The deferral must NOT fire for a name that did not resolve.
        assert!(
            !dss.errors().iter().any(|e| e.contains("not ported")),
            "{:?}",
            dss.errors()
        );
    }

    /// `Reduce` marks enabled shunt cap/reactor buses as keepers *before* the
    /// meter check — so the marking happens even on the error-1890 path
    /// (Pascal `MarkCapandReactorBuses` runs unconditionally).
    #[test]
    fn reduce_marks_cap_and_reactor_buses() {
        let mut dss = Dss::new();
        dss.command("New circuit.c1 basekv=12.47 bus1=src phases=3");
        dss.command("New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
        dss.command("New line.l2 bus1=b1 bus2=b2 length=1 r1=0.3 x1=0.6");
        dss.command("New capacitor.c bus1=b1 phases=3 kvar=600 kv=12.47");
        dss.command("New reactor.r bus1=b2 phases=3 kvar=100 kv=12.47");
        dss.command("Set voltagebases=[12.47]");
        dss.command("CalcVoltageBases");
        // Bus refs are materialized at Y-build (solve) time in this port; a
        // real `Reduce` always runs post-solve (it needs metered zones).
        dss.command("Solve");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        // No energy meter → error 1890, but the marking still ran first.
        dss.command("reduce");
        let ckt = dss.circuit().unwrap();
        let keep = |name: &str| {
            ckt.buses
                .iter()
                .find(|b| b.name.eq_ignore_ascii_case(name))
                .map(|b| b.keep)
                .unwrap_or(false)
        };
        assert!(keep("b1"), "shunt capacitor bus should be a keeper");
        assert!(keep("b2"), "shunt reactor bus should be a keeper");
    }

    /// `Reduce` with a meter present passes the precondition but the zone
    /// reduction (`Line.MergeWith`) is NOT_PORTED — the documented deferral.
    #[test]
    fn reduce_command_with_meter_deferred() {
        let mut dss = Dss::new();
        dss.command("New circuit.c1 basekv=12.47 bus1=src phases=3");
        dss.command("New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
        dss.command("New energymeter.m1 element=line.l1 terminal=1");
        dss.command("reduce");
        assert!(
            dss.errors()
                .iter()
                .any(|e| e.contains("reduction is not ported")),
            "{:?}",
            dss.errors()
        );
    }

    /// Build the 2-bus regulator micro-circuit the WP5.7 oracle probes used.
    fn reg_two_bus(dss: &mut Dss, reg_props: &str) {
        dss.command("New circuit.ctl basekv=12.47 pu=1.0 phases=3 mvasc3=2000");
        dss.command(
            "New transformer.t1 phases=3 windings=2 buses=(sourcebus, b2) \
             conns=(delta wye) kvs=(12.47 4.16) kvas=(5000 5000) xhl=8",
        );
        dss.command(&format!("New regcontrol.r1 transformer=t1 {reg_props}"));
        dss.command("New load.l1 bus1=b2 phases=3 kv=4.16 kw=300 pf=0.95");
        dss.command("Set voltagebases=[12.47, 4.16]");
        dss.command("CalcVoltageBases");
    }

    /// WP5.7: the live control loop drives the regulator to the oracle's tap.
    /// Oracle probe (pinned dss-python): iterations=6, winding-2 tap=1.01875.
    #[test]
    fn control_loop_regulates_two_bus_to_oracle_tap() {
        let mut dss = Dss::new();
        reg_two_bus(&mut dss, "winding=2 vreg=122 band=0.0001 ptratio=20");
        dss.command("Solve");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        let ckt = dss.circuit().unwrap();
        assert!(ckt.is_solved);
        assert_eq!(ckt.solution.iteration, 6);
        let taps = dss.transformer_taps();
        assert_eq!(taps[0].0, "t1");
        assert!(
            (taps[0].1[1] - 1.01875).abs() < 1e-12,
            "winding-2 tap = {}",
            taps[0].1[1]
        );
    }

    /// WP5.7 step 5: a control that cannot settle within `maxcontroliter`
    /// stops with the 485 warning and aborts the solution. Oracle probe:
    /// `maxcontroliter=2` + `maxtapchange=1` → iterations=4, tap=1.00625,
    /// error 485; the next *external* command resets the abort flag
    /// (CAPI `Text_Set_Command`).
    #[test]
    fn max_control_iterations_exceeded_warns_and_aborts() {
        let mut dss = Dss::new();
        reg_two_bus(
            &mut dss,
            "winding=2 vreg=122 band=2 ptratio=20 maxtapchange=1",
        );
        dss.command("Set maxcontroliter=2");
        dss.command("Solve");
        assert!(
            dss.errors()
                .iter()
                .any(|e| e.starts_with("Warning Max Control Iterations Exceeded.")),
            "{:?}",
            dss.errors()
        );
        let ckt = dss.circuit().unwrap();
        assert_eq!(ckt.solution.iteration, 4);
        assert!(ckt.solution.solution_abort);
        let taps = dss.transformer_taps();
        assert!(
            (taps[0].1[1] - 1.00625).abs() < 1e-12,
            "winding-2 tap = {}",
            taps[0].1[1]
        );
        // External commands reset the abort (the oracle solves again and
        // exceeds again rather than reporting "Solution aborted.").
        dss.command("Get hour");
        assert!(!dss.circuit().unwrap().solution.solution_abort);
    }

    /// Build the 2-bus + line + load + two-generator micro-circuit the
    /// GenDispatcher oracle probes used.
    fn gen_disp_two_bus(dss: &mut Dss, gd_props: &str) {
        dss.command("New circuit.a basekv=12.47 bus1=src phases=3");
        dss.command("New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
        dss.command("New load.ld1 bus1=b1 phases=3 kv=12.47 kw=5000 pf=0.95");
        dss.command("New generator.g1 bus1=b1 phases=3 kv=12.47 kw=1000 pf=1.0 model=1");
        dss.command("New generator.g2 bus1=b1 phases=3 kv=12.47 kw=1000 pf=1.0 model=1");
        dss.command(&format!(
            "New gendispatcher.gd1 element=line.l1 terminal=1 {gd_props}"
        ));
        dss.command("Set voltagebases=[12.47]");
        dss.command("CalcVoltageBases");
    }

    /// WP6.8: the control loop's GenDispatcher redispatches its generators so
    /// the monitored line power approaches `kWLimit`. Oracle probe (pinned
    /// dss-python): equal weights → g1 = g2 = 1511.569498763734 kW.
    #[test]
    fn gendispatcher_redispatches_to_oracle() {
        let mut dss = Dss::new();
        gen_disp_two_bus(
            &mut dss,
            "kwlimit=2000 kwband=100 genlist=[g1,g2] weights=[1,1]",
        );
        dss.command("Solve mode=snap");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        for g in ["g1", "g2"] {
            let (kw, _kvar) = dss.generator_kw_kvar(g).unwrap();
            assert!((kw - 1511.569498763734).abs() < 1e-6, "{g} kW = {kw}");
        }
    }

    /// Weighted redispatch [3, 1]: g1 = 1767.3542481456006, g2 = 1255.7847493818672.
    #[test]
    fn gendispatcher_respects_weights_oracle() {
        let mut dss = Dss::new();
        gen_disp_two_bus(
            &mut dss,
            "kwlimit=2000 kwband=100 genlist=[g1,g2] weights=[3,1]",
        );
        dss.command("Solve mode=snap");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        let (kw1, _) = dss.generator_kw_kvar("g1").unwrap();
        let (kw2, _) = dss.generator_kw_kvar("g2").unwrap();
        assert!((kw1 - 1767.3542481456006).abs() < 1e-6, "g1 kW = {kw1}");
        assert!((kw2 - 1255.7847493818672).abs() < 1e-6, "g2 kW = {kw2}");
    }

    /// No GenList → dispatch every enabled generator (uniform weights); same
    /// result as the explicit equal-weight list.
    #[test]
    fn gendispatcher_no_list_dispatches_all_gens() {
        let mut dss = Dss::new();
        gen_disp_two_bus(&mut dss, "kwlimit=2000 kwband=100");
        dss.command("Solve mode=snap");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        for g in ["g1", "g2"] {
            let (kw, _) = dss.generator_kw_kvar(g).unwrap();
            assert!((kw - 1511.569498763734).abs() < 1e-6, "{g} kW = {kw}");
        }
    }

    /// WP6.8: the QDiff (kvar) redispatch path, exercised end-to-end. The gens
    /// run at `pf=0.95` so they carry a dispatchable `kvarBase`, and both
    /// `kWLimit` and `kvarLimit` bind. Oracle probe (pinned dss-python): equal
    /// weights → g1 = g2 = (1509.8126154343354 kW, 591.2600618943429 kvar).
    #[test]
    fn gendispatcher_redispatches_kvar_to_oracle() {
        let mut dss = Dss::new();
        dss.command("New circuit.a basekv=12.47 bus1=src phases=3");
        dss.command("New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
        dss.command("New load.ld1 bus1=b1 phases=3 kv=12.47 kw=5000 pf=0.95");
        dss.command("New generator.g1 bus1=b1 phases=3 kv=12.47 kw=1000 pf=0.95 model=1");
        dss.command("New generator.g2 bus1=b1 phases=3 kv=12.47 kw=1000 pf=0.95 model=1");
        dss.command(
            "New gendispatcher.gd1 element=line.l1 terminal=1 \
             kwlimit=2000 kwband=100 kvarlimit=500 genlist=[g1,g2] weights=[1,1]",
        );
        dss.command("Set voltagebases=[12.47]");
        dss.command("CalcVoltageBases");
        dss.command("Solve mode=snap");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        for g in ["g1", "g2"] {
            let (kw, kvar) = dss.generator_kw_kvar(g).unwrap();
            assert!((kw - 1509.8126154343354).abs() < 1e-6, "{g} kW = {kw}");
            assert!((kvar - 591.2600618943429).abs() < 1e-6, "{g} kvar = {kvar}");
        }
    }

    /// WP6.8: the monitored *terminal* is honored (not hard-wired to 1). A later
    /// `terminal=2` overrides the helper's `terminal=1`; terminal 2 of the line
    /// sits at the load/gen bus, so the measured power drives `PDiff` strongly
    /// negative and both gens floor at `Max(1.0, …)` — a result distinct from
    /// terminal 1's 1511.57 kW, which pins that the terminal index is read.
    /// Oracle probe (pinned dss-python): g1 = g2 = 1.0 kW.
    #[test]
    fn gendispatcher_honors_monitored_terminal() {
        let mut dss = Dss::new();
        gen_disp_two_bus(
            &mut dss,
            "kwlimit=2000 kwband=100 terminal=2 genlist=[g1,g2] weights=[1,1]",
        );
        dss.command("Solve mode=snap");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        for g in ["g1", "g2"] {
            let (kw, _) = dss.generator_kw_kvar(g).unwrap();
            assert!((kw - 1.0).abs() < 1e-6, "{g} kW = {kw}");
        }
    }

    /// WP6.8 StorageController skeleton: a circuit carrying a StorageController
    /// (whose fleet is always empty in Phase 6) must still solve — the control
    /// sweep treats it as an inert no-op. The only logged error is the faithful
    /// 37201 ("No unassigned Storage Elements found") emitted at parse-time
    /// RecalcElementData, exactly as the oracle reports on a Storage-less circuit.
    #[test]
    fn storagecontroller_skeleton_solves_as_noop() {
        let mut dss = Dss::new();
        dss.command("new circuit.a basekv=12.47 bus1=src phases=3");
        dss.command("new line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
        dss.command("new load.ld1 bus1=b1 phases=3 kv=12.47 kw=3000 pf=0.95");
        dss.command("new storagecontroller.sc1 element=line.l1 terminal=1");
        // The 37201 is logged during the New command; everything after solves.
        let errs: Vec<String> = dss.errors().to_vec();
        assert_eq!(errs.len(), 1, "{errs:?}");
        assert!(errs[0].contains("No unassigned Storage Elements found"));

        dss.command("set voltagebases=[12.47]");
        dss.command("calcvoltagebases");
        dss.command("solve mode=snap");
        // No *new* errors from the control loop; the circuit converged.
        assert_eq!(dss.errors().len(), 1, "{:?}", dss.errors());
        assert!(dss.circuit().unwrap().solution.converged_flag);
    }

    /// WP5.8 step 6: time-option round trips, all values transcribed from the
    /// pinned oracle.
    #[test]
    fn time_options_round_trip_matches_oracle() {
        let query = |dss: &mut Dss, what: &str| -> String {
            dss.command(&format!("get {what}"));
            dss.result().to_string()
        };
        let mut dss = Dss::new();
        dss.command("new circuit.t2");
        dss.command("set stepsize=15m");
        assert_eq!(query(&mut dss, "stepsize"), "900");
        dss.command("set hour=5");
        assert_eq!(query(&mut dss, "hour"), "5");
        dss.command("set sec=120.5");
        assert_eq!(query(&mut dss, "sec"), "120.5");
        dss.command("set time=(2, 1800)");
        assert_eq!(query(&mut dss, "time"), "[ 2, 1800 ] !... 2.5 (hours)");
        assert_eq!(query(&mut dss, "hour"), "2");
        assert_eq!(query(&mut dss, "sec"), "1800");
        dss.command("set mode=daily");
        assert_eq!(query(&mut dss, "number"), "24");
        assert_eq!(query(&mut dss, "stepsize"), "3600");
        // DUTYCYCLE forces TIMEDRIVEN control mode and h = 1 s.
        dss.command("set mode=duty");
        assert_eq!(query(&mut dss, "controlmode"), "Time");
        assert_eq!(query(&mut dss, "stepsize"), "1");
        // Circuit defaults resolve to the built-in `loadshape.default`.
        assert_eq!(query(&mut dss, "defaultdaily"), "default");
        assert_eq!(query(&mut dss, "defaultyearly"), "default");
        assert_eq!(query(&mut dss, "pricesignal"), "25");
        assert_eq!(query(&mut dss, "pricecurve"), "");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    }

    /// WP5.8: a daily-mode solve steps the clock through `number` steps.
    #[test]
    fn daily_mode_advances_the_clock_and_solves() {
        let mut dss = Dss::new();
        dss.command("New circuit.d1 basekv=12.47 pu=1.0 phases=3 mvasc3=2000");
        dss.command("New loadshape.two npts=2 interval=1 mult=(0.5 1.0)");
        dss.command("New line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.2 length=1");
        dss.command("New load.ld bus1=b2 phases=3 kv=12.47 kw=500 pf=0.95 daily=two");
        dss.command("Set voltagebases=[12.47]");
        dss.command("CalcVoltageBases");
        dss.command("Set mode=daily stepsize=1h number=2");
        dss.command("Solve");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        let ckt = dss.circuit().unwrap();
        assert!(ckt.is_solved);
        // IncrementTime runs before each step: after 2 steps dblHour = 2.0
        // (oracle probe: dblHour 2.0, Hour 2, Seconds 0.0).
        assert_eq!(ckt.solution.int_hour, 2);
        assert_eq!(ckt.solution.dbl_hour, 2.0);
        assert_eq!(ckt.solution.t, 0.0);
        // The built-in default daily shape drove DefaultHourMult (the value
        // itself is the WP5.2 oracle-pinned GetMultAtHour).
        let expected = ckt
            .default_daily_shape_obj
            .clone()
            .expect("default shape resolved")
            .get_mult_at_hour(2.0);
        assert_eq!(ckt.default_hour_mult, expected);
    }

    /// WP5.8 step 5: `BusCoords` reads `bus, x, y` rows, skipping unknown
    /// buses silently; coordinates survive on existing buses.
    #[test]
    fn bus_coords_sets_coordinates_on_existing_buses() {
        let dir = std::env::temp_dir().join("dss_rs_buscoords_test");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("coords.csv");
        std::fs::write(&file, "sourcebus, 10.5, -3\nnosuchbus, 1, 2\nb2 7 8\n").unwrap();

        let mut dss = Dss::new();
        dss.command("New circuit.bc basekv=12.47 pu=1.0 phases=3");
        dss.command("New line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.2 length=1");
        dss.command("Set voltagebases=[12.47]");
        dss.command("CalcVoltageBases"); // builds the bus list
        dss.command(&format!(
            "BusCoords \"{}\"",
            file.to_string_lossy().replace('\\', "/")
        ));
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        let ckt = dss.circuit().unwrap();
        let sb = ckt.bus_list.find("sourcebus").unwrap();
        assert!(ckt.buses[sb].coord_defined);
        assert_eq!((ckt.buses[sb].x, ckt.buses[sb].y), (10.5, -3.0));
        let b2 = ckt.bus_list.find("b2").unwrap();
        assert_eq!((ckt.buses[b2].x, ckt.buses[b2].y), (7.0, 8.0));
        std::fs::remove_file(&file).ok();
    }

    /// The micro radial of PHASE6_PLAN §1.2 `meter_zone_micro`: one meter on the
    /// head line walks the whole feeder. Zone branches/ends/PCE transcribed from
    /// the oracle (dss-python 0.15.7 `Meters.AllBranchesInZone` /
    /// `AllEndElements` / `ZonePCE`).
    fn micro_zone_script() -> Vec<&'static str> {
        vec![
            "New circuit.test basekv=12.47 bus1=src",
            "New line.l1 bus1=src bus2=b2 length=1",
            "New line.l2 bus1=b2 bus2=b3 length=2",
            "New line.l3 bus1=b2 bus2=b4 length=1",
            "New load.ld1 bus1=b3 kV=12.47 kW=100 numcust=3",
            "New load.ld2 bus1=b4 kV=12.47 kW=50 numcust=2",
        ]
    }

    #[test]
    fn energymeter_zone_radial() {
        let mut dss = Dss::new();
        for c in micro_zone_script() {
            dss.command(c);
        }
        dss.command("New energymeter.m1 element=line.l1 terminal=1");
        dss.command("Solve");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());

        let z = dss.meter_zone("m1").expect("m1 zone");
        assert_eq!(
            z.all_branches_in_zone,
            vec!["Line.l1", "Line.l3", "Line.l2"]
        );
        assert_eq!(z.all_end_elements, vec!["Line.l3", "Line.l2"]);
        assert_eq!(z.zone_pce, vec!["Load.ld2", "Load.ld1"]);

        // TotalUpDownstreamCustomers: ld1=3 on l2, ld2=2 on l3; l1 totals 5.
        assert_eq!(branch_customers(&dss, "line.l2"), (3, 3));
        assert_eq!(branch_customers(&dss, "line.l3"), (2, 2));
        assert_eq!(branch_customers(&dss, "line.l1"), (0, 5));
    }

    #[test]
    fn energymeter_submeter_boundary() {
        let mut dss = Dss::new();
        for c in micro_zone_script() {
            dss.command(c);
        }
        dss.command("New energymeter.m1 element=line.l1 terminal=1");
        dss.command("New energymeter.m2 element=line.l2 terminal=1");
        dss.command("Solve");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());

        // m1's zone stops at the sub-meter on l2.
        let z1 = dss.meter_zone("m1").expect("m1 zone");
        assert_eq!(z1.all_branches_in_zone, vec!["Line.l1", "Line.l3"]);
        assert_eq!(z1.all_end_elements, vec!["Line.l3"]);
        assert_eq!(z1.zone_pce, vec!["Load.ld2"]);

        let z2 = dss.meter_zone("m2").expect("m2 zone");
        assert_eq!(z2.all_branches_in_zone, vec!["Line.l2"]);
        assert_eq!(z2.all_end_elements, vec!["Line.l2"]);
        assert_eq!(z2.zone_pce, vec!["Load.ld1"]);
    }

    /// `element=` must resolve to a PD element; a load triggers the Pascal
    /// "is not a Power Delivery (PD) element" error (525).
    #[test]
    fn energymeter_requires_pd_element() {
        let mut dss = Dss::new();
        dss.command("New circuit.test basekv=12.47 bus1=src");
        dss.command("New line.l1 bus1=src bus2=b2 length=1");
        dss.command("New load.ld1 bus1=b2 kV=12.47 kW=100");
        dss.command("New energymeter.m1 element=load.ld1");
        assert!(
            dss.errors()
                .iter()
                .any(|e| e.contains("not a Power Delivery")),
            "expected PD-element error, got {:?}",
            dss.errors()
        );
    }

    /// Parallel lines (l2a ∥ l2b, both b2→b3): both still join the zone; the
    /// `IsParallel` flag is internal metadata, not an exclusion. Branch/end/PCE
    /// order transcribed from the oracle (`Meters.AllBranchesInZone` etc.).
    #[test]
    fn energymeter_parallel_lines() {
        let mut dss = Dss::new();
        dss.command("New circuit.test basekv=12.47 bus1=src");
        dss.command("New line.l1 bus1=src bus2=b2 length=1");
        dss.command("New line.l2a bus1=b2 bus2=b3 length=2");
        dss.command("New line.l2b bus1=b2 bus2=b3 length=2");
        dss.command("New load.ld1 bus1=b3 kV=12.47 kW=100 numcust=1");
        dss.command("New energymeter.m1 element=line.l1 terminal=1");
        dss.command("Solve");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());

        let z = dss.meter_zone("m1").expect("m1 zone");
        assert_eq!(
            z.all_branches_in_zone,
            vec!["Line.l1", "Line.l2b", "Line.l2a"]
        );
        assert_eq!(z.all_end_elements, vec!["Line.l2b", "Line.l2a"]);
        assert_eq!(z.zone_pce, vec!["Load.ld1"]);
    }

    /// A meshed zone (l4 closes b4→b2 back to the head): the loop branch is
    /// detected and not re-added, so the walk terminates. Order from the oracle.
    #[test]
    fn energymeter_loop_zone() {
        let mut dss = Dss::new();
        dss.command("New circuit.test basekv=12.47 bus1=src");
        dss.command("New line.l1 bus1=src bus2=b2 length=1");
        dss.command("New line.l2 bus1=b2 bus2=b3 length=1");
        dss.command("New line.l3 bus1=b3 bus2=b4 length=1");
        dss.command("New line.l4 bus1=b4 bus2=b2 length=1");
        dss.command("New energymeter.m1 element=line.l1 terminal=1");
        dss.command("Solve");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());

        let z = dss.meter_zone("m1").expect("m1 zone");
        assert_eq!(
            z.all_branches_in_zone,
            vec!["Line.l1", "Line.l4", "Line.l3", "Line.l2"]
        );
        assert_eq!(z.all_end_elements, vec!["Line.l3", "Line.l2"]);
        assert!(z.zone_pce.is_empty());
    }

    /// A transformer crossing voltage bases (12.47→0.48 kV) drives
    /// `AddToVoltBaseList` to two slots; `AssignVoltBaseRegisterNames` names the
    /// per-base loss registers (`%.3g kV …`) and fills the unused slots with
    /// `Aux<n>`. Register names + branch order transcribed from the oracle.
    #[test]
    fn energymeter_multi_vbase_register_names() {
        let mut dss = Dss::new();
        dss.command("New circuit.test basekv=12.47 bus1=src");
        dss.command("New line.l1 bus1=src bus2=b2 length=1");
        dss.command(
            "New transformer.tx phases=3 windings=2 buses=[b2, b3] \
             conns=[wye, wye] kvs=[12.47, 0.48] kvas=[500, 500] xhl=5",
        );
        dss.command("New line.l2 bus1=b3 bus2=b4 length=1");
        dss.command("New load.ld1 bus1=b4 kV=0.48 kW=100");
        dss.command("New energymeter.m1 element=line.l1 terminal=1");
        dss.command("Set voltagebases=[12.47, 0.48]");
        dss.command("CalcVoltageBases");
        dss.command("Solve");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());

        let z = dss.meter_zone("m1").expect("m1 zone");
        assert_eq!(
            z.all_branches_in_zone,
            vec!["Line.l1", "Transformer.tx", "Line.l2"]
        );
        // VBaseStart = 32; the two bases occupy slots 0/1, the rest are Aux.
        assert_eq!(z.register_names[32], "12.5 kV Losses");
        assert_eq!(z.register_names[33], "0.48 kV Losses");
        assert_eq!(z.register_names[34], "Aux1");
        assert_eq!(z.register_names[35], "Aux6");
        assert_eq!(z.register_names[39], "12.5 kV Line Loss");
        assert_eq!(z.register_names[40], "0.48 kV Line Loss");
    }

    /// Manual `ZoneList` zone build. NOTE: the oracle (dss_capi 0.14.5) raises an
    /// **access violation** on a manual zone, so there is no golden — this test
    /// locks our deterministic, memory-safe behavior, and guards against the
    /// path silently degrading back to a no-op. The listed PD element is chained
    /// as a child of the metered branch (no connectivity/feeder-ends).
    #[test]
    fn energymeter_manual_zonelist() {
        let mut dss = Dss::new();
        dss.command("New circuit.test basekv=12.47 bus1=src");
        dss.command("New line.l1 bus1=src bus2=b2 length=1");
        dss.command("New line.l2 bus1=b2 bus2=b3 length=2");
        dss.command("New line.lx bus1=b2 bus2=b9 length=1"); // not in the zonelist
        dss.command("New load.ld1 bus1=b3 kV=12.47 kW=100 numcust=3");
        dss.command("New energymeter.m1 element=line.l1 terminal=1 zonelist=[line.l2]");
        dss.command("Solve");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());

        let z = dss.meter_zone("m1").expect("m1 zone");
        // l2 is chained from l1; lx (not listed) is excluded. The downstream
        // load at l2's far bus is still collected.
        assert_eq!(z.all_branches_in_zone, vec!["Line.l1", "Line.l2"]);
        assert!(!z.all_branches_in_zone.iter().any(|b| b == "Line.lx"));
        assert_eq!(z.zone_pce, vec!["Load.ld1"]);
        // Manual zones populate no feeder ends (Pascal skips ZoneEndsList).
        assert!(z.all_end_elements.is_empty());
    }

    /// A terminal number past the metered element's terminal count is the Pascal
    /// 524 "Terminal no. ... does not exist" error.
    #[test]
    fn energymeter_bad_terminal() {
        let mut dss = Dss::new();
        dss.command("New circuit.test basekv=12.47 bus1=src");
        dss.command("New line.l1 bus1=src bus2=b2 length=1");
        dss.command("New energymeter.m1 element=line.l1 terminal=3");
        assert!(
            dss.errors().iter().any(|e| e.contains("does not exist")),
            "expected terminal-does-not-exist error, got {:?}",
            dss.errors()
        );
    }

    /// A disabled meter builds no zone (Pascal `BranchList := NIL`): the zone
    /// lists are empty. (The oracle errs #5501 on `AllBranchesInZone` here; we
    /// expose the empty zone instead of erroring.)
    #[test]
    fn energymeter_disabled_empty_zone() {
        let mut dss = Dss::new();
        dss.command("New circuit.test basekv=12.47 bus1=src");
        dss.command("New line.l1 bus1=src bus2=b2 length=1");
        dss.command("New load.ld1 bus1=b2 kV=12.47 kW=100");
        dss.command("New energymeter.m1 element=line.l1 terminal=1 enabled=no");
        dss.command("Solve");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());

        let z = dss.meter_zone("m1").expect("m1 exists");
        assert!(z.all_branches_in_zone.is_empty());
        assert!(z.zone_pce.is_empty());
    }

    /// Pascal gates `EndEdit` recalc on `NeedsRecalc`: a meter created without an
    /// `element` (or edited on an unrelated property) must NOT raise the
    /// "Circuit Element not set" error. Oracle: such a meter is created cleanly.
    #[test]
    fn energymeter_no_element_no_revalidation() {
        let mut dss = Dss::new();
        dss.command("New circuit.test basekv=12.47 bus1=src");
        dss.command("New line.l1 bus1=src bus2=b2 length=1");
        dss.command("New energymeter.mz"); // no element set
        assert!(
            dss.errors().is_empty(),
            "a bare meter must not error, got {:?}",
            dss.errors()
        );
        // Editing an unrelated property on a valid meter must not re-validate.
        dss.command("New load.ld1 bus1=b2 kV=12.47 kW=100");
        dss.command("New energymeter.m1 element=line.l1 terminal=1");
        dss.command("Edit energymeter.m1 kVANormal=5000");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    }

    /// Helper: `(BranchNumCustomers, BranchTotalCustomers)` for a named element.
    fn branch_customers(dss: &Dss, full: &str) -> (i32, i32) {
        let (cls, name) = full.split_once('.').unwrap();
        for class in &dss.classes {
            if !class.props.class_name().eq_ignore_ascii_case(cls) {
                continue;
            }
            for obj in &class.objects {
                if obj.data().name().eq_ignore_ascii_case(name)
                    && let Some(e) = obj.as_ckt_element()
                {
                    return (e.cd().branch_num_customers, e.cd().branch_total_customers);
                }
            }
        }
        panic!("element {full} not found");
    }

    /// Helper: fetch a meter register value by name.
    fn meter_reg(dss: &Dss, meter: &str, reg_name: &str) -> f64 {
        dss.meter_registers(meter)
            .unwrap_or_else(|| panic!("meter {meter} not found"))
            .into_iter()
            .find(|(n, _)| n == reg_name)
            .unwrap_or_else(|| panic!("register {reg_name} not found"))
            .1
    }

    /// Build the 2-bus daily case shared by the register tests. Loadshape ramps
    /// 1→2→3 over 3 one-hour steps; the meter is on the source line.
    fn daily_meter_case(trapezoidal: bool) -> Dss {
        let mut dss = Dss::new();
        dss.command("New circuit.t basekv=12.47 bus1=src");
        dss.command("New loadshape.ls npts=3 interval=1 mult=(1.0 2.0 3.0)");
        dss.command("New line.l1 bus1=src bus2=b2 length=1 r1=0.1 x1=0.1");
        dss.command("New load.ld1 bus1=b2 kV=12.47 kW=1000 pf=1 model=1 daily=ls");
        dss.command("New energymeter.m1 element=line.l1 terminal=1");
        dss.command("Set voltagebases=[12.47]");
        dss.command("CalcVoltageBases");
        // `Set mode=` resets the trapezoidal flag, so set it afterwards.
        dss.command("Set mode=daily number=3 stepsize=1h time=(0,0)");
        dss.command(if trapezoidal {
            "Set trapezoidal=yes"
        } else {
            "Set trapezoidal=no"
        });
        dss.command("Solve");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        dss
    }

    /// Plain Euler integration: `kWh = Σ interval·P`. Oracle (dss-python
    /// 0.15.7) register values for the 1→2→3 daily ramp.
    #[test]
    fn energymeter_daily_registers_euler() {
        let dss = daily_meter_case(false);
        let approx = |got: f64, want: f64| {
            assert!(
                (got - want).abs() <= 1e-6 * want.abs().max(1.0),
                "got {got}, want {want}"
            );
        };
        approx(meter_reg(&dss, "m1", "kWh"), 6009.009963043285);
        approx(meter_reg(&dss, "m1", "kvarh"), 8.436633138535129);
        approx(meter_reg(&dss, "m1", "Zone kWh"), 5999.979170340599);
        approx(meter_reg(&dss, "m1", "Max kW"), 3005.7947873123644);
        approx(meter_reg(&dss, "m1", "Line Losses"), 9.038717950944731);
        approx(
            meter_reg(&dss, "m1", "Zone Max kW Losses"),
            5.814433198962477,
        );
        // The single voltage base bucket carries the same line losses.
        approx(
            meter_reg(&dss, "m1", "12.5 kV Line Loss"),
            9.038717950944731,
        );
    }

    /// Trapezoidal integration: the first sample after reset is skipped, then
    /// `kWh += 0.5·interval·(P + P_prev)`. Same circuit, oracle values.
    #[test]
    fn energymeter_daily_registers_trapezoidal() {
        let dss = daily_meter_case(true);
        let approx = |got: f64, want: f64| {
            assert!(
                (got - want).abs() <= 1e-6 * want.abs().max(1.0),
                "got {got}, want {want}"
            );
        };
        approx(meter_reg(&dss, "m1", "kWh"), 4005.7905368432225);
        approx(meter_reg(&dss, "m1", "Zone kWh"), 3999.9865469246124);
        // Drag-hand maxima are independent of the integration rule.
        approx(meter_reg(&dss, "m1", "Max kW"), 3005.7947873123644);
        approx(
            meter_reg(&dss, "m1", "Zone Max kW Losses"),
            5.814433198962477,
        );
    }

    /// `Reset Meters` zeroes the registers and re-primes the drag-hand maxima to
    /// the large-negative sentinel.
    #[test]
    fn energymeter_reset_registers() {
        let mut dss = daily_meter_case(false);
        assert!(meter_reg(&dss, "m1", "kWh") > 1.0, "registers accumulated");
        dss.command("Reset Meters");
        assert_eq!(meter_reg(&dss, "m1", "kWh"), 0.0);
        assert_eq!(meter_reg(&dss, "m1", "Zone kWh"), 0.0);
        // Drag-hand registers reset to -1e50.
        assert_eq!(meter_reg(&dss, "m1", "Max kW"), -1.0e50);
        assert_eq!(meter_reg(&dss, "m1", "Zone Max kW Losses"), -1.0e50);
    }

    /// Helper used by the new register tests: build a 3-step daily case from a
    /// list of `New ...` commands, run it, and return the solved `Dss`. The
    /// loadshape `ls` (1→2→3) and the daily-mode/trapezoidal-off boilerplate are
    /// shared; callers pass the topology + `voltagebases`.
    fn meter_case(decls: &[&str], voltagebases: &str, extra_set: &[&str]) -> Dss {
        let mut dss = Dss::new();
        dss.command("New circuit.t basekv=12.47 bus1=src");
        dss.command("New loadshape.ls npts=3 interval=1 mult=(1.0 2.0 3.0)");
        for d in decls {
            dss.command(d);
        }
        dss.command(&format!("Set voltagebases=[{voltagebases}]"));
        dss.command("CalcVoltageBases");
        for s in extra_set {
            dss.command(s);
        }
        dss.command("Set mode=daily number=3 stepsize=1h time=(0,0)");
        dss.command("Set trapezoidal=no");
        dss.command("Solve");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        dss
    }

    fn approx_meter(dss: &Dss, reg_name: &str, want: f64) {
        let got = meter_reg(dss, "m1", reg_name);
        assert!(
            (got - want).abs() <= 1e-6 * want.abs().max(1.0),
            "{reg_name}: got {got}, want {want}"
        );
    }

    /// A generator in the zone accumulates the Gen registers (`Accumulate_Gen`:
    /// `−Power[1]·0.001` into the gen totals, *not* the zone-load totals). Oracle
    /// values for a 500 kW gen on the same 1→2→3 daily ramp.
    #[test]
    fn energymeter_generator_registers() {
        let dss = meter_case(
            &[
                "New line.l1 bus1=src bus2=b2 length=1 r1=0.1 x1=0.1",
                "New load.ld1 bus1=b2 kV=12.47 kW=1000 pf=1 model=1 daily=ls",
                "New generator.g1 bus1=b2 kV=12.47 kW=500 pf=1 model=1 daily=ls",
                "New energymeter.m1 element=line.l1 terminal=1",
            ],
            "12.47",
            &[],
        );
        approx_meter(&dss, "Gen kWh", 2999.9974045506274);
        approx_meter(&dss, "Gen kvarh", -0.0010792012877156054);
        approx_meter(&dss, "Gen Max kW", 1499.9981626190265);
        approx_meter(&dss, "Gen Max kVA", 1499.9981626191664);
        // Zone load is unaffected by the generator (gen has its own totals).
        approx_meter(&dss, "Zone kWh", 5999.994809101255);
    }

    /// 3-phase line sequence-mode loss split (`GetSeqLosses`, 3-phase only):
    /// balanced line ⇒ all loss in the positive/line mode, ~0 zero-mode.
    #[test]
    fn energymeter_sequence_mode_losses() {
        let dss = meter_case(
            &[
                "New line.l1 bus1=src bus2=b2 length=1 r1=0.1 x1=0.1",
                "New load.ld1 bus1=b2 kV=12.47 kW=1000 pf=1 model=1 daily=ls",
                "New energymeter.m1 element=line.l1 terminal=1",
            ],
            "12.47",
            &[],
        );
        approx_meter(&dss, "Line Mode Line Losses", 9.038717950944614);
        approx_meter(&dss, "3-phase Line Losses", 9.038717950944731);
        approx_meter(&dss, "1- and 2-phase Line Losses", 0.0);
        // Balanced ⇒ zero-sequence loss is numerically ~0 (1e-20).
        assert!(
            meter_reg(&dss, "m1", "Zero Mode Line Losses").abs() < 1e-9,
            "zero-mode loss should be ~0 for a balanced line"
        );
    }

    /// A transformer in the zone exercises the load/no-load loss split
    /// (`GetLosses` override) and the second voltage-base bucket (the 4.16 kV
    /// secondary, reached via line `l2`). Oracle values.
    #[test]
    fn energymeter_transformer_loss_split_and_vbase() {
        let dss = meter_case(
            &[
                "New line.l1 bus1=src bus2=b2 length=1 r1=0.1 x1=0.1",
                "New transformer.t1 windings=2 buses=(b2 b3) conns=(wye wye) \
                 kvs=(12.47 4.16) kvas=(2000 2000) xhl=5 %loadloss=1 %noloadloss=0.2",
                "New line.l2 bus1=b3 bus2=b4 length=0.5 r1=0.05 x1=0.05",
                "New load.ld1 bus1=b4 kV=4.16 kW=1000 pf=1 model=1 daily=ls",
                "New energymeter.m1 element=line.l1 terminal=1",
            ],
            "12.47 4.16",
            &[],
        );
        approx_meter(&dss, "Transformer Losses", 85.06511357026721);
        approx_meter(&dss, "Load Losses kWh", 103.96306991988196);
        approx_meter(&dss, "No Load Losses kWh", 11.675484334236636);
        approx_meter(&dss, "Line Losses", 30.57344068385137);
        // First voltage-base bucket (12.5 kV primary side): transformer split.
        approx_meter(&dss, "12.5 kV Load Loss", 73.389629);
        approx_meter(&dss, "12.5 kV No Load Loss", 11.675484);
        // Second voltage-base bucket (4.16 kV secondary): line l2 losses +
        // the load energy bucketed by its parent branch's voltage base.
        approx_meter(&dss, "4.16 kV Line Loss", 21.134364);
        approx_meter(&dss, "4.16 kV Load Energy", 5999.665065);
    }

    /// An under-rated line drives the overload registers and the *radial*
    /// EEN/UE marking (`ExcesskVANorm/Emerg` set `Overload_EEN/UE`, loads marked
    /// by the degree of overload). Oracle values.
    #[test]
    fn energymeter_overload_and_radial_een_ue() {
        let dss = meter_case(
            &[
                "New line.l1 bus1=src bus2=b2 length=1 r1=0.1 x1=0.1 normamps=50 emergamps=70",
                "New load.ld1 bus1=b2 kV=12.47 kW=1000 pf=1 model=1 daily=ls",
                "New energymeter.m1 element=line.l1 terminal=1",
            ],
            "12.47",
            &[],
        );
        approx_meter(&dss, "Overload kWh Normal", 2849.1631405361386);
        approx_meter(&dss, "Overload kWh Emerg", 1985.482037568384);
        approx_meter(&dss, "Load EEN", 7062.605747589624);
        approx_meter(&dss, "Load UE", 3616.152913382251);
    }

    /// A high-impedance line with ample current rating: no line overload, so the
    /// EEN/UE come from the load's *voltage* criterion (`ExceedsNormal`/
    /// `Unserved`, `VminNormal`/`VminEmerg` defaults). Oracle values.
    #[test]
    fn energymeter_voltage_een_ue() {
        let dss = meter_case(
            &[
                "New line.l1 bus1=src bus2=b2 length=1 r1=2 x1=2 normamps=2000 emergamps=3000",
                "New load.ld1 bus1=b2 kV=12.47 kW=4000 pf=1 model=1 daily=ls",
                "New energymeter.m1 element=line.l1 terminal=1",
            ],
            "12.47",
            &["Set normvminpu=0.95 emergvminpu=0.90"],
        );
        // No line overload ⇒ overload-energy registers stay 0.
        approx_meter(&dss, "Overload kWh Normal", 0.0);
        approx_meter(&dss, "Overload kWh Emerg", 0.0);
        // EEN/UE come purely from the voltage criterion.
        approx_meter(&dss, "Load EEN", 28188.694199630165);
        approx_meter(&dss, "Load UE", 11344.628473647135);
    }

    /// `Reset` (no argument) must reset controls too (Pascal `DoResetControls`):
    /// a CapControl that opened its bank during the solve has the bank driven
    /// back to its `InitialState` (closed) by the reset.
    #[test]
    fn reset_command_resets_controls() {
        let mut dss = Dss::new();
        dss.command("New circuit.t basekv=12.47 bus1=src");
        dss.command("New line.l1 bus1=src bus2=b2 length=1 r1=0.5 x1=1.0");
        dss.command("New load.ld1 bus1=b2 kV=12.47 kW=50 pf=0.99 model=1");
        dss.command("New capacitor.c1 bus1=b2 kV=12.47 kvar=600 numsteps=1");
        // kvar control opens the bank when the sensed kvar is below `offsetting`;
        // the tiny load keeps it below, so the solve switches the bank OUT.
        dss.command(
            "New capcontrol.cc1 element=line.l1 terminal=1 capacitor=c1 \
             type=kvar ptratio=1 onsetting=200 offsetting=100",
        );
        dss.command("Set voltagebases=[12.47]");
        dss.command("CalcVoltageBases");
        dss.command("Solve mode=snap");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        // The control opened the bank during the solve.
        assert_eq!(
            dss.capacitor_closed("c1"),
            Some(false),
            "control should have opened the bank"
        );
        // No-arg Reset must run DoResetControls → bank back to InitialState.
        dss.command("Reset");
        assert_eq!(
            dss.capacitor_closed("c1"),
            Some(true),
            "Reset must reset controls (close the bank to InitialState)"
        );
    }

    // --- WP6.6 reliability ------------------------------------------------

    /// Two-section radial feeder (src→b1→b2) with per-line fault data and a
    /// load on each section. Solved snapshot, EnergyMeter on the source line.
    /// `units=mi` keeps `len=1` (so the fault-rate math is unchanged) while
    /// making `MilesThisLine = 1` per line for the miles-accumulator assertions.
    fn reliability_feeder() -> Dss {
        let mut dss = Dss::new();
        dss.command("New circuit.t basekv=12.47 bus1=src phases=3");
        dss.command(
            "New line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1 \
             faultrate=0.2 pctperm=80 repair=4",
        );
        dss.command(
            "New line.l2 bus1=b1 bus2=b2 length=1 units=mi r1=0.1 x1=0.1 \
             faultrate=0.3 pctperm=90 repair=5",
        );
        dss.command("New load.ld1 bus1=b1 phases=3 kv=12.47 kw=100 numcust=10");
        dss.command("New load.ld2 bus1=b2 phases=3 kv=12.47 kw=200 numcust=25");
        dss.command("New energymeter.m1 element=line.l1 terminal=1");
        dss.command("Set voltagebases=[12.47]");
        dss.command("CalcVoltageBases");
        dss.command("Solve mode=snap");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        dss
    }

    /// Branching feeder: src→b1, then two laterals b1→b2 and b1→b3, exercising
    /// the parent customer roll-up at the junction bus b1.
    fn branching_reliability_feeder() -> Dss {
        let mut dss = Dss::new();
        dss.command("New circuit.t basekv=12.47 bus1=src phases=3");
        dss.command(
            "New line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1 \
             faultrate=0.2 pctperm=80",
        );
        dss.command(
            "New line.l2 bus1=b1 bus2=b2 length=1 units=mi r1=0.1 x1=0.1 \
             faultrate=0.3 pctperm=90",
        );
        dss.command(
            "New line.l3 bus1=b1 bus2=b3 length=1 units=mi r1=0.1 x1=0.1 \
             faultrate=0.5 pctperm=100",
        );
        dss.command("New load.ld1 bus1=b1 phases=3 kv=12.47 kw=100 numcust=10");
        dss.command("New load.ld2 bus1=b2 phases=3 kv=12.47 kw=200 numcust=25");
        dss.command("New load.ld3 bus1=b3 phases=3 kv=12.47 kw=150 numcust=7");
        dss.command("New energymeter.m1 element=line.l1 terminal=1");
        dss.command("Set voltagebases=[12.47]");
        dss.command("CalcVoltageBases");
        dss.command("Solve mode=snap");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        dss
    }

    fn bus_f64(dss: &Dss, bus: &str, f: impl Fn(&crate::circuit::bus::Bus) -> f64) -> f64 {
        let ckt = dss.circuit.as_ref().unwrap();
        let idx = ckt.bus_list.find(bus).expect("bus not found");
        f(&ckt.buses[idx])
    }

    fn bus_total_miles(dss: &Dss, bus: &str) -> f64 {
        bus_f64(dss, bus, |b| b.bus_total_miles)
    }

    fn bus_section_id(dss: &Dss, bus: &str) -> i32 {
        let ckt = dss.circuit.as_ref().unwrap();
        let idx = ckt.bus_list.find(bus).expect("bus not found");
        ckt.buses[idx].bus_section_id
    }

    fn accum_miles(dss: &Dss, full: &str) -> f64 {
        let (cls, name) = full.split_once('.').unwrap();
        for class in &dss.classes {
            if !class.props.class_name().eq_ignore_ascii_case(cls) {
                continue;
            }
            for obj in &class.objects {
                if obj.data().name().eq_ignore_ascii_case(name)
                    && let Some(e) = obj.as_ckt_element()
                {
                    return e.cd().accumulated_miles_downstream;
                }
            }
        }
        panic!("element {full} not found");
    }

    fn branch_section_id(dss: &Dss, full: &str) -> i32 {
        let (cls, name) = full.split_once('.').unwrap();
        for class in &dss.classes {
            if !class.props.class_name().eq_ignore_ascii_case(cls) {
                continue;
            }
            for obj in &class.objects {
                if obj.data().name().eq_ignore_ascii_case(name)
                    && let Some(e) = obj.as_ckt_element()
                {
                    return e.cd().branch_section_id;
                }
            }
        }
        panic!("element {full} not found");
    }

    fn meter_assume_restoration(dss: &Dss, name: &str) -> bool {
        for class in &dss.classes {
            for obj in &class.objects {
                if obj.data().name().eq_ignore_ascii_case(name)
                    && let Some(em) = obj
                        .as_any()
                        .downcast_ref::<crate::elements::meter::energymeter::EnergyMeter>()
                {
                    return em.assume_restoration();
                }
            }
        }
        panic!("meter {name} not found");
    }

    fn bus_flt_rate(dss: &Dss, bus: &str) -> f64 {
        let ckt = dss.circuit.as_ref().unwrap();
        let idx = ckt.bus_list.find(bus).expect("bus not found");
        ckt.buses[idx].bus_flt_rate
    }

    fn bus_total_custs(dss: &Dss, bus: &str) -> i32 {
        let ckt = dss.circuit.as_ref().unwrap();
        let idx = ckt.bus_list.find(bus).expect("bus not found");
        ckt.buses[idx].bus_total_num_customers
    }

    fn accum_flt_rate(dss: &Dss, full: &str) -> f64 {
        let (cls, name) = full.split_once('.').unwrap();
        for class in &dss.classes {
            if !class.props.class_name().eq_ignore_ascii_case(cls) {
                continue;
            }
            for obj in &class.objects {
                if obj.data().name().eq_ignore_ascii_case(name)
                    && let Some(e) = obj.as_ckt_element()
                {
                    return e.cd().accumulated_br_flt_rate;
                }
            }
        }
        panic!("element {full} not found");
    }

    /// With no OCP device (Relay/Recloser/Fuse — all Phase 7) the zone has zero
    /// sections, so `Relcalc` aborts with error 52902 exactly like the oracle
    /// (dss-python raises `DSSException (#52902)` on the same feeder).
    #[test]
    fn relcalc_no_ocp_device_aborts() {
        let mut dss = reliability_feeder();
        dss.command("Relcalc");
        assert!(
            dss.errors().iter().any(|e| e
                .contains("No Overcurrent Protection device (Relay, Recloser, or Fuse) defined")),
            "Relcalc must abort without OCP devices, got {:?}",
            dss.errors()
        );
    }

    /// Although `Relcalc` aborts, the backward fault-rate sweep and the
    /// up/downstream customer rollup run *before* the section check, so the bus
    /// and branch accumulators are populated. Hand-computed from the feeder:
    /// `BranchFltRate = FaultRate·pctperm·0.01·Len` (0.16 for l1, 0.27 for l2);
    /// `AccumulatedBrFltRate` rolls the downstream bus rate up each branch.
    #[test]
    fn relcalc_backward_sweep_accumulators() {
        let mut dss = reliability_feeder();
        dss.command("Relcalc");

        // l2: ToBus b2 has 0 fault rate → accumulated = its own 0.27.
        assert!((accum_flt_rate(&dss, "line.l2") - 0.27).abs() < 1e-12);
        // l1: ToBus b1 carries l2's 0.27 → accumulated = 0.27 + 0.16 = 0.43.
        assert!((accum_flt_rate(&dss, "line.l1") - 0.43).abs() < 1e-12);

        // FROM-bus accumulated failure rates (no OCP → roll up to FROM bus).
        assert!((bus_flt_rate(&dss, "src") - 0.43).abs() < 1e-12);
        assert!((bus_flt_rate(&dss, "b1") - 0.27).abs() < 1e-12);
        // b2 is a downstream (TO) bus only → never accumulated.
        assert!(bus_flt_rate(&dss, "b2").abs() < 1e-12);

        // Up/downstream customers: src sees all 35, b1 sees l2's 25.
        assert_eq!(bus_total_custs(&dss, "src"), 35);
        assert_eq!(bus_total_custs(&dss, "b1"), 25);
    }

    /// `AccumFltRate` also sweeps line miles: `AccumulatedMilesDownStream =
    /// ToBus.BusTotalMiles + MilesThisLine`, rolled up into `FromBus.BusTotalMiles`.
    /// With `units=mi length=1`, `MilesThisLine = 1` for each line.
    #[test]
    fn relcalc_miles_accumulators() {
        let mut dss = reliability_feeder();
        dss.command("Relcalc");

        // l2: ToBus b2 has 0 miles → accumulated = its own 1.0.
        assert!((accum_miles(&dss, "line.l2") - 1.0).abs() < 1e-12);
        // l1: ToBus b1 carries l2's 1.0 → accumulated = 1.0 + 1.0 = 2.0.
        assert!((accum_miles(&dss, "line.l1") - 2.0).abs() < 1e-12);

        // FROM-bus total miles roll up the same way.
        assert!((bus_total_miles(&dss, "src") - 2.0).abs() < 1e-12);
        assert!((bus_total_miles(&dss, "b1") - 1.0).abs() < 1e-12);
        // b2 is a TO-only end bus → never accumulated.
        assert!(bus_total_miles(&dss, "b2").abs() < 1e-12);
    }

    /// Junction roll-up: a single feeder (l1) splitting into two laterals
    /// (l2→b2, l3→b3). The junction bus b1 and the metered branch l1 must
    /// accumulate the failure rates and customers of *both* laterals.
    #[test]
    fn relcalc_branching_customer_rollup() {
        let mut dss = branching_reliability_feeder();
        dss.command("Relcalc");

        // Branch fault rates: l1=0.16, l2=0.27, l3=0.50.
        // b1 (junction) FROM-bus rate = l2 + l3 = 0.27 + 0.50 = 0.77.
        assert!((bus_flt_rate(&dss, "b1") - 0.77).abs() < 1e-12);
        // l1 accumulates b1's 0.77 plus its own 0.16 = 0.93; rolled to src.
        assert!((accum_flt_rate(&dss, "line.l1") - 0.93).abs() < 1e-12);
        assert!((bus_flt_rate(&dss, "src") - 0.93).abs() < 1e-12);

        // Customers: b1 totals both laterals' loads (25 + 7) → 32; src all 42.
        assert_eq!(bus_total_custs(&dss, "b1"), 32);
        assert_eq!(bus_total_custs(&dss, "src"), 42);
    }

    /// With no OCP device every zone bus and branch stays in section 0 (the
    /// pre-first-OCP section), and the forward sweep never increments
    /// `SectionCount`.
    #[test]
    fn relcalc_no_sections_without_ocp() {
        let mut dss = reliability_feeder();
        dss.command("Relcalc");

        for bus in ["src", "b1", "b2"] {
            assert_eq!(bus_section_id(&dss, bus), 0, "bus {bus} section");
        }
        for branch in ["line.l1", "line.l2"] {
            assert_eq!(
                branch_section_id(&dss, branch),
                0,
                "branch {branch} section"
            );
        }
    }

    /// The single positional `RelCalc` parameter is the `AssumeRestoration`
    /// yes/no flag (Pascal `pMeter.AssumeRestoration := AssumeRestoration`). It
    /// defaults FALSE and is stored on the meter for the customer roll-up.
    #[test]
    fn relcalc_assume_restoration_parsed() {
        let mut dss = reliability_feeder();
        // Default (no param) → FALSE.
        dss.command("Relcalc");
        assert!(!meter_assume_restoration(&dss, "m1"));

        // `Relcalc yes` → TRUE (still aborts: no OCP devices).
        dss.command("Relcalc yes");
        assert!(meter_assume_restoration(&dss, "m1"));
        assert!(
            dss.errors().iter().any(|e| e
                .contains("No Overcurrent Protection device (Relay, Recloser, or Fuse) defined")),
            "Relcalc yes must still abort without OCP devices, got {:?}",
            dss.errors()
        );
    }

    // ---- WP6.7: Sensor + load allocation -------------------------------------

    /// A radial feeder with an EnergyMeter at the head and two ConnectedkVA-spec
    /// loads. The meter's `SensorCurrent` defaults to 400 A, so `allocateloads`
    /// scales the zone loads to push the metered current toward that peak.
    fn allocation_feeder() -> Dss {
        let mut dss = Dss::new();
        dss.command("clear");
        dss.command("new circuit.a basekv=12.47 bus1=src phases=3");
        dss.command("new line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
        dss.command("new line.l2 bus1=b1 bus2=b2 length=1 r1=0.3 x1=0.6");
        dss.command("new load.ld1 bus1=b1 phases=3 kv=12.47 xfkva=500 allocationfactor=0.5 pf=0.9");
        dss.command("new load.ld2 bus1=b2 phases=3 kv=12.47 xfkva=800 allocationfactor=0.5 pf=0.9");
        dss.command("new energymeter.m1 element=line.l1 terminal=1");
        dss.command("set voltagebases=[12.47]");
        dss.command("calcvoltagebases");
        dss.command("solve mode=snap");
        assert!(dss.errors().is_empty(), "build errors: {:?}", dss.errors());
        dss
    }

    fn close_rel(a: f64, e: f64) -> bool {
        (a - e).abs() <= 1e-3 + 1e-4 * e.abs()
    }

    /// `allocateloads` with the default `MaxAllocationIterations = 2`. Values
    /// transcribed from the pinned oracle (`Loads.kW` / `AllocationFactor`).
    #[test]
    fn allocateloads_meter_drives_zone() {
        let mut dss = allocation_feeder();
        dss.command("allocateloads");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        let (kw1, f1) = dss.load_alloc("ld1").unwrap();
        let (kw2, f2) = dss.load_alloc("ld2").unwrap();
        assert!(close_rel(kw1, 2867.625566), "ld1 kW {kw1}");
        assert!(close_rel(kw2, 4588.200906), "ld2 kW {kw2}");
        assert!(close_rel(f1, 6.372501), "ld1 factor {f1}");
        assert!(close_rel(f2, 6.372501), "ld2 factor {f2}");
    }

    /// `Set NumAllocIterations=4` runs two more allocation passes, converging
    /// the loads slightly (oracle-pinned).
    #[test]
    fn allocateloads_honors_numallociterations() {
        let mut dss = allocation_feeder();
        dss.command("set numallociterations=4");
        dss.command("allocateloads");
        let (kw1, f1) = dss.load_alloc("ld1").unwrap();
        let (kw2, _) = dss.load_alloc("ld2").unwrap();
        assert!(close_rel(kw1, 2863.277886), "ld1 kW {kw1}");
        assert!(close_rel(kw2, 4581.244618), "ld2 kW {kw2}");
        assert!(close_rel(f1, 6.36284), "ld1 factor {f1}");
    }

    /// `Set AllocationFactors=X` sets every load's kVA allocation factor; for a
    /// ConnectedkVA-spec load `kWbase = xfkVA · factor · |pf|`.
    #[test]
    fn set_allocation_factors_scales_all_loads() {
        let mut dss = allocation_feeder();
        dss.command("set allocationfactors=0.8");
        let (kw1, f1) = dss.load_alloc("ld1").unwrap();
        let (kw2, f2) = dss.load_alloc("ld2").unwrap();
        assert!((kw1 - 360.0).abs() < 1e-9, "ld1 {kw1}"); // 500·0.8·0.9
        assert!((kw2 - 576.0).abs() < 1e-9, "ld2 {kw2}"); // 800·0.8·0.9
        assert!((f1 - 0.8).abs() < 1e-12);
        assert!((f2 - 0.8).abs() < 1e-12);
        // A non-positive factor is rejected (Pascal error 271).
        dss.command("set allocationfactors=0");
        assert!(
            dss.errors()
                .iter()
                .any(|e| e.contains("Allocation Factor must be greater than zero")),
            "{:?}",
            dss.errors()
        );
    }

    /// A Sensor on the mid-feeder line (measured `currents` set in a *separate*
    /// edit so they survive `RecalcElementData`'s `ZeroSensorArrays`) gives its
    /// downstream load its own allocation target; the meter still drives the
    /// upstream load. Oracle-pinned.
    #[test]
    fn allocateloads_with_sensor() {
        let mut dss = Dss::new();
        dss.command("clear");
        dss.command("new circuit.a basekv=12.47 bus1=src phases=3");
        dss.command("new line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
        dss.command("new line.l2 bus1=b1 bus2=b2 length=1 r1=0.3 x1=0.6");
        dss.command("new load.ld1 bus1=b1 phases=3 kv=12.47 xfkva=500 allocationfactor=0.5 pf=0.9");
        dss.command("new load.ld2 bus1=b2 phases=3 kv=12.47 xfkva=800 allocationfactor=0.5 pf=0.9");
        dss.command("new energymeter.m1 element=line.l1 terminal=1");
        dss.command("new sensor.s1 element=line.l2 terminal=1");
        dss.command("edit sensor.s1 currents=[20,20,20]");
        dss.command("set voltagebases=[12.47]");
        dss.command("calcvoltagebases");
        dss.command("solve mode=snap");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        dss.command("allocateloads");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        let (kw1, _) = dss.load_alloc("ld1").unwrap();
        let (kw2, _) = dss.load_alloc("ld2").unwrap();
        assert!(close_rel(kw1, 6780.125059), "ld1 kW {kw1}");
        assert!(close_rel(kw2, 382.584034), "ld2 kW {kw2}");
    }

    /// A bare Sensor (no `element=`) records the Pascal 666 error; defining the
    /// element makes it valid and adopts the line's phase count.
    #[test]
    fn sensor_requires_element() {
        let mut dss = Dss::new();
        dss.command("clear");
        dss.command("new circuit.a basekv=12.47 bus1=src phases=3");
        dss.command("new line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
        dss.command("new sensor.s1 terminal=1 kvbase=12.47");
        assert!(
            dss.errors()
                .iter()
                .any(|e| e.contains("Circuit Element is not set")),
            "bare sensor must error, got {:?}",
            dss.errors()
        );
    }

    // The replay scenarios below (kWh-spec, single-phase per-phase, P/Q-sensor)
    // are also covered by the data-driven gate `tests/golden_allocation.rs`;
    // they are kept here as well for clearer per-case failure messages.

    /// `allocateloads` over **kWh/Cfactor-spec** loads (`LoadSpec::KwhPf`): the
    /// allocation factor feeds `Set_AllocationFactor`'s `c_factor` branch, not
    /// `kva_allocation_factor`. Oracle-pinned `Loads.kW`/`AllocationFactor`.
    #[test]
    fn allocateloads_kwh_spec_loads() {
        let mut dss = Dss::new();
        dss.command("clear");
        dss.command("new circuit.a basekv=12.47 bus1=src phases=3");
        dss.command("new line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
        dss.command("new line.l2 bus1=b1 bus2=b2 length=1 r1=0.3 x1=0.6");
        dss.command("new load.ld1 bus1=b1 phases=3 kv=12.47 kwh=200000 cfactor=0.3 pf=0.9");
        dss.command("new load.ld2 bus1=b2 phases=3 kv=12.47 kwh=350000 cfactor=0.3 pf=0.9");
        dss.command("new energymeter.m1 element=line.l1 terminal=1");
        dss.command("set voltagebases=[12.47]");
        dss.command("calcvoltagebases");
        dss.command("solve mode=snap");
        dss.command("allocateloads");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        let (kw1, f1) = dss.load_alloc("ld1").unwrap();
        let (kw2, f2) = dss.load_alloc("ld2").unwrap();
        assert!(close_rel(kw1, 2710.478969), "ld1 kW {kw1}");
        assert!(close_rel(kw2, 4743.338196), "ld2 kW {kw2}");
        assert!(close_rel(f1, 9.757724), "ld1 cfactor {f1}");
        assert!(close_rel(f2, 9.757724), "ld2 cfactor {f2}");
    }

    /// `allocateloads` over **single-phase** loads on distinct phases: each load
    /// is scaled by its connected phase's `PhsAllocationFactor[ConnectedPhase]`
    /// (the meter is the sensor). Unbalanced xfkVA → distinct per-phase factors
    /// (an off-by-one in the phase index would cross-wire them). Oracle-pinned.
    #[test]
    fn allocateloads_single_phase_per_phase_factor() {
        let mut dss = Dss::new();
        dss.command("clear");
        dss.command("new circuit.a basekv=12.47 bus1=src phases=3");
        dss.command("new line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
        dss.command("new load.ld1 bus1=b1.1 phases=1 kv=7.2 xfkva=200 allocationfactor=0.5 pf=0.9");
        dss.command("new load.ld2 bus1=b1.2 phases=1 kv=7.2 xfkva=400 allocationfactor=0.5 pf=0.9");
        dss.command("new load.ld3 bus1=b1.3 phases=1 kv=7.2 xfkva=600 allocationfactor=0.5 pf=0.9");
        dss.command("new energymeter.m1 element=line.l1 terminal=1");
        dss.command("set voltagebases=[12.47,7.2]");
        dss.command("calcvoltagebases");
        dss.command("solve mode=snap");
        dss.command("allocateloads");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        let (kw1, f1) = dss.load_alloc("ld1").unwrap();
        let (kw2, f2) = dss.load_alloc("ld2").unwrap();
        let (kw3, f3) = dss.load_alloc("ld3").unwrap();
        // The factors are phase-distinct (13.9 / 6.95 / 4.63) — this is the part
        // that pins the connected-phase indexing.
        assert!(close_rel(f1, 13.902785), "ld1 factor {f1}");
        assert!(close_rel(f2, 6.951545), "ld2 factor {f2}");
        assert!(close_rel(f3, 4.634581), "ld3 factor {f3}");
        assert!(close_rel(kw1, 2502.501239), "ld1 kW {kw1}");
        assert!(close_rel(kw2, 2502.556163), "ld2 kW {kw2}");
        assert!(close_rel(kw3, 2502.673607), "ld3 kW {kw3}");
    }

    /// `allocateloads` driven by a **P/Q (kWs/kvars) Sensor**: the sensor's
    /// `UpdateCurrentVector` converts |S|/Vbase to a per-phase current target
    /// that then drives its downstream load, while the meter drives the
    /// upstream load. Oracle-pinned.
    #[test]
    fn allocateloads_pq_sensor() {
        let mut dss = Dss::new();
        dss.command("clear");
        dss.command("new circuit.a basekv=12.47 bus1=src phases=3");
        dss.command("new line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
        dss.command("new line.l2 bus1=b1 bus2=b2 length=1 r1=0.3 x1=0.6");
        dss.command("new load.ld1 bus1=b1 phases=3 kv=12.47 xfkva=500 allocationfactor=0.5 pf=0.9");
        dss.command("new load.ld2 bus1=b2 phases=3 kv=12.47 xfkva=800 allocationfactor=0.5 pf=0.9");
        dss.command("new energymeter.m1 element=line.l1 terminal=1");
        dss.command("new sensor.s1 element=line.l2 terminal=1 kvbase=12.47");
        // P/Q in a separate edit so they survive RecalcElementData's zeroing.
        dss.command("edit sensor.s1 kWs=[400,400,400] kvars=[200,200,200]");
        dss.command("set voltagebases=[12.47]");
        dss.command("calcvoltagebases");
        dss.command("solve mode=snap");
        dss.command("allocateloads");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        let (kw1, _) = dss.load_alloc("ld1").unwrap();
        let (kw2, _) = dss.load_alloc("ld2").unwrap();
        assert!(close_rel(kw1, 5431.462098), "ld1 kW {kw1}");
        assert!(close_rel(kw2, 1179.744649), "ld2 kW {kw2}");
    }

    /// Feeder shared by the `TakeSample` tests: a Sensor on line `l2` term 1
    /// (bus `b1`), one 3-phase load downstream, solved.
    fn sample_feeder(conn: &str) -> Dss {
        let mut dss = Dss::new();
        dss.command("clear");
        dss.command("new circuit.a basekv=12.47 bus1=src phases=3");
        dss.command("new line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
        dss.command("new line.l2 bus1=b1 bus2=b2 length=1 r1=0.3 x1=0.6");
        dss.command("new load.ld2 bus1=b2 phases=3 kv=12.47 kw=1000 pf=0.9");
        dss.command(&format!(
            "new sensor.s1 element=line.l2 terminal=1 kvbase=12.47 conn={conn}"
        ));
        dss.command("set voltagebases=[12.47]");
        dss.command("calcvoltagebases");
        dss.command("solve mode=snap");
        assert!(dss.errors().is_empty(), "build: {:?}", dss.errors());
        dss
    }

    fn cclose(a: num_complex::Complex64, re: f64, im: f64) -> bool {
        (a.re - re).abs() <= 1e-3 + 1e-4 * re.abs() && (a.im - im).abs() <= 1e-3 + 1e-4 * im.abs()
    }

    /// `TakeSample` (wye): `CalculatedCurrent` = the metered element's terminal-1
    /// currents; `CalculatedVoltage` = the terminal node voltages. Oracle-pinned.
    #[test]
    fn sensor_take_sample_wye() {
        let mut dss = sample_feeder("wye");
        let (curr, volt) = dss.sensor_sample("s1").unwrap();
        assert!(cclose(curr[0], 46.530078, -22.890880), "I0 {:?}", curr[0]);
        assert!(cclose(curr[1], -43.089122, -28.850790), "I1 {:?}", curr[1]);
        assert!(cclose(curr[2], -3.440956, 51.741669), "I2 {:?}", curr[2]);
        assert!(cclose(volt[0], 7169.263686, -24.130402), "V0 {:?}", volt[0]);
        assert!(
            cclose(volt[1], -3605.529385, -6196.699277),
            "V1 {:?}",
            volt[1]
        );
        assert!(
            cclose(volt[2], -3563.734300, 6220.829680),
            "V2 {:?}",
            volt[2]
        );
    }

    /// `TakeSample` (delta): `CalculatedVoltage[i] = VTerminal[i] -
    /// VTerminal[RotatePhases(i)]` (DeltaDirection +1 → L-L differences).
    /// Oracle-pinned (computed from the same node voltages).
    #[test]
    fn sensor_take_sample_delta() {
        let mut dss = sample_feeder("delta");
        let (_curr, volt) = dss.sensor_sample("s1").unwrap();
        assert!(
            cclose(volt[0], 10774.793071, 6172.568875),
            "V0 {:?}",
            volt[0]
        );
        assert!(
            cclose(volt[1], -41.795084, -12417.528957),
            "V1 {:?}",
            volt[1]
        );
        assert!(
            cclose(volt[2], -10732.997986, 6244.960082),
            "V2 {:?}",
            volt[2]
        );
    }
}

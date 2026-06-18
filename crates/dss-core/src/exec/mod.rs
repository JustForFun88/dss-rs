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

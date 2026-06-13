//! Port of `Meters/EnergyMeter.pas` — `TEnergyMeterObj`, the metering device
//! that accumulates energy/loss registers over a **zone** of the circuit
//! (everything downstream of its metered PD-element terminal).
//!
//! Like every meter element it has no Yprim (`CalcYPrim` is empty, `GetCurrents`
//! returns zeros); it joins the device list (so `ProcessBusDefs` allocates its
//! `NodeRef` from `SetBus(1, <metered bus>)`) and the circuit `energy_meters`
//! list, but never PD/PC.
//!
//! WP6.4 scope: the property table (1–24 + the `CktElementClass` tail), the
//! ctor register-name initialization, `MakeLike`, `RecalcElementData`,
//! `ResetRegisters`, and the zone-build surface (`MakeMeterZoneLists` /
//! `AddToVoltBaseList` / `AssignVoltBaseRegisterNames` / `GetPCEatZone`, driven
//! from [`crate::solution::meters`]). The register accumulation (`TakeSample`,
//! WP6.5) and reliability indices (WP6.6) land in later work packages.

use num_complex::Complex64;

use crate::circuit::ckt_tree::CktTree;
use crate::elements::ckt::CktElementData;
use crate::elements::meter::meter_element::MeterElementData;
use crate::elements::pd::capacitor::Capacitor;
use crate::elements::pd::line::Line;
use crate::elements::pd::reactor::Reactor;
use crate::elements::pd::transformer::Transformer;
use crate::elements::traits::{CktElement, ElemRef, SysCtx};
use crate::obj::base::{DssObjData, DssObject};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

/// `NumEMVbase` — voltage-base loss bucket count.
pub const NUM_EM_VBASE: usize = 7;
/// `NumEMRegisters = 32 + 5·NumEMVbase`.
pub const NUM_EM_REGISTERS: usize = 32 + 5 * NUM_EM_VBASE;
/// `ord(EMRegister.VBaseStart)`.
const VBASE_START: usize = 32;
/// `MaxVBaseCount = (NumEMRegisters - VBaseStart) div 5`.
const MAX_VBASE_COUNT: usize = (NUM_EM_REGISTERS - VBASE_START) / 5;

/// 1-based property ordinals (`TEnergyMeterProp` + the `TCktElementClass` tail).
pub mod prop {
    pub const ELEMENT: usize = 1;
    pub const TERMINAL: usize = 2;
    pub const ACTION: usize = 3;
    pub const OPTION: usize = 4;
    pub const KVA_NORMAL: usize = 5;
    pub const KVA_EMERG: usize = 6;
    pub const PEAK_CURRENT: usize = 7;
    pub const ZONE_LIST: usize = 8;
    pub const LOCAL_ONLY: usize = 9;
    pub const MASK: usize = 10;
    pub const LOSSES: usize = 11;
    pub const LINE_LOSSES: usize = 12;
    pub const XFMR_LOSSES: usize = 13;
    pub const SEQ_LOSSES: usize = 14;
    pub const THREE_PHASE_LOSSES: usize = 15;
    pub const VBASE_LOSSES: usize = 16;
    pub const PHASE_VOLTAGE_REPORT: usize = 17;
    pub const INT_RATE: usize = 18;
    pub const INT_DURATION: usize = 19;
    pub const SAIFI: usize = 20;
    pub const SAIFI_KW: usize = 21;
    pub const SAIDI: usize = 22;
    pub const CAIDI: usize = 23;
    pub const CUST_INTERRUPTS: usize = 24;
    // CktElementClass tail:
    pub const BASE_FREQ: usize = 25;
    pub const ENABLED: usize = 26;
    pub const NUM_PROPS: usize = 27; // incl. Like
}

/// 1-based register ordinals (`EMRegister`, EnergyMeter.pas l.120). Indexing
/// into [`EnergyMeter::registers`] subtracts 1.
pub mod reg {
    pub const KWH: usize = 1;
    pub const KVARH: usize = 2;
    pub const MAX_KW: usize = 3;
    pub const MAX_KVA: usize = 4;
    pub const ZONE_KWH: usize = 5;
    pub const ZONE_KVARH: usize = 6;
    pub const ZONE_MAX_KW: usize = 7;
    pub const ZONE_MAX_KVA: usize = 8;
    pub const OVERLOAD_KWH_NORM: usize = 9;
    pub const OVERLOAD_KWH_EMERG: usize = 10;
    pub const LOAD_EEN: usize = 11;
    pub const LOAD_UE: usize = 12;
    pub const ZONE_LOSSES_KWH: usize = 13;
    pub const ZONE_LOSSES_KVARH: usize = 14;
    pub const LOSSES_MAX_KW: usize = 15;
    pub const LOSSES_MAX_KVAR: usize = 16;
    pub const LOAD_LOSSES_KWH: usize = 17;
    pub const LOAD_LOSSES_KVARH: usize = 18;
    pub const NO_LOAD_LOSSES_KWH: usize = 19;
    pub const NO_LOAD_LOSSES_KVARH: usize = 20;
    pub const MAX_LOAD_LOSSES: usize = 21;
    pub const MAX_NO_LOAD_LOSSES: usize = 22;
    pub const LINE_LOSSES_KWH: usize = 23;
    pub const TRANSFORMER_LOSSES_KWH: usize = 24;
    pub const LINE_MODE_LINE_LOSS: usize = 25;
    pub const ZERO_MODE_LINE_LOSS: usize = 26;
    pub const THREE_PHASE_LINE_LOSS: usize = 27;
    pub const ONE_PHASE_LINE_LOSS: usize = 28;
    pub const GEN_KWH: usize = 29;
    pub const GEN_KVARH: usize = 30;
    pub const GEN_MAX_KW: usize = 31;
    pub const GEN_MAX_KVA: usize = 32;
    /// `EMRegister.VBaseStart` — anchor (0 added → first vbase reg is +1).
    pub const VBASE_START: usize = 32;
}

/// `TEnergyMeter.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    let defs = vec![
        // Pascal `DSSObjectReferenceProperty` with `PropertyOffset2 = 0`: any
        // circuit element by full name (defaults to the first circuit element).
        PropDef::object_ref_any("element"),
        PropDef::integer("terminal"),
        PropDef::action("action", enums.energy_meter_action),
        PropDef::string_list("option"),
        PropDef::double("kVANormal"),
        PropDef::double("kVAEmerg"),
        // `DoubleVArrayProperty` over `SensorCurrent` (length = nphases).
        PropDef::double_v_array("PeakCurrent"),
        PropDef::string_list("ZoneList"),
        PropDef::boolean("LocalOnly"),
        PropDef::double_f_array("Mask", NUM_EM_REGISTERS),
        PropDef::boolean("Losses"),
        PropDef::boolean("LineLosses"),
        PropDef::boolean("XfmrLosses"),
        PropDef::boolean("SeqLosses"),
        PropDef::boolean("3phaseLosses"),
        PropDef::boolean("VbaseLosses"),
        PropDef::boolean("PhaseVoltageReport"),
        PropDef::double("Int_Rate"),
        PropDef::double("Int_Duration"),
        // Read-only reliability props (Pascal `SilentReadOnly`); stored, never
        // written by the gate scenarios.
        PropDef::double("SAIFI"),
        PropDef::double("SAIFIkW"),
        PropDef::double("SAIDI"),
        PropDef::double("CAIDI"),
        PropDef::double("CustInterrupts"),
        // CktElementClass tail:
        PropDef::double("basefreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("enabled"),
    ];
    debug_assert_eq!(defs.len(), prop::NUM_PROPS - 1);
    ClassProps::new("EnergyMeter", defs, true)
}

/// Parse-time snapshot of the metered element (the WP4.7 `RefSnapshot`
/// pattern): `RecalcElementData` runs at `EndEdit` after the foreign-class view
/// is gone, so the parse-relevant shape is captured when `element=` resolves.
#[derive(Debug, Clone, Default)]
pub struct EmSnapshot {
    pub full_name: String,
    /// Whether the metered element is a Power Delivery element (Pascal
    /// `MeteredElement is TPDElement`).
    pub is_pd: bool,
    pub nphases: usize,
    pub nconds: usize,
    pub nterms: usize,
    /// 1-based terminal bus names (`buses[term - 1]`).
    pub buses: Vec<String>,
}

/// `TEnergyMeterObj`.
#[derive(Debug, Clone)]
pub struct EnergyMeter {
    pub med: MeterElementData,

    // Option flags (Pascal `SetOptions`/`GetOptions`).
    excess_flag: bool,
    zone_is_radial: bool,
    voltage_ue_only: bool,
    local_only: bool,

    // Loss-reporting switches (Pascal `FLosses`...).
    f_losses: bool,
    f_line_losses: bool,
    f_xfmr_losses: bool,
    f_seq_losses: bool,
    f_3phase_losses: bool,
    f_vbase_losses: bool,
    f_phase_voltage_report: bool,

    /// `DefinedZoneList` (manual zone specification).
    defined_zone_list: Vec<String>,

    max_zone_kva_norm: f64,
    max_zone_kva_emerg: f64,

    // Source reliability inputs (read-write doubles).
    source_num_interruptions: f64,
    source_int_duration: f64,

    // Reliability outputs (read-only; WP6.6 fills them).
    saifi: f64,
    saifi_kw: f64,
    saidi: f64,
    caidi: f64,
    cust_interrupts: f64,

    /// FullName of the metered element (`Class.name`) for the dump.
    element_full_name: String,
    /// Parse-time snapshot of the metered element.
    metered_snap: Option<EmSnapshot>,

    // Register machinery (WP6.5 fills TakeSample/Integrate).
    register_names: Vec<String>,
    registers: Vec<f64>,
    derivatives: Vec<f64>,
    totals_mask: Vec<f64>,
    first_sample_after_reset: bool,
    /// Pascal `Flg.NeedsRecalc`: set when `element`/`terminal` change, gates the
    /// `EndEdit` recalc so unrelated property edits don't re-run validation.
    needs_recalc: bool,

    // Voltage-base list (built by the zone walk).
    vbase_list: Vec<f64>,
    vbase_count: usize,

    // Zone topology (built by `MakeMeterZoneLists`). `None` until the zone is
    // built (Pascal `BranchList = NIL`).
    branch_list: Option<CktTree>,
    /// `SequenceList`: branches meter→ends in radial order (= the BranchList
    /// `GoForward` order).
    sequence_list: Vec<ElemRef>,
    /// Tree node index for each `sequence_list` entry (for shunt collection).
    sequence_nodes: Vec<usize>,
    /// `LoadList`: loads in the zone.
    load_list: Vec<ElemRef>,
    /// `ZoneEndsList` resolved to `(branch element, end bus)` pairs.
    zone_ends: Vec<(ElemRef, usize)>,
    /// `ZonePCE`: all PC elements in the zone (`GetPCEatZone`).
    zone_pce: Vec<ElemRef>,
}

impl EnergyMeter {
    /// Pascal `TEnergyMeterObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut med = MeterElementData::new(name, prop::NUM_PROPS);
        med.cd.nphases = 3;
        med.cd.nconds = 3;
        med.cd.set_nterms(1);
        med.metered_terminal = 1;
        // AllocateSensorArrays (calc arrays empty until a metered element is
        // adopted); SensorCurrent defaults to 400 A per phase.
        med.allocate_sensor_arrays(0);
        med.sensor_current = vec![400.0; med.cd.nphases];

        let mut em = Self {
            med,
            excess_flag: true,
            zone_is_radial: true,
            voltage_ue_only: false,
            local_only: false,
            f_losses: true,
            f_line_losses: true,
            f_xfmr_losses: true,
            f_seq_losses: true,
            f_3phase_losses: true,
            f_vbase_losses: true,
            f_phase_voltage_report: false,
            defined_zone_list: Vec::new(),
            max_zone_kva_norm: 0.0,
            max_zone_kva_emerg: 0.0,
            source_num_interruptions: 0.0,
            source_int_duration: 0.0,
            saifi: 0.0,
            saifi_kw: 0.0,
            saidi: 0.0,
            caidi: 0.0,
            cust_interrupts: 0.0,
            element_full_name: String::new(),
            metered_snap: None,
            register_names: default_register_names(),
            registers: vec![0.0; NUM_EM_REGISTERS],
            derivatives: vec![0.0; NUM_EM_REGISTERS],
            totals_mask: vec![1.0; NUM_EM_REGISTERS],
            first_sample_after_reset: true,
            needs_recalc: false,
            vbase_list: vec![0.0; MAX_VBASE_COUNT],
            vbase_count: 0,
            branch_list: None,
            sequence_list: Vec::new(),
            sequence_nodes: Vec::new(),
            load_list: Vec::new(),
            zone_ends: Vec::new(),
            zone_pce: Vec::new(),
        };
        em.reset_registers();
        em
    }

    // --- Read-only accessors for the zone API + class sweeps ---------------
    pub fn metered_element(&self) -> Option<ElemRef> {
        self.med.metered_element
    }
    pub fn metered_terminal(&self) -> i32 {
        self.med.metered_terminal
    }
    pub fn enabled(&self) -> bool {
        self.med.cd.enabled
    }
    pub fn register_names(&self) -> &[String] {
        &self.register_names
    }
    pub fn sequence_list(&self) -> &[ElemRef] {
        &self.sequence_list
    }
    pub fn load_list(&self) -> &[ElemRef] {
        &self.load_list
    }
    pub fn zone_pce(&self) -> &[ElemRef] {
        &self.zone_pce
    }
    /// `ZoneEndsList` resolved to the branch elements (Pascal `AllEndElements`).
    pub fn zone_end_elements(&self) -> Vec<ElemRef> {
        self.zone_ends.iter().map(|&(r, _)| r).collect()
    }
    pub fn has_branch_list(&self) -> bool {
        self.branch_list.is_some()
    }

    /// Pascal `TEnergyMeterObj.ResetRegisters`: zero the registers/derivatives
    /// and prime the drag-hand maxima to a large negative number.
    pub fn reset_registers(&mut self) {
        for r in &mut self.registers {
            *r = 0.0;
        }
        for d in &mut self.derivatives {
            *d = 0.0;
        }
        // Drag-hand registers (1-based EMRegister ordinals → 0-based here).
        for ord in [3, 4, 7, 8, 21, 22, 15, 16, 31, 32] {
            self.registers[ord - 1] = -1.0e50;
        }
        self.first_sample_after_reset = true;
    }

    /// Pascal `RecalcElementData`: validate the metered element (must be a PD
    /// element), check the terminal, and — when the element changed — adopt its
    /// phase/conductor counts, set the meter's bus and throw the branch list
    /// away (it is rebuilt by the next zone reset).
    pub fn recalc(&mut self, errors: &mut Vec<String>) {
        // Pascal `RecalcElementData` clears NeedsRecalc before validating.
        self.needs_recalc = false;
        let Some(snap) = self.metered_snap.clone() else {
            errors.push(format!(
                "EnergyMeter: \"{}\" Circuit Element not set. Element must be defined previously.",
                self.med.cd.obj.name()
            ));
            return;
        };
        if !snap.is_pd {
            errors.push(format!(
                "EnergyMeter: \"{}\" Circuit Element \"{}\" is not a Power Delivery (PD) element. Element must be a PD element.",
                self.med.cd.obj.name(),
                snap.full_name
            ));
            self.med.metered_element = None;
            return;
        }
        if self.med.metered_terminal as usize > snap.nterms {
            errors.push(format!(
                "EnergyMeter: \"{}\" Terminal no. \"{}\" does not exist. Respecify terminal no.",
                self.med.cd.obj.name(),
                self.med.metered_terminal
            ));
            return;
        }
        if self.med.metered_element_changed {
            let bus = snap
                .buses
                .get(self.med.metered_terminal as usize - 1)
                .cloned()
                .unwrap_or_default();
            self.med.cd.set_bus(1, &bus);
            self.med.cd.nphases = snap.nphases;
            self.med.cd.set_nconds(snap.nconds);
            self.med.allocate_sensor_arrays(snap.nphases * snap.nterms);
            self.branch_list = None;
            self.med.metered_element_changed = false;
        }
    }

    /// Pascal `AssignVoltBaseRegisterNames` (l.3082): name the per-voltage-base
    /// loss registers from the accumulated `VBaseList` (in line-to-line kV),
    /// filling unused slots with `Aux<n>`.
    fn assign_volt_base_register_names(&mut self) {
        let sqrt3 = 3.0_f64.sqrt();
        let mut ireg = 1;
        for i in 0..MAX_VBASE_COUNT {
            if self.vbase_list[i] > 0.0 {
                let vbase = self.vbase_list[i] * sqrt3;
                let base = VBASE_START + i; // 0-based slot of "<vbase> kV Losses"
                self.register_names[base] = format!("{} kV Losses", fmt_3g(vbase));
                self.register_names[base + MAX_VBASE_COUNT] =
                    format!("{} kV Line Loss", fmt_3g(vbase));
                self.register_names[base + 2 * MAX_VBASE_COUNT] =
                    format!("{} kV Load Loss", fmt_3g(vbase));
                self.register_names[base + 3 * MAX_VBASE_COUNT] =
                    format!("{} kV No Load Loss", fmt_3g(vbase));
                self.register_names[base + 4 * MAX_VBASE_COUNT] =
                    format!("{} kV Load Energy", fmt_3g(vbase));
            } else {
                for k in 0..5 {
                    self.register_names[VBASE_START + i + k * MAX_VBASE_COUNT] =
                        format!("Aux{ireg}");
                    ireg += 1;
                }
            }
        }
        // Pascal's trailing `Aux` loop spans
        // `1 + VBaseStart + 5·MaxVBaseCount .. NumEMRegisters`, which is empty
        // for the standard register layout (68..=67); kept for fidelity.
        #[allow(clippy::reversed_empty_ranges)]
        for i in (VBASE_START + 5 * MAX_VBASE_COUNT)..NUM_EM_REGISTERS {
            self.register_names[i] = format!("Aux{ireg}");
            ireg += 1;
        }
    }

    // --- Zone-build write-back (driven by `solution::meters`) --------------

    /// Install the freshly-built zone topology and (re)assign the voltage-base
    /// register names. `vbase_list`/`vbase_count` come from the walk's local
    /// `AddToVoltBaseList` accumulator.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn install_zone(
        &mut self,
        branch_list: Option<CktTree>,
        sequence_list: Vec<ElemRef>,
        sequence_nodes: Vec<usize>,
        load_list: Vec<ElemRef>,
        zone_ends: Vec<(ElemRef, usize)>,
        zone_pce: Vec<ElemRef>,
        vbase_list: Vec<f64>,
        vbase_count: usize,
    ) {
        self.branch_list = branch_list;
        self.sequence_list = sequence_list;
        self.sequence_nodes = sequence_nodes;
        self.load_list = load_list;
        self.zone_ends = zone_ends;
        self.zone_pce = zone_pce;
        self.vbase_list = vbase_list;
        self.vbase_count = vbase_count;
        self.assign_volt_base_register_names();
    }

    pub(crate) fn defined_zone_list(&self) -> &[String] {
        &self.defined_zone_list
    }

    /// Register values (1-based ordinals → 0-based slots). For the test API.
    pub fn registers(&self) -> &[f64] {
        &self.registers
    }

    pub fn has_been_sampled(&self) -> bool {
        !self.first_sample_after_reset
    }

    /// Pascal `CheckBranchList`: `TakeSample` exits early when the zone was
    /// never built. Returns the sweep state (config snapshot + the registers
    /// and branch tree moved out for the walk) or `None`.
    pub(crate) fn begin_take_sample(
        &mut self,
        trapezoidal: bool,
    ) -> Option<(CktTree, SampleState)> {
        let tree = self.branch_list.take()?;
        let state = SampleState {
            local_only: self.local_only,
            zone_is_radial: self.zone_is_radial,
            excess_flag: self.excess_flag,
            voltage_ue_only: self.voltage_ue_only,
            f_losses: self.f_losses,
            f_line_losses: self.f_line_losses,
            f_xfmr_losses: self.f_xfmr_losses,
            f_seq_losses: self.f_seq_losses,
            f_3phase_losses: self.f_3phase_losses,
            f_vbase_losses: self.f_vbase_losses,
            max_zone_kva_norm: self.max_zone_kva_norm,
            max_zone_kva_emerg: self.max_zone_kva_emerg,
            metered_element: self.med.metered_element,
            metered_terminal: self.med.metered_terminal.max(0) as usize,
            registers: std::mem::take(&mut self.registers),
            derivatives: std::mem::take(&mut self.derivatives),
            first_sample_after_reset: self.first_sample_after_reset,
            trapezoidal,
        };
        Some((tree, state))
    }

    /// Restore the branch tree and write back the accumulated registers.
    pub(crate) fn end_take_sample(&mut self, tree: CktTree, state: SampleState) {
        self.branch_list = Some(tree);
        self.registers = state.registers;
        self.derivatives = state.derivatives;
        self.first_sample_after_reset = state.first_sample_after_reset;
    }
}

/// Mutable sweep state for `TakeSample`: the config snapshot plus the register
/// accumulators moved out of the meter for the duration of the zone walk
/// (the meter object itself is borrowed by the element store during the walk).
pub(crate) struct SampleState {
    pub local_only: bool,
    pub zone_is_radial: bool,
    pub excess_flag: bool,
    pub voltage_ue_only: bool,
    pub f_losses: bool,
    pub f_line_losses: bool,
    pub f_xfmr_losses: bool,
    pub f_seq_losses: bool,
    pub f_3phase_losses: bool,
    pub f_vbase_losses: bool,
    pub max_zone_kva_norm: f64,
    pub max_zone_kva_emerg: f64,
    pub metered_element: Option<ElemRef>,
    pub metered_terminal: usize,
    pub registers: Vec<f64>,
    pub derivatives: Vec<f64>,
    pub first_sample_after_reset: bool,
    pub trapezoidal: bool,
}

impl SampleState {
    /// Pascal `TEnergyMeterObj.Integrate` (l.1271): trapezoidal rule when the
    /// circuit flag is set (skipped on the first sample after reset), else plain
    /// Euler. `reg` is the 1-based ordinal. Always records the derivative.
    pub(crate) fn integrate(&mut self, reg: usize, deriv: f64, interval: f64) {
        let i = reg - 1;
        if self.trapezoidal {
            if !self.first_sample_after_reset {
                self.registers[i] += 0.5 * interval * (deriv + self.derivatives[i]);
            }
        } else {
            self.registers[i] += interval * deriv;
        }
        self.derivatives[i] = deriv;
    }

    /// Pascal `TEnergyMeterObj.SetDragHandRegister` (l.2858): keep the running
    /// maximum.
    pub(crate) fn set_drag(&mut self, reg: usize, value: f64) {
        let i = reg - 1;
        if value > self.registers[i] {
            self.registers[i] = value;
            self.derivatives[i] = value;
        }
    }
}

/// Pascal `'%.3g'` formatting (used by `AssignVoltBaseRegisterNames`).
fn fmt_3g(v: f64) -> String {
    crate::util::fmt_g(v, 3)
}

/// The fixed (non-voltage-base) register names, padded to `NumEMRegisters`.
fn default_register_names() -> Vec<String> {
    let fixed = [
        "kWh",
        "kvarh",
        "Max kW",
        "Max kVA",
        "Zone kWh",
        "Zone kvarh",
        "Zone Max kW",
        "Zone Max kVA",
        "Overload kWh Normal",
        "Overload kWh Emerg",
        "Load EEN",
        "Load UE",
        "Zone Losses kWh",
        "Zone Losses kvarh",
        "Zone Max kW Losses",
        "Zone Max kvar Losses",
        "Load Losses kWh",
        "Load Losses kvarh",
        "No Load Losses kWh",
        "No Load Losses kvarh",
        "Max kW Load Losses",
        "Max kW No Load Losses",
        "Line Losses",
        "Transformer Losses",
        "Line Mode Line Losses",
        "Zero Mode Line Losses",
        "3-phase Line Losses",
        "1- and 2-phase Line Losses",
        "Gen kWh",
        "Gen kvarh",
        "Gen Max kW",
        "Gen Max kVA",
    ];
    let mut names: Vec<String> = fixed.iter().map(|s| s.to_string()).collect();
    names.resize(NUM_EM_REGISTERS, String::new());
    names
}

/// Capture the parse-relevant shape of the metered element (the RefSnapshot).
pub(crate) fn capture_metered(full_name: String, obj: &dyn DssObject) -> EmSnapshot {
    let elem = obj
        .as_ckt_element()
        .expect("element= resolves to a ckt elem");
    let cd = elem.cd();
    let is_pd = obj.as_any().downcast_ref::<Line>().is_some()
        || obj.as_any().downcast_ref::<Transformer>().is_some()
        || obj.as_any().downcast_ref::<Capacitor>().is_some()
        || obj.as_any().downcast_ref::<Reactor>().is_some();
    EmSnapshot {
        full_name,
        is_pd,
        nphases: cd.nphases,
        nconds: cd.nconds,
        nterms: cd.nterms,
        buses: (1..=cd.nterms).map(|i| cd.get_bus(i).to_string()).collect(),
    }
}

impl CktElement for EnergyMeter {
    fn cd(&self) -> &CktElementData {
        &self.med.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.med.cd
    }

    fn recalc_element_data(&mut self, _sys: &SysCtx) {
        let mut errors = Vec::new();
        self.recalc(&mut errors);
        for e in errors {
            self.med.cd.obj.push_error(e);
        }
    }

    /// `TEnergyMeterObj.CalcYPrim` is empty — a meter never stamps admittance.
    fn calc_yprim(&mut self, _sys: &SysCtx) {}

    /// `TEnergyMeterObj.GetCurrents` returns zeros.
    fn get_currents(&mut self, _sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
        curr.fill(Complex64::ZERO);
    }
}

impl DssObject for EnergyMeter {
    fn data(&self) -> &DssObjData {
        &self.med.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.med.cd.obj
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn as_ckt_element(&self) -> Option<&dyn CktElement> {
        Some(self)
    }
    fn as_ckt_element_mut(&mut self) -> Option<&mut dyn CktElement> {
        Some(self)
    }

    fn get_i32(&self, idx: usize) -> i32 {
        match idx {
            prop::TERMINAL => self.med.metered_terminal,
            _ => unreachable!("EnergyMeter has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        match idx {
            prop::TERMINAL => self.med.metered_terminal = value,
            _ => unreachable!("EnergyMeter has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use prop::*;
        match idx {
            LOCAL_ONLY => self.local_only,
            LOSSES => self.f_losses,
            LINE_LOSSES => self.f_line_losses,
            XFMR_LOSSES => self.f_xfmr_losses,
            SEQ_LOSSES => self.f_seq_losses,
            THREE_PHASE_LOSSES => self.f_3phase_losses,
            VBASE_LOSSES => self.f_vbase_losses,
            PHASE_VOLTAGE_REPORT => self.f_phase_voltage_report,
            ENABLED => self.med.cd.enabled,
            _ => unreachable!("EnergyMeter has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use prop::*;
        match idx {
            LOCAL_ONLY => self.local_only = value,
            LOSSES => self.f_losses = value,
            LINE_LOSSES => self.f_line_losses = value,
            XFMR_LOSSES => self.f_xfmr_losses = value,
            SEQ_LOSSES => self.f_seq_losses = value,
            THREE_PHASE_LOSSES => self.f_3phase_losses = value,
            VBASE_LOSSES => self.f_vbase_losses = value,
            PHASE_VOLTAGE_REPORT => self.f_phase_voltage_report = value,
            ENABLED => self.med.cd.set_enabled(value),
            _ => unreachable!("EnergyMeter has no boolean property {idx}"),
        }
    }

    fn get_f64(&self, idx: usize) -> f64 {
        use prop::*;
        match idx {
            KVA_NORMAL => self.max_zone_kva_norm,
            KVA_EMERG => self.max_zone_kva_emerg,
            INT_RATE => self.source_num_interruptions,
            INT_DURATION => self.source_int_duration,
            SAIFI => self.saifi,
            SAIFI_KW => self.saifi_kw,
            SAIDI => self.saidi,
            CAIDI => self.caidi,
            CUST_INTERRUPTS => self.cust_interrupts,
            BASE_FREQ => self.med.cd.base_frequency,
            _ => unreachable!("EnergyMeter has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            KVA_NORMAL => self.max_zone_kva_norm = value,
            KVA_EMERG => self.max_zone_kva_emerg = value,
            INT_RATE => self.source_num_interruptions = value,
            INT_DURATION => self.source_int_duration = value,
            // Read-only reliability props (Pascal SilentReadOnly): ignore writes.
            SAIFI | SAIFI_KW | SAIDI | CAIDI | CUST_INTERRUPTS => {}
            BASE_FREQ => self.med.cd.base_frequency = value,
            _ => unreachable!("EnergyMeter has no double property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        match idx {
            prop::PEAK_CURRENT => Some(&self.med.sensor_current),
            prop::MASK => Some(&self.totals_mask),
            _ => unreachable!("EnergyMeter has no double-array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        match idx {
            prop::PEAK_CURRENT => self.med.sensor_current = value,
            prop::MASK => {
                self.totals_mask = value;
                self.totals_mask.resize(NUM_EM_REGISTERS, 0.0);
            }
            _ => unreachable!("EnergyMeter has no double-array property {idx}"),
        }
    }
    fn array_size(&self, idx: usize) -> usize {
        match idx {
            prop::PEAK_CURRENT => self.med.cd.nphases,
            _ => unreachable!("EnergyMeter has no function-sized array {idx}"),
        }
    }

    fn get_string_list(&self, idx: usize) -> Vec<String> {
        match idx {
            // Pascal `GetOptions`: E/T, R/M, V/C.
            prop::OPTION => vec![
                if self.excess_flag { "E" } else { "T" }.to_string(),
                if self.zone_is_radial { "R" } else { "M" }.to_string(),
                if self.voltage_ue_only { "V" } else { "C" }.to_string(),
            ],
            prop::ZONE_LIST => self.defined_zone_list.clone(),
            _ => unreachable!("EnergyMeter has no string-list property {idx}"),
        }
    }
    fn set_string_list(&mut self, idx: usize, value: Vec<String>) {
        match idx {
            // Pascal `SetOptions`: branch on the first character of each token.
            prop::OPTION => {
                for s in value {
                    match s.chars().next().map(|c| c.to_ascii_lowercase()) {
                        Some('e') => self.excess_flag = true,
                        Some('t') => self.excess_flag = false,
                        Some('r') => self.zone_is_radial = true,
                        Some('m') => self.zone_is_radial = false,
                        Some('c') => self.voltage_ue_only = false,
                        Some('v') => self.voltage_ue_only = true,
                        _ => {}
                    }
                }
            }
            prop::ZONE_LIST => self.defined_zone_list = value,
            _ => unreachable!("EnergyMeter has no string-list property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        match idx {
            prop::ELEMENT => self.element_full_name.clone(),
            _ => unreachable!("EnergyMeter has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        match idx {
            prop::ELEMENT => self.element_full_name = value,
            _ => unreachable!("EnergyMeter has no string property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.med.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.med.cd.get_bus(terminal).to_string()
    }

    /// Resolve `element=` (any circuit class by full name) and snapshot it.
    fn set_object_ref(
        &mut self,
        idx: usize,
        name: String,
        resolved: Option<(ElemRef, &dyn DssObject)>,
    ) {
        match idx {
            prop::ELEMENT => {
                self.element_full_name = name.clone();
                match resolved {
                    Some((r, obj)) => {
                        self.med.metered_element = Some(r);
                        self.med.metered_element_changed = true;
                        self.metered_snap = Some(capture_metered(name, obj));
                    }
                    None => {
                        self.med.metered_element = None;
                        self.metered_snap = None;
                    }
                }
            }
            _ => unreachable!("EnergyMeter has no object-ref property {idx}"),
        }
    }

    fn side_effects(&mut self, idx: usize, prev_int: i32) {
        match idx {
            prop::ELEMENT | prop::TERMINAL => {
                self.med.metered_element_changed = true;
                self.needs_recalc = true;
            }
            prop::MASK => {
                // Pascal: the slots past the supplied values default to 1.0.
                let start = (prev_int.max(0) as usize).min(NUM_EM_REGISTERS);
                for v in &mut self.totals_mask[start..] {
                    *v = 1.0;
                }
            }
            _ => {}
        }
    }

    /// Pascal `DoAction`: Clear → `ResetRegisters`; the others
    /// (Allocate/Reduce/Save/TakeSample/ZoneDump) need the live circuit and are
    /// driven from the executive / later work packages, so they no-op here.
    fn do_action(&mut self, ordinal: i32, _errors: &mut Vec<String>) {
        if ordinal == 1 {
            self.reset_registers();
        }
    }

    fn end_edit(&mut self) {
        // Pascal `EndEdit`: only recalc when a basic datum (element/terminal)
        // changed, so editing e.g. `kVANormal` alone doesn't re-run validation.
        if !self.needs_recalc {
            return;
        }
        let mut errors = Vec::new();
        self.recalc(&mut errors);
        for e in errors {
            self.med.cd.obj.push_error(e);
        }
    }

    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(o) = other.as_any().downcast_ref::<EnergyMeter>() else {
            return;
        };
        self.med.cd.make_like_base(&o.med.cd);
        self.med.cd.nphases = o.med.cd.nphases;
        self.med.cd.set_nconds(o.med.cd.nconds);
        self.med.metered_element = o.med.metered_element;
        self.med.metered_terminal = o.med.metered_terminal;
        self.metered_snap = o.metered_snap.clone();
        self.element_full_name = o.element_full_name.clone();
        self.excess_flag = o.excess_flag;
        self.max_zone_kva_norm = o.max_zone_kva_norm;
        self.max_zone_kva_emerg = o.max_zone_kva_emerg;
        self.source_num_interruptions = o.source_num_interruptions;
        self.source_int_duration = o.source_int_duration;
        self.defined_zone_list = o.defined_zone_list.clone();
        self.local_only = o.local_only;
        self.voltage_ue_only = o.voltage_ue_only;
        self.f_losses = o.f_losses;
        self.f_line_losses = o.f_line_losses;
        self.f_xfmr_losses = o.f_xfmr_losses;
        self.f_seq_losses = o.f_seq_losses;
        self.f_3phase_losses = o.f_3phase_losses;
        self.f_vbase_losses = o.f_vbase_losses;
        self.f_phase_voltage_report = o.f_phase_voltage_report;
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

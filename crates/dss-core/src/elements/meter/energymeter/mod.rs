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
//!
//! Split into submodules mirroring `monitor/`, `reactor/`, `transformer/`:
//! - this `mod.rs` — register/property ordinals, `class_props`, the `EmSnapshot`
//!   and `EnergyMeter` structs, `new`, and the read-only/accessor surface.
//! - `zone.rs` — `RecalcElementData`, the zone-build write-back (`install_zone`),
//!   and `AssignVoltBaseRegisterNames`.
//! - `sample.rs` — the register machinery (`ResetRegisters`, the
//!   `begin/end_take_sample` borrow dance) plus `SampleState` and its
//!   `Integrate`/`SetDragHandRegister` helpers.
//! - `accessors.rs` — the `impl CktElement` / `impl DssObject` surfaces and the
//!   `capture_metered` RefSnapshot helper.

use crate::circuit::ckt_tree::CktTree;
use crate::elements::meter::meter_element::MeterElementData;
use crate::elements::traits::ElemId;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use crate::solution::meters::demand_interval::MeterStream;

mod accessors;
mod dump;
mod sample;
mod zone;

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
        PropDef::object_ref_any("Element"),
        PropDef::integer("Terminal"),
        PropDef::action("Action", enums.energy_meter_action),
        PropDef::string_list("Option"),
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
        // Pascal `PropertyNameJSON[__3PhaseLosses] := 'ThreePhaseLosses'`
        // (`EnergyMeter.pas:631`) — the JSON/schema key spells out the leading digit.
        PropDef::boolean("3PhaseLosses").json_name("ThreePhaseLosses"),
        PropDef::boolean("VBaseLosses"),
        PropDef::boolean("PhaseVoltageReport"),
        PropDef::double("Int_Rate"),
        PropDef::double("Int_Duration"),
        // Read-only reliability props (Pascal `SilentReadOnly` WITH a real
        // `PropertyOffset`, `EnergyMeter.pas:670-676`): the port's `READ_ONLY`
        // marks the schema `readOnly` + elides the default, while the `?`/props
        // surface still returns the stored value.
        PropDef::double("SAIFI").flags(PropFlags::READ_ONLY),
        PropDef::double("SAIFIkW").flags(PropFlags::READ_ONLY),
        PropDef::double("SAIDI").flags(PropFlags::READ_ONLY),
        PropDef::double("CAIDI").flags(PropFlags::READ_ONLY),
        PropDef::double("CustInterrupts").flags(PropFlags::READ_ONLY),
        // CktElementClass tail:
        PropDef::double("BaseFreq").flags(
            PropFlags::DYNAMIC_DEFAULT
                | PropFlags::NON_NEGATIVE
                | PropFlags::NON_ZERO
                | PropFlags::UNITS_HZ,
        ),
        PropDef::enabled("Enabled"),
    ];
    debug_assert_eq!(defs.len(), prop::NUM_PROPS - 1);
    ClassProps::new("EnergyMeter", defs, true)
}

/// Pascal `TFeederSection` record (EnergyMeter.pas l.162): one entry per feeder
/// section (the span between two over-current-protection devices). All-zero on
/// allocation (`ReallocMem` + the explicit init loop). Computed by
/// `CalcReliabilityIndices` (the `RelCalc` sweep) and **persisted on the meter**
/// (Pascal `FeederSections: pFeederSections`), read back by `Export Sections`.
#[derive(Debug, Clone, Default)]
pub struct FeederSection {
    /// 1=Fuse; 2=Recloser; 3=Relay.
    pub ocp_device_type: i32,
    pub n_customers: i32,
    pub n_branches: i32,
    pub total_customers: i32,
    /// 1-based `SequenceList` index of the PD element with the OCP device at
    /// the section head.
    pub seq_index: usize,
    pub average_repair_time: f64,
    pub sect_fault_rate: f64,
    pub sum_flt_rates_x_repair_hrs: f64,
    pub sum_branch_flt_rates: f64,
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

    /// Pascal `AssumeRestoration` (EnergyMeter.pas l.414): not a parsed property
    /// — set programmatically by `DoLambdaCalcs` (the `RelCalc` command) and read
    /// by both `CalcNum_Int` (forward sweep) and `TotalUpDownstreamCustomers`
    /// (customer roll-up at zone build). Defaults FALSE; not copied by `MakeLike`,
    /// matching Pascal.
    assume_restoration: bool,

    // Reliability outputs (read-only; WP6.6 fills them).
    saifi: f64,
    saifi_kw: f64,
    saidi: f64,
    caidi: f64,
    cust_interrupts: f64,

    /// Pascal `SectionCount` (EnergyMeter.pas l.420): the number of feeder
    /// sections the last `CalcReliabilityIndices` forward sweep counted. FPC
    /// zero-inits it; the sweep re-assigns it each `RelCalc` run — including
    /// the no-OCP abort, which leaves it 0 (so a subsequent `Export Sections`
    /// writes no rows) while `FeederSections` keeps the stale prior array,
    /// exactly like Pascal.
    section_count: i32,
    /// Pascal `FeederSections` (EnergyMeter.pas l.422): the per-section data,
    /// indices `0..=section_count` (slot 0 = the span before the first OCP
    /// device, never reported). Empty until a `RelCalc` completes.
    feeder_sections: Vec<FeederSection>,

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

    // --- Demand-interval machinery (WP8.3 step 4) --------------------------
    /// Pascal `This_Meter_DIFileIsOpen`.
    di_file_is_open: bool,
    /// Pascal `VPhaseReportFileIsOpen`.
    v_phase_report_file_is_open: bool,
    /// `DI_MHandle` — the per-meter demand-interval stream (DI-verbose mode).
    di_stream: Option<MeterStream>,
    /// `PHV_MHandle` — the phase-voltage report stream.
    phv_stream: Option<MeterStream>,
    /// `VphaseMax`/`VphaseMin`/`VphaseAccum`/`VphaseAccumCount`
    /// (EnergyMeter.pas l.349-352): per-(vbase, phase) pu-voltage extremes for
    /// the phase-voltage report, `jiIndex(j, i) = (i-1)*3 + j` → 0-based
    /// `(vbase_slot)*3 + phase-1`. Re-zeroed each `TakeSample` (per vbase in
    /// use); written by `WriteDemandIntervalData`.
    vphase_max: Vec<f64>,
    vphase_min: Vec<f64>,
    vphase_accum: Vec<f64>,
    vphase_accum_count: Vec<i32>,

    // Zone topology (built by `MakeMeterZoneLists`). `None` until the zone is
    // built (Pascal `BranchList = NIL`).
    branch_list: Option<CktTree>,
    /// `SequenceList`: branches meter→ends in radial order (= the BranchList
    /// `GoForward` order).
    sequence_list: Vec<ElemId>,
    /// Tree node index for each `sequence_list` entry (for shunt collection).
    sequence_nodes: Vec<usize>,
    /// `LoadList`: loads in the zone.
    load_list: Vec<ElemId>,
    /// `ZoneEndsList` resolved to `(branch element, end bus)` pairs.
    zone_ends: Vec<(ElemId, usize)>,
    /// `ZonePCE`: all PC elements in the zone (`GetPCEatZone`).
    zone_pce: Vec<ElemId>,
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
            assume_restoration: false,
            saifi: 0.0,
            saifi_kw: 0.0,
            saidi: 0.0,
            caidi: 0.0,
            cust_interrupts: 0.0,
            section_count: 0,
            feeder_sections: Vec::new(),
            di_file_is_open: false,
            v_phase_report_file_is_open: false,
            di_stream: None,
            phv_stream: None,
            vphase_max: vec![0.0; 3 * MAX_VBASE_COUNT],
            vphase_min: vec![0.0; 3 * MAX_VBASE_COUNT],
            vphase_accum: vec![0.0; 3 * MAX_VBASE_COUNT],
            vphase_accum_count: vec![0; 3 * MAX_VBASE_COUNT],
            // Pascal `TEnergyMeterObj.Create` (`EnergyMeter.pas:959`):
            // `MeteredElement := ActiveCircuit.CktElements.Get(1)` — defaults to
            // the first circuit element (the auto-created `Vsource.source`); every
            // real deck overrides it via the `element=` property before solve, so
            // it is observable only as the all-default sample's schema default.
            element_full_name: "Vsource.source".to_string(),
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
    pub fn metered_element(&self) -> Option<ElemId> {
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
    pub fn sequence_list(&self) -> &[ElemId] {
        &self.sequence_list
    }
    pub fn load_list(&self) -> &[ElemId] {
        &self.load_list
    }
    pub fn zone_pce(&self) -> &[ElemId] {
        &self.zone_pce
    }
    /// `Source_NumInterruptions` (annual interruptions of the upline circuit).
    pub fn source_num_interruptions(&self) -> f64 {
        self.source_num_interruptions
    }
    /// `Source_IntDuration` (average interruption duration of upline circuit).
    pub fn source_int_duration(&self) -> f64 {
        self.source_int_duration
    }
    /// Pascal `AssumeRestoration` (set by `DoLambdaCalcs`, read by the customer
    /// roll-up at zone build).
    pub fn assume_restoration(&self) -> bool {
        self.assume_restoration
    }
    /// Pascal `pMeter.AssumeRestoration := AssumeRestoration` in `DoLambdaCalcs`.
    pub(crate) fn set_assume_restoration(&mut self, value: bool) {
        self.assume_restoration = value;
    }
    /// Pascal `SectionCount`: sections counted by the last `RelCalc` sweep.
    pub fn section_count(&self) -> i32 {
        self.section_count
    }
    /// Pascal `FeederSections` (indices `0..=section_count`; empty until a
    /// `RelCalc` completes).
    pub fn feeder_sections(&self) -> &[FeederSection] {
        &self.feeder_sections
    }
    /// The forward interruption sweep's `SectionCount` write-back — assigned
    /// even on the no-OCP abort (Pascal mutates the field during the sweep,
    /// leaving 0 there), while `FeederSections` is only reallocated on success.
    pub(crate) fn set_section_count(&mut self, count: i32) {
        self.section_count = count;
    }
    /// Pascal `ReallocMem(FeederSections, …)` + the fill loops: replace the
    /// persisted per-section array (successful `CalcReliabilityIndices` only).
    pub(crate) fn set_feeder_sections(&mut self, sections: Vec<FeederSection>) {
        self.feeder_sections = sections;
    }
    /// Write back the reliability indices computed by `CalcReliabilityIndices`.
    pub(crate) fn set_reliability_results(
        &mut self,
        saifi: f64,
        saifi_kw: f64,
        saidi: f64,
        caidi: f64,
        cust_interrupts: f64,
    ) {
        self.saifi = saifi;
        self.saifi_kw = saifi_kw;
        self.saidi = saidi;
        self.caidi = caidi;
        self.cust_interrupts = cust_interrupts;
    }
    /// `ZoneEndsList` resolved to the branch elements (Pascal `AllEndElements`).
    pub fn zone_end_elements(&self) -> Vec<ElemId> {
        self.zone_ends.iter().map(|&(r, _)| r).collect()
    }
    pub fn has_branch_list(&self) -> bool {
        self.branch_list.is_some()
    }
    /// Move the branch tree out for a zone walk that needs the element store
    /// borrowed alongside it (the `solution::meters` free-function pattern —
    /// e.g. `InterpolateCoordinates`); pair with [`Self::put_branch_list`].
    /// `None` == Pascal `BranchList = NIL` (`CheckBranchList` fails).
    pub(crate) fn take_branch_list(&mut self) -> Option<CktTree> {
        self.branch_list.take()
    }
    /// Restore the branch tree taken by [`Self::take_branch_list`].
    pub(crate) fn put_branch_list(&mut self, tree: CktTree) {
        self.branch_list = Some(tree);
    }
    /// Pascal `BranchList` — the built zone tree (`None` == `BranchList = NIL`).
    /// Read-only for the zone-tree reports (`Show Loops`/`Show Zone`), which walk
    /// it via [`Self::sequence_list`] + [`Self::sequence_nodes`].
    pub fn branch_list(&self) -> Option<&CktTree> {
        self.branch_list.as_ref()
    }
    /// The [`CktTree`] node index for each [`Self::sequence_list`] entry — the
    /// `BranchList.PresentBranch` node reached at that point in the
    /// `First`/`GoForward` walk (used to read the branch's loop/parallel flags and
    /// shunt-object list without re-walking the tree cursor).
    pub fn sequence_nodes(&self) -> &[usize] {
        &self.sequence_nodes
    }

    /// Register values (1-based ordinals → 0-based slots). For the test API.
    pub fn registers(&self) -> &[f64] {
        &self.registers
    }

    /// The per-register derivatives (last integrated values) — the demand-
    /// interval row payload.
    pub(crate) fn derivatives(&self) -> &[f64] {
        &self.derivatives
    }
    /// `TotalsMask` — weights for the class `DI_RegisterTotals`/`Totals` sums.
    pub(crate) fn totals_mask(&self) -> &[f64] {
        &self.totals_mask
    }
    /// `FPhaseVoltageReport` (`PhaseVoltageReport=yes`).
    pub(crate) fn phase_voltage_report(&self) -> bool {
        self.f_phase_voltage_report
    }
    /// The voltage-base list + its in-use count (for the PHV header/rows).
    pub(crate) fn vbase_view(&self) -> (Vec<f64>, usize) {
        (self.vbase_list.clone(), self.vbase_count)
    }
    /// The phase-voltage accumulators (max, min, accum, count) in `jiIndex`
    /// layout.
    pub(crate) fn v_phase_view(&self) -> (&[f64], &[f64], &[f64], &[i32]) {
        (
            &self.vphase_max,
            &self.vphase_min,
            &self.vphase_accum,
            &self.vphase_accum_count,
        )
    }
    // --- Demand-interval stream plumbing (Pascal `DI_MHandle`/`PHV_MHandle` +
    // --- the open flags), driven by `solution::meters::demand_interval`.
    pub(crate) fn di_file_is_open(&self) -> bool {
        self.di_file_is_open
    }
    pub(crate) fn di_file_open(&mut self, open: bool) {
        self.di_file_is_open = open;
    }
    pub(crate) fn v_phase_report_open(&self) -> bool {
        self.v_phase_report_file_is_open
    }
    pub(crate) fn set_v_phase_report_open(&mut self, open: bool) {
        self.v_phase_report_file_is_open = open;
    }
    pub(crate) fn set_di_stream(&mut self, s: Option<MeterStream>) {
        self.di_stream = s;
    }
    pub(crate) fn take_di_stream(&mut self) -> Option<MeterStream> {
        self.di_stream.take()
    }
    pub(crate) fn di_stream_mut(&mut self) -> Option<&mut MeterStream> {
        self.di_stream.as_mut()
    }
    pub(crate) fn set_phv_stream(&mut self, s: Option<MeterStream>) {
        self.phv_stream = s;
    }
    pub(crate) fn take_phv_stream(&mut self) -> Option<MeterStream> {
        self.phv_stream.take()
    }
    pub(crate) fn phv_stream_mut(&mut self) -> Option<&mut MeterStream> {
        self.phv_stream.as_mut()
    }

    pub fn has_been_sampled(&self) -> bool {
        !self.first_sample_after_reset
    }

    pub(crate) fn defined_zone_list(&self) -> &[String] {
        &self.defined_zone_list
    }
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

//! Mapped string enumerations, port of Pascal `TDSSEnum` (DSSClass.pas).
//!
//! These drive `MappedStringEnum*` properties: user strings are matched
//! against the enum names with a min/max-character disambiguation window
//! (`StringToOrdinal`), and ordinals are rendered back for dumps
//! (`OrdinalToString`). "Hybrid" enums (e.g. monitored phase) fall back to
//! parsing the string as an integer.
//!
//! Split into submodules (no behavioral change): the [`DssEnum`] type and its
//! matching logic live in [`enum_def`]; the [`EnumRegistry`] table of instances
//! (`EnumRegistry::new`/`get`) in [`registry`]; this module keeps the registry
//! struct and the `EnumId` alias.

#[cfg(test)]
mod tests;

mod enum_def;
mod registry;

pub use enum_def::DssEnum;

/// The enum instances the engine has needed so far, mirroring the ones built
/// in `TDSSContext.Create` (DSSClass.pas). More are added as classes that
/// reference them get ported.
#[derive(Debug)]
pub struct EnumRegistry {
    enums: Vec<DssEnum>,
    /// 'Length Unit' (`DSS.UnitsEnum`).
    pub units: EnumId,
    /// 'Earth Model' (`DSS.EarthModelEnum`).
    pub earth_model: EnumId,
    /// 'Scan Type' (`DSS.ScanTypeEnum`).
    pub scan_type: EnumId,
    /// 'Sequence Type' (`DSS.SequenceEnum`).
    pub sequence: EnumId,
    /// 'Connection' (`DSS.ConnectionEnum`).
    pub connection: EnumId,
    /// 'VSource: Model' (Vsource.pas `ModelEnum`).
    pub vsource_model: EnumId,
    /// 'Load: Model' (Load.pas `LoadModelEnum`).
    pub load_model: EnumId,
    /// 'Load: Status' (Load.pas `LoadStatusEnum`).
    pub load_status: EnumId,
    /// 'Line Type' (`DSS.LineTypeEnum`).
    pub line_type: EnumId,
    /// 'Solution Mode' (`DSS.SolveModeEnum`).
    pub solve_mode: EnumId,
    /// 'Solution Algorithm' (`DSS.SolveAlgEnum`).
    pub solve_alg: EnumId,
    /// 'Control Mode' (`DSS.ControlModeEnum`).
    pub control_mode: EnumId,
    /// 'Random Type' (`DSS.RandomModeEnum`).
    pub random_mode: EnumId,
    /// 'Load Solution Model' (`DSS.DefaultLoadModelEnum`).
    pub default_load_model: EnumId,
    /// 'Circuit Model' (`DSS.CktModelEnum`).
    pub ckt_model: EnumId,
    /// 'Core Type' (`DSS.CoreTypeEnum`).
    pub core_type: EnumId,
    /// 'Phase Sequence' reused for transformer LeadLag (`DSS.LeadLagEnum`).
    pub lead_lag: EnumId,
    /// 'RegControl: Phase Selection' (RegControl.pas `PhaseEnum`).
    pub reg_control_phase: EnumId,
    /// 'Monitored Phase' (`DSS.MonPhaseEnum`, CapControl PT/CT phase).
    pub mon_phase: EnumId,
    /// 'CapControl: Type' (CapControl.pas `TypeEnum`).
    pub cap_control_type: EnumId,
    /// 'LoadShape: Action' (LoadShape.pas `ActionEnum`).
    pub load_shape_action: EnumId,
    /// 'LoadShape: Interpolation' (LoadShape.pas `InterpEnum`).
    pub load_shape_interp: EnumId,
    /// 'TShape: Action' (TempShape.pas `ActionEnum`).
    pub t_shape_action: EnumId,
    /// 'PriceShape: Action' (PriceShape.pas `ActionEnum`).
    pub price_shape_action: EnumId,
    /// 'Generator: Dispatch Mode' (generator.pas `GenDispModeEnum`).
    pub gen_disp_mode: EnumId,
    /// 'Generator: Status' (generator.pas `GenStatusEnum`).
    pub gen_status: EnumId,
    /// 'Generator: Model' (generator.pas `GenModelEnum`).
    pub gen_model: EnumId,
    /// 'PVSystem: Model' (PVsystem.pas `PVSystemModelEnum`).
    pub pvsystem_model: EnumId,
    /// 'Inverter Control Mode' (DSSClass.pas `InvControlModeEnum`; GFL/GFM).
    pub inv_control_mode: EnumId,
    /// 'Storage: State' (Storage.pas `StateEnum`; Charging/Idling/Discharging).
    pub storage_state: EnumId,
    /// 'Storage: Dispatch Mode' (Storage.pas `DispatchModeEnum`).
    pub storage_dispatch_mode: EnumId,
    /// 'IndMach012: Slip Option' (IndMach012.pas `SlipOptionEnum`).
    pub ind_mach_slip_option: EnumId,
    /// 'VSConverter: Control Mode' (VSConverter.pas `ModeEnum`).
    pub vsc_mode: EnumId,
    /// 'UPFC: Mode' (UPFC.pas `UPFCModeEnum`; Off..DoubleReference_Dual, 0..5).
    pub upfc_mode: EnumId,
    /// 'Monitor: Action' (Monitor.pas `ActionEnum`).
    pub monitor_action: EnumId,
    pub energy_meter_action: EnumId,
    /// 'StorageController: Discharge Mode' (StorageController.pas `DischargeModeEnum`).
    pub storage_ctrl_discharge_mode: EnumId,
    /// 'StorageController: Charge Mode' (StorageController.pas `ChargeModeEnum`).
    pub storage_ctrl_charge_mode: EnumId,
    /// 'SwtControl: Action' (SwtControl.pas `ActionEnum`).
    pub swt_control_action: EnumId,
    /// 'SwtControl: State' (SwtControl.pas `StateEnum`).
    pub swt_control_state: EnumId,
    /// 'Fuse: Action' (fuse.pas `ActionEnum`).
    pub fuse_action: EnumId,
    /// 'Fuse: State' (fuse.pas `StateEnum`).
    pub fuse_state: EnumId,
    /// 'Recloser: Action' (Recloser.pas `ActionEnum`).
    pub recloser_action: EnumId,
    /// 'Recloser: State' (Recloser.pas `StateEnum`).
    pub recloser_state: EnumId,
    /// 'Relay: Type' (Relay.pas `RelayTypeEnum`).
    pub relay_type: EnumId,
    /// 'Relay: Action' (Relay.pas `ActionEnum`).
    pub relay_action: EnumId,
    /// 'Relay: State' (Relay.pas `StateEnum`).
    pub relay_state: EnumId,
    /// 'InvControl: Control Mode' (InvControl.pas `ModeEnum`; Voltvar..GFM).
    pub invcontrol_mode: EnumId,
    /// 'InvControl: Combi Mode' (InvControl.pas `CombiModeEnum`; VV_VW/VV_DRC).
    pub invcontrol_combi: EnumId,
    /// 'InvControl: Voltage Curve X Ref' (InvControl.pas `VoltageCurveXRefEnum`).
    pub invcontrol_voltage_curvex: EnumId,
    /// 'InvControl: Volt-Watt Y-Axis' (InvControl.pas `VoltWattYAxisEnum`).
    pub invcontrol_voltwatt_yaxis: EnumId,
    /// 'InvControl: Rate-of-change Mode' (InvControl.pas `RoCEnum`).
    pub invcontrol_roc: EnumId,
    /// 'InvControl: Reactive Power Reference' (InvControl.pas `RefQEnum`).
    pub invcontrol_reac_power: EnumId,
    /// 'InvControl: Control Model' (InvControl.pas `ControlModelEnum`).
    pub invcontrol_model: EnumId,
    /// 'AutoAdd Device Type' (`DSS.AddTypeEnum`, GENADD/CAPADD).
    pub add_type: EnumId,
    /// 'DynamicExp: Domain' (DynamicExp.pas `DomainEnum`).
    pub dynamic_exp_domain: EnumId,
}

/// Index of an enum inside the registry — what Pascal stored as a raw
/// `TDSSEnum` pointer in `PropertyOffset2`.
pub type EnumId = usize;

impl Default for EnumRegistry {
    fn default() -> Self {
        Self::new()
    }
}

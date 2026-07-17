//! Control-element enum registrations — RegControl phase selection, the
//! CapControl monitored-phase and type, and the StorageController discharge /
//! charge modes. Split out of `registry/mod.rs` (no behavioral change).

use super::super::{DssEnum, EnumId};
use crate::elements::control::control_elem::CTRL_STATE_KEEP;

pub(super) struct ControlEnums {
    pub(super) reg_control_phase: EnumId,
    pub(super) mon_phase: EnumId,
    pub(super) cap_control_type: EnumId,
    pub(super) storage_ctrl_discharge_mode: EnumId,
    pub(super) storage_ctrl_charge_mode: EnumId,
    pub(super) swt_control_action: EnumId,
    pub(super) swt_control_state: EnumId,
    pub(super) fuse_action: EnumId,
    pub(super) fuse_state: EnumId,
    pub(super) recloser_action: EnumId,
    pub(super) recloser_state: EnumId,
    pub(super) relay_type: EnumId,
    pub(super) relay_action: EnumId,
    pub(super) relay_state: EnumId,
    pub(super) invcontrol_mode: EnumId,
    pub(super) invcontrol_combi: EnumId,
    pub(super) invcontrol_voltage_curvex: EnumId,
    pub(super) invcontrol_voltwatt_yaxis: EnumId,
    pub(super) invcontrol_roc: EnumId,
    pub(super) invcontrol_reac_power: EnumId,
    pub(super) invcontrol_model: EnumId,
    pub(super) espvl_control_type: EnumId,
}

pub(super) fn register(push: &mut dyn FnMut(DssEnum) -> EnumId) -> ControlEnums {
    // RegControl.pas: PhaseEnum (hybrid — falls back to a phase number).
    let mut rcp = DssEnum::new(
        "RegControl: Phase Selection",
        true,
        2,
        2,
        &["min", "max"],
        &[-3, -2],
    );
    rcp.hybrid = true;
    let reg_control_phase = push(rcp);

    // DSSClass.pas:1184 MonPhaseEnum (hybrid — falls back to a phase no.).
    let mut monph = DssEnum::new(
        "Monitored Phase",
        true,
        1,
        2,
        &["min", "max", "avg"],
        &[-3, -2, -1],
    );
    monph.hybrid = true;
    let mon_phase = push(monph);

    // CapControl.pas:244 TypeEnum ('UserControl' is commented out upstream).
    let cap_control_type = push(DssEnum::new(
        "CapControl: Type",
        true,
        1,
        1,
        &[
            "Current",
            "Voltage",
            "kvar",
            "Time",
            "PowerFactor",
            "Follow",
        ],
        &[0, 1, 2, 3, 4, 5],
    ));
    // StorageController.pas TStorageController.Create: DischargeModeEnum /
    // ChargeModeEnum (non-sequential ordinals; the Pascal `False` flag).
    // MODEFOLLOW=1 MODELOADSHAPE=2 MODESUPPORT=3 MODETIME=4 MODEPEAKSHAVE=5
    // MODESCHEDULE=6 MODEPEAKSHAVELOW=7 CURRENTPEAKSHAVE=8 CURRENTPEAKSHAVELOW=9.
    let storage_ctrl_discharge_mode = push(DssEnum::new(
        "StorageController: Discharge Mode",
        false,
        1,
        2,
        &[
            "Peakshave",
            "Follow",
            "Support",
            "Loadshape",
            "Time",
            "Schedule",
            "I-Peakshave",
        ],
        &[5, 1, 3, 2, 4, 6, 8],
    ));
    let storage_ctrl_charge_mode = push(DssEnum::new(
        "StorageController: Charge Mode",
        false,
        1,
        1,
        &["Loadshape", "Time", "PeakshaveLow", "I-PeakshaveLow"],
        &[2, 4, 7, 9],
    ));
    // SwtControl.pas TSwtControl.Create: ActionEnum / StateEnum. Both map onto
    // the control's `CurrentAction` field; the ordinals are EControlAction
    // (CTRL_CLOSE=2, CTRL_OPEN=1). Action renders close/open, State renders
    // closed/open; both default to CTRL_CLOSE.
    let swt_control_action = push(DssEnum::new(
        "SwtControl: Action",
        false,
        1,
        1,
        &["close", "open"],
        &[2, 1],
    ));
    let swt_control_state = push(DssEnum::new(
        "SwtControl: State",
        false,
        1,
        1,
        &["closed", "open"],
        &[2, 1],
    ));
    // fuse.pas TFuse.Create: ActionEnum / StateEnum. EControlAction ordinals
    // (CTRL_CLOSE=2, CTRL_OPEN=1); Action renders close/open, State/Normal render
    // closed/open.
    let fuse_action = push(DssEnum::new(
        "Fuse: Action",
        false,
        1,
        1,
        &["close", "open"],
        &[2, 1],
    ));
    let fuse_state = push(DssEnum::new(
        "Fuse: State",
        false,
        1,
        1,
        &["closed", "open"],
        &[2, 1],
    ));
    // Recloser.pas TRecloser.Create: ActionEnum / StateEnum. EControlAction
    // ordinals (CTRL_CLOSE=2, CTRL_OPEN=1); both carry a third `trip` spelling
    // that maps to CTRL_OPEN (so Action/State/Normal render close/open — the
    // reverse-lookup of ordinal 1 picks the first name, `open`, not `trip`).
    let recloser_action = push(DssEnum::new(
        "Recloser: Action",
        false,
        1,
        1,
        &["close", "open", "trip"],
        &[2, 1, 1],
    ));
    let recloser_state = push(DssEnum::new(
        "Recloser: State",
        false,
        1,
        1,
        &["closed", "open", "trip"],
        &[2, 1, 1],
    ));
    // Relay.pas TRelay.Create: RelayTypeEnum / ActionEnum / StateEnum. The type
    // enum is non-sequential (its `46`/`47` ordinals skip `2`); `AltNamesValid`
    // is false upstream, so only these primary spellings parse. CURRENT=0
    // VOLTAGE=1 REVPOWER=3 NEGCURRENT=4 NEGVOLTAGE=5 GENERIC=6 DISTANCE=7 TD21=8
    // DOC=9 (the `2` ordinal is deliberately unused upstream).
    let relay_type = push(DssEnum::new(
        "Relay: Type",
        false,
        1,
        2,
        &[
            "Current",
            "Voltage",
            "ReversePower",
            "46",
            "47",
            "Generic",
            "Distance",
            "TD21",
            "DOC",
        ],
        &[0, 1, 3, 4, 5, 6, 7, 8, 9],
    ));
    // Relay.pas r4133 `InterpretRelayState`: parsing is FIRST-CHARACTER ONLY —
    // `case LowerCase(param)[1] of 'o': CTRL_OPEN; 'c': CTRL_CLOSE; end` with no
    // else arm, so any other spelling (`trip`, `xyz`, ...) leaves the state
    // slot UNCHANGED (empirically on oddie:r4133: `normal=trip` → Normal stays
    // `[closed,closed,closed]`). Reproduced with `allow_longer` + `max_chars=1`
    // (match on the leading char alone: `openx`→open, `cs`→closed) and
    // `default_value = CTRL_STATE_KEEP` (unmatched → keep, no parse error). This
    // deliberately does NOT carry the old `trip`→open alias, which diverged from
    // r4133. (The Recloser enums, WP-U2.2's lane, keep their own separate defs.)
    let mut relay_action = DssEnum::new("Relay: Action", false, 1, 1, &["close", "open"], &[2, 1]);
    relay_action.allow_longer = true;
    relay_action.default_value = CTRL_STATE_KEEP;
    let relay_action = push(relay_action);
    let mut relay_state = DssEnum::new("Relay: State", false, 1, 1, &["closed", "open"], &[2, 1]);
    relay_state.allow_longer = true;
    relay_state.default_value = CTRL_STATE_KEEP;
    let relay_state = push(relay_state);
    // InvControl.pas TInvControl.Create: the seven smart-inverter enums. The
    // control-mode ordinals are VOLTVAR=1 VOLTWATT=2 DRC=3 WATTPF=4 WATTVAR=5
    // AVR=6 GFM=7 (NONE_MODE=0 dumps ''); CombiMode VV_VW=1 VV_DRC=2. All seven
    // are SequentialOrdinals=True upstream.
    let invcontrol_mode = push(DssEnum::new(
        "InvControl: Control Mode",
        true,
        1,
        5,
        &[
            "Voltvar",
            "VoltWatt",
            "DynamicReaccurr",
            "WattPF",
            "Wattvar",
            "AVR",
            "GFM",
        ],
        &[1, 2, 3, 4, 5, 6, 7],
    ));
    let invcontrol_combi = push(DssEnum::new(
        "InvControl: Combi Mode",
        true,
        4,
        4,
        &["VV_VW", "VV_DRC"],
        &[1, 2],
    ));
    let invcontrol_voltage_curvex = push(DssEnum::new(
        "InvControl: Voltage Curve X Ref",
        true,
        1,
        2,
        &["Rated", "Avg", "RAvg"],
        &[0, 1, 2],
    ));
    let invcontrol_voltwatt_yaxis = push(DssEnum::new(
        "InvControl: Volt-Watt Y-Axis",
        true,
        1,
        2,
        &["PAvailablePU", "PMPPPU", "PctPMPPPU", "KVARatingPU"],
        &[0, 1, 2, 3],
    ));
    let invcontrol_roc = push(DssEnum::new(
        "InvControl: Rate-of-change Mode",
        true,
        3,
        3,
        &["Inactive", "LPF", "RiseFall"],
        &[0, 1, 2],
    ));
    // RefQEnum: AllowLonger=True upstream (e.g. "VARAVAL"/"VARMAX" accept a
    // longer typed value).
    let mut refq = DssEnum::new(
        "InvControl: Reactive Power Reference",
        true,
        4,
        4,
        &["VARAVAL", "VARMAX"],
        &[0, 1],
    );
    refq.allow_longer = true;
    let invcontrol_reac_power = push(refq);
    // ControlModelEnum: MappedIntEnum (JSONUseNumbers) — Linear=0 Exponential=1.
    let invcontrol_model = push(DssEnum::new(
        "InvControl: Control Model",
        true,
        1,
        1,
        &["Linear", "Exponential"],
        &[0, 1],
    ));
    // ESPVLControl.pas TESPVLControl.Create: TypeEnum (SequentialOrdinals=True,
    // min=max=1). SystemController=1 LocalController=2; the default Ftype=0 is
    // outside the enum, so it dumps '' (probed).
    let espvl_control_type = push(DssEnum::new(
        "ESPVLControl: Type",
        true,
        1,
        1,
        &["SystemController", "LocalController"],
        &[1, 2],
    ));
    ControlEnums {
        reg_control_phase,
        mon_phase,
        cap_control_type,
        storage_ctrl_discharge_mode,
        storage_ctrl_charge_mode,
        swt_control_action,
        swt_control_state,
        fuse_action,
        fuse_state,
        recloser_action,
        recloser_state,
        relay_type,
        relay_action,
        relay_state,
        invcontrol_mode,
        invcontrol_combi,
        invcontrol_voltage_curvex,
        invcontrol_voltwatt_yaxis,
        invcontrol_roc,
        invcontrol_reac_power,
        invcontrol_model,
        espvl_control_type,
    }
}

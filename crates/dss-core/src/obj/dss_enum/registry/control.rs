//! Control-element enum registrations — RegControl phase selection, the
//! CapControl monitored-phase and type, and the StorageController discharge /
//! charge modes. Split out of `registry/mod.rs` (no behavioral change).

use super::super::{DssEnum, EnumId};

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
    }
}

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
    ControlEnums {
        reg_control_phase,
        mon_phase,
        cap_control_type,
        storage_ctrl_discharge_mode,
        storage_ctrl_charge_mode,
    }
}

//! Power-conversion enum registrations — the wye/delta connection, the VSource
//! model, the Load model/status, and the Generator dispatch-mode/status/model.
//! Split out of `registry/mod.rs` (no behavioral change).

use super::super::{DssEnum, EnumId};

pub(super) struct PcEnums {
    pub(super) connection: EnumId,
    pub(super) vsource_model: EnumId,
    pub(super) load_model: EnumId,
    pub(super) load_status: EnumId,
    pub(super) gen_disp_mode: EnumId,
    pub(super) gen_status: EnumId,
    pub(super) gen_model: EnumId,
    pub(super) pvsystem_model: EnumId,
    pub(super) inv_control_mode: EnumId,
}

pub(super) fn register(push: &mut dyn FnMut(DssEnum) -> EnumId) -> PcEnums {
    let connection = push(DssEnum::new(
        "Connection",
        true,
        1,
        2,
        &["wye", "delta", "y", "ln", "ll"],
        &[0, 1, 0, 0, 1],
    ));

    let vsource_model = push(DssEnum::new(
        "VSource: Model",
        true,
        1,
        1,
        &["Thevenin", "Ideal"],
        &[0, 1],
    ));

    let load_model = push(DssEnum::new(
        "Load: Model",
        true,
        0,
        0,
        &[
            "Constant PQ",
            "Constant Z",
            "Motor (constant P, quadratic Q)",
            "CVR (linear P, quadratic Q)",
            "Constant I",
            "Constant P, fixed Q",
            "Constant P, fixed X",
            "ZIPV",
        ],
        &[1, 2, 3, 4, 5, 6, 7, 8],
    ));

    let mut status = DssEnum::new(
        "Load: Status",
        true,
        1,
        1,
        &["Variable", "Fixed", "Exempt"],
        &[0, 1, 2],
    );
    status.default_value = 0;
    let load_status = push(status);
    // generator.pas TGenerator.Create: GenDispModeEnum / GenStatusEnum /
    // GenModelEnum.
    let mut gdm = DssEnum::new(
        "Generator: Dispatch Mode",
        true,
        1,
        1,
        &["Default", "LoadLevel", "Price"],
        &[0, 1, 2],
    );
    gdm.default_value = 0;
    let gen_disp_mode = push(gdm);

    let mut gst = DssEnum::new(
        "Generator: Status",
        true,
        1,
        1,
        &["Variable", "Fixed"],
        &[0, 1],
    );
    gst.default_value = 0;
    let gen_status = push(gst);

    let gen_model = push(DssEnum::new(
        "Generator: Model",
        true,
        0,
        0,
        &[
            "Constant PQ",
            "Constant Z",
            "Constant P|V|",
            "Constant P, fixed Q",
            "Constant P, fixed X",
            "User model",
            "Approximate inverter model",
        ],
        &[1, 2, 3, 4, 5, 6, 7],
    ));

    // PVsystem.pas TPVsystem.Create: `PVSystemModelEnum` (JSONUseNumbers).
    let pvsystem_model = push(DssEnum::new(
        "PVSystem: Model",
        true,
        0,
        0,
        &["Constant P, PF", "Constant Y", "User model"],
        &[1, 2, 3],
    ));

    // DSSClass.pas TDSSClassesHelper: `InvControlModeEnum` (DefaultValue 0 = GFL).
    let mut icm = DssEnum::new(
        "Inverter Control Mode",
        true,
        1,
        1,
        &["GFL", "GFM"],
        &[0, 1],
    );
    icm.default_value = 0;
    let inv_control_mode = push(icm);

    PcEnums {
        connection,
        vsource_model,
        load_model,
        load_status,
        gen_disp_mode,
        gen_status,
        gen_model,
        pvsystem_model,
        inv_control_mode,
    }
}

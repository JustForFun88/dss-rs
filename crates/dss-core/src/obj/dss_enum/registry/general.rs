//! Catalog / metering enum registrations — the length unit, the LoadShape /
//! TShape / PriceShape actions and interpolation, and the Monitor / EnergyMeter
//! actions. Split out of `registry/mod.rs` (no behavioral change).

use super::super::{DssEnum, EnumId};

pub(super) struct GeneralEnums {
    pub(super) units: EnumId,
    pub(super) load_shape_action: EnumId,
    pub(super) load_shape_interp: EnumId,
    pub(super) t_shape_action: EnumId,
    pub(super) price_shape_action: EnumId,
    pub(super) monitor_action: EnumId,
    pub(super) energy_meter_action: EnumId,
}

pub(super) fn register(push: &mut dyn FnMut(DssEnum) -> EnumId) -> GeneralEnums {
    let mut units = DssEnum::new(
        "Length Unit",
        true,
        1,
        2,
        &[
            "none", "mi", "kft", "km", "m", "ft", "in", "cm", "mm", "meter", "miles",
        ],
        &[0, 1, 2, 3, 4, 5, 6, 7, 8, 4, 1],
    );
    units.default_value = 0;
    let units_id = push(units);
    // LoadShape.pas: ActionEnum (sequential, 1 char). No default — an
    // unknown action raises, like the original.
    let load_shape_action = push(DssEnum::new(
        "LoadShape: Action",
        true,
        1,
        1,
        &["Normalize", "DblSave", "SngSave"],
        &[0, 1, 2],
    ));

    // LoadShape.pas: InterpEnum (Avg/Edge). No default — unknown raises.
    let load_shape_interp = push(DssEnum::new(
        "LoadShape: Interpolation",
        true,
        1,
        1,
        &["Avg", "Edge"],
        &[0, 1],
    ));
    // TempShape.pas / PriceShape.pas: ActionEnum (DblSave/SngSave). Both
    // actions write binary files and are NOT_PORTED; the enum still parses
    // the names so a bad action raises like the original. No Normalize.
    let t_shape_action = push(DssEnum::new(
        "TShape: Action",
        true,
        1,
        1,
        &["DblSave", "SngSave"],
        &[0, 1],
    ));
    let price_shape_action = push(DssEnum::new(
        "PriceShape: Action",
        true,
        1,
        1,
        &["DblSave", "SngSave"],
        &[0, 1],
    ));
    // Monitor.pas: ActionEnum (Clear/Save/TakeSample/Process/Reset →
    // 0/1/2/3/0; Reset is an alias of Clear).
    let monitor_action = push(DssEnum::new(
        "Monitor: Action",
        true,
        1,
        1,
        &["Clear", "Save", "TakeSample", "Process", "Reset"],
        &[0, 1, 2, 3, 0],
    ));

    // EnergyMeter.pas: ActionEnum (Allocate/Clear/Reduce/Save/TakeSample/
    // ZoneDump → 0..5).
    let energy_meter_action = push(DssEnum::new(
        "EnergyMeter: Action",
        true,
        1,
        2,
        &[
            "Allocate",
            "Clear",
            "Reduce",
            "Save",
            "TakeSample",
            "ZoneDump",
        ],
        &[0, 1, 2, 3, 4, 5],
    ));
    GeneralEnums {
        units: units_id,
        load_shape_action,
        load_shape_interp,
        t_shape_action,
        price_shape_action,
        monitor_action,
        energy_meter_action,
    }
}

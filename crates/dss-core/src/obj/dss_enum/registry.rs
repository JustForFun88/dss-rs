//! `EnumRegistry::new` — the table of enum instances the engine builds in
//! `TDSSContext.Create` (DSSClass.pas). One entry per ported class that
//! references a mapped-string enum; ids are local indices into `enums`.

use super::{DssEnum, EnumId, EnumRegistry};

impl EnumRegistry {
    pub fn new() -> Self {
        let mut enums = Vec::new();
        let mut push = |e: DssEnum| -> EnumId {
            enums.push(e);
            enums.len() - 1
        };

        // Pascal TDSSContext.Create order is irrelevant here; ids are local.
        let mut earth = DssEnum::new(
            "Earth Model",
            true,
            1,
            1,
            &["Carson", "FullCarson", "Deri"],
            &[1, 2, 3],
        );
        earth.default_value = 1;
        let earth_model = push(earth);

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

        let scan_type = push(DssEnum::new(
            "Scan Type",
            true,
            1,
            1,
            &["None", "Zero", "Positive"],
            &[-1, 0, 1],
        ));

        let sequence = push(DssEnum::new(
            "Sequence Type",
            true,
            1,
            1,
            &["Negative", "Zero", "Positive"],
            &[-1, 0, 1],
        ));

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

        let mut ltype = DssEnum::new(
            "Line Type",
            true,
            2,
            4,
            &[
                "oh",
                "ug",
                "ug_ts",
                "ug_cn",
                "swt_ldbrk",
                "swt_fuse",
                "swt_sect",
                "swt_rec",
                "swt_disc",
                "swt_brk",
                "swt_elbow",
                "busbar",
            ],
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
        );
        ltype.default_value = 1;
        let line_type = push(ltype);

        let mut smode = DssEnum::new(
            "Solution Mode",
            true,
            2,
            9,
            &[
                "Snap",
                "Daily",
                "Yearly",
                "M1",
                "LD1",
                "PeakDay",
                "DutyCycle",
                "Direct",
                "MF",
                "FaultStudy",
                "M2",
                "M3",
                "LD2",
                "AutoAdd",
                "Dynamic",
                "Harmonic",
                "Time",
                "HarmonicT",
                "Snapshot",
                "Dynamics",
                "Harmonics",
                "S",
                "Y",
                "H",
                "T",
                "F",
            ],
            &[
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 0, 14, 15, 0, 2, 15,
                16, 9,
            ],
        );
        smode.default_value = 0;
        smode.use_first_found = true; // "Harm" is ambiguous in test files
        smode.try_exact_first = true;
        let solve_mode = push(smode);

        let mut alg = DssEnum::new(
            "Solution Algorithm",
            true,
            2,
            2,
            &["Normal", "Newton"],
            &[0, 1],
        );
        alg.default_value = 0;
        let solve_alg = push(alg);

        let mut cmode = DssEnum::new(
            "Control Mode",
            true,
            1,
            1,
            &["Off", "Static", "Event", "Time", "MultiRate"],
            &[-1, 0, 1, 2, 3],
        );
        cmode.default_value = 0;
        let control_mode = push(cmode);

        let mut rmode = DssEnum::new(
            "Random Type",
            true,
            1,
            1,
            &["None", "Gaussian", "Uniform", "LogNormal"],
            &[0, 1, 2, 3],
        );
        rmode.default_value = 0;
        let random_mode = push(rmode);

        let mut dlm = DssEnum::new(
            "Load Solution Model",
            true,
            1,
            1,
            &["PowerFlow", "Admittance"],
            &[1, 2],
        );
        dlm.default_value = 2;
        let default_load_model = push(dlm);

        let mut cm = DssEnum::new(
            "Circuit Model",
            true,
            1,
            1,
            &["Multiphase", "Positive"],
            &[0, 1],
        );
        cm.default_value = 0;
        let ckt_model = push(cm);

        let mut core = DssEnum::new(
            "Core Type",
            false,
            1,
            1,
            &[
                "shell",
                "1-phase",
                "3-leg",
                "4-leg",
                "5-leg",
                "core-1-phase",
            ],
            &[0, 1, 3, 4, 5, 9],
        );
        core.default_value = 0;
        let core_type = push(core);

        // Pascal 'Phase Sequence' enum reused for transformer LeadLag.
        let lead_lag = push(DssEnum::new(
            "Phase Sequence",
            true,
            1,
            1,
            &["Lag", "Lead", "ANSI", "Euro"],
            &[0, 1, 0, 1],
        ));

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

        // DSSClass.pas:1172 AddTypeEnum (GENADD=1, CAPADD=2; default CAPADD).
        let mut at = DssEnum::new(
            "AutoAdd Device Type",
            true,
            1,
            1,
            &["Generator", "Capacitor"],
            &[1, 2],
        );
        at.default_value = 2;
        let add_type = push(at);

        Self {
            enums,
            units: units_id,
            earth_model,
            scan_type,
            sequence,
            connection,
            vsource_model,
            load_model,
            load_status,
            line_type,
            solve_mode,
            solve_alg,
            control_mode,
            random_mode,
            default_load_model,
            ckt_model,
            core_type,
            lead_lag,
            reg_control_phase,
            mon_phase,
            cap_control_type,
            load_shape_action,
            load_shape_interp,
            t_shape_action,
            price_shape_action,
            gen_disp_mode,
            gen_status,
            gen_model,
            monitor_action,
            energy_meter_action,
            storage_ctrl_discharge_mode,
            storage_ctrl_charge_mode,
            add_type,
        }
    }

    pub fn get(&self, id: EnumId) -> &DssEnum {
        &self.enums[id]
    }
}

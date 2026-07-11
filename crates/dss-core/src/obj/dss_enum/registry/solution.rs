//! Solution / circuit-wide enum registrations — scan and sequence type, the
//! solution mode/algorithm, control and random mode, the default load-solution
//! model, circuit model, and the AutoAdd device type. Split out of
//! `registry/mod.rs` (no behavioral change).

use super::super::{DssEnum, EnumId};

pub(super) struct SolutionEnums {
    pub(super) scan_type: EnumId,
    pub(super) sequence: EnumId,
    pub(super) solve_mode: EnumId,
    pub(super) solve_alg: EnumId,
    pub(super) control_mode: EnumId,
    pub(super) random_mode: EnumId,
    pub(super) default_load_model: EnumId,
    pub(super) ckt_model: EnumId,
    pub(super) add_type: EnumId,
    pub(super) load_shape_class: EnumId,
}

pub(super) fn register(push: &mut dyn FnMut(DssEnum) -> EnumId) -> SolutionEnums {
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
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 0, 14, 15, 0, 2, 15, 16,
            9,
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
    // DSSClass.pas:1178 LoadShapeClassEnum (the `Set LoadShapeClass=` option,
    // consulted by the GENERALTIME/DYNAMICMODE nominal dispatch). min_match=1,
    // max_match=2; default USENONE.
    let mut lsc = DssEnum::new(
        "Load Shape Class",
        true,
        1,
        2,
        &["None", "Daily", "Yearly", "Duty"],
        &[-1, 0, 1, 2],
    );
    lsc.default_value = -1;
    let load_shape_class = push(lsc);
    SolutionEnums {
        scan_type,
        sequence,
        solve_mode,
        solve_alg,
        control_mode,
        random_mode,
        default_load_model,
        ckt_model,
        add_type,
        load_shape_class,
    }
}

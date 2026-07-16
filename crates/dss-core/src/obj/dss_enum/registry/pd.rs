//! Power-delivery / line-constants enum registrations — earth model, line type,
//! and the transformer core type and lead/lag (a reused 'Phase Sequence' enum).
//! Split out of `registry/mod.rs` (no behavioral change).

use super::super::{DssEnum, EnumId};

pub(super) struct PdEnums {
    pub(super) earth_model: EnumId,
    pub(super) line_type: EnumId,
    pub(super) core_type: EnumId,
    pub(super) lead_lag: EnumId,
    pub(super) autotrans_connection: EnumId,
    pub(super) gic_transformer_type: EnumId,
}

pub(super) fn register(push: &mut dyn FnMut(DssEnum) -> EnumId) -> PdEnums {
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
    // dss_capi 0.15.x (DSSClass.pas:1071, SVN 4103) widens the max match length
    // 4 -> 5: with 4 the eight `swt_*` names all collapse to the ambiguous 4-char
    // prefix `swt_`, so a 5-char abbreviation like `swt_l` fell back to the
    // default `oh` (WP-U1.4 LineType enum-width fix). Full names always matched
    // (the exact-name shortcut fires regardless of max length); only abbreviations
    // of length 5 that disambiguate the `swt_` group are affected.
    let mut ltype = DssEnum::new(
        "Line Type",
        true,
        2,
        5,
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

    // Pascal `AutoTransConnectionEnum` (AutoTrans.pas:320): adds `series` (2)
    // to the wye/delta set. Aliases y/ln → wye, ll → delta, and `s`/`ser…`
    // matches `series` by prefix. The capitalized display names (`Wye`/`Delta`/
    // `Series`) are JSON-only in Pascal; OrdinalToString uses the lowercase
    // first-array names (sequential 0/1/2), so `Conn` renders `series`.
    let autotrans_connection = push(DssEnum::new(
        "AutoTrans: Connection",
        true,
        1,
        2,
        &["wye", "delta", "series", "y", "ln", "ll"],
        &[0, 1, 2, 0, 0, 1],
    ));

    // Pascal `TGICTransformer.Create` (GICTransformer.pas:139):
    // `TDSSEnum.Create('GICTransformer: Type', True, 1, 1, ['GSU','Auto','YY'],
    // [SPEC_GSU, SPEC_AUTO, SPEC_YY])` with SPEC_GSU=1/SPEC_AUTO=2/SPEC_YY=3
    // (sequential; a single leading char disambiguates g/a/y).
    let gic_transformer_type = push(DssEnum::new(
        "GICTransformer: Type",
        true,
        1,
        1,
        &["GSU", "Auto", "YY"],
        &[1, 2, 3],
    ));

    PdEnums {
        earth_model,
        line_type,
        core_type,
        lead_lag,
        autotrans_connection,
        gic_transformer_type,
    }
}

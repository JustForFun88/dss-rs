use super::*;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropEngine};
use dss_parser::{Parser, ParserVars};

/// Apply `edits` to `obj` through the property engine and return all
/// errors (engine + deferred side-effect errors), like the executive does.
fn apply(cls: &ClassProps, obj: &mut dyn DssObject, edits: &[(&str, &str)]) -> Vec<String> {
    let enums = EnumRegistry::new();
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let mut errors = Vec::new();
    for (name, value) in edits {
        let idx = cls.property_index(name).expect("known property");
        let mut eng = PropEngine {
            parser: &mut parser,
            vars: &vars,
            enums: &enums,
            errors: &mut errors,
            foreign: None,
        };
        cls.edit_property(obj, idx, value, &mut eng).unwrap();
    }
    obj.end_edit();
    errors.extend(obj.data_mut().take_errors());
    errors
}

fn get(cls: &ClassProps, obj: &dyn DssObject, name: &str) -> String {
    let enums = EnumRegistry::new();
    let idx = cls.property_index(name).unwrap();
    cls.get_value(obj, idx, &enums)
}

/// Numeric value of a scalar property (our dump prints full f64 precision;
/// the oracle uses `%g`, so derived values are compared numerically — the
/// `props.json` gate already pins the exact oracle strings within tol).
fn getf(cls: &ClassProps, obj: &dyn DssObject, name: &str) -> f64 {
    get(cls, obj, name).parse().unwrap()
}

// ----- WireData (ConductorData defaults / couplings) --------------------

#[test]
fn wiredata_diam_and_dynamic_defaults() {
    // Oracle (dss-python 0.15.7): diam sets radius (scale 0.5); GMR and
    // CapRadius default from radius; Rac defaults from Rdc; radunits seeds
    // GMRunits.
    let enums = EnumRegistry::new();
    let cls = wire_data::class_props(&enums);
    let mut obj = WireDataObj::new("wd");
    let errs = apply(
        &cls,
        &mut obj,
        &[
            ("rdc", "0.05"),
            ("diam", "0.1"),
            ("runits", "ft"),
            ("radunits", "ft"),
        ],
    );
    assert!(errs.is_empty(), "{errs:?}");
    assert!((getf(&cls, &obj, "rac") - 0.051).abs() < 1e-9);
    assert!((getf(&cls, &obj, "radius") - 0.05).abs() < 1e-12);
    assert!((getf(&cls, &obj, "gmrac") - 0.7788 * 0.05).abs() < 1e-12);
    assert!((getf(&cls, &obj, "capradius") - 0.05).abs() < 1e-12);
    assert!((getf(&cls, &obj, "diam") - 0.1).abs() < 1e-12);
    assert_eq!(get(&cls, &obj, "gmrunits"), "ft"); // seeded from radunits
}

#[test]
fn wiredata_gmr_seeds_radius_and_emergamps() {
    // GMRac seeds radius (GMR/0.7788); GMRunits seeds radunits; normamps
    // seeds emergamps (×1.5).
    let enums = EnumRegistry::new();
    let cls = wire_data::class_props(&enums);
    let mut obj = WireDataObj::new("wg");
    let errs = apply(
        &cls,
        &mut obj,
        &[
            ("rdc", "0.04"),
            ("gmrac", "0.02"),
            ("gmrunits", "ft"),
            ("normamps", "400"),
        ],
    );
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "radunits"), "ft"); // seeded from gmrunits
    assert_eq!(get(&cls, &obj, "emergamps"), "600");
    // radius = 0.02 / 0.7788
    let r: f64 = get(&cls, &obj, "radius").parse().unwrap();
    assert!((r - 0.02 / 0.7788).abs() < 1e-9, "radius {r}");
}

#[test]
fn wiredata_makelike_does_not_copy_ratings() {
    // Pascal `TConductorDataObj.MakeLike` copies neither NumAmpRatings nor
    // AmpRatings, so a `like=` wire keeps its own default `[ -1]` / Seasons 1.
    let enums = EnumRegistry::new();
    let cls = wire_data::class_props(&enums);
    let mut src = WireDataObj::new("w1");
    apply(
        &cls,
        &mut src,
        &[
            ("rdc", "0.0526"),
            ("radius", "0.0306"),
            ("seasons", "2"),
            ("ratings", "600 800"),
        ],
    );
    assert_eq!(get(&cls, &src, "ratings"), "[ 600 800]");

    let mut dst = WireDataObj::new("w2");
    dst.make_like(&src);
    assert!((getf(&cls, &dst, "rdc") - 0.0526).abs() < 1e-12); // conductor data copied
    assert_eq!(get(&cls, &dst, "seasons"), "1"); // ratings NOT copied
    assert_eq!(get(&cls, &dst, "ratings"), "[ -1]");
}

// ----- CNData -----------------------------------------------------------

#[test]
fn cndata_strand_gmr_default() {
    // DiaStrand seeds GmrStrand = 0.7788 * 0.5 * DiaStrand when unset.
    let enums = EnumRegistry::new();
    let cls = cn_data::class_props(&enums);
    let mut obj = CnDataObj::new("cn");
    let errs = apply(&cls, &mut obj, &[("diastrand", "0.0641")]);
    assert!(errs.is_empty(), "{errs:?}");
    let g: f64 = get(&cls, &obj, "gmrstrand").parse().unwrap();
    assert!((g - 0.7788 * 0.5 * 0.0641).abs() < 1e-12, "gmrstrand {g}");
}

#[test]
fn cndata_too_few_strands_errors() {
    let enums = EnumRegistry::new();
    let cls = cn_data::class_props(&enums);
    let mut obj = CnDataObj::new("cn");
    let errs = apply(&cls, &mut obj, &[("k", "1")]);
    assert!(
        errs.iter()
            .any(|e| e.contains("at least 2 concentric neutral strands")),
        "{errs:?}"
    );
}

#[test]
fn cndata_low_permittivity_errors() {
    // EpsR has no NonNegative flag, so the side-effect <1.0 check is the
    // only guard (message uses the bare name).
    let enums = EnumRegistry::new();
    let cls = cn_data::class_props(&enums);
    let mut obj = CnDataObj::new("cn1");
    let errs = apply(&cls, &mut obj, &[("epsr", "0.5")]);
    assert!(
        errs.iter()
            .any(|e| e.contains("permittivity must be greater than one for CableData cn1")),
        "{errs:?}"
    );
}

// ----- TSData -----------------------------------------------------------

#[test]
fn tsdata_tapelap_out_of_range_errors() {
    let enums = EnumRegistry::new();
    let cls = ts_data::class_props(&enums);
    let mut obj = TsDataObj::new("ts1");
    let errs = apply(&cls, &mut obj, &[("tapelap", "150")]);
    assert!(
        errs.iter()
            .any(|e| e.contains("Tap lap must range from 0 to 100")),
        "{errs:?}"
    );
}

#[test]
fn tsdata_defaults() {
    // TapeLap default 20; Rac from Rdc; GMR/CapRadius from radius.
    let enums = EnumRegistry::new();
    let cls = ts_data::class_props(&enums);
    let obj = TsDataObj::new("ts0");
    assert_eq!(get(&cls, &obj, "tapelap"), "20");
    assert_eq!(get(&cls, &obj, "epsr"), "2.3");
    assert_eq!(get(&cls, &obj, "diam"), "-2");
}

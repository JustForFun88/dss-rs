use super::*;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropEngine};
use dss_parser::{Parser, ParserVars};

fn edited(edits: &[(&str, &str)]) -> (ClassProps, TShapeObj, crate::diag::ErrorLog) {
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let mut obj = TShapeObj::new("t");
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let mut errors = crate::diag::ErrorLog::new();
    for (name, value) in edits {
        let idx = cls.property_index(name).expect("known property");
        let mut eng = PropEngine {
            parser: &mut parser,
            vars: &vars,
            enums: &enums,
            errors: &mut errors,
            foreign: None,
        };
        cls.edit_property(&mut obj, idx, value, &mut eng).unwrap();
    }
    obj.end_edit(&crate::elements::traits::SysCtx::parse_default());
    errors.extend(obj.data_mut().take_errors());
    (cls, obj, errors)
}

fn get(cls: &ClassProps, obj: &TShapeObj, name: &str) -> String {
    let enums = EnumRegistry::new();
    let idx = cls.property_index(name).unwrap();
    cls.get_value(obj, idx, &enums)
}

#[test]
fn defaults_match_oracle() {
    // Oracle (dss-python 0.15.7) `new tshape.t`: empty data, Mean/StdDev 0
    // (no error, unlike LoadShape), Interval 1, SInterval 3600.
    let (cls, obj, errs) = edited(&[]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "NPts"), "0");
    assert_eq!(get(&cls, &obj, "Interval"), "1");
    assert_eq!(get(&cls, &obj, "Temp"), "");
    assert_eq!(get(&cls, &obj, "Hour"), "");
    assert_eq!(get(&cls, &obj, "Mean"), "0");
    assert_eq!(get(&cls, &obj, "StdDev"), "0");
    assert_eq!(get(&cls, &obj, "SInterval"), "3600");
    assert_eq!(get(&cls, &obj, "MInterval"), "60");
    assert_eq!(get(&cls, &obj, "Action"), "");
}

#[test]
fn fixed_interval_temps_and_stats() {
    // mult=(20 30 50 80) interval=1 → Mean 45, StdDev computed.
    let (cls, obj, errs) = edited(&[("npts", "4"), ("interval", "1"), ("temp", "20 30 50 80")]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "Temp"), "[ 20 30 50 80]");
    assert!((get(&cls, &obj, "Mean").parse::<f64>().unwrap() - 45.0).abs() < 1e-12);
    // RCDMeanAndStdDev is the sample std-dev (n-1 denominator): the variance
    // of [20,30,50,80] (mean 45) is (25²+15²+5²+35²)/3 = 2100/3 = 700, so the
    // std-dev is sqrt(700) (oracle prints 26.4575131106459).
    let want_std = 700.0_f64.sqrt();
    assert!((get(&cls, &obj, "StdDev").parse::<f64>().unwrap() - want_std).abs() < 1e-9);
}

#[test]
fn second_and_minute_interval_aliases() {
    let (cls, obj, errs) = edited(&[("npts", "4"), ("sinterval", "900"), ("temp", "1 2 4 8")]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "Interval"), "0.25");
    assert_eq!(get(&cls, &obj, "SInterval"), "900");
    assert_eq!(get(&cls, &obj, "MInterval"), "15");
}

#[test]
fn explicit_mean_stddev_override_computation() {
    let (cls, obj, errs) = edited(&[
        ("npts", "4"),
        ("interval", "1"),
        ("temp", "20 30 50 80"),
        ("mean", "5"),
        ("stddev", "2"),
    ]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "Mean"), "5");
    assert_eq!(get(&cls, &obj, "StdDev"), "2");
}

#[test]
fn get_temperature_fixed_interval_wraps() {
    // Hand-traced from the legacy even-interval branch. round(hr) wraps.
    let (_cls, mut obj, errs) =
        edited(&[("npts", "4"), ("interval", "1"), ("temp", "10 20 30 40")]);
    assert!(errs.is_empty(), "{errs:?}");
    for (hr, want) in [
        (0.0, 40.0), // round(0)=0 → wraps to NumPoints → TValues[4]
        (1.0, 10.0),
        (2.0, 20.0),
        (4.0, 40.0),
        (4.25, 40.0),
        (5.0, 10.0), // 5 mod 4 = 1 → TValues[1]
    ] {
        assert!(
            (obj.get_temperature(hr) - want).abs() < 1e-12,
            "hr {hr}: {} != {want}",
            obj.get_temperature(hr)
        );
    }
}

#[test]
fn get_temperature_variable_interval_interpolates() {
    // hour=[1,2,4], temp=[10,20,40]; interval=0. Legacy 1-based lookup.
    let (_cls, mut obj, errs) = edited(&[
        ("npts", "3"),
        ("interval", "0"),
        ("hour", "1 2 4"),
        ("temp", "10 20 40"),
    ]);
    assert!(errs.is_empty(), "{errs:?}");
    for (hr, want) in [
        (1.0, 10.0),
        (1.5, 15.0),
        (2.0, 20.0),
        (3.0, 30.0),
        (4.0, 40.0),
    ] {
        assert!(
            (obj.get_temperature(hr) - want).abs() < 1e-12,
            "hr {hr}: {} != {want}",
            obj.get_temperature(hr)
        );
    }
    // hr=5 wraps: 5 - trunc(5/4)*4 = 1 → exact point TValues[1]=10.
    assert!((obj.get_temperature(5.0) - 10.0).abs() < 1e-12);
}

#[test]
fn make_like_copies() {
    let (cls, base, errs) = edited(&[("npts", "3"), ("interval", "2"), ("temp", "11 22 33")]);
    assert!(errs.is_empty(), "{errs:?}");
    let mut obj = TShapeObj::new("t");
    obj.make_like(&base);
    obj.end_edit(&crate::elements::traits::SysCtx::parse_default());
    assert_eq!(get(&cls, &obj, "NPts"), "3");
    assert_eq!(get(&cls, &obj, "Interval"), "2");
    assert_eq!(get(&cls, &obj, "Temp"), "[ 11 22 33]");
}

#[test]
fn binary_file_props_queue_a_binary_file_load() {
    // WPG.1: SngFile/DblFile are ported (deferred FileLoad, like CSVFile).
    for name in ["sngfile", "dblfile"] {
        let (_cls, mut obj, errs) = edited(&[("npts", "4"), ("interval", "1"), (name, "t.bin")]);
        assert!(errs.is_empty(), "{name}: {errs:?}");
        let loads = obj.take_file_loads();
        assert_eq!(loads.len(), 1, "{name}");
        assert_eq!(loads[0].filename, "t.bin", "{name}");
        assert!(loads[0].binary, "{name}");
    }
}

#[test]
fn read_sng_file_fixed_interval() {
    let (cls, mut obj, _) = edited(&[("npts", "4"), ("interval", "1")]);
    let mut bytes = Vec::new();
    for v in [18.0f32, 19.5, 22.0, 26.5] {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    obj.core.read_sng_file(&bytes);
    obj.end_edit(&crate::elements::traits::SysCtx::parse_default());
    assert_eq!(get(&cls, &obj, "NPts"), "4");
    assert_eq!(get(&cls, &obj, "Temp"), "[ 18 19.5 22 26.5]");
}

#[test]
fn read_dbl_file_variable_interval() {
    let (cls, mut obj, _) = edited(&[("npts", "2"), ("interval", "0")]);
    let mut bytes = Vec::new();
    for (h, v) in [(0.0f64, 18.0f64), (3.0, 26.5)] {
        bytes.extend_from_slice(&h.to_le_bytes());
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    obj.core.read_dbl_file(&bytes);
    obj.end_edit(&crate::elements::traits::SysCtx::parse_default());
    assert_eq!(get(&cls, &obj, "Hour"), "[ 0 3]");
    assert_eq!(get(&cls, &obj, "Temp"), "[ 18 26.5]");
}

#[test]
fn csvfile_queues_a_file_load() {
    let (_cls, mut obj, errs) =
        edited(&[("npts", "6"), ("interval", "1"), ("csvfile", "temp.csv")]);
    let loads = obj.take_file_loads();
    assert_eq!(loads.len(), 1);
    assert_eq!(loads[0].prop, CSVFILE);
    assert_eq!(loads[0].filename, "temp.csv");
    assert!(errs.is_empty(), "{errs:?}");
}

#[test]
fn action_dblsave_queues_bare_name_double_save() {
    // WPG.17: `Action=DblSave/SngSave` now queues a binary write (Pascal
    // `TTShapeObj.SaveToDblFile`/`SaveToSngFile`). TShape uses the bare `<name>`
    // filename (no `_P`/`_Q` split), a single value series, GlobalResult tag `Temp`.
    let (_cls, mut obj, errs) = edited(&[
        ("npts", "4"),
        ("interval", "1"),
        ("temp", "20 30 50 80"),
        ("action", "dblsave"),
    ]);
    assert!(errs.is_empty(), "{errs:?}");
    let saves = obj.take_shape_saves();
    assert_eq!(saves.len(), 1);
    let s = &saves[0];
    assert!(!s.sng, "dblsave → double precision");
    assert!(!s.p_suffix, "TShape writes the bare <name>");
    assert_eq!(s.result_tag, "Temp");
    assert_eq!(s.values, vec![20.0, 30.0, 50.0, 80.0]);
    assert!(s.q_values.is_none(), "TShape has no Q series");
}

#[test]
fn action_sngsave_queues_single_precision() {
    let (_cls, mut obj, errs) = edited(&[
        ("npts", "3"),
        ("interval", "1"),
        ("temp", "10 20 30"),
        ("action", "sngsave"),
    ]);
    assert!(errs.is_empty(), "{errs:?}");
    let saves = obj.take_shape_saves();
    assert_eq!(saves.len(), 1);
    assert!(saves[0].sng, "sngsave → single precision");
    assert_eq!(saves[0].values, vec![10.0, 20.0, 30.0]);
}

#[test]
fn action_save_undefined_series_errors() {
    // Pascal `if not Assigned(TValues)` → `DoSimpleMsg('%s Temperatures not
    // defined.', …)` and no queued save.
    let (_cls, mut obj, errs) = edited(&[("action", "dblsave")]);
    assert!(
        errs.iter()
            .any(|e| e.to_lowercase().contains("temperatures not defined")),
        "{errs:?}"
    );
    assert!(obj.take_shape_saves().is_empty());
}

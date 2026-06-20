use super::*;
use crate::obj::base::DssObject;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropEngine};
use dss_parser::{Parser, ParserVars};

fn edited(edits: &[(&str, &str)]) -> (ClassProps, LoadShapeObj, Vec<String>) {
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let mut obj = LoadShapeObj::new("d");
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
        cls.edit_property(&mut obj, idx, value, &mut eng).unwrap();
    }
    obj.end_edit();
    errors.extend(obj.data_mut().take_errors());
    (cls, obj, errors)
}

fn get(cls: &ClassProps, obj: &LoadShapeObj, name: &str) -> String {
    let enums = EnumRegistry::new();
    let idx = cls.property_index(name).unwrap();
    cls.get_value(obj, idx, &enums)
}

#[test]
fn defaults_match_oracle() {
    // Oracle (dss-python 0.15.7) `new loadshape.d`.
    let (cls, obj, errs) = edited(&[]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "NPts"), "0");
    assert_eq!(get(&cls, &obj, "Interval"), "1");
    assert_eq!(get(&cls, &obj, "Mult"), "");
    assert_eq!(get(&cls, &obj, "PMax"), "1");
    assert_eq!(get(&cls, &obj, "QMax"), "0");
    assert_eq!(get(&cls, &obj, "SInterval"), "3600");
    assert_eq!(get(&cls, &obj, "MInterval"), "60");
    assert_eq!(get(&cls, &obj, "Interpolation"), "Avg");
    assert_eq!(get(&cls, &obj, "Action"), "");
}

#[test]
fn fixed_interval_arrays_and_stats() {
    // Oracle: mult=(1 2 4 8) interval=1 → Mean 3.75, StdDev 3.0956…, PMax 8.
    let (cls, obj, errs) = edited(&[("npts", "4"), ("interval", "1"), ("mult", "1 2 4 8")]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "Mult"), "[ 1 2 4 8]");
    assert_eq!(get(&cls, &obj, "PMult"), "[ 1 2 4 8]");
    assert_eq!(get(&cls, &obj, "PMax"), "8");
    assert!((get(&cls, &obj, "Mean").parse::<f64>().unwrap() - 3.75).abs() < 1e-12);
    assert!((get(&cls, &obj, "StdDev").parse::<f64>().unwrap() - 3.09569593683445).abs() < 1e-9);
}

#[test]
fn qmult_drives_coincident_qmax() {
    // Oracle: QMax = Q at the index where |P| peaks (index 3 → 0.8).
    let (cls, obj, errs) = edited(&[
        ("npts", "4"),
        ("interval", "1"),
        ("mult", "1 2 4 8"),
        ("qmult", ".5 .6 .7 .8"),
    ]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "QMult"), "[ 0.5 0.6 0.7 0.8]");
    assert_eq!(get(&cls, &obj, "QMax"), "0.8");
}

#[test]
fn second_and_minute_interval_aliases() {
    // sinterval=900 → 0.25 hr; minterval mirrors it.
    let (cls, obj, errs) = edited(&[("npts", "4"), ("sinterval", "900"), ("mult", "1 2 4 8")]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "Interval"), "0.25");
    assert_eq!(get(&cls, &obj, "SInterval"), "900");
    assert_eq!(get(&cls, &obj, "MInterval"), "15");
}

#[test]
fn hour_array_sets_variable_interval() {
    let (cls, obj, errs) = edited(&[
        ("npts", "3"),
        ("interval", "0"),
        ("hour", "1 2 4"),
        ("mult", "1 2 4"),
    ]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "Interval"), "0");
    assert_eq!(get(&cls, &obj, "Hour"), "[ 1 2 4]");
    // Trapezoid mean over the curve.
    assert!((get(&cls, &obj, "Mean").parse::<f64>().unwrap() - 2.5).abs() < 1e-12);
}

#[test]
fn normalize_scales_to_peak() {
    // Oracle: action=normalize divides by peak (8) → max becomes 1.
    let (cls, obj, errs) = edited(&[
        ("npts", "4"),
        ("interval", "1"),
        ("mult", "2 4 6 8"),
        ("action", "normalize"),
    ]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "Mult"), "[ 0.25 0.5 0.75 1]");
    assert_eq!(get(&cls, &obj, "PMax"), "1");
}

#[test]
fn normalize_uses_pbase_when_set() {
    // Oracle: pbase=10 → divide by 10 (not the peak).
    let (cls, obj, errs) = edited(&[
        ("npts", "4"),
        ("interval", "1"),
        ("mult", "2 4 6 8"),
        ("pbase", "10"),
        ("action", "normalize"),
    ]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "Mult"), "[ 0.2 0.4 0.6 0.8]");
}

#[test]
fn explicit_mean_stddev_override_computation() {
    let (cls, obj, errs) = edited(&[
        ("npts", "4"),
        ("interval", "1"),
        ("mult", "2 4 6 8"),
        ("mean", "5"),
        ("stddev", "2"),
    ]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "Mean"), "5");
    assert_eq!(get(&cls, &obj, "StdDev"), "2");
}

#[test]
fn get_mult_at_hour_fixed_interval_wraps() {
    // Hand-traced from the Pascal even-interval branch (Avg, no Q).
    // dP = [10, 20, 30, 40], interval=1. round(hr) wraps: hr 0 → last point.
    let (_cls, mut obj, errs) =
        edited(&[("npts", "4"), ("interval", "1"), ("mult", "10 20 30 40")]);
    assert!(errs.is_empty(), "{errs:?}");
    let cases = [
        (0.0, 40.0), // round(0)=0 → wraps to NumPoints → dP[3]
        (1.0, 10.0), // round(1)=1 → dP[0]
        (2.0, 20.0),
        (3.0, 30.0),
        (4.0, 40.0),
        (4.25, 40.0), // round(4.25)=4 → dP[3]
        (5.0, 10.0),  // 5 mod 4 = 1 → dP[0]
    ];
    for (hr, want) in cases {
        let m = obj.get_mult_at_hour(hr);
        assert!(
            (m.re - want).abs() < 1e-12,
            "P at hr {hr}: {} != {want}",
            m.re
        );
        // No Q array, not actual: im mirrors re.
        assert!((m.im - want).abs() < 1e-12, "Q at hr {hr}");
    }
}

#[test]
fn get_mult_at_hour_variable_interval_interpolates() {
    // Hand-traced from the Pascal random-interval branch (Avg).
    // hour=[1,2,4], mult=[1,2,4].
    let (_cls, mut obj, errs) = edited(&[
        ("npts", "3"),
        ("interval", "0"),
        ("hour", "1 2 4"),
        ("mult", "1 2 4"),
    ]);
    assert!(errs.is_empty(), "{errs:?}");
    for (hr, want) in [(1.0, 1.0), (1.5, 1.5), (2.0, 2.0), (3.0, 3.0), (4.0, 4.0)] {
        let m = obj.get_mult_at_hour(hr);
        assert!((m.re - want).abs() < 1e-12, "hr {hr}: {} != {want}", m.re);
    }
    // hr=5 wraps: 5 - trunc(5/4)*4 = 1 → exact point dP[0]=1.
    assert!((obj.get_mult_at_hour(5.0).re - 1.0).abs() < 1e-12);
}

#[test]
fn get_mult_at_hour_use_actual_zero_q() {
    // UseActual: missing Q multiplier → 0, not a mirror of P.
    let (_cls, mut obj, errs) = edited(&[
        ("npts", "2"),
        ("interval", "1"),
        ("mult", "3 6"),
        ("useactual", "yes"),
    ]);
    assert!(errs.is_empty(), "{errs:?}");
    let m = obj.get_mult_at_hour(1.0);
    assert!((m.re - 3.0).abs() < 1e-12);
    assert_eq!(m.im, 0.0);
}

#[test]
fn make_like_copies_and_recomputes() {
    let (cls, base, errs) = edited(&[
        ("npts", "3"),
        ("interval", "2"),
        ("mult", "1 2 3"),
        ("qmult", "4 5 6"),
        ("pbase", "7"),
        ("useactual", "yes"),
    ]);
    assert!(errs.is_empty(), "{errs:?}");
    let mut obj = LoadShapeObj::new("d");
    obj.make_like(&base);
    obj.end_edit();
    assert_eq!(get(&cls, &obj, "NPts"), "3");
    assert_eq!(get(&cls, &obj, "Interval"), "2");
    assert_eq!(get(&cls, &obj, "Mult"), "[ 1 2 3]");
    assert_eq!(get(&cls, &obj, "QMult"), "[ 4 5 6]");
    assert_eq!(get(&cls, &obj, "PMax"), "3");
    assert_eq!(get(&cls, &obj, "QMax"), "6");
    assert_eq!(get(&cls, &obj, "PBase"), "7");
    assert_eq!(get(&cls, &obj, "UseActual"), "Yes");
}

#[test]
fn binary_file_props_are_not_ported() {
    // CSVFile is now ported; SngFile/DblFile/PQCSVFile stay NOT_PORTED.
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    for name in ["sngfile", "dblfile", "pqcsvfile"] {
        let mut obj = LoadShapeObj::new("d");
        let mut parser = Parser::new();
        let vars = ParserVars::new();
        let mut errors = Vec::new();
        let idx = cls.property_index(name).unwrap();
        let mut eng = PropEngine {
            parser: &mut parser,
            vars: &vars,
            enums: &enums,
            errors: &mut errors,
            foreign: None,
        };
        let err = cls
            .edit_property(&mut obj, idx, "shape.bin", &mut eng)
            .unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("not ported"),
            "{name}: {err}"
        );
    }
}

#[test]
fn csvfile_queues_a_file_load() {
    // Setting CSVFile records a deferred load (the executive does the read).
    let (_cls, mut obj, errs) = edited(&[("npts", "6"), ("interval", "1"), ("csvfile", "day.csv")]);
    // No executive here, so the file is never read — but the request is
    // queued and the stored filename reads back.
    let loads = obj.take_file_loads();
    assert_eq!(loads.len(), 1);
    assert_eq!(loads[0].prop, prop::CSVFILE);
    assert_eq!(loads[0].filename, "day.csv");
    assert!(errs.is_empty(), "{errs:?}");
}

#[test]
fn read_csv_file_fixed_and_variable_interval() {
    // Fixed interval: one mult per row; oracle Mult=[0.3 0.5 0.9 1 0.7 0.4].
    let (cls, mut obj, _) = edited(&[("npts", "6"), ("interval", "1")]);
    obj.read_csv_file("0.3\n0.5\n0.9\n1.0\n0.7\n0.4\n");
    obj.end_edit();
    assert_eq!(get(&cls, &obj, "NPts"), "6");
    assert_eq!(get(&cls, &obj, "Mult"), "[ 0.3 0.5 0.9 1 0.7 0.4]");
    assert_eq!(get(&cls, &obj, "PMax"), "1");

    // Variable interval: `hour, mult` per row; oracle Hour=[0 2 5 9].
    let (cls, mut obj, _) = edited(&[("npts", "4"), ("interval", "0")]);
    obj.read_csv_file("0,0.30\n2,0.55\n5,0.95\n9,0.60\n");
    obj.end_edit();
    assert_eq!(get(&cls, &obj, "Hour"), "[ 0 2 5 9]");
    assert_eq!(get(&cls, &obj, "Mult"), "[ 0.3 0.55 0.95 0.6]");
    assert_eq!(get(&cls, &obj, "PMax"), "0.95");
}

#[test]
fn read_csv_file_shrinks_npts_to_lines_read() {
    // Oracle: npts=10 but only 6 rows in the file → NumPoints becomes 6.
    let (cls, mut obj, _) = edited(&[("npts", "10"), ("interval", "1")]);
    obj.read_csv_file("0.3\n0.5\n0.9\n1.0\n0.7\n0.4\n");
    obj.end_edit();
    assert_eq!(get(&cls, &obj, "NPts"), "6");
    assert_eq!(get(&cls, &obj, "Mult"), "[ 0.3 0.5 0.9 1 0.7 0.4]");
}

/// Full path through the executive: a temp CSV resolved relative to the
/// script's current directory, read and parsed (Pascal `DoCSVFile`). Values
/// are transcribed from the pinned oracle.
#[test]
fn csvfile_through_executive_matches_oracle() {
    use crate::exec::Dss;

    let dir = std::env::temp_dir().join(format!(
        "dss_ls_csv_{}_{}",
        std::process::id(),
        // a per-test nonce so parallel runs don't collide
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let day6 = dir.join("day6.csv");
    std::fs::write(&day6, "0.3\n0.5\n0.9\n1.0\n0.7\n0.4\n").unwrap();

    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.p");
    dss.command(&format!(
        "New LoadShape.d npts=6 interval=1 csvfile=\"{}\"",
        day6.display()
    ));
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    dss.command("? LoadShape.d.npts");
    assert_eq!(dss.result(), "6");
    dss.command("? LoadShape.d.mult");
    assert_eq!(dss.result(), "[ 0.3 0.5 0.9 1 0.7 0.4]");
    dss.command("? LoadShape.d.pmax");
    assert_eq!(dss.result(), "1");
    // Oracle Mean 0.633333…, StdDev 0.280475786239502.
    dss.command("? LoadShape.d.mean");
    assert!((dss.result().parse::<f64>().unwrap() - 0.633_333_333_333_333).abs() < 1e-9);
    dss.command("? LoadShape.d.stddev");
    assert!((dss.result().parse::<f64>().unwrap() - 0.280_475_786_239_502).abs() < 1e-9);

    std::fs::remove_dir_all(&dir).ok();
}

/// A missing CSV file is Pascal error 613 (recorded, edit continues).
#[test]
fn csvfile_missing_records_error() {
    use crate::exec::Dss;
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.p");
    dss.command("New LoadShape.d npts=6 interval=1 csvfile=does_not_exist_42.csv");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("Error opening file")),
        "expected a 613-style error, got {:?}",
        dss.errors()
    );
}

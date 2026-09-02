use super::*;
use crate::obj::base::DssObject;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropEngine};
use dss_parser::{Parser, ParserVars};

fn edited(edits: &[(&str, &str)]) -> (ClassProps, XyCurveObj, crate::diag::ErrorLog) {
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let mut obj = XyCurveObj::new("c1");
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

            was_quoted: false,
        };
        cls.edit_property(&mut obj, idx, value, &mut eng).unwrap();
    }
    obj.end_edit(&crate::elements::traits::SysCtx::parse_default());
    errors.extend(obj.data_mut().take_errors());
    (cls, obj, errors)
}

fn get(cls: &ClassProps, obj: &XyCurveObj, name: &str) -> String {
    let enums = EnumRegistry::new();
    let idx = cls.property_index(name).unwrap();
    cls.get_value(obj, idx, &enums)
}

#[test]
fn arrays_and_points_round_trip() {
    // Oracle (dss-python 0.15.7): points reads back interleaved.
    let (cls, obj, errs) = edited(&[
        ("npts", "4"),
        ("yarray", "10 20 30 40"),
        ("xarray", "1 2 3 4"),
    ]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "npts"), "4");
    assert_eq!(get(&cls, &obj, "xarray"), "[ 1 2 3 4]");
    assert_eq!(get(&cls, &obj, "yarray"), "[ 10 20 30 40]");
    assert_eq!(get(&cls, &obj, "points"), "[ 1 10 2 20 3 30 4 40]");
    // First point feeds the X/Y accessors.
    assert_eq!(get(&cls, &obj, "x"), "1");
    assert_eq!(get(&cls, &obj, "y"), "10");
}

#[test]
fn get_y_value_interpolates_and_extrapolates() {
    // Values transcribed from the oracle (probe_xy3.py).
    let (_cls, mut obj, errs) = edited(&[
        ("npts", "4"),
        ("xarray", "1 2 3 4"),
        ("yarray", "10 20 30 40"),
    ]);
    assert!(errs.is_empty(), "{errs:?}");
    for (x, y) in [
        (0.5, 5.0),
        (1.0, 10.0),
        (1.5, 15.0),
        (2.5, 25.0),
        (4.0, 40.0),
        (5.0, 50.0),
    ] {
        assert!((obj.get_y_value(x) - y).abs() < 1e-9, "GetY({x})");
    }
}

#[test]
fn x_setter_syncs_y_with_shift_and_scale() {
    // Oracle (probe_xy2.py): scales/shifts applied, then `x=2` → y=20.
    let (cls, mut obj, errs) = edited(&[
        ("npts", "3"),
        ("xarray", "0 1 2"),
        ("yarray", "0 10 20"),
        ("xscale", "2"),
        ("yscale", "3"),
        ("xshift", "1"),
        ("yshift", "5"),
    ]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "x"), "1");
    assert_eq!(get(&cls, &obj, "y"), "5");
    // edit x=2
    let enums = EnumRegistry::new();
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let mut errors = crate::diag::ErrorLog::new();
    let idx = cls.property_index("x").unwrap();
    let mut eng = PropEngine {
        parser: &mut parser,
        vars: &vars,
        enums: &enums,
        errors: &mut errors,
        foreign: None,

        was_quoted: false,
    };
    cls.edit_property(&mut obj, idx, "2", &mut eng).unwrap();
    assert_eq!(get(&cls, &obj, "x"), "2");
    assert_eq!(get(&cls, &obj, "y"), "20");
}

#[test]
fn set_points_splits_pairs() {
    // The oracle crashes on `points=`, so this path is Rust-only.
    let (cls, obj, errs) = edited(&[("points", "1 10 2 20 3 30")]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "npts"), "3");
    assert_eq!(get(&cls, &obj, "xarray"), "[ 1 2 3]");
    assert_eq!(get(&cls, &obj, "yarray"), "[ 10 20 30]");
    assert_eq!(get(&cls, &obj, "points"), "[ 1 10 2 20 3 30]");
}

#[test]
fn file_props_queue_a_file_load() {
    // WPG.17: CSVFile/SngFile/DblFile are ported (deferred FileLoad); csv is a
    // text load, sng/dbl are binary loads.
    for (name, prop_idx, binary) in [
        ("csvfile", prop::CSVFILE, false),
        ("sngfile", prop::SNGFILE, true),
        ("dblfile", prop::DBLFILE, true),
    ] {
        let (_cls, mut obj, errs) = edited(&[("npts", "4"), (name, "x.bin")]);
        assert!(
            !errs.iter().any(|e| e.to_lowercase().contains("not ported")),
            "{name}: {errs:?}"
        );
        let loads = obj.take_file_loads();
        assert_eq!(loads.len(), 1, "{name}");
        assert_eq!(loads[0].prop, prop_idx, "{name}");
        assert_eq!(loads[0].filename, "x.bin", "{name}");
        assert_eq!(loads[0].binary, binary, "{name}");
    }
}

#[test]
fn read_csv_file_parses_xy_and_shrinks() {
    // Pascal `DoCSVFile` with pA=XValues, pB=YValues. AuxParser takes comma or
    // whitespace as the delimiter, so mix both forms in the fixture.
    let (cls, mut obj, _) = edited(&[("npts", "4")]);
    obj.read_csv_file("60, 1.0\n180 1.4\n300, 2.1\n420 3.0\n");
    obj.sync_first_point();
    assert_eq!(get(&cls, &obj, "npts"), "4");
    assert_eq!(get(&cls, &obj, "xarray"), "[ 60 180 300 420]");
    assert_eq!(get(&cls, &obj, "yarray"), "[ 1 1.4 2.1 3]");
    assert_eq!(get(&cls, &obj, "points"), "[ 60 1 180 1.4 300 2.1 420 3]");
    // First point feeds the X/Y accessors (Pascal :357-361).
    assert_eq!(get(&cls, &obj, "x"), "60");
    assert_eq!(get(&cls, &obj, "y"), "1");

    // Short file: npts=6 but only 4 rows → NumPoints := i (unconditional).
    let (cls, mut obj, _) = edited(&[("npts", "6")]);
    obj.read_csv_file("60, 1.0\n180, 1.4\n300, 2.1\n420, 3.0\n");
    assert_eq!(get(&cls, &obj, "npts"), "4");
    assert_eq!(get(&cls, &obj, "yarray"), "[ 1 1.4 2.1 3]");
}

#[test]
fn read_sng_file_parses_pairs() {
    // LE f32 (x, y) pairs; the f32→f64 widen is transcribed from the pinned
    // oracle (0.95 → 0.949999988079071, etc.).
    let (cls, mut obj, _) = edited(&[("npts", "4")]);
    let mut bytes = Vec::new();
    for (x, y) in [
        (60.0f32, 1.0f32),
        (180.0, 0.95),
        (300.0, 0.88),
        (420.0, 0.80),
    ] {
        bytes.extend_from_slice(&x.to_le_bytes());
        bytes.extend_from_slice(&y.to_le_bytes());
    }
    obj.read_sng_file(&bytes);
    obj.sync_first_point();
    assert_eq!(get(&cls, &obj, "npts"), "4");
    assert_eq!(get(&cls, &obj, "xarray"), "[ 60 180 300 420]");
    assert_eq!(
        get(&cls, &obj, "yarray"),
        "[ 1 0.949999988079071 0.879999995231628 0.800000011920929]"
    );
    assert_eq!(get(&cls, &obj, "x"), "60");
    assert_eq!(get(&cls, &obj, "y"), "1");

    // Short file: npts=5 but only 3 full pairs → shrink to 3 (Pascal
    // `if i <> NumPoints then NumPoints := i`).
    let (cls, mut obj, _) = edited(&[("npts", "5")]);
    let mut bytes = Vec::new();
    for (x, y) in [(60.0f32, 1.0f32), (180.0, 0.95), (300.0, 0.88)] {
        bytes.extend_from_slice(&x.to_le_bytes());
        bytes.extend_from_slice(&y.to_le_bytes());
    }
    obj.read_sng_file(&bytes);
    assert_eq!(get(&cls, &obj, "npts"), "3");
    assert_eq!(get(&cls, &obj, "xarray"), "[ 60 180 300]");
}

#[test]
fn read_dbl_file_parses_pairs() {
    // LE f64 (x, y) pairs — exact, no widen.
    let (cls, mut obj, _) = edited(&[("npts", "4")]);
    let mut bytes = Vec::new();
    for (x, y) in [(60.0f64, 1.0f64), (180.0, 1.2), (300.0, 1.5), (420.0, 2.0)] {
        bytes.extend_from_slice(&x.to_le_bytes());
        bytes.extend_from_slice(&y.to_le_bytes());
    }
    obj.read_dbl_file(&bytes);
    obj.sync_first_point();
    assert_eq!(get(&cls, &obj, "npts"), "4");
    assert_eq!(get(&cls, &obj, "xarray"), "[ 60 180 300 420]");
    assert_eq!(get(&cls, &obj, "yarray"), "[ 1 1.2 1.5 2]");
    assert_eq!(get(&cls, &obj, "x"), "60");
    assert_eq!(get(&cls, &obj, "y"), "1");

    // Short file: npts=4 but only 2 full pairs → shrink to 2.
    let (cls, mut obj, _) = edited(&[("npts", "4")]);
    let mut bytes = Vec::new();
    for (x, y) in [(60.0f64, 1.0f64), (180.0, 1.2)] {
        bytes.extend_from_slice(&x.to_le_bytes());
        bytes.extend_from_slice(&y.to_le_bytes());
    }
    obj.read_dbl_file(&bytes);
    assert_eq!(get(&cls, &obj, "npts"), "2");
    assert_eq!(get(&cls, &obj, "yarray"), "[ 1 1.2]");
}

/// Full path through the executive for all three file readers: temp
/// `.csv`/`.sng`/`.dbl` files resolved relative to the script's current dir,
/// read and parsed (Pascal `DoCSVFile`/`DoSngFile`/`DoDblFile`). Values are
/// transcribed from the pinned oracle (dss-python 0.15.7).
#[test]
fn xycurve_files_through_executive_match_oracle() {
    use crate::exec::Dss;

    let dir = std::env::temp_dir().join(format!(
        "dss_xyf_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let rc = dir.join("rc.csv");
    std::fs::write(&rc, "60, 1.0\n180, 1.4\n300, 2.1\n420, 3.0\n").unwrap();

    let lc = dir.join("lc.sng");
    let mut sng = Vec::new();
    for (x, y) in [
        (60.0f32, 1.0f32),
        (180.0, 0.95),
        (300.0, 0.88),
        (420.0, 0.80),
    ] {
        sng.extend_from_slice(&x.to_le_bytes());
        sng.extend_from_slice(&y.to_le_bytes());
    }
    std::fs::write(&lc, &sng).unwrap();

    let rc2 = dir.join("rc2.dbl");
    let mut dbl = Vec::new();
    for (x, y) in [(60.0f64, 1.0f64), (180.0, 1.2), (300.0, 1.5), (420.0, 2.0)] {
        dbl.extend_from_slice(&x.to_le_bytes());
        dbl.extend_from_slice(&y.to_le_bytes());
    }
    std::fs::write(&rc2, &dbl).unwrap();

    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.p");
    dss.command(&format!(
        "New XYcurve.rc_csv npts=4 csvfile=\"{}\"",
        rc.display()
    ));
    dss.command(&format!(
        "New XYcurve.lc_sng npts=4 sngfile=\"{}\"",
        lc.display()
    ));
    dss.command(&format!(
        "New XYcurve.rc_dbl npts=4 dblfile=\"{}\"",
        rc2.display()
    ));
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    // CSVFile
    dss.command("? XYcurve.rc_csv.npts");
    assert_eq!(dss.result(), "4");
    dss.command("? XYcurve.rc_csv.xarray");
    assert_eq!(dss.result(), "[ 60 180 300 420]");
    dss.command("? XYcurve.rc_csv.yarray");
    assert_eq!(dss.result(), "[ 1 1.4 2.1 3]");
    dss.command("? XYcurve.rc_csv.x");
    assert_eq!(dss.result(), "60");
    dss.command("? XYcurve.rc_csv.y");
    assert_eq!(dss.result(), "1");

    // SngFile (f32→f64 widen, oracle-transcribed).
    dss.command("? XYcurve.lc_sng.yarray");
    assert_eq!(
        dss.result(),
        "[ 1 0.949999988079071 0.879999995231628 0.800000011920929]"
    );
    dss.command("? XYcurve.lc_sng.points");
    assert_eq!(
        dss.result(),
        "[ 60 1 180 0.949999988079071 300 0.879999995231628 420 0.800000011920929]"
    );

    // DblFile
    dss.command("? XYcurve.rc_dbl.yarray");
    assert_eq!(dss.result(), "[ 1 1.2 1.5 2]");
    dss.command("? XYcurve.rc_dbl.x");
    assert_eq!(dss.result(), "60");
    dss.command("? XYcurve.rc_dbl.y");
    assert_eq!(dss.result(), "1");

    std::fs::remove_dir_all(&dir).ok();
}

/// A missing file (any of the three props) is the executive's generic open
/// error (Pascal 615/58613); the edit continues.
#[test]
fn missing_file_records_error() {
    use crate::exec::Dss;
    for (prop_name, ext) in [("csvfile", "csv"), ("sngfile", "sng"), ("dblfile", "dbl")] {
        let mut dss = Dss::new();
        dss.command("clear");
        dss.command("new circuit.p");
        dss.command(&format!(
            "New XYcurve.c npts=4 {prop_name}=does_not_exist_42.{ext}"
        ));
        assert!(
            dss.errors()
                .iter()
                .any(|e| e.contains("Error opening file")),
            "{prop_name}: expected an open error, got {:?}",
            dss.errors()
        );
    }
}

#[test]
fn make_like_copies_curve() {
    let (cls, base, _) = edited(&[("npts", "2"), ("xarray", "0 1"), ("yarray", "5 7")]);
    let mut obj = XyCurveObj::new("derived");
    obj.make_like(&base);
    assert_eq!(get(&cls, &obj, "npts"), "2");
    assert_eq!(get(&cls, &obj, "xarray"), "[ 0 1]");
    assert_eq!(get(&cls, &obj, "yarray"), "[ 5 7]");
}

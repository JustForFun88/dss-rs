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
fn binary_and_pq_file_props_queue_a_file_load() {
    // WPG.1: SngFile/DblFile/PQCSVFile are ported (deferred FileLoad, like
    // CSVFile); SngFile/DblFile queue a *binary* load, PQCSVFile a text one.
    for (name, prop, binary) in [
        ("sngfile", prop::SNGFILE, true),
        ("dblfile", prop::DBLFILE, true),
        ("pqcsvfile", prop::PQCSVFILE, false),
    ] {
        let (_cls, mut obj, errs) = edited(&[("npts", "4"), ("interval", "1"), (name, "s.bin")]);
        assert!(errs.is_empty(), "{name}: {errs:?}");
        let loads = obj.take_file_loads();
        assert_eq!(loads.len(), 1, "{name}");
        assert_eq!(loads[0].prop, prop, "{name}");
        assert_eq!(loads[0].filename, "s.bin", "{name}");
        assert_eq!(loads[0].binary, binary, "{name}");
    }
}

#[test]
fn read_sng_file_fixed_interval() {
    // 4 x f32 P multipliers, little-endian (Pascal `ReadSngFile`). Values are
    // exactly representable in f32 so the f32->f64 widen round-trips exactly.
    let (cls, mut obj, _) = edited(&[("npts", "4"), ("interval", "1")]);
    let mut bytes = Vec::new();
    for v in [0.25f32, 0.5, 0.75, 1.0] {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    obj.read_sng_file(&bytes);
    obj.end_edit();
    assert_eq!(get(&cls, &obj, "NPts"), "4");
    assert_eq!(get(&cls, &obj, "Mult"), "[ 0.25 0.5 0.75 1]");
}

#[test]
fn read_sng_file_variable_interval_pairs_and_shrinks() {
    // (hour, mult) f32 pairs; a short final pair is dropped and NumPoints
    // shrinks to the count actually read (Pascal `NumPoints := i`).
    let (cls, mut obj, _) = edited(&[("npts", "5"), ("interval", "0")]);
    let mut bytes = Vec::new();
    for (h, m) in [(0.0f32, 0.25f32), (1.0, 0.5), (2.0, 0.75)] {
        bytes.extend_from_slice(&h.to_le_bytes());
        bytes.extend_from_slice(&m.to_le_bytes());
    }
    obj.read_sng_file(&bytes);
    obj.end_edit();
    assert_eq!(get(&cls, &obj, "NPts"), "3");
    assert_eq!(get(&cls, &obj, "Hour"), "[ 0 1 2]");
    assert_eq!(get(&cls, &obj, "Mult"), "[ 0.25 0.5 0.75]");
}

#[test]
fn read_dbl_file_fixed_and_variable_interval() {
    let (cls, mut obj, _) = edited(&[("npts", "3"), ("interval", "1")]);
    let mut bytes = Vec::new();
    for v in [0.3f64, 0.5, 0.9] {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    obj.read_dbl_file(&bytes);
    obj.end_edit();
    assert_eq!(get(&cls, &obj, "Mult"), "[ 0.3 0.5 0.9]");

    let (cls, mut obj, _) = edited(&[("npts", "2"), ("interval", "0")]);
    let mut bytes = Vec::new();
    for (h, m) in [(0.0f64, 0.4f64), (2.0, 0.8)] {
        bytes.extend_from_slice(&h.to_le_bytes());
        bytes.extend_from_slice(&m.to_le_bytes());
    }
    obj.read_dbl_file(&bytes);
    obj.end_edit();
    assert_eq!(get(&cls, &obj, "Hour"), "[ 0 2]");
    assert_eq!(get(&cls, &obj, "Mult"), "[ 0.4 0.8]");
}

#[test]
fn read_pq_csv_file_fixed_and_variable_interval() {
    let (cls, mut obj, _) = edited(&[("npts", "3"), ("interval", "1")]);
    obj.read_pq_csv_file("0.3, 0.2\n0.5, 0.4\n0.9, 0.7\n");
    obj.end_edit();
    assert_eq!(get(&cls, &obj, "Mult"), "[ 0.3 0.5 0.9]");
    assert_eq!(get(&cls, &obj, "QMult"), "[ 0.2 0.4 0.7]");

    let (cls, mut obj, _) = edited(&[("npts", "2"), ("interval", "0")]);
    obj.read_pq_csv_file("0, 0.3, 0.2\n1, 0.5, 0.4\n");
    obj.end_edit();
    assert_eq!(get(&cls, &obj, "Hour"), "[ 0 1]");
    assert_eq!(get(&cls, &obj, "Mult"), "[ 0.3 0.5]");
    assert_eq!(get(&cls, &obj, "QMult"), "[ 0.2 0.4]");
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

/// Full path through the executive for the binary readers (WPG.1): a temp
/// `.sng`/`.dbl` file resolved relative to the script's current directory,
/// read as raw bytes and parsed (Pascal `ReadSngFile`/`ReadDblFile`). Values
/// are transcribed from the pinned oracle (dss-python 0.15.7).
#[test]
fn sng_and_dbl_file_through_executive_match_oracle() {
    use crate::exec::Dss;

    let dir = std::env::temp_dir().join(format!(
        "dss_ls_bin_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let mult = [0.4f32, 0.55, 0.75, 0.95, 1.0, 0.9, 0.7, 0.5];
    let sng8 = dir.join("ls8.sng");
    let mut sng_bytes = Vec::new();
    for v in mult {
        sng_bytes.extend_from_slice(&v.to_le_bytes());
    }
    std::fs::write(&sng8, &sng_bytes).unwrap();

    let dbl8 = dir.join("ls8.dbl");
    let mut dbl_bytes = Vec::new();
    for v in mult {
        dbl_bytes.extend_from_slice(&(v as f64).to_le_bytes());
    }
    std::fs::write(&dbl8, &dbl_bytes).unwrap();

    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.p");
    dss.command(&format!(
        "New LoadShape.s npts=8 interval=1 sngfile=\"{}\"",
        sng8.display()
    ));
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.command("? LoadShape.s.npts");
    assert_eq!(dss.result(), "8");
    dss.command("? LoadShape.s.mult");
    // Oracle: [ 0.400000005960464 0.550000011920929 0.75 0.949999988079071 1
    // 0.899999976158142 0.699999988079071 0.5] (f32->f64 widen, not the
    // literal decimal).
    assert_eq!(
        dss.result(),
        "[ 0.400000005960464 0.550000011920929 0.75 0.949999988079071 1 \
         0.899999976158142 0.699999988079071 0.5]"
    );

    dss.command(&format!(
        "New LoadShape.d npts=8 interval=1 dblfile=\"{}\"",
        dbl8.display()
    ));
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.command("? LoadShape.d.mult");
    assert_eq!(
        dss.result(),
        "[ 0.400000005960464 0.550000011920929 0.75 0.949999988079071 1 \
         0.899999976158142 0.699999988079071 0.5]"
    );

    // Interval=0 variant: (hour, mult) f32 pairs, hours 0..7 (Pascal's
    // "Interval = 0" branch of ReadSngFile).
    let sng8v = dir.join("ls8v.sng");
    let mut sngv_bytes = Vec::new();
    for (h, m) in mult.iter().enumerate() {
        sngv_bytes.extend_from_slice(&(h as f32).to_le_bytes());
        sngv_bytes.extend_from_slice(&m.to_le_bytes());
    }
    std::fs::write(&sng8v, &sngv_bytes).unwrap();
    dss.command(&format!(
        "New LoadShape.s0 npts=8 interval=0 sngfile=\"{}\"",
        sng8v.display()
    ));
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.command("? LoadShape.s0.hour");
    assert_eq!(dss.result(), "[ 0 1 2 3 4 5 6 7]");
    dss.command("? LoadShape.s0.mult");
    assert_eq!(
        dss.result(),
        "[ 0.400000005960464 0.550000011920929 0.75 0.949999988079071 1 \
         0.899999976158142 0.699999988079071 0.5]"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// A missing SngFile/DblFile is Pascal error 615/617 (recorded, edit
/// continues) — same generic executive open-error message as CSVFile.
#[test]
fn sng_and_dbl_file_missing_records_error() {
    use crate::exec::Dss;
    for prop in ["sngfile", "dblfile"] {
        let mut dss = Dss::new();
        dss.command("clear");
        dss.command("new circuit.p");
        dss.command(&format!(
            "New LoadShape.d npts=6 interval=1 {prop}=does_not_exist_42.bin"
        ));
        assert!(
            dss.errors()
                .iter()
                .any(|e| e.contains("Error opening file")),
            "{prop}: expected an open-error, got {:?}",
            dss.errors()
        );
    }
}

/// FPC probe battery for the single-precision storage paths (Pascal
/// `sP`/`sH`): every expected value below is the exact bit pattern produced
/// by an FPC 3.2.2 x86_64 probe replicating the Pascal expressions
/// (`tools/fpc/single_prec_probe.pas`, run 2026-07-07) — the same compiler/
/// RTL the pinned oracle's dss_capi backend is built with.
#[test]
#[allow(clippy::approx_constant)] // 3.14159… is the probe's Hr input, not a PI use
fn sng_single_storage_matches_fpc_bit_exact() {
    let sh: [f32; 4] = [0.1f64 as f32, 2.3f64 as f32, 4.7f64 as f32, 8.9f64 as f32];
    let sp: [f32; 4] = [
        (1.0f64 / 3.0) as f32,
        0.123456789f64 as f32,
        0.777777777f64 as f32,
        0.999999999f64 as f32,
    ];
    let mut pairs = Vec::new();
    for (h, p) in sh.iter().zip(sp.iter()) {
        pairs.extend_from_slice(&h.to_le_bytes());
        pairs.extend_from_slice(&p.to_le_bytes());
    }

    // Variable interval: single storage live, interpolation + curve stats.
    let (_, mut obj, errs) = edited(&[("npts", "4"), ("interval", "0")]);
    assert!(errs.is_empty(), "{errs:?}");
    obj.read_sng_file(&pairs);
    assert!(obj.s_p.is_some() && obj.s_h.is_some(), "float32 path taken");
    let m = obj.get_mult_at_hour(3.14159265358979);
    assert_eq!(
        m.re.to_bits(),
        0x3FD695F8134B71D8,
        "GetMultAtHourSingle mixed-precision interpolation (got {:016X})",
        m.re.to_bits()
    );
    assert_eq!(
        obj.mean().to_bits(),
        0x3FE355E8807CF518,
        "CurveMeanAndStdDevSingle mean"
    );
    assert_eq!(
        obj.std_dev().to_bits(),
        0x3FD60240860CE49D,
        "CurveMeanAndStdDevSingle stddev"
    );

    // Fixed interval: RCD single stats (S is an f32 accumulator; FPC's
    // Sqrt(Single) overload rounds the result to f32).
    let mut bare = Vec::new();
    for p in sp.iter() {
        bare.extend_from_slice(&p.to_le_bytes());
    }
    let (_, mut obj, errs) = edited(&[("npts", "4"), ("interval", "1")]);
    assert!(errs.is_empty(), "{errs:?}");
    obj.read_sng_file(&bare);
    assert!(obj.s_p.is_some() && obj.s_h.is_none());
    assert_eq!(
        obj.mean().to_bits(),
        0x3FE1E06526000000,
        "RCDMeanAndStdDevSingle mean"
    );
    assert_eq!(
        obj.std_dev().to_bits(),
        0x3FD9ADD3C0000000,
        "RCDMeanAndStdDevSingle stddev"
    );
}

/// Pascal `UseFloat64` call sites: a later `QMult=`/`Mult=`/`Hour=` edit (or
/// `MemoryMapping=yes`) ends single storage; the widened f64 view keeps the
/// f32-quantized values so property renders are unchanged.
#[test]
fn sng_single_storage_transitions_to_f64_on_edits() {
    let bare: Vec<u8> = [0.25f32, 0.5, 0.75]
        .iter()
        .flat_map(|p| p.to_le_bytes())
        .collect();

    let (cls, mut obj, _) = edited(&[("npts", "3"), ("interval", "1")]);
    obj.read_sng_file(&bare);
    assert!(obj.s_p.is_some());
    let before = get(&cls, &obj, "Mult");

    // QMult= runs UseFloat64 first (LoadShape.pas:804): singles dropped, the
    // P view unchanged.
    obj.set_f64_array(super::prop::QMULT, vec![1.0, 1.0, 1.0]);
    assert!(obj.s_p.is_none(), "QMult= must end single storage");
    assert_eq!(get(&cls, &obj, "Mult"), before);

    // A fresh SngFile read with QMult set takes the float64 path.
    obj.read_sng_file(&bare);
    assert!(obj.s_p.is_none(), "float64 path with QMult set");
    assert_eq!(get(&cls, &obj, "Mult"), before);
}

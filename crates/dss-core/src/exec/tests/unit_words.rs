//! A length unit is one exact documented word in every class (coordinator
//! ruling 2026-10-04). Each unit property takes the words its documentation
//! lists, written whole in any case, blanks around a word being no part of
//! it, and refuses any other text at its token, naming the property, the word
//! and the list: the property keeps what it had and nothing is read in
//! another unit.
//!
//! The lists come from the property help of each class. A position or height
//! (LineGeometry and LineSpacing `units`, Line `HeightUnit`) takes
//! [`POSITION`]. Line `units` and the `RUnits`, `GMRUnits` and `RadUnits` of
//! WireData, CNData and TSData, which name `none` as their default, take
//! [`LENGTH`]. LineCode `units` and `Show LineConstants`, which list `me` as
//! well, take [`CODE`]. `mm` is a unit word everywhere. The decks are under
//! `crates/dss-core/tests/data/unit_words/`, and anything a pin writes goes to
//! its own scratch directory.

use std::path::{Path, PathBuf};

use num_complex::Complex64;

use super::common::query;
use super::line_geometry_rules::{X_BAND, carson_x_of, xmatrix};
use crate::elements::pd::line::Line;
use crate::exec::*;

/// The words of a position or height unit.
const POSITION: &[&str] = &["mi", "kft", "km", "m", "ft", "in", "cm", "mm"];
/// The words of a length unit whose documented default is `none`.
const LENGTH: &[&str] = &["none", "mi", "kft", "km", "m", "ft", "in", "cm", "mm"];
/// The words of a LineCode unit and of the line-constants report.
const CODE: &[&str] = &["none", "mi", "kft", "km", "m", "me", "ft", "in", "cm", "mm"];

/// Every unit property, as `(class, object of classes.dss, property, words)`.
const UNIT_PROPS: [(&str, &str, &str, &[&str]); 14] = [
    ("Line", "l", "Units", LENGTH),
    ("Line", "l", "HeightUnit", POSITION),
    ("LineCode", "c", "Units", CODE),
    ("LineGeometry", "g", "Units", POSITION),
    ("LineSpacing", "s", "Units", POSITION),
    ("WireData", "w", "RUnits", LENGTH),
    ("WireData", "w", "GMRUnits", LENGTH),
    ("WireData", "w", "RadUnits", LENGTH),
    ("CNData", "cn", "RUnits", LENGTH),
    ("CNData", "cn", "GMRUnits", LENGTH),
    ("CNData", "cn", "RadUnits", LENGTH),
    ("TSData", "ts", "RUnits", LENGTH),
    ("TSData", "ts", "GMRUnits", LENGTH),
    ("TSData", "ts", "RadUnits", LENGTH),
];

/// Words that are no unit word of any list: longer spellings, prefixes and
/// words a reader of the first letters maps to some unit.
const NOT_WHOLE: &[&str] = &[
    "miles",
    "meter",
    "metre",
    "meters",
    "f",
    "i",
    "c",
    "kf",
    "kms",
    "min",
    "mils",
    "feet",
    "inches",
    "kilometer",
    "millimeter",
    "mmm",
];

// ----- decks, scratch directories, readers ---------------------------------

/// The tracked deck `name` of this folder.
fn deck(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/unit_words")
        .join(name)
}

/// `p` with forward slashes, as a DSS command quotes it.
fn slash(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// A per-test scratch directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let d = std::env::temp_dir().join(format!("dss_unitwords_{tag}_{}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        std::fs::create_dir_all(&d).unwrap_or_else(|e| panic!("mkdir {}: {e}", d.display()));
        Scratch(d)
    }

    /// A subdirectory, created.
    fn dir(&self, sub: &str) -> PathBuf {
        let d = self.0.join(sub);
        std::fs::create_dir_all(&d).unwrap_or_else(|e| panic!("mkdir {}: {e}", d.display()));
        d
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

/// A fresh engine with deck `name` compiled.
fn compile(name: &str) -> Dss {
    let mut dss = Dss::new();
    dss.command(&format!("compile \"{}\"", slash(&deck(name))));
    dss
}

/// The error log's texts, in order.
fn errors(dss: &Dss) -> Vec<String> {
    dss.errors().iter().map(|e| e.message.clone()).collect()
}

/// Run `cmd` and return the errors it logged.
fn run(dss: &mut Dss, cmd: &str) -> Vec<String> {
    let before = dss.errors().len();
    dss.command(cmd);
    errors(dss)[before..].to_vec()
}

/// The unit `word` names, as `?` prints it.
fn printed(word: &str) -> String {
    match word.to_ascii_lowercase().as_str() {
        "me" => "m".to_string(),
        w => w.to_string(),
    }
}

/// The refusal of `word` written to `prop` of `class.object`.
fn refused(class: &str, object: &str, prop: &str, word: &str, words: &[&str]) -> String {
    format!(
        "{class}.{object}.{prop}: \"{word}\" is not one of the length units {}.",
        words.join(", ")
    )
}

/// Every word of each unit property's list, in lower and upper case, reads
/// back as its unit, with no message. LineCode reads `me` as metres.
#[test]
fn every_class_reads_its_documented_words_whole() {
    let mut dss = compile("classes.dss");
    assert!(dss.errors().is_empty(), "{:?}", errors(&dss));
    for (class, object, prop, words) in UNIT_PROPS {
        for word in words {
            for written in [word.to_string(), word.to_ascii_uppercase()] {
                let ctx = format!("{class}.{object}.{prop}={written}");
                let errs = run(&mut dss, &format!("Edit {class}.{object} {prop}={written}"));
                assert!(errs.is_empty(), "{ctx}: {errs:?}");
                assert_eq!(
                    query(&mut dss, &format!("{class}.{object}.{prop}")),
                    printed(word),
                    "{ctx}"
                );
            }
        }
    }
}

/// A word outside a property's list is refused at its token, naming the
/// class, the object, the property, the word and the list, and the property
/// keeps the word it had. `none` is refused by the position units (their list
/// has no `none`, and `HeightUnit` keeps its default `m`), `me` by every
/// property but LineCode `units`. A JSON record carrying such a word is
/// refused the same way, and the wire keeps `none` for `miles` too, which a
/// reader of the first letters takes as `mi`. Save writes the word each
/// property kept.
#[test]
fn every_class_refuses_a_word_that_is_not_whole() {
    let scratch = Scratch::new("refuse");
    let mut dss = compile("classes.dss");
    assert!(dss.errors().is_empty(), "{:?}", errors(&dss));
    for (class, object, prop, words) in UNIT_PROPS {
        let edit = |word: &str| format!("Edit {class}.{object} {prop}={word}");
        let ask = format!("{class}.{object}.{prop}");
        assert!(run(&mut dss, &edit("km")).is_empty(), "{ask}=km");
        let mut outside: Vec<&str> = NOT_WHOLE.to_vec();
        outside.extend(["none", "me"].iter().filter(|w| !words.contains(w)));
        for word in outside {
            let ctx = format!("{ask}={word}");
            assert_eq!(
                run(&mut dss, &edit(word)),
                [refused(class, object, prop, word, words)],
                "{ctx}"
            );
            assert_eq!(query(&mut dss, &ask), "km", "{ctx}");
        }
    }

    // `HeightUnit` keeps its default `m` on a new Line.
    let errs = run(
        &mut dss,
        "New Line.h bus1=src.1.2.3 bus2=h.1.2.3 HeightUnit=none",
    );
    assert_eq!(errs, [refused("Line", "h", "HeightUnit", "none", POSITION)]);
    assert_eq!(query(&mut dss, "Line.h.HeightUnit"), "m");

    let dir = scratch.dir("save");
    dss.command(&format!("save circuit dir=\"{}\"", slash(&dir)));
    for (class, object, prop, _) in UNIT_PROPS {
        let file = dir.join(format!("{class}.dss"));
        let text = std::fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("read {}: {e}", file.display()))
            .to_ascii_lowercase();
        let head = format!("new \"{}.{object}\"", class.to_ascii_lowercase());
        let line = text
            .lines()
            .find(|l| l.starts_with(&head))
            .unwrap_or_else(|| panic!("no {head} line in {text}"));
        let key = format!(" {}=", prop.to_ascii_lowercase());
        let values: Vec<&str> = line
            .match_indices(&key)
            .map(|(at, _)| line[at + key.len()..].split(' ').next().unwrap_or(""))
            .collect();
        assert!(
            !values.is_empty(),
            "{class}.{object}.{prop} is not saved: {line}"
        );
        assert!(values.iter().all(|v| *v == "km"), "{line}");
    }

    for word in ["feet", "miles"] {
        let mut imported = Dss::new();
        let json =
            format!(r#"{{"Name":"uwjson","WireData":[{{"Name":"w","GMRUnits":"{word}"}}]}}"#);
        assert_eq!(imported.circuit_from_json(&json), Ok(()), "{word}");
        assert_eq!(
            errors(&imported),
            [refused("WireData", "w", "GMRUnits", word, LENGTH)],
            "{word}"
        );
        assert_eq!(
            query(&mut imported, "WireData.w.GMRUnits"),
            "none",
            "{word}"
        );
    }
}

/// The series admittance between the two terminals of phase 1 of Line `name`,
/// as its primitive Y holds it.
fn series_y11(dss: &Dss, name: &str) -> Complex64 {
    let ci = dss.class_by_name["line"];
    let oi = dss.classes[ci].name_to_idx[name];
    let l = dss.classes[ci].arena.get::<Line>(oi).expect("a Line");
    let y = l.cd.yprim.as_ref().expect("the primitive Y is built");
    y.get(0, 0)
}

/// A refused word leaves the physics as written without it. A Line of 1 km
/// on a LineCode in km, given `units=miles` (a reader of the first letters
/// takes miles, 1.609344 km), keeps km: its series impedance is the code's
/// 0.3 + j0.6 Ω/km over 1 km, 1/Y11 of the primitive Y (the code's sequence
/// values are equal, so the phases are uncoupled, and it has no shunt). A
/// wire whose GMR is 0.0244 ft, given `GMRunits=meter` (read as metres by a
/// reader of the first letters), keeps feet: a geometry on it reads
/// X11 = (ω μ0 / 2π) ln(De / GMR) at GMR = 0.0244 × 0.3048 m, and each mutual
/// term at its distance in feet ([`carson_x_of`]).
#[test]
fn a_refused_word_leaves_the_physics_as_written_without_it() {
    let mut line = compile("line_on_a_code_in_km.dss");
    assert_eq!(
        errors(&line),
        [refused("Line", "l", "Units", "miles", LENGTH)]
    );
    assert_eq!(query(&mut line, "Line.l.Units"), "km");
    let z = 1.0 / series_y11(&line, "l");
    let want = Complex64::new(0.3, 0.6);
    assert!(
        (z - want).norm() <= 1e-12 * want.norm(),
        "Z11 {z}, want {want}"
    );

    let wire = compile("wire_gmr_in_feet.dss");
    assert_eq!(
        errors(&wire),
        [refused("WireData", "w", "GMRUnits", "meter", LENGTH)]
    );
    let gmr = 0.0244 * 0.3048;
    let xs: [f64; 3] = [-4.0 * 0.3048, 0.0, 4.0 * 0.3048];
    let want = carson_x_of(3, |i, j| if i == j { gmr } else { (xs[i] - xs[j]).abs() });
    let x = xmatrix(&wire, "L");
    for i in 0..3 {
        for j in 0..3 {
            assert!(
                (x[i][j] - want[i][j]).abs() <= X_BAND,
                "X{}{} = {} Ω/mi, simple Carson gives {}",
                i + 1,
                j + 1,
                x[i][j],
                want[i][j]
            );
        }
    }
}

/// Blanks around a word are no part of it: each word of each unit property's
/// list, written with a blank before it, after it or both, reads back as its
/// unit with no message, in the text and the JSON import alike, so the unit
/// read is the unit stored. A reader of the first letters would store `"km "`
/// as kft and `"ft "`, `" ft"` or `"m "` as no unit. A word outside the list
/// with blanks around it is refused naming the text as written, blanks
/// included, and the property keeps its unit. On the deck the physics
/// follows the word: a Line of 1 km on a LineCode in `"km "` takes the code's
/// 0.3 + j0.6 Ω, a wire whose GMR is in `" ft"` keeps feet after
/// `Radunits=in`, and a Line on a spacing in `"m "` is built in metres, its
/// reactance the simple Carson value at GMR = 0.0244 × 0.3048 m and 1.2192 m
/// between neighbours ([`carson_x_of`]).
#[test]
fn a_word_with_blanks_around_it_reads_as_the_word() {
    let mut dss = compile("classes.dss");
    assert!(dss.errors().is_empty(), "{:?}", errors(&dss));
    for (class, object, prop, words) in UNIT_PROPS {
        for word in words {
            for written in [format!("{word} "), format!(" {word}"), format!(" {word} ")] {
                let ctx = format!("{class}.{object}.{prop}=\"{written}\"");
                let errs = run(
                    &mut dss,
                    &format!("Edit {class}.{object} {prop}=\"{written}\""),
                );
                assert!(errs.is_empty(), "{ctx}: {errs:?}");
                assert_eq!(
                    query(&mut dss, &format!("{class}.{object}.{prop}")),
                    printed(word),
                    "{ctx}"
                );
            }
        }
        let ask = format!("{class}.{object}.{prop}");
        let kept = printed(words.last().expect("a word list"));
        assert_eq!(
            run(
                &mut dss,
                &format!("Edit {class}.{object} {prop}=\" feet \"")
            ),
            [refused(class, object, prop, " feet ", words)],
            "{ask}"
        );
        assert_eq!(query(&mut dss, &ask), kept, "{ask}");
    }

    let mut imported = Dss::new();
    let json = r#"{"Name":"uwpad","LineCode":[{"Name":"c","NPhases":3,"Units":" km"}],
        "WireData":[{"Name":"w","GMRUnits":"ft ","RadUnits":"in"}],
        "LineSpacing":[{"Name":"s","NConds":3,"NPhases":3,"X":[-4,0,4],"H":[28,28,28],
        "Units":"m "}]}"#;
    assert_eq!(imported.circuit_from_json(json), Ok(()));
    assert!(imported.errors().is_empty(), "{:?}", errors(&imported));
    for (ask, want) in [
        ("LineCode.c.Units", "km"),
        ("WireData.w.GMRUnits", "ft"),
        ("LineSpacing.s.Units", "m"),
    ] {
        assert_eq!(query(&mut imported, ask), want, "{ask}");
    }

    let padded = compile("padded_words.dss");
    assert!(padded.errors().is_empty(), "{:?}", errors(&padded));
    let z = 1.0 / series_y11(&padded, "lc");
    let want = Complex64::new(0.3, 0.6);
    assert!(
        (z - want).norm() <= 1e-12 * want.norm(),
        "Z11 {z}, want {want}"
    );
    let gmr = 0.0244 * 0.3048;
    let xs: [f64; 3] = [-1.2192, 0.0, 1.2192];
    let want = carson_x_of(3, |i, j| if i == j { gmr } else { (xs[i] - xs[j]).abs() });
    let x = xmatrix(&padded, "L");
    for i in 0..3 {
        for j in 0..3 {
            assert!(
                (x[i][j] - want[i][j]).abs() <= X_BAND,
                "X{}{} = {} Ω/mi, simple Carson gives {}",
                i + 1,
                j + 1,
                x[i][j],
                want[i][j]
            );
        }
    }
}

/// The R, X and C matrices `LineConstantsCode.dss` prints, in order.
fn code_numbers(dir: &Path) -> Vec<f64> {
    let file = dir.join("LineConstantsCode.dss");
    let text =
        std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
    text.lines()
        .filter_map(|l| l.split_once("matrix=["))
        .flat_map(|(_, row)| {
            row.trim_end_matches(']')
                .split(['|', ' '])
                .filter(|v| !v.is_empty())
                .map(|v| v.parse::<f64>().unwrap_or_else(|e| panic!("{v}: {e}")))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The files of `dir` with their contents, by name.
fn files(dir: &Path) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("list {}: {e}", dir.display()))
        .map(|e| {
            let p = e.expect("a directory entry").path();
            let name = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let text = std::fs::read_to_string(&p).unwrap_or_default();
            (name, text)
        })
        .collect();
    out.sort();
    out
}

/// `Show LineConstants` reads its units argument as one word of [`CODE`]. A
/// word outside the list is one message, naming the word as written, blanks
/// included, and no report is written. Blanks around a listed word are no
/// part of it, so `"km "` and `" km"` write the report of `km`. `me` is
/// metres, the same report as `m`. In `mm` every R, X and C value is the
/// value per metre times 10⁻³, to the six digits printed.
#[test]
fn show_line_constants_reads_one_word() {
    let scratch = Scratch::new("show");
    let mut dss = compile("show_geometry.dss");
    assert!(dss.errors().is_empty(), "{:?}", errors(&dss));
    let mut report = |tag: &str, units: &str| -> (PathBuf, Vec<String>) {
        let dir = scratch.dir(tag);
        dss.command(&format!("Set DataPath=\"{}\"", slash(&dir)));
        let errs = run(&mut dss, &format!("Show LineConstants 60 {units} 100"));
        (dir, errs)
    };

    let (dir, errs) = report("feet", "feet");
    assert_eq!(
        errs,
        [format!(
            "Show LineConstants: \"feet\" is not one of the length units {}.",
            CODE.join(", ")
        )]
    );
    assert!(files(&dir).is_empty(), "{:?}", files(&dir));
    let (dir, errs) = report("feet_padded", "\" feet \"");
    assert_eq!(
        errs,
        [format!(
            "Show LineConstants: \" feet \" is not one of the length units {}.",
            CODE.join(", ")
        )]
    );
    assert!(files(&dir).is_empty(), "{:?}", files(&dir));

    let (km, errs) = report("km", "km");
    assert!(errs.is_empty(), "{errs:?}");
    assert!(!files(&km).is_empty());
    for (tag, padded) in [("km_after", "\"km \""), ("km_before", "\" km\"")] {
        let (dir, errs) = report(tag, padded);
        assert!(errs.is_empty(), "{padded}: {errs:?}");
        assert_eq!(files(&dir), files(&km), "{padded}");
    }

    let (m, errs) = report("m", "m");
    assert!(errs.is_empty(), "{errs:?}");
    let (me, errs) = report("me", "me");
    assert!(errs.is_empty(), "{errs:?}");
    assert!(!files(&m).is_empty());
    assert_eq!(files(&me), files(&m));

    let (mm, errs) = report("mm", "mm");
    assert!(errs.is_empty(), "{errs:?}");
    let per_m = code_numbers(&m);
    let per_mm = code_numbers(&mm);
    assert_eq!(per_m.len(), 18, "{per_m:?}");
    assert_eq!(per_mm.len(), per_m.len());
    for (k, (got, base)) in per_mm.iter().zip(&per_m).enumerate() {
        let want = base * 1e-3;
        let digit = 10f64.powf(want.abs().log10().floor() - 5.0);
        assert!(
            (got - want).abs() <= 0.5 * digit,
            "value {k}: {got} per mm, {base} per m"
        );
    }
}

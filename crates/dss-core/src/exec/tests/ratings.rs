//! A rating that is not set (user decision 2026-10-04): its spelling and parse,
//! the defaults that start not set, how a Line or a LineGeometry inherits one,
//! and what `Save`, the LineCode `Dump`, JSON, `like=`, MakePosSequence and
//! `Set %Normal` do with it. The loading reports are pinned in
//! `golden_reports.rs` (`export_capacity_prints_zero_for_a_rating_that_is_not_set`,
//! `export_overloads_ignores_a_rating_that_is_not_set`,
//! `show_currents_prints_zero_for_a_rating_that_is_not_set`,
//! `show_ratings_prints_none`).

use super::common::{query, query_f64};
use crate::exec::*;

/// A fresh engine with `lines` run, and no error.
fn run(lines: &[&str]) -> Dss {
    let mut dss = Dss::new();
    for l in lines {
        dss.command(l);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    dss
}

const WIRE: &str = "gmrac=0.0244 diam=0.721 rac=0.306 runits=mi radunits=in gmrunits=ft";

/// The deck of `golden_reports.rs` `unrated_lines_deck`, solved: six lines,
/// each feeding 9 MW, and a 1000 ohm fault on `b6`.
fn probe_lines() -> Vec<String> {
    let mut deck = vec![
        "clear".to_string(),
        "new circuit.c basekv=12.47 bus1=src pu=1.0".to_string(),
        format!("new wiredata.w1 {WIRE}"),
        format!("new wiredata.w2 {WIRE} normamps=530"),
        format!("new wiredata.w3 {WIRE} normamps=-1"),
        "new linespacing.sp nconds=3 nphases=3 units=ft x=[-4 0 4] h=[28 28 28]".to_string(),
        "new line.l1 bus1=src bus2=b1 spacing=sp wires=[w2 w1 w2] length=1 units=mi".to_string(),
        "new line.l2 bus1=src bus2=b2 normamps=-1 emergamps=-1 length=1 units=mi".to_string(),
        "new line.l3 bus1=src bus2=b3 normamps=0 emergamps=600 length=1 units=mi".to_string(),
        "new linegeometry.g nconds=3 nphases=3 cond=1 wire=w1 x=-4 h=28 units=ft cond=2 wire=w1 \
         x=0 h=28 units=ft cond=3 wire=w1 x=4 h=28 units=ft"
            .to_string(),
        "new line.l4 bus1=src bus2=b4 geometry=g length=1 units=mi".to_string(),
        "new line.l5 bus1=src bus2=b5 normamps=-1 emergamps=600 length=1 units=mi".to_string(),
        "new line.l6 bus1=src bus2=b6 spacing=sp wires=[w2 w2 w2] length=1 units=mi".to_string(),
        "new fault.f1 bus1=b6 phases=1 r=1000".to_string(),
    ];
    for i in 1..=6 {
        deck.push(format!("new load.ld{i} bus1=b{i} kw=9000 kv=12.47"));
    }
    deck.extend(["set voltagebases=[12.47]", "calcv", "solve"].map(String::from));
    deck
}

fn run_owned(lines: &[String]) -> Dss {
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    run(&refs)
}

/// The factors a Capacitor and a Reactor apply to their rated phase current to
/// derive a rating nobody typed.
const CAPACITOR_NORMAL: f64 = 1.35;
const CAPACITOR_EMERGENCY: f64 = 1.8;
const REACTOR_NORMAL: f64 = 1.0;
const REACTOR_EMERGENCY: f64 = 1.35;

/// Asserts that `what`, a rating of a three-phase wye Capacitor or Reactor at
/// 12.47 kV, reads the rating derived from `kvar`: the rated phase current,
/// `kvar / 3` over `12.47 / √3` kV, times `factor`, within 1e-12 relative.
fn assert_kvar_rating(dss: &mut Dss, what: &str, kvar: f64, factor: f64, why: &str) {
    let want = kvar / 3.0 / (12.47 / 3f64.sqrt()) * factor;
    let text = query(dss, what);
    let got: f64 = text
        .parse()
        .unwrap_or_else(|_| panic!("{what} {why}: reads {text}, want {want}"));
    assert!(
        ((got - want) / want).abs() < 1e-12,
        "{what} {why}: {got}, want {factor} times the rated phase current of {kvar} kvar, {want}"
    );
}

/// `none` (any case) and `-1`, typed or computed, read as a rating that is not
/// set and read back `none`, on every class with a current rating. Any other
/// number is that rating: `-2`, `0` and `530` read back as typed. A quoted
/// value may carry whitespace around `none` as it may around a number.
#[test]
fn none_and_minus_one_read_as_a_rating_that_is_not_set() {
    let objects = [
        "line.x bus1=a bus2=b",
        "linecode.x",
        "linegeometry.x",
        "wiredata.x",
        "cndata.x",
        "tsdata.x",
        "fault.x bus1=a",
        "gictransformer.x bush=a",
        "reactor.x bus1=a",
        "capacitor.x bus1=a",
    ];
    for obj in objects {
        let name = obj.split_whitespace().next().unwrap();
        for prop in ["normamps", "emergamps"] {
            let mut dss = run(&["new circuit.c", &format!("new {obj}")]);
            for (input, want) in [
                ("none", "none"),
                ("530", "530"),
                ("NONE", "none"),
                ("-2", "-2"),
                ("-1", "none"),
                ("0", "0"),
                ("-1.0", "none"),
                ("(1 2 -)", "none"),
                (r#"" 530 ""#, "530"),
                (r#"" none ""#, "none"),
            ] {
                dss.command(&format!("edit {name} {prop}={input}"));
                assert!(
                    dss.errors().is_empty(),
                    "{name}.{prop}={input}: {:?}",
                    dss.error_texts()
                );
                assert_eq!(
                    query(&mut dss, &format!("{name}.{prop}")),
                    want,
                    "{name}.{prop}={input}"
                );
            }
        }
    }
}

/// A rating is a number, `none` or `-1`, so a value that is not a finite
/// number is refused with a message that names the rating, and the property
/// keeps what it held: an inline `(0 0 /)` (NaN) or `(1 0 /)` (infinity), a
/// seasonal entry, a seasonal entry read from a file, and a `Set %Normal`
/// that would rate every Line at an infinite or NaN number.
#[test]
fn a_rating_that_is_not_a_finite_number_is_refused() {
    let objects = [
        "line.x bus1=a bus2=b",
        "linecode.x",
        "linegeometry.x",
        "wiredata.x",
        "fault.x bus1=a",
        "gictransformer.x bush=a",
        "reactor.x bus1=a",
        "capacitor.x bus1=a",
    ];
    for obj in objects {
        let name = obj.split_whitespace().next().unwrap();
        for prop in ["normamps", "emergamps"] {
            let mut dss = run(&["new circuit.c", &format!("new {obj} {prop}=530")]);
            for input in ["(0 0 /)", "(1 0 /)", "(-1 0 /)"] {
                dss.command(&format!("edit {name} {prop}={input}"));
                let errors = dss.error_texts().join(" | ").to_ascii_lowercase();
                // The message quotes the value as the parser hands it over, without its parentheses.
                let value = input.trim_start_matches('(').trim_end_matches(')');
                assert!(
                    errors.contains(&format!(
                        "{name}.{prop}: the rating {value} is not a finite number"
                    )),
                    "{name}.{prop}={input} is refused with a message naming the rating: {errors}"
                );
                dss.errors.clear();
                assert_eq!(
                    query(&mut dss, &format!("{name}.{prop}")),
                    "530",
                    "{name}.{prop}={input} leaves the rating as it was"
                );
            }
        }
    }

    let dir = std::env::temp_dir().join(format!("dss_rating_nan_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let file = dir.join("ratings.txt");
    std::fs::write(&file, "400\n(0 0 /)\n").expect("ratings file");
    let mut dss = run(&[
        "new circuit.c",
        "new line.l bus1=a bus2=b seasons=2 ratings=[400 500] normamps=530 emergamps=795",
        "set %normal=80",
    ]);
    dss.command("edit line.l ratings=[400 (1 0 /)]");
    let errors = dss.error_texts().join(" | ").to_ascii_lowercase();
    assert!(
        errors.contains("line.l.ratings: the rating") && errors.contains("not a finite number"),
        "an infinite seasonal entry is refused: {errors}"
    );
    dss.errors.clear();
    dss.command(&format!(
        "edit line.l ratings=(file=\"{}\")",
        file.to_string_lossy().replace('\\', "/")
    ));
    let errors = dss.error_texts().join(" | ").to_ascii_lowercase();
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        errors.contains("line.l.ratings: a rating in file"),
        "a NaN seasonal entry read from a file is refused: {errors}"
    );
    dss.errors.clear();
    assert_eq!(query(&mut dss, "line.l.ratings"), "[ 400 500]");
    assert_eq!(query(&mut dss, "line.l.seasons"), "2");

    for pct in ["(1 0 /)", "(0 0 /)"] {
        dss.command(&format!("set %normal={pct}"));
        let errors = dss.error_texts().join(" | ").to_ascii_lowercase();
        assert!(
            errors.contains("set %normal:") && errors.contains("not a finite percentage"),
            "set %normal={pct} is refused: {errors}"
        );
        dss.errors.clear();
        dss.command("get %normal");
        assert_eq!(dss.result(), "80", "set %normal={pct} keeps the factor");
        assert_eq!(
            query(&mut dss, "line.l.normamps"),
            "636",
            "set %normal={pct} leaves the ratings 80 % gave"
        );
    }
}

/// The ratings nothing sets start not set: a conductor's, a LineGeometry's
/// (and their one seasonal entry), a Fault's and a GICTransformer's. Both
/// gating oracles print a number for them, `-1` (`[ -1]`) for a conductor and
/// `0` (`[ 0]`) for the others (the props goldens). A Line and a LineCode keep
/// their default ratings.
#[test]
fn unset_ratings_read_none_by_default() {
    let mut dss = run(&[
        "new circuit.c",
        "new wiredata.w",
        "new cndata.cn",
        "new tsdata.ts",
        "new linegeometry.g",
        "new fault.f bus1=a",
        "new gictransformer.t bush=a",
        "new line.l bus1=a bus2=b",
        "new linecode.lc",
    ]);
    for obj in ["wiredata.w", "cndata.cn", "tsdata.ts", "linegeometry.g"] {
        assert_eq!(query(&mut dss, &format!("{obj}.normamps")), "none", "{obj}");
        assert_eq!(
            query(&mut dss, &format!("{obj}.emergamps")),
            "none",
            "{obj}"
        );
        assert_eq!(
            query(&mut dss, &format!("{obj}.ratings")),
            "[ none]",
            "{obj}"
        );
    }
    for obj in ["fault.f", "gictransformer.t"] {
        assert_eq!(query(&mut dss, &format!("{obj}.normamps")), "none", "{obj}");
        assert_eq!(
            query(&mut dss, &format!("{obj}.emergamps")),
            "none",
            "{obj}"
        );
    }
    for obj in ["line.l", "linecode.lc"] {
        assert_eq!(query(&mut dss, &format!("{obj}.normamps")), "400", "{obj}");
        assert_eq!(query(&mut dss, &format!("{obj}.emergamps")), "600", "{obj}");
        assert_eq!(
            query(&mut dss, &format!("{obj}.ratings")),
            "[ 400]",
            "{obj}"
        );
    }
}

/// A conductor's emergency rating follows a set normal rating (1.5 times it),
/// and a normal rating that is not set derives no emergency rating:
/// `normamps=-1` leaves the emergency rating not set, where both gating oracles
/// derive `-1.5` (1.5 times their `-1`). A typed negative or not-set emergency
/// rating is overwritten by a later normal rating, as on both gating oracles
/// (`530`/`795` for `emergamps=-2 normamps=530` and `emergamps=-1
/// normamps=530`). The documentation says the emergency rating "Defaults to
/// 1.5 * Normal Amps if not specified", and a typed rating is specified:
/// whether it may be overwritten is the open question Q7, and `w4` and `w5`
/// record its narrow path, not a rule.
#[test]
fn conductor_emergency_rating_follows_a_set_normal_rating_only() {
    let mut dss = run(&[
        "new circuit.c",
        "new wiredata.w1 normamps=-1",
        "new wiredata.w2 normamps=530",
        "new wiredata.w3 emergamps=600 normamps=none",
        "new wiredata.w4 emergamps=-2 normamps=530",
        "new wiredata.w5 emergamps=none normamps=530",
    ]);
    let q7 = "open question Q7: the documentation says the emergency rating \"Defaults to 1.5 \
              * Normal Amps if not specified\", a typed rating is specified, and this \
              overwrite is the narrow path until the user answers";
    for (w, norm, emerg, why) in [
        ("w1", "none", "none", ""),
        ("w2", "530", "795", ""),
        ("w3", "none", "600", ""),
        ("w4", "530", "795", q7),
        ("w5", "530", "795", q7),
    ] {
        assert_eq!(
            query(&mut dss, &format!("wiredata.{w}.normamps")),
            norm,
            "{w} {why}"
        );
        assert_eq!(
            query(&mut dss, &format!("wiredata.{w}.emergamps")),
            emerg,
            "{w} {why}"
        );
    }
}

/// A LineGeometry rating typed `none` or `-1` is kept against a later rated
/// conductor, and only a rating never typed, or typed `0`, takes the
/// conductor's. With `w2` rated `530`: `normamps=none` or `normamps=-1` before
/// `cond=1 wire=w2` reads `none`/`795`, `emergamps=-1` before the wire
/// `530`/`none`, `normamps=-1 wires=[w2]` `none`/`795`, `normamps=-1` after the
/// wire `none`/`795`, and a geometry typed `-1`/`-1` before its three wires
/// `none`/`none`, so its Line prints `0.00, 0.00` in `Export Capacity`. A
/// geometry that types no rating, or types `0`, reads `530`/`795`, and so does
/// the Line on its three-wire twin, whose capacity is taken against `530` and
/// `795`. Both gating oracles read the same ratings with `-1` for `none` and
/// divide the kept `-1` in `Export Capacity` (2026-10-04, the pinned
/// dss-python and the r4133 DLL through `epri-worker`). A conductor's typed
/// `-1` emergency rating is still derived from a later normal rating
/// (`wx`, `530`/`795` on all three engines). Whether a derivation or a fill may
/// overwrite a typed rating at all is the open question Q7 of the
/// `DIVERGENCES.md` row, and this test records its narrow path.
#[test]
fn a_geometry_rating_typed_not_set_is_kept_against_a_later_conductor() {
    let mut dss = run(&[
        "new circuit.c basekv=12.47 bus1=src pu=1.0",
        &format!("new wiredata.w2 {WIRE} normamps=530"),
        &format!("new wiredata.wx {WIRE} emergamps=-1 normamps=530"),
        "new linegeometry.ga nconds=1 nphases=1 normamps=none cond=1 wire=w2 x=0 h=28 units=ft",
        "new linegeometry.gm nconds=1 nphases=1 normamps=-1 cond=1 wire=w2 x=0 h=28 units=ft",
        "new linegeometry.gb nconds=1 nphases=1 emergamps=-1 cond=1 wire=w2 x=0 h=28 units=ft",
        "new linegeometry.gw nconds=1 nphases=1 normamps=-1 wires=[w2] cond=1 x=0 h=28 units=ft",
        "new linegeometry.gy nconds=1 nphases=1 cond=1 wire=w2 x=0 h=28 units=ft normamps=-1",
        "new linegeometry.gz nconds=1 nphases=1 normamps=0 cond=1 wire=w2 x=0 h=28 units=ft",
        "new linegeometry.gn nconds=1 nphases=1 cond=1 wire=w2 x=0 h=28 units=ft",
        "new linegeometry.g3 nconds=3 nphases=3 normamps=-1 emergamps=-1 cond=1 wire=w2 x=-4 \
         h=28 units=ft cond=2 wire=w2 x=0 h=28 units=ft cond=3 wire=w2 x=4 h=28 units=ft",
        "new linegeometry.g3n nconds=3 nphases=3 cond=1 wire=w2 x=-4 h=28 units=ft cond=2 \
         wire=w2 x=0 h=28 units=ft cond=3 wire=w2 x=4 h=28 units=ft",
        "new line.l bus1=src bus2=b geometry=g3 length=1 units=mi",
        "new line.n bus1=src bus2=n geometry=g3n length=1 units=mi",
        "new load.ld bus1=b kw=9000 kv=12.47",
        "new load.ln bus1=n kw=9000 kv=12.47",
        "set voltagebases=[12.47]",
        "calcv",
        "solve",
    ]);
    for (what, norm, emerg) in [
        ("linegeometry.ga", "none", "795"),
        ("linegeometry.gm", "none", "795"),
        ("linegeometry.gb", "530", "none"),
        ("linegeometry.gw", "none", "795"),
        ("linegeometry.gy", "none", "795"),
        ("linegeometry.g3", "none", "none"),
        ("line.l", "none", "none"),
        ("linegeometry.gz", "530", "795"),
        ("linegeometry.gn", "530", "795"),
        ("linegeometry.g3n", "530", "795"),
        ("line.n", "530", "795"),
        ("wiredata.wx", "530", "795"),
    ] {
        for (prop, want) in [("normamps", norm), ("emergamps", emerg)] {
            assert_eq!(
                query(&mut dss, &format!("{what}.{prop}")),
                want,
                "{what}.{prop}: the narrow path of open question Q7 (a typed rating is kept, \
                 one never typed or typed 0 is filled)"
            );
        }
    }
    let dir = std::env::temp_dir().join(format!("dss_rating_geokeep_{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dss.command(&format!(
        "set datapath=\"{}\"",
        dir.to_string_lossy().replace('\\', "/")
    ));
    dss.command("export capacity");
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    let text = std::fs::read_to_string(dss.last_result_file()).expect("capacity");
    std::fs::remove_dir_all(&dir).ok();
    let row = |name: &str| -> Vec<String> {
        text.lines()
            .find(|l| l.starts_with(name))
            .unwrap_or_else(|| panic!("no {name} in {text}"))
            .split(',')
            .map(|s| s.trim().to_string())
            .collect()
    };
    assert_eq!(
        row("Line.L")[2..4],
        ["0.00", "0.00"],
        "Line.L: a geometry rating typed not set gives no loading (open question Q7)"
    );
    let n = row("Line.N");
    let imax: f64 = n[1].parse().expect("Imax");
    assert_eq!(n[2], format!("{:.2}", imax / 530.0 * 100.0), "Line.N");
    assert_eq!(n[3], format!("{:.2}", imax / 795.0 * 100.0), "Line.N");
}

/// A seasonal rating not supplied is not set: a short `ratings=` and a
/// `seasons=` that grows the array leave the new entries not set, where the
/// pinned oracle prints `0` for them. `seasons=2` alone grows every class's
/// array by a rating that is not set, where both gating oracles add a `0`
/// (`[ -1 0]` for WireData, CNData and TSData, `[ 400 0]` for a LineCode,
/// `[ 0 0]` for a LineGeometry), and a Line on that LineCode inherits the
/// entry not set (`lx` `[ 400 none]`, where both oracles read `[ 400 0]`).
/// `none` and `-1` entries read as not set, a typed `0` stays `0`, and a `-1`
/// read from a file is not set too: a Line of `seasons=3` whose
/// `ratings=(file=...)` holds `400`, `-1` and `600` reads `[ 400 none 600]`,
/// where both oracles read `[ 400 -1 600]`. The oracle readings were measured
/// 2026-10-05 on the pinned dss-python and on the r4133 DLL through
/// `epri-worker`.
#[test]
fn seasonal_ratings_not_supplied_are_not_set() {
    let dir = std::env::temp_dir().join(format!("dss_rating_file_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let file = dir.join("ratings.txt");
    std::fs::write(&file, "400\n-1\n600\n").expect("ratings file");
    let from_file = format!(
        "new line.f bus1=a bus2=b seasons=3 ratings=(file=\"{}\")",
        file.to_string_lossy().replace('\\', "/")
    );
    let mut dss = run(&[
        "new circuit.c",
        "new wiredata.w seasons=3 ratings=[600]",
        "new line.l bus1=a bus2=b seasons=2",
        "new line.m bus1=a bus2=b seasons=3 ratings=[400 none -1]",
        "new line.n bus1=a bus2=b seasons=2 ratings=[400 0]",
        "new wiredata.w2 seasons=2",
        "new cndata.c2 seasons=2",
        "new tsdata.t2 seasons=2",
        "new linecode.lc2 seasons=2",
        "new line.lx bus1=a bus2=b linecode=lc2",
        "new linegeometry.g2 seasons=2",
        from_file.as_str(),
    ]);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(query(&mut dss, "wiredata.w.ratings"), "[ 600 none none]");
    assert_eq!(query(&mut dss, "line.l.ratings"), "[ 400 none]");
    assert_eq!(query(&mut dss, "line.m.ratings"), "[ 400 none none]");
    assert_eq!(query(&mut dss, "line.n.ratings"), "[ 400 0]");
    // Collected, so a failure names every class whose fill is wrong.
    let mut wrong = Vec::new();
    for (what, want) in [
        ("wiredata.w2.ratings", "[ none none]"),
        ("cndata.c2.ratings", "[ none none]"),
        ("tsdata.t2.ratings", "[ none none]"),
        ("linecode.lc2.ratings", "[ 400 none]"),
        ("line.lx.ratings", "[ 400 none]"),
        ("linegeometry.g2.ratings", "[ none none]"),
    ] {
        let got = query(&mut dss, what);
        if got != want {
            wrong.push(format!("{what} after seasons=2 reads {got}, not {want}"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
    assert_eq!(
        query(&mut dss, "line.f.ratings"),
        "[ 400 none 600]",
        "a -1 read from a file is not set"
    );
    assert_eq!(query(&mut dss, "line.f.seasons"), "3");
}

/// The LineCode `Dump` writes its `Ratings` list itself and prints a seasonal
/// rating that is not set as `none`: `seasons=3 ratings=[400 -1]` dumps
/// `~ Ratings=[400,none,none,]`, where the pinned dss-python dumps
/// `[400,-1,0,]` (2026-10-04, r4133 stops every LineCode `Dump` at that line
/// with error 303).
#[test]
fn line_code_dump_prints_none_for_a_seasonal_rating_that_is_not_set() {
    let mut dss = run(&[
        "new circuit.c",
        "new linecode.lc seasons=3 ratings=[400 -1]",
    ]);
    let dir = std::env::temp_dir().join(format!("dss_rating_lcdump_{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dss.command(&format!(
        "set datapath=\"{}\"",
        dir.to_string_lossy().replace('\\', "/")
    ));
    dss.command("dump linecode.lc");
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    let text = std::fs::read_to_string(dss.last_result_file()).expect("dump file");
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        text.lines().any(|l| l == "~ Ratings=[400,none,none,]"),
        "{text}"
    );
    assert_eq!(query(&mut dss, "linecode.lc.ratings"), "[ 400 none none]");
}

/// A Line on a spacing takes the ratings of its weakest phase wire, and a
/// phase wire without a rating leaves the Line's not set, in any order: `l1`
/// (`[w2 w1 w2]`, `w1` unrated) and its twin `[w1 w2 w2]` read `none`/`none`,
/// `l6` on rated wires `530`/`795`. r4133 reads `-1`/`-1` for `l1` (its
/// minimum takes the sentinel), the pinned capi 0.14.5 the first wire's
/// `530`/`795`.
#[test]
fn line_on_an_unrated_phase_wire_is_not_set() {
    let mut dss = run_owned(&probe_lines());
    dss.command("new line.l7 bus1=src bus2=b7 spacing=sp wires=[w1 w2 w2] length=1 units=mi");
    dss.command("new load.ld7 bus1=b7 kw=9000 kv=12.47");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    for (line, norm, emerg) in [
        ("l1", "none", "none"),
        ("l7", "none", "none"),
        ("l6", "530", "795"),
    ] {
        assert_eq!(
            query(&mut dss, &format!("line.{line}.normamps")),
            norm,
            "{line}"
        );
        assert_eq!(
            query(&mut dss, &format!("line.{line}.emergamps")),
            emerg,
            "{line}"
        );
    }
}

/// A Line on a geometry of unrated wires reads `none`/`none`/`[ none]`, where
/// both gating oracles read `0`/`0`/`[ 0]`.
#[test]
fn line_on_a_geometry_of_unrated_wires_is_not_set() {
    let mut dss = run_owned(&probe_lines());
    assert_eq!(query(&mut dss, "line.l4.normamps"), "none");
    assert_eq!(query(&mut dss, "line.l4.emergamps"), "none");
    assert_eq!(query(&mut dss, "line.l4.ratings"), "[ none]");
    assert_eq!(query(&mut dss, "linegeometry.g.normamps"), "none");
}

/// The weakest phase wire governs a spacing Line whatever the conductor order:
/// phase wires rated `530`, `0` and `400` in the orders `[a b c]`, `[a c b]`
/// and `[b a c]` all give `0`/`0`. r4133 takes `0` as
/// "no wire seen yet" and gives `400`/`600`, `0`/`0` and `400`/`600`, measured
/// on the r4133 DLL through `epri-worker` 2026-10-04. The pinned capi 0.14.5
/// takes the first wire: `530`, `530`, `0`.
#[test]
fn spacing_minimum_does_not_depend_on_conductor_order() {
    for order in ["a b c", "a c b", "b a c"] {
        let mut dss = run(&[
            "new circuit.c basekv=12.47 bus1=src",
            &format!("new wiredata.a {WIRE} normamps=530"),
            &format!("new wiredata.b {WIRE} normamps=0"),
            &format!("new wiredata.c {WIRE} normamps=400"),
            "new linespacing.sp nconds=3 nphases=3 units=ft x=[-4 0 4] h=[28 28 28]",
            &format!("new line.l1 bus1=src bus2=b1 spacing=sp wires=[{order}] length=1 units=mi"),
            "new load.ld1 bus1=b1 kw=9000 kv=12.47",
            "set voltagebases=[12.47]",
            "calcv",
            "solve",
        ]);
        assert_eq!(query(&mut dss, "line.l1.normamps"), "0", "[{order}]");
        assert_eq!(query(&mut dss, "line.l1.emergamps"), "0", "[{order}]");
    }
}

/// A spacing Line's normal and emergency ratings are each the lowest among
/// its phase wires', because the line current flows through every phase wire,
/// and a phase wire without one leaves that rating not set, in every order:
/// `wb` (`none`/`600`) with `wc` (`none`/`300`) reads `none`/`300`, `wd`
/// (`400`/`600`) with `we` (`400`/`500`) `400`/`500`, `wf` (`400`/`800`) with
/// `wg` (`530`/`795`) `400`/`795`, and `wd` with `wh` (`400`/`none`)
/// `400`/`none`. `Export Capacity` divides by those ratings. Both gating
/// oracles keep the emergency rating of the wire that governs the normal one
/// (r4133 the first of equal normal ratings, capi 0.14.5 the first wire):
/// `[wb wc wb]` `-1`/`600` but `[wc wb wb]` `-1`/`300`, `[wd we wd]` `400`/`600`
/// but `[we wd wd]` `400`/`500`, `[wf wg wg]` `400`/`800`, `[wh wd wd]`
/// `400`/`-1` but `[wd wh wd]` `400`/`600` (2026-10-04, the pinned dss-python
/// and the r4133 DLL through `epri-worker`).
#[test]
fn spacing_ratings_are_each_the_weakest_phase_wires_in_any_order() {
    let mut deck = vec![
        "new circuit.c basekv=12.47 bus1=src".to_string(),
        format!("new wiredata.wb {WIRE} emergamps=600 normamps=none"),
        format!("new wiredata.wc {WIRE} emergamps=300 normamps=none"),
        format!("new wiredata.wd {WIRE} normamps=400 emergamps=600"),
        format!("new wiredata.we {WIRE} normamps=400 emergamps=500"),
        format!("new wiredata.wf {WIRE} normamps=400 emergamps=800"),
        format!("new wiredata.wg {WIRE} normamps=530 emergamps=795"),
        format!("new wiredata.wh {WIRE} normamps=400 emergamps=none"),
        "new linespacing.sp nconds=3 nphases=3 units=ft x=[-4 0 4] h=[28 28 28]".to_string(),
    ];
    let sets = [
        ("o", ["wb wc wb", "wc wb wb", "wb wb wc"], None, Some(300.0)),
        (
            "t",
            ["wd we wd", "we wd wd", "wd wd we"],
            Some(400.0),
            Some(500.0),
        ),
        (
            "n",
            ["wf wg wg", "wg wf wg", "wg wg wf"],
            Some(400.0),
            Some(795.0),
        ),
        ("h", ["wd wh wd", "wh wd wd", "wd wd wh"], Some(400.0), None),
    ];
    let mut k = 0;
    for (set, orders, _, _) in &sets {
        for (i, wires) in orders.iter().enumerate() {
            k += 1;
            deck.push(format!(
                "new line.{set}{i} bus1=src bus2=b{k} spacing=sp wires=[{wires}] length=1 \
                 units=mi"
            ));
            deck.push(format!("new load.ld{k} bus1=b{k} kw=4000 kv=12.47"));
        }
    }
    deck.extend(["set voltagebases=[12.47]", "calcv", "solve"].map(String::from));
    let mut dss = run_owned(&deck);
    let dir = std::env::temp_dir().join(format!("dss_rating_spacing_{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dss.command(&format!(
        "set datapath=\"{}\"",
        dir.to_string_lossy().replace('\\', "/")
    ));
    dss.command("export capacity");
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    let text = std::fs::read_to_string(dss.last_result_file()).expect("capacity");
    std::fs::remove_dir_all(&dir).ok();
    let shown = |r: Option<f64>| r.map_or("none".to_string(), |v| format!("{v}"));
    let pct = |imax: f64, r: Option<f64>| {
        r.map_or("0.00".to_string(), |v| format!("{:.2}", imax / v * 100.0))
    };
    for (set, orders, norm, emerg) in &sets {
        for (i, wires) in orders.iter().enumerate() {
            let line = format!("{set}{i}");
            assert_eq!(
                query(&mut dss, &format!("line.{line}.normamps")),
                shown(*norm),
                "[{wires}]"
            );
            assert_eq!(
                query(&mut dss, &format!("line.{line}.emergamps")),
                shown(*emerg),
                "[{wires}]"
            );
            let name = format!("Line.{}", line.to_ascii_uppercase());
            let row: Vec<&str> = text
                .lines()
                .find(|l| l.starts_with(&format!("{name},")))
                .unwrap_or_else(|| panic!("no {name} in {text}"))
                .split(',')
                .map(str::trim)
                .collect();
            let imax: f64 = row[1].parse().expect("Imax");
            assert_eq!(
                row[2..4],
                [pct(imax, *norm), pct(imax, *emerg)],
                "[{wires}]"
            );
        }
    }
}

/// `Save` omits a rating that is not set, on the class overrides (`Line`,
/// `LineGeometry`) as on the generic walk (`WireData` `w3`, `wa` and `wb`,
/// `CNData` `cn1`, `TSData` `ts1`, `Fault` `fn`, `LineCode` `lcn`, `Capacitor`
/// `c1`, `Reactor` `r1`), omits a seasonal array none of whose entries is set
/// (`gx` and `lcn` typed `ratings=[none]`), and writes a partly set one with
/// its `none` entries, which the reload reads back in place (`l8`
/// `[ 400 none]`, `l10` `[ none 600]`). The reload compiles without an error
/// and reads `w1`, `w3`, `cn1`, `ts1`, the Fault, `gx` and the spacing Line
/// `l1` not set again. `w1`, `l4` and `l9` type no rating (`l4` and `l9` take
/// theirs from their geometry), so nothing of theirs is written with or
/// without the omission.
///
/// A Line typed `normamps=-1` reloads at the class defaults (`l2` at
/// `400`/`600`, `l5` at `400` beside its `600`), and so does a LineCode typed
/// `normamps=none ratings=[none]` (`lcn`, `400` and `[ 400]`). A LineGeometry
/// typed `normamps=none` after a rated `wire=` (`gy`, and `g3` after three)
/// or before it (`gt`, kept not set in memory) reloads with its wires'
/// `530`/`795`, and so does the Line on `g3` (`l9`, `none`/`795` before the
/// save). A conductor that saved only one rating reloads the other derived
/// from it (`wa` `400` from its `600`, `wb` `795` from its `530`), and a
/// Capacitor or a Reactor reloads the rating it saved without as derived from
/// its kvar (`c1` `NormAmps` and `r1` `EmergAmps` at 1.35 times the rated phase
/// current of 600 kvar at the class default 12.47 kV, 37.5023029706172 A). This
/// is the open question Q2 (should `Save` write `NormAmps=none` for a class
/// whose default is a value or is derived), recorded here as the present
/// behaviour, not argued for.
#[test]
fn save_omits_a_rating_that_is_not_set() {
    let mut lines = probe_lines();
    lines.extend([
        "new linegeometry.gx nconds=1 nphases=1 cond=1 wire=w1 x=0 h=28 units=ft normamps=none \
         emergamps=none ratings=[none]"
            .to_string(),
        "new linegeometry.gy nconds=1 nphases=1 cond=1 wire=w2 x=0 h=28 units=ft normamps=none \
         emergamps=none"
            .to_string(),
        "new linegeometry.gt nconds=1 nphases=1 normamps=none emergamps=none cond=1 wire=w2 x=0 \
         h=28 units=ft"
            .to_string(),
        "new linegeometry.g3 nconds=3 nphases=3 cond=1 wire=w2 x=-4 h=28 units=ft cond=2 wire=w2 \
         x=0 h=28 units=ft cond=3 wire=w2 x=4 h=28 units=ft normamps=none"
            .to_string(),
        "new line.l9 bus1=src bus2=b9 geometry=g3 length=1 units=mi".to_string(),
        format!("new wiredata.wa {WIRE} emergamps=600 normamps=none"),
        format!("new wiredata.wb {WIRE} normamps=530 emergamps=none"),
        "new line.l8 bus1=src bus2=b8 seasons=2 ratings=[400 none] length=1 units=mi".to_string(),
        "new line.l10 bus1=src bus2=b10 seasons=2 ratings=[none 600] length=1 units=mi".to_string(),
        "new fault.fn bus1=b5 phases=1 r=1000 normamps=none emergamps=-1".to_string(),
        "new linecode.lcn nphases=3 r1=0.1 x1=0.3 normamps=none ratings=[none]".to_string(),
        "new cndata.cn1 normamps=none".to_string(),
        "new tsdata.ts1 emergamps=-1".to_string(),
        "new capacitor.c1 bus1=src kvar=600 normamps=none".to_string(),
        "new reactor.r1 bus1=b5 kvar=600 emergamps=-1".to_string(),
    ]);
    let mut dss = run_owned(&lines);
    for (what, want) in [
        ("linegeometry.gt.normamps", "none"),
        ("linegeometry.gt.emergamps", "none"),
        ("linegeometry.g3.normamps", "none"),
        ("linegeometry.g3.emergamps", "795"),
        ("line.l9.normamps", "none"),
        ("line.l9.emergamps", "795"),
        ("line.l10.ratings", "[ none 600]"),
        ("wiredata.w3.normamps", "none"),
        ("wiredata.w3.emergamps", "none"),
        ("fault.fn.normamps", "none"),
        ("fault.fn.emergamps", "none"),
        ("linecode.lcn.normamps", "none"),
        ("linecode.lcn.ratings", "[ none]"),
        ("wiredata.wa.normamps", "none"),
        ("wiredata.wa.emergamps", "600"),
        ("wiredata.wb.normamps", "530"),
        ("wiredata.wb.emergamps", "none"),
        ("cndata.cn1.normamps", "none"),
        ("cndata.cn1.emergamps", "none"),
        ("tsdata.ts1.normamps", "none"),
        ("tsdata.ts1.emergamps", "none"),
        ("capacitor.c1.normamps", "none"),
        ("reactor.r1.emergamps", "none"),
    ] {
        assert_eq!(query(&mut dss, what), want, "{what} before the save");
    }
    // `c1` and `r1` derive the rating they did not type from their kvar.
    let derived = [
        ("capacitor.c1.emergamps", CAPACITOR_EMERGENCY),
        ("reactor.r1.normamps", REACTOR_NORMAL),
    ];
    for (what, factor) in derived {
        assert_kvar_rating(&mut dss, what, 600.0, factor, "before the save");
    }
    let root = std::env::temp_dir().join(format!("dss_rating_save_{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    let fwd = |p: &std::path::Path| p.to_string_lossy().replace('\\', "/");
    let one = root.join("one");
    std::fs::create_dir_all(&one).expect("scratch dir");
    dss.command(&format!(
        "save class=line file=Line.dss dir=\"{}\"",
        fwd(&one)
    ));
    dss.command(&format!(
        "save class=linegeometry file=LineGeometry.dss dir=\"{}\"",
        fwd(&one)
    ));
    dss.command(&format!("save circuit dir=\"{}\"", fwd(&root.join("ckt"))));
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());

    let line_of = |file: &std::path::Path, needle: &str| -> String {
        let text =
            std::fs::read_to_string(file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        text.lines()
            .find(|l| {
                l.to_ascii_lowercase()
                    .contains(&needle.to_ascii_lowercase())
            })
            .unwrap_or_else(|| panic!("no {needle} in {}:\n{text}", file.display()))
            .to_ascii_lowercase()
    };
    let rating_written = |saved: &str| {
        ["normamps=", "emergamps=", "ratings="]
            .iter()
            .any(|k| saved.contains(k))
    };
    for dir in [root.join("ckt"), one.clone()] {
        for l in ["l1", "l2"] {
            let saved = line_of(&dir.join("Line.dss"), &format!("\"line.{l}\""));
            assert!(
                !saved.contains("normamps=") && !saved.contains("emergamps="),
                "{l}: {saved}"
            );
        }
        let l4 = line_of(&dir.join("Line.dss"), "\"line.l4\"");
        assert!(
            !rating_written(&l4),
            "l4 takes its ratings from its geometry, so none is in its set chain: {l4}"
        );
        let l5 = line_of(&dir.join("Line.dss"), "\"line.l5\"");
        assert!(
            !l5.contains("normamps=") && l5.contains("emergamps=600"),
            "{l5}"
        );
        let l8 = line_of(&dir.join("Line.dss"), "\"line.l8\"");
        assert!(l8.contains("ratings=[ 400 none]"), "{l8}");
        let l10 = line_of(&dir.join("Line.dss"), "\"line.l10\"");
        assert!(l10.contains("ratings=[ none 600]"), "{l10}");
        let l9 = line_of(&dir.join("Line.dss"), "\"line.l9\"");
        assert!(
            !rating_written(&l9),
            "l9 takes its ratings from its geometry, so none is in its set chain: {l9}"
        );
        for g in ["gx", "gy", "gt", "g3"] {
            let saved = line_of(
                &dir.join("LineGeometry.dss"),
                &format!("\"linegeometry.{g}\""),
            );
            assert!(
                !saved.contains("normamps=")
                    && !saved.contains("emergamps=")
                    && !saved.contains("ratings="),
                "{g}: {saved}"
            );
        }
    }
    let w1 = line_of(&root.join("ckt").join("WireData.dss"), "\"wiredata.w1\"");
    assert!(
        !rating_written(&w1),
        "w1 types no rating, so none is in its set chain: {w1}"
    );
    // The generic walk: each of these typed `none` or `-1` for every rating it
    // set, and every one is left out.
    let written: Vec<String> = [
        ("WireData.dss", "wiredata.w3"),
        ("Fault.dss", "fault.fn"),
        ("LineCode.dss", "linecode.lcn"),
        ("CNData.dss", "cndata.cn1"),
        ("TSData.dss", "tsdata.ts1"),
        ("Capacitor.dss", "capacitor.c1"),
        ("Reactor.dss", "reactor.r1"),
    ]
    .into_iter()
    .map(|(file, obj)| line_of(&root.join("ckt").join(file), &format!("\"{obj}\"")))
    .filter(|saved| rating_written(saved))
    .collect();
    assert!(
        written.is_empty(),
        "a rating that is not set was written: {written:?}"
    );
    let wa = line_of(&root.join("ckt").join("WireData.dss"), "\"wiredata.wa\"");
    assert!(
        !wa.contains("normamps=") && wa.contains("emergamps=600"),
        "open question Q2: {wa}"
    );
    let wb = line_of(&root.join("ckt").join("WireData.dss"), "\"wiredata.wb\"");
    assert!(
        wb.contains("normamps=530") && !wb.contains("emergamps="),
        "open question Q2: {wb}"
    );

    let mut back = Dss::new();
    back.command(&format!(
        "compile \"{}\"",
        fwd(&root.join("ckt").join("Master.dss"))
    ));
    assert!(back.errors().is_empty(), "{:?}", back.error_texts());
    for (what, want) in [
        ("wiredata.w1.normamps", "none"),
        ("wiredata.w3.normamps", "none"),
        ("wiredata.w3.emergamps", "none"),
        ("fault.fn.normamps", "none"),
        ("fault.fn.emergamps", "none"),
        ("linegeometry.gx.normamps", "none"),
        ("linegeometry.gx.ratings", "[ none]"),
        ("line.l1.normamps", "none"),
        ("line.l1.emergamps", "none"),
        ("line.l4.normamps", "none"),
        ("line.l5.emergamps", "600"),
        ("line.l8.ratings", "[ 400 none]"),
        ("line.l10.ratings", "[ none 600]"),
        ("wiredata.wa.emergamps", "600"),
        ("wiredata.wb.normamps", "530"),
        ("cndata.cn1.normamps", "none"),
        ("cndata.cn1.emergamps", "none"),
        ("tsdata.ts1.normamps", "none"),
        ("tsdata.ts1.emergamps", "none"),
    ] {
        assert_eq!(query(&mut back, what), want, "{what} after the reload");
    }
    for (what, factor) in derived {
        assert_kvar_rating(&mut back, what, 600.0, factor, "after the reload");
    }
    let class_default = "reloads at the class default";
    let wire_fill = "reloads with its wires' rating";
    let partner = "reloads with the rating derived from its saved partner";
    for (what, want, how) in [
        ("line.l2.normamps", "400", class_default),
        ("line.l2.emergamps", "600", class_default),
        ("line.l5.normamps", "400", class_default),
        ("linecode.lcn.normamps", "400", class_default),
        ("linecode.lcn.ratings", "[ 400]", class_default),
        ("linegeometry.gy.normamps", "530", wire_fill),
        ("linegeometry.gy.emergamps", "795", wire_fill),
        ("linegeometry.gt.normamps", "530", wire_fill),
        ("linegeometry.gt.emergamps", "795", wire_fill),
        ("linegeometry.g3.normamps", "530", wire_fill),
        ("linegeometry.g3.emergamps", "795", wire_fill),
        ("line.l9.normamps", "530", wire_fill),
        ("line.l9.emergamps", "795", wire_fill),
        ("wiredata.wa.normamps", "400", partner),
        ("wiredata.wb.emergamps", "795", partner),
    ] {
        assert_eq!(
            query(&mut back, what),
            want,
            "open question Q2: {what} saved without its unset rating {how}"
        );
    }
    for (what, factor) in [
        ("capacitor.c1.normamps", CAPACITOR_NORMAL),
        ("reactor.r1.emergamps", REACTOR_EMERGENCY),
    ] {
        assert_kvar_rating(
            &mut back,
            what,
            600.0,
            factor,
            "(open question Q2) saved without its unset rating reloads with the rating derived \
             from its kvar",
        );
    }
    std::fs::remove_dir_all(&root).ok();
}

/// JSON writes a rating that is not set as `null` in the full sweep and omits
/// it from the set-order one (as `Save` does), a seasonal array none of whose
/// entries is set included (`ln`), and a load reads `null` and `-1` as not set.
#[test]
fn json_writes_a_rating_that_is_not_set_as_null_or_omits_it() {
    use crate::report::export::json::{Json, JsonOpts, parse_json};
    let member = |text: &str, key: &str| -> Option<Json> {
        match parse_json(text).expect("JSON") {
            Json::Obj(m) => m.into_iter().find(|(k, _)| k == key).map(|(_, v)| v),
            other => panic!("not an object: {other:?}"),
        }
    };
    let dss = run(&[
        "new circuit.c",
        "new wiredata.w emergamps=600 normamps=none",
        "new line.l bus1=a bus2=b normamps=none seasons=2 ratings=[400 none]",
        "new line.ln bus1=a bus2=c ratings=[none]",
    ]);
    let full = dss
        .obj_to_json("wiredata.w", JsonOpts::FULL)
        .expect("wiredata json");
    assert_eq!(member(&full, "NormAmps"), Some(Json::Null), "{full}");
    assert_eq!(
        member(&full, "EmergAmps"),
        Some(Json::Float(600.0)),
        "{full}"
    );
    assert_eq!(
        member(&full, "Ratings"),
        Some(Json::Arr(vec![Json::Null])),
        "{full}"
    );
    let set_only = dss
        .obj_to_json("wiredata.w", JsonOpts::NONE)
        .expect("wiredata json");
    assert_eq!(member(&set_only, "NormAmps"), None, "{set_only}");
    let line = dss
        .obj_to_json("line.l", JsonOpts::NONE)
        .expect("line json");
    assert_eq!(member(&line, "NormAmps"), None, "{line}");
    assert_eq!(
        member(&line, "Ratings"),
        Some(Json::Arr(vec![Json::Float(400.0), Json::Null])),
        "{line}"
    );
    let unrated = dss
        .obj_to_json("line.ln", JsonOpts::NONE)
        .expect("line json");
    assert_eq!(
        member(&unrated, "Ratings"),
        None,
        "a seasonal array none of whose entries is set is omitted from the set-order sweep: \
         {unrated}"
    );
    let unrated = dss
        .obj_to_json("line.ln", JsonOpts::FULL)
        .expect("line json");
    assert_eq!(
        member(&unrated, "Ratings"),
        Some(Json::Arr(vec![Json::Null])),
        "{unrated}"
    );

    let mut back = Dss::new();
    back.circuit_from_json(
        r#"{"Name": "c", "WireData": [{"Name": "wn", "NormAmps": null, "EmergAmps": 600}],
            "Line": [{"Name": "a", "Bus1": "x", "Bus2": "y",
            "NormAmps": null, "EmergAmps": -1, "Ratings": [null]},
            {"Name": "b", "Bus1": "x", "Bus2": "z", "NormAmps": 300},
            {"Name": "c", "Bus1": "x", "Bus2": "w", "Ratings": [null, 600]}]}"#,
    )
    .expect("JSON load");
    for (what, want) in [
        ("wiredata.wn.normamps", "400"),
        ("wiredata.wn.emergamps", "600"),
    ] {
        assert_eq!(
            query(&mut back, what),
            want,
            "open question Q2: a null rating loaded before its partner is derived from it"
        );
    }
    assert!(back.errors().is_empty(), "{:?}", back.error_texts());
    for (what, want) in [
        ("line.a.normamps", "none"),
        ("line.a.emergamps", "none"),
        ("line.a.ratings", "[ none]"),
        ("line.b.normamps", "300"),
        ("line.b.emergamps", "600"),
    ] {
        assert_eq!(query(&mut back, what), want, "{what} after the JSON load");
    }
    assert_eq!(
        query(&mut back, "line.c.ratings"),
        "[ none]",
        "a null entry keeps its season: the first season reads not set, and the load \
         reads as many entries as `Seasons` holds, which JSON never sets (one here)"
    );
}

/// MakePosSequence keeps a Line rating that is not set: it replays the Line's
/// ratings as they were, `none` included.
///
/// The replay also marks the ratings set, so a rating nobody typed survives
/// the conversion through `Save`: `s` takes `530`/`795` from its spacing's
/// wires, the conversion drops the spacing, and `Save` writes `NormAmps=530
/// EmergAmps=795`, which the reload reads back. Both gating oracles save and
/// reload the same two numbers (2026-10-05, this deck with `-1` for `none`, on
/// the pinned dss-python and on the r4133 DLL through `epri-worker`).
#[test]
fn make_pos_sequence_keeps_a_line_rating_that_is_not_set() {
    let mut dss = run(&[
        "new circuit.c basekv=12.47 bus1=src",
        &format!("new wiredata.w2 {WIRE} normamps=530"),
        "new linespacing.sp nconds=3 nphases=3 units=ft x=[-4 0 4] h=[28 28 28]",
        "new line.l bus1=src bus2=b phases=3 r1=0.1 x1=0.3 r0=0.3 x0=0.9 c1=0 c0=0 length=1 \
         normamps=none emergamps=none",
        "new line.m bus1=b bus2=c phases=3 r1=0.1 x1=0.3 r0=0.3 x0=0.9 c1=0 c0=0 length=1 \
         normamps=300 emergamps=none",
        "new line.s bus1=c bus2=d spacing=sp wires=[w2 w2 w2] length=1 units=mi",
        "new load.ld bus1=d kw=100 kv=12.47",
        "set voltagebases=[12.47]",
        "calcv",
        "makeposseq",
    ]);
    assert_eq!(query(&mut dss, "line.l.phases"), "1");
    assert_eq!(query(&mut dss, "line.l.normamps"), "none");
    assert_eq!(query(&mut dss, "line.l.emergamps"), "none");
    assert_eq!(query(&mut dss, "line.m.normamps"), "300");
    assert_eq!(query(&mut dss, "line.m.emergamps"), "none");
    assert_eq!(query(&mut dss, "line.s.phases"), "1");
    assert_eq!(query(&mut dss, "line.s.normamps"), "530");
    assert_eq!(query(&mut dss, "line.s.emergamps"), "795");

    let dir = std::env::temp_dir().join(format!("dss_rating_posseq_{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    let fwd = |p: &std::path::Path| p.to_string_lossy().replace('\\', "/");
    dss.command(&format!("save circuit dir=\"{}\"", fwd(&dir)));
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    let saved = std::fs::read_to_string(dir.join("Line.dss")).expect("the saved Line.dss");
    let s = saved
        .lines()
        .find(|l| l.to_ascii_lowercase().contains("\"line.s\""))
        .unwrap_or_else(|| panic!("no Line.s in:\n{saved}"));
    assert!(
        s.contains("NormAmps=530 EmergAmps=795"),
        "the replayed ratings are written: {s}"
    );
    let mut back = Dss::new();
    back.command(&format!("compile \"{}\"", fwd(&dir.join("Master.dss"))));
    assert!(back.errors().is_empty(), "{:?}", back.error_texts());
    for (what, want) in [("line.s.normamps", "530"), ("line.s.emergamps", "795")] {
        assert_eq!(query(&mut back, what), want, "{what} after the reload");
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// `like=` copies a rating that is not set, and a typed rating stays typed.
/// `fault.g`, typed `50`/`70` before `like=f`, reads `none`/`none`: the copy
/// replaces its typed ratings by the template's.
///
/// A Capacitor or a Reactor derives a rating nobody typed from its kvar: its
/// rated phase current times 1.35 (normal) and 1.8 (emergency) for a
/// Capacitor, times 1 and 1.35 for a Reactor. Its copy keeps a typed rating,
/// `none` included, under any kvar (`cb`, `cd`, `rb`, `rd`), and derives an
/// untyped one from its own kvar (`cb` emergency, `cg`, `rg`).
/// Both gating oracles derive every copy's rating from kvar (this deck with
/// `-1` for `none`: `cb` `37.5023029706172`/`50.0030706274896`, `cd`
/// `75.0046059412345`/`100.006141254979`, `rb`
/// `27.7794836819387`/`37.5023029706172`, `rd`
/// `55.5589673638774`/`75.0046059412345`), and their `Save` writes the derived
/// numbers of a copy as typed ratings (2026-10-04, the pinned dss-python and
/// the r4133 DLL through `epri-worker`). `cg` and `rg` read the same on all
/// three.
#[test]
fn like_copies_a_rating_that_is_not_set() {
    let mut dss = run(&[
        "new circuit.c",
        "new line.a bus1=x bus2=y normamps=none emergamps=500",
        "new line.b like=a",
        "new wiredata.w emergamps=600 normamps=none",
        "new wiredata.v like=w",
        "new fault.f bus1=x",
        "new fault.g bus1=x normamps=50 emergamps=70",
        "edit fault.g like=f",
        "new capacitor.ca bus1=x kvar=600 kv=12.47 normamps=none",
        "new capacitor.cb like=ca",
        "new capacitor.cc bus1=x kvar=600 kv=12.47 normamps=50 emergamps=70",
        "new capacitor.cd like=cc kvar=1200",
        "new capacitor.cf bus1=x kvar=600 kv=12.47",
        "new capacitor.cg like=cf kvar=1200",
        "new reactor.ra bus1=x kvar=600 kv=12.47 normamps=none emergamps=none",
        "new reactor.rb like=ra",
        "new reactor.rc bus1=x kvar=600 kv=12.47 normamps=50 emergamps=70",
        "new reactor.rd like=rc kvar=1200",
        "new reactor.rf bus1=x kvar=600 kv=12.47",
        "new reactor.rg like=rf kvar=1200",
    ]);
    for (what, want) in [
        ("line.b.normamps", "none"),
        ("line.b.emergamps", "500"),
        ("wiredata.v.normamps", "none"),
        ("wiredata.v.emergamps", "600"),
        ("fault.g.normamps", "none"),
        ("fault.g.emergamps", "none"),
        ("capacitor.cb.normamps", "none"),
        ("capacitor.cd.normamps", "50"),
        ("capacitor.cd.emergamps", "70"),
        ("reactor.rb.normamps", "none"),
        ("reactor.rb.emergamps", "none"),
        ("reactor.rd.normamps", "50"),
        ("reactor.rd.emergamps", "70"),
    ] {
        assert_eq!(query(&mut dss, what), want, "{what}");
    }
    for (what, kvar, factor) in [
        ("capacitor.cb.emergamps", 600.0, CAPACITOR_EMERGENCY),
        ("capacitor.cg.normamps", 1200.0, CAPACITOR_NORMAL),
        ("capacitor.cg.emergamps", 1200.0, CAPACITOR_EMERGENCY),
        ("reactor.rg.normamps", 1200.0, REACTOR_NORMAL),
        ("reactor.rg.emergamps", 1200.0, REACTOR_EMERGENCY),
    ] {
        assert_kvar_rating(&mut dss, what, kvar, factor, "derives from its own kvar");
    }
}

/// A rating derived as exactly `-1` is not set: `-1` is the input token of a
/// rating that is not set. The one exception is the current rating a
/// Transformer or an AutoTrans derives from its kVA rating, which has no
/// not-set state (`a_transformer_or_autotrans_derives_its_current_rating_whatever_is_typed`).
///
/// A one-phase Capacitor of `-1` kvar derives a normal rating of
/// `-1 / 1.35 · 1.35` at 1.35 kV and an emergency rating of `-1 / 1.8 · 1.8`
/// at 1.8 kV, a one-phase Reactor of `-1` kvar a normal rating of `-1 / 1` at
/// 1 kV and an emergency rating of `-1 / 1.35 · 1.35` at 1.35 kV. Each of those
/// reads `none` beside the number its partner derives, where both gating
/// oracles read `-1` (2026-10-09, the pinned dss-python and the r4133 DLL
/// through `epri-worker`).
///
/// `wiredata.wa` (`emergamps=-1.5`) derives a normal rating of `-1.5 / 1.5`,
/// so it and the spacing Line `s` on three of it read `none`/`-1.5`, and `s`
/// prints `0.00` in the normal column of `Export Capacity`. `Set %Normal=80`
/// rates `line.a` (`emergamps=-1.25`) at `0.8 · -1.25`, which reads `none`
/// and prints `0.00` there too, while `s` gets the number `-1.2`, which all
/// three engines divide by (`-17939.09`). Both gating oracles store the `-1`:
/// `wa` and `s` read `-1`, `line.a` reads `-1` on dss_capi 0.14.5 and its typed
/// `530` on r4133, and both divide by `-1` in `Export Capacity` (`s`
/// `-21524.71`, `line.a` `-21178.66`) (2026-10-04, the pinned dss-python and
/// the r4133 DLL through `epri-worker`). `Save` writes no normal rating for
/// `line.a`, so a reload reads the Line default `400`: the open question Q2.
#[test]
fn a_rating_derived_as_minus_one_is_not_set() {
    let mut kvar = run(&[
        "new circuit.k basekv=1.35 phases=1 bus1=src pu=1.0",
        "new capacitor.c1 bus1=src phases=1 kv=1.35 kvar=-1",
        "new capacitor.c2 bus1=src phases=1 kv=1.8 kvar=-1",
        "new reactor.r1 bus1=src phases=1 kv=1 kvar=-1",
        "new reactor.r2 bus1=src phases=1 kv=1.35 kvar=-1",
    ]);
    let mut numbers = Vec::new();
    for what in [
        "capacitor.c1.normamps",
        "capacitor.c2.emergamps",
        "reactor.r1.normamps",
        "reactor.r2.emergamps",
    ] {
        let got = query(&mut kvar, what);
        if got != "none" {
            numbers.push(format!("{what} = {got}"));
        }
    }
    assert!(
        numbers.is_empty(),
        "a rating derived as -1 from kvar reads none: {numbers:?}"
    );
    // The partner of each: -1 kvar over the phase kV, times its factor.
    for (what, kv, factor) in [
        ("capacitor.c1.emergamps", 1.35, 1.8),
        ("capacitor.c2.normamps", 1.8, 1.35),
        ("reactor.r1.emergamps", 1.0, 1.35),
        ("reactor.r2.normamps", 1.35, 1.0),
    ] {
        let want = -1.0 / kv * factor;
        let got = query_f64(&mut kvar, what);
        assert!(
            ((got - want) / want).abs() < 1e-12,
            "{what}: {got}, want {want}"
        );
    }

    let mut dss = run(&[
        "new circuit.c basekv=12.47 bus1=src pu=1.0",
        &format!("new wiredata.wa {WIRE} emergamps=-1.5"),
        "new linespacing.sp nconds=3 nphases=3 units=ft x=[-4 0 4] h=[28 28 28]",
        "new line.s bus1=src bus2=s spacing=sp wires=[wa wa wa] length=1 units=mi",
        "new line.a bus1=src bus2=a length=1 units=mi normamps=530 emergamps=-1.25",
        "new load.ls bus1=s kw=4000 kv=12.47",
        "new load.la bus1=a kw=4000 kv=12.47",
        "set voltagebases=[12.47]",
        "calcv",
        "solve",
    ]);
    for (what, want) in [
        ("wiredata.wa.normamps", "none"),
        ("wiredata.wa.emergamps", "-1.5"),
        ("line.s.normamps", "none"),
        ("line.s.emergamps", "-1.5"),
    ] {
        assert_eq!(query(&mut dss, what), want, "{what}");
    }
    let capacity = |dss: &mut Dss, tag: &str| -> String {
        let dir = std::env::temp_dir().join(format!("dss_rating_{tag}_{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dss.command(&format!(
            "set datapath=\"{}\"",
            dir.to_string_lossy().replace('\\', "/")
        ));
        dss.command("export capacity");
        assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
        let text = std::fs::read_to_string(dss.last_result_file()).expect("capacity");
        std::fs::remove_dir_all(&dir).ok();
        text
    };
    let normal_pct = |text: &str, name: &str| -> String {
        text.lines()
            .find(|l| l.starts_with(name))
            .unwrap_or_else(|| panic!("no {name} in {text}"))
            .split(',')
            .nth(2)
            .expect("%normal")
            .trim()
            .to_string()
    };
    let before = capacity(&mut dss, "minus_one_a");
    assert_eq!(normal_pct(&before, "Line.S"), "0.00", "{before}");

    dss.command("set %normal=80");
    assert_eq!(query(&mut dss, "line.a.normamps"), "none");
    assert_eq!(query(&mut dss, "line.s.normamps"), "-1.2");
    dss.command("solve");
    let after = capacity(&mut dss, "minus_one_b");
    assert_eq!(normal_pct(&after, "Line.A"), "0.00", "{after}");

    let saved =
        std::env::temp_dir().join(format!("dss_rating_minus_one_save_{}", std::process::id()));
    std::fs::remove_dir_all(&saved).ok();
    let fwd = |p: &std::path::Path| p.to_string_lossy().replace('\\', "/");
    dss.command(&format!("save circuit dir=\"{}\"", fwd(&saved)));
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    let line_file = std::fs::read_to_string(saved.join("Line.dss")).expect("saved lines");
    let saved_a = line_file
        .lines()
        .find(|s| s.to_ascii_lowercase().contains("\"line.a\""))
        .unwrap_or_else(|| panic!("no line.a in {line_file}"))
        .to_ascii_lowercase();
    let q2 = "open question Q2: Save omits a rating that is not set";
    assert!(!saved_a.contains("normamps="), "{q2}: {saved_a}");
    let mut back = Dss::new();
    back.command(&format!("compile \"{}\"", fwd(&saved.join("Master.dss"))));
    assert_eq!(query(&mut back, "line.a.normamps"), "400", "{q2}");
    assert_eq!(query(&mut back, "wiredata.wa.normamps"), "none");
    std::fs::remove_dir_all(&saved).ok();
}

/// A Transformer and an AutoTrans derive `NormAmps` and `EmergAmps` from their
/// kVA ratings at every recalculation, so a typed `none`, `-1` or number reads
/// back the derived rating: `1.1 · kVA` and `1.5 · kVA` over the phase count and
/// the winding 1 phase voltage, for an AutoTrans the series voltage. Both
/// gating oracles read the same after a typed `-1` (`25.4645267084438`,
/// `34.7243546024234`, `152.848446716868` and `208.429700068457` on dss_capi
/// 0.14.5, `25.465`, `34.724`, `152.85` and `208.43` on r4133) and refuse the
/// word `none` (2026-10-09, the pinned dss-python and the r4133 DLL through
/// `epri-worker`). A typed rating overwritten by the derived one is the
/// narrow path of the open question Q7.
///
/// The kVA ratings have no not-set state, so a current rating derived as
/// exactly `-1` A is a negative rating like any other: a one-phase Transformer
/// with a 1 kV winding 1 and an AutoTrans with a 1 kV series winding, typed
/// `normhkva=-1 emerghkva=-1`, read back `-1`, as their `-1.01` twins read
/// `-1.01` and as both gating oracles read them (same date and engines). Their
/// loading takes the magnitude
/// (`golden_reports.rs::a_transformer_rating_derived_as_minus_one_ampere_loads_by_its_magnitude`).
#[test]
fn a_transformer_or_autotrans_derives_its_current_rating_whatever_is_typed() {
    let mut dss = run(&[
        "new circuit.c basekv=12.47 bus1=src pu=1.0",
        "new transformer.t phases=3 windings=2 buses=[src b] conns=[wye wye] kvs=[12.47 0.48] \
         kvas=[500 500] xhl=6",
        "new autotrans.at phases=3 windings=2 buses=[src c] conns=[s w] kvs=[12.47 4.16] \
         kvas=[2000 2000] xhx=5",
    ]);
    let sqrt3 = 3f64.sqrt();
    let t_kv = 12.47 / sqrt3;
    let at_kv = (12.47 - 4.16) / sqrt3;
    let derived = [
        ("transformer.t", "normamps", 1.1 * 500.0 / 3.0 / t_kv),
        ("transformer.t", "emergamps", 1.5 * 500.0 / 3.0 / t_kv),
        ("autotrans.at", "normamps", 1.1 * 2000.0 / 3.0 / at_kv),
        ("autotrans.at", "emergamps", 1.5 * 2000.0 / 3.0 / at_kv),
    ];
    for typed in ["none", "-1", "530"] {
        for (obj, prop, want) in derived {
            dss.command(&format!("edit {obj} {prop}={typed}"));
            assert!(
                dss.errors().is_empty(),
                "{obj}.{prop}={typed}: {:?}",
                dss.error_texts()
            );
            let got = query_f64(&mut dss, &format!("{obj}.{prop}"));
            assert!(
                ((got - want) / want).abs() < 1e-12,
                "{obj}.{prop}={typed} reads the rating derived from kVA, {want}: {got} \
                 (open question Q7: a derivation overwrites a typed rating)"
            );
        }
    }

    let mut one_kv = run(&[
        "new circuit.c basekv=1 phases=1 bus1=src pu=1.0",
        "new transformer.t1 phases=1 windings=2 buses=[src b1] conns=[wye wye] kvs=[1 0.48] \
         kvas=[50 50] normhkva=-1 emerghkva=-1",
        "new transformer.t2 phases=1 windings=2 buses=[src b2] conns=[wye wye] kvs=[1 0.48] \
         kvas=[50 50] normhkva=-1.01 emerghkva=-1.01",
        "new autotrans.a1 phases=1 windings=2 buses=[src c1] conns=[s w] kvs=[2 1] kvas=[50 50] \
         normhkva=-1 emerghkva=-1",
        "new autotrans.a2 phases=1 windings=2 buses=[src c2] conns=[s w] kvs=[2 1] kvas=[50 50] \
         normhkva=-1.01 emerghkva=-1.01",
    ]);
    let mut wrong = Vec::new();
    for (obj, want) in [
        ("transformer.t1", "-1"),
        ("transformer.t2", "-1.01"),
        ("autotrans.a1", "-1"),
        ("autotrans.a2", "-1.01"),
    ] {
        for prop in ["normamps", "emergamps"] {
            let got = query(&mut one_kv, &format!("{obj}.{prop}"));
            if got != want {
                wrong.push(format!("{obj}.{prop} = {got}, want {want}"));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "a kVA rating over a 1 kV winding derives that many amperes, a number: {wrong:?}"
    );
}

/// `Set %Normal=pct` rates every Line's normal rating at `pct` percent of its
/// emergency rating and `Get %Normal` reads the factor back: `0` before, `80`
/// after, on both gating oracles. `l3` and `l5` (emergency `600`) read `480`
/// and `l6` (emergency `795`) `636`, the live value both oracles compute. `l1`
/// and `l2`, whose emergency rating is not set, stay not set, where both
/// oracles compute `-0.8` from the `-1` sentinel (2026-10-04, the pinned
/// dss-python and the r4133 DLL through `epri-worker`). `tx`, typed
/// `normamps=300` beside an emergency rating that is not set, becomes not set
/// too: 80 % of a rating that is not set is not set. Both oracles replace its
/// live `300` by `-0.8` and divide by it (`-59830.53` in `Export Capacity` at
/// 478.644 A on a deck of `tx`, typed `emergamps=-1`, and its load alone, where
/// r4133's `?` still reads `300`, measured the same way on 2026-10-05). The
/// Capacity pin then divides by the new ratings and prints `0.00, 0.00` for
/// `l1`, `l2` and `tx`.
///
/// What `?` and `Save` show afterwards is open. Q9a: `?` reads the live
/// rating, as capi 0.14.5 does, where r4133 reads the text typed before (`0`,
/// `-1` and `530` for `l3`, `l5` and `ta`). Q9b: `Save` writes the live rating
/// of a Line that typed one (`ta` writes `NormAmps=636` and reloads `636`) and
/// nothing for the others, so a Line at the defaults (`td`) reloads `400` and
/// the spacing Line `l6` reloads its wires' `530`. r4133 writes the typed
/// text, capi 0.14.5 the live value of a typed or spacing-rated Line (`636`
/// for `l6` too), and neither writes a rating for `td`.
#[test]
fn set_pct_normal_rates_every_line_from_its_emergency_rating() {
    let mut lines = probe_lines();
    lines.extend([
        "new line.ta bus1=src bus2=b9 normamps=530 emergamps=795 length=1 units=mi".to_string(),
        "new line.td bus1=src bus2=b10 length=1 units=mi".to_string(),
        "new line.tx bus1=src bus2=b11 normamps=300 emergamps=none length=1 units=mi".to_string(),
        "new load.ld11 bus1=b11 kw=9000 kv=12.47".to_string(),
    ]);
    let mut dss = run_owned(&lines);
    dss.command("get %normal");
    assert_eq!(dss.result(), "0");
    dss.command("set %normal=80");
    dss.command("get %normal");
    assert_eq!(dss.result(), "80");
    assert_eq!(
        query(&mut dss, "line.tx.normamps"),
        "none",
        "80 % of an emergency rating that is not set is not set and replaces the typed 300 \
         (open question Q9a: `?` reads the live rating)"
    );
    for (l, want) in [
        ("l1", "none"),
        ("l2", "none"),
        ("l3", "480"),
        ("l5", "480"),
        ("l6", "636"),
        ("ta", "636"),
        ("td", "480"),
    ] {
        assert_eq!(
            query(&mut dss, &format!("line.{l}.normamps")),
            want,
            "open question Q9a: `?` reads the live rating of {l} after Set %Normal"
        );
    }
    let saved = std::env::temp_dir().join(format!("dss_rating_pctsave_{}", std::process::id()));
    std::fs::remove_dir_all(&saved).ok();
    let fwd = |p: &std::path::Path| p.to_string_lossy().replace('\\', "/");
    dss.command(&format!("save circuit dir=\"{}\"", fwd(&saved)));
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    let line_file = std::fs::read_to_string(saved.join("Line.dss")).expect("saved lines");
    let saved_line = |l: &str| -> String {
        line_file
            .lines()
            .find(|s| s.to_ascii_lowercase().contains(&format!("\"line.{l}\"")))
            .unwrap_or_else(|| panic!("no line.{l} in {line_file}"))
            .to_ascii_lowercase()
    };
    let q9b = "open question Q9b: Save after Set %Normal";
    assert!(
        saved_line("ta").contains("normamps=636"),
        "{q9b}: {}",
        saved_line("ta")
    );
    for l in ["td", "l6"] {
        assert!(
            !saved_line(l).contains("normamps="),
            "{q9b}: {}",
            saved_line(l)
        );
    }
    let mut back = Dss::new();
    back.command(&format!("compile \"{}\"", fwd(&saved.join("Master.dss"))));
    for (l, want) in [("ta", "636"), ("td", "400"), ("l6", "530")] {
        assert_eq!(
            query(&mut back, &format!("line.{l}.normamps")),
            want,
            "{q9b}: line.{l} after the reload"
        );
    }
    std::fs::remove_dir_all(&saved).ok();
    dss.command("solve");
    let dir = std::env::temp_dir().join(format!("dss_rating_pctnormal_{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dss.command(&format!(
        "set datapath=\"{}\"",
        dir.to_string_lossy().replace('\\', "/")
    ));
    dss.command("export capacity");
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    let text = std::fs::read_to_string(dss.last_result_file()).expect("capacity");
    std::fs::remove_dir_all(&dir).ok();
    let row = |name: &str| -> Vec<String> {
        text.lines()
            .find(|l| l.starts_with(name))
            .unwrap_or_else(|| panic!("no {name} in {text}"))
            .split(',')
            .map(|s| s.trim().to_string())
            .collect()
    };
    for name in ["Line.L1", "Line.L2", "Line.TX"] {
        assert_eq!(row(name)[2..4], ["0.00", "0.00"], "{name}");
    }
    for (name, norm, emerg) in [("Line.L3", 480.0, 600.0), ("Line.L5", 480.0, 600.0)] {
        let r = row(name);
        let imax: f64 = r[1].parse().expect("Imax");
        assert_eq!(r[2], format!("{:.2}", imax / norm * 100.0), "{name}");
        assert_eq!(r[3], format!("{:.2}", imax / emerg * 100.0), "{name}");
    }
}

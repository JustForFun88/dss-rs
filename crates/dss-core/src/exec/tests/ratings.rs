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
        "new line.l1 bus1=src bus2=b1 spacing=sp wires=[w1 w1 w1] length=1 units=mi".to_string(),
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

/// A typed or computed `none`, in any case, and every typed value that is not
/// positive read back `none` on every class with a current rating, while a
/// positive number reads back as typed. A quoted value may carry blanks around
/// `none` as it may around a number.
#[test]
fn none_and_minus_one_read_as_a_rating_that_is_not_set() {
    let objects = [
        "line.x bus1=a bus2=b",
        "linecode.x",
        "linegeometry.x nconds=1",
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
                ("-2", "none"),
                ("-1", "none"),
                ("0", "none"),
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

/// A rating value that is not a finite number is refused with a message naming
/// the rating and the value as the parser hands it over, inline, as a seasonal
/// entry, as an entry of a ratings file and as the `Set %Normal` percentage,
/// and the property keeps what it held. The read-only Transformer and AutoTrans
/// current ratings refuse every value before it is read and keep the derived
/// rating.
#[test]
fn a_rating_that_is_not_a_finite_number_is_refused() {
    const NOT_FINITE: [&str; 3] = ["(1 0 /)", "(-1 0 /)", "(0 0 /)"];
    // The value the parser hands over: the input without its parentheses.
    let handed = |input: &str| input[1..input.len() - 1].to_string();
    let objects = [
        ("line.x bus1=a bus2=b", "Line"),
        ("linecode.x", "LineCode"),
        ("linegeometry.x nconds=1", "LineGeometry"),
        ("wiredata.x", "WireData"),
        ("cndata.x", "CNData"),
        ("tsdata.x", "TSData"),
        ("fault.x bus1=a", "Fault"),
        ("gictransformer.x bush=a", "GICTransformer"),
        ("reactor.x bus1=a", "Reactor"),
        ("capacitor.x bus1=a", "Capacitor"),
    ];
    for (obj, class) in objects {
        let name = obj.split_whitespace().next().unwrap();
        for prop in ["NormAmps", "EmergAmps"] {
            let mut dss = run(&["new circuit.c", &format!("new {obj} {prop}=530")]);
            for input in NOT_FINITE {
                dss.command(&format!("edit {name} {prop}={input}"));
                assert_eq!(
                    dss.error_texts(),
                    [format!(
                        "{class}.x.{prop}: the rating {} is not a finite number.",
                        handed(input)
                    )],
                    "{name}.{prop}={input}"
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

    for (obj, class, kva) in [
        ("transformer.x", "Transformer", "NormHkVA"),
        ("autotrans.x", "AutoTrans", "NormHkVA"),
    ] {
        let mut dss = run(&["new circuit.c", &format!("new {obj}")]);
        for (prop, kind, set_by) in [
            ("NormAmps", "normal", kva),
            ("EmergAmps", "emergency", "EmergHkVA"),
        ] {
            let derived = query(&mut dss, &format!("{obj}.{prop}"));
            assert_ne!(
                derived, "none",
                "{obj}.{prop} is derived from its kVA rating"
            );
            for input in std::iter::once("530").chain(NOT_FINITE) {
                dss.command(&format!("edit {obj} {prop}={input}"));
                assert_eq!(
                    dss.error_texts(),
                    [format!(
                        "{class}.x.{prop}: read only, the {kind} rating is set by {set_by}."
                    )],
                    "{obj}.{prop}={input}"
                );
                dss.errors.clear();
                assert_eq!(
                    query(&mut dss, &format!("{obj}.{prop}")),
                    derived,
                    "{obj}.{prop}={input} leaves the derived rating as it was"
                );
            }
        }
    }

    let dir = std::env::temp_dir().join(format!("dss_rating_nan_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let mut dss = run(&[
        "new circuit.c",
        "new line.l bus1=a bus2=b seasons=3 ratings=[400 500 600] normamps=530 emergamps=795",
        "set %normal=80",
    ]);
    for input in NOT_FINITE {
        dss.command(&format!("edit line.l ratings=[400 {input} 600]"));
        assert_eq!(
            dss.error_texts(),
            [format!(
                "Line.l.Ratings: the rating {} is not a finite number.",
                handed(input)
            )],
            "a seasonal entry {input} is refused, naming that entry"
        );
        dss.errors.clear();

        // Two entries against three seasons: an accepted file would leave two.
        let file = dir.join("ratings.txt");
        std::fs::write(&file, format!("400\n{input}\n")).expect("ratings file");
        let path = file.to_string_lossy().replace('\\', "/");
        dss.command(&format!("edit line.l ratings=(file=\"{path}\")"));
        assert_eq!(
            dss.error_texts(),
            [format!(
                "Line.l.Ratings: a rating in file \"{path}\" is not a finite number."
            )],
            "a seasonal entry {input} read from a file is refused"
        );
        dss.errors.clear();
        assert_eq!(query(&mut dss, "line.l.seasons"), "3", "{input}");
        assert_eq!(
            query(&mut dss, "line.l.ratings"),
            "[ 400 500 600]",
            "{input}"
        );
    }
    std::fs::remove_dir_all(&dir).ok();

    for (pct, read) in [("(1 0 /)", "inf"), ("(-1 0 /)", "-inf"), ("(0 0 /)", "NaN")] {
        dss.command(&format!("set %normal={pct}"));
        assert_eq!(
            dss.error_texts(),
            [format!(
                "Set %Normal: {read} is not a finite percentage of the emergency rating."
            )],
            "set %normal={pct} is refused"
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

/// A conductor's emergency rating nobody typed follows a set normal rating, and
/// a normal rating that is not set derives none. A typed emergency rating,
/// negative or not set, stays as typed.
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
    assert_ratings(
        &mut dss,
        &[
            ("wiredata.w1", "none", "none"),
            ("wiredata.w2", "530", "795"),
            ("wiredata.w3", "none", "600"),
            ("wiredata.w4", "530", "none"),
            ("wiredata.w5", "530", "none"),
        ],
        "a typed partner stays",
    );
}

/// A LineGeometry rating typed not set or zero is kept against a later rated
/// conductor, given by `wire=` or `wires=` before or after it, and only a
/// rating never typed takes the conductors' rating. A conductor's typed
/// emergency rating that is not set stays against a later normal rating.
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
        ("linegeometry.gz", "none", "795"),
        ("linegeometry.gn", "530", "795"),
        ("linegeometry.g3n", "530", "795"),
        ("line.n", "530", "795"),
        ("wiredata.wx", "530", "none"),
    ] {
        assert_ratings(
            &mut dss,
            &[(what, norm, emerg)],
            "(a typed rating is kept, one never typed is derived)",
        );
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
        "Line.L: a geometry rating typed not set gives no loading"
    );
    let n = row("Line.N");
    let imax: f64 = n[1].parse().expect("Imax");
    assert_eq!(n[2], format!("{:.2}", imax / 530.0 * 100.0), "Line.N");
    assert_eq!(n[3], format!("{:.2}", imax / 795.0 * 100.0), "Line.N");
}

/// A seasonal rating not supplied, by a short `ratings=` or a `seasons=` that
/// grows the array, is not set on every class with seasonal ratings, and a Line
/// inherits it from its LineCode. A typed `none`, a typed value that is not
/// positive and the not-set token read from a ratings file are not set too.
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
        "new linegeometry.g2 nconds=1 seasons=2",
        from_file.as_str(),
    ]);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(query(&mut dss, "wiredata.w.ratings"), "[ 600 none none]");
    assert_eq!(query(&mut dss, "line.l.ratings"), "[ 400 none]");
    assert_eq!(query(&mut dss, "line.m.ratings"), "[ 400 none none]");
    assert_eq!(query(&mut dss, "line.n.ratings"), "[ 400 none]");
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

/// A spacing Line takes the ratings of its weakest phase wire, so an unrated
/// phase wire leaves the Line's rating not set. Unrated wires alone log
/// nothing, while an unrated wire beside rated ones logs the partly rated error
/// for each rating, naming that wire.
#[test]
fn line_on_an_unrated_phase_wire_is_not_set() {
    let mut dss = run_owned(&probe_lines());
    dss.command("new line.l7 bus1=src bus2=b7 spacing=sp wires=[w1 w2 w2] length=1 units=mi");
    assert_eq!(
        dss.error_texts(),
        [
            "Line.l7: phase conductor 1 (WireData.w1) has no normal rating where another phase \
             conductor has one, so the Line's normal rating is not set.",
            "Line.l7: phase conductor 1 (WireData.w1) has no emergency rating where another \
             phase conductor has one, so the Line's emergency rating is not set.",
        ]
    );
    dss.command("new load.ld7 bus1=b7 kw=9000 kv=12.47");
    dss.command("solve");
    assert_eq!(dss.errors().len(), 2, "{:?}", dss.error_texts());
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

/// The weakest phase wire governs a spacing Line whatever the conductor order,
/// a typed zero being no rating. Every order reads both ratings not set and
/// logs the partly rated error naming the wire typed zero at its position.
#[test]
fn spacing_minimum_does_not_depend_on_conductor_order() {
    for (order, position) in [("a b c", 2), ("a c b", 3), ("b a c", 1)] {
        let mut dss = run(&[
            "new circuit.c basekv=12.47 bus1=src",
            &format!("new wiredata.a {WIRE} normamps=530"),
            &format!("new wiredata.b {WIRE} normamps=0"),
            &format!("new wiredata.c {WIRE} normamps=400"),
            "new linespacing.sp nconds=3 nphases=3 units=ft x=[-4 0 4] h=[28 28 28]",
        ]);
        for c in [
            &format!("new line.l1 bus1=src bus2=b1 spacing=sp wires=[{order}] length=1 units=mi"),
            "new load.ld1 bus1=b1 kw=9000 kv=12.47",
            "set voltagebases=[12.47]",
            "calcv",
            "solve",
        ] {
            dss.command(c);
        }
        assert_eq!(
            dss.error_texts(),
            ["normal", "emergency"].map(|kind| format!(
                "Line.l1: phase conductor {position} (WireData.b) has no {kind} rating where \
                 another phase conductor has one, so the Line's {kind} rating is not set."
            )),
            "[{order}]"
        );
        assert_eq!(query(&mut dss, "wiredata.b.normamps"), "none", "[{order}]");
        assert_eq!(query(&mut dss, "wiredata.b.emergamps"), "none", "[{order}]");
        assert_eq!(query(&mut dss, "line.l1.normamps"), "none", "[{order}]");
        assert_eq!(query(&mut dss, "line.l1.emergamps"), "none", "[{order}]");
    }
}

/// A spacing Line's normal and emergency ratings are each the lowest of its
/// phase wires', in every order, and a phase wire without one leaves that
/// rating not set. `Export Capacity` divides by those ratings, and only a Line
/// on a partly rated set logs the partly rated error.
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
    let mut dss = Dss::new();
    for l in &deck {
        dss.command(l);
    }
    assert_eq!(
        dss.error_texts(),
        [(0, 2), (1, 1), (2, 3)].map(|(i, position)| format!(
            "Line.h{i}: phase conductor {position} (WireData.wh) has no emergency rating where \
             another phase conductor has one, so the Line's emergency rating is not set."
        ))
    );
    let partly_rated = dss.errors().len();
    let dir = std::env::temp_dir().join(format!("dss_rating_spacing_{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dss.command(&format!(
        "set datapath=\"{}\"",
        dir.to_string_lossy().replace('\\', "/")
    ));
    dss.command("export capacity");
    assert_eq!(dss.errors().len(), partly_rated, "{:?}", dss.error_texts());
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

/// A concentric-neutral cable (250 kcmil AA with 13 #14 copper strands) for
/// the cable legs of the spacing pins.
const CN: &str = "k=13 DiaStrand=0.0641 GmrStrand=0.00208 Rstrand=14.8722 EpsR=2.3 \
                  InsLayer=0.22 DiaIns=1.06 DiaCable=1.29 Rac=0.41 GMRac=0.0171 diam=0.567 \
                  Runits=mi radunits=in gmrunits=ft";

/// A spacing Line whose phase conductors are partly rated, by `wires=`,
/// `conductors=`, `cncables=` or `tscables=`, logs an error naming the first
/// phase conductor without the rating, leaves that rating not set and still
/// builds and solves. Each rating is derived when the edit that gave the
/// conductors ends, unless it was typed after them in that edit or a later one.
#[test]
fn a_partly_rated_spacing_line_is_an_error_naming_the_conductor() {
    let mut dss = run(&[
        "new circuit.c basekv=12.47 bus1=src pu=1.0",
        &format!("new wiredata.wa {WIRE} normamps=530 emergamps=795"),
        &format!("new wiredata.wb {WIRE} normamps=400 emergamps=none"),
        &format!("new wiredata.wc {WIRE} emergamps=600 normamps=none"),
        &format!("new wiredata.wd {WIRE} normamps=400 emergamps=600"),
        &format!("new wiredata.wu {WIRE}"),
        &format!("new cndata.ca {CN} normamps=350 emergamps=500"),
        &format!("new cndata.cb {CN} normamps=300 emergamps=none"),
        &format!("new tsdata.ta {TS} normamps=350 emergamps=500"),
        &format!("new tsdata.tb {TS} normamps=300 emergamps=none"),
        "new linespacing.sp nconds=3 nphases=3 units=ft x=[-4 0 4] h=[28 28 28]",
        "new linespacing.spc nconds=3 nphases=3 units=ft x=[-0.5 0 0.5] h=[-4 -4 -4]",
    ]);
    for (what, want) in [
        ("wiredata.wb.emergamps", "none"),
        ("wiredata.wc.normamps", "none"),
        ("cndata.cb.emergamps", "none"),
        ("tsdata.tb.emergamps", "none"),
        ("wiredata.wu.normamps", "none"),
        ("wiredata.wu.emergamps", "none"),
    ] {
        assert_eq!(query(&mut dss, what), want, "{what}");
    }
    let edits: [(&str, &[&str]); 11] = [
        (
            "new line.lw bus1=src bus2=b1 spacing=sp wires=[wa wb wa] length=1 units=mi",
            &[
                "Line.lw: phase conductor 2 (WireData.wb) has no emergency rating where another \
               phase conductor has one, so the Line's emergency rating is not set.",
            ],
        ),
        (
            "new line.lc bus1=src bus2=b2 spacing=spc cncables=[ca cb ca] length=1 units=mi",
            &[
                "Line.lc: phase conductor 2 (CNData.cb) has no emergency rating where another \
               phase conductor has one, so the Line's emergency rating is not set.",
            ],
        ),
        (
            "new line.lk bus1=src bus2=b3 spacing=sp conductors=[wiredata.wa wiredata.wb \
             wiredata.wa] length=1 units=mi",
            &[
                "Line.lk: phase conductor 2 (WireData.wb) has no emergency rating where another \
               phase conductor has one, so the Line's emergency rating is not set.",
            ],
        ),
        (
            "new line.ln bus1=src bus2=b4 spacing=sp wires=[wa wb wa] normamps=500 length=1 \
             units=mi",
            &[
                "Line.ln: phase conductor 2 (WireData.wb) has no emergency rating where another \
               phase conductor has one, so the Line's emergency rating is not set.",
            ],
        ),
        (
            "new line.lo bus1=src bus2=b5 spacing=sp wires=[wa wd wa] normamps=500 length=1 \
             units=mi",
            &[],
        ),
        (
            "new line.ls bus1=src bus2=b6 spacing=sp wires=[wa wb wa] length=1 units=mi",
            &[
                "Line.ls: phase conductor 2 (WireData.wb) has no emergency rating where another \
               phase conductor has one, so the Line's emergency rating is not set.",
            ],
        ),
        (
            "new line.lm bus1=src bus2=b7 spacing=sp wires=[wa wc wa] normamps=500 length=1 \
             units=mi",
            &[],
        ),
        (
            "new line.lp bus1=src bus2=b8 spacing=sp wires=[wa wc wa] length=1 units=mi",
            &[
                "Line.lp: phase conductor 2 (WireData.wc) has no normal rating where another \
               phase conductor has one, so the Line's normal rating is not set.",
            ],
        ),
        (
            "new line.lr bus1=src bus2=b9 spacing=sp wires=[wa wd wa] seasons=2 \
             ratings=[400 500] length=1 units=mi",
            &[],
        ),
        (
            "new line.lu bus1=src bus2=b10 spacing=sp wires=[wu wa wu] length=1 units=mi",
            &[
                "Line.lu: phase conductor 1 (WireData.wu) has no normal rating where another \
               phase conductor has one, so the Line's normal rating is not set.",
                "Line.lu: phase conductor 1 (WireData.wu) has no emergency rating where another \
               phase conductor has one, so the Line's emergency rating is not set.",
            ],
        ),
        (
            "new line.lt bus1=src bus2=b11 spacing=spc tscables=[ta tb ta] length=1 units=mi",
            &[
                "Line.lt: phase conductor 2 (TSData.tb) has no emergency rating where another \
               phase conductor has one, so the Line's emergency rating is not set.",
            ],
        ),
    ];
    for (edit, messages) in edits {
        let before = dss.errors().len();
        dss.command(edit);
        assert_eq!(dss.error_texts()[before..], *messages, "{edit}");
        if edit.starts_with("new line.ls") || edit.starts_with("new line.lp") {
            dss.command("~ normamps=500");
            assert_eq!(
                dss.errors().len(),
                before + 1,
                "{edit}: the later rating logs nothing: {:?}",
                dss.error_texts()
            );
        }
    }
    let logged = dss.errors().len();
    let want = [
        ("lw", "400", "none"),
        ("lc", "300", "none"),
        ("lk", "400", "none"),
        ("ln", "500", "none"),
        ("lo", "500", "600"),
        ("ls", "500", "none"),
        ("lm", "500", "600"),
        ("lp", "500", "600"),
        ("lr", "400", "600"),
        ("lu", "none", "none"),
        ("lt", "300", "none"),
    ];
    let readings = |dss: &mut Dss| -> Vec<(String, String)> {
        want.iter()
            .map(|(l, _, _)| {
                (
                    query(dss, &format!("line.{l}.normamps")),
                    query(dss, &format!("line.{l}.emergamps")),
                )
            })
            .collect()
    };
    let expected: Vec<(String, String)> = want
        .iter()
        .map(|(_, n, e)| (n.to_string(), e.to_string()))
        .collect();
    assert_eq!(readings(&mut dss), expected, "before the solve");
    assert_eq!(query(&mut dss, "line.lr.seasons"), "2");
    assert_eq!(query(&mut dss, "line.lr.ratings"), "[ 400 500]");

    for i in 1..=11 {
        dss.command(&format!("new load.ld{i} bus1=b{i} kw=1000 kv=12.47"));
    }
    let dir = std::env::temp_dir().join(format!("dss_rating_partly_{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("scratch dir");
    for c in [
        "set voltagebases=[12.47]",
        "calcv",
        "solve",
        &format!("set datapath=\"{}\"", fwd(&dir)),
        "export capacity",
    ] {
        dss.command(c);
    }
    assert_eq!(
        dss.errors().len(),
        logged,
        "the solve runs: {:?}",
        dss.error_texts()
    );
    let text = std::fs::read_to_string(dss.last_result_file()).expect("capacity");
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(readings(&mut dss), expected, "after the solve");
    for (l, norm, emerg) in want {
        let row = capacity_row(&text, &format!("Line.{},", l.to_ascii_uppercase()));
        let imax: f64 = row[1].parse().expect("Imax");
        assert!(imax > 40.0, "{l} carries its load: {row:?}");
        let pct = |r: &str| {
            r.parse::<f64>()
                .map_or("0.00".to_string(), |v| format!("{:.2}", imax / v * 100.0))
        };
        assert_eq!(row[2..4], [pct(norm), pct(emerg)], "{l}");
    }
}

/// A spacing Line whose phase conductors are all unrated has neither rating set
/// and logs nothing, before and after the solve.
#[test]
fn an_unrated_line_is_not_set_without_an_error() {
    let mut dss = run(&[
        "new circuit.c basekv=12.47 bus1=src pu=1.0",
        &format!("new wiredata.wu {WIRE}"),
        "new linespacing.sp nconds=3 nphases=3 units=ft x=[-4 0 4] h=[28 28 28]",
        "new line.lu bus1=src bus2=b1 spacing=sp wires=[wu wu wu] length=1 units=mi",
    ]);
    assert_eq!(query(&mut dss, "line.lu.normamps"), "none");
    assert_eq!(query(&mut dss, "line.lu.emergamps"), "none");
    for c in [
        "new load.ld1 bus1=b1 kw=1000 kv=12.47",
        "set voltagebases=[12.47]",
        "calcv",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    assert_eq!(query(&mut dss, "line.lu.normamps"), "none");
    assert_eq!(query(&mut dss, "line.lu.emergamps"), "none");
}

/// A 750 kcmil tape-shielded cable for the TSData legs.
const TS: &str = "Diashield=0.034918 epsR=2.4 radunits=m tapelayer=0.0001 diains=0.03478 \
                  diacable=0.0394 inslayer=0.006395 Rdc=0.055155 runits=km radius=0.011";

/// `Export Capacity` written for `dss`, a deck that logged errors before,
/// into a scratch folder, as text. The export logs none.
fn capacity_text_after_errors(dss: &mut Dss, tag: &str) -> String {
    let before = dss.errors().len();
    let dir = std::env::temp_dir().join(format!("dss_rating_{tag}_{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dss.command(&format!("set datapath=\"{}\"", fwd(&dir)));
    dss.command("export capacity");
    assert_eq!(dss.errors().len(), before, "{:?}", dss.error_texts());
    let text = std::fs::read_to_string(dss.last_result_file()).expect("capacity");
    std::fs::remove_dir_all(&dir).ok();
    text
}

/// Asserts the normal and the emergency rating of each `(object, normal,
/// emergency)`, `why` naming the state of the deck.
fn assert_ratings(dss: &mut Dss, want: &[(&str, &str, &str)], why: &str) {
    for (what, norm, emerg) in want {
        assert_eq!(
            query(dss, &format!("{what}.normamps")),
            *norm,
            "{what}.normamps {why}"
        );
        assert_eq!(
            query(dss, &format!("{what}.emergamps")),
            *emerg,
            "{what}.emergamps {why}"
        );
    }
}

/// A typed conductor rating, a negative or not-set one included, is never
/// replaced by one derived from its partner, typed in one edit or over two, on
/// WireData, CNData and TSData alike.
#[test]
fn a_typed_conductor_rating_is_never_overwritten() {
    let mut dss = run(&[
        "new circuit.c",
        &format!("new wiredata.c1 {WIRE} emergamps=-2 normamps=530"),
        &format!("new wiredata.c2 {WIRE} emergamps=-1 normamps=530"),
        &format!("new wiredata.c3 {WIRE} emergamps=none normamps=530"),
        &format!("new wiredata.c4 {WIRE} normamps=-1 emergamps=795"),
        &format!("new wiredata.c5 {WIRE} emergamps=-2"),
        "edit wiredata.c5 normamps=530",
        &format!("new wiredata.c6 {WIRE} normamps=-1"),
        "edit wiredata.c6 emergamps=795",
        &format!("new cndata.k1 {CN} emergamps=-2 normamps=350"),
        &format!("new tsdata.t1 {TS} emergamps=-2 normamps=600"),
    ]);
    assert_ratings(
        &mut dss,
        &[
            ("wiredata.c1", "530", "none"),
            ("wiredata.c2", "530", "none"),
            ("wiredata.c3", "530", "none"),
            ("wiredata.c4", "none", "795"),
            ("wiredata.c5", "530", "none"),
            ("wiredata.c6", "none", "795"),
            ("cndata.k1", "350", "none"),
            ("tsdata.t1", "600", "none"),
        ],
        "keeps its typed partner",
    );
}

/// A conductor rating nobody typed follows its typed partner at the end of
/// every edit, by `Edit`, `~`, the property form and a `like=` copy, while a
/// typed partner stays. A typed rating that becomes not set leaves its partner
/// not set, on WireData, CNData and TSData alike.
#[test]
fn a_derived_conductor_rating_follows_the_last_typed_rating() {
    let mut dss = run(&[
        "new circuit.c",
        &format!("new wiredata.d {WIRE} normamps=530"),
    ]);
    let steps: Vec<(String, &str, &str, &str)> = vec![
        (
            "edit wiredata.d normamps=600".into(),
            "wiredata.d",
            "600",
            "900",
        ),
        ("~ normamps=700".into(), "wiredata.d", "700", "1050"),
        (
            "wiredata.d.normamps=800".into(),
            "wiredata.d",
            "800",
            "1200",
        ),
        (
            format!("new wiredata.e {WIRE} emergamps=600"),
            "wiredata.e",
            "400",
            "600",
        ),
        (
            "edit wiredata.e emergamps=900".into(),
            "wiredata.e",
            "600",
            "900",
        ),
        (
            format!("new wiredata.f {WIRE} normamps=530"),
            "wiredata.f",
            "530",
            "795",
        ),
        (
            "edit wiredata.f emergamps=1000".into(),
            "wiredata.f",
            "530",
            "1000",
        ),
        (
            "edit wiredata.f normamps=800".into(),
            "wiredata.f",
            "800",
            "1000",
        ),
        (
            format!("new cndata.k {CN} normamps=350"),
            "cndata.k",
            "350",
            "525",
        ),
        (
            "edit cndata.k normamps=400".into(),
            "cndata.k",
            "400",
            "600",
        ),
        (
            format!("new tsdata.t {TS} normamps=600"),
            "tsdata.t",
            "600",
            "900",
        ),
        (
            "edit tsdata.t normamps=700".into(),
            "tsdata.t",
            "700",
            "1050",
        ),
        ("new wiredata.g like=d".into(), "wiredata.g", "800", "1200"),
        (
            "edit wiredata.g normamps=100".into(),
            "wiredata.g",
            "100",
            "150",
        ),
        (
            "edit wiredata.d normamps=none".into(),
            "wiredata.d",
            "none",
            "none",
        ),
        (
            "edit wiredata.e emergamps=-1".into(),
            "wiredata.e",
            "none",
            "none",
        ),
        (
            "edit cndata.k normamps=none".into(),
            "cndata.k",
            "none",
            "none",
        ),
        (
            format!("new tsdata.u {TS} emergamps=900"),
            "tsdata.u",
            "600",
            "900",
        ),
        (
            "edit tsdata.u emergamps=-1".into(),
            "tsdata.u",
            "none",
            "none",
        ),
    ];
    for (edit, what, norm, emerg) in steps {
        dss.command(&edit);
        assert!(dss.errors().is_empty(), "{edit}: {:?}", dss.error_texts());
        assert_ratings(&mut dss, &[(what, norm, emerg)], &format!("after {edit}"));
    }
}

/// A rating typed on a LineGeometry, zero included, is never filled from its
/// conductors, and a typed `Seasons` or `Ratings` keeps both. Untyped seasonal
/// ratings follow the phase conductor at the lowest position with several
/// seasons, whatever the order of the edits, and the scalar ratings are the
/// weakest phase conductor's.
#[test]
fn a_typed_geometry_rating_is_never_filled() {
    let mut dss = run(&[
        "new circuit.c",
        &format!("new wiredata.w2 {WIRE} normamps=530"),
        &format!("new wiredata.w3 {WIRE} normamps=400 seasons=3 ratings=[400 500 600]"),
        &format!("new wiredata.w4 {WIRE} normamps=450 seasons=2 ratings=[450 550]"),
        "new linegeometry.gz nconds=1 nphases=1 normamps=0 cond=1 wire=w2 x=0 h=28 units=ft",
        "new linegeometry.ge nconds=1 nphases=1 emergamps=0 cond=1 wire=w2 x=0 h=28 units=ft",
        "new linegeometry.gr nconds=1 nphases=1 ratings=[500] cond=1 wire=w3 x=0 h=28 units=ft",
        "new linegeometry.gs nconds=1 nphases=1 seasons=1 cond=1 wire=w3 x=0 h=28 units=ft",
        "new linegeometry.gu nconds=1 nphases=1 cond=1 wire=w3 x=0 h=28 units=ft",
        "new linegeometry.gq nconds=2 nphases=1 cond=1 wire=w2 x=0 h=28 units=ft cond=2 wire=w3 \
         x=1 h=24 units=ft",
        "new linegeometry.gc nconds=2 nphases=1 conductors=[none, wiredata.w3]",
        "~ cond=1 wire=w2 x=0 h=28 units=ft cond=2 x=1 h=24 units=ft",
        "new linegeometry.go nconds=2 nphases=2 cond=1 wire=w4 x=0 h=28 units=ft cond=2 wire=w3 \
         x=1 h=28 units=ft",
        "new linegeometry.gt nconds=2 nphases=2 cond=2 wire=w3 x=1 h=28 units=ft cond=1 wire=w4 \
         x=0 h=28 units=ft",
        "new linegeometry.gp nconds=2 nphases=2 conductors=[none, wiredata.w3]",
        "~ cond=1 wire=w4 x=0 h=28 units=ft cond=2 x=1 h=28 units=ft",
        "new linegeometry.gx nconds=2 nphases=2 cond=1 wire=w3 x=0 h=28 units=ft cond=2 wire=w4 \
         x=1 h=28 units=ft",
        "~ cond=1 wire=w4",
        "new linegeometry.gm nconds=2 nphases=2 cond=1 wire=w2 x=0 h=28 units=ft cond=2 wire=w3 \
         x=1 h=28 units=ft",
        "new linegeometry.gn nconds=2 nphases=2 cond=2 wire=w3 x=1 h=28 units=ft cond=1 wire=w2 \
         x=0 h=28 units=ft",
    ]);
    for (g, norm, emerg, seasons, ratings) in [
        ("gz", "none", "795", "1", "[ none]"),
        ("ge", "530", "none", "1", "[ none]"),
        ("gr", "400", "600", "1", "[ 500]"),
        ("gs", "400", "600", "1", "[ none]"),
        ("gu", "400", "600", "3", "[ 400 500 600]"),
        ("gq", "530", "795", "1", "[ none]"),
        ("gc", "530", "795", "1", "[ none]"),
        ("go", "400", "600", "2", "[ 450 550]"),
        ("gt", "400", "600", "2", "[ 450 550]"),
        ("gp", "400", "600", "2", "[ 450 550]"),
        ("gx", "450", "675", "2", "[ 450 550]"),
        ("gm", "400", "600", "3", "[ 400 500 600]"),
        ("gn", "400", "600", "3", "[ 400 500 600]"),
    ] {
        let what = format!("linegeometry.{g}");
        assert_ratings(&mut dss, &[(what.as_str(), norm, emerg)], "");
        assert_eq!(
            query(&mut dss, &format!("{what}.seasons")),
            seasons,
            "{what}"
        );
        assert_eq!(
            query(&mut dss, &format!("{what}.ratings")),
            ratings,
            "{what}"
        );
    }
}

/// A LineGeometry rating nobody typed is derived when each edit ends from the
/// phase positions that hold a conductor, and an unrated phase conductor leaves
/// it not set while a Line on the geometry logs the partly rated error. A rated
/// neutral beside unrated phase conductors rates nothing and logs nothing, as
/// no reader compares a neutral current with the rating.
#[test]
fn a_geometry_rating_derives_over_its_phase_positions() {
    let geometry = |name: &str, wires: [&str; 3]| {
        format!(
            "new linegeometry.{name} nconds=3 nphases=3 cond=1 wire={} x=-4 h=28 units=ft cond=2 \
             wire={} x=0 h=28 units=ft cond=3 wire={} x=4 h=28 units=ft",
            wires[0], wires[1], wires[2]
        )
    };
    let mut dss = run(&[
        "new circuit.c basekv=12.47 bus1=src pu=1.0",
        &format!("new wiredata.wa {WIRE} normamps=530"),
        &format!("new wiredata.wb {WIRE} normamps=400"),
        &format!("new wiredata.wc {WIRE} normamps=300"),
        &format!("new wiredata.wu {WIRE}"),
        &format!("new wiredata.wn {WIRE} normamps=100"),
        &geometry("g1", ["wu", "wa", "wa"]),
        &geometry("g2", ["wa", "wa", "wu"]),
        "new linegeometry.g3 nconds=2 nphases=3 cond=1 wire=wa x=-4 h=28 units=ft cond=2 wire=wa \
         x=4 h=28 units=ft",
        "new linegeometry.g4 nconds=3 nphases=3 cond=1 wire=wa x=-4 h=28 units=ft cond=2 wire=wb \
         x=0 h=28 units=ft",
    ]);
    assert_ratings(
        &mut dss,
        &[("linegeometry.g4", "400", "600")],
        "after its first edit",
    );
    dss.command("~ cond=3 wire=wc x=4 h=28 units=ft");
    dss.command(
        "new linegeometry.g5 nconds=4 nphases=3 cond=1 wire=wu x=-4 h=28 units=ft cond=2 wire=wu \
         x=0 h=28 units=ft cond=3 wire=wu x=4 h=28 units=ft cond=4 wire=wn x=0 h=24 units=ft",
    );
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    let message = |line: &str, position: usize, geometry: &str, kind: &str| {
        format!(
            "Line.{line}: phase conductor {position} (WireData.wu) of LineGeometry.{geometry} has \
             no {kind} rating where another phase conductor has one, so the Line's {kind} rating \
             is not set."
        )
    };
    for (edit, messages) in [
        (
            "new line.l1 bus1=src bus2=b1 geometry=g1 length=1 units=mi",
            vec![
                message("l1", 1, "g1", "normal"),
                message("l1", 1, "g1", "emergency"),
            ],
        ),
        (
            "new line.l2 bus1=src bus2=b2 geometry=g2 length=1 units=mi",
            vec![
                message("l2", 3, "g2", "normal"),
                message("l2", 3, "g2", "emergency"),
            ],
        ),
        (
            "new line.l3 bus1=src bus2=b3 geometry=g3 length=1 units=mi",
            vec![],
        ),
        (
            "new line.l4 bus1=src bus2=b4 geometry=g4 length=1 units=mi",
            vec![],
        ),
        (
            "new line.l5 bus1=src bus2=b5 geometry=g5 length=1 units=mi",
            vec![],
        ),
    ] {
        let before = dss.errors().len();
        dss.command(edit);
        assert_eq!(dss.error_texts()[before..], messages, "{edit}");
    }
    let logged = dss.errors().len();
    let want = [
        ("linegeometry.g1", "none", "none"),
        ("linegeometry.g2", "none", "none"),
        ("linegeometry.g3", "530", "795"),
        ("linegeometry.g4", "300", "450"),
        ("linegeometry.g5", "none", "none"),
        ("line.l1", "none", "none"),
        ("line.l2", "none", "none"),
        ("line.l3", "530", "795"),
        ("line.l4", "300", "450"),
        ("line.l5", "none", "none"),
    ];
    assert_ratings(&mut dss, &want, "before the solve");
    for i in [1, 2, 4, 5] {
        dss.command(&format!("new load.ld{i} bus1=b{i} kw=1000 kv=12.47"));
    }
    for c in ["set voltagebases=[12.47]", "calcv", "solve"] {
        dss.command(c);
    }
    assert_eq!(
        dss.errors().len(),
        logged,
        "the solve runs: {:?}",
        dss.error_texts()
    );
    assert_ratings(&mut dss, &want, "after the solve");
    let text = capacity_text_after_errors(&mut dss, "phase_positions");
    for (line, norm, emerg) in [
        ("L1", None, None),
        ("L2", None, None),
        ("L4", Some(300.0), Some(450.0)),
        ("L5", None, None),
    ] {
        let row = capacity_row(&text, &format!("Line.{line},"));
        let imax: f64 = row[1].parse().expect("Imax");
        assert!(imax > 40.0, "{line} carries its load: {row:?}");
        let pct =
            |r: Option<f64>| r.map_or("0.00".to_string(), |v| format!("{:.2}", imax / v * 100.0));
        assert_eq!(row[2..4], [pct(norm), pct(emerg)], "{line}");
    }
}

/// A LineGeometry whose phase conductors carry different ratings is rated by
/// its weakest phase conductor, and so are a Line on it and a Line on the same
/// conductors through a LineSpacing, before and after the solve. `Export
/// Capacity` divides each Line's current by those ratings on the geometry of
/// the gated asymmetric decks.
#[test]
fn a_geometry_line_is_rated_by_its_weakest_phase_conductor() {
    let mut dss = run(&[
        "new circuit.c basekv=12.47 pu=1.0 angle=0 phases=3 bus1=src mvasc3=2000 mvasc1=2100",
        "set earthmodel=carson",
        "new wiredata.ACSR_336_4 normamps=530 diam=0.721 gmrac=0.29280 rdc=0.057954545 \
         runits=kft radunits=in gmrunits=in",
        "new wiredata.ACSR_4-0 normamps=340 diam=0.563 gmrac=0.09768 rdc=0.112121212 runits=kft \
         radunits=in gmrunits=in",
        "new wiredata.ACSR_1-0 normamps=230 diam=0.398 gmrac=0.05352 rdc=0.212121212 runits=kft \
         radunits=in gmrunits=in",
        "new wiredata.ACSR_2 normamps=180 diam=0.316 gmrac=0.05016 rdc=0.320075758 runits=kft \
         radunits=in gmrunits=in",
        "new linegeometry.oh3 nconds=4 nphases=3 reduce=y",
        "~ cond=1 wire=ACSR_336_4 x=-4.0 h=29.0 units=ft",
        "~ cond=2 wire=ACSR_4-0 x=-0.5 h=27.0 units=ft",
        "~ cond=3 wire=ACSR_1-0 x=3.5 h=30.0 units=ft",
        "~ cond=4 wire=ACSR_2 x=0.3 h=24.0 units=ft",
        "new linespacing.sp3 nconds=4 nphases=3 units=ft x=[-4 -0.5 3.5 0.3] h=[29 27 30 24]",
        "new line.lgeo bus1=src bus2=b1 geometry=oh3 length=6 units=kft",
        "new line.lsp bus1=src bus2=b2 spacing=sp3 wires=[ACSR_336_4 ACSR_4-0 ACSR_1-0 ACSR_2] \
         length=6 units=kft",
        "new load.la bus1=b1 kw=3000 pf=0.9 kv=12.47",
        "new load.lb bus1=b2 kw=3000 pf=0.9 kv=12.47",
        "set voltagebases=[12.47]",
        "calcv",
    ]);
    let want = [
        ("linegeometry.oh3", "230", "345"),
        ("line.lgeo", "230", "345"),
        ("line.lsp", "230", "345"),
    ];
    assert_ratings(&mut dss, &want, "before the solve");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    assert_ratings(&mut dss, &want, "after the solve");
    let text = capacity_text(&mut dss, "weakest_phase");
    for line in ["LGEO", "LSP"] {
        let row = capacity_row(&text, &format!("Line.{line},"));
        let imax: f64 = row[1].parse().expect("Imax");
        assert!(
            (imax - 160.334).abs() < 5e-4,
            "{line} carries the oracles' 160.334 A: {row:?}"
        );
        assert_eq!(
            row[2..4],
            [
                format!("{:.2}", imax / 230.0 * 100.0),
                format!("{:.2}", imax / 345.0 * 100.0)
            ],
            "{line}"
        );
    }
}

/// A geometry Line whose geometry derived a rating from partly rated phase
/// conductors logs an error naming that conductor and the geometry, leaves the
/// rating not set and still builds and solves, while the geometry itself logs
/// nothing. A rating typed on the Line after `geometry=`, or on the geometry,
/// stays as typed and is not checked.
#[test]
fn a_partly_rated_geometry_line_is_an_error_naming_the_conductor() {
    let geometry = |name: &str, extra: &str, wires: [&str; 3]| {
        format!(
            "new linegeometry.{name} nconds=3 nphases=3 {extra} cond=1 wire={} x=-4 h=28 units=ft \
             cond=2 wire={} x=0 h=28 units=ft cond=3 wire={} x=4 h=28 units=ft",
            wires[0], wires[1], wires[2]
        )
    };
    let xyz = "cond=1 x=-4 h=28 units=ft cond=2 x=0 h=28 units=ft cond=3 x=4 h=28 units=ft";
    let mut dss = run(&[
        "new circuit.c basekv=12.47 bus1=src pu=1.0",
        &format!("new wiredata.wa {WIRE} normamps=530 emergamps=795"),
        &format!("new wiredata.wb {WIRE} normamps=400 emergamps=none"),
        &format!("new wiredata.wc {WIRE} emergamps=600 normamps=none"),
        &format!("new wiredata.wd {WIRE} normamps=400 emergamps=600"),
        &format!("new wiredata.wu {WIRE}"),
        &format!("new cndata.ca {CN} normamps=350 emergamps=500"),
        &format!("new cndata.cb {CN} normamps=300 emergamps=none"),
        &geometry("gb", "", ["wa", "wb", "wa"]),
        &geometry("gc", "", ["wa", "wc", "wa"]),
        &geometry("gd", "", ["wa", "wd", "wa"]),
        &geometry("gt", "emergamps=700", ["wa", "wb", "wa"]),
        &geometry("gu", "", ["wu", "wa", "wu"]),
        &format!("new linegeometry.gw nconds=3 nphases=3 wires=[wa wb wa] {xyz}"),
        &format!(
            "new linegeometry.gk nconds=3 nphases=3 conductors=[wiredata.wa wiredata.wb \
             wiredata.wa] {xyz}"
        ),
        "new linegeometry.gn nconds=3 nphases=3 cncables=[ca cb ca] cond=1 x=-0.5 h=-4 units=ft \
         cond=2 x=0 h=-4 units=ft cond=3 x=0.5 h=-4 units=ft",
    ]);
    assert_ratings(
        &mut dss,
        &[
            ("linegeometry.gb", "400", "none"),
            ("linegeometry.gc", "none", "600"),
            ("linegeometry.gd", "400", "600"),
            ("linegeometry.gt", "400", "700"),
            ("linegeometry.gw", "400", "none"),
            ("linegeometry.gk", "400", "none"),
            ("linegeometry.gn", "300", "none"),
            ("linegeometry.gu", "none", "none"),
        ],
        "logs nothing",
    );
    let message = |line: &str, conductor: &str, geometry: &str, kind: &str| {
        format!(
            "Line.{line}: phase conductor 2 ({conductor}) of LineGeometry.{geometry} has no \
             {kind} rating where another phase conductor has one, so the Line's {kind} rating \
             is not set."
        )
    };
    let edits = [
        (
            "new line.lb bus1=src bus2=b1 geometry=gb length=1 units=mi",
            vec![message("lb", "WireData.wb", "gb", "emergency")],
        ),
        (
            "new line.ln bus1=src bus2=b2 geometry=gb normamps=500 length=1 units=mi",
            vec![message("ln", "WireData.wb", "gb", "emergency")],
        ),
        (
            "new line.lo bus1=src bus2=b3 geometry=gd normamps=500 length=1 units=mi",
            vec![],
        ),
        (
            "new line.ls bus1=src bus2=b4 geometry=gb length=1 units=mi",
            vec![message("ls", "WireData.wb", "gb", "emergency")],
        ),
        (
            "new line.lm bus1=src bus2=b5 geometry=gc normamps=500 length=1 units=mi",
            vec![],
        ),
        (
            "new line.lp bus1=src bus2=b6 geometry=gc length=1 units=mi",
            vec![message("lp", "WireData.wc", "gc", "normal")],
        ),
        (
            "new line.lt bus1=src bus2=b7 geometry=gt length=1 units=mi",
            vec![],
        ),
        (
            "new line.lr bus1=src bus2=b8 geometry=gd seasons=2 ratings=[400 500] length=1 \
             units=mi",
            vec![],
        ),
        (
            "new line.lw bus1=src bus2=b9 geometry=gw length=1 units=mi",
            vec![message("lw", "WireData.wb", "gw", "emergency")],
        ),
        (
            "new line.lc bus1=src bus2=b10 geometry=gn length=1 units=mi",
            vec![message("lc", "CNData.cb", "gn", "emergency")],
        ),
        (
            "new line.lk bus1=src bus2=b11 geometry=gk length=1 units=mi",
            vec![message("lk", "WireData.wb", "gk", "emergency")],
        ),
        (
            "new line.lu bus1=src bus2=b12 geometry=gu length=1 units=mi",
            vec![
                "Line.lu: phase conductor 1 (WireData.wu) of LineGeometry.gu has no normal rating \
                 where another phase conductor has one, so the Line's normal rating is not set."
                    .to_string(),
                "Line.lu: phase conductor 1 (WireData.wu) of LineGeometry.gu has no emergency \
                 rating where another phase conductor has one, so the Line's emergency rating is \
                 not set."
                    .to_string(),
            ],
        ),
    ];
    for (edit, messages) in edits {
        let before = dss.errors().len();
        dss.command(edit);
        assert_eq!(dss.error_texts()[before..], messages, "{edit}");
        if edit.starts_with("new line.ls") || edit.starts_with("new line.lp") {
            dss.command("~ normamps=500");
            assert_eq!(
                dss.errors().len(),
                before + 1,
                "{edit}: the later rating logs nothing: {:?}",
                dss.error_texts()
            );
        }
    }
    let logged = dss.errors().len();
    let want = [
        ("lb", "400", "none"),
        ("ln", "500", "none"),
        ("lo", "500", "600"),
        ("ls", "500", "none"),
        ("lm", "500", "600"),
        ("lp", "500", "600"),
        ("lt", "400", "700"),
        ("lr", "400", "600"),
        ("lw", "400", "none"),
        ("lc", "300", "none"),
        ("lk", "400", "none"),
        ("lu", "none", "none"),
    ];
    let names: Vec<String> = want.iter().map(|(l, _, _)| format!("line.{l}")).collect();
    let lines: Vec<(&str, &str, &str)> = names
        .iter()
        .zip(&want)
        .map(|(l, (_, n, e))| (l.as_str(), *n, *e))
        .collect();
    assert_ratings(&mut dss, &lines, "before the solve");
    assert_eq!(query(&mut dss, "line.lr.seasons"), "2");
    assert_eq!(query(&mut dss, "line.lr.ratings"), "[ 400 500]");
    for i in 1..=12 {
        dss.command(&format!("new load.ld{i} bus1=b{i} kw=1000 kv=12.47"));
    }
    for c in ["set voltagebases=[12.47]", "calcv", "solve"] {
        dss.command(c);
    }
    assert_eq!(
        dss.errors().len(),
        logged,
        "the solve runs: {:?}",
        dss.error_texts()
    );
    assert_ratings(&mut dss, &lines, "after the solve");
    let text = capacity_text_after_errors(&mut dss, "partly_geometry");
    for (l, norm, emerg) in want {
        let row = capacity_row(&text, &format!("Line.{},", l.to_ascii_uppercase()));
        let imax: f64 = row[1].parse().expect("Imax");
        assert!(imax > 40.0, "{l} carries its load: {row:?}");
        let pct = |r: &str| {
            r.parse::<f64>()
                .map_or("0.00".to_string(), |v| format!("{:.2}", imax / v * 100.0))
        };
        assert_eq!(row[2..4], [pct(norm), pct(emerg)], "{l}");
    }
}

/// `Save circuit` into a scratch folder of its own, removed on drop.
struct Saved {
    root: std::path::PathBuf,
}

impl Saved {
    fn new(dss: &mut Dss, tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!("dss_rating_{tag}_{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let before = dss.errors().len();
        dss.command(&format!("save circuit dir=\"{}\"", fwd(&root)));
        assert_eq!(
            dss.errors().len(),
            before,
            "{tag}: save errors: {:?}",
            dss.error_texts()
        );
        Self { root }
    }

    /// The saved record of `obj` (`Class.name`): its `New` line and the `~`
    /// lines after it, as written, found in the class files `Master.dss`
    /// redirects to.
    fn record(&self, obj: &str) -> String {
        let master =
            std::fs::read_to_string(self.root.join("Master.dss")).expect("the saved Master.dss");
        let files = master.lines().filter_map(|l| {
            let (verb, file) = l.trim().split_once(char::is_whitespace)?;
            let file = file.split_whitespace().next()?.trim_matches('"');
            verb.eq_ignore_ascii_case("redirect")
                .then(|| self.root.join(file))
        });
        for path in files {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if let Some(rec) = record_in(&text, obj) {
                return rec;
            }
        }
        panic!("no {obj} saved under {}", self.root.display())
    }

    /// The rating tokens of the saved record of `obj`, in the order written.
    fn ratings(&self, obj: &str) -> Vec<String> {
        rating_tokens(&self.record(obj))
    }

    /// The saved deck compiled into a fresh engine, with no error.
    fn reload(&self) -> Dss {
        let mut back = Dss::new();
        back.command(&format!(
            "compile \"{}\"",
            fwd(&self.root.join("Master.dss"))
        ));
        assert!(back.errors().is_empty(), "{:?}", back.error_texts());
        back
    }
}

impl Drop for Saved {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).ok();
    }
}

/// The record of `obj` (`Class.name`) in a saved class file: its `New` line
/// and the `~` lines after it, as written.
fn record_in(text: &str, obj: &str) -> Option<String> {
    let head = format!("new \"{}\"", obj.to_ascii_lowercase());
    let mut lines = text.lines();
    while let Some(l) = lines.next() {
        if l.to_ascii_lowercase().starts_with(&head) {
            let mut rec = l.to_string();
            for more in lines.by_ref() {
                if !more.starts_with('~') {
                    break;
                }
                rec.push('\n');
                rec.push_str(more);
            }
            return Some(rec);
        }
    }
    None
}

/// `Save class=` of each class into a scratch folder of its own, removed on
/// drop. A class save skips an object an earlier save of the session wrote,
/// so it runs before `Saved::new`.
struct ClassSaves {
    root: std::path::PathBuf,
}

impl ClassSaves {
    fn new(dss: &mut Dss, tag: &str, classes: &[&str]) -> Self {
        let root =
            std::env::temp_dir().join(format!("dss_rating_class_{tag}_{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let before = dss.errors().len();
        for class in classes {
            dss.command(&format!(
                "save class={class} file={class}.dss dir=\"{}\"",
                fwd(&root)
            ));
        }
        assert_eq!(
            dss.errors().len(),
            before,
            "{tag}: save class errors: {:?}",
            dss.error_texts()
        );
        Self { root }
    }

    /// The rating tokens of the record of `obj` in its class file.
    fn ratings(&self, obj: &str) -> Vec<String> {
        let class = obj.split('.').next().expect("Class.name");
        let path = self.root.join(format!("{class}.dss"));
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let rec = record_in(&text, obj)
            .unwrap_or_else(|| panic!("no {obj} in {}:\n{text}", path.display()));
        rating_tokens(&rec)
    }
}

impl Drop for ClassSaves {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).ok();
    }
}

fn fwd(p: &std::path::Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// The `NormAmps=`, `EmergAmps=` and `Ratings=` tokens of a saved record, in
/// the order written, a `Ratings` array whole.
fn rating_tokens(record: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (at, _) in record.match_indices('=') {
        let key_start = record[..at].rfind(char::is_whitespace).map_or(0, |i| i + 1);
        let key = &record[key_start..at];
        if !["normamps", "emergamps", "ratings"].contains(&key.to_ascii_lowercase().as_str()) {
            continue;
        }
        let rest = &record[at + 1..];
        let len = if rest.starts_with('[') {
            rest.find(']').map_or(rest.len(), |i| i + 1)
        } else {
            rest.find(char::is_whitespace).unwrap_or(rest.len())
        };
        out.push(format!("{key}={}", &rest[..len]));
    }
    out
}

/// `Save` writes a rating only when it was typed, by the class overrides and
/// the generic walk alike, a typed `none` as the not-set token and any other
/// typed number as typed. The deck saved by `Save circuit` or `Save class=`
/// reloads every rating as it read before the save.
#[test]
fn save_omits_a_rating_never_typed() {
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
    let mut queries = Vec::new();
    for obj in [
        "line.l1",
        "line.l2",
        "line.l3",
        "line.l4",
        "line.l5",
        "line.l6",
        "line.l8",
        "line.l9",
        "line.l10",
        "linegeometry.gx",
        "linegeometry.gy",
        "linegeometry.gt",
        "linegeometry.g3",
        "linecode.lcn",
        "wiredata.w1",
        "wiredata.w3",
        "wiredata.wa",
        "wiredata.wb",
        "cndata.cn1",
        "tsdata.ts1",
    ] {
        for prop in ["normamps", "emergamps", "ratings"] {
            queries.push(format!("{obj}.{prop}"));
        }
    }
    for obj in ["fault.fn", "capacitor.c1", "reactor.r1"] {
        for prop in ["normamps", "emergamps"] {
            queries.push(format!("{obj}.{prop}"));
        }
    }
    let before: Vec<String> = queries.iter().map(|q| query(&mut dss, q)).collect();
    for (what, want) in [
        ("line.l2.normamps", "none"),
        ("linegeometry.gt.normamps", "none"),
        ("linegeometry.gt.emergamps", "none"),
        ("linegeometry.g3.normamps", "none"),
        ("linegeometry.g3.emergamps", "795"),
        ("line.l9.normamps", "none"),
        ("line.l9.emergamps", "795"),
        ("line.l10.ratings", "[ none 600]"),
        ("linecode.lcn.normamps", "none"),
        ("wiredata.wa.normamps", "none"),
        ("wiredata.wb.emergamps", "none"),
        ("capacitor.c1.normamps", "none"),
        ("reactor.r1.emergamps", "none"),
    ] {
        let at = queries.iter().position(|q| q == what).expect(what);
        assert_eq!(before[at], want, "{what} before the save");
    }

    let classes = [
        "Line",
        "LineGeometry",
        "WireData",
        "Fault",
        "LineCode",
        "CNData",
        "TSData",
        "Capacitor",
        "Reactor",
    ];
    let class_saves = ClassSaves::new(&mut dss, "never_typed", &classes);
    let saved = Saved::new(&mut dss, "never_typed");
    for obj in ["Line.l1", "Line.l4", "Line.l6", "Line.l9", "WireData.w1"] {
        assert_eq!(
            saved.ratings(obj),
            Vec::<String>::new(),
            "{obj} types no rating: {}",
            saved.record(obj)
        );
        assert_eq!(
            class_saves.ratings(obj),
            Vec::<String>::new(),
            "{obj} in its class save"
        );
    }
    for (obj, want) in [
        ("Line.l2", &["NormAmps=-1", "EmergAmps=-1"][..]),
        ("Line.l3", &["NormAmps=0", "EmergAmps=600"]),
        ("Line.l5", &["NormAmps=-1", "EmergAmps=600"]),
        ("Line.l8", &["Ratings=[ 400 -1]"]),
        ("Line.l10", &["Ratings=[ -1 600]"]),
        (
            "LineGeometry.gx",
            &["NormAmps=-1", "EmergAmps=-1", "Ratings=[ -1]"],
        ),
        ("LineGeometry.gy", &["NormAmps=-1", "EmergAmps=-1"]),
        ("LineGeometry.gt", &["NormAmps=-1", "EmergAmps=-1"]),
        ("LineGeometry.g3", &["NormAmps=-1"]),
        ("WireData.w3", &["NormAmps=-1"]),
        ("WireData.wa", &["EmergAmps=600", "NormAmps=-1"]),
        ("WireData.wb", &["NormAmps=530", "EmergAmps=-1"]),
        ("Fault.fn", &["NormAmps=-1", "EmergAmps=-1"]),
        ("LineCode.lcn", &["NormAmps=-1", "Ratings=[ -1]"]),
        ("CNData.cn1", &["NormAmps=-1"]),
        ("TSData.ts1", &["EmergAmps=-1"]),
        ("Capacitor.c1", &["NormAmps=-1"]),
        ("Reactor.r1", &["EmergAmps=-1"]),
    ] {
        assert_eq!(saved.ratings(obj), want, "{}", saved.record(obj));
        assert_eq!(class_saves.ratings(obj), want, "{obj} in its class save");
    }

    let mut back = saved.reload();
    let after: Vec<String> = queries.iter().map(|q| query(&mut back, q)).collect();
    let moved: Vec<String> = queries
        .iter()
        .zip(before.iter().zip(&after))
        .filter(|(_, (b, a))| b != a)
        .map(|(q, (b, a))| format!("{q}: {b} -> {a}"))
        .collect();
    assert!(moved.is_empty(), "the reload moved {moved:#?}");
}

/// `Save` writes a rating as it was typed, by the Line and LineGeometry
/// overrides and the generic walk, and a `Seasons` edit that drops a typed
/// season drops it from the save too. Every object reloads the ratings it read,
/// and `Save class=` writes the same rating tokens as `Save circuit`.
#[test]
fn save_writes_what_was_typed() {
    let mut dss = run(&[
        "new circuit.c basekv=12.47 bus1=src pu=1.0",
        "new linecode.lc1 nphases=3 r1=0.1 x1=0.3 r0=0.3 x0=0.9 units=mi",
        "new line.tn bus1=src bus2=a normamps=none length=1 units=mi",
        "new line.tm bus1=src bus2=b normamps=-1 length=1 units=mi",
        "new line.t2 bus1=src bus2=c normamps=-2 emergamps=500 length=1 units=mi",
        "new line.t0 bus1=src bus2=d normamps=0 length=1 units=mi",
        "new line.ts bus1=src bus2=e seasons=2 ratings=[none 500] length=1 units=mi",
        "new line.lc bus1=src bus2=f linecode=lc1 length=1 units=mi",
        "new line.u bus1=src bus2=g length=1 units=mi",
        "new linegeometry.gn nconds=1 nphases=1 normamps=none",
        "new linegeometry.gm nconds=1 nphases=1 normamps=-2",
        "new line.sg bus1=src bus2=h seasons=3 ratings=[400 500 600] length=1 units=mi",
        "edit line.sg seasons=2",
        "edit line.sg seasons=3",
        "new wiredata.wsg diam=0.721 gmrac=0.0244 rac=0.306 runits=mi gmrunits=ft radunits=in          seasons=3 ratings=[400 500 600]",
        "edit wiredata.wsg seasons=1",
        "edit wiredata.wsg seasons=3",
    ]);
    let queries: Vec<String> = [
        "line.tn",
        "line.tm",
        "line.t2",
        "line.t0",
        "line.ts",
        "line.lc",
        "line.u",
        "linegeometry.gn",
        "linegeometry.gm",
        "line.sg",
        "wiredata.wsg",
    ]
    .iter()
    .flat_map(|o| ["normamps", "emergamps", "ratings"].map(|p| format!("{o}.{p}")))
    .collect();
    let before: Vec<String> = queries.iter().map(|q| query(&mut dss, q)).collect();
    let class_saves = ClassSaves::new(&mut dss, "typed", &["Line", "LineGeometry", "WireData"]);
    let saved = Saved::new(&mut dss, "typed");
    for (obj, want) in [
        ("Line.tn", &["NormAmps=-1"][..]),
        ("Line.tm", &["NormAmps=-1"]),
        ("Line.t2", &["NormAmps=-2", "EmergAmps=500"]),
        ("Line.t0", &["NormAmps=0"]),
        ("Line.ts", &["Ratings=[ -1 500]"]),
        ("Line.lc", &[]),
        ("Line.u", &[]),
        ("LineGeometry.gn", &["NormAmps=-1"]),
        ("LineGeometry.gm", &["NormAmps=-2"]),
        ("Line.sg", &["Ratings=[ 400 500]"]),
        ("WireData.wsg", &["Ratings=[ 400]"]),
    ] {
        assert_eq!(saved.ratings(obj), want, "{}", saved.record(obj));
        assert_eq!(class_saves.ratings(obj), want, "{obj} in its class save");
    }
    let mut back = saved.reload();
    for (q, b) in queries.iter().zip(&before) {
        assert_eq!(&query(&mut back, q), b, "{q} after the reload");
    }
    for (what, want) in [
        ("line.tn.normamps", "none"),
        ("line.t2.normamps", "none"),
        ("line.ts.ratings", "[ none 500]"),
        ("line.lc.normamps", "400"),
        ("line.lc.emergamps", "600"),
        ("line.u.normamps", "400"),
        ("linegeometry.gm.normamps", "none"),
        ("line.sg.ratings", "[ 400 500 none]"),
        ("wiredata.wsg.ratings", "[ 400 none none]"),
    ] {
        assert_eq!(query(&mut back, what), want, "{what} after the reload");
    }
}

/// JSON writes a rating that is not set as `null` in the full sweep and, when
/// it was typed, in the set-order one, and an import reads `null` and the
/// not-set token as not set. A `null` loaded before its partner is a typed
/// rating and is never derived from it.
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
    assert_eq!(
        member(&set_only, "NormAmps"),
        Some(Json::Null),
        "{set_only}"
    );
    let line = dss
        .obj_to_json("line.l", JsonOpts::NONE)
        .expect("line json");
    assert_eq!(member(&line, "NormAmps"), Some(Json::Null), "{line}");
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
        Some(Json::Arr(vec![Json::Null])),
        "a seasonal array typed with no entry set is written in the set-order sweep: {unrated}"
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
        ("wiredata.wn.normamps", "none"),
        ("wiredata.wn.emergamps", "600"),
    ] {
        assert_eq!(
            query(&mut back, what),
            want,
            "a null rating loaded before its partner stays typed"
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
    for (what, want) in [("line.c.seasons", "2"), ("line.c.ratings", "[ none 600]")] {
        assert_eq!(
            query(&mut back, what),
            want,
            "{what}: a null entry keeps its season and the array sets the season count"
        );
    }
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

/// A Capacitor or Reactor rating derived as exactly the not-set token is not
/// set, while the Transformer and AutoTrans current ratings stay numbers. A
/// typed negative rating is no rating and derives nothing, also by `Set
/// %Normal`, and `Save` writes it as typed.
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
    for what in [
        "wiredata.wa.normamps",
        "wiredata.wa.emergamps",
        "line.s.normamps",
        "line.s.emergamps",
        "line.a.emergamps",
    ] {
        assert_eq!(
            query(&mut dss, what),
            "none",
            "{what}: a typed negative is not set"
        );
    }
    assert_eq!(query(&mut dss, "line.a.normamps"), "530");
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
    let emergency_pct = |text: &str, name: &str| -> String {
        text.lines()
            .find(|l| l.starts_with(name))
            .unwrap_or_else(|| panic!("no {name} in {text}"))
            .split(',')
            .nth(3)
            .expect("%emergency")
            .trim()
            .to_string()
    };
    let before = capacity(&mut dss, "minus_one_a");
    assert_eq!(normal_pct(&before, "Line.S"), "0.00", "{before}");
    assert_eq!(emergency_pct(&before, "Line.S"), "0.00", "{before}");

    dss.command("set %normal=80");
    assert_eq!(query(&mut dss, "line.a.normamps"), "none");
    assert_eq!(query(&mut dss, "line.s.normamps"), "none");
    dss.command("solve");
    let after = capacity(&mut dss, "minus_one_b");
    assert_eq!(normal_pct(&after, "Line.A"), "0.00", "{after}");
    assert_eq!(normal_pct(&after, "Line.S"), "0.00", "{after}");

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
    assert!(
        saved_a.contains("normamps=530 emergamps=-1.25"),
        "Save writes the typed numbers, not the live rating: {saved_a}"
    );
    let mut back = Dss::new();
    back.command(&format!("compile \"{}\"", fwd(&saved.join("Master.dss"))));
    assert_eq!(query(&mut back, "line.a.normamps"), "530");
    assert_eq!(query(&mut back, "line.a.emergamps"), "none");
    assert_eq!(query(&mut back, "wiredata.wa.normamps"), "none");
    std::fs::remove_dir_all(&saved).ok();

    dss.command("edit wiredata.wa normamps=530");
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    assert_ratings(
        &mut dss,
        &[("wiredata.wa", "530", "none")],
        "the typed -1.5 stays against a later normal rating",
    );
}

/// The deck of the Transformer and AutoTrans rating pins: a 3-phase 500 kVA
/// 12.47/0.48 kV Transformer `t` and a 2000 kVA 12.47/4.16 kV AutoTrans `at`.
const XF_DECK: [&str; 3] = [
    "new circuit.c basekv=12.47 bus1=src pu=1.0",
    "new transformer.t phases=3 windings=2 buses=[src b] conns=[wye wye] kvs=[12.47 0.48] \
     kvas=[500 500] xhl=6",
    "new autotrans.at phases=3 windings=2 buses=[src c] conns=[s w] kvs=[12.47 4.16] \
     kvas=[2000 2000] xhx=5",
];

/// Asserts that `obj` reads the current ratings derived from `kva`: `1.1 · kVA`
/// and `1.5 · kVA` over three phases and the phase voltage `kv_ln`.
fn assert_derived_amps(dss: &mut Dss, obj: &str, kva: f64, kv_ln: f64, why: &str) {
    for (prop, factor) in [("normamps", 1.1), ("emergamps", 1.5)] {
        let want = factor * kva / 3.0 / kv_ln;
        let got = query_f64(dss, &format!("{obj}.{prop}"));
        assert!(
            ((got - want) / want).abs() < 1e-12,
            "{obj}.{prop} reads the rating derived from {kva} kVA, {want}, {why}: {got}"
        );
    }
}

/// The `NormAmps` and `EmergAmps` of a Transformer and an AutoTrans are read
/// only, so a typed value is refused at its token on every route with a message
/// naming the kVA property that sets the rating, and the rating derived from
/// the kVA rating stays. `Save` and the set-order JSON export write neither, so
/// the saved deck reloads with no message and the same derived ratings.
#[test]
fn a_typed_transformer_current_rating_is_refused() {
    use crate::report::export::json::{Json, JsonOpts, parse_json, write_compact};
    const T_NORM: &str = "Transformer.t.NormAmps: read only, the normal rating is set by NormHkVA.";
    const T_EMERG: &str =
        "Transformer.t.EmergAmps: read only, the emergency rating is set by EmergHkVA.";
    const AT_NORM: &str = "AutoTrans.at.NormAmps: read only, the normal rating is set by NormHkVA.";
    const AT_EMERG: &str =
        "AutoTrans.at.EmergAmps: read only, the emergency rating is set by EmergHkVA.";
    let t_kv = 12.47 / 3f64.sqrt();
    let at_kv = (12.47 - 4.16) / 3f64.sqrt();
    let expect_errors = |dss: &mut Dss, want: &[&str], what: &str| {
        assert_eq!(dss.error_texts(), want, "{what}");
        dss.errors.clear();
    };

    let mut dss = run(&XF_DECK);
    for typed in ["400", "none", "-1", "0", "-2", "(1 0 /)"] {
        for (obj, prop, msg) in [
            ("transformer.t", "normamps", T_NORM),
            ("transformer.t", "emergamps", T_EMERG),
            ("autotrans.at", "normamps", AT_NORM),
            ("autotrans.at", "emergamps", AT_EMERG),
        ] {
            let cmd = format!("edit {obj} {prop}={typed}");
            dss.command(&cmd);
            expect_errors(&mut dss, &[msg], &cmd);
        }
        let why = format!("after a typed {typed}");
        assert_derived_amps(&mut dss, "transformer.t", 500.0, t_kv, &why);
        assert_derived_amps(&mut dss, "autotrans.at", 2000.0, at_kv, &why);
    }

    dss.command("edit transformer.t xhl=6");
    dss.command("~ normamps=400 emergamps=600");
    expect_errors(&mut dss, &[T_NORM, T_EMERG], "a `~` line");
    dss.command("edit autotrans.at xhx=5");
    dss.command("~ normamps=400 emergamps=600");
    expect_errors(&mut dss, &[AT_NORM, AT_EMERG], "a `~` line");
    dss.command("transformer.t.normamps=400");
    expect_errors(&mut dss, &[T_NORM], "the property form");
    dss.command("autotrans.at.emergamps=600");
    expect_errors(&mut dss, &[AT_EMERG], "the property form");
    dss.command(
        "new transformer.td phases=3 windings=2 buses=[src d] conns=[wye wye] \
         kvs=[12.47 0.48] kvas=[500 500] xhl=6 normamps=400 emergamps=600",
    );
    expect_errors(
        &mut dss,
        &[
            "Transformer.td.NormAmps: read only, the normal rating is set by NormHkVA.",
            "Transformer.td.EmergAmps: read only, the emergency rating is set by EmergHkVA.",
        ],
        "the definition",
    );
    dss.command(
        "new autotrans.ad phases=3 windings=2 buses=[src e] conns=[s w] kvs=[12.47 4.16] \
         kvas=[2000 2000] xhx=5 normamps=400 emergamps=600",
    );
    expect_errors(
        &mut dss,
        &[
            "AutoTrans.ad.NormAmps: read only, the normal rating is set by NormHkVA.",
            "AutoTrans.ad.EmergAmps: read only, the emergency rating is set by EmergHkVA.",
        ],
        "the definition",
    );
    dss.command("new transformer.t2 like=t normamps=400");
    expect_errors(
        &mut dss,
        &["Transformer.t2.NormAmps: read only, the normal rating is set by NormHkVA."],
        "a like= copy",
    );
    dss.command("new autotrans.at2 like=at emergamps=600");
    expect_errors(
        &mut dss,
        &["AutoTrans.at2.EmergAmps: read only, the emergency rating is set by EmergHkVA."],
        "a like= copy",
    );
    dss.command("edit transformer.t normamps=400 kvas=[600 600]");
    expect_errors(&mut dss, &[T_NORM], "a token after the refused one");
    dss.command("edit autotrans.at emergamps=600 kvas=[3000 3000]");
    expect_errors(&mut dss, &[AT_EMERG], "a token after the refused one");
    let readings = [
        ("transformer.t", 600.0, t_kv),
        ("autotrans.at", 3000.0, at_kv),
        ("transformer.td", 500.0, t_kv),
        ("autotrans.ad", 2000.0, at_kv),
        ("transformer.t2", 500.0, t_kv),
        ("autotrans.at2", 2000.0, at_kv),
    ];
    for (obj, kva, kv) in readings {
        assert_derived_amps(&mut dss, obj, kva, kv, "after every route");
    }

    let saved =
        std::env::temp_dir().join(format!("dss_rating_read_only_save_{}", std::process::id()));
    std::fs::remove_dir_all(&saved).ok();
    dss.command(&format!("save circuit dir=\"{}\"", fwd(&saved)));
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    for file in ["Transformer.dss", "AutoTrans.dss"] {
        let text = std::fs::read_to_string(saved.join(file)).expect("saved file");
        let lower = text.to_ascii_lowercase();
        assert!(
            !lower.contains("normamps") && !lower.contains("emergamps"),
            "Save writes no current rating of a Transformer or an AutoTrans: {text}"
        );
    }
    let mut back = Dss::new();
    back.command(&format!("compile \"{}\"", fwd(&saved.join("Master.dss"))));
    assert!(back.errors().is_empty(), "{:?}", back.error_texts());
    for (obj, kva, kv) in readings {
        assert_derived_amps(&mut back, obj, kva, kv, "after the reload");
    }
    std::fs::remove_dir_all(&saved).ok();

    let member = |text: &str, key: &str| -> Option<Json> {
        match parse_json(text).expect("JSON") {
            Json::Obj(m) => m.into_iter().find(|(k, _)| k == key).map(|(_, v)| v),
            other => panic!("not an object: {other:?}"),
        }
    };
    for obj in ["transformer.t", "autotrans.at"] {
        let set_order = dss.obj_to_json(obj, JsonOpts::NONE).expect("json");
        for key in ["NormAmps", "EmergAmps"] {
            assert_eq!(member(&set_order, key), None, "{obj} {key}: {set_order}");
        }
    }
    let full = dss
        .obj_to_json("transformer.t", JsonOpts::FULL)
        .expect("json");
    for key in ["NormAmps", "EmergAmps"] {
        assert_eq!(member(&full, key), None, "transformer.t {key}: {full}");
    }
    let full = dss
        .obj_to_json("autotrans.at", JsonOpts::FULL)
        .expect("json");
    for (key, factor) in [("NormAmps", 1.1), ("EmergAmps", 1.5)] {
        let want = factor * 3000.0 / 3.0 / at_kv;
        match member(&full, key) {
            Some(Json::Float(got)) => assert!(
                ((got - want) / want).abs() < 1e-12,
                "autotrans.at {key}: {got}, want {want}"
            ),
            other => panic!("autotrans.at {key}: {other:?} in {full}"),
        }
    }

    let circuit = dss.circuit_to_json(JsonOpts::NONE).expect("circuit json");
    let mut back = Dss::new();
    back.circuit_from_json(&circuit).expect("JSON load");
    assert!(back.errors().is_empty(), "{:?}", back.error_texts());
    for (obj, kva, kv) in readings {
        assert_derived_amps(&mut back, obj, kva, kv, "after the set-order JSON import");
    }
    for (class, name, key, msg, kva, kv) in [
        ("Transformer", "t", "NormAmps", T_NORM, 600.0, t_kv),
        ("AutoTrans", "at", "EmergAmps", AT_EMERG, 3000.0, at_kv),
    ] {
        let mut tree = parse_json(&circuit).expect("JSON");
        let Json::Obj(root) = &mut tree else {
            panic!("not an object: {circuit}")
        };
        let Some((_, Json::Arr(objects))) = root.iter_mut().find(|(k, _)| k == class) else {
            panic!("no {class} array in {circuit}")
        };
        let named = Json::Str(name.to_string());
        let Some(Json::Obj(members)) = objects.iter_mut().find(
            |o| matches!(o, Json::Obj(m) if m.iter().any(|(k, v)| k == "Name" && *v == named)),
        ) else {
            panic!("no {class}.{name} in {circuit}")
        };
        members.push((key.to_string(), Json::Float(400.0)));
        let mut text = String::new();
        write_compact(&tree, &mut text);
        let mut back = Dss::new();
        back.circuit_from_json(&text).expect("JSON load");
        assert_eq!(
            back.error_texts(),
            [msg],
            "a JSON import carrying {class}.{name} {key}"
        );
        let obj = format!("{class}.{name}").to_ascii_lowercase();
        assert_derived_amps(&mut back, &obj, kva, kv, "after the refused JSON key");
        // The import loads the Transformers before the AutoTrans, each class in
        // its definition order, and stops at the refused object.
        let load_order = [
            "transformer.t",
            "transformer.td",
            "transformer.t2",
            "autotrans.at",
            "autotrans.ad",
            "autotrans.at2",
        ];
        let refused = load_order
            .iter()
            .position(|o| *o == obj)
            .expect("in the load order");
        for (o, o_kva, o_kv) in readings
            .iter()
            .filter(|(o, _, _)| load_order[..refused].contains(o))
        {
            assert_derived_amps(
                &mut back,
                o,
                *o_kva,
                *o_kv,
                "loaded before the refused object",
            );
        }
        for o in &load_order[refused + 1..] {
            let before = back.errors().len();
            query(&mut back, &format!("{o}.kvas"));
            assert_eq!(
                back.error_texts()[before..],
                [format!("Error! Object \"{o}\" not found.")],
                "the import stops at {obj}, so {o} is not loaded"
            );
        }
    }

    let mut xfm1 = Dss::new();
    for line in [
        "new circuit.c basekv=4.16 bus1=src pu=1.0",
        "New Transformer.XFM1  Phases=3   Windings=2  XHL=2",
        "~ wdg=1 bus=633       conn=Wye kv=4.16    kva=500    %r=.55",
        "~ wdg=2 bus=634       conn=Wye kv=0.480    kva=500    %r=.55   normamps=400 emergamps=600",
    ] {
        xfm1.command(line);
    }
    assert_eq!(
        xfm1.error_texts(),
        [
            "Transformer.xfm1.NormAmps: read only, the normal rating is set by NormHkVA.",
            "Transformer.xfm1.EmergAmps: read only, the emergency rating is set by EmergHkVA.",
        ],
        "the StoCtrl_SeasonTarget XFM1 definition"
    );
    assert_derived_amps(
        &mut xfm1,
        "transformer.xfm1",
        500.0,
        4.16 / 3f64.sqrt(),
        "the StoCtrl_SeasonTarget XFM1 typed 400 A and 600 A",
    );
}

/// The kVA ratings of a Transformer and an AutoTrans have no not-set state, so
/// a current rating derived as exactly the not-set token is a negative rating
/// like any other and reads back as derived.
#[test]
fn a_transformer_current_rating_derived_as_minus_one_ampere_is_a_rating() {
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

/// `Set %Normal` rates every Line's normal rating at the given percentage of
/// its emergency rating, a Line with no emergency rating and a percentage that
/// is not positive leaving it not set, and `Get %Normal` reads the factor back.
/// Afterwards `?` reads the live rating, `Save` writes the number typed and
/// `Export Capacity` divides by the new ratings.
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
        "80 % of an emergency rating that is not set is not set and replaces the typed 300"
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
            "`?` reads the live rating of {l} after Set %Normal"
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
    let q9b = "Save after Set %Normal writes the typed rating";
    assert!(
        saved_line("ta").contains("normamps=530"),
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
    for (l, want) in [("ta", "530"), ("td", "400"), ("l6", "530")] {
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

    for pct in ["0", "-80"] {
        dss.command(&format!("set %normal={pct}"));
        assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
        dss.command("get %normal");
        assert_eq!(dss.result(), pct);
        for l in ["l3", "l5", "l6", "ta", "td"] {
            assert_eq!(
                query(&mut dss, &format!("line.{l}.normamps")),
                "none",
                "Set %Normal={pct}: a percentage of zero or below names no rating (line.{l})"
            );
        }
        assert_eq!(query(&mut dss, "line.ta.emergamps"), "795");
        let dir = std::env::temp_dir().join(format!(
            "dss_rating_pctzero_{}_{}",
            pct.replace('-', "m"),
            std::process::id()
        ));
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
        let r: Vec<String> = text
            .lines()
            .find(|l| l.starts_with("Line.L3"))
            .unwrap_or_else(|| panic!("no Line.L3 in {text}"))
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
        let imax: f64 = r[1].parse().expect("Imax");
        assert_eq!(r[2], "0.00", "Set %Normal={pct}: {text}");
        assert_eq!(
            r[3],
            format!("{:.2}", imax / 600.0 * 100.0),
            "Set %Normal={pct}"
        );
    }
}

/// `Export Capacity` written for `dss` into a scratch folder, as text.
fn capacity_text(dss: &mut Dss, tag: &str) -> String {
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
}

/// The `Export Capacity` row of `name` (`Class.NAME`), split at its commas.
fn capacity_row(text: &str, name: &str) -> Vec<String> {
    text.lines()
        .find(|l| l.starts_with(name))
        .unwrap_or_else(|| panic!("no {name} in {text}"))
        .split(',')
        .map(|s| s.trim().to_string())
        .collect()
}

/// A typed zero or negative rating names no rating on every class: `?` reads it
/// `none`, the JSON sweep writes `null` and no loading measures against it,
/// while `Save` writes the number typed and reloads the same readings. A
/// positively rated twin carrying the same current writes the one overload row.
#[test]
fn a_typed_zero_or_negative_rating_reads_none() {
    let mut dss = run(&[
        "new circuit.c basekv=12.47 bus1=src pu=1.0",
        "new line.l bus1=src bus2=b length=1 units=mi normamps=0 emergamps=-2 seasons=3 \
         ratings=[0 -3 400]",
        "new load.ld bus1=b kw=9000 kv=12.47",
        "new line.r bus1=src bus2=br length=1 units=mi normamps=400 emergamps=600",
        "new load.lr bus1=br kw=9000 kv=12.47",
        "set voltagebases=[12.47]",
        "calcv",
        "solve",
    ]);
    for (prop, want) in [
        ("normamps", "none"),
        ("emergamps", "none"),
        ("ratings", "[ none none 400]"),
    ] {
        assert_eq!(query(&mut dss, &format!("line.l.{prop}")), want, "{prop}");
    }
    {
        use crate::report::export::json::{Json, JsonOpts, parse_json};
        let full = dss
            .obj_to_json("line.l", JsonOpts::FULL)
            .expect("line json");
        let Json::Obj(members) = parse_json(&full).expect("JSON") else {
            panic!("not an object: {full}");
        };
        let member = |key: &str| members.iter().find(|(k, _)| k == key).map(|(_, v)| v);
        assert_eq!(member("NormAmps"), Some(&Json::Null), "{full}");
        assert_eq!(member("EmergAmps"), Some(&Json::Null), "{full}");
        assert_eq!(
            member("Ratings"),
            Some(&Json::Arr(vec![Json::Null, Json::Null, Json::Float(400.0)])),
            "{full}"
        );
    }
    {
        let saved = Saved::new(&mut dss, "typed_zero_save");
        assert_eq!(
            saved.ratings("Line.l"),
            ["NormAmps=0", "EmergAmps=-2", "Ratings=[ 0 -3 400]"],
            "{}",
            saved.record("Line.l")
        );
        let mut back = saved.reload();
        for (prop, want) in [
            ("normamps", "none"),
            ("emergamps", "none"),
            ("ratings", "[ none none 400]"),
        ] {
            assert_eq!(
                query(&mut back, &format!("line.l.{prop}")),
                want,
                "{prop} after the reload"
            );
        }
    }
    let capacity = capacity_text(&mut dss, "typed_zero");
    let row = capacity_row(&capacity, "Line.L");
    let imax: f64 = row[1].parse().expect("Imax");
    assert!(imax > 400.0, "the load draws past 400 A: {row:?}");
    assert_eq!(row[2..4], ["0.00", "0.00"], "{capacity}");
    let twin = capacity_row(&capacity, "Line.R");
    let twin_imax: f64 = twin[1].parse().expect("Imax");
    assert_eq!(twin[1], row[1], "the twin carries the same current");
    assert_eq!(
        twin[2..4],
        [
            format!("{:.2}", twin_imax / 400.0 * 100.0),
            format!("{:.2}", twin_imax / 600.0 * 100.0)
        ],
        "{capacity}"
    );
    let dir =
        std::env::temp_dir().join(format!("dss_rating_typed_zero_ovl_{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dss.command(&format!(
        "set datapath=\"{}\"",
        dir.to_string_lossy().replace('\\', "/")
    ));
    dss.command("export overloads");
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    let overloads = std::fs::read_to_string(dss.last_result_file()).expect("overloads");
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        !overloads.to_ascii_lowercase().contains("line.l"),
        "a typed 0 or negative rating is no overload:\n{overloads}"
    );
    let rows: Vec<Vec<&str>> = overloads
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split(',').map(str::trim).collect())
        .collect();
    assert_eq!(rows.len(), 1, "the twin's row only:\n{overloads}");
    // Element, Terminal, I1, AmpsOver, kVAOver, %Normal, %Emergency, ...
    let over = &rows[0];
    assert_eq!(over[0], "\"Line.R\"", "{overloads}");
    assert_eq!(
        [over[3], over[5], over[6]],
        [
            format!("{:.2}", twin_imax - 400.0),
            format!("{:.1}", twin_imax / 400.0 * 100.0),
            format!("{:.1}", twin_imax / 600.0 * 100.0),
        ],
        "the twin's amps over its normal rating and its loadings:\n{overloads}"
    );

    // Collected, so a failure names every class that reads a number.
    let mut wrong = Vec::new();
    for obj in [
        "line.x bus1=a bus2=b",
        "linecode.x",
        "linegeometry.x nconds=1",
        "wiredata.x",
        "cndata.x",
        "tsdata.x",
        "fault.x bus1=a",
        "gictransformer.x bush=a",
        "reactor.x bus1=a",
        "capacitor.x bus1=a",
    ] {
        let name = obj.split_whitespace().next().unwrap();
        for (norm, emerg) in [("0", "-2"), ("-2", "0")] {
            let mut dss = run(&[
                "new circuit.c",
                &format!("new {obj} normamps={norm} emergamps={emerg}"),
            ]);
            for prop in ["normamps", "emergamps"] {
                let got = query(&mut dss, &format!("{name}.{prop}"));
                if got != "none" {
                    wrong.push(format!(
                        "{name} normamps={norm} emergamps={emerg}: {prop} reads {got}"
                    ));
                }
            }
            let saved = Saved::new(&mut dss, "typed_zero_class");
            let got = saved.ratings(name);
            if got != [format!("NormAmps={norm}"), format!("EmergAmps={emerg}")] {
                wrong.push(format!(
                    "{name} normamps={norm} emergamps={emerg}: Save writes {got:?}"
                ));
            }
        }
    }
    for obj in [
        "linecode.x",
        "linegeometry.x nconds=1",
        "wiredata.x",
        "cndata.x",
        "tsdata.x",
    ] {
        let name = obj.split_whitespace().next().unwrap();
        let mut dss = run(&[
            "new circuit.c",
            &format!("new {obj} seasons=3 ratings=[0 -3 400]"),
        ]);
        let got = query(&mut dss, &format!("{name}.ratings"));
        if got != "[ none none 400]" {
            wrong.push(format!("{name} ratings=[0 -3 400] reads {got}"));
        }
        let got = Saved::new(&mut dss, "typed_zero_seasons").ratings(name);
        if got != ["Ratings=[ 0 -3 400]"] {
            wrong.push(format!("{name} ratings=[0 -3 400]: Save writes {got:?}"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");

    let mut geometry_line = run(&[
        "new circuit.c basekv=12.47 bus1=src pu=1.0",
        &format!("new wiredata.w {WIRE} normamps=530"),
        "new linegeometry.g nconds=3 nphases=3 cond=1 wire=w x=-4 h=28 units=ft cond=2 wire=w \
         x=0 h=28 units=ft cond=3 wire=w x=4 h=28 units=ft",
        "new line.lg bus1=src bus2=b geometry=g Seasons=1 Ratings=[0,] normamps=650 \
         emergamps=975 length=1 units=mi",
    ]);
    for (prop, want) in [
        ("seasons", "1"),
        ("ratings", "[ none]"),
        ("normamps", "650"),
        ("emergamps", "975"),
    ] {
        assert_eq!(
            query(&mut geometry_line, &format!("line.lg.{prop}")),
            want,
            "line.lg.{prop}: the ckt24 zone 2 form"
        );
    }
    let saved = Saved::new(&mut geometry_line, "typed_zero_geometry");
    let record = saved.record("Line.lg");
    assert!(record.contains(" Seasons=1 "), "{record}");
    assert_eq!(
        saved.ratings("Line.lg"),
        ["Ratings=[ 0]", "NormAmps=650", "EmergAmps=975"],
        "{record}"
    );
}

/// A Capacitor rating derived as zero, from a bank given no kvar, a `cmatrix`
/// bank or a bank of zero kvar, is not set. The Capacitor rows of
/// `not_set_ratings::LANE_NOT_SET_RATINGS` excuse the zero the oracles print.
#[test]
fn a_derived_zero_rating_is_not_set() {
    let mut dss = run(&[
        "new circuit.c basekv=12.47 bus1=src pu=1.0",
        "new capacitor.c1 bus1=src phases=3 cmatrix=(2.8 | -0.6 2.8 | -0.6 -0.6 2.8)",
        "new capacitor.k0 bus1=src kv=12.47 kvar=0",
        "set voltagebases=[12.47]",
        "calcv",
        "solve",
    ]);
    for name in ["c1", "k0"] {
        for prop in ["normamps", "emergamps"] {
            assert_eq!(
                query(&mut dss, &format!("capacitor.{name}.{prop}")),
                "none",
                "capacitor.{name}.{prop}: a derived zero is not set"
            );
        }
    }
    let capacity = capacity_text(&mut dss, "derived_zero");
    for name in ["Capacitor.C1", "Capacitor.K0"] {
        assert_eq!(
            capacity_row(&capacity, name)[2..4],
            ["0.00", "0.00"],
            "{name}"
        );
    }
}

/// A Capacitor or Reactor rating derived negative from a negative kvar is a
/// rating, and `Export Capacity` measures the current against its magnitude. A
/// typed negative conductor rating is no rating and derives nothing.
#[test]
fn a_derived_negative_rating_loads_against_its_magnitude() {
    let mut dss = run(&[
        "new circuit.c basekv=12.47 bus1=src pu=1.0",
        "new line.l1 bus1=src bus2=b length=1 units=kft",
        "new capacitor.e bus1=b kv=12.47 kvar=-300",
        "new line.l2 bus1=src bus2=r length=1 units=kft",
        "new reactor.x bus1=r kv=12.47 kvar=-300",
        "set maxiterations=20",
        "solve",
    ]);
    assert_eq!(query(&mut dss, "capacitor.e.normamps"), "-18.7511514853086");
    assert_eq!(
        query(&mut dss, "capacitor.e.emergamps"),
        "-25.0015353137448"
    );
    for (what, factor) in [
        ("capacitor.e.normamps", CAPACITOR_NORMAL),
        ("capacitor.e.emergamps", CAPACITOR_EMERGENCY),
        ("reactor.x.normamps", REACTOR_NORMAL),
        ("reactor.x.emergamps", REACTOR_EMERGENCY),
    ] {
        assert_kvar_rating(&mut dss, what, -300.0, factor, "a derived negative rating");
    }
    let capacity = capacity_text(&mut dss, "derived_negative");
    for (name, obj, oracle) in [
        ("Capacitor.E", "capacitor.e", ["74.06", "55.54"]),
        ("Reactor.X", "reactor.x", ["100.02", "74.09"]),
    ] {
        let row = capacity_row(&capacity, name);
        let imax: f64 = row[1].parse().expect("Imax");
        let by_magnitude = ["normamps", "emergamps"].map(|prop| {
            let rating = query_f64(&mut dss, &format!("{obj}.{prop}"));
            format!("{:.2}", imax / rating.abs() * 100.0)
        });
        assert_eq!(
            row[2..4],
            by_magnitude,
            "{name}: the loading against the magnitude: {capacity}"
        );
        assert_eq!(
            by_magnitude, oracle,
            "{name}: the magnitudes of the loading both oracles print"
        );
    }

    let mut wire = run(&[
        "new circuit.c",
        &format!("new wiredata.w {WIRE} emergamps=-1.6"),
    ]);
    for prop in ["normamps", "emergamps"] {
        assert_eq!(
            query(&mut wire, &format!("wiredata.w.{prop}")),
            "none",
            "a typed negative is no rating and derives nothing"
        );
    }
}

/// A later `linecode=`, `geometry=` or spacing conductor list is a definition
/// of the Line that replaces the ratings typed before it, the seasonal ones
/// when it brings its own, and `Save` writes none it replaced, while a rating
/// typed after the definition applies and is saved. Each Line reloads the
/// seasons and ratings it read.
#[test]
fn a_later_line_definition_replaces_a_typed_line_rating() {
    let mut dss = run(&[
        "new circuit.c basekv=12.47 bus1=src pu=1.0",
        &format!("new wiredata.w {WIRE} normamps=530 emergamps=795"),
        "new linegeometry.g nconds=3 nphases=3 cond=1 wire=w x=-4 h=28 units=ft cond=2 wire=w \
         x=0 h=28 units=ft cond=3 wire=w x=4 h=28 units=ft",
        "new linecode.lc1 nphases=3 r1=0.1 x1=0.3 r0=0.3 x0=0.9 units=mi",
        "new line.la bus1=src bus2=a normamps=500 linecode=lc1 length=1 units=mi",
        "new line.lb bus1=src bus2=b linecode=lc1 normamps=500 length=1 units=mi",
        "new line.lc bus1=src bus2=c normamps=500 geometry=g length=1 units=mi",
        "new line.ld bus1=src bus2=d geometry=g normamps=500 length=1 units=mi",
        "new linespacing.sp nconds=3 nphases=3 units=ft x=[-4 0 4] h=[28 28 28]",
        "new line.le bus1=src bus2=e normamps=500 spacing=sp wires=[w w w] length=1 units=mi",
        "new line.lf bus1=src bus2=f spacing=sp normamps=500 wires=[w w w] length=1 units=mi",
        "new line.lg bus1=src bus2=g normamps=500 spacing=sp length=1 units=mi",
        "~ wires=[w w w]",
        "new line.lh bus1=src bus2=h spacing=sp wires=[w w w] normamps=500 length=1 units=mi",
        &format!(
            "new wiredata.w3 {WIRE} normamps=400 emergamps=600 seasons=3 \
             ratings=[400 500 600]"
        ),
        "new line.sa bus1=src bus2=sa seasons=2 ratings=[500 600] linecode=lc1 length=1 units=mi",
        "new line.sc bus1=src bus2=sc seasons=2 ratings=[450 550] length=1 units=mi",
        "~ spacing=sp wires=[w3 w3 w3]",
        "new line.sd bus1=src bus2=sd seasons=2 ratings=[450 550] length=1 units=mi",
        "~ spacing=sp wires=[w w w]",
    ]);
    let want = [
        ("la", "400", "600", &[][..]),
        ("lb", "500", "600", &["NormAmps=500"]),
        ("lc", "530", "795", &[]),
        ("ld", "500", "795", &["NormAmps=500"]),
        ("le", "530", "795", &[]),
        ("lf", "530", "795", &[]),
        ("lg", "530", "795", &[]),
        ("lh", "500", "795", &["NormAmps=500"]),
        ("sa", "400", "600", &[]),
        ("sc", "400", "600", &[]),
        ("sd", "530", "795", &["Ratings=[ 450 550]"]),
    ];
    let seasonal = [
        ("sa", "1", "[ 400]"),
        ("sc", "3", "[ 400 500 600]"),
        ("sd", "2", "[ 450 550]"),
    ];
    let seasonal_readings = |dss: &mut Dss| -> Vec<(String, String)> {
        seasonal
            .iter()
            .map(|(l, _, _)| {
                (
                    query(dss, &format!("line.{l}.seasons")),
                    query(dss, &format!("line.{l}.ratings")),
                )
            })
            .collect()
    };
    let seasonal_want: Vec<(String, String)> = seasonal
        .iter()
        .map(|(_, s, r)| (s.to_string(), r.to_string()))
        .collect();
    for (l, norm, emerg, _) in want {
        assert_eq!(query(&mut dss, &format!("line.{l}.normamps")), norm, "{l}");
        assert_eq!(
            query(&mut dss, &format!("line.{l}.emergamps")),
            emerg,
            "{l}"
        );
    }
    assert_eq!(seasonal_readings(&mut dss), seasonal_want, "sa, sc, sd");
    let saved = Saved::new(&mut dss, "later_definition");
    for (l, _, _, tokens) in want {
        let obj = format!("Line.{l}");
        assert_eq!(saved.ratings(&obj), tokens, "{}", saved.record(&obj));
    }
    let mut back = saved.reload();
    assert_eq!(
        seasonal_readings(&mut back),
        seasonal_want,
        "sa, sc, sd reloaded"
    );
    for (l, norm, emerg, _) in want {
        assert_eq!(
            query(&mut back, &format!("line.{l}.normamps")),
            norm,
            "{l} reloaded"
        );
        assert_eq!(
            query(&mut back, &format!("line.{l}.emergamps")),
            emerg,
            "{l} reloaded"
        );
    }
}

/// After `Set %Normal` the `?` command and the set-order JSON export read the
/// live rating and `Save` writes the number typed, for a typed, a default and a
/// spacing Line, so the reload reads the typed ratings. The live rating of a
/// spacing Line holds for the run, through a solve at another frequency and a
/// harmonics solve.
#[test]
fn question_mark_reads_the_live_rating_and_save_the_typed_one_after_set_pct_normal() {
    use crate::report::export::json::{Json, JsonOpts, parse_json};
    let mut dss = run(&[
        "new circuit.c basekv=12.47 bus1=src pu=1.0",
        "new line.t bus1=src bus2=e normamps=530 emergamps=795 length=1 units=mi",
        "new line.u bus1=src bus2=f length=1 units=mi",
        &format!("new wiredata.w {WIRE} normamps=530 emergamps=795"),
        "new linespacing.sp nconds=3 nphases=3 units=ft x=[-4 0 4] h=[28 28 28]",
        "new line.s bus1=src bus2=a spacing=sp wires=[w w w] length=1 units=mi",
        "new load.l bus1=a kw=1000 kv=12.47",
        "set voltagebases=[12.47]",
        "calcv",
        "set %normal=80",
    ]);
    for (what, want) in [
        ("line.t.normamps", "636"),
        ("line.t.emergamps", "795"),
        ("line.u.normamps", "480"),
        ("line.u.emergamps", "600"),
        ("line.s.normamps", "636"),
        ("line.s.emergamps", "795"),
    ] {
        assert_eq!(query(&mut dss, what), want, "{what}");
    }
    let json = dss
        .obj_to_json("line.t", JsonOpts::NONE)
        .expect("line json");
    let Json::Obj(members) = parse_json(&json).expect("JSON") else {
        panic!("not an object: {json}");
    };
    let normamps = members
        .iter()
        .find(|(k, _)| k == "NormAmps")
        .map(|(_, v)| v);
    assert_eq!(normamps, Some(&Json::Float(636.0)), "{json}");

    let saved = Saved::new(&mut dss, "pct_normal");
    assert_eq!(
        saved.ratings("Line.t"),
        ["NormAmps=530", "EmergAmps=795"],
        "{}",
        saved.record("Line.t")
    );
    for l in ["Line.u", "Line.s"] {
        assert_eq!(
            saved.ratings(l),
            Vec::<String>::new(),
            "{}",
            saved.record(l)
        );
    }
    let mut back = saved.reload();
    for (what, want) in [
        ("line.t.normamps", "530"),
        ("line.t.emergamps", "795"),
        ("line.u.normamps", "400"),
        ("line.u.emergamps", "600"),
        ("line.s.normamps", "530"),
        ("line.s.emergamps", "795"),
    ] {
        assert_eq!(query(&mut back, what), want, "{what} after the reload");
    }
    drop(saved);

    for (step, commands) in [
        ("a Solve", &["solve"][..]),
        ("a harmonics Solve", &["solve mode=harmonics"]),
        (
            "a Solve at 50 Hz",
            &["set mode=snapshot", "set frequency=50", "solve"],
        ),
    ] {
        for c in commands {
            dss.command(c);
        }
        assert!(dss.errors().is_empty(), "{step}: {:?}", dss.error_texts());
        assert_eq!(query(&mut dss, "line.s.normamps"), "636", "after {step}");
        assert_eq!(query(&mut dss, "line.s.emergamps"), "795", "after {step}");
    }
}

/// A rating typed not set survives a JSON export and import as `null`, a
/// seasonal array included, and the import takes the season count from the
/// array. A Line on a linecode writes the linecode's ratings, and a Line nobody
/// rated has no rating key and reloads the class default.
#[test]
fn a_json_round_trip_keeps_a_typed_rating_that_is_not_set() {
    use crate::report::export::json::{Json, JsonOpts, parse_json};
    let dss = run(&[
        "new circuit.c basekv=12.47 bus1=src pu=1.0",
        "new linecode.lc1 nphases=3 r1=0.1 x1=0.3 r0=0.3 x0=0.9 units=mi",
        "new linecode.lcn nphases=3 r1=0.1 x1=0.3 r0=0.3 x0=0.9 units=mi normamps=none",
        "new line.a bus1=src bus2=a normamps=none length=1 units=mi",
        "new line.b bus1=src bus2=b normamps=0 length=1 units=mi",
        "new line.c bus1=src bus2=c normamps=-2 length=1 units=mi",
        "new line.d bus1=src bus2=d ratings=[none] length=1 units=mi",
        "new line.e bus1=src bus2=e seasons=2 ratings=[0 -2] length=1 units=mi",
        "new line.s bus1=src bus2=s seasons=2 ratings=[400 500] length=1 units=mi",
        "new line.f bus1=src bus2=f linecode=lc1 length=1 units=mi",
        "new line.n bus1=src bus2=g linecode=lcn length=1 units=mi",
        "new line.g bus1=src bus2=h length=1 units=mi",
    ]);
    let member = |obj: &str, key: &str| -> Option<Json> {
        let text = dss.obj_to_json(obj, JsonOpts::NONE).expect("json");
        match parse_json(&text).expect("JSON") {
            Json::Obj(m) => m.into_iter().find(|(k, _)| k == key).map(|(_, v)| v),
            other => panic!("not an object: {other:?}"),
        }
    };
    let null = Some(Json::Null);
    let float = |v: f64| Some(Json::Float(v));
    for (obj, key, want) in [
        ("line.a", "NormAmps", null.clone()),
        ("line.b", "NormAmps", null.clone()),
        ("line.c", "NormAmps", null.clone()),
        ("line.d", "Ratings", Some(Json::Arr(vec![Json::Null]))),
        (
            "line.e",
            "Ratings",
            Some(Json::Arr(vec![Json::Null, Json::Null])),
        ),
        (
            "line.f",
            "Ratings",
            Some(Json::Arr(vec![Json::Float(400.0)])),
        ),
        ("line.f", "NormAmps", float(400.0)),
        ("line.f", "EmergAmps", float(600.0)),
        ("line.n", "NormAmps", None),
        ("line.g", "NormAmps", None),
        ("line.g", "EmergAmps", None),
        ("line.g", "Ratings", None),
    ] {
        assert_eq!(member(obj, key), want, "{obj} {key}");
    }

    let text = dss.circuit_to_json(JsonOpts::NONE).expect("circuit json");
    let mut back = Dss::new();
    back.circuit_from_json(&text).expect("JSON load");
    assert!(back.errors().is_empty(), "{:?}", back.error_texts());
    for (what, want) in [
        ("line.a.normamps", "none"),
        ("line.b.normamps", "none"),
        ("line.c.normamps", "none"),
        ("line.d.ratings", "[ none]"),
        ("line.e.seasons", "2"),
        ("line.e.ratings", "[ none none]"),
        ("line.s.seasons", "2"),
        ("line.s.ratings", "[ 400 500]"),
        ("line.f.normamps", "400"),
        ("line.f.ratings", "[ 400]"),
        ("line.n.normamps", "none"),
        ("line.g.normamps", "400"),
        ("line.g.ratings", "[ 400]"),
    ] {
        assert_eq!(query(&mut back, what), want, "{what} after the JSON import");
    }
}

/// `like=` copies the typed value of exactly the ratings the class copies, so
/// `Save` of the copy writes only those, on the conductors, LineCode,
/// LineGeometry, Line, Fault, GICTransformer, Capacitor and Reactor. Each saved
/// deck reloads the copies as they read in the session.
#[test]
fn a_like_copy_saves_only_the_typed_ratings_it_holds() {
    let mut lines = vec![
        "new circuit.c basekv=12.47 bus1=src pu=1.0".to_string(),
        format!("new wiredata.w1 {WIRE} normamps=530 seasons=3 ratings=[400 500 600]"),
        "new wiredata.w2 like=w1".to_string(),
        "new line.l bus1=src bus2=a normamps=0 seasons=2 ratings=[0 500] length=1 units=mi"
            .to_string(),
        "new line.m bus1=src bus2=b like=l".to_string(),
        format!("new wiredata.wg {WIRE} normamps=530"),
        "new linegeometry.g1 nconds=3 nphases=3 cond=1 wire=wg x=-4 h=28 units=ft cond=2 wire=wg \
         x=0 h=28 units=ft cond=3 wire=wg x=4 h=28 units=ft normamps=500"
            .to_string(),
        "new linegeometry.g2 like=g1".to_string(),
    ];
    let mut queries: Vec<String> = [
        "wiredata.w2.normamps",
        "wiredata.w2.emergamps",
        "wiredata.w2.seasons",
        "wiredata.w2.ratings",
        "line.m.normamps",
        "line.m.emergamps",
        "line.m.ratings",
        "linegeometry.g2.normamps",
        "linegeometry.g2.emergamps",
    ]
    .map(String::from)
    .to_vec();
    let mut records = vec![
        ("WireData.w2".to_string(), vec!["NormAmps=530"]),
        ("Line.m".to_string(), vec!["NormAmps=0", "Ratings=[ 0 500]"]),
        ("LineGeometry.g2".to_string(), vec!["NormAmps=500"]),
    ];
    for (cls, class, def) in [
        ("linecode", "LineCode", "nphases=3 r1=0.1 x1=0.2 units=mi"),
        ("cndata", "CNData", CN),
        ("tsdata", "TSData", TS),
        ("fault", "Fault", "bus1=src"),
        ("gictransformer", "GICTransformer", "bush=src"),
        ("capacitor", "Capacitor", "bus1=src kvar=100 kv=12.47"),
        ("reactor", "Reactor", "bus1=src kvar=100 kv=12.47"),
    ] {
        lines.extend([
            format!("new {cls}.f {def}"),
            format!("new {cls}.r {def} normamps=60 emergamps=80"),
            format!("new {cls}.g {def} normamps=50 emergamps=70"),
            format!("edit {cls}.g like=f"),
            format!("new {cls}.h {def} normamps=50 emergamps=70"),
            format!("edit {cls}.h like=r"),
        ]);
        for obj in ["g", "h"] {
            queries.push(format!("{cls}.{obj}.normamps"));
            queries.push(format!("{cls}.{obj}.emergamps"));
        }
        records.push((format!("{class}.g"), vec![]));
        records.push((format!("{class}.h"), vec!["NormAmps=60", "EmergAmps=80"]));
    }
    let mut dss = run_owned(&lines);
    let before: Vec<String> = queries.iter().map(|q| query(&mut dss, q)).collect();
    assert_eq!(
        before[7..9],
        ["500", "795"],
        "the geometry copy keeps the typed normal rating, the emergency one from its wires"
    );
    let saved = Saved::new(&mut dss, "like_copy");
    for (obj, want) in &records {
        assert_eq!(saved.ratings(obj), *want, "{}", saved.record(obj));
    }
    let mut back = saved.reload();
    for (q, b) in queries.iter().zip(&before) {
        assert_eq!(&query(&mut back, q), b, "{q} after the reload");
    }
}

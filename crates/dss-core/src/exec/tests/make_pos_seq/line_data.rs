//! A Line is what its data define: a spacing named again after a miss, after
//! a `linecode=`, a `switch=yes` or a matrix, another spacing with as many
//! positions, a conductor edit and a Line no solve has built solve, reduce,
//! save and load back as a twin built with the same data from the start, and
//! MakePosSequence reduces what the data define at the base frequency
//! whatever the last build left.

use super::assert_refused;
use crate::elements::pos_seq::balance;
use crate::exec::tests::common::query;
use crate::exec::*;
use num_complex::Complex64;

const WIRE: &str = "new wiredata.w runits=m gmrunits=m radunits=m rac=0.0003 gmrac=0.005 \
                    radius=0.01 normamps=400";
const THIN: &str = "new wiredata.t runits=m gmrunits=m radunits=m rac=0.0009 gmrac=0.003 \
                    radius=0.006 normamps=200";
const SPACING: &str = "new linespacing.sp nconds=3 nphases=3 x=[-1 0 1] h=[10 10 10] units=m";
/// Another spacing with as many positions as `sp`.
const SPACING2: &str = "new linespacing.sp2 nconds=3 nphases=3 x=[-1.5 0 1.5] h=[10 10 10] units=m";
/// A spacing with a fourth position, for a neutral.
const SPACING4: &str = "new linespacing.sp4 nconds=4 nphases=3 x=[-1 0 1 0] h=[10 10 10 8] units=m";
const GEOMETRY: &str = "new linegeometry.g nconds=3 nphases=3 cond=1 wire=w x=-1 h=10 units=m \
                        cond=2 wire=w x=0 h=10 units=m cond=3 wire=w x=1 h=10 units=m";
const LINECODES: [&str; 2] = [
    "new linecode.lc nphases=3 r1=0.3 x1=0.6 r0=0.9 x0=1.8 c1=10 c0=5 units=km",
    "new linecode.lcmi nphases=3 r1=0.3 x1=0.6 r0=0.9 x0=1.8 c1=10 c0=5 units=mi",
];
const CABLES: [&str; 5] = [
    "new cndata.cn runits=mi radunits=in gmrunits=in rac=0.41 diam=0.567 gmrac=0.2052 \
     normamps=260 epsr=2.3 ins=0.22 diains=1.06 diacable=1.29 k=13 diastrand=0.0641 \
     gmrstrand=0.02496 rstrand=14.8722",
    "new cndata.cns runits=mi radunits=in gmrunits=in rac=0.8 diam=0.4 gmrac=0.15 \
     normamps=150 epsr=2.3 ins=0.22 diains=0.9 diacable=1.1 k=10 diastrand=0.0641 \
     gmrstrand=0.02496 rstrand=14.8722",
    "new tsdata.ts runits=mi radunits=in gmrunits=in rac=0.97 diam=0.368 gmrac=0.1332 \
     normamps=165 epsr=2.3 ins=0.22 diains=0.82 diacable=1.06 diashield=0.88 \
     tapelayer=0.005 tapelap=20",
    "new tsdata.tss runits=mi radunits=in gmrunits=in rac=1.4 diam=0.3 gmrac=0.1 \
     normamps=120 epsr=2.3 ins=0.22 diains=0.7 diacable=0.95 diashield=0.75 \
     tapelayer=0.005 tapelap=20",
    "new linespacing.ug nconds=3 nphases=3 x=[-0.5 0 0.5] h=[-4 -4 -4] units=ft",
];
const LOAD: &str = "new load.ld bus1=c phases=3 kv=12.47 kw=1000 pf=0.9 model=2";

/// The whole message of a `spacing=` naming no LineSpacing that exists.
const MISS: &str = "Line.l.Spacing: LineSpacing object \"nonexistent\" not found.";

/// The reason a Line on `sp4` with no conductor in its slots is refused.
const NO_PHASE_CONDUCTORS: &str = "The number of valid phase conductors (not \"None\") in \
     Line.l is different than the number defined in its spacing. In this case, you must set \
     the phases parameter of the line to the correct number (phases=0).";

/// `cmd` on `dss`, adding no error.
fn ok(dss: &mut Dss, cmd: &str, what: &str) {
    let n = dss.errors().len();
    dss.command(cmd);
    assert!(
        dss.errors().len() == n,
        "{what}: `{cmd}`: {:?}",
        &dss.errors()[n..]
    );
}

/// `cmd` on `dss` and the text and code of each error it adds.
fn errors_of(dss: &mut Dss, cmd: &str) -> Vec<(String, Option<u32>)> {
    let n = dss.errors().len();
    dss.command(cmd);
    dss.errors()[n..]
        .iter()
        .map(|e| (e.text().to_string(), e.code))
        .collect()
}

/// The stiff circuit `name` with the wires `w` and `t`, the cables `cn`,
/// `cns`, `ts` and `tss`, the spacings `sp`, `sp2`, `sp4` and `ug`, the
/// geometry `g` on `w`, the linecodes `lc` in km and `lcmi` in miles, the
/// Line `l` from `b` to `c` given by `line` and then by `length` (its length
/// and unit), and a constant-impedance load on `c`. The load makes a solve
/// one linear solve, so twins solve to the same bits.
fn deck_of(name: &str, line: &str, length: &str) -> Dss {
    let mut dss = balance::stiff(name);
    let line = format!("new line.l bus1=b bus2=c phases=3 {line} {length}");
    let catalog = [WIRE, THIN, SPACING, SPACING2, SPACING4, GEOMETRY];
    let tail = [line.as_str(), LOAD];
    for cmd in catalog
        .into_iter()
        .chain(LINECODES)
        .chain(CABLES)
        .chain(tail)
    {
        ok(&mut dss, cmd, name);
    }
    dss
}

/// [`deck_of`] with the Line 1 km long.
fn deck(name: &str, line: &str) -> Dss {
    deck_of(name, line, "length=1 units=km")
}

/// The bits of a complex number, so two readings compare bit for bit.
fn bits(z: Complex64) -> (u64, u64) {
    (z.re.to_bits(), z.im.to_bits())
}

/// The bits of every node voltage, bus by bus.
fn voltages(dss: &Dss) -> Vec<(String, Vec<(u64, u64)>)> {
    let mut buses: Vec<_> = dss
        .all_bus_voltages()
        .into_iter()
        .map(|b| (b.name, b.node_v.into_iter().map(bits).collect()))
        .collect();
    buses.sort();
    buses
}

/// The bits of the Line's YPrim.
fn yprim(dss: &Dss) -> Vec<(u64, u64)> {
    let (_, y) = dss.element_yprim("Line.l").expect("the Line has a YPrim");
    y.into_iter().map(bits).collect()
}

/// The Line's properties a reduction writes, read back.
fn reduced(dss: &mut Dss) -> Vec<String> {
    ["phases", "r1", "x1", "c1", "normamps", "emergamps", "units"]
        .into_iter()
        .map(|p| query(dss, &format!("line.l.{p}")))
        .collect()
}

/// `got` and `twin`, both solved, hold one Line impedance and one solution:
/// the Line's YPrim and the node voltages bit for bit, then the Line
/// `makeposseq` writes and the node voltages of one more solve.
fn assert_twins(got: &mut Dss, twin: &mut Dss, what: &str) {
    for dss in [&*got, &*twin] {
        assert!(dss.circuit().unwrap().is_solved, "{what}: not solved");
    }
    assert_eq!(yprim(got), yprim(twin), "{what}: the Line's YPrim");
    assert_eq!(voltages(got), voltages(twin), "{what}: the node voltages");
    ok(got, "makeposseq", what);
    ok(twin, "makeposseq", what);
    let reduced_twin = reduced(twin);
    assert_eq!(reduced_twin[0], "1", "{what}: the reduced twin");
    assert_eq!(reduced(got), reduced_twin, "{what}: the reduced Line");
    ok(got, "solve", what);
    ok(twin, "solve", what);
    assert_eq!(
        voltages(got),
        voltages(twin),
        "{what}: the node voltages of the reduced circuit"
    );
}

/// `set voltagebases`, `calcvoltagebases` and `solve`, adding no error.
fn solve(dss: &mut Dss, what: &str) {
    for cmd in ["set voltagebases=[12.47]", "calcvoltagebases", "solve"] {
        ok(dss, cmd, what);
    }
}

/// The circuit `got` saves loads back, with no error, as the solved `twin`:
/// once solved, the Line's YPrim and the node voltages are the same bits.
/// Returns the circuit loaded back, solved.
fn assert_loads_back_as(got: &mut Dss, twin: &Dss, what: &str) -> Dss {
    let dir = std::env::temp_dir()
        .join("dss_line_data_save")
        .join(format!("{what}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.to_string_lossy().replace('\\', "/");
    ok(got, &format!("save circuit dir=\"{path}\""), what);
    let mut saved = Dss::new();
    ok(&mut saved, &format!("compile \"{path}/Master.dss\""), what);
    let _ = std::fs::remove_dir_all(&dir);
    let what = format!("{what}, loaded back from its Save");
    solve(&mut saved, &what);
    assert_eq!(yprim(&saved), yprim(twin), "{what}: the Line's YPrim");
    assert_eq!(
        voltages(&saved),
        voltages(twin),
        "{what}: the node voltages"
    );
    saved
}

/// A Line whose `spacing=` named no LineSpacing that exists and then named its
/// spacing again keeps its wires, so it is the Line that never missed, and so
/// is a Line given another spacing with as many positions, `sp2`, the Line
/// defined on `sp2`. With the miss and the restore before any solve or after
/// one, and with `sp2` before any solve, it solves to the twin's YPrim and
/// node voltages bit for bit, its Save loads back as the twin (the spacing
/// written before the wires), MakePosSequence reduces it
/// as it reduces the twin, and the reduced circuits solve alike. C.1 reads
/// 7174.376145 V on `sp` and 7173.692291 V on `sp2` (2026-10-23). The Line
/// stays on its spacing through the miss, so the restore is no model switch:
/// ratings given after the wires (500 A and 700 A) stay the Line's in the
/// solve, in the Line loaded back and in the reduced Line, where a model
/// switch would take the wires' 400 A and 600 A again.
#[test]
fn a_spacing_named_again_after_a_miss_keeps_the_wires() {
    const RATED: &str = " normamps=500 emergamps=700";
    for (tag, solved, spacing, ratings, c1_want) in [
        ("rsp", false, "sp", "", 7174.376145),
        ("rsps", true, "sp", "", 7174.376145),
        ("rsp2", false, "sp2", "", 7173.692291),
        ("rspa", false, "sp", RATED, 7174.376145),
        ("rspas", true, "sp", RATED, 7174.376145),
    ] {
        let mut got = deck(tag, &format!("spacing=sp wires=[w w w]{ratings}"));
        let mut twin = deck(tag, &format!("spacing={spacing} wires=[w w w]{ratings}"));
        if solved {
            solve(&mut got, tag);
            solve(&mut twin, tag);
        }
        if spacing == "sp" {
            assert_eq!(
                errors_of(&mut got, "edit line.l spacing=nonexistent"),
                [(MISS.to_string(), None)],
                "{tag}: the miss"
            );
        }
        ok(&mut got, &format!("edit line.l spacing={spacing}"), tag);
        assert_eq!(query(&mut got, "line.l.wires"), "[w, w, w]", "{tag}");
        solve(&mut got, tag);
        solve(&mut twin, tag);
        let c1 = balance::v1(&got, "c");
        assert!((c1 - c1_want).abs() < 5e-7, "{tag}: C.1 {c1}");
        let mut saved = assert_loads_back_as(&mut got, &twin, tag);
        if ratings == RATED {
            for (dss, what) in [(&mut got, "the Line"), (&mut saved, "the Line loaded back")] {
                let read = [
                    query(dss, "line.l.normamps"),
                    query(dss, "line.l.emergamps"),
                ];
                assert_eq!(read, ["500", "700"], "{tag}: the ratings of {what}");
            }
        }
        assert_twins(&mut got, &mut twin, tag);
        if ratings == RATED {
            let read = reduced(&mut got);
            assert_eq!(read[4..6], ["500", "700"], "{tag}: the reduced ratings");
        }
    }
}

/// A spacing with another number of positions, `sp4`, leaves the Line's
/// slots empty, so the Line holds no conductor and has no impedance to
/// reduce: before any solve or after one, `? line.l.wires` reads four empty
/// slots and MakePosSequence refuses the Line and changes nothing, and with
/// no solve before, the solve refuses it too (code 482).
#[test]
fn a_spacing_with_other_positions_leaves_the_line_no_conductors() {
    for (tag, solved) in [("sp4", false), ("sp4s", true)] {
        let mut dss = deck(tag, "spacing=sp wires=[w w w]");
        if solved {
            solve(&mut dss, tag);
        }
        ok(&mut dss, "edit line.l spacing=sp4", tag);
        assert_eq!(query(&mut dss, "line.l.wires"), "[, , , ]", "{tag}");
        assert_refused(
            &mut dss,
            tag,
            &format!("MakePosSequence: {NO_PHASE_CONDUCTORS} Nothing was changed."),
            None,
        );
        if !solved {
            assert_eq!(
                errors_of(&mut dss, "solve"),
                [(
                    format!("Error Encountered in Solve: {NO_PHASE_CONDUCTORS}"),
                    Some(482)
                )],
                "{tag}: the solve"
            );
        }
    }
}

/// A `spacing=` after a `linecode=` gives the spacing Line when the fetch
/// keeps a conductor at every phase position, since the Line's data then
/// define a spacing Line: with or without a solve before the two edits, and
/// with a linecode given in another length unit than the Line's, it solves
/// to the Line defined on the spacing and its wires bit for bit, loads back
/// from its Save as that Line, and reduces as it does. C.1 reads
/// 7174.376145 V (2026-10-23).
#[test]
fn a_spacing_after_a_linecode_gives_the_spacing_line() {
    for (tag, solved, code) in [
        ("lcsp", false, "lc"),
        ("lcsps", true, "lc"),
        ("lcmisp", false, "lcmi"),
    ] {
        let mut got = deck(tag, "spacing=sp wires=[w w w]");
        let mut twin = deck(tag, "spacing=sp wires=[w w w]");
        if solved {
            solve(&mut got, tag);
        }
        ok(&mut got, &format!("edit line.l linecode={code}"), tag);
        ok(&mut got, "edit line.l spacing=sp", tag);
        assert_eq!(query(&mut got, "line.l.linecode"), "", "{tag}");
        assert_eq!(query(&mut got, "line.l.wires"), "[w, w, w]", "{tag}");
        solve(&mut got, tag);
        solve(&mut twin, tag);
        let c1 = balance::v1(&got, "c");
        assert!((c1 - 7174.376145).abs() < 5e-7, "{tag}: C.1 {c1}");
        assert_loads_back_as(&mut got, &twin, tag);
        assert_twins(&mut got, &mut twin, tag);
    }
}

/// A `spacing=` that gives the Line its model saves after every property set
/// before it, so the save replays a `switch=yes`, or a matrix that followed a
/// `linecode=`, first, and the spacing and its wires then take the model as
/// they did in memory. After `switch=yes`, after the matrix (which leaves the
/// Line 1 long in no unit) and after the matrix with `units=km` given again,
/// the Line solves to its twin, the Line defined with the final data, bit for
/// bit at 60 Hz and at 180 Hz, its Save loads back as that twin at both
/// frequencies, and MakePosSequence reduces it as it reduces the twin. C.1
/// reads 7199.557832 V after the switch, 7199.532581 V after the matrix and
/// 7174.376145 V with `units=km`, as on r4133 (2026-10-23).
#[test]
fn a_spacing_that_gives_the_line_its_model_saves_after_what_came_before() {
    const LINECODE: &str = "edit line.l linecode=lc";
    const MATRIX: &str = "edit line.l rmatrix=[0.3 | 0.09 0.3 | 0.09 0.09 0.3] \
                          xmatrix=[1.0 | 0.4 1.0 | 0.4 0.4 1.0] \
                          cmatrix=[3.4 | -1.1 3.4 | -1.1 -1.1 3.4]";
    for (tag, edits, twin_line, twin_length, c1_want) in [
        (
            "swsp",
            &["edit line.l switch=yes"][..],
            "switch=yes spacing=sp wires=[w w w]",
            "",
            7199.557832,
        ),
        (
            "mxsp",
            &[LINECODE, MATRIX][..],
            "spacing=sp wires=[w w w]",
            "length=1",
            7199.532581,
        ),
        (
            "mxkmsp",
            &[LINECODE, MATRIX, "edit line.l units=km"][..],
            "spacing=sp wires=[w w w]",
            "length=1 units=km",
            7174.376145,
        ),
    ] {
        let mut got = deck(tag, "spacing=sp wires=[w w w]");
        let mut twin = deck_of(tag, twin_line, twin_length);
        for &cmd in edits {
            ok(&mut got, cmd, tag);
        }
        ok(&mut got, "edit line.l spacing=sp", tag);
        solve(&mut got, tag);
        solve(&mut twin, tag);
        let c1 = balance::v1(&got, "c");
        assert!((c1 - c1_want).abs() < 5e-7, "{tag}: C.1 {c1}");
        let mut saved = assert_loads_back_as(&mut got, &twin, tag);
        for dss in [&mut got, &mut twin, &mut saved] {
            ok(dss, "set frequency=180", tag);
            ok(dss, "solve", tag);
        }
        for (dss, what) in [(&got, "the Line"), (&saved, "the Line loaded back")] {
            let at = format!("{tag}: {what} at 180 Hz");
            assert_eq!(yprim(dss), yprim(&twin), "{at}: the Line's YPrim");
            assert_eq!(voltages(dss), voltages(&twin), "{at}: the node voltages");
        }
        for dss in [&mut got, &mut twin] {
            ok(dss, "set frequency=60", tag);
            ok(dss, "solve", tag);
        }
        assert_twins(&mut got, &mut twin, tag);
    }
}

/// An open item, not a rule: a `spacing=` after a `linecode=` whose fetch
/// keeps conductors at some phase positions and none at another (`wires=[w
/// w none]` given before the linecode) leaves the Line on the linecode's
/// impedance while `? line.l.linecode` reads empty, so it solves and reduces
/// as the Line defined by the linecode, bit for bit. Its data, the spacing
/// and two phase conductors, define no three-phase Line: its twin defined
/// with those data is refused by `CalcVoltageBases` and the solve, and its
/// Save writes no impedance and does not load back. The rule for a
/// `spacing=` without a full set of phase conductors is queued, and this pin
/// holds the present behaviour until it lands. Beside it, the same order
/// with every phase conductor given gives the spacing Line, the rule
/// `a_spacing_after_a_linecode_gives_the_spacing_line` pins.
#[test]
fn a_partial_conductor_set_keeps_the_linecode_impedance() {
    for (tag, wires, read, twin_line) in [
        ("lcpart", "[w w none]", "[w, w, ]", "linecode=lc"),
        ("lcfull", "[w w w]", "[w, w, w]", "spacing=sp wires=[w w w]"),
    ] {
        let mut got = deck(tag, "spacing=sp wires=[w w w]");
        let mut twin = deck(tag, twin_line);
        ok(&mut got, &format!("edit line.l wires={wires}"), tag);
        ok(&mut got, "edit line.l linecode=lc", tag);
        ok(&mut got, "edit line.l spacing=sp", tag);
        assert_eq!(query(&mut got, "line.l.linecode"), "", "{tag}");
        assert_eq!(query(&mut got, "line.l.wires"), read, "{tag}");
        solve(&mut got, tag);
        solve(&mut twin, tag);
        assert_twins(&mut got, &mut twin, tag);
    }
}

/// A `wires=` edit of a defined Line gives the Line of those wires: the next
/// build makes its impedance from them whether a solve built the old one or
/// not. Edited before any solve or after one, it solves to the twin defined
/// with those wires bit for bit and reduces as that twin does, and edited
/// after a solve and reduced at once, it is reduced from those wires too.
/// C.1 reads 7146.604713 V with `t` against 7174.376145 V with `w`
/// (2026-10-23).
#[test]
fn a_wires_edit_gives_the_line_of_those_wires() {
    for (tag, solved_before, solve_after) in [
        ("wre", false, true),
        ("wres", true, true),
        ("wresm", true, false),
    ] {
        let mut got = deck(tag, "spacing=sp wires=[w w w]");
        let mut twin = deck(tag, "spacing=sp wires=[t t t]");
        if solved_before {
            solve(&mut got, tag);
            solve(&mut twin, tag);
        }
        ok(&mut got, "edit line.l wires=[t t t]", tag);
        if solve_after {
            solve(&mut got, tag);
            solve(&mut twin, tag);
            let c1 = balance::v1(&got, "c");
            assert!((c1 - 7146.604713).abs() < 5e-7, "{tag}: C.1 {c1}");
            assert_twins(&mut got, &mut twin, tag);
        } else {
            ok(&mut got, "makeposseq", tag);
            ok(&mut twin, "makeposseq", tag);
            assert_eq!(
                reduced(&mut got),
                reduced(&mut twin),
                "{tag}: the reduced Line"
            );
            solve(&mut got, tag);
            solve(&mut twin, tag);
            assert_eq!(voltages(&got), voltages(&twin), "{tag}: the node voltages");
        }
    }
}

/// A `conductors=`, `cncables=` or `tscables=` edit after a solve gives the
/// Line of those conductors, as a `wires=` edit does: the next solve builds
/// its impedance from them, so it solves to the twin defined with them bit
/// for bit and reduces as that twin does.
#[test]
fn a_conductor_list_edit_after_a_solve_gives_the_line_of_those_conductors() {
    for (tag, line, edit, twin_line) in [
        (
            "cds",
            "spacing=sp conductors=[wiredata.w wiredata.w wiredata.w]",
            "conductors=[wiredata.t wiredata.t wiredata.t]",
            "spacing=sp conductors=[wiredata.t wiredata.t wiredata.t]",
        ),
        (
            "cns",
            "spacing=ug cncables=[cn cn cn]",
            "cncables=[cns cns cns]",
            "spacing=ug cncables=[cns cns cns]",
        ),
        (
            "tss",
            "spacing=ug tscables=[ts ts ts]",
            "tscables=[tss tss tss]",
            "spacing=ug tscables=[tss tss tss]",
        ),
    ] {
        let mut got = deck(tag, line);
        let mut twin = deck(tag, twin_line);
        solve(&mut got, tag);
        let before = yprim(&got);
        ok(&mut got, &format!("edit line.l {edit}"), tag);
        solve(&mut got, tag);
        solve(&mut twin, tag);
        assert_ne!(yprim(&got), before, "{tag}: the edit moves the YPrim");
        assert_twins(&mut got, &mut twin, tag);
    }
}

/// A geometry or spacing Line that no solve has built is reduced from its
/// conductors, as the Line that a solve built before the reduction: the
/// reduced Line and the node voltages of the reduced circuit are the same
/// bits. The default impedance the Line holds before a build is no part of
/// the reduction, and the reduced Line carries the ratings of its weakest
/// phase conductor (`t`, 200 A and 300 A, on `[t w w]`).
#[test]
fn a_line_no_solve_built_is_reduced_from_its_conductors() {
    for (tag, line) in [
        ("nbs", "spacing=sp wires=[w w w]"),
        ("nbm", "spacing=sp wires=[t w w]"),
        ("nbg", "geometry=g"),
    ] {
        let mut got = deck(tag, line);
        let mut twin = deck(tag, line);
        solve(&mut twin, tag);
        ok(&mut got, "makeposseq", tag);
        ok(&mut twin, "makeposseq", tag);
        let reduced_got = reduced(&mut got);
        assert_ne!(reduced_got[1], "0.058", "{tag}: the default r1");
        if tag == "nbm" {
            assert_eq!(reduced_got[4..6], ["200", "300"], "{tag}: the ratings");
        }
        assert_eq!(reduced_got, reduced(&mut twin), "{tag}: the reduced Line");
        solve(&mut got, tag);
        solve(&mut twin, tag);
        assert_eq!(voltages(&got), voltages(&twin), "{tag}: the node voltages");
    }
}

/// MakePosSequence reduces a geometry or spacing Line from the impedance its
/// present data give at the base frequency, per unit of its own length,
/// whatever the last build left. A spacing with as many positions, a new
/// length or a new length unit given after a solve, a solve at 120 Hz and a
/// harmonics study before the reduction each reduce to the twin: the Line
/// defined with the final data, solved at the base frequency and reduced. The reduced Line and the node voltages of the
/// reduced circuit are the same bits. The solve between such an edit and the
/// reduction is no part of the pin.
#[test]
fn a_line_is_reduced_from_its_present_data_at_the_base_frequency() {
    let spacing = "spacing=sp wires=[w w w]";
    let (km, geometry) = ("length=1 units=km", "geometry=g");
    for (tag, line, before, twin_line, twin_length) in [
        (
            "rsp2",
            spacing,
            "edit line.l spacing=sp2",
            "spacing=sp2 wires=[w w w]",
            km,
        ),
        (
            "rlen",
            spacing,
            "edit line.l length=2",
            spacing,
            "length=2 units=km",
        ),
        (
            "runit",
            spacing,
            "edit line.l units=mi",
            spacing,
            "length=1 units=mi",
        ),
        (
            "runitg",
            geometry,
            "edit line.l units=mi",
            geometry,
            "length=1 units=mi",
        ),
        ("rf120", spacing, "set frequency=120", spacing, km),
        ("rf120g", geometry, "set frequency=120", geometry, km),
        ("rharm", spacing, "solve mode=harmonics", spacing, km),
        ("rharmg", geometry, "solve mode=harmonics", geometry, km),
    ] {
        let mut got = deck(tag, line);
        let mut twin = deck_of(tag, twin_line, twin_length);
        solve(&mut got, tag);
        solve(&mut twin, tag);
        ok(&mut got, before, tag);
        match before {
            "set frequency=120" => {
                ok(&mut got, "solve", tag);
                ok(&mut got, "set frequency=60", tag);
            }
            "solve mode=harmonics" => ok(&mut got, "set mode=snap", tag),
            _ => {}
        }
        ok(&mut got, "makeposseq", tag);
        ok(&mut twin, "makeposseq", tag);
        let reduced_twin = reduced(&mut twin);
        assert_eq!(reduced_twin[0], "1", "{tag}: the reduced twin");
        assert_eq!(reduced(&mut got), reduced_twin, "{tag}: the reduced Line");
        ok(&mut got, "solve", tag);
        ok(&mut twin, "solve", tag);
        assert_eq!(voltages(&got), voltages(&twin), "{tag}: the node voltages");
    }
}

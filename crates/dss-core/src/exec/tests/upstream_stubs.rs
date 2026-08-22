//! The r4133 **upstream-stub** property rows (`PropFlags::UPSTREAM_STUB`,
//! R4133_PROPS_PLAN RP1.1): Generator `Rneut`/`Xneut` and Sensor `Action`.
//!
//! These are properties EPRI r4133 still *registers* — so they occupy a display
//! slot and `AllPropertyNames` reports them — but no longer implements. The Edit
//! loop stores the raw parse string like it does for every property
//! (`Version8/Source/PCElements/generator.pas:625`,
//! `Version8/Source/Meters/Sensor.pas:253`), and the property's own arm then
//! either logs a soft `DoSimpleMsg` (`generator.pas:651-652`, messages
//! 5611/5612) or does nothing at all (`TSensorObj.Set_Action` is an empty body,
//! `Sensor.pas:850-854`). Nothing is recalculated and no Y is touched — for
//! Generator the neutral stamping the two once fed is commented-out dead text
//! (`generator.pas:1294-1303`).
//!
//! This module pins that contract from the outside, on the script surface: the
//! stored-and-echoed value, the messages (present for one class, absent for the
//! other), the display slots the census `shape.txt` gap is closed at, the
//! `MakeLike` copy, and — the part that makes "stub" a claim rather than a label
//! — that a write moves neither the assembled Y nor the solution.

use crate::exec::Dss;

use super::common::{dss_with_circuit, query};

/// A two-bus deck with one wye generator, solved. `stubs` is appended to the
/// generator's `New` line, so the only difference between the two runs the test
/// compares is whether the upstream-stub properties were written at all — every
/// other command, and the whole solve path, is identical.
fn solved_deck(stubs: &str) -> Dss {
    let mut dss = Dss::new();
    let gen_line = format!(
        "New Generator.g1 bus1=b1 phases=3 kv=12.47 kw=300 pf=0.9 model=1 conn=wye {stubs}"
    );
    for line in [
        "Clear",
        "New Circuit.stubs basekv=12.47 phases=3 bus1=sourcebus mvasc3=200 mvasc1=210",
        "New Line.l1 bus1=sourcebus bus2=b1 phases=3 r1=0.30 x1=0.90 r0=0.9 x0=2.7 c1=3.0 c0=1.5 length=2 units=km",
        "New Load.ld1 bus1=b1 phases=3 kv=12.47 kw=800 pf=0.95 model=1",
        &gen_line,
        "Set voltagebases=[12.47]",
        "Calcvoltagebases",
        "Solve",
    ] {
        dss.command(line);
        // The stub writes log 5611/5612 by design; nothing else may.
        assert!(
            dss.errors()
                .iter()
                .all(|e| matches!(e.code, Some(5611 | 5612))),
            "`{line}` -> {:?}",
            dss.errors()
        );
    }
    assert!(dss.circuit().unwrap().is_solved, "the deck must solve");
    dss
}

/// The assembled system Y as coordinate-sorted `(row, col, re, im)` triplets.
type YTriplets = Vec<(usize, usize, f64, f64)>;
/// Solved node voltages as `(re, im)` pairs.
type NodeVoltages = Vec<(f64, f64)>;

/// The assembled system Y as a coordinate-sorted triplet list, plus the solved
/// node voltages — the two states an impedance property would move.
fn y_and_voltages(dss: &mut Dss) -> (YTriplets, NodeVoltages) {
    let (_, mut trip) = dss.system_y_csc().expect("the solved circuit has a Y");
    trip.sort_by_key(|(r, c, _)| (*r, *c));
    let y = trip
        .into_iter()
        .map(|(r, c, z)| (r, c, z.re, z.im))
        .collect();
    let v = dss
        .circuit()
        .unwrap()
        .solution
        .node_v
        .iter()
        .map(|z| (z.re, z.im))
        .collect();
    (y, v)
}

/// The message text and code each `Rneut`/`Xneut` write answers with
/// (`generator.pas:651-652`), and the fact that they are `DoSimpleMsg`
/// record-and-continue, not `DoErrorMsg`.
#[test]
fn generator_neutral_stubs_store_echo_and_log_their_message() {
    let mut dss = dss_with_circuit();
    dss.command("New Generator.g1 bus1=b1 phases=3 kv=12.47 kw=100");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    // `InitPropertyValues[14]`/`[15] := '0'` (`generator.pas:2567-2568`).
    assert_eq!(query(&mut dss, "Generator.g1.Rneut"), "0");
    assert_eq!(query(&mut dss, "Generator.g1.Xneut"), "0");

    let seen = dss.errors().len();
    dss.command("Edit Generator.g1 rneut=1.25 xneut=-1");
    let logged: Vec<(Option<u32>, String, bool)> = dss.errors()[seen..]
        .iter()
        .map(|e| (e.code, e.message.clone(), e.abort))
        .collect();
    assert_eq!(
        logged,
        [
            (
                Some(5611),
                "Rneut property has been deleted. Use external impedance.".to_string(),
                false,
            ),
            (
                Some(5612),
                "Xneut property has been deleted. Use external impedance.".to_string(),
                false,
            ),
        ],
        "each write must answer with its own r4133 message, in write order, as \
         `DoSimpleMsg` (record and continue) — not an abort and not a parse error"
    );

    // Stored verbatim despite the message: r4133 assigns `PropertyValue[]`
    // *before* the arm runs (`generator.pas:625`), so the echo is the raw
    // string — `-1` is not normalized, and neither value is parsed as a number.
    assert_eq!(query(&mut dss, "Generator.g1.Rneut"), "1.25");
    assert_eq!(query(&mut dss, "Generator.g1.Xneut"), "-1");

    // Not a number: the store is textual, so a non-numeric write round-trips too
    // (upstream would store it the same way — the arm never parses).
    let seen = dss.errors().len();
    dss.command("Edit Generator.g1 rneut=notanumber");
    assert_eq!(query(&mut dss, "Generator.g1.Rneut"), "notanumber");
    let codes: Vec<Option<u32>> = dss.errors()[seen..].iter().map(|e| e.code).collect();
    assert_eq!(
        codes,
        [Some(5611)],
        "a stub write is never a parse error — only its own message, got {:?}",
        &dss.errors()[seen..]
    );
}

/// Sensor `Action` is the silent half of the mechanism: stored and echoed with
/// no message at all, because `TSensorObj.Set_Action` has an empty body
/// (`Sensor.pas:850-854`).
#[test]
fn sensor_action_stores_and_echoes_silently() {
    let mut dss = dss_with_circuit();
    dss.command("New Line.l1 bus1=b1 bus2=b2 phases=3 length=1");
    dss.command("New Sensor.s1 element=Line.l1 terminal=1 kvbase=12.47");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    // `InitPropertyValues[13] := ''` (`Sensor.pas:807`).
    assert_eq!(query(&mut dss, "Sensor.s1.Action"), "");

    dss.command("Edit Sensor.s1 action=SQERROR");
    assert!(
        dss.errors().is_empty(),
        "the Sensor stub carries no `stub_msg`, so the write must be silent: {:?}",
        dss.errors()
    );
    assert_eq!(
        query(&mut dss, "Sensor.s1.Action"),
        "SQERROR",
        "the raw parse string is stored, case and all (`Sensor.pas:253`)"
    );
}

/// The claim the flag's name makes, checked against engine state: writing the
/// Generator stubs moves neither the assembled Y nor the solved voltages, on a
/// deck where a real neutral impedance on a wye generator would move both.
///
/// Two arms, both compared **exactly** (bit-for-bit), because the write is
/// supposed to be a string assignment and any movement at all is the bug:
///
/// 1. **Two independent runs** of the same deck, one with `rneut=`/`xneut=` on
///    the generator's `New` line and one without. Each is solved once from a
///    fresh `Dss`, so the comparison is free of the iteration-path drift a
///    re-`Solve` from an already-converged state introduces (that drift is ~1e-8
///    relative here and has nothing to do with these properties).
/// 2. **An in-place edit** of the solved circuit: Y must not move even as the
///    edit runs the class's `EndEdit`/`RecalcElementData` path, which is what a
///    live impedance property would ride into `CalcYPrim`.
#[test]
fn generator_neutral_stubs_move_neither_y_nor_the_solution() {
    let mut plain = solved_deck("");
    let mut stubbed = solved_deck("rneut=12.5 xneut=-1");

    let (y_plain, v_plain) = y_and_voltages(&mut plain);
    let (y_stubbed, v_stubbed) = y_and_voltages(&mut stubbed);
    assert!(
        y_plain.len() > 10 && v_plain.len() > 3,
        "the deck must actually have a Y and a solution to compare"
    );
    assert_eq!(
        y_stubbed, y_plain,
        "a deck that writes the upstream stubs must assemble a bit-identical \
         system Y (a real neutral impedance on a wye generator would not)"
    );
    assert_eq!(
        stubbed.circuit().unwrap().solution.iteration,
        plain.circuit().unwrap().solution.iteration,
        "…and converge in the same number of iterations"
    );
    assert_eq!(v_stubbed, v_plain, "…to bit-identical node voltages");
    // The values are still there — the no-op is in the engine, not in the store.
    assert_eq!(query(&mut stubbed, "Generator.g1.Rneut"), "12.5");
    assert_eq!(query(&mut stubbed, "Generator.g1.Xneut"), "-1");

    // Arm 2: the same write as a live edit of a solved circuit.
    let (y_before, _) = y_and_voltages(&mut plain);
    plain.command("Edit Generator.g1 rneut=999 xneut=999");
    let (y_after, _) = y_and_voltages(&mut plain);
    assert_eq!(
        y_after, y_before,
        "editing an upstream stub on a solved circuit must not move Y"
    );
}

/// Display slots. r4133 registers the two Generator stubs immediately after
/// `conn` (`generator.pas:440-443`) and `action` as the last of `TSensor`'s own
/// properties (`Sensor.pas:183`), which is what makes the two oracle tables 50
/// and 16 names long — the `oracle_count` the vendored census records in
/// `tests/corpus/props_r4133/shape.txt`. Closing that gap is RP1.1's deliverable,
/// so the positions are pinned, not just the presence.
#[test]
fn upstream_stub_display_slots_follow_r4133_registration() {
    let mut dss = dss_with_circuit();
    dss.command("New Generator.g1 bus1=b1 phases=3 kv=12.47 kw=100");
    dss.command("New Line.l1 bus1=b1 bus2=b2 phases=3 length=1");
    dss.command("New Sensor.s1 element=Line.l1 terminal=1 kvbase=12.47");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let names = |dss: &mut Dss, full: &str| -> Vec<String> {
        dss.element_properties(full)
            .expect("element exists")
            .into_iter()
            .map(|(n, _)| n)
            .collect()
    };

    let g = names(&mut dss, "Generator.g1");
    assert_eq!(
        g.len(),
        50,
        "r4133's Generator table is 50 names long (shape.txt: oracle_count=50)"
    );
    assert_eq!(
        &g[13..18],
        [
            "DispValue".to_string(),
            "Conn".to_string(),
            "Rneut".to_string(),
            "Xneut".to_string(),
            "Status".to_string(),
        ],
        "Rneut/Xneut sit at display slots 16/17, between Conn and Status"
    );

    let s = names(&mut dss, "Sensor.s1");
    assert_eq!(
        s.len(),
        16,
        "r4133's Sensor table is 16 names long (shape.txt: oracle_count=16)"
    );
    assert_eq!(
        &s[11..15],
        [
            "Weight".to_string(),
            "Action".to_string(),
            "BaseFreq".to_string(),
            "Enabled".to_string(),
        ],
        "Action is slot 13 — the last of TSensor's own props, ahead of the \
         inherited CktElement tail"
    );
}

/// `like=` copies the stub strings, because r4133's `MakeLike` copies the donor's
/// whole property-string array (`generator.pas:828`, `Sensor.pas:428`). For every
/// other property the port renders a live field, so this is the one place where
/// that upstream array copy is observable in the port at all.
#[test]
fn make_like_carries_the_stub_strings() {
    let mut dss = dss_with_circuit();
    dss.command("New Generator.g1 bus1=b1 phases=3 kv=12.47 kw=100 rneut=7 xneut=8");
    dss.command("New Generator.g2 like=g1 bus1=b2");
    assert_eq!(query(&mut dss, "Generator.g2.Rneut"), "7");
    assert_eq!(query(&mut dss, "Generator.g2.Xneut"), "8");

    dss.command("New Line.l1 bus1=b1 bus2=b2 phases=3 length=1");
    dss.command("New Sensor.s1 element=Line.l1 terminal=1 kvbase=12.47 action=sqerror");
    dss.command("New Sensor.s2 like=s1");
    assert_eq!(query(&mut dss, "Sensor.s2.Action"), "sqerror");
}

/// The stub rows stay off the 0.14.5-pinned full-enumeration surfaces
/// (`PropFlags::HIDE_R4133`): neither pinned capture — 0.14.5 nor capi015 —
/// knows the names, so a `Dump` line or a JSON key for them would be a byte the
/// oracle can never produce. The `?` query above proves the other half: the
/// property-table surfaces do expose them.
#[test]
fn stub_rows_are_absent_from_dump_and_json() {
    // Its own circuit name: `Dump` writes `<circuit>_PropertyDump.txt` into the
    // process-wide cwd, and the whole file runs in parallel with every other
    // test in this binary.
    let mut dss = Dss::new();
    dss.command("New circuit.stubdump");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.command("New Generator.g1 bus1=b1 phases=3 kv=12.47 kw=100 rneut=7");
    dss.command("New Line.l1 bus1=b1 bus2=b2 phases=3 length=1");
    dss.command("New Sensor.s1 element=Line.l1 terminal=1 kvbase=12.47 action=sqerror");

    // `Dump` writes `<circuit>_PropertyDump.txt`; collect the exact paths so the
    // test leaves no artifact behind (deleted by name, never a recursive wipe).
    let mut written: Vec<String> = Vec::new();
    let mut dump = |dss: &mut Dss, cmd: &str| -> String {
        dss.command(cmd);
        let path = dss.last_result_file().to_string();
        let text = std::fs::read_to_string(&path).expect("dump file");
        written.push(path);
        text
    };

    let gen_dump = dump(&mut dss, "Dump Generator.g1");
    let sensor_dump = dump(&mut dss, "Dump Sensor.s1");
    // Cleanup before the assertions, so a failure cannot leave the artifact.
    written.sort();
    written.dedup();
    for path in &written {
        let _ = std::fs::remove_file(path);
    }

    assert!(
        gen_dump.contains("~ Conn=") && gen_dump.contains("~ Status="),
        "the Dump surface must still be complete around the hidden pair: {gen_dump}"
    );
    for absent in ["Rneut", "Xneut"] {
        assert!(
            !gen_dump.contains(absent),
            "{absent} must not reach the 0.14.5-pinned Dump text: {gen_dump}"
        );
    }
    assert!(
        sensor_dump.contains("~ Weight=") && !sensor_dump.contains("Action"),
        "Sensor Action must not reach the Dump text: {sensor_dump}"
    );

    for (full, absent) in [
        ("Generator.g1", &["Rneut", "Xneut"][..]),
        ("Sensor.s1", &["Action"][..]),
    ] {
        let json = dss
            .obj_to_json(full, crate::report::export::json::JsonOpts::FULL)
            .expect("object renders as JSON");
        for key in absent {
            assert!(
                !json.contains(key),
                "{full}: {key} must not reach the 0.14.5-pinned AltDSS JSON: {json}"
            );
        }
    }
}

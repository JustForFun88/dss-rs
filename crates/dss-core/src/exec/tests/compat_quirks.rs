//! Expected-value pins for the Stage F *single-site upstream quirk* rows whose
//! observable lives at the command surface rather than inside one element's
//! unit tests (`DE_PASCALIZE_PLAN.md` Part IV.2 + `PORTING_PLAN.md` §4.1 rule 4).
//!
//! Tests for a *split* row assert against `crate::compat`'s lane constant, so
//! they are meaningful in **both** builds: the parity lane pins the reproduced
//! upstream value the gating oracles return, the default lane pins the clean
//! fix. Tests for a row that is still reproduced in both lanes pin the upstream
//! value outright, and say what blocked the flip.

use super::common::{dss_with_circuit, query, query_f64};
use crate::exec::Dss;

/// EXPECTED-VALUE-PIN(max_device_name_length): the `Show` device-name column is
/// sized from its own content, in **both** lanes.
///
/// The row this replaces reproduced dss_capi's width of **0**: its
/// `SetMaxDeviceNameLength` zeroes the unit variable
/// (`.inputs/dss_capi/src/Common/ShowResults.pas:116`) and then accumulates the
/// maximum inside `with DSS.ActiveCircuit do` (`:117-121`), where the name
/// resolves to the shadowing `TDSSCircuit` field (`src/Common/Circuit.pas:100`,
/// initialized to 30 at `:379`) — so the writers, which read the unit variable,
/// format against 0. r4133 has no such field: `MaxDeviceNameLength` is a unit
/// variable only (`Version8/Source/Common/ShowResults.pas:66`) and the same loop
/// (`:79-90`) leaves the honest width behind. A defect on one side and the
/// authority's honest width on the other, so `GOLDEN_REBASE_PLAN.md` G2.6 tore
/// the reproduction down and this pin lost its lane branches.
///
/// Three claims, all unconditional:
///
/// 1. the width itself is the longest `Class.Name` in the circuit;
/// 2. what `Pad(EncloseQuotes(FullName), width + 2) + IntToStr(term)`
///    (`ShowResults.pas:1375`) *does* with that width — asserted on the Pascal
///    primitive the parity kernel replays, not on a report: a name shorter than
///    the field gets a terminal column of its own, while the *longest* name,
///    which fills `width + 2` exactly, still glues. That boundary is the one
///    `golden_reports.rs::busflow_expected` is written around, which is why it
///    is stated here rather than left implicit;
/// 3. that the width the report *formatter* uses is that same measured one —
///    asserted on the real `Show busflow` text, so the seven `report::show`
///    call sites are covered too and not just the measuring function. Two
///    assertions: the terminal is a token of its own (a width at or below the
///    quoted name's length would glue it to the closing quote, which is exactly
///    the torn-down layout), and it starts no earlier than column `width + 2`
///    (so a width merely *smaller* than the measured one, which still separates,
///    fails too).
///
/// Claim 3 is what bites when a call site regresses, and it bites in the
/// **parity** lane: `compat::render_rows` replays Pascal's `Pad`, so the width
/// reaches the bytes there. The default lane's table kernel builds its columns
/// from the cell *text* and ignores the declared width entirely
/// (`report::table::render_rows_table_impl`), so no report can observe a width
/// regression there — what covers the default lane is claim 1, on the measuring
/// function both lanes share. The pin is unconditional all the same (a teardown
/// pin may not branch on the lane), and its `width + 2` bound holds in both:
/// measured on this fixture, the terminal lands at column 32 under `Pad` and at
/// 34 under the table kernel, whose widest cell here is the same 30-char name.
///
/// The bound is a `>=` and not an equality because the two kernels legitimately
/// place the column differently (32 vs 34); its lower end is precisely the
/// parity kernel's own `width + 2`, i.e. the value the torn-down row collapsed
/// to 2.
#[test]
fn device_name_column_is_sized_from_its_content() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.probe basekv=12.47 pu=1.0 phases=3 bus1=sourcebus");
    dss.command("new line.l1 bus1=sourcebus bus2=b2 phases=3 r1=0.1 x1=0.2 length=1");
    // The longest full name in the circuit: `Capacitor.cap_with_a_long_name`.
    dss.command("new capacitor.cap_with_a_long_name bus1=b2 phases=3 kvar=600 kv=12.47");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let longest = "Capacitor.cap_with_a_long_name";
    assert_eq!(longest.len(), 30, "the fixture's longest full name");

    let measured = {
        let ckt = dss.circuit().expect("solved circuit");
        crate::report::show::device_name_width(&dss.classes, ckt)
    };
    assert_eq!(
        measured,
        longest.len(),
        "the honest width is the longest `Class.Name` in the circuit"
    );

    // The layout `Pad` gives that width — Pascal's primitive in isolation, which
    // is what the parity kernel replays. The column is sized `width + 2` where
    // `width` already counts the *unquoted* name, so the longest element exactly
    // fills it and still glues; the split shows on every shorter name, which is
    // what a column is for. (The engine's own text is claim 3, below.)
    let row = |full: &str| {
        format!(
            "{}{}",
            crate::report::format::pad(&crate::report::format::enclose_quotes(full), measured + 2),
            1
        )
    };
    let short = row("Line.l1");
    assert!(
        !short.contains("\"1"),
        "a name shorter than the field gets a terminal column of its own; the \
         glued form is upstream's width-0 layout ({short:?})"
    );
    assert!(
        row(longest).contains("\"1"),
        "the longest name fills the column exactly, so it glues even at the \
         honest width — the boundary this row's transform is written around"
    );

    // …and the report formatter really uses that width. `Line.l1` reaches `b2`
    // by its **second** terminal (`check_bus_reference` returns the matched
    // terminal), so its seq-power row carries a `2`; the seq-*current* rows
    // above it are uppercased by `WriteSeqCurrents`, so this prefix selects the
    // power row unambiguously.
    let text = {
        let Dss {
            classes, circuit, ..
        } = &mut dss;
        let ckt = circuit.as_ref().expect("solved circuit");
        let bus_idx = ckt.bus_list.find("b2").expect("bus b2");
        let sys = crate::solution::solution::sys_ctx(ckt);
        let node_v = ckt.solution.node_v.clone();
        crate::report::show::show_bus_powers(classes, ckt, &sys, &node_v, bus_idx, 0, 0)
    };
    let power_row = text
        .lines()
        .find(|l| l.starts_with("\"Line.l1\""))
        .unwrap_or_else(|| panic!("no `Line.l1` seq-power row in\n{text}"));
    let close = power_row.rfind('"').expect("the closing quote of the name") + 1;
    let term_col = close
        + power_row[close..]
            .find(|c: char| c != ' ')
            .unwrap_or_else(|| panic!("nothing after the name in {power_row:?}"));
    assert_eq!(
        power_row.split_whitespace().take(2).collect::<Vec<_>>(),
        ["\"Line.l1\"", "2"],
        "the terminal number must be a token of its own; glued to the closing \
         quote it is upstream's collapsed layout ({power_row:?})"
    );
    assert!(
        term_col >= measured + 2,
        "the `Show busflow` writer placed the terminal at column {term_col}, \
         before the measured column {} — it padded the name field to less than \
         the width the engine computed ({power_row:?})",
        measured + 2
    );
}

/// Expected-value pin for the F-FMT **table-rendering** row
/// [`crate::compat::render_rows`] — §F-FMT step 2's table crate.
///
/// The observable is a real `Show` report, not the renderer in isolation: the
/// same solved circuit is rendered by the lane's kernel and three claims are
/// asserted as equalities against the lane, so both builds are checked.
///
/// 1. **The parity lane's bytes are still Pascal's.** `Show Losses`'s aggregate
///    block is `Pad(label, 30) + Format('%10.1f') + ' kW'`, so the unit lands at
///    column 40 exactly; the table kernel puts it wherever the column ends up.
/// 2. **The default lane really is the table crate**, i.e. it sizes the
///    element-name column from *its own rendered content* while the parity
///    kernel pads to the width the engine hands it — `Pad(EncloseQuotes(name),
///    MaxDeviceNameLength + 2)`, a circuit-wide maximum. The fixture separates
///    the two by carrying a **Load** whose name is far longer than either Line's:
///    `Show Losses` lists only power-delivery elements, so that name never
///    reaches the table, yet it does set the Pascal field width. Each row is
///    then reconstructed whole from `Pad(EncloseQuotes(name), width + 2) +
///    Format('%10.5f, ', …)` and compared as an **equality**, the same way claim
///    1 treats the aggregate line: a bound on the number's column would accept a
///    parity kernel that padded to any width past the field.
///
///    (Until `GOLDEN_REBASE_PLAN.md` G2.6 this claim was written as "the two
///    rows' numbers start at the *same* column only when a table sized them" —
///    which worked only because the parity lane's width was stuck at 0, i.e.
///    because of the defect G2.6 removed. With an honest width `Pad` aligns the
///    rows with each other too, so that comparison stopped discriminating and
///    the claim is now made against each kernel's own sizing rule.)
/// 3. **Neither kernel moves a field**: both renderings carry the identical
///    token stream — the property `report::table` guarantees structurally and
///    this pins at a report the executive actually produced.
#[test]
fn show_table_layout_is_the_lane_kernel() {
    let parity = crate::compat::ORACLE_PARITY;
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.probe basekv=12.47 pu=1.0 phases=3 bus1=sourcebus");
    dss.command("new line.l1 bus1=sourcebus bus2=b2 phases=3 r1=0.1 x1=0.2 length=1");
    dss.command("new line.a_much_longer_line_name bus1=b2 bus2=b3 phases=3 r1=0.1 x1=0.2 length=1");
    // Not a power-delivery element, so it never appears in `Show Losses` — but
    // it *is* the circuit's longest full name, which is what sizes Pascal's
    // field. That gap between "widest name in the circuit" and "widest name in
    // this table" is what separates the two kernels (claim 2).
    dss.command(
        "new load.ld_with_a_name_longer_than_any_line bus1=b3 phases=3 kv=12.47 kw=1000 pf=0.95",
    );
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let width = {
        let ckt = dss.circuit().expect("solved circuit");
        crate::report::show::device_name_width(&dss.classes, ckt)
    };
    assert_eq!(
        width,
        "Load.ld_with_a_name_longer_than_any_line".len(),
        "the fixture's widest full name must be the Load's, or claim 2 stops \
         separating the kernels"
    );

    let text = {
        let Dss {
            classes, circuit, ..
        } = &mut dss;
        let ckt = circuit.as_ref().expect("solved circuit");
        let sys = crate::solution::solution::sys_ctx(ckt);
        let node_v = ckt.solution.node_v.clone();
        crate::report::show::show_losses(classes, ckt, &sys, &node_v)
    };

    // 1. the aggregate block's line, reconstructed from Pascal's own arithmetic:
    //    `Pad(label, 30) + Format('%10.1f') + ' kW'`. Asserting the whole line
    //    rather than a column index keeps the claim content-independent — the
    //    parity kernel must reproduce that formula whatever the value is, and
    //    the table kernel must not (it sizes both columns from the run).
    let total = text
        .lines()
        .find(|l| l.starts_with("TOTAL LOSSES="))
        .unwrap_or_else(|| panic!("no TOTAL LOSSES row in\n{text}"));
    let toks: Vec<&str> = total.split_whitespace().collect();
    assert_eq!(toks, ["TOTAL", "LOSSES=", toks[2], "kW"], "{total:?}");
    let pascal = format!(
        "{}{:>10} kW",
        crate::report::format::pad("TOTAL LOSSES=", 30),
        toks[2]
    );
    assert_eq!(
        *total == pascal,
        parity,
        "the parity kernel writes Pad(label,30)+%10.1f+' kW' exactly; the table \
         kernel sizes the columns instead ({total:?} vs {pascal:?})"
    );

    // 2. the element-name column is content-sized in the default lane only: the
    //    parity kernel pads it to the engine's circuit-wide `width + 2`, which
    //    the Load's name inflates past anything this table renders.
    //
    //    Like claim 1 this is Pascal's own arithmetic reconstructed and compared
    //    whole — `Pad(EncloseQuotes(name), width + 2)` then
    //    `Format('%10.5f, ', kLosses.re)` (`ShowResults.pas` `ShowLosses`) — not
    //    a bound on a column index. A bound would let the parity kernel pad to
    //    any width past the field and still pass; the equality pins the field.
    let rows: Vec<&str> = text.lines().filter(|l| l.starts_with('"')).collect();
    assert_eq!(rows.len(), 2, "one row per Line: {rows:?}");
    for row in &rows {
        let close = row.rfind('"').expect("the closing quote") + 1;
        let (name, rest) = row.split_at(close);
        let kw = rest
            .split_whitespace()
            .next()
            .unwrap_or_else(|| panic!("no kW number after the name in {row:?}"))
            .trim_end_matches(',');
        let pascal = format!("{}{kw:>10},", crate::report::format::pad(name, width + 2));
        assert_eq!(
            row.starts_with(&pascal),
            parity,
            "the parity kernel writes `Pad(EncloseQuotes(name), width + 2)` with \
             the circuit-wide width ({width} + 2 here) then `%10.5f, `; the table \
             kernel sizes the column from the names it actually prints, which are \
             far shorter, and carries no comma ({row:?} vs {pascal:?})"
        );
    }

    // 3. …and no field moved between the two.
    let fields = |l: &str| {
        l.split(|c: char| c.is_whitespace() || c == ',')
            .filter(|f| !f.is_empty())
            .count()
    };
    assert_eq!(fields(rows[0]), fields(rows[1]), "{rows:?}");
    assert_eq!(fields(rows[0]), 4, "name, kW, % of power, kvar: {rows:?}");
}

/// Expected-value pin for the F-FMT table row at the **per-bus / per-element**
/// reports F.4e converted (`Show Voltages`' node form here — the same seam
/// carries `Currents`/`Powers`/`BusFlow`/`Buses`/`Elements`/`Meters`/`Faults`/
/// `Mismatch`).
///
/// Two claims, both equalities against the lane so either build checks both:
///
/// 1. **The name column is the lane's.** Pascal fills it with `PadDots` — a
///    space then dots — so the parity lane's short bus names carry a dot run and
///    the table kernel's, which pads with its own gutter, carries none.
/// 2. **No field moved.** Every node row of a 3-phase bus carries the same
///    twelve fields in either lane (name, node, |V|, `/_`, angle, pu, base kV,
///    then the line-line group) — the token-stream guarantee of `report::table`,
///    asserted here on a report the executive really produced.
#[test]
fn show_voltage_table_layout_is_the_lane_kernel() {
    let parity = crate::compat::ORACLE_PARITY;
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.probe basekv=12.47 pu=1.0 phases=3 bus1=sourcebus");
    dss.command("new line.l1 bus1=sourcebus bus2=b2 phases=3 r1=0.1 x1=0.2 length=1");
    dss.command("new line.l2 bus1=b2 bus2=a_much_longer_bus_name phases=3 r1=0.1 x1=0.2 length=1");
    dss.command("new load.ld bus1=a_much_longer_bus_name phases=3 kv=12.47 kw=1000 pf=0.95");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let ckt = dss.circuit().expect("solved circuit");
    let text = crate::report::show::show_voltages_nodes(ckt, false);

    // 1. `PadDots` vs the table kernel's gutter.
    let dotted = text.lines().filter(|l| l.contains(" ....")).count();
    assert_eq!(
        dotted > 0,
        parity,
        "the parity kernel fills the bus column with Pascal's dots; the table \
         kernel sizes it from content\n{text}"
    );

    // 2. …and every row still carries its twelve fields.
    let rows: Vec<&str> = text.lines().filter(|l| l.contains("/_")).collect();
    assert!(rows.len() >= 9, "three 3-phase buses: {rows:?}");
    for r in &rows {
        let fields = r
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|f| !f.is_empty() && !f.bytes().all(|b| b == b'.'))
            .count();
        assert_eq!(fields, 12, "line-ground + line-line groups: {r:?}");
    }
}

/// Expected-value pin for the round row at the **`Set time=`** boundary
/// (`dss_parser::compat::round_i32`).
///
/// `TExecHelper.Set_Time` (`ExecHelper.pas:1406`) is
/// `DynaVars.intHour := Round(TimeArray[1])` — an Int64 `Round` assigned to an
/// `Integer`, over a number the deck supplies raw — and `Solution.Hour` reads
/// it straight back, so the conversion is observable from the deck language.
///
/// The values are the pinned oracle's, probed with `set time=(x,0)` then
/// `Solution.Hour`: `3e9 → -1294967296`, `1e10 → 1410065408`,
/// `1e20 → 0`. Those are FPC's wrapped integer-indefinite results; the default
/// lane saturates instead. Its sibling `set hour=` has always gone through the
/// same kernel (via `make_integer`), and this test asserts the two commands
/// agree — which is what F-settle W4 found they did *not* do: `Set time=` was
/// left on a bare saturating cast, so the parity lane silently diverged from
/// the oracle it is defined to reproduce.
#[test]
fn set_time_hour_is_the_lane_kernel() {
    let parity = crate::compat::ORACLE_PARITY;
    // (deck value, oracle/parity hour, default-lane saturating hour)
    let cases: [(&str, i64, i64); 5] = [
        ("3e9", -1_294_967_296, i32::MAX as i64),
        ("1e10", 1_410_065_408, i32::MAX as i64),
        ("1e20", 0, i32::MAX as i64),
        ("-1e20", 0, i32::MIN as i64),
        ("2147483648", -2_147_483_648, i32::MAX as i64),
    ];

    for (deck, parity_hour, default_hour) in cases {
        let expected = if parity { parity_hour } else { default_hour };

        let mut dss = dss_with_circuit();
        dss.command(&format!("set time=({deck},0)"));
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        let via_time = i64::from(dss.circuit().unwrap().solution.int_hour);
        assert_eq!(
            via_time, expected,
            "`set time=({deck},0)` hour, lane parity = {parity}"
        );

        // `set hour=` is the same Pascal `Round` into the same field; the two
        // spellings must not disagree in either lane.
        let mut dss = dss_with_circuit();
        dss.command(&format!("set hour={deck}"));
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        let via_hour = i64::from(dss.circuit().unwrap().solution.int_hour);
        assert_eq!(
            via_hour, via_time,
            "`set hour={deck}` and `set time=({deck},0)` must round alike"
        );
    }
}

/// Expected-value pin for the round row's **array** kernel
/// (`dss_parser::compat::round_f64`) at the `TPropertyFlag.ApplyRound`
/// boundary, whose carrier is `GrowthShape.year`
/// (`GrowthShape.pas:165` sets the flag).
///
/// Pascal applies `doubles[i] := Round(doubles[i])`, so an out-of-Int64 element
/// does not keep its magnitude — it becomes the integer-indefinite sentinel
/// widened back to floating point. Probed on the pinned oracle:
/// `new growthshape.g npts=2 year=[1e20,2] mult=[1,1]` then
/// `? growthshape.g.year` gives `[ -9.22337203685478E18 2]`. The default lane
/// rounds in place and keeps `1e20`.
#[test]
fn apply_round_out_of_range_is_the_lane_kernel() {
    let mut dss = dss_with_circuit();
    dss.command("New GrowthShape.g npts=2 year=[1e20,2] mult=[1.0,1.0]");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let text = query(&mut dss, "GrowthShape.g.year");
    let first: f64 = text
        .trim_matches(|c| c == '[' || c == ']')
        .split_whitespace()
        .next()
        .expect("year array renders at least one element")
        .parse()
        .expect("the first year element is numeric");

    let expected = if crate::compat::ORACLE_PARITY {
        -9_223_372_036_854_775_808.0 // i64::MIN as f64
    } else {
        1e20
    };
    // Compared through the property *text*, which renders 15 significant
    // digits (`-9.22337203685478E18` — byte-identical to the oracle's), so the
    // last few bits of the sentinel do not survive the round trip. A relative
    // bound of 1e-12 is still ~8 orders tighter than the gap between the two
    // lanes' values, which differ in sign and by a factor of ten.
    assert!(
        ((first - expected) / expected).abs() < 1e-12,
        "ApplyRound writes Round's Int64 back into the Double: got {first:e}, \
         expected {expected:e} (lane parity = {})",
        crate::compat::ORACLE_PARITY
    );

    // The in-range element is untouched in both lanes — so this pins the
    // sentinel path, not "rounding is broken".
    assert!(
        text.contains('2'),
        "the second year element survives: {text}"
    );
}

/// Expected-value pin, in both lanes, for the text getter of a
/// `DoubleSymMatrixProperty`: it renders the matrix the object stores.
///
/// The authority renders the stored values. r4133 has no typed property table:
/// `Fault.GMatrix` is rendered by hand from the dereferenced pointer under an
/// `If Assigned` (`Version8/Source/PDElements/Fault.pas:695-717`), and
/// Capacitor/Reactor fall through to `TDSSObject.GetPropertyValue`, i.e. the
/// stored property text (`Version8/Source/General/DSSObject.pas:112-115`).
/// Neither path can print zeros.
///
/// dss_capi's generic arm addresses the *pointer field* as if it were the array
/// (`src/General/DSSObjectHelper.pas:2296-2313`), so it reads uninitialized
/// memory and answers denormal garbage (~0) whatever was stored — the captured
/// `props` goldens hold that as an all-zero matrix. Reading uninitialized memory
/// is UB, so neither the garbage nor its zero surrogate is reproduced in any
/// lane; the 33 affected golden cells keep their **shape** compared and drop
/// only their values (`props_roundtrip::LANE_SKIP_PROP_VALUES`), with the real
/// numbers pinned here.
///
/// Its JSON arm repeats the very same slip (`:1242-1266`), so this is not a
/// case of upstream disagreeing with itself — both dss_capi paths are wrong and
/// both of ours are right. The JSON view is asserted below for that reason.
#[test]
fn sym_matrix_text_getter_renders_the_stored_matrix() {
    let mut dss = dss_with_circuit();
    dss.command(
        "New Capacitor.c1 bus1=b1 phases=3 \
         cmatrix=(2.8 | -0.6 2.8 | -0.6 -0.6 2.8)",
    );
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let text = query(&mut dss, "Capacitor.c1.CMatrix");
    assert_eq!(
        text, "(2.8 |-0.6 2.8 |-0.6 -0.6 2.8 )",
        "the text getter prints the stored lower triangle in both lanes"
    );

    // The shape is what the goldens still compare in both lanes — only the
    // numbers are dropped there (`props_roundtrip::LANE_SKIP_PROP_VALUES`).
    assert!(text.starts_with('(') && text.ends_with(" )"));
    assert_eq!(text.matches('|').count(), 2, "3×3 lower triangle: two rows");

    // The JSON view carries the same stored values, in every lane.
    let json = dss
        .obj_to_json("Capacitor.c1", Default::default())
        .expect("Capacitor.c1 renders as JSON");
    // Spelled through the seam (`compat::json_float`), because *this* assertion
    // is about the stored values reaching the JSON view at all — not about the
    // F-FMT row that decides how they are printed.
    let (diag, off) = (
        crate::compat::json_float(2.8),
        crate::compat::json_float(-0.6),
    );
    assert!(
        json.contains(&diag) && json.contains(&off),
        "the JSON view carries the stored matrix in every lane: {json}"
    );
}

/// EXPECTED-VALUE-PIN(GIC_TRANSFORMER_G2_SCALES_OFF_PCT_R1): a GICTransformer's
/// `%R`-specified second winding takes **`%R2`**, in both lanes.
///
/// `TGICTransformerObj.RecalcElementData` computes
/// `G2 := 100.0 / (FZBase2 * FPctR1)` in both oracle revisions — pinned
/// dss_capi 0.14.5 `src/PDElements/GICTransformer.pas:441`, EPRI r4133
/// `Version8/Source/PDElements/GICTransformer.pas:495` — a copy of the `G1`
/// line above it with the base renamed and the percentage not, so a user's
/// `%R2` is stored, read back, and never used. That the same procedure's
/// `else` arm inverts the pair honestly (`FPctR2 := 100.0 / (FZBase2 * G2)`)
/// is what makes it a slip rather than a "winding 2 repeats winding 1"
/// convention; `GOLDEN_REBASE_PLAN.md` G2.5 fixed it in both lanes.
///
/// **The assertion is a transitive cover, not a captured number.** The ohms
/// spec (`R1=`/`R2=`) takes that `else` arm, never had the slip, and is
/// oracle-gated unchanged on both channels (`asymmetric/gic/*` carry one:
/// `tg2`/`tg3` with `R1=0.2 R2=0.1`). So the `%R` path is pinned against the
/// `R` path: given `%R_w` on bases that make `ZBase_w` a round number, the two
/// specs must produce the *same element*. `R2` is stored as the conductance
/// `G2` behind the property `INVERSE_VALUE` flag, so `? …R2` reads back `1/G2`
/// — the observable below.
#[test]
fn gic_transformer_pct_r2_drives_winding_two() {
    // Asymmetric bases AND asymmetric percentages, so neither winding can
    // borrow the other's number unnoticed: ZBase1 = 200²/100 = 400 Ω and
    // ZBase2 = 100²/100 = 100 Ω, so %R1 = 1 → R1 = 4 Ω and %R2 = 3 → R2 = 3 Ω,
    // while the upstream reading (ZBase2·%R1/100) would be 1 Ω — three distinct
    // values, so no pair of them can coincide by accident.
    let build = |spec: &str| {
        let mut dss = dss_with_circuit();
        dss.command(&format!(
            "new GICTransformer.g busH=sourcebus busNH=sourcebus.0.0.0 \
             busX=bx busNX=bx.0.0.0 type=YY kvll1=200 kvll2=100 mva=100 {spec}"
        ));
        dss.command("calcvoltagebases");
        dss.command("solve");
        assert!(dss.errors().is_empty(), "{spec}: {:?}", dss.errors());
        dss
    };

    let mut pct = build("%R1=1 %R2=3");
    // The same element written the other way: R_w = ZBase_w · %R_w / 100.
    let mut ohms = build("R1=4 R2=3");

    for prop in ["R1", "R2"] {
        let from_pct = query_f64(&mut pct, &format!("GICTransformer.g.{prop}"));
        let from_ohms = query_f64(&mut ohms, &format!("GICTransformer.g.{prop}"));
        assert!(
            (from_pct - from_ohms).abs() <= 1e-12 * from_ohms.abs(),
            "{prop}: the %R spec gives {from_pct}, the ohms spec {from_ohms} — \
             the two arms of RecalcElementData must be mutual inverses"
        );
    }
    // Explicitly not the upstream reading, which reused %R1 for winding 2 and
    // would report R2 = ZBase2·%R1/100 = 1 Ω here.
    let r2 = query_f64(&mut pct, "GICTransformer.g.R2");
    assert!(
        (r2 - 1.0).abs() > 1e-6,
        "R2 came back as ZBase2·%R1/100 = 1 Ω — the `%R1` slip is back"
    );

    // …and the admittance really moved with it: the `%R`-specified element
    // stamps the same YPrim as its ohms twin.
    let (order_p, yp) = pct
        .element_yprim("GICTransformer.g")
        .expect("%R-spec YPrim");
    let (order_o, yo) = ohms
        .element_yprim("GICTransformer.g")
        .expect("ohms-spec YPrim");
    assert_eq!(order_p, order_o);
    for (i, (x, y)) in yp.iter().zip(yo.iter()).enumerate() {
        assert!(
            (x - y).norm() <= 1e-18 + 1e-12 * y.norm(),
            "YPrim[{i}]: %R spec {x} vs ohms spec {y}"
        );
    }
}

/// Expected-value pin for the Stage F single-site quirk
/// [`crate::compat::profile_ll_pu_divisor`] — `Export Profile`'s line-to-line
/// per-unit column.
///
/// Upstream divides the L-L volt magnitude by the four-digit literal `1732.0`
/// while the **line-to-neutral** arms of the same procedure divide by the exact
/// `1000.0` (`Common/ExportResults.pas`, three L-L sites against eight L-N
/// ones; r4133 identical). `Bus.kVBase` is the L-N base kV, so the L-L divisor
/// should be `1000·√3` and the literal makes every reported L-L per-unit
/// 2.93e-5 relative high.
///
/// **The assertion is the physics, not a captured number.** On a *balanced*
/// three-phase bus `|V_LL| = √3·|V_LN|` exactly, so the two reports must print
/// the **same** per-unit for the same bus:
///
/// ```text
/// pu_LL = √3·|V_LN| / (kVBase·1000·√3) = |V_LN| / (kVBase·1000) = pu_LN
/// ```
///
/// The default lane satisfies that identity; the parity lane misses it by
/// exactly the divisor ratio `1000·√3 / 1732.0`, which is what the truncation
/// is. So this test states what the golden's excluded column can no longer
/// state, and it cannot be satisfied by an engine that merely swapped one
/// constant for another wrong one.
#[test]
fn export_profile_ll_pu_is_the_lane_kernel() {
    use std::fmt::Write as _;

    let dir = std::env::temp_dir().join("dss_rs_profile_ll_pin");
    let _ = std::fs::create_dir_all(&dir);

    let mut dss = crate::exec::Dss::new();
    dss.command("clear");
    // A balanced, perfectly transposed radial feeder: equal r1/r0 and x1/x0 and
    // a balanced 3-phase load, so every bus voltage is a symmetric positive
    // sequence set and `|V_LL| = √3·|V_LN|` holds to the last bit.
    dss.command("new circuit.prof basekv=12.47 phases=3 bus1=src mvasc3=20000 mvasc1=20000");
    dss.command(
        "new line.l1 bus1=src bus2=b length=1 units=km \
         r1=0.1 x1=0.3 r0=0.1 x0=0.3 c1=0 c0=0",
    );
    dss.command("new load.ld bus1=b phases=3 kv=12.47 kw=500 pf=1 model=1");
    dss.command("new energymeter.m element=line.l1 terminal=1");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    let mut dp = String::new();
    write!(dp, "set datapath=\"{}\"", dir.display()).unwrap();
    dss.command(&dp);
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let read_pu = |dss: &mut crate::exec::Dss, cmd: &str| -> Vec<f64> {
        dss.command(cmd);
        assert!(dss.errors().is_empty(), "{cmd}: {:?}", dss.errors());
        let text = std::fs::read_to_string(dir.join("prof_EXP_Profile.csv"))
            .unwrap_or_else(|e| panic!("{cmd} produced no report: {e}"));
        // Columns: Name, Distance1, puV1, Distance2, puV2, …
        text.lines()
            .skip(1)
            .filter(|l| !l.trim().is_empty())
            .flat_map(|l| {
                let f: Vec<&str> = l.split(',').collect();
                [
                    f[2].trim().parse::<f64>().unwrap(),
                    f[4].trim().parse::<f64>().unwrap(),
                ]
            })
            .collect()
    };

    let ln = read_pu(&mut dss, "export profile");
    let ll = read_pu(&mut dss, "export profile ll3ph");
    assert!(
        !ln.is_empty() && ln.len() == ll.len(),
        "fixture degenerated"
    );

    // `WriteNewLine` renders the per-unit at 6 significant digits, so each of
    // the two reports carries up to half a unit in the 6th digit (5e-6 absolute
    // near 1.0) and their difference up to ~1.1e-5. That is the *reading* floor
    // of this surface, not a tolerance on the engine: the divergence the row
    // introduces is 2.93e-5 relative, ~2.8x it, which is exactly why the two
    // lanes stay distinguishable through a 6-digit report at all.
    let read_floor = 1.1e-5;
    let ratio = 1000.0 * crate::util::sqrt3() / 1732.0;
    assert!(
        (ratio - 1.000_029_334_6).abs() < 1e-10,
        "the divisor ratio moved: {ratio}"
    );

    for (i, (&a, &b)) in ln.iter().zip(ll.iter()).enumerate() {
        assert!(a > 0.9 && a < 1.1, "row {i}: implausible L-N pu {a}");
        let expected = if crate::compat::ORACLE_PARITY {
            // Parity keeps `1732.0`: every L-L pu is the balanced L-N pu scaled
            // up by the truncation.
            a * ratio
        } else {
            // The default lane divides by `1000·√3`, so the identity holds.
            a
        };
        assert!(
            (b - expected).abs() <= read_floor,
            "row {i}: L-L pu {b} is not the lane's expectation {expected} \
             (L-N pu {a}, ORACLE_PARITY = {})",
            crate::compat::ORACLE_PARITY
        );
        // And the *other* lane's value must be distinguishable, or this test
        // would pass in both builds and pin nothing.
        let other = if crate::compat::ORACLE_PARITY {
            a
        } else {
            a * ratio
        };
        assert!(
            (b - other).abs() > read_floor,
            "row {i}: the two lanes' L-L pu are indistinguishable at the \
             report's own resolution — the divisor split has stopped working"
        );
    }

    let _ = std::fs::remove_file(dir.join("prof_EXP_Profile.csv"));
}

/// Every property in the live class table that carries `flag`, as
/// `Class.PropName`, sorted so the assertion does not depend on class
/// registration order (which is an IV.1 permanent semantic and must stay
/// unobservable here).
fn carriers_of(dss: &crate::exec::Dss, flag: crate::obj::props::PropFlags) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for cls in &dss.classes {
        let props = &cls.props;
        for i in 1..=props.num_properties() {
            if props.prop(i).flags.contains(flag) {
                out.push(format!("{}.{}", props.class_name(), props.property_name(i)));
            }
        }
    }
    out.sort();
    out
}

/// Escape pin for the **0.15.x hide-flag waiver** — the one Stage F exit item
/// that is not a compat marker (`UPGRADE_PLAN` §5 wants a grep for the flag's
/// name to come back empty; disposition in `docs/upgrade/DIVERGENCES.md`
/// §"Line/LineGeometry Conductors"; the F.3aa measurement lives on the flag
/// itself, [`crate::obj::props::PropFlags::HIDE_015X`], and this file follows
/// that doc's convention of not repeating the name in prose — see it for why).
///
/// The escape is quantified — "13 byte goldens move, the corpus does not" — and
/// a measurement is only worth as much as the population it was taken over. So
/// this pins that population: the flag's carrier set. If a later WP adds a sixth
/// carrier, or moves one, the recorded blast radius silently stops describing
/// the tree; this test fails instead.
///
/// It also pins the sibling [`crate::obj::props::PropFlags::HIDE_R4133`]'s
/// carrier set. That flag was **carrier-free** from WP-U2.5 until
/// R4133_PROPS_PLAN RP1.1, which is how `hidden_from_full_enum` came to be a
/// synonym for the 0.15.x flag — the premise of the F.3aa measurement. RP1.1
/// gave it three carriers again (the Generator/Sensor upstream stubs) and RP1.2
/// a fourth (AutoTrans `XfmrCode`, a real port — the flag's criterion is
/// "absent from both pinned tables", not "not implemented"), so the
/// premise is now the weaker but sufficient one: the two flags' carrier sets are
/// **class-disjoint** — F.3aa's blast radius was measured over Line and
/// LineGeometry alone, and no r4133 carrier touches either — which the two lists
/// below state jointly. The carrier-free era was also the empirical proof that
/// `UPGRADE_PLAN` §5's criterion as worded is unreachable: a flag with zero
/// carriers still leaves its definition, its predicate arm and the comments
/// naming it for `rg` to find.
#[test]
fn hide_015x_carrier_set_is_the_measured_escape() {
    let dss = dss_with_circuit();

    let hide_015x = carriers_of(&dss, crate::obj::props::PropFlags::HIDE_015X);
    assert_eq!(
        hide_015x,
        [
            "Line.Conductors",
            "Line.EpsRMedium",
            "Line.HeightOffset",
            "Line.HeightUnit",
            "LineGeometry.Conductors",
        ],
        "this carrier set is the population the Stage F escape was measured over \
         (13 byte goldens: +4 Dump rows on Line, +1 on LineGeometry, the two \
         JSON micro views and the three schema walks). Changing it invalidates \
         that record — re-measure and update the flag's doc in \
         `obj/props/prop_flags.rs` before touching this list"
    );

    let hide_r4133 = carriers_of(&dss, crate::obj::props::PropFlags::HIDE_R4133);
    assert_eq!(
        hide_r4133,
        [
            "AutoTrans.XfmrCode",
            "Generator.Rneut",
            "Generator.Xneut",
            "Sensor.Action",
        ],
        "the r4133 hide flag's carrier set moved. RP1.1 re-armed it with the \
         three upstream stubs and RP1.2 added the real AutoTrans `XfmrCode` port \
         (the flag says only 'absent from both pinned tables', not 'not \
         implemented'); each carrier is also a `port_hidden_property` row of \
         `tests/golden/json/schema_divergences.json`, so a carrier added or \
         dropped here without that row is a schema byte gate that silently stops \
         describing the tree. This list is also the population the flag's \
         un-hide blast radius was measured over (8 artifacts, no corpus case) — \
         re-measure it and update the doc on `PropFlags::HIDE_R4133` before \
         touching this list"
    );

    // The disjointness the F.3aa measurement now rests on: no class carries both
    // flags, so the Line/LineGeometry blast radius is still isolated.
    let class_of = |s: &String| s.split('.').next().unwrap_or("").to_string();
    let a: std::collections::BTreeSet<String> = hide_015x.iter().map(class_of).collect();
    let b: std::collections::BTreeSet<String> = hide_r4133.iter().map(class_of).collect();
    assert!(
        a.is_disjoint(&b),
        "the two hide flags now share a class ({:?}) — `hidden_from_full_enum` no \
         longer isolates the F.3aa population and that measurement must be redone",
        a.intersection(&b).collect::<Vec<_>>()
    );
}

/// The [`crate::obj::props::PropFlags::UPSTREAM_STUB`] carrier set, pinned the
/// same way and for the same reason as the hide flags above (R4133_PROPS_PLAN
/// RP1.1).
///
/// The flag buys three guarantees at once — the row is *listed*, its value is
/// stored and echoed, and the write is never a parse error — so a row that
/// acquires it stops being engine state without any other diff saying so. The
/// list is therefore an equality, and it is paired with the two invariants that
/// make the mechanism honest: exactly the Generator pair carries a
/// [`crate::obj::props::StubMessage`] (Sensor's `Action` is silent upstream), and
/// every carrier is `HIDE_R4133` (an r4133-only name the pinned captures cannot
/// enumerate).
#[test]
fn upstream_stub_rows_are_the_measured_set() {
    use crate::obj::props::PropFlags;
    let dss = dss_with_circuit();

    assert_eq!(
        carriers_of(&dss, PropFlags::UPSTREAM_STUB),
        ["Generator.Rneut", "Generator.Xneut", "Sensor.Action"],
        "the upstream-stub population moved. Every row here is a property r4133 \
         still registers but no longer implements (`generator.pas:441-442,651-652`; \
         `Sensor.pas:183,850-854`); adding one takes a property out of the engine's \
         state, so it belongs in the commit that argues for it"
    );

    // Per-row: which stubs answer with a message, and that all of them are hidden
    // from the 0.14.5-pinned full-enumeration surfaces.
    let mut with_msg: Vec<String> = Vec::new();
    for cls in &dss.classes {
        let props = &cls.props;
        for i in 1..=props.num_properties() {
            let pd = props.prop(i);
            if !pd.flags.contains(PropFlags::UPSTREAM_STUB) {
                continue;
            }
            assert!(
                pd.flags.contains(PropFlags::HIDE_R4133),
                "{}.{} is an upstream stub that the pinned 0.14.5/capi015 tables \
                 cannot know, so it must also be HIDE_R4133",
                props.class_name(),
                pd.name
            );
            if let Some(m) = pd.stub_message {
                with_msg.push(format!("{}.{}={}", props.class_name(), pd.name, m.code));
            }
        }
    }
    with_msg.sort();
    assert_eq!(
        with_msg,
        ["Generator.Rneut=5611", "Generator.Xneut=5612"],
        "only the Generator pair logs on write (`generator.pas:651-652`); \
         Sensor's `Action` stores silently (`Set_Action` is an empty body, \
         `Sensor.pas:850-854`)"
    );
}

/// The [`crate::obj::props::PropDef::ref_miss_message`] carrier set — the
/// sibling mechanism of the stub message above, pinned for the same reason
/// (R4133_PROPS_PLAN RP1.2).
///
/// The field is *relaxing by nature*: the row it sits on stops answering an
/// unresolved name with the unified `DSSObjectHelper` #401 (which also NILs the
/// reference) and answers with r4133's own legacy `Fetch<X>` message instead,
/// writing nothing at all. Only a reference r4133 still resolves outside the
/// property system may have it, so the population is an equality — a future row
/// that acquires `ref_miss_msg(...)` silently drops #401 from a property that
/// has a capi counterpart, and no other test would redden.
///
/// Its build-time half — the field is only ever read by the one `parse_into` arm
/// that takes a `PropType::ObjectRef` with a named `object_class`, so anywhere
/// else it is a dead message — is asserted once per class in `ClassProps::new`,
/// and re-asserted here over the live tables so a reader of this test sees it.
#[test]
fn ref_miss_message_rows_are_the_measured_set() {
    use crate::obj::props::PropType;
    let dss = dss_with_circuit();

    let mut carriers: Vec<String> = Vec::new();
    for cls in &dss.classes {
        let props = &cls.props;
        for i in 1..=props.num_properties() {
            let pd = props.prop(i);
            let Some(m) = pd.ref_miss_message else {
                continue;
            };
            assert!(
                pd.ptype == PropType::ObjectRef && pd.object_class.is_some_and(|c| !c.is_empty()),
                "{}.{} carries a ref_miss_message on a row `parse_into` never reads it on",
                props.class_name(),
                pd.name
            );
            carriers.push(format!(
                "{}.{}={} {:?}",
                props.class_name(),
                pd.name,
                m.code,
                m.prefix
            ));
        }
    }
    carriers.sort();
    assert_eq!(
        carriers,
        ["AutoTrans.XfmrCode=100180 \"Xfmr Code:\""],
        "the legacy-Fetch miss population moved. The one row is AutoTrans \
         `XfmrCode`, whose miss arm is r4133's own `DoSimpleMsg('Xfmr Code:' + \
         Code + ' not found.', 100180)` (`AutoTrans.pas:2394-2395`) and which \
         has no dss_capi 0.14.5 counterpart at all (the property was deleted \
         there). Every other object reference keeps the unified #401 path"
    );
}

/// Escape pin for the **collision** that makes the 0.15.x hide flag and the
/// `Line.Wires → "Conductors"` `json_name` masquerade one atomic change.
///
/// Line declares *two* properties that render the JSON key `Conductors`: the
/// legacy `Wires` array, given `json_name = "Conductors"` at class build so the
/// port emits dss_capi's key from the 0.14.5-era storage, and the real 0.15.x
/// `Conductors` proxy array, hidden by the flag. Exactly one of the two is
/// hidden, so exactly one key is emitted — and dropping the flag on its own
/// makes the FULL view of a Line carry `"Conductors":[]` **twice** (measured in
/// F.3aa: once after `Spacing`, once after `HeightUnit`).
///
/// That is why `UPGRADE_PLAN` §5 lists the masquerade drop *inside* the same
/// exit item, and it is the half of the argument no golden states: the byte
/// goldens would fail on the flip either way, but only this says *why the naive
/// flip is wrong* rather than merely stale. Both halves are asserted here, in
/// both lanes, because the row is reproduced in both.
#[test]
fn line_json_conductors_key_is_owned_by_the_masquerade() {
    let mut dss = dss_with_circuit();
    dss.command("new Line.l1 bus1=b1 bus2=b2 phases=3 length=1");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    // (1) The structural precondition: two props, one key, exactly one hidden.
    let line = &dss.classes[dss.class_by_name["line"]].props;
    let colliding: Vec<(&str, bool)> = (1..=line.num_properties())
        .filter(|&i| line.prop(i).json_key(false) == "Conductors")
        .map(|i| {
            (
                line.property_name(i),
                line.prop(i).flags.hidden_from_full_enum(),
            )
        })
        .collect();
    assert_eq!(
        colliding,
        [("Wires", false), ("Conductors", true)],
        "Line must declare exactly two props rendering the JSON key \
         \"Conductors\" — the visible `Wires` masquerade and the hidden real \
         `Conductors`. If either half moved, the other must move with it \
         (UPGRADE_PLAN §5)"
    );

    // (2) The observable that precondition buys: one key in the emitted bytes
    //     of the FULL view (`build.rs`'s full-enumeration branch).
    let full = dss
        .obj_to_json("Line.l1", crate::report::export::json::JsonOpts::FULL)
        .expect("Line.l1 renders as JSON");
    assert_eq!(
        full.matches("\"Conductors\"").count(),
        1,
        "the FULL Line view must carry exactly one \"Conductors\" key; two means \
         the hide flag was dropped without dropping the `Wires` masquerade: \
         {full}"
    );
    assert!(
        !full.contains("EpsRMedium"),
        "a hidden 0.15.x prop must not reach the FULL JSON view: {full}"
    );

    // (3) The flag's cost, and the other gated branch: the *set-order* view
    //     drops a hidden 0.15.x property the user explicitly typed, so a JSON
    //     round-trip of this Line loses `EpsRMedium=2.5` — while the `?` query,
    //     which the flag does not gate, still answers it. That asymmetry is the
    //     product-visible price of the waiver, and the reason the row is worth
    //     an UPGRADE rung rather than indefinite deferral.
    dss.command("edit Line.l1 epsrmedium=2.5");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let set_order = dss
        .obj_to_json("Line.l1", Default::default())
        .expect("Line.l1 renders as JSON");
    assert!(
        !set_order.contains("EpsRMedium"),
        "a hidden 0.15.x prop is dropped from the set-order view even when set: \
         {set_order}"
    );
    assert_eq!(
        query(&mut dss, "Line.l1.EpsRMedium"),
        "2.5",
        "the `?` named-query surface must still expose a hidden 0.15.x prop — \
         that is what makes the flag a rendering deferral rather than a deletion"
    );
}

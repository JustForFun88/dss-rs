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

/// Expected-value pin for the F-FMT **table-layout** row
/// [`crate::compat::max_device_name_length`] — the `Show` half of §F-FMT step 2.
///
/// Two claims, both asserted as equalities against the lane so they are checked
/// in either build:
///
/// 1. the width itself — 0 in the parity lane (what the pinned 0.14.5 backend
///    returns regardless of the names) versus the longest `Class.Name` in the
///    circuit in the default lane;
/// 2. what that width *does*, at the one place it is observable: `Show BusFlow`
///    writes `Pad(EncloseQuotes(FullName), width + 2) + IntToStr(term)`
///    (`ShowResults.pas:1375`), so at width 0 the terminal number is glued to the
///    closing quote and at the honest width it is a column of its own. Asserting
///    the rendered row, not just the number, is what keeps this a *layout* pin
///    rather than a restatement of the constant.
#[test]
fn device_name_column_width_is_the_lane_kernel() {
    let parity = crate::compat::ORACLE_PARITY;
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

    let ckt = dss.circuit().expect("solved circuit");
    let measured = crate::report::show::device_name_width(&dss.classes, ckt);
    assert_eq!(
        measured,
        longest.len(),
        "the honest width is computed in both lanes"
    );
    let width = crate::compat::max_device_name_length(measured);
    assert_eq!(
        width,
        if parity { 0 } else { longest.len() },
        "the device-name column width is the lane's (parity = {parity})"
    );

    // The layout consequence, on the row the width actually formats. Note the
    // column is sized `width + 2` where `width` already counts the *unquoted*
    // name, so the longest element exactly fills it and still glues in **both**
    // lanes — the split shows on every shorter name, which is what a column is
    // for.
    let row = |full: &str| {
        format!(
            "{}{}",
            crate::report::format::pad(&crate::report::format::enclose_quotes(full), width + 2),
            1
        )
    };
    let short = row("Line.l1");
    assert_eq!(
        short.contains("\"1"),
        parity,
        "at width 0 the terminal number is glued to the closing quote; at the \
         honest width it is a separate column ({short:?})"
    );
    assert!(
        row(longest).contains("\"1"),
        "the longest name fills the column exactly in either lane"
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
///    element-name column from its own content: the two rows' quoted names have
///    different lengths and their following field starts at the *same* column
///    only when a table sized them.
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
    dss.command("new load.ld bus1=b3 phases=3 kv=12.47 kw=1000 pf=0.95");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let text = {
        let Dss {
            classes, circuit, ..
        } = &mut dss;
        let ckt = circuit.as_ref().expect("solved circuit");
        let sys = crate::solution::solution::sys_ctx(ckt);
        let node_v = ckt.solution.node_v.clone();
        crate::report::show::show_losses(classes, ckt, &sys, &node_v)
    };

    // Where the field after `prefix_len` characters of `l` begins.
    let next_field = |l: &str, from: usize| {
        from + l[from..]
            .find(|c: char| c != ' ')
            .unwrap_or_else(|| panic!("no field after column {from} of {l:?}"))
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

    // 2. the element-name column is content-sized in the default lane only.
    //    (The kW field's `', '` separator is the table kernel's gutter there, so
    //    the row is matched by its quoted name, not by a comma.)
    let rows: Vec<&str> = text.lines().filter(|l| l.starts_with('"')).collect();
    assert_eq!(rows.len(), 2, "one row per Line: {rows:?}");
    let kw_col = |l: &str| next_field(l, l.rfind('"').expect("the closing quote") + 1);
    assert_eq!(
        kw_col(rows[0]) == kw_col(rows[1]),
        !parity,
        "the table kernel pads both names to one column; the parity kernel's \
         `Pad(name, 0 + 2)` leaves each row its own width ({rows:?})"
    );

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

/// Expected-value pin for the Stage F single-site quirk
/// [`crate::compat::SYM_MATRIX_GETTER_RENDERS_ZEROS`].
///
/// Upstream's `GetObjPropertyValue` arm for a `DoubleSymMatrixProperty` reads
/// uninitialized memory, so the `?` query answers a matrix of denormal garbage
/// (~0) whatever the object stores. The parity lane keeps the deterministic
/// surrogate the captured goldens hold — a zero matrix of the declared order —
/// and the default lane renders the stored lower triangle.
///
/// The clean fix needs no argument beyond upstream itself: the **same
/// property's** JSON exporter reads `darray[(i-1)*Norder + j] / scale` and emits
/// the real numbers in both engines, so only the text path is wrong. That is
/// asserted here too — the JSON view is checked to carry the stored values in
/// *both* lanes, which is what makes the text getter a defect rather than a
/// convention.
#[test]
fn sym_matrix_text_getter_is_lane_split() {
    let mut dss = dss_with_circuit();
    dss.command(
        "New Capacitor.c1 bus1=b1 phases=3 \
         cmatrix=(2.8 | -0.6 2.8 | -0.6 -0.6 2.8)",
    );
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let text = query(&mut dss, "Capacitor.c1.CMatrix");
    let expected = if crate::compat::ORACLE_PARITY {
        "(0 |0 0 |0 0 0 )"
    } else {
        "(2.8 |-0.6 2.8 |-0.6 -0.6 2.8 )"
    };
    assert_eq!(
        text,
        expected,
        "the text getter prints zeros in the parity lane and the stored lower \
         triangle in the default lane (lane parity = {})",
        crate::compat::ORACLE_PARITY
    );

    // The shape is identical in both lanes — only the numbers move. (This is
    // what `props_roundtrip::LANE_SKIP_PROP_VALUES` still gates by comparing the
    // rendered skeleton against the oracle in the default lane.)
    assert!(text.starts_with('(') && text.ends_with(" )"));
    assert_eq!(text.matches('|').count(), 2, "3×3 lower triangle: two rows");

    // The JSON exporter of the very same property has always emitted the stored
    // values, in every lane — upstream disagreeing with itself is the evidence.
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
        "the JSON view must carry the stored matrix in every lane: {json}"
    );
}

/// GICTransformer's `%R`-specified second-winding conductance scales off
/// **`%R1`**, not `%R2` — reproduced in **both** lanes, pinned here.
///
/// `TGICTransformerObj.RecalcElementData` (`GICTransformer.pas:441`) computes
/// `G2 := 100.0 / (FZBase2 * FPctR1)`; the line above it is
/// `G1 := 100.0 / (FZBase1 * FPctR1)`, so the copy-paste left `FPctR1` driving
/// both and a user's `%R2` is silently ignored. That the `else` branch inverts
/// the pair correctly (`FPctR2 := 100.0 / (FZBase2 * G2)`) is what makes it a
/// slip rather than a convention.
///
/// **Why it is not (yet) a lane split.** F.3k implemented the fix, ran the
/// 520-case gate against it and reverted: `asymmetric/gic/gictransformer_gic.dss`
/// builds `GICTransformer.tg3 … %R1=0.2 %R2=0.15`, and honouring `%R2` moves
/// that deck's GIC current 4.50e-4 vs the `capi_v0145` oracle where 1.00e-6 is
/// allowed (`gic/gic_midi.dss`: 1.02e-4 vs 1.07e-6). Both gating oracles
/// reproduce the quirk, so the default-lane fix costs those decks' primary
/// physical channel and owes the Newton row's full treatment.
///
/// `R2` is stored as the conductance `G2` behind the property `INVERSE_VALUE`
/// flag, so `? GICTransformer.g.R2` reads back `1/G2` = `ZBase2·%R_used/100` —
/// the observable pinned below. With `kv1 == kv2` and a shared `MVA`,
/// `ZBase1 == ZBase2`, so `R2` must collapse onto `R1` whatever `%R2` said.
#[test]
fn gic_transformer_g2_reproduces_the_pct_r1_bug() {
    let mut dss = dss_with_circuit();
    // %R-specified spec, deliberately asymmetric, on a symmetric voltage/MVA
    // base so ZBase1 == ZBase2 = 100²/100 = 100 Ω.
    dss.command(
        "new GICTransformer.g busH=b1 busNH=b2 busX=b3 busNX=b4 type=YY \
         kvll1=100 kvll2=100 mva=100 %R1=1 %R2=4",
    );
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let z_base = 100.0f64 * 100.0 / 100.0;
    let r1 = query_f64(&mut dss, "GICTransformer.g.R1");
    let r2 = query_f64(&mut dss, "GICTransformer.g.R2");

    // Winding 1 is correct upstream: R1 = ZBase1·%R1/100.
    assert!(
        (r1 - z_base * 0.01).abs() < 1e-12,
        "R1 must be ZBase·%R1/100, got {r1}"
    );
    // Winding 2 ignores %R2=4 and reuses %R1=1 — the reproduced bug.
    assert!(
        (r2 - z_base * 0.01).abs() < 1e-12,
        "upstream scales G2 off %R1 (GICTransformer.pas:441), so R2 must equal \
         R1 = {r1} despite %R2=4; got {r2}"
    );
    assert!(
        (r2 - z_base * 0.04).abs() > 1e-6,
        "if this now equals ZBase·%R2/100 the quirk was fixed — see the Stage F \
         note at the reproduction site before re-baselining anything"
    );

    // The conductance spec (`R1=`/`R2=`, the `else` branch) never went through
    // the quirk: it is the exact inverse map, and it honours both windings.
    let mut ohms = dss_with_circuit();
    ohms.command(
        "new GICTransformer.g busH=b1 busNH=b2 busX=b3 busNX=b4 type=YY \
         kvll1=100 kvll2=100 mva=100 R1=1 R2=4",
    );
    assert!(ohms.errors().is_empty(), "{:?}", ohms.errors());
    assert!((query_f64(&mut ohms, "GICTransformer.g.R1") - 1.0).abs() < 1e-12);
    assert!((query_f64(&mut ohms, "GICTransformer.g.R2") - 4.0).abs() < 1e-12);
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
/// It also pins the sibling [`crate::obj::props::PropFlags::HIDE_R4133`] as
/// **carrier-free** (WP-U2.5 retired its last one), which is what makes
/// `hidden_from_full_enum` today a synonym for the 0.15.x flag — the premise of
/// the measurement — and is simultaneously the empirical proof that the §5
/// criterion as worded is unreachable: a flag with zero carriers still leaves
/// its definition, its predicate arm and the comments naming it for `rg` to
/// find.
#[test]
fn hide_015x_carrier_set_is_the_measured_escape() {
    let dss = dss_with_circuit();

    assert_eq!(
        carriers_of(&dss, crate::obj::props::PropFlags::HIDE_015X),
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

    assert!(
        carriers_of(&dss, crate::obj::props::PropFlags::HIDE_R4133).is_empty(),
        "the r4133 hide flag lost its carrier-free state (WP-U2.5): \
         `hidden_from_full_enum` is no longer a synonym for the 0.15.x flag, so \
         the F.3aa measurement no longer isolates that row"
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

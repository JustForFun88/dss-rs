//! AutoTrans `XfmrCode` — EPRI r4133 property 39 (R4133_PROPS_PLAN RP1.2).
//!
//! r4133 registers `PropertyName^[39] := 'XfmrCode'`
//! (`Version8/Source/PDElements/AutoTrans.pas:329`) and answers it with the
//! auto's **own** `TAutoTransObj.FetchXfmrCode` (`:520` → `:2339-2396`), which
//! is the Transformer's routine plus three deviations: winding 1 is forced
//! `SERIES` and winding 2 `WYE` regardless of the code, the code's
//! `XHL/XHT/XLT` land in `puXHX/puXHT/puXXT`, and every winding comes out
//! `RdcSpecified`. dss_capi 0.14.5 deleted the property outright
//! (`.inputs/dss_capi/src/PDElements/AutoTrans.pas:76,125`), so there is no
//! capi witness for any of it: the corpus deck
//! `asymmetric:autotrans/autotrans_xfmrcode.dss` gates the single-phase form on
//! the r4133 channel, and everything multi-phase is pinned here.
//!
//! Two r4133 statements in this arm are upstream bugs and are **not**
//! reproduced (CLAUDE.md — never, in any lane); both are pinned below by the
//! behaviour the port has *instead*:
//!
//! - `DoSimpleMsg('XFmrCode Property not used with AutoTrans object.', 100131)`
//!   (`:567`), fired unconditionally right after `:520` applied the code — a
//!   message that contradicts the statement above it, and (measured) the one
//!   that ends up on the API errno surface even when the real failure was the
//!   `Xfmr Code:… not found.` of `:2395`
//!   (`investigations/to_opendss/36-autotrans-xfmrcode-not-used-message.md`).
//! - `NConds := Fnphases + 1` (`:2353`), the Transformer's conductor rule
//!   copy-pasted into a class that needs `2 * Fnphases`
//!   (`investigations/to_opendss/37-autotrans-fetchxfmrcode-nconds.md`).

use crate::exec::Dss;

use super::common::query;

/// The 3-winding `XfmrCode` both decks below are built from. Its winding
/// connections are `wye`/`delta`/`delta` **on purpose**: the auto must end up
/// `Series`/`Wye`/`Delta`, so a missing override is visible in the first two.
///
/// **Every value here is off the AutoTrans default**, and
/// [`no_asserted_cell_can_be_read_off_the_defaults`] enforces it: a cell that
/// happens to coincide with what `TAutoWinding.Init`/`Create` produce anyway
/// (`auto_trans/mod.rs`, e.g. winding 1 at 115 kV, `%r` 0.2, tap 1, min/max
/// 0.9/1.1, 32 taps, `%loadloss` 0.4) would still read correctly with its copy
/// statement deleted, so it could not pin anything.
const CODE: &str = "New XfmrCode.c3 phases=3 windings=3";
const CODE_TAIL: &[&str] = &[
    "~ wdg=1 conn=wye kV=138 kVA=50000 tap=1.025 maxtap=1.2 mintap=0.8 numtaps=40 rdcohms=0.31 %r=0.33",
    "~ wdg=2 conn=delta kV=69 kVA=50000 tap=0.975 maxtap=1.15 mintap=0.85 numtaps=24 rdcohms=0.12 %r=0.19",
    "~ wdg=3 conn=delta kV=13.8 kVA=20000 tap=1.05 maxtap=1.25 mintap=0.75 numtaps=20 rdcohms=0.05 %r=0.35",
    "~ xhl=8.5 xht=24.5 xlt=28.5 %imag=0.35 %noloadloss=0.11 thermal=2.5 n=0.9 m=0.7 \
     flrise=63 hsrise=17 ppm_antifloat=1.5 normhkva=61000 emerghkva=83000",
];

/// The same electrical model written out property by property — every field
/// `FetchXfmrCode` copies, in an order that lands the same values (the auto's
/// own `kVA`/`%R` side effects run on both paths, so `normhkva`/`emerghkva`
/// come last, exactly as the code's own edit left them).
const EXPLICIT_TAIL: &[&str] = &[
    "~ wdg=1 bus=sourcebus conn=s kV=138 kVA=50000 tap=1.025 maxtap=1.2 mintap=0.8 numtaps=40 rdcohms=0.31 %r=0.33",
    "~ wdg=2 bus=mid conn=w kV=69 kVA=50000 tap=0.975 maxtap=1.15 mintap=0.85 numtaps=24 rdcohms=0.12 %r=0.19",
    "~ wdg=3 bus=tert conn=d kV=13.8 kVA=20000 tap=1.05 maxtap=1.25 mintap=0.75 numtaps=20 rdcohms=0.05 %r=0.35",
    "~ xhx=8.5 xht=24.5 xxt=28.5 %imag=0.35 %noloadloss=0.11 thermal=2.5 n=0.9 m=0.7 \
     flrise=63 hsrise=17 ppm_antifloat=1.5 normhkva=61000 emerghkva=83000",
];

/// The per-winding cells `FetchXfmrCode` copies (`:2362-2371`), as
/// `(winding, property, expected)` — read back off the `?` surface through the
/// `Wdg=` cursor. Shared by the two tests below so the assertion table and its
/// discrimination guard cannot drift apart.
const PER_WINDING: &[(u32, &str, &str)] = &[
    (1, "kV", "138"),
    (1, "kVA", "50000"),
    (1, "Tap", "1.025"),
    (1, "%R", "0.33"),
    (1, "RDCOhms", "0.31"),
    (1, "MaxTap", "1.2"),
    (1, "MinTap", "0.8"),
    (1, "NumTaps", "40"),
    (2, "kV", "69"),
    (2, "kVA", "50000"),
    (2, "Tap", "0.975"),
    (2, "%R", "0.19"),
    (2, "RDCOhms", "0.12"),
    (2, "MaxTap", "1.15"),
    (2, "MinTap", "0.85"),
    (2, "NumTaps", "24"),
    (3, "kV", "13.8"),
    (3, "kVA", "20000"),
    (3, "Tap", "1.05"),
    (3, "%R", "0.35"),
    (3, "RDCOhms", "0.05"),
    (3, "MaxTap", "1.25"),
    (3, "MinTap", "0.75"),
    (3, "NumTaps", "20"),
];

/// The whole-element cells: the reactance rename and short-circuit array
/// (`:2374-2377`), the thermal/loss/rating scalars (`:2378-2388`) and the stored
/// code name (`:2349`). `%LoadLoss` is the code's own 0.33 + 0.19 = 0.52 — the
/// AutoTrans default is 0.4, so the cell discriminates.
const WHOLE_ELEMENT: &[(&str, &str)] = &[
    ("XHX", "8.5"),
    ("XHT", "24.5"),
    ("XXT", "28.5"),
    ("XSCArray", "[ 8.5 24.5 28.5]"),
    ("Thermal", "2.5"),
    ("n", "0.9"),
    ("m", "0.7"),
    ("FLRise", "63"),
    ("HSRise", "17"),
    ("%LoadLoss", "0.52"),
    ("%NoLoadLoss", "0.11"),
    ("%IMag", "0.35"),
    ("NormHkVA", "61000"),
    ("EmergHkVA", "83000"),
    ("ppm_Antifloat", "1.5"),
    ("XfmrCode", "c3"),
];

/// Header + loads + solve, shared by the two builders so the ONLY difference
/// between them is how the autotransformer was specified.
fn run(auto: &[String]) -> Dss {
    let mut dss = Dss::new();
    let mut script: Vec<String> = vec![
        "Clear".into(),
        "New Circuit.xcauto basekv=138 phases=3 bus1=sourcebus mvasc3=20000 mvasc1=18000".into(),
    ];
    script.extend(auto.iter().cloned());
    script.extend(
        [
            "New Load.l1 bus1=mid phases=3 kv=69 kw=20000 pf=0.92 model=1",
            "New Load.l2 bus1=tert phases=3 kv=13.8 kw=5000 pf=0.95 model=1",
            "Set voltagebases=[138 69 13.8]",
            "Calcvoltagebases",
            "Set tolerance=1e-10",
            "Solve",
        ]
        .iter()
        .map(|s| s.to_string()),
    );
    for line in &script {
        dss.command(line);
        assert!(dss.errors().is_empty(), "`{line}` -> {:?}", dss.errors());
    }
    assert!(dss.circuit().unwrap().is_solved, "the deck must solve");
    dss
}

/// The auto built from the `XfmrCode`.
fn coded() -> Dss {
    let mut lines: Vec<String> = vec![CODE.to_string()];
    lines.extend(CODE_TAIL.iter().map(|s| s.to_string()));
    lines.push("New AutoTrans.t1 xfmrcode=c3".to_string());
    lines.push("~ wdg=1 bus=sourcebus".to_string());
    lines.push("~ wdg=2 bus=mid".to_string());
    lines.push("~ wdg=3 bus=tert".to_string());
    run(&lines)
}

/// The same auto written out property by property.
fn explicit() -> Dss {
    let mut lines: Vec<String> = vec!["New AutoTrans.t1 phases=3 windings=3".to_string()];
    lines.extend(EXPLICIT_TAIL.iter().map(|s| s.to_string()));
    run(&lines)
}

/// The solved state a wrong copy would move: the assembled system Y as
/// coordinate-sorted triplets, plus the node voltages.
type Solved = (Vec<(usize, usize, f64, f64)>, Vec<(f64, f64)>);

fn solved(dss: &mut Dss) -> Solved {
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

/// Pascal `:2356-2361` — *"No Choice for 1st two"*. The code declares
/// `wye`/`delta`/`delta`; the auto must come out `Series`/`Wye`/`Delta`, so the
/// override wins on windings 1 and 2 and only the tertiary keeps the code's own
/// connection. This is the deviation that makes the routine an *auto*
/// `FetchXfmrCode` rather than the Transformer's.
#[test]
fn the_first_two_winding_connections_are_forced() {
    let mut dss = coded();
    assert_eq!(
        query(&mut dss, "AutoTrans.t1.conns"),
        "[series, wye, delta, ]"
    );
    for (w, conn) in [(1, "series"), (2, "wye"), (3, "delta")] {
        dss.command(&format!("Edit AutoTrans.t1 wdg={w}"));
        assert_eq!(
            query(&mut dss, "AutoTrans.t1.conn"),
            conn,
            "winding {w} connection"
        );
    }
}

/// Every scalar `:2362-2388` copies, read back off the `?` surface (the
/// per-winding ones through the `Wdg=` cursor). `%R` is the one that proves the
/// copy is per-winding rather than a `%loadloss` re-split: winding 3's 0.35
/// survives while `%loadloss` reads the code's own 0.33 + 0.19.
#[test]
fn every_copied_field_arrives() {
    let mut dss = coded();
    let mut active = 0;
    for &(w, prop, want) in PER_WINDING {
        if w != active {
            dss.command(&format!("Edit AutoTrans.t1 wdg={w}"));
            assert!(dss.errors().is_empty(), "{:?}", dss.errors());
            active = w;
        }
        assert_eq!(
            query(&mut dss, &format!("AutoTrans.t1.{prop}")),
            want,
            "winding {w} {prop}"
        );
    }
    for &(prop, want) in WHOLE_ELEMENT {
        assert_eq!(
            query(&mut dss, &format!("AutoTrans.t1.{prop}")),
            want,
            "{prop}"
        );
    }
}

/// The discrimination guard for the table above: **no** asserted cell may be a
/// value the auto holds without the copy.
///
/// `FetchXfmrCode` starts with `SetNumWindings` (`:2352`), which re-`Init`s
/// every winding to the class defaults, and then assigns field by field — so a
/// deleted assignment leaves that field at its default. A cell whose expected
/// value *is* the default therefore reads correctly either way and pins nothing
/// (measured: with the pre-audit fixture, deleting
/// `self.pct_load_loss = code.pct_load_loss()` left `every_copied_field_arrives`
/// green, because the code's own `%loadloss` was 0.21 + 0.19 = the default 0.4).
/// This test fails instead, on the cell, the moment the fixture drifts back onto
/// a default.
///
/// **The comparison is a proxy.** The real condition is "the cell does not
/// survive deleting its own copy statement", and a bare auto of the same shape
/// stands in for "the copy never happened". The two agree for every cell that
/// `RecalcElementData` leaves alone, and differ only where the recalc
/// re-derives a copied field from other fields:
///
/// - `RDCOhms` is re-derived from `%R`/`kV`/`kVA` (`:1140-1145`) only while
///   `RdcSpecified` is false. That is the bare auto's state, but the copy path
///   sets `RdcSpecified := TRUE` (`:2367`), so a deleted `RdcOhms :=` copy would
///   leave the `TAutoWinding` init placeholder `Sqr(kVLL)/(kVA/1000)·Rdcpu`
///   (`:1461-1462`) instead of the bare auto's reading. The second leg below
///   compares the fixture against that placeholder, computed from the bare
///   auto's own init `kV`, `kVA` and `%R`.
/// - `TapIncrement` is re-derived from `MaxTap`/`MinTap`/`NumTaps` on every
///   recalc (`:1096-1097`), so its copy (`:2368`) can be deleted without any
///   cell moving. The table carries no `TapIncrement` cell for that reason.
/// - `XSCArray` would be re-derived from `XHX`/`XHT`/`XXT` under `XHXChanged`
///   (`:1100`), but that flag is cleared by the recalc that ends `Create`, so on
///   this path the cell is carried by its own copy (`:2377`).
#[test]
fn no_asserted_cell_can_be_read_off_the_defaults() {
    let mut dss = Dss::new();
    for line in [
        "New Circuit.xcdefaults basekv=138 phases=3 bus1=b1",
        "New AutoTrans.d1 phases=3 windings=3",
        "~ wdg=1 bus=b1",
        "~ wdg=2 bus=b2",
        "~ wdg=3 bus=b3",
    ] {
        dss.command(line);
        assert!(dss.errors().is_empty(), "`{line}` -> {:?}", dss.errors());
    }
    // Every read must be live: a property name that stops resolving answers
    // `Property Unknown` (no error: `do_query_cmd`), an object that stops
    // resolving logs an error, and a broken getter answers an empty string —
    // each of which would pass the `assert_ne!` trivially. The one cell whose
    // true default IS empty is the bare auto's code name.
    let cell = |dss: &mut Dss, prop: &str| {
        let seen = dss.errors().len();
        let got = query(dss, &format!("AutoTrans.d1.{prop}"));
        assert_eq!(dss.errors().len(), seen, "`? {prop}` -> {:?}", dss.errors());
        assert_ne!(got, "Property Unknown", "`? {prop}` does not resolve");
        assert!(
            !got.is_empty() || prop == "XfmrCode",
            "`? {prop}` answered nothing"
        );
        got
    };
    let mut active = 0;
    for &(w, prop, want) in PER_WINDING {
        if w != active {
            dss.command(&format!("Edit AutoTrans.d1 wdg={w}"));
            assert!(dss.errors().is_empty(), "wdg={w} -> {:?}", dss.errors());
            active = w;
        }
        assert_ne!(
            cell(&mut dss, prop),
            want,
            "winding {w} {prop}: the fixture asks for the AutoTrans default, so \
             that cell would pass with its copy statement deleted — pick a \
             non-default value in `CODE_TAIL`/`EXPLICIT_TAIL`"
        );
    }
    for &(prop, want) in WHOLE_ELEMENT {
        assert_ne!(
            cell(&mut dss, prop),
            want,
            "{prop}: the fixture asks for the AutoTrans default, so that cell \
             would pass with its copy statement deleted"
        );
    }

    // Second leg: the recalc-derived `RDCOhms` against the init placeholder a
    // deleted copy would leave. The margin is a discrimination floor, not a
    // tolerance: a fixture value within 1e-6 of the placeholder (well inside
    // the `?` render's significant digits) counts as landing on it.
    let mut checked = 0;
    for &(w, prop, want) in PER_WINDING.iter().filter(|c| c.1 == "RDCOhms") {
        dss.command(&format!("Edit AutoTrans.d1 wdg={w}"));
        assert!(dss.errors().is_empty(), "wdg={w} -> {:?}", dss.errors());
        let num = |dss: &mut Dss, p: &str| -> f64 {
            let s = cell(dss, p);
            s.parse().unwrap_or_else(|e| panic!("`? {p}` = {s:?}: {e}"))
        };
        let (kv, kva, pct_r) = (
            num(&mut dss, "kV"),
            num(&mut dss, "kVA"),
            num(&mut dss, "%R"),
        );
        let placeholder = kv * kv / (kva / 1000.0) * (0.85 * pct_r / 100.0);
        let want: f64 = want.parse().expect("a numeric fixture cell");
        assert!(
            (want - placeholder).abs() > 1e-6 * placeholder.abs(),
            "winding {w} {prop}: the fixture's {want} is the init placeholder \
             {placeholder}, which a deleted `RdcOhms :=` copy would leave"
        );
        checked += 1;
    }
    assert_eq!(checked, 3, "one RDCOhms cell per winding");

    // The sentinel `cell` rejects is what a name that does not resolve answers.
    assert_eq!(
        query(&mut dss, "AutoTrans.d1.NoSuchProperty"),
        "Property Unknown"
    );
}

/// `RdcSpecified := TRUE` (`:2367`) is not decoration: it selects the
/// `Rdcpu := RdcOhms/(VBase²/VABase)` branch of `RecalcElementData` (`:1140-1147`)
/// over the `0.85 · Rpu` default, which would *overwrite* the copied `RdcOhms`.
/// A copy that forgot the flag would therefore report a different `RDCOhms` than
/// the code holds — so reading the copied value straight back is the pin.
#[test]
fn the_copy_marks_every_winding_rdc_specified() {
    let mut dss = coded();
    for (w, rdc) in [(1, "0.31"), (2, "0.12"), (3, "0.05")] {
        dss.command(&format!("Edit AutoTrans.t1 wdg={w}"));
        assert_eq!(
            query(&mut dss, "AutoTrans.t1.RDCOhms"),
            rdc,
            "winding {w} kept the code's RdcOhms, so RdcSpecified was set"
        );
    }
    // The control: an auto that never saw a code re-derives RdcOhms from Rpu.
    let mut plain = Dss::new();
    plain.command("New Circuit.rdc basekv=115 phases=3 bus1=b1");
    plain.command("New AutoTrans.a1 phases=3 windings=2");
    plain.command("~ wdg=1 bus=b1 conn=s kV=115 kVA=50000 %r=0.21");
    plain.command("~ wdg=2 bus=b2 conn=w kV=69 kVA=50000 %r=0.19");
    plain.command("Edit AutoTrans.a1 wdg=1");
    assert_ne!(
        query(&mut plain, "AutoTrans.a1.RDCOhms"),
        "0.31",
        "the default path must NOT land on the code's value by accident"
    );
}

/// The conductor count. r4133 leaves `NConds = Fnphases + 1` here (`:2353`);
/// an autotransformer needs `2 · Fnphases`, which is what every other r4133
/// AutoTrans path sets (`:538`, `:798`, `:891`) and what `SetNodeRef`'s series
/// aliasing indexes into. `Yorder = NConds · NTerms` is visible from outside as
/// the length of the element's terminal-current vector: 6 · 3 = **18** here,
/// where the reproduced bug would give (3 + 1) · 3 = 12.
#[test]
fn the_conductor_count_stays_two_per_phase() {
    let mut dss = coded();
    let snaps = dss.snapshot_elements();
    let t1 = snaps
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case("AutoTrans.t1"))
        .expect("the auto is in the circuit");
    assert_eq!(
        t1.currents.len(),
        18,
        "Yorder = 2·nphases·nwindings; r4133's NConds := Fnphases+1 would give 12"
    );
    assert_eq!(t1.powers.len(), 18);
}

/// The whole point of the property: an auto configured from a code is the same
/// machine as the auto written out longhand. Both decks are solved from the same
/// header and loads, so the assembled Y and every node voltage must agree
/// **exactly** — this is the correctness statement no oracle channel can make
/// for a multi-phase auto, because r4133's own `NConds` slip puts it on a
/// different circuit (`:2353`).
#[test]
fn a_coded_auto_equals_the_longhand_one() {
    let mut a = coded();
    let mut b = explicit();
    let (ya, va) = solved(&mut a);
    let (yb, vb) = solved(&mut b);
    assert_eq!(ya, yb, "assembled Y differs between the two specifications");
    assert_eq!(va, vb, "solved node voltages differ");
}

/// The miss arm (`:2394-2395`): r4133's `else` is one `DoSimpleMsg` — number
/// 100180, the text `'Xfmr Code:' + Code + ' not found.'` — and it changes
/// nothing at all. Re-measured 2026-09-27 on the epri-worker (r4133 DLL) with
/// this file's deck (`coded()`, winding 1 at 138 kV): after
/// `Edit AutoTrans.t1 xfmrcode=nosuch` the winding data still reads
/// `kvs = [138, 69, 13.8, ]` and `conns = [Series, wye, delta, ]`, i.e. the
/// model is untouched.
///
/// (r4133's *echo* of property 39 does move — it has no `GetPropertyValue` arm
/// for it, so `? …xfmrcode` answers the raw parse store and reports `nosuch`
/// while the field still says `c3`. The port renders the live field in both
/// lanes; echo-sourced cells are excluded, never imitated.)
#[test]
fn a_missing_code_logs_100180_and_changes_nothing() {
    let mut dss = coded();
    let before = solved(&mut dss);
    let seen = dss.errors().len();

    dss.command("Edit AutoTrans.t1 xfmrcode=nosuch");
    let logged: Vec<(Option<u32>, String, bool)> = dss.errors()[seen..]
        .iter()
        .map(|e| (e.code, e.message.clone(), e.abort))
        .collect();
    assert_eq!(
        logged,
        [(
            Some(100180),
            "Xfmr Code:nosuch not found.".to_string(),
            false,
        )],
        "the miss answers with r4133's own message, not the generic #401"
    );

    assert_eq!(
        query(&mut dss, "AutoTrans.t1.XfmrCode"),
        "c3",
        "a miss leaves the stored name alone"
    );
    assert_eq!(
        query(&mut dss, "AutoTrans.t1.conns"),
        "[series, wye, delta, ]"
    );
    assert_eq!(
        solved(&mut dss),
        before,
        "a miss moves neither the assembled Y nor the standing solution"
    );
    // …and it did not even invalidate the model: a fresh solve rebuilds the
    // identical Y. (Only the Y is compared here — the second solve starts from
    // the first one's answer, so the node voltages land on a different point of
    // the same fixpoint's convergence band.)
    dss.command("Solve");
    assert_eq!(
        solved(&mut dss).0,
        before.0,
        "a miss leaves the rebuilt Y identical"
    );
}

/// `xfmrcode=` with no value: r4133's parser never reaches property 39 at all
/// (measured — the property store keeps its previous string and no message is
/// logged), so the port must be silent too. This is the one path where the
/// `ref_miss_message` row must NOT fire.
#[test]
fn an_empty_code_name_is_a_no_op() {
    let mut dss = coded();
    let seen = dss.errors().len();
    dss.command("Edit AutoTrans.t1 xfmrcode=");
    assert!(
        dss.errors()[seen..].is_empty(),
        "an empty name logs nothing: {:?}",
        &dss.errors()[seen..]
    );
    assert_eq!(query(&mut dss, "AutoTrans.t1.XfmrCode"), "c3");
}

/// The upstream bug that is deliberately absent: r4133 answers **every**
/// `xfmrcode=` write — successful ones included — with
/// `'XFmrCode Property not used with AutoTrans object.'` (#100131, `:567`),
/// which is false the moment `:520` has just applied the code. Nothing in the
/// port logs it, on the hit path or the miss path.
#[test]
fn the_contradictory_not_used_message_is_never_logged() {
    let mut dss = coded();
    assert!(
        dss.errors().is_empty(),
        "a successful xfmrcode= is silent: {:?}",
        dss.errors()
    );
    dss.command("Edit AutoTrans.t1 xfmrcode=nosuch");
    assert!(
        dss.errors()
            .iter()
            .all(|e| e.code != Some(100131) && !e.message.contains("not used with AutoTrans")),
        "#100131 is an upstream bug and must not be reproduced: {:?}",
        dss.errors()
    );
}

/// Display slot. r4133 registers `XfmrCode` as its 39th own property, between
/// `bank` (38) and `XRConst` (40) (`AutoTrans.pas:328-330`), which is what makes
/// its table 53 names long — the `oracle_count` the vendored census records in
/// `tests/corpus/props_r4133/shape.txt` against the port's pre-RP1.2 52.
#[test]
fn xfmrcode_sits_at_display_slot_39() {
    let mut dss = Dss::new();
    dss.command("New Circuit.slots basekv=115 phases=3 bus1=b1");
    dss.command("New AutoTrans.a1 phases=3 windings=2");
    dss.command("~ wdg=1 bus=b1 conn=s kV=115 kVA=50000");
    dss.command("~ wdg=2 bus=b2 conn=w kV=69 kVA=50000");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let names: Vec<String> = dss
        .element_properties("AutoTrans.a1")
        .expect("element exists")
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    assert_eq!(
        names.len(),
        53,
        "r4133's AutoTrans table is 53 names long (shape.txt: oracle_count=53)"
    );
    assert_eq!(
        &names[37..40],
        [
            "Bank".to_string(),
            "XfmrCode".to_string(),
            "XRConst".to_string(),
        ],
        "XfmrCode is slot 39, between Bank and XRConst"
    );
}

/// `like=` copies the code NAME (`MakeLike`, `AutoTrans.pas:848`); the
/// electrical data rides in with the winding/impedance copies that surround it,
/// so the clone is the same machine without re-resolving the code.
#[test]
fn make_like_carries_the_code_name_and_the_model() {
    let mut dss = coded();
    dss.command("New AutoTrans.t2 like=t1");
    dss.command("~ wdg=1 bus=sourcebus");
    dss.command("~ wdg=2 bus=mid2");
    dss.command("~ wdg=3 bus=tert2");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "AutoTrans.t2.XfmrCode"), "c3");
    assert_eq!(
        query(&mut dss, "AutoTrans.t2.conns"),
        "[series, wye, delta, ]"
    );
    dss.command("Edit AutoTrans.t2 wdg=3");
    assert_eq!(query(&mut dss, "AutoTrans.t2.RDCOhms"), "0.05");
}

/// Which surfaces show the row. `HIDE_R4133` keeps the r4133-only name off the
/// 0.14.5-pinned full-enumeration surfaces (`Dump`, `Dump commands`, JSON,
/// schema) — no pinned capture can produce that byte — but `Save` is
/// deliberately outside that set: r4133's `TAutoTransObj.SaveWrite` writes every
/// explicitly-set property except the winding scalars it rewrites as arrays
/// (`AutoTrans.pas:1171-1200`), flag-blind, so a deck that named a code gets it
/// back. The `?` surface exposes it either way (the tests above read it).
#[test]
fn save_writes_the_code_name_while_dump_hides_it() {
    let dir = std::env::temp_dir().join(format!("dss_autoxc_save_{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();

    let mut dss = coded();
    dss.command(&format!(
        "Save circuit dir=\"{}\"",
        dir.to_string_lossy().replace('\\', "/")
    ));
    let saved = std::fs::read_to_string(dir.join("AutoTrans.dss")).expect("saved class file");
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        saved.contains("XfmrCode=c3"),
        "Save writes the explicitly-set code name, as r4133's flag-blind \
         SaveWrite does: {saved}"
    );

    // The Dump text enumerates the property table and must skip the hidden row.
    // `Dump` writes `<circuit>_PropertyDump.txt` into the process-wide cwd, so
    // read it back by the exact path the command reports and delete it by name
    // (never a recursive wipe) before asserting, so a failure leaves nothing
    // behind. `coded()`'s circuit name (`xcauto`) is this file's alone.
    dss.command("Dump AutoTrans.t1");
    let path = dss.last_result_file().to_string();
    let dumped = std::fs::read_to_string(&path).expect("dump file");
    let _ = std::fs::remove_file(&path);
    assert!(
        dumped.contains("~ XHX=") && dumped.contains("~ XRConst="),
        "the Dump surface must still be complete around the hidden row: {dumped}"
    );
    assert!(
        !dumped.to_lowercase().contains("xfmrcode"),
        "a HIDE_R4133 row must not reach the 0.14.5-pinned Dump: {dumped}"
    );
}

/// `Nphases := Obj.Fnphases` is the setter `Set_NPhases`, which ignores a
/// non-positive value (`If Value>0 Then Fnphases := Value`, r4133
/// `Common/CktElement.pas:365-368`). An `XfmrCode` stores `phases=0` unguarded,
/// so fetching one must leave the element's phase count where it was, on the
/// auto (`AutoTrans.pas:2351`) and on the Transformer it was copied from
/// (`Transformer.pas:2333`), instead of zeroing it. Only the phase write is
/// dropped: the rest of the fetch still runs, which the code's `XHL` arriving
/// on both elements shows (a fetch that bailed out on the zero would keep the
/// phase count too).
#[test]
fn a_code_with_zero_phases_keeps_the_phase_count() {
    let mut dss = Dss::new();
    for line in [
        "New Circuit.zp basekv=69 phases=3 bus1=src",
        "New XfmrCode.z0 phases=3 windings=2 xhl=12.5",
        "New AutoTrans.a1 phases=3 windings=2",
        "~ wdg=1 bus=src conn=s kV=69 kVA=10000",
        "~ wdg=2 bus=lo conn=w kV=34.5 kVA=10000",
        "New Transformer.x1 phases=3 windings=2 buses=[src lo2] kVs=[69 34.5]",
    ] {
        dss.command(line);
        assert!(dss.errors().is_empty(), "`{line}` -> {:?}", dss.errors());
    }
    // r4133 stores `phases=0` on a code unguarded (`1: FNphases :=
    // Parser.IntValue`, `General/XfmrCode.pas:327`); the port's property row
    // refuses it ("cannot be zero", the 0.14.5 `NonZero` flag), so the state the
    // fetch must survive is written straight into the object.
    let ord = dss.class_by_name["xfmrcode"];
    let arena = &mut dss.classes[ord].arena;
    let z0 = (0..arena.len())
        .find(|&i| arena[i].data().name().eq_ignore_ascii_case("z0"))
        .expect("XfmrCode.z0 exists");
    arena
        .obj_mut(z0)
        .set_i32(crate::elements::general::xfmr_code::prop::PHASES, 0);
    assert_eq!(
        query(&mut dss, "XfmrCode.z0.phases"),
        "0",
        "the code holds 0"
    );
    assert_ne!(query(&mut dss, "AutoTrans.a1.XHX"), "12.5");
    assert_ne!(query(&mut dss, "Transformer.x1.XHL"), "12.5");

    for line in [
        "Edit AutoTrans.a1 xfmrcode=z0",
        "Edit Transformer.x1 xfmrcode=z0",
    ] {
        dss.command(line);
        assert!(dss.errors().is_empty(), "`{line}` -> {:?}", dss.errors());
    }
    assert_eq!(query(&mut dss, "AutoTrans.a1.phases"), "3");
    assert_eq!(query(&mut dss, "Transformer.x1.phases"), "3");
    assert_eq!(query(&mut dss, "AutoTrans.a1.XHX"), "12.5", "the fetch ran");
    assert_eq!(
        query(&mut dss, "Transformer.x1.XHL"),
        "12.5",
        "the fetch ran"
    );
}

/// `XfmrCode.{name}`: `np` phases, `nw` wye windings of `kva` kVA at `pct_r` %R.
fn code_lines(name: &str, np: u32, nw: u32, kva: f64, pct_r: f64) -> Vec<String> {
    let kvs: &[f64] = if np == 3 {
        &[69.0, 34.5, 13.8]
    } else {
        &[39.8, 19.9, 7.97]
    };
    let mut lines = vec![format!("New XfmrCode.{name} phases={np} windings={nw}")];
    for (i, kv) in kvs.iter().take(nw as usize).enumerate() {
        lines.push(format!(
            "~ wdg={} conn=wye kV={kv} kVA={kva} %r={pct_r}",
            i + 1
        ));
    }
    lines
}

/// `XfmrCode.k{np}{nw}`: `np` phases, `nw` wye windings of 10 MVA.
fn shape_code(np: u32, nw: u32) -> Vec<String> {
    code_lines(&format!("k{np}{nw}"), np, nw, 10000.0, 0.5)
}

/// `{kind}.t1` on `src`/`lo`, specified by `xfmrcode=` with the code of shape
/// `built`, solved. Both codes are defined in the same order in every deck.
fn shape_deck(kind: &str, codes: [(u32, u32); 2], built: (u32, u32)) -> Dss {
    let lines: Vec<String> = codes
        .iter()
        .flat_map(|&(np, nw)| shape_code(np, nw))
        .collect();
    code_deck(kind, &lines, &format!("k{}{}", built.0, built.1))
}

/// `{kind}.t1` on `src`/`lo`, specified by `xfmrcode={built}` after the code
/// definitions `codes`, solved.
fn code_deck(kind: &str, codes: &[String], built: &str) -> Dss {
    let mut dss = Dss::new();
    let mut lines = vec!["New Circuit.rf basekv=69 phases=3 bus1=src".to_string()];
    lines.extend(codes.iter().cloned());
    lines.extend([
        format!("New {kind}.t1 xfmrcode={built}"),
        "~ wdg=1 bus=src".to_string(),
        "~ wdg=2 bus=lo".to_string(),
        "Set tolerance=1e-10".to_string(),
        "Solve".to_string(),
    ]);
    for line in &lines {
        dss.command(line);
        assert!(dss.errors().is_empty(), "`{line}` -> {:?}", dss.errors());
    }
    assert!(dss.circuit().unwrap().is_solved, "the deck must solve");
    dss
}

/// Solve with `{kind}.t1` built from code `from`, re-fetch code `to` (a
/// different terminal shape) with no `bus=` after it, re-solve: the element must
/// end up exactly the machine a deck built from `to` directly holds.
fn assert_refetch_reprocesses(kind: &str, from: (u32, u32), to: (u32, u32)) {
    let codes = [from, to];
    let mut refetched = shape_deck(kind, codes, from);
    let edit = format!("Edit {kind}.t1 xfmrcode=k{}{}", to.0, to.1);
    refetched.command(&edit);
    assert!(refetched.errors().is_empty(), "{:?}", refetched.errors());
    assert!(
        refetched.circuit().unwrap().bus_name_redefined,
        "`{edit}` re-created the terminals, so it must flag BusNameRedefined"
    );
    refetched.command("Solve");
    assert!(refetched.errors().is_empty(), "{:?}", refetched.errors());
    assert!(
        refetched.circuit().unwrap().is_solved,
        "the re-solve converges"
    );

    let mut direct = shape_deck(kind, codes, to);
    assert_same_machine(kind, &mut refetched, &mut direct);
}

/// `{kind}.t1`'s terminals as `(bus name, node references)`: the wiring
/// `ReProcessBusDefs` gives each terminal, which a re-created terminal loses
/// (`Terminal::init` leaves no bus and all-ground node references).
fn t1_terminals(dss: &Dss, kind: &str) -> Vec<(Option<String>, Vec<usize>)> {
    let ckt = dss.circuit().unwrap();
    let arena = &dss.classes[dss.class_by_name[kind.to_ascii_lowercase().as_str()]].arena;
    let t1 = (0..arena.len())
        .find(|&i| arena[i].data().name().eq_ignore_ascii_case("t1"))
        .expect("t1 exists");
    arena
        .try_ckt_elem(t1)
        .expect("t1 is a circuit element")
        .cd()
        .terminals
        .iter()
        .map(|t| {
            let bus = t.bus_ref.and_then(|b| ckt.bus_list.name(b));
            (bus.map(str::to_string), t.term_node_ref.clone())
        })
        .collect()
}

/// `re` (re-fetched, re-solved) holds exactly the machine `direct` (built from
/// the new code) holds: `{kind}.t1`'s terminal wiring (bus names and node
/// references), the assembled Y, the node voltages, and `{kind}.t1`'s terminal
/// currents and powers. The wiring is compared on its own because the other
/// four read the element's flat `NodeRef`, which a re-created terminal leaves
/// in place, so only the wiring shows terminals that were re-created and never
/// re-wired.
fn assert_same_machine(kind: &str, re: &mut Dss, direct: &mut Dss) {
    let wiring = t1_terminals(direct, kind);
    assert!(
        wiring.iter().all(|(bus, _)| bus.is_some()),
        "{kind}: the direct deck wires every terminal: {wiring:?}"
    );
    assert_eq!(
        t1_terminals(re, kind),
        wiring,
        "{kind}: re-fetched t1 terminal wiring differs"
    );
    let (y_re, v_re) = solved(re);
    let (y_direct, v_direct) = solved(direct);
    assert_eq!(
        y_re, y_direct,
        "{kind}: re-fetched Y differs from the direct one"
    );
    assert_eq!(v_re, v_direct, "{kind}: re-fetched voltages differ");
    let name = format!("{kind}.t1");
    let terminal = |dss: &mut Dss| {
        let t1 = dss
            .snapshot_elements()
            .into_iter()
            .find(|s| s.name.eq_ignore_ascii_case(&name))
            .expect("t1 is in the circuit");
        (t1.currents, t1.powers)
    };
    assert_eq!(
        terminal(&mut *re),
        terminal(&mut *direct),
        "{kind}: re-fetched t1 currents/powers differ"
    );
}

/// A re-fetch that changes the terminal shape after a solve. `SetNumWindings`
/// re-creates the terminals (blank bus references) and only `Set_NConds`
/// raises `BusNameRedefined` (`Common/CktElement.pas:359`), which with the
/// auto's correct `2 * Fnphases` never fires, so `fetch_xfmr_code` raises the
/// flag from the shape change itself. With that raise removed (measured
/// 2026-09-30 on every leg of this pin and the Transformer one) the port's
/// re-solve aborts with #482 on every leg: `element "t1" has no node
/// references` where the winding or conductor count grows, a singular system Y
/// where it shrinks.
///
/// r4133 behaves differently on every leg. Measured on the epri-worker (r4133
/// DLL), same decks with no `bus=` after the edit (2026-09-27; the 3 -> 2
/// winding and 1 -> 3 phase legs 2026-09-30): every `xfmrcode=` on an auto
/// also logs #100131 (`AutoTrans.pas:567`), and a 3-phase auto is off, on its
/// wrong `NConds := Fnphases + 1` (`AutoTrans.pas:2353`). That wrong value is
/// what reprocesses the buses wherever the auto ends up 3-phase: 2 -> 3
/// windings gains the `T1_3.1-3` nodes (with `LO.1` at 300 746 V on a 34.5 kV
/// winding), 1 -> 3 phases re-solves to r4133's own direct build of the code,
/// and 3 -> 2 windings drops the `T1_3` nodes but aborts the re-solve with
/// #482 (`Error Encountered in Solve: Invalid pointer operation`). 3-phase ->
/// 1-phase re-solves with no message on the old node list, every voltage
/// unchanged. The 1-phase auto (2 -> 3 windings), where that value is right,
/// is not reprocessed and its re-solve aborts with #482 (`Error Encountered in
/// Solve: Aborting`).
#[test]
fn a_shape_changing_code_after_a_solve_reprocesses_the_buses() {
    assert_refetch_reprocesses("AutoTrans", (3, 2), (3, 3)); // NTerms 2 -> 3; r4133: via Fnphases + 1
    assert_refetch_reprocesses("AutoTrans", (3, 3), (3, 2)); // NTerms 3 -> 2; r4133: #482
    assert_refetch_reprocesses("AutoTrans", (3, 2), (1, 2)); // NConds 6 -> 2; r4133: stale
    assert_refetch_reprocesses("AutoTrans", (1, 2), (3, 2)); // NConds 2 -> 6; r4133: via Fnphases + 1
    assert_refetch_reprocesses("AutoTrans", (1, 2), (1, 3)); // NTerms 2 -> 3; r4133: #482
}

/// The Transformer twin has the same hole in r4133 itself: its `NConds :=
/// Fnphases + 1` (`Transformer.pas:2335`) repeats the value `SetNumWindings`
/// just wrote (`:997`), so it never flags. Measured on the epri-worker (r4133
/// DLL; 2026-09-27, the 3 -> 2 winding leg and the legs from 1 phase
/// 2026-09-30): every leg that grows the winding or phase count aborts the
/// re-solve with #482 and gains no node (`T1_3`, `LO.2-3`), and every leg that
/// shrinks it re-solves on the old node list with every voltage unchanged. An
/// upstream bug, not reproduced: the port reprocesses and solves the new
/// machine.
#[test]
fn a_shape_changing_code_reprocesses_a_transformer_too() {
    assert_refetch_reprocesses("Transformer", (3, 2), (3, 3)); // NTerms 2 -> 3; r4133: #482
    assert_refetch_reprocesses("Transformer", (3, 3), (3, 2)); // NTerms 3 -> 2; r4133: stale
    assert_refetch_reprocesses("Transformer", (3, 2), (1, 2)); // NConds 4 -> 2; r4133: stale
    assert_refetch_reprocesses("Transformer", (1, 2), (3, 2)); // NConds 2 -> 4; r4133: #482
    assert_refetch_reprocesses("Transformer", (1, 2), (1, 3)); // NTerms 2 -> 3; r4133: #482
}

/// The converse: a code of the same shape keeps the terminals, so it raises
/// nothing (the Pascal `NConds :=` with the right value is a no-op too). With
/// no flag nothing re-wires them, so after re-fetching another code of the same
/// shape and re-solving, `t1`'s terminals must still carry the bus names and
/// node references a deck built from that code gives them, and the solved
/// machine must equal that deck's.
#[test]
fn a_same_shape_code_does_not_flag_the_buses() {
    let mut codes: Vec<String> = [(3, 2), (3, 3)]
        .iter()
        .flat_map(|&(np, nw)| shape_code(np, nw))
        .collect();
    codes.extend(code_lines("k32b", 3, 2, 12000.0, 0.7));
    for kind in ["AutoTrans", "Transformer"] {
        let mut dss = code_deck(kind, &codes, "k32");
        assert!(!dss.circuit().unwrap().bus_name_redefined);
        for code in ["k32", "k32b"] {
            dss.command(&format!("Edit {kind}.t1 xfmrcode={code}"));
            assert!(dss.errors().is_empty(), "{:?}", dss.errors());
            assert!(
                !dss.circuit().unwrap().bus_name_redefined,
                "{kind}: a same-shape code ({code}) must not reprocess the buses"
            );
        }
        dss.command("Solve");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        let mut direct = code_deck(kind, &codes, "k32b");
        assert_same_machine(kind, &mut dss, &mut direct);
    }
}

/// `bank=` (property 38) is a recorded r4133 divergence the port keeps: r4133
/// comments the assignment out (`38: {XfmrBank := Param};`, `AutoTrans.pas:519`)
/// and answers every write with `DoSimpleMsg('Bank Property not used with
/// AutoTrans object.', 100130)` (`:566`), so its `XfmrBank` stays `''`. The port
/// follows dss_capi 0.14.5 (`AutoTrans.pas:482` there): the name is accepted
/// without a message, stored, read back live by `?` (the getter reads the field,
/// not the parse store) and keys the CIM bank (`cim/power_xfmr.rs`): the
/// exported `PowerTransformer` is named after it, where an auto with no bank
/// is its own bank. Adopting #100130 is an open decision (`auto_trans/mod.rs`,
/// the `Bank` row); this pin holds the behaviour until it is taken.
#[test]
fn bank_is_stored_silently_where_r4133_logs_100130() {
    // `Export CIM100 fil=` writes the one combined file; it is read back by the
    // path the command reports and deleted by name.
    let cim = |dss: &mut Dss, tag: &str| {
        let path =
            std::env::temp_dir().join(format!("dss_autoxc_cim_{tag}_{}.xml", std::process::id()));
        let seen = dss.errors().len();
        dss.command(&format!(
            "Export CIM100 fil=\"{}\"",
            path.to_string_lossy().replace('\\', "/")
        ));
        assert_eq!(dss.errors().len(), seen, "{:?}", &dss.errors()[seen..]);
        let file = dss.last_result_file().to_string();
        let text = std::fs::read_to_string(&file).expect("CIM export file");
        let _ = std::fs::remove_file(&file);
        text
    };
    let mut plain = coded();
    let own_bank = cim(&mut plain, "plain");
    assert!(!own_bank.contains(">b7<"), "no bank named b7 without bank=");

    let mut dss = coded();
    let seen = dss.errors().len();
    dss.command("Edit AutoTrans.t1 bank=b7");
    assert!(
        dss.errors()[seen..].is_empty(),
        "the port accepts bank= silently (r4133: #100130): {:?}",
        &dss.errors()[seen..]
    );
    assert_eq!(query(&mut dss, "AutoTrans.t1.bank"), "b7");
    assert!(
        dss.errors()
            .iter()
            .all(|e| e.code != Some(100130) && !e.message.contains("Bank Property not used")),
        "#100130 is not logged: {:?}",
        dss.errors()
    );
    // The CIM consumer: the stored name keys the auto's bank.
    assert!(
        cim(&mut dss, "bank").contains(">b7<"),
        "the exported bank is named b7"
    );
}

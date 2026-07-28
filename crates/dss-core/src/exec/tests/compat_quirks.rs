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
    assert!(
        json.contains("2.7999999999999998E+000") && json.contains("-5.9999999999999998E-001"),
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

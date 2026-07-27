//! Stage F.3i — the physical end of IV.2 **row 2**'s *no split* verdict
//! (`DE_PASCALIZE_PLAN.md` Part IV.2; the measurement is in
//! `dss-core/src/compat.rs`'s "Dense inverse" section).
//!
//! The unit-level twin (`compat::tests::dense_inverse_kernels_differ_by_one_
//! ulp_on_an_ideal_switch`) pins that the two dense-inverse kernels land one ULP
//! apart on a `switch=yes r1=1e-6` impedance matrix. This file pins what that
//! ULP is worth: on the vendored `Test/AutoTrans/Auto1bus-step1.dss` — a 330 MVA
//! autotransformer behind a `mvasc3=2e6` near-ideal source, the
//! `large_near_ideal_source` tolerance family — the parity kernel keeps the
//! no-load switch split at **exactly zero**, while a single ULP of difference in
//! the switch admittance is worth `1.4524` of reported conductor power against
//! that family's `1e-1` floor (measured 2026-07-27: `y·ΔV` with `y = 1e9 S` and
//! `ΔV` ≈ 1 ULP of the 92.95 kV node = 15.6 mA; the flipped kernel read
//! `1.45235132964843072`, a −1 ULP perturbation of the parity inverse
//! `1.45235132836994740`).
//!
//! So this is a **zero-tolerance** test on purpose: it is the tripwire for any
//! future attempt to swap the dense-inverse kernel (faer LU included) — it fails
//! next to the documented reason instead of surfacing as an unexplained corpus
//! divergence. It runs in both lanes, since the row has no lane split.

use dss_core::exec::Dss;

#[test]
fn dense_inverse_keeps_the_ideal_switch_split_at_exactly_zero() {
    let deck = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/corpus/electricdss-tst/Test/AutoTrans/Auto1bus-step1.dss"
    );
    let mut dss = Dss::new();
    dss.command(&format!("compile \"{deck}\""));
    assert!(dss.errors().is_empty(), "compile: {:?}", dss.errors());

    let snap = dss.snapshot_elements();
    let low = snap
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case("Line.low"))
        .expect("Line.low is in the snapshot");

    // The LV switch carries no load current in the deck's no-load check, and
    // the oracle reports exactly 0 — reachable only while both switch-coupled
    // nodes solve to bit-identical voltages.
    for (k, p) in low.powers.iter().enumerate() {
        assert_eq!(
            p.re, 0.0,
            "Line.low conductor {k} active power must be exactly 0 (dense-inverse row is no-split)"
        );
    }
    for (k, i) in low.currents.iter().enumerate() {
        assert_eq!(i.re, 0.0, "Line.low conductor {k} current re");
        assert_eq!(i.im, 0.0, "Line.low conductor {k} current im");
    }

    // Non-vacuity: the deck really is energized — the transformer draws its
    // (tiny, cancellation-dominated) no-load current, which is the quantity the
    // ULP amplification would move.
    let t1a = snap
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case("Transformer.t1a"))
        .expect("Transformer.t1a is in the snapshot");
    assert!(
        t1a.currents[0].norm() > 1e-3,
        "t1a must be energized, got {:?}",
        t1a.currents[0]
    );
}

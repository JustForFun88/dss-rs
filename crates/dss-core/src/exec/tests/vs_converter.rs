//! WP7.8 VSConverter targeted gate. A well-conditioned converter operating point —
//! a stiff DC source pins `Vdc`, and the AC modulation `m0=0.5` gives a definite
//! source/grid mismatch so the AC terminal current is large and uniquely
//! determined. The element is a 2-terminal AC/DC bridge: the first `phases - Ndc`
//! conductors are AC (a voltage source `Vdc·0.353553·m0∠d0` behind `Rac+jXac`),
//! the last `Ndc` are DC (a power-balance current source `Idc = Pac/|Vdc|`).
//!
//! **The oracle's `VSConverter.Currents` is buggy and is NOT pinned.** Pascal
//! `TVSConverterObj.GetCurrents` calls `GetInjCurrents(ComplexBuffer)`, and inside
//! `GetInjCurrents` the line `YPrim.MVMult(Curr, ComplexBuffer)` has `Curr` ==
//! `ComplexBuffer` (a self-aliased matrix-vector multiply) and then reads
//! `ComplexBuffer` again for the `Pac` power estimate — so the *reported* converter
//! currents violate KCL (here the oracle reports |I_ac| ≈ 1248 A where the physics
//! demand ≈ 390 A). The Rust port computes the **physically-correct** current (it
//! matches the oracle's *source* currents by KCL exactly), and we deliberately do
//! not reproduce the upstream self-report bug (cf. the WP7.6 `Powers`-before-
//! `Currents` oracle quirk — "не порти баг эталона"). The converter is therefore
//! gated against the oracle's **correctly-reported source currents + KCL**, which
//! validate its effect on the solved circuit without touching the buggy self-report.

use crate::exec::*;

fn vsc_dss() -> Dss {
    let mut dss = Dss::new();
    dss.command("Set DefaultBaseFrequency=60");
    dss.command(
        "New Circuit.t basekv=0.48 phases=3 bus1=src pu=1.0 r1=0.01 x1=0.05 r0=0.01 x0=0.05",
    );
    // A stiff 1 kV DC source directly on the converter's DC node pins Vdc.
    dss.command("New Vsource.dc bus1=src.4 basekv=1.0 pu=1.0 phases=1 r1=0.001 x1=0.0");
    dss.command(
        "New VSConverter.v1 phases=4 Ndc=1 bus1=src.1.2.3.4 kVac=0.48 kVdc=1.0 kW=50 \
         Rac=0.05 Xac=0.2 m0=0.5 d0=0",
    );
    dss.command("Set voltagebases=[0.48, 1.0]");
    dss.command("calcv");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "solve: {:?}", dss.errors());
    dss
}

#[test]
fn vsconverter_circuit_matches_oracle() {
    let mut dss = vsc_dss();
    // Same converged iteration count as the oracle (dss-python 0.15.7): 3.
    assert_eq!(
        dss.circuit().unwrap().solution.iteration,
        3,
        "iteration count"
    );

    let snaps = dss.snapshot_elements();
    let get = |name: &str| {
        snaps
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
            .unwrap_or_else(|| panic!("element {name} not found"))
    };

    // The converter's effect on the circuit is determined by the (correctly-
    // reported) source currents — these are the oracle pins. The main 3-phase
    // source's terminal-1 currents (re, im), dss-python 0.15.7 at full f64
    // precision (Rust matches these bit-for-bit — agreement is ~1e-10 rel; pinned
    // at the project's calibrated 1e-6 faer-vs-KLU current floor, TOLERANCE_NOTES).
    let main = get("Vsource.source");
    let want_src: [(f64, f64); 3] = [
        (-91.0829860344, 379.5124412447),
        (374.2089081749, -110.8760409062),
        (-283.1259222135, -268.6364003169),
    ];
    for (k, &(re, im)) in want_src.iter().enumerate() {
        let (ar, ai) = (main.currents[k].re, main.currents[k].im);
        let mag = (re * re + im * im).sqrt();
        assert!(
            (ar - re).hypot(ai - im) < 1e-6 * mag,
            "main source I[{k}] = ({ar:.10}, {ai:.10}) vs oracle ({re}, {im})"
        );
    }
    // The DC source carries the power-balance current Idc (oracle 48.2995954896 A,
    // full f64; Rust matches to ~2e-12 rel).
    let dc = get("Vsource.dc");
    assert!(
        (dc.currents[0].re - 48.2995954896).hypot(dc.currents[0].im) < 1e-6 * 48.2995954896,
        "DC source I = ({:.10}, {:.10})",
        dc.currents[0].re,
        dc.currents[0].im
    );

    // KCL ties the converter to the oracle-pinned sources: the VSConverter's AC
    // terminal-1 currents are the exact negatives of the main-source currents
    // (only the source and the converter share src.1/2/3). This validates the
    // converter's physically-correct current (≈390 A, NOT the oracle's buggy
    // self-reported ≈1248 A). The KCL residual is ~2e-8 rel (the solver floor).
    let v = get("VSConverter.v1");
    for k in 0..3 {
        let (vr, vi) = (v.currents[k].re, v.currents[k].im);
        let (sr, si) = (main.currents[k].re, main.currents[k].im);
        let mag = (sr * sr + si * si).sqrt();
        assert!(
            (vr + sr).hypot(vi + si) < 1e-6 * mag,
            "KCL: VSConverter AC I[{k}] must equal -(source I[{k}])"
        );
    }
    // The converter's DC terminal current is its post-solve *recomputed* Idc
    // (GetCurrents re-runs GetInjCurrents), which carries the model's one-iteration
    // lag, so it differs from the *injected* Idc — the DC-source current pinned
    // above — by the lag (~0.004 A, ~9e-5 rel). The oracle's own DC value is
    // corrupted by the self-alias bug (it reports 49.998 A), so this is a tight
    // regression guard against the verified Rust f64, not an oracle pin.
    let (vdr, vdi) = (v.currents[3].re, v.currents[3].im); // term1 cond4 = DC
    assert_eq!(vdi, 0.0, "VSConverter DC current must be purely real");
    assert!(
        (vdr.hypot(vdi) - 48.3039943741).abs() < 1e-6 * 48.3039943741,
        "VSConverter DC current magnitude = {vdr:.10}"
    );

    // Terminal 2 AC conductors are the exact series mirror of terminal 1 — in
    // BOTH components. The real-part arm is the original assertion (unchanged);
    // the imaginary arm is the W3 settler's strengthening (the pre-existing
    // shape only checked `re`, so a mirror broken purely in reactive current
    // would have passed). Measured 2026-07-26: both residuals are *exactly* 0.0
    // (the series element negates terminal 1 into terminal 2), so the arm holds
    // with room to spare at the same 1e-6 bound / denominator shape as the
    // real one.
    for k in 0..3 {
        let denom = v.currents[k].re.abs().max(1.0);
        assert!(
            (v.currents[k].re + v.currents[k + 4].re).abs() < 1e-6 * denom,
            "VSConverter I_term2[{k}] must equal -I_term1[{k}]"
        );
        let denom_im = v.currents[k].im.abs().max(1.0);
        assert!(
            (v.currents[k].im + v.currents[k + 4].im).abs() < 1e-6 * denom_im,
            "VSConverter I_term2[{k}] (imag) must equal -I_term1[{k}]"
        );
    }
}

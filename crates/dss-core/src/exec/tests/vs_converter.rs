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
    // source's terminal-1 currents (re, im), dss-python 0.15.7:
    let main = get("Vsource.source");
    let want_src: [(f64, f64); 3] = [(-91.083, 379.51), (374.21, -110.88), (-283.13, -268.64)];
    for (k, &(re, im)) in want_src.iter().enumerate() {
        let (ar, ai) = (main.currents[2 * k], main.currents[2 * k + 1]);
        let mag = (re * re + im * im).sqrt();
        assert!(
            (ar - re).hypot(ai - im) < 1e-3 * mag,
            "main source I[{k}] = ({ar:.4}, {ai:.4}) vs oracle ({re}, {im})"
        );
    }
    // The DC source carries the power-balance current Idc (oracle 48.3 A).
    let dc = get("Vsource.dc");
    assert!(
        (dc.currents[0] - 48.3).hypot(dc.currents[1]) < 1e-3 * 48.3,
        "DC source I = ({}, {})",
        dc.currents[0],
        dc.currents[1]
    );

    // KCL ties the converter to the oracle-pinned sources: the VSConverter's AC
    // terminal-1 currents are the exact negatives of the main-source currents
    // (only the source and the converter share src.1/2/3). This validates the
    // converter's physically-correct current (≈390 A, NOT the oracle's buggy
    // self-reported ≈1248 A).
    let v = get("VSConverter.v1");
    for k in 0..3 {
        let (vr, vi) = (v.currents[2 * k], v.currents[2 * k + 1]);
        let (sr, si) = (main.currents[2 * k], main.currents[2 * k + 1]);
        let mag = (sr * sr + si * si).sqrt();
        assert!(
            (vr + sr).hypot(vi + si) < 1e-3 * mag,
            "KCL: VSConverter AC I[{k}] must equal -(source I[{k}])"
        );
    }
    // The converter's DC terminal current balances the DC source (KCL at src.4).
    let (vdr, vdi) = (v.currents[6], v.currents[7]); // term1 cond4 = DC
    assert!(
        (vdr.hypot(vdi) - 48.3).abs() < 1e-3 * 48.3,
        "VSConverter DC current magnitude = {}",
        vdr.hypot(vdi)
    );

    // Terminal 2 AC conductors are the exact series mirror of terminal 1.
    for k in 0..3 {
        let denom = v.currents[2 * k].abs().max(1.0);
        assert!(
            (v.currents[2 * k] + v.currents[2 * (k + 4)]).abs() < 1e-6 * denom,
            "VSConverter I_term2[{k}] must equal -I_term1[{k}]"
        );
    }
}

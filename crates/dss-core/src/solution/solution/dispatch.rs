//! The top-level `TSolutionObj.Solve` mode dispatcher and `SetVoltageBases`
//! (the `CalcVoltageBases` command).

use crate::circuit::Circuit;
use crate::compat;
use crate::solution::ymatrix::initialize_node_vbase;
use crate::util::sqrt3;

use super::dynamics::solve_dynamic;
use super::fault_study::solve_fault_study;
use super::harmonics::{solve_harmonic, solve_harmonic_t};
use super::monte_carlo::{solve_monte_fault, solve_monte1, solve_monte2, solve_monte3};
use super::power_flow::{solve_direct, solve_snap, solve_zero_load_snapshot};
use super::time_series::{
    solve_daily, solve_duty, solve_general_time, solve_ld1, solve_ld2, solve_peak_day, solve_yearly,
};
use super::{SolveEnv, SolveMode, SolveResult};

/// Pascal `TSolutionObj.Solve`: the mode dispatcher (Phase 3: Snapshot and
/// Direct; the rest report error 481/482 style messages).
pub fn solve(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    ckt.is_solved = false;
    ckt.solution_was_attempted = true;

    if ckt.emerg_min_volts >= ckt.normal_min_volts {
        env.errors.push(
            "Error: Emergency Min Voltage Must Be Less Than Normal Min Voltage! Solution Not Executed."
                .to_string(),
        );
        return Ok(());
    }
    if ckt.solution.solution_abort {
        env.errors.push("Solution aborted.".to_string());
        return Ok(());
    }

    // dss_capi 0.15.x `55400a29`: sync the global season index at the solved
    // hour so the seasonal report paths read it without re-evaluating the
    // XYCurve per element (Pascal `TSolutionObj.Solve`/`SolveSnap` etc.).
    crate::solution::meters::sync_seasonal_rating_idx(ckt, env.store);

    // Grid-forming inverter mode is fully ported: power-flow / time-series /
    // direct / harmonic (WPG.13) plus the dynamic-model GFM branch
    // (`DoDynamicMode`/`IntegrateStates` GFM, WPG.17). `is_dynamic_model` is TRUE
    // for Dynamic AND FaultStudy AND MonteFault (Pascal `Set_Mode`, Solution.pas
    // l.2088-2094); all three now drive the real GFM injection, so there is no
    // refusal. (MonteFault with an *empty* Faults list is an upstream NIL-deref
    // Access Violation — `PickAFault`/`Randomize`, SolutionAlgs.pas l.701/725 —
    // independent of GFM; the port does not reproduce that UB: `pick_a_fault`
    // safely returns `None` and the direct solve proceeds fault-free.)

    ckt.default_growth_factor = if ckt.solution.year == 0 {
        1.0
    } else {
        ckt.default_growth_rate.powi(ckt.solution.year - 1)
    };

    let result = match ckt.solution.mode {
        SolveMode::Snapshot => solve_snap(ckt, env),
        SolveMode::Yearly => solve_yearly(ckt, env),
        SolveMode::Daily => solve_daily(ckt, env),
        SolveMode::DutyCycle => solve_duty(ckt, env),
        SolveMode::PeakDay => solve_peak_day(ckt, env),
        SolveMode::Direct => solve_direct(ckt, env),
        SolveMode::Dynamic => solve_dynamic(ckt, env),
        SolveMode::FaultStudy => solve_fault_study(ckt, env),
        SolveMode::Harmonic => solve_harmonic(ckt, env),
        SolveMode::HarmonicT => solve_harmonic_t(ckt, env),
        SolveMode::LD1 => solve_ld1(ckt, env),
        SolveMode::LD2 => solve_ld2(ckt, env),
        SolveMode::Time => solve_general_time(ckt, env),
        SolveMode::Monte1 => solve_monte1(ckt, env),
        SolveMode::Monte2 => solve_monte2(ckt, env),
        SolveMode::Monte3 => solve_monte3(ckt, env),
        SolveMode::MonteFault => solve_monte_fault(ckt, env),
        // AutoAdd is intercepted at the executive layer (exec/auto_add.rs),
        // before this dispatcher runs, because its winner instantiation
        // re-enters the executive command path — it never reaches here. Every
        // other mode is ported, so this arm is defensive-only; it keeps the
        // Pascal-exact `TSolutionObj.Solve` else-branch error (#481).
        SolveMode::AutoAdd => {
            env.errors.push(crate::diag::DssDiagnostic::msg(
                "Unknown solution mode.",
                Some(481),
            ));
            Ok(())
        }
    };
    if let Err(e) = &result {
        env.errors.push(crate::diag::DssDiagnostic::msg(
            format!("Error Encountered in Solve: {e}"),
            Some(482),
        ));
        ckt.solution.solution_abort = true;
    }
    Ok(())
}

/// Pascal `nearestBasekV`: the legal base closest to `kv` in **relative**
/// distance (`|1 − kv/base|`), scanning `legal_bases` in order and stopping at
/// the first `0.0` entry; a tie keeps the earlier base.
fn nearest_base_kv(legal_bases: &[f64], kv: f64) -> f64 {
    let mut result = 0.0;
    let mut min_diff = 1.0e50;
    for &test_kv in legal_bases {
        if test_kv == 0.0 {
            break;
        }
        let diff = (1.0 - kv / test_kv).abs();
        if diff < min_diff {
            min_diff = diff;
            result = test_kv;
        }
    }
    result
}

/// Pascal `SetVoltageBases` (the `CalcVoltageBases` command): zero-load
/// solve, then assign each bus the nearest legal base.
pub fn set_voltage_bases(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    // Pascal `SetVoltageBases` (Solution.pas l.1083): suppress the meter-zone
    // auto-build during the zero-load snapshot — the voltage bases are not
    // available yet — by forcing both gate flags TRUE, then rebuild the zones
    // explicitly once `kVBase` is set, so `AddToVoltBaseList` sees valid bases.
    let saved_zones_computed = ckt.meter_zones_computed;
    let saved_zones_locked = ckt.zones_locked;
    ckt.meter_zones_computed = true;
    ckt.zones_locked = true;

    solve_zero_load_snapshot(ckt, env)?;

    for b in 0..ckt.buses.len() {
        // Pascal `SetVoltageBases` (Solution.pas:1103, r4133 :2541): scale the
        // solved L-N magnitude into an L-L kV estimate, snap that to the
        // nearest legal base, and store the matched base back as an L-N value.
        // The scale is upstream's own `√3/1000` written truncated —
        // `compat::kv_base_search_scale` is its Stage F lane row (the divide
        // below is the full-precision `SQRT3` of the same Pascal statement,
        // identical in both lanes).
        let vref = ckt.buses[b].get_ref(0);
        let kv_est = ckt.solution.node_v[vref].norm() * compat::kv_base_search_scale();
        let base = nearest_base_kv(&ckt.legal_voltage_bases, kv_est);
        ckt.buses[b].kv_base = base / sqrt3();
    }

    initialize_node_vbase(ckt); // for convergence test
    ckt.is_solved = true;

    // Now build the meter zones with the freshly-assigned voltage bases.
    ckt.meter_zones_computed = saved_zones_computed;
    ckt.zones_locked = saved_zones_locked;
    crate::solution::meters::do_reset_meter_zones(ckt, env.store);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::Dss;

    /// The two legal bases of the knife-edge fixture, and the L-L kV a source
    /// must present so that the **exact** scale lands just above their tie and
    /// the **truncated** one just below it.
    const TIE_BASES: [f64; 2] = [12.47, 13.2];
    const TIE_SOURCE_KV: f64 = 12.8248;

    /// The Stage F `kv_base_search_scale` row at the argmin it feeds.
    ///
    /// The truncated literal is 2.93e-5 relative below the `SQRT3/1000` the
    /// same Pascal statement names, and `nearestBasekV` is a *relative*-distance
    /// argmin — so the two scales can only disagree when the estimate sits
    /// within that 2.93e-5 of a tie between two adjacent legal bases. This pins
    /// both sides: identical selection away from the tie, opposite selections
    /// inside the window.
    #[test]
    fn kv_base_search_scale_moves_only_a_tied_selection() {
        let trunc = compat::kv_base_search_scale_truncated_impl();
        let exact = compat::kv_base_search_scale_exact_impl();
        assert_eq!(trunc, 0.001732);
        assert_eq!(exact, sqrt3() / 1000.0);
        assert_eq!(
            compat::kv_base_search_scale(),
            if compat::ORACLE_PARITY { trunc } else { exact },
            "the alias must resolve to this build's lane kernel"
        );
        // The documented bound: the truncated literal searches 2.93e-5 low.
        let rel = (trunc / exact - 1.0).abs();
        assert!(
            (2.9333e-5..2.9334e-5).contains(&rel),
            "truncated-vs-exact scale gap moved: {rel:e}"
        );

        // In this metric two bases tie at their harmonic mean.
        let tie = 2.0 * TIE_BASES[0] * TIE_BASES[1] / (TIE_BASES[0] + TIE_BASES[1]);
        let v_ln = TIE_SOURCE_KV * 1000.0 / sqrt3();
        assert!(v_ln * exact > tie, "fixture must sit above the tie");
        assert!(v_ln * trunc < tie, "…and its truncated estimate below it");
        assert_eq!(nearest_base_kv(&TIE_BASES, v_ln * exact), TIE_BASES[1]);
        assert_eq!(nearest_base_kv(&TIE_BASES, v_ln * trunc), TIE_BASES[0]);

        // Away from the tie — every golden and every gated corpus deck — the
        // 2.93e-5 cannot reach the next base, so both scales pick the same one.
        for kv in [12.47, 13.2, 12.6, 13.0] {
            let v = kv * 1000.0 / sqrt3();
            assert_eq!(
                nearest_base_kv(&TIE_BASES, v * exact),
                nearest_base_kv(&TIE_BASES, v * trunc),
                "the scales must agree away from the tie ({kv} kV)"
            );
        }
    }

    /// The same row end-to-end, through the `CalcVoltageBases` command: the bus
    /// base the two lanes store for a feeder deliberately built inside the
    /// tie window differs by a whole legal base.
    #[test]
    fn calc_voltage_bases_snaps_to_the_lane_estimate() {
        let mut dss = Dss::new();
        dss.command(&format!("New circuit.tie basekv={TIE_SOURCE_KV} pu=1.0"));
        dss.command("New Line.l1 bus1=sourcebus bus2=b2 length=1 r1=0.1 x1=0.1 c1=0 c0=0");
        dss.command(&format!(
            "Set voltagebases=[{} {}]",
            TIE_BASES[0], TIE_BASES[1]
        ));
        dss.command("calcvoltagebases");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());

        let picked = if compat::ORACLE_PARITY {
            TIE_BASES[0]
        } else {
            TIE_BASES[1]
        };
        let expected = picked / sqrt3();
        let ckt = dss.circuit().expect("circuit");
        assert!(!ckt.buses.is_empty());
        for (i, bus) in ckt.buses.iter().enumerate() {
            assert!(
                (bus.kv_base - expected).abs() < 1e-12,
                "bus {i}: kVBase {} != {expected} (lane picked {picked} kV)",
                bus.kv_base
            );
        }
    }
}

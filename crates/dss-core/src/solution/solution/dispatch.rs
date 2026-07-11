//! The top-level `TSolutionObj.Solve` mode dispatcher and `SetVoltageBases`
//! (the `CalcVoltageBases` command).

use crate::circuit::Circuit;
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
            env.errors.push("Unknown solution mode.".to_string());
            Ok(())
        }
    };
    if let Err(e) = &result {
        env.errors.push(format!("Error Encountered in Solve: {e}"));
        ckt.solution.solution_abort = true;
    }
    Ok(())
}

/// Pascal `nearestBasekV`.
fn nearest_base_kv(ckt: &Circuit, kv: f64) -> f64 {
    let mut result = 0.0;
    let mut min_diff = 1.0e50;
    for &test_kv in &ckt.legal_voltage_bases {
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
        // TODO(compat): Pascal scales |V| by the literal 0.001732 (truncated
        // √3/1000) before the nearest-base search, then divides the matched
        // base by the full-precision SQRT3. Reproduced exactly; clean fix is
        // a single constant after final acceptance.
        let vref = ckt.buses[b].get_ref(0);
        let kv_est = ckt.solution.node_v[vref].norm() * 0.001732;
        ckt.buses[b].kv_base = nearest_base_kv(ckt, kv_est) / sqrt3();
    }

    initialize_node_vbase(ckt); // for convergence test
    ckt.is_solved = true;

    // Now build the meter zones with the freshly-assigned voltage bases.
    ckt.meter_zones_computed = saved_zones_computed;
    ckt.zones_locked = saved_zones_locked;
    crate::solution::meters::do_reset_meter_zones(ckt, env.store);

    Ok(())
}

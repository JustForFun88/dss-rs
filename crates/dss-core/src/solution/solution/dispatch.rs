//! The top-level `TSolutionObj.Solve` mode dispatcher and `SetVoltageBases`
//! (the `CalcVoltageBases` command).

use crate::circuit::Circuit;
use crate::elements::pc::pvsystem::PVSystem;
use crate::elements::pc::storage::Storage;
use crate::solution::ymatrix::initialize_node_vbase;
use crate::util::sqrt3;

use super::dynamics::solve_dynamic;
use super::fault_study::solve_fault_study;
use super::harmonics::{solve_harmonic, solve_harmonic_t};
use super::power_flow::{solve_direct, solve_snap, solve_zero_load_snapshot};
use super::time_series::{solve_daily, solve_duty, solve_peak_day, solve_yearly};
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

    // Grid-forming inverter mode (PVSystem `ControlMode=GFM`) is WP7.7
    // (dynamics): `DoGFM_Mode`/`CalcGFMYprim` are not ported. Refuse the solve
    // with an explicit error rather than silently running the regular PQ model,
    // which would give plausible-but-wrong numbers (the deferral-is-never-a-
    // silent-fallback convention). No gated case sets GFM.
    for r in ckt.pv_systems.clone() {
        let gfm_name = env
            .store
            .obj(r)
            .as_any()
            .downcast_ref::<PVSystem>()
            .and_then(|pv| {
                (pv.cd.enabled && pv.base.gfm_mode).then(|| pv.cd.obj.name().to_string())
            });
        if let Some(name) = gfm_name {
            env.errors.push(format!(
                "PVSystem.{name}: grid-forming inverter mode (ControlMode=GFM) is not \
                 ported yet (Phase 7 WP7.7)."
            ));
            ckt.solution.solution_abort = true;
            return Ok(());
        }
    }
    // Same guard for Storage `ControlMode=GFM` (DoGFM_Mode/CalcGFMYprim: WP7.7).
    for r in ckt.storages.clone() {
        let gfm_name = env
            .store
            .obj(r)
            .as_any()
            .downcast_ref::<Storage>()
            .and_then(|st| {
                (st.cd.enabled && st.base.gfm_mode).then(|| st.cd.obj.name().to_string())
            });
        if let Some(name) = gfm_name {
            env.errors.push(format!(
                "Storage.{name}: grid-forming inverter mode (ControlMode=GFM) is not \
                 ported yet (Phase 7 WP7.7)."
            ));
            ckt.solution.solution_abort = true;
            return Ok(());
        }
    }

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
        _ => {
            env.errors
                .push("Unknown solution mode.".to_string() + " (not ported in Phase 5)");
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

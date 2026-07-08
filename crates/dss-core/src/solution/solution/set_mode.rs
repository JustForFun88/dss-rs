//! Pascal `TSolutionObj.Set_Mode`.

use crate::circuit::Circuit;

use super::{ADMITTANCE, CONTROLSOFF, SolveMode, TIMEDRIVEN};

/// Pascal `TSolutionObj.Set_Mode` (`Solution.pas` l.2010): reset the clock,
/// revert control/load models, apply per-mode defaults. Returns whether the
/// mode was actually changed (the `OK_for_Dynamics`/`OK_for_Harmonics` guards
/// can refuse). The caller (the executive) runs the Pascal `Set_Mode` reset
/// tail afterwards — `MonitorClass.ResetAll`, `EnergyMeterClass.ResetAll`,
/// `DoResetFaults`, `DoResetControls` (only when this returns `true`).
pub fn set_mode(ckt: &mut Circuit, value: SolveMode, errors: &mut Vec<String>) -> bool {
    let sol = &mut ckt.solution;
    sol.int_hour = 0;
    sol.t = 0.0;
    sol.update_dbl_hour();
    ckt.trapezoidal_integration = false;

    // Pascal `OK_for_Dynamics` / `OK_for_Harmonics`: entering a dynamics or
    // harmonics mode requires a solved circuit (errors 486/487). The
    // machine-state initialization behind a *successful* entry
    // (`calcInitialMachineStates` / `InitializeForHarmonics`) runs in the
    // executive `Set mode=` handler after this returns `true` (set_cmd.rs). The
    // Dynamic, Harmonic(T) and FaultStudy solves are ported (WP7.7 / WP7.6 /
    // WP7.9); the MonteFault solve still errors (no corpus case, WP7.9).
    let value_is_dynamic = matches!(
        value,
        SolveMode::MonteFault | SolveMode::Dynamic | SolveMode::FaultStudy
    );
    let value_is_harmonic = matches!(value, SolveMode::Harmonic | SolveMode::HarmonicT);
    if !ckt.solution.is_dynamic_model && value_is_dynamic && !ckt.is_solved {
        errors.push(
            "Circuit must be solved in a non-dynamic mode before entering Dynamics or Fault study modes!\nIf you attempted to solve, then the solution has not yet converged.".to_string(),
        );
        return false;
    }
    if !ckt.solution.is_harmonic_model
        && value_is_harmonic
        && !(ckt.is_solved && ckt.solution.frequency == ckt.fundamental)
    {
        errors.push(
            "Circuit must be solved in a fundamental frequency power flow or direct mode before entering Harmonics mode!".to_string(),
        );
        return false;
    }
    if ckt.solution.is_harmonic_model && !value_is_harmonic {
        // Leaving harmonics mode: reset to fundamental. (`InvalidateAllPCElements`
        // is covered by the frequency change forcing a Y rebuild.)
        let fundamental = ckt.fundamental;
        ckt.solution.set_frequency(fundamental, fundamental);
    }
    // NOT_PORTED(WP7.7 step 2): Pascal `OK_for_Dynamics` (Solution.pas l.2185)
    // runs `ckt.InvalidateAllPCELEMENTS()` when *leaving* dynamics mode, to force
    // a YPrim recompute for machines whose primitive is mode-dependent (the
    // Norton `Yeq` they present in dynamics differs from their power-flow YPrim).
    // Inert until the DER `CalcYPrimMatrix` dynamics branch lands (step 2): no
    // currently-ported element has a dynamics-dependent YPrim, and the leave path
    // does not change frequency, so there is nothing to invalidate yet. Lands with
    // the machine YPrims in step 2 (needs the whole-circuit invalidate that reaches
    // the element store, unlike this `&mut Circuit`-only `set_mode`).

    let sol = &mut ckt.solution;
    sol.mode = value;
    sol.control_mode = sol.default_control_mode; // Revert to default mode
    sol.load_model = sol.default_load_model;
    sol.is_dynamic_model = false;
    sol.is_harmonic_model = false;
    sol.solution_initialized = false; // reinitialize solution when mode set (except dynamics)
    sol.preserve_node_voltages = false;
    sol.sample_the_meters = false;

    // Reset defaults for solution modes.
    match value {
        SolveMode::PeakDay | SolveMode::Daily => {
            sol.h = 3600.0;
            sol.number_of_times = 24;
            sol.sample_the_meters = true;
        }
        SolveMode::Snapshot => {
            sol.interval_hrs = 1.0;
            sol.number_of_times = 1;
        }
        SolveMode::Yearly => {
            sol.interval_hrs = 1.0;
            sol.h = 3600.0;
            sol.number_of_times = 8760;
            sol.sample_the_meters = true;
        }
        SolveMode::DutyCycle => {
            sol.h = 1.0;
            sol.control_mode = TIMEDRIVEN;
        }
        SolveMode::Dynamic => {
            sol.h = 0.001;
            sol.control_mode = TIMEDRIVEN;
            sol.is_dynamic_model = true;
            sol.preserve_node_voltages = true;
        }
        SolveMode::Time => {
            // GENERALTIME
            sol.interval_hrs = 1.0;
            sol.h = 3600.0;
            sol.number_of_times = 1; // just one time step per Solve call expected
        }
        SolveMode::Monte1 => {
            sol.interval_hrs = 1.0;
            sol.sample_the_meters = true;
        }
        SolveMode::Monte2 => {
            sol.h = 3600.0;
            sol.sample_the_meters = true;
        }
        SolveMode::Monte3 => {
            sol.interval_hrs = 1.0;
            sol.sample_the_meters = true;
        }
        SolveMode::MonteFault | SolveMode::FaultStudy => {
            sol.is_dynamic_model = true;
        }
        SolveMode::LD1 => {
            sol.h = 3600.0;
            ckt.trapezoidal_integration = true;
            sol.sample_the_meters = true;
        }
        SolveMode::LD2 => {
            sol.int_hour = 1;
            ckt.trapezoidal_integration = true;
            sol.sample_the_meters = true;
        }
        SolveMode::AutoAdd => {
            sol.interval_hrs = 1.0;
            // `ckt.AutoAddObj.ModeChanged := TRUE` is set after the match (it
            // borrows `ckt`, disjoint from the `sol` borrow held here).
        }
        SolveMode::Harmonic => {
            sol.control_mode = CONTROLSOFF;
            sol.is_harmonic_model = true;
            sol.load_model = ADMITTANCE;
            sol.preserve_node_voltages = true;
        }
        SolveMode::HarmonicT => {
            sol.interval_hrs = 1.0;
            sol.h = 3600.0;
            sol.number_of_times = 1;
            sol.control_mode = CONTROLSOFF;
            sol.is_harmonic_model = true;
            sol.load_model = ADMITTANCE;
            sol.preserve_node_voltages = true;
        }
        SolveMode::Direct => {}
    }
    // Pascal `TSolveMode.AUTOADDFLAG: ckt.AutoAddObj.ModeChanged := TRUE`
    // (Solution.pas l.2111) — forces `MakeBusList` to rebuild on the next
    // AutoAdd solve.
    if value == SolveMode::AutoAdd {
        ckt.auto_add_obj.mode_changed = true;
    }
    true
}

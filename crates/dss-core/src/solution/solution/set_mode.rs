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
    // Dynamic, Harmonic(T), FaultStudy and MonteFault solves are all ported
    // (WP7.7 / WP7.6 / WP7.9 / GAPS WPG.4).
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
        // Leaving harmonics mode (`OK_for_Harmonics`, Solution.pas l.2210):
        // `InvalidateAllPCELEMENTS()` + reset to fundamental. The invalidate's
        // observable half is the forced Y rebuild (see the dynamics-leave note
        // below) — set it unconditionally like Pascal, not only via the
        // frequency change (which is a no-op when the last solved harmonic was
        // already the fundamental).
        let fundamental = ckt.fundamental;
        ckt.solution.set_frequency(fundamental, fundamental);
        ckt.solution.system_y_changed = true;
    }
    // Pascal `OK_for_Dynamics` (Solution.pas l.2185): leaving dynamics mode runs
    // `ckt.InvalidateAllPCELEMENTS()` — YprimInvalid on every PC element plus
    // `SystemYChanged := TRUE` (Circuit.pas l.2321) — so the next solve rebuilds
    // the system Y without the machines' dynamics YPrims (the Norton `Yeq` a
    // Generator/IndMach012 presents in dynamics, the Storage/PVSystem GFM
    // short-circuit YPrim). In this port every `build_y_matrix` recomputes every
    // element's YPrim (the oracle's effective default, see ymatrix.rs), so the
    // observable half is exactly the rebuild trigger. (Ported at the WP8.8 exit
    // sweep — the old "inert, no mode-dependent YPrim exists" note predates the
    // WP7.7 machines and the WPG.13/WPG.17 GFM model.)
    if ckt.solution.is_dynamic_model && !value_is_dynamic {
        ckt.solution.system_y_changed = true;
    }

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

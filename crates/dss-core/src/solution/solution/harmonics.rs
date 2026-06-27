//! The harmonics solve mode: `SolveHarmonic` / `SolveHarmonicT`
//! (`SolutionAlgs.pas`), the frequency sweep (`CollectAllFrequencies` /
//! `AddFrequency`), `InitializeForHarmonics` (`Utilities.pas`) and the in-memory
//! fundamental-voltage save/restore (`savePresentVoltages` /
//! `RetrieveSavedVoltages`).
//!
//! Scope (WP7.6): the current-source harmonic family (VSource + Load; Isource is
//! not ported in this crate) landed in step 1, and the
//! voltage-source-behind-reactance DERs (Generator / PVSystem / Storage —
//! `InitHarmonics` / `DoHarmonicMode`) in step 2. Their per-element harmonic
//! injection runs through the shared `init_harmonics` / `inj_currents` hooks; no
//! solve-mode-level special-casing remains.

use crate::circuit::Circuit;
use crate::util::EPSILON;

use super::power_flow::{solve_direct, solve_snap};
use super::time_series::end_of_time_step_cleanup;
use super::{SolveEnv, SolveResult, sys_ctx};

/// Pascal `InitializeForHarmonics` (`Utilities.pas` l.960): save the present
/// (fundamental) voltages, then initialise every enabled PC element's harmonic
/// base values. Called from `OK_for_Harmonics` when entering harmonics mode (the
/// `Set mode=harmonics` handler), once the circuit is solved at fundamental.
/// Returns false if any element aborts the solution (mirrors `SolutionAbort`).
pub(crate) fn initialize_for_harmonics(ckt: &mut Circuit, env: &mut SolveEnv) -> bool {
    // `savePresentVoltages`: the original spills NodeV to a `.dbl` file; here we
    // keep the fundamental voltage vector in memory.
    ckt.solution.saved_node_v = ckt.solution.node_v.clone();

    let sys = sys_ctx(ckt);
    let node_v = ckt.solution.node_v.clone();
    for &r in &ckt.pc_elements {
        let elem = env.store.ckt_elem_mut(r);
        if elem.cd().enabled {
            elem.init_harmonics(&sys, &node_v);
            // Pascal `InitializeForHarmonics` `Exit`s the instant an element
            // aborts the solution (no element does in step 1, but step 2's DER
            // `InitHarmonics` can).
            if ckt.solution.solution_abort {
                return false;
            }
        }
    }
    !ckt.solution.solution_abort
}

/// Pascal `SolveHarmonic` (`SolutionAlgs.pas` l.1011): sweep the harmonic
/// frequency list, capturing the fundamental first. Each frequency forces a Y
/// rebuild (via `Set_Frequency`), and every non-fundamental frequency is solved
/// directly and sampled into the monitors.
pub(super) fn solve_harmonic(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    // Last solution was something other than fundamental: reset to it and reload
    // the saved fundamental voltages (Pascal `RetrieveSavedVoltages`).
    if ckt.solution.frequency != ckt.fundamental {
        let f = ckt.fundamental;
        ckt.solution.set_frequency(f, f);
        if !retrieve_saved_voltages(ckt) {
            env.errors
                .push("Saved results do not match present circuit. Aborting.".to_string());
            return Ok(());
        }
    }

    // Store the fundamental frequency in the monitors.
    crate::solution::monitors::sample_all_monitors(ckt, env, false);

    let freqs = harmonic_frequency_list(ckt, env);
    for f in freqs {
        ckt.solution.set_frequency(f, ckt.fundamental); // forces rebuild of SystemY
        if (ckt.solution.harmonic - 1.0).abs() > EPSILON {
            // Skip the fundamental (already solved + sampled above).
            solve_direct(ckt, env)?;
            crate::solution::monitors::sample_all_monitors(ckt, env, false);
            // Storage devices are assumed to stay the same (no time variation).
        }
    }
    // Pascal `MonitorClass.SaveAll()` flushes the per-monitor temp stream; this
    // port keeps the sample buffer in memory, so there is nothing to flush.
    Ok(())
}

/// Pascal `SolveHarmonicT` (`SolutionAlgs.pas` l.1097): sequential-time
/// harmonics — one fundamental power-flow step, then the harmonic sweep, with
/// `EndOfTimeStepCleanup` per frequency and a final `IncrementTime`.
pub(super) fn solve_harmonic_t(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    ckt.solution.interval_hrs = ckt.solution.h / 3600.0; // for energy meters / storage
    if ckt.solution.solution_abort {
        return Ok(());
    }

    if ckt.solution.frequency != ckt.fundamental {
        let f = ckt.fundamental;
        ckt.solution.set_frequency(f, f);
        if !retrieve_saved_voltages(ckt) {
            env.errors
                .push("Saved results do not match present circuit. Aborting.".to_string());
            return Ok(());
        }
    }

    if !initialize_for_harmonics(ckt, env) {
        return Ok(()); // SolutionAbort set by an element; abort the sweep
    }

    solve_snap(ckt, env)?;
    crate::solution::monitors::sample_all_monitors(ckt, env, false); // fundamental

    let freqs = harmonic_frequency_list(ckt, env);
    for f in freqs {
        ckt.solution.set_frequency(f, ckt.fundamental); // forces rebuild of SystemY
        if (ckt.solution.harmonic - 1.0).abs() > EPSILON {
            // SolveHarmTime: a snapshot solve at this frequency.
            solve_snap(ckt, env)?;
            crate::solution::monitors::sample_all_monitors(ckt, env, false);
            end_of_time_step_cleanup(ckt, env);
        }
    }
    ckt.solution.increment_time();
    Ok(())
}

/// The frequencies to solve at: every spectrum frequency in use
/// (`CollectAllFrequencies`) when `DoAllHarmonics`, else `harmonic_list` scaled
/// by the fundamental.
fn harmonic_frequency_list(ckt: &Circuit, env: &SolveEnv) -> Vec<f64> {
    if ckt.solution.do_all_harmonics {
        collect_all_frequencies(ckt, env)
    } else {
        ckt.solution
            .harmonic_list
            .iter()
            .map(|&h| ckt.fundamental * h)
            .collect()
    }
}

/// Pascal `CollectAllFrequencies` (`SolutionAlgs.pas` l.952): accumulate every
/// unique frequency in use. Pass 1 walks the sources at their own base frequency
/// (`GetSourceFrequency`); pass 2 walks the PC elements at the system
/// fundamental. `AddFrequency` dedups (within 0.1 Hz) and keeps the list sorted,
/// so iterating the elements directly yields the same set as the original
/// mark-then-collect over `SpectrumClass`.
fn collect_all_frequencies(ckt: &Circuit, env: &SolveEnv) -> Vec<f64> {
    let fundamental = ckt.fundamental;
    let mut list: Vec<f64> = Vec::new();

    for &r in &ckt.sources {
        let elem = env.store.ckt_elem(r);
        if !elem.cd().enabled {
            continue;
        }
        let f = elem.source_frequency().unwrap_or(fundamental);
        if let Some(spectrum) = elem.harmonic_spectrum()
            && let Some(harm) = spectrum.harmonics()
        {
            for &h in harm {
                add_frequency(&mut list, h * f);
            }
        }
    }

    for &r in &ckt.pc_elements {
        let elem = env.store.ckt_elem(r);
        if !elem.cd().enabled {
            continue;
        }
        if let Some(spectrum) = elem.harmonic_spectrum()
            && let Some(harm) = spectrum.harmonics()
        {
            for &h in harm {
                add_frequency(&mut list, h * fundamental);
            }
        }
    }

    list
}

/// Pascal `AddFrequency` (`SolutionAlgs.pas` l.895): add `f` to the ascending
/// list unless an entry already sits within 0.1 Hz of it (round-off tolerance).
fn add_frequency(list: &mut Vec<f64>, f: f64) {
    if list.iter().any(|&x| (f - x).abs() < 0.1) {
        return; // Already in list, nothing to do
    }
    let pos = list.iter().position(|&x| f < x).unwrap_or(list.len());
    list.insert(pos, f);
}

/// Pascal `RetrieveSavedVoltages` (`Utilities.pas` l.914): reload `NodeV` from
/// the saved fundamental vector when the node count still matches; otherwise the
/// saved results do not match the present circuit and the caller aborts.
fn retrieve_saved_voltages(ckt: &mut Circuit) -> bool {
    let saved = &ckt.solution.saved_node_v;
    if saved.is_empty() || saved.len() != ckt.solution.node_v.len() {
        return false;
    }
    ckt.solution.node_v.clone_from(&ckt.solution.saved_node_v);
    true
}

//! The `SolutionAlgs.pas` MonteCarlo solve modes: `SolveMonte1` (l.367),
//! `SolveMonte2` (l.418), `SolveMonte3` (l.491), and `SolveMonteFault` (l.725)
//! with `PickAFault` (l.701).
//!
//! Each mode draws through the engine-global FPC RNG ([`crate::support::mathutil::
//! FpcRng`], on `Circuit::rng`). Under `Set random=none` (`RandomType=0`) every
//! draw collapses to a deterministic constant — `Load.Randomize(0)` and
//! `Fault.Randomize`'s `else` set `RandomMult := 1.0`, and Monte2/Monte3's
//! `case Randomtype of` has no `none` branch so `LoadMultiplier` is left
//! unchanged — which is the only oracle-pinnable configuration (GAPS_PLAN.md
//! §2.1; the RNG generator itself is gated by the `rng.rs` fixed-seed tests).
//!
//! Upstream's `Randomize` for the per-load Monte1 multiplier lives inside
//! `SetNominalLoad` (the `MONTECARLO1` arm); here [`randomize_all_loads`] hoists
//! that draw to the top of each case, so the `MONTECARLO1` arm consumes the
//! freshly-drawn `RandomMult` (the net per-case effect and the engine-global RNG
//! stream both match — see the `Monte1` arm in `load/nominal.rs`).

use crate::circuit::Circuit;
use crate::elements::pc::load::Load;
use crate::elements::pd::fault::Fault;
use crate::elements::traits::{CktElement, ElemId};
use crate::support::mathutil::gauss;

use super::power_flow::{set_generator_disp_ref, solve_direct, solve_snap};
use super::time_series::{end_of_time_step_cleanup, sample_all_monitors_and_meters};
use super::{ADMITTANCE, GAUSSIAN, LOGNORMAL, SolveEnv, SolveResult, UNIFORM};
use crate::elements::traits::TypedStore;

/// Pascal `TLoadObj.Randomize` hoisted over every enabled load, in circuit
/// order — the per-case draw batch `SolveMonte1`'s `SetNominalLoad`/`MONTECARLO1`
/// arm would otherwise perform inline. One draw per load (except `random=none`,
/// which draws nothing), from the shared engine RNG.
fn randomize_all_loads(ckt: &mut Circuit, env: &mut SolveEnv, random_type: i32) {
    for r in ckt.loads.clone() {
        if let Some(load) = env.store.typed_mut::<Load>(r)
            && load.cd().enabled
        {
            load.randomize(random_type, &mut ckt.rng);
        }
    }
}

/// Pascal `TSolutionAlgs.SolveMonte1` (`SolutionAlgs.pas` l.367): `NumberOfTimes`
/// snapshot cases, `intHour` as the case counter, each load re-randomised per
/// case (`RandomMult` feeds the `MONTECARLO1` nominal factor). Does **not** open
/// the demand-interval files (unlike Monte2/Monte3), but its `finally` still
/// `SaveAll`s the monitors and — when `SampleTheMeters` — closes any open DI set.
pub(super) fn solve_monte1(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    // Always set LoadMultiplier WITH prop in case the matrix must be rebuilt.
    ckt.load_multiplier = 1.0;
    ckt.solution.interval_hrs = 1.0; // needed for energy meters and storage devices
    ckt.solution.int_hour = 0;
    ckt.solution.dbl_hour = 0.0; // Use hour to denote Case number
    ckt.solution.t = 0.0;

    let result = solve_monte1_body(ckt, env);
    // Pascal `finally`: MonitorClass.SaveAll(); if SampleTheMeters CloseAllDIFiles.
    crate::solution::monitors::save_all_monitors(ckt, env);
    if ckt.solution.sample_the_meters {
        crate::solution::meters::close_all_di_files(ckt, env.store, env.errors);
    }
    result
}

fn solve_monte1_body(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    let random_type = ckt.solution.random_type;
    for _ in 1..=ckt.solution.number_of_times {
        if ckt.solution.solution_abort {
            // Pascal `else` arm: SOLUTION_ABORT + 'Solution Aborted' + Break.
            env.errors.push("Solution Aborted".to_string());
            break;
        }
        ckt.solution.int_hour += 1;
        randomize_all_loads(ckt, env, random_type);
        solve_snap(ckt, env)?;
        // MonitorClass.SampleAll(); if SampleTheMeters EnergyMeterClass.SampleAll().
        let sample_meters = ckt.solution.sample_the_meters;
        sample_all_monitors_and_meters(ckt, env, sample_meters);
        // NB: Monte1 does NOT call EndOfTimeStepCleanup.
    }
    Ok(())
}

/// Pascal `TSolutionAlgs.SolveMonte2` (l.418): a daily load solution for
/// `NumberOfTimes` random days. `LoadMultiplier` is drawn once per day (UNIFORM
/// → `Random`, GAUSSIAN → `Gauss(DefaultDailyShape.Mean, .StdDev)`; no `none`
/// branch, so `random=none` leaves it unchanged), then an inner `Ndaily =
/// Round(24/IntervalHrs)` step loop advances the clock over the default daily
/// shape with `EndOfTimeStepCleanup`.
pub(super) fn solve_monte2(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    ckt.solution.t = 0.0;
    ckt.solution.int_hour = 0;
    ckt.solution.dbl_hour = 0.0;
    ckt.solution.interval_hrs = ckt.solution.h / 3600.0;
    if !ckt.em_di.di_files_are_open {
        crate::solution::meters::open_all_di_files(ckt, env.store);
    }
    let result = solve_monte2_body(ckt, env);
    crate::solution::monitors::save_all_monitors(ckt, env);
    if ckt.solution.sample_the_meters {
        crate::solution::meters::close_all_di_files(ckt, env.store, env.errors);
    }
    result
}

fn solve_monte2_body(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    let random_type = ckt.solution.random_type;
    // TODO(compat): FPC `Round` is banker's rounding (ties-to-even); this index
    // is always in i32 range, so `round_ties_even` reproduces it.
    let ndaily = (24.0 / ckt.solution.interval_hrs).round_ties_even() as i32;
    for _ in 1..=ckt.solution.number_of_times {
        if ckt.solution.solution_abort {
            // Note the trailing period — Monte2's abort text differs from
            // Monte1/Monte3 ('Solution Aborted.' vs 'Solution Aborted').
            env.errors.push("Solution Aborted.".to_string());
            break;
        }
        draw_load_multiplier(ckt, random_type, /*allow_lognormal=*/ false);
        for _ in 1..=ndaily {
            ckt.solution.increment_time();
            let dbl_hour = ckt.solution.dbl_hour;
            match ckt.default_daily_shape_obj.as_mut() {
                Some(shape) => ckt.default_hour_mult = shape.get_mult_at_hour(dbl_hour),
                None => return Err("Default daily load shape not found.".to_string()),
            }
            solve_snap(ckt, env)?;
            let sample_meters = ckt.solution.sample_the_meters;
            sample_all_monitors_and_meters(ckt, env, sample_meters);
            end_of_time_step_cleanup(ckt, env);
        }
    }
    Ok(())
}

/// Pascal `TSolutionAlgs.SolveMonte3` (l.491): time held fixed, only the global
/// `LoadMultiplier` varies across `NumberOfTimes` cases (UNIFORM/GAUSSIAN/
/// LOGNORMAL draws; `random=none` leaves it unchanged). `DefaultHourMult`/
/// `PriceSignal` are computed once, before the loop, from the fixed clock.
pub(super) fn solve_monte3(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    ckt.solution.interval_hrs = 1.0;
    if !ckt.em_di.di_files_are_open {
        crate::solution::meters::open_all_di_files(ckt, env.store);
    }
    // Time must be set before entering this routine (the deck's `Set time=`).
    let dbl_hour = ckt.solution.dbl_hour;
    match ckt.default_daily_shape_obj.as_mut() {
        Some(shape) => ckt.default_hour_mult = shape.get_mult_at_hour(dbl_hour),
        None => return Err("Default daily load shape not found.".to_string()),
    }
    if let Some(curve) = ckt.price_curve_obj.as_mut() {
        ckt.price_signal = curve.get_price(dbl_hour);
    }
    let result = solve_monte3_body(ckt, env);
    crate::solution::monitors::save_all_monitors(ckt, env);
    if ckt.solution.sample_the_meters {
        crate::solution::meters::close_all_di_files(ckt, env.store, env.errors);
    }
    result
}

fn solve_monte3_body(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    let random_type = ckt.solution.random_type;
    for _ in 1..=ckt.solution.number_of_times {
        if ckt.solution.solution_abort {
            env.errors.push("Solution Aborted".to_string());
            break;
        }
        draw_load_multiplier(ckt, random_type, /*allow_lognormal=*/ true);
        solve_snap(ckt, env)?;
        let sample_meters = ckt.solution.sample_the_meters;
        sample_all_monitors_and_meters(ckt, env, sample_meters);
        // NB: Monte3 does NOT call EndOfTimeStepCleanup.
    }
    Ok(())
}

/// The shared `case Randomtype of` `LoadMultiplier` draw of `SolveMonte2`
/// (l.448) / `SolveMonte3` (l.520). Monte2 lacks the LOGNORMAL arm; `none` (and
/// any unlisted type) leaves `LoadMultiplier` untouched — the deterministic
/// gated path.
fn draw_load_multiplier(ckt: &mut Circuit, random_type: i32, allow_lognormal: bool) {
    match random_type {
        UNIFORM => ckt.load_multiplier = ckt.rng.next_f64(),
        GAUSSIAN => {
            let (mean, std_dev) = ckt
                .default_daily_shape_obj
                .as_ref()
                .map(|s| (s.mean(), s.std_dev()))
                .unwrap_or((0.0, 0.0));
            let m = gauss(mean, std_dev, || ckt.rng.next_f64());
            ckt.load_multiplier = m;
        }
        LOGNORMAL if allow_lognormal => {
            let mean = ckt
                .default_daily_shape_obj
                .as_ref()
                .map(|s| s.mean())
                .unwrap_or(0.0);
            let m = crate::support::mathutil::quasi_log_normal(mean, || ckt.rng.next_f64());
            ckt.load_multiplier = m;
        }
        // none (0) and (for Monte2) LOGNORMAL: LoadMultiplier unchanged.
        _ => {}
    }
}

/// Pascal `TSolutionAlgs.PickAFault` (l.701): enable one randomly-chosen fault,
/// disable the rest. `Whichone = Trunc(Random·NumFaults)+1`, clamped to
/// `NumFaults`. With a single fault this is always fault 1, though the RNG draw
/// is still consumed (GAPS_PLAN.md §2.1). Returns the enabled fault's ref (the
/// Pascal `ActiveFaultObj`).
fn pick_a_fault(ckt: &mut Circuit, env: &mut SolveEnv) -> Option<ElemId> {
    let faults = ckt.faults.clone();
    let num_faults = faults.len();
    if num_faults == 0 {
        return None;
    }
    let raw = (ckt.rng.next_f64() * num_faults as f64).trunc() as i64 + 1;
    let whichone = raw.min(num_faults as i64);

    let mut active = None;
    for (i, &r) in faults.iter().enumerate() {
        let enable = (i as i64 + 1) == whichone;
        if let Some(fault) = env.store.typed_mut::<Fault>(r) {
            let was = fault.cd().enabled;
            fault.cd_mut().set_enabled(enable);
            if fault.cd().enabled != was {
                // Pascal `Set_Enabled` raises SystemYChanged (via the bus-name
                // redefine → Y rebuild); force the rebuild the direct solve gates on.
                ckt.solution.system_y_changed = true;
            }
        }
        if enable {
            active = Some(r);
        }
    }
    active
}

/// Pascal `TSolutionAlgs.SolveMonteFault` (l.725): `NumberOfTimes` direct
/// (ADMITTANCE) solves, each enabling one random fault (`PickAFault`) and
/// re-randomising its resistance (`Fault.Randomize`). Samples only the monitors
/// (no meter sampling — Set_Mode leaves `SampleTheMeters=false`).
pub(super) fn solve_monte_fault(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    ckt.solution.load_model = ADMITTANCE; // All direct solution
    ckt.load_multiplier = 1.0; // Always set WITH prop in case the matrix must rebuild
    ckt.solution.int_hour = 0;
    ckt.solution.dbl_hour = 0.0; // Use hour to denote Case number
    ckt.solution.t = 0.0;

    set_generator_disp_ref(ckt);

    let result = solve_monte_fault_body(ckt, env);
    // Pascal `finally`: MonitorClass.SaveAll() (no DI close — MF never opens them).
    crate::solution::monitors::save_all_monitors(ckt, env);
    result
}

fn solve_monte_fault_body(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    let random_type = ckt.solution.random_type;
    for _ in 1..=ckt.solution.number_of_times {
        // Pascal MF has NO `else` arm — an aborted step is simply skipped.
        if ckt.solution.solution_abort {
            continue;
        }
        ckt.solution.int_hour += 1;
        let picked = pick_a_fault(ckt, env); // Randomly enable one of the faults
        if let Some(r) = picked {
            // ActiveFaultObj.Randomize() — jitter the fault resistance.
            if let Some(fault) = env.store.typed_mut::<Fault>(r) {
                fault.randomize(random_type, &mut ckt.rng);
            }
            // `Fault.Randomize` sets YPrimInvalid := TRUE (CktElement.pas raises
            // SystemYChanged as the side effect); force the direct-solve rebuild.
            ckt.solution.system_y_changed = true;
        }
        solve_direct(ckt, env)?;
        crate::solution::monitors::sample_all_monitors(ckt, env, false);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Fixed-seed unit coverage for the RNG-DRIVEN dispatch that the gating
    //! decks never exercise (every Monte deck runs `random=none`, the
    //! deterministic 1.0 / unchanged path). Expected values are derived
    //! **externally** from the RNG draw sequence pinned in
    //! [`crate::support::mathutil`]`::rng` (the canonical MT19937 stream for seed
    //! 12345, independently verified there against `mt19937ar.out` / CPython),
    //! never captured from these functions' own output (GAPS_PLAN.md §2.1).

    use super::*;
    use crate::elements::general::load_shape::{self, LoadShapeObj};
    use crate::elements::traits::ElemStore;
    use crate::obj::base::DssObject;
    use dss_parser::{Parser, ParserVars};

    const SEED: u32 = 12345;
    /// First `FpcRng::next_f64()` draw for seed 12345 (`rng.rs`
    /// `f64_scaling_is_bit_exact`).
    const D0_BITS: u64 = 0x3fedbf6a3c400000;
    /// Second `next_f64()` draw for seed 12345 (`rng.rs`).
    const D1_BITS: u64 = 0x3fec7c25bca00000;
    /// First `Gauss(0,1)` result for seed 12345 (`rng.rs`
    /// `gauss_matches_fpc_bit_exact`). `Gauss(0,1) = (Σ12 Random − 6.0)`
    /// exactly (`·1.0`/`+0.0` are exact), so `Gauss(m,s) = G01·s + m` and
    /// `QuasiLognormal(m) = exp(G01)·m` bit-for-bit.
    const G01_0_BITS: u64 = 0x3fc62af569800000;

    fn seeded_ckt() -> Circuit {
        let mut ckt = Circuit::new("mc", 60.0);
        ckt.rng.set_seed(SEED);
        ckt
    }

    fn shape_with_mean_std(mean: f64, std_dev: f64) -> LoadShapeObj {
        let mut s = LoadShapeObj::new("mc_shape");
        s.set_f64(load_shape::prop::MEAN, mean);
        s.set_f64(load_shape::prop::STDDEV, std_dev);
        s
    }

    // --- draw_load_multiplier (SolveMonte2/SolveMonte3 LoadMultiplier draw) ---

    #[test]
    fn draw_load_multiplier_uniform_is_next_f64() {
        let mut ckt = seeded_ckt();
        ckt.load_multiplier = f64::NAN; // sentinel that MUST be overwritten
        draw_load_multiplier(&mut ckt, UNIFORM, true);
        assert_eq!(ckt.load_multiplier.to_bits(), D0_BITS);
    }

    #[test]
    fn draw_load_multiplier_gaussian_uses_default_daily_shape_mean_std() {
        let mut ckt = seeded_ckt();
        ckt.default_daily_shape_obj = Some(shape_with_mean_std(0.75, 0.20));
        draw_load_multiplier(&mut ckt, GAUSSIAN, true);
        let expected = f64::from_bits(G01_0_BITS) * 0.20 + 0.75;
        assert_eq!(ckt.load_multiplier, expected);
    }

    #[test]
    fn draw_load_multiplier_lognormal_monte3_uses_shape_mean() {
        let mut ckt = seeded_ckt();
        ckt.default_daily_shape_obj = Some(shape_with_mean_std(2.0, 0.30));
        draw_load_multiplier(&mut ckt, LOGNORMAL, /*allow_lognormal=*/ true);
        let expected = f64::from_bits(G01_0_BITS).exp() * 2.0;
        assert_eq!(ckt.load_multiplier, expected);
    }

    #[test]
    fn draw_load_multiplier_monte2_has_no_lognormal_arm() {
        // Monte2's `case Randomtype of` lacks a LOGNORMAL branch: LoadMultiplier
        // is left unchanged and NO draw is consumed (distinct from Monte3).
        let mut ckt = seeded_ckt();
        ckt.default_daily_shape_obj = Some(shape_with_mean_std(2.0, 0.30));
        ckt.load_multiplier = 42.0;
        draw_load_multiplier(&mut ckt, LOGNORMAL, /*allow_lognormal=*/ false);
        assert_eq!(ckt.load_multiplier, 42.0);
        assert_eq!(
            ckt.rng.next_f64().to_bits(),
            D0_BITS,
            "Monte2 LOGNORMAL must not consume a draw"
        );
    }

    #[test]
    fn draw_load_multiplier_none_leaves_unchanged_and_undrawn() {
        let mut ckt = seeded_ckt();
        ckt.load_multiplier = 42.0;
        draw_load_multiplier(&mut ckt, 0 /* random=none */, true);
        assert_eq!(ckt.load_multiplier, 42.0);
        assert_eq!(
            ckt.rng.next_f64().to_bits(),
            D0_BITS,
            "random=none must not consume a draw"
        );
    }

    // --- pick_a_fault (SolveMonteFault PickAFault) ---

    /// A test-only [`ElemStore`] backed by a real one-class `ClassArena` of
    /// `Fault` objects — the same typed storage production runs on, so
    /// `pick_a_fault`'s `TypedStore::typed_mut::<Fault>` resolves here exactly
    /// as it does live (a flat `Vec<Fault>` could not: the class ordinal in the
    /// `ElemId` has to be the real `Fault` one).
    struct FaultStore {
        arena: crate::obj::arena::ClassArena,
    }

    impl FaultStore {
        fn fault(&self, i: usize) -> &Fault {
            self.arena.get::<Fault>(i).expect("Fault arena")
        }
        fn fault_mut(&mut self, i: usize) -> &mut Fault {
            self.arena.get_mut::<Fault>(i).expect("Fault arena")
        }
        fn len(&self) -> usize {
            self.arena.len()
        }
    }

    impl ElemStore for FaultStore {
        fn obj_mut(&mut self, r: ElemId) -> &mut dyn DssObject {
            self.arena.obj_mut(r.index())
        }
        fn obj(&self, r: ElemId) -> &dyn DssObject {
            self.arena.obj(r.index())
        }
        fn kind(&self, _r: ElemId) -> crate::circuit::ElemKind {
            unimplemented!()
        }
        fn ckt_elem(&self, _r: ElemId) -> &dyn CktElement {
            unimplemented!()
        }
        fn ckt_elem_mut(&mut self, _r: ElemId) -> &mut dyn CktElement {
            unimplemented!()
        }
        fn find_ckt_element(&self, _full_name: &str) -> Option<ElemId> {
            None
        }
        fn find_general(&self, _class_name: &str, _obj_name: &str) -> Option<ElemId> {
            None
        }
        fn pair_mut(&mut self, _a: ElemId, _b: ElemId) -> (&mut dyn DssObject, &mut dyn DssObject) {
            unimplemented!()
        }
        fn triple_mut(
            &mut self,
            _a: ElemId,
            _b: ElemId,
            _c: ElemId,
        ) -> (&mut dyn DssObject, &mut dyn DssObject, &mut dyn DssObject) {
            unimplemented!()
        }
        fn arena(&self, _cls: usize) -> &crate::obj::arena::ClassArena {
            &self.arena
        }
        fn arena_mut(&mut self, _cls: usize) -> &mut crate::obj::arena::ClassArena {
            &mut self.arena
        }
        fn arena_pair_mut(
            &mut self,
            _a: usize,
            _b: usize,
        ) -> (
            &mut crate::obj::arena::ClassArena,
            &mut crate::obj::arena::ClassArena,
        ) {
            unimplemented!()
        }
        fn arena_triple_mut(
            &mut self,
            _a: usize,
            _b: usize,
            _c: usize,
        ) -> (
            &mut crate::obj::arena::ClassArena,
            &mut crate::obj::arena::ClassArena,
            &mut crate::obj::arena::ClassArena,
        ) {
            unimplemented!()
        }
    }

    fn n_faults(n: usize) -> FaultStore {
        let mut arena =
            crate::obj::arena::ClassArena::empty_for("Fault").expect("Fault arena variant");
        for i in 0..n {
            arena.push_new(&format!("f{i}"));
        }
        let mut store = FaultStore { arena };
        // Enable all up front so the "disable the rest" path is observable.
        for i in 0..n {
            store.fault_mut(i).cd_mut().set_enabled(true);
        }
        store
    }

    /// The typed handle of fault `idx` — the real `Fault` class ordinal, as the
    /// registry hands it out.
    fn fault_id(idx: usize) -> ElemId {
        <Fault as crate::obj::arena::ArenaClass>::id(idx)
    }

    #[test]
    fn pick_a_fault_multi_selects_the_trunc_random_index() {
        // N=3 faults, seed 12345. Each call draws once, then enables fault
        //   whichone = min(Trunc(Random·3)+1, 3)   (Pascal PickAFault).
        // Draws (rng.rs-pinned f64 for seed 12345):
        //   d0 = 0.9296160866506398 → Trunc(2.7888)+1 = 3 → idx 2  (== N: top
        //        boundary; the `.min(N)` clamp is defensive — Random<1 keeps
        //        Trunc(Random·N) ≤ N−1 so raw ≤ N always — reproduced 1:1)
        //   d1 = 0.8901547130662948 → Trunc(2.6705)+1 = 3 → idx 2
        //   d2 = 0.3163755603600293 → Trunc(0.9491)+1 = 1 → idx 0
        // montefault.dss has ONE fault (always idx 0), so this multi-fault index
        // computation is otherwise uncovered.
        let mut store = n_faults(3);
        let mut ckt = Circuit::new("mc", 60.0);
        ckt.rng.set_seed(SEED);
        ckt.faults = (0..3).map(fault_id).collect();
        let mut parser = Parser::new();
        let vars = ParserVars::new();
        let mut errors = crate::diag::ErrorLog::new();

        for &want in &[2usize, 2, 0] {
            let picked = {
                let mut env = SolveEnv {
                    store: &mut store,
                    parser: &mut parser,
                    vars: &vars,
                    errors: &mut errors,
                };
                pick_a_fault(&mut ckt, &mut env).expect("one fault is enabled")
            };
            assert_eq!(picked.index(), want, "returned the enabled fault's ref");
            for i in 0..store.len() {
                assert_eq!(
                    store.fault(i).cd().enabled,
                    i == want,
                    "only fault #{want} may stay enabled"
                );
            }
        }
    }

    #[test]
    fn pick_a_fault_single_is_always_first_but_consumes_a_draw() {
        // The montefault.dss configuration: one fault, always idx 0 — yet the
        // RNG draw is still consumed (GAPS_PLAN.md §2.1), so the stream advances.
        let mut store = n_faults(1);
        store.fault_mut(0).cd_mut().set_enabled(false);
        let mut ckt = Circuit::new("mc", 60.0);
        ckt.rng.set_seed(SEED);
        ckt.faults = vec![fault_id(0)];
        let mut parser = Parser::new();
        let vars = ParserVars::new();
        let mut errors = crate::diag::ErrorLog::new();

        let picked = {
            let mut env = SolveEnv {
                store: &mut store,
                parser: &mut parser,
                vars: &vars,
                errors: &mut errors,
            };
            pick_a_fault(&mut ckt, &mut env).expect("the one fault is enabled")
        };
        assert_eq!(picked.index(), 0);
        assert!(store.fault(0).cd().enabled);
        // Exactly one draw (d0) was consumed, so the next draw is d1.
        assert_eq!(ckt.rng.next_f64().to_bits(), D1_BITS);
    }
}

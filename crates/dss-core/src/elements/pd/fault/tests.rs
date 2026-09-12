use super::*;
use crate::elements::traits::{CktElement, SysCtx};
use crate::exec::Dss;
use crate::solution::{LoadSolutionModel, RandomType, SolveMode};
use crate::support::mathutil::FpcRng;

fn test_sys() -> SysCtx {
    SysCtx {
        frequency: 60.0,
        fundamental: 60.0,
        is_harmonic_model: false,
        is_dynamic_model: false,
        load_model: LoadSolutionModel::PowerFlow,
        mode: SolveMode::Snapshot,
        active_load_shape_class: crate::solution::USENONE,
        load_multiplier: 1.0,
        gen_multiplier: 1.0,
        generator_dispatch_reference: 0.0,
        price_signal: 25.0,
        default_growth_factor: 1.0,
        year: 0,
        dbl_hour: 0.0,
        solution_count: 0,
        iteration: 0,
        in_show_results: false,
        loads_need_updating: false,
        neglect_load_y: false,
        long_line_correction: false,
        positive_sequence: false,
        time_of_day: 0.0,
        dyna_h: 0.0,
        dyna_t: 0.0,
        iteration_flag: crate::support::dynamics::IterationFlag::NewTimeStep,
        last_solution_was_direct: false,
        ncim: false,
    }
}

/// Mirror the `phases=N` side effect for the direct-`calc_yprim` unit tests.
fn set_phases(f: &mut Fault, n: usize) {
    f.cd.nphases = n;
    f.cd.set_nconds(n);
    f.cd.yorder = f.cd.nterms * f.cd.nconds;
}

#[test]
fn default_is_1ph_grounded_shunt() {
    let f = Fault::new("f");
    assert_eq!(f.cd.nphases, 1);
    assert_eq!(f.cd.nconds, 1);
    assert_eq!(f.cd.nterms, 2);
    assert_eq!(f.cd.yorder, 2);
    assert!(f.is_shunt);
    assert_eq!(f.spec_type, 1);
    assert_eq!(f.g, 10000.0); // r = 1/10000
    assert!(f.is_on);
    assert_eq!(f.min_amps, 5.0);
    assert_eq!(f.get_bus_name(2), "f_1.0"); // pre-Bus1 grounded default
}

/// 1φ `r=1` → `G=1`; YPrim is the two-terminal `[[1,-1],[-1,1]]` (oracle-probed).
#[test]
fn yprim_1ph_r1_matches_oracle() {
    let mut f = Fault::new("f");
    f.g = 1.0; // r=1 → G=1
    f.calc_yprim(&test_sys());
    let yp = f.cd.yprim.as_ref().unwrap();
    assert_eq!(yp.get(0, 0), Complex64::new(1.0, 0.0));
    assert_eq!(yp.get(1, 1), Complex64::new(1.0, 0.0));
    assert_eq!(yp.get(0, 1), Complex64::new(-1.0, 0.0));
    assert_eq!(yp.get(1, 0), Complex64::new(-1.0, 0.0));
}

/// 3φ `r=2` → `G=0.5` per phase, uncoupled diagonal blocks (oracle-probed).
#[test]
fn yprim_3ph_r2_matches_oracle() {
    let mut f = Fault::new("f");
    set_phases(&mut f, 3);
    f.g = 0.5; // r=2 → G=0.5
    f.calc_yprim(&test_sys());
    let yp = f.cd.yprim.as_ref().unwrap();
    for i in 0..3 {
        assert_eq!(yp.get(i, i), Complex64::new(0.5, 0.0));
        assert_eq!(yp.get(i + 3, i + 3), Complex64::new(0.5, 0.0));
        assert_eq!(yp.get(i, i + 3), Complex64::new(-0.5, 0.0));
        assert_eq!(yp.get(i + 3, i), Complex64::new(-0.5, 0.0));
        // Off-diagonal within a block is zero (uncoupled).
        for j in 0..3 {
            if i != j {
                assert_eq!(yp.get(i, j), Complex64::ZERO);
            }
        }
    }
}

/// 2φ `Gmatrix=(1 | 0.5 2)` → the full symmetric conductance stamped in both
/// terminals with negated cross-blocks (oracle-probed).
#[test]
fn yprim_2ph_gmatrix_matches_oracle() {
    let mut f = Fault::new("f");
    set_phases(&mut f, 2);
    f.spec_type = 2;
    f.gmatrix = Some(vec![1.0, 0.5, 0.5, 2.0]); // row-major nphases²
    f.calc_yprim(&test_sys());
    let yp = f.cd.yprim.as_ref().unwrap();
    let g = [[1.0, 0.5], [0.5, 2.0]];
    for (i, row) in g.iter().enumerate() {
        for (j, &gij) in row.iter().enumerate() {
            let v = Complex64::new(gij, 0.0);
            assert_eq!(yp.get(i, j), v);
            assert_eq!(yp.get(i + 2, j + 2), v);
            assert_eq!(yp.get(i, j + 2), -v);
            assert_eq!(yp.get(j + 2, i), -v);
        }
    }
}

/// `Is_ON = false` stamps zero conductance (the fault is electrically absent).
#[test]
fn yprim_off_stamps_zero() {
    let mut f = Fault::new("f");
    f.g = 1.0;
    f.is_on = false;
    f.calc_yprim(&test_sys());
    let yp = f.cd.yprim.as_ref().unwrap();
    for i in 0..2 {
        for j in 0..2 {
            assert_eq!(yp.get(i, j), Complex64::ZERO);
        }
    }
}

#[test]
fn make_like_copies_spec() {
    let mut base = Fault::new("base");
    set_phases(&mut base, 3);
    base.g = 0.25;
    base.spec_type = 1;
    base.min_amps = 12.0;
    base.is_temporary = true;
    base.on_time = 2.0;

    let mut f = Fault::new("f");
    f.make_like(&base);
    assert_eq!(f.cd.nphases, 3);
    assert_eq!(f.cd.yorder, 6);
    assert_eq!(f.g, 0.25);
    assert_eq!(f.min_amps, 12.0);
    assert!(f.is_temporary);
    assert_eq!(f.on_time, 2.0);
}

/// End-to-end: a 3φ `r=5` fault on bus `b` pulls it down and converges in 2
/// iterations, drawing ~1342.8 A per conductor (oracle-probed full-circuit).
#[test]
fn snapshot_fault_pulls_bus_down() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.t basekv=12.47 phases=3 bus1=src basefreq=60",
        "new line.l1 bus1=src bus2=b r1=0.3 x1=0.6 length=1",
        "new fault.f phases=3 bus1=b r=5",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "engine errors: {:?}", dss.errors());
    let ckt = dss.circuit().expect("circuit");
    assert!(ckt.is_solved);
    assert_eq!(ckt.solution.iteration, 2);

    let snaps = dss.snapshot_elements();
    let fault = snaps
        .iter()
        .find(|s| s.name == "Fault.f")
        .expect("fault snapshot");
    // Conductor-1 terminal current magnitude (re/im interleaved).
    let imag = (fault.currents[0].re.powi(2) + fault.currents[0].im.powi(2)).sqrt();
    assert!(
        (imag - 1342.808).abs() < 0.5,
        "fault current {imag} vs oracle 1342.808"
    );
}

/// A temporary fault with `ONtime=1.5` starts off and applies once duty-mode
/// time passes 1.5 s, logging `**APPLIED**` (oracle-probed event log).
#[test]
fn temporary_fault_applies_in_duty_mode() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.t basekv=12.47 phases=3 bus1=src basefreq=60",
        "new line.l1 bus1=src bus2=b r1=0.3 x1=0.6 length=1",
        "new fault.f phases=3 bus1=b r=5 ontime=1.5 temporary=yes minamps=10",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "set mode=duty number=1 stepsize=1 hour=0",
        "solve", // t -> 1, below ONtime: stays off
        "solve", // t -> 2, above ONtime: applies
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "engine errors: {:?}", dss.errors());
    let applied = dss
        .event_log()
        .iter()
        .any(|s| s.contains("Element=Fault.f, Action=**APPLIED**"));
    assert!(
        applied,
        "expected a Fault.f **APPLIED** event; log = {:?}",
        dss.event_log()
    );
}

/// A temporary fault whose `MinAmps` exceeds its fault current self-clears the
/// step after it applies: `**APPLIED**` then `**CLEARED**` (oracle-probed).
#[test]
fn temporary_fault_clears_below_minamps() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.t basekv=12.47 phases=3 bus1=src basefreq=60",
        "new line.l1 bus1=src bus2=b r1=0.3 x1=0.6 length=1",
        // MinAmps above the ~1342 A fault current → FaultStillGoing is false.
        "new fault.f phases=3 bus1=b r=5 ontime=0.5 temporary=yes minamps=99999",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "set mode=duty number=1 stepsize=1 hour=0",
        "solve", // t -> 1, above ONtime: applies
        "solve", // t -> 2: current below MinAmps -> self-clears
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "engine errors: {:?}", dss.errors());
    let log = dss.event_log();
    assert!(
        log.iter()
            .any(|s| s.contains("Element=Fault.f, Action=**APPLIED**")),
        "expected **APPLIED**; log = {log:?}"
    );
    assert!(
        log.iter()
            .any(|s| s.contains("Element=Fault.f, Action=**CLEARED**")),
        "expected **CLEARED**; log = {log:?}"
    );
}

// --- TFaultObj.Randomize (the MonteFault per-fault resistance jitter) ---
//
// Fixed-seed coverage of the RNG-driven arms the gating decks never reach
// (montefault.dss runs `random=none`). Expected values are derived externally
// from the seed-12345 draw sequence pinned in `support::mathutil::rng` — the
// canonical MT19937 stream verified there against `mt19937ar.out` / CPython —
// never captured from `randomize` itself (GAPS_PLAN.md §2.1). `Gauss(0,1) =
// Σ12 Random − 6.0` exactly, so `Gauss(1,s) = G01·s + 1` and
// `QuasiLognormal(1) = exp(G01)` bit-for-bit.

/// First `next_f64()` draw for seed 12345 (`rng.rs`).
const RND_D0_BITS: u64 = 0x3fedbf6a3c400000;
/// First `Gauss(0,1)` result for seed 12345 (`rng.rs`).
const RND_G01_0_BITS: u64 = 0x3fc62af569800000;

#[test]
fn randomize_uniform_draws_next_f64() {
    let mut f = Fault::new("fx");
    let mut rng = FpcRng::from_seed(12345);
    f.randomize(RandomType::Uniform, &mut rng);
    assert_eq!(f.random_mult.to_bits(), RND_D0_BITS);
    assert!(f.cd().yprim_invalid, "Randomize forces a YPrim rebuild");
}

#[test]
fn randomize_gaussian_is_gauss_one_stddev() {
    let mut f = Fault::new("fx");
    f.stddev = 0.25;
    let mut rng = FpcRng::from_seed(12345);
    f.randomize(RandomType::Gaussian, &mut rng);
    let expected = f64::from_bits(RND_G01_0_BITS) * 0.25 + 1.0;
    assert_eq!(f.random_mult, expected);
}

#[test]
fn randomize_lognormal_is_quasi_lognormal_one() {
    let mut f = Fault::new("fx");
    let mut rng = FpcRng::from_seed(12345);
    f.randomize(RandomType::LogNormal, &mut rng);
    let expected = f64::from_bits(RND_G01_0_BITS).exp(); // QuasiLognormal(1.0)
    assert_eq!(f.random_mult, expected);
}

#[test]
fn randomize_none_sets_one_and_draws_nothing() {
    let mut f = Fault::new("fx");
    f.stddev = 0.25;
    let mut rng = FpcRng::from_seed(12345);
    f.randomize(RandomType::None, &mut rng);
    assert_eq!(f.random_mult, 1.0);
    assert_eq!(
        rng.next_f64().to_bits(),
        RND_D0_BITS,
        "random=none must not consume a draw"
    );
}

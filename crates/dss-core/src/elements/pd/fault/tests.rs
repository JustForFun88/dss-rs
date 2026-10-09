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

/// A `gmatrix` fault reduces to the positive-sequence self term of its nodal
/// conductance, `S(G) = (Σ G_ii − Σ_{i<j} G_ij) / 3` = 0.12 S for
/// `[0.1|-0.02 0.1|-0.02 -0.02 0.1]`. The action writes the resistance
/// `1 / S(G)` in ohms (the property stores its inverse), and the reduced fault
/// keeps its power and its `|V1| · S(G)` = about 863.95 A per phase, the
/// current of its `r=8.333333333333334` twin before and after. Both oracles
/// keep `G11` alone, 0.1 S.
#[test]
fn make_pos_sequence_gmatrix_fault_keeps_its_current() {
    use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx, balance};
    let g = [0.1, -0.02, -0.02, -0.02, 0.1, -0.02, -0.02, -0.02, 0.1];
    let gc: Vec<Complex64> = g.iter().map(|&v| Complex64::new(v, 0.0)).collect();
    let g1 = balance::seq11(&gc, 3).re;
    balance::assert_rel(g1, 0.12, 1e-12, "S(G)");

    let mut f = Fault::new("fg");
    set_phases(&mut f, 3);
    f.spec_type = 2;
    f.gmatrix = Some(g.to_vec());
    let plan = f.make_pos_sequence(&PosSeqCtx::default());
    assert_eq!(plan.actions.len(), 4, "{:?}", plan.actions);
    assert_eq!(plan.actions[0], PosSeqAction::BeginEdit);
    assert_eq!(plan.actions[1], PosSeqAction::SetI32(prop::PHASES, 1));
    match plan.actions[2] {
        PosSeqAction::SetF64(idx, r) if idx == prop::R => {
            balance::assert_rel(r, 1.0 / g1, 1e-12, "the R action carries ohms");
        }
        ref a => panic!("expected SetF64(R), got {a:?}"),
    }
    assert_eq!(plan.actions[3], PosSeqAction::EndEdit);

    let mut dss = balance::stiff("fltg");
    dss.command("new fault.fg bus1=b phases=3 gmatrix=[0.1 | -0.02 0.1 | -0.02 -0.02 0.1]");
    dss.command("new fault.fr bus1=b phases=3 r=8.333333333333334");
    let pairs = balance::reduce(&mut dss, &["Fault.fg", "Fault.fr"]);
    let [(fg0, fg1), (fr0, fr1)] = pairs[..] else {
        unreachable!()
    };
    balance::assert_kept(fg0, fg1, "Fault.fg");
    balance::assert_kept(fr0, fg0, "the r twin before makeposseq");
    balance::assert_kept(fr1, fg1, "the r twin after makeposseq");
    let v = balance::v1(&dss, "b");
    balance::assert_rel(
        fg1.re,
        3.0 * v * v * g1 / 1000.0,
        1e-9,
        "kW vs 3 |V1|² S(G)",
    );
    balance::assert_rel(
        balance::query_f64(&mut dss, "Fault.fg.r"),
        1.0 / g1,
        1e-12,
        "Fault.fg.r after makeposseq",
    );
    let snap = dss.snapshot_elements();
    let fg = snap
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case("Fault.fg"))
        .unwrap();
    balance::assert_rel(fg.currents[0].norm(), v * g1, 1e-9, "|I| vs |V1| S(G)");
}

/// Two phases reduce by the same self term, `S(G) = (G11 + G22 − G12) / 3`.
/// Under a balanced voltage the two-phase fault draws
/// `|V1|² (G11 + G22 − G12)`, three times `|V1|² S(G)`, so the one-phase
/// fault keeps its power: `[0.12 | 0 0.12]` gets an `R` of 12.5 ohm (about
/// 12440.07 kW) and `[0.12 | -0.02 0.12]` one of `3 / 0.26` ohm. The `r=`
/// spelling of the first fault keeps its per-phase value under the reduction,
/// which is open question 1, so it is not asserted here.
#[test]
fn make_pos_sequence_two_phase_gmatrix_fault_keeps_its_power() {
    use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx, balance};
    let cases = [
        ("f2", "[0.12 | 0 0.12]", [0.12, 0.0, 0.0, 0.12], 12.5),
        (
            "f2c",
            "[0.12 | -0.02 0.12]",
            [0.12, -0.02, -0.02, 0.12],
            3.0 / 0.26,
        ),
    ];
    for (name, _, g, r) in cases {
        let mut f = Fault::new(name);
        set_phases(&mut f, 2);
        f.spec_type = 2;
        f.gmatrix = Some(g.to_vec());
        let plan = f.make_pos_sequence(&PosSeqCtx::default());
        assert_eq!(plan.actions.len(), 4, "{name}: {:?}", plan.actions);
        assert_eq!(plan.actions[1], PosSeqAction::SetI32(prop::PHASES, 1));
        match plan.actions[2] {
            PosSeqAction::SetF64(idx, got) if idx == prop::R => {
                balance::assert_rel(got, r, 1e-12, name);
            }
            ref a => panic!("{name}: expected SetF64(R), got {a:?}"),
        }
    }

    let mut dss = balance::stiff("flt2");
    for (name, gm, ..) in cases {
        dss.command(&format!(
            "new fault.{name} bus1=b.1.2 phases=2 gmatrix={gm}"
        ));
    }
    let pairs = balance::reduce(&mut dss, &["Fault.f2", "Fault.f2c"]);
    let v = balance::v1(&dss, "b");
    for ((name, _, g, r), (before, after)) in cases.into_iter().zip(pairs) {
        let full = format!("Fault.{name}");
        balance::assert_kept(before, after, &full);
        balance::assert_rel(
            after.re,
            v * v * (g[0] + g[3] - g[1]) / 1000.0,
            1e-9,
            &format!("{full}: kW vs |V1|² (G11 + G22 − G12)"),
        );
        balance::assert_rel(
            balance::query_f64(&mut dss, &format!("{full}.r")),
            r,
            1e-12,
            &format!("{full}.r after makeposseq"),
        );
    }
}

/// A `gmatrix` with no positive-sequence conductance (`S(G) <= 0`) gets only
/// the bare `Phases := 1`, so the one-phase fault reads the first entry of the
/// 3 x 3 matrix it keeps. For the all-zero matrix, the only resistance network
/// with `S(G) = 0`, that keeps the power: the fault draws nothing before and
/// after `makeposseq`, and the solve after it accepts the one-phase fault
/// holding the larger matrix. A matrix with `S(G) < 0` is no resistance
/// network (it delivers power under a balanced voltage), and how it reduces is
/// left open, so it is not pinned.
#[test]
fn make_pos_sequence_gmatrix_fault_with_no_conductance_draws_nothing() {
    use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx, balance};
    let mut f = Fault::new("f0");
    set_phases(&mut f, 3);
    f.spec_type = 2;
    f.gmatrix = Some(vec![0.0; 9]);
    let plan = f.make_pos_sequence(&PosSeqCtx::default());
    assert_eq!(plan.actions, vec![PosSeqAction::SetI32(prop::PHASES, 1)]);

    let mut dss = balance::stiff("flt0");
    dss.command("new fault.f0 bus1=b phases=3 gmatrix=[0 | 0 0 | 0 0 0]");
    let [(before, after)] = balance::reduce(&mut dss, &["Fault.f0"])[..] else {
        unreachable!()
    };
    assert_eq!(before, Complex64::ZERO, "before makeposseq");
    assert_eq!(after, Complex64::ZERO, "after makeposseq");
    dss.command("? fault.f0.phases");
    assert_eq!(dss.result(), "1", "phases after makeposseq");
    dss.command("? fault.f0.gmatrix");
    assert_eq!(
        dss.result(),
        "(0 |0 0 |0 0 0 )",
        "the matrix the fault keeps"
    );
}

/// A `gmatrix` keeps the order it was given in, so after `phases=4` the 3 x 3
/// matrix is short of the 16 entries the fault stamps. The solve refuses the
/// fault with a message naming it, its matrix and the remedy, the reports after
/// the refusal read the circuit as not solved, `Dump` and the JSON export print
/// the 3 x 3 matrix the fault holds, `makeposseq` leaves the fault as it is,
/// and a disabled fault is out of the model and refuses nothing.
#[test]
fn a_gmatrix_short_of_the_phases_refuses_the_solve() {
    use crate::elements::pos_seq::balance;
    let message = "Fault.fshort has 4 phases but its gmatrix has 3 x 3 entries. \
                   Specify gmatrix for 4 phases. Aborting solution.";
    let refused = |dss: &Dss| dss.error_texts().iter().any(|t| t.contains(message));
    let mut dss = balance::stiff_metered("fshort");
    dss.command("new fault.fshort bus1=c phases=3 gmatrix=[0.1 | -0.02 0.1 | -0.02 -0.02 0.1]");
    dss.command("edit fault.fshort phases=4");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve");
    assert!(refused(&dss), "{:?}", dss.error_texts());
    assert!(!dss.circuit().unwrap().is_solved, "solved");
    balance::reports_after_refusal(&mut dss, message, "fshort");

    let n = dss.error_texts().len();
    dss.command("dump fault.fshort debug");
    assert_eq!(dss.error_texts().len(), n, "{:?}", &dss.error_texts()[n..]);
    let dumped = std::fs::read_to_string(dss.last_result_file()).unwrap();
    assert!(
        dumped.contains("~ GMatrix= (0.100 |-0.020 0.100 |-0.020 -0.020 0.100 )"),
        "{dumped}"
    );
    let mut twin = balance::stiff("ftwin");
    twin.command("new fault.fshort bus1=b phases=3 gmatrix=[0.1 | -0.02 0.1 | -0.02 -0.02 0.1]");
    assert_eq!(
        balance::json_matrix(&dss, "Fault.fshort", "gmatrix"),
        balance::json_matrix(&twin, "Fault.fshort", "gmatrix"),
        "the JSON export of the matrix the fault holds"
    );

    dss.command("makeposseq");
    dss.command("? fault.fshort.phases");
    assert_eq!(dss.result(), "4", "the refused fault is not reduced");
    let n = dss.error_texts().len();
    dss.command("solve");
    assert!(
        dss.error_texts()[n..].iter().any(|t| t.contains(message)),
        "{:?}",
        &dss.error_texts()[n..]
    );

    let mut dss = balance::stiff_metered("fshort");
    dss.command("new fault.fshort bus1=c phases=3 gmatrix=[0.1 | -0.02 0.1 | -0.02 -0.02 0.1]");
    dss.command("edit fault.fshort phases=4 enabled=no");
    balance::solve_clean(&mut dss, "with the fault disabled");
}

/// A matrix that a `phases=` edit left of another order is kept, and the solve
/// refuses its element. `?` reads the matrix at the order it was given, and
/// `Save circuit` writes it behind `Phases=` at that order, so the reload
/// holds the same matrix and phases and refuses the same elements with the
/// same messages. Written at the new order and padded with zeros, it would
/// reload as a matrix nobody gave and solve in silence.
#[test]
fn a_refused_matrix_element_saves_and_reloads_refused() {
    use crate::elements::pos_seq::balance;
    let refusals = |dss: &Dss| {
        let mut texts: Vec<String> = dss
            .error_texts()
            .into_iter()
            .filter(|t| t.contains("phases but its"))
            .map(|t| t.to_string())
            .collect();
        texts.sort();
        texts.dedup();
        texts
    };
    let matrices = [
        ("fault.f.gmatrix", "(0.1 |-0.02 0.1 |-0.02 -0.02 0.1 )"),
        ("capacitor.c.cmatrix", "(10 |-2 10 |-2 -2 10 )"),
        ("reactor.r.rmatrix", "(1 |0.2 1 |0.2 0.2 1 )"),
        ("reactor.r.xmatrix", "(10 |1 10 |1 1 10 )"),
    ];
    let read_back = |dss: &mut Dss, when: &str| {
        for (what, value) in matrices {
            dss.command(&format!("? {what}"));
            assert_eq!(dss.result(), value, "{when}: {what}");
        }
        for (what, value) in [
            ("fault.f.phases", "4"),
            ("capacitor.c.phases", "2"),
            ("reactor.r.phases", "4"),
        ] {
            dss.command(&format!("? {what}"));
            assert_eq!(dss.result(), value, "{when}: {what}");
        }
    };

    let mut dss = balance::stiff("psave");
    for cmd in [
        "new fault.f bus1=b phases=3 gmatrix=[0.1 | -0.02 0.1 | -0.02 -0.02 0.1]",
        "edit fault.f phases=4",
        "new capacitor.c bus1=b phases=3 cmatrix=[10 | -2 10 | -2 -2 10]",
        "edit capacitor.c phases=2",
        "new reactor.r bus1=b phases=3 rmatrix=[1 | 0.2 1 | 0.2 0.2 1] \
         xmatrix=[10 | 1 10 | 1 1 10]",
        "edit reactor.r phases=4",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "solve",
    ] {
        dss.command(cmd);
    }
    let before = refusals(&dss);
    assert_eq!(before.len(), 3, "{:?}", dss.error_texts());
    assert!(!dss.circuit().unwrap().is_solved, "solved");
    read_back(&mut dss, "before the save");

    let dir = std::env::temp_dir().join(format!("dss_fault_{}_psave", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.to_string_lossy().replace('\\', "/");
    dss.command(&format!("save circuit dir=\"{path}\""));
    let saved: String = ["Fault.dss", "Capacitor.dss", "Reactor.dss"]
        .iter()
        .map(|f| std::fs::read_to_string(dir.join(f)).unwrap_or_else(|e| panic!("{f}: {e}")))
        .collect();
    let mut back = Dss::new();
    back.command(&format!("compile \"{path}/Master.dss\""));
    back.command("solve");
    let _ = std::fs::remove_dir_all(&dir);
    for token in [
        "Phases=3 GMatrix=(0.1 |-0.02 0.1 |-0.02 -0.02 0.1 )",
        "Phases=3 CMatrix=(10 |-2 10 |-2 -2 10 )",
        "Phases=3 RMatrix=(1 |0.2 1 |0.2 0.2 1 ) XMatrix=(10 |1 10 |1 1 10 )",
    ] {
        assert!(saved.contains(token), "{token} in {saved}");
    }
    assert_eq!(refusals(&back), before, "{:?}", back.error_texts());
    assert!(!back.circuit().unwrap().is_solved, "the reload solved");
    read_back(&mut back, "after the reload");
}

/// A 3 x 3 `gmatrix` after `phases=2` holds more entries than the two phases
/// stamp, and its first four are no matrix the user gave (`G11 G12 G13 G21`).
/// The solve refuses the fault with the order message, the reports after the
/// refusal read the circuit as not solved, `makeposseq` leaves the fault at
/// two phases and the next solve refuses again. `Dump` prints the 3 x 3
/// matrix the fault holds.
#[test]
fn a_gmatrix_larger_than_the_phases_refuses_the_solve() {
    use crate::elements::pos_seq::balance;
    let message = "Fault.flarge has 2 phases but its gmatrix has 3 x 3 entries. \
                   Specify gmatrix for 2 phases. Aborting solution.";
    let refused = |dss: &Dss| dss.error_texts().iter().any(|t| t.contains(message));
    let mut dss = balance::stiff_metered("flarge");
    dss.command("new fault.flarge bus1=c phases=3 gmatrix=[0.1 | -0.02 0.1 | -0.03 -0.04 0.1]");
    dss.command("edit fault.flarge phases=2 bus1=c.1.2");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve");
    assert!(refused(&dss), "{:?}", dss.error_texts());
    assert!(!dss.circuit().unwrap().is_solved, "solved");
    balance::reports_after_refusal(&mut dss, message, "flarge");

    let n = dss.error_texts().len();
    dss.command("dump fault.flarge debug");
    assert_eq!(dss.error_texts().len(), n, "{:?}", &dss.error_texts()[n..]);
    let dumped = std::fs::read_to_string(dss.last_result_file()).unwrap();
    assert!(
        dumped.contains("~ GMatrix= (0.100 |-0.020 0.100 |-0.030 -0.040 0.100 )"),
        "{dumped}"
    );

    dss.command("makeposseq");
    dss.command("? fault.flarge.phases");
    assert_eq!(dss.result(), "2", "the refused fault is not reduced");
    let n = dss.error_texts().len();
    dss.command("solve");
    assert!(
        dss.error_texts()[n..].iter().any(|t| t.contains(message)),
        "{:?}",
        &dss.error_texts()[n..]
    );
}

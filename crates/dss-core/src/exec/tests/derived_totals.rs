//! The three "derived totals" element surfaces of `GOLDEN_REBASE_PLAN.md`
//! G1.3c — `CktElement.CplxSeqCurrents`, `CktElement.CplxSeqVoltages` and
//! `CktElement.TotalPowers` — as they come out of
//! [`Dss::snapshot_elements`](crate::exec::Dss::snapshot_elements).
//!
//! These are the sub-step's expected-value pins; the live corpus gate compares
//! all three against both oracle channels case by case. What a floor cannot
//! express, and what these tests nail down in-engine against numbers read from
//! the pinned dss-python 0.15.7 / dss_capi 0.14.5 oracle on the very decks
//! below (`tmp/g13c/probe_f1_capi.py`, 2026-09-05, in the exact `compile`-only
//! state each test reads), is:
//!
//! 1. that `CplxSeq*` are the **same** 012 components whose magnitudes the
//!    G1.3b surfaces report — one transform, one `norm()` — so the two cannot
//!    drift, and that the complex pair carries the *angle*, which the magnitude
//!    channel cannot see at all;
//! 2. that the n/A sentinel here is `(-1, 0)` on **both** engines (r4133
//!    `DCktElement.pas:60`/`:106`, capi `CAPI_Alt.pas:268`/`:324`) — unlike
//!    `SeqPowers`, whose capi spelling `(-1, -1)` needs a comparator fold;
//! 3. that the positive-sequence arm writes slot `3t+1` of each terminal here
//!    on both engines: modes 13/14 call the shared `Calc*` helpers (`iV := 2`
//!    on a ONE-based buffer, stride 3), so r4133's zero-based `SeqPowers` slot
//!    defect (mode 9, `Count := 2` with stride 1) does not reach this surface;
//! 4. that `TotalPowers` sums `GetPhasePower`'s conductor slots per terminal in
//!    **W/var** and scales the total by `0.001` once (r4133
//!    `DCktElement.pas:1132`, capi `CAPI_Alt.pas:1138-1139`), not per
//!    conductor;
//! 5. that a disabled element reports zeros and a 0-terminal element reports
//!    nothing — the two shapes both captures normalize away.
//!
//! Pascal: r4133 `Version8/Source/DDLL/DCktElement.pas:885-928` (`CktElementV`
//! mode `13`, CplxSeqVoltages), `:931-975` (mode `14`, CplxSeqCurrents) over
//! `CalcSeqCurrents` `:30-80` / `CalcSeqVoltages` `:84-122`, and `:1109-1139`
//! (mode `20`, TotalPowers) over `GetPhasePower`
//! (`Common/CktElement.pas:1041-1071`); capi `CAPI/CAPI_Alt.pas:872-895`,
//! `:898-925` and `:1108-1141`, facades `CAPI/CAPI_CktElement.pas:735-743`,
//! `:752-760`, `:1043-1053`. All three are fastdss `ICktElement._columns`
//! entries (`dss/ICktElement.py:60`/`:66`/`:53` on `origin/fastdss`) — but
//! fastdss *removes* `TotalPowers` in the COM/Oddie configuration
//! (`tests/save_outputs.py:198-200`), so gating it live is stronger than
//! fastdss parity.

use crate::exec::{Dss, ElementSnapshot, SeqArm};
use crate::support::mathutil::SymComp;
use num_complex::Complex64;

/// The `feeder` tolerance tier — `tests/harness/mod.rs::tol_for` (`"feeder"`),
/// the tier IEEE13 is gated at, restated here because a `src` unit test cannot
/// reach the integration harness. Every band below is that tier's *derived*
/// image (`tests/TOLERANCE_NOTES.md`), never a fresh number.
const FEEDER_I_ABS: f64 = 1e-5;
const FEEDER_I_REL: f64 = 1e-7;
const FEEDER_V_ABS: f64 = 1e-6;
const FEEDER_V_REL: f64 = 1e-8;
/// The `micro` tier — `modes/makeposseq/makeposseq_report.dss`'s own
/// (`tests/corpus/manifests/population.lock.json`: `kind=micro`).
const MICRO_I_ABS: f64 = 1e-6;
const MICRO_I_REL: f64 = 1e-9;
const MICRO_V_ABS: f64 = 1e-6;
const MICRO_V_REL: f64 = 1e-9;

const IEEE13: &str = "electricdss-tst/Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss";

/// Compile a vendored corpus deck by absolute path and return the live engine.
/// The private twin of `derived_seq`'s helper (a per-module copy, the
/// convention in `exec/tests/`): every deck read here is read-only — IEEE13's
/// `Show` block is commented out, `makeposseq_report.dss` ends in `sample` and
/// `midi_fuse.dss` is an in-repo synthetic deck; none writes a file. Never
/// `.inputs/` — the corpus tree is the one the live gate reads (CLAUDE.md).
fn compile_corpus_deck(rel: &str) -> Dss {
    let deck = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(rel);
    assert!(deck.is_file(), "vendored corpus deck missing: {deck:?}");
    let mut dss = Dss::new();
    dss.command(&format!("compile \"{}\"", deck.display()));
    assert!(dss.errors().is_empty(), "{rel}: {:?}", dss.errors());
    dss
}

fn elem<'a>(snaps: &'a [ElementSnapshot], name: &str) -> &'a ElementSnapshot {
    snaps
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("{name} not in the snapshot"))
}

/// The band of one 012 slot, restated from `tests/TOLERANCE_NOTES.md` §G1.3b:
/// `X012 = Ap2s·Xph` with every `|Ap2s[k][j]| = 1/3`, so
/// `|ΔX012_k| ≤ (1/3)·Σ_j |ΔXph_j| ≤ abs + rel·mean_j|Xph_j|` — a bound on the
/// **complex** difference, which is what this module compares (G1.3b could only
/// use it through the weaker `||a| − |b|| ≤ |a − b|`). No coefficient moves.
fn seq_slot_band(abs: f64, rel: f64, mean_phase_mag: f64) -> f64 {
    abs + rel * mean_phase_mag
}

/// `TotalPowers`' per-terminal band: `assert_power_close`'s per-conductor floor
/// (`tests/harness/mod.rs:1550` — `i_abs·max(1, |V_kV|) + i_rel·|S|`, with
/// `|V_kV| = |S_kW| / |I_A|`) summed over exactly the conductors the terminal
/// sums. A derivation of the gated power floor, not a new class.
fn total_power_band(e: &ElementSnapshot, t: usize, i_abs: f64, i_rel: f64) -> f64 {
    (0..e.n_conds)
        .map(|c| {
            let k = t * e.n_conds + c;
            let p = e.powers[k].norm();
            let i = e.currents[k].norm();
            let vkv = if i > 1e-12 { p / i } else { 1.0 };
            i_abs * vkv.max(1.0) + i_rel * p
        })
        .sum()
}

// Expected-value pin — `CplxSeqCurrents` is the un-`Cabs`'d `CalcSeqCurrents`
// output: the SAME 012 components `SeqCurrents` reports the magnitude of
// (r4133 `DCktElement.pas:931-975` copies the helper's buffer out unchanged;
// capi `CAPI_Alt.pas:898-925` likewise), so the port derives both from one
// transform. The complex pair carries the angle, which no magnitude channel can
// see — a pure phase rotation is invisible to `SeqCurrents` and reddens this
// surface, which is the strengthening G1.3c buys.
/// `cplx_seq_currents` are the terminal's own 012 current components, and
/// `seq_currents` is exactly their `norm()`.
#[test]
fn cplx_seq_currents_are_the_012_components_whose_magnitudes_are_seq_currents() {
    let mut dss = compile_corpus_deck(IEEE13);
    let snaps = dss.snapshot_elements();
    let l = elem(&snaps, "Line.650632");
    assert_eq!((l.n_terms, l.n_conds, l.n_phases), (2, 3, 3));
    assert_eq!(l.seq_arm, SeqArm::ThreePhase);
    assert_eq!(l.cplx_seq_currents.len(), 3 * l.n_terms);

    // Leg 1 — one transform, one `norm()`: bit-exact, not a band.
    for (k, z) in l.cplx_seq_currents.iter().enumerate() {
        assert_eq!(
            l.seq_currents[k],
            z.norm(),
            "slot {k}: SeqCurrents must be |CplxSeqCurrents| bit-for-bit",
        );
    }

    // Leg 2 — the kernel: each terminal's three slots are
    // `SymComp::default().phase_to_sym` of that terminal's own first three
    // conductor currents (`k := (j-1)*NConds` on both engines: r4133
    // `DCktElement.pas:69`/`:70`, capi `CAPI_Alt.pas:279`/`:281`).
    let sym = SymComp::default();
    for (t, chunk) in l.currents.chunks(l.n_conds).take(l.n_terms).enumerate() {
        let iph = [chunk[0], chunk[1], chunk[2]];
        let mut i012 = [Complex64::ZERO; 3];
        sym.phase_to_sym(&iph, &mut i012);
        for (k, i) in i012.iter().enumerate() {
            assert_eq!(
                l.cplx_seq_currents[3 * t + k],
                *i,
                "terminal {t} slot {k} must be Ap2s·Iph of that terminal",
            );
        }
    }

    // Leg 3 — the oracle, at the derived `feeder` band on the COMPLEX
    // difference. dss_capi 0.14.5 on this deck, in this state (2026-09-05):
    const ORACLE_I012: [[(f64, f64); 3]; 2] = [
        [
            (43.8356745684313, 19.550424805255346),
            (471.00298485599956, -229.15643483055032),
            (-21.323942110636636, -60.52662498402454),
        ],
        [
            (-43.83567320569145, -19.55042358549835),
            (-471.0029622594792, 229.15763878037205),
            (21.32394286111932, 60.52662500669507),
        ],
    ];
    for (t, chunk) in l.currents.chunks(l.n_conds).take(l.n_terms).enumerate() {
        let mean_i = chunk.iter().take(3).map(|z| z.norm()).sum::<f64>() / 3.0;
        let band = seq_slot_band(FEEDER_I_ABS, FEEDER_I_REL, mean_i);
        for (k, &(re, im)) in ORACLE_I012[t].iter().enumerate() {
            let got = l.cplx_seq_currents[3 * t + k];
            assert!(
                (got - Complex64::new(re, im)).norm() <= band,
                "Line.650632 terminal {t} I012_{k} = {got} A; dss_capi 0.14.5 \
                 reads {re} + j{im} A, band {band}",
            );
        }
    }

    // Leg 4 — the angle really is new information: rotating the current by a
    // microradian leaves every `SeqCurrents` magnitude untouched (an exact
    // invariant of `|z|`) but displaces the complex slot by `|I|·1e-6`, an
    // order of magnitude outside its own band. This is the property the live
    // comparator's non-vacuity demo M1 exercises.
    let i_plus = l.cplx_seq_currents[1];
    let rotated = i_plus * Complex64::from_polar(1.0, 1e-6);
    let mean_i = l.currents[..3].iter().map(|z| z.norm()).sum::<f64>() / 3.0;
    let band = seq_slot_band(FEEDER_I_ABS, FEEDER_I_REL, mean_i);
    assert!(
        (rotated.norm() - i_plus.norm()).abs() <= 1e-12 * i_plus.norm(),
        "a rotation is norm-preserving: the magnitude channel cannot see it",
    );
    assert!(
        (rotated - i_plus).norm() > 8.0 * band,
        "a 1e-6 rad rotation must be far outside the slot band: displacement \
         {} A vs band {band} A",
        (rotated - i_plus).norm(),
    );
}

// Expected-value pin — `CplxSeqVoltages` is the un-`Cabs`'d `CalcSeqVoltages`
// output over the node voltages this element's `NodeRef` points at (r4133
// `DCktElement.pas:885-928` over `:84-122`; capi `CAPI_Alt.pas:872-895` over
// `:294-338`). The second leg ties it to `voltages_mag_ang`, the element's own
// already-gated view of those same node voltages (G1.3a), so a `NodeRef`
// mis-mapping cannot hide in either surface alone.
/// `cplx_seq_voltages` are the 012 components of the terminal's node voltages,
/// and `seq_voltages` is exactly their `norm()`.
#[test]
fn cplx_seq_voltages_are_the_012_components_of_the_terminals_node_voltages() {
    let mut dss = compile_corpus_deck(IEEE13);
    let snaps = dss.snapshot_elements();
    let l = elem(&snaps, "Line.650632");
    assert_eq!(l.cplx_seq_voltages.len(), 3 * l.n_terms);

    // Leg 1 — one transform, one `norm()`.
    for (k, z) in l.cplx_seq_voltages.iter().enumerate() {
        assert_eq!(
            l.seq_voltages[k],
            z.norm(),
            "slot {k}: SeqVoltages must be |CplxSeqVoltages| bit-for-bit",
        );
    }

    // Leg 2 — the same components come out of `voltages_mag_ang`, i.e. both
    // surfaces read the one `NodeRef` mapping. `voltages_mag_ang` renders its
    // angle through `CDANG`'s TRUNCATED `57.29577951` (`Ucomplex.pas:118`, the
    // compat-tagged precision site pinned by
    // `exec::tests::derived_polar::currents_mag_ang_is_ctopolardeg_of_the_reported_current`),
    // so the round trip inverts that same constant and the only residual left
    // is f64 rounding.
    const TRUNCATED_RAD_TO_DEG: f64 = 57.29577951;
    /// `(180/π − 57.29577951) / (180/π)` — the constant's own relative deficit,
    /// the figure `derived_polar` measures on the current side.
    const TRUNCATION_REL: f64 = 5.3797e-11;
    let sym = SymComp::default();
    for (t, chunk) in l
        .voltages_mag_ang
        .chunks(l.n_conds)
        .take(l.n_terms)
        .enumerate()
    {
        let vph: [Complex64; 3] = std::array::from_fn(|j| {
            Complex64::from_polar(chunk[j].mag, chunk[j].ang / TRUNCATED_RAD_TO_DEG)
        });
        let mut v012 = [Complex64::ZERO; 3];
        sym.phase_to_sym(&vph, &mut v012);
        let mean_v = chunk.iter().take(3).map(|p| p.mag).sum::<f64>() / 3.0;
        let round_trip = 1e-9 + 1e-12 * mean_v;
        for (k, v) in v012.iter().enumerate() {
            let got = l.cplx_seq_voltages[3 * t + k];
            assert!(
                (got - v).norm() <= round_trip,
                "terminal {t} slot {k}: {got} V from NodeV vs {v} V from \
                 VoltagesMagAng (round-trip band {round_trip})",
            );
        }

        // …and the discriminating half: reconstructing with the FULL-precision
        // `180/π` instead misses by the truncation, `|ΔV012_k| ≤ (1/3)Σ_j
        // |V_j|·|Δθ_j|` with `|Δθ_j| = |θ_j|·TRUNCATION_REL ≤ π·TRUNCATION_REL`
        // — a displacement that is real (well above f64 noise) and bounded.
        // So this leg pins the truncated constant from the sequence side too.
        let vph_full: [Complex64; 3] =
            std::array::from_fn(|j| Complex64::from_polar(chunk[j].mag, chunk[j].ang.to_radians()));
        let mut v012_full = [Complex64::ZERO; 3];
        sym.phase_to_sym(&vph_full, &mut v012_full);
        // Read on the worst slot: the drift is `Σ_j Ap2s[k][j]·V_j·jΔθ_j`, and
        // on a near-balanced terminal the `+` slot's three contributions cancel
        // (the angle error is proportional to the angle itself), while the `0`
        // slot's do not — the same near-cancellation that makes `V0` the
        // sensitive component here.
        let drift = (0..3)
            .map(|k| (v012_full[k] - l.cplx_seq_voltages[3 * t + k]).norm())
            .fold(0.0_f64, f64::max);
        let bound = std::f64::consts::PI * TRUNCATION_REL * mean_v + round_trip;
        assert!(
            drift > 10.0 * round_trip && drift <= bound,
            "terminal {t}: the full-precision 180/π reconstruction must miss by \
             the truncated constant's own deficit — drift {drift} V, bound \
             {bound} V (f64 round trip {round_trip} V)",
        );
    }

    // Leg 3 — the oracle, at the derived `feeder` band on the complex
    // difference. dss_capi 0.14.5 on this deck, in this state (2026-09-05):
    const ORACLE_V012: [[(f64, f64); 3]; 2] = [
        [
            (7.502824500599218, 12.956165656257213),
            (2521.4403665242953, -0.6109149903238507),
            (7.412929203204499, -12.924579289428493),
        ],
        [
            (3.174342559196589, -24.88493910565876),
            (2437.995445663162, -92.47103082295766),
            (-7.319541573666299, 9.833110669199073),
        ],
    ];
    for (t, chunk) in l
        .voltages_mag_ang
        .chunks(l.n_conds)
        .take(l.n_terms)
        .enumerate()
    {
        let mean_v = chunk.iter().take(3).map(|p| p.mag).sum::<f64>() / 3.0;
        let band = seq_slot_band(FEEDER_V_ABS, FEEDER_V_REL, mean_v);
        for (k, &(re, im)) in ORACLE_V012[t].iter().enumerate() {
            let got = l.cplx_seq_voltages[3 * t + k];
            assert!(
                (got - Complex64::new(re, im)).norm() <= band,
                "Line.650632 terminal {t} V012_{k} = {got} V; dss_capi 0.14.5 \
                 reads {re} + j{im} V, band {band}",
            );
        }
    }
}

// Expected-value pin — the n/A sentinel of the two complex surfaces. BOTH
// engines write `(-1, 0)` here: r4133 `Cmplx(-1.0, 0.0)`
// (`DCktElement.pas:60`, `:106`) and capi the FPC `ucomplex` real assignment
// `i012[i] := -1` / `V012[i] := -1` (`CAPI_Alt.pas:268`, `:324`), which is the
// same complex value. So — unlike `SeqPowers`, whose capi spelling is
// `(-1, -1)` (`CAPI_Alt.pas:567`) and needs a comparator fold — this surface
// needs NO channel normalization at all, and the live gate compares the
// sentinel slot-for-slot on both channels.
/// The `CplxSeq*` n/A sentinel is `(-1, 0)` on both engines — no fold exists.
#[test]
fn cplx_seq_na_sentinel_is_minus_one_plus_zero_j_on_both_engines() {
    let mut dss = compile_corpus_deck(IEEE13);
    let snaps = dss.snapshot_elements();
    // Measured on this deck, in this state (dss_capi 0.14.5, 2026-09-05):
    // `Load.634a` (1 terminal, 1 phase, non-positive-sequence circuit) reads
    // `[-1,0, -1,0, -1,0]` for BOTH CplxSeqCurrents and CplxSeqVoltages, and
    // `Capacitor.cap2` (2 terminals) the same, six slots each — while their
    // `SeqPowers` read `(-1, -1)` on that same channel.
    for (name, nterms) in [("Load.634a", 1usize), ("Capacitor.cap2", 2)] {
        let e = elem(&snaps, name);
        assert_eq!(e.seq_arm, SeqArm::NotAvailable, "{name} arm");
        assert_eq!(e.n_terms, nterms);
        let want = vec![Complex64::new(-1.0, 0.0); 3 * nterms];
        assert_eq!(
            e.cplx_seq_currents, want,
            "{name}: both engines write (-1, 0) here (r4133 DCktElement.pas:60, \
             capi CAPI_Alt.pas:268)",
        );
        assert_eq!(
            e.cplx_seq_voltages, want,
            "{name}: both engines write (-1, 0) here (r4133 DCktElement.pas:106, \
             capi CAPI_Alt.pas:324)",
        );
        // Stated the other way round as well, so the pin names the value it
        // refuses: capi's `SeqPowers` spelling must not leak onto this surface.
        assert!(
            e.cplx_seq_currents.iter().all(|z| z.im == 0.0)
                && e.cplx_seq_voltages.iter().all(|z| z.im == 0.0),
            "{name}: no slot may carry SeqPowers' capi (-1, -1) imaginary part",
        );
        // And the magnitude surfaces stay the `Cabs` of exactly this value.
        assert_eq!(e.seq_currents, vec![1.0; 3 * nterms]);
        assert_eq!(e.seq_voltages, vec![1.0; 3 * nterms]);
    }
}

// Expected-value pin — the positive-sequence slot on the COMPLEX surfaces.
// Modes 13/14 call the shared `Calc*` helpers, whose `iV := 2` indexes a
// ONE-based `pComplexArray` with `Inc(iV, 3)` (r4133 `DCktElement.pas:50`/`:55`,
// `:97`/`:102`; capi `CAPI_Alt.pas:255`/`:261`, `:312`/`:318`), i.e. zero-based
// slot `3t+1` — so BOTH engines agree here and r4133's zero-based `SeqPowers`
// slot defect (mode 9: `Count := 2` `:760` with stride 1 `:768`) does not reach
// this surface. The zero slots are an equality, not a band: upstream writes one
// slot per terminal into a buffer it has just initialized.
/// Each terminal's single conductor lands in that terminal's `+` slot, with the
/// `0` and `−` slots exactly zero.
#[test]
fn cplx_seq_positive_sequence_lands_in_the_positive_slot_of_each_terminal() {
    let mut dss = compile_corpus_deck("modes/makeposseq/makeposseq_report.dss");
    let snaps = dss.snapshot_elements();
    let l1 = elem(&snaps, "Line.l1");
    assert_eq!(l1.seq_arm, SeqArm::PosSeqSinglePhase);
    assert_eq!((l1.n_terms, l1.n_phases), (2, 1));
    assert_eq!(l1.cplx_seq_currents.len(), 6);
    assert_eq!(l1.cplx_seq_voltages.len(), 6);

    // dss_capi 0.14.5 on this deck (2026-09-05), the `+` slots of the two
    // terminals; r4133 agrees on this surface (see the comment above).
    for (slot, (re, im)) in [
        (1usize, (74.74295349036993_f64, -2.53820658841596_f64)),
        (4, (-74.74281031211785, 2.5494711796709453)),
    ] {
        let got = l1.cplx_seq_currents[slot];
        let phase_mag = l1.currents[slot / 3 * l1.n_conds].norm();
        let band = seq_slot_band(MICRO_I_ABS, MICRO_I_REL, phase_mag);
        assert!(
            (got - Complex64::new(re, im)).norm() <= band,
            "Line.l1 I012 slot {slot} = {got} A; dss_capi 0.14.5 reads \
             {re} + j{im} A, band {band}",
        );
    }
    for (slot, (re, im)) in [
        (1usize, (7338.108857258833_f64, -63.6336101639641_f64)),
        (4, (7309.075622555988, -122.53899233011673)),
    ] {
        let got = l1.cplx_seq_voltages[slot];
        let phase_mag = l1.voltages_mag_ang[slot / 3 * l1.n_conds].mag;
        let band = seq_slot_band(MICRO_V_ABS, MICRO_V_REL, phase_mag);
        assert!(
            (got - Complex64::new(re, im)).norm() <= band,
            "Line.l1 V012 slot {slot} = {got} V; dss_capi 0.14.5 reads \
             {re} + j{im} V, band {band}",
        );
    }
    for slot in [0usize, 2, 3, 5] {
        assert_eq!(
            l1.cplx_seq_currents[slot],
            Complex64::ZERO,
            "Line.l1 I012 slot {slot} must be exactly zero",
        );
        assert_eq!(
            l1.cplx_seq_voltages[slot],
            Complex64::ZERO,
            "Line.l1 V012 slot {slot} must be exactly zero",
        );
    }
    // The two terminals carry visibly different voltages, so a slot swap could
    // not pass this pin by numeric accident.
    assert!(
        (l1.cplx_seq_voltages[1] - l1.cplx_seq_voltages[4]).norm() > 2.0e1,
        "the two terminals' V+ must differ: {:?}",
        l1.cplx_seq_voltages,
    );
}

// Expected-value pin — `TotalPowers` sums `GetPhasePower`'s conductor slots of
// ONE terminal (`myInit := (j-1)*NConds+1 … myEnd := NConds*j`: r4133
// `DCktElement.pas:1126-1127`, capi `CAPI_Alt.pas:1132-1133`) in W/var and
// scales the sum by `0.001` ONCE (r4133 `:1132`, capi `:1138-1139`). Summing
// the already-scaled `Powers` instead re-rounds per conductor; the port
// accumulates unscaled, so the two forms differ only by that rounding.
/// Each terminal's `total_powers` is that terminal's own conductor sum, and the
/// element total agrees with `Σ powers` to the scaling-order residual.
#[test]
fn total_powers_are_the_terminal_sums_of_the_phase_powers() {
    let mut dss = compile_corpus_deck(IEEE13);
    let snaps = dss.snapshot_elements();

    // Leg 1 — the oracle, per terminal. dss_capi 0.14.5 on this deck
    // (2026-09-05), read BEFORE any scratch-buffer `GetCurrents` (the group-A
    // capture-order rule this surface belongs to):
    for (name, want) in [
        (
            "Line.650632",
            [
                (3566.8569962298698_f64, 1735.986251004695_f64),
                (-3506.121224364779, -1539.9766038196383),
            ],
        ),
        (
            "Transformer.sub",
            [
                (3567.211813149213, 1736.5765097253802),
                (-3567.179525340583, -1736.3140409863654),
            ],
        ),
    ] {
        let e = elem(&snaps, name);
        assert_eq!(e.total_powers.len(), e.n_terms, "{name} length");
        assert_eq!(e.n_terms, 2);
        for (t, &(kw, kvar)) in want.iter().enumerate() {
            let band = total_power_band(e, t, FEEDER_I_ABS, FEEDER_I_REL);
            let got = e.total_powers[t];
            assert!(
                (got - Complex64::new(kw, kvar)).norm() <= band,
                "{name} terminal {t} TotalPowers = {got} kVA; dss_capi 0.14.5 \
                 reads {kw} + j{kvar} kVA, band {band}",
            );
        }
        // Leg 2 — the terminal really is the row's own: the two terminals of
        // this line differ by ~7 MVA, so a repeated-terminal defect (the one
        // `Export SeqCurrents` carries for `Iresidual`, CLAUDE.md upstream bug
        // 1) could not pass.
        assert!(
            (e.total_powers[0] - e.total_powers[1]).norm() > 1e3,
            "{name}: the two terminals must differ",
        );
    }

    // Leg 3 — the scaling order. `Σ_j total_powers[j]` and `Σ_k powers[k]` are
    // the same sum with the `0.001` applied at different points, so they agree
    // to a rounding residual only: measured worst 3.595093471822542e-13 kVA
    // over every element of IEEE13 (`Line.670671`), the same order as the
    // 5.084229945850415e-13 kVA the spec measured on the oracle's own numbers.
    // A per-conductor `0.001` is therefore fidelity, not a gate — the residual
    // is ten orders below any tolerance.
    let mut worst = 0.0_f64;
    let mut worst_at = String::new();
    for e in &snaps {
        let a: Complex64 = e.total_powers.iter().sum();
        let b: Complex64 = e.powers.iter().sum();
        if (a - b).norm() > worst {
            worst = (a - b).norm();
            worst_at.clone_from(&e.name);
        }
        // And per terminal, against the element's own conductor chunk.
        for (t, chunk) in e
            .powers
            .chunks(e.n_conds.max(1))
            .take(e.n_terms)
            .enumerate()
        {
            let s: Complex64 = chunk.iter().sum();
            assert!(
                (e.total_powers[t] - s).norm() <= 1e-9 + 1e-12 * s.norm(),
                "{} terminal {t}: TotalPowers {} kVA vs Σ Powers {s} kVA",
                e.name,
                e.total_powers[t],
            );
        }
    }
    assert!(!worst_at.is_empty(), "no element was compared");
    assert!(
        worst < 1e-9,
        "the scaling-order residual must stay a rounding effect; got \
         {worst:e} kVA at {worst_at}",
    );
}

// Expected-value pin — a disabled element carries no power. `GetPhasePower`
// takes its `Else For i := 1 to Yorder Do PowerBuffer^[i] := CZERO` arm
// (r4133 `Common/CktElement.pas:1071`), so mode 20 — which has NO `Enabled` and
// no `NodeRef` guard (`DCktElement.pas:1109-1139`) — returns `NTerms` zeros,
// while capi's `NodeRef = NIL` guard (`CAPI_Alt.pas:1119`) returns a 2-double
// `[0, 0]` instead. Both captures read the surface for `Enabled` elements only
// (the shape disagreement is removed, not normalized); this pin is the
// engine-side half.
/// A never-enabled element reports `n_terms` exact zeros.
#[test]
fn total_powers_are_zero_on_a_disabled_element() {
    let mut dss = compile_corpus_deck("controls/fuse/midi_fuse.dss");
    let snaps = dss.snapshot_elements();
    // `new line.tie bus1=l4e bus2=l6e switch=yes enabled=no` — 2 terminals,
    // 3 conductors, and `SetNodeRef` never ran for it.
    let tie = elem(&snaps, "Line.tie");
    assert!(!tie.enabled, "Line.tie is `enabled=no` in the deck");
    assert_eq!(tie.n_terms, 2);
    assert_eq!(
        tie.total_powers,
        vec![Complex64::ZERO; 2],
        "a disabled element's terminals carry no power (dss_capi 0.14.5 answers \
         its 2-double [0, 0] sentinel here; r4133 answers 2 zeros)",
    );
    // The complex sequence surfaces keep the element's shape, all zero.
    assert_eq!(tie.cplx_seq_currents, vec![Complex64::ZERO; 6]);
    assert_eq!(tie.cplx_seq_voltages, vec![Complex64::ZERO; 6]);
}

// Expected-value pin — a 0-terminal element has no slots at all.
// `TUPFCControlObj.Create` never assigns `Nterms` (r4133
// `Version8/Source/Controls/UPFCControl.pas:230-246`), so mode 20's
// `setlength(myCmplxArray, Nterms)` and modes 13/14's `setlength(…, 3*NTerms)`
// all go to length 0. capi's sentinels differ by guard: measured on
// `controls/upfc/upfc_dual.dss` (2026-09-05) `UPFCControl.myupfcctrl` answers
// `TotalPowers = [0, 0]` (the `NodeRef = NIL` 2-double return,
// `CAPI_Alt.pas:1119-1123`), `CplxSeqCurrents = []` (no `NodeRef` guard) and
// `CplxSeqVoltages = [0.0]` (its one-element `DefaultResult`) — three shapes
// for one element, all capture-boundary noise. The engine answers "no
// terminals" with the empty vector.
/// A 0-terminal element reports no totals and no complex sequence slots.
#[test]
fn a_zero_terminal_element_has_no_total_powers() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.zeroterm basekv=12.47 pu=1.0 phases=3 bus1=src",
        "new line.l1 bus1=src.1.2.3 bus2=b1.1.2.3 phases=3 r1=0.3 x1=0.7 length=1",
        "new load.wye bus1=b1.1.2.3 phases=3 conn=wye kv=12.47 kw=100",
        "new upfccontrol.uc",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let snaps = dss.snapshot_elements();
    let uc = elem(&snaps, "UPFCControl.uc");
    assert_eq!((uc.n_terms, uc.n_phases), (0, 0));
    assert!(uc.total_powers.is_empty(), "{:?}", uc.total_powers);
    assert!(
        uc.cplx_seq_currents.is_empty(),
        "{:?}",
        uc.cplx_seq_currents
    );
    assert!(
        uc.cplx_seq_voltages.is_empty(),
        "{:?}",
        uc.cplx_seq_voltages
    );

    // The three-phase line on the same circuit still answers in full — the
    // empty vectors above are a property of the element, not of the snapshot.
    let l1 = elem(&snaps, "Line.l1");
    assert_eq!(l1.total_powers.len(), 2);
    assert_eq!(l1.cplx_seq_currents.len(), 6);
}

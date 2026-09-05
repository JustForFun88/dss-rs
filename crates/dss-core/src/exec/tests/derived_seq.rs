//! The three sequence element surfaces of `GOLDEN_REBASE_PLAN.md` G1.3b —
//! `CktElement.SeqCurrents`, `CktElement.SeqVoltages` and
//! `CktElement.SeqPowers` — as they come out of
//! [`Dss::snapshot_elements`](crate::exec::Dss::snapshot_elements).
//!
//! These are the sub-step's expected-value pins. The live corpus gate compares
//! the three arrays against both oracle channels case by case, at a floor that
//! carries an extra absolute term on the `r4133` channel (that build transforms
//! with a different 012 matrix — `Shared/mathutil.pas:302-303` + `:562-564`,
//! modelled exactly by [`SymComp::official`]). What a floor cannot express, and
//! what these tests nail down in-engine against numbers read from *both*
//! oracles on the very decks below, is:
//!
//! 1. **which slot** the positive-sequence single-phase arm writes. All four
//!    `Calc*` helpers on both engines write slot `3t+1` of terminal `t`
//!    (`iV := 2` on a ONE-based `pComplexArray`, `Inc(iV, 3)`), and so does
//!    capi's `SeqPowers` on its zero-based result (`iCount := 1`,
//!    `inc(icount, 3)`). r4133's `SeqPowers` transplanted that `2` into a
//!    zero-based array and kept a stride of 1 (`Count := 2`, `inc(count)`), so
//!    it writes slots `2, 3, 4, …`. The port emits the correct layout;
//! 2. **which sentinel** the n/A arm emits: `(-1, 0)` (r4133, the behavioral
//!    authority) and not `(-1, -1)` (dss_capi 0.14.5) — while the two
//!    *magnitude* arrays read `Cabs(-1 + 0j) = 1.0` on both engines, so they
//!    need no normalization at all;
//! 3. that the arm itself follows `NPhases` and the circuit's
//!    `PositiveSequence` flag, never a value;
//! 4. that the `0.003` is a three-phase kVA conversion applied *inside* the arm
//!    and **not** the `PositiveSequence` ×3 that `Get_Powers` applies to
//!    `Powers`;
//! 5. that a never-enabled element — the state both captures skip — carries no
//!    payload.
//!
//! Pascal: r4133 `Version8/Source/DDLL/DCktElement.pas:660-698` (`CktElementV`
//! mode `7`, SeqVoltages), `:700-737` (mode `8`, SeqCurrents), `:739-797`
//! (mode `9`, SeqPowers) over `CalcSeqCurrents` `:30-80` / `CalcSeqVoltages`
//! `:84-122`; capi `CAPI/CAPI_Alt.pas:620-659`, `:490-527` and `:529-593`
//! (`Alt_CE_Get_SeqPowers_`, facade `:594-618`) over `_CalcSeqCurrents`
//! `:236-290` / `CalcSeqVoltages` `:294-338`. All three are fastdss
//! `ICktElement._columns` surfaces (`dss/ICktElement.py:59`/`:62`/`:65`,
//! properties `:501`/`:492`/`:483` on `origin/fastdss`).

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
/// The `micro` tier, for the four-element positive-sequence fixture below.
const MICRO_I_ABS: f64 = 1e-6;
const MICRO_I_REL: f64 = 1e-9;
const MICRO_V_ABS: f64 = 1e-6;
const MICRO_V_REL: f64 = 1e-9;

const IEEE13: &str = "electricdss-tst/Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss";

/// Compile a vendored corpus deck by absolute path and return the live engine.
///
/// All decks read here are read-only: `IEEE13Nodeckt.dss` has no active
/// `export`/`show`/`save` (its `Show` block is commented out), and
/// `asymmetric/line/line_posseq_phases_asym.dss` / `controls/fuse/midi_fuse.dss`
/// are in-repo synthetic decks that write no file, so no directory guard is
/// needed. Never `.inputs/` — the corpus tree is the one the live gate reads
/// (CLAUDE.md).
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

/// The single-phase `Set CktModel=Positive` fixture that reaches the
/// positive-sequence arm: a source, two lines and a load, all 1-phase.
///
/// This is the deck both oracles were probed on for the numbers pinned below
/// (dss-python 0.15.7 / dss_capi 0.14.5 and the EPRI r4133 DLL through
/// `crates/dss-epri`, 2026-09-05). It is deliberately NOT a corpus case: the
/// arm is already gated live on the six `modes/makeposseq/*` decks, which are
/// `capi_v0145`-gated — the channel whose slot layout is right — and adding a
/// corpus deck is another sub-step's business (the G1.2 precedent).
fn posseq_fixture() -> Dss {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "set defaultbasefrequency=60",
        "new circuit.p1 basekv=12.47 pu=1.0 phases=1 bus1=src",
        "~ r1=0.1 x1=1.0 r0=0.1 x0=1.0",
        "set cktmodel=positive",
        "new line.l1 bus1=src bus2=b1 phases=1 r1=0.3 x1=0.6 c1=0 length=1",
        "new line.l2 bus1=b1 bus2=b2 phases=1 r1=0.3 x1=0.6 c1=0 length=1",
        "new load.ld bus1=b2 phases=1 kv=7.2 kw=100 pf=0.95 model=1",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

/// The `bS` band of one sequence slot: the image of that terminal's voltage and
/// current bands under `S = V·conj(I)·0.003`, i.e.
/// `0.003·(bV·(|I012| + bI) + |V012|·bI)` — the second-order term included
/// (`tests/TOLERANCE_NOTES.md` §G1.3b). `bv`/`bi` are the terminal's already
/// gated voltage/current bands; nothing new is introduced here.
fn power_band(bv: f64, bi: f64, v012: f64, i012: f64) -> f64 {
    0.003 * (bv * (i012 + bi) + v012 * bi)
}

// Expected-value pin — the three-arm selector: both engines choose the arm from
// the element's `NPhases` and the CIRCUIT's `PositiveSequence` flag, never from
// a value (r4133 `DDLL/DCktElement.pas:41`/`:43`, `:92`/`:94`, `:752`/`:754`;
// capi `CAPI/CAPI_Alt.pas:245`/`:248`, `:305`/`:308`, `:551`/`:553`). No
// capture field spells the arm out, so the live comparator derives it from the
// port's structural state and asserts the shape the oracle actually returned;
// this pin is the engine-side half of that contract.
/// `SeqArm` follows `NPhases` and `Set CktModel=Positive`, and every arm fills
/// exactly `3·NTerms` slots.
#[test]
fn the_seq_arm_follows_nphases_and_the_circuit_model() {
    let mut dss = compile_corpus_deck(IEEE13);
    let snaps = dss.snapshot_elements();
    for (name, arm, nphases) in [
        // A 3-phase line in a non-positive-sequence circuit.
        ("Line.650632", SeqArm::ThreePhase, 3),
        // 1-phase, but the circuit is NOT positive-sequence → n/A.
        ("Load.634a", SeqArm::NotAvailable, 1),
        ("Capacitor.cap2", SeqArm::NotAvailable, 1),
    ] {
        let e = elem(&snaps, name);
        assert_eq!(e.n_phases, nphases, "{name} phase count");
        assert_eq!(e.seq_arm, arm, "{name} arm");
        for (what, len) in [
            ("seq_currents", e.seq_currents.len()),
            ("seq_voltages", e.seq_voltages.len()),
            ("seq_powers", e.seq_powers.len()),
        ] {
            assert_eq!(len, 3 * e.n_terms, "{name} {what} length");
        }
    }

    // The same 1-phase shape under `Set CktModel=Positive`: the arm flips.
    let mut ps = posseq_fixture();
    let snaps = ps.snapshot_elements();
    for name in ["Vsource.source", "Line.l1", "Line.l2", "Load.ld"] {
        let e = elem(&snaps, name);
        assert_eq!(e.n_phases, 1, "{name} is single-phase in this fixture");
        assert_eq!(
            e.seq_arm,
            SeqArm::PosSeqSinglePhase,
            "{name} must take the positive-sequence arm",
        );
    }

    // The discriminating third case: a TWO-phase element in a
    // positive-sequence circuit is still n/A — the arm needs `NPhases = 1`,
    // not merely "not 3". `line_posseq_phases_asym.dss` flips `CktModel` to
    // Positive and then edits `Line.lph` down to `phases=2`.
    let mut asym = compile_corpus_deck("asymmetric/line/line_posseq_phases_asym.dss");
    let snaps = asym.snapshot_elements();
    let lph = elem(&snaps, "Line.lph");
    assert_eq!(lph.n_phases, 2);
    assert_eq!(
        lph.seq_arm,
        SeqArm::NotAvailable,
        "2-phase under CktModel=Positive is n/A, not the posseq arm",
    );
    assert_eq!(elem(&snaps, "Load.ld").seq_arm, SeqArm::NotAvailable);
    // …while the 3-phase `Vsource` on that same circuit still transforms.
    let src = elem(&snaps, "Vsource.source");
    assert_eq!(src.n_phases, 3);
    assert_eq!(src.seq_arm, SeqArm::ThreePhase);
}

// Expected-value pin — the positive-sequence slot: on the 1-phase
// positive-sequence arm the terminal's single conductor is reported as that
// terminal's POSITIVE-sequence component, i.e. slot `3t+1` of a zero-based
// `(0, +, −)` array, with `3t` and `3t+2` exactly zero. dss_capi 0.14.5 does
// that (`CAPI_Alt.pas:555` `iCount := 1`, `:562` `inc(icount, 3)`), and so do
// all four `Calc*` helpers on BOTH engines on their one-based buffers (r4133
// `:50`/`:55`, `:97`/`:102`; capi `:255`/`:261`, `:312`/`:318`). r4133's
// `SeqPowers` (mode 9) transplanted the one-based `2` into a zero-based dynamic
// array and kept a stride of 1 (`DCktElement.pas:760` `Count := 2`, `:768`
// `inc(count)`), so it writes slots `2, 3, 4, …` — one slot late and walking
// through the following terminals' slots. The port emits the correct layout;
// the defect is reported upstream and the layout is oracle-gated live on the
// `capi_v0145` channel (the six `modes/makeposseq/*` decks), which has it right.
/// `SeqPowers` puts each terminal's power in that terminal's positive-sequence
/// slot — `Load.ld` `[0, 812.167…, 0]`, not r4133's `[0, 0, 812.167…]`.
#[test]
fn seq_powers_positive_sequence_lands_in_the_positive_slot_of_each_terminal() {
    let mut dss = posseq_fixture();
    let snaps = dss.snapshot_elements();

    // --- one terminal. Both oracles on this very deck (2026-09-05):
    //   dss_capi 0.14.5 : [0, 0, 812.1670536378161, 266.9465630516689, 0, 0]
    //   EPRI r4133      : [0, 0, 0, 0, 812.1670536378159, 266.94656305166865]
    // (interleaved re/im) — the same number in two different slots.
    let ld = elem(&snaps, "Load.ld");
    assert_eq!(ld.n_terms, 1);
    assert_eq!(ld.seq_powers.len(), 3);
    const LD_KW: f64 = 812.1670536378161;
    const LD_KVAR: f64 = 266.9465630516689;
    let band = power_band(
        MICRO_V_ABS + MICRO_V_REL * ld.seq_voltages[1],
        MICRO_I_ABS + MICRO_I_REL * ld.seq_currents[1],
        ld.seq_voltages[1],
        ld.seq_currents[1],
    );
    assert!(
        (ld.seq_powers[1] - Complex64::new(LD_KW, LD_KVAR)).norm() <= band,
        "Load.ld S+ = {} kVA; dss_capi 0.14.5 reads {LD_KW} + j{LD_KVAR} kVA, band {band}",
        ld.seq_powers[1],
    );
    // The zero- and negative-sequence slots are EXACTLY zero: upstream
    // initializes the whole result and then writes one slot per terminal, so
    // this is an equality, not a band. Slot 2 is where r4133 puts the number.
    assert_eq!(ld.seq_powers[0], Complex64::ZERO, "the S0 slot");
    assert_eq!(
        ld.seq_powers[2],
        Complex64::ZERO,
        "the S− slot: r4133 writes {LD_KW} + j{LD_KVAR} kVA here \
         (DCktElement.pas:760 `Count := 2`)",
    );

    // --- two terminals: now r4133's stride-1 walk shows as well as its offset.
    // Oracles on this deck for `Line.l1`:
    //   dss_capi 0.14.5 : slots 1 and 4 — the two terminals' `+` slots
    //   EPRI r4133      : slots 2 and 3 — terminal 1's `−` slot, then
    //                     terminal 2's `0` slot
    let l1 = elem(&snaps, "Line.l1");
    assert_eq!(l1.n_terms, 2);
    assert_eq!(l1.seq_powers.len(), 6);
    for (slot, kw, kvar) in [
        (1usize, 813.1117805705803_f64, 268.8358368032447_f64),
        (4, -812.6394176312116, -267.89111092450753),
    ] {
        let band = power_band(
            MICRO_V_ABS + MICRO_V_REL * l1.seq_voltages[slot],
            MICRO_I_ABS + MICRO_I_REL * l1.seq_currents[slot],
            l1.seq_voltages[slot],
            l1.seq_currents[slot],
        );
        assert!(
            (l1.seq_powers[slot] - Complex64::new(kw, kvar)).norm() <= band,
            "Line.l1 slot {slot} = {} kVA; dss_capi 0.14.5 reads {kw} + j{kvar} kVA, band {band}",
            l1.seq_powers[slot],
        );
    }
    for slot in [0usize, 2, 3, 5] {
        assert_eq!(
            l1.seq_powers[slot],
            Complex64::ZERO,
            "Line.l1 slot {slot} must be exactly zero — r4133 fills 2 and 3 \
             instead of 1 and 4",
        );
    }
    // The two live numbers are ~1626 kW apart, so a slot swap could never pass
    // this pin by numeric accident.
    assert!(
        (l1.seq_powers[1] - l1.seq_powers[4]).norm() > 1.6e3,
        "the two terminals' powers must be far apart for the slot test to bite",
    );

    // The same layout on the two magnitude arrays, where BOTH engines already
    // agree (they go through `Calc*`, not through mode 9): `Line.l1`'s two
    // terminals read |I+| = 22.90955500243419 A on capi and
    // 22.909555002433038 A on r4133, and every other slot is exactly zero.
    for (slot, mag) in [(1usize, 22.90955500243419_f64), (4, 22.90955500243419)] {
        assert!(
            (l1.seq_currents[slot] - mag).abs() <= MICRO_I_ABS + MICRO_I_REL * mag,
            "Line.l1 |I+| slot {slot} = {}; oracle {mag} A",
            l1.seq_currents[slot],
        );
    }
    for slot in [0usize, 2, 3, 5] {
        assert_eq!(l1.seq_currents[slot], 0.0, "|I| slot {slot}");
        assert_eq!(l1.seq_voltages[slot], 0.0, "|V| slot {slot}");
    }
}

// Expected-value pin — the n/A sentinel of `SeqPowers`: an element that is
// neither 3-phase nor 1-phase-in-a-positive-sequence-circuit gets a constant
// sentinel instead of a computation. The two engines spell it differently —
// r4133 `DCktElement.pas:772` `cmplx(-1.0, 0)`, dss_capi 0.14.5
// `CAPI_Alt.pas:567` `cmplx(-1.0, -1.0)` — and r4133 is the behavioral
// authority, so the port emits `(-1, 0)`. capi's spelling is a capture-boundary
// shape normalized in the harness comparator under the structural
// `SeqArm::NotAvailable` predicate (never on the value), documented in
// `tests/TOLERANCE_NOTES.md` beside the `PROPS_NORM_R4133` precedent as *not* a
// tolerance.
/// The `SeqPowers` n/A sentinel is r4133's `(-1, 0)`, not capi 0.14.5's
/// `(-1, -1)`.
#[test]
fn seq_powers_na_sentinel_is_r4133s_minus_one_plus_zero_j() {
    let mut dss = compile_corpus_deck(IEEE13);
    let snaps = dss.snapshot_elements();
    // Measured on IEEE13 (2026-09-05): `Load.634a` (1 terminal, 1 phase) reads
    // [-1,-1, -1,-1, -1,-1] on dss_capi 0.14.5 and [-1,0, -1,0, -1,0] on r4133;
    // `Capacitor.cap2` (2 terminals) the same, six slots each.
    for (name, nterms) in [("Load.634a", 1usize), ("Capacitor.cap2", 2)] {
        let e = elem(&snaps, name);
        assert_eq!(e.seq_arm, SeqArm::NotAvailable, "{name} arm");
        assert_eq!(e.n_terms, nterms);
        assert_eq!(
            e.seq_powers,
            vec![Complex64::new(-1.0, 0.0); 3 * nterms],
            "{name}: the port emits r4133's (-1, 0) sentinel; dss_capi 0.14.5 \
             emits (-1, -1) (CAPI_Alt.pas:567)",
        );
        // Stated the other way round, so the pin names both numbers and cannot
        // be satisfied by capi's spelling.
        assert!(
            e.seq_powers.iter().all(|s| s.im == 0.0),
            "{name}: no slot may carry capi's -1 imaginary part",
        );
    }
}

// Expected-value pin — the n/A arm needs no normalization on the two magnitude
// surfaces: `CalcSeqCurrents`/`CalcSeqVoltages` fill their complex buffer with
// `Cmplx(-1.0, 0.0)` on r4133 (`DCktElement.pas:60`, `:106`) and with the FPC
// `ucomplex` real-assignment `-1` — which is the same `(-1, 0)` — on capi
// (`CAPI_Alt.pas:268`, `:324`); the API layer then returns `Cabs` of it. So
// both engines report exactly `1.0`, and the D-b2 sentinel split is
// `SeqPowers`-only.
/// `SeqCurrents`/`SeqVoltages` are exactly `1.0` on the n/A arm — the `Cabs` of
/// the `(-1, 0)` sentinel, identical on both engines.
#[test]
fn seq_currents_and_seq_voltages_are_one_on_the_na_arm() {
    let mut dss = compile_corpus_deck(IEEE13);
    let snaps = dss.snapshot_elements();
    for (name, nterms) in [("Load.634a", 1usize), ("Capacitor.cap2", 2)] {
        let e = elem(&snaps, name);
        assert_eq!(e.seq_arm, SeqArm::NotAvailable);
        assert_eq!(
            e.seq_currents,
            vec![1.0; 3 * nterms],
            "{name}: both oracles read 1.0 per slot (Cabs(-1 + 0j))",
        );
        assert_eq!(e.seq_voltages, vec![1.0; 3 * nterms], "{name} |V012|");
    }
}

// Expected-value pin — the transform reads the ROW's own terminal: each
// terminal's three slots are the 012 components of that terminal's own first
// three conductors (`k := (j-1)*NConds` on both engines: r4133
// `DCktElement.pas:47`/`:53`, capi `CAPI_Alt.pas:280`/`:316`). This is the API
// path, NOT the `Export SeqCurrents` report path
// (`report/export/seq_currents.rs`), which branches on `nphases >= 3`, has no
// sentinel arm and carries the report's own rating/`Iresidual` logic — a frozen
// golden that must not be re-plumbed.
/// `SeqCurrents` is `|Ap2s·Iph|` of the terminal's own conductors, and the two
/// terminals of a line answer with different numbers.
#[test]
fn seq_currents_are_the_012_magnitudes_of_the_terminals_own_conductors() {
    let mut dss = compile_corpus_deck(IEEE13);
    let snaps = dss.snapshot_elements();
    let l = elem(&snaps, "Line.650632");
    assert_eq!((l.n_terms, l.n_conds, l.n_phases), (2, 3, 3));
    assert_eq!(l.seq_arm, SeqArm::ThreePhase);

    // Leg 1 — the kernel, exactly: the reported magnitudes are `Cabs` of
    // `SymComp::default().phase_to_sym` of the very terminal current the
    // snapshot reports, read through a per-terminal chunk (no flat offset).
    let sym = SymComp::default();
    for (t, chunk) in l.currents.chunks(l.n_conds).take(l.n_terms).enumerate() {
        let iph = [chunk[0], chunk[1], chunk[2]];
        let mut i012 = [Complex64::ZERO; 3];
        sym.phase_to_sym(&iph, &mut i012);
        for (k, i) in i012.iter().enumerate() {
            assert_eq!(
                l.seq_currents[3 * t + k],
                i.norm(),
                "terminal {t} slot {k} must be |Ap2s·Iph| of that terminal",
            );
        }
    }

    // Leg 2 — the oracle, at the derived `feeder` band. `|I012_k|` is banded
    // against the terminal's PHASE magnitudes, not against its own (possibly
    // tiny) modulus: `X012 = Ap2s·Xph` with every `|Ap2s[k][j]| = 1/3`, so
    // `|ΔX012_k| ≤ x_abs + x_rel·mean_j|Xph_j|` (`tests/TOLERANCE_NOTES.md`
    // §G1.3b). Oracle (dss-python 0.15.7) for the two terminals:
    //   T1 [47.99776531189091, 523.7904956826815, 64.17306942241835]
    //   T2 [47.997763570489255, 523.7910020882374, 64.17306969317711]
    // and r4133 reads 47.99776531189064 / 523.7904959165978 / 64.17306942448099
    // for T1 — the ~2.3e-7 A gap between the two oracles is exactly the
    // truncated-matrix term this sub-step models.
    const ORACLE_I012: [[f64; 3]; 2] = [
        [47.99776531189091, 523.7904956826815, 64.17306942241835],
        [47.997763570489255, 523.7910020882374, 64.17306969317711],
    ];
    const ORACLE_V012: [[f64; 3]; 2] = [
        [14.971793613297331, 2521.4404405330097, 14.899539227118602],
        [25.086582967304878, 2439.7484879830918, 12.258293285826781],
    ];
    for (t, chunk) in l.currents.chunks(l.n_conds).take(l.n_terms).enumerate() {
        let mean_i = chunk.iter().take(3).map(|z| z.norm()).sum::<f64>() / 3.0;
        let band = FEEDER_I_ABS + FEEDER_I_REL * mean_i;
        for (k, want) in ORACLE_I012[t].iter().enumerate() {
            assert!(
                (l.seq_currents[3 * t + k] - want).abs() <= band,
                "Line.650632 terminal {t} |I012_{k}| = {} A; oracle {want} A, band {band}",
                l.seq_currents[3 * t + k],
            );
        }
    }
    // The voltage side, at the same construction: the mean phase magnitude of
    // the terminal is recovered from `voltages_mag_ang`, itself an already
    // gated channel (G1.3a).
    for (t, chunk) in l
        .voltages_mag_ang
        .chunks(l.n_conds)
        .take(l.n_terms)
        .enumerate()
    {
        let mean_v = chunk.iter().take(3).map(|p| p.mag).sum::<f64>() / 3.0;
        let band = FEEDER_V_ABS + FEEDER_V_REL * mean_v;
        for (k, want) in ORACLE_V012[t].iter().enumerate() {
            assert!(
                (l.seq_voltages[3 * t + k] - want).abs() <= band,
                "Line.650632 terminal {t} |V012_{k}| = {} V; oracle {want} V, band {band}",
                l.seq_voltages[3 * t + k],
            );
        }
    }

    // Leg 3 — the two terminals really are different rows: a transform that
    // repeated terminal 1 (the defect `Export SeqCurrents` carries for its
    // `Iresidual` column, CLAUDE.md upstream bug 1) would make these equal.
    assert!(
        (l.seq_voltages[1] - l.seq_voltages[4]).abs() > 8e1,
        "the two terminals' |V+| must differ: {:?}",
        l.seq_voltages,
    );
}

// Expected-value pin — the `0.003`: both engines convert inside the arm
// (r4133 `DCktElement.pas:767`/`:788` `cmulreal(…, 0.003)`, capi
// `CAPI_Alt.pas:561` and `:588-590`), a THREE-PHASE kVA conversion applied
// unconditionally. It is *not* the `PositiveSequence` ×3 that `Get_Powers`
// applies to `Powers` (`Common/CktElement.pas`, ported at the `powers` block of
// `exec/view.rs`): on a positive-sequence circuit the two scalings coincide
// numerically, which is exactly why the discriminating leg has to be stated.
/// `SeqPowers` is `3 × (V012·conj(I012)·0.001)`, and the sum over a terminal's
/// three sequence slots is that terminal's phase power in kW.
#[test]
fn seq_powers_are_three_times_the_012_kva_product() {
    // Leg 1 — the Fortescue power identity, on a NON-positive-sequence feeder.
    // With `Ap2sᵀ·conj(Ap2s) = (1/3)·I`, `Σ_k V012_k·conj(I012_k) =
    // (1/3)·Σ_j Vph_j·conj(Iph_j)`, so `Σ_k SeqPowers_k = 0.001·Σ_j Vph·conj(Iph)
    // = Σ_j Powers_j` (kW). A `0.001` in place of the `0.003` would miss by 3×.
    let mut dss = compile_corpus_deck(IEEE13);
    let snaps = dss.snapshot_elements();
    let l = elem(&snaps, "Line.650632");
    for t in 0..l.n_terms {
        let seq_sum: Complex64 = l.seq_powers[3 * t..3 * t + 3].iter().sum();
        let phase_sum: Complex64 = l
            .powers
            .chunks(l.n_conds)
            .nth(t)
            .expect("one chunk per terminal")
            .iter()
            .take(3)
            .sum();
        // Both sides are formed from the same doubles inside one snapshot, so
        // the only gap is the transform's own rounding.
        let band = 1e-9 + 1e-12 * phase_sum.norm();
        assert!(
            (seq_sum - phase_sum).norm() <= band,
            "terminal {t}: Σ SeqPowers = {seq_sum} kVA but Σ Powers = {phase_sum} kVA \
             (band {band}) — the 0.003 is a 3-phase kVA conversion",
        );
        assert!(
            phase_sum.norm() > 1e3,
            "the identity must be checked on a loaded terminal, not on noise",
        );
    }

    // Leg 2 — the discriminating one: on a POSITIVE-SEQUENCE circuit `Powers`
    // carries the ×3 (`S := S·3` before the `·0.001`) and `SeqPowers` does not
    // carry a second one, so the two land on the SAME number — 812.167… kW for
    // `Load.ld`, not 3× that. Both oracles agree there (capi
    // `Powers = [812.1670536378161, 266.9465630516689]`, r4133
    // `[812.1670536378159, 266.9465630516687]`), which is what makes "SeqPowers
    // does not inherit the ×3" checkable at all.
    let mut ps = posseq_fixture();
    let snaps = ps.snapshot_elements();
    let ld = elem(&snaps, "Load.ld");
    assert_eq!(ld.seq_arm, SeqArm::PosSeqSinglePhase);
    let s_plus = ld.seq_powers[1];
    let p0 = ld.powers[0];
    assert!(
        (s_plus - p0).norm() <= 1e-9 + 1e-12 * p0.norm(),
        "Load.ld S+ = {s_plus} kVA must equal Powers[0] = {p0} kVA — both are \
         V·conj(I)·0.003 (0.001 × the positive-sequence ×3 on one side, the \
         3-phase kVA conversion on the other)",
    );
    assert!(
        (s_plus.re - 812.1670536378161).abs() < 1e-6,
        "Load.ld S+ = {s_plus} kVA; the oracles read 812.1670536378161 kW — a \
         SeqPowers that inherited the ×3 would read 2436.5 kW",
    );
}

// Expected-value pin — a never-enabled element has no sequence payload. Both
// oracles refuse the read: r4133 guards `If Enabled` on modes 7 and 8
// (`DCktElement.pas:671`, `:711`) and capi guards `(not elem.Enabled)` on
// `Alt_CE_Get_SeqVoltages` (`CAPI_Alt.pas:633`) and `Alt_CE_Get_SeqCurrents`
// (`:501`). `SeqPowers` is the load-bearing one: r4133's mode 9 has NEITHER an
// `Enabled` nor a `NodeRef` guard and would dereference nil at `:781`, while
// capi's facade skips the `Enabled` test (`:605`, commented out) and its helper
// exits at `:544` AFTER the caller already resized the result at `:607`,
// returning uninitialized memory. Both captures therefore read the three
// surfaces for `Enabled` elements only; this pin is the engine-side half.
/// A never-enabled element keeps the `3·NTerms` shape and reads all zeros.
#[test]
fn a_never_enabled_element_has_no_seq_payload() {
    let mut dss = compile_corpus_deck("controls/fuse/midi_fuse.dss");
    let snaps = dss.snapshot_elements();
    // `new line.tie bus1=l4e bus2=l6e switch=yes enabled=no` — the open tie
    // that would close midi_fuse's loop: 3 conductors × 2 terminals, and
    // `SetNodeRef` never ran for it (`reprocess_bus_defs` walks enabled
    // elements only), so it has no `NodeRef` at all.
    let tie = elem(&snaps, "Line.tie");
    assert!(!tie.enabled, "Line.tie is `enabled=no` in the deck");
    assert_eq!(
        tie.seq_arm,
        SeqArm::ThreePhase,
        "it is still a 3-phase line"
    );
    assert_eq!(tie.seq_currents, vec![0.0; 6]);
    assert_eq!(tie.seq_voltages, vec![0.0; 6]);
    assert_eq!(tie.seq_powers, vec![Complex64::ZERO; 6]);
}

// Expected-value pin — a 0-terminal element has no slots at all.
// `TUPFCControlObj.Create` never assigns `Nterms` (r4133
// `Version8/Source/Controls/UPFCControl.pas:230-246`), so `3·NTerms = 0`: the
// port answers with three empty vectors. Upstream's two shapes there are a
// capture-boundary concern — capi returns its one-element `DefaultResult`
// (`CAPI_Utils.pas`) wherever a `NodeRef = NIL` guard fires and a genuinely
// empty array where none does, r4133 returns empty throughout (measured
// 2026-09-05 on `controls/upfc/upfc_dual.dss`, `upfc_statcom.dss` and
// `asymmetric/upfc/midi_upfc_asym.dss`, worker errno 0 on all three modes).
/// A 0-terminal element reports three empty sequence vectors.
#[test]
fn a_zero_terminal_element_has_no_seq_slots() {
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
    assert_eq!(uc.seq_arm, SeqArm::NotAvailable);
    assert!(uc.seq_currents.is_empty(), "{:?}", uc.seq_currents);
    assert!(uc.seq_voltages.is_empty(), "{:?}", uc.seq_voltages);
    assert!(uc.seq_powers.is_empty(), "{:?}", uc.seq_powers);
}

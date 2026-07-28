//! Stage F **two-lane test policy** — the drift model of `DE_PASCALIZE_PLAN.md`
//! Part IV.2, implemented once here and shared by every golden driver.
//!
//! The engine ships in two builds. The **parity lane**
//! (`--features dss-core/oracle-parity`) selects the bit-compat kernels of
//! `dss_core::compat` and must keep EVERY existing oracle gate green forever —
//! byte goldens, checkpoint Y, corpus floors, iteration counts, discrete
//! states; it never re-baselines. The **default lane** (no features) is the
//! idiomatic-Rust product: its kernels differ from the oracle's at the ulp
//! (F.3), and after F-FMT (F.4) its report text is rendered natively.
//!
//! What each quantity class does in each lane — the plan's drift-model table,
//! turned into code:
//!
//! | Quantity class | parity lane | default lane |
//! |---|---|---|
//! | Continuous results (V, I, S, losses, registers) | the calibrated oracle floors (`tol_for`) | **the same floors, unchanged** — ulp-level kernel drift lands orders below them |
//! | Discrete state (taps, control state, action counts, event log) | exact vs oracle | **exact vs oracle** — a knife-edge flip is investigated per the `CLAUDE.md` prove-it rule, never blanket-relaxed |
//! | Iteration count vs an oracle golden | exact (`==`, or `<=` on the r4133 channel) | [`compare_iterations`] / [`compare_iterations_le`] — a documented ±[`ITER_SLACK`] band |
//! | Text reports (Dump/Save/Show/Export) | byte-exact vs the committed golden | [`compare_report`] — the *same* committed golden, compared parsed-numeric |
//! | Deliberate divergences (the F.3 upstream-bug fixes) | reproduced, oracle-compared | excluded field-by-field, pinned by their own expected-value tests |
//!
//! **No tolerance is introduced anywhere by this split.** The default-lane
//! report policies are built by [`exact_value_policy`] (`rel = abs = 0`): the
//! numbers must still parse to *exactly* the oracle's f64: only their rendering
//! (digit count, padding, column widths) is allowed to move. The physical
//! floors stay where `tol_for`/`tests/TOLERANCE_NOTES.md` put them.
//!
//! **Which goldens are lane-split, and why (the scoping rule).** A byte golden
//! is routed through [`compare_report`] iff its writer renders a number through
//! the **F-FMT seam** — `util::fmt_g`, `report::format`'s `g`/`fixed*`/`g_w`/
//! `fpc_sci_w`/`pad` family, `comma_text` — i.e. iff F.4 can change its bytes.
//! Everything else keeps the byte compare in *both* lanes, because it is the
//! strictly stronger check and nothing in Stage F can move it:
//!
//! * `Show Loops`/`Zone`/`Controlled`/`Isolated`/`Topology`, `Export UUIDs`,
//!   `Show PV2PQ_Conversions`, the incidence-matrix CSVs — identifier / tree /
//!   integer text, no float rendered at all → byte-exact in both lanes.
//! * the CIM XML profiles and the AltDSS JSON captures — their writers call no
//!   `report::format` float helper (JSON renders through its own `{:.16E}`
//!   helper in `export/json/mod.rs`, which is outside the F-FMT inventory) →
//!   byte-exact in both lanes. (`export/json/circuit.rs`'s two `fixed_w_fpc`
//!   calls render *DSS script text* inside the payload; if F.4 moves them, that
//!   commit routes whatever golden covers them, per this same rule.)
//! * the `Action=SngSave/DblSave` binary goldens — raw little-endian f32/f64,
//!   zero formatting freedom → byte-exact in both lanes.
//!
//! **F.2 staging (this commit).** Both lanes still select the parity kernels
//! everywhere (`compat`'s F.1 staging) and rendering is untouched, so every
//! branch below is exercised on byte-identical engine output: the default lane
//! passes its parsed-numeric compare *and* would pass a byte compare, and the
//! whole gate is green in both lanes trivially. The branches become load-bearing
//! in F.3 (kernel flips) and F.4 (F-FMT). Their non-vacuity is proven by the
//! unit tests at the bottom of this file, which assert the *split itself*: a
//! rendering-only difference must pass in the default lane and FAIL in the
//! parity lane, in whichever lane the suite is running.

use super::{ElemChannels, ExportPolicy, RowPolicy, compare_export};

/// `true` in the parity build (`--features dss-core/oracle-parity`), `false` in
/// the default build.
///
/// The single lane switch of the test suite: every policy in this module
/// branches on this `const`, so **both arms always compile** (and both are
/// type-checked in both lanes) exactly like the engine's `compat` kernels.
/// `oracle_parity_cfg_gate.rs` sanctions the cfg string here because this is
/// test code; no golden driver may read the cfg directly — it calls these
/// helpers.
pub const PARITY: bool = cfg!(feature = "oracle-parity");

/// Default-lane slack on an iteration count compared against an **oracle**
/// golden, in iterations.
///
/// The drift model: an ulp-level kernel difference can flip the convergence
/// test one step early or late, i.e. ±1 — and nothing more, because the
/// fixpoint itself is still pinned at the calibrated voltage floors. Growth
/// beyond this band is a regression signal, not something to widen: it means
/// the default lane is walking a *different* trajectory, which the `CLAUDE.md`
/// prove-it rule says to diagnose (tighten the loop tolerance, isolate the
/// suspect, dump both per-iteration trajectories) before touching any bound.
/// M3c (parallel LU) and WP-R1 (iterative refinement) may later need a wider,
/// *measured* band; they must raise it here with the measurement, per case.
///
/// The parity lane ignores this constant entirely — it stays exact forever.
pub const ITER_SLACK: i32 = 1;

/// The corpus cases whose element `Powers`/`Losses` the **default** lane does
/// not oracle-compare — the drift model's "deliberate divergences … excluded
/// field-by-field" row, and the only such exclusion in the suite.
///
/// `compat::POWERS_REUSE_STALE_NEWTON_ITERMINAL` (CLAUDE.md upstream bug 5):
/// `DoNewtonSolution` leaves `Iterminal` stamped from the pre-final voltage
/// guess, so upstream's cache-aware `Get_Powers`/`Get_Losses` report a
/// one-step-stale current after `Set algorithm=Newton` while `Currents`
/// recomputes fresh. The parity lane reproduces that and still compares both
/// channels against **both** gating oracles; the default lane recomputes all
/// three reads at the converged `NodeV`, which is deliberately *not* what any
/// oracle reports (measured: 4.86e-4 kVA on `newton.dss`, 2.46e-3 kVA on
/// `newton_feeder.dss`, both on `Vsource.source` conductor 0 — ~60× and ~35×
/// their tiers' floors, so this can never be mistaken for drift).
///
/// These are the only two gated decks that run a Newton solve, and the quirk is
/// unobservable after every other algorithm (the cache is invalid at read time,
/// so both lanes recompute the same current).
///
/// **What still gates them in the default lane**: the element name set, terminal
/// **currents**, node voltages, the system Y, discrete state, and the iteration
/// count — everything except the two `S = V·conj(I)` channels. Those are pinned
/// by `dss_core::exec::tests::newton::newton_powers_are_the_lane_kernel`, which
/// asserts the default lane's Newton powers equal the *normal* algorithm's on
/// the same deck to 1e-8 kVA (the parity lane's differ by ≥ 1e-1 kVA) — and the
/// normal algorithm's powers are oracle-gated on ~500 other corpus cases, which
/// closes the loop transitively.
const LANE_SKIP_ELEM_POWERS: &[&str] =
    &["modes:newton/newton.dss", "modes:newton/newton_feeder.dss"];

/// Which element sub-channels the current lane oracle-compares for the corpus
/// case `label` — [`ElemChannels::ALL`] everywhere except
/// [`LANE_SKIP_ELEM_POWERS`] in the default lane.
pub fn elem_channels_for(label: &str) -> ElemChannels {
    if !PARITY && LANE_SKIP_ELEM_POWERS.contains(&label) {
        ElemChannels::CURRENTS_ONLY
    } else {
        ElemChannels::ALL
    }
}

/// The individual `(case label, element, property)` probe cells the **default**
/// lane does not oracle-compare, because a Stage F row deliberately moves that
/// one number. The drift model's "deliberate divergences … excluded
/// field-by-field" row again, at the finest granularity the corpus gate has:
/// one property of one element of one case. Everything else about the case —
/// every other probe, every element channel, the voltages, the system Y, the
/// discrete state and the iteration count — stays oracle-gated in both lanes.
///
/// * `Generator.g_kva` / `Generator.g_mva`, properties `kva` and `mva`, on
///   `makeposseq_pc.dss`:
///   `compat::GENERATOR_POSSEQ_RATING_GUARDS_READ_XDP_SLOTS`. Upstream's
///   `had_kVA`/`had_MVA` guards read the `Xdp`/`Xdpp` slots, so the oracle
///   leaves each rating exactly as declared across the deck's two `makeposseq`
///   calls (250 kVA, 0.30 MVA); the default lane divides by the phase count on
///   the first call (→ 83.33 kVA, → 0.10 MVA; the second call is a no-op
///   because the machine is 1-phase by then). Both property names are listed
///   for each generator because they are **one field**: `PropertyOffset` aims
///   `kVA` and `MVA` at the same `GenVars.kVArating` (`generator.pas:620` and
///   `:640`), so a divergence in the rating necessarily shows in both readouts.
///
///   The rating reaches no power-flow quantity — it scales `Xdp`/`Xdpp`
///   (`:1281-1282`) and the dynamics inertia (`:2436-2437`), neither of which a
///   snapshot solve touches — so these four cells are all that moves. The two
///   generators' own `phases`/`kv`/`kw` probes, `g_kvar`'s four probes, every
///   other property of the same two elements, and the full-model compare of all
///   13 elements stay oracle-gated in both lanes. Replacement pins:
///   `dss_core::elements::pc::generator::tests::
///   makeposseq_generator_{kva,mva}_guard_is_the_lane_slot` and
///   `…::makeposseq_generator_xdp_trips_the_kva_divide_only_in_parity`.
const LANE_SKIP_PROBE_PROPS: &[(&str, &str, &str)] = &[
    (
        "modes:makeposseq/makeposseq_pc.dss",
        "Generator.g_kva",
        "kva",
    ),
    (
        "modes:makeposseq/makeposseq_pc.dss",
        "Generator.g_kva",
        "mva",
    ),
    (
        "modes:makeposseq/makeposseq_pc.dss",
        "Generator.g_mva",
        "kva",
    ),
    (
        "modes:makeposseq/makeposseq_pc.dss",
        "Generator.g_mva",
        "mva",
    ),
];

/// The oracle event-log capture as the **current lane** expects to see it: the
/// identity in the parity lane, and in the default lane the two Relay rows of
/// Stage F applied to it as an *enumerated* rewrite.
///
/// The oracle stays the source of truth for every other line — this is the same
/// expected-value-transform shape `golden_cim`/`golden_json` use, chosen for the
/// same reason: the relay decks' event logs are the whole point of those 13
/// gated cases, so re-capturing them for the default lane would trade an oracle
/// proof for two label changes.
///
/// * `compat::RELAY_SAMPLE_TRACE_IGNORES_DEBUGTRACE` — the unguarded
///   `Debug Sample: Relay.<name>` state trace disappears from the default
///   lane's log, so those lines are dropped from the expectation. Only the
///   Relay's are: the Recloser writes the byte-identical line *guarded*, so an
///   oracle `Debug Sample: Recloser.…` line means the user asked for it and
///   both lanes must still produce it.
///
///   The drop is unconditional, which is exact only while no gated deck sets
///   `DebugTrace=yes` on a relay — verified (`rg -li debugtrace
///   tests/corpus/controls` is empty), and the failure mode if one ever does is
///   a loud length mismatch here, never a silent pass: the default lane would
///   then emit a line the expectation dropped. Such a deck adds its relay's
///   `DebugTrace` to this predicate rather than widening the drop.
/// * `compat::RELAY_RESET_EVENT_IS_LABELLED_RECLOSER` — the copy-pasted
///   `Recloser.<name>` label on a relay's reset event becomes `Relay.<name>`.
///   `device_is_relay` decides which lines those are: it must answer "the
///   circuit has a Relay of this name and no Recloser of it", so a genuine
///   recloser reset (identical wording, `Recloser.pas:909`/`:924`) is never
///   touched, and a circuit holding both classes under one name is left alone
///   to fail the compare loudly rather than be silently rewritten.
pub fn expected_eventlog(lines: &[String], device_is_relay: impl Fn(&str) -> bool) -> Vec<String> {
    if PARITY {
        return lines.to_vec();
    }
    lines
        .iter()
        .filter(|l| !l.contains(", Element=Debug Sample: Relay."))
        .map(|l| match reset_device_name(l) {
            Some(name) if device_is_relay(name) => l
                .replace(&format!(", Element=Recloser.{name},"), &{
                    format!(", Element=Relay.{name},")
                }),
            _ => l.clone(),
        })
        .collect()
}

/// The device name of a `Recloser.<name>` **reset** event line, or `None` for
/// any other line. Matches only the two wordings the copy-paste produced
/// (`Relay.pas:1196`/`:1212` == `Recloser.pas:909`/`:924`), so no other event of
/// a real recloser can be caught by the rewrite above.
fn reset_device_name(line: &str) -> Option<&str> {
    let rest = line.split_once(", Element=Recloser.")?.1;
    let (name, action) = rest.split_once(", Action=")?;
    let is_reset = action.starts_with("PHASE ")
        && (action.ends_with("RESET (1PH RESET)") || action.ends_with("RESET (3PH RESET)"));
    is_reset.then_some(name)
}

/// One oracle monitor-channel capture as the **current lane** expects to see
/// it: the identity in the parity lane; in the default lane, dss-python's
/// unflushed-stream placeholder rewritten to the empty channel the engine
/// actually reports (`compat::MONITOR_CHANNEL_PADS_THE_UNFLUSHED_STREAM`).
///
/// `flushed_records` is the Rust monitor's own flush cursor, and it is what
/// makes this transform safe rather than circular. The rewrite fires **only**
/// when that cursor is 0 — the one state in which `Monitors.Channel`'s
/// `cnt == 272` short-circuit can trigger — and it insists the capture really
/// is the placeholder (exactly one sample, exactly `0.0`). So:
///
/// * a monitor that flushed records is compared strictly, in both lanes;
/// * an engine that lost real samples reports `flushed_records > 0` with an
///   empty channel and fails the length check as before;
/// * an oracle that stops emitting the placeholder (a dss-python change) makes
///   `is_placeholder` false, and the untransformed capture then fails loudly —
///   the transform can never rot into a silent pass.
pub fn expected_monitor_channel(flushed_records: usize, capture: &[f64]) -> Vec<f64> {
    let is_placeholder = capture.len() == 1 && capture[0] == 0.0;
    if PARITY || flushed_records != 0 || !is_placeholder {
        return capture.to_vec();
    }
    Vec::new()
}

/// Whether the current lane oracle-compares the probe cell `(label, element,
/// prop)` — always in the parity lane, everywhere but
/// [`LANE_SKIP_PROBE_PROPS`] in the default lane.
pub fn probe_is_gated(label: &str, element: &str, prop: &str) -> bool {
    PARITY
        || !LANE_SKIP_PROBE_PROPS.iter().any(|(l, e, p)| {
            *l == label && e.eq_ignore_ascii_case(element) && p.eq_ignore_ascii_case(prop)
        })
}

/// The same cells as [`probe_is_gated`], spelled the way the whole-element
/// property dump keys them: lowercase `(element, property)` pairs for the case
/// `label`, empty in the parity lane.
///
/// A case that requests `compare_all_properties` reads the excluded cell a
/// second time, through `AllPropertyNames`; the runner neutralizes it exactly
/// as a ledger `property` scope does — by rewriting that one oracle value to
/// the Rust `?`-surface value — so the dump's count/order contract and every
/// other property of the same element stay gated.
pub fn skipped_prop_keys(label: &str) -> Vec<(String, String)> {
    if PARITY {
        return Vec::new();
    }
    LANE_SKIP_PROBE_PROPS
        .iter()
        .filter(|(l, _, _)| *l == label)
        .map(|(_, e, p)| (e.to_lowercase(), p.to_lowercase()))
        .collect()
}

/// Byte-exact line comparison (only CRLF→LF normalized), with a per-line
/// failure message pointing at the first divergence.
///
/// The strongest report gate: beyond the token content it pins leading
/// indentation, padding runs and trailing spaces — a formatter-side off-by-one
/// in a tab depth or a column width, invisible to a whitespace tokenizer, fails
/// here. Used directly (both lanes) for the goldens no Stage F step can move,
/// and as [`compare_report`]'s parity-lane arm.
pub fn assert_bytes_eq(oracle: &str, rust: &str, ctx: &str) {
    let o = oracle.replace("\r\n", "\n");
    let r = rust.replace("\r\n", "\n");
    if o != r {
        let ol: Vec<&str> = o.split('\n').collect();
        let rl: Vec<&str> = r.split('\n').collect();
        for (i, (a, b)) in ol.iter().zip(rl.iter()).enumerate() {
            assert_eq!(
                a,
                b,
                "{ctx}: line {} differs\n  oracle: {a:?}\n  rust:   {b:?}",
                i + 1
            );
        }
        assert_eq!(
            ol.len(),
            rl.len(),
            "{ctx}: line count differs (oracle {}, rust {})",
            ol.len(),
            rl.len()
        );
    }
}

/// The lane policy for a **text report golden whose writer renders numbers
/// through the F-FMT seam**: parity lane = the byte compare, unchanged;
/// default lane = the same committed golden compared **parsed-numeric** with
/// the existing tokenizer (`compare_export`/[`ExportPolicy`], PHASE8_PLAN
/// §2.3).
///
/// What the default lane still pins: the line count, every row's field count,
/// every text token (case-insensitively), and every number's *value* under the
/// policy's tolerance — which for the byte-golden families is exactly zero
/// ([`exact_value_policy`]). What it stops pinning: how the numbers are
/// spelled and how the columns are padded. That is precisely the freedom F-FMT
/// buys, and nothing else.
///
/// Enforces the scoping rule mechanically (in **both** lanes, so a mis-scoped
/// call site cannot hide in the lane that is not being run): a golden routed
/// here must actually carry a rendered number. A number-free report has nothing
/// F-FMT can move, so splitting it would only weaken the default lane into a
/// near-vacuous compare — those call [`assert_bytes_eq`] instead.
pub fn compare_report(oracle: &str, rust: &str, policy: &ExportPolicy, ctx: &str) {
    assert!(
        oracle
            .split(|c: char| c.is_whitespace() || c == ',' || c == '=')
            .any(|t| t.parse::<f64>().is_ok()),
        "{ctx}: this golden renders no number, so lane-splitting it only \
         weakens the default lane — compare it with `lane::assert_bytes_eq` \
         (byte-exact in both lanes) instead"
    );
    if PARITY {
        assert_bytes_eq(oracle, rust, ctx);
    } else {
        compare_export(oracle, rust, policy, ctx);
    }
}

/// An **exact-value** tokenizing policy for [`compare_report`]: `sep`
/// tokenization, no verbatim header block, row-for-row, `rel = abs = 0`.
///
/// Every number must parse to bit-identical f64; only its rendering may differ.
/// Stage F therefore adds **no** tolerance to the byte-golden families — it
/// only stops pinning glyphs. (`sep == ' '` selects `split_fields`' fixed-width
/// mode: split on whitespace *or* commas, dropping pad-dot runs.)
pub fn exact_value_policy(sep: char) -> ExportPolicy {
    ExportPolicy {
        sep,
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: Vec::new(),
    }
}

/// Iteration count vs an **exact-contract** oracle channel (the pinned
/// dss-python `capi_v0145` goldens): exact in the parity lane, a documented
/// ±[`ITER_SLACK`] band in the default lane.
pub fn compare_iterations(rust: i32, oracle: i32, ctx: &str) {
    if PARITY {
        assert_eq!(rust, oracle, "{ctx}: iteration count differs");
        return;
    }
    let delta = rust - oracle;
    assert!(
        delta.abs() <= ITER_SLACK,
        "{ctx}: iteration count drifted {delta:+} from the oracle's {oracle} \
         (Rust {rust}) — beyond the Stage F default-lane slack of \
         ±{ITER_SLACK}. The drift model allows a one-step shift at the \
         convergence boundary and nothing more: diagnose the trajectory \
         (CLAUDE.md prove-it rule) instead of widening the band."
    );
    if delta != 0 {
        eprintln!(
            "{ctx}: NOTE default-lane iteration drift {delta:+} (Rust {rust} vs oracle {oracle})"
        );
    }
}

/// Iteration count vs a **different engine line** (the r4133 channel), where
/// the contract is "never MORE than the oracle": the bound is the oracle's
/// count in the parity lane, and the oracle's count + [`ITER_SLACK`] in the
/// default lane.
pub fn compare_iterations_le(rust: i32, oracle: i32, ctx: &str) {
    let bound = if PARITY { oracle } else { oracle + ITER_SLACK };
    assert!(
        rust <= bound,
        "{ctx}: Rust used MORE iterations than the r4133 oracle ({rust} > {bound})"
    );
    if rust < oracle {
        eprintln!(
            "{ctx}: NOTE Rust converged in {rust} iterations vs the r4133 \
             oracle's {oracle} (allowed: <=; investigate if unexpected)"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ElemChannels, ITER_SLACK, LANE_SKIP_ELEM_POWERS, LANE_SKIP_PROBE_PROPS, PARITY,
        assert_bytes_eq, compare_iterations, compare_iterations_le, compare_report,
        elem_channels_for, exact_value_policy, expected_eventlog, expected_monitor_channel,
        probe_is_gated,
    };

    /// Run `f`, returning `true` when it passed. Silences the panic hook so a
    /// deliberate failure does not print a scary backtrace.
    fn passes(f: impl FnOnce() + std::panic::UnwindSafe) -> bool {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let r = std::panic::catch_unwind(f);
        std::panic::set_hook(prev);
        r.is_ok()
    }

    /// Non-vacuity of the whole split: the harness's lane const must equal the
    /// lane the **engine** was compiled in. If the feature ever stopped
    /// propagating into the integration-test crate (a Cargo wiring slip), every
    /// lane branch here would silently run the default policy against a parity
    /// engine and the parity gate would evaporate — this catches that.
    #[test]
    fn lane_const_tracks_the_engine_build() {
        assert_eq!(
            PARITY,
            dss_core::compat::ORACLE_PARITY,
            "the test crate and the engine disagree about the lane"
        );
    }

    /// The monitor transform is exactly dss-python's unflushed placeholder, and
    /// nothing else: it fires only at `flushed_records == 0` and only on a
    /// literal one-element `[0.0]` capture, so real data — including a genuine
    /// single zero sample on a *flushed* monitor — is compared strictly in both
    /// lanes.
    #[test]
    fn monitor_transform_is_the_unflushed_placeholder() {
        let placeholder = [0.0];
        // The one rewritten cell.
        assert_eq!(
            expected_monitor_channel(0, &placeholder),
            if PARITY { vec![0.0] } else { Vec::new() },
            "the placeholder is dropped in the default lane only"
        );
        // Same capture, flushed monitor: never rewritten.
        assert_eq!(expected_monitor_channel(1, &placeholder), vec![0.0]);
        // Unflushed, but not the placeholder shape: never rewritten, so an
        // oracle that stops padding fails loudly instead of passing silently.
        for cap in [vec![0.0, 0.0], vec![1.0], vec![]] {
            let expect = cap.clone();
            assert_eq!(
                expected_monitor_channel(0, &cap),
                expect,
                "only a literal [0.0] is the placeholder"
            );
        }
        assert_eq!(expected_monitor_channel(0, &[1e-30]), vec![1e-30]);
    }

    /// The one element-channel exclusion: the `newton*` decks' `Powers`/
    /// `Losses` are dropped in the default lane only, their **currents** are
    /// kept in both, and no other case is touched. Asserted as an equality
    /// against the lane so the test is meaningful in both.
    #[test]
    fn newton_powers_are_the_only_element_channel_exclusion() {
        for label in LANE_SKIP_ELEM_POWERS {
            let ch = elem_channels_for(label);
            assert!(ch.currents, "{label}: currents stay gated in every lane");
            assert_eq!(
                ch,
                if PARITY {
                    ElemChannels::ALL
                } else {
                    ElemChannels::CURRENTS_ONLY
                },
                "{label}: powers/losses are excluded in the default lane only"
            );
        }
        // Nothing else is excluded — including a label that merely *contains* an
        // excluded one (the match is exact, not a substring).
        for label in [
            "modes:newton/newton.dss step 0",
            "modes:newton/newton_other.dss",
            "modes:harmonics/harm.dss",
            "solvable_now:IEEE13Nodeckt.dss",
        ] {
            assert_eq!(
                elem_channels_for(label),
                ElemChannels::ALL,
                "{label} must stay fully gated in both lanes"
            );
        }
    }

    /// The probe-cell exclusion is exactly one cell wide, in the default lane
    /// only: the excluded `(case, element, prop)` triple stops being gated
    /// there, and its *siblings* — another property of the same element, the
    /// same property on another element, the same cell on another case — keep
    /// their oracle compare in both lanes. Asserted as an equality against the
    /// lane so the test is meaningful in both.
    #[test]
    fn probe_exclusion_is_one_cell_wide() {
        for (label, element, prop) in LANE_SKIP_PROBE_PROPS {
            assert_eq!(
                probe_is_gated(label, element, prop),
                PARITY,
                "{label} {element}.{prop} is dropped in the default lane only"
            );
            // Case-insensitive on the element/property, exact on the label —
            // the same spelling rules the oracle capture and the manifest use.
            assert_eq!(probe_is_gated(label, &element.to_uppercase(), prop), PARITY);
            assert!(
                probe_is_gated(&format!("{label} step 0"), element, prop),
                "the case label match is exact, not a prefix"
            );
            for other in ["kv", "kw", "phases"] {
                assert!(
                    probe_is_gated(label, element, other),
                    "{label} {element}.{other} must stay gated in both lanes"
                );
            }
            assert!(
                probe_is_gated(label, "Generator.g_plain", prop),
                "only the named element's cell is excluded"
            );
        }
    }

    /// The event-log transform touches exactly the two Relay rows and nothing
    /// else: it drops the relay's unguarded state trace, relabels the relay's
    /// reset event, and leaves every recloser line — including the *identically
    /// worded* recloser reset and the recloser's own (guarded) trace — alone.
    /// Asserted against the lane, so the parity arm's identity is checked too.
    #[test]
    fn eventlog_transform_is_the_two_relay_rows() {
        let ev = |el: &str, action: &str| {
            format!("Hour=0, Sec=0.5, ControlIter=1, Element={el}, Action={action}")
        };
        let lines = vec![
            ev(
                "Debug Sample: Relay.r1",
                "FPRESENTSTATE: [CLOSED, CLOSED, ]",
            ),
            ev("Debug Sample: Recloser.rc1", "FPRESENTSTATE: [CLOSED, ]"),
            ev("Relay.r1", "PHASE 1 OPENED ON PH CURVE (1PH TRIP)"),
            ev("Recloser.r1", "PHASE ALL RESET (3PH RESET)"),
            ev("Recloser.rc1", "PHASE ALL RESET (3PH RESET)"),
            ev("Recloser.r1", "PHASE 1 CLOSED (1PH RECLOSING)"),
        ];
        let out = expected_eventlog(&lines, |n| n.eq_ignore_ascii_case("r1"));
        if PARITY {
            assert_eq!(out, lines, "the parity arm must be the identity");
            return;
        }
        assert_eq!(
            out,
            vec![
                lines[1].clone(),
                lines[2].clone(),
                ev("Relay.r1", "PHASE ALL RESET (3PH RESET)"),
                lines[4].clone(),
                // Not a reset wording -> the copy-paste rewrite must not fire,
                // even though the device name is a relay's.
                lines[5].clone(),
            ]
        );
    }

    /// The relabel is refused when the name is ambiguous — a circuit holding a
    /// Relay *and* a Recloser called `r1` must fail its compare loudly instead
    /// of having a genuine recloser event rewritten into a relay's.
    #[test]
    fn eventlog_reset_relabel_needs_an_unambiguous_relay() {
        let line = "Hour=0, Sec=1, ControlIter=1, Element=Recloser.r1, \
                    Action=PHASE 1 RESET (1PH RESET)"
            .to_string();
        let lines = vec![line];
        assert_eq!(
            expected_eventlog(&lines, |_| false),
            lines,
            "no relay of that name (or a recloser shares it): leave it alone"
        );
    }

    /// A **rendering-only** difference (same value, different glyphs) is the
    /// exact freedom F-FMT buys: it must FAIL in the parity lane (byte compare)
    /// and PASS in the default lane (parsed-numeric compare). Asserted as an
    /// equality against the lane, so this test is meaningful in *both* lanes.
    #[test]
    fn report_rendering_difference_is_lane_split() {
        let oracle = "bus1, 1.5, 12.47\nbus2, 2, 0.5\n";
        let rendered = "bus1, 1.50, 12.470\nbus2, 2.0, .5\n";
        let policy = exact_value_policy(',');
        let ok = passes(|| compare_report(oracle, rendered, &policy, "probe"));
        assert_eq!(
            ok, !PARITY,
            "rendering-only difference must pass in the default lane and fail \
             in the parity lane (lane parity = {PARITY})"
        );
    }

    /// The scoping guard fires in **both** lanes: a golden that renders no
    /// number has nothing F-FMT can move, so routing it through the split
    /// (instead of `assert_bytes_eq`) is a call-site mistake — rejected even
    /// when the two sides are identical and the compare would trivially pass.
    #[test]
    fn number_free_golden_is_rejected_by_the_scoping_guard() {
        let text = "Transformer.reg1, RegControl.reg1\n";
        let policy = exact_value_policy(',');
        assert!(
            !passes(|| compare_report(text, text, &policy, "probe")),
            "a number-free golden must be rejected by the scoping guard"
        );
    }

    /// A **value** difference fails in BOTH lanes: the default-lane policy is
    /// exact-value (`rel = abs = 0`), so Stage F loosens no number anywhere.
    #[test]
    fn report_value_difference_fails_in_both_lanes() {
        let oracle = "bus1, 1.5, 12.47\n";
        let changed = "bus1, 1.5, 12.48\n";
        let policy = exact_value_policy(',');
        assert!(
            !passes(|| compare_report(oracle, changed, &policy, "probe")),
            "a changed value must fail in every lane"
        );
        // …and so does a last-ulp difference: the policy is `rel = abs = 0`.
        let ulp = format!("bus1, 1.5, {}\n", 12.47_f64 + f64::EPSILON * 12.0);
        assert!(
            !passes(move || compare_report(oracle, &ulp, &policy, "probe")),
            "a 1-ulp value difference must fail in every lane"
        );
    }

    /// Structure is still pinned in the default lane: a dropped row, an extra
    /// column, or a changed identifier fails there too.
    #[test]
    fn report_structure_is_pinned_in_both_lanes() {
        let oracle = "bus1, 1.5\nbus2, 2.5\n";
        let policy = exact_value_policy(',');
        for (what, rust) in [
            ("dropped row", "bus1, 1.5\n"),
            ("extra column", "bus1, 1.5, 9\nbus2, 2.5, 9\n"),
            ("renamed identifier", "busX, 1.5\nbus2, 2.5\n"),
        ] {
            assert!(
                !passes(|| compare_report(oracle, rust, &policy, "probe")),
                "{what} must fail in every lane"
            );
        }
    }

    /// The `Key=Value` script tokens of the `Dump`/`Save` goldens: same rule —
    /// a re-rendered value passes only in the default lane, a changed value and
    /// a changed key fail in both.
    #[test]
    fn dump_key_value_token_is_lane_split() {
        let oracle = "~ R=1.1\n~ kV=12.47\n";
        let policy = exact_value_policy(' ');
        let ok = passes(|| compare_report(oracle, "~ R=1.100\n~ kV=12.470\n", &policy, "probe"));
        assert_eq!(
            ok, !PARITY,
            "a re-rendered Key=Value token must pass in the default lane only"
        );
        assert!(
            !passes(|| compare_report(oracle, "~ R=1.2\n~ kV=12.47\n", &policy, "probe")),
            "a changed Key=Value value must fail in every lane"
        );
        assert!(
            !passes(|| compare_report(oracle, "~ Rp=1.1\n~ kV=12.47\n", &policy, "probe")),
            "a changed Key=Value key must fail in every lane"
        );
    }

    /// `assert_bytes_eq` is lane-independent (it is the both-lanes gate for the
    /// goldens Stage F cannot move): only CRLF normalization is forgiven.
    #[test]
    fn bytes_eq_forgives_only_line_endings() {
        assert_bytes_eq("a\r\nb\r\n", "a\nb\n", "probe");
        assert!(!passes(|| assert_bytes_eq("a  b\n", "a b\n", "probe")));
        assert!(!passes(|| assert_bytes_eq("a\nb\n", "a\n", "probe")));
    }

    /// Iteration policy: exact in the parity lane, ±`ITER_SLACK` in the
    /// default lane, and beyond the band it fails in both.
    #[test]
    fn iteration_policy_is_lane_split() {
        compare_iterations(7, 7, "probe"); // equal: always fine
        let ok = passes(|| compare_iterations(8, 7, "probe"));
        assert_eq!(ok, !PARITY, "a one-step drift is default-lane-only slack");
        assert!(
            !passes(|| compare_iterations(7 + ITER_SLACK + 1, 7, "probe")),
            "drift beyond the slack must fail in every lane"
        );
    }

    /// The r4133 `<=` policy keeps its direction in both lanes; only the bound
    /// gains the default lane's slack.
    #[test]
    fn iteration_le_policy_is_lane_split() {
        compare_iterations_le(6, 7, "probe"); // fewer: always allowed
        let ok = passes(|| compare_iterations_le(8, 7, "probe"));
        assert_eq!(
            ok, !PARITY,
            "one over the oracle is default-lane-only slack"
        );
        assert!(
            !passes(|| compare_iterations_le(7 + ITER_SLACK + 1, 7, "probe")),
            "more than slack over the oracle must fail in every lane"
        );
    }
}

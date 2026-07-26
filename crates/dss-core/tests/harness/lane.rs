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

use super::{ExportPolicy, RowPolicy, compare_export};

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
        ITER_SLACK, PARITY, assert_bytes_eq, compare_iterations, compare_iterations_le,
        compare_report, exact_value_policy,
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

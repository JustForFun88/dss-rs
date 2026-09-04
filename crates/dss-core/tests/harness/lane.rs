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
//! the **F-FMT seam** — `compat::fmt_g`, `report::format`'s `g`/`fixed*`/`g_w`/
//! `fpc_sci_w`/`pad` family — i.e. iff F.4 can change its bytes. Everything else
//! keeps the byte compare in *both* lanes, because it is the strictly stronger
//! check and nothing in Stage F can move it:
//!
//! * `Show Loops`/`Zone`/`Controlled`/`Isolated`/`Topology`, `Export UUIDs`,
//!   `Show PV2PQ_Conversions`, the incidence-matrix CSVs — identifier / tree /
//!   integer text, no float rendered at all → byte-exact in both lanes.
//! * the CIM XML profiles — their writers call no `report::format` float helper
//!   → byte-exact in both lanes.
//! * the `Action=SngSave/DblSave` binary goldens — raw little-endian f32/f64,
//!   zero formatting freedom → byte-exact in both lanes.
//!
//! **F.4 added one more shape.** The AltDSS JSON captures used to sit in the
//! byte-exact group; F.4 gave their writer two lane rows of its own
//! (`compat::json_float`, `compat::JSON_LINE_BREAK`), so they now go through
//! [`compare_json`] — byte-exact in the parity lane, token-for-token with
//! bit-exact numeric equality in the default one. `export/json/circuit.rs`'s
//! `%8.2f` weights render *DSS script text* inside the payload, and strings are
//! compared verbatim, so that one row is enumerated at the driver
//! (`golden_json.rs::WEIGHT_TIES_AWAY`) instead.
//!
//! **Where a rendering row cannot be absorbed by a comparator, it is
//! enumerated, never widened.** Three helpers do that, each fail-on-stale:
//! [`expected_eventlog`]'s [`EVENTLOG_REROUNDED`] cells, [`expected_rerounded`]
//! for the `Dump` goldens, and the driver-local JSON lists. `CLAUDE.md`'s
//! no-fudging rule is why: a widened band would also absorb a real last-digit
//! change of a *value*, and these rows only re-spell one.
//!
//! Non-vacuity of the whole split is proven by the unit tests at the bottom of
//! this file, which assert the *split itself*: a rendering-only difference must
//! pass in the default lane and FAIL in the parity lane, in whichever lane the
//! suite is running.

use std::sync::atomic::{AtomicUsize, Ordering};

use super::{ColSel, ColTol, ElemChannels, ExportPolicy, GateSpec, RowPolicy, compare_export};

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
/// The parity lane ignores this constant entirely — it stays exact forever,
/// and that is where the band's **compensating control** lives: the same 13
/// routed sites compare `assert_eq!(rust, oracle)` in the parity build, which
/// is one of the five mandatory gate commands (and now a CI step), so an
/// iteration count that moved at all still fails somewhere in `cargo test`.
/// The band is not backstopped by `tools/lanes/lane_diff.ps1` — that job
/// compares the two *lanes*, not either lane against the oracle, and is not
/// part of `cargo test` at all.
pub const ITER_SLACK: i32 = 1;

/// The corpus cases whose element `Powers`/`Losses` **neither** lane
/// oracle-compares — the drift model's "deliberate divergences … excluded
/// field-by-field" row, and the only such exclusion in the suite.
///
/// CLAUDE.md upstream bug 5, torn down in both lanes by
/// `GOLDEN_REBASE_PLAN.md` G2.3: `DoNewtonSolution` leaves `Iterminal` stamped
/// from the pre-final voltage guess, so upstream's cache-aware
/// `Get_Powers`/`Get_Losses` report a one-step-stale current after
/// `Set algorithm=Newton` while `Currents` recomputes fresh — one read of one
/// element with `S != V·conj(I)`. The engine recomputes all three reads at the
/// converged `NodeV`, which is deliberately *not* what any oracle reports
/// (measured against `capi_v0145`: 4.86e-4 kVA on `newton.dss`, 2.46e-3 kVA on
/// `newton_feeder.dss`, both on `Vsource.source` conductor 0 — ~60× and ~35×
/// their tiers' floors, so this can never be mistaken for drift). Bumping the
/// oracle is no escape: the quirk is in every vendored official rev (r3723,
/// r4088, r4133), so the `r4133` channel misreports these two channels too.
///
/// These are the only two gated decks that run a Newton solve, and the bug is
/// unobservable after every other algorithm (the cache is invalid at read time,
/// so the cache-aware read recomputes the same current) — which is why the
/// exclusion stays these two decks' two channels and nothing wider.
///
/// **What still gates them**: the element name set, terminal **currents**, node
/// voltages, the system Y, discrete state, and the iteration count — everything
/// except the two `S = V·conj(I)` channels. Those are pinned by
/// `dss_core::exec::tests::newton::newton_powers_match_the_normal_algorithm`,
/// which asserts the Newton powers equal the *normal* algorithm's on the same
/// deck to 1e-8 kVA (upstream's stale read differs by ≥ 1e-1 kVA) — and the
/// normal algorithm's powers are oracle-gated on ~500 other corpus cases, which
/// closes the loop transitively.
const LANE_SKIP_ELEM_POWERS: &[&str] =
    &["modes:newton/newton.dss", "modes:newton/newton_feeder.dss"];

// LANE-EXCLUSION(POWERS_REUSE_STALE_NEWTON_ITERMINAL): the two decks' powers and
// losses are dropped from the oracle compare in **both** lanes — no oracle
// channel reports them at the converged `NodeV`, so there is no lane in which
// comparing them would be right.
/// Which element sub-channels the corpus gate oracle-compares for the case
/// `label` — [`ElemChannels::ALL`] everywhere except [`LANE_SKIP_ELEM_POWERS`],
/// in either lane.
///
/// # G1.3a: the three derived polar channels do NOT join the exclusion
///
/// `CurrentsMagAng`, `VoltagesMagAng` and `Residuals`
/// (`GOLDEN_REBASE_PLAN.md` G1.3a) stay compared on the two `newton*` decks.
/// The staleness above is confined to the **cache-aware** read path —
/// `Get_Powers`/`Get_Losses` reuse `ComputeIterminal`, which returns the stamped
/// `Iterminal` untouched when the solution count already matches (r4133
/// `Common/CktElement.pas:632-640`, called from `Get_Power` `:666` at `:680` and
/// from `Get_Losses` `:707` at `:743`). The three new surfaces do not use it:
/// upstream reads them through a scratch `GetCurrents`
/// (r4133 `DDLL/DCktElement.pas:837`/`:1069`) and `VoltagesMagAng` only reads
/// `NodeV` through `NodeRef` (`:1096-1100`), never `Iterminal`. So the two decks
/// *gain* three oracle-compared channels here — a strengthening, not a widening
/// — and [`ElemChannels::CURRENTS_ONLY`] keeps all three `true`.
pub fn elem_channels_for(label: &str) -> ElemChannels {
    if LANE_SKIP_ELEM_POWERS.contains(&label) {
        ElemChannels::CURRENTS_ONLY
    } else {
        ElemChannels::ALL
    }
}

/// The oracle event-log capture as the **current lane** expects to see it: the
/// two Relay label rewrites in *both* lanes, plus — in the default lane only —
/// the enumerated `%g` re-spellings.
///
/// The oracle stays the source of truth for every other line — this is the same
/// expected-value-transform shape `golden_cim`/`golden_json` use, chosen for the
/// same reason: the relay decks' event logs are the whole point of the 17 gated
/// `oracle: "r4133"` cases that carry a relay and compare one (nine under
/// `controls/relay/`, two under `controls/combo/`, two under
/// `controls/fuse/indmach_r4133/`, the four TD21 decks — measured over
/// `population.lock.json`'s `evlog=1` rigor fields, whose `family_rigor` map
/// covers the synthetic families and whose `solvable_now` map covers the
/// vendored decks), so re-capturing them would trade an oracle proof for two
/// label changes.
///
/// # Both lanes: the two Relay rows (`GOLDEN_REBASE_PLAN.md` G2.2d)
///
/// Neither is a precision row — they are upstream label mistakes, so the engine
/// emits the correct log in both lanes and the rewrite here is what keeps every
/// other line oracle-compared.
///
// LANE-EXCLUSION(RELAY_SAMPLE_TRACE_IGNORES_DEBUGTRACE): the `Debug Sample:
// Relay.` drop below is unconditional — both lanes gate the line on
// `DebugTrace`, so neither emits it for the gated decks.
/// * The unguarded `Debug Sample: Relay.<name>` state trace
///   (`Relay.pas:1325`, no `if DebugTrace`) is absent from **both** lanes' logs
///   now, so those lines are dropped from the expectation. Only the Relay's
///   are: the Recloser writes the byte-identical line *guarded*
///   (`Recloser.pas:1044`), so an oracle `Debug Sample: Recloser.…` line means
///   the user asked for it and both lanes must still produce it.
///
///   The drop is unconditional in the other sense too — it does not consult the
///   relay's `DebugTrace` — which is exact only while no deck that compares an
///   event log turns the flag **on**. Measured over the whole corpus: the only
///   `debugtrace=yes` on a relay is the `BatchEdit Relay..* debugtrace=yes` of
///   `Examples/DOCTechNote/ExamplesMaster.dss`, whose four including decks carry
///   `evlog=0`; every relay in the four gated TD21 decks spells `debugtrace=no`
///   explicitly, and no other gated deck names the property at all. The failure
///   mode if one ever does turn it on is a loud length mismatch here, never a
///   silent pass: the engine would then emit a line the expectation dropped.
///   Such a deck adds its relay's `DebugTrace` to this predicate rather than
///   widening the drop.
///
// LANE-EXCLUSION(RELAY_RESET_EVENT_IS_LABELLED_RECLOSER): the `Recloser.<n>` →
// `Relay.<n>` relabel below is unconditional — both lanes name the class that
// emitted the reset event.
/// * The copy-pasted `Recloser.<name>` label on a relay's reset event
///   (`Relay.pas:1196`/`:1212`, verbatim from `Recloser.pas:909`/`:924`)
///   becomes `Relay.<name>`. `device_is_relay` decides which lines those are:
///   it must answer "the circuit has a Relay of this name and no Recloser of
///   it", so a genuine recloser reset (identical wording) is never touched, and
///   a circuit holding both classes under one name is left alone to fail the
///   compare loudly rather than be silently rewritten.
///
/// # Default lane only: the `%g` cells
///
/// `compat::fmt_g` (F.4) is a **precision** row and is still lane-split (until
/// `GOLDEN_REBASE_PLAN.md` G4.1), so [`EVENTLOG_REROUNDED`] — the enumerated
/// cells where its two kernels spell a *traced* number's last printed digit
/// differently — stays behind the parity guard, together with its
/// [`REROUND_VISITS`]/[`REROUND_HITS`] accounting. Applying it in the parity
/// lane would hand that lane the native `%g` spelling against FPC-spelled
/// engine output — a 1e-5 gap against `compare_eventlog`'s
/// `assert_value_matches_tol(…, 1e-6, 1e-9)` on the carrier case. The mixed
/// shape is the one `golden_json::lane_expected_json` uses for the same reason.
pub fn expected_eventlog(
    label: &str,
    lines: &[String],
    device_is_relay: impl Fn(&str) -> bool,
) -> Vec<String> {
    let relabelled: Vec<String> = lines
        .iter()
        .filter(|l| !l.contains(", Element=Debug Sample: Relay."))
        .map(|l| match reset_device_name(l) {
            Some(name) if device_is_relay(name) => l
                .replace(&format!(", Element=Recloser.{name},"), &{
                    format!(", Element=Relay.{name},")
                }),
            _ => l.clone(),
        })
        .collect();
    if PARITY {
        return relabelled;
    }

    let cells: Vec<&(&str, &str, &str, &str)> =
        EVENTLOG_REROUNDED.iter().filter(|c| c.0 == label).collect();
    let mut hits = vec![0usize; cells.len()];

    let out: Vec<String> = relabelled
        .into_iter()
        .map(|l| {
            cells
                .iter()
                .zip(hits.iter_mut())
                .fold(l, |acc, ((_, key, from, to), hit)| {
                    let (next, n) = reround_cell(&acc, key, from, to);
                    *hit += n;
                    next
                })
        })
        .collect();

    for (idx, ((case, key, from, to), hit)) in cells.iter().zip(&hits).enumerate() {
        // Over-broad guard: a cell re-spells ONE rendered number, so it may
        // never match twice within one log.
        assert!(
            *hit <= 1,
            "over-broad event-log re-round cell: ({case}, {key}, {from} -> {to}) \
             fired {hit} times in one log; a cell names a single rendered \
             number, not a pattern"
        );
        // Liveness is a per-*case* property, not a per-step one: the event log
        // grows as the case steps, so a cell legitimately fires 0 times at the
        // steps before its line is emitted (measured on the carrier: cell 1
        // fires in all 24 compared checkpoints, cell 2 in 23 of them).
        // Accumulated here and asserted once, after the gate has walked every
        // case, by [`assert_reround_cells_are_live`].
        REROUND_VISITS[idx].fetch_add(1, Ordering::Relaxed);
        REROUND_HITS[idx].fetch_add(*hit, Ordering::Relaxed);
    }
    out
}

/// Per-cell counters behind [`assert_reround_cells_are_live`], indexed exactly
/// like [`EVENTLOG_REROUNDED`].
///
/// The per-case filter in [`expected_eventlog`] preserves order and every cell
/// today belongs to the same case, so the filtered index is the global one; a
/// future second carrier must key these by the cell's position in the constant.
static REROUND_VISITS: [AtomicUsize; EVENTLOG_REROUNDED.len()] =
    [const { AtomicUsize::new(0) }; EVENTLOG_REROUNDED.len()];
static REROUND_HITS: [AtomicUsize; EVENTLOG_REROUNDED.len()] =
    [const { AtomicUsize::new(0) }; EVENTLOG_REROUNDED.len()];

/// **Fail-on-stale for [`EVENTLOG_REROUNDED`]**, asserted once at the end of the
/// corpus gate.
///
/// A cell whose case was compared but which never fired is exempting a number
/// that is no longer there — the staleness the ledger, `ESCAPE_REGISTER` and
/// [`expected_rerounded`] are all guarded against, and the one this list lacked
/// until F-settle W4 (probe: two invented rows, one with a key occurring
/// nowhere in the corpus, passed the whole gate unremarked — while the module
/// doc above already claimed this helper was fail-on-stale).
///
/// Silent when a cell's case was never visited, so `DSS_GATE_ONLY` runs and the
/// parity lane (which applies no re-round cell — it returns from
/// [`expected_eventlog`] above the fold, after the two unconditional Relay
/// label rewrites) do not trip it.
pub fn assert_reround_cells_are_live() {
    if PARITY {
        return;
    }
    for (idx, (case, key, from, to)) in EVENTLOG_REROUNDED.iter().enumerate() {
        let visits = REROUND_VISITS[idx].load(Ordering::Relaxed);
        let hits = REROUND_HITS[idx].load(Ordering::Relaxed);
        assert!(
            visits == 0 || hits > 0,
            "stale event-log re-round cell: ({case}, {key}, {from} -> {to}) \
             matched nothing across {visits} compared step(s) of its case. Each \
             cell is a number the default lane stops comparing against the \
             oracle, so it must name a cell that is really there — drop it, or \
             re-measure it."
        );
    }
}

/// The event-log cells F-FMT's `%g` row (`compat::fmt_g`) re-spells in the
/// default lane, as `(parity spelling, default spelling)`.
///
/// **The one carrier, found by measurement over the whole 520-case gate:**
/// `controls:invcontrol/midi_invcontrol_drc.dss`, the DRC trigger trace
/// (`InvControl.pas`; `inv_control/compute.rs` renders `QoutPU` with
/// `fmt_g(v, 3)`). `QoutputDRCpu` there is ≈ -0.00187499999…, i.e. just *below*
/// the 3-significant-digit half boundary: FPC's `GRISU1_F2A_AGRESSIVE_ROUNDUP`
/// sees a `4` followed by 9s and forces the round-up to `-0.00188`, while one
/// correct rounding of the `f64` gives `-0.00187`. It is a **traced display
/// value** — the DRC trigger itself compares the `f64`s — so nothing about the
/// control decision moves, and the rest of that log line (and every other line
/// of that case) stays compared against the oracle.
///
/// Each cell names its **case**, and is applied only to that case's log — the
/// list used to be applied to every line of every gated case, which is far
/// wider than the one carrier it was measured on. It is also fail-on-stale:
/// [`expected_eventlog`] asserts every cell of the case it is comparing fired
/// exactly once. Until F-settle W4 nothing checked that, so a stale entry sat
/// there silently exempting a cell (probe: two invented rows, one with a key
/// that occurs nowhere, passed the gate unremarked) — while the module doc two
/// screens up already claimed this helper was fail-on-stale.
///
/// A cell that stops occurring can therefore no longer degrade quietly; and it
/// must never match a line it was not written for, which is why the keys carry
/// the property name and the entries carry the case.
const EVENTLOG_REROUNDED: &[(&str, &str, &str, &str)] = &[
    (
        "controls:invcontrol/midi_invcontrol_drc.dss",
        "QoutPU=",
        "-0.00188",
        "-0.00187",
    ),
    (
        "controls:invcontrol/midi_invcontrol_drc.dss",
        "QoutPU=",
        "-0.00113",
        "-0.00112",
    ),
];

/// Re-spell one `<key><value>` cell of an event-log line, returning the rewritten
/// line and **how many times it fired**.
///
/// The key is located **case-insensitively** (the capture reaches this transform
/// in the engine's wording, and the comparator upper-cases before matching, so a
/// cell must not depend on which of the two it sees); `from` is then matched
/// case-**sensitively** — it is a digit string, so there is no case to fold. The
/// key's own characters are left exactly as found: the comparator's structural
/// check reads them too.
///
/// Lower-casing is **ASCII-only**, deliberately. `str::to_lowercase` is
/// Unicode-aware and can change a string's byte length (`U+0130` → `i` + a
/// combining dot, 2 bytes → 3), while the offsets it produces are used to slice
/// the *original* line — a skew that silently missed the rewrite on one probe
/// input and panicked with "byte index is not a char boundary" on another
/// (F-settle W4). ASCII folding is length-preserving, so the offsets are
/// correct by construction, and it matches the port's P6 convention.
fn reround_cell(line: &str, key: &str, from: &str, to: &str) -> (String, usize) {
    let (lower_l, lower_k) = (line.to_ascii_lowercase(), key.to_ascii_lowercase());
    let mut out = String::with_capacity(line.len());
    let mut i = 0usize;
    let mut hits = 0usize;
    while let Some(rel) = lower_l[i..].find(&lower_k) {
        let value_at = i + rel + key.len();
        out.push_str(&line[i..value_at]);
        if line[value_at..].starts_with(from) {
            out.push_str(to);
            hits += 1;
            i = value_at + from.len();
        } else {
            i = value_at;
        }
    }
    out.push_str(&line[i..]);
    (out, hits)
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

// LANE-EXCLUSION(MONITOR_CHANNEL_PADS_THE_UNFLUSHED_STREAM): the `[0.0]` a
// header-only monitor stream is read back as is fabricated by the oracles'
// **client** stream decoders, not by any engine, so it is normalized away from
// the capture unconditionally — in both lanes and on both gating channels.
/// One oracle monitor-channel capture with the client-side unflushed-stream
/// placeholder normalized away.
///
/// **Not a lane row, and not a channel row.** `dss-python`'s
/// `IMonitors.Channel` (`dss/IMonitors.py:28-55`) never calls the engine's
/// `Monitors_Get_Channel`: it pulls the raw `ByteStream` and short-circuits
/// `if cnt == 272: return np.zeros((1,), dtype=np.float32)`, 272 being the
/// header-only stream size. The `r4133` channel's captures come from our own
/// bridge, which decodes that same ByteStream "exactly like dss-python"
/// (`crates/dss-epri/src/dss.rs:625-634`) — **that** decoder, not any Pascal
/// accessor, is what this transform is measured against on that channel.
///
/// So the placeholder is a **client artifact of both readers we gate against**,
/// never an engine value in either lane. `GOLDEN_REBASE_PLAN.md` G2.4 therefore
/// reclassified it from a lane split into this capture normalization: both
/// lanes' engines report the empty channel, and both lanes strip the
/// placeholder from either channel's capture. (Scoping the strip to
/// `capi_v0145` was measured and rejected — it reds the three gated
/// `modes/time/generaltime*` decks on `r4133`, whose bridge decoder pads
/// exactly like dss-python.)
///
/// Neither *engine* accessor returns the empty channel in this state either,
/// which is a separate upstream defect the port declines rather than a reason
/// to pad: `Monitors_Get_Channel` keeps its empty `DefaultResult` only for
/// `SampleCount <= 0`/an invalid index (`CAPI_Monitors.pas:304-320`) and
/// otherwise hands back `SampleCount` zeros read out of stream bytes it never
/// wrote (`:321-330`), and r4133's `DMonitors.pas:509-541` pads `[0]` only at
/// `SampleCount = 0` and otherwise walks that same unwritten region. Both are
/// unreachable through the two clients above, so nothing here observes them.
///
/// `flushed_records` is the Rust monitor's own flush cursor, and it is what
/// makes this transform safe rather than circular. The rewrite fires **only**
/// when that cursor is 0 — the one state in which the `cnt == 272`
/// short-circuit can appear — and it insists the capture really is the
/// placeholder (exactly one sample, exactly `0.0`). So:
///
/// * a monitor that flushed records is compared strictly, in both lanes;
/// * an engine that lost real samples reports `flushed_records > 0` with an
///   empty channel and fails the length check as before;
/// * a capture that is neither the placeholder nor empty stays untransformed
///   and fails loudly.
///
/// The one shape those guards do **not** catch is a client that stops padding
/// and returns `[]`: since G2.4 both lanes' engines also report `[]`, so such a
/// capture would compare equal and quietly turn this normalization into dead
/// code. [`assert_monitor_pad_is_live`] closes that hole the way
/// [`assert_reround_cells_are_live`] closes the event-log one — every
/// unflushed-monitor capture the gate visits must actually have been the
/// placeholder.
pub fn expected_monitor_channel(flushed_records: usize, capture: &[f64]) -> Vec<f64> {
    if flushed_records == 0 {
        // Two independent counters rather than visits-vs-hits: the assert below
        // reads them from another thread while the corpus gate is still
        // comparing, and a single non-atomic "visit then hit" pair would make
        // an honest run look momentarily stale.
        if is_monitor_pad(capture) {
            MONITOR_PAD_HITS.fetch_add(1, Ordering::Relaxed);
        } else {
            MONITOR_PAD_MISSES.fetch_add(1, Ordering::Relaxed);
        }
    }
    strip_monitor_pad(flushed_records, capture)
}

/// The client placeholder's exact shape: one sample, exactly `0.0`.
fn is_monitor_pad(capture: &[f64]) -> bool {
    capture.len() == 1 && capture[0] == 0.0
}

/// [`expected_monitor_channel`] without the liveness accounting — the pure
/// transform, so the unit tests below can enumerate the shapes it must *not*
/// rewrite without poisoning the counters of whatever test binary they run in.
fn strip_monitor_pad(flushed_records: usize, capture: &[f64]) -> Vec<f64> {
    if flushed_records != 0 || !is_monitor_pad(capture) {
        return capture.to_vec();
    }
    Vec::new()
}

/// Counters behind [`assert_monitor_pad_is_live`]: unflushed-monitor channel
/// captures that **were** the client placeholder, and those that were not.
static MONITOR_PAD_HITS: AtomicUsize = AtomicUsize::new(0);
static MONITOR_PAD_MISSES: AtomicUsize = AtomicUsize::new(0);

/// **Fail-on-stale for [`expected_monitor_channel`]**, asserted once at the end
/// of the corpus gate.
///
/// Since G2.4 the engine reports the empty channel in both lanes, so an oracle
/// client that stopped padding would produce a capture equal to the engine's
/// answer and the normalization would silently become a no-op — the one rot the
/// transform's own shape guards cannot see. Every unflushed-monitor capture the
/// gate compares therefore has to *be* the placeholder; one that is not means
/// the reader changed and the transform needs re-measuring, not silence.
///
/// Silent when nothing unflushed was visited, so `DSS_GATE_ONLY` runs (and any
/// binary that compares no monitor at all) do not trip it.
pub fn assert_monitor_pad_is_live() {
    let hits = MONITOR_PAD_HITS.load(Ordering::Relaxed);
    let misses = MONITOR_PAD_MISSES.load(Ordering::Relaxed);
    assert_eq!(
        misses, 0,
        "the monitor-channel placeholder normalization is going stale: \
         {misses} unflushed-monitor channel capture(s) were NOT the client \
         `[0.0]` placeholder ({hits} were). Both gating clients pad a \
         header-only ByteStream (`dss/IMonitors.py:28-55`, \
         `crates/dss-epri/src/dss.rs:625-634`); if one stopped, an empty \
         capture now matches the engine's own empty channel and this transform \
         is dead code — re-measure it or drop it."
    );
}

/// The oracle capture as the **current lane** spells it after F-FMT's `%g` row
/// — the identity in the parity lane, and in the default lane the enumerated
/// set of cells where `compat::fmt_g`'s two kernels round the *last printed
/// digit* differently.
///
/// # Why an enumeration and not a tolerance
///
/// The two `%g` kernels render the same `f64`; they disagree only where FPC's
/// two-stage decimal rounding (a correctly-rounded 17-digit form, re-rounded to
/// `sig` digits half-away-from-zero) crosses a boundary a single correct
/// rounding does not — measured at 225 of the 52 792 renders of the committed
/// FPC battery, always a last-digit difference
/// (`crates/dss-core/tests/fmt_battery.rs`). Relaxing [`exact_value_policy`] to
/// absorb that would silently absorb a *real* last-digit value change too, and
/// `CLAUDE.md` forbids widening a band to make a comparison pass. So the golden
/// keeps `rel = abs = 0` and each affected cell is named here instead, in the
/// shape `golden_cim`/`golden_json`/`fault_dump_expected` already use: a
/// literal `from → to` pair that must match **exactly once**, so a recaptured
/// golden or a moved engine value fails loudly rather than being rewritten.
///
/// Each pair is verifiable by hand: `from` is what FPC prints for the value,
/// `to` is what one correct rounding prints, and they differ in the final digit
/// only.
pub fn expected_rerounded(oracle: &str, cells: &[(&str, &str)]) -> String {
    if PARITY {
        return oracle.to_string();
    }
    let mut out = oracle.to_string();
    for (from, to) in cells {
        assert_eq!(
            out.matches(from).count(),
            1,
            "the F-FMT re-rounding cell {from:?} matches {} times, not once — \
             if the golden was recaptured or the engine value moved, re-derive \
             the pair; do not loosen the compare",
            out.matches(from).count()
        );
        assert!(
            is_last_digit_respelling(from, to),
            "{from:?} → {to:?} is not a last-digit re-spelling: this helper may \
             only change how a number is printed, never which number it is"
        );
        out = out.replace(from, to);
    }
    out
}

/// Are `a` and `b` the same `Key=Number` cell with the number re-rounded in its
/// last printed place?
///
/// Checked, not assumed: the keys must be identical, both values must parse,
/// and they must differ by no more than one unit in the last decimal place `a`
/// prints (plus the ≤½-ulp each side that re-parsing costs). A pair that edits
/// a value — or a key — fails.
fn is_last_digit_respelling(a: &str, b: &str) -> bool {
    let (Some((ka, va)), Some((kb, vb))) = (a.rsplit_once('='), b.rsplit_once('=')) else {
        return false;
    };
    if ka != kb || va == vb {
        return false;
    }
    let (Ok(x), Ok(y)) = (va.parse::<f64>(), vb.parse::<f64>()) else {
        return false;
    };
    let frac = va.split_once('.').map_or(0, |(_, f)| {
        f.trim_end_matches(|c: char| !c.is_ascii_digit()).len()
    });
    let unit = 10f64.powi(-(frac as i32)) + 2.0 * f64::EPSILON * x.abs().max(y.abs());
    (x - y).abs() <= unit
}

/// The lane policy for an **AltDSS JSON byte golden**: byte-exact in the parity
/// lane, token-for-token with numeric equality in the default lane.
///
/// F.4 gave the JSON writer two lane rows — `compat::json_float` (fpjson's fixed
/// 17-significant scientific vs the shortest round-tripping literal) and
/// `compat::JSON_LINE_BREAK` (the Windows `sLineBreak` the oracle captured vs a
/// plain `\n`). Both change how the document *looks* and neither changes what it
/// *says*, which is exactly the F-FMT contract, so the default lane compares the
/// same committed golden as a token stream:
///
/// * every structural character, in order — so key order, nesting, array length
///   and the object/array shape stay pinned exactly as before;
/// * every string **verbatim**, including its escaping — the DSS script text the
///   `PostCommands` carry is therefore still byte-gated;
/// * every number by **value**, bit-for-bit (`f64::to_bits`, so `-0` stays
///   distinct from `0`) — no tolerance whatsoever, only the spelling is free;
/// * `true`/`false`/`null` verbatim.
///
/// What it stops pinning is whitespace and float spelling, and nothing else.
pub fn compare_json(oracle: &str, rust: &str, ctx: &str) {
    if PARITY {
        assert_eq!(rust, oracle, "{ctx}");
        return;
    }
    let want = json_tokens(oracle, &format!("{ctx} (golden)"));
    let got = json_tokens(rust, &format!("{ctx} (rust)"));
    for (i, (a, b)) in want.iter().zip(got.iter()).enumerate() {
        assert_eq!(a, b, "{ctx}: JSON token {i} differs");
    }
    assert_eq!(
        want.len(),
        got.len(),
        "{ctx}: JSON token count differs (golden {}, rust {})",
        want.len(),
        got.len()
    );
}

/// One lexical item of a JSON document, at the granularity [`compare_json`]
/// compares: numbers by value, everything else verbatim.
#[derive(Debug, PartialEq, Eq)]
enum JsonTok {
    /// A structural character: `{ } [ ] : ,`.
    Punct(char),
    /// A string literal **including** its quotes and escapes, verbatim.
    Str(String),
    /// A number, as its `f64` bit pattern — `0` and `-0` stay distinct and no
    /// two different values can ever compare equal.
    Num(u64),
    /// `true`, `false` or `null`.
    Lit(String),
}

/// Lex `text` into [`JsonTok`]s, skipping whitespace. Panics with `ctx` on
/// anything that is not well-formed JSON lexically — a malformed document must
/// fail loudly rather than compare as a short token stream.
fn json_tokens(text: &str, ctx: &str) -> Vec<JsonTok> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        let c = b[i];
        match c {
            b' ' | b'\t' | b'\r' | b'\n' => i += 1,
            b'{' | b'}' | b'[' | b']' | b':' | b',' => {
                out.push(JsonTok::Punct(c as char));
                i += 1;
            }
            b'"' => {
                let start = i;
                i += 1;
                while i < b.len() {
                    match b[i] {
                        b'\\' => i += 2,
                        b'"' => {
                            i += 1;
                            break;
                        }
                        _ => i += 1,
                    }
                }
                assert!(i <= b.len(), "{ctx}: unterminated string at byte {start}");
                out.push(JsonTok::Str(text[start..i].to_string()));
            }
            b't' | b'f' | b'n' => {
                let start = i;
                while i < b.len() && b[i].is_ascii_alphabetic() {
                    i += 1;
                }
                let lit = &text[start..i];
                assert!(
                    matches!(lit, "true" | "false" | "null"),
                    "{ctx}: unknown literal {lit:?} at byte {start}"
                );
                out.push(JsonTok::Lit(lit.to_string()));
            }
            _ => {
                let start = i;
                while i < b.len()
                    && (b[i].is_ascii_digit() || matches!(b[i], b'+' | b'-' | b'.' | b'e' | b'E'))
                {
                    i += 1;
                }
                let num = &text[start..i];
                let v: f64 = num
                    .parse()
                    .unwrap_or_else(|_| panic!("{ctx}: not a JSON number: {num:?}"));
                out.push(JsonTok::Num(v.to_bits()));
            }
        }
    }
    assert!(!out.is_empty(), "{ctx}: empty JSON document");
    out
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

/// `Export Profile`'s line-to-line per-unit policy for the **current lane**.
///
/// The parity lane returns `base` untouched — every column of the three L-L
/// variants stays byte/exact-value compared against the oracle capture.
///
/// The default lane excludes exactly the two per-unit columns (`puV1` at index
/// 2, `puV2` at index 4) of those three goldens, because
/// `compat::profile_ll_pu_divisor` deliberately diverges there: upstream
/// divides the L-L magnitude by the four-digit `1732.0` while the *same
/// procedure*'s line-to-neutral arms divide by the exact `1000.0`, so the
/// reported L-L per-unit is 2.93e-5 relative high. This is the drift model's
/// "deliberate divergences … excluded from oracle comparison **at those
/// fields**", implemented with the mechanism the `Iresidual` row already uses
/// ([`GateSpec::Mask`]) — an exclusion, never a widened tolerance.
///
/// **What still gates these goldens in the default lane**: the header line, the
/// row set and its order, the element name, both distance columns, the phase
/// `Color`, `Thickness`, `Linetype`, the marker fields — i.e. every guard,
/// filter and layout decision the three L-L arms make — and, in full, the four
/// **line-to-neutral** variants of the very same report, whose `1000.0`
/// divisor the row does not touch. The excluded cells are pinned instead by
/// `dss_core::exec::tests::compat_quirks::export_profile_ll_pu_is_the_lane_kernel`,
/// which asserts the physical identity `pu_LL == pu_LN` on a balanced feeder
/// (true only with the correct divisor) and that the other lane's value is
/// distinguishable at the report's own 6-digit resolution.
pub fn profile_ll_policy(base: ExportPolicy) -> ExportPolicy {
    if PARITY {
        return base;
    }
    let mut p = base;
    for col in [2usize, 4] {
        p.col_tol.push(ColTol {
            sel: ColSel::Index(col),
            rel: 0.0,
            abs: 0.0,
            gate: Some(GateSpec::Mask),
        });
    }
    p
}

#[cfg(test)]
mod tests {
    use super::{
        ElemChannels, ITER_SLACK, LANE_SKIP_ELEM_POWERS, MONITOR_PAD_HITS, MONITOR_PAD_MISSES,
        Ordering, PARITY, assert_bytes_eq, assert_monitor_pad_is_live,
        assert_reround_cells_are_live, compare_iterations, compare_iterations_le, compare_report,
        elem_channels_for, exact_value_policy, expected_eventlog, expected_monitor_channel,
        strip_monitor_pad,
    };

    thread_local! {
        /// Set while [`passes`] is running its closure — the *only* panics this
        /// module's hook is allowed to swallow.
        ///
        /// Thread-local because the panic hook is **process-global** and these
        /// tests share their binary with `corpus_gate_all_cases_match_engines`:
        /// the first version of `passes` installed a no-op hook for the duration
        /// of its closure, which silenced *every other thread's* panic message
        /// for that window. That is not hypothetical — it swallowed the corpus
        /// gate's failing-case list on an F.5 gate run (the test reported
        /// "519/520, 1 failed" with no case name and no way to get one, since
        /// the message is produced by the hook). Two `passes` calls overlapping
        /// could also restore each other's saved hook and leave the no-op
        /// installed permanently.
        static SILENCE_PANICS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    /// Run `f`, returning `true` when it passed.
    ///
    /// A deliberate failure must not print a scary backtrace, but silence is
    /// scoped to *this thread inside this helper*: the hook is installed once
    /// and delegates to the previous one for every panic that is not ours, so a
    /// concurrent test's panic message survives intact.
    fn passes(f: impl FnOnce() + std::panic::UnwindSafe) -> bool {
        static HOOK: std::sync::Once = std::sync::Once::new();
        HOOK.call_once(|| {
            let prev = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                if !SILENCE_PANICS.with(std::cell::Cell::get) {
                    prev(info);
                }
            }));
        });
        SILENCE_PANICS.with(|s| s.set(true));
        let r = std::panic::catch_unwind(f);
        SILENCE_PANICS.with(|s| s.set(false));
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

    /// The monitor transform is exactly the clients' unflushed placeholder, and
    /// nothing else: it fires only at `flushed_records == 0` and only on a
    /// literal one-element `[0.0]` capture, so real data — including a genuine
    /// single zero sample on a *flushed* monitor — is compared strictly in both
    /// lanes.
    ///
    /// The negative shapes are asserted against the pure [`strip_monitor_pad`],
    /// not the public wrapper, precisely because the wrapper counts: a
    /// non-placeholder visit charged here would sit in the same statics
    /// [`assert_monitor_pad_is_live`] reads at the end of the corpus-gate
    /// binary. The two wrapper calls this test does make are safe (one
    /// placeholder, one flushed capture — a hit and a non-candidate).
    #[test]
    fn monitor_transform_is_the_unflushed_placeholder() {
        let placeholder = [0.0];
        // The one rewritten cell — rewritten in BOTH lanes since
        // `GOLDEN_REBASE_PLAN.md` G2.4: the pad is the oracle clients' stream
        // decoder, not an engine's, so no lane of ours ever emits it.
        assert_eq!(
            expected_monitor_channel(0, &placeholder),
            Vec::<f64>::new(),
            "the client placeholder is dropped in both lanes"
        );
        // Same capture, flushed monitor: never rewritten.
        assert_eq!(expected_monitor_channel(1, &placeholder), vec![0.0]);
        // Unflushed, but not the placeholder shape: never rewritten. For
        // `[0.0, 0.0]`, `[1.0]` and `[1e-30]` that is also what fails the
        // compare loudly; the empty capture is the one shape the engine now
        // reports too, so its loudness comes from `assert_monitor_pad_is_live`
        // instead (asserted below).
        for cap in [vec![0.0, 0.0], vec![1.0], vec![], vec![1e-30]] {
            let expect = cap.clone();
            assert_eq!(
                strip_monitor_pad(0, &cap),
                expect,
                "only a literal [0.0] is the placeholder"
            );
        }
    }

    /// The liveness half of the monitor row: a run in which some
    /// unflushed-monitor capture was *not* the placeholder is a reader that
    /// stopped padding, and it must fail rather than leave the normalization
    /// dead. Asserted on the real statics, which this test can only push in the
    /// safe direction (it charges hits, never misses), so it stays
    /// order-independent against a corpus gate sharing the same binary.
    #[test]
    fn monitor_pad_liveness_is_asserted_not_assumed() {
        // Nothing visited, or only placeholders visited: silent, always.
        assert_monitor_pad_is_live();
        let before = MONITOR_PAD_HITS.load(Ordering::Relaxed);
        assert_eq!(
            expected_monitor_channel(0, &[0.0]),
            Vec::<f64>::new(),
            "a placeholder capture is a hit"
        );
        assert!(
            MONITOR_PAD_HITS.load(Ordering::Relaxed) > before,
            "the hit counter is wired to the transform"
        );
        assert_monitor_pad_is_live();
        // A flushed capture is not accounted at all — only the unflushed state
        // can carry the placeholder.
        let misses = MONITOR_PAD_MISSES.load(Ordering::Relaxed);
        assert_eq!(
            expected_monitor_channel(3, &[1.0, 2.0, 3.0]),
            vec![1.0, 2.0, 3.0]
        );
        assert_eq!(
            MONITOR_PAD_MISSES.load(Ordering::Relaxed),
            misses,
            "a flushed monitor is not a placeholder candidate (a non-zero miss \
             count here means the corpus gate sharing this binary just recorded \
             a real miss — see assert_monitor_pad_is_live)"
        );
    }

    /// The one element-channel exclusion: the `newton*` decks' `Powers`/
    /// `Losses` are dropped in **both** lanes since `GOLDEN_REBASE_PLAN.md`
    /// G2.3 (no oracle channel reports them at the converged `NodeV`), their
    /// **currents** are kept in both, and no other case is touched.
    #[test]
    fn newton_powers_are_the_only_element_channel_exclusion() {
        for label in LANE_SKIP_ELEM_POWERS {
            let ch = elem_channels_for(label);
            assert!(ch.currents, "{label}: currents stay gated in every lane");
            // Field by field, not just `== CURRENTS_ONLY`: comparing a value
            // against the very constant it was built from is a tautology, so
            // flipping a field OF the constant would silently drop that channel
            // on every gated case in both lanes (G1.3a audit settlement).
            assert!(
                ch.currents_mag_ang && ch.voltages_mag_ang && ch.residuals,
                "{label}: the G1.3a polar channels stay gated in every lane —                  they render `Currents`/`NodeV`, not the cache-aware                  `Get_Powers`/`Get_Losses` read the Newton staleness lives in"
            );
            assert!(
                !ch.powers && !ch.losses,
                "{label}: powers/losses are the excluded pair"
            );
            assert_eq!(
                ch,
                ElemChannels::CURRENTS_ONLY,
                "{label}: powers/losses are excluded in both lanes"
            );
        }
        // The unexcluded default is every channel — the same anti-tautology
        // rule applied to `ALL` itself, so a field flipped there cannot go
        // unnoticed either.
        let all = ElemChannels::ALL;
        assert!(
            all.currents
                && all.powers
                && all.losses
                && all.currents_mag_ang
                && all.voltages_mag_ang
                && all.residuals,
            "ElemChannels::ALL must compare every channel; a `false` here              removes that channel from every gated case in both lanes"
        );
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

    /// The event-log transform touches exactly the two Relay rows and nothing
    /// else: it drops the relay's unguarded state trace, relabels the relay's
    /// reset event, and leaves every recloser line — including the *identically
    /// worded* recloser reset and the recloser's own (guarded) trace — alone.
    ///
    /// Asserted **without** a lane branch since `GOLDEN_REBASE_PLAN.md` G2.2d:
    /// both rows are upstream label mistakes, both lanes emit the corrected log,
    /// so both lanes apply both rewrites to the oracle capture. (The synthetic
    /// case owns no [`super::EVENTLOG_REROUNDED`] cell, so the still-lane-split
    /// `%g` fold cannot reach this expectation either way — that row has its own
    /// tests below.)
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
        let out = expected_eventlog("synthetic:relay/relabel.dss", &lines, |n| {
            n.eq_ignore_ascii_case("r1")
        });
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

    /// The F-FMT event-log cell: the traced `QoutPU` is re-spelled in the
    /// default lane only, case-insensitively (the capture reaches the transform
    /// in the engine's wording, the comparator upper-cases), and no other number
    /// on the same line — or a `QoutPU` carrying a different value — is touched.
    #[test]
    fn eventlog_reround_is_the_one_traced_cell() {
        let line = |q: &str| {
            format!(
                "Hour=1, Sec=0, ControlIter=2, Element=InvControl.ic, PVSystem.pv1c, \
                 Action=**Ready to change var output due to DRC trigger in DRC mode**, \
                 Vavgpu= 0.99868, VPriorpu=0.99892, QoutPU={q}, QDesiredEndpu=0"
            )
        };
        // The carrier's log must contain *both* recorded cells, each exactly
        // once — that is what `expected_eventlog` now asserts, so the synthetic
        // input has to be a faithful stand-in for the real capture.
        const CARRIER: &str = "controls:invcontrol/midi_invcontrol_drc.dss";
        let both = [line("-0.00188"), line("-0.00113")];
        let out = expected_eventlog(CARRIER, &both, |_| false);
        assert_eq!(
            out,
            if PARITY {
                vec![line("-0.00188"), line("-0.00113")]
            } else {
                vec![line("-0.00187"), line("-0.00112")]
            },
            "the traced cells are re-spelled in the default lane only"
        );
        // Upper-cased capture: same rewrite, the rest of the line untouched.
        let upper: Vec<String> = both.iter().map(|l| l.to_uppercase()).collect();
        let got = expected_eventlog(CARRIER, &upper, |_| false);
        assert_eq!(
            got[0].contains("-0.00187"),
            !PARITY,
            "the match must not depend on the capture's case"
        );
        assert!(got[0].contains("VAVGPU= 0.99868"), "no other cell moves");

        // A different value under the same key is left alone in both lanes —
        // checked on a case that owns no cells, since a carrier whose cells are
        // absent is exactly the staleness the assertion above now rejects.
        for other in ["-0.00187", "-0.0019", "0.00188"] {
            assert_eq!(
                expected_eventlog("synthetic:no/cells.dss", &[line(other)], |_| false),
                vec![line(other)]
            );
        }
    }

    /// A cell that names a case is applied to that case **only**, and a cell
    /// that no longer occurs in its own case fails loudly rather than sitting
    /// there un-gating a number.
    #[test]
    fn eventlog_reround_cells_are_case_scoped_and_fail_on_stale() {
        let line = |q: &str| format!("Hour=1, Element=InvControl.ic, QoutPU={q}, End=0");

        // Same text, a different case: untouched in both lanes.
        assert_eq!(
            expected_eventlog("controls:invcontrol/other.dss", &[line("-0.00188")], |_| {
                false
            }),
            vec![line("-0.00188")],
            "a re-round cell must not reach a case it does not name"
        );

        // The over-broad guard: one cell may re-spell one rendered number, so a
        // log carrying the same cell twice is refused. (The parity lane returns
        // before the re-round fold, so no cell runs there and there is nothing
        // to over-match.)
        if !PARITY {
            let twice = std::panic::catch_unwind(|| {
                expected_eventlog(
                    "controls:invcontrol/midi_invcontrol_drc.dss",
                    &["Hour=1, QoutPU=-0.00188, Then=1, QoutPU=-0.00188, End=0".to_string()],
                    |_| false,
                )
            });
            assert!(
                twice.is_err(),
                "a cell matching twice in one log is a pattern, not a cell"
            );
        }

        // Liveness is asserted per *case*, not per step — the log grows as the
        // case steps, so a cell fires 0 times before its line is emitted. With
        // no case visited, the check must stay silent (this is also what makes
        // it safe under `DSS_GATE_ONLY`).
        assert_reround_cells_are_live();
    }

    /// The relabel is refused when the name is ambiguous — a circuit holding a
    /// Relay *and* a Recloser called `r1` must fail its compare loudly instead
    /// of having a genuine recloser event rewritten into a relay's. Since G2.2d
    /// the rewrite runs in both lanes, so this refusal is asserted in both: the
    /// predicate, not the lane, is what holds it back.
    #[test]
    fn eventlog_reset_relabel_needs_an_unambiguous_relay() {
        let line = "Hour=0, Sec=1, ControlIter=1, Element=Recloser.r1, \
                    Action=PHASE 1 RESET (1PH RESET)"
            .to_string();
        let lines = vec![line];
        assert_eq!(
            expected_eventlog("synthetic:relay/ambiguous.dss", &lines, |_| false),
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

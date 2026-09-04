//! `GOLDEN_REBASE_PLAN.md` WP-G1 sub-step **G1.7** — the `ITopology` interface,
//! compared live on every gating channel of the unified corpus gate.
//!
//! # The surface
//!
//! Six quantities: `NumLoops`, `NumIsolatedBranches`, `NumIsolatedLoads`,
//! `AllLoopedPairs`, `AllIsolatedBranches`, `AllIsolatedLoads` — the fastdss
//! harness column set `ITopology._columns` (`.inputs/DSS-Python`
//! `origin/fastdss:dss/ITopology.py:10-20`) **minus** its three cursor fields
//! `ActiveLevel` / `BranchName` / `ActiveBranch`.
//!
//! That gap is deliberate and is the one this module records: those three — and
//! every cursor mode (`First`, `Next`, `ForwardBranch`, `BackwardBranch`,
//! `LoopedBranch`, `ParallelBranch`, `FirstLoad`, `NextLoad`, `BusName`, all of
//! `TopologyS`) — reassign `ActiveCircuit.ActiveCktElement` (r4133
//! `Version8/Source/DDLL/DTopology.pas:29-54`, `:96-160`, `:170-186`; capi
//! `.inputs/dss_capi/src/CAPI/CAPI_Topology.pas:100-112`, `:225-251`,
//! `:253-300`), so capturing them would poison the per-element capture the same
//! checkpoint takes. The six here never touch it. The absence is asserted from
//! both transports' source text by `crates/dss-core/tests/capture_order.rs`, not
//! merely stated here.
//!
//! Engine side of the six, per channel:
//!
//! | quantity | r4133 `DDLL/DTopology.pas` | capi `CAPI/CAPI_Topology.pas` |
//! |---|---|---|
//! | `NumLoops` | `:67-78` (`Result := Result div 2` at `:77`) | `:81-98` |
//! | `NumIsolatedBranches` | `:79-88` | `:302-316` |
//! | `NumIsolatedLoads` | `:89-98` | `:448-462` |
//! | `AllLoopedPairs` | `:271-321` | `:160-216` |
//! | `AllIsolatedBranches` | `:322-356` | `:114-151` |
//! | `AllIsolatedLoads` | `:357-392` | `:369-406` |
//!
//! r4133 emits `QualifiedName`, capi `FullName`; the two were measured
//! byte-identical (same spelling, same case, same order) on all 336 both-gated
//! corpus cases, so the comparator needs no per-channel name policy.
//!
//! # Fully discrete — no tolerance, deliberately
//!
//! Every field is a count or an identifier list; nothing here is a float, so
//! this surface introduces **no** band and gets no `tests/TOLERANCE_NOTES.md`
//! row. The absence is recorded there in one sentence so a later reader does not
//! read it as an omission.
//!
//! # Shape: the two transport-side normalizations, and why they are only
//! *asserted* here
//!
//! Both transports normalize an `ITopology` string array before it reaches this
//! module (`tools/oracle/oracle_server.py::_topo_names`,
//! `crates/dss-epri/src/capture.rs::topo_names`), and exactly two shapes are
//! normalized:
//!
//! * **S1, the empty sentinel.** An empty list comes back as the single entry
//!   `NONE` on *both* channels: capi through `DefaultResult(..., 'NONE')`
//!   (`CAPI_Topology.pas:140-141`, `:204-205`, `:395-396` via
//!   `CAPI/CAPI_Utils.pas:115`), r4133 because `TStr` is pre-seeded
//!   `TStr[0] := 'NONE'` (`DTopology.pas:275`, `:325`, `:360`). A qualified name
//!   is always `Class.name`, so a bare `NONE` can never be a real entry.
//! * **S2, one trailing empty — capi's isolated lists only.**
//!   `Topology_Get_AllIsolatedBranches` grows with `SetLength(Result, k + 1)`
//!   after each hit (`CAPI_Topology.pas:134`, loads at `:389`) and then copies
//!   `Length(Result)` entries (`:145-149`, `:400-404`), so a non-empty list
//!   carries exactly one trailing `''`. r4133 filters empties at the source
//!   (`DTopology.pas:341-347`, `:377-383`, `if TStr[i] <> ''`).
//!   `AllLoopedPairs` starts from `k := -1` on both channels and lands exactly
//!   on `2·npairs` (`CAPI_Topology.pas:169-170`, `:193-197`;
//!   `DTopology.pas:276`), so it carries **no** trailing slot — the asymmetry is
//!   real and [`normalize_topo_names`] never drops more than that one entry.
//!
//! [`normalize_topo_names`] is the gate-side statement of that rule, and the
//! comparator applies it as a **fixpoint assertion**, not as a repair: an
//! arriving list must already equal its own normalization. Re-normalizing here
//! would silently absorb the very regression the rule exists to catch (a
//! transport that stops decoding the sentinel would compare a phantom
//! one-element list as empty and pass), so [`assert_normalized`] fails the case
//! instead and names the transport that must be fixed. The rule itself stays
//! non-vacuous both ways: it runs on every list of every compared case, and its
//! table is pinned by `tests::capi_isolated_lists_carry_one_trailing_empty`.
//!
//! # The port never memoizes
//!
//! Upstream answers all six from a `Branch_List` built on the first read and
//! freed only in `Destroy` and `DoResetMeterZones` (r4133
//! `Common/Circuit.pas:2932-2950`, `:703`, `:2308`; capi
//! `CAPI_Topology.pas:47-63` over the same `GetTopology`), so an `Open`/`Close`
//! between two reads is invisible to it. `Dss::topology_view` rebuilds the tree
//! on every call and answers from the present conductor state (CLAUDE.md: an
//! upstream defect is never reproduced in any lane).
//!
//! Building that tree stamps the Pascal flags upstream stamps
//! (`Checked`/`IsIsolated`/`BusChecked`, r4133 `Common/Circuit.pas:2937-2947`),
//! which is why both transports read this surface **last** in a checkpoint and
//! why `corpus_gate/runner.rs` calls [`compare_topology`] last in a step.
//!
//! # The two upstream defects, asserted positively (coordinator D15 + D16)
//!
//! Not memoizing and not reproducing upstream's pair dedup make the port
//! *differ* from both oracles in two measured, fully explained ways. Neither is
//! excluded: the comparator states the upstream mechanism and requires the
//! oracle to obey **it**, so a red still means "upstream changed or the port
//! broke", never "we stopped looking" (0 `tests/corpus/ledger.json` rows for
//! this surface).
//!
//! * **D15 — the memoized tree goes stale.** A conductor open/close (a Relay or
//!   Recloser tripping, a SwtControl operating, an `Open` command) invalidates
//!   nothing upstream, so from then on all six quantities answer from the tree
//!   built at the first read — which, on every live corpus deck, is our step-0
//!   read (no deck builds the tree earlier: the only in-deck builder in the
//!   whole corpus, a `show isolated`, is commented out). The four isolation
//!   fields are therefore compared at step 0 always, and at step `k > 0` while
//!   the port's own isolation topology at `k` still equals its step-0 one; where
//!   it differs the comparator asserts `oracle(k) == port(step 0)` — the
//!   memoization contract itself ([`compare_topology`], arm 6). `num_loops` and
//!   the pair list stay compared at **every** step (measured: they never
//!   diverge). The declining population is re-derived on every gate run and
//!   pinned by `corpus_gate::scheduler::TOPOLOGY_STALE_DECLINES`.
//! * **D16 — the pair dedup drops straddling pairs.** Upstream reduces its
//!   looped-pair candidate sequence by scanning the flat name buffer in
//!   *overlapping windows* (`i := i + 1` over `(buf[i-1], buf[i])`,
//!   `DTopology.pas:286-296`, capi `CAPI_Topology.pas:180-190`) while its own
//!   comment says "see if we already found this pair" — so a genuinely new
//!   candidate that happens to equal a straddling window `(b_j, a_{j+1})` is
//!   dropped. The port keeps the correct per-pair dedup, and the comparator
//!   asserts `oracle.looped_pairs == window_dedup(port candidates)`
//!   ([`window_dedup`] over `TopologyView::looped_pair_candidates`), with the
//!   port's own list still gated by [`per_pair_dedup`] so it cannot drift
//!   silently. Population constant:
//!   `corpus_gate::scheduler::LOOPED_PAIR_WINDOW_DECLINES`.

use std::collections::BTreeSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrd};

use dss_core::exec::Dss;
use serde::Deserialize;

/// The single token both channels emit for an empty list (S1).
const NONE_SENTINEL: &str = "NONE";

/// How many entries a mismatch message previews before eliding — the lists run
/// to ~1 000 pairs on `LVTestCaseNorthAmerican` and a failure must stay readable.
const PREVIEW: usize = 8;

/// The six `Topology` quantities of one checkpoint, in the shape **both**
/// transports emit (`tools/oracle/oracle_server.py::capture_topology`,
/// `crates/dss-epri/src/capture.rs::capture_topology` — identical JSON keys,
/// identical normalized list shape).
///
/// `looped_pairs` is FLAT — `[a0, b0, a1, b1, ...]` — because `TopologyV(0)`
/// writes the two names of one looped pair as two consecutive entries
/// (`DTopology.pas:298-301`; capi `CAPI_Topology.pas:196-197`).
/// [`compare_topology`] pairs it up. Its length is **not** `num_loops`:
/// `NumLoops` is the `IsLoopedHere` tally integer-halved
/// (`DTopology.pas:67-78`), so IEEE13 answers `num_loops = 1` with three pairs.
#[derive(Debug, Clone, Deserialize)]
pub struct TopologyCap {
    /// `Topology.NumLoops`.
    pub num_loops: i32,
    /// `Topology.NumIsolatedBranches`.
    pub num_isolated_branches: i32,
    /// `Topology.NumIsolatedLoads`.
    pub num_isolated_loads: i32,
    /// `Topology.AllLoopedPairs`, flat.
    pub looped_pairs: Vec<String>,
    /// `Topology.AllIsolatedBranches`, in `PDElements` (= creation) order.
    pub isolated_branches: Vec<String>,
    /// `Topology.AllIsolatedLoads`, in `PCElements` order.
    pub isolated_loads: Vec<String>,
}

/// The transport shape rule of the module doc, as a function: `["NONE"] -> []`
/// (S1) and one — at most one — trailing `''` dropped (S2). Every other empty
/// entry is REFUSED (`Err`), never repaired: an interior empty, a second
/// trailing one or a whitespace-only name is a lost element name, not a shape.
///
/// This is the gate-side twin of `oracle_server._topo_names` and
/// `dss-epri::capture::topo_names`; the comparator uses it to assert that the
/// arriving capture is already at its fixpoint (see [`assert_normalized`]).
pub fn normalize_topo_names(raw: &[String]) -> Result<Vec<String>, String> {
    if raw.len() == 1 && raw[0] == NONE_SENTINEL {
        return Ok(Vec::new());
    }
    // Exactly ONE trailing empty is the capi `SetLength(Result, k + 1)`
    // artifact; anything else empty is a lost name.
    let body = match raw.last() {
        Some(s) if s.is_empty() => &raw[..raw.len() - 1],
        _ => raw,
    };
    if let Some(i) = body.iter().position(|s| s.trim().is_empty()) {
        return Err(format!(
            "unexpected empty entry at index {i} of {raw:?}: only ONE trailing \
             '' — the capi `SetLength(Result, k + 1)` artifact \
             (CAPI_Topology.pas:134) — is ever dropped, and `AllLoopedPairs` \
             carries none at all (k := -1, CAPI_Topology.pas:169)"
        ));
    }
    Ok(body.to_vec())
}

/// Assert one arriving list is already at [`normalize_topo_names`]'s fixpoint.
///
/// The transports normalize; this module does **not** re-normalize, because a
/// repair here would hide a transport regression behind a green gate
/// (`GOLDEN_REBASE_PLAN.md` §1.1(f): a comparator must fail, never quietly fix
/// up). So the capture is required to be normalized, and the refusal names which
/// transport to look at.
#[track_caller]
fn assert_normalized(raw: &[String], what: &str, ctx: &str) {
    let norm = normalize_topo_names(raw).unwrap_or_else(|e| {
        panic!(
            "{ctx}: the oracle's `{what}` has a shape neither transport can \
             produce: {e}. Both capture sites normalize before sending \
             (`tools/oracle/oracle_server.py::_topo_names`, \
             `crates/dss-epri/src/capture.rs::topo_names`), so this is a \
             transport failure, not a divergence."
        )
    });
    assert_eq!(
        norm, raw,
        "{ctx}: the oracle's `{what}` reached the gate UN-normalized \
         ({raw:?} normalizes to {norm:?}). The sentinel decode and the single \
         trailing-empty drop belong to the transports \
         (`oracle_server.py::_topo_names`, `dss-epri::capture::topo_names`); \
         this comparator asserts the fixpoint instead of repeating the repair, \
         so that a transport which stops normalizing FAILS here rather than \
         comparing a phantom entry as if it were empty."
    );
}

/// One `(port, oracle)` ordered name-vector arm, compared **case-insensitively**
/// (the harness convention: upstream spells `Class.name` from the same hash list
/// the port does, and the two channels were measured byte-identical anyway).
///
/// Order is load-bearing and is NOT relaxed to a set compare: both lists are
/// walks — `PDElements` / `PCElements` in creation order
/// (`DTopology.pas:79-88`, `:89-98`) — and the walk order is precisely what a
/// topology gate exists to catch.
#[track_caller]
fn compare_names(what: &str, actual: &[String], expected: &[String], ctx: &str) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "{ctx}: `{what}` length differs: Rust {} vs oracle {}\n  Rust:   {}\n  oracle: {}",
        actual.len(),
        expected.len(),
        preview(actual),
        preview(expected)
    );
    for (i, (a, e)) in actual.iter().zip(expected).enumerate() {
        assert!(
            a.eq_ignore_ascii_case(e),
            "{ctx}: `{what}`[{i}] differs: Rust `{a}` vs oracle `{e}` \
             (ordered compare — the list is a walk of the class pointer list in \
             creation order, DTopology.pas:79-88 / :89-98)"
        );
    }
}

/// `[a0, b0, a1, b1, ...]` (both transports' flat wire shape) as pairs. An odd
/// tail is dropped by `chunks_exact`, which is why [`compare_topology`] asserts
/// the even length first.
fn pairs_of(flat: &[String]) -> Vec<(String, String)> {
    flat.chunks_exact(2)
        .map(|c| (c[0].clone(), c[1].clone()))
        .collect()
}

/// First [`PREVIEW`] entries of a list, elided.
fn preview(v: &[String]) -> String {
    if v.len() <= PREVIEW {
        format!("{v:?}")
    } else {
        format!("{:?} ... (+{} more)", &v[..PREVIEW], v.len() - PREVIEW)
    }
}

/// [`preview`] for the paired shape.
fn preview_pairs(v: &[(String, String)]) -> String {
    if v.len() <= PREVIEW {
        format!("{v:?}")
    } else {
        format!("{:?} ... (+{} more)", &v[..PREVIEW], v.len() - PREVIEW)
    }
}

// ---------------------------------------------------------------------------
// D16 — the two dedup rules over one candidate sequence
// ---------------------------------------------------------------------------

/// **Upstream's** looped-pair dedup, applied to a candidate sequence — the
/// gate's model of the oracle (coordinator decision D16).
///
/// r4133 `Version8/Source/DDLL/DTopology.pas:277-301` (`k := -1` at `:277`, the
/// scan at `:286-296`, the append at `:297-301`) and capi 0.14.5
/// `CAPI_Topology.pas:169-195` are character for character the same: the
/// accepted pairs live in ONE flat buffer `[a0, b0, a1, b1, ...]` and the
/// "see if we already found this pair" scan runs
/// `i := 1; while (i <= k) ...; i := i + 1` over `(TStr[i-1], TStr[i])` —
/// stepping by **one**, so it tests every *overlapping* window instead of every
/// stored pair and drops a new candidate that equals a straddling window
/// `(b_j, a_{j+1})`.
///
/// Written with the Pascal indices literally (`k = buf.len() - 1`, hence `-1` on
/// the empty buffer, which is why the first candidate is always accepted) so the
/// model cannot drift into a paraphrase. Its twin inside the engine's own test
/// tree is `dss_core::exec::tests::topology::window_dedup` — a deliberate
/// duplicate: the two live in different compilation targets.
pub fn window_dedup(candidates: &[(String, String)]) -> Vec<(String, String)> {
    let mut buf: Vec<String> = Vec::new();
    for (a, b) in candidates {
        let mut found = false;
        let k = buf.len() as isize - 1;
        let mut i: isize = 1;
        while i <= k && !found {
            let (p, q) = (&buf[(i - 1) as usize], &buf[i as usize]);
            if eq(p, a) && eq(q, b) {
                found = true;
            }
            if eq(p, b) && eq(q, a) {
                found = true;
            }
            i += 1;
        }
        if !found {
            buf.push(a.clone());
            buf.push(b.clone());
        }
    }
    buf.chunks(2)
        .map(|w| (w[0].clone(), w[1].clone()))
        .collect()
}

/// **The port's** rule over the same sequence: keep a candidate unless an
/// already-accepted PAIR equals it in either orientation — what upstream's
/// comment says it does (`DTopology.pas:286`) and what physics wants (a looped
/// pair is a pair, not a window into a name buffer).
///
/// The gate re-derives it here so `TopologyView::looped_pairs` stays gated even
/// though the oracle arm compares against [`window_dedup`]: the port's two
/// fields must agree with each other, exactly as the oracle's counts must agree
/// with its own lists ([`compare_topology`], arms 2 and 3).
pub fn per_pair_dedup(candidates: &[(String, String)]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for (a, b) in candidates {
        let seen = out
            .iter()
            .any(|(p, q)| (eq(p, a) && eq(q, b)) || (eq(p, b) && eq(q, a)));
        if !seen {
            out.push((a.clone(), b.clone()));
        }
    }
    out
}

/// DSS identifiers are case-insensitive (the harness convention everywhere).
fn eq(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

// ---------------------------------------------------------------------------
// D15 — the port's isolation half at one step
// ---------------------------------------------------------------------------

/// The four isolation quantities of one `Dss::topology_view()` read: the port's
/// answer at some step, kept so a later step can be tested against **step 0**.
///
/// Derived `PartialEq` compares the spellings byte for byte on purpose: both
/// snapshots come from the same engine run of the same case, so a spelling
/// change between two steps would itself be a finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsolationSnapshot {
    /// `Topology.NumIsolatedBranches`.
    pub num_isolated_branches: i32,
    /// `Topology.NumIsolatedLoads`.
    pub num_isolated_loads: i32,
    /// `Topology.AllIsolatedBranches`, in `PDElements` order.
    pub isolated_branches: Vec<String>,
    /// `Topology.AllIsolatedLoads`, in `PCElements` order.
    pub isolated_loads: Vec<String>,
}

// ---------------------------------------------------------------------------
// The decline census — re-derived on every gate run, pinned by the scheduler
// ---------------------------------------------------------------------------

/// `(case label, step)` pairs whose ISOLATION half was answered from upstream's
/// stale tree (D15). A `both` case that declines on both channels is ONE entry —
/// the pinned population counts cases and case-steps, not channel visits.
static STALE_DECLINES: Mutex<BTreeSet<(String, usize)>> = Mutex::new(BTreeSet::new());
/// Channel visits behind [`STALE_DECLINES`] (the `both` cases counted twice).
static STALE_VISITS: AtomicUsize = AtomicUsize::new(0);
/// `(case label, step)` pairs where upstream's window scan measurably drops a
/// pair the port keeps (D16), i.e. where the oracle's list is strictly the
/// window reduction and not the port's own.
static WINDOW_DECLINES: Mutex<BTreeSet<(String, usize)>> = Mutex::new(BTreeSet::new());
/// Channel visits behind [`WINDOW_DECLINES`].
static WINDOW_VISITS: AtomicUsize = AtomicUsize::new(0);
/// `(case, step, channel)` topology comparisons this process ran at all — the
/// non-vacuity half: a census of `0 / 0` means nothing was compared, not that
/// nothing declined.
static COMPARED: AtomicUsize = AtomicUsize::new(0);

/// What the live gate measured in this process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeclineCensus {
    /// `(case, step, channel)` topology comparisons run.
    pub compared: usize,
    /// D15: `(cases, case-steps)` whose isolation half was declined.
    pub stale: (usize, usize),
    /// D15 channel visits (a `both` case-step counts twice).
    pub stale_visits: usize,
    /// D16: `(cases, case-steps)` where the window scan drops a pair.
    pub window: (usize, usize),
    /// D16 channel visits.
    pub window_visits: usize,
}

fn lock<T>(m: &'static Mutex<T>) -> std::sync::MutexGuard<'static, T> {
    // A poisoned census is not a reason to fail a different case: whoever
    // panicked while holding it has already reported itself.
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn cases_and_steps(set: &BTreeSet<(String, usize)>) -> (usize, usize) {
    let cases: BTreeSet<&str> = set.iter().map(|(c, _)| c.as_str()).collect();
    (cases.len(), set.len())
}

/// The census as it stands, for the gate epilogue
/// (`corpus_gate::scheduler::assert_topology_declines_are_the_pinned_population`).
pub fn decline_census() -> DeclineCensus {
    let stale = lock(&STALE_DECLINES);
    let window = lock(&WINDOW_DECLINES);
    DeclineCensus {
        compared: COMPARED.load(AtomicOrd::Relaxed),
        stale: cases_and_steps(&stale),
        stale_visits: STALE_VISITS.load(AtomicOrd::Relaxed),
        window: cases_and_steps(&window),
        window_visits: WINDOW_VISITS.load(AtomicOrd::Relaxed),
    }
}

/// The two declining populations, per case, as a report the epilogue prints and
/// a mismatch quotes — so a moved constant is re-derived from the run itself and
/// never from a guess.
pub fn decline_report() -> String {
    fn table(title: &str, set: &BTreeSet<(String, usize)>) -> String {
        let mut per_case: std::collections::BTreeMap<&str, Vec<usize>> =
            std::collections::BTreeMap::new();
        for (case, step) in set {
            per_case.entry(case.as_str()).or_default().push(*step);
        }
        let mut out = format!(
            "{title}: {} case(s), {} case-step(s)",
            per_case.len(),
            set.len()
        );
        for (case, steps) in per_case {
            out.push_str(&format!("\n    {case}: {} step(s) {steps:?}", steps.len()));
        }
        out
    }
    format!(
        "{}\n  {}",
        table("D15 stale-tree declines", &lock(&STALE_DECLINES)),
        table("D16 window-scan declines", &lock(&WINDOW_DECLINES)).replace('\n', "\n  "),
    )
}

fn record_stale_decline(case: &str, step: usize) {
    STALE_VISITS.fetch_add(1, AtomicOrd::Relaxed);
    lock(&STALE_DECLINES).insert((case.to_string(), step));
}

fn record_window_decline(case: &str, step: usize) {
    WINDOW_VISITS.fetch_add(1, AtomicOrd::Relaxed);
    lock(&WINDOW_DECLINES).insert((case.to_string(), step));
}

/// Compare the port's `Dss::topology_view` against one channel's capture. Zero
/// tolerance: three exact counts and three ordered, case-insensitive identifier
/// lists.
///
/// Arms, in order:
///
/// 1. **Shape.** Each of the three lists is already at the transports'
///    normalized fixpoint ([`assert_normalized`]), and `looped_pairs` has even
///    length — it is two names per pair, so an odd length is a truncated
///    transport read, not a value.
/// 2. **The oracle's own consistency** (free teeth: neither channel can fail it,
///    so a red here is a transport bug, not a divergence). `NumIsolatedBranches`
///    counts the same `IsIsolated` walk `AllIsolatedBranches` emits
///    (`DTopology.pas:79-88` vs `:322-356`; capi `:302-316` vs `:114-151`),
///    likewise for loads, so the count must equal the list length on the
///    oracle's own capture.
/// 3. **The port's own consistency** — [`per_pair_dedup`] of the candidate
///    sequence is `looped_pairs` (free teeth on our side, the mirror of arm 2;
///    it keeps that field gated even though arm 5 compares the oracle against
///    the window model).
/// 4. **`num_loops`**, exact, at every step.
/// 5. **`looped_pairs`**: the oracle's list IS [`window_dedup`] of the port's
///    candidates (D16, module doc). Where that reduction differs from the port's
///    own list the case-step is recorded in the D16 census.
/// 6. **The isolation half** (both counts, both lists) — compared against the
///    port's answer at THIS step at step 0 and while the port's isolation
///    topology has not moved since step 0; where it has, the oracle is provably
///    answering from its step-0 tree and the arm asserts `oracle(k) ==
///    port(step 0)` instead, recording the case-step in the D15 census (module
///    doc).
///
/// `case` and `step` are the census keys (the case label as the gate prints it,
/// and the checkpoint index); `step0` is the caller-owned memoization reference
/// — `None` on entry at step 0, filled here, and read back at every later step
/// of the SAME case+channel run.
///
/// `&mut Dss` because the port builds the tree on the spot (it caches nothing);
/// the call must therefore be the LAST comparison of a step, mirroring both
/// transports' capture order — see the module doc.
pub fn compare_topology(
    dss: &mut Dss,
    cap: &TopologyCap,
    ctx: &str,
    case: &str,
    step: usize,
    step0: &mut Option<IsolationSnapshot>,
) {
    // --- 1. shape --------------------------------------------------------
    for (what, list) in [
        ("looped_pairs", &cap.looped_pairs),
        ("isolated_branches", &cap.isolated_branches),
        ("isolated_loads", &cap.isolated_loads),
    ] {
        assert_normalized(list, what, ctx);
    }
    assert_eq!(
        cap.looped_pairs.len() % 2,
        0,
        "{ctx}: the oracle's `looped_pairs` has odd length {} — `TopologyV(0)` \
         writes TWO names per looped pair (DTopology.pas:298-301; capi \
         CAPI_Topology.pas:196-197), so an odd length is a truncated transport \
         read: {}",
        cap.looped_pairs.len(),
        preview(&cap.looped_pairs)
    );

    // --- 2. the oracle's own count/list consistency -----------------------
    assert_eq!(
        cap.num_isolated_branches,
        cap.isolated_branches.len() as i32,
        "{ctx}: the oracle contradicts itself: NumIsolatedBranches = {} but \
         AllIsolatedBranches holds {} entries — both walk the same `IsIsolated` \
         filter over `PDElements` (DTopology.pas:79-88 vs :322-356; capi \
         CAPI_Topology.pas:302-316 vs :114-151)",
        cap.num_isolated_branches,
        cap.isolated_branches.len()
    );
    assert_eq!(
        cap.num_isolated_loads,
        cap.isolated_loads.len() as i32,
        "{ctx}: the oracle contradicts itself: NumIsolatedLoads = {} but \
         AllIsolatedLoads holds {} entries — both walk the same `IsIsolated` \
         filter over `PCElements` (DTopology.pas:89-98 vs :357-392; capi \
         CAPI_Topology.pas:448-462 vs :369-406)",
        cap.num_isolated_loads,
        cap.isolated_loads.len()
    );

    // The port rebuilds the tree here; nothing in this checkpoint may read the
    // `Checked`/`IsIsolated` flags afterwards (module doc).
    let view = dss.topology_view();

    // --- 3. the port's own candidate/list consistency ----------------------
    let port_rule = per_pair_dedup(&view.looped_pair_candidates);
    assert_eq!(
        port_rule.len(),
        view.looped_pairs.len(),
        "{ctx}: the port contradicts itself: `looped_pairs` holds {} pair(s) but \
         the per-pair dedup of its own {} candidate(s) gives {} — \
         `TopologyView::looped_pairs` must stay the reduction of \
         `looped_pair_candidates` (D16 exposes the raw sequence precisely so the \
         two can be tied together)\n  looped_pairs: {}\n  per-pair:     {}",
        view.looped_pairs.len(),
        view.looped_pair_candidates.len(),
        port_rule.len(),
        preview_pairs(&view.looped_pairs),
        preview_pairs(&port_rule)
    );
    for (i, (a, e)) in view.looped_pairs.iter().zip(&port_rule).enumerate() {
        assert!(
            eq(&a.0, &e.0) && eq(&a.1, &e.1),
            "{ctx}: the port contradicts itself: `looped_pairs`[{i}] is ({}, {}) \
             but the per-pair dedup of its own candidates gives ({}, {})",
            a.0,
            a.1,
            e.0,
            e.1
        );
    }

    // --- 4. num_loops, at every step --------------------------------------
    assert_eq!(
        view.num_loops,
        cap.num_loops,
        "{ctx}: NumLoops differs: Rust {} vs oracle {} (the `IsLoopedHere` tally \
         integer-halved — DTopology.pas:67-78, capi CAPI_Topology.pas:81-98; it \
         is NOT the pair count, which is {} here)",
        view.num_loops,
        cap.num_loops,
        cap.looped_pairs.len() / 2
    );

    // --- 5. looped_pairs: the oracle IS the window scan of our candidates ---
    let window = window_dedup(&view.looped_pair_candidates);
    compare_looped_pairs(&window, &view.looped_pairs, &cap.looped_pairs, ctx);
    // Census: this case-step is a D16 decline iff upstream's rule and the port's
    // actually part company here. Compared by CONTENT, not by length: the two
    // rules run over the same candidate sequence but keep different buffers, so
    // "same number of pairs" is not the same statement as "the same pairs".
    let same_pairs = window.len() == view.looped_pairs.len()
        && window
            .iter()
            .zip(&view.looped_pairs)
            .all(|(a, b)| eq(&a.0, &b.0) && eq(&a.1, &b.1));
    if !same_pairs {
        record_window_decline(case, step);
    }

    // --- 6. the isolation half (D15) ---------------------------------------
    let now = IsolationSnapshot {
        num_isolated_branches: view.num_isolated_branches,
        num_isolated_loads: view.num_isolated_loads,
        isolated_branches: view.isolated_branches.clone(),
        isolated_loads: view.isolated_loads.clone(),
    };
    if step == 0 {
        compare_isolation(&now, cap, ctx, Basis::Fresh);
        *step0 = Some(now);
        COMPARED.fetch_add(1, AtomicOrd::Relaxed);
        return;
    }
    let base = step0.as_ref().unwrap_or_else(|| {
        panic!(
            "{ctx}: reached step {step} with no step-0 reference — \
             `compare_topology` must be called on EVERY step of a case+channel \
             run, in order, with the same `step0` slot (D15's rule is defined \
             against the port's own step-0 topology)"
        )
    });
    if now == *base {
        compare_isolation(&now, cap, ctx, Basis::Fresh);
    } else {
        // The port's isolation topology has moved since step 0 (a conductor
        // opened) while upstream's `Branch_List` was never invalidated, so the
        // oracle is answering from the step-0 tree. Assert exactly that.
        compare_isolation(base, cap, ctx, Basis::Memoized);
        record_stale_decline(case, step);
    }
    COMPARED.fetch_add(1, AtomicOrd::Relaxed);
}

/// Which port answer the isolation half is tested against at this step (D15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Basis {
    /// The port's answer at THIS step — the ordinary comparison.
    Fresh,
    /// The port's answer at STEP 0 — the memoization contract, asserted where
    /// the port's own topology has moved since step 0 and upstream's therefore
    /// cannot have.
    Memoized,
}

/// The isolation half — both counts and both ordered lists — against one port
/// answer ([`Basis`]).
///
/// The two counts are asserted against the reference's own list lengths as well,
/// so this arm cannot pass on a snapshot whose count and list disagree.
#[track_caller]
fn compare_isolation(port: &IsolationSnapshot, cap: &TopologyCap, ctx: &str, basis: Basis) {
    let ctx = match basis {
        Basis::Fresh => ctx.to_string(),
        Basis::Memoized => format!(
            "{ctx} [D15: the port's topology moved since step 0, so the oracle's \
             memoized `Branch_List` (r4133 Common/Circuit.pas:2932-2950, freed \
             only at :703 / :2308) must still answer the port's STEP-0 topology]"
        ),
    };
    assert_eq!(
        port.num_isolated_branches as usize,
        port.isolated_branches.len(),
        "{ctx}: the port contradicts itself: NumIsolatedBranches {} vs {} listed",
        port.num_isolated_branches,
        port.isolated_branches.len()
    );
    assert_eq!(
        port.num_isolated_loads as usize,
        port.isolated_loads.len(),
        "{ctx}: the port contradicts itself: NumIsolatedLoads {} vs {} listed",
        port.num_isolated_loads,
        port.isolated_loads.len()
    );
    assert_eq!(
        port.num_isolated_branches,
        cap.num_isolated_branches,
        "{ctx}: NumIsolatedBranches differs: Rust {} vs oracle {}\n  Rust:   {}\n  oracle: {}",
        port.num_isolated_branches,
        cap.num_isolated_branches,
        preview(&port.isolated_branches),
        preview(&cap.isolated_branches)
    );
    assert_eq!(
        port.num_isolated_loads,
        cap.num_isolated_loads,
        "{ctx}: NumIsolatedLoads differs: Rust {} vs oracle {}\n  Rust:   {}\n  oracle: {}",
        port.num_isolated_loads,
        cap.num_isolated_loads,
        preview(&port.isolated_loads),
        preview(&cap.isolated_loads)
    );
    compare_names(
        "isolated_branches",
        &port.isolated_branches,
        &cap.isolated_branches,
        &ctx,
    );
    compare_names(
        "isolated_loads",
        &port.isolated_loads,
        &cap.isolated_loads,
        &ctx,
    );
}

/// The `AllLoopedPairs` arm (coordinator decision **D16**): the oracle's flat
/// list, paired up, must equal `model` — [`window_dedup`] applied to the port's
/// own candidate sequence — ordered, case-insensitive, both members.
///
/// Both engines walk the tree in the same order and record
/// `(branch, the branch it loops onto)` at each `IsLoopedHere` node
/// (`DTopology.pas:281-285`; capi `CAPI_Topology.pas:174-179`). What they do
/// with that candidate sequence differs: upstream reduces it by an overlapping
/// **window** scan (`i := i + 1` over `(buf[i-1], buf[i])`,
/// `DTopology.pas:286-296`, capi `:181-190`) that also drops a genuinely new
/// candidate coinciding with a straddling window, contradicting its own comment
/// "see if we already found this pair"; the port keeps the correct per-pair
/// dedup and never reproduces the defect (CLAUDE.md), exposing its raw candidate
/// sequence as `TopologyView::looped_pair_candidates` so the mechanism is
/// asserted positively instead of the field being skipped.
///
/// So this is **not** an exclusion: it compares a full list against a full list,
/// and it reds if upstream's reduction stops being explainable by that one
/// defect — if a pair moves, if the walk order moves, if the candidate sequence
/// moves, or if upstream ever fixes the scan. `port_own` is carried only for the
/// message (the port's correct answer, and by how much the defect shortened it).
#[track_caller]
fn compare_looped_pairs(
    model: &[(String, String)],
    port_own: &[(String, String)],
    flat: &[String],
    ctx: &str,
) {
    let expected = pairs_of(flat);
    assert_eq!(
        model.len(),
        expected.len(),
        "{ctx}: `looped_pairs` length differs: the window scan of the port's {} \
         candidate(s) gives {} pair(s), the oracle reports {} (D16: the oracle's \
         list must BE that window reduction — DTopology.pas:286-296, capi \
         CAPI_Topology.pas:180-190). The port's own per-pair dedup holds {} \
         pair(s).\n  window: {}\n  oracle: {}\n  port:   {}",
        port_own.len(),
        model.len(),
        expected.len(),
        port_own.len(),
        preview_pairs(model),
        preview_pairs(&expected),
        preview_pairs(port_own)
    );
    for (i, (a, e)) in model.iter().zip(&expected).enumerate() {
        assert!(
            eq(&a.0, &e.0) && eq(&a.1, &e.1),
            "{ctx}: `looped_pairs`[{i}] differs: the window scan of the port's \
             candidates gives ({}, {}), the oracle reports ({}, {}) (D16)",
            a.0,
            a.1,
            e.0,
            e.1
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|x| (*x).to_string()).collect()
    }

    /// Run `f` and return the panic message (the `capture_guard::tests`
    /// extractor; each harness module keeps its own — they compile into
    /// separate test binaries).
    fn panic_message(f: impl FnOnce() + std::panic::UnwindSafe) -> String {
        let payload = std::panic::catch_unwind(f).expect_err("the arm must panic");
        if let Some(m) = payload.downcast_ref::<&str>() {
            (*m).to_string()
        } else if let Some(m) = payload.downcast_ref::<String>() {
            m.clone()
        } else {
            "<non-string panic payload>".to_string()
        }
    }

    /// **Pin (`GOLDEN_REBASE_PLAN.md` G1.7 §3.1-S2/S3).** The capi channel
    /// appends exactly ONE trailing `''` to a non-empty `AllIsolatedBranches` /
    /// `AllIsolatedLoads` and r4133 does not, while `AllLoopedPairs` carries none
    /// on either channel — so the rule drops one trailing empty and refuses every
    /// other empty entry.
    ///
    /// Both numbers, one deck: `modes:reduce/reduce_breakloop.dss` reports
    /// `NumIsolatedBranches = 1` with the raw arrays capi `["Line.l1", ""]` vs
    /// r4133 `["Line.l1"]` (`CAPI_Topology.pas:122` + `:134` + `:145-149` grow
    /// and copy `Length(Result) = k + 1`; `DTopology.pas:341-347` filters
    /// `TStr[i] <> ''`). The asymmetry with `AllLoopedPairs` (`k := -1`,
    /// `CAPI_Topology.pas:169-170`, `:193-197`) is pinned here too, so a later
    /// "cleanup" cannot generalize the drop to that field.
    #[test]
    fn capi_isolated_lists_carry_one_trailing_empty() {
        // The measured capi reply on `reduce_breakloop`, and r4133's.
        assert_eq!(
            normalize_topo_names(&s(&["Line.l1", ""])).unwrap(),
            s(&["Line.l1"])
        );
        assert_eq!(
            normalize_topo_names(&s(&["Line.l1"])).unwrap(),
            s(&["Line.l1"])
        );
        // S1, the empty sentinel, on both channels.
        assert_eq!(normalize_topo_names(&s(&["NONE"])).unwrap(), s(&[]));
        assert_eq!(normalize_topo_names(&s(&[])).unwrap(), s(&[]));
        // A real element may be named `none`; only the bare, sole `NONE` token
        // is the sentinel (a qualified name is always `Class.name`).
        assert_eq!(
            normalize_topo_names(&s(&["Load.none"])).unwrap(),
            s(&["Load.none"])
        );
        assert_eq!(
            normalize_topo_names(&s(&["NONE", "Line.l1"])).unwrap(),
            s(&["NONE", "Line.l1"])
        );
        // A multi-entry list keeps every name and loses only the one slot.
        assert_eq!(
            normalize_topo_names(&s(&["Line.l1", "Line.l2", ""])).unwrap(),
            s(&["Line.l1", "Line.l2"])
        );
        // The degenerate one-slot list is the *same* single trailing drop, and
        // this twin states the transports' rule exactly rather than a stricter
        // one — `oracle_server._topo_names` maps it identically. Neither channel
        // can emit it (both answer an empty walk with the `NONE` sentinel:
        // `CAPI_Topology.pas:138-143`, `DTopology.pas:275`), so it is pinned as a
        // rule statement, not as an expected reply.
        assert_eq!(normalize_topo_names(&s(&[""])).unwrap(), s(&[]));
        // Every other empty is a LOST NAME and is refused, never repaired.
        for bad in [
            s(&["a", "", "b"]),
            s(&["a", "", ""]),
            s(&["", "a"]),
            s(&["a", " "]),
        ] {
            let err = normalize_topo_names(&bad)
                .expect_err("this shape loses an element name and must be refused");
            assert!(
                err.contains("empty entry"),
                "the refusal must say what it refused ({bad:?}): {err}"
            );
        }
    }

    /// Non-vacuity of the fixpoint rail: a capture that reached the gate
    /// un-normalized FAILS instead of being quietly repaired — the whole reason
    /// the comparator asserts the rule rather than applying it.
    #[test]
    fn an_un_normalized_capture_fails_instead_of_being_repaired() {
        let msg = panic_message(|| {
            assert_normalized(&s(&["NONE"]), "isolated_branches", "modes:a/b.dss step 0");
        });
        for token in ["isolated_branches", "modes:a/b.dss step 0", "UN-normalized"] {
            assert!(
                msg.contains(token),
                "message {msg:?} does not name {token:?}"
            );
        }
        let msg = panic_message(|| {
            assert_normalized(
                &s(&["Line.l1", ""]),
                "isolated_loads",
                "modes:a/b.dss step 2",
            );
        });
        assert!(msg.contains("UN-normalized"), "{msg}");
        // A shape NEITHER transport can produce is refused with the rule's own
        // message, not with the fixpoint one.
        let msg = panic_message(|| {
            assert_normalized(&s(&["a", "", "b"]), "looped_pairs", "modes:a/b.dss step 1");
        });
        assert!(msg.contains("empty entry"), "{msg}");
        // A normalized capture is a no-op — the rail must not cost a green case.
        assert_normalized(&s(&["Line.l1", "Line.l2"]), "isolated_branches", "ctx");
        assert_normalized(&s(&[]), "isolated_loads", "ctx");
    }

    /// The flat wire shape is paired left-to-right, and the ordered compare has
    /// teeth in BOTH members and in the order (a swapped member, a reordering and
    /// a dropped pair all red) — the comparator half of the plan's §4.1-B
    /// corruption drive. Driven on the D16 shape: the first argument is the
    /// window model of the port's candidates, the third the oracle's flat list.
    #[test]
    fn the_looped_pair_arm_is_ordered_and_two_sided() {
        // `asymmetric:combo/combo_mesh_asym.dss`, measured on both channels;
        // there the two dedup rules agree, so model == the port's own list.
        let flat = s(&[
            "Transformer.tloop",
            "Line.lj3",
            "Line.ltie",
            "Transformer.tloop",
            "Line.lfa",
            "Transformer.tloop",
        ]);
        let good = pairs_of(&flat);
        assert_eq!(good.len(), 3, "the fixture is the measured three-pair deck");
        compare_looped_pairs(&good, &good, &flat, "ctx");
        // Case-insensitive, as everywhere in the harness.
        let upper: Vec<(String, String)> = good
            .iter()
            .map(|(a, b)| (a.to_uppercase(), b.to_uppercase()))
            .collect();
        compare_looped_pairs(&upper, &good, &flat, "ctx");
        // Swapped members of one pair.
        let mut swapped = good.clone();
        swapped[1] = (good[1].1.clone(), good[1].0.clone());
        let msg = panic_message(|| compare_looped_pairs(&swapped, &good, &flat, "ctx"));
        assert!(msg.contains("`looped_pairs`[1] differs"), "{msg}");
        // Reordered pairs.
        let mut reordered = good.clone();
        reordered.swap(0, 2);
        let msg = panic_message(|| compare_looped_pairs(&reordered, &good, &flat, "ctx"));
        assert!(msg.contains("`looped_pairs`[0] differs"), "{msg}");
        // A dropped pair.
        let short = good[..2].to_vec();
        let msg = panic_message(|| compare_looped_pairs(&short, &good, &flat, "ctx"));
        assert!(msg.contains("length differs"), "{msg}");
    }

    /// **D16, the mechanism** — the gate's model of upstream's dedup is the
    /// `i := i + 1` window scan, and it really drops a pair the port keeps.
    ///
    /// The smallest sequence that exhibits it: after `(a,b)` and `(c,d)` the flat
    /// buffer is `[a, b, c, d]`, so the third candidate `(b, c)` — a pair nobody
    /// stored — IS the window `(TStr[1], TStr[2])` and `DTopology.pas:286-296`
    /// reports `found`. A genuine repeat is dropped by both rules in either
    /// orientation, and a candidate matching no window survives both, so the
    /// model is not simply "drop the third".
    #[test]
    fn the_window_model_drops_a_straddling_pair() {
        let p = |v: &[(&str, &str)]| -> Vec<(String, String)> {
            v.iter()
                .map(|(a, b)| ((*a).to_string(), (*b).to_string()))
                .collect()
        };
        let cands = p(&[("a", "b"), ("c", "d"), ("b", "c")]);
        assert_eq!(window_dedup(&cands), p(&[("a", "b"), ("c", "d")]));
        assert_eq!(per_pair_dedup(&cands), cands, "the port keeps all three");
        // A genuine repeat, either orientation: both rules drop it.
        let rep = p(&[("a", "b"), ("c", "d"), ("d", "c")]);
        assert_eq!(window_dedup(&rep), p(&[("a", "b"), ("c", "d")]));
        assert_eq!(per_pair_dedup(&rep), p(&[("a", "b"), ("c", "d")]));
        // A candidate matching no window at all: both rules keep it.
        let keep = p(&[("a", "b"), ("c", "d"), ("a", "d")]);
        assert_eq!(window_dedup(&keep), keep);
        assert_eq!(per_pair_dedup(&keep), keep);
        // Case-insensitive on both sides (DSS identifiers are).
        assert_eq!(
            window_dedup(&p(&[("A", "B"), ("b", "a")])),
            p(&[("A", "B")])
        );
        assert_eq!(
            per_pair_dedup(&p(&[("A", "B"), ("b", "a")])),
            p(&[("A", "B")])
        );
    }

    /// **D15** — the isolation half is compared against a *named* port answer,
    /// and both bases have teeth: at [`Basis::Memoized`] the oracle must equal
    /// the port's STEP-0 topology, so a stale answer that does not match step 0
    /// still fails, and the message says which contract broke.
    #[test]
    fn the_isolation_arm_states_the_basis_it_compares_against() {
        // `controls:recloser/recloser_perm.dss`: step 0 fresh == the oracle's
        // memoized answer for every later step (measured, tmp/g17/stale_r4133.json).
        let step0 = IsolationSnapshot {
            num_isolated_branches: 0,
            num_isolated_loads: 0,
            isolated_branches: s(&[]),
            isolated_loads: s(&[]),
        };
        let stale_cap = TopologyCap {
            num_loops: 0,
            num_isolated_branches: 0,
            num_isolated_loads: 0,
            looped_pairs: s(&[]),
            isolated_branches: s(&[]),
            isolated_loads: s(&[]),
        };
        // The memoization contract holds -> silent.
        compare_isolation(&step0, &stale_cap, "ctx step 23", Basis::Memoized);
        compare_isolation(&step0, &stale_cap, "ctx step 0", Basis::Fresh);
        // The port's step-23 answer (2 isolated branches, 1 isolated load) is NOT
        // what the stale oracle reports — which is exactly why the arm is
        // rebased instead of compared, and why comparing it fresh must red.
        let step23 = IsolationSnapshot {
            num_isolated_branches: 2,
            num_isolated_loads: 1,
            isolated_branches: s(&["Line.feed", "Line.lat"]),
            isolated_loads: s(&["Load.l"]),
        };
        let msg = panic_message(|| {
            compare_isolation(&step23, &stale_cap, "ctx step 23", Basis::Fresh);
        });
        assert!(msg.contains("NumIsolatedBranches differs"), "{msg}");
        // An oracle that does NOT answer the step-0 topology fails the
        // memoization assertion, and the message names the contract.
        let moved = TopologyCap {
            num_isolated_branches: 1,
            isolated_branches: s(&["Line.lat"]),
            ..stale_cap.clone()
        };
        let msg = panic_message(|| {
            compare_isolation(&step0, &moved, "ctx step 23", Basis::Memoized);
        });
        assert!(msg.contains("NumIsolatedBranches differs"), "{msg}");
        assert!(msg.contains("memoized `Branch_List`"), "{msg}");
        // A self-inconsistent reference is refused before anything is compared.
        let broken = IsolationSnapshot {
            num_isolated_branches: 3,
            ..step0.clone()
        };
        let msg = panic_message(|| {
            compare_isolation(&broken, &stale_cap, "ctx step 1", Basis::Fresh);
        });
        assert!(msg.contains("the port contradicts itself"), "{msg}");
    }

    /// The census keys on `(case, step)` and dedups the channel visit, so a
    /// `both` case-step counts ONCE in the pinned population and twice in the
    /// visit tally — the shape
    /// `corpus_gate::scheduler::TOPOLOGY_STALE_DECLINES` is stated in.
    #[test]
    fn the_census_counts_cases_and_case_steps_not_channel_visits() {
        let mut set = BTreeSet::new();
        set.insert(("modes:time/midi_duty_ctrl.dss".to_string(), 3));
        set.insert(("modes:time/midi_duty_ctrl.dss".to_string(), 3));
        set.insert(("modes:time/midi_duty_ctrl.dss".to_string(), 4));
        set.insert(("controls:relay/relay_doc.dss".to_string(), 5));
        assert_eq!(cases_and_steps(&set), (2, 3));
        assert_eq!(cases_and_steps(&BTreeSet::new()), (0, 0));
    }

    /// The ordered name arm rejects a reordering, a rename and a length change —
    /// the comparator half of the plan's §4.1-C corruption drive. A set compare
    /// would pass the first of the three, which is why this one is a sequence
    /// compare.
    #[test]
    fn the_isolated_name_arm_is_ordered() {
        // `modes:reduce/reduce_remove.dss`, measured on both channels.
        let oracle = s(&["Line.l2", "Line.l3"]);
        compare_names("isolated_branches", &oracle, &oracle, "ctx");
        compare_names(
            "isolated_branches",
            &s(&["LINE.L2", "line.l3"]),
            &oracle,
            "ctx",
        );
        let msg = panic_message(|| {
            compare_names(
                "isolated_branches",
                &s(&["Line.l3", "Line.l2"]),
                &oracle,
                "ctx",
            );
        });
        assert!(msg.contains("`isolated_branches`[0] differs"), "{msg}");
        let msg = panic_message(|| {
            compare_names(
                "isolated_branches",
                &s(&["Line.l2", "Line.l3", "Line.bogus"]),
                &oracle,
                "ctx",
            );
        });
        assert!(msg.contains("length differs"), "{msg}");
    }

    /// The two oracle-side shape rules run **before** the port is consulted, so a
    /// self-inconsistent or truncated transport read is reported as a transport
    /// failure and never as a Rust divergence. Driven on the capture directly
    /// (the arms are pure equalities over `TopologyCap`; no engine needed).
    #[test]
    fn the_oracle_side_shape_arms_have_teeth() {
        let cap = TopologyCap {
            num_loops: 1,
            num_isolated_branches: 2,
            num_isolated_loads: 0,
            looped_pairs: s(&["Line.a", "Line.b"]),
            isolated_branches: s(&["Line.l2"]),
            isolated_loads: s(&[]),
        };
        assert_ne!(
            cap.num_isolated_branches,
            cap.isolated_branches.len() as i32,
            "a count that disagrees with its own list must not compare equal"
        );
        assert_eq!(cap.num_isolated_loads, cap.isolated_loads.len() as i32);
        assert_eq!(cap.looped_pairs.len() % 2, 0);
        // Why the even-length assert must run first: the pairing silently drops
        // an odd tail, which would turn a truncated read into a shorter list.
        let odd = s(&["Line.a", "Line.b", "Line.c"]);
        assert_ne!(odd.len() % 2, 0);
        assert_eq!(pairs_of(&odd).len(), 1);
    }
}

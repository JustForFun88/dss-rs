//! Hand-rolled scheduler for the unified corpus gate (UNIFIED_GATE_PLAN §3, D7a).
//!
//! One `#[test]` runs the UNION of all four manifests. The task list is grouped
//! by case directory (§3.3 simplification: task = case-dir group, its cases run
//! sequentially inside the task, so THIS test never schedules two cases of one
//! folder at once), pre-sorted longest-first by a static weight, and drained by
//! `available_parallelism()` (or `DSS_GATE_JOBS`) threads via an `AtomicUsize`
//! cursor + `std::thread::scope` — NOT rayon/tokio (tasks block on oracle
//! subprocess I/O; the global rayon pool belongs to faer inside the solves).
//!
//! That grouping is necessary but **not sufficient**, and since coordinator
//! decision **D33(2)** (G1.10a) a per-directory claim backs it: libtest runs
//! this `#[test]` concurrently with its siblings in the same binary, and
//! `corpus_ad_matches_normal_mode` compiles `ad_sweep.json`'s decks IN PLACE —
//! among them `8500-Node/Run_8500Node.dss`, `Run_8500Node_Unbal.dss` and
//! `Run_RecloserSiting.DSS`, whose own `Show`/`Export` lines run during
//! `compile` and land in a case directory this gate is measuring (G1.10a F4 run
//! 2 measured the consequence: one case's created-file SET carrying another
//! deck's reports, `tmp/g110a/f_F4_unexpected_run2.md`). The claim therefore
//! lives one level down, in [`crate::runner::CorpusGuard`] — exclusive per
//! canonical case directory, reentrant for the owning thread — so every
//! producer that guards a case dir takes it: this scheduler's cases (held
//! across both oracle captures, the port run and the sweep), the AD sweep, and
//! the opt-in report tests.
//!
//! The pinned `capi_v0145` channel is served by a persistent [`WorkerPool`]; the
//! `r4133` channel by a persistent [`EpriPool`] of `epri-worker` bridge processes
//! (Phase C/D — the pre-Phase-C one-shot target-rev `Oracle` shim is retired;
//! one-shots remain only for `isolate`/serial runs). Each case is
//! `catch_unwind`-wrapped; the test fails iff any case failed, printing the
//! COMPLETE failure list in manifest order.
//!
//! Contamination-proof knobs (§4 Phase B DONE bar):
//! * `DSS_GATE_SERIAL=1` — T=1 and the pinned channel uses a FRESH one-shot
//!   process per case (no persistent-worker state reuse): the reference run.
//! * `DSS_GATE_SHUFFLE=<seed>` — shuffle the task order (order-sensitivity probe).
//! * `DSS_GATE_DUMP=<path>` — write a label-sorted `{label: {verdict, result}}`
//!   artifact for a three-way bit-diff.
//! * `DSS_GATE_JOBS=<n>` — override the scheduler thread count.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::engines::{
    CaseResult, Channel, EpriOneShot, EpriPool, Oracle, WorkerPool, build_run_request,
};
use crate::harness::{self, CensusBlindSpots};
use crate::ledger::LedgerRuntime;
use crate::manifest::{
    EngineChannel, FAMILIES, SolvableCase, corpus_file, family_file, load_family, load_solvable,
};
use crate::props_census::{Mode as CensusMode, Row as CensusRow, RowKind as CensusRowKind};
use crate::runner::{
    CorpusGuard, assert_deferred_rust_smoke, assert_pending_errors_loudly, compare_with_result,
    panic_msg, run_and_compare_abort, run_rust_capture,
};

// ---------------------------------------------------------------------------
// Unified case model.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum CaseClass {
    Live,
    Abort,
    Pending,
    /// `defer_ledger` — parked from live oracle comparison (Phase D ledger seed);
    /// Rust-smoke only.
    Deferred,
}

struct UnifiedCase {
    order: usize,
    label: String,
    abs: String,
    dir_key: String,
    weight: u64,
    class: CaseClass,
    case: SolvableCase,
}

struct Task {
    dir_key: String,
    weight: u64,
    cases: Vec<UnifiedCase>,
}

/// Static longest-first weight: coarse solve-cost tier × step count.
fn kind_weight(kind: &str) -> u64 {
    if kind.starts_with("large") {
        1000
    } else if kind == "feeder" {
        10
    } else if kind == "midi" {
        3
    } else {
        1 // micro / synthetic
    }
}

/// Directory key for grouping (case-insensitive, forward-slashed parent).
fn dir_key_of(abs: &str) -> String {
    abs.rsplit_once('/')
        .map(|(d, _)| d)
        .unwrap_or("")
        .to_lowercase()
}

/// Apply the per-source property-forcing rule to a live case.
///
/// **Since R4133_PROPS RP4.1 (2026-09-03) the property compare runs on BOTH
/// channels.** It used to be a capi_v0145-channel feature — §1.2 pinned property
/// parity to the capi oracle while the r4133 bridge had no all-properties
/// capture — so it was forced only for cases gating capi, and [`run_one_case`]
/// plus [`seed_one`] masked it off per-channel on the r4133 request. Both
/// premises are gone: the bridge grew `capture_all_properties`
/// (`crates/dss-epri/src/capture.rs`) at the unified gate's Phase D, and RP4.1
/// removed the two masks, so an r4133-gating case now has its property table
/// compared exactly like a capi one. The request costs an extra per-case
/// `? name.Like` + `? name.prop` sweep on the r4133 worker.
///
/// The rule is therefore **every live case**, with the plan's one cost guard:
/// `kind=large*` decks stay out on the `solvable_now` arm. That guard is also
/// definitional — the RP0.1/RP0.2 census population every property measurement
/// in `R4133_PROPS_PLAN.md` rests on is exactly "live, non-`large`"
/// ([`run_props_census`]).
///
/// Written without a channel predicate on purpose: `gates_capi() ||
/// gates_r4133()` is a tautology over the three legal `engines` values
/// (`capi_v0145|both|r4133`), and a tautological guard reads like a filter while
/// filtering nothing (the RegControl no-load-zone precedent, CLAUDE.md).
fn force_properties(source: &str, c: &mut SolvableCase, fam_props: bool) {
    match source {
        "solvable_now" => {
            if !c.kind.starts_with("large") {
                c.compare_all_properties = true;
            }
        }
        _ => {
            // family: fam_props is the family-level flag (true for all three).
            c.compare_all_properties |= fam_props;
        }
    }
}

/// **The forced property population, pinned** — `(cases forced, of them
/// `engines: "both"`, `engines: "r4133"`, `engines: "capi_v0145"`)`.
///
/// Measured off the four manifests at RP4.1's audit settlement (2026-09-03) and
/// re-derived by [`the_property_forcing_rule_is_every_live_non_large_case`] on
/// every run: 526 cases → 522 live → **443** forced once the 79 live
/// `kind=large*` decks (all of them `solvable_now`) come off, of which **399**
/// gate the r4133 channel (312 `both` + 87 r4133-only) and 44 are capi-only.
///
/// Moved from `(440, 313, 83, 44)` by GOLDEN_REBASE G1.4a (2026-09-04,
/// coordinator decisions D12/D14): three `both` GICTransformer decks
/// (`asymmetric:gic/gic_midi.dss`, `asymmetric:gic/gictransformer_gic.dss`,
/// `solvable_now:…/GICExample/GIC_Example.dss`) became `r4133`-only because capi
/// 0.14.5 is nondeterministic on them, and the split-out
/// `modes:makeposseq/makeposseq_gic.dss` joined the corpus on the same channel.
/// GOLDEN_REBASE G1.6(i) then added `controls:energymeter/midi_relcalc.dss`
/// (`both`, `micro`) — the corpus's only `AllocateLoads` deck — so the two
/// lanes' cases met at the 2026-09-05 merges: 524 -> **526** cases, 520 -> 522
/// live, `(441, 310, 87, 44)` -> `(443, 312, 87, 44)` (`midi_relcalc` for
/// G1.6(i), `faultstudy_micro` for G1.5).
///
/// 443 is the census population every property measurement in
/// `R4133_PROPS_PLAN.md` rests on ([`run_props_census`] walks the same set), so
/// this lock also keeps the census and the gate talking about one population —
/// the FROZEN census extracts under `tests/corpus/props_r4133/` are a 2026-08-08
/// data lock and stay at the population of that day.
const FORCED_PROPS_POPULATION: (usize, usize, usize, usize) = (443, 312, 87, 44);

/// Apply the per-source **element-extras** forcing rule to a live case
/// (`GOLDEN_REBASE_PLAN.md` §1.1(e), G1.3d(i)).
///
/// Same shape and the same one cost guard as [`force_properties`] /
/// [`force_derived`]: **every live case, minus `kind=large*` on the
/// `solvable_now` arm.** Both channels honour the request
/// (`engines::build_run_request`'s `element_extras` key), so there is no channel
/// predicate here either — writing one would be the same tautology
/// [`force_properties`] documents.
///
/// **No opt-in list, deliberately.** [`force_derived`] carries
/// [`DERIVED_MANIFEST_OPT_INS`] because the fastdss reference harness *skips*
/// `Residuals` on those two decks, so gating them is where our comparator is
/// strictly stronger than the harness we are reaching parity with. That same
/// table — `.inputs/DSS-Python` `origin/fastdss`
/// `tests/compare_outputs.py:32-88` `KNOWN_COM_DIFF` — has **no** row for
/// `NodeOrder`, `EnergyMeter` or the three counts, so there is no deck an
/// opt-in would buy anything on: a const here would be dead weight that reads
/// like a decision.
///
/// Kept as a **separate** function with a separate lock
/// ([`FORCED_ELEMENT_EXTRAS_POPULATION`]) rather than folded into
/// [`force_derived`], for the reason that one is kept separate from
/// [`force_properties`]: the two rules answer to different sub-steps
/// (G1.3d(ii) widens this flag, G1.3b/c widen that one), and a later divergence
/// must show as its own lock's diff.
fn force_element_extras(source: &str, c: &mut SolvableCase) {
    match source {
        "solvable_now" => {
            if !c.kind.starts_with("large") {
                c.compare_element_extras = true;
            }
        }
        _ => c.compare_element_extras = true,
    }
}

/// **The forced element-extras population, pinned** — `(cases with
/// `compare_element_extras` on, of them `engines: "both"`, `engines: "r4133"`,
/// `engines: "capi_v0145"`)`.
///
/// Measured off the four manifests at G1.3d(i) (2026-09-04, re-derived at the
/// merge into `update` 2026-09-05 after G1.4a's D12/D14 corpus flips) and by
/// [`the_element_extras_forcing_rule_is_every_live_non_large_case`] on every
/// run: exactly [`FORCED_PROPS_POPULATION`]'s live non-`large` population,
/// because the rule is the same one and there are no opt-ins
/// ([`force_element_extras`]). It is nevertheless its own const: the two rules
/// are free to diverge, and a divergence must land as a reviewed diff here.
///
/// It exists for the reason [`FORCED_DERIVED_POPULATION`] does — the
/// population lock fingerprints the **manifest** flag (`Case::rigor`'s `elemx=`
/// token in `population_lock.rs`), not the effective one, so a re-mask of
/// [`force_element_extras`] would stop comparing the five discrete channels on
/// every one of these cases without moving one byte of `population.lock.json`.
const FORCED_ELEMENT_EXTRAS_POPULATION: (usize, usize, usize, usize) = (443, 312, 87, 44);

/// **The property-forcing rule is a rule, not a habit** — the static half of
/// RP4.1's re-mask alarm (audit settlement, 2026-09-03).
///
/// [`force_properties`] is what the whole r4133 property compare hangs on, and
/// nothing else can see it: `population.lock.json` fingerprints manifest flags
/// and per-case ledger tags, not scheduler code, and the live guard
/// `props_norm::assert_r4133_props_compare_ran` is a boolean — a *partial*
/// re-mask (re-adding `gates_capi()`, which would drop the 87 r4133-only cases
/// while the 312 `both` ones keep walking) passes it. This test is the one that
/// does not: it walks the four manifests without an oracle and asserts the
/// forced set **is** the live non-`large` population, cell for cell, with the
/// per-`engines` split pinned by [`FORCED_PROPS_POPULATION`].
///
/// It also guards the family arm's missing cost guard: the arm ORs the
/// family-level flag with no `kind=large*` test, which is inert only while no
/// family case is `large`. That premise is asserted here rather than assumed,
/// so a family deck that ever grows into `large` is a review, not a silent
/// +1000-weight property sweep.
#[test]
fn the_property_forcing_rule_is_every_live_non_large_case() {
    let cases = build_unified_cases();
    let mut forced = (0usize, 0usize, 0usize, 0usize);
    let mut wrong: Vec<String> = Vec::new();
    for uc in &cases {
        let live = uc.class == CaseClass::Live;
        let large = uc.case.kind.starts_with("large");
        let family = uc.label.split(':').next() != Some("solvable_now");
        assert!(
            !(family && large),
            "{}: a family deck is `kind={}` — `force_properties`' family arm carries no `large` \
             cost guard, so this case would be property-forced without review. Add the guard, or \
             re-classify the deck.",
            uc.label,
            uc.case.kind
        );
        // The rule, per source: every live case, minus `large` on `solvable_now`.
        // A manifest may also set the flag itself, which only ever ADDS.
        let expected = live && !large;
        if uc.case.compare_all_properties != expected {
            wrong.push(format!(
                "{}: kind={} engines={} class={} → compare_all_properties={} (expected {expected})",
                uc.label,
                uc.case.kind,
                uc.case.engines,
                if live { "live" } else { "not-live" },
                uc.case.compare_all_properties,
            ));
        }
        if uc.case.compare_all_properties {
            forced.0 += 1;
            match uc.case.engines.as_str() {
                "both" => forced.1 += 1,
                "r4133" => forced.2 += 1,
                _ => forced.3 += 1,
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "the property-forcing rule is `every live non-`large` case` since R4133_PROPS RP4.1 \
         (2026-09-03) — these cases disagree with it:\n  {}",
        wrong.join("\n  ")
    );
    assert_eq!(
        forced, FORCED_PROPS_POPULATION,
        "(forced, both, r4133-only, capi-only) moved. A DROP in the r4133 halves is a re-mask of \
         the property request — the thing `props_norm::assert_r4133_props_compare_ran` cannot \
         see, because the `both` half keeps it green. A legitimate corpus change moves this lock \
         together with `population.lock.json`."
    );
}

/// Apply the per-source **PDElements**-forcing rule to a live case
/// (`GOLDEN_REBASE_PLAN.md` §G1.6b).
///
/// The rule is [`force_properties`]': **every live case**, with the same one
/// cost guard — `kind=large*` decks stay out on the `solvable_now` arm. It is
/// deliberately the same population, so the two whole-model surfaces the gate
/// forces (properties and PDElements) never drift into talking about different
/// case sets; [`FORCED_PDELEMENTS_POPULATION`] re-derives it and pins the split.
///
/// Two shape notes, both deliberate:
///
/// * no channel predicate — `gates_capi() || gates_r4133()` is a tautology over
///   the three legal `engines` values, and a tautological guard reads like a
///   filter while filtering nothing (the RegControl no-load-zone precedent,
///   CLAUDE.md). Both transports capture the walk
///   (`tools/oracle/oracle_server.py::capture_pd_elements`,
///   `crates/dss-epri/src/capture.rs::capture_pd_elements`), and D2 requires
///   both channels in the same commit;
/// * no family-flag parameter — [`force_properties`] takes `fam_props` because
///   `Family` carries a per-family `compare_all_properties`; there is no such
///   knob for this surface and inventing one would be a false switch (all three
///   families would set it). The family arm therefore forces unconditionally,
///   exactly what `fam_props` evaluates to for all three families today, and
///   the missing `large` guard on that arm is asserted inert by
///   [`the_pdelements_forcing_rule_is_every_live_non_large_case`] rather than
///   assumed.
///
/// Cost: measured ≈ 5 µs per PD element on the capi channel (1 283 elements in
/// 5.9 ms), i.e. well under a second added across the whole population against
/// a ~155 s corpus gate.
fn force_pdelements(source: &str, c: &mut SolvableCase) {
    match source {
        "solvable_now" => {
            if !c.kind.starts_with("large") {
                c.compare_pdelements = true;
            }
        }
        // family: the surface applies to every live family deck.
        _ => c.compare_pdelements = true,
    }
}

/// **The forced PDElements population, pinned** — `(cases forced, of them
/// `engines: "both"`, `engines: "r4133"`, `engines: "capi_v0145"`)`.
///
/// Identical to [`FORCED_PROPS_POPULATION`] by construction (both rules are
/// "every live case, minus `kind=large*` on `solvable_now`"), and re-derived —
/// never copied — by [`the_pdelements_forcing_rule_is_every_live_non_large_case`]
/// on every run. It is written out rather than aliased so that a future
/// divergence between the two rules shows up as a lock diff on the surface that
/// moved, instead of silently following the other one.
const FORCED_PDELEMENTS_POPULATION: (usize, usize, usize, usize) = (443, 312, 87, 44);

/// Apply the per-source **derived-channel** forcing rule to a live case
/// (`GOLDEN_REBASE_PLAN.md` §1.1(e), G1.3a).
///
/// Same shape and the same one cost guard as [`force_properties`]: **every live
/// case, minus `kind=large*` on the `solvable_now` arm.** Both channels honor
/// the request (`engines::build_run_request`'s `derived` key), so there is no
/// channel predicate here either — writing one would be the same tautology
/// [`force_properties`] documents.
///
/// Kept as a **separate** function and a separate lock
/// ([`FORCED_DERIVED_POPULATION`]) rather than folded into the property rule
/// even though the two forcing rules coincide today (the populations already
/// differ by the two opt-ins): they answer to different plans (the RP4.1
/// census vs WP-G1's surface roll-out), and a later divergence of the rules
/// — G1.3b/c widen this flag's surface, and a heavy deck could earn an
/// opt-out from one and not the other — must show as its own lock's diff.
///
/// The family arm carries no `large` test, exactly like [`force_properties`]';
/// it is inert while no family deck is `kind=large*`, which
/// [`the_property_forcing_rule_is_every_live_non_large_case`] asserts over this
/// very population rather than assuming.
///
/// A manifest may also set `compare_derived` itself, which only ever ADDS — see
/// [`DERIVED_MANIFEST_OPT_INS`].
fn force_derived(source: &str, c: &mut SolvableCase) {
    match source {
        "solvable_now" => {
            if !c.kind.starts_with("large") {
                c.compare_derived = true;
            }
        }
        _ => c.compare_derived = true,
    }
}

/// The cases whose **manifest** switches `compare_derived` on, on top of
/// [`force_derived`]'s rule — pinned here so an opt-in is a reviewed line of
/// Rust, not a JSON key nobody re-reads.
///
/// Both are `kind=large_near_ideal_source` (hence outside the cost guard) but
/// carry only 8 and 5 elements, so the guard's cost rationale does not apply to
/// them; and they are exactly the two decks the fastdss reference harness
/// **skips** `Residuals` on — `.inputs/DSS-Python` `origin/fastdss`
/// `tests/compare_outputs.py:56-59` ("Close enough for the system"). Gating them
/// is where our comparator is strictly stronger than the harness we are reaching
/// parity with (`GOLDEN_REBASE_PLAN.md` §1.1), so the opt-in is the point, not a
/// convenience.
const DERIVED_MANIFEST_OPT_INS: &[&str] = &[
    "solvable_now:Test/AutoTrans/Auto3bus.dss",
    "solvable_now:Test/AutoTrans/AutoHLT.dss",
];

/// **The forced derived-channel population, pinned** — `(cases with
/// `compare_derived` on, of them `engines: "both"`, `engines: "r4133"`,
/// `engines: "capi_v0145"`)`.
///
/// Measured off the four manifests at G1.3a (2026-09-04), moved from
/// `(442, 315, 83, 44)` by G1.4a's D12/D14 channel flip and from
/// `(443, 312, 87, 44)` by G1.6(i)'s new deck at the 2026-09-05 lane merge (see
/// [`FORCED_PROPS_POPULATION`]), and re-derived by
/// [`the_derived_forcing_rule_is_every_live_non_large_case_plus_the_opt_ins`] on
/// every run: [`FORCED_PROPS_POPULATION`]'s 443 live non-`large` cases plus the
/// two [`DERIVED_MANIFEST_OPT_INS`] decks (both `engines: "both"`).
///
/// The population lock fingerprints the **manifest** flag, not the effective
/// one ([`Case::rigor`]'s `derived=` token in `population_lock.rs`), so a
/// re-mask of [`force_derived`] would move 443 cases without moving one byte of
/// `population.lock.json`. This const is the only thing that sees it — the
/// reason [`FORCED_PROPS_POPULATION`] exists, applied to the second rule.
const FORCED_DERIVED_POPULATION: (usize, usize, usize, usize) = (445, 314, 87, 44);

/// **The PDElements-forcing rule is a rule, not a habit** — the static half of
/// G1.6b's re-mask alarm, modeled on
/// [`the_property_forcing_rule_is_every_live_non_large_case`] and load-bearing
/// for the same reason: `population.lock.json` fingerprints the *manifest*
/// `compare_pdelements` flag (`population_lock.rs::rigor`'s `pde=` token), and
/// no manifest sets it — the whole population is scheduler-forced, so the lock
/// cannot see a re-mask here at all.
///
/// `harness::assert_pd_elements_compare_ran` is the live half, and it is a
/// boolean: re-adding a channel predicate (say `gates_capi()`, which drops the
/// 87 `engines: "r4133"` cases while the 312 `both` ones keep walking) passes
/// it. This test does not — it walks the four manifests without an oracle and
/// asserts the forced set **is** the live non-`large` population, cell for
/// cell, with the per-`engines` split pinned by
/// [`FORCED_PDELEMENTS_POPULATION`].
#[test]
fn the_pdelements_forcing_rule_is_every_live_non_large_case() {
    let cases = build_unified_cases();
    let mut forced = (0usize, 0usize, 0usize, 0usize);
    let mut wrong: Vec<String> = Vec::new();
    for uc in &cases {
        let live = uc.class == CaseClass::Live;
        let large = uc.case.kind.starts_with("large");
        let family = uc.label.split(':').next() != Some("solvable_now");
        assert!(
            !(family && large),
            "{}: a family deck is `kind={}` — `force_pdelements`' family arm carries no `large` \
             cost guard, so this case would be PDElements-forced without review. Add the guard, \
             or re-classify the deck.",
            uc.label,
            uc.case.kind
        );
        // The rule, per source: every live case, minus `large` on
        // `solvable_now`. A manifest may also set the flag itself, which only
        // ever ADDS (none does today — the `props=0`-everywhere shape).
        let expected = live && !large;
        if uc.case.compare_pdelements != expected {
            wrong.push(format!(
                "{}: kind={} engines={} class={} → compare_pdelements={} (expected {expected})",
                uc.label,
                uc.case.kind,
                uc.case.engines,
                if live { "live" } else { "not-live" },
                uc.case.compare_pdelements,
            ));
        }
        if uc.case.compare_pdelements {
            forced.0 += 1;
            match uc.case.engines.as_str() {
                "both" => forced.1 += 1,
                "r4133" => forced.2 += 1,
                _ => forced.3 += 1,
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "the PDElements-forcing rule is `every live non-`large` case` since GOLDEN_REBASE G1.6b \
         (2026-09-04) — these cases disagree with it:\n  {}",
        wrong.join("\n  ")
    );
    assert_eq!(
        forced, FORCED_PDELEMENTS_POPULATION,
        "(forced, both, r4133-only, capi-only) moved. A DROP in the r4133 halves is a re-mask of \
         the PDElements request — the thing `harness::assert_pd_elements_compare_ran` cannot \
         see once the `both` half keeps it green. A legitimate corpus change moves this lock \
         together with `population.lock.json`."
    );
    // The two forced surfaces must stay one population (see
    // `FORCED_PDELEMENTS_POPULATION`): a divergence here means one of the two
    // rules moved without the other, which is a review, not a silent split.
    assert_eq!(
        forced, FORCED_PROPS_POPULATION,
        "the PDElements and property forcing rules have drifted apart"
    );
}

/// **The derived-channel forcing rule is a rule, not a habit** (G1.3a) — the
/// twin of [`the_property_forcing_rule_is_every_live_non_large_case`], for the
/// same reason: `population.lock.json` fingerprints the *manifest* flag, so a
/// re-mask of [`force_derived`] (a channel predicate, a widened `large` guard, a
/// dropped source arm) would silently stop comparing
/// `CurrentsMagAng`/`VoltagesMagAng`/`Residuals` on hundreds of cases while
/// every lock, every ledger digest and the two opt-in decks stayed green.
///
/// It walks the four manifests without an oracle and asserts the flagged set
/// **is** `live non-large` ∪ [`DERIVED_MANIFEST_OPT_INS`], case by case, with
/// the per-`engines` split pinned by [`FORCED_DERIVED_POPULATION`]. Each opt-in
/// is additionally required to be a live case the rule would *not* have covered
/// — otherwise the entry is dead weight that reads like a decision.
#[test]
fn the_derived_forcing_rule_is_every_live_non_large_case_plus_the_opt_ins() {
    let cases = build_unified_cases();
    let mut flagged = (0usize, 0usize, 0usize, 0usize);
    let mut wrong: Vec<String> = Vec::new();
    let mut opt_in_seen: Vec<&str> = Vec::new();
    for uc in &cases {
        let live = uc.class == CaseClass::Live;
        let large = uc.case.kind.starts_with("large");
        let opt_in = DERIVED_MANIFEST_OPT_INS.contains(&uc.label.as_str());
        if opt_in {
            opt_in_seen.push(uc.label.as_str());
            assert!(
                live && large,
                "{}: listed in `DERIVED_MANIFEST_OPT_INS`, but it is {} and kind={} — the \
                 forcing rule already covers it, so the manifest key is dead weight. Drop the \
                 key and the entry.",
                uc.label,
                if live { "live" } else { "not-live" },
                uc.case.kind
            );
        }
        // The rule, per source: every live case, minus `large` on `solvable_now`
        // — plus the reviewed manifest opt-ins, which only ever ADD.
        let expected = live && (!large || opt_in);
        if uc.case.compare_derived != expected {
            wrong.push(format!(
                "{}: kind={} engines={} class={} → compare_derived={} (expected {expected})",
                uc.label,
                uc.case.kind,
                uc.case.engines,
                if live { "live" } else { "not-live" },
                uc.case.compare_derived,
            ));
        }
        if uc.case.compare_derived {
            flagged.0 += 1;
            match uc.case.engines.as_str() {
                "both" => flagged.1 += 1,
                "r4133" => flagged.2 += 1,
                _ => flagged.3 += 1,
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "the derived-channel forcing rule is `every live non-`large` case, plus \
         DERIVED_MANIFEST_OPT_INS` (GOLDEN_REBASE_PLAN.md G1.3a) — these cases disagree with \
         it:\n  {}",
        wrong.join("\n  ")
    );
    assert_eq!(
        opt_in_seen, DERIVED_MANIFEST_OPT_INS,
        "an opt-in label matched no case (a renamed or dropped deck): the manifest key it names \
         is gone, so the entry masks nothing and must be pruned"
    );
    assert_eq!(
        flagged, FORCED_DERIVED_POPULATION,
        "(flagged, both, r4133-only, capi-only) moved. A DROP means the derived channels stopped \
         being compared on that many cases — invisible to `population.lock.json`, which records \
         the manifest flag and not `force_derived`'s effect. A legitimate corpus change moves \
         this lock together with the lock file."
    );
}

/// Force the **bus voltage surface** on (`GOLDEN_REBASE_PLAN.md` G1.4a, §1.1(d)).
///
/// The manifests set `compare_bus` on no case (526 × `bus=0` in
/// `population.lock.json`), exactly as they set `compare_all_properties` on
/// none: a surface that is only compared where a manifest opts in is a surface
/// nobody compares. The rule is therefore the same one
/// [`force_properties`] runs — **every live case whose `kind` does not start
/// with `large`** — written once here, for every source: unlike the property
/// arm there is no family-level bus flag to OR in, and the `large` cost guard
/// is not source-specific (that no family deck is `large` is asserted by
/// [`the_property_forcing_rule_is_every_live_non_large_case`], so the two rules
/// select the same 443 cases and the two surfaces talk about one population).
///
/// The surface is the fastdss `ActiveBus` facade (`origin/fastdss`
/// `tests/save_outputs.py:351` over `dss/IBus.py:19-53` `_columns`); the arms
/// wired at G1.4a are the ones capi 0.14.5 and r4133 run identically —
/// `puVoltages` (`CAPI/CAPI_Alt.pas:2251-2280` == `DDLL/DBus.pas:399-430`),
/// `VMagAngle` (`:2573-2597` == `:659-689`), `puVmagAngle` (`:2540-2571` ==
/// `:690-723`), `Nodes`/`kVBase` (`:2143-2163` == `:319-345`) and the
/// circuit-level `AllBusVmagPu` (`CAPI_Circuit.pas:521-548` ==
/// `DDLL/DCircuit.pas:481-500`).
///
/// Cost, measured in the sub-step's micro-parts F2/F3: ~8.4 µs per bus on the
/// capi transport and ~7–8 µs on the r4133 one, over the ~99.7 k bus-steps this
/// rule selects ⇒ well under a second per channel per gate run.
fn force_bus(c: &mut SolvableCase) {
    if !c.kind.starts_with("large") {
        c.compare_bus = true;
    }
}

/// **The forced bus population, pinned** — `(cases forced, of them
/// `engines: "both"`, `engines: "r4133"`, `engines: "capi_v0145"`)`, the same
/// shape (and, by construction, the same numbers) as
/// [`FORCED_PROPS_POPULATION`].
///
/// Its own lock, not an alias of the property one: `population.lock.json`
/// fingerprints the **manifest** flag (`bus=0` on all 526 cases), so nothing
/// else in the tree can see this rule at all. G1.0 requires a force rule to
/// ship with its own population pin (`TESTING.md` §"adding a live surface"), and
/// a silent narrowing here — a channel predicate, a second `kind` prefix — would
/// otherwise leave the gate green on a smaller corpus.
const FORCED_BUS_POPULATION: (usize, usize, usize, usize) = (443, 312, 87, 44);

/// **The bus-forcing rule is a rule, not a habit** — the static twin of
/// [`the_property_forcing_rule_is_every_live_non_large_case`] for G1.4a.
///
/// Walks the four manifests without an oracle and asserts that the forced set
/// **is** the live non-`large` population, case for case, with the per-`engines`
/// split pinned by [`FORCED_BUS_POPULATION`]. `capture_guard::require_capture`
/// inside the runner proves the flag never greens on an empty capture; this
/// test proves the flag is actually ON where it should be, which no capture
/// guard can see.
#[test]
fn the_bus_forcing_rule_is_every_live_non_large_case() {
    let cases = build_unified_cases();
    let mut forced = (0usize, 0usize, 0usize, 0usize);
    let mut wrong: Vec<String> = Vec::new();
    for uc in &cases {
        let live = uc.class == CaseClass::Live;
        let expected = live && !uc.case.kind.starts_with("large");
        if uc.case.compare_bus != expected {
            wrong.push(format!(
                "{}: kind={} engines={} class={} → compare_bus={} (expected {expected})",
                uc.label,
                uc.case.kind,
                uc.case.engines,
                if live { "live" } else { "not-live" },
                uc.case.compare_bus,
            ));
        }
        if uc.case.compare_bus {
            forced.0 += 1;
            match uc.case.engines.as_str() {
                "both" => forced.1 += 1,
                "r4133" => forced.2 += 1,
                _ => forced.3 += 1,
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "the bus-forcing rule is `every live non-`large` case` (GOLDEN_REBASE G1.4a) — these          cases disagree with it:
  {}",
        wrong.join("
  ")
    );
    assert_eq!(
        forced, FORCED_BUS_POPULATION,
        "(forced, both, r4133-only, capi-only) moved. A DROP is a narrowing of the bus surface          that nothing else can see: `population.lock.json` fingerprints the MANIFEST flag, which          is `bus=0` on every case. A legitimate corpus change moves this lock together with          `population.lock.json` and `FORCED_PROPS_POPULATION`."
    );
}

/// Force the **short-circuit surface** on (`GOLDEN_REBASE_PLAN.md` G1.5,
/// §1.1(d)).
///
/// The same predicate as [`force_bus`] — **every live case whose `kind` does
/// not start with `large`** — and deliberately so: the six short-circuit arms
/// are appended to the ONE per-bus walk `compare_bus` drives on both
/// transports (`oracle_server.py::capture_all_buses`,
/// `dss-epri::capture::capture_all_buses`), so a case that compares the bus
/// surface can compare this one for the cost of six more reads, and a case
/// that does not cannot compare it at all. That implication is asserted, never
/// or-ed in behind the manifest's back: in `engines::build_run_request` at
/// runtime and in [`the_zsc_forcing_rule_is_every_live_non_large_case`]
/// statically.
///
/// The arms, with their Pascal pairs — capi `src/CAPI/CAPI_Alt.pas` ==
/// r4133 `Version8/Source/DDLL/DBus.pas`: `Zsc1` (`:2294-2303` == `:461-474`),
/// `Zsc0` (`:2283-2292` == `:476-489`), `ZscMatrix` (`:2305-2334` ==
/// `:431-459`), `YscMatrix` (`:2336-2365` == `:491-518`), `Isc`
/// (`:2202-2224` == `:374-397`) and `Voc` (`:2227-2249` == `:351-372`). Only
/// four vendored decks plus the G1.5 micro deck run a fault study, but `Voc`
/// is live on every `PreserveNodeVoltages` deck (`Common/Ymatrix.pas:170`
/// refreshes `VBus` from `BuildYMatrix`) and the not-run SENTINEL shape is
/// itself gated everywhere else — which is what makes the wide population
/// worth its cost.
///
/// Cost, measured end-to-end (read + JSON + transport) in the sub-step's
/// micro-part F3 on the matrices-present worst case (`IEEE123Master-SC`,
/// 133 buses x 10 steps): **28.8 µs per bus-step** on the capi transport and
/// **2.14 µs** on the r4133 one, i.e. ≲ 3 s per channel per gate run over the
/// ~99.7 k bus-steps this rule selects — and much less in practice, since a
/// bus with no matrix ships one or two doubles.
fn force_zsc(c: &mut SolvableCase) {
    if !c.kind.starts_with("large") {
        c.compare_zsc = true;
    }
}

/// **The forced short-circuit population, pinned** — `(cases forced, of them
/// `engines: "both"`, `engines: "r4133"`, `engines: "capi_v0145"`)`, the same
/// shape as [`FORCED_BUS_POPULATION`] and, because the two rules share a
/// predicate, the same numbers. Its own lock all the same:
/// `population.lock.json` fingerprints the **manifest** flag (`zsc=0` on every
/// case but the G1.5 micro deck), so nothing else in the tree can see this
/// rule, and a silent narrowing here — a channel predicate, a second `kind`
/// prefix — would leave the gate green on a smaller corpus.
const FORCED_ZSC_POPULATION: (usize, usize, usize, usize) = (443, 312, 87, 44);

/// **The short-circuit forcing rule is a rule, not a habit** — the static twin
/// of [`the_bus_forcing_rule_is_every_live_non_large_case`] for G1.5, plus the
/// `compare_zsc ⇒ compare_bus` implication the shared per-bus walk rests on
/// (checked on the FORCED case, i.e. after both rules have run, so a manifest
/// that sets `compare_zsc` on a case the bus rule skips fails here rather than
/// in the middle of a gate run).
#[test]
fn the_zsc_forcing_rule_is_every_live_non_large_case() {
    let cases = build_unified_cases();
    let mut forced = (0usize, 0usize, 0usize, 0usize);
    let mut wrong: Vec<String> = Vec::new();
    for uc in &cases {
        let live = uc.class == CaseClass::Live;
        let expected = live && !uc.case.kind.starts_with("large");
        if uc.case.compare_zsc != expected {
            wrong.push(format!(
                "{}: kind={} engines={} class={} → compare_zsc={} (expected {expected})",
                uc.label,
                uc.case.kind,
                uc.case.engines,
                if live { "live" } else { "not-live" },
                uc.case.compare_zsc,
            ));
        }
        if uc.case.compare_zsc && !uc.case.compare_bus {
            wrong.push(format!(
                "{}: compare_zsc without compare_bus — the six short-circuit arms ride the                  bus walk (GOLDEN_REBASE_PLAN.md G1.5 §2.a)",
                uc.label,
            ));
        }
        if uc.case.compare_zsc {
            forced.0 += 1;
            match uc.case.engines.as_str() {
                "both" => forced.1 += 1,
                "r4133" => forced.2 += 1,
                _ => forced.3 += 1,
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "the short-circuit forcing rule is `every live non-`large` case` (GOLDEN_REBASE          G1.5) — these cases disagree with it:
  {}",
        wrong.join("
  ")
    );
    assert_eq!(
        forced, FORCED_ZSC_POPULATION,
        "(forced, both, r4133-only, capi-only) moved. A DROP is a narrowing of the          short-circuit surface that nothing else can see: `population.lock.json`          fingerprints the MANIFEST flag. A legitimate corpus change moves this lock          together with `population.lock.json`, `FORCED_BUS_POPULATION` and          `FORCED_PROPS_POPULATION`."
    );
}

/// **The element-extras forcing rule is a rule, not a habit** (G1.3d(i)) — the
/// twin of
/// [`the_derived_forcing_rule_is_every_live_non_large_case_plus_the_opt_ins`],
/// for the same reason: `population.lock.json` fingerprints the *manifest*
/// flag, so a re-mask of [`force_element_extras`] (a channel predicate, a
/// widened `large` guard, a dropped source arm) would silently stop comparing
/// `NumTerminals`/`NumConductors`/`NumPhases`/`EnergyMeter`/`NodeOrder` on
/// hundreds of cases while every lock and every ledger digest stayed green.
///
/// It walks the four manifests without an oracle and asserts the flagged set
/// **is** the live non-`large` population, case by case, with the per-`engines`
/// split pinned by [`FORCED_ELEMENT_EXTRAS_POPULATION`]. The absence of an
/// opt-in table is asserted too, in the only way that survives a rename: the
/// expected set carries no exception, so a manifest key switching the flag on
/// for a `large` deck reds here instead of quietly widening the population
/// ([`force_element_extras`] documents why there is nothing to opt in).
///
/// The model here (`live && !large`, for **every** source) is one notch stricter
/// than [`force_element_extras`], whose `large` exception sits on the
/// `solvable_now` arm alone — the family arm ORs the flag on with no `kind`
/// test. The two agree only while no family deck is `large`, so that premise is
/// asserted rather than assumed (the
/// [`the_property_forcing_rule_is_every_live_non_large_case`] precedent, G1.3d(i)
/// audit settlement 2026-09-05): a family deck growing into `large` is a review,
/// not a silently forced extras sweep and not a confusing red here.
#[test]
fn the_element_extras_forcing_rule_is_every_live_non_large_case() {
    let cases = build_unified_cases();
    let mut flagged = (0usize, 0usize, 0usize, 0usize);
    let mut wrong: Vec<String> = Vec::new();
    for uc in &cases {
        let live = uc.class == CaseClass::Live;
        let large = uc.case.kind.starts_with("large");
        let family = uc.label.split(':').next() != Some("solvable_now");
        assert!(
            !(family && large),
            "{}: a family deck is `kind={}` — `force_element_extras`' family arm carries no \
             `large` cost guard, so this case would be extras-forced without review. Add the \
             guard, or re-classify the deck.",
            uc.label,
            uc.case.kind
        );
        // The rule, per source: every live case, minus `large` on `solvable_now`
        // (equivalently `live && !large` while the assert above holds).
        let expected = live && !large;
        if uc.case.compare_element_extras != expected {
            wrong.push(format!(
                "{}: kind={} engines={} class={} → compare_element_extras={} \
                 (expected {expected})",
                uc.label,
                uc.case.kind,
                uc.case.engines,
                if live { "live" } else { "not-live" },
                uc.case.compare_element_extras,
            ));
        }
        if uc.case.compare_element_extras {
            flagged.0 += 1;
            match uc.case.engines.as_str() {
                "both" => flagged.1 += 1,
                "r4133" => flagged.2 += 1,
                _ => flagged.3 += 1,
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "the element-extras forcing rule is `every live non-`large` case` \
         (GOLDEN_REBASE_PLAN.md G1.3d(i)) — these cases disagree with it:\n  {}",
        wrong.join("\n  ")
    );
    assert_eq!(
        flagged, FORCED_ELEMENT_EXTRAS_POPULATION,
        "(flagged, both, r4133-only, capi-only) moved. A DROP means the ten discrete extras \
         (the three counts, `NodeOrder`, `EnergyMeter` and the five control-derived scalars) \
         AND `PhaseLosses` stopped being compared on that many cases — invisible to \
         `population.lock.json`, which records the manifest flag and not `force_element_extras`' \
         effect. A legitimate corpus change moves this lock together with the lock file."
    );
}

// ---------------------------------------------------------------------------
// GOLDEN_REBASE G1.7 — the topology surface: its forced population and the two
// decline populations (coordinator decisions D15/D16)
// ---------------------------------------------------------------------------

/// Apply the G1.7 topology-forcing rule to a live case.
///
/// The surface is six of the nine `ITopology` fields the fastdss harness dumps
/// (`origin/fastdss` `dss/ITopology.py:10-20` `_columns`, reached from
/// `tests/save_outputs.py:372`): `NumLoops`, `NumIsolatedBranches`,
/// `NumIsolatedLoads`, `AllLoopedPairs`, `AllIsolatedBranches`,
/// `AllIsolatedLoads` — r4133 `Version8/Source/DDLL/DTopology.pas:65-94` (the
/// three counts) and `:270-390` (the three lists), capi 0.14.5
/// `src/CAPI/CAPI_Topology.pas:81-97`/`:113-215`/`:369-405`. The other three
/// (`ActiveLevel`, `BranchName`, `ActiveBranch`) are deliberately not compared:
/// reading them reassigns `ActiveCircuit.ActiveCktElement`
/// (`DTopology.pas:29-54`, `:96-160`, `:170-186`) and would poison the
/// per-element capture — the justified parity gap TESTING.md records.
///
/// The rule is **every live case whose `kind` does not start with `large`** —
/// deliberately the same population [`force_properties`] forces, so the gate
/// reasons about ONE forced set instead of two ([`FORCED_TOPOLOGY_POPULATION`]
/// asserts the two stay equal). It costs six extra reads per checkpoint per
/// channel; both oracles memoize the branch tree (r4133
/// `Common/Circuit.pas:2932-2950`) so the read is nearly free there, while the
/// port rebuilds it on every call (it never caches — CLAUDE.md), and that is
/// what the plan's `kind=large*` cost guard buys here: the 8500-Node class
/// carries hundreds of loops over dozens of steps, in both lanes.
///
/// Unlike [`force_properties`] there is no per-source arm: no family deck is
/// `kind=large*` (asserted by
/// [`the_property_forcing_rule_is_every_live_non_large_case`]), so one rule
/// covers all four manifests. A manifest may also declare the flag itself,
/// which only ever ADDS — and must, for the surface to reach the anti-shrink
/// lock at all ([`TOPOLOGY_DECLARED_IN_MANIFEST`]).
fn force_topology(c: &mut SolvableCase) {
    if !c.kind.starts_with("large") {
        c.compare_topology = true;
    }
}

/// **The forced topology population, pinned** — `(cases forced, of them
/// `engines: "both"`, `engines: "r4133"`, `engines: "capi_v0145"`)`.
///
/// Re-derived from the four manifests by
/// [`the_topology_forcing_rule_is_every_live_non_large_case`] on every run:
/// 526 cases → 522 live → **443** forced once the 79 live `kind=large*` decks
/// come off, of which 312 are `both`, 87 r4133-only and 44 capi-only (re-measured
/// 2026-09-05 on the merged tree, after D12/D14 moved four `GICTransformer`
/// decks onto `r4133` and added `modes:makeposseq/makeposseq_gic.dss`, and after
/// G1.6(i) added `controls:energymeter/midi_relcalc.dss`). Identical to
/// [`FORCED_PROPS_POPULATION`] by construction, and
/// the test asserts that equality instead of leaving it a comment: the two
/// rules are one population by design, so a drift between them is a review.
///
/// The lock this constant backs up is `population.lock.json`, which records the
/// **manifest** flag (`population_lock.rs::rigor`'s `topo=` token) and therefore
/// cannot see the scheduler-side forcing at all — the same blind spot RP4.1's
/// property lock has, and the reason both rules carry a re-derivation test. It
/// also fixes the two G1.7 decline populations: [`TOPOLOGY_STALE_DECLINES`] and
/// [`LOOPED_PAIR_WINDOW_DECLINES`] were measured over exactly this set, so
/// narrowing it moves them too.
const FORCED_TOPOLOGY_POPULATION: (usize, usize, usize, usize) = (443, 312, 87, 44);

/// **The manifest rows that declare `compare_topology` themselves**, with the
/// gating channel each one carries.
///
/// The forcing rule above is invisible to `population.lock.json`, so without at
/// least one declared row the surface would arm the whole gate while leaving the
/// anti-shrink lock byte-identical — and switching it off again would leave no
/// diff either. These seven decks are the §1.1(f) acceptance witnesses: every
/// gating channel is represented (`capi_v0145`, `r4133`, `both`), and each row's
/// measured step-0 answer is non-trivial in at least one of the six quantities,
/// so the declaration is a statement about real values and not about zeros
/// (`tmp/g17` probes, 2026-09-04; the numbers are quoted in the G1.7 record):
///
/// * `13Bus/IEEE13Nodeckt.dss` — 3 looped pairs → `NumLoops = 1` (the halving).
/// * `Microgrid/ISource/Master.DSS` — all three counts non-zero (1 / 3 / 4).
/// * `HarmonicsTMode/IEEE_519.DSS` — the capi-only channel, 0 / 2 / 2.
/// * `controls:fuse/indmach_r4133/indmach_dyn.dss` — the r4133-only channel,
///   1 / 7 / 4.
/// * `controls:recloser/recloser_perm.dss` — the D15 pin deck (24 steps, the
///   memoization witness).
/// * `asymmetric:combo/combo_mesh_asym.dss` — the deliberately meshed deck,
///   `NumLoops = 2`.
/// * `modes:reduce/reduce_breakloop.dss` — `NumIsolatedBranches = 1`
///   (`Line.l1`), also the capi trailing-empty witness.
const TOPOLOGY_DECLARED_IN_MANIFEST: &[(&str, &str)] = &[
    (
        "solvable_now:Version8/Distrib/Examples/HarmonicsTMode/IEEE_519.DSS",
        "capi_v0145",
    ),
    (
        "solvable_now:Version8/Distrib/Examples/Microgrid/ISource/Master.DSS",
        "both",
    ),
    (
        "solvable_now:Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss",
        "both",
    ),
    ("asymmetric:combo/combo_mesh_asym.dss", "both"),
    ("controls:fuse/indmach_r4133/indmach_dyn.dss", "r4133"),
    ("controls:recloser/recloser_perm.dss", "r4133"),
    ("modes:reduce/reduce_breakloop.dss", "both"),
];

/// **The topology-forcing rule is a rule, not a habit** — the G1.7 twin of
/// [`the_property_forcing_rule_is_every_live_non_large_case`], and for the same
/// reason: nothing else can see [`force_topology`]. `population.lock.json`
/// fingerprints manifest flags, and the live census
/// [`assert_topology_declines_are_the_pinned_population`] only asks whether the
/// surface ran at all, so a *partial* re-mask (say a `gates_capi()` guard, which
/// would drop the 83 r4133-only cases while the 313 `both` ones keep walking)
/// passes both. This test walks the four manifests without an oracle and asserts
/// the forced set **is** the live non-`large` population, cell for cell.
#[test]
fn the_topology_forcing_rule_is_every_live_non_large_case() {
    let cases = build_unified_cases();
    let mut forced = (0usize, 0usize, 0usize, 0usize);
    let mut wrong: Vec<String> = Vec::new();
    for uc in &cases {
        let live = uc.class == CaseClass::Live;
        let large = uc.case.kind.starts_with("large");
        // The rule: every live case, minus `large`. A manifest may also declare
        // the flag itself, which only ever ADDS — so a declared row outside the
        // rule (a `large` deck, or a pending/abort/deferred one) is a review,
        // not a silent widening, and reds here.
        let expected = live && !large;
        if uc.case.compare_topology != expected {
            wrong.push(format!(
                "{}: kind={} engines={} class={} → compare_topology={} (expected {expected})",
                uc.label,
                uc.case.kind,
                uc.case.engines,
                if live { "live" } else { "not-live" },
                uc.case.compare_topology,
            ));
        }
        if uc.case.compare_topology {
            forced.0 += 1;
            match uc.case.engines.as_str() {
                "both" => forced.1 += 1,
                "r4133" => forced.2 += 1,
                _ => forced.3 += 1,
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "the topology-forcing rule is `every live non-`large` case` (GOLDEN_REBASE G1.7, \
         2026-09-05) — these cases disagree with it:\n  {}",
        wrong.join("\n  ")
    );
    assert_eq!(
        forced, FORCED_TOPOLOGY_POPULATION,
        "(forced, both, r4133-only, capi-only) moved. A DROP in either r4133 half is a \
         per-channel re-mask of the topology request — the thing the live census cannot see, \
         because it only asks whether the surface ran at all. It also re-measures \
         `TOPOLOGY_STALE_DECLINES` / `LOOPED_PAIR_WINDOW_DECLINES`, which were derived over \
         exactly this population. A legitimate corpus change moves this lock together with \
         `population.lock.json`."
    );
    assert_eq!(
        FORCED_TOPOLOGY_POPULATION, FORCED_PROPS_POPULATION,
        "G1.7 forces the topology surface over the SAME population as the property surface (one \
         rule, one set to reason about — `force_topology`'s doc). If that is deliberately no \
         longer true, narrow one rule, drop this assertion, and re-measure the two topology \
         decline constants, which are a function of this population."
    );
}

/// **The surface reaches the anti-shrink lock** — the static half of the G1.7
/// acceptance (`GOLDEN_REBASE_PLAN.md` §1.1(f): the flag is set on at least one
/// case per gating channel).
///
/// [`force_topology`] arms the gate but is scheduler code; only a *declared*
/// manifest row reaches `population_lock.rs::rigor`'s `topo=` token, so this
/// test pins exactly which rows carry the declaration and on which channel.
/// Deleting one — the cheapest way to shrink the surface's recorded footprint —
/// fails here and in `population.lock.json`, never silently.
#[test]
fn the_topology_surface_is_declared_on_every_gating_channel() {
    let mut declared: Vec<(String, String)> = Vec::new();
    for c in load_solvable() {
        if c.compare_topology {
            declared.push((format!("solvable_now:{}", c.path), c.engines.clone()));
        }
    }
    for fam in FAMILIES {
        for c in load_family(fam.name) {
            if c.compare_topology {
                declared.push((format!("{}:{}", fam.name, c.path), c.engines.clone()));
            }
        }
    }
    declared.sort();
    let mut want: Vec<(String, String)> = TOPOLOGY_DECLARED_IN_MANIFEST
        .iter()
        .map(|(l, e)| ((*l).to_string(), (*e).to_string()))
        .collect();
    want.sort();
    assert_eq!(
        declared, want,
        "the manifest declarations of `compare_topology` moved. They are what puts the surface \
         into `population.lock.json` (the `topo=` rigor token) — the scheduler-side \
         `force_topology` is invisible to it — so this set is pinned, and a change to it belongs \
         in the same commit as a regenerated lock."
    );
    for ch in ["capi_v0145", "r4133", "both"] {
        assert!(
            declared.iter().any(|(_, e)| e == ch),
            "no manifest case declares `compare_topology` with engines={ch:?}; \
             GOLDEN_REBASE_PLAN.md §1.1(f) wants the flag set on at least one case per gating \
             channel, so that each channel's capture path is exercised by a declared row and not \
             only by the scheduler's forcing rule"
        );
    }
}

/// **D15 — the population whose isolation half is answered from upstream's
/// stale tree**, as `(cases, case-steps)`.
///
/// Both oracles memoize `Branch_List` (r4133 `Common/Circuit.pas:2932-2950`,
/// freed only in `Destroy` `:703` and `DoResetMeterZones` `:2308`; capi
/// identical) and invalidate nothing on a conductor open/close, so once a Relay,
/// Recloser or SwtControl has operated they answer `NumIsolated*` / `Isolated*`
/// from the tree built at step 0. The port never caches (CLAUDE.md: an upstream
/// defect is never reproduced), so at those steps
/// `harness::topology::compare_topology` rebases the isolation half onto the
/// port's OWN step-0 answer and asserts the memoization contract instead of
/// comparing against the fresh one — a positive assertion, not an exclusion, and
/// 0 `ledger.json` rows.
///
/// Measured 2026-09-04 (`tmp/g17/STOP.md`, one fresh compile per step over all
/// 105 multi-step r4133-gating live cases plus the 10 capi-gating multi-step
/// cases whose conductor state changes): 15 `controls/*` cases on the r4133
/// channel plus `modes/time/midi_duty_ctrl.dss`, which is `engines: "both"` and
/// declines identically on both channels — the pair is counted ONCE here (the
/// channel visits behind it are reported separately by the census).
const TOPOLOGY_STALE_DECLINES: (usize, usize) = (16, 135);

/// **D16 — the population where upstream's looped-pair dedup measurably drops a
/// pair the port keeps**, as `(cases, case-steps)`.
///
/// Upstream scans its flat name buffer in overlapping windows (`i := i + 1` over
/// `(buf[i-1], buf[i])`, r4133 `DDLL/DTopology.pas:286-296`, capi
/// `CAPI_Topology.pas:180-190`) while its own comment says "see if we already
/// found this pair", so a genuinely new candidate that equals a straddling
/// window is dropped. The port keeps the correct per-pair dedup and the
/// comparator asserts `oracle.looped_pairs ==
/// harness::topology::window_dedup(port candidates)`; this constant counts where
/// that model and the port's own list part company — compared by content, not
/// by length, since the two rules keep different buffers — i.e. where the
/// defect actually bites.
///
/// Measured 2026-09-04 over the forced topology population (every live
/// non-`large` case): six multi-step `controls/midi_*` decks — `midi_protection`
/// (24 steps), `midi_recloser_perm` (24), `midi_recloser_temp` (16),
/// `midi_swtcontrol` (12), `midi_relay_4647` (10), `midi_fuse` (8) — plus the
/// single-step `epri_dpv/M1/Master_NoPV.dss` and
/// `ADiakoptics/ckt24/Torn_Circuit/zone_2/master.dss`. It is a function of that
/// population: the LVTestCase and ckt24 feeders diverge the same way but are
/// `kind=large*` and therefore out of the compare (the same live non-`large`
/// rule [`force_properties`] applies).
const LOOPED_PAIR_WINDOW_DECLINES: (usize, usize) = (8, 96);

/// **Both decline populations are re-derived on every full gate run and pinned
/// in BOTH directions** — the fail-on-stale rule coordinator decisions D15 and
/// D16 attach to their 0-ledger-row settlements.
///
/// A population that GREW means the port (or upstream) started declining
/// somewhere new and nobody looked; one that SHRANK means a decline stopped
/// happening, so the pin behind it is a statement about nothing. Either way the
/// gate must fail rather than absorb it — the ledger's own fail-on-stale
/// discipline (`ledger::assert_all_hit`) applied to a settlement that
/// deliberately writes no ledger rows.
///
/// Silent in two documented situations, and in no others:
///
/// * `DSS_GATE_ONLY` is set — a filtered run holds a filtered population, so its
///   census cannot be the pinned one (the mandatory gate never sets it).
/// * No manifest case requests `compare_topology` at all. That is a **fact about
///   the manifests**, re-read here rather than assumed: while G1.7's surface flag
///   is unwired the census is legitimately empty, and the moment the flag is
///   forced or set the assertion arms itself — including the `compared > 0`
///   non-vacuity check, which is what catches a later re-mask of the request.
pub(crate) fn assert_topology_declines_are_the_pinned_population() {
    if std::env::var("DSS_GATE_ONLY").is_ok() {
        return;
    }
    let requested = build_unified_cases()
        .iter()
        .filter(|uc| uc.class == CaseClass::Live)
        .any(|uc| uc.case.compare_topology);
    let census = harness::topology::decline_census();
    if !requested {
        assert_eq!(
            (census.compared, census.stale, census.window),
            (0, (0, 0), (0, 0)),
            "no manifest case requests `compare_topology`, yet the topology \
             comparator ran: {census:?}"
        );
        eprintln!(
            "corpus_gate topology: the surface is not requested by any live case \
             (GOLDEN_REBASE G1.7 F6 wires the flag) — nothing to re-derive"
        );
        return;
    }
    eprintln!(
        "corpus_gate topology: {} compared (case, step, channel) triple(s); \
         D15 stale declines {:?} ({} channel visit(s)), D16 window declines {:?} \
         ({} channel visit(s))\n  {}",
        census.compared,
        census.stale,
        census.stale_visits,
        census.window,
        census.window_visits,
        harness::topology::decline_report()
    );
    assert!(
        census.compared > 0,
        "the topology compare never ran: a live case requests \
         `compare_topology` but no (case, step, channel) triple reached \
         `harness::topology::compare_topology`. The request is masked off \
         somewhere between the manifest and the transports \
         (`build_run_request`, the capture sites) — the two populations below \
         would then be trivially 0 and their pins would say nothing."
    );
    assert_eq!(
        census.stale,
        TOPOLOGY_STALE_DECLINES,
        "the D15 stale-tree decline population moved (measured {:?}, pinned {:?}). \
         It is re-derived on every run and fails in BOTH directions: a bigger \
         population means a case-step started answering from a stale tree with \
         nobody looking, a smaller one means the settlement now covers less than \
         it claims. Re-measure, move the constant WITH the record, and re-read \
         the pin `topology_pins::topology_reads_a_freshly_built_tree`.\n{}",
        census.stale,
        TOPOLOGY_STALE_DECLINES,
        harness::topology::decline_report()
    );
    assert_eq!(
        census.window,
        LOOPED_PAIR_WINDOW_DECLINES,
        "the D16 window-scan decline population moved (measured {:?}, pinned \
         {:?}) — same rule, both directions. A shrink is the interesting one: it \
         would mean upstream's `i := i + 1` scan stopped dropping straddling \
         pairs, which is the defect the whole settlement is built on \
         (`topology_pins::looped_pairs_lose_the_straddling_window`).\n{}",
        census.window,
        LOOPED_PAIR_WINDOW_DECLINES,
        harness::topology::decline_report()
    );
}

// ---------------------------------------------------------------------------
// GOLDEN_REBASE G1.8 — the incidence-matrix surface: its forced population and
// the manifest rows that carry the declaration into the anti-shrink lock
// ---------------------------------------------------------------------------

/// Apply the G1.8 incidence-forcing rule to a live case.
///
/// The surface is the four flat incidence quantities the fastdss harness dumps
/// for `Solution` — `IncMatrix`, `Laplacian`, `IncMatrixCols`, `IncMatrixRows`
/// (`origin/fastdss` `tests/save_outputs.py:187`, built by the `CalcIncMatrix` /
/// `CalcLaplacian` pair the same harness issues at `:117-118`). They are read on
/// r4133 from `Version8/Source/DDLL/DSolution.pas:542-568` (`IncMatrix`),
/// `:589-608` (rows), `:609-639` (cols) and `:640-666` (`Laplacian`); on capi
/// 0.14.5 from `src/CAPI/CAPI_Solution.pas:897-926`, `:953-971`, `:979-1021` and
/// `:860-889` — the two integer getters allocating one cell too many (`:910`,
/// `:873`, each carrying upstream's own `//TODO: remove the +1`), which the capi
/// capture drops after asserting it is zero.
///
/// Two neighbours of that group are deliberately never requested on either
/// channel. `BusLevels` (`DSolution.pas:569-588`) does
/// `setlength(myIntArray, ArrSize)` and then `for IMIdx := 0 to ArrSize` — one
/// write past the array — and therefore sits on the bridge's do-not-call
/// register (`crates/dss-epri/src/modes.rs:271-280`). The ORDERED builder
/// `Calc_Inc_Matrix_Org` (`Common/Solution.pas:3146`) calls `GetTopology`
/// (`:3173`), which rebuilds and re-memoizes the branch tree G1.7's two decline
/// censuses are defined on; the flat `Calc_Inc_Matrix` (`:3046`) is what this
/// surface drives, and the golden `*_org_*` stems keep covering the other one.
///
/// The rule is **every live case whose `kind` does not start with `large`** —
/// deliberately the same population [`force_properties`] and [`force_topology`]
/// force, so the gate keeps reasoning about ONE forced set
/// ([`FORCED_INC_MATRIX_POPULATION`] asserts all three stay equal). The cost
/// guard is what buys the surface: the live `large*` decks are a small slice of
/// the corpus but over half of the capi oracle's wall time and payload, and
/// nothing caches the matrix — the pair rebuilds it from scratch on every call,
/// on both oracles and on the port.
///
/// As with [`force_topology`] there is no per-source arm (no family deck is
/// `kind=large*`, asserted by
/// [`the_property_forcing_rule_is_every_live_non_large_case`]), and a manifest
/// may also declare the flag itself — which only ever ADDS, and is the only way
/// the surface reaches `population.lock.json` at all
/// ([`INC_MATRIX_DECLARED_IN_MANIFEST`]).
fn force_inc_matrix(c: &mut SolvableCase) {
    if !c.kind.starts_with("large") {
        c.compare_inc_matrix = true;
    }
}

/// **The forced incidence population, pinned** — `(cases forced, of them
/// `engines: "both"`, `engines: "r4133"`, `engines: "capi_v0145"`)`.
///
/// Re-derived from the four manifests by
/// [`the_inc_matrix_forcing_rule_is_every_live_non_large_case`] on every run:
/// 526 cases → 522 live → **443** forced once the live `kind=large*` decks come
/// off, of which 312 are `both`, 87 r4133-only and 44 capi-only (measured
/// 2026-09-05 on the merged tree; `(440, 313, 83, 44)` on the lane, before G1.4a's
/// D12/D14 flips and the three micro decks the other lanes added). Identical to
/// [`FORCED_TOPOLOGY_POPULATION`] and
/// [`FORCED_PROPS_POPULATION`] by construction, and the test asserts those
/// equalities instead of leaving them a comment.
///
/// The lock it backs up is `population.lock.json`, which records the **manifest**
/// flag (`population_lock.rs::rigor`'s `incm=` token) and cannot see the
/// scheduler-side forcing at all — the same blind spot the property and topology
/// rules carry, and the reason all three have a re-derivation test. It also
/// fixes settlement S-INC's decline population: `harness::inc_matrix`'s
/// `INC_UPSTREAM_ROW_DECLINES` was measured over exactly this set, so narrowing
/// it moves that constant too.
const FORCED_INC_MATRIX_POPULATION: (usize, usize, usize, usize) = (443, 312, 87, 44);

/// **The manifest rows that declare `compare_inc_matrix` themselves**, with the
/// gating channel each one carries.
///
/// The forcing rule above is invisible to `population.lock.json`, so without at
/// least one declared row the surface would arm the whole gate while leaving the
/// anti-shrink lock byte-identical — and switching it off again would leave no
/// diff either. These six decks are the §1.1(f) acceptance witnesses: every
/// gating channel is represented (`capi_v0145`, `r4133`, `both`), and each row's
/// measured step-0 answer is non-trivial in at least one of the four quantities
/// (`tmp/g18` probes, 2026-09-05; both channels agree exactly after the capture
/// normalizations, so one set of numbers describes both):
///
/// * `asymmetric:capacitor/midi_capacitor_asym.dss` — `both`; all four row
///   builders (lines, transformers, series capacitors, series reactors) fire in
///   one deck: 39 rows / 36 cols, 79 incidence and 112 Laplacian triples.
/// * `asymmetric:reactor/reactor_asym.dss` — `both`; the settlement S-INC
///   witness: 6 rows / 5 cols, 12 triples, upstream's largest row index **6**
///   against **6** row names, because its shunt `Reactor.rsh` still advances the
///   row cursor (r4133 `Common/Solution.pas:3039`).
/// * `solvable_now:Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss` —
///   `both`, 24 steps of 17 rows / 16 cols, 34 incidence and 46 Laplacian
///   triples; the deck every other WP-G1 surface is also declared on.
/// * `solvable_now:Version8/Distrib/Examples/HarmonicsTMode/IEEE_519.DSS` — the
///   `capi_v0145`-only channel: 7 rows / 8 cols, 14 and 22 triples.
/// * `controls:fuse/indmach_r4133/indmach_dyn.dss` — the `r4133`-only channel:
///   2 steps of 13 rows / 12 cols, 26 and 34 triples.
/// * `solvable_now:Test/CableParameters.dss` — `both`, and the only declared
///   deck with an **empty** `IncMat`: the raw payload is r4133's bare `[0]`
///   sentinel (`DSolution.pas:545-546`) / capi's lone unwritten `+1` cell, and
///   `IncMatrixRows` arrives as `['']` on capi (`CAPI_Solution.pas:959`) and
///   `['None']` on r4133 (`DSolution.pas:605`). The sentinel path is therefore
///   declared, not merely forced.
const INC_MATRIX_DECLARED_IN_MANIFEST: &[(&str, &str)] = &[
    ("solvable_now:Test/CableParameters.dss", "both"),
    (
        "solvable_now:Version8/Distrib/Examples/HarmonicsTMode/IEEE_519.DSS",
        "capi_v0145",
    ),
    (
        "solvable_now:Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss",
        "both",
    ),
    ("asymmetric:capacitor/midi_capacitor_asym.dss", "both"),
    ("asymmetric:reactor/reactor_asym.dss", "both"),
    ("controls:fuse/indmach_r4133/indmach_dyn.dss", "r4133"),
];

/// Which gating channels the manifests ask the incidence surface for — the
/// arming half of `harness::inc_matrix`'s decline census, read HERE because that
/// module compiles into ~20 test binaries with no manifest access (the G1.7
/// predicate in [`assert_topology_declines_are_the_pinned_population`], moved
/// across the module boundary by the G1.8 audit settlement).
pub(crate) fn inc_matrix_requested_channels() -> harness::inc_matrix::RequestedChannels {
    let mut out = harness::inc_matrix::RequestedChannels {
        capi: false,
        r4133: false,
    };
    for uc in build_unified_cases()
        .iter()
        .filter(|uc| uc.class == CaseClass::Live && uc.case.compare_inc_matrix)
    {
        out.capi |= uc.case.engines == "both" || uc.case.engines == "capi_v0145";
        out.r4133 |= uc.case.engines == "both" || uc.case.engines == "r4133";
    }
    out
}

/// **The incidence-forcing rule is a rule, not a habit** — the G1.8 twin of
/// [`the_topology_forcing_rule_is_every_live_non_large_case`], and for the same
/// reason: nothing else can see [`force_inc_matrix`]. `population.lock.json`
/// fingerprints manifest flags only, and the live census
/// `harness::inc_matrix::assert_declines_are_the_pinned_population` sees the
/// SHAPE of the re-mask (nothing compared, or nothing compared on one channel)
/// rather than its size — a rule narrowed from 440 cases to 400 leaves both
/// green. This test walks the four manifests without an oracle and asserts the
/// forced set **is** the live non-`large` population, cell for cell.
#[test]
fn the_inc_matrix_forcing_rule_is_every_live_non_large_case() {
    let cases = build_unified_cases();
    let mut forced = (0usize, 0usize, 0usize, 0usize);
    let mut wrong: Vec<String> = Vec::new();
    for uc in &cases {
        let live = uc.class == CaseClass::Live;
        let large = uc.case.kind.starts_with("large");
        // The rule: every live case, minus `large`. A manifest may also declare
        // the flag itself, which only ever ADDS — so a declared row outside the
        // rule (a `large` deck, or a pending/abort/deferred one) is a review,
        // not a silent widening, and reds here.
        let expected = live && !large;
        if uc.case.compare_inc_matrix != expected {
            wrong.push(format!(
                "{}: kind={} engines={} class={} → compare_inc_matrix={} (expected {expected})",
                uc.label,
                uc.case.kind,
                uc.case.engines,
                if live { "live" } else { "not-live" },
                uc.case.compare_inc_matrix,
            ));
        }
        if uc.case.compare_inc_matrix {
            forced.0 += 1;
            match uc.case.engines.as_str() {
                "both" => forced.1 += 1,
                "r4133" => forced.2 += 1,
                _ => forced.3 += 1,
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "the incidence-forcing rule is `every live non-`large` case` (GOLDEN_REBASE G1.8, \
         2026-09-05) — these cases disagree with it:\n  {}",
        wrong.join("\n  ")
    );
    assert_eq!(
        forced, FORCED_INC_MATRIX_POPULATION,
        "(forced, both, r4133-only, capi-only) moved. A DROP in either r4133 half is a \
         per-channel re-mask of the incidence request — the thing the live census cannot see, \
         because it only counts the comparisons that did run. It also re-measures \
         `harness::inc_matrix::INC_UPSTREAM_ROW_DECLINES`, which was derived over exactly this \
         population. A legitimate corpus change moves this lock together with \
         `population.lock.json`."
    );
    assert_eq!(
        FORCED_INC_MATRIX_POPULATION, FORCED_TOPOLOGY_POPULATION,
        "G1.8 forces the incidence surface over the SAME population as the topology surface (one \
         rule, one set to reason about — `force_inc_matrix`'s doc). If that is deliberately no \
         longer true, narrow one rule, drop this assertion, and re-measure \
         `INC_UPSTREAM_ROW_DECLINES`, which is a function of this population."
    );
    assert_eq!(
        FORCED_INC_MATRIX_POPULATION, FORCED_PROPS_POPULATION,
        "…and the same population as the property surface, which is where the `live non-`large`` \
         rule came from (RP0.1/RP0.2's census). Three rules, one forced set."
    );
}

/// **The surface reaches the anti-shrink lock** — the static half of the G1.8
/// acceptance (`GOLDEN_REBASE_PLAN.md` §1.1(f): the flag is set on at least one
/// case per gating channel).
///
/// [`force_inc_matrix`] arms the gate but is scheduler code; only a *declared*
/// manifest row reaches `population_lock.rs::rigor`'s `incm=` token, so this
/// test pins exactly which rows carry the declaration and on which channel.
/// Deleting one — the cheapest way to shrink the surface's recorded footprint —
/// fails here and in `population.lock.json`, never silently.
#[test]
fn the_inc_matrix_surface_is_declared_on_every_gating_channel() {
    let mut declared: Vec<(String, String)> = Vec::new();
    for c in load_solvable() {
        if c.compare_inc_matrix {
            declared.push((format!("solvable_now:{}", c.path), c.engines.clone()));
        }
    }
    for fam in FAMILIES {
        for c in load_family(fam.name) {
            if c.compare_inc_matrix {
                declared.push((format!("{}:{}", fam.name, c.path), c.engines.clone()));
            }
        }
    }
    declared.sort();
    let mut want: Vec<(String, String)> = INC_MATRIX_DECLARED_IN_MANIFEST
        .iter()
        .map(|(l, e)| ((*l).to_string(), (*e).to_string()))
        .collect();
    want.sort();
    assert_eq!(
        declared, want,
        "the manifest declarations of `compare_inc_matrix` moved. They are what puts the surface \
         into `population.lock.json` (the `incm=` rigor token) — the scheduler-side \
         `force_inc_matrix` is invisible to it — so this set is pinned, and a change to it \
         belongs in the same commit as a regenerated lock."
    );
    for ch in ["capi_v0145", "r4133", "both"] {
        assert!(
            declared.iter().any(|(_, e)| e == ch),
            "no manifest case declares `compare_inc_matrix` with engines={ch:?}; \
             GOLDEN_REBASE_PLAN.md §1.1(f) wants the flag set on at least one case per gating \
             channel, so that each channel's capture path is exercised by a declared row and not \
             only by the scheduler's forcing rule"
        );
    }
}

/// **G1.10a - the run's created FILE SET.** Force `compare_run_files` on every
/// live case whose `kind` does not start with `large`.
///
/// The surface is the set of filesystem entries the run
/// (`clear -> compile -> post -> n_steps x solve`) creates under the case
/// directory - the case dir's own entries plus everything under a directory the
/// run created - as `/`-joined, ASCII-case-folded relative names, a trailing `/`
/// marking a created directory (`harness::run_files`, over the classification
/// `dss_epri::guard::CorpusGuard::classify` and its Python twin
/// `tools/oracle/corpus_guard.py`). It is well defined because `Compile` points
/// the output directory at the deck's own folder (r4133
/// `Version8/Source/Common/DSSGlobals.pas:962`, capi 0.14.5 twin
/// `src/Common/DSSGlobals.pas:563`) and every executive writer prefixes its name
/// with it (r4133 `Version8/Source/Executive/ExportOptions.pas:401`,
/// `Executive/ShowOptions.pas:208-211`).
///
/// Upstream never gated it: fastdss archives every `*.csv` under the run dir
/// (`origin/fastdss` `tests/save_outputs.py:597-609`) and its comparison SKIPS a
/// name the other side does not have (`tests/compare_outputs.py:412-421`), so a
/// file one engine writes and the other does not is invisible there. New
/// coverage, not catch-up.
///
/// The rule is **every live case whose `kind` does not start with `large`** -
/// deliberately the same population [`force_properties`], [`force_topology`] and
/// [`force_inc_matrix`] force, so the gate keeps reasoning about ONE forced set
/// ([`FORCED_RUN_FILES_POPULATION`] asserts all four stay equal). The cost is a
/// recursive listing of the case dir: free on the oracle side (both guards
/// already walk that tree to snapshot and to sweep - the classification is now
/// *reported* instead of discarded) and one walk before + one after on the port
/// side, which is why the `large*` decks, whose directories hold the corpus's
/// biggest file trees, stay out with the rest of the one forced set.
///
/// As with [`force_topology`] there is no per-source arm (no family deck is
/// `kind=large*`, asserted by
/// [`the_property_forcing_rule_is_every_live_non_large_case`]), and a manifest
/// may also declare the flag itself - which only ever ADDS, and is the only way
/// the surface reaches `population.lock.json` at all
/// ([`RUN_FILES_DECLARED_IN_MANIFEST`]).
fn force_run_files(c: &mut SolvableCase) {
    if !c.kind.starts_with("large") {
        c.compare_run_files = true;
    }
}

/// **The forced run-file population, pinned** - `(cases forced, of them
/// `engines: "both"`, `engines: "r4133"`, `engines: "capi_v0145"`)`, counted over
/// the cases the RULE reaches (the one declared `large` witness is counted
/// separately by the test below).
///
/// Re-derived from the four manifests by
/// [`the_run_files_forcing_rule_is_every_live_non_large_case`] on every run:
/// 526 cases -> 522 live -> **443** forced once the live `kind=large*` decks come
/// off, of which 312 are `both`, 87 r4133-only and 44 capi-only (measured
/// 2026-09-05 on lane `lane-s`, synced with `update` at `44c1d51f`). Identical to
/// [`FORCED_INC_MATRIX_POPULATION`], [`FORCED_TOPOLOGY_POPULATION`] and
/// [`FORCED_PROPS_POPULATION`] by construction, and the test asserts those
/// equalities instead of leaving them a comment.
///
/// The lock it backs up is `population.lock.json`, which records the **manifest**
/// flag (`population_lock.rs::rigor`'s `runf=` token) and cannot see the
/// scheduler-side forcing at all - the same blind spot the other three rules
/// carry, and the reason all four have a re-derivation test. The D25/Q2
/// engine-scratch decline census (`harness::run_files::scratch_census`) is
/// measured over exactly this set, so narrowing the rule re-measures it too.
const FORCED_RUN_FILES_POPULATION: (usize, usize, usize, usize) = (443, 312, 87, 44);

/// **The manifest rows that declare `compare_run_files` themselves**, with the
/// gating channel each one carries.
///
/// The forcing rule above is invisible to `population.lock.json`, so without at
/// least one declared row the surface would arm the whole gate while leaving the
/// anti-shrink lock byte-identical - and switching it off again would leave no
/// diff either. These six decks are the section 1.1(f) acceptance witnesses:
/// every gating channel is represented (`capi_v0145`, `r4133`, `both`), and each
/// row's created set was measured on its own channel(s) before it was declared
/// (`tmp/g110a` F0/F1/F2 probes, 2026-09-05; the two oracles' raw spellings
/// differ and fold onto one member, so one set of names describes both):
///
/// * `solvable_now:Test/REACTORTest.DSS` - `both`; 8 files from `Show`/`Dump`
///   over the deck's TWO circuits (`reactortest*` + `reactortest2*`), the
///   plainest non-monitor writer set in the corpus.
/// * `solvable_now:Version8/Distrib/IEEETestCases/NEVTestCase/Run_NEV.dss` -
///   `both`; 7 created names of which **1** is the engines' internal harmonics
///   scratch `nev_savedvoltages.dbl` (r4133 `Common/Utilities.pas:1512-1521`,
///   written only by `InitializeForHarmonics`), split off symmetrically by
///   D25/Q2 and counted, 6 compared - the declared witness of that
///   normalization.
/// * `solvable_now:Version8/Distrib/Examples/StoCtrl_SeasonTarget/Run_example.dss`
///   (`both`): the created **directory** convention: `ieee13nodecktmod/`,
///   `ieee13nodecktmod/di_yr_0/` and the six DI CSVs under it are all members
///   (8 names). It is also the hook G1.10c reads the DI tree's contents from.
/// * `solvable_now:Test/DistanceRelayTest.DSS` - the `r4133`-only channel; 1
///   file, the deck's own `export eventlog` (`rly21test_eventlog.txt`).
/// * `solvable_now:Version8/Distrib/Examples/HarmonicsTMode/IEEE_519.DSS` - the
///   `capi_v0145`-only channel, and the measured **empty** set: the deck's two
///   writers (`export monitor MPCC`, `show monitor MPCC` - capi's `ShowMonitor`
///   re-writes the same CSV) and the harmonics scratch all target files that are
///   *vendored* in the corpus (`IEEE_519_Mon_mpcc_1.csv` and
///   `IEEE_519_SavedVoltages.dbl` are tracked), so nothing is CREATED and the
///   honest answer is the empty set. It is a real witness all the same: the
///   presence rail (`capture_guard::require_capture_opt`) fails the case if the
///   capi transport answers `None`, so this row is what proves that channel's
///   capture path is wired. It is also the only capi-only witness available - of
///   the 44 capi-only live cases exactly three reach a file-writing command and
///   all three write into vendored names (`tmp/g110a/scan_writers.txt`).
/// * `solvable_now:Version8/Distrib/Examples/CIM/IEEE13_CDPSM.dss` - `both`,
///   `kind=large`, so it is reached ONLY by the manifest declaration (which "only
///   ever ADDS") and not by [`force_run_files`]: 1 file, the CIM export
///   (`ieee13nodeckt_cim100x.xml`; r4133 spells it `.XML` and the fold makes the
///   two channels agree). It proves the flag itself works where the force rule
///   does not reach.
///
/// No family manifest declares the flag: measured over the three family
/// manifests, no `asymmetric`/`controls`/`modes` deck issues a file-writing
/// command at all, so a family row would declare an unconditionally empty set.
const RUN_FILES_DECLARED_IN_MANIFEST: &[(&str, &str)] = &[
    ("solvable_now:Test/DistanceRelayTest.DSS", "r4133"),
    ("solvable_now:Test/REACTORTest.DSS", "both"),
    (
        "solvable_now:Version8/Distrib/Examples/CIM/IEEE13_CDPSM.dss",
        "both",
    ),
    (
        "solvable_now:Version8/Distrib/Examples/HarmonicsTMode/IEEE_519.DSS",
        "capi_v0145",
    ),
    (
        "solvable_now:Version8/Distrib/Examples/StoCtrl_SeasonTarget/Run_example.dss",
        "both",
    ),
    (
        "solvable_now:Version8/Distrib/IEEETestCases/NEVTestCase/Run_NEV.dss",
        "both",
    ),
];

/// **The run-file forcing rule is a rule, not a habit** - the G1.10a twin of
/// [`the_inc_matrix_forcing_rule_is_every_live_non_large_case`], and for the same
/// reason: nothing else can see [`force_run_files`]. `population.lock.json`
/// fingerprints manifest flags only, and a comparison that never runs leaves no
/// trace anywhere - a rule narrowed from 443 cases to 40 would stay green
/// everywhere else. This test walks the four manifests without an oracle and
/// asserts the forced set **is** the live non-`large` population, cell for cell,
/// plus exactly the declared rows on top of it.
#[test]
fn the_run_files_forcing_rule_is_every_live_non_large_case() {
    let cases = build_unified_cases();
    let mut forced = (0usize, 0usize, 0usize, 0usize);
    let mut wrong: Vec<String> = Vec::new();
    for uc in &cases {
        let live = uc.class == CaseClass::Live;
        let large = uc.case.kind.starts_with("large");
        // The rule: every live case, minus `large`. A manifest may also declare
        // the flag itself, which only ever ADDS - so the declared `large` witness
        // (`IEEE13_CDPSM.dss`) is expected here and is what the `declared` term
        // admits; a declared row on a pending/abort/deferred case never reaches
        // the forcing pass at all and so reds below.
        let declared = RUN_FILES_DECLARED_IN_MANIFEST
            .iter()
            .any(|(l, _)| *l == uc.label);
        let expected = live && (!large || declared);
        if uc.case.compare_run_files != expected {
            wrong.push(format!(
                "{}: kind={} engines={} class={} declared={declared} -> compare_run_files={} \
                 (expected {expected})",
                uc.label,
                uc.case.kind,
                uc.case.engines,
                if live { "live" } else { "not-live" },
                uc.case.compare_run_files,
            ));
        }
        if uc.case.compare_run_files {
            forced.0 += 1;
            match uc.case.engines.as_str() {
                "both" => forced.1 += 1,
                "r4133" => forced.2 += 1,
                _ => forced.3 += 1,
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "the run-file forcing rule is `every live non-`large` case` (GOLDEN_REBASE G1.10a, \
         2026-09-05), plus whatever `RUN_FILES_DECLARED_IN_MANIFEST` adds - these cases \
         disagree with it:\n  {}",
        wrong.join("\n  ")
    );
    // The declared `large` witness is the one case the rule itself does not
    // reach, so the pinned tuple counts the rule's own population and this
    // assertion counts the declaration that reaches past it.
    let declared_large: Vec<&str> = cases
        .iter()
        .filter(|uc| {
            uc.class == CaseClass::Live
                && uc.case.kind.starts_with("large")
                && uc.case.compare_run_files
        })
        .map(|uc| uc.label.as_str())
        .collect();
    assert_eq!(
        declared_large,
        ["solvable_now:Version8/Distrib/Examples/CIM/IEEE13_CDPSM.dss"],
        "exactly one `kind=large*` case declares `compare_run_files` - the proof that a manifest \
         declaration reaches where `force_run_files` does not"
    );
    let by_rule = (
        forced.0 - declared_large.len(),
        forced.1 - declared_large.len(),
        forced.2,
        forced.3,
    );
    assert_eq!(
        by_rule, FORCED_RUN_FILES_POPULATION,
        "(forced, both, r4133-only, capi-only) moved. A DROP in either r4133 half is a \
         per-channel re-mask of the run-file request - invisible to every other gate, because \
         a comparison that does not run reports nothing. It also re-measures the D25/Q2 \
         scratch-file decline census, which was derived over exactly this population. A \
         legitimate corpus change moves this lock together with `population.lock.json`."
    );
    assert_eq!(
        FORCED_RUN_FILES_POPULATION, FORCED_INC_MATRIX_POPULATION,
        "G1.10a forces the run-file surface over the SAME population as the incidence surface \
         (one rule, one set to reason about - `force_run_files`'s doc). If that is deliberately \
         no longer true, narrow one rule, drop this assertion, and re-measure the scratch-file \
         decline census, which is a function of this population."
    );
    assert_eq!(
        FORCED_RUN_FILES_POPULATION, FORCED_TOPOLOGY_POPULATION,
        "...and the same population as the topology surface."
    );
    assert_eq!(
        FORCED_RUN_FILES_POPULATION, FORCED_PROPS_POPULATION,
        "...and the same population as the property surface, which is where the `live \
         non-`large`` rule came from (RP0.1/RP0.2's census). Four rules, one forced set."
    );
}

/// **The surface reaches the anti-shrink lock** - the static half of the G1.10a
/// acceptance (`GOLDEN_REBASE_PLAN.md` section 1.1(f): the flag is set on at
/// least one case per gating channel).
///
/// [`force_run_files`] arms the gate but is scheduler code; only a *declared*
/// manifest row reaches `population_lock.rs::rigor`'s `runf=` token, so this test
/// pins exactly which rows carry the declaration and on which channel. Deleting
/// one - the cheapest way to shrink the surface's recorded footprint - fails here
/// and in `population.lock.json`, never silently.
#[test]
fn the_run_files_surface_is_declared_on_every_gating_channel() {
    let mut declared: Vec<(String, String)> = Vec::new();
    for c in load_solvable() {
        if c.compare_run_files {
            declared.push((format!("solvable_now:{}", c.path), c.engines.clone()));
        }
    }
    for fam in FAMILIES {
        for c in load_family(fam.name) {
            if c.compare_run_files {
                declared.push((format!("{}:{}", fam.name, c.path), c.engines.clone()));
            }
        }
    }
    declared.sort();
    let mut want: Vec<(String, String)> = RUN_FILES_DECLARED_IN_MANIFEST
        .iter()
        .map(|(l, e)| ((*l).to_string(), (*e).to_string()))
        .collect();
    want.sort();
    assert_eq!(
        declared, want,
        "the manifest declarations of `compare_run_files` moved. They are what puts the surface \
         into `population.lock.json` (the `runf=` rigor token) - the scheduler-side \
         `force_run_files` is invisible to it - so this set is pinned, and a change to it \
         belongs in the same commit as a regenerated lock."
    );
    for ch in ["capi_v0145", "r4133", "both"] {
        assert!(
            declared.iter().any(|(_, e)| e == ch),
            "no manifest case declares `compare_run_files` with engines={ch:?}; \
             GOLDEN_REBASE_PLAN.md section 1.1(f) wants the flag set on at least one case per \
             gating channel, so that each channel's capture path is exercised by a declared row \
             and not only by the scheduler's forcing rule"
        );
    }
}

/// **D25/Q2 — the population where an ORACLE writes an engine-internal scratch
/// file the port, by design, never writes**, as `(cases, names)`.
///
/// Both Pascal engines round-trip the fundamental solution through disk when a
/// harmonics run starts: `SavePresentVoltages` writes
/// `<OutputDirectory><CircuitName_>SavedVoltages.dbl` (r4133
/// `Common/Utilities.pas:1512-1521`, called from `InitializeForHarmonics`
/// `:1608`) and `RetrieveSavedVoltages` reads it back (`:1554-1564`, consumed at
/// `SolutionAlgs.pas:1056`/`:1131`); dss_capi 0.14.5 is the same. The port keeps
/// that state in memory (`crates/dss-core/src/solution/solution/state.rs`,
/// `solution/solution/harmonics.rs`) and writes no such file, so the name is
/// **not** a divergence to triage but an artifact of the oracles' internal
/// plumbing — the D25/Q2 settlement splits it off SYMMETRICALLY in
/// `harness::run_files::compare_run_files` (asserting the port side of the split
/// is empty, so a port that ever starts writing one is a real change and not a
/// normalization) and counts it here. **0 `ledger.json` rows**: a row per
/// harmonics deck would be ~a dozen entries for one structural cause, which is
/// exactly the shape coordinator decisions D8/D15/D16 rejected.
///
/// The pin is that census, re-derived on every full gate run and failing in
/// BOTH directions — a bigger population means a new name started being split
/// off with nobody looking, a smaller one means the normalization now covers
/// less than it claims. Both numbers of one row are pinned literally by
/// `run_files_pins::the_harmonics_scratch_file_is_declined_on_the_nev_deck`.
///
/// **Measured `(9, 9)` on 2026-09-06** (GOLDEN_REBASE G1.10a F4e, lane
/// `lane-s`), read off COMPLETED full 526-case drives — the value is never
/// guessed, because the epilogue that calls the assertion below
/// (`corpus_gate.rs`) runs only after the per-case failure report, so a drive
/// with ANY red case never reaches it and a partial run cannot found a
/// fail-on-stale population. Nine cases, nine names, one name each: the four
/// `modes:harmonics/*` decks, `modes:inputformat/xycurve_files`, and four
/// `solvable_now:` harmonics decks (`Test/PVSystemTestHarm.dss`,
/// `Examples/FreqScan/Run_Scan.dss`, `Examples/HarmonicsVariableLoad/IEEE_519.DSS`,
/// `IEEETestCases/NEVTestCase/Run_NEV.dss`), each declining exactly one
/// `<CircuitName_>savedvoltages.dbl`. That is the whole population that reaches
/// `InitializeForHarmonics` on either oracle, and it is identical on the two
/// lanes — the full row table is in the record
/// (`docs/phase-records/golden-rebase.md`).
///
/// `(cases, names)` and not `(cases, names, visits)`: the census key is
/// `(case label, scratch name)` with no channel in it, so a `both` case that
/// declines the same name on both channels is one row either way, and the
/// process-wide visit counter also counts this binary's own
/// `harness::run_files` unit fixtures. Those fixtures are why the population is
/// read off the decline TABLE and restricted to labels the manifests actually
/// carry — every other label must be one of them, which the assertion checks
/// rather than assumes.
const SCRATCH_FILE_DECLINES: (usize, usize) = (9, 9);

/// The D25/Q2 census, re-derived from this run and pinned in both directions —
/// the [`assert_topology_declines_are_the_pinned_population`] shape for a
/// settlement that deliberately writes no ledger rows.
///
/// Silent in the two documented situations that shape has: `DSS_GATE_ONLY` is
/// set (a filtered run holds a filtered population), and no live manifest case
/// requests `compare_run_files` at all (a fact re-read from the manifests here,
/// not assumed, so the assertion arms itself the moment the surface is forced
/// or declared).
pub(crate) fn assert_scratch_declines_are_the_pinned_population() {
    if std::env::var("DSS_GATE_ONLY").is_ok() {
        return;
    }
    let cases = build_unified_cases();
    let requested = cases
        .iter()
        .filter(|uc| uc.class == CaseClass::Live)
        .any(|uc| uc.case.compare_run_files);
    #[cfg(not(windows))]
    {
        assert!(
            !requested,
            "`compare_run_files` is Windows-only (the classification lives in the \
             `#[cfg(windows)]` crate `dss-epri`), yet a live case requests it"
        );
    }
    #[cfg(windows)]
    {
        let gate_labels: std::collections::BTreeSet<&str> =
            cases.iter().map(|uc| uc.label.as_str()).collect();
        let table = harness::run_files::scratch_decline_table();
        let census = harness::run_files::scratch_census();
        // Rows this binary's own `harness::run_files` unit fixtures wrote. They
        // share the process-wide census with the gate, so they are separated by
        // label and REFUSED when they are neither a manifest case nor one of
        // those fixtures — a typo'd or foreign label must never be absorbed into
        // the pinned population.
        let (gate_rows, fixture_rows): (Vec<_>, Vec<_>) = table
            .iter()
            .partition(|(case, _)| gate_labels.contains(case.as_str()));
        let foreign: Vec<&String> = fixture_rows
            .iter()
            .map(|(case, _)| *case)
            .filter(|case| !case.starts_with("unit:"))
            .collect();
        assert!(
            foreign.is_empty(),
            "the D25/Q2 scratch census carries the label(s) {foreign:?}, which are \
             neither a manifest case nor one of this binary's `unit:` fixtures. The \
             pinned population is a fact about the corpus, so an unrecognized \
             producer fails instead of being counted or dropped."
        );
        let measured = (
            gate_rows.len(),
            gate_rows
                .iter()
                .map(|(_, names)| names.len())
                .sum::<usize>(),
        );
        let report: Vec<String> = gate_rows
            .iter()
            .map(|(case, names)| format!("{case} -> {names:?}"))
            .collect();
        if !requested {
            assert_eq!(
                measured,
                (0, 0),
                "no manifest case requests `compare_run_files`, yet the run-file \
                 comparator split engine-scratch files off real gate cases:\n  {}",
                report.join("\n  ")
            );
            eprintln!(
                "corpus_gate run files: the surface is not requested by any live case \
                 - nothing to re-derive"
            );
            return;
        }
        eprintln!(
            "corpus_gate run files: D25/Q2 engine-scratch declines {measured:?} \
             (+{} unit fixture row(s); {} channel visit(s) and {} set compare(s) \
             process-wide)\n  {}",
            fixture_rows.len(),
            census.visits,
            census.compared,
            report.join("\n  ")
        );
        assert_eq!(
            measured,
            SCRATCH_FILE_DECLINES,
            "the D25/Q2 engine-scratch decline population moved (measured {:?}, \
             pinned {:?}). It is re-derived on every run and fails in BOTH \
             directions: a bigger population means a case started having a name \
             split off its created-file SET with nobody looking, a smaller one \
             means the normalization now covers less than it claims. Re-measure, \
             move the constant WITH the record, and re-read the pin \
             `run_files_pins::the_harmonics_scratch_file_is_declined_on_the_nev_deck`.\
             \n  {}",
            measured,
            SCRATCH_FILE_DECLINES,
            report.join("\n  ")
        );
        assert!(
            measured.0 > 0,
            "the D25/Q2 scratch census counted nothing on a run where live cases \
             request `compare_run_files`. The pinned population would then be a \
             statement about nothing - the surface is masked off somewhere \
             between the manifest and the transports."
        );
    }
}
// ---------------------------------------------------------------------------
// GOLDEN_REBASE G1.10b — the run-file CONTENTS census (coordinator decisions
// D40(1)/(9), D43(1)).
// ---------------------------------------------------------------------------

/// What one `(case, channel)` contents comparison measured, as the gating call
/// site saw it.
///
/// Recorded in `corpus_gate/runner.rs` from the `CellTally`
/// `harness::run_file_contents::compare_run_file_cells` hands back, and never
/// inside that comparator (coordinator decision **D24**, the
/// [`harness::record_seq_arm`] precedent): the `harness::run_file_contents`
/// fixtures in THIS test binary call the comparator too — several of them on
/// the very Storage-trace kind the read-back tail below is derived from — so
/// under the mandatory `cargo test --workspace` shape a comparator-side census
/// reads the gating population *plus* the fixtures. Counting at the call site
/// makes the population gate-only by construction, which is what lets the
/// constants below be pinned exactly instead of as a floor.
#[cfg(windows)]
#[derive(Clone, Debug)]
pub(crate) struct RunFileContentsRow {
    /// The manifest case label, so a moved census names the deck.
    case: String,
    /// `capi_v0145` / `r4133` — the gating channel this comparison ran against.
    channel: &'static str,
    /// Report files whose contents were compared cell by cell.
    files: usize,
    /// Cells compared under the D40(1) `case floor + print ulp` rule.
    compared: usize,
    /// Cells the column map declined: the Storage trace's `%-.g` columns, which
    /// the two ORACLES spell differently from each other (**D40(3)**, counts
    /// corrected by **D43(5)**), and an angle whose paired magnitude is below
    /// the case's own current floor, where the angle carries no information.
    declined: usize,
    /// Files accounting a reader read-back tail
    /// (`ReportKind::accounts_readback_tail` — today the Storage `DebugTrace`).
    trace_files: usize,
    /// `oracle_rows − port_rows` summed over those files.
    trace_tail: usize,
}

#[cfg(windows)]
static RUN_FILE_CONTENTS_ROWS: Mutex<Vec<RunFileContentsRow>> = Mutex::new(Vec::new());

/// Record one `(case, channel)` contents comparison for the epilogue below.
///
/// Called once per gated comparison from `corpus_gate/runner.rs`, including the
/// (many) cases whose run produced no selected report at all — a row with
/// `files = 0` is what makes "the surface was requested and compared nothing"
/// visible instead of absent.
#[cfg(windows)]
pub(crate) fn record_run_file_contents(
    case: &str,
    channel: &'static str,
    tally: harness::run_file_contents::CellTally,
) {
    RUN_FILE_CONTENTS_ROWS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(RunFileContentsRow {
            case: case.to_string(),
            channel,
            files: tally.files,
            compared: tally.compared,
            declined: tally.declined,
            trace_files: tally.trace_files,
            trace_tail: tally.trace_tail,
        });
}

/// `(cases, file comparisons, compared cells)` — the G1.10b contents surface's
/// non-vacuity census, re-derived on every full run and pinned in BOTH
/// directions.
///
/// A comparison census is the only thing that keeps a contents surface from
/// silently comparing nothing (spec §2.3(5); coordinator decision **D40(9)**
/// makes "a census that compares 0 cells on a gating channel" a STOP). A
/// smaller population means a report stopped travelling — a selection pattern
/// that no longer matches, a deck that stopped exporting, a transport that
/// stopped copying — and a bigger one means a kind started being compared with
/// nobody choosing its policy.
///
/// The key is `(case, channel)`, so a `both` case that exports a report
/// contributes one file comparison per channel; `cases` counts the distinct
/// manifest labels behind them.
///
/// **Measured `(7, 32, 3 035 190)` on 2026-09-12** (lane `lane-s`), read off a
/// COMPLETED full 526-case default-lane drive and confirmed on the parity lane:
/// the six forced `engines: "both"` decks that export a report this surface has
/// a policy for (`Test/PVSystemTest.dss`, `…/WampServer/testcommandline.dss`,
/// `…/NEVTestCase/Run_NEV.dss`, `…/8500-Node/Run_8500Node.dss`,
/// `…/8500-Node/Run_8500Node_Unbal.dss`, `…/GFM_IEEE8500/Run_8500Node_Unbal.dss`)
/// plus `…/StorageTechNote/Example_9_3_Price/Storage_price.dss` for the Storage
/// `DebugTrace` — 16 file comparisons and 1 517 59x cells per channel.
/// Never guessed: the epilogue runs only after the per-case failure report, so
/// a drive with ANY red case never reaches it and a partial run cannot found a
/// fail-on-stale population. The row table is in the record
/// (`docs/phase-records/golden-rebase.md`).
#[cfg(windows)]
const RUN_FILE_CONTENTS_COMPARED: (usize, usize, usize) = (7, 32, 3_035_190);

/// `(cases, (case, channel) comparisons, declined cells)` — the same census for
/// the cells the column maps deliberately do NOT compare.
///
/// Deliberate deviation from spec §2.3(5)'s `(files, names)`: the declined
/// *names* are a STATIC property of the column maps
/// (`harness::run_file_contents::PrintFmt::DeclinedG`), pinned where they are
/// declared (`the_trace_layout_follows_its_own_header`) and by the both-numbers
/// pin `the_two_sig_trace_columns_are_declined_on_both_channels`; what a run
/// can silently change is how many cells fall into them, which is what this
/// census pins.
///
/// **Measured `(4, 8, 14 832)` on 2026-09-12**, same drives as
/// [`RUN_FILE_CONTENTS_COMPARED`]: the Storage trace's 36 declined columns over
/// its 98 compared records (3 528 cells per channel) plus the below-floor angle
/// cells of the three 8500-Node decks (1 293–1 297 per (deck, channel)). The
/// two channels differ by two cells on `Run_8500Node.dss` (capi 1 293 / r4133
/// 1 295) because the arm is two-sided — `|I| = max(|I_oracle|, |I_port|)` —
/// so which cells fall below the case's own current floor is a property of the
/// pair, not of the port alone.
#[cfg(windows)]
const RUN_FILE_CONTENTS_DECLINES: (usize, usize, usize) = (4, 8, 14_832);

/// `(files accounting a tail, records per file)` — the Storage `DebugTrace`
/// read-back tail, coordinator decision **D43(1)(iii)**.
///
/// `WriteTraceRecord` is unconditional at the END of `GetTerminalCurrents`,
/// OUTSIDE the `IterminalSolutionCount <> SolutionCount` cache test (r4133
/// `Version8/Source/PCElements/Storage.pas:2874`, test at `:2868-2871`; capi
/// 0.14.5 `src/PCElements/Storage.pas:2356`, test at `:2348-2353`), so every
/// terminal-current READ appends a record: the file's row count is a property
/// of the reader as well as of the run. The two oracle transports read the
/// element six times after the last solve and append six records; the port's
/// `exec::view::snapshot_elements` recomputes ONCE at the converged `NodeV`
/// (GOLDEN_REBASE G2.3) and appends two — so the port's file is the oracle's
/// minus a constant tail, never longer (`compare_one` refuses that outright)
/// and its solve records are compared cell by cell under the unchanged D40(1)
/// rule.
///
/// Pinned per FILE and in BOTH directions: a gap that grows is a port that
/// stopped writing solve records or a transport that grew an element read; a
/// gap that shrinks is a port that started logging the gate's own capture; a
/// file count that moves is the trace comparison going quiet (or a second deck
/// starting one whose tail nobody measured). The per-file detail (case,
/// channel, both row counts) is in
/// `harness::run_file_contents::trace_tail_census`.
///
/// **Measured `(2, 4)` on 2026-09-12**: `Storage_price.dss` × both channels,
/// oracle 102 data rows vs port 98 on each — 96 identical solve records (48
/// hours × 2 iterations), oracle tail 6, port tail 2. Both numbers live in the
/// pin
/// `run_file_contents_pins::the_storage_trace_tail_is_the_readers_footprint`.
#[cfg(windows)]
const TRACE_READBACK_RECORDS: (usize, usize) = (2, 4);

/// The G1.10b contents census, re-derived from this run and pinned in both
/// directions — the [`assert_scratch_declines_are_the_pinned_population`] shape
/// for a settlement that deliberately writes NO ledger rows.
///
/// Silent in the two documented situations that shape has: `DSS_GATE_ONLY` is
/// set (a filtered run holds a filtered population — the measurement is still
/// reported, just not asserted), and no live manifest case requests
/// `compare_run_files` at all (a fact re-read from the manifests here, not
/// assumed, so the assertion arms itself the moment the surface is forced or
/// declared).
pub(crate) fn assert_run_file_contents_census_is_the_pinned_population() {
    // The surface is Windows-only for the reason `harness::run_files` is (the
    // `r4133` transport is a Win64 DLL); a live case requesting it elsewhere is
    // already refused by `assert_scratch_declines_are_the_pinned_population`.
    #[cfg(windows)]
    {
        let rows = RUN_FILE_CONTENTS_ROWS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let sum = |f: fn(&RunFileContentsRow) -> usize| rows.iter().map(f).sum::<usize>();
        let cases: BTreeSet<&str> = rows
            .iter()
            .filter(|r| r.files > 0)
            .map(|r| r.case.as_str())
            .collect();
        let compared = (cases.len(), sum(|r| r.files), sum(|r| r.compared));
        let declined_rows: Vec<&RunFileContentsRow> =
            rows.iter().filter(|r| r.declined > 0).collect();
        let declined_cases: BTreeSet<&str> =
            declined_rows.iter().map(|r| r.case.as_str()).collect();
        let declines = (
            declined_cases.len(),
            declined_rows.len(),
            sum(|r| r.declined),
        );
        let trace = (sum(|r| r.trace_files), sum(|r| r.trace_tail));
        // Per channel, because D40(9)'s STOP is per gating channel: a surface
        // that compares everything on one channel and nothing on the other is
        // exactly as masked as one that compares nothing at all.
        let mut per_channel: BTreeMap<&str, (usize, usize, usize)> = BTreeMap::new();
        for r in &rows {
            let e = per_channel.entry(r.channel).or_default();
            e.0 += r.files;
            e.1 += r.compared;
            e.2 += r.declined;
        }
        let report: Vec<String> = rows
            .iter()
            .filter(|r| r.files > 0)
            .map(|r| {
                format!(
                    "{} [{}] -> {} file(s), {} compared / {} declined cell(s), \
                     read-back tail {} over {} file(s)",
                    r.case, r.channel, r.files, r.compared, r.declined, r.trace_tail, r.trace_files
                )
            })
            .collect();
        // `harness::run_files::contents_census` is the transport-side half —
        // files matched and oracle bytes decoded, process-wide (this binary's
        // fixtures included, which is why it is REPORTED and never asserted;
        // the gate-only rows above are what the constants pin).
        let transported = harness::run_files::contents_census();
        eprintln!(
            "corpus_gate run-file contents: compared {compared:?}, declines \
             {declines:?}, read-back tail {trace:?}, per channel {per_channel:?} \
             over {} visit(s); transported (process-wide) {transported:?}\n  {}",
            rows.len(),
            report.join("\n  ")
        );
        if std::env::var("DSS_GATE_ONLY").is_ok() {
            return;
        }
        let all = build_unified_cases();
        let requested = all
            .iter()
            .filter(|uc| uc.class == CaseClass::Live)
            .any(|uc| uc.case.compare_run_files);
        if !requested {
            assert_eq!(
                (compared.1, compared.2),
                (0, 0),
                "no manifest case requests `compare_run_files`, yet the contents \
                 comparator ran on real gate cases:\n  {}",
                report.join("\n  ")
            );
            eprintln!(
                "corpus_gate run-file contents: the surface is not requested by any \
                 live case - nothing to re-derive"
            );
            return;
        }
        // Every label the census carries must be a manifest case: the recorder
        // is called only from the gate's runner, so a foreign label would mean
        // the population had stopped being a fact about the corpus.
        let gate_labels: BTreeSet<&str> = all.iter().map(|uc| uc.label.as_str()).collect();
        let foreign: Vec<&str> = rows
            .iter()
            .map(|r| r.case.as_str())
            .filter(|case| !gate_labels.contains(case))
            .collect();
        assert!(
            foreign.is_empty(),
            "the G1.10b contents census carries the label(s) {foreign:?}, which are \
             not manifest cases. The recorder runs only at the gate's own call \
             site, so an unrecognized producer fails instead of being counted."
        );
        assert_eq!(
            compared,
            RUN_FILE_CONTENTS_COMPARED,
            "the G1.10b run-file CONTENTS population moved (measured {:?}, pinned \
             {:?}). It is re-derived on every run and fails in BOTH directions: a \
             smaller population means a report stopped travelling (a selection \
             pattern that no longer matches, a deck that stopped exporting, a \
             transport that stopped copying), a bigger one means a kind started \
             being compared without anyone choosing its policy. Re-measure off a \
             COMPLETED full drive, move the constant WITH the record, and re-read \
             the pins in `run_file_contents_pins`.\n  {}",
            compared,
            RUN_FILE_CONTENTS_COMPARED,
            report.join("\n  ")
        );
        assert_eq!(
            declines,
            RUN_FILE_CONTENTS_DECLINES,
            "the G1.10b declined-cell population moved (measured {:?}, pinned \
             {:?}). The declined SET is static (the column maps' \
             `PrintFmt::DeclinedG` and the below-floor angle arm), so a moved cell \
             count means a file grew or lost rows, or a column changed class. \
             Re-measure off a COMPLETED full drive and move the constant WITH the \
             record.\n  {}",
            declines,
            RUN_FILE_CONTENTS_DECLINES,
            report.join("\n  ")
        );
        for (channel, (files, compared_cells, _)) in &per_channel {
            assert!(
                *files > 0 && *compared_cells > 0,
                "the G1.10b contents census compared {compared_cells} cell(s) over \
                 {files} file(s) on the gating channel `{channel}`. A surface that \
                 is live on one channel and silent on the other is masked \
                 (coordinator decision D40(9)): re-read the selection \
                 (`dss_epri::guard::RUN_FILE_CONTENTS_PATTERNS`), that transport's \
                 copy step and the sidecar read."
            );
        }
        for r in rows.iter().filter(|r| r.trace_files > 0) {
            assert_eq!(
                r.trace_tail,
                r.trace_files * TRACE_READBACK_RECORDS.1,
                "{} [{}]: the Storage `DebugTrace` read-back tail is {} record(s) \
                 over {} file(s), not the pinned {} per file. The oracle's file is \
                 the port's plus the records the transport's OWN post-solve \
                 element reads appended (r4133 PCElements/Storage.pas:2874, capi \
                 0.14.5 :2356) — a moved gap is a port that stopped writing solve \
                 records, a transport that grew an element read, or a port that \
                 started logging the gate's own capture. Re-measure and move the \
                 constant WITH the record and the pin \
                 `the_storage_trace_tail_is_the_readers_footprint`.",
                r.case,
                r.channel,
                r.trace_tail,
                r.trace_files,
                TRACE_READBACK_RECORDS.1
            );
        }
        assert_eq!(
            trace.0, TRACE_READBACK_RECORDS.0,
            "the Storage `DebugTrace` comparison ran on {} file(s), not the pinned \
             {}. Fail-on-stale in both directions: fewer means the only deck that \
             sets `debugtrace=yes` stopped reaching the surface (or its contents \
             stopped being selected), more means another deck started writing a \
             trace whose tail nobody measured.",
            trace.0, TRACE_READBACK_RECORDS.0
        );
    }
}

/// Build one unified case, applying the exact per-source property-forcing +
/// classification of the pre-Phase-B gates.
fn make_case(
    order: usize,
    source: &str,
    mut c: SolvableCase,
    abs: String,
    fam_props: bool,
) -> UnifiedCase {
    // Structural guard carried over from the pre-Phase-B family gate
    // (`family_cases_match_oracle`): `pending` (Rust must error loudly, oracle-free)
    // and `expect_solve_abort` (both engines must abort the solve) are contradictory
    // contracts. The classifier below resolves ties pending-first, so without this a
    // both-flags manifest case would silently skip the stronger abort contract.
    // Applied to EVERY source here (the old check was family-only) — strictly stronger.
    assert!(
        !(c.pending && c.expect_solve_abort.is_some()),
        "{source}:{}: `pending` and `expect_solve_abort` are mutually exclusive",
        c.path
    );
    crate::manifest::assert_defer_ledger_is_valid(&format!("{source}:{}", c.path), &c);
    let class = if c.defer_ledger.is_some() {
        CaseClass::Deferred
    } else if c.pending {
        CaseClass::Pending
    } else if c.expect_solve_abort.is_some() {
        CaseClass::Abort
    } else {
        CaseClass::Live
    };
    if class == CaseClass::Live {
        force_properties(source, &mut c, fam_props);
        force_pdelements(source, &mut c);
        force_derived(source, &mut c);
        force_bus(&mut c);
        force_zsc(&mut c);
        force_element_extras(source, &mut c);
        force_topology(&mut c);
        force_inc_matrix(&mut c);
        force_run_files(&mut c);
    }
    let weight = kind_weight(&c.kind) * (c.n_steps.max(1) as u64);
    let dir_key = dir_key_of(&abs);
    UnifiedCase {
        order,
        label: format!("{source}:{}", c.path),
        abs,
        dir_key,
        weight,
        class,
        case: c,
    }
}

/// Build the union of all four manifests into unified cases.
fn build_unified_cases() -> Vec<UnifiedCase> {
    let mut out: Vec<UnifiedCase> = Vec::new();
    for c in load_solvable() {
        let abs = corpus_file(&c.path);
        out.push(make_case(out.len(), "solvable_now", c, abs, false));
    }
    for fam in FAMILIES {
        for c in load_family(fam.name) {
            let abs = family_file(fam.name, &c.path);
            out.push(make_case(
                out.len(),
                fam.name,
                c,
                abs,
                fam.compare_all_properties,
            ));
        }
    }
    out
}

/// Coordinator decision **D33(2)**, the scheduler's half: two manifest rows in
/// ONE case directory are never scheduled at the same time, because
/// [`build_tasks`] puts them in ONE task and a task is drained by a single
/// thread, in manifest order. Case-insensitively — a Windows path can reach one
/// physical folder through two spellings, and [`dir_key_of`] folds them.
///
/// The other half of the guarantee is against the SIBLING `#[test]`s of this
/// binary (see the module doc) and is pinned by
/// `runner::corpus_guard_serializes_two_threads_in_one_case_directory`.
#[test]
fn two_manifest_rows_in_one_case_directory_land_in_one_task() {
    let mk = |order: usize, abs: &str| UnifiedCase {
        order,
        label: format!("fixture:{abs}"),
        abs: abs.to_string(),
        dir_key: dir_key_of(abs),
        weight: 1,
        class: CaseClass::Live,
        case: SolvableCase::default(),
    };
    // The `IEEETestCases/8500-Node` shape: several rows writing
    // `<CircuitName>_*` reports into one folder, one of them spelled with a
    // different case, plus one row in a different folder.
    let tasks = build_tasks(
        vec![
            mk(0, "C:/corpus/8500-Node/Run_8500Node.dss"),
            mk(1, "C:/corpus/8500-Node/P174_Run_Voltage_Profile.DSS"),
            mk(2, "C:/corpus/8500-node/Master-unbal.dss"),
            mk(3, "C:/corpus/AutoTrans/Auto3bus.dss"),
        ],
        None,
    );
    assert_eq!(
        tasks.len(),
        2,
        "three rows of one case directory (two spellings) plus one row of \
         another must be exactly two tasks, got {:?}",
        tasks.iter().map(|t| &t.dir_key).collect::<Vec<_>>()
    );
    let shared = tasks
        .iter()
        .find(|t| t.dir_key.ends_with("8500-node"))
        .expect("the shared case directory has a task");
    assert_eq!(
        shared.cases.iter().map(|c| c.order).collect::<Vec<_>>(),
        vec![0, 1, 2],
        "every row of one case directory belongs to that directory's single \
         task, in manifest order — a task is drained by ONE thread, which is \
         what keeps their file-writing windows apart (D33(2))"
    );
    let other = tasks
        .iter()
        .find(|t| t.dir_key.ends_with("autotrans"))
        .expect("the other case directory has its own task");
    assert_eq!(other.cases.len(), 1);
}

/// A tiny deterministic PRNG (splitmix64) for the shuffle probe — avoids a dev
/// dependency and keeps the shuffle reproducible from `DSS_GATE_SHUFFLE=<seed>`.
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Group cases into per-directory tasks, sorted longest-first (or shuffled).
fn build_tasks(cases: Vec<UnifiedCase>, shuffle_seed: Option<u64>) -> Vec<Task> {
    let mut by_dir: BTreeMap<String, Task> = BTreeMap::new();
    for uc in cases {
        let t = by_dir.entry(uc.dir_key.clone()).or_insert_with(|| Task {
            dir_key: uc.dir_key.clone(),
            weight: 0,
            cases: Vec::new(),
        });
        t.weight += uc.weight;
        t.cases.push(uc);
    }
    let mut tasks: Vec<Task> = by_dir.into_values().collect();
    // Keep cases within a task in manifest order (stable — build order).
    for t in &mut tasks {
        t.cases.sort_by_key(|c| c.order);
    }
    match shuffle_seed {
        None => {
            // Longest-first; tie-break on dir_key for determinism.
            tasks.sort_by(|a, b| b.weight.cmp(&a.weight).then(a.dir_key.cmp(&b.dir_key)));
        }
        Some(seed) => {
            // Deterministic Fisher–Yates shuffle of the task order.
            let mut state = seed ^ 0xD1B5_4A32_D192_ED03;
            for i in (1..tasks.len()).rev() {
                let j = (splitmix64(&mut state) % (i as u64 + 1)) as usize;
                tasks.swap(i, j);
            }
        }
    }
    tasks
}

// ---------------------------------------------------------------------------
// Execution.
// ---------------------------------------------------------------------------

pub(crate) struct CaseOutcome {
    pub(crate) order: usize,
    pub(crate) label: String,
    pub(crate) ok: bool,
    pub(crate) reason: String,
    /// Raw oracle result JSON (live cases only) — retained only when dumping,
    /// for the three-way contamination bit-diff.
    pub(crate) result: Option<Value>,
}

/// Shared, read-only execution context borrowed by every scheduler thread.
struct Ctx<'a> {
    serial: bool,
    dumping: bool,
    capi_pool: Option<&'a WorkerPool>,
    capi_oneshot: Option<&'a Oracle>,
    epri_pool: Option<&'a EpriPool>,
    epri_oneshot: Option<&'a EpriOneShot>,
    ledger: &'a LedgerRuntime,
}

impl<'a> Ctx<'a> {
    /// The transport for one (case, channel): the persistent pool normally, or a
    /// throwaway one-shot for `isolate`/serial (both channels honor the flag).
    fn channel(&self, uc: &UnifiedCase, ch: EngineChannel) -> Channel<'a> {
        let one_shot = uc.case.isolate || self.serial;
        match ch {
            EngineChannel::CapiV0145 => {
                if one_shot {
                    Channel::CapiOneShot(
                        self.capi_oneshot
                            .expect("capi one-shot pre-created for serial/isolate capi cases"),
                    )
                } else {
                    Channel::CapiPool(
                        self.capi_pool
                            .expect("capi pool pre-created for pooled capi cases"),
                    )
                }
            }
            EngineChannel::R4133 => {
                if one_shot {
                    Channel::EpriOneShot(
                        self.epri_oneshot
                            .expect("epri one-shot pre-created for serial/isolate r4133 cases"),
                    )
                } else {
                    Channel::EpriPool(
                        self.epri_pool
                            .expect("epri pool pre-created for pooled r4133 cases"),
                    )
                }
            }
        }
    }
}

/// Run + compare one case, fully `catch_unwind`-isolated → a `CaseOutcome`.
fn run_one_case(uc: &UnifiedCase, ctx: &Ctx) -> CaseOutcome {
    let _guard = CorpusGuard::new(&uc.abs);
    let ok_outcome = |result: Option<Value>| CaseOutcome {
        order: uc.order,
        label: uc.label.clone(),
        ok: true,
        reason: String::new(),
        result,
    };
    let fail_outcome = |reason: String, result: Option<Value>| CaseOutcome {
        order: uc.order,
        label: uc.label.clone(),
        ok: false,
        reason,
        result,
    };

    match uc.class {
        CaseClass::Pending => {
            // Pending is oracle-free (Rust must error loudly) — no channel.
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                assert_pending_errors_loudly(&uc.label, &uc.abs, &uc.case);
            }));
            match res {
                Ok(()) => ok_outcome(None),
                Err(e) => fail_outcome(panic_msg(e), None),
            }
        }
        CaseClass::Deferred => {
            // Parked from live compare (Phase D ledger seed); Rust-smoke only.
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                assert_deferred_rust_smoke(&uc.label, &uc.abs, &uc.case);
            }));
            match res {
                Ok(()) => ok_outcome(None),
                Err(e) => fail_outcome(panic_msg(e), None),
            }
        }
        CaseClass::Abort => {
            // Abort against every gating channel (Phase C: exactly one).
            for ch in uc.case.engine_channels() {
                // A ledger `skip` entry (a hard-crash deck on this channel) means
                // the channel is not sent at all; the other channel still gates.
                if ctx.ledger.channel_is_skipped(&uc.label, ch) {
                    continue;
                }
                let channel = ctx.channel(uc, ch);
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run_and_compare_abort(&channel, &uc.label, &uc.abs, &uc.case);
                }));
                if let Err(e) = res {
                    return fail_outcome(format!("[{ch:?}] {}", panic_msg(e)), None);
                }
            }
            ok_outcome(None)
        }
        CaseClass::Live => {
            // Live-compare against every gating channel. The Rust engine is re-run
            // per channel inside `compare_with_result`; `both` (Phase D) thus runs
            // it twice — the seam is intentional. A ledger `skip` entry drops a
            // hard-crash channel; the ledger's field scopes partition the compare.
            let mut dump_val: Option<Value> = None;
            for ch in uc.case.engine_channels() {
                if ctx.ledger.channel_is_skipped(&uc.label, ch) {
                    continue;
                }
                let channel = ctx.channel(uc, ch);
                // R4133_PROPS RP4.1 (2026-09-03): the per-channel
                // `compare_all_properties = false` that used to stand here is
                // GONE — property parity is no longer capi_v0145-only, and the
                // r4133 request now carries the same all-properties capture the
                // capi one does ([`force_properties`], `dss-epri::capture`). The
                // clone is what the compare closure below owns; it is no longer
                // per-channel, so both channels see the same case.
                let cc = uc.case.clone();
                let req = build_run_request(&uc.abs, &cc);
                let resp = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    channel.call(&req)
                })) {
                    Ok(r) => r,
                    Err(e) => {
                        return fail_outcome(
                            format!("[{ch:?}] oracle fetch panicked: {}", panic_msg(e)),
                            dump_val,
                        );
                    }
                };
                if !resp.ok {
                    return fail_outcome(
                        format!("[{ch:?}] oracle case failed: {:?}", resp.error),
                        dump_val,
                    );
                }
                let Some(val) = resp.result else {
                    return fail_outcome(
                        format!("[{ch:?}] oracle ok response missing result"),
                        dump_val,
                    );
                };
                if ctx.dumping && dump_val.is_none() {
                    dump_val = Some(val.clone());
                }
                let label = uc.label.clone();
                let abs = uc.abs.clone();
                let view = ctx.ledger.view(&uc.label, ch);
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                    let oc: CaseResult = serde_json::from_value(val)
                        .unwrap_or_else(|e| panic!("{label}: malformed CaseResult: {e}"));
                    compare_with_result(&oc, &label, &abs, &cc, ch, Some(&view));
                }));
                if let Err(e) = res {
                    return fail_outcome(format!("[{ch:?}] {}", panic_msg(e)), dump_val);
                }
            }
            ok_outcome(dump_val)
        }
    }
}

/// The result of one gate run: outcomes (manifest order) + wall-clock + config.
pub(crate) struct GateRun {
    pub(crate) outcomes: Vec<CaseOutcome>,
    pub(crate) elapsed: Duration,
    pub(crate) mode: String,
    pub(crate) jobs: usize,
    pub(crate) pool_size: usize,
    pub(crate) total: usize,
    /// The loaded ledger, kept for the `#[test]` to assert fail-on-stale and print
    /// per-entry hit accounting (§1.3 runtime rule).
    pub(crate) ledger: Arc<LedgerRuntime>,
}

/// Run the whole unified gate once (mode from env). Pure execution — the
/// `#[test]` wrapper does the asserting + reporting + optional dump.
pub(crate) fn run_gate() -> GateRun {
    let serial = std::env::var("DSS_GATE_SERIAL")
        .map(|v| v == "1")
        .unwrap_or(false);
    let shuffle_seed = std::env::var("DSS_GATE_SHUFFLE")
        .ok()
        .and_then(|s| s.parse::<u64>().ok());
    let dumping = std::env::var("DSS_GATE_DUMP").is_ok();
    let jobs = if serial {
        1
    } else {
        std::env::var("DSS_GATE_JOBS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|&n| n >= 1)
            .unwrap_or_else(|| {
                std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(4)
            })
    };
    let pool_size = (jobs / 2).max(2);

    let start = Instant::now();
    let mut cases = build_unified_cases();
    // Optional case filter (`DSS_GATE_ONLY=<substr>[,<substr>...]`) for cheap
    // targeted verification under load — the FULL gate (no filter) is the one that
    // gates commits. When set, prints a loud banner so a filtered run is never
    // mistaken for the full gate.
    if let Ok(only) = std::env::var("DSS_GATE_ONLY") {
        let subs: Vec<String> = only
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let before = cases.len();
        cases.retain(|c| subs.iter().any(|s| c.label.contains(s)));
        eprintln!(
            "corpus_gate: DSS_GATE_ONLY filter kept {}/{before} case(s) — THIS IS A PARTIAL \
             RUN, not the commit gate",
            cases.len()
        );
        // A filter matching NOTHING must fail loudly, never green a 0/0 run (a
        // leftover exported env var would otherwise silently neuter the gate —
        // pre-E/F audit UGA-T5).
        assert!(
            !cases.is_empty(),
            "DSS_GATE_ONLY={only:?} matched no case labels — refusing to green a \
             zero-case gate run; fix or unset the filter"
        );
    }
    let total = cases.len();

    // Which transports does the case set actually need? A case can gate the
    // capi_v0145 channel, the r4133 channel, or (Phase D) both; `isolate`/serial
    // routes an engine run through a throwaway one-shot instead of the pool.
    // Only Live/Abort cases actually drive an oracle channel (Pending is
    // oracle-free; Deferred is Rust-smoke-only).
    let any = |ch: EngineChannel, isolate: bool| {
        cases.iter().any(|c| {
            matches!(c.class, CaseClass::Live | CaseClass::Abort)
                && c.case.engine_channels().contains(&ch)
                && c.case.isolate == isolate
        })
    };
    let has_capi = any(EngineChannel::CapiV0145, true) || any(EngineChannel::CapiV0145, false);
    let has_epri = any(EngineChannel::R4133, true) || any(EngineChannel::R4133, false);
    // In serial mode EVERY engine run uses a fresh one-shot process (the
    // contamination-proof reference); otherwise only `isolate` cases do.
    let needs_capi_oneshot = any(EngineChannel::CapiV0145, true) || (serial && has_capi);
    let needs_capi_pool = !serial && any(EngineChannel::CapiV0145, false);
    let needs_epri_oneshot = any(EngineChannel::R4133, true) || (serial && has_epri);
    let needs_epri_pool = !serial && any(EngineChannel::R4133, false);

    // `for_spec(None)` constructs the pinned one-shot oracle AND ping-verifies it;
    // the pools/one-shots ping-verify their engine identity on spawn.
    let capi_oneshot: Option<Oracle> = needs_capi_oneshot.then(|| Oracle::for_spec(None));
    let capi_pool: Option<WorkerPool> = needs_capi_pool.then(|| WorkerPool::new(pool_size));
    let epri_oneshot: Option<EpriOneShot> = needs_epri_oneshot.then(EpriOneShot::new);
    let epri_pool: Option<EpriPool> = needs_epri_pool.then(|| EpriPool::new(pool_size));

    let ledger = Arc::new(LedgerRuntime::load());
    // GateRun keeps its own handle to the same runtime (shared hit counters); the
    // `ctx` borrow of `ledger` lives until the end of the function, so move the
    // clone (not the borrowed original) into the result.
    let ledger_result = Arc::clone(&ledger);
    let tasks = build_tasks(cases, shuffle_seed);
    let ctx = Ctx {
        serial,
        dumping,
        capi_pool: capi_pool.as_ref(),
        capi_oneshot: capi_oneshot.as_ref(),
        epri_pool: epri_pool.as_ref(),
        epri_oneshot: epri_oneshot.as_ref(),
        ledger: &ledger,
    };

    let cursor = AtomicUsize::new(0);
    let results: Mutex<Vec<CaseOutcome>> = Mutex::new(Vec::with_capacity(total));

    std::thread::scope(|s| {
        for _ in 0..jobs {
            s.spawn(|| {
                loop {
                    let idx = cursor.fetch_add(1, Ordering::Relaxed);
                    if idx >= tasks.len() {
                        break;
                    }
                    let task = &tasks[idx];
                    let mut local: Vec<CaseOutcome> = Vec::with_capacity(task.cases.len());
                    for uc in &task.cases {
                        local.push(run_one_case(uc, &ctx));
                    }
                    results.lock().unwrap().extend(local);
                }
            });
        }
    });

    if let Some(p) = &capi_pool {
        p.close();
    }
    if let Some(p) = &epri_pool {
        p.close();
    }

    let mut outcomes = results.into_inner().unwrap();
    outcomes.sort_by_key(|o| o.order);
    let elapsed = start.elapsed();

    let mode = if serial {
        "serial-oneshot".to_string()
    } else if let Some(seed) = shuffle_seed {
        format!("persistent-parallel-shuffled(seed={seed})")
    } else {
        "persistent-parallel".to_string()
    };

    GateRun {
        outcomes,
        elapsed,
        mode,
        jobs,
        pool_size,
        total,
        ledger: ledger_result,
    }
}

// ---------------------------------------------------------------------------
// Seeding report mode (§4 Phase D step 2 — DSS_GATE_SEED_LEDGER=1).
// ---------------------------------------------------------------------------

/// One seeding measurement: how a case compares against ONE channel, with NO
/// ledger applied (raw divergence), so triage sees where each case would need a
/// ledger entry to gate `both`.
#[derive(serde::Serialize)]
struct SeedRecord {
    case: String,
    channel: String,
    status: String,
    reason: String,
}

/// Run every live case against BOTH channels regardless of its `engines`,
/// measuring the raw (ledger-free) divergence, and write candidate triage data to
/// `tmp/ledger_candidates.json`. Never asserts — a report tool.
pub(crate) fn seed_ledger() {
    let jobs = std::env::var("DSS_GATE_JOBS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&n| n >= 1)
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
        });
    let pool_size = (jobs / 2).max(2);
    let start = Instant::now();

    // Optional case filter (`DSS_GATE_SEED_ONLY=<substr>[,<substr>...]`) — measure
    // only matching labels, for cheap targeted re-measurement under load.
    let only: Vec<String> = std::env::var("DSS_GATE_SEED_ONLY")
        .ok()
        .map(|s| {
            s.split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let cases = build_unified_cases();
    // Seed only Live cases (Pending is oracle-free; Abort has its own contract) —
    // but a Deferred case IS a primary seed, so include it as Live.
    let seedable: Vec<UnifiedCase> = cases
        .into_iter()
        .filter(|c| matches!(c.class, CaseClass::Live | CaseClass::Deferred))
        .filter(|c| only.is_empty() || only.iter().any(|s| c.label.contains(s)))
        .collect();
    let total = seedable.len();

    let capi_pool = WorkerPool::new(pool_size);
    let epri_pool = EpriPool::new(pool_size);
    let capi_oneshot = Oracle::for_spec(None);
    let epri_oneshot = EpriOneShot::new();
    let ctx = Ctx {
        serial: false,
        dumping: false,
        capi_pool: Some(&capi_pool),
        capi_oneshot: Some(&capi_oneshot),
        epri_pool: Some(&epri_pool),
        epri_oneshot: Some(&epri_oneshot),
        // A throwaway empty ledger — seeding measures the RAW divergence.
        ledger: EMPTY_LEDGER_FOR_SEEDING.get_or_init(LedgerRuntime::empty),
    };

    // Group by dir like the main gate (avoid same-dir contention).
    let tasks = build_tasks(seedable, None);
    let cursor = AtomicUsize::new(0);
    let records: Mutex<Vec<SeedRecord>> = Mutex::new(Vec::with_capacity(2 * total));

    std::thread::scope(|s| {
        for _ in 0..jobs {
            s.spawn(|| {
                loop {
                    let idx = cursor.fetch_add(1, Ordering::Relaxed);
                    if idx >= tasks.len() {
                        break;
                    }
                    let mut local = Vec::new();
                    for uc in &tasks[idx].cases {
                        for ch in [EngineChannel::CapiV0145, EngineChannel::R4133] {
                            local.push(seed_one(uc, ch, &ctx));
                        }
                    }
                    records.lock().unwrap().extend(local);
                }
            });
        }
    });

    capi_pool.close();
    epri_pool.close();

    let mut records = records.into_inner().unwrap();
    records.sort_by(|a, b| a.case.cmp(&b.case).then(a.channel.cmp(&b.channel)));
    let matched = records.iter().filter(|r| r.status == "match").count();
    let diverged = records.iter().filter(|r| r.status == "diverge").count();
    let errored = records.iter().filter(|r| r.status == "error").count();

    let out: std::path::PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "tmp"]
        .iter()
        .collect::<std::path::PathBuf>()
        .join("ledger_candidates.json");
    let _ = std::fs::create_dir_all(out.parent().unwrap());
    let json = serde_json::json!({
        "total_cases": total,
        "measurements": records.len(),
        "match": matched,
        "diverge": diverged,
        "error": errored,
        "records": records,
    });
    std::fs::write(&out, serde_json::to_string_pretty(&json).unwrap())
        .unwrap_or_else(|e| panic!("write {}: {e}", out.display()));
    eprintln!(
        "seed_ledger: {total} case(s) x2 channels = {} measurement(s): {matched} match, \
         {diverged} diverge, {errored} error in {:.1}s -> {}",
        records.len(),
        start.elapsed().as_secs_f64(),
        out.display()
    );
}

/// The throwaway empty ledger the two REPORT modes share (seeding and the
/// property census): both measure the RAW divergence, with no entry applied.
static EMPTY_LEDGER_FOR_SEEDING: std::sync::OnceLock<LedgerRuntime> = std::sync::OnceLock::new();

/// Measure one (case, channel) with no ledger; catch every panic into a status.
fn seed_one(uc: &UnifiedCase, ch: EngineChannel, ctx: &Ctx) -> SeedRecord {
    let channel_name = match ch {
        EngineChannel::CapiV0145 => "capi_v0145",
        EngineChannel::R4133 => "r4133",
    };
    let mk = |status: &str, reason: String| SeedRecord {
        case: uc.label.clone(),
        channel: channel_name.to_string(),
        status: status.to_string(),
        reason: reason.chars().take(300).collect(),
    };
    let _guard = CorpusGuard::new(&uc.abs);
    let channel = ctx.channel(uc, ch);
    // R4133_PROPS RP4.1 (2026-09-03): the seeding path's per-channel
    // `compare_all_properties = false` went with the gate path's — a seeding
    // measurement must see exactly what the gate compares, on both channels. The
    // clone is what the compare closure below owns.
    let cc = uc.case.clone();
    let req = build_run_request(&uc.abs, &cc);
    let resp = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| channel.call(&req))) {
        Ok(r) => r,
        Err(e) => return mk("error", format!("fetch panic: {}", panic_msg(e))),
    };
    if !resp.ok {
        return mk("error", format!("oracle: {:?}", resp.error));
    }
    let Some(val) = resp.result else {
        return mk("error", "ok response missing result".to_string());
    };
    let label = uc.label.clone();
    let abs = uc.abs.clone();
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let oc: CaseResult =
            serde_json::from_value(val).unwrap_or_else(|e| panic!("malformed CaseResult: {e}"));
        compare_with_result(&oc, &label, &abs, &cc, ch, None);
    }));
    match res {
        Ok(()) => mk("match", String::new()),
        Err(e) => mk("diverge", panic_msg(e)),
    }
}

// ---------------------------------------------------------------------------
// Property census mode (`R4133_PROPS_PLAN.md` RP0.2 — DSS_PROPS_CENSUS).
// ---------------------------------------------------------------------------

/// Walk every live case on BOTH channels with `all_properties` forced on — which
/// until R4133_PROPS RP4.1 (2026-09-03) meant bypassing the §1.1 gate masks that
/// kept property parity capi-only, and since RP4.1 (which deleted those masks)
/// means only ignoring the case's own `engines` key — and
/// COLLECT every divergent cell instead of asserting. Writes
/// `tmp/props_census.json` plus the per-channel RP0.1 extracts
/// ([`crate::props_census`]); **asserts nothing about the data**.
///
/// This is the permanent successor of the scratch test that produced the
/// 2026-08-08 census vendored at `tests/corpus/props_r4133/`. It reproduces that
/// walk's population: live cases only (`pending`/`abort`/`defer_ledger` have
/// their own contracts and no property capture), `large`-kind decks excluded
/// (they are excluded from property forcing on the gate too,
/// [`force_properties`], and the vendored census carries none), the case's own
/// `engines` key ignored — a capi-only deck is still measured against r4133,
/// which is where the census's GenDispatcher and Sensor shape rows come from.
///
/// The normal gate path is untouched: nothing here runs unless the env var is
/// set. (The per-channel masks this doc used to point at, in [`run_one_case`]
/// and [`seed_one`], were removed by RP4.1; what still separates this walk from
/// the gate is that it ignores `engines` and asserts nothing.)
pub(crate) fn run_props_census(raw_mode: &str) {
    let mode = CensusMode::from_env(raw_mode);
    let jobs = std::env::var("DSS_GATE_JOBS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&n| n >= 1)
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
        });
    let pool_size = (jobs / 2).max(2);
    let start = Instant::now();

    let mut cases: Vec<UnifiedCase> = build_unified_cases()
        .into_iter()
        .filter(|c| c.class == CaseClass::Live && !c.case.kind.starts_with("large"))
        .collect();
    // Same filter (and same loud empty-match refusal) as the gate: a census that
    // silently measured zero cases would look exactly like a clean one. The
    // filter travels into the artifacts (`gate_only`), so a bounded run cannot
    // be mistaken for a full one later.
    let gate_only = std::env::var("DSS_GATE_ONLY").ok();
    if let Some(only) = gate_only.clone() {
        let subs: Vec<String> = only
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let before = cases.len();
        cases.retain(|c| subs.iter().any(|s| c.label.contains(s)));
        eprintln!(
            "props_census: DSS_GATE_ONLY filter kept {}/{before} case(s)",
            cases.len()
        );
        assert!(
            !cases.is_empty(),
            "DSS_GATE_ONLY={only:?} matched no case labels — refusing to write an empty census"
        );
    }
    let total = cases.len();

    let capi_pool = WorkerPool::new(pool_size);
    let epri_pool = EpriPool::new(pool_size);
    let capi_oneshot = Oracle::for_spec(None);
    let epri_oneshot = EpriOneShot::new();
    // The claims mode's LAST chain link (plan §1.1(e)): a `property`-scoped
    // ledger entry NAMES a cell. It is loaded SEPARATELY from `Ctx.ledger`,
    // which stays empty on purpose — the walk must keep measuring the raw
    // divergence (RP0.2's baseline, and a `skip` entry must not hide a case),
    // while the disposition query answers "would an entry claim this cell at
    // RP4.1?". Only the non-asserting `property_scope_keys` is consulted, so a
    // census can never fail on a pin, and this runtime's hit counters are its
    // own (the gate's `assert_all_hit` reads a different instance).
    let claims_ledger = mode.annotates().then(LedgerRuntime::load);
    let ctx = Ctx {
        serial: false,
        dumping: false,
        capi_pool: Some(&capi_pool),
        capi_oneshot: Some(&capi_oneshot),
        epri_pool: Some(&epri_pool),
        epri_oneshot: Some(&epri_oneshot),
        // The census measures the RAW divergence — no ledger entry applies, and
        // no channel `skip` entry hides a case (the vendored census records the
        // #303 crash decks as `oracle_error` rows for exactly that reason).
        ledger: EMPTY_LEDGER_FOR_SEEDING.get_or_init(LedgerRuntime::empty),
    };

    let tasks = build_tasks(cases, None);
    let cursor = AtomicUsize::new(0);
    #[allow(clippy::type_complexity)]
    let collected: Mutex<(Vec<CensusRow>, BTreeMap<&'static str, CensusBlindSpots>)> =
        Mutex::new((Vec::new(), BTreeMap::new()));

    std::thread::scope(|s| {
        for _ in 0..jobs {
            s.spawn(|| {
                loop {
                    let idx = cursor.fetch_add(1, Ordering::Relaxed);
                    if idx >= tasks.len() {
                        break;
                    }
                    let mut rows = Vec::new();
                    let mut blind: BTreeMap<&'static str, CensusBlindSpots> = BTreeMap::new();
                    for uc in &tasks[idx].cases {
                        for ch in [EngineChannel::CapiV0145, EngineChannel::R4133] {
                            let (r, b) = census_one(uc, ch, &ctx, mode, claims_ledger.as_ref());
                            rows.extend(r);
                            blind.entry(channel_tag(ch)).or_default().add(b);
                        }
                    }
                    let mut guard = collected.lock().unwrap();
                    guard.0.extend(rows);
                    for (k, v) in blind {
                        guard.1.entry(k).or_default().add(v);
                    }
                }
            });
        }
    });

    capi_pool.close();
    epri_pool.close();

    let (rows, blind) = collected.into_inner().unwrap();
    crate::props_census::write_artifacts(rows, mode, total, &blind, gate_only.as_deref());
    eprintln!(
        "props_census: {total} case(s) x2 channels in {:.1}s",
        start.elapsed().as_secs_f64()
    );
}

/// The census's spelling of a gate channel.
fn channel_tag(ch: EngineChannel) -> &'static str {
    ch.props_channel().tag()
}

/// Measure one (case, channel) for the census: force the property capture,
/// re-run the Rust engine step by step and collect every divergent cell. Every
/// failure mode — a dead oracle, a crashing deck, a panicking Rust walk —
/// becomes a ROW, never a panic. The second return value is what the walk could
/// not look at (see [`harness::CensusBlindSpots`]).
///
/// `mode` and `claims_ledger` are the disposition mode's two extras (RP2.1): in
/// [`CensusMode::Claims`] every value row is annotated with the r4133 policy
/// chain's verdict for that cell, the ledger link reading this case's
/// `property` scopes. The WALK is identical in both modes — see
/// [`crate::props_census::Disposition`] for why annotating the raw population is
/// the same statement as re-walking with the armed policy.
fn census_one(
    uc: &UnifiedCase,
    ch: EngineChannel,
    ctx: &Ctx,
    mode: CensusMode,
    claims_ledger: Option<&LedgerRuntime>,
) -> (Vec<CensusRow>, CensusBlindSpots) {
    let pch = ch.props_channel();
    // The vendored README's §"The in-scope filter": a case the RP4.1 unmask will
    // actually compare on r4133 is one whose manifest entry gates that channel.
    // Read off the case's own `engines` key, exactly as the filter defines it.
    let in_scope = uc.case.engines != "capi_v0145";
    let oracle_error = |detail: String| {
        (
            vec![CensusRow::error(
                &uc.label,
                pch,
                in_scope,
                CensusRowKind::OracleError { detail },
            )],
            CensusBlindSpots::default(),
        )
    };

    let _guard = CorpusGuard::new(&uc.abs);
    let channel = ctx.channel(uc, ch);
    let mut cc = uc.case.clone();
    // The knob's whole point: the property capture is requested on BOTH channels,
    // regardless of the plan §1.1 masks the gate and the seeding path apply.
    cc.compare_all_properties = true;
    let req = build_run_request(&uc.abs, &cc);
    let resp = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| channel.call(&req))) {
        Ok(r) => r,
        Err(e) => return oracle_error(format!("fetch panic: {}", panic_msg(e))),
    };
    if !resp.ok {
        return oracle_error(format!("{:?}", resp.error));
    }
    let Some(val) = resp.result else {
        return oracle_error("ok response missing result".to_string());
    };

    // Deserialization happens OUTSIDE the Rust-side `catch_unwind` on purpose: a
    // channel that answers `ok: true` with a payload the harness cannot parse is
    // an ORACLE defect, and in a mode whose only diagnostic is the row kind it
    // must not be filed against the Rust engine (RP0.2 audit).
    let oc: CaseResult = match serde_json::from_value(val) {
        Ok(oc) => oc,
        Err(e) => return oracle_error(format!("malformed CaseResult: {e}")),
    };

    let label = uc.label.clone();
    let abs = uc.abs.clone();
    // The ledger link, resolved once per (case, channel): the `property` scopes
    // that NAME a cell here. Empty in plain mode and whenever no entry applies.
    let ledger_view = claims_ledger.map(|rt| rt.view(&uc.label, ch));
    let walked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let tol = harness::tol_for(&cc.kind);
        let (mut dss, _baseline) = run_rust_capture(&label, &abs, &cc);
        let mut rows: Vec<CensusRow> = Vec::new();
        let mut blind = CensusBlindSpots::default();
        for (i, cp) in oc.checkpoints.iter().enumerate() {
            dss.command("solve");
            // A capture that came back empty means the request was not honored —
            // record it instead of reporting a spuriously clean step.
            if cp.all_properties.is_empty() {
                rows.push(CensusRow::error(
                    &label,
                    pch,
                    in_scope,
                    CensusRowKind::OracleError {
                        detail: format!("step {i}: empty all_properties capture"),
                    },
                ));
                continue;
            }
            let mut found = Vec::new();
            blind.add(harness::collect_prop_divergences(
                &mut dss,
                &cp.all_properties,
                &tol,
                pch,
                &mut found,
            ));
            let ledger_named = match &ledger_view {
                Some(v) => v.property_scope_keys(&cp.all_properties),
                None => BTreeSet::new(),
            };
            rows.extend(found.into_iter().map(|f| {
                let mut row = CensusRow::from_harness(&label, pch, in_scope, i, f);
                if mode.annotates() {
                    row.annotate(&ledger_named);
                }
                row
            }));
        }
        (rows, blind)
    }));
    match walked {
        Ok(out) => out,
        Err(e) => (
            vec![CensusRow::error(
                &uc.label,
                pch,
                in_scope,
                CensusRowKind::RustError {
                    detail: panic_msg(e),
                },
            )],
            CensusBlindSpots::default(),
        ),
    }
}

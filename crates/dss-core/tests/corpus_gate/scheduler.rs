//! Hand-rolled scheduler for the unified corpus gate.
//!
//! One `#[test]` runs the union of all four manifests. Every case is one task,
//! pre-sorted longest-first by a static weight, and drained by
//! `available_parallelism()` (or `DSS_GATE_JOBS`) threads via an `AtomicUsize`
//! cursor and `std::thread::scope`, not rayon or tokio: tasks block on engine
//! subprocess I/O, and the global rayon pool belongs to faer inside the solves.
//!
//! Two cases of one folder may run at once because no producer runs in the
//! vendored tree: every (case, producer) run compiles its own fresh scratch
//! copy ([`crate::scratch::ScratchCopy`]) and the copy is removed afterwards,
//! or the case fails naming the producer.
//!
//! The pinned `capi_v0145` channel is served by a persistent [`WorkerPool`], the
//! `r4133` channel by a persistent [`EpriPool`] of `epri-worker` bridge
//! processes. One-shots serve only `isolate` and serial runs. Each case is
//! `catch_unwind`-wrapped. The test fails when any case failed, printing the
//! complete failure list in manifest order.
//!
//! The contamination-proof knobs `DSS_GATE_SERIAL`, `DSS_GATE_SHUFFLE`,
//! `DSS_GATE_DUMP` and `DSS_GATE_JOBS` are read here and listed in the module
//! doc of `corpus_gate.rs`.

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
    assert_deferred_rust_smoke, assert_pending_errors_loudly, compare_with_result, panic_msg,
    run_and_compare_abort, run_rust_capture,
};
use crate::scratch::{PORT, ScratchCopy};

// ---------------------------------------------------------------------------
// Unified case model.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum CaseClass {
    Live,
    Abort,
    Pending,
    /// `defer_ledger`: parked from the live engine comparison, smoke-run on this
    /// engine only.
    Deferred,
}

struct UnifiedCase {
    order: usize,
    label: String,
    abs: String,
    weight: u64,
    class: CaseClass,
    case: SolvableCase,
}

/// One scheduled unit: exactly one case (the `cases` vector is the drain loops'
/// shape, kept so a future grouping stays a local change).
struct Task {
    key: String,
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

/// Apply the per-source property-forcing rule to a live case.
///
/// The property compare runs on both channels: the r4133 bridge captures all
/// properties (`crates/dss-epri/src/capture.rs`), so an r4133-gating case has
/// its property table compared exactly like a capi one. The request costs an
/// extra per-case `? name.Like` + `? name.prop` sweep on the r4133 worker.
///
/// The rule is therefore **every live case**, with one cost guard: `kind=large*`
/// decks stay out on the `solvable_now` arm. That guard is also definitional:
/// the census population every property measurement rests on is exactly "live,
/// non-`large`" ([`run_props_census`]).
///
/// Written without a channel predicate on purpose: `gates_capi() ||
/// gates_r4133()` is a tautology over the three legal `engines` values
/// (`capi_v0145|both|r4133`), and a tautological guard reads like a filter while
/// filtering nothing.
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
/// Re-derived off the four manifests by
/// [`the_property_forcing_rule_is_every_live_non_large_case`] on every run: the
/// live cases minus the `kind=large*` decks (all of them `solvable_now`). A
/// corpus change moves it together with `population.lock.json`.
///
/// The forced set is the population [`run_props_census`] walks, so this lock
/// also keeps the census and the gate talking about one population. The frozen
/// census extracts under `tests/corpus/props_r4133/` are a data lock and stay
/// at the population they were measured on.
const FORCED_PROPS_POPULATION: (usize, usize, usize, usize) = (444, 312, 87, 45);

/// Apply the per-source **element-extras** forcing rule to a live case.
///
/// Same shape and the same one cost guard as [`force_properties`] /
/// [`force_derived`]: **every live case, minus `kind=large*` on the
/// `solvable_now` arm.** Both channels honour the request
/// (`engines::build_run_request`'s `element_extras` key), so there is no channel
/// predicate here either — writing one would be the same tautology
/// [`force_properties`] documents.
///
/// **No opt-in list, deliberately.** [`force_derived`] carries
/// [`DERIVED_MANIFEST_OPT_INS`] because the dss-python reference harness skips
/// `Residuals` on those two decks. That harness's known-difference table has no
/// row for `NodeOrder`, `EnergyMeter` or the three counts, so there is no deck
/// an opt-in would buy anything on: a const here would be dead weight that
/// reads like a decision.
///
/// Kept as a **separate** function with a separate lock
/// ([`FORCED_ELEMENT_EXTRAS_POPULATION`]) rather than folded into
/// [`force_derived`], for the reason that one is kept separate from
/// [`force_properties`]: the two rules cover different surfaces, and a later
/// divergence must show as its own lock's diff.
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
/// Re-derived off the four manifests by
/// [`the_element_extras_forcing_rule_is_every_live_non_large_case`] on every
/// run: exactly [`FORCED_PROPS_POPULATION`]'s live non-`large` population,
/// because the rule is the same one and there are no opt-ins
/// ([`force_element_extras`]). It is nevertheless its own const: the two rules
/// are free to diverge, and a divergence must land as a reviewed diff here.
///
/// It exists for the reason [`FORCED_DERIVED_POPULATION`] does: the population
/// lock fingerprints the **manifest** flag (`Case::rigor`'s `elemx=` token in
/// `population_lock.rs`), not the effective one, so a re-mask of
/// [`force_element_extras`] would stop comparing the five discrete channels on
/// every one of these cases without moving one byte of `population.lock.json`.
const FORCED_ELEMENT_EXTRAS_POPULATION: (usize, usize, usize, usize) = (444, 312, 87, 45);

/// The property-forcing rule forces exactly the live non-`large` cases, with the
/// per-`engines` split pinned by [`FORCED_PROPS_POPULATION`]. It is the static
/// half of the re-mask alarm: the live guard is a boolean that a partial
/// re-mask passes.
#[test]
fn the_property_forcing_rule_is_every_live_non_large_case() {
    // `population.lock.json` fingerprints manifest flags and per-case ledger
    // tags, not scheduler code, and `props_norm::assert_r4133_props_compare_ran`
    // stays green when re-adding `gates_capi()` drops the r4133-only cases while
    // the `both` ones keep walking. This test walks the four manifests without
    // an engine and catches that.
    //
    // It also guards the family arm's missing cost guard: the arm ORs the
    // family-level flag with no `kind=large*` test, which is inert only while no
    // family case is `large`. That premise is asserted here rather than assumed.
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
        "the property-forcing rule is `every live non-`large` case` — these cases \
         disagree with it:\n  {}",
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

/// Apply the per-source **PDElements**-forcing rule to a live case.
///
/// The rule is [`force_properties`]': **every live case**, with the same one
/// cost guard — `kind=large*` decks stay out on the `solvable_now` arm. It is
/// deliberately the same population, so the two whole-model surfaces the gate
/// forces (properties and PDElements) never drift into talking about different
/// case sets; [`FORCED_PDELEMENTS_POPULATION`] re-derives it and pins the split.
///
/// Two shape notes, both deliberate:
///
/// * no channel predicate: `gates_capi() || gates_r4133()` is a tautology over
///   the three legal `engines` values, and a tautological guard reads like a
///   filter while filtering nothing. Both transports capture the walk
///   (`tools/oracle/oracle_server.py::capture_pd_elements`,
///   `crates/dss-epri/src/capture.rs::capture_pd_elements`);
/// * no family-flag parameter — [`force_properties`] takes `fam_props` because
///   `Family` carries a per-family `compare_all_properties`; there is no such
///   knob for this surface and inventing one would be a false switch (all three
///   families would set it). The family arm therefore forces unconditionally,
///   exactly what `fam_props` evaluates to for all three families today, and
///   the missing `large` guard on that arm is asserted inert by
///   [`the_pdelements_forcing_rule_is_every_live_non_large_case`] rather than
///   assumed.
///
/// The walk adds well under a second across the whole population.
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
const FORCED_PDELEMENTS_POPULATION: (usize, usize, usize, usize) = (444, 312, 87, 45);

/// Apply the per-source **derived-channel** forcing rule to a live case.
///
/// Same shape and the same one cost guard as [`force_properties`]: **every live
/// case, minus `kind=large*` on the `solvable_now` arm.** Both channels honor
/// the request (`engines::build_run_request`'s `derived` key), so there is no
/// channel predicate here either — writing one would be the same tautology
/// [`force_properties`] documents.
///
/// Kept as a **separate** function and a separate lock
/// ([`FORCED_DERIVED_POPULATION`]) rather than folded into the property rule
/// even though the two forcing rules coincide (the populations differ by the
/// two opt-ins): they cover different surfaces, a heavy deck could earn an
/// opt-out from one and not the other, and a later divergence of the rules must
/// show as its own lock's diff.
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
/// carry only a handful of elements, so the guard's cost rationale does not
/// apply to them. They are exactly the two decks the dss-python reference
/// harness skips `Residuals` on, so gating them is where this comparator is
/// strictly stronger than that harness: the opt-in is the point, not a
/// convenience.
const DERIVED_MANIFEST_OPT_INS: &[&str] = &[
    "solvable_now:Test/AutoTrans/Auto3bus.dss",
    "solvable_now:Test/AutoTrans/AutoHLT.dss",
];

/// **The forced derived-channel population, pinned** — `(cases with
/// `compare_derived` on, of them `engines: "both"`, `engines: "r4133"`,
/// `engines: "capi_v0145"`)`.
///
/// Re-derived by
/// [`the_derived_forcing_rule_is_every_live_non_large_case_plus_the_opt_ins`] on
/// every run: [`FORCED_PROPS_POPULATION`]'s live non-`large` cases plus the two
/// [`DERIVED_MANIFEST_OPT_INS`] decks (both `engines: "both"`).
///
/// The population lock fingerprints the **manifest** flag, not the effective
/// one (`Case::rigor`'s `derived=` token in `population_lock.rs`), so a re-mask
/// of [`force_derived`] would drop every forced case without moving one byte of
/// `population.lock.json`. This const is the only thing that sees it, the reason
/// [`FORCED_PROPS_POPULATION`] exists, applied to the second rule.
const FORCED_DERIVED_POPULATION: (usize, usize, usize, usize) = (446, 314, 87, 45);

/// The PDElements-forcing rule forces exactly the live non-`large` cases, with
/// the per-`engines` split pinned by [`FORCED_PDELEMENTS_POPULATION`]. No
/// manifest sets the flag, so `population.lock.json` cannot see a re-mask here.
#[test]
fn the_pdelements_forcing_rule_is_every_live_non_large_case() {
    // `harness::assert_pd_elements_compare_ran` is the live half, and it is a
    // boolean: re-adding a channel predicate (say `gates_capi()`, which drops
    // the `engines: "r4133"` cases while the `both` ones keep walking) passes it.
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
        "the PDElements-forcing rule is `every live non-`large` case` — these cases \
         disagree with it:\n  {}",
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

/// The derived-channel forcing rule flags exactly the live non-`large` cases
/// plus [`DERIVED_MANIFEST_OPT_INS`], with the per-`engines` split pinned by
/// [`FORCED_DERIVED_POPULATION`]. Each opt-in must be a live case the rule
/// would not have covered, or the entry is dead weight.
#[test]
fn the_derived_forcing_rule_is_every_live_non_large_case_plus_the_opt_ins() {
    // `population.lock.json` fingerprints the manifest flag, so a re-mask of
    // `force_derived` (a channel predicate, a widened `large` guard, a dropped
    // source arm) would silently stop comparing
    // `CurrentsMagAng`/`VoltagesMagAng`/`Residuals` while every lock stayed
    // green.
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
         DERIVED_MANIFEST_OPT_INS` — these cases disagree with \
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

/// Force the **bus voltage surface** on.
///
/// The manifests set `compare_bus` on no case (`bus=0` in
/// `population.lock.json`), exactly as they set `compare_all_properties` on
/// none: a surface that is only compared where a manifest opts in is a surface
/// nobody compares. The rule is therefore the same one [`force_properties`]
/// runs, **every live case whose `kind` does not start with `large`**, written
/// once here for every source: unlike the property arm there is no
/// family-level bus flag to OR in, and the `large` cost guard is not
/// source-specific (that no family deck is `large` is asserted by
/// [`the_property_forcing_rule_is_every_live_non_large_case`], so the two rules
/// select the same cases and the two surfaces talk about one population).
///
/// The surface is the dss-python `ActiveBus` facade. Its arms here are the ones
/// DSS C-API 0.14.5 and r4133 read alike: `puVoltages`, `VMagAngle`,
/// `puVmagAngle`, `Nodes`, `kVBase` and the circuit-level `AllBusVmagPu`. The
/// reads add well under a second per channel per gate run.
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
/// fingerprints the **manifest** flag (`bus=0` on every case), so nothing else
/// in the tree can see this rule at all. A force rule ships with its own
/// population pin, because a silent narrowing here (a channel predicate, a
/// second `kind` prefix) would otherwise leave the gate green on a smaller
/// corpus.
const FORCED_BUS_POPULATION: (usize, usize, usize, usize) = (444, 312, 87, 45);

/// The bus-forcing rule forces exactly the live non-`large` cases, with the
/// per-`engines` split pinned by [`FORCED_BUS_POPULATION`]. The runner's capture
/// guard proves the flag never greens on an empty capture, and this test proves
/// the flag is on where it should be.
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
        "the bus-forcing rule is `every live non-`large` case` — these          cases disagree with it:
  {}",
        wrong.join("
  ")
    );
    assert_eq!(
        forced, FORCED_BUS_POPULATION,
        "(forced, both, r4133-only, capi-only) moved. A DROP is a narrowing of the bus surface          that nothing else can see: `population.lock.json` fingerprints the MANIFEST flag, which          is `bus=0` on every case. A legitimate corpus change moves this lock together with          `population.lock.json` and `FORCED_PROPS_POPULATION`."
    );
}

/// Force the **short-circuit surface** on.
///
/// The same predicate as [`force_bus`], **every live case whose `kind` does
/// not start with `large`**, and deliberately so: the six short-circuit arms
/// extend the one per-bus walk `compare_bus` drives on both transports
/// (`oracle_server.py::capture_all_buses`,
/// `dss-epri::capture::capture_all_buses`), so a case that compares the bus
/// surface can compare this one for the cost of six more reads, and a case
/// that does not cannot compare it at all. That implication is asserted, never
/// or-ed in behind the manifest's back: in `engines::build_run_request` at
/// runtime and in [`the_zsc_forcing_rule_is_every_live_non_large_case`]
/// statically.
///
/// The arms are `Zsc1`, `Zsc0`, `ZscMatrix`, `YscMatrix`, `Isc` and `Voc`. Only
/// a few decks run a fault study, but `Voc` is live on every
/// `PreserveNodeVoltages` deck (building the Y matrix refreshes the bus
/// voltages) and the not-run sentinel shape is itself gated everywhere else,
/// which is what makes the wide population worth its cost: a few seconds per
/// channel per gate run at most, since a bus with no matrix ships one or two
/// doubles.
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
/// case but the fault-study micro deck), so nothing else in the tree can see
/// this rule, and a silent narrowing here (a channel predicate, a second `kind`
/// prefix) would leave the gate green on a smaller corpus.
const FORCED_ZSC_POPULATION: (usize, usize, usize, usize) = (444, 312, 87, 45);

/// The short-circuit forcing rule forces exactly the live non-`large` cases, and
/// every forced case also compares the bus surface its arms ride. The
/// implication is checked after both rules have run, so a manifest that sets
/// `compare_zsc` on a case the bus rule skips fails here, not mid-gate.
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
                "{}: compare_zsc without compare_bus — the six short-circuit arms ride the                  bus walk",
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
        "the short-circuit forcing rule is `every live non-`large` case` — these cases disagree with it:
  {}",
        wrong.join("
  ")
    );
    assert_eq!(
        forced, FORCED_ZSC_POPULATION,
        "(forced, both, r4133-only, capi-only) moved. A DROP is a narrowing of the          short-circuit surface that nothing else can see: `population.lock.json`          fingerprints the MANIFEST flag. A legitimate corpus change moves this lock          together with `population.lock.json`, `FORCED_BUS_POPULATION` and          `FORCED_PROPS_POPULATION`."
    );
}

/// The element-extras forcing rule flags exactly the live non-`large` cases,
/// with no opt-in, and the per-`engines` split pinned by
/// [`FORCED_ELEMENT_EXTRAS_POPULATION`]. A manifest key switching the flag on
/// for a `large` deck reds here instead of quietly widening the population.
#[test]
fn the_element_extras_forcing_rule_is_every_live_non_large_case() {
    // `population.lock.json` fingerprints the manifest flag, so a re-mask of
    // `force_element_extras` (a channel predicate, a widened `large` guard, a
    // dropped source arm) would silently stop comparing
    // `NumTerminals`/`NumConductors`/`NumPhases`/`EnergyMeter`/`NodeOrder`
    // while every lock stayed green.
    //
    // The model here (`live && !large`, for every source) is one notch stricter
    // than `force_element_extras`, whose `large` exception sits on the
    // `solvable_now` arm alone. The two agree only while no family deck is
    // `large`, so that premise is asserted rather than assumed.
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
         — these cases disagree with it:\n  {}",
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
// The topology surface: its forced population and the two decline populations
// ---------------------------------------------------------------------------

/// Apply the topology-forcing rule to a live case.
///
/// The surface is six of the nine `ITopology` fields the dss-python harness
/// dumps: `NumLoops`, `NumIsolatedBranches`, `NumIsolatedLoads`,
/// `AllLoopedPairs`, `AllIsolatedBranches`, `AllIsolatedLoads`. The other three
/// (`ActiveLevel`, `BranchName`, `ActiveBranch`) are deliberately not compared:
/// reading them reassigns the active circuit element and would poison the
/// per-element capture.
///
/// The rule is **every live case whose `kind` does not start with `large`**,
/// deliberately the same population [`force_properties`] forces, so the gate
/// reasons about one forced set instead of two ([`FORCED_TOPOLOGY_POPULATION`]
/// asserts the two stay equal). It costs six extra reads per checkpoint per
/// channel. Both OpenDSS engines memoize the branch tree, so the read is nearly
/// free there, while this engine rebuilds it on every call (a memoized tree
/// goes stale, see [`TOPOLOGY_STALE_DECLINES`]). That is what the `kind=large*`
/// cost guard buys here: the 8500-Node class carries hundreds of loops over
/// dozens of steps, in both lanes.
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
/// [`the_topology_forcing_rule_is_every_live_non_large_case`] on every run: the
/// live cases minus the `kind=large*` decks. Identical to
/// [`FORCED_PROPS_POPULATION`] by construction, and the test asserts that
/// equality instead of leaving it a comment: the two rules are one population
/// by design, so a drift between them is a review.
///
/// The lock this constant backs up is `population.lock.json`, which records the
/// **manifest** flag (`population_lock.rs::rigor`'s `topo=` token) and therefore
/// cannot see the scheduler-side forcing at all, the same blind spot the
/// property lock has. It also fixes the two decline populations:
/// [`TOPOLOGY_STALE_DECLINES`] and [`LOOPED_PAIR_WINDOW_DECLINES`] are measured
/// over exactly this set, so narrowing it moves them too.
const FORCED_TOPOLOGY_POPULATION: (usize, usize, usize, usize) = (444, 312, 87, 45);

/// **The manifest rows that declare `compare_topology` themselves**, with the
/// gating channel each one carries.
///
/// The forcing rule above is invisible to `population.lock.json`, so without at
/// least one declared row the surface would arm the whole gate while leaving the
/// anti-shrink lock byte-identical, and switching it off again would leave no
/// diff either. These decks are the surface's witnesses: every gating channel
/// is represented (`capi_v0145`, `r4133`, `both`), and each row's step-0 answer
/// is non-trivial in at least one of the six quantities, so the declaration is
/// a statement about real values and not about zeros:
///
/// * `13Bus/IEEE13Nodeckt.dss`: looped pairs, so `NumLoops` is their half.
/// * `Microgrid/ISource/Master.DSS`: all three counts non-zero.
/// * `HarmonicsTMode/IEEE_519.DSS`: the capi-only channel.
/// * `controls:fuse/indmach_r4133/indmach_dyn.dss`: the r4133-only channel.
/// * `controls:recloser/recloser_perm.dss`: the stale-tree pin deck (the
///   memoization witness).
/// * `asymmetric:combo/combo_mesh_asym.dss`: the deliberately meshed deck.
/// * `modes:reduce/reduce_breakloop.dss`: an isolated branch (`Line.l1`), also
///   the capi trailing-empty witness.
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

/// The topology-forcing rule forces exactly the live non-`large` cases, the same
/// population as the property surface. Nothing else sees a partial re-mask of
/// [`force_topology`]: the lock fingerprints manifest flags and the live census
/// only asks whether the surface ran at all.
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
        "the topology-forcing rule is `every live non-`large` case` — these cases \
         disagree with it:\n  {}",
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
        "the topology surface is forced over the SAME population as the property surface (one \
         rule, one set to reason about — `force_topology`'s doc). If that is deliberately no \
         longer true, narrow one rule, drop this assertion, and re-measure the two topology \
         decline constants, which are a function of this population."
    );
}

/// The manifest rows that declare `compare_topology` are exactly
/// [`TOPOLOGY_DECLARED_IN_MANIFEST`], at least one per gating channel. Only a
/// declared row reaches `population.lock.json`, so deleting one fails here, never
/// silently.
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
             the flag must be set on at least one case per gating \
             channel, so that each channel's capture path is exercised by a declared row and not \
             only by the scheduler's forcing rule"
        );
    }
}

/// **The population whose isolation half the OpenDSS engines answer from a
/// stale tree**, as `(cases, case-steps)`.
///
/// Both OpenDSS engines memoize the branch list and invalidate nothing on a
/// conductor open or close, so once a Relay, Recloser or SwtControl has operated
/// they answer `NumIsolated*` / `Isolated*` from the tree built at step 0. This
/// engine never caches (an OpenDSS bug is never reproduced), so at those steps
/// `harness::topology::compare_topology` rebases the isolation half onto this
/// engine's own step-0 answer and asserts the memoization contract instead of
/// comparing against the fresh one: a positive assertion, not an exclusion, and
/// no `ledger.json` rows.
///
/// The population is the `controls/*` cases on the r4133 channel plus
/// `modes/time/midi_duty_ctrl.dss`, which is `engines: "both"` and declines
/// identically on both channels. The pair is counted once here (the channel
/// visits behind it are reported separately by the census).
const TOPOLOGY_STALE_DECLINES: (usize, usize) = (16, 135);

/// **The population where the OpenDSS engines' looped-pair dedup drops a pair
/// this engine keeps**, as `(cases, case-steps)`.
///
/// The OpenDSS engines scan their flat name buffer in overlapping windows while
/// the intent is to skip a pair already found, so a genuinely new candidate that
/// equals a straddling window is dropped. This engine keeps the correct per-pair
/// dedup and the comparator asserts `engine looped pairs ==
/// harness::topology::window_dedup(this engine's candidates)`. This constant
/// counts where that model and this engine's own list part company (compared by
/// content, not by length, since the two rules keep different buffers), i.e.
/// where the OpenDSS bug actually bites.
///
/// It is a function of the forced topology population: multi-step
/// `controls/midi_*` decks plus `epri_dpv/M1/Master_NoPV.dss` and
/// `ADiakoptics/ckt24/Torn_Circuit/zone_2/master.dss`. The LVTestCase and ckt24
/// feeders diverge the same way but are `kind=large*` and therefore out of the
/// compare.
const LOOPED_PAIR_WINDOW_DECLINES: (usize, usize) = (8, 96);

/// **Both decline populations are re-derived on every full gate run and pinned
/// in both directions**, the fail-on-stale rule for two normalizations that
/// write no ledger rows.
///
/// A population that grew means this engine (or an OpenDSS engine) started
/// declining somewhere new and nobody looked. One that shrank means a decline
/// stopped happening, so the pin behind it is a statement about nothing. Either
/// way the gate must fail rather than absorb it: the ledger's own fail-on-stale
/// discipline (`ledger::assert_all_hit`) applied to a normalization that
/// deliberately writes no ledger rows.
///
/// Silent in two documented situations, and in no others:
///
/// * `DSS_GATE_ONLY` is set — a filtered run holds a filtered population, so its
///   census cannot be the pinned one (the mandatory gate never sets it).
/// * No manifest case requests `compare_topology` at all. That is a **fact about
///   the manifests**, re-read here rather than assumed: while the surface flag
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
             — nothing to re-derive"
        );
        return;
    }
    eprintln!(
        "corpus_gate topology: {} compared (case, step, channel) triple(s); \
         stale-tree declines {:?} ({} channel visit(s)), window declines {:?} \
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
        "the stale-tree decline population moved (measured {:?}, pinned {:?}). \
         It is re-derived on every run and fails in BOTH directions: a bigger \
         population means a case-step started answering from a stale tree with \
         nobody looking, a smaller one means the normalization now covers less \
         than it claims. Re-measure, move the constant, and re-read the pin \
         `topology_pins::topology_reads_a_freshly_built_tree`.\n{}",
        census.stale,
        TOPOLOGY_STALE_DECLINES,
        harness::topology::decline_report()
    );
    assert_eq!(
        census.window,
        LOOPED_PAIR_WINDOW_DECLINES,
        "the window-scan decline population moved (measured {:?}, pinned \
         {:?}) — same rule, both directions. A shrink is the interesting one: it \
         would mean the OpenDSS engines' overlapping-window scan stopped dropping \
         straddling pairs, which is the OpenDSS bug the normalization is built on \
         (`topology_pins::looped_pairs_lose_the_straddling_window`).\n{}",
        census.window,
        LOOPED_PAIR_WINDOW_DECLINES,
        harness::topology::decline_report()
    );
}

// ---------------------------------------------------------------------------
// The incidence-matrix surface: its forced population and the manifest rows
// that carry the declaration into the anti-shrink lock
// ---------------------------------------------------------------------------

/// Apply the incidence-forcing rule to a live case.
///
/// The surface is the four flat incidence quantities the dss-python harness
/// dumps for `Solution`: `IncMatrix`, `Laplacian`, `IncMatrixCols`,
/// `IncMatrixRows`, built by `CalcIncMatrix` / `CalcLaplacian`. On capi 0.14.5
/// the two integer getters allocate one cell too many, which the capi capture
/// drops after asserting it is zero.
///
/// Two neighbours of that group are deliberately never requested on either
/// channel. `BusLevels` writes one element past its array in r4133 and
/// therefore sits on the bridge's do-not-call register
/// (`crates/dss-epri/src/modes.rs`). The ordered builder rebuilds and
/// re-memoizes the branch tree the two topology decline censuses are defined
/// on. The flat builder is what this surface drives, and the golden `*_org_*`
/// stems keep covering the ordered one.
///
/// The rule is **every live case whose `kind` does not start with `large`**,
/// deliberately the same population [`force_properties`] and [`force_topology`]
/// force, so the gate keeps reasoning about one forced set
/// ([`FORCED_INC_MATRIX_POPULATION`] asserts all three stay equal). The cost
/// guard is what buys the surface: the live `large*` decks are a small slice of
/// the corpus but over half of the capi engine's wall time and payload, and
/// nothing caches the matrix: the pair rebuilds it from scratch on every call,
/// on both OpenDSS engines and on this engine.
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
/// [`the_inc_matrix_forcing_rule_is_every_live_non_large_case`] on every run: the
/// live cases minus the `kind=large*` decks. Identical to
/// [`FORCED_TOPOLOGY_POPULATION`] and [`FORCED_PROPS_POPULATION`] by
/// construction, and the test asserts those equalities instead of leaving them a
/// comment.
///
/// The lock it backs up is `population.lock.json`, which records the **manifest**
/// flag (`population_lock.rs::rigor`'s `incm=` token) and cannot see the
/// scheduler-side forcing at all, the same blind spot the property and topology
/// rules carry. It also fixes the incidence decline population:
/// `harness::inc_matrix`'s `INC_UPSTREAM_ROW_DECLINES` is measured over exactly
/// this set, so narrowing it moves that constant too.
const FORCED_INC_MATRIX_POPULATION: (usize, usize, usize, usize) = (444, 312, 87, 45);

/// **The manifest rows that declare `compare_inc_matrix` themselves**, with the
/// gating channel each one carries.
///
/// The forcing rule above is invisible to `population.lock.json`, so without at
/// least one declared row the surface would arm the whole gate while leaving the
/// anti-shrink lock byte-identical, and switching it off again would leave no
/// diff either. These decks are the surface's witnesses: every gating channel is
/// represented (`capi_v0145`, `r4133`, `both`), and each row's step-0 answer is
/// non-trivial in at least one of the four quantities:
///
/// * `asymmetric:capacitor/midi_capacitor_asym.dss`: `both`, all four row
///   builders (lines, transformers, series capacitors, series reactors) fire in
///   one deck.
/// * `asymmetric:reactor/reactor_asym.dss`: `both`, the row-cursor witness: its
///   shunt `Reactor.rsh` still advances the OpenDSS engines' row cursor.
/// * `solvable_now:Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss`:
///   `both`, multi-step, the deck the other live surfaces are also declared on.
/// * `solvable_now:Version8/Distrib/Examples/HarmonicsTMode/IEEE_519.DSS`: the
///   `capi_v0145`-only channel.
/// * `controls:fuse/indmach_r4133/indmach_dyn.dss`: the `r4133`-only channel.
/// * `solvable_now:Test/CableParameters.dss`: `both`, and the only declared deck
///   with an **empty** `IncMat`: the raw payload is r4133's bare `[0]` sentinel
///   or capi's lone unwritten extra cell, and `IncMatrixRows` arrives as `['']`
///   on capi and `['None']` on r4133. The sentinel path is therefore declared,
///   not merely forced.
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
/// arming half of `harness::inc_matrix`'s decline census, read HERE because the
/// manifest reader is this binary's and the harness library cannot name it,
/// the same split as the topology predicate in
/// [`assert_topology_declines_are_the_pinned_population`].
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

/// The incidence-forcing rule forces exactly the live non-`large` cases, the
/// same population as the topology and property surfaces. The live census sees
/// only the shape of a re-mask, not its size, so a narrowed rule reds only here.
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
        "the incidence-forcing rule is `every live non-`large` case` — these cases \
         disagree with it:\n  {}",
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
        "the incidence surface is forced over the SAME population as the topology surface (one \
         rule, one set to reason about — `force_inc_matrix`'s doc). If that is deliberately no \
         longer true, narrow one rule, drop this assertion, and re-measure \
         `INC_UPSTREAM_ROW_DECLINES`, which is a function of this population."
    );
    assert_eq!(
        FORCED_INC_MATRIX_POPULATION, FORCED_PROPS_POPULATION,
        "…and the same population as the property surface, which is where the `live non-`large`` \
         rule came from (the property census). Three rules, one forced set."
    );
}

/// The manifest rows that declare `compare_inc_matrix` are exactly
/// [`INC_MATRIX_DECLARED_IN_MANIFEST`], at least one per gating channel. Only a
/// declared row reaches `population.lock.json`, so deleting one fails here, never
/// silently.
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
             the flag must be set on at least one case per gating \
             channel, so that each channel's capture path is exercised by a declared row and not \
             only by the scheduler's forcing rule"
        );
    }
}

/// **The run's created file set.** Force `compare_run_files` on every live case
/// whose `kind` does not start with `large`.
///
/// The surface is the set of filesystem entries the run
/// (`clear -> compile -> post -> n_steps x solve`) creates under the case
/// directory - the case dir's own entries plus everything under a directory the
/// run created - as `/`-joined, ASCII-case-folded relative names, a trailing `/`
/// marking a created directory (`harness::run_files`, over the classification
/// `dss_epri::guard::CorpusGuard::classify` and its Python twin
/// `tools/oracle/corpus_guard.py`). It is well defined because `Compile` points
/// the output directory at the deck's own folder and every executive writer
/// prefixes its name with it.
///
/// The dss-python reference harness does not gate it: it archives every `*.csv`
/// under the run dir and skips a name the other side does not have, so a file
/// one engine writes and the other does not is invisible there.
///
/// The rule is **every live case whose `kind` does not start with `large`**,
/// deliberately the same population [`force_properties`], [`force_topology`] and
/// [`force_inc_matrix`] force, so the gate keeps reasoning about one forced set
/// ([`FORCED_RUN_FILES_POPULATION`] asserts all four stay equal). The cost is a
/// recursive listing of the case dir: free on the engine side (both guards
/// already walk that tree to snapshot and to sweep, and report the
/// classification) and one walk before and one after on this engine's side,
/// which is why the `large*` decks, whose directories hold the corpus's biggest
/// file trees, stay out with the rest of the one forced set.
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
/// [`the_run_files_forcing_rule_is_every_live_non_large_case`] on every run: the
/// live cases minus the `kind=large*` decks. Identical to
/// [`FORCED_INC_MATRIX_POPULATION`], [`FORCED_TOPOLOGY_POPULATION`] and
/// [`FORCED_PROPS_POPULATION`] by construction, and the test asserts those
/// equalities instead of leaving them a comment.
///
/// The lock it backs up is `population.lock.json`, which records the **manifest**
/// flag (`population_lock.rs::rigor`'s `runf=` token) and cannot see the
/// scheduler-side forcing at all, the same blind spot the other three rules
/// carry. The engine-scratch decline census
/// (`harness::run_files::scratch_census`) is measured over exactly this set, so
/// narrowing the rule re-measures it too.
const FORCED_RUN_FILES_POPULATION: (usize, usize, usize, usize) = (444, 312, 87, 45);

/// **The manifest rows that declare `compare_run_files` themselves**, with the
/// gating channel each one carries.
///
/// The forcing rule above is invisible to `population.lock.json`, so without at
/// least one declared row the surface would arm the whole gate while leaving the
/// anti-shrink lock byte-identical, and switching it off again would leave no
/// diff either. These decks are the surface's witnesses: every gating channel is
/// represented (`capi_v0145`, `r4133`, `both`). The two engines' raw spellings
/// differ and fold onto one member, so one set of names describes both:
///
/// * `solvable_now:Test/REACTORTest.DSS`: `both`, files from `Show`/`Dump` over
///   the deck's two circuits, the plainest non-monitor writer set in the corpus.
/// * `solvable_now:Version8/Distrib/IEEETestCases/NEVTestCase/Run_NEV.dss`:
///   `both`, one created name is the engines' internal harmonics scratch
///   `nev_savedvoltages.dbl`, split off symmetrically and counted: the declared
///   witness of that normalization.
/// * `solvable_now:Version8/Distrib/Examples/StoCtrl_SeasonTarget/Run_example.dss`:
///   `both`, the created **directory** convention: `ieee13nodecktmod/`,
///   `ieee13nodecktmod/di_yr_0/` and the DI CSVs under it are all members. It
///   is also a deck the DI contents surface reads.
/// * `solvable_now:Test/DistanceRelayTest.DSS`: the `r4133`-only channel, the
///   deck's own `export eventlog`.
/// * `solvable_now:Version8/Distrib/Examples/HarmonicsTMode/IEEE_519.DSS`: the
///   `capi_v0145`-only channel, and the **empty** set: the deck's writers and the
///   harmonics scratch all target files that are vendored in the corpus, so
///   nothing is created. It is a real witness all the same: the presence rail
///   (`capture_guard::require_capture_opt`) fails the case if the capi transport
///   answers `None`, so this row is what proves that channel's capture path is
///   wired. Every capi-only live case that writes a file writes into vendored
///   names.
/// * `solvable_now:Version8/Distrib/Examples/CIM/IEEE13_CDPSM.dss`: `both`,
///   `kind=large`, so it is reached only by the manifest declaration and not by
///   [`force_run_files`]: the CIM export (r4133 spells it `.XML` and the fold
///   makes the two channels agree). It proves the flag itself works where the
///   force rule does not reach.
///
/// No family manifest declares the flag: no `asymmetric`/`controls`/`modes` deck
/// issues a file-writing command at all, so a family row would declare an
/// unconditionally empty set.
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

/// The run-file forcing rule forces exactly the live non-`large` cases plus the
/// declared rows on top of them. A comparison that never runs leaves no trace
/// anywhere, so a narrowed rule reds only here.
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
        "the run-file forcing rule is `every live non-`large` case`, plus whatever \
         `RUN_FILES_DECLARED_IN_MANIFEST` adds - these cases disagree with it:\n  {}",
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
         a comparison that does not run reports nothing. It also re-measures the \
         scratch-file decline census, which was derived over exactly this population. A \
         legitimate corpus change moves this lock together with `population.lock.json`."
    );
    assert_eq!(
        FORCED_RUN_FILES_POPULATION, FORCED_INC_MATRIX_POPULATION,
        "the run-file surface is forced over the SAME population as the incidence surface \
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
         non-`large`` rule came from (the property census). Four rules, one forced set."
    );
}

/// The manifest rows that declare `compare_run_files` are exactly
/// [`RUN_FILES_DECLARED_IN_MANIFEST`], at least one per gating channel. Only a
/// declared row reaches `population.lock.json`, so deleting one fails here, never
/// silently.
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
             the flag must be set on at least one case per \
             gating channel, so that each channel's capture path is exercised by a declared row \
             and not only by the scheduler's forcing rule"
        );
    }
}

/// **The contents of the run-created demand-interval tree.** Force `compare_di`
/// on **every live case**, with no `kind` test.
///
/// The surface is every file of the `DI_yr_<year>` tree an EnergyMeter writes
/// when a deck enables `Set DemandInterval=`, compared cell by cell at the
/// calibrated tier each column's physical quantity already has
/// (`harness::di::compare_di`).
///
/// Unlike [`force_run_files`] the rule carries **no `large` test**, so this is
/// the one forced surface whose population is not the shared `live non-`large``
/// set: most of the decks that produce a DI tree are `kind=large`
/// (`EPRITestCircuits/ckt5`, `ckt7`, `Examples/StoCtrl_Current_PeakShave`),
/// so the `large` test would silently drop them. The rule costs, on the two
/// engine transports, one `/di_yr_` test per created name
/// (`dss_epri::guard::is_di_member`) over a walk their guards already do: a case
/// with no DI tree pays a string test and answers the empty set. On this
/// engine's side it is not free: [`super::runner`] builds its `RunFileProbe`
/// whenever `compare_run_files || compare_di`, so the live `large*` decks
/// outside [`FORCED_RUN_FILES_POPULATION`] get a directory snapshot before the
/// run and a classification and sweep after it, a few seconds per lane per full
/// gate. It also makes this engine sweep its own droppings on those decks, so a
/// leak there fails the case loudly instead of being swept by the outer guard
/// alone. That is also why [`FORCED_DI_POPULATION`] is deliberately not folded
/// into the four-way equality below: it is a different population, and saying
/// so out loud is the point.
///
/// The dss-python reference harness does not gate the contents either: it skips
/// a name the other side does not have and does not fail on a cell mismatch.
///
/// A manifest may also declare the flag itself, which only ever ADDS and is the
/// only way the surface reaches `population.lock.json`
/// ([`DI_DECLARED_IN_MANIFEST`]).
fn force_di(c: &mut SolvableCase) {
    c.compare_di = true;
}

/// **The forced demand-interval population, pinned** - `(cases forced, of them
/// `engines: "both"`, `engines: "r4133"`, `engines: "capi_v0145"`)`.
///
/// Re-derived from the four manifests by
/// [`the_di_forcing_rule_is_every_live_case`] on every run: every live case.
/// Bigger than [`FORCED_RUN_FILES_POPULATION`] by exactly the live `kind=large*`
/// decks, which is the whole point of the rule (see [`force_di`]).
///
/// The lock it backs up is `population.lock.json`, which records the
/// **manifest** flag (`population_lock.rs::rigor`'s `di=` token) and cannot see
/// the scheduler-side forcing at all - the same blind spot the other rules
/// carry, and the reason this one has a re-derivation test too.
const FORCED_DI_POPULATION: (usize, usize, usize, usize) = (523, 366, 101, 56);

/// **The manifest rows that declare `compare_di` themselves** - the five decks
/// that enable demand-interval metering - with the gating channel each carries.
///
/// The forcing rule above is invisible to `population.lock.json`, so without
/// declared rows the surface would arm the whole gate while leaving the
/// anti-shrink lock byte-identical. These rows are the surface's witnesses, and
/// they are exactly the corpus's DI producers, derived from the decks, not
/// chosen, by [`every_deck_that_enables_demand_interval_declares_compare_di`]:
///
/// * `EPRITestCircuits/ckt5/Run_ckt5.dss`: a yearly run on a large feeder.
/// * `EPRITestCircuits/ckt7/RunDSS_ckt7.dss`: the one case carrying ledger rows
///   on this surface (the `kvarh` cancellation and the `Min LV Bus` argmin tie,
///   `tests/corpus/ledger.json`).
/// * `Examples/StoCtrl_Current_PeakShave/master.dss`: one of the cases whose run
///   leaves the DI streams re-opened, which is what makes the capture slot
///   contractual (`capture_order::check_run_tail_order`).
/// * `Examples/StoCtrl_SeasonTarget/Run_example.dss`: also the run-file
///   surface's declared witness for the created-directory convention.
/// * `IEEETestCases/123Bus/Run_YearlySim.dss`: the smallest producer and the
///   only one whose tree hangs under `di_yr_1`.
///
/// All five gate on `both`, so both gating channels carry a declared row; there
/// is no single-channel DI deck to declare (the census below re-derives that
/// from the corpus on every run rather than trusting this list).
const DI_DECLARED_IN_MANIFEST: &[(&str, &str)] = &[
    (
        "solvable_now:Version8/Distrib/EPRITestCircuits/ckt5/Run_ckt5.dss",
        "both",
    ),
    (
        "solvable_now:Version8/Distrib/EPRITestCircuits/ckt7/RunDSS_ckt7.dss",
        "both",
    ),
    (
        "solvable_now:Version8/Distrib/Examples/StoCtrl_Current_PeakShave/master.dss",
        "both",
    ),
    (
        "solvable_now:Version8/Distrib/Examples/StoCtrl_SeasonTarget/Run_example.dss",
        "both",
    ),
    (
        "solvable_now:Version8/Distrib/IEEETestCases/123Bus/Run_YearlySim.dss",
        "both",
    ),
];

/// The demand-interval forcing rule forces every live case, with the split
/// pinned by [`FORCED_DI_POPULATION`]. A rule narrowed to the declared cases
/// would stay green everywhere else while the other cases stopped proving they
/// write no DI tree.
#[test]
fn the_di_forcing_rule_is_every_live_case() {
    let cases = build_unified_cases();
    let mut forced = (0usize, 0usize, 0usize, 0usize);
    let mut wrong: Vec<String> = Vec::new();
    for uc in &cases {
        let expected = uc.class == CaseClass::Live;
        if uc.case.compare_di != expected {
            wrong.push(format!(
                "{}: kind={} engines={} class={} -> compare_di={} (expected {expected})",
                uc.label,
                uc.case.kind,
                uc.case.engines,
                if expected { "live" } else { "not-live" },
                uc.case.compare_di,
            ));
        }
        if uc.case.compare_di {
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
        "the demand-interval forcing rule is `every live case` - these cases disagree \
         with it:\n  {}",
        wrong.join("\n  ")
    );
    assert_eq!(
        forced, FORCED_DI_POPULATION,
        "(forced, both, r4133-only, capi-only) moved. A DROP in either half re-masks the \
         demand-interval request on that channel - invisible to every other gate, because a \
         comparison that does not run reports nothing. A legitimate corpus change moves this \
         lock together with `population.lock.json`."
    );
    assert!(
        FORCED_DI_POPULATION.0 > FORCED_RUN_FILES_POPULATION.0,
        "the DI rule has no `large` test, so its population must exceed the shared \
         `live non-`large`` set; if the two are equal the `large` decks - most of the DI \
         producers - have silently dropped out"
    );
    // …and how many producers a `large` test would gate, derived from the
    // manifest instead of written into a doc, and failing on stale in both
    // directions.
    let producer_kinds: Vec<&str> = cases
        .iter()
        .filter(|uc| DI_TREE_CASES.contains(&uc.label.as_str()))
        .map(|uc| uc.case.kind.as_str())
        .collect();
    let large = producer_kinds
        .iter()
        .filter(|k| k.starts_with("large"))
        .count();
    assert_eq!(
        (producer_kinds.len(), large),
        (DI_TREE_CASES.len(), 3),
        "the DI producers' kinds are {producer_kinds:?}: a `large` test on the \
         forcing rule would drop {large} of them"
    );
}

/// The manifest rows that declare `compare_di` are exactly
/// [`DI_DECLARED_IN_MANIFEST`], covering both gating channels. Only a declared
/// row reaches `population.lock.json`, so deleting one fails here, never
/// silently.
#[test]
fn the_di_surface_is_declared_on_every_gating_channel() {
    let mut declared: Vec<(String, String)> = Vec::new();
    for c in load_solvable() {
        if c.compare_di {
            declared.push((format!("solvable_now:{}", c.path), c.engines.clone()));
        }
    }
    for fam in FAMILIES {
        for c in load_family(fam.name) {
            if c.compare_di {
                declared.push((format!("{}:{}", fam.name, c.path), c.engines.clone()));
            }
        }
    }
    declared.sort();
    let mut want: Vec<(String, String)> = DI_DECLARED_IN_MANIFEST
        .iter()
        .map(|(l, e)| ((*l).to_string(), (*e).to_string()))
        .collect();
    want.sort();
    assert_eq!(
        declared, want,
        "the manifest declarations of `compare_di` moved. They are what puts the surface into \
         `population.lock.json` (the `di=` rigor token) - the scheduler-side `force_di` is \
         invisible to it - so this set is pinned, and a change to it belongs in the same commit \
         as a regenerated lock."
    );
    for ch in ["capi_v0145", "r4133"] {
        assert!(
            declared.iter().any(|(_, e)| e == ch || e == "both"),
            "no manifest case declares `compare_di` on the {ch:?} channel; \
             the flag must be set on at least one case per gating \
             channel, so that each channel's capture path is exercised by a declared row and not \
             only by the scheduler's forcing rule"
        );
    }
}

/// Every live case whose deck (or anything it `Redirect`s / `Compile`s) enables
/// `Set DemandInterval=` declares `compare_di`, and nothing else does. A new DI
/// deck would otherwise be compared but never recorded by the anti-shrink lock.
#[test]
fn every_deck_that_enables_demand_interval_declares_compare_di() {
    // The scanner and its own non-vacuity drive live in `manifest.rs`
    // (`manifest::deck_enables_demand_interval`, its
    // `the_demand_interval_scanner_reads_the_option_and_its_abbreviation`).
    let mut enabling: Vec<String> = Vec::new();
    for uc in build_unified_cases() {
        if uc.class != CaseClass::Live {
            continue;
        }
        if crate::manifest::case_enables_demand_interval(&uc.abs) {
            enabling.push(uc.label.clone());
        }
    }
    enabling.sort();
    let mut declared: Vec<String> = DI_DECLARED_IN_MANIFEST
        .iter()
        .map(|(l, _)| (*l).to_string())
        .collect();
    declared.sort();
    assert_eq!(
        enabling, declared,
        "the live cases whose deck enables demand-interval metering are no longer exactly the \
         cases that declare `compare_di`. A deck that enables it writes a DI tree the gate \
         compares, so its case must carry the declaration (that is what reaches \
         `population.lock.json`); a declaration on a deck that does not enable it would pin an \
         empty surface."
    );
}

/// **The population where an OpenDSS engine writes an engine-internal scratch
/// file this engine, by design, never writes**, as `(cases, names)`.
///
/// Both OpenDSS engines round-trip the fundamental solution through disk when a
/// harmonics run starts: they write
/// `<OutputDirectory><CircuitName_>SavedVoltages.dbl` and read it back. This
/// engine keeps that state in memory
/// (`crates/dss-core/src/solution/solution/state.rs`,
/// `solution/solution/harmonics.rs`) and writes no such file, so the name is
/// **not** a divergence to triage but an artifact of the engines' internal
/// plumbing. `harness::run_files::compare_run_files` splits it off both sides
/// (asserting this engine's side of the split is empty, so an engine that ever
/// starts writing one is a real change and not a normalization) and counts it
/// here. No `ledger.json` rows: a row per harmonics deck would be a dozen
/// entries for one structural cause.
///
/// The pin is that census, re-derived on every full gate run and failing in
/// both directions: a bigger population means a new name started being split
/// off with nobody looking, a smaller one means the normalization now covers
/// less than it claims. Both numbers of one row are pinned literally by
/// `run_files_pins::the_harmonics_scratch_file_is_declined_on_the_nev_deck`.
///
/// It is read off completed full drives only: the epilogue that calls the
/// assertion below (`corpus_gate.rs`) runs only after the per-case failure
/// report, so a drive with any red case never reaches it. The population is
/// every case that starts a harmonics run (the `modes:harmonics/*` decks,
/// `modes:inputformat/xycurve_files` and the `solvable_now:` harmonics decks),
/// each declining exactly one `<CircuitName_>savedvoltages.dbl`, identical on
/// the two lanes.
///
/// `(cases, names)` and not `(cases, names, visits)`: the census key is
/// `(case label, scratch name)` with no channel in it, so a `both` case that
/// declines the same name on both channels is one row either way, and the
/// process-wide visit counter counts every caller in the process, not only the
/// gate's cases. That is why the population is read off the decline TABLE, and
/// every label in it must be one the manifests carry: the harness's `unit:`
/// fixtures run in its own lib test binary, so a row of any other label is a
/// foreign producer and fails.
const SCRATCH_FILE_DECLINES: (usize, usize) = (9, 9);

/// The engine-scratch census, re-derived from this run and pinned in both
/// directions: the [`assert_topology_declines_are_the_pinned_population`] shape
/// for a normalization that deliberately writes no ledger rows.
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
        // Every row is a manifest case's: no fixture runs in this process, so a
        // typo'd or foreign label is never absorbed into the pinned population.
        let gate_rows: Vec<_> = table.iter().collect();
        let foreign: Vec<&String> = gate_rows
            .iter()
            .map(|(case, _)| *case)
            .filter(|case| !gate_labels.contains(case.as_str()))
            .collect();
        assert!(
            foreign.is_empty(),
            "the engine-scratch census carries the label(s) {foreign:?}, which are \
             not manifest cases. The pinned population is a fact about the corpus, \
             so an unrecognized producer fails instead of being counted or dropped."
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
            "corpus_gate run files: engine-scratch declines {measured:?} \
             ({} channel visit(s) and {} set compare(s) process-wide)\n  {}",
            census.visits,
            census.compared,
            report.join("\n  ")
        );
        assert_eq!(
            measured,
            SCRATCH_FILE_DECLINES,
            "the engine-scratch decline population moved (measured {:?}, \
             pinned {:?}). It is re-derived on every run and fails in BOTH \
             directions: a bigger population means a case started having a name \
             split off its created-file SET with nobody looking, a smaller one \
             means the normalization now covers less than it claims. Re-measure, \
             move the constant, and re-read the pin \
             `run_files_pins::the_harmonics_scratch_file_is_declined_on_the_nev_deck`.\
             \n  {}",
            measured,
            SCRATCH_FILE_DECLINES,
            report.join("\n  ")
        );
        assert!(
            measured.0 > 0,
            "the engine-scratch census counted nothing on a run where live cases \
             request `compare_run_files`. The pinned population would then be a \
             statement about nothing - the surface is masked off somewhere \
             between the manifest and the transports."
        );
    }
}
// ---------------------------------------------------------------------------
// The run-file contents census.
// ---------------------------------------------------------------------------

/// What one `(case, channel)` contents comparison measured, as the gating call
/// site saw it.
///
/// Recorded in `corpus_gate/runner.rs` from the `CellTally`
/// `harness::run_file_contents::compare_run_file_cells` hands back, and never
/// inside that comparator (the [`harness::record_seq_arm`] precedent): the
/// `harness::run_file_contents`
/// fixtures call the comparator too — several of them on the very
/// Storage-trace kind the read-back tail below is derived from — and a
/// comparator-side census would read the gating population *plus* whatever
/// else ran in the process. Counting at the call site makes the population
/// gate-only by construction, wherever a fixture runs and under either runner,
/// which lets the constants below be pinned exactly.
#[cfg(windows)]
#[derive(Clone, Debug)]
pub(crate) struct RunFileContentsRow {
    /// The manifest case label, so a moved census names the deck.
    case: String,
    /// `capi_v0145` / `r4133` — the gating channel this comparison ran against.
    channel: &'static str,
    /// Report files whose contents were compared cell by cell.
    files: usize,
    /// Cells compared under the `case floor + print ulp` rule.
    compared: usize,
    /// Cells the column map declined: the Storage trace's `%-.g` columns, which
    /// the two OpenDSS engines spell differently from each other, and an angle
    /// whose paired magnitude is below the case's own current floor, where the
    /// angle carries no information.
    declined: usize,
    /// Files accounting a reader read-back tail
    /// (`ReportKind::accounts_readback_tail` — today the Storage `DebugTrace`).
    trace_files: usize,
    /// `engine_rows − this_engine_rows` summed over those files.
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

/// `(cases, file comparisons, compared cells)`: the run-file contents surface's
/// non-vacuity census, re-derived on every full run and pinned in both
/// directions.
///
/// A comparison census is the only thing that keeps a contents surface from
/// silently comparing nothing, and a census that compares no cell on a gating
/// channel fails. A smaller population means a report stopped travelling (a
/// selection pattern
/// that no longer matches, a deck that stopped exporting, a transport that
/// stopped copying — and a bigger one means a kind started being compared with
/// nobody choosing its policy.
///
/// The key is `(case, channel)`, so a `both` case that exports a report
/// contributes one file comparison per channel; `cases` counts the distinct
/// manifest labels behind them.
///
/// It is read off completed full drives in both lanes: the forced
/// `engines: "both"` decks that export a report this surface has a policy for
/// (`Test/PVSystemTest.dss`, `…/WampServer/testcommandline.dss`,
/// `…/NEVTestCase/Run_NEV.dss`, `…/8500-Node/Run_8500Node.dss`,
/// `…/8500-Node/Run_8500Node_Unbal.dss`, `…/GFM_IEEE8500/Run_8500Node_Unbal.dss`)
/// plus `…/StorageTechNote/Example_9_3_Price/Storage_price.dss` for the Storage
/// `DebugTrace`. The epilogue runs only after the per-case failure report, so a
/// drive with any red case never reaches it.
#[cfg(windows)]
const RUN_FILE_CONTENTS_COMPARED: (usize, usize, usize) = (7, 32, 3_035_190);

/// `(cases, (case, channel) comparisons, declined cells)` — the same census for
/// the cells the column maps deliberately do NOT compare.
///
/// Counted in cells, not names: the declined *names* are a static property of
/// the column maps
/// (`harness::run_file_contents::PrintFmt::DeclinedG`), pinned where they are
/// declared (`the_trace_layout_follows_its_own_header`) and by the both-numbers
/// pin `the_two_sig_trace_columns_are_declined_on_both_channels`; what a run
/// can silently change is how many cells fall into them, which is what this
/// census pins.
///
/// Read off the same drives as [`RUN_FILE_CONTENTS_COMPARED`]: the Storage
/// trace's declined columns over its compared records plus the below-floor angle
/// cells of the 8500-Node decks. The two channels can differ on one deck because
/// the arm is two-sided (`|I| = max(|I_engine|, |I_this|)`), so which cells fall
/// below the case's own current floor is a property of the pair, not of this
/// engine alone.
#[cfg(windows)]
const RUN_FILE_CONTENTS_DECLINES: (usize, usize, usize) = (4, 8, 14_832);

/// `(files accounting a tail, records per file)`: the Storage `DebugTrace`
/// read-back tail.
///
/// The OpenDSS engines write a trace record on every terminal-current read,
/// outside their current cache, so the file's row count is a property of the
/// reader as well as of the run. The two engine transports read the element
/// several times after the last solve and append a record each time. This
/// engine's `exec::view::snapshot_elements` recomputes once at the converged
/// node voltages, so its file is the engine's minus a constant tail, never
/// longer (`compare_one` refuses that outright), and its solve records are
/// compared cell by cell.
///
/// Pinned per (case, channel) row as `tail == files × records` and in both
/// directions. There is one trace file per row, so that is per file. A gap that
/// grows is this engine no longer writing solve records or a transport that grew
/// an element read. A gap that shrinks is this engine logging the gate's own
/// capture. A file count that moves is the trace comparison going quiet (or a
/// second deck starting one whose tail nobody measured). The per-file detail
/// (case, channel, both row counts) is in
/// `harness::run_file_contents::trace_tail_census`, and the pin
/// `run_file_contents_pins::the_storage_trace_tail_is_the_readers_footprint`
/// holds both row counts of the one trace deck.
#[cfg(windows)]
const TRACE_READBACK_RECORDS: (usize, usize) = (2, 4);

/// The run-file contents census, re-derived from this run and pinned in both
/// directions: the [`assert_scratch_declines_are_the_pinned_population`] shape
/// for a surface that deliberately writes no ledger rows.
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
        // Per channel, because the non-vacuity rule is per gating channel: a
        // surface that compares everything on one channel and nothing on the
        // other is exactly as masked as one that compares nothing at all.
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
        // files matched and engine bytes decoded, process-wide (every caller in
        // the process included, which is why it is REPORTED and never asserted;
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
            "the run-file contents census carries the label(s) {foreign:?}, which are \
             not manifest cases. The recorder runs only at the gate's own call \
             site, so an unrecognized producer fails instead of being counted."
        );
        assert_eq!(
            compared,
            RUN_FILE_CONTENTS_COMPARED,
            "the run-file CONTENTS population moved (measured {:?}, pinned \
             {:?}). It is re-derived on every run and fails in BOTH directions: a \
             smaller population means a report stopped travelling (a selection \
             pattern that no longer matches, a deck that stopped exporting, a \
             transport that stopped copying), a bigger one means a kind started \
             being compared without anyone choosing its policy. Re-measure off a \
             COMPLETED full drive, move the constant, and re-read the pins in \
             `run_file_contents_pins`.\n  {}",
            compared,
            RUN_FILE_CONTENTS_COMPARED,
            report.join("\n  ")
        );
        assert_eq!(
            declines,
            RUN_FILE_CONTENTS_DECLINES,
            "the declined-cell population moved (measured {:?}, pinned \
             {:?}). The declined SET is static (the column maps' \
             `PrintFmt::DeclinedG` and the below-floor angle arm), so a moved cell \
             count means a file grew or lost rows, or a column changed class. \
             Re-measure off a COMPLETED full drive and move the constant.\n  {}",
            declines,
            RUN_FILE_CONTENTS_DECLINES,
            report.join("\n  ")
        );
        // This loop walks the channels that PRODUCED rows: a channel that
        // produced none passes it vacuously and is caught by the exact
        // `RUN_FILE_CONTENTS_COMPARED` triple above instead (a silent channel
        // halves `compared`). Stated because the message below promises more
        // than the loop alone delivers.
        for (channel, (files, compared_cells, _)) in &per_channel {
            assert!(
                *files > 0 && *compared_cells > 0,
                "the run-file contents census compared {compared_cells} cell(s) over \
                 {files} file(s) on the gating channel `{channel}`. A surface that \
                 is live on one channel and silent on the other is masked: re-read \
                 the selection \
                 (`dss_epri::guard::RUN_FILE_CONTENTS_PATTERNS`), that transport's \
                 copy step and the sidecar read."
            );
        }
        for r in rows.iter().filter(|r| r.trace_files > 0) {
            assert_eq!(
                r.trace_tail,
                r.trace_files * TRACE_READBACK_RECORDS.1,
                "{} [{}]: the Storage `DebugTrace` read-back tail is {} record(s) \
                 over {} file(s), not the pinned {} per file. The engine's file is \
                 this engine's plus the records the transport's OWN post-solve \
                 element reads appended — a moved gap is this engine no longer \
                 writing solve records, a transport that grew an element read, or \
                 this engine logging the gate's own capture. Re-measure and move \
                 the constant and the pin \
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

/// **The demand-interval census**, as
/// `(cases with a non-empty DI tree, (case, channel) comparisons, files
/// compared, cells compared)`.
///
/// The non-vacuity half of the surface: a comparator that compared nothing reports
/// nothing, so the four numbers are re-derived from THIS run and pinned in
/// BOTH directions. A smaller `comparisons` is a re-masked request (the
/// forcing rule narrowed, a channel dropped); a smaller `files`/`cells` is a
/// selection or a class table that reaches less than it claims; a bigger one is
/// a producer nobody triaged.
///
/// It is read off completed full drives in both lanes: the epilogue that calls
/// the assertion below (`corpus_gate.rs`) runs only after the per-case failure
/// report, so a drive with any red case never reaches it.
///
/// Each number is derivable, which is what makes a move readable rather than a
/// mystery:
///
/// * **comparisons** = the live (case, channel) pairs of
///   [`FORCED_DI_POPULATION`] minus the channel-level `skip` entries in
///   `tests/corpus/ledger.json`, whose channel never runs the case at all. Every
///   live case answers the request, which is the point: a case that writes no DI
///   tree answers the empty tree, and that answer is compared.
/// * **files** = the DI files of the producers in [`DI_TREE_CASES`], on both of
///   their channels (`engines: "both"` on all of them).
/// * **cells** = every cell of those files minus the ledger-excluded ckt7
///   columns (`kvarh` and `min lv bus`), which are not counted because they are
///   not compared.
///
/// `cases with a tree` is pinned by label in [`DI_TREE_CASES`], which is where
/// the one `closedi` deck that never enables `Set DemandInterval=`
/// ([`DI_FREE_CLOSEDI_CASE`]) is pinned DI-free: no DI file is written without
/// the option. It is still compared (its empty tree counts among the
/// comparisons), which is what makes the pin a measurement rather than an
/// exemption.
const DI_TREE_CENSUS: (usize, usize, usize, usize) = (5, 884, 72, 9_793_064);

/// The case labels that produced a demand-interval tree — the identity half of
/// [`DI_TREE_CENSUS`] (a count alone would let one producer replace another).
/// The accounting behind it is keyed by (label, channel) and each producer's
/// channel set is asserted against
/// its manifest `engines`, so a channel compared twice — which leaves every
/// total in [`DI_TREE_CENSUS`] unchanged — fails too.
const DI_TREE_CASES: [&str; 5] = [
    "solvable_now:Version8/Distrib/EPRITestCircuits/ckt5/Run_ckt5.dss",
    "solvable_now:Version8/Distrib/EPRITestCircuits/ckt7/RunDSS_ckt7.dss",
    "solvable_now:Version8/Distrib/Examples/StoCtrl_Current_PeakShave/master.dss",
    "solvable_now:Version8/Distrib/Examples/StoCtrl_SeasonTarget/Run_example.dss",
    "solvable_now:Version8/Distrib/IEEETestCases/123Bus/Run_YearlySim.dss",
];

/// The case that issues `closedi` and writes no DI tree, pinned by name so
/// [`DI_TREE_CENSUS`]'s producer count is a statement about this deck too.
const DI_FREE_CLOSEDI_CASE: &str =
    "solvable_now:Version8/Distrib/Examples/Scripts/Storage-Quasi-Static-Example/Run_Demo1.dss";

/// The demand-interval census, re-derived from this run and pinned in both
/// directions:
/// the [`assert_scratch_declines_are_the_pinned_population`] shape for the
/// surface's non-vacuity rail.
///
/// Silent in the two documented situations that shape has: `DSS_GATE_ONLY` is
/// set (a filtered run holds a filtered population), and no live manifest case
/// requests `compare_di` at all (a fact re-read from the manifests here, not
/// assumed, so the assertion arms itself the moment the surface is forced or
/// declared).
pub(crate) fn assert_di_census_is_the_pinned_population() {
    if std::env::var("DSS_GATE_ONLY").is_ok() {
        return;
    }
    let cases = build_unified_cases();
    let requested = cases
        .iter()
        .filter(|uc| uc.class == CaseClass::Live)
        .any(|uc| uc.case.compare_di);
    #[cfg(not(windows))]
    {
        assert!(
            !requested,
            "`compare_di` is Windows-only (its `r4133` transport and the created-file \
             classification live in the `#[cfg(windows)]` comparator `harness::di`), yet a \
             live case requests it"
        );
    }
    #[cfg(windows)]
    {
        let gate_labels: std::collections::BTreeSet<&str> =
            cases.iter().map(|uc| uc.label.as_str()).collect();
        // Every row is a manifest case's: no fixture runs in this process, so a
        // typo'd or foreign label is never absorbed into the pinned population
        // (the engine-scratch census's rule, one surface over).
        let gate_rows = harness::di::di_account();
        let foreign: Vec<&String> = gate_rows
            .iter()
            .map(|((label, _), _)| label)
            .filter(|label| !gate_labels.contains(label.as_str()))
            .collect();
        assert!(
            foreign.is_empty(),
            "the demand-interval census carries the label(s) {foreign:?}, which are \
             not manifest cases. The pinned population is a fact about the corpus, so an \
             unrecognized producer fails instead of being counted or dropped."
        );
        // One row per (case, channel), so the producer list is the distinct
        // labels — the rows are sorted by label
        // first, so consecutive dedup is exact.
        let mut with_tree: Vec<&str> = gate_rows
            .iter()
            .filter(|(_, (_, files, _))| *files > 0)
            .map(|((label, _), _)| label.as_str())
            .collect();
        with_tree.dedup();
        // …and each producer must carry exactly its own gating channels: a
        // channel SWAP (the same case compared twice on one channel) leaves
        // every total below unchanged and is invisible without this.
        let mut channels: std::collections::BTreeMap<&str, Vec<&str>> =
            std::collections::BTreeMap::new();
        for ((label, channel), (_, files, _)) in &gate_rows {
            if *files > 0 {
                channels
                    .entry(label.as_str())
                    .or_default()
                    .push(channel.as_str());
            }
        }
        let wrong_channels: Vec<String> = channels
            .iter()
            .filter_map(|(label, got)| {
                let engines = cases
                    .iter()
                    .find(|uc| uc.label == *label)
                    .map(|uc| uc.case.engines.as_str())
                    .unwrap_or("");
                let want: Vec<&str> = match engines {
                    "both" => vec!["capi_v0145", "r4133"],
                    "r4133" => vec!["r4133"],
                    _ => vec!["capi_v0145"],
                };
                (*got != want).then(|| format!("{label}: {got:?}, expected {want:?}"))
            })
            .collect();
        assert!(
            wrong_channels.is_empty(),
            "the demand-interval tree of these producers was not compared on \
             exactly their gating channels — a channel compared twice or not at \
             all leaves the census totals below untouched, so it is asserted \
             here:\n  {}",
            wrong_channels.join("\n  ")
        );
        let measured = (
            with_tree.len(),
            gate_rows.iter().map(|(_, (c, _, _))| *c).sum::<usize>(),
            gate_rows.iter().map(|(_, (_, f, _))| *f).sum::<usize>(),
            gate_rows.iter().map(|(_, (_, _, n))| *n).sum::<usize>(),
        );
        let excluded = harness::di::di_excluded_columns();
        let report: Vec<String> = gate_rows
            .iter()
            .filter(|(_, (_, files, _))| *files > 0)
            .map(|((label, channel), (c, f, n))| {
                format!("{label} [{channel}] -> {c} comparison(s), {f} file(s), {n} cell(s)")
            })
            .collect();
        if !requested {
            assert_eq!(
                measured,
                (0, 0, 0, 0),
                "no manifest case requests `compare_di`, yet the demand-interval comparator \
                 ran on real gate cases:\n  {}",
                report.join("\n  ")
            );
            eprintln!(
                "corpus_gate demand interval: the surface is not requested by any live case \
                 - nothing to re-derive"
            );
            return;
        }
        eprintln!(
            "corpus_gate demand interval: census {measured:?}; \
             ledger-excluded column(s): {excluded:?}\n  {}",
            report.join("\n  ")
        );
        assert_eq!(
            with_tree, DI_TREE_CASES,
            "the cases that produce a demand-interval tree moved. The tree exists exactly \
             where a deck enables `Set DemandInterval=` \
             (`every_deck_that_enables_demand_interval_declares_compare_di`), so a case \
             appearing here is a new producer nobody triaged and a case disappearing is a \
             producer that went silent - both are corpus facts that move the constant."
        );
        assert!(
            !with_tree.contains(&DI_FREE_CLOSEDI_CASE),
            "{DI_FREE_CLOSEDI_CASE} issues `closedi` without ever enabling \
             `Set DemandInterval=`, so `SaveDemandInterval` is false and no DI file is \
             written - a tree appearing there means this engine or an OpenDSS engine \
             started writing demand-interval files without the option"
        );
        assert_eq!(
            measured,
            DI_TREE_CENSUS,
            "the demand-interval census moved (measured {:?}, pinned {:?}). It is \
             re-derived on every run and fails in BOTH directions: fewer comparisons means the \
             request was re-masked somewhere between the manifest and the transports, fewer \
             files or cells means the selection or the class table reaches less than it claims, \
             and more of either means a producer or a column nobody triaged. Re-measure on a \
             COMPLETED full drive, move the constant, and re-read the pins \
             `di_pins::the_ckt7_hourly_kvarh_is_a_cross_engine_indeterminate` and \
             `di_pins::the_ckt7_min_lv_bus_is_an_argmin_over_a_tie`.\n  {}",
            measured,
            DI_TREE_CENSUS,
            report.join("\n  ")
        );
        assert!(
            measured.3 > 0,
            "the demand-interval census compared ZERO cells on a run where live cases request \
             `compare_di`. The pinned population would then be a statement about nothing - \
             the surface is masked off somewhere between the manifest and the comparator."
        );
    }
}

/// Build one unified case, applying the per-source forcing rules and the case
/// classification.
fn make_case(
    order: usize,
    source: &str,
    mut c: SolvableCase,
    abs: String,
    fam_props: bool,
) -> UnifiedCase {
    // `pending` (this engine must error loudly, no OpenDSS engine runs) and
    // `expect_solve_abort` (both engines must abort the solve) are contradictory
    // contracts. The classifier below resolves ties pending-first, so without
    // this a both-flags manifest case would silently skip the stronger abort
    // contract. Applied to every source.
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
        force_di(&mut c);
    }
    let weight = kind_weight(&c.kind) * (c.n_steps.max(1) as u64);
    UnifiedCase {
        order,
        label: format!("{source}:{}", c.path),
        abs,
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

/// Every case is its own task, even two manifest rows in one case directory
/// (they run in two scratch copies, so nothing couples them), and the tasks
/// drain heaviest first, ties in label order.
#[test]
fn every_case_is_its_own_task_heaviest_first() {
    let mk = |order: usize, abs: &str, weight: u64| UnifiedCase {
        order,
        label: format!("fixture:{abs}"),
        abs: abs.to_string(),
        weight,
        class: CaseClass::Live,
        case: SolvableCase::default(),
    };
    // The `IEEETestCases/8500-Node` shape: several rows in one folder (one of
    // them spelled with a different case) plus one row in another.
    let tasks = build_tasks(
        vec![
            mk(0, "C:/corpus/8500-Node/Run_8500Node.dss", 10),
            mk(1, "C:/corpus/8500-Node/P174_Run_Voltage_Profile.DSS", 30),
            mk(2, "C:/corpus/8500-node/Master-unbal.dss", 10),
            mk(3, "C:/corpus/AutoTrans/Auto3bus.dss", 20),
        ],
        None,
    );
    assert!(
        tasks.iter().all(|t| t.cases.len() == 1),
        "a task is exactly one case"
    );
    assert_eq!(
        tasks.iter().map(|t| t.cases[0].order).collect::<Vec<_>>(),
        vec![1, 3, 2, 0],
        "heaviest first; equal weights in label order"
    );
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

/// One task per case, sorted longest-first (or shuffled).
fn build_tasks(cases: Vec<UnifiedCase>, shuffle_seed: Option<u64>) -> Vec<Task> {
    let mut tasks: Vec<Task> = cases
        .into_iter()
        .map(|uc| Task {
            key: uc.label.to_lowercase(),
            weight: uc.weight,
            cases: vec![uc],
        })
        .collect();
    match shuffle_seed {
        None => {
            // Longest-first; tie-break on the case label for determinism.
            tasks.sort_by(|a, b| b.weight.cmp(&a.weight).then(a.key.cmp(&b.key)));
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
    /// Raw engine result JSON (live cases only) — retained only when dumping,
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
            // Pending runs no OpenDSS engine (this engine must error loudly).
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                assert_pending_errors_loudly(&uc.label, &uc.abs, &uc.case);
            }));
            match res {
                Ok(()) => ok_outcome(None),
                Err(e) => fail_outcome(panic_msg(e), None),
            }
        }
        CaseClass::Deferred => {
            // Parked from the live compare: smoke-run on this engine only.
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                assert_deferred_rust_smoke(&uc.label, &uc.abs, &uc.case);
            }));
            match res {
                Ok(()) => ok_outcome(None),
                Err(e) => fail_outcome(panic_msg(e), None),
            }
        }
        CaseClass::Abort => {
            // Abort against every gating channel.
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
            // Live-compare against every gating channel. This engine is re-run
            // per channel inside `compare_with_result`, so `both` runs it twice:
            // the seam is intentional. A ledger `skip` entry drops a
            // hard-crash channel; the ledger's field scopes partition the compare.
            let mut dump_val: Option<Value> = None;
            for ch in uc.case.engine_channels() {
                if ctx.ledger.channel_is_skipped(&uc.label, ch) {
                    continue;
                }
                let channel = ctx.channel(uc, ch);
                // Both channels see the same case: the r4133 request carries the
                // same all-properties capture the capi one does
                // (`force_properties`, `dss-epri::capture`). The clone is what
                // the compare closure below owns.
                let cc = uc.case.clone();
                // The channel compiles its own fresh scratch copy; the request's
                // sidecars stay keyed by the vendored deck.
                let (copy, req) = match channel_copy(uc, channel.tag(), &cc) {
                    Ok(v) => v,
                    Err(e) => return fail_outcome(format!("[{ch:?}] {e}"), dump_val),
                };
                let resp = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    channel.call(&req)
                })) {
                    Ok(r) => r,
                    Err(e) => {
                        return fail_outcome(
                            format!("[{ch:?}] engine fetch panicked: {}", panic_msg(e)),
                            dump_val,
                        );
                    }
                };
                if let Err(e) = copy.remove() {
                    return fail_outcome(format!("[{ch:?}] {e}"), dump_val);
                }
                if !resp.ok {
                    return fail_outcome(
                        format!("[{ch:?}] engine case failed: {:?}", resp.error),
                        dump_val,
                    );
                }
                let Some(val) = resp.result else {
                    return fail_outcome(
                        format!("[{ch:?}] engine ok response missing result"),
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

/// The channel's fresh scratch copy of `uc` and the run request that points
/// its producer at it, `catch_unwind`-isolated: a copy that cannot
/// be made fails the case with the copy's own message, never the scheduler
/// thread that runs it.
fn channel_copy(
    uc: &UnifiedCase,
    tag: &'static str,
    cc: &SolvableCase,
) -> Result<(ScratchCopy, Value), String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let copy = ScratchCopy::new(&uc.abs, tag);
        let req = build_run_request(copy.deck(), &uc.abs, cc);
        (copy, req)
    }))
    .map_err(panic_msg)
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
    /// per-entry hit accounting.
    pub(crate) ledger: Arc<LedgerRuntime>,
    /// What changed under `tests/corpus/` between the photograph [`run_gate`]
    /// takes before its first case and the one it takes after its last
    /// ([`crate::scratch::TreePhoto::changes`]); empty on a healthy run.
    pub(crate) tree_changes: Vec<String>,
}

impl GateRun {
    /// Every manifest case produced an outcome (a worker thread that panicked
    /// outside `catch_unwind` drops one), and the vendored tree is exactly as
    /// the walk found it (the read-only-tree rail).
    /// Every gate producer runs in its own scratch copy and every corpus guard
    /// brackets that copy, so a listing or mtime change during the walk is a
    /// producer writing into the vendored tree again, or a tree writer in
    /// another test process that happened to overlap the walk (a writer outside
    /// the walk's window is not seen here). It is printed and fails the gate
    /// with the failed cases listed next to it, so neither hides the other.
    pub(crate) fn assert_complete(&self) {
        assert_eq!(
            self.outcomes.len(),
            self.total,
            "scheduler dropped case outcomes ({} of {} collected) — a worker thread \
             panicked outside catch_unwind",
            self.outcomes.len(),
            self.total
        );
        if self.tree_changes.is_empty() {
            return;
        }
        let failed: Vec<&str> = self
            .outcomes
            .iter()
            .filter(|o| !o.ok)
            .map(|o| o.label.as_str())
            .collect();
        let shown: Vec<&str> = self
            .tree_changes
            .iter()
            .take(40)
            .map(String::as_str)
            .collect();
        let msg = format!(
            "corpus_gate: the vendored tree tests/corpus/ changed during the gate walk \
             ({} entr(ies); `+` appeared, `-` vanished, `~` changed, and a folder's `~` \
             is an entry created or removed in it): {}{}. Every producer runs in a \
             scratch copy, so this is a producer writing into \
             the vendored tree: run it through `harness::scratch::ScratchCopy`, never \
             exclude the entry. {} case(s) failed in the same run: [{}]",
            self.tree_changes.len(),
            shown.join(", "),
            if self.tree_changes.len() > shown.len() {
                ", ..."
            } else {
                ""
            },
            failed.len(),
            failed.join(", "),
        );
        eprintln!("{msg}");
        panic!("{msg}");
    }
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

    // Photograph the vendored tree before the first case;
    // `GateRun::assert_complete` compares it with the one taken after the last.
    let corpus = crate::scratch::corpus_root();
    let tree_before = crate::scratch::TreePhoto::take(&corpus);
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
        // leftover exported env var would otherwise silently neuter the gate).
        assert!(
            !cases.is_empty(),
            "DSS_GATE_ONLY={only:?} matched no case labels — refusing to green a \
             zero-case gate run; fix or unset the filter"
        );
    }
    let total = cases.len();

    // Which transports does the case set actually need? A case can gate the
    // capi_v0145 channel, the r4133 channel, or both; `isolate`/serial routes an
    // engine run through a throwaway one-shot instead of the pool. Only
    // Live/Abort cases actually drive an engine channel (Pending runs no OpenDSS
    // engine, Deferred is a smoke run of this engine only).
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

    // `for_spec(None)` constructs the pinned one-shot engine AND ping-verifies it;
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
    let tree_changes = tree_before.changes(&crate::scratch::TreePhoto::take(&corpus));

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
        tree_changes,
    }
}

// ---------------------------------------------------------------------------
// Seeding report mode (DSS_GATE_SEED_LEDGER).
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
    // Seed only Live cases (Pending runs no OpenDSS engine; Abort has its own contract) —
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
    let channel = ctx.channel(uc, ch);
    // A seeding measurement sees exactly what the gate compares, on both
    // channels. The clone is what the compare closure below owns.
    let cc = uc.case.clone();
    let (copy, req) = match channel_copy(uc, channel.tag(), &cc) {
        Ok(v) => v,
        Err(e) => return mk("error", e),
    };
    let resp = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| channel.call(&req))) {
        Ok(r) => r,
        Err(e) => return mk("error", format!("fetch panic: {}", panic_msg(e))),
    };
    if let Err(e) = copy.remove() {
        return mk("error", e);
    }
    if !resp.ok {
        return mk("error", format!("engine: {:?}", resp.error));
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
// Property census mode (DSS_PROPS_CENSUS).
// ---------------------------------------------------------------------------

/// Walk every live case on both channels with `all_properties` forced on,
/// ignoring the case's own `engines` key, and collect every divergent cell
/// instead of asserting. Writes `tmp/props_census.json` plus the per-channel
/// census extracts ([`crate::props_census`]); **asserts nothing about the
/// data**.
///
/// It reproduces the population of the census vendored at
/// `tests/corpus/props_r4133/`: live cases only (`pending`/`abort`/`defer_ledger`
/// have their own contracts and no property capture), `large`-kind decks
/// excluded (they are excluded from property forcing on the gate too,
/// [`force_properties`], and the vendored census carries none), the case's own
/// `engines` key ignored: a capi-only deck is still measured against r4133,
/// which is where the census's GenDispatcher and Sensor shape rows come from.
///
/// The normal gate path is untouched: nothing here runs unless the env var is
/// set. What separates this walk from the gate is that it ignores `engines` and
/// asserts nothing.
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
    // The claims mode's last chain link: a `property`-scoped
    // ledger entry NAMES a cell. It is loaded SEPARATELY from `Ctx.ledger`,
    // which stays empty on purpose — the walk must keep measuring the raw
    // divergence (the plain baseline, and a `skip` entry must not hide a case),
    // while the disposition query answers "would an entry claim this cell?".
    // Only the non-asserting `property_scope_keys` is consulted, so a
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
/// failure mode — a dead engine, a crashing deck, a panicking Rust walk —
/// becomes a ROW, never a panic. The second return value is what the walk could
/// not look at (see [`harness::CensusBlindSpots`]).
///
/// `mode` and `claims_ledger` are the disposition mode's two extras: in
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
    // The in-scope filter: a case the gate compares on r4133 is one whose
    // manifest entry gates that channel, read off the case's own `engines` key.
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

    let channel = ctx.channel(uc, ch);
    let mut cc = uc.case.clone();
    // The knob's whole point: the property capture is requested on BOTH channels,
    // regardless of the case's own `engines` key.
    cc.compare_all_properties = true;
    let (copy, req) = match channel_copy(uc, channel.tag(), &cc) {
        Ok(v) => v,
        Err(e) => return oracle_error(e),
    };
    let resp = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| channel.call(&req))) {
        Ok(r) => r,
        Err(e) => return oracle_error(format!("fetch panic: {}", panic_msg(e))),
    };
    if let Err(e) = copy.remove() {
        return oracle_error(e);
    }
    if !resp.ok {
        return oracle_error(format!("{:?}", resp.error));
    }
    let Some(val) = resp.result else {
        return oracle_error("ok response missing result".to_string());
    };

    // Deserialization happens OUTSIDE the Rust-side `catch_unwind` on purpose: a
    // channel that answers `ok: true` with a payload the harness cannot parse is
    // an engine-side defect, and in a mode whose only diagnostic is the row kind
    // it must not be filed against this engine.
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
        // This engine's own scratch copy, removed when the walk ends.
        let copy = ScratchCopy::new(&abs, PORT);
        let (mut dss, _baseline) = run_rust_capture(&label, copy.deck(), &cc);
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
        // A copy this engine still holds after its walk is a leak on the census
        // as on the gate: the walk's RustError row names the producer.
        drop(dss);
        copy.finish();
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

/// [`GateRun::assert_complete`] passes a complete run whose tree photographs
/// agree, and fails one whose `tree_changes` is non-empty, naming every change
/// and the cases that failed in the same run (and none that passed). The
/// photograph itself is pinned by
/// `scratch::tests::a_tree_photograph_sees_every_way_a_producer_touches_the_tree`.
#[test]
fn a_changed_vendored_tree_fails_the_gate_naming_the_failed_cases() {
    let run = |tree_changes: Vec<String>| GateRun {
        outcomes: vec![
            CaseOutcome {
                order: 0,
                label: "fixture:red.dss".to_string(),
                ok: false,
                reason: "a fixture failure".to_string(),
                result: None,
            },
            CaseOutcome {
                order: 1,
                label: "fixture:green.dss".to_string(),
                ok: true,
                reason: String::new(),
                result: None,
            },
        ],
        elapsed: Duration::ZERO,
        mode: "fixture".to_string(),
        jobs: 1,
        pool_size: 2,
        total: 2,
        ledger: Arc::new(LedgerRuntime::empty()),
        tree_changes,
    };
    run(Vec::new()).assert_complete();
    let caught = std::panic::catch_unwind(|| {
        run(vec![
            "~electricdss-tst/Test".to_string(),
            "+electricdss-tst/Test/left.csv".to_string(),
        ])
        .assert_complete()
    });
    let msg = panic_msg(caught.expect_err("a changed vendored tree must fail the gate"));
    assert!(
        msg.contains("the vendored tree tests/corpus/ changed during the gate walk")
            && msg.contains("~electricdss-tst/Test")
            && msg.contains("+electricdss-tst/Test/left.csv")
            && msg.contains("fixture:red.dss")
            && !msg.contains("fixture:green.dss"),
        "{msg}"
    );
}

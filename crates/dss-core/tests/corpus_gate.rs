//! Unified live corpus gate (`UNIFIED_GATE_PLAN.md`; successor of `corpus_live.rs`,
//! git-renamed to preserve history). For each solvable case the gate runs the
//! **Rust** engine once and compares it against the **pinned dss-python oracle**
//! (`capi_v0145` channel, served by a persistent worker pool) — node order, full
//! system Y, node voltages, every element's currents/powers/losses, YPrim blocks,
//! injection, discrete control state, monitors/meters, and the opt-in element
//! channels — per step, reusing `harness/mod.rs`. The `r4133` channel is served
//! by the in-house `dss-epri` bridge pool (`epri-worker`); `engines:"both"`
//! cases gate on BOTH channels, partitioned by the divergence ledger (Phase C/D
//! — the pre-Phase-C one-shot target-rev Oracle path is retired).
//!
//! Phase B (this file's module tree): the four pre-existing live-compare tests
//! (`corpus_live_solvable_cases_match_oracle` + `{asymmetric,controls,modes}_
//! cases_match_oracle`) are unified into ONE scheduler-driven `#[test]`
//! (`corpus_gate_all_cases_match_engines`) with persistent oracle worker pools,
//! a hand-rolled priority thread pool, per-case `catch_unwind`, and an aggregate
//! failure report. Case populations, comparisons, tolerances, and iteration
//! policies are UNCHANGED. Structural manifest guards, the opt-in report tests,
//! and the A-Diakoptics sweep are preserved (relocated, not changed).
//!
//! Submodules: `manifest` (schema + loading + family completeness), `engines`
//! (one-shot Oracle + persistent WorkerPool + Channel), `runner`
//! (`run_rust_capture`/`compare_capture` + abort/pending + CorpusGuard),
//! `scheduler` (task grouping + thread pool + contamination-proof modes).

mod harness;

// The gate's own module tree lives under `tests/corpus_gate/` (a subdirectory,
// so cargo does not pick the pieces up as separate integration-test binaries);
// `#[path]` is required because this crate-root file resolves plain `mod` names
// against `tests/` directly.
#[path = "corpus_gate/engines.rs"]
mod engines;
#[path = "corpus_gate/ledger.rs"]
mod ledger;
#[path = "corpus_gate/manifest.rs"]
mod manifest;
#[path = "corpus_gate/props_census.rs"]
mod props_census;
#[path = "corpus_gate/runner.rs"]
mod runner;
#[path = "corpus_gate/scheduler.rs"]
mod scheduler;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use num_complex::Complex64;
use serde::Deserialize;
use serde_json::{Value, json};

use dss_core::exec::Dss;
use dss_core::solution::ControlMode;

use engines::Oracle;
use manifest::{
    AD_OFF_REASONS, FAMILIES, SolvableCase, ad_disposition_is_valid, corpus_file, family_file,
    load_family, load_solvable, manifests_dir,
};
use runner::{CorpusGuard, panic_msg, run_and_compare};
use scheduler::{CaseOutcome, GateRun, run_gate};

// ===========================================================================
// The unified mandatory gate.
// ===========================================================================

/// The whole live corpus gate: every case from all four manifests, run on the
/// persistent pinned worker pool (+ one-shot target-rev shim), compared with the
/// unchanged mandate. Fails iff any case failed, printing the complete list.
#[test]
fn corpus_gate_all_cases_match_engines() {
    // Seeding report mode (§4 Phase D step 2): run every case against BOTH
    // channels regardless of its `engines`, measure the raw divergence, and write
    // candidate ledger entries — never asserting. Consumed by hand for triage.
    if std::env::var("DSS_GATE_SEED_LEDGER").is_ok() {
        scheduler::seed_ledger();
        return;
    }
    let run = run_gate();

    if let Ok(path) = std::env::var("DSS_GATE_DUMP") {
        write_gate_dump(&path, &run);
    }

    let failures: Vec<&CaseOutcome> = run.outcomes.iter().filter(|o| !o.ok).collect();
    eprintln!(
        "corpus_gate [{}]: {}/{} case(s) passed, {} failed, jobs={} pool={} in {:.1}s",
        run.mode,
        run.total - failures.len(),
        run.total,
        failures.len(),
        run.jobs,
        run.pool_size,
        run.elapsed.as_secs_f64(),
    );
    // Ledger hit accounting (§1.3): report every entry with its hit count.
    let hits = run.ledger.hit_report();
    if !hits.is_empty() {
        let total_hits: usize = hits.iter().map(|(_, _, _, n)| n).sum();
        eprintln!(
            "corpus_gate ledger: {} entry(ies), {total_hits} total hit(s):",
            hits.len()
        );
        for (id, ch, kind, n) in &hits {
            eprintln!("  {id} [{ch:?} {kind}]: {n} hit(s)");
        }
    }
    // The bus surface's one structural normalization, said out loud (G1.4a,
    // coordinator decision D11(2)): on a case whose `voltages` field is already
    // ledger-excluded, `harness::compare_bus` drops the three continuous per-bus
    // arrays — exact images of the node-voltage band over the same
    // `Solution.NodeV` — because re-raising that one triaged cause per bus would
    // mean ten new ledger rows for a divergence already pinned. Bus count, name
    // sequence, `nodes`, `kv_base` and every array length stay compared there.
    // G1.5 extends the same flag to exactly two more arrays and no further:
    // `harness::compare_bus_short_circuit` drops the `Voc` and `Isc` VALUES —
    // `Voc` is a copy of that same `NodeV` (`Common/Solution.pas:4070-4083`) and
    // `Isc = Ysc·Voc` (`SolutionAlgs.pas:785-796`) — while `Zsc`/`Ysc`/`Zsc1`/
    // `Zsc0` stay fully compared, being functions of `Y` alone and independent
    // of the solution vector that was triaged.
    // It is not a mask and must never read as one, so every suppressed case is
    // listed next to the entry that caused it
    // (`ledger::LedgerRuntime::bus_array_suppressions`, driven both ways by
    // `a_suppressed_bus_array_is_named_with_the_entry_that_caused_it`).
    let suppressed = run.ledger.bus_array_suppressions();
    if !suppressed.is_empty() {
        eprintln!(
            "corpus_gate bus arrays suppressed (D11(2)) on {} (case, channel) pair(s) — \
             count/names/nodes/kv_base still compared:",
            suppressed.len()
        );
        for (case, id, ch) in &suppressed {
            eprintln!("  {case} [{ch:?}]: by ledger entry `{id}`");
        }
    }
    assert_eq!(
        run.outcomes.len(),
        run.total,
        "scheduler dropped case outcomes ({} of {} collected) — a worker thread \
         panicked outside catch_unwind",
        run.outcomes.len(),
        run.total
    );
    if !failures.is_empty() {
        let mut msg = format!(
            "corpus_gate [{}]: {} of {} case(s) failed:\n",
            run.mode,
            failures.len(),
            run.total
        );
        for f in &failures {
            let first = f.reason.lines().next().unwrap_or("<no message>");
            msg.push_str(&format!("  {}\n      {first}\n", f.label));
        }
        // Printed BEFORE the panic on purpose (G1.5 audit settlement): a panic
        // message travels through the process-global panic hook, which another
        // test in this binary may have replaced while it drives an expected
        // failure (`harness`' own `reds`). The run report is the only diagnosis
        // a red gate leaves behind — a 2026-09-05 red printed the summary and
        // the ledger table but no case list at all — so it goes out on its own.
        eprintln!("{msg}");
        panic!("{msg}");
    }
    // Fail-on-stale (§1.3 runtime rule / §5 R3): every applicable ledger entry
    // must have been hit within its envelope; a stale/unhit entry fails the gate.
    // Only checked once all cases passed — a failing case may not have reached its
    // ledger scope, which would produce misleading staleness noise. A partial run
    // (`DSS_GATE_ONLY`, a dev filter — never the commit gate) skips the check: an
    // unhit entry there only means its case was filtered out, not that it is stale.
    if std::env::var("DSS_GATE_ONLY").is_err()
        && let Err(stale) = run.ledger.assert_all_hit()
    {
        panic!("{stale}");
    }
    // The same discipline for the Stage F default-lane event-log re-round
    // cells: each one exempts a rendered number from the oracle compare, so one
    // that stopped occurring must fail rather than quietly become a no-op.
    // Self-silencing under `DSS_GATE_ONLY` and in the parity lane.
    harness::lane::assert_reround_cells_are_live();
    // And the population fact behind GOLDEN_REBASE G1.3d(ii)'s D-ii-1 costing
    // ZERO ledger rows: no gated element carries two controls, so r4133's
    // per-edit `Set_ControlledElement` re-attach (which the port follows) can
    // never reorder a list against capi 0.14.5's here. Re-derived from the
    // oracle's own `NumControls` on every full run, and loud in both directions
    // (a two-control element, or a census that counted nothing) — the D15/D16
    // shape, added by the G1.3d(ii) audit settlement.
    harness::assert_no_multi_control_element();
    // And for G2.4's monitor-channel normalization, which needs it for the
    // opposite reason: since both lanes' engines now report the empty channel, a
    // client that stopped padding would make the transform a silent no-op rather
    // than a loud mismatch. Self-silencing when no unflushed monitor was
    // compared, so `DSS_GATE_ONLY` runs do not trip it.
    harness::lane::assert_monitor_pad_is_live();
    // And the same fail-on-stale discipline for GOLDEN_REBASE G1.7's two
    // topology settlements (coordinator decisions D15/D16), which deliberately
    // write NO ledger rows: the populations where the comparator rebases the
    // isolation half onto the port's step-0 answer (upstream's memoized tree)
    // and where upstream's window scan drops a looped pair are re-derived from
    // this run and must equal their pinned constants in both directions. Placed
    // with the other live rails, i.e. after the per-case failures are reported:
    // a failing case may not have reached its topology compare, and a census
    // measured from a partial run would be noise on top of a real failure.
    scheduler::assert_topology_declines_are_the_pinned_population();
    // And the GLOBAL half of the r4133 property accounting (plan §RP4.1): the
    // two per-row asserts BELOW say nothing when NO row was visited, which is
    // exactly what a re-mask of the r4133 property request would produce — a
    // silent, green gate. This one fails when the whole r4133 property compare
    // never ran, counted at the single gating call site
    // (`harness::compare_all_properties`'s r4133 arm) rather than off the
    // tables, whose statics sibling unit tests in this binary legitimately move.
    // It is the loud half of "a scheduler-side re-mask is invisible to
    // `population.lock.json`"; the other half is the landed `property`-scoped
    // r4133 entries going NEVER APPLIED in `assert_all_hit` above, and the
    // static third is `scheduler::the_property_forcing_rule_is_every_live_non_
    // large_case`, which also catches a PARTIAL re-mask this boolean cannot see.
    //
    // It runs BEFORE the two per-row guards (RP4.1 audit settlement,
    // 2026-09-03): "did the compare run at all" is the precondition that makes
    // their per-row verdicts mean anything, and running it first keeps a
    // wholesale re-mask reporting as one line instead of 19 rows of
    // "narrowed row never visited".
    let (props_walks, props_elements) = harness::props_norm::r4133_props_walk_counters();
    eprintln!("corpus_gate r4133 props: {props_walks} gating walk(s), {props_elements} element(s)");
    harness::props_norm::assert_r4133_props_compare_ran();
    // And for the RP2.1 r4133 property-normalization rows, for the first
    // reason: each row lets the engine spell a property value differently from
    // r4133, so one that stops folding anything must fail rather than sit in
    // the table. **Live since RP4.1** (2026-09-03) — the r4133 props path is
    // no longer masked (`corpus_gate/scheduler.rs::force_properties`), so a full
    // gate run visits these rows for real; it was wired dormant at RP2.1 so the
    // flip would arm it instead of having to remember it. It skips a
    // `DSS_GATE_ONLY` run by an explicit check rather than by zero visits, which
    // is where it parts company with the two above: a row spans cases, so a
    // filtered run can visit one without reaching the case that makes it fold
    // (RP4.1 measured exactly that).
    harness::props_norm::assert_norm_rows_are_live();
    // And for the RP2.3 r4133 property-ECHO rows, for the same reason with a
    // sharper edge: each row stops the r4133 value compare of a whole pair, so
    // one that excludes nothing is a mask over a divergence that is no longer
    // there. Live on the same schedule (it was zero-visit until RP4.1's unmask),
    // skipping a `DSS_GATE_ONLY` run the same explicit way, plus silent for the
    // two rows whose cited cells sit on capi-only cases
    // (`props_norm::ECHO_ROWS_WITH_NO_IN_SCOPE_CELL`). For the 20 rows narrowed
    // to measured spellings the staleness signal is inverted — visits == 0 while
    // the seam ran (RP4.1 audit settlement) — because a narrowed row can only
    // ever count a divergent cell.
    harness::props_norm::assert_echo_rows_are_live();
    // And the two GOLDEN_REBASE G1.6b PDElements guards, in that order for the
    // reason the property pair is in that order: "did the walk run at all, on
    // BOTH channels" is the precondition that makes the per-row verdicts mean
    // anything. The first one exists because the per-case rail cannot be a
    // count contract here — 96 of the 372 walked live capi cases legitimately
    // hold no PD element, so `[] == []` is a valid case outcome and a global
    // collapse to zero would be silently green. The second is the fail-on-stale
    // for `PD_SKIP_FIELDS`, whose eight rows each drop an oracle cell that is
    // read out of uninitialized memory; a row that stops excluding a divergence
    // is a mask over nothing. Both self-silence under `DSS_GATE_ONLY`.
    let (pd_cw, pd_ce, pd_rw, pd_re) = harness::pd_walk_counters();
    eprintln!(
        "corpus_gate PDElements: capi_v0145 {pd_cw} walk(s) / {pd_ce} element(s), \
         r4133 {pd_rw} walk(s) / {pd_re} element(s)"
    );
    for r in harness::PD_SKIP_FIELDS {
        let (v, h) = harness::pd_skip_counters(r.channel, r.class, r.field)
            .expect("every shipped row is findable by its own key");
        eprintln!(
            "corpus_gate PDElements skip {}/{}.{}: {v} visit(s), {h} hit(s)",
            r.channel, r.class, r.field
        );
    }
    harness::assert_pd_elements_compare_ran();
    harness::assert_pd_skip_rows_are_live();
    // And the two GOLDEN_REBASE G1.6(i) reliability guards, in the same order
    // and for the same reason: "did the surface run at all, on BOTH channels"
    // is what makes the per-row verdicts mean anything. The first one exists
    // because `compare_reliability` is a MANIFEST flag with no scheduler force
    // rule (the predicate "defines an EnergyMeter" is not a manifest field), so
    // losing the rows would silently compare nothing. The second is the
    // fail-on-stale for `RELIABILITY_SKIP_FIELDS`, whose four rows drop the two
    // `Meters` arrays both oracles read out of uninitialized memory until a deck
    // runs `AllocateLoads`; that table is policed on VISITS only - its `hits`
    // are printed here because a fresh `ReallocMem` region often reads back as
    // the port's own 0.0, which would make a hit rule fail at random. Both
    // self-silence under `DSS_GATE_ONLY`.
    let (rel_cw, rel_cm, rel_rw, rel_rm) = harness::reliability_walk_counters();
    eprintln!(
        "corpus_gate reliability: capi_v0145 {rel_cw} payload(s) / {rel_cm} meter(s), \
         r4133 {rel_rw} payload(s) / {rel_rm} meter(s)"
    );
    for r in harness::RELIABILITY_SKIP_FIELDS {
        let (v, h) = harness::reliability_skip_counters(r.channel, r.field)
            .expect("every shipped row is findable by its own key");
        eprintln!(
            "corpus_gate reliability skip {}/{}: {v} visit(s), {h} hit(s)",
            r.channel, r.field
        );
    }
    harness::assert_reliability_compare_ran();
    harness::assert_reliability_skip_rows_are_live();
    // And for G1.5's short-circuit surface, for the first reason again: its
    // content gate is `assert_eq!(port_ran, oracle_ran)`, which is equally
    // satisfied when NEITHER side ran a study, so a deck that quietly stopped
    // solving one would leave the whole non-trivial half of the surface
    // comparing sentinel lengths, `Zsc1`/`Zsc0` = 0 and a zero `Isc` — green
    // over nothing, and invisible to `population.lock.json` (which fingerprints
    // the manifest flag) and to `MODES_REQUIRED` (which fingerprints the deck
    // path). The pair is re-derived on every run and asserted exactly, so it
    // fails on a drop AND on a growth (G1.5 audit settlement, T1).
    let (sc_walks, sc_buses) = harness::sc_study_counters();
    eprintln!(
        "corpus_gate short-circuit: {sc_walks} walk(s) carrying a real matrix, \
         {sc_buses} bus(es) compared full"
    );
    harness::assert_sc_study_compare_ran();
    // And for G1.4b's distance surface, for the first reason once more: its
    // comparator is an exact equality over `DistFromMeter`, which is `0.0` on
    // every corpus case that defines no EnergyMeter — so a regression that
    // stopped the zone walk from writing distances at all would leave the whole
    // surface green (both sides reporting the meterless zero) on every case.
    // The pair is re-derived on every run and asserted exactly, failing on a
    // drop AND on a growth; `population.lock.json` cannot see it, because this
    // surface rides `compare_bus` and sets no manifest flag of its own.
    let (dist_walks, dist_buses) = harness::distance_counters();
    eprintln!(
        "corpus_gate distance: {dist_walks} walk(s) carrying a non-zero DistFromMeter, \
         {dist_buses} bus(es) compared non-zero"
    );
    harness::assert_distance_compare_ran();
    // And for G1.4c's four exception classes, for the SECOND reason: none of
    // them is excluded — each is closed by a POSITIVE assertion of the upstream
    // mechanism over the port's own state (`oracle == upstream_walk(port)`, the
    // D15/D16 settlement shape). A deck that stopped carrying its class would
    // leave that assertion running over nothing, and one that started carrying
    // it would go unnoticed, so the four `(walks, buses)` pairs are re-derived
    // on every run and asserted EXACTLY — failing on a drop AND on a growth.
    // Invisible to `population.lock.json`, which fingerprints the manifest flag:
    // this surface rides `compare_bus` and sets no flag of its own.
    let sv = harness::seq_vll_counters();
    eprintln!(
        "corpus_gate seq/vll: r4133 sentinel {:?}, ground substitution {:?}, \
         VLL pairing declines {:?}, r4133 hang refusals {:?} (walk(s), bus(es))",
        sv.r4133_seq_sentinel,
        sv.seq_ground_substitution,
        sv.vll_upstream_pairing_declines,
        sv.r4133_vll_hang
    );
    harness::assert_seq_vll_populations();
    // And for G1.4d's four at-bus divergence classes, for the SECOND reason
    // again: none of them is excluded either — each channel's own walk is
    // asserted POSITIVELY over the port's attachment facts, and where the port
    // and a channel disagree the disagreement is COUNTED. A deck that stopped
    // carrying its class would leave that assertion running over nothing, and
    // one that started carrying it would go unnoticed, so the four
    // `(walks, records)` pairs are re-derived on every run and asserted EXACTLY
    // — failing on a drop AND on a growth. Invisible to
    // `population.lock.json`, which fingerprints the manifest flag: this
    // surface rides `compare_bus` and sets no flag of its own.
    let ab = harness::at_bus_counters();
    eprintln!(
        "corpus_gate at-bus: PDE terminal-3 declines {:?}, capi node-ref drops {:?}, \
         capi node-ref adds {:?}, PCE declines {:?} (walk(s), (bus, element) record(s))",
        ab.pde_terminal3_declines, ab.capi_noderef_drops, ab.capi_noderef_adds, ab.pce_declines
    );
    harness::assert_at_bus_populations();
}

/// The property census (`R4133_PROPS_PLAN.md` RP0.2, `DSS_PROPS_CENSUS`): walk
/// every live case on BOTH channels with `all_properties` forced on, collect
/// every divergent cell and write `tmp/props_census.json` + the RP0.1 extracts.
/// Asserts nothing about the data — a divergence is the measurement, not a
/// failure.
///
/// It lives in its OWN `#[test]` rather than diverting the mandatory gate
/// (RP0.2 audit): dispatching `corpus_gate_all_cases_match_engines` on the env
/// var meant a stray `DSS_PROPS_CENSUS=1` in a shell or CI environment turned
/// the one mandatory live comparison into a green no-op. Here the var only ARMS
/// this test; the gate always runs the gate. Unset, this is a no-op that costs
/// nothing on `cargo test --workspace`.
///
/// Run it alone — the census is a full second pass over the corpus:
/// `DSS_PROPS_CENSUS=1 cargo test -p dss-core --test corpus_gate
/// corpus_gate_props_census -- --nocapture` (see `TESTING.md`).
#[test]
fn corpus_gate_props_census() {
    let Ok(raw) = std::env::var("DSS_PROPS_CENSUS") else {
        eprintln!(
            "props_census: DSS_PROPS_CENSUS unset — nothing measured. Set it to `1` to run \
             the plain census (R4133_PROPS_PLAN.md RP0.2)."
        );
        return;
    };
    scheduler::run_props_census(&raw);
}

/// Write the contamination-proof artifact: a label-sorted
/// `{label: {verdict, result}}` map (verdict = "ok" or the first failure line;
/// result = the gate-relevant oracle model for live cases). Three runs (serial
/// one-shot, persistent parallel, persistent parallel shuffled) must bit-diff
/// EMPTY.
///
/// Only the `harness::skip_prop_ub` (`SKIP_PROPS`) property VALUES are stripped
/// from each checkpoint's `all_properties` before dumping — NOT the whole block,
/// and deliberately **not** the Stage F lane exclusions, which are deterministic
/// and so belong in the bit-diff (that is why the predicate is `skip_prop_ub`
/// and not `skip_prop`). The UB core of that set — the DoubleSymMatrix
/// `RMatrix`/`XMatrix`/`CMatrix`/`GMatrix` pairs and the shunt-PD reliability
/// inputs — renders UNINITIALIZED heap memory in the upstream dss_capi
/// getter: a persistent worker's heap carries residue from prior cases where a
/// fresh process's is zeroed, so those garbage bytes are order-dependent BY
/// CONSTRUCTION — keeping them would make the bit-diff report the upstream UB, not
/// real contamination (this is the ONLY source of cross-process nondeterminism;
/// every other property is a deterministic function of the deck → byte-identical
/// across processes). The stripped set is exactly what the gate excludes from its
/// VALUE compare, so no gate-asserted property is dropped: every property the gate
/// checks is RETAINED, and the three-way bit-diff independently re-proves its
/// byte-stability across reused workers (closing the settle-B2/B3 completeness gap
/// where a within-tolerance oracle-property drift could otherwise escape both the
/// verdict channel and a whole-block strip). The property NAME is kept in all
/// cases (only the value string is nulled), preserving the property-index shape.
///
/// `SKIP_PROPS` is a SUPERSET of that UB core since the changed-default rows
/// (e)/(f) and RP3.8's live-render rows (g): those eight are deterministic on
/// both sides, so nulling them here is a deliberate, recorded loss rather than
/// the argument above (the twin note at `harness::skip_prop_ub` says the same).
/// **What that loss is, precisely** (RP3.8 audit settlement): `skip_prop_ub` is
/// channel-BLIND while those eight rows are `SKIP_PROPS_CAPI_ONLY` — the gate
/// *does* value-compare them on the `r4133` channel — so for r4133 cases the
/// "stripped set == the set the gate excludes" equivalence does NOT hold, and
/// five of the nulled properties (the StorageController aggregates) are exactly
/// the ones that sum ANOTHER class's arena, where a cross-case pointer or
/// ordering leak would show first. What still covers them: each of the three
/// runs value-compares them live on the r4133 channel and any divergence lands
/// in that run's `verdict` string, which IS in the bit-diff — so a real
/// contamination still fails the artifact. What is genuinely lost is the
/// narrower signal the strip was widened to keep: a *within-tolerance* drift of
/// those eight oracle-side renders across reused workers. Making the strip
/// channel-aware would restore it; that is the alternative, not a claim that
/// nothing was given up.
fn write_gate_dump(path: &str, run: &GateRun) {
    fn strip_ub_properties(v: &mut Value) {
        if let Some(cps) = v.get_mut("checkpoints").and_then(|c| c.as_array_mut()) {
            for cp in cps {
                let Some(elems) = cp.get_mut("all_properties").and_then(|a| a.as_array_mut())
                else {
                    continue;
                };
                for el in elems {
                    let class = el
                        .get("element")
                        .and_then(|e| e.as_str())
                        .and_then(|s| s.split('.').next())
                        .unwrap_or("")
                        .to_string();
                    let Some(props) = el.get_mut("props").and_then(|p| p.as_array_mut()) else {
                        continue;
                    };
                    for pair in props {
                        let Some(arr) = pair.as_array_mut() else {
                            continue;
                        };
                        let name = arr
                            .first()
                            .and_then(|n| n.as_str())
                            .unwrap_or("")
                            .to_string();
                        // Null the VALUE (keep the name) for the UB heap-garbage
                        // props the gate itself excludes from the value compare.
                        if arr.len() >= 2 && harness::skip_prop_ub(&class, &name) {
                            arr[1] = Value::Null;
                        }
                    }
                }
            }
        }
    }
    let mut entries: BTreeMap<String, Value> = BTreeMap::new();
    for o in &run.outcomes {
        let verdict = if o.ok {
            "ok".to_string()
        } else {
            // Only the first line — deterministic, path/thread-id free.
            o.reason.lines().next().unwrap_or("").to_string()
        };
        let mut result = o.result.clone().unwrap_or(Value::Null);
        strip_ub_properties(&mut result);
        let mut e = serde_json::Map::new();
        e.insert("verdict".to_string(), Value::String(verdict));
        e.insert("result".to_string(), result);
        entries.insert(o.label.clone(), Value::Object(e));
    }
    let text = serde_json::to_string_pretty(&entries).expect("serialize gate dump");
    if let Some(parent) = Path::new(path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(path, text).unwrap_or_else(|e| panic!("write gate dump {path}: {e}"));
    eprintln!(
        "corpus_gate [{}]: wrote contamination dump -> {path}",
        run.mode
    );
}

// ===========================================================================
// Capture-order contract (GOLDEN_REBASE_PLAN.md §1.1(a), coordinator decision
// D3; TESTING.md §"Capture order is contractual on the capi channel").
// ===========================================================================

/// The six per-bus reads, capi marker first, r4133 marker second — ONE table,
/// so the fixed order and the cross-transport correspondence are the same fact.
/// (`CAPI_Alt.pas:2143`/`Bus.kVBase`/`:2071`/`:2251`/`:2573`/`:2540` == r4133
/// `DDLL/DBus.pas:319`/`BUSF(0)`/`:122`/`:399`/`:659`/`:690`.)
///
/// `Distance` (GOLDEN_REBASE G1.4b) is read with the bus's other SCALAR
/// attribute and ahead of the value arrays on both transports — it is group C
/// like every other row here (`BUSF(5)` returns the stored `DistFromMeter` and
/// touches nothing), so its position is a cross-transport contract, not a
/// staleness rule.
const BUS_READ_ORDER: [(&str, &str); 6] = [
    ("b.Nodes", "engine.bus_nodes()"),
    ("b.kVBase", "engine.bus_kvbase()"),
    ("b.Distance", "engine.bus_distance()"),
    ("b.puVoltages", "engine.bus_pu_voltages()"),
    ("b.VMagAngle", "engine.bus_vmag_angle()"),
    ("b.puVmagAngle", "engine.bus_pu_vmag_angle()"),
];

/// The four per-bus SEQUENCE + LINE-TO-LINE reads (GOLDEN_REBASE G1.4c), same
/// table shape: capi marker first, r4133 marker second. They are inserted
/// BETWEEN the five voltage arms and the six short-circuit ones of the ONE
/// per-bus walk [`BUS_READ_ORDER`] measures, so the three tables together are
/// the whole per-bus read order on both transports.
/// (`CAPI/CAPI_Alt.pas:2165`/`:2367`/`:2473`/`:2400` == r4133
/// `DDLL/DBus.pas:284`/`:520`/`:549`/`:603`.)
///
/// The r4133 transport serves BOTH line-to-line arms from a single guarded
/// call — `Engine::bus_vll_pair`, which re-reads `Bus.Nodes` itself and refuses
/// `BUSV(11)`/`BUSV(12)` whole where `DBus.pas:580-584` would not terminate —
/// so the `puVLL` row's r4133 marker is the destructuring that publishes
/// `pu_vll` out of that one call, which is the position capi's `b.puVLL` read
/// occupies. That the capture reaches the two arms ONLY through the guard is
/// asserted by
/// [`the_sequence_and_line_to_line_capture_reads_in_one_fixed_order_on_both_transports`].
const SEQ_VLL_READ_ORDER: [(&str, &str); 4] = [
    ("b.SeqVoltages", "engine.bus_seq_voltages()"),
    ("b.CplxSeqVoltages", "engine.bus_cplx_seq_voltages()"),
    ("b.VLL", "engine.bus_vll_pair()"),
    ("b.puVLL", "Some((vll, pu_vll))"),
];

/// The six per-bus SHORT-CIRCUIT reads (GOLDEN_REBASE G1.5), same table shape:
/// capi marker first, r4133 marker second. They are appended to the ONE per-bus
/// walk [`BUS_READ_ORDER`] measures — not a second `SetActiveBus` pass — so the
/// two tables together are the whole per-bus read order on both transports.
/// (`CAPI_Alt.pas:2294`/`:2283`/`:2305`/`:2336`/`:2202`/`:2227` == r4133
/// `DDLL/DBus.pas:461`/`:476`/`:431`/`:491`/`:374`/`:351`.)
const SC_READ_ORDER: [(&str, &str); 6] = [
    ("b.Zsc1", "engine.bus_zsc1()"),
    ("b.Zsc0", "engine.bus_zsc0()"),
    ("b.ZscMatrix", "engine.bus_zsc_matrix()"),
    ("b.YscMatrix", "engine.bus_ysc_matrix()"),
    ("b.Isc", "engine.bus_isc()"),
    ("b.Voc", "engine.bus_voc()"),
];

/// The two per-bus AT-BUS reads (GOLDEN_REBASE G1.4d), same table shape: capi
/// marker first, r4133 marker second. They close the ONE per-bus walk on both
/// transports — read after every row of [`BUS_READ_ORDER`],
/// [`SEQ_VLL_READ_ORDER`] and [`SC_READ_ORDER`] — so the four tables together
/// are the whole per-bus read order.
/// (`CAPI/CAPI_Bus.pas:773-788`/`:790-805` == r4133 `DDLL/DBus.pas:840`/`:867`.)
///
/// **Why last.** On the r4133 channel these are the block's only
/// `ModeEffect::Impure` reads: `getP*atBus` drives `DSS_Class.First`/`Next`
/// (`Common/Circuit.pas:1517-1527`, `:1563-1572`) and `TDSSClass.Get_First`/
/// `Get_Next` (`Common/DSSClass.pas:342-371`) assign
/// `ActiveCircuit.ActiveCktElement` plus each walked class's own cursor. They
/// move neither `ActiveBusIndex` (`DBus.pas:849`, `:876` only pass
/// `BusList.Get(ActiveBusIndex)` down — asserted live in
/// `crates/dss-epri/tests/modes.rs`) nor any `Iterminal` cache, so the surface
/// is still capture-group C
/// ([`the_at_bus_surface_is_order_free_in_the_mode_table`] in
/// `capture_order.rs` pins that from the mode table). The capi twin is pure —
/// its `for elem in DSS_Class` is a `TDSSPointerEnumerator`
/// (`Shared/DSSPointerList.pas:17-27`) with its own index — so the LAST
/// placement is a cross-transport contract taken from the stricter channel,
/// exactly like the rest of this table set.
const AT_BUS_READ_ORDER: [(&str, &str); 2] = [
    ("b.AllPCEatBus", "engine.bus_all_pce_at_bus()"),
    ("b.AllPDEatBus", "engine.bus_all_pde_at_bus()"),
];

/// Read a repo file (`rel` is repo-relative) for a source-order assertion.
fn repo_text(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// Byte offset of `needle` in `hay`, asserting it occurs EXACTLY once — a marker
/// that gained a second occurrence would otherwise silently start measuring a
/// different site.
fn sole_offset(hay: &str, needle: &str, what: &str) -> usize {
    let n = hay.matches(needle).count();
    assert_eq!(
        n, 1,
        "{what}: marker {needle:?} occurs {n} time(s), expected exactly 1 — \
         re-point the capture-order test at the read it is meant to measure"
    );
    hay.find(needle).expect("checked above")
}

/// Assert `markers` appear in `hay` in the given order.
fn assert_source_order(hay: &str, markers: &[&str], what: &str) {
    let offs: Vec<usize> = markers.iter().map(|m| sole_offset(hay, m, what)).collect();
    for i in 1..offs.len() {
        assert!(
            offs[i - 1] < offs[i],
            "{what}: {:?} must be read before {:?} (offsets {} vs {})",
            markers[i - 1],
            markers[i],
            offs[i - 1],
            offs[i]
        );
    }
}

/// The bus surface's capture order is a CONTRACT, on both transports
/// (`GOLDEN_REBASE_PLAN.md` G1.4a; the §1.1(a)/D3 partition).
///
/// Every bus read is **group C — order-free**: `puVoltages`/`VMagAngle`/
/// `puVmagAngle`/`Nodes`/`kVBase` and `AllBusVmagPu` go straight to
/// `Solution.NodeV` (`CAPI/CAPI_Alt.pas:2276` == r4133 `DDLL/DBus.pas:423`) and
/// move only `ActiveBusIndex`; none goes through `ComputeIterminal` (group A)
/// or `GetCurrents` into a scratch buffer (group B), so no bus read can stale a
/// cached `Iterminal` and none is staled by one. The order below is therefore
/// pinned as a **contract between the two transports** — the capi capture and
/// the r4133 capture must ship the same five quantities read the same way, so a
/// divergence is the engines' and never the harness's — and not as protection
/// against CLAUDE.md's upstream bug 4.
///
/// Three things are asserted, all off the transports' own source text:
/// 1. the checkpoint slot — the bus block sits after `variables`/`eventlog`/
///    `ctrlqueue` and BEFORE `all_properties`, which stays the last read of the
///    step on both transports (WP8.5b: the `?` property sweep perturbs the
///    active-element cursor);
/// 2. the per-bus read order, identical on both transports ([`BUS_READ_ORDER`]);
/// 3. the group-C claim itself: neither bus-capture body touches an
///    element-scoped accessor, which is what would make the slot matter;
/// 4. (G1.5) that the six short-circuit arms took **no slot of their own**: the
///    `zsc` request field is consumed inside this same `"buses"` slot, as the
///    argument of the one `capture_all_buses` call, so the list in (1) is
///    complete and stays complete. Their read ORDER is
///    [`the_short_circuit_capture_reads_in_one_fixed_order_on_both_transports`].
#[test]
fn the_bus_capture_reads_in_one_fixed_order_on_both_transports() {
    // ---- capi transport: tools/oracle/oracle_server.py ---------------------
    let py = repo_text("tools/oracle/oracle_server.py");
    assert_source_order(
        &py,
        &[
            "\"variables\":",
            "\"eventlog\":",
            "\"ctrlqueue\":",
            "\"buses\":",
            "\"all_bus_vmag_pu\":",
            // G1.4b: the two circuit-level `DistFromMeter` views take no slot of
            // their own either — they sit inside the same order-free block,
            // behind the same `want_buses` flag, still ahead of the property
            // sweep that must stay last.
            "\"all_bus_distances\":",
            "\"all_node_distances\":",
            "\"all_properties\":",
        ],
        "oracle_server.py checkpoint slot",
    );
    // (4) G1.5 rides this slot: the `zsc` request field reaches the checkpoint
    // only as the argument of the ONE `capture_all_buses` call inside the
    // `"buses"` slot. A future sub-step that gave the short-circuit arms their
    // own checkpoint key would move that call out from between these two
    // offsets and fail here instead of silently changing the read order.
    let py_sc_call = sole_offset(
        &py,
        "capture_all_buses(ckt, want_zsc)",
        "oracle_server.py zsc rides the bus slot",
    );
    let py_buses = sole_offset(&py, "\"buses\":", "oracle_server.py checkpoint slot");
    let py_vmag = sole_offset(
        &py,
        "\"all_bus_vmag_pu\":",
        "oracle_server.py checkpoint slot",
    );
    assert!(
        py_buses < py_sc_call && py_sc_call < py_vmag,
        "oracle_server.py: the `zsc` arms must be captured inside the `\"buses\"`          checkpoint slot (offsets {py_buses} < {py_sc_call} < {py_vmag})"
    );
    let py_body = fn_body(&py, "def capture_all_buses(", |l| l.starts_with("def "));
    let py_code = strip_python_docstring(&py_body);
    let capi_markers: Vec<&str> = BUS_READ_ORDER.iter().map(|(c, _)| *c).collect();
    assert_source_order(
        &py_code,
        &capi_markers,
        "oracle_server.py capture_all_buses",
    );

    // ---- r4133 transport: crates/dss-epri/src/capture.rs -------------------
    let rs = repo_text("crates/dss-epri/src/capture.rs");
    assert_source_order(
        &rs,
        &[
            "req.eventlog",
            "req.ctrlqueue",
            "req.buses",
            "req.all_properties",
        ],
        "capture.rs run_case slot",
    );
    // (4), r4133 side: same rule, same shape.
    let rs_sc_call = sole_offset(
        &rs,
        "capture_all_buses(engine, req.zsc)",
        "capture.rs zsc rides the bus slot",
    );
    let rs_buses = sole_offset(&rs, "req.buses", "capture.rs run_case slot");
    let rs_props = sole_offset(&rs, "req.all_properties", "capture.rs run_case slot");
    assert!(
        rs_buses < rs_sc_call && rs_sc_call < rs_props,
        "capture.rs: the `zsc` arms must be captured inside the `req.buses` block          (offsets {rs_buses} < {rs_sc_call} < {rs_props})"
    );
    // G1.4b, r4133 side of the same rule: the two circuit-level `DistFromMeter`
    // views are captured inside the `req.buses` block, right after
    // `AllBusVmagPu` and still before the property sweep — the mirror of the
    // capi slot list above.
    assert_source_order(
        &rs,
        &[
            "req.buses",
            "capture_all_bus_vmag_pu(engine)",
            "capture_all_bus_distances(engine)",
            "capture_all_node_distances(engine)",
            "req.all_properties",
        ],
        "capture.rs run_case distance slot",
    );
    let rs_body = fn_body(&rs, "fn capture_all_buses(", |l| l == "}");
    let rs_code = strip_rust_line_comments(&rs_body);
    let epri_markers: Vec<&str> = BUS_READ_ORDER.iter().map(|(_, r)| *r).collect();
    assert_source_order(&rs_code, &epri_markers, "capture.rs capture_all_buses");

    // ---- (3) the group-C claim ---------------------------------------------
    // An element-scoped read inside either bus capture would put a group-A/B
    // quantity in the middle of an order-free block, and the slot above would
    // stop being free. Scanned on code only (docstring/comments stripped), so
    // the prose that EXPLAINS the rule cannot trip it.
    for (what, code, forbidden) in [
        (
            "oracle_server.py capture_all_buses",
            py_code.as_str(),
            ["ActiveCktElement", "SetActiveElement"].as_slice(),
        ),
        (
            "capture.rs capture_all_buses",
            rs_code.as_str(),
            ["set_active_element", ".element_"].as_slice(),
        ),
    ] {
        for f in forbidden {
            assert!(
                !code.contains(f),
                "{what}: {f:?} appears in a capture documented as group C \
                 (order-free) — an element-scoped read there breaks the \
                 §1.1(a)/D3 partition; re-classify the block or drop the read"
            );
        }
    }
}

/// The SHORT-CIRCUIT capture order is a CONTRACT too, on both transports
/// (GOLDEN_REBASE_PLAN.md G1.5 §2.a; the §1.1(a)/D3 partition).
///
/// Like the five voltage arms, all six of `Zsc1`/`Zsc0`/`ZscMatrix`/
/// `YscMatrix`/`Isc`/`Voc` are **group C — order-free**: each one moves only
/// `ActiveBusIndex` and then reads a field of the bus object
/// (`Bus.Zsc`/`Ysc`/`BusCurrent`/`VBus` — capi `CAPI/CAPI_Alt.pas:2202-2365`,
/// r4133 `DDLL/DBus.pas:351-518`), never `ComputeIterminal` (group A) and never
/// `GetCurrents` into a scratch buffer (group B). The order is pinned as a
/// contract BETWEEN the two transports, so a divergence is the engines' and
/// never the harness'.
///
/// Three things are asserted, all off the transports' own source text:
/// 1. the six SC markers appear in the same order on both transports
///    ([`SC_READ_ORDER`]);
/// 2. they sit AFTER the five [`BUS_READ_ORDER`] reads, in the same
///    `capture_all_buses` body — i.e. they extend the one per-bus walk instead
///    of opening a second `SetActiveBus` pass (the walk's checkpoint slot is
///    pinned by [`the_bus_capture_reads_in_one_fixed_order_on_both_transports`],
///    which this test deliberately does not repeat);
/// 3. the two claims that are specific to THIS surface and that no other test
///    covers: the SC segment touches no element-scoped accessor (group C), and
///    it never RUNS or refreshes a study. `Zsc`/`Ysc` exist only because the
///    case's own deck solved a fault study (`Common/SolutionAlgs.pas:875-912`);
///    a `ZscRefresh`/`Solve` inside the capture would make the oracle answer a
///    question the gate asked rather than the one the deck did — self-fulfilling
///    on every one of the ~440 forced cases whose deck runs no study at all.
#[test]
fn the_short_circuit_capture_reads_in_one_fixed_order_on_both_transports() {
    // ---- capi transport: tools/oracle/oracle_server.py ---------------------
    let py = repo_text("tools/oracle/oracle_server.py");
    let py_body = fn_body(&py, "def capture_all_buses(", |l| l.starts_with("def "));
    let py_code = strip_python_docstring(&py_body);
    // (1) + (2) in one pass: the five voltage markers then the six SC markers.
    let capi_markers: Vec<&str> = BUS_READ_ORDER
        .iter()
        .map(|(c, _)| *c)
        .chain(SC_READ_ORDER.iter().map(|(c, _)| *c))
        .collect();
    assert_source_order(
        &py_code,
        &capi_markers,
        "oracle_server.py capture_all_buses (voltage arms then SC arms)",
    );

    // ---- r4133 transport: crates/dss-epri/src/capture.rs -------------------
    let rs = repo_text("crates/dss-epri/src/capture.rs");
    let rs_body = fn_body(&rs, "fn capture_all_buses(", |l| l == "}");
    let rs_code = strip_rust_line_comments(&rs_body);
    let epri_markers: Vec<&str> = BUS_READ_ORDER
        .iter()
        .map(|(_, r)| *r)
        .chain(SC_READ_ORDER.iter().map(|(_, r)| *r))
        .collect();
    assert_source_order(
        &rs_code,
        &epri_markers,
        "capture.rs capture_all_buses (voltage arms then SC arms)",
    );

    // ---- (3) the two claims this surface adds ------------------------------
    // Scanned on the SC SEGMENT only — from the first SC marker to the end of
    // the body — so this is a statement about the block G1.5 added, not a
    // re-run of the whole-body scan the voltage test already does. Comments and
    // docstrings are stripped, so the prose that EXPLAINS a rule cannot trip it.
    for (what, code, first, forbidden) in [
        (
            "oracle_server.py capture_all_buses",
            py_code.as_str(),
            SC_READ_ORDER[0].0,
            // element-scoped (group A/B) + anything that would (re)run a study
            [
                "ActiveCktElement",
                "SetActiveElement",
                "ZscRefresh",
                "Solve",
            ]
            .as_slice(),
        ),
        (
            "capture.rs capture_all_buses",
            rs_code.as_str(),
            SC_READ_ORDER[0].1,
            [
                "set_active_element",
                ".element_",
                "zsc_refresh",
                ".solve(",
                "command(",
            ]
            .as_slice(),
        ),
    ] {
        let at = sole_offset(code, first, what);
        let segment = &code[at..];
        for f in forbidden {
            assert!(
                !segment.contains(f),
                "{what}: {f:?} appears in the short-circuit segment — either it is an \
                 element-scoped read inside a block documented as group C (order-free), \
                 or it re-runs/refreshes the study the DECK is supposed to have run \
                 (GOLDEN_REBASE_PLAN.md G1.5 §2.a). Re-classify the block or drop the read"
            );
        }
    }
}

/// The SEQUENCE + LINE-TO-LINE capture order is a CONTRACT too, on both
/// transports (GOLDEN_REBASE_PLAN.md G1.4c §2.a/§4.2; the §1.1(a)/D3
/// partition).
///
/// All four of `SeqVoltages`/`CplxSeqVoltages`/`VLL`/`puVLL` are **group C —
/// order-free**: each one moves only `ActiveBusIndex` and then reads
/// `Solution.NodeV` through the bus object (`CAPI/CAPI_Alt.pas:2190`, `:2386`,
/// `:2532`, `:2464` == r4133 `DDLL/DBus.pas:305`, `:536`, `:588`, `:644`),
/// never `ComputeIterminal` (group A) and never `GetCurrents` into a scratch
/// buffer (group B). The order is pinned as a contract BETWEEN the two
/// transports — the two captures must ship the same four quantities read the
/// same way, so a divergence is the engines' and never the harness'.
///
/// Three things are asserted, all off the transports' own source text:
/// 1. all **15** per-bus markers — [`BUS_READ_ORDER`], then
///    [`SEQ_VLL_READ_ORDER`], then [`SC_READ_ORDER`] — appear in that one order
///    on both transports. The four new arms therefore extend the single
///    `capture_all_buses` walk whose checkpoint slot
///    [`the_bus_capture_reads_in_one_fixed_order_on_both_transports`] pins;
///    they take no slot and no request field of their own (the surface rides
///    `compare_bus`, `manifest::Case::compare_bus`).
/// 2. the claim specific to THIS surface on the r4133 side: the capture reaches
///    `BUSV(11)`/`BUSV(12)` **only** through the guarded dispatcher
///    `Engine::bus_vll_pair`. The raw `bus_vll`/`bus_pu_vll` accessors still
///    exist for the mode-table proof walk, and a capture that called one of
///    them directly would hang the whole gate on the `NEVTestCase`
///    `double-1..6` buses (`dss-epri::modes::bus_vll_would_hang`,
///    `DBus.pas:580-584`) instead of recording a refusal.
/// 3. and on the capi side: that transport never claims a refusal — its
///    `vll_declined` is the constant `False`, emitted after the two L-L arms.
///    Its partner scan is bounded (`CAPI_Alt.pas:2512-2514`, `for k := 1 to 3`),
///    and the comparator's `SecondLoopHang` cross-check of the harness' replay
///    against the bridge's own predicate is only meaningful because exactly one
///    transport is free to refuse.
#[test]
fn the_sequence_and_line_to_line_capture_reads_in_one_fixed_order_on_both_transports() {
    // ---- capi transport: tools/oracle/oracle_server.py ---------------------
    let py = repo_text("tools/oracle/oracle_server.py");
    let py_body = fn_body(&py, "def capture_all_buses(", |l| l.starts_with("def "));
    let py_code = strip_python_docstring(&py_body);
    // (1) the whole per-bus read order in one pass: 6 voltage/scalar (the sixth
    // is G1.4b's `Distance`) + 4 G1.4c + 6 SC.
    let capi_markers: Vec<&str> = BUS_READ_ORDER
        .iter()
        .map(|(c, _)| *c)
        .chain(SEQ_VLL_READ_ORDER.iter().map(|(c, _)| *c))
        .chain(SC_READ_ORDER.iter().map(|(c, _)| *c))
        .collect();
    assert_eq!(
        capi_markers.len(),
        16,
        "the per-bus read order is 6 voltage/scalar (incl. G1.4b Distance) + 4 sequence/L-L          + 6 short-circuit arms"
    );
    assert_source_order(
        &py_code,
        &capi_markers,
        "oracle_server.py capture_all_buses (voltage, sequence/L-L, then SC arms)",
    );

    // ---- r4133 transport: crates/dss-epri/src/capture.rs -------------------
    let rs = repo_text("crates/dss-epri/src/capture.rs");
    let rs_body = fn_body(&rs, "fn capture_all_buses(", |l| l == "}");
    let rs_code = strip_rust_line_comments(&rs_body);
    let epri_markers: Vec<&str> = BUS_READ_ORDER
        .iter()
        .map(|(_, r)| *r)
        .chain(SEQ_VLL_READ_ORDER.iter().map(|(_, r)| *r))
        .chain(SC_READ_ORDER.iter().map(|(_, r)| *r))
        .collect();
    assert_source_order(
        &rs_code,
        &epri_markers,
        "capture.rs capture_all_buses (voltage, sequence/L-L, then SC arms)",
    );

    // ---- (2) the L-L arms go through the guard, never around it ------------
    for raw in ["engine.bus_vll()", "engine.bus_pu_vll()"] {
        assert!(
            !rs_code.contains(raw),
            "capture.rs capture_all_buses: {raw:?} is the UNGUARDED accessor — the \
             capture must reach `BUSV(11)`/`BUSV(12)` only through \
             `engine.bus_vll_pair()`, which refuses the pair on a bus whose partner \
             scan would not terminate (GOLDEN_REBASE G1.4c; DBus.pas:580-584)"
        );
    }

    // ---- (3) the capi transport never refuses ------------------------------
    let declined = sole_offset(
        &py_code,
        "cap[\"vll_declined\"] = False",
        "oracle_server.py capture_all_buses vll_declined",
    );
    let pu_vll = sole_offset(&py_code, "b.puVLL", "oracle_server.py capture_all_buses");
    assert!(
        declined > pu_vll,
        "oracle_server.py capture_all_buses: `vll_declined` must be the constant \
         `False` emitted alongside the two L-L arms (offsets {pu_vll} then {declined}) \
         — this transport's partner scan is bounded (`CAPI_Alt.pas:2512-2514`) and \
         cannot refuse"
    );
}

/// The AT-BUS capture order is a CONTRACT too, on both transports
/// (GOLDEN_REBASE_PLAN.md G1.4d §2.3; the §1.1(a)/D3 partition).
///
/// `Bus.AllPCEatBus`/`Bus.AllPDEatBus` are the LAST two reads of the one
/// per-bus walk on both channels. That placement is not cosmetic: on the r4133
/// channel they are the block's only `ModeEffect::Impure` reads — `getP*atBus`
/// drives `DSS_Class.First`/`Next` (`Common/Circuit.pas:1517-1527`,
/// `:1563-1572`) and `TDSSClass.Get_First`/`Get_Next`
/// (`Common/DSSClass.pas:342-371`) assign `ActiveCircuit.ActiveCktElement` plus
/// each walked class's own `ActiveElement` cursor (measured live,
/// `crates/dss-epri/tests/modes.rs::the_at_bus_reads_move_the_active_element`).
/// They still move no `Iterminal` cache and no `ActiveBusIndex`, so the surface
/// stays capture-group C — pinned from the mode table by
/// `capture_order.rs::the_at_bus_surface_is_order_free_in_the_mode_table`.
///
/// Four things are asserted, all off the transports' own source text:
/// 1. all **18** per-bus markers — [`BUS_READ_ORDER`], [`SEQ_VLL_READ_ORDER`],
///    [`SC_READ_ORDER`], then [`AT_BUS_READ_ORDER`] — appear in that one order
///    on both transports, so the two arms extend the single `capture_all_buses`
///    walk and take no checkpoint slot and no request field of their own (the
///    surface rides `compare_bus`, `manifest::Case::compare_bus`).
/// 2. they are read **unconditionally**, not from inside the `zsc` block: their
///    statement indentation is the walk's own, one level shallower than the
///    short-circuit reads. A future edit that tucked them under `want_sc` would
///    ship an empty list on every case whose deck runs no fault study.
/// 3. the capi transport ships the reply **RAW** — the whole right-hand side is
///    the identity comprehension over the accessor. The trailing `''` and the
///    `['None']` substitution on that wire are the pinned dss-python facade's
///    (`dss/IBus.py`), not dss_capi's, and normalizing them in the transport
///    would erase exactly the per-channel convention the comparator asserts.
/// 4. the r4133 transport instead carries its own two wire rails (no empty
///    entry; the empty-answer word alone or not at all), because its DDLL arm
///    filters `getP*atBus`' trailing slot and re-emits the word itself
///    (`DBus.pas:853`, `:862-863`, `:880`, `:894-895`) — a violation there is
///    this transport misreading the buffer, not a divergence of the list.
#[test]
fn the_at_bus_capture_reads_last_in_one_fixed_order_on_both_transports() {
    // ---- capi transport: tools/oracle/oracle_server.py ---------------------
    let py = repo_text("tools/oracle/oracle_server.py");
    let py_body = fn_body(&py, "def capture_all_buses(", |l| l.starts_with("def "));
    let py_code = strip_python_docstring(&py_body);
    // (1) the whole per-bus read order in one pass: 6 voltage/scalar + 4
    // sequence/L-L + 6 short-circuit + 2 at-bus.
    let capi_markers: Vec<&str> = BUS_READ_ORDER
        .iter()
        .map(|(c, _)| *c)
        .chain(SEQ_VLL_READ_ORDER.iter().map(|(c, _)| *c))
        .chain(SC_READ_ORDER.iter().map(|(c, _)| *c))
        .chain(AT_BUS_READ_ORDER.iter().map(|(c, _)| *c))
        .collect();
    assert_eq!(
        capi_markers.len(),
        18,
        "the per-bus read order is 6 voltage/scalar + 4 sequence/L-L + 6 short-circuit \
         + 2 at-bus arms"
    );
    assert_source_order(
        &py_code,
        &capi_markers,
        "oracle_server.py capture_all_buses (voltage, sequence/L-L, SC, then at-bus arms)",
    );

    // ---- r4133 transport: crates/dss-epri/src/capture.rs -------------------
    let rs = repo_text("crates/dss-epri/src/capture.rs");
    let rs_body = fn_body(&rs, "fn capture_all_buses(", |l| l == "}");
    let rs_code = strip_rust_line_comments(&rs_body);
    let epri_markers: Vec<&str> = BUS_READ_ORDER
        .iter()
        .map(|(_, r)| *r)
        .chain(SEQ_VLL_READ_ORDER.iter().map(|(_, r)| *r))
        .chain(SC_READ_ORDER.iter().map(|(_, r)| *r))
        .chain(AT_BUS_READ_ORDER.iter().map(|(_, r)| *r))
        .collect();
    assert_source_order(
        &rs_code,
        &epri_markers,
        "capture.rs capture_all_buses (voltage, sequence/L-L, SC, then at-bus arms)",
    );

    // ---- (2) unconditional: shallower than the `zsc`-gated reads -----------
    let indent_of = |code: &str, needle: &str, what: &str| -> usize {
        let at = sole_offset(code, needle, what);
        let line_start = code[..at].rfind('\n').map(|i| i + 1).unwrap_or(0);
        code[line_start..at].len() - code[line_start..at].trim_start().len()
    };
    for (what, code, at_bus, gated) in [
        (
            "oracle_server.py capture_all_buses",
            py_code.as_str(),
            AT_BUS_READ_ORDER[0].0,
            SC_READ_ORDER[0].0,
        ),
        (
            "capture.rs capture_all_buses",
            rs_code.as_str(),
            AT_BUS_READ_ORDER[0].1,
            SC_READ_ORDER[0].1,
        ),
    ] {
        let at_bus_indent = indent_of(code, at_bus, what);
        let gated_indent = indent_of(code, gated, what);
        assert!(
            at_bus_indent < gated_indent,
            "{what}: the at-bus arms are indented {at_bus_indent} and the `zsc`-gated \
             short-circuit arms {gated_indent} — the at-bus pair must be read \
             UNCONDITIONALLY in the walk's own scope (it rides `compare_bus`, not \
             `zsc`), otherwise every case whose deck runs no fault study ships two \
             empty lists the comparator would read as a real answer"
        );
    }

    // ---- (3) the capi transport normalizes nothing -------------------------
    for (key, accessor) in [
        ("all_pce_at_bus", "b.AllPCEatBus"),
        ("all_pde_at_bus", "b.AllPDEatBus"),
    ] {
        let raw = format!("cap[\"{key}\"] = [str(x) for x in {accessor}]");
        assert!(
            py_code.contains(&raw),
            "oracle_server.py capture_all_buses: the `{key}` arm must be the RAW \
             comprehension `{raw}` — the trailing `''` and the empty-answer \
             substitution on this wire are the pinned dss-python facade's \
             (`dss/IBus.py`), and a transport that filtered them would erase the \
             per-channel convention the comparator asserts (GOLDEN_REBASE G1.4d)"
        );
    }

    // ---- (4) the r4133 transport carries its own two wire rails ------------
    let at = sole_offset(
        &rs_code,
        AT_BUS_READ_ORDER[0].1,
        "capture.rs capture_all_buses",
    );
    let segment = &rs_code[at..];
    for (rail, why) in [
        (
            "s.is_empty()",
            "no entry may be empty — DBus.pas:853/:880 filter getP*atBus' trailing slot",
        ),
        (
            "s == \"None\"",
            "the empty-answer word may appear only alone — Circuit.pas:1505/:1551 seed it \
             as the sole entry and DBus.pas:862-863/:894-895 re-emit it",
        ),
    ] {
        assert!(
            segment.contains(rail),
            "capture.rs capture_all_buses: the at-bus segment must assert the rail \
             `{rail}` ({why}); without it a misread buffer reaches the comparator as a \
             divergence of the LIST instead of failing the case"
        );
    }
}

/// D2's cross-transport validation of the bus capture, live (G1.4a audit
/// settlement AC-3): the r4133 capture is compared against the capi capture on
/// one gated `both` case, so a wiring defect on either transport — a swapped
/// slot, an unscaled magnitude, a node set in insertion order — surfaces as
/// "the two oracles disagree" instead of only as a slower Rust-vs-oracle red.
///
/// The deck is `asymmetric:line/line_asym.dss` (`engines: "both"`, `micro`, one
/// step, no ledger entry): 5 buses, three different node sets, and `b2` acquires
/// nodes `2,3` from `line.l2` before node `1` from `line.l1`, so insertion order
/// ≠ ascending order there — the distinction convention 1 and convention 2 are
/// built on. `compare_bus` is set by hand exactly as the scheduler's `force_bus`
/// rule sets it for every live non-`large` case.
///
/// The band is **twice** the node-voltage band: each transport is within one
/// band of the port on this case (the gate asserts precisely that, both
/// channels), so `|A − B| ≤ |A − P| + |P − B| ≤ 2·band`. Angles are compared by
/// reconstructing the phasor rather than by a degree band — the same disc, and
/// no seam to fold. Measured worst on this deck (2026-09-05): well inside; F3's
/// wider one-off over four decks put the worst relative gap at 2.2e-9
/// (`tmp/g14a/f3_xcheck.json`).
#[test]
fn the_two_transports_agree_on_the_bus_capture_of_a_gated_both_case() {
    let mut case = load_family("asymmetric")
        .into_iter()
        .find(|c| c.path == "line/line_asym.dss")
        .expect("asymmetric:line/line_asym.dss must be in the family manifest");
    assert_eq!(
        case.engines, "both",
        "the cross-transport check needs a case both channels gate"
    );
    case.compare_bus = true; // scheduler::force_bus
    let abs = family_file("asymmetric", &case.path);
    let req = engines::build_run_request(&abs, &case);

    let capi = Oracle::for_spec(None).run_case(&abs, &case);
    let resp = engines::EpriOneShot::new().call(&req);
    assert!(resp.ok, "r4133 one-shot failed: {:?}", resp.error);
    let epri: engines::CaseResult =
        serde_json::from_value(resp.result.expect("r4133 ok response missing result"))
            .expect("r4133 malformed CaseResult");

    let tol = harness::tol_for(&case.kind);
    let band = |v_volts: f64| 2.0 * (tol.v_abs + tol.v_rel * v_volts.abs());
    assert_eq!(
        capi.checkpoints.len(),
        epri.checkpoints.len(),
        "the transports disagree on the step count"
    );
    let mut worst: f64 = 0.0;
    for (s, (a, b)) in capi.checkpoints.iter().zip(&epri.checkpoints).enumerate() {
        assert!(
            !a.buses.is_empty(),
            "step {s}: the capi bus capture is empty"
        );
        assert_eq!(
            a.buses.len(),
            b.buses.len(),
            "step {s}: bus count differs between the transports"
        );
        let mut check = |what: &str, x: f64, y: f64, scale_v: f64| {
            let allowed = band(scale_v);
            assert!(
                (x - y).abs() <= allowed,
                "step {s}: {what}: capi {x} vs r4133 {y} \
                 (|diff| = {:.3e} > allowed {allowed:.3e})",
                (x - y).abs()
            );
            worst = worst.max((x - y).abs());
        };
        for (ba, bb) in a.buses.iter().zip(&b.buses) {
            assert!(
                ba.name.eq_ignore_ascii_case(&bb.name),
                "step {s}: bus name differs: {} vs {}",
                ba.name,
                bb.name
            );
            assert_eq!(ba.nodes, bb.nodes, "step {s}: bus {} nodes differ", ba.name);
            assert_eq!(
                ba.kv_base, bb.kv_base,
                "step {s}: bus {} kVBase differs",
                ba.name
            );
            // G1.4b: `DistFromMeter` is a zone-build output, not a solve
            // output — no band, no `BaseFactor`, EXACT on both transports
            // (measured bit-equal over four decks, `tmp/g14b/f1_probe.json`).
            // `line_asym.dss` defines no EnergyMeter, so what this pins here is
            // the shape both channels must agree on: neither invents a distance.
            assert_eq!(
                ba.distance, bb.distance,
                "step {s}: bus {} Distance differs: capi {} vs r4133 {} km",
                ba.name, ba.distance, bb.distance
            );
            // `BaseFactor` (`CAPI_Alt.pas:2262-2265` == `DBus.pas:413-414`): the pu arrays
            // are volts over this, so scaling a pu gap back by it puts every
            // comparison on the one physical band.
            let bf = if ba.kv_base > 0.0 {
                1000.0 * ba.kv_base
            } else {
                1.0
            };
            for k in 0..ba.nodes.len() {
                let (re_a, im_a) = (ba.pu_voltages[2 * k], ba.pu_voltages[2 * k + 1]);
                let (re_b, im_b) = (bb.pu_voltages[2 * k], bb.pu_voltages[2 * k + 1]);
                let mag_v = (re_b * re_b + im_b * im_b).sqrt() * bf;
                check("puVoltages re", re_a * bf, re_b * bf, mag_v);
                check("puVoltages im", im_a * bf, im_b * bf, mag_v);
                // Polar: reconstruct the phasor, so the ±180° seam and the
                // magnitude/angle split need no band of their own.
                for (what, arr_a, arr_b, scale) in [
                    ("VMagAngle", &ba.vmag_angle, &bb.vmag_angle, 1.0),
                    ("puVMagAngle", &ba.pu_vmag_angle, &bb.pu_vmag_angle, bf),
                ] {
                    let (ma, aa) = (arr_a[2 * k] * scale, arr_a[2 * k + 1].to_radians());
                    let (mb, ab) = (arr_b[2 * k] * scale, arr_b[2 * k + 1].to_radians());
                    check(&format!("{what} magnitude"), ma, mb, mb);
                    check(&format!("{what} re"), ma * aa.cos(), mb * ab.cos(), mb);
                    check(&format!("{what} im"), ma * aa.sin(), mb * ab.sin(), mb);
                }
            }
        }
        // `AllBusVmagPu` (convention 2): bus-list order × the bus's own nodes,
        // so the per-entry `BaseFactor` is rebuilt from the bus block above.
        let bfs: Vec<f64> = a
            .buses
            .iter()
            .flat_map(|bu| {
                let bf = if bu.kv_base > 0.0 {
                    1000.0 * bu.kv_base
                } else {
                    1.0
                };
                std::iter::repeat_n(bf, bu.nodes.len())
            })
            .collect();
        assert_eq!(
            a.all_bus_vmag_pu.len(),
            bfs.len(),
            "step {s}: AllBusVmagPu length disagrees with the bus node total"
        );
        assert_eq!(
            a.all_bus_vmag_pu.len(),
            b.all_bus_vmag_pu.len(),
            "step {s}: AllBusVmagPu length differs between the transports"
        );
        for (k, (x, y)) in a.all_bus_vmag_pu.iter().zip(&b.all_bus_vmag_pu).enumerate() {
            check(
                &format!("AllBusVmagPu entry {k}"),
                x * bfs[k],
                y * bfs[k],
                y * bfs[k],
            );
        }
        // G1.4b: the two circuit-level `DistFromMeter` views, exactly. Their
        // lengths carry the two orderings (`NumBuses` and `Σ NumNodesThisBus`),
        // which is the same identity the per-bus walk above asserts from the
        // other end.
        assert_eq!(
            (a.all_bus_distances.len(), a.all_node_distances.len()),
            (a.buses.len(), bfs.len()),
            "step {s}: the capi distance arrays disagree with its own bus walk"
        );
        assert_eq!(
            a.all_bus_distances, b.all_bus_distances,
            "step {s}: AllBusDistances differs between the transports"
        );
        assert_eq!(
            a.all_node_distances, b.all_node_distances,
            "step {s}: AllNodeDistances differs between the transports"
        );
    }
    eprintln!(
        "cross-transport bus capture on asymmetric:line/line_asym.dss: \
         worst |capi − r4133| = {worst:.3e} V"
    );
}

/// D2's cross-transport validation for the **distance** arm, on a case that
/// actually has a meter zone (G1.4b audit settlement, AC-4).
///
/// The bus-capture check above rides `asymmetric:line/line_asym.dss`, which
/// defines no EnergyMeter: every `Distance` it compares is `0.0` against `0.0`,
/// so it pins the SHAPE (neither transport invents a distance) and not the
/// quantity. `controls:energymeter/energymeter_sym.dss` gates on both channels
/// and carries a real zone — `line.feed` (src→m1) and `line.lat` (m1→m2), each
/// `length=1` with `units` unset, and `ConvertLineUnits` returns `1.0` whenever
/// either side is `UNITS_NONE` (`Shared/LineUnits.pas:110-115`), so the walk's
/// own sum is `src = 0`, `m1 = 1`, `m2 = 2` km. That makes this the place where
/// the two transports are pinned against each other on a NON-ZERO
/// `DistFromMeter`, exactly (`rel = abs = 0`, no band anywhere), across all 24
/// daily steps — a stale `ActiveBusIndex` or a misindexed `CircuitV` mode on
/// either side reds here instead of hiding behind an all-zero comparison.
#[test]
fn the_two_transports_agree_on_the_bus_distances_of_a_metered_both_case() {
    let mut case = load_family("controls")
        .into_iter()
        .find(|c| c.path == "energymeter/energymeter_sym.dss")
        .expect("controls:energymeter/energymeter_sym.dss must be in the family manifest");
    assert_eq!(
        case.engines, "both",
        "the cross-transport check needs a case both channels gate"
    );
    case.compare_bus = true; // scheduler::force_bus
    let abs = family_file("controls", &case.path);
    let req = engines::build_run_request(&abs, &case);

    let capi = Oracle::for_spec(None).run_case(&abs, &case);
    let resp = engines::EpriOneShot::new().call(&req);
    assert!(resp.ok, "r4133 one-shot failed: {:?}", resp.error);
    let epri: engines::CaseResult =
        serde_json::from_value(resp.result.expect("r4133 ok response missing result"))
            .expect("r4133 malformed CaseResult");

    assert_eq!(
        capi.checkpoints.len(),
        epri.checkpoints.len(),
        "the transports disagree on the step count"
    );
    assert!(
        !capi.checkpoints.is_empty(),
        "the metered case captured no step"
    );
    let mut nonzero = 0usize;
    for (s, (a, b)) in capi.checkpoints.iter().zip(&epri.checkpoints).enumerate() {
        assert_eq!(
            a.buses.len(),
            b.buses.len(),
            "step {s}: bus count differs between the transports"
        );
        // The zone walk's own numbers, by NAME — the deck's three buses, in
        // whatever order the two BusLists report them.
        let mut seen: Vec<(String, f64)> = Vec::new();
        for (ba, bb) in a.buses.iter().zip(&b.buses) {
            assert!(
                ba.name.eq_ignore_ascii_case(&bb.name),
                "step {s}: bus name differs: {} vs {}",
                ba.name,
                bb.name
            );
            assert_eq!(
                ba.distance, bb.distance,
                "step {s}: bus {} Distance differs: capi {} vs r4133 {} km \
                 (a zone-build output — compared exactly on both transports)",
                ba.name, ba.distance, bb.distance
            );
            if ba.distance != 0.0 {
                nonzero += 1;
            }
            seen.push((ba.name.to_ascii_lowercase(), ba.distance));
        }
        seen.sort_by(|x, y| x.0.cmp(&y.0));
        assert_eq!(
            seen,
            vec![
                ("m1".to_string(), 1.0),
                ("m2".to_string(), 2.0),
                ("src".to_string(), 0.0),
            ],
            "step {s}: the meter zone's own km moved (both transports agree with each other, \
             so a move here is upstream's, not a transport's)"
        );
        assert_eq!(
            (a.all_bus_distances.len(), a.all_node_distances.len()),
            (a.buses.len(), a.buses.iter().map(|bu| bu.nodes.len()).sum()),
            "step {s}: the capi distance arrays disagree with its own bus walk"
        );
        assert_eq!(
            a.all_bus_distances, b.all_bus_distances,
            "step {s}: AllBusDistances differs between the transports"
        );
        assert_eq!(
            a.all_node_distances, b.all_node_distances,
            "step {s}: AllNodeDistances differs between the transports"
        );
    }
    assert_eq!(
        nonzero,
        2 * capi.checkpoints.len(),
        "the comparison is vacuous unless m1 and m2 carry a non-zero distance at every step"
    );
    eprintln!(
        "cross-transport bus distances on controls:energymeter/energymeter_sym.dss: \
         {} step(s), {nonzero} non-zero bus distance(s), capi == r4133 exactly",
        capi.checkpoints.len()
    );
}

/// D2's cross-transport validation for the **short-circuit** arms (G1.5 audit
/// settlement, T4). Its bus-surface sibling above sets `compare_bus` only, so
/// `build_run_request` ships `"zsc": false` and no `Zsc`/`Ysc`/`Isc`/`Voc`
/// value was ever compared transport-to-transport in CI. What this catches
/// that a Rust-vs-oracle red catches more slowly: a wiring defect on ONE
/// transport — the node arrays permuted into ascending order, a swapped
/// `Zsc1`/`Zsc0` slot, `Isc` read where `Voc` belongs, an unscaled arm — shows
/// up here as "the two oracles disagree", which localizes it to the bridge
/// instead of the engine. A TRANSPOSED matrix read stays invisible to it, as
/// it is to every value comparison in the suite (`Zsc`/`Ysc` are symmetric to
/// ≤ 4.66e-10 corpus-wide — the measurement that made spec §4's transpose demo
/// vacuous); that convention is pinned offline instead, by
/// `exec::view::bus_sc_tests::flatten_row_major_walks_i_outer_on_an_asymmetric_matrix`.
///
/// The deck is `modes:faultstudy/faultstudy_micro.dss` — `engines: "both"`,
/// `kind: micro`, one step, no ledger entry, and the only corpus case that runs
/// a fault study at the tightest tier. Its `b2` is reached as `bus2=b2.2.1.3`
/// with a 1-phase 5 Ω reactor on node `1`, so the `Zsc` diagonal is
/// position-dependent (`[1][1]` ≈ 1.665+1.662j Ω against ≈ 1.063+2.874j at
/// `[0][0]`/`[2][2]`): a permuted or transposed read on either transport moves
/// ~0.6 Ω, six orders past the band.
///
/// The band is **twice** the tier's, by the same triangle argument the bus
/// sibling uses: each transport is within one band of the port (the gate
/// asserts precisely that, on both channels), so `|A − B| ≤ 2·band`.
#[test]
fn the_two_transports_agree_on_the_short_circuit_capture_of_a_gated_both_case() {
    let mut case = load_family("modes")
        .into_iter()
        .find(|c| c.path == "faultstudy/faultstudy_micro.dss")
        .expect("modes:faultstudy/faultstudy_micro.dss must be in the family manifest");
    assert_eq!(
        case.engines, "both",
        "the cross-transport check needs a case both channels gate"
    );
    assert!(
        case.compare_zsc,
        "the manifest row must carry compare_zsc: this is the surface's witness deck"
    );
    case.compare_bus = true; // scheduler::force_bus; `compare_zsc` implies it
    let abs = family_file("modes", &case.path);
    let req = engines::build_run_request(&abs, &case);

    let capi = Oracle::for_spec(None).run_case(&abs, &case);
    let resp = engines::EpriOneShot::new().call(&req);
    assert!(resp.ok, "r4133 one-shot failed: {:?}", resp.error);
    let epri: engines::CaseResult =
        serde_json::from_value(resp.result.expect("r4133 ok response missing result"))
            .expect("r4133 malformed CaseResult");

    let tol = harness::tol_for(&case.kind);
    assert_eq!(
        capi.checkpoints.len(),
        epri.checkpoints.len(),
        "the transports disagree on the step count"
    );
    let mut worst: f64 = 0.0;
    let mut full_matrices = 0usize;
    for (s, (a, b)) in capi.checkpoints.iter().zip(&epri.checkpoints).enumerate() {
        assert_eq!(
            a.buses.len(),
            b.buses.len(),
            "step {s}: bus count differs between the transports"
        );
        for (ba, bb) in a.buses.iter().zip(&b.buses) {
            assert!(
                ba.name.eq_ignore_ascii_case(&bb.name),
                "step {s}: bus name differs: {} vs {}",
                ba.name,
                bb.name
            );
            let n = ba.nodes.len();
            // Every arm, per entry, at twice its own tier band — the same
            // assignment the comparator makes (`v_*` for the impedances and
            // `Voc`, `y_*` for `Ysc`, `i_*` for `Isc`).
            for (what, xa, xb, rel, ab) in [
                ("Zsc1", &ba.zsc1, &bb.zsc1, tol.v_rel, tol.v_abs),
                ("Zsc0", &ba.zsc0, &bb.zsc0, tol.v_rel, tol.v_abs),
                ("ZscMatrix", &ba.zsc, &bb.zsc, tol.v_rel, tol.v_abs),
                ("YscMatrix", &ba.ysc, &bb.ysc, tol.y_rel, tol.y_abs),
                ("Isc", &ba.isc, &bb.isc, tol.i_rel, tol.i_abs),
                ("Voc", &ba.voc, &bb.voc, tol.v_rel, tol.v_abs),
            ] {
                assert_eq!(
                    xa.len(),
                    xb.len(),
                    "step {s}: bus {} {what} length differs between the transports",
                    ba.name
                );
                if what == "ZscMatrix" && n >= 2 && xa.len() == 2 * n * n {
                    full_matrices += 1;
                }
                for (k, (x, y)) in xa.iter().zip(xb).enumerate() {
                    let allowed = 2.0 * (ab + rel * y.abs());
                    assert!(
                        (x - y).abs() <= allowed,
                        "step {s}: bus {} {what} entry {k}: capi {x} vs r4133 {y} \
                         (|diff| = {:.3e} > allowed {allowed:.3e})",
                        ba.name,
                        (x - y).abs()
                    );
                    worst = worst.max((x - y).abs());
                }
            }
        }
    }
    // Non-vacuity, twice over: this deck runs a study, so the comparison above
    // must have walked real `n x n` matrices and not two agreeing sentinels…
    assert!(
        full_matrices >= 3,
        "the cross-transport short-circuit check compared {full_matrices} full \
         matrices; the deck must publish one per bus after its fault study"
    );
    // …and the content it walked is position-dependent on BOTH transports, so
    // a node-order defect on either one is a ~0.6 Ω move against a ~1e-6 band.
    for (tag, cp) in [
        ("capi", &capi.checkpoints[0]),
        ("r4133", &epri.checkpoints[0]),
    ] {
        let b2 = cp
            .buses
            .iter()
            .find(|b| b.name.eq_ignore_ascii_case("b2"))
            .expect("the deck's b2");
        assert_eq!(b2.zsc.len(), 2 * 3 * 3, "{tag}: b2 must carry a 3x3 Zsc");
        let d = |k: usize| Complex64::new(b2.zsc[2 * (3 * k + k)], b2.zsc[2 * (3 * k + k) + 1]);
        assert!(
            (d(1) - d(0)).norm() > 0.5,
            "{tag}: b2's Zsc diagonal must stay position-dependent (the 1-phase \
             reactor on node 1), got {} vs {}",
            d(1),
            d(0)
        );
    }
    eprintln!(
        "cross-transport short-circuit capture on modes:faultstudy/faultstudy_micro.dss: \
         {full_matrices} full matrices, worst |capi − r4133| = {worst:.3e}"
    );
}

/// The short-circuit surface's fail-on-stale, pinned in **both** directions
/// offline (G1.5 audit settlement, T1). The shipped statics cannot be rewound
/// once a gate run has moved them, so the rule is exercised through its
/// injected-counter form — the same split
/// `harness::props_norm::check_r4133_props_compare_ran` uses.
#[test]
fn the_short_circuit_study_guard_is_silent_on_the_measured_population() {
    harness::check_sc_study_compare_ran(10, 646);
}

/// …and fires when the corpus stops running fault studies — the regression
/// the comparator's own `assert_eq!(port_ran, oracle_ran)` cannot see, because
/// it is equally satisfied when NEITHER side ran one.
#[test]
#[should_panic(expected = "not (10, 646)")]
fn the_short_circuit_study_guard_fires_when_a_deck_stops_solving_a_study() {
    // one deck's two channels gone: 10 - 2 walks, 646 - 2*3 buses
    harness::check_sc_study_compare_ran(8, 640);
}

/// The distance surface's fail-on-stale, pinned in **both** directions offline
/// (GOLDEN_REBASE G1.4b), for the reason its short-circuit sibling above is:
/// the shipped statics cannot be rewound once a gate run has moved them, so the
/// rule is exercised through its injected-counter form.
#[test]
fn the_distance_guard_is_silent_on_the_measured_population() {
    harness::check_distance_compare_ran(867, 79_137);
}

/// …and fires when a deck stops building its meter zone — the regression the
/// comparator's own equality cannot see, because a port and an oracle that both
/// forgot the zone agree on the meterless `0.0` bus for bus.
#[test]
#[should_panic(expected = "not (867, 79137)")]
fn the_distance_guard_fires_when_a_deck_stops_building_its_meter_zone() {
    // one 4-bus deck's two channels gone: 2 walks, 2*2 non-zero buses
    harness::check_distance_compare_ran(865, 79_133);
}

/// ...and on a GROWTH — a new metered case, a new step or a channel that
/// starts gating. Both directions are driven because both documents that
/// describe the pair claim both (`TESTING.md`, and the constant's own doc):
/// the rule is one `assert_eq!` over the tuple, so a growth must be
/// re-derived off a completed run and moved deliberately, never absorbed.
#[test]
#[should_panic(expected = "not (867, 79137)")]
fn the_distance_guard_fires_when_a_metered_case_arrives() {
    // one 4-bus deck's two channels added: 2 walks, 2*2 non-zero buses
    harness::check_distance_compare_ran(869, 79_141);
}

/// The at-bus surface's four fail-on-stale populations (GOLDEN_REBASE G1.4d),
/// pinned in **both** directions offline for the reason their distance and
/// short-circuit siblings above are: the shipped statics cannot be rewound once
/// a gate run has moved them, so the rule is exercised through its
/// injected-counter form.
///
/// The measured tuple is the one a completed full default-lane gate printed on
/// its own `corpus_gate at-bus:` line (2026-09-06, 526/526 cases).
#[test]
fn the_at_bus_guard_is_silent_on_the_measured_populations() {
    harness::check_at_bus_populations(harness::AtBusPopulation {
        pde_terminal3_declines: (279, 279),
        capi_noderef_drops: (18, 147),
        capi_noderef_adds: (8, 11),
        pce_declines: (0, 0),
    });
}

/// …and fires when a class stops being carried — the regression neither the
/// comparator nor `population.lock.json` can see, because the mechanism
/// assertion is equally satisfied by a channel and a port that BOTH stopped
/// listing the element, and the lock fingerprints the manifest flag (this
/// surface rides `compare_bus` and sets none of its own).
#[test]
#[should_panic(expected = "`capi node-ref drops` came out (17, 146)")]
fn the_at_bus_guard_fires_when_a_deck_stops_carrying_its_class() {
    harness::check_at_bus_populations(harness::AtBusPopulation {
        pde_terminal3_declines: (279, 279),
        // one deck's single record gone
        capi_noderef_drops: (17, 146),
        capi_noderef_adds: (8, 11),
        pce_declines: (0, 0),
    });
}

/// …and on a GROWTH, including the `(0, 0)` PCE counterpart: a PC-class
/// divergence must become a gate failure to be triaged, never a number that
/// quietly grows.
#[test]
#[should_panic(expected = "`PCE at-bus declines` came out (1, 1)")]
fn the_at_bus_guard_fires_when_a_pce_divergence_appears() {
    harness::check_at_bus_populations(harness::AtBusPopulation {
        pde_terminal3_declines: (279, 279),
        capi_noderef_drops: (18, 147),
        capi_noderef_adds: (8, 11),
        pce_declines: (1, 1),
    });
}

/// …and on the class-A GROWTH — a new deck (or a moved port answer) reaching
/// r4133's terminal-1/2 blind spot. Class A is projected away on both sides of
/// the r4133 mechanism assertion, so this counter is its only live witness and
/// gets a drive of its own (G1.4d audit settlement T5).
#[test]
#[should_panic(expected = "`PDE terminal-3 declines` came out (280, 280)")]
fn the_at_bus_guard_fires_when_the_terminal3_class_grows() {
    harness::check_at_bus_populations(harness::AtBusPopulation {
        // one more 3rd-winding bus reached
        pde_terminal3_declines: (280, 280),
        capi_noderef_drops: (18, 147),
        capi_noderef_adds: (8, 11),
        pce_declines: (0, 0),
    });
}

/// …and when capi's stale-reference class SHRINKS — the direction that says a
/// deck stopped exercising the `Circuit.pas:1756` staleness the entry describes
/// (the fourth tuple's own drive, same settlement).
#[test]
#[should_panic(expected = "`capi node-ref adds` came out (7, 10)")]
fn the_at_bus_guard_fires_when_a_stale_node_ref_stops_naming_an_element() {
    harness::check_at_bus_populations(harness::AtBusPopulation {
        pde_terminal3_declines: (279, 279),
        capi_noderef_drops: (18, 147),
        // one deck's stale reference gone
        capi_noderef_adds: (7, 10),
        pce_declines: (0, 0),
    });
}

/// GOLDEN_REBASE G1.4b — the two `MakeBusList` decks of coordinator decision
/// **D9**, pinned against the distances BOTH oracle channels measure.
///
/// D9 was a real port bug: r4133 runs `DoResetMeterZones` INSIDE
/// `TDSSCircuit.ReProcessBusDefs` (`Common/Circuit.pas:2411`, capi `:2246`)
/// while the port had hoisted it out, so a deck that issues `MakeBusList` after
/// defining an EnergyMeter consumed the rebuild flag and kept EMPTY zones — no
/// customers, no parent PD, no `DistFromMeter`. It was fixed on lane `lane-m`
/// (G1.6b micro-part F0) and reached this lane with the `update` sync before
/// G1.4b F1. These are the only two decks in the corpus that both issue
/// `MakeBusList` and define a meter, so this is where the fix is observable on
/// THIS surface: without it every number below would be `0.0` and the whole
/// comparison would still be green, because the oracles would be compared
/// against a port that simply reports nothing.
///
/// The expected values are the ORACLES', not the port's: measured live on
/// 2026-09-05 through the real capture paths of both channels
/// (`oracle_server.py` one-shot and `epri-worker`), bit-identical between them
/// — `tmp/g14b/f2_pins.json`, the run recorded in the G1.4b handoff. They are
/// exact `f64` equalities, the same `rel = abs = 0` the live comparator uses.
///
/// `1.609344` and `3.218688` are 1 and 2 international miles in km — the
/// `len · ConvertLineUnits(units, UNITS_KM)` sum of the zone walk, over the
/// identical `To_Meters(UNITS_MILES) = 1609.344` both engines carry (r4133
/// `Version8/Source/Shared/LineUnits.pas:81`, capi `src/Shared/LineUnits.pas:108`;
/// the `1609.3` of `Version7/Source/Deprecated_LazDSS` and
/// `Version8/Source/CMD_Lazz` is in neither build).
#[test]
fn the_make_bus_list_decks_report_the_zone_distances_both_oracles_measure() {
    /// One pinned deck: its `BusList`, the oracles' own `DistFromMeter` per
    /// bus (km) and the bus's node count — what the node array's run lengths
    /// must reproduce.
    struct DistPin {
        rel: &'static str,
        names: &'static [&'static str],
        km: &'static [f64],
        node_counts: &'static [usize],
    }
    #[rustfmt::skip]
    let decks = [
        DistPin {
            rel: "Test/indmachtest/Master.DSS",
            names: &["sourcebus", "b1", "b2", "b3"],
            km: &[0.0, 0.0, 1.609344, 3.218688],
            node_counts: &[4, 4, 3, 3],
        },
        DistPin {
            rel: concat!(
                "Version8/Distrib/Examples/ADiakoptics/IEEE_13_Bus/",
                "Torn_Circuit/Master_Interconnected.dss"
            ),
            names: &[
                "sourcebus", "rg60", "632", "670", "633", "645", "646", "634",
                "650", "671", "692", "675", "684", "652", "611", "680",
            ],
            km: &[
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.406_298_400_000_000_06,
                0.407_298_400_000_000_06,
                0.559_698_4,
                0.497_738_400_000_000_1,
                0.741_578_400_000_000_1,
                0.589_178_400_000_000_1,
                0.711_098_4,
            ],
            node_counts: &[3, 3, 3, 3, 3, 2, 2, 3, 3, 3, 3, 3, 2, 1, 1, 3],
        },
    ];
    for DistPin {
        rel,
        names,
        km: want,
        node_counts,
    } in decks
    {
        let abs = corpus_file(rel);
        let scratch = ad_scratch("dist");
        let mut dss = Dss::new();
        dss.command("clear");
        dss.command(&format!("compile \"{abs}\""));
        dss.command(&format!("set datapath=\"{}\"", scratch.display()));
        dss.command("solve");
        let views = dss.all_bus_voltages();
        let got: Vec<String> = views.iter().map(|v| v.name.clone()).collect();
        assert_eq!(
            got,
            names.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            "{rel}: BusList order moved"
        );
        assert_eq!(
            dss.all_bus_distances(),
            want,
            "{rel}: AllBusDistances is not what both oracles measure"
        );
        for (v, w) in views.iter().zip(want) {
            assert_eq!(v.distance, *w, "{rel}: bus {} Distance", v.name);
        }
        // …and the node array is that vector expanded by `NumNodesThisBus`.
        let expect_nodes: Vec<f64> = want
            .iter()
            .zip(node_counts)
            .flat_map(|(d, n)| std::iter::repeat_n(*d, *n))
            .collect();
        assert_eq!(
            dss.all_node_distances(),
            expect_nodes,
            "{rel}: AllNodeDistances is not the bus vector expanded by NumNodesThisBus"
        );
        // The D9 signal itself: the zone really was rebuilt after `MakeBusList`.
        assert!(
            want.iter().any(|d| *d != 0.0),
            "{rel}: this pin is vacuous unless the deck's zone carries a distance"
        );
        let _ = std::fs::remove_dir_all(&scratch);
    }
}

/// GOLDEN_REBASE G1.4b — the three merged-line distances of
/// `modes:reduce/midi_reduce.dss`, the expected-value pin behind the ledger
/// entry `reduce-merge-units-lost-midi-capi-distance` (coordinator decision
/// D29 step 3).
///
/// `DoReduceDefault` merges three matrix-model line pairs here. The pinned
/// dss_capi 0.14.5 loses the merged line's `LengthUnits` (cause
/// `line-merge-length-units-reset`: r4133
/// `Version8/Source/PDElements/Line.pas:1794-1796` re-applies the saved units
/// AFTER the matrix edits, capi `src/PDElements/Line.pas:1806-1817` lets
/// `ResetLengthUnits` wipe them one statement later), and
/// `MakeMeterZoneLists` multiplies every line branch by
/// `ConvertLineUnits(LengthUnits, UNITS_KM)`, which is `1.0` whenever either
/// side is `UNITS_NONE` (`Shared/LineUnits.pas:110-115`). So capi consumes the
/// merged 4 kft lines as 4 **km** and its `Bus.Distance` runs 4.0 km high on
/// exactly these three buses, while the port (RP3.5, r4133's behaviour) reads
/// `4 kft = 1.2192 km`:
///
/// | bus | port == r4133 | capi 0.14.5 |
/// |---|---|---|
/// | `l2e` | `2.7432` | `5.524` = `1.524 + 4.0` |
/// | `l3e` | `3.3528000000000002` | `6.1335999999999995` = `2.1336 + 4.0` |
/// | `l9e` | `10.364200000000004` | `13.145000000000003` = `9.145 + 4.0` |
///
/// Both numbers, as the ledger requires. Measured live on 2026-09-05 through
/// the real capture paths of all three engines (`tmp/g14b/probe_reduce2.py`,
/// the G1.4b F2 handoff); the capi column is what the ledger exclusion drops,
/// and this pin is what keeps the port honest in its place — without it the
/// three buses would be compared against nothing at all.
///
/// The `units`/`length` assertions tie the numbers to their cause: the
/// distances are `1.2192 km` above their upstream bus precisely because the
/// merged lines still answer `kft`.
#[test]
fn the_reduced_midi_deck_reports_the_merged_lines_kft_distances() {
    let abs = family_file("modes", "reduce/midi_reduce.dss");
    let scratch = ad_scratch("midi-reduce-dist");
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile {abs:?}"));
    dss.command(&format!("set datapath={:?}", scratch.display().to_string()));

    // The deck reduces and re-solves itself; the three merges are its point.
    for line in ["l2a~l2b", "l3a~l3b", "bb14_15~l9a"] {
        dss.command(&format!("? line.{line}.units"));
        assert_eq!(
            dss.result(),
            "kft",
            "line.{line} lost the units DoReduceDefault saved (RP3.5)"
        );
        dss.command(&format!("? line.{line}.length"));
        assert_eq!(dss.result(), "4", "line.{line} merged length");
    }

    let want = [
        ("l2e", 2.7432),
        ("l3e", 3.3528000000000002),
        ("l9e", 10.364200000000004),
    ];
    let views = dss.all_bus_voltages();
    let all_bus = dss.all_bus_distances();
    for (name, km) in want {
        let i = views
            .iter()
            .position(|v| v.name.eq_ignore_ascii_case(name))
            .unwrap_or_else(|| panic!("bus {name} is not in the reduced BusList"));
        assert_eq!(
            views[i].distance, km,
            "bus {name} Distance: the merged 4 kft line contributes 1.2192 km, not 4.0 (capi 0.14.5 reports the 4.0; see the ledger entry)"
        );
        assert_eq!(all_bus[i], km, "bus {name} AllBusDistances slot");
    }
    let _ = std::fs::remove_dir_all(&scratch);
}

/// GOLDEN_REBASE **G1.4d** — the port's own `AllPCEatBus` / `AllPDEatBus`
/// answer on `modes:makeposseq/makeposseq_xfmr.dss`, the one corpus deck that
/// carries all three divergence classes at once (coordinator decision D26).
///
/// The deck builds a substation transformer, a **3-winding** transformer, an
/// `AutoTrans` and two 1-phase transformers, solves, runs `makeposseq` — which
/// disables `Transformer.t1_off` because it is not on phase 1 (r4133
/// `Version8/Source/PDElements/Transformer.pas:1698`) and rebuilds `BusList`
/// from the ENABLED elements only, dropping `b6` — and solves again. That
/// leaves one element disabled with pre-renumbering node references, which is
/// exactly the state the two oracles read differently:
///
/// | bus | port (**S4**) `AllPDEatBus` | r4133 | capi 0.14.5 |
/// |---|---|---|---|
/// | `b1` | `sub, t3, at, t1_ok, t1_off` | **the same five** | the same **minus `t1_off`** — class B |
/// | `b3` | `t3` (its THIRD winding) | `[]`, i.e. `['None']` — class A | `t3` |
/// | `b4` | `at` | `at` | `t1_off, at` — an element that is NOT there, class C |
///
/// Both numbers, as the settlement requires; the oracle columns were measured
/// live on 2026-09-06 through the real capture paths of both channels
/// (`tmp/g14d/fixture_makeposseq_xfmr.json`) and are asserted against this
/// port state by [`the_makeposseq_xfmr_at_bus_wires_are_each_channels_own_walk`].
/// Each class is an upstream defect that the port does not reproduce:
///
/// * **A** — r4133 tests `StripExtension(GetBus(1))`/`GetBus(2)` only
///   (`Common/Circuit.pas:1520-1522`), so no winding past the second is ever
///   found, although the function's own header promises *"all PDE connected to
///   the bus"* (`:1490-1492`).
/// * **B**/**C** — capi's fast path intersects the element's `TermNodeRef` with
///   the bus's node references (`Common/Circuit.pas:1746-1767`) instead of
///   taking its own *"Original code as fallback"* name test (`:1778-1785`), so
///   a disabled element is dropped from the bus it is wired to and named at the
///   bus that inherited its stale reference.
///
/// The attachment facts are pinned beside the lists because they are what the
/// live comparator replays each oracle's walk over — without them the three
/// classes would be numbers with no mechanism behind them.
#[test]
fn the_makeposseq_xfmr_deck_reports_the_at_bus_lists_the_port_computes() {
    let dss = makeposseq_xfmr_solved();

    // The port's S4 answer, bus by bus, in `BusList` order. `b6` is absent:
    // `makeposseq` disabled the transformer that named it and the rebuild kept
    // only the buses enabled elements name.
    #[rustfmt::skip]
    let want: [(&str, &[&str], &[&str]); 6] = [
        ("src", &["Vsource.source"], &["Transformer.sub"]),
        ("b1",  &[],                 &["Transformer.sub", "Transformer.t3",
                                       "AutoTrans.at", "Transformer.t1_ok",
                                       "Transformer.t1_off"]),
        ("b2",  &["Load.ld2"],       &["Transformer.t3"]),
        ("b3",  &[],                 &["Transformer.t3"]),
        ("b4",  &["Load.ld4"],       &["AutoTrans.at"]),
        ("b5",  &["Load.ld5"],       &["Transformer.t1_ok"]),
    ];
    let got = dss.all_bus_elements();
    assert_eq!(
        got.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(),
        want.iter().map(|(n, ..)| *n).collect::<Vec<_>>(),
        "the deck's BusList moved — this pin is keyed by it"
    );
    for (v, (name, pce, pde)) in got.iter().zip(want) {
        assert_eq!(v.pce, pce, "bus {name} AllPCEatBus");
        assert_eq!(v.pde, pde, "bus {name} AllPDEatBus");
        // …and the single-bus accessor answers the same, case-insensitively.
        let one = dss
            .bus_elements(&name.to_ascii_uppercase())
            .unwrap_or_else(|| panic!("bus {name} is in the BusList"));
        assert_eq!(one.pce, v.pce, "bus {name}: bus_elements disagrees on PCE");
        assert_eq!(one.pde, v.pde, "bus {name}: bus_elements disagrees on PDE");
    }

    // The mechanism, from the raw attachment facts the comparator reads.
    let bus = |n: &str| {
        got.iter()
            .find(|v| v.name.eq_ignore_ascii_case(n))
            .unwrap_or_else(|| panic!("bus {n}"))
    };
    let att = |n: &str, e: &str| {
        bus(n)
            .attachments
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case(e))
            .unwrap_or_else(|| panic!("{e} is not attached to {n}"))
    };
    // Class A: only the transformer's THIRD terminal names `b3`.
    let t3_at_b3 = att("b3", "Transformer.t3");
    assert_eq!(
        t3_at_b3.by_name,
        vec![3],
        "b3: which terminal of t3 names it"
    );
    assert!(t3_at_b3.enabled && t3_at_b3.is_pd && t3_at_b3.series);
    // Classes B and C: one disabled element, wired to `b1`, whose stale node
    // reference now belongs to `b4`.
    let off_at_b1 = att("b1", "Transformer.t1_off");
    assert!(!off_at_b1.enabled, "makeposseq disables t1_off");
    assert_eq!(off_at_b1.by_name, vec![1], "t1_off's winding 1 is on b1.2");
    assert!(
        off_at_b1.by_node_ref.is_empty(),
        "b1 is where capi's fast path LOSES t1_off (class B): {:?}",
        off_at_b1.by_node_ref
    );
    let off_at_b4 = att("b4", "Transformer.t1_off");
    assert!(
        off_at_b4.by_name.is_empty(),
        "t1_off names b1 and b6, never b4"
    );
    assert!(
        !off_at_b4.by_node_ref.is_empty(),
        "b4 is where capi's fast path FINDS t1_off through a stale reference (class C)"
    );
}

/// GOLDEN_REBASE **G1.4d** — the two oracles' RAW replies for the same deck,
/// each accepted by its own channel's mechanism assertion, each divergence
/// COUNTED (0 ledger rows).
///
/// The literals are what the two transports put on the wire, verbatim, measured
/// live on 2026-09-06 (`tmp/g14d/fixture_makeposseq_xfmr.json`), so the pin also
/// records the two wire CONVENTIONS the comparator asserts:
///
/// * **capi** ships one trailing `''` on every non-empty reply and `['None']`
///   on an empty one — both are the pinned **dss-python facade's**
///   (`dss/IBus.py`: `result.append('')` under *"added for full compatibility
///   with COM"*, `else: result = ['None']`), not dss_capi's: the C API passes
///   `useNone = False` (`CAPI/CAPI_Bus.pas:784`, `:801`) and returns `[]`.
///   `src.AllPDEatBus` is therefore `['Transformer.sub', '']` on that wire.
/// * **r4133** ships neither artifact — the DDLL filters `getP*atBus`' trailing
///   empty slot and re-emits the lone `'None'` (`DDLL/DBus.pas:853`,
///   `:862-863`, `:880`, `:894-895`) — so the same bus is `['Transformer.sub']`.
///
/// Both normalize to the one element the port lists. The counts are the
/// per-class records of
/// [`the_makeposseq_xfmr_deck_reports_the_at_bus_lists_the_port_computes`]:
/// one class-A record on the r4133 channel (`b3`), one class-B and one class-C
/// record on the capi channel (`b1` and `b4`). The deck gates on
/// `capi_v0145` (`population.lock.json`), so the r4133 half of this pin is the
/// only place its measured reply is held — which is exactly why it is here.
#[test]
fn the_makeposseq_xfmr_at_bus_wires_are_each_channels_own_walk() {
    let dss = makeposseq_xfmr_solved();

    /// `(bus, AllPCEatBus, AllPDEatBus)`, raw.
    type Reply = (
        &'static str,
        &'static [&'static str],
        &'static [&'static str],
    );
    #[rustfmt::skip]
    const CAPI: [Reply; 6] = [
        ("src", &["Vsource.source", ""], &["Transformer.sub", ""]),
        ("b1",  &["None"],               &["Transformer.sub", "Transformer.t3",
                                           "Transformer.t1_ok", "AutoTrans.at", ""]),
        ("b2",  &["Load.ld2", ""],       &["Transformer.t3", ""]),
        ("b3",  &["None"],               &["Transformer.t3", ""]),
        ("b4",  &["Load.ld4", ""],       &["Transformer.t1_off", "AutoTrans.at", ""]),
        ("b5",  &["Load.ld5", ""],       &["Transformer.t1_ok", ""]),
    ];
    #[rustfmt::skip]
    const R4133: [Reply; 6] = [
        ("src", &["Vsource.source"], &["Transformer.sub"]),
        ("b1",  &["None"],           &["Transformer.sub", "Transformer.t3",
                                       "Transformer.t1_ok", "Transformer.t1_off",
                                       "AutoTrans.at"]),
        ("b2",  &["Load.ld2"],       &["Transformer.t3"]),
        ("b3",  &["None"],           &["None"]),
        ("b4",  &["Load.ld4"],       &["AutoTrans.at"]),
        ("b5",  &["Load.ld5"],       &["Transformer.t1_ok"]),
    ];

    let capture = |replies: &[Reply]| -> Vec<harness::BusCap> {
        let views = dss.all_bus_voltages();
        assert_eq!(
            views.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(),
            replies.iter().map(|r| r.0).collect::<Vec<_>>(),
            "the deck's BusList moved — the wire tables are keyed by it"
        );
        views
            .iter()
            .zip(replies)
            .map(|(v, (_, pce, pde))| {
                let mut nodes = v.nodes.clone();
                nodes.sort_unstable();
                harness::BusCap {
                    name: v.name.clone(),
                    kv_base: v.kv_base,
                    distance: v.distance,
                    nodes,
                    pu_voltages: Vec::new(),
                    vmag_angle: Vec::new(),
                    pu_vmag_angle: Vec::new(),
                    zsc1: Vec::new(),
                    zsc0: Vec::new(),
                    zsc: Vec::new(),
                    ysc: Vec::new(),
                    isc: Vec::new(),
                    voc: Vec::new(),
                    seq_voltages: Vec::new(),
                    cplx_seq_voltages: Vec::new(),
                    vll: Vec::new(),
                    pu_vll: Vec::new(),
                    vll_declined: false,
                    all_pce_at_bus: pce.iter().map(|s| (*s).to_string()).collect(),
                    all_pde_at_bus: pde.iter().map(|s| (*s).to_string()).collect(),
                }
            })
            .collect()
    };

    assert_eq!(
        harness::compare_bus_at_bus(
            &dss,
            &capture(&R4133),
            harness::PropsChannel::R4133,
            "G1.4d pin: makeposseq_xfmr on r4133",
        ),
        harness::AtBusCounts {
            pde_terminal3_declines: 1,
            capi_noderef_drops: 0,
            capi_noderef_adds: 0,
            pce_declines: 0,
        },
        "b3: r4133's name test cannot see the 3rd winding the port lists"
    );
    assert_eq!(
        harness::compare_bus_at_bus(
            &dss,
            &capture(&CAPI),
            harness::PropsChannel::CapiV0145,
            "G1.4d pin: makeposseq_xfmr on capi_v0145",
        ),
        harness::AtBusCounts {
            pde_terminal3_declines: 0,
            capi_noderef_drops: 1,
            capi_noderef_adds: 1,
            pce_declines: 0,
        },
        "b1 loses t1_off and b4 gains it — one stale TermNodeRef, both directions"
    );
}

/// `modes:makeposseq/makeposseq_xfmr.dss` compiled the way the gate runs it
/// (the deck issues its own `solve`, `makeposseq` and second `solve`), in a
/// scratch data directory so nothing is written beside the corpus.
fn makeposseq_xfmr_solved() -> Dss {
    let abs = family_file("modes", "makeposseq/makeposseq_xfmr.dss");
    let scratch = ad_scratch("makeposseq-xfmr-at-bus");
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("set datapath={:?}", scratch.display().to_string()));
    dss.command(&format!("compile {abs:?}"));
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let _ = std::fs::remove_dir_all(&scratch);
    dss
}

/// The text of the function whose signature line contains `head`, up to the
/// first later line that satisfies `is_end`.
///
/// Line-based on purpose: the working tree carries both CRLF and LF files
/// (`core.autocrlf`), so a byte-offset scan for `"\n}\n"` silently runs past the
/// end of a CRLF function and swallows its neighbours — which is exactly how
/// this test first went wrong.
fn fn_body(src: &str, head: &str, is_end: fn(&str) -> bool) -> String {
    let lines: Vec<&str> = src.lines().collect();
    let n = lines.iter().filter(|l| l.contains(head)).count();
    assert_eq!(
        n, 1,
        "capture-order test: {head:?} matches {n} line(s), expected exactly 1"
    );
    let start = lines
        .iter()
        .position(|l| l.contains(head))
        .expect("checked");
    let mut out: Vec<&str> = vec![lines[start]];
    for l in &lines[start + 1..] {
        out.push(l);
        if is_end(l) {
            break;
        }
    }
    out.join("\n")
}

/// Drop the leading `"""…"""` docstring so a doc that NAMES a forbidden read
/// cannot fail the scan that looks for the read itself.
fn strip_python_docstring(body: &str) -> String {
    let Some(open) = body.find("\"\"\"") else {
        return body.to_string();
    };
    let after = open + 3;
    match body[after..].find("\"\"\"") {
        Some(close) => body[after + close + 3..].to_string(),
        None => body[after..].to_string(),
    }
}

/// Same, for Rust `//`-comment lines inside a function body.
fn strip_rust_line_comments(body: &str) -> String {
    body.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

// ===========================================================================
// Classifier (growth engine): opt-in via DSS_LIVE_CLASSIFY=1.
// ===========================================================================

#[derive(Debug, Deserialize)]
struct SkipManifest {
    #[serde(default)]
    cases: Vec<SkipCase>,
}

#[derive(Debug, Deserialize)]
struct SkipCase {
    path: String,
}

fn classify_enabled() -> bool {
    std::env::var("DSS_LIVE_CLASSIFY")
        .map(|v| v == "1")
        .unwrap_or(false)
}

#[test]
fn corpus_live_classify() {
    if !classify_enabled() {
        eprintln!("SKIPPED classify: set DSS_LIVE_CLASSIFY=1 to probe candidate cases");
        return;
    }
    let oracle = Oracle::for_spec(None);
    let p = manifests_dir().join("skipped_needs_investigation.json");
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    let m: SkipManifest =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()));

    let mut solvable: Vec<String> = Vec::new();
    let mut failures: Vec<(String, String)> = Vec::new();
    let total = m.cases.len();
    for (i, c) in m.cases.iter().enumerate() {
        let path = c.path.clone();
        let oref = &oracle;
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let abs = corpus_file(&path);
            let case = SolvableCase {
                path: path.clone(),
                kind: "feeder".to_string(),
                n_steps: 1,
                ..Default::default()
            };
            run_and_compare(oref, &path, &abs, &case);
        }));
        match res {
            Ok(()) => solvable.push(c.path.clone()),
            Err(e) => failures.push((c.path.clone(), panic_msg(e))),
        }
        if (i + 1) % 25 == 0 {
            eprintln!("classify: {}/{total} probed", i + 1);
        }
    }

    solvable.sort();
    failures.sort();
    let report = json!({
        "total": total,
        "solvable": solvable,
        "failures": failures
            .iter()
            .map(|(pth, r)| json!({
                "path": pth,
                "reason": r.chars().take(400).collect::<String>(),
            }))
            .collect::<Vec<_>>(),
    });
    let rp: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "tmp"]
        .iter()
        .collect::<PathBuf>()
        .join("classify_report.json");
    let _ = std::fs::create_dir_all(rp.parent().unwrap());
    std::fs::write(&rp, serde_json::to_string_pretty(&report).unwrap())
        .unwrap_or_else(|e| panic!("write {}: {e}", rp.display()));
    eprintln!(
        "classify: {} solvable, {} failed (of {total}); report -> {}",
        solvable.len(),
        failures.len(),
        rp.display()
    );
}

// ===========================================================================
// WP8.5b property-parity pilot (report-first): opt-in via DSS_LIVE_PROPS=1.
// ===========================================================================

fn props_pilot_enabled() -> bool {
    std::env::var("DSS_LIVE_PROPS")
        .map(|v| v == "1")
        .unwrap_or(false)
}

#[test]
fn corpus_live_properties() {
    if !props_pilot_enabled() {
        eprintln!("SKIPPED props: set DSS_LIVE_PROPS=1 to sweep all-property parity");
        return;
    }
    let oracle = Oracle::for_spec(None);

    // The property-parity pilot runs against the PINNED oracle only → include
    // just cases that gate the capi_v0145 channel (skip r4133-only cases).
    let mut universe: Vec<(String, String, SolvableCase)> = Vec::new();
    for fam in FAMILIES {
        for c in load_family(fam.name) {
            if c.pending || !c.gates_capi() || c.expect_solve_abort.is_some() {
                continue;
            }
            let abs = family_file(fam.name, &c.path);
            universe.push((format!("{}:{}", fam.name, c.path), abs, c));
        }
    }
    for c in load_solvable() {
        if !c.gates_capi() || c.expect_solve_abort.is_some() {
            continue;
        }
        universe.push((format!("solvable_now:{}", c.path), corpus_file(&c.path), c));
    }

    let cap = std::env::var("DSS_LIVE_PROPS_MAX")
        .ok()
        .and_then(|s| s.parse::<usize>().ok());
    let total = universe.len();

    let mut covered: Vec<String> = Vec::new();
    let mut subset_skipped: Vec<String> = Vec::new();
    let mut failures: Vec<(String, String)> = Vec::new();
    let mut sweep_elems = 0usize;
    let mut sweep_cmps = 0usize;

    for (i, (label, abs, c)) in universe.iter().enumerate() {
        if let Some(cap) = cap
            && covered.len() + failures.len() >= cap
        {
            subset_skipped.push(label.clone());
            continue;
        }
        let mut case = c.clone();
        case.compare_all_properties = true;
        let oref = &oracle;
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = CorpusGuard::new(abs);
            let oc = oref.run_case(abs, &case);
            let tol = harness::tol_for(&case.kind);
            let mut dss = Dss::new();
            dss.command("clear");
            dss.command(&format!("compile \"{abs}\""));
            for p in &case.post {
                dss.command(p);
            }
            assert!(
                dss.errors().is_empty(),
                "Rust compile errors: {:?}",
                dss.errors()
            );
            let mut elems = 0usize;
            let mut cmps = 0usize;
            for cp in &oc.checkpoints {
                dss.command("solve");
                assert!(
                    dss.errors().is_empty(),
                    "Rust solve errors: {:?}",
                    dss.errors()
                );
                assert!(cp.converged, "oracle non-convergence");
                elems += cp.all_properties.len();
                cmps += cp
                    .all_properties
                    .iter()
                    .map(|p| p.props.len())
                    .sum::<usize>();
                // The pilot sweeps the PINNED oracle only (see the universe
                // filter above), so the property policy is the capi one.
                harness::compare_all_properties(
                    &mut dss,
                    &cp.all_properties,
                    &tol,
                    harness::PropsChannel::CapiV0145,
                    label,
                );
            }
            (elems, cmps)
        }));
        match res {
            Ok((e, cm)) => {
                covered.push(label.clone());
                sweep_elems += e;
                sweep_cmps += cm;
            }
            Err(e) => failures.push((label.clone(), panic_msg(e))),
        }
        if (i + 1) % 10 == 0 {
            eprintln!("props: {}/{total} swept", i + 1);
        }
    }

    covered.sort();
    subset_skipped.sort();
    failures.sort();
    let report = json!({
        "total": total,
        "covered_count": covered.len(),
        "failed_count": failures.len(),
        "subset_skipped_count": subset_skipped.len(),
        "sweep_elements_x_steps": sweep_elems,
        "sweep_value_comparisons": sweep_cmps,
        "covered": covered,
        "subset_skipped": subset_skipped,
        "failures": failures
            .iter()
            .map(|(pth, r)| json!({
                "path": pth,
                "reason": r.chars().take(600).collect::<String>(),
            }))
            .collect::<Vec<_>>(),
    });
    let rp: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "tmp"]
        .iter()
        .collect::<PathBuf>()
        .join("props_report.json");
    let _ = std::fs::create_dir_all(rp.parent().unwrap());
    std::fs::write(&rp, serde_json::to_string_pretty(&report).unwrap())
        .unwrap_or_else(|e| panic!("write {}: {e}", rp.display()));
    eprintln!(
        "props: {} covered, {} failed, {} subset-skipped (of {total}); \
         {sweep_elems} element-steps × props = {sweep_cmps} value comparisons; report -> {}",
        covered.len(),
        failures.len(),
        subset_skipped.len(),
        rp.display()
    );
}

// ===========================================================================
// WP-AD.4 — the corpus-wide A-Diakoptics <-> normal sweep (rust-vs-rust).
// ===========================================================================

#[derive(Debug, Clone, Deserialize)]
struct AdSweepCase {
    path: String,
    ad: String,
}

#[derive(Debug, Deserialize)]
struct AdSweepManifest {
    #[serde(default)]
    cases: Vec<AdSweepCase>,
}

fn load_ad_sweep() -> Vec<AdSweepCase> {
    let p = manifests_dir().join("ad_sweep.json");
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    let m: AdSweepManifest =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()));
    m.cases
}

/// The AD-sweep node-voltage comparison ceiling (WP-AD.4).
const AD_SWEEP_TIER: f64 = 2.0e-3;

fn ad_node_voltages(dss: &Dss) -> BTreeMap<String, Complex64> {
    let ckt = dss.circuit().expect("circuit");
    (1..=ckt.num_nodes)
        .map(|i| (ckt.node_name(i), ckt.solution.node_v[i]))
        .collect()
}

fn ad_max_rel_gap(
    a: &BTreeMap<String, Complex64>,
    b: &BTreeMap<String, Complex64>,
) -> (f64, String, usize) {
    let mut worst = 0.0;
    let mut wn = String::new();
    let mut matched = 0usize;
    for (name, va) in a {
        if let Some(vb) = b.get(name) {
            matched += 1;
            let dv = (va - vb).norm();
            let base = va.norm();
            let rel = if base > 1e-6 { dv / base } else { dv };
            if rel > worst {
                worst = rel;
                wn = name.clone();
            }
        }
    }
    (worst, wn, matched)
}

fn ad_scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "dss_adsweep_{tag}_{}_{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap_or_else(|e| panic!("mkdir {}: {e}", d.display()));
    d
}

fn ad_solve_normal(abs: &str, controls_off: bool) -> Result<Dss, String> {
    let scratch = ad_scratch("norm");
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile \"{abs}\""));
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    if controls_off {
        dss.command("set controlmode=off");
    }
    dss.command("solve mode=snap");
    let ckt = dss.circuit().ok_or_else(|| "no circuit".to_string())?;
    if !ckt.is_solved {
        return Err(format!(
            "normal snapshot did not converge: {}",
            dss.result()
        ));
    }
    Ok(dss)
}

fn ad_solve_ad(abs: &str, controls_off: bool) -> Result<Dss, String> {
    let scratch = ad_scratch("ad");
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile \"{abs}\""));
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    if controls_off {
        dss.command("set controlmode=off");
    }
    dss.command("solve mode=snap");
    if !dss.circuit().is_some_and(|c| c.is_solved) {
        return Err(format!("base snapshot did not converge: {}", dss.result()));
    }
    dss.command("set Num_SubCircuits=2");
    let base_errs = dss.errors().len();
    dss.command("set ADiakoptics=True");
    if !dss.circuit().is_some_and(|c| c.solution.adiakoptics) {
        let msg = dss
            .errors()
            .get(base_errs)
            .map(|e| e.text().to_string())
            .unwrap_or_else(|| dss.result().to_string());
        return Err(format!("ad-init: {msg}"));
    }
    if !controls_off {
        let mode = dss
            .circuit()
            .map(|c| c.solution.default_control_mode)
            .unwrap_or_default();
        let mode_cmd = match mode {
            ControlMode::ControlsOff => "off",
            ControlMode::EventDriven => "event",
            ControlMode::TimeDriven => "time",
            ControlMode::Static | ControlMode::MultiRate => "static",
        };
        dss.command(&format!("set controlmode={mode_cmd}"));
    }
    dss.command("solve mode=snap");
    if !dss.circuit().is_some_and(|c| c.is_solved) {
        return Err(format!("AD snapshot did not converge: {}", dss.result()));
    }
    Ok(dss)
}

fn ad_run_case_abs(abs: &str, label: &str, ad: &str, full: bool) {
    let controls_off = !full;
    let _guard = CorpusGuard::new(abs);
    let vn = ad_node_voltages(
        &ad_solve_normal(abs, controls_off)
            .unwrap_or_else(|e| panic!("AD sweep {label}: normal arm failed: {e}")),
    );
    let va = ad_node_voltages(&ad_solve_ad(abs, controls_off).unwrap_or_else(|e| {
        panic!("AD sweep {label}: `{ad}` disposition but AD init/solve failed: {e}")
    }));
    let (gap, node, matched) = ad_max_rel_gap(&vn, &va);
    assert!(
        matched >= vn.len().saturating_sub(vn.len() / 20).max(1),
        "AD sweep {label}: only {matched}/{} nodes matched by name (mapping bug)",
        vn.len()
    );
    assert!(
        gap < AD_SWEEP_TIER,
        "AD sweep {label} ({ad}): AD-vs-normal node-V gap {gap:.3e} @ {node} exceeds \
         the AD tier {AD_SWEEP_TIER:.1e} -- classify off with a specific reason, \
         do not widen (section 5)"
    );
}

fn ad_classify_enabled() -> bool {
    std::env::var("DSS_AD_CLASSIFY")
        .map(|v| v == "1")
        .unwrap_or(false)
}

#[test]
fn ad_decompose_probe() {
    let Ok(rel) = std::env::var("DSS_AD_DECOMPOSE") else {
        eprintln!("SKIPPED ad_decompose: set DSS_AD_DECOMPOSE=<corpus-rel-path>");
        return;
    };
    let abs = corpus_file(&rel);
    let _guard = CorpusGuard::new(&abs);
    let vn = ad_node_voltages(&ad_solve_normal(&abs, true).expect("orig normal"));
    let scratch = ad_scratch("decomp");
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile \"{abs}\""));
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("set controlmode=off");
    dss.command("solve mode=snap");
    dss.command("set Num_SubCircuits=2");
    dss.command("set ADiakoptics=True");
    let inited = dss.circuit().is_some_and(|c| c.solution.adiakoptics);
    if inited {
        dss.command("solve mode=snap");
    }
    let va = ad_node_voltages(&dss);
    let inter = scratch
        .join("Torn_Circuit")
        .join("Master_Interconnected.dss");
    let mut di = Dss::new();
    di.command("clear");
    di.command(&format!(
        "compile \"{}\"",
        inter.display().to_string().replace('\\', "/")
    ));
    di.command("set controlmode=off");
    di.command("solve mode=snap");
    let solved_inter = di.circuit().is_some_and(|c| c.is_solved);
    let vi = ad_node_voltages(&di);
    let (leg1, n1, m1) = ad_max_rel_gap(&vn, &vi);
    let (leg2, n2, m2) = ad_max_rel_gap(&vi, &va);
    let (tot, nt, _) = ad_max_rel_gap(&vn, &va);
    eprintln!(
        "AD DECOMPOSE {rel}\n  inited={inited} inter_solved={solved_inter}\n  \
         leg1 save-roundtrip (orig-normal vs inter-normal) = {leg1:.3e} @ {n1} (matched {m1})\n  \
         leg2 AD (inter-normal vs AD)                       = {leg2:.3e} @ {n2} (matched {m2})\n  \
         total (orig-normal vs AD)                          = {tot:.3e} @ {nt}"
    );
}

#[test]
fn corpus_ad_matches_normal_mode() {
    let cases = load_ad_sweep();
    assert!(!cases.is_empty(), "ad_sweep.json must not be empty");
    for c in &cases {
        assert!(
            ad_disposition_is_valid(&c.ad),
            "{}: invalid ad disposition {:?}",
            c.path,
            c.ad
        );
    }
    if ad_classify_enabled() {
        ad_classify(&cases);
        return;
    }
    let start = Instant::now();
    let mut entries: Vec<(String, String, String)> = cases
        .iter()
        .map(|c| (corpus_file(&c.path), c.path.clone(), c.ad.clone()))
        .collect();
    for fam in ["asymmetric", "controls", "modes"] {
        for c in load_family(fam) {
            let ad = c
                .ad
                .clone()
                .unwrap_or_else(|| panic!("{fam}:{}: missing mandatory `ad` disposition", c.path));
            assert!(
                ad_disposition_is_valid(&ad),
                "{fam}:{}: invalid ad disposition {ad:?} (not in AD_OFF_REASONS)",
                c.path
            );
            entries.push((family_file(fam, &c.path), format!("{fam}/{}", c.path), ad));
        }
    }
    let total = entries.len();
    let (mut full, mut pf, mut off) = (0usize, 0usize, 0usize);
    for (abs, path, ad) in &entries {
        match ad.as_str() {
            "full" => {
                ad_run_case_abs(abs, path, ad, true);
                full += 1;
            }
            "pf" => {
                ad_run_case_abs(abs, path, ad, false);
                pf += 1;
            }
            _ => off += 1,
        }
    }
    eprintln!(
        "AD sweep: full={full} pf={pf} off={off} (of {total}: {} corpus + {} family) in {:.1}s",
        cases.len(),
        total - cases.len(),
        start.elapsed().as_secs_f64()
    );
}

#[test]
fn ad_classify_families() {
    if !ad_classify_enabled() {
        eprintln!("SKIPPED ad_classify_families: set DSS_AD_CLASSIFY=1");
        return;
    }
    let start = Instant::now();
    for fam in ["asymmetric", "controls", "modes"] {
        for c in load_family(fam) {
            if c.pending {
                println!(
                    "ADCLASSIFY\t{fam}/{}\toff:pending-feature\tNaN\tpending",
                    c.path
                );
                continue;
            }
            if c.expect_solve_abort.is_some() {
                println!(
                    "ADCLASSIFY\t{fam}/{}\toff:expect-solve-abort\tNaN\tabort",
                    c.path
                );
                continue;
            }
            let abs = family_file(fam, &c.path);
            let path = format!("{fam}/{}", c.path);
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _guard = CorpusGuard::new(&abs);
                let vn = ad_solve_normal(&abs, true).map(|d| ad_node_voltages(&d));
                let va = ad_solve_ad(&abs, true).map(|d| ad_node_voltages(&d));
                (vn, va)
            }));
            let (proposal, gap, detail) = ad_classify_outcome(res);
            println!("ADCLASSIFY\t{path}\t{proposal}\t{gap:.3e}\t{detail}");
        }
    }
    eprintln!(
        "AD classify families in {:.1}s",
        start.elapsed().as_secs_f64()
    );
}

#[allow(clippy::type_complexity)]
fn ad_classify_outcome(
    res: std::thread::Result<(
        Result<BTreeMap<String, Complex64>, String>,
        Result<BTreeMap<String, Complex64>, String>,
    )>,
) -> (String, f64, String) {
    match res {
        Err(e) => ("off:probe-panic".to_string(), f64::NAN, panic_msg(e)),
        Ok((Err(e), _)) => ("off:normal-fail".to_string(), f64::NAN, e),
        Ok((_, Err(e))) => {
            let cls = if e.contains("not lines") || e.contains("not a line") {
                "off:non-3ph-cut-only"
            } else if e.contains("error when tearing")
                || e.contains("cannot be compiled")
                || e.contains("Sub-Circuits")
                || e.contains("zone")
            {
                "off:too-small"
            } else {
                "off:ad-init-fail"
            };
            (cls.to_string(), f64::NAN, e)
        }
        Ok((Ok(vn), Ok(va))) => {
            let (gap, node, _matched) = ad_max_rel_gap(&vn, &va);
            let prop = if gap < AD_SWEEP_TIER {
                "pf"
            } else {
                "off:ad-gap"
            };
            (prop.to_string(), gap, node)
        }
    }
}

fn ad_classify(cases: &[AdSweepCase]) {
    let start = Instant::now();
    for c in cases {
        let path = c.path.clone();
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let abs = corpus_file(&path);
            let _guard = CorpusGuard::new(&abs);
            let vn = ad_solve_normal(&abs, true).map(|d| ad_node_voltages(&d));
            let va = ad_solve_ad(&abs, true).map(|d| ad_node_voltages(&d));
            (vn, va)
        }));
        let (proposal, gap, detail) = ad_classify_outcome(res);
        println!("ADCLASSIFY\t{path}\t{proposal}\t{gap:.3e}\t{detail}");
    }
    eprintln!(
        "AD classify: {} cases probed in {:.1}s",
        cases.len(),
        start.elapsed().as_secs_f64()
    );
}

/// WP-AD.4 bijection: `ad_sweep.json` must carry a disposition for EXACTLY the
/// `solvable_now.json` entry-point set.
#[test]
fn ad_sweep_covers_solvable_now() {
    let solvable: BTreeSet<String> = load_solvable()
        .into_iter()
        .map(|c| c.path.replace('\\', "/"))
        .collect();
    let sweep = load_ad_sweep();
    let swept: BTreeSet<String> = sweep.iter().map(|c| c.path.replace('\\', "/")).collect();
    assert_eq!(swept.len(), sweep.len(), "duplicate paths in ad_sweep.json");
    let missing: Vec<&String> = solvable.difference(&swept).collect();
    assert!(
        missing.is_empty(),
        "{} solvable_now path(s) missing an AD disposition in ad_sweep.json:\n  {}",
        missing.len(),
        missing
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );
    let extra: Vec<&String> = swept.difference(&solvable).collect();
    assert!(
        extra.is_empty(),
        "{} ad_sweep.json path(s) are not solvable_now entry points:\n  {}",
        extra.len(),
        extra
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );
    for c in &sweep {
        assert!(
            ad_disposition_is_valid(&c.ad),
            "{}: invalid ad disposition {:?} (full|pf|off:<reason>)",
            c.path,
            c.ad
        );
    }
    eprintln!(
        "ad_sweep.json: {} dispositions cover solvable_now exactly",
        sweep.len()
    );
}

/// Keep the AD off-reason allowlist referenced from this binary (authoritative
/// in `manifest`) so an accidental emptying trips a test here too.
#[test]
fn ad_off_reasons_are_nonempty() {
    assert!(
        !AD_OFF_REASONS.is_empty(),
        "AD_OFF_REASONS allowlist must not be empty"
    );
}

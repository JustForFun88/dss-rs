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
    // And for G1.8's settlement S-INC, which writes no ledger rows either: the
    // population where upstream's incidence ROW CURSOR runs past `Inc_Mat_Rows`
    // (it advances for every reactor, r4133 `Common/Solution.pas:3039`, while
    // the port emits dense rows) and the comparator therefore compares its
    // remapped answer. Re-derived from this run and pinned in both directions;
    // the rule lives beside the comparator (`harness::inc_matrix`, which has no
    // manifest access) while its ARMING predicate is read off the manifests here
    // — the G1.7 shape, restored by the G1.8 audit settlement so that a deleted
    // or per-channel-narrowed `if c.compare_inc_matrix` block in the runner reds
    // instead of silencing the whole surface.
    harness::inc_matrix::assert_declines_are_the_pinned_population(
        scheduler::inc_matrix_requested_channels(),
    );
    // And for G1.10a's D25/Q2 settlement, which writes no ledger rows either:
    // the population where an ORACLE reports the Pascal engines' harmonics disk
    // round-trip (`<CircuitName_>SavedVoltages.dbl`, r4133
    // `Common/Utilities.pas:1512-1521`) and the comparator splits that name off
    // BOTH sides — the port keeps that state in memory and writes no such file.
    // Re-derived from this run and pinned in both directions; the arming
    // predicate is read off the manifests, so a re-masked `compare_run_files`
    // request reds here instead of silently emptying the census.
    scheduler::assert_scratch_declines_are_the_pinned_population();
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

/// The five per-bus reads, capi marker first, r4133 marker second — ONE table,
/// so the fixed order and the cross-transport correspondence are the same fact.
/// (`CAPI_Alt.pas:2143`/`Bus.kVBase`/`:2251`/`:2573`/`:2540` == r4133
/// `DDLL/DBus.pas:319`/`BUSF(0)`/`:399`/`:659`/`:690`.)
const BUS_READ_ORDER: [(&str, &str); 5] = [
    ("b.Nodes", "engine.bus_nodes()"),
    ("b.kVBase", "engine.bus_kvbase()"),
    ("b.puVoltages", "engine.bus_pu_voltages()"),
    ("b.VMagAngle", "engine.bus_vmag_angle()"),
    ("b.puVmagAngle", "engine.bus_pu_vmag_angle()"),
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

    // Coordinator decision D35(3): this test drives BOTH oracle transports on a
    // corpus deck, so it is a producer in that case directory exactly like the
    // scheduler's own run and must hold the same exclusive claim — otherwise it
    // races the gate's cases (and the `#[test]`s that compile decks in place) in
    // the same binary, which is the shape the one-off
    // `espvlcontrol.dss [capi] "You must create a new circuit object first"` red
    // took (`runner::CorpusGuard`, D33(2)/D35(2)).
    let _guard = runner::CorpusGuard::new(&abs);

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
    }
    eprintln!(
        "cross-transport bus capture on asymmetric:line/line_asym.dss: \
         worst |capi − r4133| = {worst:.3e} V"
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

    // The same D35(3) claim its bus-surface sibling takes: a two-transport
    // producer in a corpus case directory holds the directory for the whole
    // case (`runner::CorpusGuard`).
    let _guard = runner::CorpusGuard::new(&abs);

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

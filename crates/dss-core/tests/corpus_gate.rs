//! Unified live corpus gate (`UNIFIED_GATE_PLAN.md`; successor of `corpus_live.rs`,
//! git-renamed to preserve history). For each solvable case the gate runs the
//! **Rust** engine once and compares it against the **pinned dss-python oracle**
//! (`capi_v0145` channel, served by a persistent worker pool) — node order, full
//! system Y, node voltages, every element's currents/powers/losses, YPrim blocks,
//! injection, discrete control state, monitors/meters, and the opt-in element
//! channels — per step, reusing `harness/mod.rs`. Target-rev cases (`capi015` /
//! `r3723` / `r4088` / `r4133`) keep the one-shot Oracle path (UPGRADE_PLAN.md).
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
#[path = "corpus_gate/manifest.rs"]
mod manifest;
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

use engines::Oracle;
use manifest::{
    AD_OFF_REASONS, FAMILIES, ORACLE_SPECS, SolvableCase, ad_disposition_is_valid, corpus_file,
    family_file, load_family, load_solvable, manifests_dir,
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
        panic!("{msg}");
    }
}

/// Write the contamination-proof artifact: a label-sorted
/// `{label: {verdict, result}}` map (verdict = "ok" or the first failure line;
/// result = the gate-relevant oracle model for live cases). Three runs (serial
/// one-shot, persistent parallel, persistent parallel shuffled) must bit-diff
/// EMPTY.
///
/// The per-checkpoint `all_properties` field is dropped from `result` before
/// dumping: its `RMatrix`/`XMatrix` (and the wider DoubleSymMatrix / shunt-PD
/// reliability) property values render UNINITIALIZED heap memory in the upstream
/// dss_capi getter — the documented UB that `harness::SKIP_PROPS` already
/// excludes from the VALUE compare (so it never affects a verdict). A persistent
/// worker's heap carries residue from prior cases where a fresh process's is
/// zeroed, so those garbage bytes are order-dependent by construction; keeping
/// them would make the bit-diff report the upstream UB, not real contamination.
/// Every field the gate actually asserts stays in the dump.
fn write_gate_dump(path: &str, run: &GateRun) {
    fn strip_ub_properties(v: &mut Value) {
        if let Some(cps) = v.get_mut("checkpoints").and_then(|c| c.as_array_mut()) {
            for cp in cps {
                if let Some(obj) = cp.as_object_mut() {
                    obj.remove("all_properties");
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

    let mut universe: Vec<(String, String, SolvableCase)> = Vec::new();
    for fam in FAMILIES {
        for c in load_family(fam.name) {
            if c.pending || c.oracle.is_some() || c.expect_solve_abort.is_some() {
                continue;
            }
            let abs = family_file(fam.name, &c.path);
            universe.push((format!("{}:{}", fam.name, c.path), abs, c));
        }
    }
    for c in load_solvable() {
        if c.oracle.is_some() || c.expect_solve_abort.is_some() {
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
                harness::compare_all_properties(&mut dss, &cp.all_properties, &tol, label);
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
// Opt-in A/B gate against ORIGINAL EPRI OpenDSS binaries (report-only).
// ===========================================================================

/// One triaged entry from `tests/corpus/known_diffs.json`.
#[derive(serde::Deserialize)]
struct KnownDiff {
    id: String,
    #[serde(default = "default_diff_kind")]
    kind: String,
    revs: Vec<String>,
    case_contains: String,
    #[serde(default)]
    reason_contains: Vec<String>,
    cause: String,
    #[allow(dead_code)]
    source: String,
}

fn default_diff_kind() -> String {
    "diff".to_string()
}

fn load_known_diffs() -> Vec<KnownDiff> {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "known_diffs.json",
    ]
    .iter()
    .collect();
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    #[derive(serde::Deserialize)]
    struct Catalog {
        entries: Vec<KnownDiff>,
    }
    let cat: Catalog =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
    for e in &cat.entries {
        assert!(
            !e.cause.trim().is_empty(),
            "known_diffs.json entry {:?}: `cause` is mandatory \
             (triage inventory, not a mute button)",
            e.id
        );
        match e.kind.as_str() {
            "diff" => assert!(
                !e.reason_contains.is_empty(),
                "known_diffs.json entry {:?}: a `diff` entry needs `reason_contains`",
                e.id
            ),
            "skip" => assert!(
                !e.case_contains.is_empty(),
                "known_diffs.json entry {:?}: a `skip` entry needs a non-empty \
                 `case_contains` (it matches on the case alone)",
                e.id
            ),
            k => panic!("known_diffs.json entry {:?}: unknown kind {k:?}", e.id),
        }
    }
    cat.entries
}

#[test]
fn corpus_live_opendss() {
    let Ok(rev) = std::env::var("DSS_LIVE_OPENDSS") else {
        eprintln!(
            "SKIPPED: set DSS_LIVE_OPENDSS=r3723|r4088|r4133 to compare against EPRI OpenDSS"
        );
        return;
    };
    assert!(
        matches!(rev.as_str(), "r3723" | "r4088" | "r4133"),
        "DSS_LIVE_OPENDSS={rev:?} — expected r3723|r4088|r4133 (tools/opendss/revisions.json)"
    );
    let assert_mode = std::env::var("DSS_LIVE_OPENDSS_ASSERT")
        .map(|v| v == "1")
        .unwrap_or(false);
    let oracle = Oracle::opendss(&rev);
    let engine = oracle.ping_engine(Some(&rev));
    eprintln!("opendss oracle ({rev}): {engine}");

    let mut universe: Vec<(String, String, SolvableCase)> = Vec::new();
    let mut target_rev_excluded: Vec<String> = Vec::new();
    for c in load_solvable() {
        if c.oracle.is_some() {
            target_rev_excluded.push(format!("solvable_now:{}", c.path));
            continue;
        }
        if c.expect_solve_abort.is_some() {
            continue;
        }
        let mut c = c;
        c.compare_all_properties = false;
        universe.push((format!("solvable_now:{}", c.path), corpus_file(&c.path), c));
    }
    for fam in FAMILIES {
        for c in load_family(fam.name) {
            if c.pending || c.expect_solve_abort.is_some() {
                continue;
            }
            if c.oracle.is_some() {
                target_rev_excluded.push(format!("{}:{}", fam.name, c.path));
                continue;
            }
            let abs = family_file(fam.name, &c.path);
            let mut c = c;
            c.compare_all_properties = false;
            universe.push((format!("{}:{}", fam.name, c.path), abs, c));
        }
    }
    if !target_rev_excluded.is_empty() {
        eprintln!(
            "opendss {rev}: {} target-rev case(s) excluded \
             (gated in the mandatory gate against their own `oracle` target)",
            target_rev_excluded.len()
        );
    }
    if universe.is_empty() {
        eprintln!(
            "opendss {rev}: WARNING swept universe is EMPTY ({} case(s) excluded as \
             target-rev) — the sweep is vacuous; rely on the mandatory gate",
            target_rev_excluded.len()
        );
    }

    let catalog = load_known_diffs();
    let mut hits: Vec<(String, usize)> = catalog.iter().map(|e| (e.id.clone(), 0)).collect();

    let mut matched: Vec<String> = Vec::new();
    let mut diverged: Vec<(String, String)> = Vec::new();
    let mut skipped: Vec<(String, String)> = Vec::new();
    let total = universe.len();
    for (i, (label, abs, c)) in universe.iter().enumerate() {
        if let Some(pos) = catalog.iter().position(|e| {
            e.kind == "skip" && e.revs.iter().any(|r| r == &rev) && label.contains(&e.case_contains)
        }) {
            hits[pos].1 += 1;
            skipped.push((label.clone(), catalog[pos].id.clone()));
            continue;
        }
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_and_compare(&oracle, label, abs, c);
        }));
        match res {
            Ok(()) => matched.push(label.clone()),
            Err(e) => diverged.push((label.clone(), panic_msg(e))),
        }
        if (i + 1) % 25 == 0 {
            eprintln!("opendss {rev}: {}/{total} compared", i + 1);
        }
    }

    matched.sort();
    diverged.sort();
    skipped.sort();

    let mut known: Vec<(String, String, String)> = Vec::new();
    let mut fresh: Vec<(String, String)> = Vec::new();
    for (label, reason) in &diverged {
        let hit = catalog.iter().position(|e| {
            e.kind == "diff"
                && e.revs.iter().any(|r| r == &rev)
                && label.contains(&e.case_contains)
                && e.reason_contains.iter().all(|s| reason.contains(s))
        });
        match hit {
            Some(i) => {
                hits[i].1 += 1;
                known.push((label.clone(), reason.clone(), catalog[i].id.clone()));
            }
            None => fresh.push((label.clone(), reason.clone())),
        }
    }
    for (id, n) in &hits {
        let applies = catalog
            .iter()
            .find(|e| &e.id == id)
            .is_some_and(|e| e.revs.iter().any(|r| r == &rev));
        if applies && *n == 0 {
            eprintln!(
                "opendss {rev}: WARNING known_diffs entry `{id}` had zero hits — \
                 stale? prune it (or narrow its `revs`)"
            );
        }
    }

    let trunc = |r: &str| r.chars().take(400).collect::<String>();
    let report = json!({
        "rev": rev,
        "engine": engine,
        "total": total,
        "matched": matched,
        "known_diverged": known
            .iter()
            .map(|(label, r, id)| json!({ "path": label, "known": id, "reason": trunc(r) }))
            .collect::<Vec<_>>(),
        "diverged_new": fresh
            .iter()
            .map(|(label, r)| json!({ "path": label, "reason": trunc(r) }))
            .collect::<Vec<_>>(),
        "known_skipped": skipped
            .iter()
            .map(|(label, id)| json!({ "path": label, "known": id }))
            .collect::<Vec<_>>(),
        "known_hits": hits
            .iter()
            .map(|(id, n)| json!({ "id": id, "hits": n }))
            .collect::<Vec<_>>(),
        "target_rev_excluded": target_rev_excluded,
    });
    let rp: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "tmp"]
        .iter()
        .collect::<PathBuf>()
        .join(format!("opendss_report_{rev}.json"));
    let _ = std::fs::create_dir_all(rp.parent().unwrap());
    std::fs::write(&rp, serde_json::to_string_pretty(&report).unwrap())
        .unwrap_or_else(|e| panic!("write {}: {e}", rp.display()));
    eprintln!(
        "opendss {rev}: {} matched, {} known-diverged, {} known-skipped, {} NEW (of {total}); \
         report -> {}",
        matched.len(),
        known.len(),
        skipped.len(),
        fresh.len(),
        rp.display()
    );
    if assert_mode && !fresh.is_empty() {
        panic!(
            "opendss {rev}: {} NEW divergence(s) from the EPRI engine not covered by \
             tests/corpus/known_diffs.json (DSS_LIVE_OPENDSS_ASSERT=1); see {}",
            fresh.len(),
            rp.display()
        );
    }
}

// ===========================================================================
// WP-AD.4 — the corpus-wide A-Diakoptics <-> normal sweep (rust-vs-rust).
// ===========================================================================

#[derive(Debug, Clone, Deserialize)]
struct AdSweepCase {
    path: String,
    ad: String,
    #[serde(default)]
    oracle: Option<String>,
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
            .cloned()
            .unwrap_or_else(|| dss.result().to_string());
        return Err(format!("ad-init: {msg}"));
    }
    if !controls_off {
        let mode = dss
            .circuit()
            .map(|c| c.solution.default_control_mode)
            .unwrap_or(0);
        let mode_cmd = match mode {
            -1 => "off",
            1 => "event",
            2 => "time",
            _ => "static",
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
        if let Some(spec) = &c.oracle {
            assert!(
                ORACLE_SPECS.contains(&spec.as_str()),
                "{}: unknown oracle spec {spec:?}",
                c.path
            );
        }
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

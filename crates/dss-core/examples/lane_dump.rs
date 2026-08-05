//! Stage F **parity↔default differential gate**, executable half
//! (`DE_PASCALIZE_PLAN.md` Part IV.2, "Two validation lanes … + the
//! differential gate"; step F.5).
//!
//! The two lanes are two *builds*, so no `#[test]` can compare them: a test
//! binary only ever contains one of them. This example is therefore the gate's
//! moving part — built once per lane, it walks the whole corpus manifest set,
//! solves every case on the Rust engine and writes one machine-readable record
//! per compared quantity. A second invocation (`diff`, lane-independent — it is
//! pure text arithmetic) reads the two dumps and checks them against the
//! documented per-kernel bounds.
//!
//! `tools/lanes/lane_diff.ps1` is the job that drives all three steps; see
//! `TESTING.md` §"The parity↔default differential gate" for how to run it and
//! what its output means.
//!
//! # Why this is the strongest default-lane test
//!
//! The parity lane is byte-exact against the committed report goldens and, on
//! the live oracles, is compared at the calibrated floors of
//! `tests/TOLERANCE_NOTES.md` (with its own pinned entries in
//! `tests/corpus/ledger.json`). It is *not* bitwise equal to the oracle — the
//! faer-vs-KLU last-ulp floors `CLAUDE.md` documents are real in both lanes —
//! so the honest chain is the triangle inequality:
//!
//! ```text
//! |default − oracle|  ≤  |default − parity|  +  |parity − oracle|
//! ```
//!
//! What makes this job the transitive proof is therefore not an assumed
//! premise but the **measured** left term. The 2026-07-31 run came back
//! `max |Δ| = 0` exactly on every gated kind, which makes the default lane
//! bit-identical to the parity lane and so gives it precisely the parity
//! lane's oracle standing — nothing is inherited by assumption.
//!
//! Read the second term back in whenever the first stops being zero: a future
//! non-zero `|Δ|` bounds `|default − oracle|` at *this job's bound plus the
//! case's tier*, not at this job's bound. Expect that in the MULTITHREADING
//! M3c and RESONANCE WP-R1 rungs, which are the two planned changes that make
//! the lanes genuinely diverge.
//!
//! While `|Δ| = 0` holds, the job is also sharper than the oracle comparison:
//! the oracle floors are 1e-6-class, the two lanes differ only by kernel ulps,
//! so a default-lane kernel regression that stayed inside a 1e-6 tier would
//! pass the corpus gate and fail here.
//!
//! # What is compared, and against what bound
//!
//! | record | quantity | rule |
//! |---|---|---|
//! | `errs` | engine error **count** after compile+post (not the text — the corpus gate reconciles messages) | exact |
//! | `conv` | converged flag per step | exact |
//! | `iter` | iteration count per step | ±[`ITER_SLACK`], the drift model's band |
//! | `v` | node voltage, per node per step | [`REL`] / [`ABS`] |
//! | `cur` | element terminal currents, per element per step | [`REL`] / [`ABS`] |
//! | `pow` | element terminal powers, per element per step | [`REL`] / [`ABS`] |
//! | `loss` | element losses, per element per step | [`REL`] / [`ABS`] |
//! | `y` | the assembled system Y, after the last step | [`REL`] / [`ABS`] |
//!
//! Record *keys* (case label, step, node/element name, matrix coordinate) are
//! compared **exactly** and in order, so a lane that renames, reorders, drops or
//! adds anything fails structurally before any number is looked at.
//!
//! That table is the whole dump — it is the solved-state checkpoint, not
//! "everything the corpus gate compares". Deliberately **not** dumped:
//! EnergyMeter registers, monitor channels, the event log, the control-action
//! queue, property probes and all report text; tap and switch changes appear
//! only indirectly, through `y` and `v`. Those surfaces are live-compared
//! against the oracles by the corpus gate in *both* lanes, which is where their
//! coverage comes from — so do not describe this job as dumping the "full"
//! checkpoint stream, and do not lean on it as the numeric guard for report
//! content it never reads.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use dss_core::exec::Dss;

// ---------------------------------------------------------------------------
// The documented bounds
// ---------------------------------------------------------------------------

/// Relative bound on every continuous quantity, and [`ABS`] its absolute floor:
/// a pair is accepted when `|a − b| ≤ ABS + REL·|b|` on the complex value —
/// the same form and the same numbers as the **tightest** calibrated oracle
/// tier (`harness::tol_for("micro")`: `v_rel`/`y_rel`/`i_rel` = 1e-9,
/// `*_abs` = 1e-6).
///
/// Reusing the micro tier rather than inventing a bound is deliberate, and it
/// is a *tightening*, not a loosening: the corpus's feeder/large tiers are
/// looser (1e-8 … 5e-6 rel), so every case here is held to a bound at or below
/// the one its own oracle comparison uses. `CLAUDE.md`'s no-fudging rule
/// applies unchanged — an excess is a finding to diagnose, never a number to
/// raise.
const REL: f64 = 1e-9;

/// Absolute floor of the continuous bound; see [`REL`].
const ABS: f64 = 1e-6;

/// Iteration-count slack between the lanes, in iterations — the drift model's
/// band, identical to `harness::lane::ITER_SLACK` (which grants the same slack
/// against the *oracle*). An ulp-level kernel difference can flip the
/// convergence test one step early or late and nothing more.
const ITER_SLACK: i64 = 1;

/// The rows that make the default lane deliberately answer something the parity
/// lane does not, keyed by `(case label, record kinds)`.
///
/// These are **not** silently skipped: the diff measures them like everything
/// else and prints what it measured, it just does not fail on them. Every entry
/// is hand-mirrored from an exclusion the corpus gate carries in
/// `harness::lane` — hand-mirrored, because an example cannot import the test
/// harness. Nothing checks the two lists against each other, so keep them in
/// step by hand; what *is* checked is that every entry here still fires, so a
/// stale one cannot sit around un-gating a field (fail-on-stale, added F-settle
/// W4 — the discipline the ledger, `ESCAPE_REGISTER` and `expected_rerounded`
/// already carry).
///
/// **Empty since `GOLDEN_REBASE_PLAN.md` G2.3.** Its only rows were the two
/// `modes:newton/` decks' `pow`/`loss`, where the parity lane reproduced
/// upstream's one-step-stale post-Newton `Iterminal` cache while the default
/// lane recomputed at the converged `NodeV` (`CLAUDE.md` upstream bug 5). Both
/// lanes now recompute, so those records are held to the ordinary bound like
/// every other one — and the exclusion that survives the teardown is against
/// the *oracles* (`harness::lane::LANE_SKIP_ELEM_POWERS`, now unconditional),
/// which this job never consults. A future lane split that is deliberate rather
/// than a bug adds its rows back here.
const DOCUMENTED_DIVERGENCES: &[(&str, &[&str])] = &[];

// ---------------------------------------------------------------------------
// Corpus enumeration
// ---------------------------------------------------------------------------

/// Repository root, from this crate's manifest directory.
fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

/// One dumped case: the gate's own label, the deck to compile, its manifest
/// `post` commands and step count.
struct Case {
    label: String,
    deck: PathBuf,
    post: Vec<String>,
    n_steps: usize,
    /// `false` for a `pending` deck (the port must error on it) or one whose
    /// solve aborts by design (`expect_solve_abort`) — the corpus gate compares
    /// no physical value there either, and driving them would only dump the
    /// shape of an error.
    solvable: bool,
}

/// Every manifest case, in the corpus gate's own order: `solvable_now` (decks
/// under the vendored `electricdss-tst`) then the three synthetic families.
///
/// Parsed as untyped JSON on purpose — `serde`'s derive is a dev-dependency of
/// this crate, and an example may not be the place that pulls it into the
/// product graph. The four fields read here are exactly the ones
/// `tests/corpus_gate/manifest.rs` uses to drive the Rust engine.
fn corpus_cases() -> Vec<Case> {
    let root = repo_root();
    let corpus = root.join("tests").join("corpus");
    let mut out = Vec::new();

    let sources: [(&str, PathBuf, PathBuf); 4] = [
        (
            "solvable_now",
            corpus.join("manifests").join("solvable_now.json"),
            corpus.join("electricdss-tst"),
        ),
        (
            "asymmetric",
            corpus.join("asymmetric").join("manifest.json"),
            corpus.join("asymmetric"),
        ),
        (
            "controls",
            corpus.join("controls").join("manifest.json"),
            corpus.join("controls"),
        ),
        (
            "modes",
            corpus.join("modes").join("manifest.json"),
            corpus.join("modes"),
        ),
    ];

    for (family, manifest, base) in sources {
        let text = std::fs::read_to_string(&manifest)
            .unwrap_or_else(|e| panic!("read {}: {e}", manifest.display()));
        let json: serde_json::Value = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("parse {}: {e}", manifest.display()));
        let cases = json["cases"]
            .as_array()
            .unwrap_or_else(|| panic!("{}: no `cases` array", manifest.display()));
        for c in cases {
            let path = c["path"].as_str().expect("case path").replace('\\', "/");
            let deck = base.join(&path);
            assert!(deck.is_file(), "corpus case missing: {}", deck.display());
            let pending = c["pending"].as_bool().unwrap_or(false);
            let aborts = c.get("expect_solve_abort").is_some_and(|v| !v.is_null());
            out.push(Case {
                label: format!("{family}:{path}"),
                deck,
                post: c["post"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .map(|s| s.as_str().expect("post command").to_string())
                            .collect()
                    })
                    .unwrap_or_default(),
                n_steps: c["n_steps"].as_u64().unwrap_or(1).max(1) as usize,
                solvable: !pending && !aborts,
            });
        }
    }
    assert!(
        out.len() > 400,
        "only {} corpus cases enumerated — the manifest walk is broken",
        out.len()
    );
    out
}

// ---------------------------------------------------------------------------
// dump
// ---------------------------------------------------------------------------

/// `f64` in the shortest form that round-trips exactly (Rust's `Display`), so
/// the dump is compact *and* loses no bit of the value it records.
fn num(x: f64) -> String {
    format!("{x}")
}

/// Write one record: `kind<TAB>key<TAB>v0 v1 …`.
///
/// The separator is a **tab** because a DSS object name may legally contain
/// almost anything else: the corpus holds a `Line.b1||b2`, which a `|`-separated
/// record silently reshapes into extra fields (it did, until the first run of
/// this job hit that deck).
fn rec(w: &mut impl Write, kind: &str, key: &str, values: &[f64]) {
    // A plain `assert!`, not `debug_assert!`: the job runs `--release`
    // (`tools/lanes/lane_diff.ps1`) and the workspace sets no
    // `[profile.release] debug-assertions`, so a debug assertion here would be
    // switched off in the only configuration that ever executes this code.
    assert!(
        !key.contains('\t'),
        "record key contains the separator: {key}"
    );
    let mut line = String::with_capacity(32 + 24 * values.len());
    let _ = write!(line, "{kind}\t{key}\t");
    for (i, v) in values.iter().enumerate() {
        if i > 0 {
            line.push(' ');
        }
        line.push_str(&num(*v));
    }
    writeln!(w, "{line}").expect("dump write");
}

/// Solve every corpus case on this build's engine and write the record stream
/// to `out`.
///
/// The case label and the step index are carried by their own `case`/`step`
/// records rather than repeated in every key — the dump is ~215 MB per lane
/// even so, and repeating an 80-character corpus path on each of its ~3.2
/// million value records would nearly triple that for no added information. The
/// diff tracks the same two registers while streaming, so a failure still names
/// its case and step.
fn dump(out: &Path) {
    let file = File::create(out).unwrap_or_else(|e| panic!("create {}: {e}", out.display()));
    let mut w = BufWriter::new(file);
    let lane = if dss_core::compat::ORACLE_PARITY {
        "parity"
    } else {
        "default"
    };
    writeln!(w, "lane\t{lane}\t").expect("dump write");

    let cases = corpus_cases();
    eprintln!("[lane_dump] lane={lane} cases={}", cases.len());
    for (n, case) in cases.iter().enumerate() {
        writeln!(w, "case\t{}\t", case.label).expect("dump write");
        if !case.solvable {
            writeln!(w, "skip\t\t").expect("dump write");
            continue;
        }
        if n % 50 == 0 {
            eprintln!("[lane_dump] {n}/{} {}", cases.len(), case.label);
        }
        let mut dss = Dss::new();
        dss.command("clear");
        dss.command(&format!("compile \"{}\"", case.deck.display()));
        for cmd in &case.post {
            dss.command(cmd);
        }
        rec(&mut w, "errs", "", &[dss.errors().len() as f64]);
        if dss.circuit().is_none() {
            continue;
        }

        for step in 0..case.n_steps {
            dss.command("solve");
            writeln!(w, "step\t{step}\t").expect("dump write");
            {
                let Some(ckt) = dss.circuit() else { break };
                rec(&mut w, "conv", "", &[f64::from(u8::from(ckt.is_solved))]);
                rec(&mut w, "iter", "", &[f64::from(ckt.solution.iteration)]);
                for j in 1..=ckt.num_nodes {
                    let v = ckt.solution.node_v[j];
                    rec(&mut w, "v", &ckt.node_name(j), &[v.re, v.im]);
                }
            }
            for e in dss.snapshot_elements() {
                let flat = |c: &[num_complex::Complex64]| -> Vec<f64> {
                    c.iter().flat_map(|z| [z.re, z.im]).collect()
                };
                rec(&mut w, "cur", &e.name, &flat(&e.currents));
                rec(&mut w, "pow", &e.name, &flat(&e.powers));
                rec(&mut w, "loss", &e.name, &[e.loss_w.0, e.loss_w.1]);
            }
        }

        // The assembled system Y once per case, in the state the last step left
        // it (so a lane that took a different tap/switch path shows up here as
        // well as in the voltages). Sorted by coordinate: the assembly order is
        // a shared kernel, but the record stream must not depend on it.
        if let Some((_, mut trip)) = dss.system_y_csc() {
            trip.sort_by_key(|(r, c, _)| (*r, *c));
            for (r, c, z) in trip {
                rec(&mut w, "y", &format!("{r}#{c}"), &[z.re, z.im]);
            }
        }
    }
    w.flush().expect("dump flush");
    eprintln!("[lane_dump] wrote {}", out.display());
}

// ---------------------------------------------------------------------------
// diff
// ---------------------------------------------------------------------------

/// Running measurement of one record kind: how many values were compared and
/// the largest deviation seen, with the record it happened on.
#[derive(Default)]
struct Stat {
    compared: usize,
    max_abs: f64,
    max_abs_at: String,
    max_rel: f64,
    max_rel_at: String,
}

impl Stat {
    fn see(&mut self, diff: f64, mag: f64, key: &str) {
        self.compared += 1;
        if diff > self.max_abs {
            self.max_abs = diff;
            self.max_abs_at = key.to_string();
        }
        // Relative to the reference magnitude; a difference against a zero
        // reference is reported through `max_abs` only (a relative measure has
        // no meaning there, and inventing one would print `inf`).
        if mag > 0.0 {
            let rel = diff / mag;
            if rel > self.max_rel {
                self.max_rel = rel;
                self.max_rel_at = key.to_string();
            }
        }
    }
}

/// Split `kind<TAB>key<TAB>values` — the record shape [`rec`] writes.
fn split_record(line: &str, file: &Path, n: usize) -> (String, String, Vec<f64>) {
    let mut it = line.splitn(3, '\t');
    let kind = it.next().unwrap_or_default().to_string();
    let key = it.next().unwrap_or_default().to_string();
    let rest = it.next().unwrap_or_default();
    let values = rest
        .split_whitespace()
        .map(|t| {
            t.parse::<f64>()
                .unwrap_or_else(|e| panic!("{}:{n}: not a number: {t:?} ({e})", file.display()))
        })
        .collect();
    (kind, key, values)
}

/// Is this record one of the [`DOCUMENTED_DIVERGENCES`]?
fn is_documented(case: &str, kind: &str) -> bool {
    DOCUMENTED_DIVERGENCES
        .iter()
        .any(|(label, kinds)| *label == case && kinds.contains(&kind))
}

/// Compare two dumps and report; exits non-zero when a bound is exceeded or the
/// record streams are not structurally identical.
fn diff(a: &Path, b: &Path) {
    let open = |p: &Path| {
        BufReader::new(File::open(p).unwrap_or_else(|e| panic!("open {}: {e}", p.display())))
    };
    let mut la = open(a).lines();
    let mut lb = open(b).lines();

    let mut stats: BTreeMap<String, Stat> = BTreeMap::new();
    let mut documented: BTreeMap<String, Stat> = BTreeMap::new();
    let mut failures: Vec<String> = Vec::new();
    let mut iter_drift: Vec<String> = Vec::new();
    let mut structural: Option<String> = None;
    let mut n = 0usize;
    let mut cases = 0usize;
    // The two context registers the record stream carries instead of repeating
    // the case label on every value line.
    let mut case = String::new();
    let mut step = String::new();
    // The `lane` headers of the two dumps, checked after the walk.
    let mut lanes: Option<(String, String)> = None;
    // Non-finite values on either side, which the bound checks below cannot
    // see: every comparison against a NaN is false, in both directions.
    let mut nonfinite: Vec<String> = Vec::new();

    loop {
        let (ra, rb) = (la.next(), lb.next());
        match (ra, rb) {
            (None, None) => break,
            (Some(_), None) | (None, Some(_)) => {
                structural = Some(format!(
                    "record streams have different lengths (diverge at record {})",
                    n + 1
                ));
                break;
            }
            (Some(ra), Some(rb)) => {
                n += 1;
                let (ra, rb) = (ra.expect("read a"), rb.expect("read b"));
                let (ka, key_a, va) = split_record(&ra, a, n);
                let (kb, key_b, vb) = split_record(&rb, b, n);
                if ka == "lane" {
                    // The two headers must name *different* lanes, and between
                    // them exactly {default, parity}. Skipping the comparison
                    // (what this did until F-settle W4) meant the one job that
                    // certifies default ≈ oracle could not tell a lane from
                    // itself: two dumps both headed `default` diffed clean and
                    // exited 0, so a Cargo slip that stopped `oracle-parity`
                    // reaching the example, or a stale `parity.dump` under
                    // `-SkipDump`, read as a green "bit-identical" run that
                    // proved nothing. Verified by probe.
                    lanes = Some((key_a.clone(), key_b.clone()));
                    continue;
                }
                if ka != kb || key_a != key_b || va.len() != vb.len() {
                    structural = Some(format!(
                        "record {n} differs structurally (case {case:?} step {step:?}):\n  \
                         {}: {ra}\n  {}: {rb}",
                        a.display(),
                        b.display()
                    ));
                    break;
                }
                match ka.as_str() {
                    "case" => {
                        case = key_a;
                        step.clear();
                        cases += 1;
                        continue;
                    }
                    "step" => {
                        step = key_a;
                        continue;
                    }
                    "skip" => continue,
                    _ => {}
                }

                let deliberate = is_documented(&case, &ka);
                let where_ = format!("{case}#{step} {ka} {key_a}");

                // A NaN or an infinity is never a documented divergence, and it
                // is invisible to every bound below (`d > allowed` and
                // `diff > max_abs` are both false for NaN, so it does not even
                // reach the statistics). A default lane that started producing
                // NaN voltages while still flagging converged is the single
                // worst regression this job exists to catch, so it is a hard
                // failure regardless of `deliberate` (probe-confirmed silent
                // pass before F-settle W4).
                for (side, vals) in [(a, &va), (b, &vb)] {
                    if let Some(bad) = vals.iter().find(|x| !x.is_finite()) {
                        nonfinite.push(format!("  {where_}: {bad} in {}", side.display()));
                    }
                }
                let bucket = if deliberate {
                    documented.entry(format!("{case} {ka}")).or_default()
                } else {
                    stats.entry(ka.clone()).or_default()
                };
                let hard_fail = !deliberate;

                match ka.as_str() {
                    // Exact integers: error counts and the converged flag.
                    "errs" | "conv" => {
                        for (x, y) in va.iter().zip(&vb) {
                            bucket.see((x - y).abs(), y.abs(), &where_);
                            if x != y && hard_fail {
                                failures.push(format!("  {where_}: {x} vs {y} (exact)"));
                            }
                        }
                    }
                    "iter" => {
                        for (x, y) in va.iter().zip(&vb) {
                            let d = x - y;
                            bucket.see(d.abs(), y.abs(), &where_);
                            if d != 0.0 {
                                iter_drift.push(format!("{where_}: {x} vs {y} ({d:+})"));
                            }
                            if d.abs() > ITER_SLACK as f64 && hard_fail {
                                failures.push(format!(
                                    "  {where_}: {x} vs {y} (drift {d:+}, slack ±{ITER_SLACK})"
                                ));
                            }
                        }
                    }
                    // Complex continuous quantities, compared per (re, im) pair.
                    _ => {
                        for (i, (za, zb)) in va.chunks_exact(2).zip(vb.chunks_exact(2)).enumerate()
                        {
                            let (dr, di) = (za[0] - zb[0], za[1] - zb[1]);
                            let d = dr.hypot(di);
                            let mag = zb[0].hypot(zb[1]);
                            bucket.see(d, mag, &where_);
                            let allowed = ABS + REL * mag;
                            if d > allowed && hard_fail {
                                failures.push(format!(
                                    "  {where_}[{i}]: ({}, {}) vs ({}, {}) — |Δ| = {d:e} > \
                                     allowed {allowed:e}",
                                    za[0], za[1], zb[0], zb[1]
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    println!("parity<->default differential gate");
    println!("  a = {}", a.display());
    println!("  b = {}", b.display());
    println!("  cases: {cases}, records: {n}");
    println!("  bounds: |d| <= {ABS:e} + {REL:e}*|b|, iterations +-{ITER_SLACK}");
    println!();
    println!(
        "  {:<6} {:>10}  {:>10}  {:>10}   worst record",
        "kind", "compared", "max |d|", "max rel"
    );
    for (kind, s) in &stats {
        println!(
            "  {:<6} {:>10}  {:>10.3e}  {:>10.3e}   {}",
            kind,
            s.compared,
            s.max_abs,
            s.max_rel,
            if s.max_abs > 0.0 {
                &s.max_abs_at
            } else {
                "(identical)"
            }
        );
    }
    println!();
    println!("  iteration counts that drifted: {}", iter_drift.len());
    for d in iter_drift.iter().take(20) {
        println!("    {d}");
    }
    if documented.is_empty() {
        println!("  documented divergences: none present in this dump");
    } else {
        println!("  documented divergences (measured, not gated):");
        for (what, s) in &documented {
            println!(
                "    {what}: {} values, max |Δ| = {:.3e}, max rel = {:.3e}",
                s.compared, s.max_abs, s.max_rel
            );
        }
    }
    println!();

    if let Some(msg) = structural {
        println!("VERDICT: FAIL (structural)\n{msg}");
        std::process::exit(1);
    }
    // The job's whole claim is "these two builds agree", so it has to have
    // compared two different builds.
    match &lanes {
        None => {
            println!("VERDICT: FAIL (structural)\n  neither dump carries a `lane` header");
            std::process::exit(1);
        }
        Some((la, lb)) => {
            let mut got = [la.as_str(), lb.as_str()];
            got.sort_unstable();
            if got != ["default", "parity"] {
                println!(
                    "VERDICT: FAIL (structural)\n  the two dumps are lanes {la:?} and {lb:?} — \
                     expected one `default` and one `parity`. Comparing a lane with itself \
                     proves nothing; rebuild the missing lane (see tools/lanes/lane_diff.ps1)."
                );
                std::process::exit(1);
            }
        }
    }
    if !nonfinite.is_empty() {
        println!(
            "VERDICT: FAIL — {} non-finite value(s); no bound can see these:",
            nonfinite.len()
        );
        for f in nonfinite.iter().take(40) {
            println!("{f}");
        }
        if nonfinite.len() > 40 {
            println!("  … and {} more", nonfinite.len() - 40);
        }
        std::process::exit(1);
    }
    if !failures.is_empty() {
        println!(
            "VERDICT: FAIL — {} value(s) outside the bound:",
            failures.len()
        );
        for f in failures.iter().take(40) {
            println!("{f}");
        }
        if failures.len() > 40 {
            println!("  … and {} more", failures.len() - 40);
        }
        std::process::exit(1);
    }
    // Non-vacuity: a dump pair that compared nothing must never read as a pass.
    assert!(
        n > 100_000 && stats.contains_key("v") && stats.contains_key("y"),
        "the diff compared {n} records and {} kinds — that is not a corpus dump",
        stats.len()
    );
    // Fail-on-stale: an entry that no longer matches anything is an ungated
    // field nobody is watching.
    let stale: Vec<String> = DOCUMENTED_DIVERGENCES
        .iter()
        .flat_map(|(label, kinds)| kinds.iter().map(move |k| format!("{label} {k}")))
        .filter(|key| !documented.contains_key(key))
        .collect();
    assert!(
        stale.is_empty(),
        "stale DOCUMENTED_DIVERGENCES entries — these matched no record in this \
         dump, so they exempt a field that is no longer there: {stale:?}"
    );
    println!("VERDICT: PASS");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["dump", out] => dump(Path::new(out)),
        ["diff", a, b] => diff(Path::new(a), Path::new(b)),
        _ => {
            eprintln!(
                "usage:\n  lane_dump dump <out-file>       # solve the corpus, write the records\n  \
                 lane_dump diff <lane-a> <lane-b>  # compare two dumps against the Stage F bounds\n\n\
                 Drive both through tools/lanes/lane_diff.ps1 (TESTING.md)."
            );
            std::process::exit(2);
        }
    }
}

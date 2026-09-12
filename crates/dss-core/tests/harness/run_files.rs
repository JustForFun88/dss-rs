//! `GOLDEN_REBASE_PLAN.md` WP-G1 sub-step **G1.10a** — the run-produced FILE
//! SET, compared live on every gating channel of the unified corpus gate.
//!
//! # The surface
//!
//! For one (case, channel, port-run): the **set** of filesystem entries created
//! under the case directory — which is `OutputDirectory := DataDirectory :=
//! <case dir>`, set by `Compile` -> `DSSGlobals.SetDataPath` (r4133
//! `Version8/Source/Common/DSSGlobals.pas:962`, capi 0.14.5 twin
//! `src/Common/DSSGlobals.pas:563`) — by the run
//! `clear -> compile -> post -> n_steps x solve`. Members are `/`-joined,
//! `./`-stripped, ASCII-case-folded case-dir-relative paths, with a trailing `/`
//! marking a created **directory** (whose contents are separate members).
//! Sorted, compared as an exact discrete set at `rel = abs = 0`.
//!
//! Precisely: **the case dir's own entries, plus everything under a directory
//! the run created** (coordinator decision D30(2)). A PRE-EXISTING subdirectory
//! is not descended into on any producer — every executive writer resolves its
//! name against `OutputDirectory` (`GetOutputDirectory + CircuitName_ +
//! FileName`, r4133 `Version8/Source/Executive/ExportOptions.pas:401`), so a run
//! writes into the case dir or into a tree it creates, never into a
//! subdirectory that already existed; while the gate schedules by case *dir*,
//! so a file appearing under one belongs to a concurrently running sibling case
//! (`dss_epri::guard::CorpusGuard::classify` carries the measurement).
//!
//! Purely observational: no command is added to any run on any transport, so
//! nothing about the compared model moves. What it gates is everything the
//! executive writes to disk — `Export`, `Show`, `Dump`, `Visualize`, the
//! `CloseDI` demand-interval tree, `Save`, the monitor CSVs' NAMES (their
//! contents stay out: monitor data is f32 and unsampled-monitor comparison is a
//! known oracle artifact, `TESTING.md`).
//!
//! # The CONTENTS half (G1.10b)
//!
//! Sub-step G1.10b adds the bytes of the members the gate SELECTS
//! ([`dss_epri::guard::RUN_FILE_CONTENTS_PATTERNS`] — the report CSVs whose kind
//! its own name identifies). The selection is computed once on the gate side and
//! travels in the run request, so neither oracle transport re-derives it; the
//! two transports copy the selected files into a gate-owned sidecar directory
//! (coordinator decision D40(6)) at the same strictly-last point inside the
//! guard scope where they classify the set, and this module's probe reads them
//! in place ([`RunFileProbe::finish_and_clean`]). [`read_sidecar`] decodes and
//! deletes; [`compare_run_file_contents`] pairs the sides by name and hands the
//! pairs to the per-report cell comparison.
//!
//! Provenance: new coverage, not catch-up. The upstream harness never compared
//! this set — fastdss's `tests/compare_outputs.py` skips a name missing on the
//! other side (`except KeyError: ... continue`, `:416-421`) and prints instead
//! of failing on a CSV mismatch (`except: print("COMPARE CSV ERROR:", fn)`,
//! `:517-524`).
//!
//! # One classification, three producers
//!
//! The two oracle transports report the set from the very `CorpusGuard` that
//! sweeps the corpus clean after the run
//! (`tools/oracle/corpus_guard.py::CorpusGuard.created` on the `capi_v0145`
//! channel, `dss_epri::guard::CorpusGuard::created` on `r4133`), so a reported
//! set can never disagree with the swept set. [`RunFileProbe`] is the third
//! producer and is a thin wrapper over the *same* Rust guard rather than a
//! fourth copy of the classification.
//!
//! The port needs its own bracket, and must REMOVE what it classified, for one
//! measured reason: `corpus_gate::scheduler::run_one_case` takes one outer
//! `runner::CorpusGuard` per case and then, for an `engines: "both"` case, runs
//! the Rust engine **twice** — once per channel, sequentially, in the same
//! directory. A port-side set derived from that outer guard's snapshot would be
//! correct for channel 1 and empty for channel 2, because channel 1's files are
//! already on disk. The probe's own before/after bracket plus its own sweep make
//! the two channel runs symmetric. The outer guard stays the safety net and is
//! never bypassed.
//!
//! # Normalization, not tolerance
//!
//! The case fold is a CROSS-ORACLE normalization in the `props_norm` /
//! coordinator-decision-D4 tradition: r4133 writes `EXP_VOLTAGES.CSV`
//! (`Version8/Source/Executive/ExportOptions.pas:333-356`) while dss_capi 0.14.5
//! writes `EXP_VOLTAGES.csv` (`src/Executive/ExportOptions.pas:314,343,345,381,437`),
//! and r4133 additionally lowercases deck-supplied stems
//! (`Auto1bus_HL_current.txt` -> `auto1bus_hl_current.txt`), so NO single
//! spelling satisfies both gating channels. NTFS is case-insensitive, so nothing
//! observable depends on the case. There is no floor anywhere in this module —
//! see `tests/TOLERANCE_NOTES.md` for why "no floor" is a derivation here and
//! not an omission.
//!
//! The second normalization is coordinator decision **D25/Q2**: the Pascal
//! engines round-trip the fundamental solution through DISK when harmonics is
//! initialized (`<CircuitName_>SavedVoltages.dbl`, r4133
//! `Common/Utilities.pas:1512-1521` `SavePresentVoltages`, read back by
//! `RetrieveSavedVoltages` `:1554-1564`), while the port keeps that vector in
//! memory (`crates/dss-core/src/solution/solution/state.rs:329-331`,
//! `solution/solution/harmonics.rs:222`). That name is split off SYMMETRICALLY
//! on every producer by [`dss_epri::guard::split_engine_scratch`], **counted**
//! into this module's decline census (0 `ledger.json` rows), and the port side
//! is ASSERTED empty — a positive statement of the port's behaviour, never a
//! two-sided skip.
//!
//! # Platform
//!
//! The classification itself ([`dss_epri::guard`]) is pure `std::fs` and builds
//! everywhere — coordinator decision D33(3) ungated it precisely so the gate's
//! own corpus guard needs no second copy of the rule off Windows. This module
//! is nevertheless declared under `#[cfg(windows)]` in `harness/mod.rs`: the
//! surface is only ever compared against the gate's two oracle channels, and
//! the `r4133` transport is a Win64 DLL. On any other platform the corpus gate
//! refuses a case that sets `compare_run_files` loudly
//! (`corpus_gate::runner::compare_with_result`) instead of silently comparing
//! nothing.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use dss_epri::guard::{CorpusGuard, normalize_created_name, split_engine_scratch};

use super::capture_guard;

// ---------------------------------------------------------------------------
// The port-side producer.
// ---------------------------------------------------------------------------

/// The port's before/after bracket over one Rust run of one case+channel.
///
/// A newtype over [`dss_epri::guard::CorpusGuard`] on purpose: the pre-run
/// snapshot, the created-entry classification and the sweep are then literally
/// the same code the `r4133` transport runs, so the port and the oracle can
/// never disagree about *what counts as created* — only about what was created.
///
/// Lifecycle: [`RunFileProbe::start`] immediately before the Rust engine runs,
/// [`RunFileProbe::finish_and_clean`] after the last read of that run (the
/// `compare_autoadd_log` arm reads a Rust-written file off disk, so it must come
/// first). `finish_and_clean` consumes the probe: the report is taken while the
/// guard is alive, and the guard is then `finish`ed — it removes exactly what it
/// classified, restores any pre-existing file the run overwrote, and hands back
/// anything it could NOT remove, which fails the case (D33(3)).
pub struct RunFileProbe {
    guard: CorpusGuard,
}

/// What one port run left on disk: the created-file SET (G1.10a) and the
/// CONTENTS of the members the gate selected (G1.10b).
///
/// One struct because both halves must be taken from the SAME bracket, before
/// the sweep: reading the contents after `finish()` would read files that are
/// already gone, and re-bracketing to get them would classify a different run.
pub struct RunFileReport {
    /// Every filesystem entry the run created under the case dir, normalized
    /// and sorted — the surface [`compare_run_files`] compares.
    pub created: Vec<String>,
    /// The decoded contents of the created members the gate selected
    /// ([`dss_epri::guard::RUN_FILE_CONTENTS_PATTERNS`]), keyed by normalized
    /// name — the surface [`compare_run_file_contents`] compares.
    pub contents: BTreeMap<String, String>,
}

impl RunFileProbe {
    /// Snapshot the case directory before the Rust engine touches it.
    pub fn start(case_path: &str) -> RunFileProbe {
        RunFileProbe {
            guard: CorpusGuard::new(case_path),
        }
    }

    /// Classify what the run created, then sweep it away — and fail the case if
    /// the sweep could not remove all of it.
    ///
    /// Panics — fails the case — in two situations.
    ///
    /// 1. The classification cannot be reported honestly (an incomplete pre-run
    ///    snapshot, or a directory listing that failed now). That mirrors the
    ///    guards' "an incomplete snapshot never deletes" rule in the reporting
    ///    direction: a set we are not sure of must not be compared. The sweep
    ///    still runs, because it only ever removes entries it did classify.
    /// 2. The sweep left a created entry behind (coordinator decisions
    ///    D32(2)/D33(3)). The port is a producer exactly like the two oracle
    ///    transports, and a dropping it cannot remove poisons the NEXT
    ///    producer's pre-run snapshot the same way — the entry is then
    ///    "pre-existing" and silently drops out of that producer's created set.
    ///    `corpus_gate::runner::compare_with_result` fails the case on an
    ///    oracle's `sweep_failed`; this is the port's half of that rail, so the
    ///    order-coupling cannot hide on the one producer that used to report
    ///    its leak to stderr only (`dss_epri::guard::CorpusGuard`'s `Drop`).
    #[track_caller]
    pub fn finish_and_clean(mut self, ctx: &str) -> RunFileReport {
        let created = self.guard.created();
        // G1.10b: the port's half of the CONTENTS surface, taken while the files
        // are still on disk — inside this bracket and before the sweep, which is
        // the same "strictly last, inside the guard scope" point the two oracle
        // transports copy at (`crates/dss-epri/src/capture.rs`,
        // `tools/oracle/oracle_server.py`). No sidecar here: the gate and this
        // producer are one process, so the bytes need no transport — but the
        // selection and the decode are the shared ones, so a difference between
        // the sides can only be the numbers in the files.
        let contents = created.as_deref().map(|names| {
            dss_epri::guard::read_selected_contents(
                self.guard.dir(),
                names,
                &dss_epri::guard::RUN_FILE_CONTENTS_PATTERNS[..],
            )
        });
        // Sweep + restore NOW instead of on drop: `finish` hands back what it
        // could not remove, where the drop path can only print it.
        let leaked = self.guard.finish();
        assert!(
            leaked.is_empty(),
            "{ctx}: the PORT left {} leaked dropping(s) in the case directory that \
             its own run-file probe classified as run-created and could not \
             remove: {}. A file the engine still holds open poisons every later \
             producer's pre-run snapshot (and pollutes the vendored corpus) — fix \
             the writer (open-append-close, never a held handle), never widen the \
             surface around it.",
            leaked.len(),
            leaked.join(", "),
        );
        let created = created.unwrap_or_else(|| {
            panic!(
                "{ctx}: the port's run-file probe could not list the case directory \
                 (incomplete pre-run snapshot, or a `read_dir` failure during the \
                 run). The created-file SET is compared as an exact discrete set, so \
                 an incomplete listing fails the case instead of being reported as \
                 what the run created."
            )
        });
        let contents = contents
            .expect("the created set is Some here, so the contents read ran")
            .unwrap_or_else(|e| panic!("{ctx}: {e}"));
        RunFileReport { created, contents }
    }
}

// ---------------------------------------------------------------------------
// The D25/Q2 scratch-file census — re-derived on every gate run, pinned by the
// scheduler (fail-on-stale in BOTH directions).
// ---------------------------------------------------------------------------

/// `(case label, scratch name)` for every engine-internal scratch file an ORACLE
/// channel reported and the port, by design, does not write.
static SCRATCH_DECLINES: Mutex<BTreeSet<(String, String)>> = Mutex::new(BTreeSet::new());
/// Channel visits behind [`SCRATCH_DECLINES`] (a `both` case counted twice).
static SCRATCH_VISITS: AtomicUsize = AtomicUsize::new(0);
/// (case, channel) run-file comparisons actually performed — the non-vacuity
/// half: a census of `0 / 0` means nothing was compared, not that nothing
/// declined.
static COMPARED: AtomicUsize = AtomicUsize::new(0);

/// The D25/Q2 census as it stands, for the gate epilogue and the scheduler's
/// fail-on-stale population constant.
pub struct ScratchCensus {
    /// Distinct cases in which an oracle reported an engine-internal scratch file.
    pub cases: usize,
    /// Distinct `(case, scratch name)` pairs.
    pub names: usize,
    /// Channel visits that reported at least one scratch file.
    pub visits: usize,
    /// (case, channel) comparisons performed.
    pub compared: usize,
}

fn lock<T>(m: &'static Mutex<T>) -> std::sync::MutexGuard<'static, T> {
    // A poisoned census must not fail a different case: whoever poisoned it
    // already failed its own.
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The census as it stands (see [`ScratchCensus`]).
pub fn scratch_census() -> ScratchCensus {
    let declines = lock(&SCRATCH_DECLINES);
    ScratchCensus {
        cases: declines
            .iter()
            .map(|(c, _)| c.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        names: declines.len(),
        visits: SCRATCH_VISITS.load(Ordering::Relaxed),
        compared: COMPARED.load(Ordering::Relaxed),
    }
}

/// The census rows, `case -> the scratch names declined there`, for the record
/// and for a fail-on-stale assertion's message.
pub fn scratch_decline_table() -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (case, name) in lock(&SCRATCH_DECLINES).iter() {
        out.entry(case.clone()).or_default().push(name.clone());
    }
    out
}

// ---------------------------------------------------------------------------
// GOLDEN_REBASE G1.10b — the CONTENTS of the selected run-produced files.
// ---------------------------------------------------------------------------

/// One selected report as all three producers wrote it, ready for the
/// per-report cell comparison.
///
/// The texts are already decoded and newline-folded by
/// [`dss_epri::guard::decode_run_file`], so the pair differs only where the
/// engines' NUMBERS differ.
pub struct MatchedRunFile {
    /// The normalized created-set member (`nev_exp_y.csv`).
    pub name: String,
    /// The gating channel's bytes, decoded.
    pub oracle: String,
    /// The port's bytes, decoded.
    pub port: String,
}

/// Files whose contents were handed to the comparison, and their total decoded
/// byte count — the non-vacuity half of this surface, exactly as
/// [`COMPARED`] is for the SET.
static CONTENTS_FILES: AtomicUsize = AtomicUsize::new(0);
static CONTENTS_BYTES: AtomicUsize = AtomicUsize::new(0);

/// `(files, decoded bytes)` matched on this surface so far, for the gate
/// epilogue's fail-on-stale census.
pub fn contents_census() -> (usize, usize) {
    (
        CONTENTS_FILES.load(Ordering::Relaxed),
        CONTENTS_BYTES.load(Ordering::Relaxed),
    )
}

/// Read one channel's sidecar directory (coordinator decision D40(6)) and
/// delete it, returning the decoded contents keyed by normalized name.
///
/// * `dir` — the gate-owned sidecar this case's transport copied into
///   (`corpus_gate::engines::run_file_contents_dir`).
/// * `reported` — the names the transport says it copied (`CaseResult::
///   run_file_contents`). `None` is "the request carried no selection" and is
///   passed straight through, so the presence rail in
///   [`compare_run_file_contents`] is the single place that decides whether a
///   missing capture fails the case.
///
/// Every disagreement between the reply and the directory is LOUD: a reported
/// name with no file, or a file the reply did not name, fails the case — so a
/// lost copy, or a file no transport of this run wrote, can never be compared as
/// this run's output.
///
/// What makes one sidecar per case safe against the OTHER channel is not that
/// assertion (both channels copy the same names, so it cannot tell them apart —
/// G1.10b audit settlement, finding AT4-8) but three facts: `run_one_case`
/// drives a `both` case's channels strictly in sequence (fetch, compare, then
/// the next channel), `dss_epri::guard::copy_selected_contents` and its Python
/// twin `remove_dir_all` the sidecar before each copy, and this function deletes
/// it as it reads.
#[track_caller]
pub fn read_sidecar(
    dir: &std::path::Path,
    reported: Option<&[String]>,
    channel: &str,
    label: &str,
) -> Option<BTreeMap<String, String>> {
    let reported = reported?;
    let ctx = format!("{label} [{channel}] run-file contents");
    let mut on_disk: BTreeSet<String> = BTreeSet::new();
    if dir.exists() {
        let rd = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("{ctx}: cannot list the sidecar {}: {e}", dir.display()));
        for entry in rd.flatten() {
            on_disk.insert(entry.file_name().to_string_lossy().into_owned());
        }
    }
    let want: BTreeSet<String> = reported.iter().cloned().collect();
    assert_eq!(
        on_disk,
        want,
        "{ctx}: the `{channel}` transport reported {} copied report(s) but its sidecar \
         {} holds {}. The bytes of this surface travel through a gate-owned directory \
         the transport wipes and refills per case; a name in one and not the other \
         means a copy was lost, or a file from another run survived — either way the \
         case fails here instead of comparing a set that is not this run's.",
        want.len(),
        dir.display(),
        on_disk.len(),
    );
    let mut out = BTreeMap::new();
    for name in reported {
        let path = dir.join(name);
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("{ctx}: cannot read {}: {e}", path.display()));
        let text =
            dss_epri::guard::decode_run_file(&bytes, name).unwrap_or_else(|e| panic!("{ctx}: {e}"));
        out.insert(name.clone(), text);
    }
    // The gate owns the directory, so it removes it in the same bracket that
    // owns the case-dir claim — a sidecar that outlived its case would be read
    // by nobody and would grow the build tree by ~19 MB per drive.
    let _ = std::fs::remove_dir_all(dir);
    Some(out)
}

/// Pair one channel's selected report contents with the port's, and hand the
/// pairs to the per-report cell comparison.
///
/// Structure before values, exactly like [`compare_run_files`]: the presence
/// rail first (a flag-gated `None` fails the case), then the ledger partition
/// applied to BOTH sides by name, then the assertion that the two sides
/// selected the SAME files. The last one cannot fail while the set comparator
/// passes — the selection is a pure function of the created set
/// ([`dss_epri::guard::selects_contents`]) — so it is a statement of that
/// invariant, and it fires if a transport ever starts filtering on its own.
///
/// * `excluded` — the same `run_files` ledger scopes the SET comparator uses: a
///   name whose PRESENCE is triaged has no contents to compare either, and the
///   call marks the scope hit, which keeps `ledger.json` fail-on-stale.
///
/// Returns the matched pairs; the per-report-kind policy comparison
/// (`GOLDEN_REBASE_PLAN.md` G1.10b micro-part F2) is applied to exactly these.
#[track_caller]
pub fn compare_run_file_contents(
    channel: &str,
    oracle: Option<&BTreeMap<String, String>>,
    port: &BTreeMap<String, String>,
    excluded: &dyn Fn(&str) -> bool,
    label: &str,
) -> Vec<MatchedRunFile> {
    let ctx = format!("{label} [{channel}] run-file contents");
    let oracle =
        capture_guard::require_capture_opt("compare_run_files (contents)", channel, oracle, &ctx);

    let mut excluded_names: BTreeSet<String> = BTreeSet::new();
    let mut keep = |m: &BTreeMap<String, String>| -> BTreeMap<String, String> {
        m.iter()
            .filter(|(n, _)| {
                if excluded(n) {
                    excluded_names.insert((*n).clone());
                    false
                } else {
                    true
                }
            })
            .map(|(n, t)| (n.clone(), t.clone()))
            .collect()
    };
    let oracle_kept = keep(oracle);
    let port_kept = keep(port);

    let oracle_names: Vec<&String> = oracle_kept.keys().collect();
    let port_names: Vec<&String> = port_kept.keys().collect();
    assert_eq!(
        oracle_names, port_names,
        "{ctx}: the two sides handed back DIFFERENT selected reports.\n  \
         oracle: {oracle_names:?}\n  port:   {port_names:?}\n  \
         ledger-excluded on this case+channel: {excluded_names:?}\n  \
         The selection is a pure function of the created-file set \
         (`dss_epri::guard::selects_contents` over \
         `dss_epri::guard::RUN_FILE_CONTENTS_PATTERNS`), which the set comparator \
         already proved equal — so this can only mean a producer filtered on its \
         own, or a copy was lost between the case directory and the sidecar."
    );

    let mut matched = Vec::with_capacity(oracle_kept.len());
    for (name, oracle_text) in oracle_kept {
        let port_text = port_kept
            .get(&name)
            .expect("the two name lists were just asserted equal");
        CONTENTS_FILES.fetch_add(1, Ordering::Relaxed);
        CONTENTS_BYTES.fetch_add(oracle_text.len(), Ordering::Relaxed);
        matched.push(MatchedRunFile {
            name,
            oracle: oracle_text,
            port: port_text.clone(),
        });
    }
    matched
}

// ---------------------------------------------------------------------------
// The comparator.
// ---------------------------------------------------------------------------

/// Normalize one reported member and refuse a non-ASCII name.
///
/// [`normalize_created_name`] is idempotent, so re-applying it to a set the
/// producer already normalized is free; doing it here is what makes the port
/// probe, the two guards and any future producer agree by construction rather
/// than by convention. The ASCII assertion is the guard rail under the fold:
/// `to_ascii_lowercase` leaves non-ASCII alone, and Python's Unicode
/// `str.lower()` would not, so a non-ASCII name must fail loudly instead of
/// being folded by one side's locale rule.
#[track_caller]
fn norm(side: &str, raw: &str, ctx: &str) -> String {
    assert!(
        raw.is_ascii(),
        "{ctx}: {side} reported the created file {raw:?}, which is not ASCII. The \
         created-file SET is ASCII-case-folded so the two oracles' spellings agree \
         (r4133 `EXP_VOLTAGES.CSV` vs capi `EXP_VOLTAGES.csv`), and a non-ASCII name \
         would fold differently in `str.to_ascii_lowercase` (Rust) and `str.lower()` \
         (Python). No corpus deck produces one; if a new one does, extend the \
         normalizer deliberately in `dss_epri::guard::normalize_created_name` and its \
         Python twin, in the same commit."
    );
    normalize_created_name(raw, raw.ends_with('/'))
}

/// Compare one channel's created-file set against the port's.
///
/// * `channel` — the gating channel's tag (`capi_v0145` / `r4133`).
/// * `oracle` — `CaseResult::run_files`; `None` fails the case through the
///   presence rail, which is the whole point of the flag-gated `Option`.
/// * `port` — [`RunFileProbe::finish_and_clean`]'s answer.
/// * `excluded` — the ledger partition: `true` for a member an applicable
///   `run_files` scope excludes on this (case, channel). Field-by-field, never a
///   whole-case skip; the call marks the scope hit, which is what keeps
///   `ledger.json` fail-on-stale.
/// * `label` — the gate's case label, used as the census key and in the panic.
#[track_caller]
pub fn compare_run_files(
    channel: &str,
    oracle: Option<&[String]>,
    port: &[String],
    excluded: &dyn Fn(&str) -> bool,
    label: &str,
) {
    let ctx = format!("{label} [{channel}] run files");
    let oracle = capture_guard::require_capture_opt("compare_run_files", channel, oracle, &ctx);

    let oracle_all: Vec<String> = oracle.iter().map(|n| norm("the oracle", n, &ctx)).collect();
    let port_all: Vec<String> = port.iter().map(|n| norm("the port", n, &ctx)).collect();

    // D25/Q2 — the engine-internal scratch split, applied SYMMETRICALLY.
    let (oracle_kept, oracle_scratch) = split_engine_scratch(&oracle_all);
    let (port_kept, port_scratch) = split_engine_scratch(&port_all);
    assert!(
        port_scratch.is_empty(),
        "{ctx}: the PORT reported the engine-internal scratch file(s) {port_scratch:?}. \
         The port keeps the fundamental solution in memory \
         (`crates/dss-core/src/solution/solution/state.rs:329-331`, \
         `solution/solution/harmonics.rs:222`) and must never write the Pascal engines' \
         harmonics disk round-trip (r4133 `Common/Utilities.pas:1512-1521`). D25/Q2 \
         splits that name off symmetrically ONLY because the port does not write it; a \
         port that starts writing it is a real change, not a normalization."
    );
    if !oracle_scratch.is_empty() {
        SCRATCH_VISITS.fetch_add(1, Ordering::Relaxed);
        let mut census = lock(&SCRATCH_DECLINES);
        for name in &oracle_scratch {
            census.insert((label.to_string(), name.clone()));
        }
    }

    // The ledger partition: a matched name leaves BOTH sides, so an excluded
    // divergence can never hide a second one on the same case.
    let mut excluded_names: BTreeSet<String> = BTreeSet::new();
    let mut keep = |set: Vec<String>| -> BTreeSet<String> {
        set.into_iter()
            .filter(|n| {
                if excluded(n) {
                    excluded_names.insert(n.clone());
                    false
                } else {
                    true
                }
            })
            .collect()
    };
    let oracle_set = keep(oracle_kept);
    let port_set = keep(port_kept);

    COMPARED.fetch_add(1, Ordering::Relaxed);
    if oracle_set == port_set {
        return;
    }
    let missing: Vec<&String> = oracle_set.difference(&port_set).collect();
    let extra: Vec<&String> = port_set.difference(&oracle_set).collect();
    panic!(
        "{ctx}: the created-file SET differs.\n  \
         missing (the oracle created it, the port did not): {missing:?}\n  \
         extra   (the port created it, the oracle did not): {extra:?}\n  \
         oracle set ({}): {oracle_set:?}\n  \
         port set   ({}): {port_set:?}\n  \
         engine-internal scratch split off (D25/Q2): oracle {oracle_scratch:?}, port []\n  \
         ledger-excluded on this case+channel: {excluded_names:?}\n  \
         Names are `/`-joined, ASCII-case-folded, a trailing `/` marking a created \
         directory. Triage per TESTING.md: a genuine product divergence gets a \
         `run_files` ledger exclusion with a `name_re` plus an expected-value pin \
         naming both sets; a missing writer is a port gap and is ported.",
        oracle_set.len(),
        port_set.len(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nothing_excluded(_: &str) -> bool {
        false
    }

    fn v(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| (*s).to_string()).collect()
    }

    /// The happy path, and the normalizer's three jobs at once: a `\` separator,
    /// a `./` prefix and an upper-case spelling all reach the same member.
    #[test]
    fn equal_sets_compare_equal_after_normalization() {
        compare_run_files(
            "capi_v0145",
            Some(&v(&["exp_y.csv", "sub/report.txt"])),
            &v(&["./EXP_Y.CSV", "sub\\Report.Txt"]),
            &nothing_excluded,
            "unit:normalization",
        );
    }

    /// The cross-oracle spelling this fold exists for: r4133 lowercases the
    /// deck-supplied stem, capi preserves it, and both must land on one member.
    #[test]
    fn the_two_oracle_spellings_fold_to_one_member() {
        for spelling in ["Auto1bus_HL_current.txt", "auto1bus_hl_current.txt"] {
            compare_run_files(
                "r4133",
                Some(&v(&[spelling])),
                &v(&["Auto1bus_HL_current.txt"]),
                &nothing_excluded,
                "unit:fold",
            );
        }
    }

    /// A created DIRECTORY is a member in its own right, and its contents are
    /// separate members — the `CloseDI` tree's shape.
    #[test]
    fn a_created_directory_and_its_contents_are_all_members() {
        let both = v(&[
            "ieee13nodecktmod/",
            "ieee13nodecktmod/di_yr_0/",
            "ieee13nodecktmod/di_yr_0/totals_1.csv",
        ]);
        compare_run_files(
            "capi_v0145",
            Some(&both),
            &both,
            &nothing_excluded,
            "unit:di-tree",
        );
    }

    #[test]
    #[should_panic(expected = "missing (the oracle created it, the port did not)")]
    fn a_file_the_port_never_wrote_fails() {
        compare_run_files(
            "capi_v0145",
            Some(&v(&["exp_y.csv", "exp_yprim.csv"])),
            &v(&["exp_y.csv"]),
            &nothing_excluded,
            "unit:missing",
        );
    }

    #[test]
    #[should_panic(expected = "extra   (the port created it, the oracle did not)")]
    fn a_file_only_the_port_wrote_fails() {
        compare_run_files(
            "r4133",
            Some(&v(&["exp_y.csv"])),
            &v(&["exp_y.csv", "__corrupt.csv"]),
            &nothing_excluded,
            "unit:extra",
        );
    }

    /// The fold must not swallow the extension: two stems that differ only there
    /// stay two members.
    #[test]
    #[should_panic(expected = "missing (the oracle created it, the port did not)")]
    fn the_fold_keeps_the_extension() {
        compare_run_files(
            "capi_v0145",
            Some(&v(&["nev_savedvoltages.txt", "nev_exp_y.csv"])),
            &v(&["nev_savedvoltages.txt"]),
            &nothing_excluded,
            "unit:extension",
        );
    }

    #[test]
    #[should_panic(expected = "compare_run_files")]
    fn an_absent_capture_fails_the_case() {
        compare_run_files(
            "r4133",
            None,
            &v(&["exp_y.csv"]),
            &nothing_excluded,
            "unit:presence",
        );
    }

    #[test]
    #[should_panic(expected = "which is not ASCII")]
    fn a_non_ascii_name_is_refused_instead_of_folded() {
        compare_run_files(
            "capi_v0145",
            Some(&v(&["exp_ÿ.csv"])),
            &v(&["exp_ÿ.csv"]),
            &nothing_excluded,
            "unit:ascii",
        );
    }

    /// The ledger partition removes the name from BOTH sides — and only that
    /// name: a second divergence on the same case still fails.
    #[test]
    fn a_ledger_scope_partitions_one_name_and_nothing_else() {
        let excluded = |n: &str| n == "testygd_transformer_tr1_pq.dsv";
        compare_run_files(
            "r4133",
            Some(&v(&["testygd_transformer_tr1_pq.dsv", "exp_y.csv"])),
            &v(&["exp_y.csv"]),
            &excluded,
            "unit:ledger",
        );
    }

    #[test]
    #[should_panic(expected = "missing (the oracle created it, the port did not)")]
    fn a_ledger_scope_does_not_widen_to_a_second_name() {
        let excluded = |n: &str| n == "testygd_transformer_tr1_pq.dsv";
        compare_run_files(
            "r4133",
            Some(&v(&[
                "testygd_transformer_tr1_pq.dsv",
                "testygd_transformer_tr1_pq.dbl",
            ])),
            &v(&[]),
            &excluded,
            "unit:ledger-narrow",
        );
    }

    /// D25/Q2: the oracle's harmonics scratch file is split off symmetrically
    /// (so the sets match) and COUNTED, not silently dropped.
    #[test]
    fn the_engine_scratch_file_is_split_off_and_counted() {
        let before = scratch_census();
        compare_run_files(
            "capi_v0145",
            Some(&v(&["nev_exp_y.csv", "nev_savedvoltages.dbl"])),
            &v(&["nev_exp_y.csv"]),
            &nothing_excluded,
            "unit:scratch",
        );
        let after = scratch_census();
        // `>=`, not `== +1`: the census is a process-wide static that the live
        // gate writes from other threads once the manifest flag is on.
        assert!(after.names > before.names);
        assert!(after.compared > before.compared);
        assert!(
            scratch_decline_table()
                .get("unit:scratch")
                .is_some_and(|names| names == &["nev_savedvoltages.dbl".to_string()]),
            "the decline table must name the case and the scratch file it declined"
        );
    }

    /// The `.dbl` scratch name is split off; the user-visible `.Txt` twin that
    /// all three engines write is NOT (it stays a compared member).
    #[test]
    #[should_panic(expected = "missing (the oracle created it, the port did not)")]
    fn the_saved_voltages_text_twin_is_not_scratch() {
        compare_run_files(
            "capi_v0145",
            Some(&v(&["nev_savedvoltages.txt"])),
            &v(&[]),
            &nothing_excluded,
            "unit:scratch-txt",
        );
    }

    /// The port writing a scratch file is a real change, never a normalization.
    #[test]
    #[should_panic(expected = "the PORT reported the engine-internal scratch file")]
    fn the_port_may_never_report_a_scratch_file() {
        compare_run_files(
            "r4133",
            Some(&v(&["nev_savedvoltages.dbl"])),
            &v(&["nev_savedvoltages.dbl"]),
            &nothing_excluded,
            "unit:scratch-port",
        );
    }

    /// The probe reports what a run created and leaves the directory as it found
    /// it — the property that makes channel 2 of a `both` case symmetric with
    /// channel 1.
    #[test]
    fn the_probe_reports_then_restores() {
        let root = std::env::temp_dir().join(format!(
            "dss_run_files_probe_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(root.join("pre")).unwrap();
        let case = root.join("case.dss");
        std::fs::write(&case, b"! deck\n").unwrap();
        std::fs::write(root.join("pre/keep.txt"), b"keep\n").unwrap();

        let before: BTreeSet<String> = walk(&root);
        let probe = RunFileProbe::start(case.to_str().unwrap());
        std::fs::write(root.join("EXP_Y.CSV"), b"y\n").unwrap();
        // A real `Export Voltages`: the executive prefixes the circuit name
        // (r4133 `Executive/ExportOptions.pas:401`), which is what the G1.10b
        // selection matches — the un-prefixed `EXP_Y.CSV` above therefore stays
        // a SET member whose CONTENTS nobody asked for.
        std::fs::write(root.join("Fbs_EXP_VOLTAGES.CSV"), b"Bus,V\r\n a,1\r\n").unwrap();
        std::fs::create_dir_all(root.join("DI_yr_0/Sub")).unwrap();
        std::fs::write(root.join("DI_yr_0/Sub/deep.DBL"), b"d\n").unwrap();
        // Stands for a concurrently running sibling case writing into its own
        // (here pre-existing) directory: neither reported nor swept — D30(2).
        std::fs::write(root.join("pre/New_Report.Txt"), b"r\n").unwrap();
        let report = probe.finish_and_clean("unit:probe");

        assert_eq!(
            report.contents.keys().collect::<Vec<_>>(),
            vec!["fbs_exp_voltages.csv"],
            "the probe reads the CONTENTS of the SELECTED members only — the \
             created `di_yr_0/` tree, the sibling case's file and the un-prefixed \
             `exp_y.csv` are not reports whose kind their own name identifies"
        );
        assert_eq!(
            report.contents["fbs_exp_voltages.csv"], "Bus,V\n a,1\n",
            "and it decodes them with the newline fold both transports apply"
        );
        assert_eq!(
            report.created,
            vec![
                "di_yr_0/".to_string(),
                "di_yr_0/sub/".to_string(),
                "di_yr_0/sub/deep.dbl".to_string(),
                "exp_y.csv".to_string(),
                "fbs_exp_voltages.csv".to_string(),
            ],
            "the probe reports the case dir's own entries and the created \
             directory's whole tree, and nothing under a PRE-EXISTING \
             subdirectory"
        );
        let mut want = before.clone();
        want.insert("pre/new_report.txt".to_string());
        assert_eq!(
            walk(&root),
            want,
            "the probe restores the case dir and leaves the sibling case's file \
             alone"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    /// Coordinator decision **D33(3)**: the port is a producer exactly like the
    /// two oracle transports, so a dropping IT cannot remove fails the case
    /// loudly. Before this the probe dropped its guard, whose `Drop` prints the
    /// leak to stderr and returns — and a survivor is precisely what makes the
    /// NEXT producer of the case snapshot it as pre-existing and silently drop
    /// that name from its created set (the order-coupling D32(2) closed on the
    /// oracle side, measured there as dss_capi's never-closed Storage trace
    /// stream).
    ///
    /// The fixture holds the created file open with `FILE_SHARE_READ` only —
    /// what the FPC/Delphi file APIs do — so `remove_file` fails with a sharing
    /// violation, the same way `os.remove` did on the real leak
    /// (`dss_epri::guard::tests::a_created_file_the_sweep_cannot_remove_is_reported_as_sweep_failed`).
    #[test]
    #[should_panic(expected = "leaked dropping")]
    fn a_port_dropping_the_probe_cannot_remove_fails_the_case() {
        use std::os::windows::fs::OpenOptionsExt;
        /// `FILE_SHARE_READ` — deletion of the open file is NOT shared, so
        /// `DeleteFile` fails with a sharing violation (winnt.h).
        const FILE_SHARE_READ: u32 = 0x0000_0001;

        let root = std::env::temp_dir().join(format!(
            "dss_run_files_leak_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        // Cleaned on ENTRY: the panic below is the point of the test, so this
        // run cannot clean up after itself.
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).unwrap();
        let case = root.join("case.dss");
        std::fs::write(&case, b"! deck\n").unwrap();

        let probe = RunFileProbe::start(case.to_str().unwrap());
        std::fs::write(root.join("STOR_storage1.CSV"), b"hour, t, ...\n").unwrap();
        let held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(root.join("STOR_storage1.CSV"))
            .expect("open the created file the way an engine holds its trace");
        let report = probe.finish_and_clean("unit:port-leak");
        drop(held);
        unreachable!(
            "the probe must fail the case, not report {:?}",
            report.created
        );
    }

    // -----------------------------------------------------------------------
    // G1.10b — the CONTENTS transport: the sidecar read and the pairing rails.
    // -----------------------------------------------------------------------

    fn m(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(n, t)| ((*n).to_string(), (*t).to_string()))
            .collect()
    }

    fn sidecar(tag: &str, files: &[(&str, &[u8])]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "dss_run_file_contents_{tag}_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        for (name, bytes) in files {
            std::fs::write(dir.join(name), bytes).unwrap();
        }
        dir
    }

    /// "The request carried no selection" travels as `None` all the way to the
    /// presence rail — [`read_sidecar`] never invents an empty map for it.
    #[test]
    fn an_unrequested_sidecar_stays_none() {
        let dir = std::env::temp_dir().join("dss_run_file_contents_never_created");
        assert!(read_sidecar(&dir, None, "r4133", "unit:contents-off").is_none());
    }

    /// The happy path: the bytes decode with folded newlines, and the gate
    /// DELETES the directory it just read (it owns the scratch).
    #[test]
    fn the_sidecar_is_decoded_and_then_removed() {
        let dir = sidecar("read", &[("nev_exp_y.csv", b"Row,Col\r\n1,2\r\n")]);
        let got = read_sidecar(
            &dir,
            Some(&v(&["nev_exp_y.csv"])),
            "capi_v0145",
            "unit:contents-read",
        )
        .expect("requested");
        assert_eq!(got["nev_exp_y.csv"], "Row,Col\n1,2\n");
        assert!(!dir.exists(), "the gate owns the sidecar and removes it");
    }

    /// A name the reply promised but the sidecar does not hold fails the case —
    /// a lost copy can never shrink the compared set in silence.
    #[test]
    #[should_panic(expected = "copied report(s) but its sidecar")]
    fn a_report_missing_from_the_sidecar_fails_the_case() {
        let dir = sidecar("lost", &[("nev_exp_y.csv", b"y\n")]);
        let _ = read_sidecar(
            &dir,
            Some(&v(&["nev_exp_y.csv", "nev_exp_yprim.csv"])),
            "r4133",
            "unit:contents-lost",
        );
    }

    /// ...and the other direction: a file the reply did NOT name (the other
    /// channel's leftover, or an earlier case's) fails too, which is what makes
    /// one sidecar per case safe.
    #[test]
    #[should_panic(expected = "copied report(s) but its sidecar")]
    fn a_stale_file_in_the_sidecar_fails_the_case() {
        let dir = sidecar(
            "stale",
            &[("nev_exp_y.csv", b"y\n"), ("other_exp_y.csv", b"old\n")],
        );
        let _ = read_sidecar(
            &dir,
            Some(&v(&["nev_exp_y.csv"])),
            "capi_v0145",
            "unit:contents-stale",
        );
    }

    /// Invalid UTF-8 is REFUSED, never lossily decoded into `U+FFFD`s that
    /// would compare equal to each other.
    #[test]
    #[should_panic(expected = "is not valid UTF-8")]
    fn a_non_utf8_report_fails_loudly() {
        let dir = sidecar("utf8", &[("nev_exp_y.csv", &[b'a', 0xFF, b'\n'])]);
        let _ = read_sidecar(
            &dir,
            Some(&v(&["nev_exp_y.csv"])),
            "r4133",
            "unit:contents-utf8",
        );
    }

    /// The presence rail: the manifest flag is on, so a channel that reported
    /// no contents at all fails the case instead of comparing nothing.
    #[test]
    #[should_panic(expected = "compare_run_files (contents)")]
    fn an_absent_contents_capture_fails_the_case() {
        compare_run_file_contents(
            "r4133",
            None,
            &m(&[("nev_exp_y.csv", "y\n")]),
            &nothing_excluded,
            "unit:contents-presence",
        );
    }

    /// The pairs come back keyed by name, and the census moves with them — a
    /// contents surface that matched nothing would otherwise pass vacuously.
    #[test]
    fn the_matched_pairs_carry_both_sides_and_move_the_census() {
        let before = contents_census();
        let matched = compare_run_file_contents(
            "capi_v0145",
            Some(&m(&[("nev_exp_y.csv", "a\n"), ("stor_s1.csv", "b\n")])),
            &m(&[("nev_exp_y.csv", "a\n"), ("stor_s1.csv", "c\n")]),
            &nothing_excluded,
            "unit:contents-pairs",
        );
        assert_eq!(
            matched.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
            vec!["nev_exp_y.csv", "stor_s1.csv"]
        );
        // F1 pairs; it does not compare cells — the differing `stor_s1.csv`
        // travels to the caller intact (micro-part F2 is what reds on it).
        assert_eq!(matched[1].oracle, "b\n");
        assert_eq!(matched[1].port, "c\n");
        let after = contents_census();
        assert!(after.0 >= before.0 + 2 && after.1 > before.1);
    }

    /// The ledger partition drops a triaged NAME from both sides — the same
    /// `run_files` scopes the set comparator honours, so a name whose presence
    /// is triaged has no contents to compare either.
    #[test]
    fn a_ledger_scope_drops_the_contents_of_that_name_on_both_sides() {
        let excluded = |n: &str| n == "stor_s1.csv";
        let matched = compare_run_file_contents(
            "capi_v0145",
            Some(&m(&[("nev_exp_y.csv", "a\n"), ("stor_s1.csv", "b\n")])),
            &m(&[("nev_exp_y.csv", "a\n"), ("stor_s1.csv", "zzz\n")]),
            &excluded,
            "unit:contents-ledger",
        );
        assert_eq!(
            matched.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
            vec!["nev_exp_y.csv"]
        );
    }

    /// The two sides must have SELECTED the same files: the selection is a pure
    /// function of the created set, so an asymmetry means a producer filtered on
    /// its own or a copy was lost.
    #[test]
    #[should_panic(expected = "handed back DIFFERENT selected reports")]
    fn an_asymmetric_selection_fails_the_case() {
        compare_run_file_contents(
            "r4133",
            Some(&m(&[
                ("nev_exp_y.csv", "a\n"),
                ("nev_exp_yprim.csv", "b\n"),
            ])),
            &m(&[("nev_exp_y.csv", "a\n")]),
            &nothing_excluded,
            "unit:contents-asym",
        );
    }

    fn walk(dir: &std::path::Path) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let mut stack = vec![(dir.to_path_buf(), String::new())];
        while let Some((d, prefix)) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                let rel = if prefix.is_empty() {
                    name
                } else {
                    format!("{prefix}/{name}")
                };
                let is_dir = e.file_type().unwrap().is_dir();
                out.insert(normalize_created_name(&rel, is_dir));
                if is_dir {
                    stack.push((e.path(), rel));
                }
            }
        }
        out
    }
}

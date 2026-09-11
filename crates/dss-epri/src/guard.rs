//! Corpus-hygiene guard — a Rust port of `tools/oracle/corpus_guard.py` (and the
//! identical `CorpusGuard` in `crates/dss-core/tests/corpus_live.rs`). A deck's
//! `Compile` sets `OutputDirectory := <case dir>`, so a migrated deck's
//! `Export`/`Show`/`Save` (and any `<CircuitName>/DI_yr_*` demand-interval tree)
//! writes report files next to the vendored deck. This snapshots the case dir
//! before the run and restores it on drop: delete anything the run created,
//! rewrite any small pre-existing file it overwrote.
//!
//! The snapshot is RECURSIVE, and — critically — an *incomplete* snapshot
//! (any `read_dir` failure) disables deletion entirely, so a transient lock can
//! never make the guard classify pre-existing corpus files as run-created and
//! delete them (the war story documented in the Python original).
//!
//! GOLDEN_REBASE G1.10a added the *reporting* half: the classification that
//! decides what to delete is also the gate's `compare_run_files` surface — the
//! set of filesystem entries a run created under the case dir. One
//! classification, two consumers ([`CorpusGuard::created`] and
//! [`CorpusGuard::sweep_created`]), so the reported set can never disagree with
//! the swept set. The surface — and therefore the sweep — is **the case dir's
//! own entries, plus everything under a directory the run created**: a
//! PRE-EXISTING subdirectory is never descended into, because the gate schedules
//! by case *dir* and a report file appearing under one belongs to a
//! concurrently running sibling case, not to this run (D30(2); the citations and
//! the measurement are on [`CorpusGuard::classify`]). The classification itself
//! is the free [`classify_created`], so the gate's outer guard
//! (`crates/dss-core/tests/corpus_gate/runner.rs`), whose snapshot is shared per
//! directory, runs the identical rule — one classification, three consumers
//! (D32(3)).
//!
//! And the sweep is no longer allowed to fail quietly (D32(2)): whatever it
//! could not remove comes back from [`CorpusGuard::finish`] as `sweep_failed`
//! and travels in the transport's reply, so a producer that leaks an open file
//! handle can never again silence a LATER producer's created set by leaving the
//! file behind as "pre-existing".

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Buffer files up to this size for overwrite-restore (OpenDSS writes small text
/// reports, never the multi-MiB input data files). Mirrors the Python
/// `_RESTORE_MAX` and the Rust gate's `RESTORE_MAX`.
const RESTORE_MAX: u64 = 2 * 1024 * 1024;

/// Engine-internal scratch files (GOLDEN_REBASE G1.10a, coordinator decision
/// D25/Q2). The Pascal engines round-trip the fundamental solution through DISK
/// when harmonics is initialized: `SavePresentVoltages` writes
/// `<CircuitName_>SavedVoltages.dbl` (r4133
/// `Version8/Source/Common/Utilities.pas:1512-1521`, reached only from
/// `InitializeForHarmonics` `:1599-1608`; read back by `RetrieveSavedVoltages`
/// `:1554-1564`, consumed at `Common/SolutionAlgs.pas:1056,1131`; dss_capi
/// 0.14.5 twin `src/Common/Utilities.pas:883` / `:914-923`, consumed at
/// `src/Common/SolutionAlgs.pas:1030,1110`). The port keeps that vector in
/// memory (`crates/dss-core/src/solution/solution/state.rs:329-331`,
/// `solution/solution/harmonics.rs:222`) and writes no such file, so the name is
/// split off SYMMETRICALLY on every channel instead of becoming one ledger row
/// per harmonics deck.
///
/// NOT scratch: `<CircuitName_>SavedVoltages.Txt`, the user-visible
/// `Save Voltages` output `VDIFF` reads back (r4133 `Common/Solution.pas:3973` +
/// `Executive/ExecHelper.pas:3321`, capi `src/Common/Solution.pas:2288` +
/// `src/Executive/ExecHelper.pas:3387`, port
/// `crates/dss-core/src/exec/report.rs:2490-2522`) — all three engines write it,
/// so it stays a compared member of the set.
/// The match is a SUFFIX, not the full `<CircuitName_>SavedVoltages.dbl` shape
/// (G1.10a audit settlement, finding AC-8: a conscious choice, not an
/// oversight). It cannot hide anything: the split is SYMMETRIC — a name that
/// matches leaves the compared set on the oracle side and on the port side
/// alike, the port is asserted to produce none
/// (`harness::run_files::compare_run_files`), and every declined name is counted
/// into the fail-on-stale `SCRATCH_FILE_DECLINES` population
/// (`corpus_gate/scheduler.rs`), which reds in BOTH directions. So a future file
/// that happens to end in the suffix moves the population and fails the gate as
/// a population move, instead of vanishing from the surface.
pub const ENGINE_SCRATCH_SUFFIXES: [&str; 1] = ["savedvoltages.dbl"];

/// Canonical member of the created-file set: `/`-joined, `./`-stripped,
/// ASCII-case-folded, with a trailing `/` marking a run-created DIRECTORY (its
/// contents are separate members). Idempotent, so a consumer may re-apply it.
///
/// The case fold is a CROSS-ORACLE NORMALIZATION, not a tolerance: r4133 writes
/// `EXP_VOLTAGES.CSV` (`Version8/Source/Executive/ExportOptions.pas:333-356`)
/// while dss_capi 0.14.5 writes `EXP_VOLTAGES.csv`
/// (`src/Executive/ExportOptions.pas:314,343,345,381,437`), and r4133
/// additionally lowercases deck-supplied stems (`Auto1bus_HL_current.txt` ->
/// `auto1bus_hl_current.txt`), so no single spelling can satisfy both gating
/// channels; NTFS is case-insensitive, so nothing observable depends on it (the
/// R-18 decision, recorded in `DIVERGENCES.md`). The fold is ASCII-only — the
/// exact twin of the Python `ascii_lower` — so a non-ASCII name survives
/// unfolded and the comparator can refuse it loudly.
///
/// Twin: `tools/oracle/corpus_guard.py::normalize_created_name`.
pub fn normalize_created_name(rel: &str, is_dir: bool) -> String {
    let mut s = rel.replace('\\', "/");
    while let Some(rest) = s.strip_prefix("./") {
        s = rest.to_string();
    }
    while s.contains("//") {
        s = s.replace("//", "/");
    }
    let mut s = s.trim_end_matches('/').to_ascii_lowercase();
    if is_dir {
        s.push('/');
    }
    s
}

/// True for an engine-internal scratch file (see [`ENGINE_SCRATCH_SUFFIXES`]):
/// one the Pascal engines write only to read back within the same run.
///
/// Twin: `tools/oracle/corpus_guard.py::is_engine_scratch_file`.
pub fn is_engine_scratch_file(name: &str) -> bool {
    let folded = name.to_ascii_lowercase();
    ENGINE_SCRATCH_SUFFIXES
        .iter()
        .any(|sfx| folded.ends_with(sfx))
}

/// Partition a created-file set into `(compared, engine-internal scratch)`.
///
/// The structural normalization of decision D25/Q2, applied SYMMETRICALLY to
/// every channel: the caller compares the first list and counts the second (the
/// gate's fail-on-stale `SCRATCH_FILE_DECLINES` population constant), so the
/// decline stays visible instead of being silently masked.
///
/// Twin: `tools/oracle/corpus_guard.py::split_engine_scratch`.
pub fn split_engine_scratch(names: &[String]) -> (Vec<String>, Vec<String>) {
    let mut kept = Vec::new();
    let mut scratch = Vec::new();
    for n in names {
        if is_engine_scratch_file(n) {
            scratch.push(n.clone());
        } else {
            kept.push(n.clone());
        }
    }
    (kept, scratch)
}

// ---------------------------------------------------------------------------
// GOLDEN_REBASE G1.10b — which created files carry their CONTENTS to the gate.
// ---------------------------------------------------------------------------

/// The G1.10b CONTENTS selection: the created-file names whose BYTES the gate
/// compares (G1.10a compares the set; this is its second half).
///
/// Declared ONCE, here, and shipped to both oracle transports in the run request
/// (`run_file_contents`) so that no transport ever re-derives which files it
/// must hand back — the same "one classification, three producers" property
/// G1.10a established for the set itself
/// (`crates/dss-core/tests/corpus_gate/engines.rs::build_run_request`).
///
/// Each entry is a pattern over a [`normalize_created_name`] member (already
/// `/`-joined and ASCII-case-folded) carrying exactly one `*`, which stands for
/// any run of characters other than `/` — see [`selects_contents`].
///
/// Where the names come from, in r4133:
/// * `<CircuitName_>EXP_{VOLTAGES,CURRENTS,POWERS,Y,YPRIM,Profile}.CSV` — the
///   default-name table of `Version8/Source/Executive/ExportOptions.pas:331-397`,
///   prefixed `GetOutputDirectory + CircuitName_[ActiveActor]` at `:401`
///   (dss_capi 0.14.5 twin `src/Executive/ExportOptions.pas:314-437`).
/// * `EXP_PV_<NAME>.CSV` — the `/m` ("multiple") arm of `Export PVSystem`
///   writes one file per PVSystem and does NOT prefix the circuit name
///   (`Version8/Source/Common/ExportResults.pas:2010`).
/// * `STOR_<NAME>.CSV` — the Storage `DebugTrace` file
///   (`Version8/Source/PCElements/Storage.pas:1073-1085`, ported by G1.10a
///   micro-part F4a into `crates/dss-core/src/elements/pc/storage/trace.rs`).
///
/// Deliberately NOT selected, each recorded by the gate's census with its reason
/// instead of compared ad hoc (coordinator decision D40(3)/D40(5)): the monitor
/// CSVs (A6 — f32 data, and unsampled-monitor comparison is a known oracle
/// artifact, `TESTING.md`), the `Show`/`Dump` text reports, the demand-interval
/// tree (G1.10c owns it), the export kinds with no golden `ExportPolicy`
/// (`Jacobian`, `deltaF`, `deltaZ`, `cim100`), and a deck-named
/// `export ... file=<name>`, whose file name does not identify the report kind.
///
/// Provenance: new coverage, not catch-up — the upstream harness's CSV compare
/// is non-gating (`DSS-Python origin/fastdss tests/compare_outputs.py:517-527`
/// prints `COMPARE CSV ERROR` with its `raise` commented out at `:527`, and
/// `:416-421` skips a file missing on one side).
pub const RUN_FILE_CONTENTS_PATTERNS: [&str; 8] = [
    "*_exp_currents.csv",
    "*_exp_powers.csv",
    "*_exp_profile.csv",
    "*_exp_voltages.csv",
    "*_exp_y.csv",
    "*_exp_yprim.csv",
    "exp_pv_*.csv",
    "stor_*.csv",
];

/// Refuse a selection pattern that is not exactly one `*` between two literal
/// ASCII parts. The shape is pinned rather than grown into a glob dialect
/// because the pattern list crosses a process boundary into a Python twin
/// (`tools/oracle/corpus_guard.py::selects_contents`): two glob engines that
/// disagreed on one deck would silently compare different files on the two
/// gating channels.
///
/// Twin: `tools/oracle/corpus_guard.py::check_contents_pattern`.
pub fn check_contents_pattern(pattern: &str) -> Result<(), String> {
    if pattern.matches('*').count() != 1 {
        return Err(format!(
            "run-file contents pattern {pattern:?} must carry exactly one `*` (it \
             stands for one run of characters other than `/`); the shape is fixed \
             because `tools/oracle/corpus_guard.py` matches the same patterns"
        ));
    }
    if !pattern.is_ascii() || pattern != pattern.to_ascii_lowercase() {
        return Err(format!(
            "run-file contents pattern {pattern:?} must be lower-case ASCII: it is \
             matched against a `normalize_created_name` member, which is already \
             ASCII-case-folded"
        ));
    }
    Ok(())
}

/// True when `pattern` — one `*` standing for any run of characters other than
/// `/` — matches the normalized created-set member `name`.
///
/// Twin: `tools/oracle/corpus_guard.py::contents_pattern_matches`.
fn contents_pattern_matches(pattern: &str, name: &str) -> bool {
    let Some((head, tail)) = pattern.split_once('*') else {
        return false;
    };
    name.len() >= head.len() + tail.len()
        && name.starts_with(head)
        && name.ends_with(tail)
        && !name[head.len()..name.len() - tail.len()].contains('/')
}

/// True when the gate asked for this member's contents (see
/// [`RUN_FILE_CONTENTS_PATTERNS`]). A created DIRECTORY (trailing `/`) never
/// matches: only file bytes travel.
///
/// Twin: `tools/oracle/corpus_guard.py::selects_contents`.
pub fn selects_contents<S: AsRef<str>>(patterns: &[S], name: &str) -> bool {
    !name.ends_with('/')
        && patterns
            .iter()
            .any(|p| contents_pattern_matches(p.as_ref(), name))
}

/// Resolve the selected members of `created` to the real paths they were written
/// under, listing `case_dir` ONCE.
///
/// Not a second classification: `created` is the one the caller already made
/// ([`CorpusGuard::created`]), and this only maps each selected member back to
/// the spelling its producer used on disk — r4133 writes `EXP_VOLTAGES.CSV`
/// where dss_capi writes `EXP_VOLTAGES.csv` ([`normalize_created_name`]'s
/// cross-oracle fold), so a normalized name is not a path.
///
/// Every failure is loud: a selected member that is not a direct child of the
/// case directory, or that the directory no longer lists as a file, is an error
/// rather than a quietly missing file — the whole point of the surface is that
/// every file the run wrote reaches the gate.
fn resolve_selected<S: AsRef<str>>(
    case_dir: &Path,
    created: &[String],
    patterns: &[S],
) -> Result<Vec<(String, PathBuf)>, String> {
    for p in patterns {
        check_contents_pattern(p.as_ref())?;
    }
    let wanted: Vec<&String> = created
        .iter()
        .filter(|n| selects_contents(patterns, n))
        .collect();
    if wanted.is_empty() {
        return Ok(Vec::new());
    }
    let rd = std::fs::read_dir(case_dir).map_err(|e| {
        format!(
            "run-file contents: cannot list the case directory {}: {e}",
            case_dir.display()
        )
    })?;
    let mut on_disk: BTreeMap<String, PathBuf> = BTreeMap::new();
    for entry in rd.flatten() {
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(true) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        on_disk.insert(normalize_created_name(&name, false), entry.path());
    }
    let mut out = Vec::with_capacity(wanted.len());
    for name in wanted {
        if name.contains('/') {
            return Err(format!(
                "run-file contents: the selected member {name:?} lives under a \
                 run-created subdirectory. Every selected report is written to \
                 `GetOutputDirectory + CircuitName_ + FileName` (r4133 \
                 `Version8/Source/Executive/ExportOptions.pas:401`), i.e. into the case \
                 directory itself; a nested one means the selection patterns \
                 (`RUN_FILE_CONTENTS_PATTERNS`) now reach a tree they were not written \
                 for — widen them deliberately, never by accident."
            ));
        }
        let path = on_disk.get(name.as_str()).ok_or_else(|| {
            format!(
                "run-file contents: {name:?} is in this run's created-file set but the \
                 case directory {} no longer lists it as a file. A selected report must \
                 still be on disk when its contents are taken — the read is the last \
                 thing the run does, inside the guard scope, before the sweep.",
                case_dir.display()
            )
        })?;
        out.push((name.clone(), path.clone()));
    }
    Ok(out)
}

/// Decode one run-produced report file: UTF-8 with a LOUD refusal (never a lossy
/// decode), and `\r\n` / `\r` folded to `\n`.
///
/// The fold is [`crate::capture`]'s `read_autoadd_log` contract carried over:
/// `oracle_server.py` reads its side in Python text mode (universal newlines),
/// so a comparison that did not fold would turn a transport detail into a
/// per-line divergence. The refusal is the other half: the Pascal writers emit
/// ASCII through `Format`/`Writeln`, so a byte sequence that is not UTF-8 means a
/// truncated or mis-transferred file, and a lossy decode would compare a
/// `U+FFFD` against a real character and call it a match.
pub fn decode_run_file(bytes: &[u8], what: &str) -> Result<String, String> {
    let text = std::str::from_utf8(bytes).map_err(|e| {
        format!(
            "run-file contents: {what} is not valid UTF-8 (first bad byte at offset {}). \
             The report writers emit ASCII text, so this is a truncated or \
             mis-transferred file. Refused instead of lossily decoded — a `U+FFFD` \
             compared against a real character is a silent pass.",
            e.valid_up_to()
        )
    })?;
    Ok(text.replace("\r\n", "\n").replace('\r', "\n"))
}

/// Copy the selected members' bytes into the gate-owned sidecar directory and
/// return the names copied, sorted — an oracle TRANSPORT's half of the G1.10b
/// contents surface (coordinator decision D40(6)).
///
/// Bytes travel on disk rather than inline in the line-JSON reply because the
/// selection reaches ~19 MB per channel per full drive (`NEV_EXP_Y.csv` alone is
/// 4.6 MB), which would cross the worker pipes twice on every `both` case.
///
/// The sidecar is WIPED and recreated here, so a file left by an earlier case or
/// by the other channel can never be read as this run's output; the gate then
/// asserts the directory holds exactly the returned names and deletes it
/// (`crates/dss-core/tests/harness/run_files.rs::read_sidecar`).
///
/// `patterns` empty ⇒ the gate did not ask: no directory is touched and `None`
/// comes back, which keeps an off-flag reply identical to a pre-G1.10b one and
/// lets the gate's presence rail tell "not requested" from "requested, and this
/// deck wrote none of the selected reports".
///
/// Twin: `tools/oracle/corpus_guard.py::copy_selected_contents`.
pub fn copy_selected_contents<S: AsRef<str>>(
    case_dir: &Path,
    created: Option<&[String]>,
    patterns: &[S],
    sidecar: Option<&str>,
) -> Result<Option<Vec<String>>, String> {
    if patterns.is_empty() {
        return Ok(None);
    }
    let sidecar = sidecar.ok_or_else(|| {
        "run-file contents: the request carries selection patterns but no \
         `run_file_contents_dir`. The bytes travel through a gate-owned sidecar \
         directory; without one the transport would have to inline megabytes into the \
         reply, which coordinator decision D40(6) rules out."
            .to_string()
    })?;
    let created = created.ok_or_else(|| {
        "run-file contents: the request asks for file contents but this run's \
         created-file set could not be classified (incomplete pre-run snapshot). The \
         contents are a subset of that set, so they cannot be reported either."
            .to_string()
    })?;
    let selected = resolve_selected(case_dir, created, patterns)?;
    let dir = Path::new(sidecar);
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("run-file contents: cannot create the sidecar {sidecar}: {e}"))?;
    let mut names = Vec::with_capacity(selected.len());
    for (name, path) in selected {
        std::fs::copy(&path, dir.join(&name)).map_err(|e| {
            format!(
                "run-file contents: cannot copy {} into the sidecar {sidecar}: {e}",
                path.display()
            )
        })?;
        names.push(name);
    }
    names.sort();
    Ok(Some(names))
}

/// Read the selected members' contents straight out of the case directory — the
/// PORT's half of the same surface, where no sidecar is needed because the gate
/// and the producer are one process
/// (`crates/dss-core/tests/harness/run_files.rs::RunFileProbe::finish_and_clean`).
///
/// Same selection and the same decode ([`decode_run_file`]) as the two
/// transports, so a difference between the sides can only be the files' contents.
pub fn read_selected_contents<S: AsRef<str>>(
    case_dir: &Path,
    created: &[String],
    patterns: &[S],
) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    for (name, path) in resolve_selected(case_dir, created, patterns)? {
        let bytes = std::fs::read(&path)
            .map_err(|e| format!("run-file contents: cannot read {}: {e}", path.display()))?;
        let text = decode_run_file(&bytes, &name)?;
        out.insert(name, text);
    }
    Ok(out)
}

/// One classification of a case directory into "created by this run" and "was
/// already there" — the rule, its citations and its measurement are on
/// [`CorpusGuard::classify`], which is the entry point for a guard that owns its
/// own snapshot.
///
/// Free-standing (coordinator decision D32(3)) so the gate's OUTER guard
/// `crates/dss-core/tests/corpus_gate/runner.rs::CorpusGuard` — which holds a
/// *refcounted per-directory* snapshot shared by every guard active on that
/// directory, not a per-guard one — sweeps with exactly the code the two oracle
/// transports and the port probe report from. One classification, three
/// consumers: without it the outer guard kept descending into pre-existing
/// subdirectories and could delete a concurrently running sibling case's live
/// output.
pub struct Created {
    /// Removal roots: the `(path, is_dir)` of every created entry of the case
    /// directory ITSELF. A run-created directory is ONE root, removed as a whole
    /// tree — so the roots are always direct children of the case dir.
    pub roots: Vec<(PathBuf, bool)>,
    /// The normalized name of every created entry, a created directory's whole
    /// contents included ([`normalize_created_name`]).
    pub names: BTreeSet<String>,
    /// False when any directory listing failed. An incomplete classification may
    /// still be SWEPT (the sweep removes only what it did classify, never more)
    /// but must never be REPORTED as the created set.
    pub complete: bool,
}

/// Classify `dir` against the pre-run snapshot `pre_existing` (`/`-joined
/// relative paths, files AND directories). See [`CorpusGuard::classify`] for the
/// rule, the r4133/capi citations and the F2b measurement behind it.
pub fn classify_created(dir: &Path, pre_existing: &BTreeSet<String>) -> Created {
    let mut c = Created {
        roots: Vec::new(),
        names: BTreeSet::new(),
        complete: true,
    };
    c.complete = classify_into(dir, "", pre_existing, &mut c.roots, &mut c.names);
    c
}

fn classify_into(
    dir: &Path,
    prefix: &str,
    pre_existing: &BTreeSet<String>,
    roots: &mut Vec<(PathBuf, bool)>,
    out: &mut BTreeSet<String>,
) -> bool {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return false;
    };
    let mut ok = true;
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if pre_existing.contains(&rel) {
            // Pre-existing. A pre-existing DIRECTORY is deliberately not
            // descended into: nothing this run writes can land there, and what
            // does land there belongs to a concurrently running sibling case
            // (see the rule on `CorpusGuard::classify`).
            continue;
        }
        out.insert(normalize_created_name(&rel, is_dir));
        roots.push((entry.path(), is_dir));
        if is_dir {
            // Run-created directory (the DI `<CircuitName>/` tree): every entry
            // below it is run-created too, and the whole tree is one removal
            // root.
            ok &= collect_tree(&entry.path(), &rel, out);
        }
    }
    ok
}

/// List a run-created directory: every entry under it is run-created.
///
/// Twin: `tools/oracle/corpus_guard.py::CorpusGuard._collect_tree`.
fn collect_tree(dir: &Path, prefix: &str, out: &mut BTreeSet<String>) -> bool {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return false;
    };
    let mut ok = true;
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = format!("{prefix}/{name}");
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        out.insert(normalize_created_name(&rel, is_dir));
        if is_dir {
            ok &= collect_tree(&entry.path(), &rel, out);
        }
    }
    ok
}

pub struct CorpusGuard {
    dir: PathBuf,
    names: BTreeSet<String>,
    buf: BTreeMap<String, Vec<u8>>,
    snapshot_ok: bool,
    /// Set by [`CorpusGuard::finish`]: the sweep + restore already ran and
    /// reported, so `Drop` must not repeat them.
    finished: bool,
}

impl CorpusGuard {
    pub fn new(case_path: &str) -> CorpusGuard {
        let dir = Path::new(case_path)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let mut names = BTreeSet::new();
        let mut buf = BTreeMap::new();
        let snapshot_ok = Self::snapshot(&dir, "", &mut names, &mut buf);
        CorpusGuard {
            dir,
            names,
            buf,
            snapshot_ok,
            finished: false,
        }
    }

    /// Recursively record every entry (files AND directories) as `/`-joined
    /// relative paths, buffering small files. Returns false if any directory
    /// listing failed (caller disables deletion).
    fn snapshot(
        dir: &Path,
        prefix: &str,
        names: &mut BTreeSet<String>,
        buf: &mut BTreeMap<String, Vec<u8>>,
    ) -> bool {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return false;
        };
        let mut ok = true;
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            names.insert(rel.clone());
            if is_dir {
                ok &= Self::snapshot(&entry.path(), &rel, names, buf);
                continue;
            }
            let small = entry
                .metadata()
                .map(|m| m.is_file() && m.len() <= RESTORE_MAX)
                .unwrap_or(false);
            if small && let Ok(data) = std::fs::read(entry.path()) {
                buf.insert(rel, data);
            }
        }
        ok
    }

    /// The ONE created-entry classification (G1.10a): an entry of the case
    /// directory ITSELF is run-created iff its `/`-joined relative path is
    /// absent from the pre-run snapshot. The surface is **the case dir's own
    /// entries, plus everything under a directory the run created** — a
    /// PRE-EXISTING subdirectory is never descended into (coordinator decision
    /// D30(2), measured by G1.10a micro-part F2b).
    ///
    /// Returns [`Created`]: `roots` = the `(path, is_dir)` of every created
    /// entry — exactly what the sweep removes — and `names` = the normalized
    /// name of every created entry, recursing into a created directory
    /// (`collect_tree`) so its contents are members of the set too.
    ///
    /// Why the rule loses no member: `Compile` sets
    /// `OutputDirectory := DataDirectory := <case dir>` (r4133
    /// `Version8/Source/Common/DSSGlobals.pas:962`, capi 0.14.5 twin
    /// `src/Common/DSSGlobals.pas:563`) and every executive writer prefixes its
    /// name with it — `FileName := GetOutputDirectory + CircuitName_ + FileName`
    /// (r4133 `Version8/Source/Executive/ExportOptions.pas:401`,
    /// `Executive/ShowOptions.pas:208-211`) — so a run writes into the case dir
    /// itself or into a tree it creates, never into a subdirectory that already
    /// existed. Measured over the reachable corpus: no deck targets a
    /// subdirectory.
    ///
    /// Why it must not descend: the gate's task unit is the case *dir* group,
    /// not the dir *tree* (`crates/dss-core/tests/corpus_gate/scheduler.rs`
    /// module doc), so a case in `Test/` runs concurrently with a case in
    /// `Test/AutoTrans/`; descending would report — and SWEEP — that other
    /// case's live report files. F2b measured 9 such entries over two full
    /// 526-case drives across all three producers, every one of them
    /// `Test/AutoTrans/Auto3bus_*.txt` under the sibling case directory
    /// `Test/AutoTrans` (10 manifest cases).
    ///
    /// `complete` is false if any directory listing failed: an incomplete
    /// classification may still be SWEPT (the sweep removes only what it did
    /// classify, never more) but must never be REPORTED as the created set.
    ///
    /// Twin: `tools/oracle/corpus_guard.py::CorpusGuard._classify`. The body
    /// lives in the free [`classify_created`] so the gate's outer guard, whose
    /// snapshot is shared per directory, runs the very same rule (D32(3)).
    fn classify(&self) -> Created {
        classify_created(&self.dir, &self.names)
    }

    /// The case DIRECTORY this guard brackets — `<case_path>`'s parent, resolved
    /// exactly as [`CorpusGuard::new`] resolved it, so a consumer that has to touch
    /// the same files (G1.10b reads the selected reports' contents out of it, see
    /// [`read_selected_contents`]) can never disagree with the guard about which
    /// directory is being swept.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The run-created set, sorted and normalized — the gate's
    /// `compare_run_files` surface (GOLDEN_REBASE G1.10a).
    ///
    /// `None` means "cannot be reported honestly" (an incomplete pre-run
    /// snapshot, or a failed listing now); the gate's presence rail
    /// (`harness::capture_guard::require_capture_opt`) turns that into a failed
    /// case. An empty vector is a legitimate answer — most decks create nothing.
    ///
    /// Read it as the LAST thing in the run, while the guard is still alive: it
    /// must see every file the run wrote, and the guard's `Drop` sweeps them
    /// away (the borrow checker enforces the "inside the guard scope" half of
    /// that rule; `tests/capture_order.rs` asserts the "strictly last" half from
    /// the source text of both transports).
    ///
    /// Twin: `tools/oracle/corpus_guard.py::CorpusGuard.created`.
    pub fn created(&self) -> Option<Vec<String>> {
        if !self.snapshot_ok {
            return None;
        }
        let c = self.classify();
        if !c.complete {
            return None;
        }
        Some(c.names.into_iter().collect())
    }

    /// Delete what [`CorpusGuard::classify`] classified — the same
    /// classification the [`CorpusGuard::created`] report is built from, so the
    /// reported set can never disagree with the swept set. A run-created
    /// directory is removed wholesale; a pre-existing one keeps its
    /// pre-existing contents.
    ///
    /// Returns the normalized names of created entries the case directory STILL
    /// lists afterwards — the `sweep_failed` report of coordinator decision
    /// D32(2). A failed removal used to be swallowed (`let _ = remove_file`),
    /// and that is how the leak F4 measured could hide: dss_capi never closes
    /// its Storage trace stream (`src/PCElements/Storage.pas:872`, freed only at
    /// `:871`/`:1199`), so its guard's removal failed silently, the file
    /// survived, and every LATER producer of the same case snapshotted it as
    /// pre-existing and reported an empty created set. Reporting the survivor
    /// makes that order-coupling impossible to hide again.
    ///
    /// The presence test is a fresh `read_dir` of the case directory, not
    /// `Path::exists`: every root is a direct child of it, and a Windows
    /// delete-pending entry (a removal accepted while another handle is open)
    /// is still enumerated — and still blocks the next producer's write — while
    /// `metadata` on it fails. If the re-listing itself fails, every root is
    /// reported: an unprovable removal is never reported as a clean sweep.
    fn sweep_created(&self) -> Vec<String> {
        // The completeness flag is deliberately ignored here: an incomplete walk
        // still removes every dropping it *did* classify (all of them absent
        // from the pre-run snapshot), which is strictly better hygiene than
        // skipping the sweep.
        let c = self.classify();
        for (path, is_dir) in &c.roots {
            if *is_dir {
                let _ = std::fs::remove_dir_all(path);
            } else {
                let _ = std::fs::remove_file(path);
            }
        }
        if c.roots.is_empty() {
            return Vec::new();
        }
        let still: Option<BTreeSet<String>> = std::fs::read_dir(&self.dir).ok().map(|rd| {
            rd.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        });
        c.roots
            .iter()
            .filter(|(path, _)| match &still {
                None => true,
                Some(names) => path
                    .file_name()
                    .map(|n| names.contains(&n.to_string_lossy().into_owned()))
                    .unwrap_or(true),
            })
            .map(|(path, is_dir)| {
                let name = path.file_name().unwrap_or(path.as_os_str());
                normalize_created_name(&name.to_string_lossy(), *is_dir)
            })
            .collect()
    }

    /// Sweep and restore NOW, instead of on drop, and report the created entries
    /// the sweep could not remove (see [`CorpusGuard::sweep_created`]).
    ///
    /// This is how a transport gets the `sweep_failed` list into its reply: the
    /// reply is built while the guard is still alive, so `Drop` would run too
    /// late to be reported. Call it after the last read of the run (in
    /// particular after [`CorpusGuard::created`], which must see the files);
    /// `Drop` then does nothing.
    ///
    /// An empty vector on a guard whose pre-run snapshot was incomplete is
    /// honest: no sweep is attempted at all then ("incomplete snapshot = never
    /// delete"), so nothing leaked *through a sweep* — and the run-file surface
    /// already fails that case through the presence rail, since
    /// [`CorpusGuard::created`] returns `None`.
    pub fn finish(&mut self) -> Vec<String> {
        self.finished = true;
        if !self.snapshot_ok {
            return Vec::new();
        }
        let failed = self.sweep_created();
        self.restore();
        failed
    }

    /// Rewrite every small pre-existing file the run overwrote.
    fn restore(&self) {
        for (name, data) in &self.buf {
            let p = self.dir.join(name);
            match std::fs::read(&p) {
                // Untouched by the run.
                Ok(cur) if cur == *data => {}
                // Overwritten by the run: put the vendored bytes back.
                Ok(_) => {
                    let _ = std::fs::write(&p, data);
                }
                // GONE. Never write it back (G1.10a audit settlement, the
                // settle stage's measurement): a guard on a PARENT case
                // directory photographs a sibling case's directory too, and
                // when that sibling's own guard sweeps its output this arm
                // used to RESURRECT it — the `Test/AutoTrans/Auto{1,3}bus_*`
                // residue STATUS tracks, and the shrunken created-file set it
                // causes in the next producer. A deck that genuinely deletes
                // a vendored file leaves a tracked deletion, which is loud on
                // its own. The Python twin (`corpus_guard.py::__exit__`)
                // already behaved this way: it reads before it writes.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                // Unreadable for another reason (a held handle): try, as before.
                Err(_) => {
                    let _ = std::fs::write(&p, data);
                }
            }
        }
    }
}

impl Drop for CorpusGuard {
    fn drop(&mut self) {
        if self.finished || !self.snapshot_ok {
            return;
        }
        // No caller to hand the report to (an early error return, or a producer
        // that never calls `finish`), so it goes to stderr rather than nowhere:
        // D32(2) forbids a silently swallowed removal failure.
        let leaked = self.sweep_created();
        if !leaked.is_empty() {
            eprintln!(
                "corpus guard: leaked dropping(s) under {}: {} — the sweep could not \
                 remove them (a producer is still holding the file open). The guard was \
                 dropped without `finish()`, so no reply carries this report.",
                self.dir.display(),
                leaked.join(", ")
            );
        }
        self.restore();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shared synthetic fixture of GOLDEN_REBASE G1.10a: the Python twin
    /// builds the identical tree and asserts the identical lists
    /// (`tools/oracle/corpus_guard.py`, `SELF_TEST_*` +
    /// `python corpus_guard.py --self-test`), so the two guards are provably one
    /// classification written twice.
    const PRE_EXISTING: [&str; 3] = ["case.dss", "root.txt", "pre/keep.txt"];
    const RUN_WRITES: [&str; 5] = [
        "EXP_Y.CSV",
        "pre/New_Report.Txt",
        "DI_yr_0/Totals_1.CSV",
        "DI_yr_0/Sub/deep.DBL",
        "NEV_SavedVoltages.dbl",
    ];
    /// `pre/New_Report.Txt` is deliberately ABSENT: it sits under the
    /// pre-existing `pre/`, which the classification does not descend into
    /// (D30(2)) — it stands for the sibling case's live report file.
    const CREATED: [&str; 6] = [
        "di_yr_0/",
        "di_yr_0/sub/",
        "di_yr_0/sub/deep.dbl",
        "di_yr_0/totals_1.csv",
        "exp_y.csv",
        "nev_savedvoltages.dbl",
    ];
    const SCRATCH: [&str; 1] = ["nev_savedvoltages.dbl"];
    /// What the fixture's case dir holds after the guard's sweep: the
    /// pre-existing files, plus the one write under the pre-existing
    /// subdirectory the guard must neither report nor delete.
    const AFTER_SWEEP: [&str; 4] = ["case.dss", "pre/New_Report.Txt", "pre/keep.txt", "root.txt"];

    fn scratch_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("dss-epri-guard-tests").join(tag);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    fn write(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().expect("parent")).expect("create parent");
        std::fs::write(&p, text).expect("write fixture file");
    }

    /// Every file under `root`, `/`-joined and sorted (directories appear only
    /// through their files, which is enough to prove the sweep emptied them).
    fn files_under(root: &Path, prefix: &str, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(root) else {
            return;
        };
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                files_under(&entry.path(), &rel, out);
            } else {
                out.push(rel);
            }
        }
        out.sort();
    }

    #[test]
    fn classifies_the_shared_synthetic_fixture() {
        let root = scratch_dir("fixture");
        for rel in PRE_EXISTING {
            write(&root, rel, "pre-existing\n");
        }

        let case = root.join("case.dss");
        let created = {
            let guard = CorpusGuard::new(&case.to_string_lossy());
            for rel in RUN_WRITES {
                write(&root, rel, "run-written\n");
            }
            write(&root, "root.txt", "clobbered\n"); // the run also OVERWRITES
            guard
                .created()
                .expect("a complete snapshot reports the set")
        }; // guard drops here: it sweeps exactly what it classified

        assert_eq!(created, CREATED.map(String::from).to_vec());
        let (kept, scratch) = split_engine_scratch(&created);
        assert_eq!(scratch, SCRATCH.map(String::from).to_vec());
        assert_eq!(kept.len(), CREATED.len() - SCRATCH.len());

        let mut after = Vec::new();
        files_under(&root, "", &mut after);
        assert_eq!(
            after,
            {
                let mut want = AFTER_SWEEP.map(String::from).to_vec();
                want.sort();
                want
            },
            "the sweep removes exactly what it classified: the case dir's own \
             entries and the run-created tree go, the write under the \
             pre-existing subdirectory stays (D30(2))"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("root.txt")).expect("read restored"),
            "pre-existing\n",
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The measured sibling-case race (G1.10a F2b, D30(2)): while a case in
    /// `Test/` is running, a case in the pre-existing subdirectory
    /// `Test/AutoTrans/` writes its own report files there. The outer guard must
    /// neither REPORT them (they are not this run's output) nor DELETE them
    /// (they are another run's live output).
    #[test]
    fn a_sibling_cases_files_under_a_pre_existing_subdirectory_are_neither_reported_nor_swept() {
        let root = scratch_dir("sibling-race");
        write(&root, "CableParameters.dss", "! deck\n");
        write(&root, "AutoTrans/Auto3bus.dss", "! sibling deck\n");

        let case = root.join("CableParameters.dss");
        let created = {
            let guard = CorpusGuard::new(&case.to_string_lossy());
            // this run's own output
            write(&root, "dummy_LineConstants.txt", "run-written\n");
            // the sibling case, running concurrently in its own dir
            write(&root, "AutoTrans/Auto3bus_HT_current.txt", "sibling\n");
            guard
                .created()
                .expect("a complete snapshot reports the set")
        };

        assert_eq!(created, vec!["dummy_lineconstants.txt".to_string()]);
        assert!(
            root.join("AutoTrans/Auto3bus_HT_current.txt").exists(),
            "the sweep must not delete a concurrently running sibling case's file"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_incomplete_snapshot_refuses_to_report_and_never_deletes() {
        let root = scratch_dir("incomplete");
        write(&root, "pre.txt", "pre-existing\n");
        // A guard whose pre-run snapshot failed: it must neither report a set
        // (everything would look run-created) nor delete anything on drop.
        {
            let mut guard = CorpusGuard {
                dir: root.clone(),
                names: BTreeSet::new(),
                buf: BTreeMap::new(),
                snapshot_ok: false,
                finished: false,
            };
            assert_eq!(guard.created(), None);
            // No sweep is attempted at all, so nothing leaked THROUGH a sweep;
            // the surface fails that case through the presence rail above.
            assert!(guard.finish().is_empty());
        }
        assert!(root.join("pre.txt").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn normalize_created_name_is_the_python_twin() {
        assert_eq!(normalize_created_name(r"./A\B//C.CSV", false), "a/b/c.csv");
        assert_eq!(normalize_created_name("DI_yr_0", true), "di_yr_0/");
        // idempotent, so a consumer may re-apply it
        assert_eq!(normalize_created_name("di_yr_0/", true), "di_yr_0/");
        // ASCII-only fold: a non-ASCII name survives unfolded, so the
        // comparator can refuse it loudly instead of folding it by a locale
        // rule the Python twin would apply differently.
        assert_eq!(normalize_created_name("İST.CSV", false), "İst.csv");
    }

    /// D32(2): a removal the guard cannot perform is REPORTED, never swallowed.
    ///
    /// The fixture reproduces the mechanism F4 measured on the capi channel —
    /// dss_capi creates `STOR_<name>.csv` at edit time and never closes the
    /// stream (`src/PCElements/Storage.pas:868-885`, freed only at
    /// `:871`/`:1199`), so `os.remove` fails, the file survives its own guard's
    /// sweep and the NEXT producer of that case snapshots it as pre-existing and
    /// reports an empty created set. Here the created file is held open with
    /// `FILE_SHARE_READ` only, which is exactly what the FPC/Delphi file APIs
    /// do, so `remove_file` fails the same way.
    #[cfg(windows)]
    #[test]
    fn a_created_file_the_sweep_cannot_remove_is_reported_as_sweep_failed() {
        use std::os::windows::fs::OpenOptionsExt;
        /// `FILE_SHARE_READ` — deletion of the open file is NOT shared, so
        /// `DeleteFile` fails with a sharing violation (winnt.h).
        const FILE_SHARE_READ: u32 = 0x0000_0001;

        let root = scratch_dir("sweep-failed");
        write(&root, "case.dss", "pre-existing\n");
        let case = root.join("case.dss");

        let mut guard = CorpusGuard::new(&case.to_string_lossy());
        write(&root, "STOR_s1.CSV", "hour,t,...\n"); // the run's trace file
        write(&root, "EXP_Y.CSV", "y\n"); // an ordinary report next to it
        let created = guard
            .created()
            .expect("a complete snapshot reports the set");
        assert_eq!(created, vec!["exp_y.csv", "stor_s1.csv"]);

        // A producer still holding the trace file open, the capi way.
        let held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(root.join("STOR_s1.CSV"))
            .expect("open the created file");

        let sweep_failed = guard.finish();
        assert_eq!(
            sweep_failed,
            vec!["stor_s1.csv"],
            "the entry the sweep could not remove is reported (and only it: the \
             report file next to it was removed normally)"
        );
        let mut after = Vec::new();
        files_under(&root, "", &mut after);
        assert_eq!(
            after,
            vec!["STOR_s1.CSV".to_string(), "case.dss".to_string()]
        );

        drop(held);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The clean sweep reports nothing, and `finish` disarms the drop.
    #[test]
    fn a_clean_sweep_reports_no_leaked_dropping() {
        let root = scratch_dir("sweep-clean");
        write(&root, "case.dss", "pre-existing\n");
        let case = root.join("case.dss");

        let mut guard = CorpusGuard::new(&case.to_string_lossy());
        write(&root, "EXP_Y.CSV", "y\n");
        write(&root, "DI_yr_0/Totals_1.CSV", "t\n");
        assert_eq!(
            guard.created().expect("complete"),
            vec!["di_yr_0/", "di_yr_0/totals_1.csv", "exp_y.csv"]
        );
        assert!(guard.finish().is_empty());

        let mut after = Vec::new();
        files_under(&root, "", &mut after);
        assert_eq!(after, vec!["case.dss".to_string()]);

        // `Drop` must not sweep twice: a file written after `finish` is not
        // this run's output and stays.
        write(&root, "later.txt", "next case\n");
        drop(guard);
        let mut after = Vec::new();
        files_under(&root, "", &mut after);
        assert_eq!(after, vec!["case.dss".to_string(), "later.txt".to_string()]);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The double-quoted strings of one Python list/tuple literal, so the twin's
    /// fixture can be read without a Python interpreter.
    fn py_list(src: &str, name: &str) -> Vec<String> {
        let at = src
            .find(&format!(
                "
{name} = "
            ))
            .unwrap_or_else(|| panic!("`{name}` is gone from tools/oracle/corpus_guard.py"));
        let rest = &src[at..];
        let open = rest
            .find(['(', '['])
            .unwrap_or_else(|| panic!("`{name}` is no longer a list/tuple literal"));
        let (mut depth, mut in_str, mut cur, mut out) = (0usize, false, String::new(), Vec::new());
        for ch in rest[open..].chars() {
            if in_str {
                if ch == '"' {
                    in_str = false;
                    out.push(std::mem::take(&mut cur));
                } else {
                    cur.push(ch);
                }
                continue;
            }
            match ch {
                '"' => in_str = true,
                '(' | '[' => depth += 1,
                ')' | ']' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
        }
        assert_eq!(depth, 0, "`{name}`'s literal is unterminated");
        out
    }

    /// **Guard parity (GOLDEN_REBASE G1.10a spec §4.3): the Python twin's fixture
    /// IS this fixture, and the twin's own self-test runs inside `cargo test`.**
    ///
    /// `tools/oracle/corpus_guard.py` classifies the created-file SET on the
    /// capi_v0145 transport and this module classifies it for the r4133 transport
    /// AND the port, so "one classification, three consumers" (TESTING.md) is a
    /// claim about two languages. [`classifies_the_shared_synthetic_fixture`]
    /// proves only the Rust half; before this test the Python half rested on
    /// `python tools/oracle/corpus_guard.py --self-test`, which no gate ever ran,
    /// and on the two `SELF_TEST_*`/`const` lists being equal, which nothing
    /// checked — so a drift in the Python twin that the corpus happens not to
    /// exercise would ship silently. Here the four shared name lists are compared
    /// against THIS module's constants, the derived fifth is re-derived, and the
    /// twin is executed. Python is resolved exactly as the corpus gate resolves
    /// it (`DSS_ORACLE_PYTHON`, default `python`); a missing interpreter FAILS,
    /// like every other oracle prerequisite (TESTING.md: the gate fails rather
    /// than skipping).
    #[test]
    fn the_python_twin_shares_this_fixture_and_passes_its_self_test() {
        let script = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tools")
            .join("oracle")
            .join("corpus_guard.py");
        let src = std::fs::read_to_string(&script)
            .unwrap_or_else(|e| panic!("the Python twin {script:?} must be readable: {e}"));

        for (py, rs) in [
            ("SELF_TEST_PRE_EXISTING", PRE_EXISTING.to_vec()),
            ("SELF_TEST_RUN_WRITES", RUN_WRITES.to_vec()),
            ("SELF_TEST_CREATED", CREATED.to_vec()),
            ("SELF_TEST_SCRATCH", SCRATCH.to_vec()),
        ] {
            assert_eq!(
                py_list(&src, py),
                rs,
                "the shared G1.10a fixture drifted: `corpus_guard.py::{py}` no longer                  equals this module's constant. The two guards must classify the SAME                  tree, or `compare_run_files` compares two different surfaces on the                  two channels — change both sides in one commit."
            );
        }
        // The fifth list is DERIVED on the Python side; pin the derivation itself
        // and re-derive the Rust literal from it.
        assert!(
            src.contains(
                "SELF_TEST_AFTER_SWEEP = sorted(SELF_TEST_PRE_EXISTING + (\"pre/New_Report.Txt\",))"
            ),
            "`corpus_guard.py::SELF_TEST_AFTER_SWEEP` is no longer the pre-existing              files plus the sibling case's write under the pre-existing directory"
        );
        let mut want: Vec<&str> = PRE_EXISTING.to_vec();
        want.push("pre/New_Report.Txt");
        want.sort_unstable();
        assert_eq!(
            AFTER_SWEEP.to_vec(),
            want,
            "the swept tree's two sides disagree"
        );

        let python = std::env::var("DSS_ORACLE_PYTHON").unwrap_or_else(|_| "python".to_string());
        let out = std::process::Command::new(&python)
            .arg(&script)
            .arg("--self-test")
            .output()
            .unwrap_or_else(|e| {
                panic!(
                    "could not run the Python twin's self-test with {python:?}: {e}. The                      capi_v0145 transport needs a Python interpreter anyway (set                      DSS_ORACLE_PYTHON); this test fails rather than skipping."
                )
            });
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success() && stdout.contains("OK corpus_guard self-test"),
            "the Python guard twin failed its own shared-fixture self-test              (status {:?}).
--- stdout ---
{stdout}
--- stderr ---
{}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[test]
    fn is_engine_scratch_file_matches_only_the_dbl_scratch() {
        assert!(is_engine_scratch_file("NEV_SavedVoltages.dbl"));
        assert!(is_engine_scratch_file("nev_savedvoltages.dbl"));
        // the user-visible `Save Voltages` output all three engines write
        assert!(!is_engine_scratch_file("NEV_SavedVoltages.Txt"));
        // an unrelated .dbl (e.g. the r4133 `Visualize` data pair)
        assert!(!is_engine_scratch_file("testYgD_Transformer_tr1_PQ.dbl"));
    }

    // -----------------------------------------------------------------------
    // GOLDEN_REBASE G1.10b — the CONTENTS selection, its transport and its read.
    // -----------------------------------------------------------------------

    fn pat(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_string()).collect()
    }

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_string()).collect()
    }

    /// Every shipped pattern is the pinned shape, and the shape is enforced:
    /// a second `*`, a missing one, or an upper-case letter is refused, because
    /// the Python twin matches the very same list.
    #[test]
    fn the_contents_patterns_are_the_one_shape_both_guards_can_match() {
        for p in RUN_FILE_CONTENTS_PATTERNS {
            check_contents_pattern(p).unwrap_or_else(|e| panic!("{e}"));
        }
        for bad in ["exp_*_*.csv", "exp_y.csv", "*_EXP_Y.CSV"] {
            let err = check_contents_pattern(bad).expect_err("must be refused");
            assert!(err.contains(bad), "{err}");
        }
    }

    /// The selection: the six circuit-prefixed exports, the un-prefixed
    /// `EXP_PV_<NAME>.CSV` of the `/m` arm and the Storage trace, and nothing
    /// else — in particular no monitor CSV, no `Show` text report, no
    /// demand-interval member, and never a created DIRECTORY.
    #[test]
    fn the_selection_takes_the_report_csvs_and_leaves_the_rest() {
        let p = pat(&RUN_FILE_CONTENTS_PATTERNS);
        for yes in [
            "ieee8500u_exp_currents.csv",
            "ieee8500u_exp_powers.csv",
            "ieee8500u_exp_profile.csv",
            "fbs_exp_voltages.csv",
            "nev_exp_y.csv",
            "nev_exp_yprim.csv",
            "exp_pv_pv.csv",
            "stor_storage1.csv",
        ] {
            assert!(selects_contents(&p, yes), "{yes} must be selected");
        }
        for no in [
            // Monitors (A6) and the `Show`/`Dump` text reports.
            "ieee13nodeckt_mon_m1_1.csv",
            "yydtestcase_curr_elem.txt",
            "nev_systemy.txt",
            // Policy-less kinds and the deck-named exports (D40(5)).
            "kundur_two_area_jacobian.csv",
            "kundur_two_area_deltaf.csv",
            "auto1bus_hl_current.txt",
            // Near misses of the two `*_exp_y*` patterns.
            "nev_exp_ynodelist.csv",
            "nev_exp_ycurrents.csv",
            // A created directory is never a contents member.
            "ieee13nodecktmod/",
            // ... and the `*` never spans a directory separator.
            "di_yr_0/x_exp_y.csv",
        ] {
            assert!(!selects_contents(&p, no), "{no} must NOT be selected");
        }
    }

    /// The decode contract: CRLF (and a bare CR) fold to `\n`, and a byte
    /// sequence that is not UTF-8 is REFUSED rather than lossily decoded.
    #[test]
    fn a_report_decodes_with_folded_newlines_or_fails_loudly() {
        assert_eq!(
            decode_run_file(b"a,b\r\nc,d\r\n", "x").unwrap(),
            "a,b\nc,d\n"
        );
        assert_eq!(decode_run_file(b"a\rb", "x").unwrap(), "a\nb");
        let err = decode_run_file(&[b'a', 0xFF, b'b'], "exp_y.csv").expect_err("not UTF-8");
        assert!(
            err.contains("exp_y.csv") && err.contains("offset 1"),
            "{err}"
        );
    }

    /// The transport half and the port half read the same files: the sidecar
    /// copy names exactly what the in-place read decodes, and both resolve the
    /// oracles' two spellings (`EXP_Y.CSV` / `EXP_Y.csv`) through the fold.
    #[test]
    fn the_sidecar_copy_and_the_in_place_read_select_the_same_files() {
        let root = tmp_root("contents");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("NEV_EXP_Y.CSV"), b"Row,Col\r\n1,2\r\n").unwrap();
        std::fs::write(root.join("NEV_VLN_Node.txt"), b"not selected\n").unwrap();
        let created = names(&["nev_exp_y.csv", "nev_vln_node.txt"]);
        let p = pat(&RUN_FILE_CONTENTS_PATTERNS);

        let side = root.join("sidecar");
        let copied =
            copy_selected_contents(&root, Some(&created), &p, Some(side.to_str().unwrap()))
                .unwrap()
                .expect("patterns were given, so a report comes back");
        assert_eq!(copied, vec!["nev_exp_y.csv".to_string()]);
        assert_eq!(
            std::fs::read(side.join("nev_exp_y.csv")).unwrap(),
            b"Row,Col\r\n1,2\r\n",
            "the sidecar carries the BYTES; the decode happens once, on the gate side"
        );

        let read = read_selected_contents(&root, &created, &p).unwrap();
        assert_eq!(read.keys().collect::<Vec<_>>(), vec!["nev_exp_y.csv"]);
        assert_eq!(read["nev_exp_y.csv"], "Row,Col\n1,2\n");
        std::fs::remove_dir_all(&root).ok();
    }

    /// An empty pattern list is "the gate did not ask": no sidecar is created
    /// and `None` comes back, so the presence rail can tell that apart from
    /// "asked, and this deck wrote none of the selected reports" (`Some([])`).
    #[test]
    fn no_patterns_means_not_requested_and_touches_no_directory() {
        let root = tmp_root("contents-off");
        std::fs::create_dir_all(&root).unwrap();
        let side = root.join("sidecar");
        let empty: [&str; 0] = [];
        assert!(
            copy_selected_contents(
                &root,
                Some(&names(&["nev_exp_y.csv"])),
                &empty,
                Some(side.to_str().unwrap())
            )
            .unwrap()
            .is_none()
        );
        assert!(!side.exists(), "no request, no sidecar");
        let none = copy_selected_contents(
            &root,
            Some(&[]),
            &pat(&RUN_FILE_CONTENTS_PATTERNS),
            Some(side.to_str().unwrap()),
        )
        .unwrap()
        .expect("asked");
        assert!(
            none.is_empty(),
            "asked, and this deck wrote nothing selected"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    /// The sidecar is WIPED before it is written, so a file left by the other
    /// channel (or by an earlier case) can never be read as this run's output.
    #[test]
    fn the_sidecar_never_carries_a_previous_runs_file() {
        let root = tmp_root("contents-stale");
        let side = root.join("sidecar");
        std::fs::create_dir_all(&side).unwrap();
        std::fs::write(side.join("stale_exp_y.csv"), b"from the previous case\n").unwrap();
        std::fs::write(root.join("NEV_EXP_Y.CSV"), b"fresh\n").unwrap();
        let copied = copy_selected_contents(
            &root,
            Some(&names(&["nev_exp_y.csv"])),
            &pat(&RUN_FILE_CONTENTS_PATTERNS),
            Some(side.to_str().unwrap()),
        )
        .unwrap()
        .expect("asked");
        assert_eq!(copied, vec!["nev_exp_y.csv".to_string()]);
        assert!(!side.join("stale_exp_y.csv").exists());
        std::fs::remove_dir_all(&root).ok();
    }

    /// A selected member the directory no longer lists is a LOUD failure, never
    /// a quietly missing file: the set said the run wrote it, so the gate must
    /// see it.
    #[test]
    fn a_selected_report_that_vanished_fails_loudly() {
        let root = tmp_root("contents-gone");
        std::fs::create_dir_all(&root).unwrap();
        let p = pat(&RUN_FILE_CONTENTS_PATTERNS);
        let err = read_selected_contents(&root, &names(&["nev_exp_y.csv"]), &p)
            .expect_err("the file is not there");
        assert!(err.contains("no longer lists it as a file"), "{err}");
        let err = copy_selected_contents(
            &root,
            Some(&names(&["di_yr_0/x_exp_currents.csv"])),
            &pat(&["*_exp_currents.csv", "*/*_exp_currents.csv"]),
            Some(root.join("sidecar").to_str().unwrap()),
        )
        .expect_err("a two-star pattern is refused before anything is read");
        assert!(err.contains("exactly one `*`"), "{err}");
        std::fs::remove_dir_all(&root).ok();
    }

    /// The request may not carry patterns without the sidecar directory to put
    /// the bytes in, and it may not ask for contents when the created set itself
    /// could not be classified — both are transport errors, not empty answers.
    #[test]
    fn contents_without_a_sidecar_or_without_a_created_set_is_an_error() {
        let root = tmp_root("contents-req");
        std::fs::create_dir_all(&root).unwrap();
        let p = pat(&RUN_FILE_CONTENTS_PATTERNS);
        let err = copy_selected_contents(&root, Some(&names(&["nev_exp_y.csv"])), &p, None)
            .expect_err("no sidecar");
        assert!(err.contains("run_file_contents_dir"), "{err}");
        let err = copy_selected_contents(&root, None, &p, Some(root.to_str().unwrap()))
            .expect_err("no created set");
        assert!(err.contains("incomplete pre-run snapshot"), "{err}");
        std::fs::remove_dir_all(&root).ok();
    }

    fn tmp_root(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "dss_guard_{tag}_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::remove_dir_all(&root).ok();
        root
    }
}

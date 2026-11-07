//! Case runner: the Rust engine runs ONCE per case ([`run_rust_capture`]), then
//! its per-step state is compared against a channel's [`CaseResult`]
//! ([`compare_capture`]). The OpenDSS `CaseResult` is fetched by the scheduler
//! (through a persistent [`Channel`]) and passed in.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread::ThreadId;
use std::time::{Duration, Instant};

use dss_core::exec::Dss;
use serde_json::json;

use crate::engines::{CaseResult, Channel, Oracle, run_file_contents_dir};
use crate::harness::{
    self, ExportPolicy, RelCalcOutcome, RowPolicy, Tolerances, capture_guard,
    compare_all_properties, compare_ctrlqueue, compare_discrete, compare_element_channels,
    compare_element_cplx_seq, compare_element_derived, compare_element_extras,
    compare_element_phase_losses, compare_element_seq, compare_element_total_powers,
    compare_eventlog, compare_export, compare_fingerprint, compare_injection, compare_meter,
    compare_monitor, compare_pd_elements, compare_probe, compare_reliability, compare_system_y,
    compare_variables, compare_yprim, lane, tol_for,
};
use crate::manifest::{EngineChannel, SolvableCase};

// ---------------------------------------------------------------------------
// Corpus guard: it brackets this engine's scratch copy (`compare_with_result`,
// the two smoke arms), never a vendored folder.
// ---------------------------------------------------------------------------

/// Buffer small files up to this size for overwrite-restore. Mirrors
/// `oracle_server.py`'s `_RESTORE_MAX`.
const RESTORE_MAX: u64 = 2 * 1024 * 1024;

/// One directory's pristine snapshot, shared by every guard currently active on
/// that directory.
struct Snapshot {
    names: BTreeSet<String>,
    buf: BTreeMap<String, Vec<u8>>,
    snapshot_ok: bool,
}

/// One case directory's live claim: the pristine snapshot, the thread that owns
/// the directory right now, and how deep that thread is nested inside it.
struct DirClaim {
    /// Published by [`CorpusGuard::new`] before it returns. `None` only while
    /// the owning thread is photographing the directory — a window no other
    /// thread can observe, because the claim is taken first and blocks them out.
    snap: Option<Arc<Snapshot>>,
    owner: ThreadId,
    depth: usize,
}

/// Live claims per **canonical** case directory, plus the condvar a producer
/// waiting for a directory parks on.
///
/// The corpus puts many decks in one folder (`Test/AutoTrans`, `8500-Node`, …),
/// and a deck's own `Show`/`Export` lines run during `compile`, so two
/// producers in one folder would see each other's reports in their created-file
/// SETs. The claim is therefore **exclusive per directory**. A claim only ever
/// sits on this engine's own scratch copy (`CorpusGuard::new` refuses a
/// vendored folder): `run_one_case` takes none, the OpenDSS captures run in
/// their own copies without this guard, and `deferred_smoke_in` takes one on
/// its copy. It stays **reentrant for the
/// owning thread** — the overlap fixture below takes two nested guards, which
/// share the one pristine snapshot, so the names set is always the pre-run one
/// and exactly one sweep runs, when the last of them leaves.
type DirRegistry = (Mutex<HashMap<PathBuf, DirClaim>>, Condvar);

fn dir_registry() -> &'static DirRegistry {
    static REG: OnceLock<DirRegistry> = OnceLock::new();
    REG.get_or_init(|| (Mutex::new(HashMap::new()), Condvar::new()))
}

/// How long a producer waits for a case directory before calling it a deadlock.
/// A claim is held for ONE case (both OpenDSS captures + this engine's run +
/// the sweep), and the slowest gated case is a `large` feeder at well under a
/// minute, so this bound cannot fire on a healthy run. It exists so that a
/// leaked claim fails loudly instead of hanging the gate with no output at all.
const DIR_CLAIM_DEADLINE: Duration = Duration::from_secs(600);

/// The claim key: the case directory as the filesystem itself spells it, so two
/// manifest rows reaching one physical folder through different spellings
/// (`Test/AutoTrans` vs `Test/autotrans`, a `..` segment, a junction) contend
/// for the same claim instead of running unserialized side by side. Falls back
/// to the literal path when the directory cannot be canonicalized (it does not
/// exist — there is nothing to protect).
fn dir_claim_key(dir: &Path) -> PathBuf {
    std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf())
}

pub(crate) struct CorpusGuard {
    dir: PathBuf,
    key: PathBuf,
    snap: Arc<Snapshot>,
}

impl CorpusGuard {
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

    /// Claim the case directory — blocking while another thread runs a case in
    /// it — and photograph it.
    pub(crate) fn new(case_path: &str) -> Self {
        let dir = Path::new(case_path)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let key = dir_claim_key(crate::scratch::not_vendored(&dir, "runner::CorpusGuard"));
        let me = std::thread::current().id();
        let waiting = Instant::now();
        let (reg, cv) = dir_registry();
        let mut map = reg.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            match map.get_mut(&key) {
                // Already ours: a nested guard on the same directory reuses this
                // thread's pristine snapshot rather than photographing its own
                // run's output as "vendored", and counts the nesting depth so
                // that only the outermost one sweeps.
                Some(claim) if claim.owner == me => {
                    claim.depth += 1;
                    let snap = Arc::clone(claim.snap.as_ref().expect(
                        "a nested guard is only ever taken after `new` published \
                         this thread's snapshot",
                    ));
                    return Self { dir, key, snap };
                }
                // Another producer owns the directory: wait for it to sweep and
                // leave instead of photographing its output.
                Some(claim) => {
                    let owner = claim.owner;
                    let (guard, timed_out) = cv
                        .wait_timeout(map, Duration::from_secs(30))
                        .unwrap_or_else(|e| e.into_inner());
                    map = guard;
                    assert!(
                        !(timed_out.timed_out() && waiting.elapsed() > DIR_CLAIM_DEADLINE),
                        "corpus guard: waited {:?} for the case directory {}, still \
                         claimed by {owner:?}. A claim is held for exactly one case \
                         and released by `Drop`, so this is a leaked claim (a guard \
                         that outlives its case, or a producer stuck inside one) \
                         — never widen the deadline to get past it.",
                        waiting.elapsed(),
                        key.display(),
                    );
                }
                None => {
                    map.insert(
                        key.clone(),
                        DirClaim {
                            snap: None,
                            owner: me,
                            depth: 1,
                        },
                    );
                    break;
                }
            }
        }
        drop(map);
        // The claim is this thread's now. It lives in the guard BEFORE the walk
        // starts, so a panicking photograph releases the directory on unwind
        // instead of deadlocking every other producer on it; the placeholder it
        // carries until then is the "incomplete" snapshot, which never deletes.
        let mut guard = Self {
            dir,
            key,
            snap: Arc::new(Snapshot {
                names: BTreeSet::new(),
                buf: BTreeMap::new(),
                snapshot_ok: false,
            }),
        };
        // Photographed OUTSIDE the registry lock: the walk reads every small
        // file in the directory, and holding the global map across it would
        // serialize producers working in unrelated directories.
        let mut names = BTreeSet::new();
        let mut buf = BTreeMap::new();
        let snapshot_ok = Self::snapshot(&guard.dir, "", &mut names, &mut buf);
        guard.snap = Arc::new(Snapshot {
            names,
            buf,
            snapshot_ok,
        });
        reg.lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_mut(&guard.key)
            .expect("this thread's own claim is still registered")
            .snap = Some(Arc::clone(&guard.snap));
        guard
    }

    /// Remove what the run created, classifying with the SHARED rule the two
    /// OpenDSS transports and this engine's probe report from
    /// ([`dss_epri::guard::classify_created`]).
    ///
    /// The removal roots are the case dir's own created entries, and a created
    /// directory goes as one tree. The sweep never descends into a
    /// PRE-EXISTING subdirectory, which may be another case's directory with a
    /// case running in it right now. `dss_epri::guard` is pure `std::fs`, so the
    /// one classification holds on every platform.
    ///
    /// Returns the normalized names of created entries the case directory STILL
    /// lists afterwards — the same `sweep_failed` report its two twins make
    /// (`dss_epri::guard::CorpusGuard::sweep_created`,
    /// `corpus_guard.py::_sweep_created`). On a `kind=large*` case no
    /// `RunFileProbe` brackets the run and this guard is the ONLY sweeper, so a
    /// dropping it could not remove must still be loud. `Drop` prints what comes
    /// back. Nothing here panics, because a panicking `Drop` during another
    /// panic aborts the process.
    fn sweep_created(&self) -> Vec<String> {
        let c = dss_epri::guard::classify_created(&self.dir, &self.snap.names);
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
        // A fresh listing, not `Path::exists`: every root is a direct child of
        // the case dir, and a Windows delete-pending entry (a removal accepted
        // while another handle is open) is still enumerated — and still blocks
        // the next producer's write — while `metadata` on it fails. An
        // unprovable removal is never reported as a clean sweep.
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
                dss_epri::guard::normalize_created_name(&name.to_string_lossy(), *is_dir)
            })
            .collect()
    }
}

impl Drop for CorpusGuard {
    fn drop(&mut self) {
        let (reg, cv) = dir_registry();
        {
            let mut map = reg.lock().unwrap_or_else(|e| e.into_inner());
            // A guard nested inside this thread's own: only the outermost one
            // sweeps, when every writer of this thread is done.
            match map.get_mut(&self.key) {
                Some(claim) if claim.depth > 1 => {
                    claim.depth -= 1;
                    return;
                }
                _ => {}
            }
        }
        // Last one out. Sweep and restore while the claim is STILL held, so the
        // next producer of this directory cannot photograph this run's droppings
        // as vendored; only then release the
        // directory and wake whoever is waiting for it.
        if self.snap.snapshot_ok {
            let leaked = self.sweep_created();
            if !leaked.is_empty() {
                // A removal failure is never swallowed silently. This
                // guard cannot fail the case from `Drop` (a panic here during
                // another panic aborts the process), so it says so on stderr,
                // where a full-drive log keeps it greppable.
                eprintln!(
                    "corpus guard: leaked dropping(s) under {}: {} - the sweep could not remove them (a producer still holds the file open). The folder is this engine's scratch copy, whose bounded removal (`ScratchCopy::finish`) then fails the case naming the producer.",
                    self.dir.display(),
                    leaked.join(", "),
                );
            }
            for (name, data) in &self.snap.buf {
                let p = self.dir.join(name);
                match std::fs::read(&p) {
                    // Untouched by the run.
                    Ok(cur) if cur == *data => {}
                    // Overwritten by the run: put the vendored bytes back.
                    Ok(_) => {
                        let _ = std::fs::write(&p, data);
                    }
                    // GONE. Never write it back: a guard on a PARENT case
                    // directory photographs a sibling case's directory too, and
                    // when that sibling's own guard sweeps its output, writing
                    // it back would RESURRECT it and shrink the next producer's
                    // created-file set. A deck that genuinely deletes a vendored
                    // file leaves a tracked deletion, which is loud on its own.
                    // The Python twin (`corpus_guard.py::__exit__`) reads
                    // before it writes too.
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    // Unreadable for another reason (a held handle): try, as before.
                    Err(_) => {
                        let _ = std::fs::write(&p, data);
                    }
                }
            }
        }
        let mut map = reg.lock().unwrap_or_else(|e| e.into_inner());
        map.remove(&self.key);
        cv.notify_all();
    }
}

#[test]
fn corpus_guard_restores_case_dir_recursively() {
    let root = std::env::temp_dir().join(format!("dss_guard_test_{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    let sub = root.join("support");
    std::fs::create_dir_all(&sub).unwrap();
    let case = root.join("case.dss");
    std::fs::write(&case, b"! fixture master").unwrap();
    let fixture = sub.join("fixture.txt");
    std::fs::write(&fixture, b"vendored bytes").unwrap();
    {
        let _guard = CorpusGuard::new(&case.to_string_lossy());
        std::fs::write(root.join("run_created.csv"), b"pollution").unwrap();
        std::fs::write(sub.join("run_created_inner.csv"), b"pollution").unwrap();
        let di = root.join("ckt_di").join("DI_yr_1");
        std::fs::create_dir_all(&di).unwrap();
        std::fs::write(di.join("x.csv"), b"pollution").unwrap();
        std::fs::write(&fixture, b"overwritten by the run").unwrap();
    }
    assert!(case.is_file(), "vendored master must survive");
    assert_eq!(
        std::fs::read(&fixture).unwrap(),
        b"vendored bytes",
        "overwritten pre-existing fixture must be restored"
    );
    assert!(!root.join("run_created.csv").exists());
    assert!(
        sub.join("run_created_inner.csv").exists(),
        "a file appearing under a PRE-EXISTING subdirectory must NOT be swept: \
         the gate schedules by case dir, so `support/` may itself be a manifest \
         case directory whose case is running right now, and this file is its \
         live output (the shared classification \
         `dss_epri::guard::classify_created`)"
    );
    assert!(
        !root.join("ckt_di").exists(),
        "run-created dir tree removed"
    );
    std::fs::remove_dir_all(&root).ok();
}

/// A guard on a PARENT case directory never RESURRECTS the output a sibling
/// case's guard has just swept.
///
/// A parent directory and its child are different claim keys, so they run
/// concurrently by design, and the parent's pre-run photograph may buffer what
/// the child's run already wrote. A snapshot entry that is GONE at restore
/// stays gone (a deck that deletes a vendored file shows up as a tracked
/// deletion, which is loud), while an entry that still exists and was
/// overwritten is still restored, the contract
/// `corpus_guard_restores_case_dir_recursively` pins.
#[test]
fn a_parent_guard_does_not_resurrect_a_sibling_cases_swept_output() {
    let root = std::env::temp_dir().join(format!("dss_guard_sibling_{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    let sub = root.join("AutoTrans");
    std::fs::create_dir_all(&sub).unwrap();
    let parent_case = root.join("parent.dss");
    std::fs::write(&parent_case, b"! parent case master").unwrap();
    let child_case = sub.join("child.dss");
    std::fs::write(&child_case, b"! child case master").unwrap();

    let out = sub.join("child_export.txt");
    {
        // The child case is running: its guard is up and its deck has written
        // its export.
        let gchild = CorpusGuard::new(&child_case.to_string_lossy());
        std::fs::write(&out, b"the child run's export").unwrap();
        // …and only now does the parent case start, photographing a directory
        // that already holds the child's live output.
        let gparent = CorpusGuard::new(&parent_case.to_string_lossy());
        // The child finishes first and sweeps what it created.
        drop(gchild);
        assert!(!out.exists(), "the child's own guard sweeps its own output");
        drop(gparent);
    }
    assert!(
        !out.exists(),
        "the parent guard resurrected {} — a sibling case's swept output must          never be written back by a guard that only photographed it",
        out.display()
    );
    assert!(child_case.is_file(), "the vendored child master survives");
    assert!(parent_case.is_file(), "the vendored parent master survives");
    std::fs::remove_dir_all(&root).ok();
}

/// The outer guard REPORTS a removal it could not perform, exactly like its two
/// twins
/// (`dss_epri::guard::tests::a_created_file_the_sweep_cannot_remove_is_reported_as_sweep_failed`,
/// `corpus_guard.py`'s `SELF_TEST_LEAK_*`). The fixture holds the created file
/// open with `FILE_SHARE_READ` only, as dss_capi holds a never-closed Storage
/// trace stream, so `remove_file` fails with a sharing violation.
#[cfg(windows)]
#[test]
fn the_outer_guard_reports_a_created_file_it_cannot_remove() {
    use std::os::windows::fs::OpenOptionsExt;
    /// `FILE_SHARE_READ` — deletion of the open file is NOT shared, so
    /// `DeleteFile` fails with a sharing violation (winnt.h).
    const FILE_SHARE_READ: u32 = 0x0000_0001;

    let root = std::env::temp_dir().join(format!("dss_guard_leak_{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    std::fs::create_dir_all(&root).unwrap();
    let case = root.join("case.dss");
    std::fs::write(&case, b"! fixture master").unwrap();

    let guard = CorpusGuard::new(&case.to_string_lossy());
    std::fs::write(
        root.join("STOR_s1.CSV"),
        b"hour,t
",
    )
    .unwrap();
    std::fs::write(
        root.join("EXP_Y.CSV"),
        b"y
",
    )
    .unwrap();
    let held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(root.join("STOR_s1.CSV"))
        .expect("open the created file");

    let leaked = guard.sweep_created();
    assert_eq!(
        leaked,
        vec!["stor_s1.csv".to_string()],
        "the entry the sweep could not remove is reported, and only it: the          ordinary report file next to it was removed normally"
    );
    assert!(!root.join("EXP_Y.CSV").exists());

    drop(held);
    drop(guard);
    std::fs::remove_dir_all(&root).ok();
}

/// Two guards on the **same** deck folder nested inside one thread — what
/// `assert_deferred_rust_smoke` does inside `scheduler::run_one_case`'s guard.
/// That is the only way two guards on one directory can overlap (another
/// thread blocks in `new`), and this pins the interleaving that would leak
/// without the shared snapshot: A snapshots clean → A writes → B snapshots (sees A's
/// output) → A drops and sweeps → B writes again → B drops and *keeps* it. With
/// the shared per-directory snapshot the nested guard reuses the pristine names
/// and the sweep runs once, when the last guard leaves.
#[test]
fn corpus_guard_overlapping_guards_still_sweep() {
    let root = std::env::temp_dir().join(format!("dss_guard_overlap_{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    std::fs::create_dir_all(&root).unwrap();
    let case_a = root.join("a.dss");
    let case_b = root.join("b.dss");
    std::fs::write(&case_a, b"! deck a").unwrap();
    std::fs::write(&case_b, b"! deck b").unwrap();

    let out = root.join("a_EXP_VOLTAGES.csv");
    {
        let ga = CorpusGuard::new(&case_a.to_string_lossy());
        std::fs::write(&out, b"run a output").unwrap();
        // B starts while A's output is on disk — must NOT adopt it as vendored.
        let gb = CorpusGuard::new(&case_b.to_string_lossy());
        drop(ga);
        assert!(
            out.exists(),
            "the inner guard must not sweep while the outer one is still running"
        );
        std::fs::write(&out, b"run b output").unwrap();
        drop(gb);
    }
    assert!(!out.exists(), "the last guard out must sweep the leftovers");
    assert!(case_a.is_file() && case_b.is_file(), "decks must survive");
    std::fs::remove_dir_all(&root).ok();
}

/// One producer at a time owns a case directory. Two synthetic manifest rows in
/// one folder (the `IEEETestCases/8500-Node` shape, where several rows plus
/// `ad_sweep.json`'s `pf` decks all write `<CircuitName>_*` reports into ONE
/// directory) must not have their before/after windows overlap, or one
/// producer's output lands in the other's created-file SET.
///
/// Deterministic in the direction that matters: the second thread records
/// whether the first was still holding the directory at the moment it got in,
/// and that flag is what the test asserts. The bounded `recv_timeout` below is
/// the second, weaker half (it can only make the test slower, never green).
#[test]
fn corpus_guard_serializes_two_threads_in_one_case_directory() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc;

    let root = std::env::temp_dir().join(format!("dss_guard_serial_{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    std::fs::create_dir_all(&root).unwrap();
    let case_a = root.join("a.dss");
    let case_b = root.join("b.dss");
    std::fs::write(&case_a, b"! deck a").unwrap();
    std::fs::write(&case_b, b"! deck b").unwrap();

    let a_holds = AtomicBool::new(false);
    let (trying_tx, trying_rx) = mpsc::channel::<()>();
    let (got_tx, got_rx) = mpsc::channel::<()>();

    std::thread::scope(|s| {
        let ga = CorpusGuard::new(&case_a.to_string_lossy());
        a_holds.store(true, Ordering::SeqCst);
        let b = s.spawn(|| {
            trying_tx.send(()).unwrap();
            let _gb = CorpusGuard::new(&case_b.to_string_lossy());
            // What B saw the instant it owned the directory.
            let saw_a_inside = a_holds.load(Ordering::SeqCst);
            let b_sees = std::fs::read_dir(&root)
                .unwrap()
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect::<BTreeSet<String>>();
            got_tx.send(()).unwrap();
            (saw_a_inside, b_sees)
        });
        trying_rx.recv().unwrap();
        assert!(
            got_rx.recv_timeout(Duration::from_millis(500)).is_err(),
            "the second producer entered a case directory the first one still owned"
        );
        // A's run writes its report while it still owns the folder.
        std::fs::write(root.join("a_exp_voltages.csv"), b"run a output").unwrap();
        a_holds.store(false, Ordering::SeqCst);
        drop(ga);
        got_rx
            .recv_timeout(Duration::from_secs(60))
            .expect("the second producer must get the directory once the first leaves");
        let (saw_a_inside, b_sees) = b.join().unwrap();
        assert!(
            !saw_a_inside,
            "the second producer took the case directory while the first still \
             owned it — its snapshot would photograph the first run's output"
        );
        assert!(
            !b_sees.contains("a_exp_voltages.csv"),
            "the first producer's report was still on disk when the second \
             snapshotted the directory: {b_sees:?}"
        );
    });
    assert!(case_a.is_file() && case_b.is_file(), "decks must survive");
    std::fs::remove_dir_all(&root).ok();
}

/// The other direction: the claim is per DIRECTORY, not a global lock on the
/// corpus. Two producers in DIFFERENT case folders keep running at the same
/// time, which the gate's parallelism depends on.
#[test]
fn corpus_guard_does_not_serialize_two_different_case_directories() {
    use std::sync::mpsc;

    let root = std::env::temp_dir().join(format!("dss_guard_parallel_{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    let dir_a = root.join("case_a");
    let dir_b = root.join("case_b");
    std::fs::create_dir_all(&dir_a).unwrap();
    std::fs::create_dir_all(&dir_b).unwrap();
    let case_a = dir_a.join("a.dss");
    let case_b = dir_b.join("b.dss");
    std::fs::write(&case_a, b"! deck a").unwrap();
    std::fs::write(&case_b, b"! deck b").unwrap();

    let (got_tx, got_rx) = mpsc::channel::<()>();
    std::thread::scope(|s| {
        let ga = CorpusGuard::new(&case_a.to_string_lossy());
        s.spawn(|| {
            let _gb = CorpusGuard::new(&case_b.to_string_lossy());
            got_tx.send(()).unwrap();
        });
        got_rx.recv_timeout(Duration::from_secs(60)).expect(
            "a producer in ANOTHER case directory must not wait for this one: the \
             claim is per case dir, never a global corpus lock",
        );
        drop(ga);
    });
    std::fs::remove_dir_all(&root).ok();
}

// ---------------------------------------------------------------------------
// Warnings reconciliation (unchanged).
// ---------------------------------------------------------------------------

fn assert_expected_warnings(dss: &Dss, expect: &[String], ctx: &str) {
    let errors = dss.errors();
    if expect.is_empty() {
        assert!(
            errors.is_empty(),
            "{ctx}: unexpected Rust engine errors: {errors:?}"
        );
        return;
    }
    let unexpected: Vec<_> = errors
        .iter()
        .filter(|e| !expect.iter().any(|w| e.contains(w.as_str())))
        .collect();
    assert!(
        unexpected.is_empty(),
        "{ctx}: Rust engine errors not covered by expect_warnings: {unexpected:?}"
    );
    for w in expect {
        assert!(
            errors.iter().any(|e| e.contains(w.as_str())),
            "{ctx}: expected warning {w:?} never fired (actual: {errors:?})"
        );
    }
}

// ---------------------------------------------------------------------------
// run_rust_capture + compare_capture (the split of the old run_and_compare).
// ---------------------------------------------------------------------------

/// The lowercase object names of one class among the snapshotted circuit
/// elements (`snapshot_elements` walks `Circuit.ckt_elements`, which holds the
/// control elements too).
fn class_member_names(snaps: &[dss_core::exec::ElementSnapshot], class: &str) -> BTreeSet<String> {
    snaps
        .iter()
        .filter_map(|s| s.name.split_once('.'))
        .filter(|(cls, _)| cls.eq_ignore_ascii_case(class))
        .map(|(_, name)| name.to_lowercase())
        .collect()
}

/// This channel's tag for the [`capture_guard`] refusal messages
/// (`capi_v0145` / `r4133`).
///
/// It reuses the single channel→tag mapping the tree already has
/// ([`harness::PropsChannel::tag`]) instead of adding a second copy:
/// `EngineChannel` is `pub(crate)` to this one test binary, and the harness
/// library cannot name a type of it, so the guard takes a `&str`
/// ([`EngineChannel::props_channel`] records that channel-threading trap).
fn channel_tag(channel: EngineChannel) -> &'static str {
    channel.props_channel().tag()
}

/// Compile + post + reconcile warnings; return the driven [`Dss`] (not yet
/// solved) and the baseline error count. The Rust engine runs ONCE per case;
/// [`compare_capture`] then advances + compares it step by step.
pub(crate) fn run_rust_capture(label: &str, case_path: &str, c: &SolvableCase) -> (Dss, usize) {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile \"{case_path}\""));
    for cmd in &c.post {
        dss.command(cmd);
    }
    assert_expected_warnings(&dss, &c.expect_warnings, &format!("{label}: after compile"));
    // A `compile` that produced NEITHER a circuit NOR an error can only mean
    // the master file read back with no command in it: `exec/solve.rs::do_redirect`
    // errors loudly on a missing (`Redirect file not found`) or unreadable
    // (`could not be read`) file, but a SHORT read (an empty file, or just its
    // leading comment block) yields no command and no diagnostic, and every
    // later command then fails with "You must create a new circuit object
    // first". Assert it HERE, where the deck's size on disk is still evidence,
    // instead of one step later where only the symptom shows. Never retry the
    // case: a deck that reads short means a producer is writing it.
    assert!(
        dss.circuit().is_some(),
        "{label}: `compile` left the port with no active circuit and reported no          error — the deck read back with no command in it. `{case_path}` is {}          byte(s) on disk right now; if that is its full size the deck is broken,          and if it is short a concurrent producer (a `CorpusGuard` restore, an          editor, a sync tool) was writing the file while the port read it.",
        std::fs::metadata(case_path)
            .map(|m| m.len() as i64)
            .unwrap_or(-1),
    );
    let baseline_errors = dss.errors().len();
    (dss, baseline_errors)
}

/// Compare the (once-run) Rust engine against ONE channel's `CaseResult`, per
/// step, in the fixed comparator order. Where a per-case-per-channel `ledger`
/// scope partitions a field, the untouched `harness` comparator runs on the
/// unscoped remainder while the ledger's envelope/exact-pair assert covers the
/// scoped part and records the hit. With `ledger = None` or an empty view every
/// field takes the plain path.
#[allow(clippy::too_many_arguments)]
pub(crate) fn compare_capture(
    dss: &mut Dss,
    baseline_errors: usize,
    oc: &CaseResult,
    label: &str,
    case_path: &str,
    c: &SolvableCase,
    tol: &Tolerances,
    channel: EngineChannel,
    ledger: Option<&crate::ledger::LedgerView>,
) {
    let n_steps = c.n_steps;
    let star = c.selected_elements == ["*"];
    // A view with no applicable entries behaves exactly like `None` (fast path).
    let ledger = ledger.filter(|v| !v.is_empty());
    // This engine's isolation topology at step 0 of THIS case+channel
    // run — the memoization reference the topology comparator rebases onto once
    // a conductor has opened (`harness::topology::compare_topology`).
    let mut topo_step0: Option<harness::topology::IsolationSnapshot> = None;

    // The reliability protocol: `RelCalc` is NOT idempotent (a nested or
    // adjoining zone re-reads the inner meter's `Bus.TotalMiles` as its own
    // downstream mileage on the second run), so the gate runs it exactly once
    // per case, on the LAST step, on all three engines. Assert the channel did
    // the same before comparing anything: exactly one checkpoint may carry the
    // payload and it must be the last one, and NO checkpoint may carry it when
    // the flag is off. `require_capture_opt` below then covers the per-case
    // "the flag is on but this channel sent nothing" hole.
    {
        let carriers: Vec<usize> = oc
            .checkpoints
            .iter()
            .enumerate()
            .filter(|(_, cp)| cp.reliability.is_some())
            .map(|(i, _)| i)
            .collect();
        let expected: Vec<usize> = if c.compare_reliability {
            vec![n_steps - 1]
        } else {
            Vec::new()
        };
        assert_eq!(
            carriers, expected,
            "{label} [{channel:?}]: reliability payload on checkpoint(s) {carriers:?}, \
             expected {expected:?} (compare_reliability = {}). The transport must drive \
             `RelCalc` once, after the LAST solve, and attach the payload to that \
             checkpoint only.",
            c.compare_reliability,
        );
    }

    for (i, cp) in oc.checkpoints.iter().enumerate() {
        dss.command("solve");
        assert_eq!(
            dss.errors().len(),
            baseline_errors,
            "{label} step {i}: new Rust engine errors: {:?}",
            &dss.errors()[baseline_errors.min(dss.errors().len())..]
        );
        let rust_global_result = dss.result().to_string();
        let ctx = format!("{label} step {i}");

        // Drive the executive `RelCalc` HERE - after the
        // per-step error assert and the `Text.Result`/`dss.result()` read (the
        // command overwrites the result), before every comparator of this step,
        // and only on the last one. Both transports drive it at exactly this
        // point (`oracle_server.py::run_case`, `dss-epri::capture::run_case`), so
        // the reliability payload AND the fields the sweep writes into surfaces
        // that are already gated - `PDElements.{AccumulatedL,Lambda,TotalMiles,
        // SectionID}`, the `Bus` reliability columns, EnergyMeter properties
        // #19-23 - are read post-calc on all three engines.
        //
        // The abort (errno 52902, a zone with no OCP device) is a compared
        // observable, not a failure: the port pushes its message onto
        // `Dss::errors` (`exec/solve.rs::do_relcalc_cmd` ->
        // `solution/meters/reliability.rs:53-57`, one per failing meter) and
        // `compare_reliability` asserts boolean+message symmetry against the
        // channel. Reading the new lines here is also what keeps the next step's
        // `baseline_errors` assert meaningful - and there is no next step.
        let mut rust_relcalc = RelCalcOutcome::default();
        if c.compare_reliability && i + 1 == n_steps {
            let before = dss.errors().len();
            dss.command("RelCalc");
            let new: Vec<String> = dss.errors()[before..]
                .iter()
                .map(|e| e.message.clone())
                .collect();
            rust_relcalc = RelCalcOutcome::from_new_errors(&new);
        }

        {
            let ckt = dss.circuit().expect("circuit exists");
            assert!(
                cp.converged,
                "{ctx}: oracle did not converge (persisted across the oracle's in-process retries)"
            );
            assert!(ckt.is_solved, "{ctx}: Rust did not converge");
            assert!(
                (ckt.solution.dbl_hour - cp.dbl_hour).abs() < 1e-9,
                "{ctx}: dblHour {} vs {}",
                ckt.solution.dbl_hour,
                cp.dbl_hour
            );
            // Iteration policy: the pinned capi_v0145 channel must match the
            // iteration count exactly, and the r4133 channel (a different engine
            // line) allows this engine to converge in FEWER iterations, never
            // more. A ledger `iterations` scope overrides both (exact pair, or an
            // explicit rust_le_oracle where a revision delta needs pinning).
            //
            // Both non-ledgered shapes go
            // through `harness::lane`, which keeps them exactly as above in the
            // parity lane and grants the default lane its documented
            // ±`ITER_SLACK` band. Ledger-scoped pins stay exact in BOTH lanes:
            // they are hit-tracked (fail-on-stale), so a default-lane flip that
            // moves one must be re-triaged in the ledger, not silently absorbed.
            let iters_ledgered = ledger.is_some_and(|v| {
                v.iterations_handled(i, ckt.solution.iteration, cp.iterations, &ctx)
            });
            if !iters_ledgered {
                if channel.iterations_exact() {
                    harness::lane::compare_iterations(ckt.solution.iteration, cp.iterations, &ctx);
                } else {
                    harness::lane::compare_iterations_le(
                        ckt.solution.iteration,
                        cp.iterations,
                        &ctx,
                    );
                }
            }
            let names: Vec<String> = (1..=ckt.num_nodes).map(|j| ckt.node_name(j)).collect();
            assert_eq!(names, oc.node_order, "{ctx}: node order differs");

            let mut actual = Vec::with_capacity(2 * ckt.num_nodes);
            for j in 1..=ckt.num_nodes {
                actual.push(ckt.solution.node_v[j].re);
                actual.push(ckt.solution.node_v[j].im);
            }
            let mut expected = Vec::with_capacity(actual.len());
            for (re, im) in cp.v_re.iter().zip(&cp.v_im) {
                expected.push(*re);
                expected.push(*im);
            }
            // A ledger voltage scope partitions the nodes: the scoped nodes are
            // envelope-checked inside `voltage_keep_mask` (hit recorded), the
            // unscoped remainder still meets the tier floor here.
            match ledger
                .and_then(|v| v.voltage_keep_mask(i, &oc.node_order, &actual, &expected, tol, &ctx))
            {
                Some(keep) => {
                    let mut ra = Vec::with_capacity(actual.len());
                    let mut re = Vec::with_capacity(expected.len());
                    for (ni, k) in keep.iter().enumerate() {
                        if *k {
                            ra.push(actual[2 * ni]);
                            ra.push(actual[2 * ni + 1]);
                            re.push(expected[2 * ni]);
                            re.push(expected[2 * ni + 1]);
                        }
                    }
                    harness::assert_complex_close(&ra, &re, tol.v_rel, tol.v_abs, &ctx);
                }
                None => {
                    harness::assert_complex_close(&actual, &expected, tol.v_rel, tol.v_abs, &ctx);
                }
            }
        }

        // Whole-artifact ledger exclusions: the
        // assembled Y, its fingerprint, an element's YPrim and a meter's
        // register block have no partition and no envelope, so an entry that
        // names one drops it for this (case, channel) — hit-accounted, so it
        // still fails the gate the day it stops matching. The oracle payload is
        // still demanded (a missing Y is a protocol failure, not a divergence).
        let excluded =
            |field: &str, name: Option<&str>| ledger.is_some_and(|v| v.excluded(field, name, i));
        if let Some(y) = &cp.y {
            if !excluded("y", None) {
                compare_system_y(dss, y, &oc.node_order, tol, &ctx);
            }
        } else {
            panic!("{ctx}: oracle returned no full Y (the live gate requires it)");
        }
        if !excluded("y_fingerprint", None) {
            compare_fingerprint(dss, &cp.y_fingerprint, tol, &ctx);
        }

        let snaps = dss.snapshot_elements();

        if star {
            let yprim_names: BTreeSet<String> =
                cp.yprims.iter().map(|y| y.name.to_lowercase()).collect();
            let all_names: BTreeSet<String> = snaps
                .iter()
                .filter(|s| dss.element_yprim(&s.name).is_some())
                .map(|s| s.name.to_lowercase())
                .collect();
            assert_eq!(
                yprim_names, all_names,
                "{ctx}: selected_elements=[\"*\"] must yield a YPrim block for every \
                 YPrim-bearing element"
            );
        } else {
            assert_eq!(
                cp.yprims.len(),
                c.selected_elements.len(),
                "{ctx}: oracle returned {} YPrim block(s) for {} selected element(s)",
                cp.yprims.len(),
                c.selected_elements.len()
            );
        }
        for yp in &cp.yprims {
            if !excluded("yprim", Some(&yp.name)) {
                compare_yprim(dss, yp, tol, &ctx);
            }
        }
        // A ledger `injection` scope envelope-checks the whole RHS here; an
        // `exclusion` drops it (the RHS has no sub-selector, so both forms are
        // all-or-nothing anyway).
        if !excluded("injection", None)
            && !ledger.is_some_and(|v| v.injection_handled(i, dss, &cp.injection, tol, &ctx))
        {
            compare_injection(dss, &cp.injection, tol, &ctx);
        }

        let rust_names: BTreeSet<String> = snaps.iter().map(|s| s.name.to_lowercase()).collect();
        let oracle_names: BTreeSet<String> =
            cp.elements.iter().map(|e| e.name.to_lowercase()).collect();
        assert_eq!(
            rust_names,
            oracle_names,
            "{ctx}: element name sets differ (Rust∖oracle={:?}, oracle∖Rust={:?})",
            rust_names.difference(&oracle_names).collect::<Vec<_>>(),
            oracle_names.difference(&rust_names).collect::<Vec<_>>(),
        );
        // A ledger element scope neutralizes only its pinned sub-channels: it
        // rewrites those to the Rust values (after re-asserting them inside their
        // envelope) so the standard `compare_element` treats them as equal and
        // still tier-checks the unscoped remainder (clause (b)). Elements with no
        // scope are compared against the untouched oracle cap.
        let el_rewrites = ledger
            .map(|v| v.element_rewrites(i, &snaps, &cp.elements, tol, &ctx))
            .unwrap_or_default();
        // The lane policy drops the two `S = V·conj(I)` sub-channels on the
        // `newton*` decks in **both** lanes
        // (no oracle channel reports them at the converged `NodeV`; pinned by
        // its own expected-value test) — every other case gets
        // `ElemChannels::ALL`. See `harness::lane`.
        let channels = lane::elem_channels_for(label);
        for ec in &cp.elements {
            match el_rewrites.get(&ec.name.to_lowercase()) {
                Some(rw) => compare_element_channels(&snaps, rw, tol, &ctx, channels),
                None => compare_element_channels(&snaps, ec, tol, &ctx, channels),
            }
        }

        // The five `Circuit` aggregates and the
        // ten `Solution` scalars. The surface is UNFLAGGED and universal (no
        // `G1_SURFACE_FLAGS` row, no rigor token, no forced population), so the
        // capture is demanded on every live case of every gating channel; the
        // `capture_guard` rail is not reused because its message is
        // manifest-flag shaped and there is no flag to name here.
        let agg = cp.aggregates.as_ref().unwrap_or_else(|| {
            panic!(
                "{ctx}: the `{}` capture carries no `aggregates` member. The aggregates are an \
                 unflagged, universal surface — every live case must compare it, so \
                 an absent capture FAILS the case instead of silently comparing \
                 nothing.",
                channel_tag(channel)
            )
        });
        let scalars = cp.solution_scalars.as_ref().unwrap_or_else(|| {
            panic!(
                "{ctx}: the `{}` capture carries no `solution_scalars` member \
                 (unflagged universal surface — see the `aggregates` refusal above).",
                channel_tag(channel)
            )
        });
        harness::aggregates::compare_aggregates(
            dss,
            agg,
            &snaps,
            &cp.elements,
            &el_rewrites,
            tol,
            channels,
            &ctx,
        );
        harness::aggregates::compare_solution_scalars(
            dss,
            scalars,
            cp.iterations,
            channel.iterations_exact(),
            &ctx,
        );
        // The per-element **derived** channels: `Enabled` plus the polar
        // renderings `CurrentsMagAng` / `VoltagesMagAng` / `Residuals`. Compared
        // on the same caps the loop above just used, so a ledger `element` scope
        // that pins one of these sub-channels neutralizes it here too, and an
        // unscoped element is compared against the untouched OpenDSS cap.
        //
        // The per-element symmetrical-component surfaces `SeqCurrents` /
        // `SeqVoltages` / `SeqPowers` ride on the same flag, captured in the
        // same `derived` block and compared by `harness::compare_element_seq`
        // beside — never instead of — the polar comparator, on the same
        // (possibly ledger-rewritten) caps.
        //
        // The guard is the flag's own non-vacuity rail: under `derived` BOTH
        // transports emit `enabled` for every element (present even on the
        // disabled ones, whose polar channels the capture must skip because
        // r4133 dereferences a nil `NodeRef` there), so a channel
        // that ignored the request answers with zero `enabled` fields and the
        // case fails instead of comparing nothing.
        //
        // There are two of them, for the reason `compare_element_extras` has
        // two: the two comparators read disjoint
        // capture fields, so a channel that honoured the request for the polar
        // arrays but not the sequence ones must fail the case rather than
        // compare nothing, and the two refusals must not print the same
        // sentence. `seq_i` is empty on a disabled element and on a 0-terminal
        // one (`UPFCControl`), so the rail counts the elements whose array is
        // NON-empty — every live circuit has at least one (the `Vsource`).
        if c.compare_derived {
            capture_guard::require_capture(
                "compare_derived",
                channel_tag(channel),
                cp.elements.iter().filter(|e| e.enabled.is_some()).count(),
                &ctx,
            );
            capture_guard::require_capture(
                "compare_derived (SeqCurrents)",
                channel_tag(channel),
                cp.elements.iter().filter(|e| !e.seq_i.is_empty()).count(),
                &ctx,
            );
            // Two more, for the same reason: `compare_element_total_powers`
            // and `compare_element_cplx_seq` read capture fields disjoint from
            // each other and from the two rails above, so a channel that
            // honoured the request for the polar and magnitude arrays but not
            // for `TotalPowers` (a group-**A** read, issued at the head of the
            // element) or not for the complex pair must fail the case rather
            // than compare nothing — with its own sentence, never a shared one.
            // Both arrays are empty on a disabled element (the capture skips
            // those) and on a 0-terminal one, so each rail counts the elements
            // whose array is NON-empty; every live circuit has at least one (the
            // `Vsource`).
            capture_guard::require_capture(
                "compare_derived (TotalPowers)",
                channel_tag(channel),
                cp.elements.iter().filter(|e| !e.tp_kw.is_empty()).count(),
                &ctx,
            );
            capture_guard::require_capture(
                "compare_derived (CplxSeqCurrents)",
                channel_tag(channel),
                cp.elements
                    .iter()
                    .filter(|e| !e.cseq_i_re.is_empty())
                    .count(),
                &ctx,
            );
            // The voltage half gets its own rail too: it is the one capture
            // field of this surface that no other rail counts, and it is the
            // half whose guards differ between the channels (capi's
            // `CplxSeqVoltages` alone tests for a nil `NodeRef`, where r4133
            // tests only `Enabled`), so a transport that dropped exactly this
            // key would otherwise reach the comparator's length assert rather
            // than a sentence naming the field that went missing.
            capture_guard::require_capture(
                "compare_derived (CplxSeqVoltages)",
                channel_tag(channel),
                cp.elements
                    .iter()
                    .filter(|e| !e.cseq_v_re.is_empty())
                    .count(),
                &ctx,
            );
            // The channel travels with the capture for one reason only: the
            // `SeqPowers` "not available" sentinel is spelled per channel and is
            // folded per channel (`harness::na_seq_power`), and the
            // r4133-only truncated-matrix term rides on the same argument.
            let seq_channel = channel.props_channel();
            for ec in &cp.elements {
                // A ledger `element` scope rewrites only the sub-channels it
                // names, so both comparators see the untouched OpenDSS values on
                // every channel the scope does not select.
                let cap = el_rewrites.get(&ec.name.to_lowercase()).unwrap_or(ec);
                compare_element_derived(&snaps, cap, tol, &ctx, channels);
                // The gating population's sequence-arm census: the guard that
                // keeps r4133's misplaced positive-sequence `SeqPowers` slot
                // from costing ledger rows (`harness::assert_seq_arm_population`,
                // checked in the gate's epilogue). Recorded HERE and never
                // inside the comparator (as `record_control_census` below): the
                // comparator has other callers, the `harness::seq_floors`
                // fixtures among them, and a comparator-side census would
                // count every caller in its process. Counting here keeps it
                // gate-only by construction. The comparator hands back the
                // arm it classified (`None` for the disabled and 0-terminal
                // rows it returns on), so there is one classification, not two.
                if let Some(arm) =
                    compare_element_seq(&snaps, cap, tol, &ctx, seq_channel, channels)
                {
                    harness::record_seq_arm(arm, seq_channel);
                }
                // Beside, never instead of, the three above, on the same
                // possibly-rewritten cap: the un-`Cabs`'d complex pair, which
                // needs the channel for the r4133-only truncated-matrix band
                // term and for its 0-terminal shapes, and the per-terminal
                // `TotalPowers` sums, which need neither. Neither records a
                // census.
                compare_element_cplx_seq(&snaps, cap, tol, &ctx, seq_channel, channels);
                compare_element_total_powers(&snaps, cap, tol, &ctx, channels);
            }
        }

        // The per-element **extras**. The discrete index/name scalars
        // (`NumTerminals` / `NumConductors` / `NumPhases`, `EnergyMeter` and
        // `NodeOrder`) plus the five control-derived scalars, all discrete and
        // compared exactly: no tolerance, no `ElemChannels` selector, no ledger
        // sub-channel.
        //
        // `PhaseLosses` is numeric and goes through its own
        // comparator on the powers tier, with the `ElemChannels` selector: it
        // reaches the same cache-aware `ComputeIterminal` as Powers/Losses and
        // shares their `newton*` lane exclusion (see `harness::lane`).
        //
        // Fed from the same `el_rewrites`-or-raw caps as the two loops above so
        // the block keeps their shape, which costs nothing and hides nothing: a
        // ledger rewrite is a full `ledger::clone_element_cap` with only its
        // named value channels overwritten (`ledger::rewrite_element_selected`),
        // so the extras fields it hands back are always the untouched OpenDSS ones.
        //
        // The guard is the flag's own non-vacuity rail, and there are two of
        // them because the two comparators read disjoint capture fields. Under
        // `element_extras` BOTH transports emit the nine scalars for every
        // element, so a channel that ignored the request answers with zero
        // `n_terms` fields and the case fails instead of comparing nothing; and
        // both emit `PhaseLosses` for every element too, so a channel that sent
        // the scalars but not the array is caught by the second rail rather than
        // by a length assert on the first element. `pl_kw` is skipped when empty
        // on the wire (a 0-phase `UPFCControl` legitimately has none), so the
        // rail counts the elements whose array is NON-empty — every live
        // circuit has at least one such element (the `Vsource`).
        if c.compare_element_extras {
            capture_guard::require_capture(
                "compare_element_extras",
                channel_tag(channel),
                cp.elements.iter().filter(|e| e.n_terms.is_some()).count(),
                &ctx,
            );
            capture_guard::require_capture(
                // Its own flag spelling: the two
                // rails read disjoint capture fields, so a missing `n_terms`
                // and a missing `pl_kw` must not print the same sentence.
                "compare_element_extras (PhaseLosses)",
                channel_tag(channel),
                cp.elements.iter().filter(|e| !e.pl_kw.is_empty()).count(),
                &ctx,
            );
            // The channel travels with the capture for one reason only: the
            // "no meter" sentinel is spelled per channel and is folded per
            // channel (`harness::oracle_meter_name`).
            let ch = channel.props_channel();
            for ec in &cp.elements {
                // A ledger `element` scope rewrites only the sub-channels it
                // names (`ledger.rs::rewrite_element_selected`,
                // `SUBCHANNEL_FIELDS`). The ten discrete extras are on NO
                // sub-channel, so `compare_element_extras` always sees the
                // untouched OpenDSS values: a discrete divergence can never be
                // masked by a scope. `PhaseLosses` IS a sub-channel: a scope
                // selecting `phase_losses` overwrites `pl_kw`/`pl_kvar` with
                // this engine's values (x0.001), which is how it is
                // neutralized here.
                let cap = el_rewrites.get(&ec.name.to_lowercase()).unwrap_or(ec);
                compare_element_extras(&snaps, cap, ch, &ctx);
                compare_element_phase_losses(&snaps, cap, tol, &ctx, channels);
                // The gating population's control census (`harness::
                // assert_no_multi_control_element`, checked in the gate's
                // epilogue). Counted from the OpenDSS `NumControls`, which the
                // compare above has just held this engine to.
                harness::record_control_census(cap.num_controls, cap.ocp_dev_type);
            }
        }

        compare_discrete(dss, &cp.transformers, &cp.regcontrols, &cp.capacitors, &ctx);

        // A ledger monitor scope neutralizes only its pinned channel_idx (rewrites
        // it to the Rust samples after the envelope check); the header, sample
        // count, and every other channel still go through the standard comparator.
        for m in &cp.monitors {
            if excluded("monitor", Some(&m.name)) {
                continue;
            }
            match ledger.and_then(|v| v.monitor_rewrite(dss, m, tol, &ctx)) {
                Some(rw) => compare_monitor(dss, &rw, tol, &ctx),
                None => compare_monitor(dss, m, tol, &ctx),
            }
        }
        for m in &cp.meters {
            if !excluded("meter", Some(&m.name)) {
                compare_meter(dss, m, tol, &ctx);
            }
        }

        // The reliability surface, immediately after the
        // register/zone compare so the two meter surfaces stay adjacent - the
        // same slot both transports read it in. Only the last checkpoint carries
        // it (asserted above); `require_capture_opt` turns "flag on, channel sent
        // nothing" into a case failure, and a circuit with no enabled meter is a
        // legitimate `Some` with an empty `meters` list.
        //
        // Per-value ledger exclusions are keyed on the lowercased
        // `<meter>:<field>` pair (`em:saidi`) plus the bare `totals` for the
        // circuit-level array - the `variables` shape one level up. The counts
        // (walk length, section count, array lengths) stay unconditional.
        if c.compare_reliability && i + 1 == n_steps {
            let rel = capture_guard::require_capture_opt(
                "compare_reliability",
                channel_tag(channel),
                cp.reliability.as_ref(),
                &ctx,
            );
            compare_reliability(
                dss,
                rel,
                &rust_relcalc,
                channel.props_channel(),
                &ctx,
                tol,
                &|key: &str| excluded("reliability", Some(key)),
            );
        }

        assert_eq!(
            cp.probes.len(),
            c.probes.iter().map(|p| p.props.len()).sum::<usize>(),
            "{ctx}: oracle probe count differs from the manifest spec"
        );
        for p in &cp.probes {
            let key = format!("{}.{}", p.element.to_lowercase(), p.prop.to_lowercase());
            if !excluded("probe", Some(&key))
                && !ledger.is_some_and(|v| v.probe_handled(dss, p, tol, &ctx))
            {
                compare_probe(dss, p, tol, &ctx);
            }
        }
        assert_eq!(
            cp.variables.len(),
            c.compare_variables.len(),
            "{ctx}: oracle variables-capture count differs from the manifest spec"
        );
        for v in &cp.variables {
            // Per-variable ledger exclusions.
            // The key is the lowercased `element:variable` pair —
            // `windgen.w1:pgen` — with `:` as the separator because the element
            // name already carries the class dot; it is the `element.prop`
            // probe key above one level down. `compare_variables` still asserts
            // the variable COUNT unconditionally, so an exclusion can only ever
            // drop a value comparison, never hide a missing variable.
            let elem = v.name.to_lowercase();
            compare_variables(dss, v, tol, &ctx, &|var: &str| {
                excluded("variables", Some(&format!("{elem}:{}", var.to_lowercase())))
            });
        }
        if c.compare_eventlog {
            // A ledger eventlog `line_re` scope normalizes the diffing oracle line
            // (e.g. trailing-whitespace artifact) before the compare; unmatched
            // lines pass through unchanged.
            let masked: Vec<String> = match ledger {
                Some(v) => cp
                    .eventlog
                    .iter()
                    .map(|l| v.mask_line("eventlog", l))
                    .collect(),
                None => cp.eventlog.clone(),
            };
            // Then the two Relay label rows, which apply in **both** lanes.
            // The relay/recloser split is read
            // from the Rust circuit, which the element-name set above has
            // already pinned against the oracle.
            let relays: BTreeSet<String> = class_member_names(&snaps, "relay");
            let reclosers: BTreeSet<String> = class_member_names(&snaps, "recloser");
            let expected = lane::expected_eventlog(label, &masked, |name| {
                let n = name.to_lowercase();
                relays.contains(&n) && !reclosers.contains(&n)
            });
            compare_eventlog(dss, &expected, channel.eventlog_spec(), &ctx);
        }
        if c.compare_ctrlqueue {
            match ledger {
                Some(v) => {
                    let masked: Vec<String> = cp
                        .ctrlqueue
                        .iter()
                        .map(|l| v.mask_line("ctrlqueue", l))
                        .collect();
                    compare_ctrlqueue(dss, &masked, &ctx);
                }
                None => compare_ctrlqueue(dss, &cp.ctrlqueue, &ctx),
            }
        }

        if c.compare_global_result {
            compare_export(
                &cp.global_result,
                &rust_global_result,
                &global_result_policy(),
                &format!("{ctx} GlobalResult"),
            );
        }

        // The bus voltage surface. Placed here to
        // mirror the capi transport's capture slot — after `ctrlqueue`, before
        // the `all_properties` `?` sweep that must stay last
        // (`tools/oracle/oracle_server.py`). Every bus read is class C
        // (order-free): both OpenDSS engines read `Solution.NodeV` directly and
        // move only `ActiveBusIndex`, so nothing here can stale a cached
        // `Iterminal`.
        //
        // `voltages_excluded` is the one structural rule this surface needs: the
        // bus bands are exact images of the node-voltage band over the SAME
        // `Solution.NodeV` (`harness::compare_bus`), so on a case whose
        // `voltages` field is already ledger-excluded DECK-WIDE the bus arrays
        // would re-raise a divergence that is already triaged and pinned — ten
        // new ledger rows for one cause. It suppresses only the three continuous
        // arrays; the bus count, the name sequence, `nodes`, `kv_base` and every
        // array length stay compared on those cases too. A `voltages` scope that
        // names a node subset (`node_re`) suppresses NOTHING here — it would be
        // far wider than its cause; see `LedgerView::bus_arrays_suppressed`.
        if c.compare_bus {
            capture_guard::require_capture(
                "compare_bus",
                channel_tag(channel),
                cp.buses.len(),
                &ctx,
            );
            let v_excluded = ledger.is_some_and(|v| v.bus_arrays_suppressed(i));
            harness::compare_bus(dss, &cp.buses, tol, v_excluded, &ctx);
            harness::compare_all_bus_vmag_pu(dss, &cp.all_bus_vmag_pu, tol, v_excluded, &ctx);

            // The DISTANCE half of the SAME per-bus walk: `Bus.Distance` plus
            // the two circuit-level views of the very same field,
            // `Circuit.AllBusDistances` and `Circuit.AllNodeDistances`. Like the two blocks below it rides
            // `compare_bus` — no flag of its own — and is class C (order-free):
            // all three arms return the stored bus distance from the meter and
            // move only `ActiveBusIndex`.
            //
            // `v_excluded` is deliberately NOT passed: the distance is a
            // zone-build output, not an image of `Solution.NodeV`, so a
            // `voltages` ledger cause cannot explain a divergence here and
            // suppressing it would hide a zone-build bug behind an unrelated
            // triage. Compared exactly (`rel = abs = 0`).
            //
            // What a real OpenDSS divergence gets instead is its own ledger
            // field, `distance`, selected per BUS. The closure is passed rather
            // than a precomputed flag because `excluded()` records the scope's
            // hit when it answers, and `compare_bus_distances` asks it only once
            // the exact equality has already failed, so a hit means "masked a
            // real divergence" and an OpenDSS fix leaves the scope STALE.
            //
            // The returned non-zero count feeds the run-wide fail-on-stale
            // population (`harness::assert_distance_compare_ran`, called once
            // from the gate epilogue). This is the ONE recording call site — the
            // harness' own drives must not record, or the population stops being
            // a property of the corpus.
            let nonzero_distances = harness::compare_bus_distances(
                dss,
                &cp.buses,
                &cp.all_bus_distances,
                &cp.all_node_distances,
                &|bus: &str| excluded("distance", Some(bus)),
                &ctx,
            );
            harness::record_distance_compare(nonzero_distances);

            // The SEQUENCE + LINE-TO-LINE half of the SAME per-bus walk:
            // `Bus.SeqVoltages`/`CplxSeqVoltages`/`VLL`/`puVLL`, four arms
            // appended to the one
            // `SetActiveBus` sweep the block above already paid for — hence a
            // second call on `cp.buses` rather than a second capture or a
            // second flag (the surface rides `compare_bus`, as
            // `manifest::Case::compare_bus` documents). Class C too: every arm
            // reads `Solution.NodeV` and the bus object only
            // (`CAPI/CAPI_Alt.pas:2190`/`:2532` == r4133 `DDLL/DBus.pas:305`/
            // `:588`), so nothing here can stale a cached `Iterminal`.
            //
            // It runs AFTER `compare_bus`, which pins the bus count, the name
            // sequence and the node sets first: this comparator classifies each
            // bus from its NODE SET and would otherwise be able to read a
            // divergent structure as a divergent class.
            //
            // `v_excluded` is reused NARROWED, exactly as the short-circuit arm
            // below reuses it: it drops the sequence/L-L VALUES only, while
            // every availability rule (each channel's own sentinel), every
            // length, the `vll_declined` cross-check and this engine's internal
            // identities stay compared.
            //
            // The returned class counts feed the run-wide fail-on-stale
            // populations (`harness::assert_seq_vll_populations`, called once
            // from the gate epilogue). This is the ONE recording call site —
            // the harness' own drives must not record, or the populations stop
            // being a property of the corpus.
            let seq_vll = harness::compare_bus_seq_and_vll(
                dss,
                &cp.buses,
                tol,
                channel.props_channel(),
                v_excluded,
                &ctx,
            );
            harness::record_seq_vll_populations(seq_vll);

            // The short-circuit half of the SAME per-bus walk:
            // `Bus.Zsc1`/`Zsc0`/`ZscMatrix`/`YscMatrix`/`Isc`/`Voc`, six arms
            // appended to the one
            // `SetActiveBus` sweep the block above already paid for. That is
            // why `compare_zsc` implies `compare_bus` (asserted in
            // `engines::build_run_request`, where the two flags meet) and why
            // it is nested here instead of opening a second `if`. Class C
            // too: every arm reads `Zsc`/`Ysc`/`VBus`/`BusCurrent` off the bus
            // object and moves only `ActiveBusIndex` (`CAPI/CAPI_Alt.pas:2202-2365`
            // == r4133 `DDLL/DBus.pas:351-518`), so nothing here can stale a
            // cached `Iterminal`.
            //
            // `v_excluded` is reused NARROWED: `compare_bus_short_circuit`
            // suppresses only the `Voc`/`Isc` values (a snapshot of the triaged
            // `Solution.NodeV` and its `Ysc*Voc` image) while `Zsc`/`Ysc`/
            // `Zsc1`/`Zsc0` — functions of `Y` alone — stay compared, along
            // with the study bit, the bus identity and every length. The
            // per-bus `voc` field masks one bus's `Voc` alone, asked only once
            // that bus has left its band.
            if c.compare_zsc {
                capture_guard::require_capture(
                    "compare_zsc",
                    channel_tag(channel),
                    cp.buses.len(),
                    &ctx,
                );
                // The count of buses whose full `n x n` matrices were really
                // value-compared feeds the gate epilogue's fail-on-stale
                // (`harness::assert_sc_study_compare_ran`): `port_ran ==
                // oracle_ran` is also true when
                // NEITHER ran, so without this the whole non-trivial half of
                // the surface could go quiet and stay green. This is the one
                // call site that records — the harness' own drives must not.
                let full = harness::compare_bus_short_circuit_masking_voc(
                    dss,
                    &cp.buses,
                    tol,
                    channel.props_channel(),
                    v_excluded,
                    &|bus: &str| excluded("voc", Some(bus)),
                    &ctx,
                );
                harness::record_sc_study_compare(full);
            }

            // The AT-BUS half of the SAME per-bus walk: `Bus.AllPCEatBus` /
            // `Bus.AllPDEatBus`, two arms appended to the one `SetActiveBus`
            // sweep — so, like the three blocks above, it rides `compare_bus`
            // and opens no flag of its own.
            //
            // It runs LAST because that is the order both transports read the
            // pair in, and they read it last because on the r4133 channel it is
            // the bus walk's only `ModeEffect::Impure` read: `getP*atBus`
            // drives `DSS_Class.First`/`Next` and `TDSSClass.Get_First`/
            // `Get_Next` assign `ActiveCircuit.ActiveCktElement`. It moves
            // neither `ActiveBusIndex` nor any `Iterminal` cache, so the surface
            // is still capture-group C (order-free), `crates/dss-epri/src/modes.rs`.
            //
            // `v_excluded` is deliberately NOT passed and no `Tolerances`
            // either: the wire carries element NAMES, not an image of
            // `Solution.NodeV`, so a triaged voltage cause can never explain a
            // divergence here and there is nothing to band.
            //
            // The two OpenDSS engines run two DIFFERENT walks and each
            // contradicts its own stated intent, so neither is compared against
            // this engine's answer directly: each channel's walk is asserted
            // POSITIVELY over this engine's own attachment facts
            // (`oracle == walk(port state)`). Nothing is excluded, no ledger
            // entry is involved, and the measured disagreements are COUNTED into
            // four run-wide fail-on-stale populations
            // (`harness::assert_at_bus_populations`, called once from the gate
            // epilogue). This is the ONE recording call site — the harness' own
            // drives must not record, or the populations stop being a property
            // of the corpus.
            let at_bus = harness::compare_bus_at_bus(dss, &cp.buses, channel.props_channel(), &ctx);
            harness::record_at_bus_populations(at_bus);
        }

        if c.compare_all_properties {
            capture_guard::require_capture(
                "compare_all_properties",
                channel_tag(channel),
                cp.all_properties.len(),
                &ctx,
            );
            // A ledger `property` scope pins one (element, prop) pair — an exact
            // `oracle` pin, or `num_rel` for numeric-skeleton values (same
            // contract as `probe`). To keep the monolithic `compare_all_properties`
            // count/order contract intact while excluding that one pair from the
            // value compare, rewrite its OpenDSS value to the Rust `?`-surface value
            // (the ledger already asserted the Rust value against the pin/envelope),
            // so the standard compare treats it as equal.
            let prop_keys = ledger
                .map(|v| v.property_handled_keys(dss, &cp.all_properties, tol, &ctx))
                .unwrap_or_default();
            if prop_keys.is_empty() {
                compare_all_properties(dss, &cp.all_properties, tol, channel.props_channel(), &ctx);
            } else {
                let rewritten: Vec<harness::PropsCap> = cp
                    .all_properties
                    .iter()
                    .map(|pc| {
                        let el = pc.element.to_lowercase();
                        let props = pc
                            .props
                            .iter()
                            .map(|(name, val)| {
                                if prop_keys.contains(&(el.clone(), name.to_lowercase())) {
                                    dss.command(&format!("? {}.{}", pc.element, name));
                                    (name.clone(), dss.result().to_string())
                                } else {
                                    (name.clone(), val.clone())
                                }
                            })
                            .collect();
                        harness::PropsCap {
                            element: pc.element.clone(),
                            props,
                        }
                    })
                    .collect();
                compare_all_properties(dss, &rewritten, tol, channel.props_channel(), &ctx);
            }
        }

        // The PDElements interface walk. Late in the step, next to the other
        // whole-model surfaces (the topology arm is the one that must run after
        // it): `Dss::pd_elements` is a
        // `&self` read over `Circuit.pd_elements` that depends on no active
        // element and no solve state, so its position among the comparators is
        // free — unlike the CAPTURE order, which is fixed on both transports
        // (after the meters, before the probes) because the OpenDSS engines' own
        // `ParentPDElement` read hijacks `ActiveCktElement`.
        //
        // `require_capture_opt`, not `require_capture`: many live cases hold
        // no PD element at all, so an EMPTY walk is a legitimate answer the
        // comparator must still match (`[]` against a non-empty walk of this
        // engine fails on the length assert). What must never
        // pass is an ABSENT field — a channel that ignored the request — and
        // the global collapse to zero everywhere, which
        // `harness::assert_pd_elements_compare_ran` catches in the epilogue.
        if c.compare_pdelements {
            let pde = capture_guard::require_capture_opt(
                "compare_pdelements",
                channel_tag(channel),
                cp.pd_elements.as_deref(),
                &ctx,
            );
            compare_pd_elements(dss, pde, channel.props_channel(), &ctx);
        }

        // The `ITopology` interface, compared
        // LAST in the step on purpose, mirroring both transports' capture order
        // (`oracle_server.py::run_case`, `dss-epri::capture::run`): building the
        // topology tree stamps `Checked`/`IsIsolated`/`BusChecked` on every
        // element, OpenDSS on its circuit and `Dss::topology_view` on ours, so
        // no other comparison of
        // this step may run after it. The surface is flag-gated, so the
        // `capture_guard` rail applies: the flag being ON with nothing captured
        // FAILS the case instead of comparing nothing.
        if c.compare_topology {
            let topo = capture_guard::require_capture_opt(
                "compare_topology",
                channel_tag(channel),
                cp.topology.as_ref(),
                &ctx,
            );
            // `topo_step0` carries this engine's OWN step-0 isolation topology
            // across the steps of this case+channel run: the topology
            // comparator's rebase rule is defined against it, and the engine is re-run per
            // channel, so the reference is per run and never shared.
            harness::topology::compare_topology(dss, topo, &ctx, label, i, &mut topo_step0);
        }

        // The flat incidence surface, compared
        // AFTER the topology block and therefore last of the whole step, exactly
        // as both transports capture it (`oracle_server.py::run_case`,
        // `dss-epri::capture::run`; `crates/dss-core/tests/capture_order.rs`
        // asserts the source order on both). Two independent reasons, neither of
        // them cosmetic: `Dss::inc_matrix_view` REBUILDS solution state
        // (`IncMat`, `Laplacian`, `Inc_Mat_Rows`, `IncMat_Ordered`) and on r4133
        // the same pair also moves `ActiveCktElement`; and it must FOLLOW the
        // topology read, whose memoized `Branch_List` is what the topology
        // surface's two decline censuses
        // are defined on. Flag-gated, so the `capture_guard` rail applies: the
        // flag ON with nothing captured FAILS the case instead of comparing
        // nothing.
        if c.compare_inc_matrix {
            let inc = capture_guard::require_capture_opt(
                "compare_inc_matrix",
                channel_tag(channel),
                cp.inc_matrix.as_ref(),
                &ctx,
            );
            harness::inc_matrix::compare_inc_matrix(dss, inc, channel_tag(channel), &ctx, label, i);
        }
    }

    if c.compare_autoadd_log {
        let oracle_log = capture_guard::require_capture_opt(
            "compare_autoadd_log",
            channel_tag(channel),
            oc.autoadd_log.as_deref(),
            label,
        );
        let case_name = dss
            .circuit()
            .expect("circuit exists after AutoAdd")
            .case_name
            .clone();
        let dir = Path::new(case_path)
            .parent()
            .expect("case_path has a parent dir");
        let log_path = dir.join(format!("{case_name}_AutoAddLog.csv"));
        let rust_log = std::fs::read_to_string(&log_path).unwrap_or_else(|e| {
            panic!("{label}: read Rust AutoAddLog {}: {e}", log_path.display())
        });
        compare_export(
            oracle_log,
            &rust_log,
            &autoadd_log_policy(tol),
            &format!("{label} AutoAddLog"),
        );
    }
}

/// The leak rail, checked BEFORE anything else a case does with the filesystem.
///
/// `swept` is the transport's own `sweep_failed` report: what its `CorpusGuard`
/// classified as run-created and then could NOT remove, i.e. an engine leak. On
/// a per-producer copy it poisons nothing, and for the two OpenDSS channels
/// this report is THE leak rail (their process has exited before the gate
/// removes the copy). dss_capi never closes a Storage `debugtrace` stream, so a
/// `STOR_<name>.csv` is the leak this rail catches.
///
/// `None` is a MISSING report, not a clean one: if the field defaulted to `[]`,
/// a transport that stopped emitting the key would disarm this rail in
/// silence. Both transports emit it unconditionally
/// (`tools/oracle/oracle_server.py::run_case`, `dss-epri::capture::run_case`),
/// so its absence is a broken transport and fails the case here.
fn assert_swept_clean(label: &str, producer: &str, swept: Option<&[String]>) {
    let Some(swept) = swept else {
        panic!(
            "{label}: the `{producer}` producer sent no `sweep_failed` report. \
             Every transport reports it on every case (it is the leak \
             rail); a reply without the key can never be read as \"the sweep \
             was clean\".",
        );
    };
    assert!(
        swept.is_empty(),
        "{label}: the `{producer}` producer left {} leaked dropping(s) in its \
         scratch copy that its corpus guard classified as run-created and could \
         not remove: {}. The engine still holds the file open (the dss_capi \
         Storage `DebugTrace` class); on the shared tree it poisoned the next \
         producer's snapshot, and on a copy it is still an engine leak. Fix the \
         producer - release the circuit (`clear`) before the guard sweeps, or \
         close the file - never widen the surface around it.",
        swept.len(),
        swept.join(", "),
    );
}

/// The leak rail fires on a leaked dropping, and names the producer.
#[test]
#[should_panic(expected = "left 1 leaked dropping(s)")]
fn a_transport_reporting_a_leaked_dropping_fails_the_case() {
    let reply = serde_json::json!({
        "node_order": ["a.1"],
        "n_steps": 0,
        "checkpoints": [],
        "run_files": ["stor_s1.csv"],
        "sweep_failed": ["stor_s1.csv"],
    });
    let cr: CaseResult = serde_json::from_value(reply).expect("a transport reply deserializes");
    assert_swept_clean("fixture:leak", "capi_v0145", cr.sweep_failed.as_deref());
}

/// …and a reply that simply OMITS the key is a broken transport, not a clean
/// sweep — the hole `serde(default)` on a plain `Vec` used to leave open.
#[test]
#[should_panic(expected = "sent no `sweep_failed` report")]
fn a_transport_reply_without_a_sweep_report_fails_the_case() {
    let reply = serde_json::json!({
        "node_order": ["a.1"],
        "n_steps": 0,
        "checkpoints": [],
        "run_files": ["stor_s1.csv"],
    });
    let cr: CaseResult = serde_json::from_value(reply).expect("a transport reply deserializes");
    assert_swept_clean("fixture:missing", "r4133", cr.sweep_failed.as_deref());
}

/// Assert the OpenDSS step counts, run the Rust engine once, compare against the
/// given `channel`'s capture (its iteration + eventlog-mask policy). No guard,
/// no OpenDSS fetch: the caller (scheduler or [`run_and_compare`]) owns those.
#[allow(clippy::too_many_arguments)]
pub(crate) fn compare_with_result(
    oc: &CaseResult,
    label: &str,
    case_path: &str,
    c: &SolvableCase,
    channel: EngineChannel,
    ledger: Option<&crate::ledger::LedgerView>,
) {
    assert_eq!(oc.n_steps, c.n_steps, "{label}: oracle step count");
    assert_eq!(
        oc.checkpoints.len(),
        c.n_steps,
        "{label}: oracle checkpoint count"
    );
    // The channel's own teardown `clear` raised. Surfaced FIRST (before the
    // leak assert below,
    // whose panic would otherwise hide it) and deliberately NOT a case failure:
    // the teardown runs after every read of the run, so the compared surface is
    // already captured, and the transport that raised exits after replying so a
    // pooled worker is respawned instead of reused
    // (`tools/oracle/oracle_server.py::main`). The pinned dss_capi 0.14.5
    // raises this way on the `modes:autoadd` decks (`DSSException (#303) ...
    // clear ... Access violation`), an OpenDSS fault after the capture and not
    // a divergence of this engine, so it gets a note here, never a ledger row.
    if let Some(err) = oc.teardown_error.as_deref() {
        // One line: the pinned dss_capi folds a CRLF-formatted multi-line
        // description into the message, and a per-case note must stay
        // greppable in a 526-case log.
        let err = err.split_whitespace().collect::<Vec<_>>().join(" ");
        eprintln!(
            "{label}: the `{}` producer's teardown `clear` raised AFTER the \
             capture — the surface is compared as usual and the worker is \
             recycled: {err}",
            channel_tag(channel),
        );
    }
    // The leak rail, checked BEFORE anything else this case does with the
    // filesystem. `sweep_failed` is what the channel's own `CorpusGuard`
    // classified as run-created and then could not remove: an engine leak.
    // Every producer runs its own copy, so nothing is poisoned, but the engine
    // still holds the file (dss_capi never closes a Storage `debugtrace`
    // stream). Both transports release the circuit before their guard sweeps,
    // any survivor fails the case here, and for the OpenDSS channels this
    // report is the leak rail.
    assert_swept_clean(label, channel_tag(channel), oc.sweep_failed.as_deref());
    let tol = tol_for(&c.kind);
    // This engine compiles its OWN fresh scratch copy of the case, never the
    // vendored deck and never the copy OpenDSS ran in, so `deck` is what the
    // engine and the probe see, while `case_path` (the vendored deck) keeps
    // keying the OpenDSS sidecars below. The copy is
    // removed at the end, or the case fails naming the `port` producer.
    let copy = crate::scratch::ScratchCopy::new(case_path, crate::scratch::PORT);
    let deck = copy.deck();
    // This engine's corpus guard brackets the copy (snapshot,
    // `classify_created`, sweep, restore, and the leak report on stderr) and
    // is dropped before `copy.finish()`, whose bounded removal
    // then fails the case naming the `port` producer if anything survived.
    let port_guard = CorpusGuard::new(deck);
    // The run-produced FILE SET. This engine needs its OWN before/after
    // bracket (and must sweep what it classified) because the scheduler runs the
    // Rust engine once PER CHANNEL, each run in its own scratch copy, so the
    // "before" snapshot is the copy's initial listing and no other producer's
    // file can be in it.
    //
    // The probe is also this engine's producer of the demand-interval tree's
    // CONTENTS, and most DI cases are `kind = large` (so not forced for run
    // files), so it exists whenever EITHER surface is armed. The two
    // COMPARISONS stay independently gated below: arming the file-set
    // comparison on a `large` deck is a decision of its own. On
    // `EPRITestCircuits/ckt5` the deck exports a monitor that does not exist,
    // capi raises and drops its remaining exports while r4133 and this engine
    // carry on, so the created-file SET differs by one member there.
    #[cfg(windows)]
    let run_file_probe = (c.compare_run_files || c.compare_di)
        .then(|| harness::run_files::RunFileProbe::start(deck));
    // The classification itself is portable (`dss_epri::guard` is ungated, and
    // `CorpusGuard::sweep_created` above runs it on every platform), but the
    // SURFACE is compared only against the gate's two OpenDSS
    // channels and the `r4133` transport is a Win64 DLL, so the comparator
    // `harness::run_files` stays declared under `#[cfg(windows)]`. Refuse the
    // flag loudly elsewhere rather than compare nothing (that module's
    // §Platform doc).
    #[cfg(not(windows))]
    assert!(
        !c.compare_run_files && !c.compare_di,
        "{label}: `compare_run_files` / `compare_di` are Windows-only — their \
         `r4133` channel is a Win64 DLL, so the comparators \
         `harness::run_files` and `harness::di` are declared under \
         `#[cfg(windows)]`"
    );
    let (mut dss, baseline) = run_rust_capture(label, deck, c);
    compare_capture(
        &mut dss, baseline, oc, label, deck, c, &tol, channel, ledger,
    );
    // This engine's run is over: drop the engine BEFORE the probe reads, so a file
    // the engine still holds open is closed (and flushed) first — a removal that
    // failed on an open handle would leave the file in this copy and fail the
    // case when the copy is removed.
    drop(dss);
    #[cfg(windows)]
    if let Some(probe) = run_file_probe {
        // The demand-interval tree's contents, read BEFORE the sweep
        // (`finish_and_clean` consumes the probe, so the borrow checker states
        // that half of the read-slot rule;
        // `capture_order::check_run_tail_order` states the two OpenDSS
        // transports' half).
        let port_di = c.compare_di.then(|| probe.di_tree(label));
        let port_files = probe.finish_and_clean(label, c.compare_run_files);
        // Field-by-field ledger partition, per created NAME (the `run_files`
        // field's `name_re` scopes); the call marks the scope hit, which is what
        // keeps `ledger.json` fail-on-stale honest. Run-level surface, so step 0
        // is the only step a scope can select.
        if c.compare_run_files {
            let excluded =
                |name: &str| ledger.is_some_and(|v| v.excluded("run_files", Some(name), 0));
            harness::run_files::compare_run_files(
                channel_tag(channel),
                oc.run_files.as_deref(),
                &port_files.created,
                &excluded,
                label,
            );
            // The CONTENTS of the members the gate selected. The OpenDSS bytes
            // came back through the case's sidecar directory; read them (which
            // also deletes the directory) and pair them with this engine's, read
            // in its own scratch copy by the probe above before its sweep. The same
            // `excluded` closure partitions both surfaces: a name whose PRESENCE is
            // triaged has no contents to compare either.
            let oracle_contents = harness::run_files::read_sidecar(
                &run_file_contents_dir(case_path),
                oc.run_file_contents.as_deref(),
                channel_tag(channel),
                label,
            );
            let matched = harness::run_files::compare_run_file_contents(
                channel_tag(channel),
                oracle_contents.as_ref(),
                &port_files.contents,
                &excluded,
                label,
            );
            // The per-report-kind CELL comparison: each kind's column map
            // (quantity class + print format per column) under the
            // `case floor + print ulp` rule, on exactly the pairs the structural
            // rails (presence, the sidecar/reply agreement, the two sides
            // selecting the same files) just certified. `tol` is this case's own
            // calibrated floor, the same one
            // the in-memory comparators above used on the very numbers these files
            // re-print.
            // The returned `CellTally` (and the process-wide
            // `harness::run_file_contents::trace_tail_census`) carry the Storage
            // `DebugTrace` read-back tail this (case, channel) measured — the
            // OpenDSS transport's own post-solve element reads minus this
            // engine's single recompute. The
            // scheduler epilogue, not this call, holds it against the re-derived
            // `TRACE_READBACK_RECORDS` fail-on-stale in both directions.
            let tally = harness::run_file_contents::compare_run_file_cells(
                channel_tag(channel),
                &matched,
                &tol,
                label,
            );
            // The census is recorded HERE and never inside that comparator
            // (as `record_seq_arm` above): the
            // comparator has other callers, the `harness::run_file_contents`
            // fixtures among them, and a comparator-side census would count
            // every caller in its process. Recorded for EVERY gated (case,
            // channel) — `files = 0` included — so that "the
            // surface was requested and compared nothing" is visible rather than
            // absent (`scheduler::assert_run_file_contents_census_is_the_pinned_
            // population`, the gate's epilogue).
            crate::scheduler::record_run_file_contents(label, channel_tag(channel), tally);
        }
        if let Some(port_di) = port_di {
            let tag = channel_tag(channel);
            // The OpenDSS files live in this (case, channel)'s sidecar: the
            // channel's own guard swept the originals away before this engine ever
            // ran (`corpus_gate::engines::di_sidecar_dir`).
            let oracle_di = oc
                .di
                .as_ref()
                .map(|m| read_di_sidecar(m, case_path, tag, label));
            // Per `<file>:<column>` ledger partition (the `di` field's `name_re`
            // scopes); run-level surface, so step 0 is the only selectable step.
            let excluded = |key: &str| ledger.is_some_and(|v| v.excluded("di", Some(key), 0));
            harness::di::compare_di(tag, oracle_di.as_ref(), &port_di, &tol, &excluded, label);
            // Compared and equal: drop the copies. A case that FAILED keeps its
            // sidecar for triage; the next run of the same case empties it
            // (`engines::prepare_di_sidecar`).
            crate::engines::remove_di_sidecar(case_path, tag);
        }
    }
    drop(port_guard);
    copy.finish();
}

/// Read one OpenDSS channel's demand-interval capture back off its sidecar directory.
///
/// The transports cannot answer with the file TEXT: the tree is run-created, so
/// each channel's guard sweeps it away in the channel's scratch copy before
/// this engine re-runs the same case in its own copy and this gate compares.
/// They therefore COPY the selected members into
/// `<target>/corpus_gate/di/<case key>/<channel>/`
/// (`engines::di_sidecar_dir`) and reply with
/// `normalized member name -> path relative to it`.
///
/// Every failure here is LOUD, never a skip: a member the transport claimed and
/// did not leave behind, or one that is not UTF-8, would otherwise turn into a
/// silently smaller comparison. A DI file is CSV text written by the engines'
/// own formatters on all three producers.
#[cfg(windows)]
fn read_di_sidecar(
    members: &BTreeMap<String, String>,
    case_path: &str,
    channel: &str,
    label: &str,
) -> harness::di::DiTree {
    let dir = crate::engines::di_sidecar_dir(case_path, channel);
    members
        .iter()
        .map(|(name, rel)| {
            let path = sidecar_member_path(&dir, name, rel, channel, label);
            let bytes = std::fs::read(&path).unwrap_or_else(|e| {
                panic!(
                    "{label}: the `{channel}` transport reported the demand-interval \
                     file {name} and its sidecar copy {path:?} cannot be read: \
                     {e}. The copy is made inside the transport's own guarded \
                     bracket (`engines::prepare_di_sidecar`), so a missing \
                     member is a transport bug, never a reason to compare less."
                )
            });
            let text = String::from_utf8(bytes).unwrap_or_else(|e| {
                panic!(
                    "{label}: the `{channel}` demand-interval file {name} ({path:?}) \
                     is not valid UTF-8 ({e}). The DI files are CSV text on \
                     all three producers; a non-UTF-8 byte is a writer bug, \
                     never a reason to compare the file loosely."
                )
            });
            (name.clone(), text)
        })
        .collect()
}

/// Where one member of a transport's demand-interval reply is read from —
/// `dir` joined with the transport's own relative name, refused unless that
/// name stays inside `dir`.
///
/// `Path::starts_with` is a COMPONENT-prefix test and normalizes nothing, so
/// `dir.join("../evil")` passes it while `fs::read` then leaves the sidecar
/// on a name like that. The containment is therefore
/// asserted on the relative name's own components first: every one must be a
/// plain segment — no `..`, no root, no drive prefix. Both transports build
/// these names with `dss_epri::guard::normalize_created_name` over a walk
/// rooted at the case directory, so anything else is a transport bug and fails
/// the case here instead of reading an arbitrary file as if it were a DI
/// report.
#[cfg(windows)]
fn sidecar_member_path(dir: &Path, name: &str, rel: &str, channel: &str, label: &str) -> PathBuf {
    assert!(
        Path::new(rel)
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_))),
        "{label}: the `{channel}` transport spelled the demand-interval file \
         {name} as {rel:?}, which is not a plain path inside its own sidecar \
         directory {dir:?} — a `..`, a root or a drive prefix would read \
         outside it"
    );
    let path = dir.join(rel);
    assert!(
        path.starts_with(dir),
        "{label}: the `{channel}` transport placed the demand-interval file \
         {name} at {rel:?}, which is outside its own sidecar directory {dir:?}"
    );
    path
}

/// One-shot convenience for the opt-in report tests: snapshot the case dir,
/// fetch the pinned dss_capi model once, compare against the `capi_v0145`
/// channel, without the ledger.
pub(crate) fn run_and_compare(oracle: &Oracle, label: &str, case_path: &str, c: &SolvableCase) {
    // Each producer runs in its own scratch copy: the OpenDSS one is made and
    // removed inside `run_case`, this engine's inside the compare.
    let oc = oracle.run_case(case_path, c);
    compare_with_result(&oc, label, case_path, c, EngineChannel::CapiV0145, None);
}

// ---------------------------------------------------------------------------
// Abort + pending contracts (abort over a Channel).
// ---------------------------------------------------------------------------

/// Gate a deck that BOTH engines abort at solve. OpenDSS *raises* at solve
/// (so there is no solved state to line up); assert OPENDSS aborts with the
/// expected message and the RUST engine sets `solution_abort` + surfaces it.
pub(crate) fn run_and_compare_abort(
    channel: &Channel,
    label: &str,
    case_path: &str,
    c: &SolvableCase,
) {
    let expected = c
        .expect_solve_abort
        .as_deref()
        .expect("abort case has expect_solve_abort");

    // The abort request drives `Compile` (which executes the deck's own `Solve`);
    // OpenDSS raises on the aborting solve → `ok:false` carrying the message.
    // OpenDSS and this engine each abort in their own fresh copy.
    let copy = crate::scratch::ScratchCopy::new(case_path, channel.tag());
    // Hand-built, so it refuses a vendored deck itself, as `build_run_request`
    // does for every other request (the transports' guards bracket its folder).
    let deck = crate::scratch::not_vendored(
        std::path::Path::new(copy.deck()),
        "runner::run_and_compare_abort",
    );
    let req = json!({
        "cmd": "run",
        "case_path": deck.to_string_lossy(),
        "post": c.post,
        "n_steps": c.n_steps,
        "selected_elements": c.selected_elements,
        "full_csc": true,
        "check_meters_monitors": false,
        "probes": [],
        "variables": [],
        "eventlog": false,
        "ctrlqueue": false,
    });
    let resp = channel.call(&req);
    copy.finish();
    assert!(
        !resp.ok,
        "{label}: oracle did NOT abort the solve (expected an abort containing {expected:?})"
    );
    let oracle_err = resp.error.unwrap_or_default();
    assert!(
        oracle_err.contains(expected),
        "{label}: oracle abort message {oracle_err:?} does not contain {expected:?}"
    );

    crate::scratch::in_copy(case_path, crate::scratch::PORT, |deck| {
        let _port_guard = CorpusGuard::new(deck);
        let mut dss = Dss::new();
        dss.command("clear");
        dss.command(&format!("compile \"{deck}\""));
        // The compile's declared messages, checked before `post` adds the abort.
        if !c.expect_warnings.is_empty() {
            assert_expected_warnings(&dss, &c.expect_warnings, &format!("{label}: after compile"));
        }
        for cmd in &c.post {
            dss.command(cmd);
        }
        assert!(
            dss.circuit().is_some_and(|ckt| ckt.solution.solution_abort),
            "{label}: Rust engine did NOT set solution_abort — the malformed input must abort \
             the solve like the oracle (message: {expected:?})"
        );
        assert!(
            dss.errors().iter().any(|e| e.contains(expected)),
            "{label}: Rust engine did not surface {expected:?}: {:?}",
            dss.errors()
        );
    });
}

/// A `defer_ledger` case is parked from the live OpenDSS comparison (it
/// reproduces on neither channel and has no ledger entry yet). It is still
/// **smoke-run** on the Rust engine: compile + post + solve every step must
/// converge with NO new engine errors. This catches a *convergence or
/// error-surfacing* regression (a deferred case that stops solving, NaNs out,
/// or starts erroring); it does **not** catch a *numeric-correctness*
/// regression that still converges, since no physical value is compared
/// against any reference here. Membership is preserved, but it is a reduction
/// in verification depth until the case has a ledger entry.
pub(crate) fn assert_deferred_rust_smoke(label: &str, case_path: &str, c: &SolvableCase) {
    crate::scratch::in_copy(case_path, crate::scratch::PORT, |deck| {
        deferred_smoke_in(label, deck, c)
    });
}

/// [`assert_deferred_rust_smoke`] on the port's scratch copy `deck`.
fn deferred_smoke_in(label: &str, deck: &str, c: &SolvableCase) {
    let _port_guard = CorpusGuard::new(deck);
    let (mut dss, baseline_errors) = run_rust_capture(label, deck, c);
    for i in 0..c.n_steps.max(1) {
        dss.command("solve");
        assert_eq!(
            dss.errors().len(),
            baseline_errors,
            "{label} (deferred smoke) step {i}: new Rust engine errors: {:?}",
            &dss.errors()[baseline_errors.min(dss.errors().len())..]
        );
        assert!(
            dss.circuit().is_some_and(|ckt| ckt.is_solved),
            "{label} (deferred smoke) step {i}: Rust did not converge — a deferred case \
             must still SOLVE on the Rust engine (defer_ledger parks the OpenDSS compare, \
             not the Rust smoke)"
        );
    }
}

/// Pending discipline: the unported feature must surface as an
/// engine error (a clean run means a silent fallback masks the gap).
pub(crate) fn assert_pending_errors_loudly(label: &str, case_path: &str, c: &SolvableCase) {
    crate::scratch::in_copy(case_path, crate::scratch::PORT, |deck| {
        pending_errors_in(label, deck, c)
    });
}

/// [`assert_pending_errors_loudly`] on the port's scratch copy `deck`.
fn pending_errors_in(label: &str, deck: &str, c: &SolvableCase) {
    let _port_guard = CorpusGuard::new(deck);
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile \"{deck}\""));
    for cmd in &c.post {
        dss.command(cmd);
    }
    assert!(
        !dss.errors().is_empty(),
        "{label}: pending case (wp {}) ran with NO engine error — the unported \
         feature fell back silently; if it is now ported, flip `pending: false` \
         and prove the live compare green",
        c.wp.as_deref().unwrap_or("?"),
    );
}

// ---------------------------------------------------------------------------
// Export policies (unchanged) + panic-message helper.
// ---------------------------------------------------------------------------

/// [`ExportPolicy`] for the AutoAdd `GlobalResult` line: bus name exact, GENADD
/// improvement figure on a measured faer-vs-KLU floor (see the original WPG.5
/// note in the pre-Phase-B `corpus_live.rs`).
fn global_result_policy() -> ExportPolicy {
    ExportPolicy {
        sep: ',',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 1e-10,
        abs: 1e-12,
        col_tol: Vec::new(),
    }
}

/// [`ExportPolicy`] for the `AutoAddLog.csv`: fixed header row then comma-
/// tokenized per-bus rows on the `micro` energy floor.
fn autoadd_log_policy(tol: &Tolerances) -> ExportPolicy {
    ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: tol.energy_rel,
        abs: tol.energy_abs,
        col_tol: Vec::new(),
    }
}

/// Extract a readable message from a `catch_unwind` payload.
pub(crate) fn panic_msg(e: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = e.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// [`sidecar_member_path`] refuses a transport-supplied name that leaves
    /// the sidecar directory — the containment `Path::starts_with` alone does
    /// NOT give.
    #[test]
    fn a_di_member_name_that_escapes_its_sidecar_is_refused() {
        let dir = Path::new(r"C:\gate\di\case\r4133");
        let ok = sidecar_member_path(
            dir,
            "di_yr_0/totals_1.csv",
            "di_yr_0/totals_1.csv",
            "r4133",
            "unit:di",
        );
        assert_eq!(ok, dir.join("di_yr_0").join("totals_1.csv"));

        for escape in [
            "../evil.csv",
            "di_yr_0/../../evil.csv",
            r"C:\evil.csv",
            r"\evil.csv",
        ] {
            let msg = std::panic::catch_unwind(|| {
                sidecar_member_path(dir, "di_yr_0/totals_1.csv", escape, "r4133", "unit:di")
            })
            .map(|p| panic!("{escape:?} was accepted and resolved to {p:?}"))
            .map_err(panic_msg)
            .unwrap_err();
            assert!(
                msg.contains("not a plain path inside its own sidecar directory"),
                "{escape:?} must be refused by name, not by luck: {msg}"
            );
        }
    }

    /// This engine's corpus guard brackets a scratch copy,
    /// never the vendored tree: pointed at a vendored deck it refuses before it
    /// claims or photographs anything, so no producer can run under it there.
    #[test]
    fn the_port_guard_refuses_a_vendored_case_directory() {
        let deck =
            crate::scratch::corpus_root().join("electricdss-tst/Test/AutoTrans/Auto1bus.dss");
        let msg = std::panic::catch_unwind(|| CorpusGuard::new(&deck.to_string_lossy()))
            .map(|_| panic!("a guard on the vendored {deck:?} was taken"))
            .map_err(panic_msg)
            .unwrap_err();
        assert!(
            msg.contains("runner::CorpusGuard") && msg.contains("lies inside the vendored corpus"),
            "{msg}"
        );
        // Nothing was claimed: the registry holds no key for that folder.
        let key = dir_claim_key(deck.parent().expect("a deck has a folder"));
        let reg = dir_registry().0.lock().unwrap_or_else(|e| e.into_inner());
        assert!(
            !reg.contains_key(&key),
            "a refused guard left a claim on {key:?}"
        );
    }
}

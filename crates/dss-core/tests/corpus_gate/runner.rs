//! Case runner: the Rust engine runs ONCE per case ([`run_rust_capture`]), then
//! its per-step state is compared against a channel's [`CaseResult`]
//! ([`compare_capture`]). The comparator call order and every `harness`
//! comparator are byte-identical to the pre-Phase-B `run_and_compare` — the only
//! structural change is that the oracle `CaseResult` is now fetched by the
//! scheduler (via a persistent [`Channel`]) and passed in, instead of being
//! spawned inside.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread::ThreadId;
use std::time::{Duration, Instant};

use dss_core::exec::Dss;
use serde_json::json;

use crate::engines::{CaseResult, Channel, Oracle};
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
// Corpus guard (unchanged; keeps the vendored corpus pristine across both
// engines' report/trace writes). RAII: created before the runs, restores on drop.
// ---------------------------------------------------------------------------

/// Buffer small files up to this size for overwrite-restore. Mirrors the oracle
/// server's `_RESTORE_MAX`.
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
/// The corpus puts many decks in one folder (`Test/AutoTrans`,
/// `IEEETestCases/8500-Node`, `StorageControllerTechNote/Support`, …), and this
/// test binary has more than one producer walking them. The gate's scheduler is
/// not the problem — its task unit IS the case-dir group, so its own cases in
/// one folder are already sequential — but libtest runs the gate `#[test]`
/// concurrently with its siblings in the same binary, and
/// `corpus_ad_matches_normal_mode` compiles `ad_sweep.json`'s decks **in place**
/// (`8500-Node/Run_8500Node.dss`, `Run_8500Node_Unbal.dss` and
/// `Run_RecloserSiting.DSS` all carry `ad: "pf"`). A deck's own `Show`/`Export`
/// lines run during `compile`, i.e. before `ad_solve_normal` re-points
/// `datapath` at a scratch directory, so they land in the very case directory
/// the gate is measuring.
///
/// Measured on this tree (G1.10a F4 run 2, `tmp/g110a/f_F4_unexpected_run2.md`):
/// with only the per-directory snapshot sharing below, one case's created-file
/// SET picked up another deck's reports — `ieee8500_*` and `ieee8500u_*` in a
/// single reply — on a different (case, channel) pair each run.
///
/// So the claim is **exclusive per directory** (coordinator decision D33(2)):
/// exactly one producer at a time owns a case dir, from the pre-run snapshot
/// through both oracle captures, the port run, and the sweep + restore. It is
/// **reentrant for the owning thread** — [`assert_deferred_rust_smoke`] takes a
/// second guard inside `scheduler::run_one_case`'s, and the overlap fixture
/// below takes two — and those nested guards share the one pristine snapshot,
/// so the names set is always the pre-run one and exactly one sweep runs, when
/// the last of them leaves.
type DirRegistry = (Mutex<HashMap<PathBuf, DirClaim>>, Condvar);

fn dir_registry() -> &'static DirRegistry {
    static REG: OnceLock<DirRegistry> = OnceLock::new();
    REG.get_or_init(|| (Mutex::new(HashMap::new()), Condvar::new()))
}

/// The claim key: the case directory as the filesystem itself spells it, so two
/// manifest rows reaching one physical folder through different spellings
/// (`Test/AutoTrans` vs `Test/autotrans`, a `..` segment, a junction) contend
/// for the same claim instead of running unserialized side by side. Falls back
/// to the literal path when the directory cannot be canonicalized (it does not
/// exist — there is nothing to protect).
/// How long a producer waits for a case directory before calling it a deadlock.
/// A claim is held for ONE case (both oracle captures + the port run + the
/// sweep), and the slowest gated case is a `large` feeder at well under a
/// minute, so this bound cannot fire on a healthy run — it exists so that a
/// claim leaked by a future edit fails loudly instead of hanging a 4-minute gate
/// with no output at all.
const DIR_CLAIM_DEADLINE: Duration = Duration::from_secs(600);

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
    /// it (coordinator decision D33(2)) — and photograph it.
    pub(crate) fn new(case_path: &str) -> Self {
        let dir = Path::new(case_path)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let key = dir_claim_key(&dir);
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
    /// oracle transports and the port probe report from
    /// ([`dss_epri::guard::classify_created`]; coordinator decision D32(3)).
    ///
    /// Before that this guard had its own recursion, which descended into
    /// PRE-EXISTING subdirectories — so an outer guard on `Test/` could delete
    /// the live report files of a case running concurrently in the sibling case
    /// directory `Test/AutoTrans/` (G1.10a F2b measured 9 such members over two
    /// full 526-case drives). One classification, three consumers closes the
    /// hazard everywhere: the removal roots are the case dir's own created
    /// entries, and a created directory goes as one tree.
    ///
    /// Until coordinator decision D33(3) this arm was `#[cfg(windows)]` and the
    /// other platforms kept a compile-only twin of the same rule, because
    /// `dss-epri` gated its every module on Windows. `dss_epri::guard` is pure
    /// `std::fs`, so it is now ungated (and the crate is an unconditional
    /// dev-dependency): the twin is deleted and "one classification, three
    /// consumers" is literally true on every platform.
    /// Returns the normalized names of created entries the case directory STILL
    /// lists afterwards — the same `sweep_failed` report its two twins make
    /// (`dss_epri::guard::CorpusGuard::sweep_created`,
    /// `corpus_guard.py::_sweep_created`). Until the G1.10a audit settlement
    /// (finding AC-2) this one producer discarded every removal error, so the
    /// classification was shared but the LOUDNESS was not: on a `kind=large*`
    /// case no `RunFileProbe` brackets the run and this guard is the ONLY
    /// sweeper, so a dropping it could not remove was invisible to the gate and
    /// left for a human to notice as an untracked file. `Drop` prints what comes
    /// back; nothing here panics, because a panicking `Drop` during another
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
        // as vendored (coordinator decision D33(2)); only then release the
        // directory and wake whoever is waiting for it.
        if self.snap.snapshot_ok {
            let leaked = self.sweep_created();
            if !leaked.is_empty() {
                // D32(2) forbids a silently swallowed removal failure. This
                // guard cannot fail the case from `Drop` (a panic here during
                // another panic aborts the process), so it says so on stderr,
                // where a full-drive log keeps it greppable.
                eprintln!(
                    "corpus guard: leaked dropping(s) under {}: {} — the sweep                      could not remove them (a producer is still holding the file                      open). They stay in the vendored corpus and poison the next                      producer's pre-run snapshot of this directory.",
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
         live output (D30(2)/D32(3); the shared classification \
         `dss_epri::guard::classify_created` and its F2b measurement)"
    );
    assert!(
        !root.join("ckt_di").exists(),
        "run-created dir tree removed"
    );
    std::fs::remove_dir_all(&root).ok();
}

/// G1.10a audit settlement, the settle stage's own measurement — a guard on a
/// PARENT case directory must never RESURRECT the output a sibling case's guard
/// has just swept.
///
/// `Test/` holds 36 manifest cases and `Test/AutoTrans/` five more, and the two
/// are different claim keys, so they run concurrently by design. The parent's
/// pre-run photograph used to walk into the child directory and buffer whatever
/// the child's run had already written; when the child then swept its own
/// output, the parent's `restore` found the file *missing*, took that for "the
/// run destroyed a vendored file" and wrote it back. That is the mechanism
/// behind the `Test/AutoTrans/Auto{1,3}bus_*.txt` residue STATUS has tracked
/// since 2026-08-29 (measured again here: one full `corpus_gate` drive left 9
/// such files with no `sweep_failed` report anywhere, the next left none), and
/// the residue is what silently shrinks the next producer's created-file set —
/// it red `run_files_pins::the_two_oracle_spellings_of_auto1bus_fold_to_one_member`
/// with `0` names instead of 9 during this settlement's own gate.
///
/// The fix is in `restore`: a snapshot entry that is GONE stays gone (a deck that
/// deletes a vendored file shows up as a tracked deletion, which is loud), while
/// an entry that still exists and was overwritten is still restored — the
/// contract `corpus_guard_restores_case_dir_recursively` pins.
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

/// G1.10a audit settlement (finding AC-2): the outer guard REPORTS a removal it
/// could not perform, exactly like its two twins
/// (`dss_epri::guard::tests::a_created_file_the_sweep_cannot_remove_is_reported_as_sweep_failed`,
/// `corpus_guard.py`'s `SELF_TEST_LEAK_*`). The fixture holds the created file
/// open with `FILE_SHARE_READ` only — what the FPC/Delphi file APIs do — so
/// `remove_file` fails with a sharing violation, the mechanism dss_capi's
/// never-closed Storage trace stream produced live (G1.10a F4).
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
/// Since D33(2) that is the only way two guards on one directory can overlap
/// (another thread blocks in `new`), and the interleaving that used to leak is
/// still the one this pins: A snapshots clean → A writes → B snapshots (sees A's
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

/// Coordinator decision **D33(2)**: one producer at a time owns a case
/// directory. Two synthetic manifest rows in one folder — the
/// `IEEETestCases/8500-Node` shape, where seven rows plus `ad_sweep.json`'s `pf`
/// decks all write `<CircuitName>_*` reports into ONE directory — must not have
/// their before/after windows overlap, or one producer's output lands in the
/// other's created-file SET (measured: G1.10a F4 run 2).
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

/// The other direction of D33(2): the claim is per DIRECTORY, not a global lock
/// on the corpus. Two producers in DIFFERENT case folders keep running at the
/// same time — the gate's `jobs=16` parallelism depends on it, and a global
/// lock would serialize a 4-minute gate into an hour.
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
/// `EngineChannel` is `pub(crate)` to this one test binary while `harness/`
/// compiles into ~20 others, so the guard takes a `&str`
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
    // G1.10a audit settlement (finding AT-1) — a `compile` that produced NEITHER
    // a circuit NOR an error can only mean the master file read back with no
    // command in it: `exec/solve.rs::do_redirect` errors loudly on a missing
    // (`Redirect file not found`) or unreadable (`could not be read`) file, but a
    // SHORT read — an empty file, or just its leading comment block — yields no
    // command and no diagnostic, and every later command then fails with "You
    // must create a new circuit object first" (the shape the one-off
    // `controls:espvlcontrol` red of this sub-step's part P took, on a machine at
    // 100 % CPU). Assert it HERE, where the deck's size on disk is still evidence,
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
/// step, in the fixed comparator order. Byte-identical to the pre-Phase-B loop
/// EXCEPT where a per-case-per-channel `ledger` scope partitions a field: the
/// untouched `harness` comparator runs on the unscoped remainder while the
/// ledger's envelope/exact-pair assert covers the scoped part and records the hit
/// (§1.3). With `ledger = None` or an empty view every field takes the original
/// path unchanged.
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
    // G1.7 / D15: the port's isolation topology at step 0 of THIS case+channel
    // run — the memoization reference the topology comparator rebases onto once
    // a conductor has opened (`harness::topology::compare_topology`).
    let mut topo_step0: Option<harness::topology::IsolationSnapshot> = None;

    // GOLDEN_REBASE G1.6(i) protocol: `RelCalc` is NOT idempotent (a nested or
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

        // GOLDEN_REBASE G1.6(i): drive the executive `RelCalc` HERE - after the
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
            // Iteration policy (§4 Phase C): the pinned capi_v0145 channel is an
            // exact 1:1 contract; the r4133 channel (a different engine line)
            // allows the port to converge in FEWER iterations — never more. A
            // ledger `iterations` scope overrides both (exact pair, or an explicit
            // rust_le_oracle where a target-rev delta needs pinning).
            //
            // Stage F (Part IV.2 drift model): both non-ledgered shapes go
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

        // Whole-artifact ledger exclusions (`GOLDEN_REBASE_PLAN.md` G2.5): the
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
        // `newton*` decks in **both** lanes since `GOLDEN_REBASE_PLAN.md` G2.3
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

        // `GOLDEN_REBASE_PLAN.md` G1.9 — the five `Circuit` aggregates and the
        // ten `Solution` scalars. The surface is UNFLAGGED and universal (no
        // `G1_SURFACE_FLAGS` row, no rigor token, no forced population), so the
        // capture is demanded on every live case of every gating channel; the
        // `capture_guard` rail is not reused because its message is
        // manifest-flag shaped and there is no flag to name here.
        let agg = cp.aggregates.as_ref().unwrap_or_else(|| {
            panic!(
                "{ctx}: the `{}` capture carries no `aggregates` member. G1.9 is an \
                 unflagged, universal surface — every live case must compare it, so \
                 an absent capture FAILS the case instead of silently comparing \
                 nothing (GOLDEN_REBASE_PLAN.md §1.1(f)).",
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
        // WP-G1 G1.3a: the per-element **derived** channels — `Enabled` plus
        // the polar renderings `CurrentsMagAng` / `VoltagesMagAng` / `Residuals`
        // (r4133 `DDLL/DCktElement.pas:1058`/`:1082`/`:827`). Compared on the
        // same caps the loop above just used, so a ledger `element` scope that
        // pins one of the new sub-channels neutralizes it here too, and an
        // unscoped element is compared against the untouched oracle cap.
        //
        // G1.3b rides on the same flag: the per-element symmetrical-component
        // surfaces `SeqCurrents` / `SeqVoltages` / `SeqPowers` (r4133
        // `DDLL/DCktElement.pas:700-737` / `:660-698` / `:739-797`; capi
        // `CAPI/CAPI_Alt.pas:490-527` / `:620-659` / `:529-593`), captured in the
        // same `derived` block and compared by `harness::compare_element_seq`
        // beside — never instead of — the polar comparator, on the same
        // (possibly ledger-rewritten) caps.
        //
        // The guard is the flag's own non-vacuity rail: under `derived` BOTH
        // transports emit `enabled` for every element (present even on the
        // disabled ones, whose polar channels the capture must skip — r4133
        // `CktElementV(19)` dereferences a nil `NodeRef` there), so a channel
        // that ignored the request answers with zero `enabled` fields and the
        // case fails instead of comparing nothing.
        //
        // There are two of them, for the reason `compare_element_extras` has
        // two (G1.3d(ii) audit settlement): the two comparators read disjoint
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
            // G1.3c's two, for the same reason: `compare_element_total_powers`
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
            // The voltage half gets its own rail too (G1.3c audit settlement,
            // 2026-09-06): it is the one capture field of this surface that no
            // other rail counts, and it is the half whose guards differ between
            // the channels (capi's `CplxSeqVoltages` alone tests
            // `NodeRef = NIL`, `CAPI/CAPI_Alt.pas:878`, where r4133 mode `13`
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
            // folded per channel (`harness::na_seq_power`, G1.3b D-b2), and the
            // r4133-only truncated-matrix term rides on the same argument.
            let seq_channel = channel.props_channel();
            for ec in &cp.elements {
                // A ledger `element` scope rewrites only the sub-channels it
                // names, so both comparators see the untouched oracle values on
                // every channel the scope does not select.
                let cap = el_rewrites.get(&ec.name.to_lowercase()).unwrap_or(ec);
                compare_element_derived(&snaps, cap, tol, &ctx, channels);
                // The gating population's sequence-arm census — the guard behind
                // D-b1 costing zero ledger rows (`harness::
                // assert_seq_arm_population`, checked in the gate's epilogue).
                // Recorded HERE and never inside the comparator (coordinator
                // decision D24, the `record_control_census` line below): the
                // `harness::seq_floors` fixtures call `compare_element_seq` in
                // this same test binary, several of them on the very arm the
                // guard counts, so a comparator-side census reads the gating
                // population plus the fixtures under the mandatory
                // `cargo test --workspace` shape. The comparator hands back the
                // arm it classified (`None` for the disabled and 0-terminal
                // rows it returns on), so there is one classification, not two.
                if let Some(arm) =
                    compare_element_seq(&snaps, cap, tol, &ctx, seq_channel, channels)
                {
                    harness::record_seq_arm(arm, seq_channel);
                }
                // G1.3c, beside — never instead of — the three above, on the
                // same possibly-rewritten cap: the un-`Cabs`'d complex pair
                // (r4133 `DDLL/DCktElement.pas:931-975`/`:885-928`, capi
                // `CAPI/CAPI_Alt.pas:898-925`/`:872-895`), which needs the
                // channel for the r4133-only truncated-matrix band term and for
                // its measured 0-terminal shapes, and the per-terminal
                // `TotalPowers` sums (`:1109-1139` / `:1108-1141`), which need
                // neither. Neither records a census (coordinator decision D24).
                compare_element_cplx_seq(&snaps, cap, tol, &ctx, seq_channel, channels);
                compare_element_total_powers(&snaps, cap, tol, &ctx, channels);
            }
        }

        // WP-G1 G1.3d: the per-element **extras**. Part (i)'s discrete
        // index/name scalars — `NumTerminals` / `NumConductors` / `NumPhases`
        // (r4133 `DDLL/DCktElement.pas:139`/`:144`/`:149`), `EnergyMeter`
        // (`:442`) and `NodeOrder` (`:1032`) — plus part (ii)'s five
        // control-derived scalars (`:207-262`, over `Common/Utilities.pas:3165`
        // `GetOCPDeviceType`), all discrete and compared exactly: no tolerance,
        // no `ElemChannels` selector, no ledger sub-channel.
        //
        // Part (ii)'s sixth field, `PhaseLosses` (`:637-659` over
        // `Common/CktElement.pas:1078`), is numeric and goes through its own
        // comparator on the powers tier, with the `ElemChannels` selector: it
        // reaches the same cache-aware `ComputeIterminal` as Powers/Losses and
        // shares their `newton*` lane exclusion (see `harness::lane`).
        //
        // Fed from the same `el_rewrites`-or-raw caps as the two loops above so
        // the block keeps their shape, which costs nothing and hides nothing: a
        // ledger rewrite is a full `clone_element_cap` (`ledger.rs:2292-2294`,
        // `ec.clone()`) with only its six named value channels overwritten
        // (`rewrite_element_selected`, `:1702`), so the five extras fields it
        // hands back are always the untouched oracle ones.
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
                // Its own flag spelling (G1.3d(ii) audit settlement): the two
                // rails read disjoint capture fields, so a missing `n_terms`
                // and a missing `pl_kw` must not print the same sentence.
                "compare_element_extras (PhaseLosses)",
                channel_tag(channel),
                cp.elements.iter().filter(|e| !e.pl_kw.is_empty()).count(),
                &ctx,
            );
            // The channel travels with the capture for one reason only: the
            // "no meter" sentinel is spelled per channel and is folded per
            // channel (`harness::oracle_meter_name`, G1.3d(i) audit settlement).
            let ch = channel.props_channel();
            for ec in &cp.elements {
                // A ledger `element` scope rewrites only the sub-channels it
                // names (`ledger.rs::rewrite_element_selected`,
                // `SUBCHANNEL_FIELDS`). The ten discrete extras are on NO
                // sub-channel, so `compare_element_extras` always sees the
                // untouched oracle values — a discrete divergence can never be
                // masked by a scope. `PhaseLosses` IS the seventh sub-channel
                // (G1.3d(ii)): a scope selecting `phase_losses` overwrites
                // `pl_kw`/`pl_kvar` with the port's values (x0.001), which is
                // exactly how the ten measured widenings neutralize it here.
                let cap = el_rewrites.get(&ec.name.to_lowercase()).unwrap_or(ec);
                compare_element_extras(&snaps, cap, ch, &ctx);
                compare_element_phase_losses(&snaps, cap, tol, &ctx, channels);
                // The gating population's control census — the guard behind
                // D-ii-1 costing zero ledger rows (`harness::
                // assert_no_multi_control_element`, checked in the gate's
                // epilogue). Counted from the ORACLE's `NumControls`, which the
                // compare above has just held the port to.
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

        // The reliability surface (GOLDEN_REBASE G1.6(i)), immediately after the
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
            // Per-variable ledger exclusions (`R4133_PROPS_PLAN.md` RP3.10).
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
            // Then the two Relay label rows, which apply in **both** lanes
            // (GOLDEN_REBASE_PLAN.md G2.2d). The relay/recloser split is read
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

        // The bus voltage surface (GOLDEN_REBASE_PLAN.md G1.4a). Placed here to
        // mirror the capi transport's capture slot — after `ctrlqueue`, before
        // the `all_properties` `?` sweep that must stay last
        // (`tools/oracle/oracle_server.py`). Every bus read is class C
        // (order-free) under §1.1(a)/D3: both engines read `Solution.NodeV`
        // directly (`CAPI/CAPI_Alt.pas:2275` == r4133 `DDLL/DBus.pas:423`) and
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

            // The DISTANCE half of the SAME per-bus walk (GOLDEN_REBASE_PLAN.md
            // G1.4b): `Bus.Distance` plus the two circuit-level views of the
            // very same field, `Circuit.AllBusDistances` and
            // `Circuit.AllNodeDistances`. Like the two blocks below it rides
            // `compare_bus` — no flag of its own — and is class C (order-free):
            // all three arms return the stored `TDSSBus.DistFromMeter` and move
            // only `ActiveBusIndex` (`CAPI/CAPI_Alt.pas:2071-2074`,
            // `CAPI_Circuit.pas:671-688`/`:697-722` == r4133 `DDLL/DBus.pas:122-128`,
            // `DCircuit.pas:566-580`/`:582-604`).
            //
            // `v_excluded` is deliberately NOT passed: the distance is a
            // zone-build output, not an image of `Solution.NodeV`, so a
            // `voltages` ledger cause cannot explain a divergence here and
            // suppressing it would hide a zone-build bug behind an unrelated
            // triage. Compared exactly (`rel = abs = 0`).
            //
            // What a real upstream divergence gets instead is its own ledger
            // field, `distance`, selected per BUS (coordinator decision D29
            // step 3). The closure is passed rather than a precomputed flag
            // because `excluded()` records the scope's hit when it answers, and
            // `compare_bus_distances` asks it only once the exact equality has
            // already failed — so a hit means "masked a real divergence" and a
            // fixed upstream leaves the scope STALE.
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

            // The SEQUENCE + LINE-TO-LINE half of the SAME per-bus walk
            // (GOLDEN_REBASE_PLAN.md G1.4c): `Bus.SeqVoltages`/
            // `CplxSeqVoltages`/`VLL`/`puVLL`, four arms appended to the one
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
            // length, the `vll_declined` cross-check and the port-internal
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

            // The short-circuit half of the SAME per-bus walk
            // (GOLDEN_REBASE_PLAN.md G1.5): `Bus.Zsc1`/`Zsc0`/`ZscMatrix`/
            // `YscMatrix`/`Isc`/`Voc`, six arms appended to the one
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
            // with the study bit, the bus identity and every length.
            if c.compare_zsc {
                capture_guard::require_capture(
                    "compare_zsc",
                    channel_tag(channel),
                    cp.buses.len(),
                    &ctx,
                );
                // The count of buses whose full `n x n` matrices were really
                // value-compared feeds the gate epilogue's fail-on-stale
                // (`harness::assert_sc_study_compare_ran`, G1.5 audit
                // settlement T1): `port_ran == oracle_ran` is also true when
                // NEITHER ran, so without this the whole non-trivial half of
                // the surface could go quiet and stay green. This is the one
                // call site that records — the harness' own drives must not.
                let full = harness::compare_bus_short_circuit(
                    dss,
                    &cp.buses,
                    tol,
                    channel.props_channel(),
                    v_excluded,
                    &ctx,
                );
                harness::record_sc_study_compare(full);
            }

            // The AT-BUS half of the SAME per-bus walk (GOLDEN_REBASE_PLAN.md
            // G1.4d, coordinator decision D26): `Bus.AllPCEatBus` /
            // `Bus.AllPDEatBus`, two arms appended to the one `SetActiveBus`
            // sweep — so, like the three blocks above, it rides `compare_bus`
            // and opens no flag of its own.
            //
            // It runs LAST because that is the order both transports read the
            // pair in, and they read it last because on the r4133 channel it is
            // the bus walk's only `ModeEffect::Impure` read: `getP*atBus`
            // drives `DSS_Class.First`/`Next` and `TDSSClass.Get_First`/
            // `Get_Next` assign `ActiveCircuit.ActiveCktElement`
            // (`Common/DSSClass.pas:342-371`). It moves neither `ActiveBusIndex`
            // nor any `Iterminal` cache, so the surface is still capture-group
            // C (order-free) — `crates/dss-epri/src/modes.rs`, G1.4d F0.
            //
            // `v_excluded` is deliberately NOT passed and no `Tolerances`
            // either: the wire carries element NAMES, not an image of
            // `Solution.NodeV`, so a triaged voltage cause can never explain a
            // divergence here and there is nothing to band.
            //
            // The two oracles run two DIFFERENT walks and each contradicts its
            // own stated intent, so neither is compared against the port's
            // answer directly: each channel's walk is asserted POSITIVELY over
            // the port's own attachment facts (`oracle == walk(port state)`,
            // the D15/D16/D21 shape). Nothing is excluded — G1.4d adds no
            // ledger entry — and the measured disagreements are COUNTED into
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
            // `oracle` pin, or `num_rel` for numeric-skeleton values (§1.3; same
            // contract as `probe`). To keep the monolithic `compare_all_properties`
            // count/order contract intact while excluding that one pair from the
            // value compare, rewrite its oracle value to the Rust `?`-surface value
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

        // The PDElements interface walk (GOLDEN_REBASE G1.6b). Late in the
        // step, next to the other whole-model surfaces (G1.7's topology arm is
        // the one that must run after it): `Dss::pd_elements` is a
        // `&self` read over `Circuit.pd_elements` that depends on no active
        // element and no solve state, so its position among the comparators is
        // free — unlike the CAPTURE order, which is fixed on both transports
        // (after the meters, before the probes) because the oracles' own
        // `ParentPDElement` read hijacks `ActiveCktElement`.
        //
        // `require_capture_opt`, not `require_capture`: 96 of the 372 walked
        // live capi cases hold no PD element at all, so an EMPTY walk is a
        // legitimate answer the comparator must still match (`[]` against a
        // non-empty port walk fails on the length assert). What must never
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

        // `GOLDEN_REBASE_PLAN.md` G1.7 — the `ITopology` interface, compared
        // LAST in the step on purpose, mirroring both transports' capture order
        // (`oracle_server.py::run_case`, `dss-epri::capture::run`): building the
        // topology tree stamps `Checked`/`IsIsolated`/`BusChecked` on every
        // element (r4133 `Common/Circuit.pas:2937-2947`), upstream on its
        // circuit and `Dss::topology_view` on ours, so no other comparison of
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
            // `topo_step0` carries the port's OWN step-0 isolation topology
            // across the steps of this case+channel run: coordinator decision
            // D15's rule is defined against it, and the engine is re-run per
            // channel, so the reference is per run and never shared.
            harness::topology::compare_topology(dss, topo, &ctx, label, i, &mut topo_step0);
        }

        // `GOLDEN_REBASE_PLAN.md` G1.8 — the flat incidence surface, compared
        // AFTER the topology block and therefore last of the whole step, exactly
        // as both transports capture it (`oracle_server.py::run_case`,
        // `dss-epri::capture::run`; `crates/dss-core/tests/capture_order.rs`
        // asserts the source order on both). Two independent reasons, neither of
        // them cosmetic: `Dss::inc_matrix_view` REBUILDS solution state
        // (`IncMat`, `Laplacian`, `Inc_Mat_Rows`, `IncMat_Ordered`) and on r4133
        // the same pair also moves `ActiveCktElement`
        // (`AddSeriesReac2IncMatrix` -> `ActiveDSSClass.First`, r4133
        // `Common/Solution.pas:3007-3010`); and it must FOLLOW the topology
        // read, whose memoized `Branch_List` is what G1.7's two decline censuses
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

/// G1.10a / coordinator decision D32(2), hardened by the G1.10a audit
/// settlement (finding AT-3) — hygiene H1, checked BEFORE anything else a case
/// does with the filesystem.
///
/// `swept` is the transport's own `sweep_failed` report: what its `CorpusGuard`
/// classified as run-created and then could NOT remove. Such an entry stays in
/// the case directory, so the next producer of this case (the other channel, or
/// the port's probe) snapshots it as pre-existing and silently drops that name
/// from its created set. Not hypothetical: dss_capi never closes a Storage
/// `debugtrace` stream (`src/PCElements/Storage.pas:872`, freed only at
/// `:871`/`:1199`), so the capi channel used to leak `STOR_<name>.csv` and make
/// the `r4133` channel report an empty set for that deck (G1.10a F4).
///
/// `None` is a MISSING report, not a clean one: the field was a plain
/// `Vec<String>` behind `serde(default)` until the audit settlement, so a
/// transport that stopped emitting the key deserialized to `[]` and disarmed
/// this rail in silence. Both transports emit it unconditionally
/// (`tools/oracle/oracle_server.py::run_case`, `dss-epri::capture::run_case`),
/// so its absence is a broken transport and fails the case here.
fn assert_swept_clean(label: &str, producer: &str, swept: Option<&[String]>) {
    let Some(swept) = swept else {
        panic!(
            "{label}: the `{producer}` producer sent no `sweep_failed` report.              Every transport reports it on every case (it is the D32(2) leak              rail); a reply without the key can never be read as \"the sweep              was clean\"."
        );
    };
    assert!(
        swept.is_empty(),
        "{label}: the `{producer}` producer left {} leaked dropping(s) in the          case directory that its corpus guard classified as run-created and          could not remove: {}. A file still held open by that engine poisons          every later producer's pre-run snapshot (and pollutes the vendored          corpus). Fix the producer — release the circuit (`clear`) before the          guard sweeps, or close the file — never widen the surface around it.",
        swept.len(),
        swept.join(", "),
    );
}

/// The D32(2) rail fires on a leaked dropping, and names the producer.
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

/// Assert the oracle step counts, run the Rust engine once, compare against the
/// given `channel`'s capture (its iteration + eventlog-mask policy). No guard,
/// no oracle fetch — the caller (scheduler or [`run_and_compare`]) owns those.
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
    // G1.10a / coordinator decision D33(1) — the channel's own D32(2)(a)
    // teardown `clear` raised. Surfaced FIRST (before the hygiene assert below,
    // whose panic would otherwise hide it) and deliberately NOT a case failure:
    // the teardown runs after every read of the run, so the compared surface is
    // already captured, and the transport that raised exits after replying so a
    // pooled worker is respawned instead of reused
    // (`tools/oracle/oracle_server.py::main`). The one measured producer is the
    // pinned dss_capi 0.14.5 on the two `modes:autoadd` decks (`DSSException
    // (#303) ... clear ... Access violation`) — an outdated-oracle fault, not a
    // port divergence, so it gets a note here and in `DIVERGENCES.md`, never a
    // ledger row.
    if let Some(err) = oc.teardown_error.as_deref() {
        // One line: the pinned oracle folds a CRLF-formatted multi-line
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
    // G1.10a / coordinator decision D32(2) — hygiene H1, checked BEFORE anything
    // else this case does with the filesystem. `sweep_failed` is what the
    // channel's own `CorpusGuard` classified as run-created and then could not
    // remove; the entry stays in the case directory, so the NEXT producer of
    // this case (the other channel, or the port's probe below) snapshots it as
    // pre-existing and silently drops that name from its created set. That is
    // not hypothetical: dss_capi never closes a Storage `debugtrace` stream
    // (`src/PCElements/Storage.pas:872`, freed only at `:871`/`:1199`), so the
    // capi channel used to leak `STOR_<name>.csv` and make the `r4133` channel
    // report an empty set for that deck (G1.10a F4). Both transports now release
    // the circuit before their guard sweeps, and any survivor fails the case
    // here instead of hiding — the order-coupling can never come back silently.
    assert_swept_clean(label, channel_tag(channel), oc.sweep_failed.as_deref());
    let tol = tol_for(&c.kind);
    // G1.10a — the run-produced FILE SET. The port needs its OWN before/after
    // bracket (and must sweep what it classified) because the scheduler runs the
    // Rust engine once PER CHANNEL: on an `engines: "both"` case the second run
    // would otherwise find channel 1's files already on disk and report an empty
    // created set. The outer `CorpusGuard` (`scheduler::run_one_case`) stays the
    // safety net and is never bypassed. Started here, before the engine touches
    // the directory and after the oracle result is already in hand, so anything
    // the ORACLE's own guard failed to sweep sits in the probe's "before"
    // snapshot and can never be mis-attributed to the port.
    #[cfg(windows)]
    let run_file_probe = c
        .compare_run_files
        .then(|| harness::run_files::RunFileProbe::start(case_path));
    // The classification itself is portable since D33(3) (`dss_epri::guard` is
    // ungated, and `CorpusGuard::sweep_created` above runs it on every
    // platform), but the SURFACE is compared only against the gate's two oracle
    // channels and the `r4133` transport is a Win64 DLL, so the comparator
    // `harness::run_files` stays declared under `#[cfg(windows)]`. Refuse the
    // flag loudly elsewhere rather than compare nothing (that module's
    // §Platform doc).
    #[cfg(not(windows))]
    assert!(
        !c.compare_run_files,
        "{label}: `compare_run_files` is Windows-only — its `r4133` oracle \
         channel is a Win64 DLL, so the comparator `harness::run_files` is \
         declared under `#[cfg(windows)]`"
    );
    let (mut dss, baseline) = run_rust_capture(label, case_path, c);
    compare_capture(
        &mut dss, baseline, oc, label, case_path, c, &tol, channel, ledger,
    );
    // The port's run is over: drop the engine BEFORE the probe reads, so a file
    // the engine still holds open is closed (and flushed) first — a removal that
    // failed on an open handle would make the next channel's probe see the file
    // as pre-existing.
    drop(dss);
    #[cfg(windows)]
    if let Some(probe) = run_file_probe {
        let port_files = probe.finish_and_clean(label);
        // Field-by-field ledger partition, per created NAME (the `run_files`
        // field's `name_re` scopes); the call marks the scope hit, which is what
        // keeps `ledger.json` fail-on-stale honest. Run-level surface, so step 0
        // is the only step a scope can select.
        let excluded = |name: &str| ledger.is_some_and(|v| v.excluded("run_files", Some(name), 0));
        harness::run_files::compare_run_files(
            channel_tag(channel),
            oc.run_files.as_deref(),
            &port_files,
            &excluded,
            label,
        );
    }
}

/// One-shot convenience for the opt-in report tests: snapshot the case dir,
/// fetch the pinned oracle model once, compare against the `capi_v0145` channel
/// (no ledger — the report tests predate it).
pub(crate) fn run_and_compare(oracle: &Oracle, label: &str, case_path: &str, c: &SolvableCase) {
    let _guard = CorpusGuard::new(case_path);
    let oc = oracle.run_case(case_path, c);
    compare_with_result(&oc, label, case_path, c, EngineChannel::CapiV0145, None);
}

// ---------------------------------------------------------------------------
// Abort + pending contracts (unchanged behavior; abort now over a Channel).
// ---------------------------------------------------------------------------

/// Gate a deck that BOTH engines abort at solve. The oracle *raises* at solve
/// (so there is no solved state to line up); assert the ORACLE aborts with the
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
    // the oracle raises on the aborting solve → `ok:false` carrying the message.
    let req = json!({
        "cmd": "run",
        "case_path": case_path,
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
    assert!(
        !resp.ok,
        "{label}: oracle did NOT abort the solve (expected an abort containing {expected:?})"
    );
    let oracle_err = resp.error.unwrap_or_default();
    assert!(
        oracle_err.contains(expected),
        "{label}: oracle abort message {oracle_err:?} does not contain {expected:?}"
    );

    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile \"{case_path}\""));
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
}

/// Schema v2 (§4 Phase C step c): a `defer_ledger` case is parked from live
/// oracle comparison (its validated capi015 behavior reproduces on neither
/// surviving channel; a Phase D ledger will re-gate it). It is still **smoke-run**
/// on the Rust engine — compile + post + solve every step must converge with NO
/// new engine errors. This catches a *convergence or error-surfacing* regression
/// (a deferred case that stops solving, NaNs out, or starts erroring); it does
/// **not** catch a *numeric-correctness* regression that still converges — no
/// physical value is compared against any reference here. Full numeric coverage
/// on these cases returns with the Phase D ledger. This is strictly stronger than
/// the plan's `pending` fallback (which asserts an error) and membership is
/// preserved, but it is a bounded, temporary reduction in verification depth.
pub(crate) fn assert_deferred_rust_smoke(label: &str, case_path: &str, c: &SolvableCase) {
    let (mut dss, baseline_errors) = run_rust_capture(label, case_path, c);
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
             must still SOLVE on the Rust engine (defer_ledger parks the ORACLE compare, \
             not the Rust smoke)"
        );
    }
}

/// GAPS_PLAN.md §2.3 pending discipline: the unported feature must surface as an
/// engine error (a clean run means a silent fallback masks the gap).
pub(crate) fn assert_pending_errors_loudly(label: &str, case_path: &str, c: &SolvableCase) {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile \"{case_path}\""));
    for cmd in &c.post {
        dss.command(cmd);
    }
    assert!(
        !dss.errors().is_empty(),
        "{label}: pending case (wp {}) ran with NO engine error — the unported \
         feature fell back silently; if it is now ported, flip `pending: false` \
         and prove the live compare green (GAPS_PLAN.md §3.1)",
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

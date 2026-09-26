//! Per-run scratch copies (RETRO_FIXES_PLAN.md RF-I00-01, `INFRA|1`).
//!
//! The corpus gate never runs a producer inside `tests/corpus/`: every
//! (case, producer) run gets a FRESH directory
//! `<target>/corpus-scratch/<case key>/<producer>/<run nonce>/`, the case's
//! directory closure ([`Closure`]) is copied into it, the producer compiles the
//! COPY's deck, and the copy is removed afterwards within one bounded budget
//! ([`COPY_REMOVE_ATTEMPTS`] x [`COPY_REMOVE_PAUSE`]). A copy that outlives the
//! budget fails the case naming its producer: that is the D32(2) engine-leak
//! rail on its new footing (a handle the producer still holds, the dss_capi
//! Storage `DebugTrace` class, blocks the removal exactly as it blocked the
//! old sweep).
//!
//! Why: parent (`Test/`) and child (`Test/AutoTrans/`) case directories ran
//! concurrently in ONE shared tree, and a recursive snapshot reading a sibling
//! case's report without `FILE_SHARE_DELETE` turned into "leaked dropping"
//! reds (28 of 41 red runs over waves 1-2, RETRO_FIXES §6). Three producers of
//! one case now get three copies, so an engine leak can no longer poison the
//! next producer and no order-coupling can hide a gap.
//!
//! Every surface is read from the copy the producer ran in: the transports'
//! created-file set is the listing diff against the copy's initial listing
//! (their guards snapshot the copy before the run), the CONTENTS and the
//! demand-interval tree are copied out to the gate's sidecars before the reply,
//! and the sidecars stay keyed by the VENDORED deck path
//! (`engines::build_run_request`'s `key`), so their location and bytes are the
//! ones the gate had before the copies.
//!
//! Both oracle engines leave the process working directory in the deck's
//! folder after a compile (r4133 `Executive/ExecHelper.pas:752-754`,
//! `SetCurrentDir(CurrDir)`; dss_capi `SetCurrentDSSDir`), and Windows refuses
//! to remove a directory that is some process's working directory, so both
//! transports step back to their startup directory before they reply
//! (`tools/oracle/oracle_server.py::main`, `dss-epri`'s `epri-worker`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// The producer tag of the port's own runs (the two oracle channels use their
/// channel tags, `capi_v0145` and `r4133`).
pub(crate) const PORT: &str = "port";

/// How many times a copy's removal is retried after the first attempt, and the
/// pause before each retry: 25 x 200 ms = 5 s at most, spent only while a
/// removal does not take. A transient holder (a Defender scan of a file the
/// producer just closed, a worker process that exits right after its reply)
/// is waited out; a handle held for the producer's lifetime outlasts it and
/// fails the case naming the producer.
pub(crate) const COPY_REMOVE_ATTEMPTS: u32 = 25;
/// See [`COPY_REMOVE_ATTEMPTS`].
pub(crate) const COPY_REMOVE_PAUSE: Duration = Duration::from_millis(200);

/// `<target>/corpus-scratch`, derived from the test binary's own path
/// (`<target>/<profile>/deps/<bin>`) like `engines::gate_scratch_root`, so it
/// follows `CARGO_TARGET_DIR` and a lane's `target` junction. Never under
/// `tests/corpus/`.
pub(crate) fn scratch_root() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| {
            exe.parent() // deps/
                .and_then(|p| p.parent()) // debug/ | release/
                .and_then(|p| p.parent()) // target/
                .map(|p| p.to_path_buf())
        })
        .unwrap_or_else(std::env::temp_dir)
        .join("corpus-scratch")
}

/// The vendored corpus root every closure must stay inside
/// (`tests/corpus/`, lexically normalized).
pub(crate) fn corpus_root() -> PathBuf {
    lexical(
        &[env!("CARGO_MANIFEST_DIR"), "..", "..", "tests", "corpus"]
            .iter()
            .collect::<PathBuf>(),
    )
}

/// `.` and `..` resolved on the path's own components, never through the
/// filesystem: `canonicalize` answers a `\\?\` verbatim path, in which the two
/// DSS engines would stop resolving a deck's own `..` references.
pub(crate) fn lexical(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Case-folded, forward-slashed spelling for path comparisons (the corpus is
/// case-insensitive on its home platform).
fn fold(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/").to_lowercase()
}

fn is_under(p: &Path, dir: &Path) -> bool {
    let (p, d) = (fold(p), fold(dir));
    p == d || p.starts_with(&format!("{d}/"))
}

/// What one case's copy must hold so that every file its deck reads resolves
/// inside the copy exactly as it resolves in the vendored tree.
///
/// * `tree` — the case directory, copied recursively (the old guard's scope).
/// * `flat` — the folder of every script the deck reaches OUTSIDE the case
///   directory through `Redirect`/`Compile` (or any `.dss` it names), copied
///   one level deep: both engines change the working directory to a
///   redirected file's own folder before reading it (r4133
///   `Executive/ExecHelper.pas:752-754`, `SetCurrentDir(CurrDir)` and, on a
///   compile, `SetDataPath(CurrDir)`), so its plain relative names resolve
///   there.
/// * `files` — every other existing file a reached script names outside the
///   first two sets (`mult=(file=../x.csv)`, `BusCoords ../../y.dat`, ...).
/// * `outside` — references that leave `tests/corpus/` (absolute or
///   drive-letter paths): left exactly as they are, so they resolve to the
///   same place in the copy as in the tree, and listed.
///
/// `root` is the nearest common ancestor of all of it; the copy mirrors the
/// tree below `root` sparsely (only the members above), so a `../` reference
/// climbs to the same file. Copying the whole ancestor instead would move
/// ~11 GB per gate run (38 decks climb to `Version8/Distrib`, 126 MB each);
/// the sparse closures move ~0.58 GB per lane through the scheduler (1784
/// copies, 30 746 files, measured 2026-09-26) plus ~0.1 GB through the AD
/// sweep (72 copies), and a member a closure missed would show as a
/// port compile error that no manifest `expect_warnings` covers
/// (`runner::assert_expected_warnings`), never as a silent change.
#[derive(Debug, Clone)]
pub(crate) struct Closure {
    pub(crate) root: PathBuf,
    pub(crate) deck: PathBuf,
    pub(crate) tree: PathBuf,
    pub(crate) flat: BTreeSet<PathBuf>,
    pub(crate) files: BTreeSet<PathBuf>,
    pub(crate) outside: BTreeSet<String>,
}

/// Strip a DSS line comment (`!` or `//` outside quotes).
fn strip_comment(line: &str) -> &str {
    let b = line.as_bytes();
    let mut quote: Option<u8> = None;
    for i in 0..b.len() {
        match quote {
            Some(q) if b[i] == q => quote = None,
            Some(_) => {}
            None => match b[i] {
                b'"' | b'\'' => quote = Some(b[i]),
                b'!' => return &line[..i],
                b'/' if b.get(i + 1) == Some(&b'/') => return &line[..i],
                _ => {}
            },
        }
    }
    line
}

/// Every candidate path token of one (comment-stripped) line: each quoted
/// string whole, plus every piece of the line split on the DSS delimiters.
fn tokens(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (i, seg) in line.split(['"', '\'']).enumerate() {
        if i % 2 == 1 && !seg.trim().is_empty() {
            out.push(seg.trim().to_string());
        }
        out.extend(
            seg.split(|c: char| c.is_whitespace() || "=()[]{},;".contains(c))
                .filter(|t| !t.is_empty())
                .map(str::to_string),
        );
    }
    out
}

fn is_script(p: &Path) -> bool {
    p.extension()
        .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("dss"))
}

/// One directory's entries, case-folded, listed once per closure walk: a
/// token is looked up here instead of being `stat`ed, which is what keeps the
/// walk over an 8500-node deck (every element name is a candidate token) in
/// milliseconds.
#[derive(Default)]
struct Listings(BTreeMap<String, BTreeMap<String, bool>>);

impl Listings {
    /// `Some(is_file)` when `p` exists.
    fn lookup(&mut self, p: &Path) -> Option<bool> {
        let dir = p.parent()?;
        let name = p.file_name()?.to_string_lossy().to_lowercase();
        let entries = self.0.entry(fold(dir)).or_insert_with(|| {
            std::fs::read_dir(dir)
                .map(|rd| {
                    rd.flatten()
                        .map(|e| {
                            let is_file = e.file_type().is_ok_and(|t| t.is_file());
                            (e.file_name().to_string_lossy().to_lowercase(), is_file)
                        })
                        .collect()
                })
                .unwrap_or_default()
        });
        entries.get(&name).copied()
    }

    fn is_file(&mut self, p: &Path) -> bool {
        self.lookup(p) == Some(true)
    }
}

/// Compute a deck's closure from the vendored tree (pure reads).
pub(crate) fn closure_of(deck: &Path) -> Closure {
    let deck = lexical(deck);
    let corpus = corpus_root();
    let tree = deck.parent().expect("a deck has a parent").to_path_buf();
    let mut flat: BTreeSet<PathBuf> = BTreeSet::new();
    let mut files: BTreeSet<PathBuf> = BTreeSet::new();
    let mut outside: BTreeSet<String> = BTreeSet::new();
    let mut scanned: BTreeSet<String> = BTreeSet::new();
    let mut listings = Listings::default();
    let mut stack = vec![deck.clone()];
    while let Some(script) = stack.pop() {
        if !scanned.insert(fold(&script)) {
            continue;
        }
        let Ok(bytes) = std::fs::read(&script) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes);
        let base = script
            .parent()
            .expect("a script has a parent")
            .to_path_buf();
        for raw in text.lines() {
            let line = strip_comment(raw);
            let first = line.split_whitespace().next().unwrap_or("").to_lowercase();
            let redirects = first.starts_with("redir") || first.starts_with("comp");
            for tok in tokens(line) {
                let t = Path::new(&tok);
                let absolute =
                    t.has_root() || matches!(t.components().next(), Some(Component::Prefix(_)));
                // A lone `/` or `/m` (a division, an option) is not a path.
                let named = t
                    .components()
                    .filter(|c| matches!(c, Component::Normal(_)))
                    .count()
                    >= 2;
                if absolute && named && !is_under(&lexical(t), &corpus) {
                    outside.insert(tok.clone());
                }
                let mut p = lexical(&base.join(t));
                if !listings.is_file(&p) && t.extension().is_none() {
                    let with_ext = lexical(&base.join(format!("{tok}.dss")));
                    if listings.is_file(&with_ext) {
                        p = with_ext;
                    }
                }
                if !listings.is_file(&p) || !is_under(&p, &corpus) {
                    continue;
                }
                let script_ref = redirects || is_script(&p);
                if script_ref {
                    stack.push(p.clone());
                }
                if is_under(&p, &tree) {
                    continue;
                }
                let dir = p.parent().expect("a file has a parent").to_path_buf();
                if script_ref {
                    flat.insert(dir);
                } else {
                    files.insert(p);
                }
            }
        }
    }
    // A file whose folder is copied `flat` is carried by it.
    files.retain(|f| {
        let dir = f.parent().expect("a file has a parent");
        !flat.iter().any(|d| fold(d) == fold(dir))
    });
    let mut root = tree.clone();
    for p in flat.iter().chain(files.iter()) {
        while !is_under(p, &root) {
            root = root
                .parent()
                .expect("every closure member is under the corpus root")
                .to_path_buf();
        }
    }
    Closure {
        root,
        deck,
        tree,
        flat,
        files,
        outside,
    }
}

/// One closure per vendored deck per process (a pure function of the tree).
pub(crate) fn cached_closure(deck: &str) -> Arc<Closure> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, Arc<Closure>>>> = OnceLock::new();
    let key = fold(Path::new(deck));
    let cache = CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Some(c) = cache.lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
        return Arc::clone(c);
    }
    let c = Arc::new(closure_of(Path::new(deck)));
    cache
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(key, Arc::clone(&c));
    c
}

fn copy_file(src: &Path, dst: &Path) {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|e| panic!("scratch copy: cannot create {parent:?}: {e}"));
    }
    std::fs::copy(src, dst)
        .unwrap_or_else(|e| panic!("scratch copy: cannot copy {src:?} -> {dst:?}: {e}"));
}

/// Copy a directory's regular files (and, when `recursive`, its subtrees).
fn copy_dir(src: &Path, dst: &Path, recursive: bool) {
    std::fs::create_dir_all(dst)
        .unwrap_or_else(|e| panic!("scratch copy: cannot create {dst:?}: {e}"));
    let rd =
        std::fs::read_dir(src).unwrap_or_else(|e| panic!("scratch copy: cannot list {src:?}: {e}"));
    for entry in rd {
        let entry = entry.unwrap_or_else(|e| panic!("scratch copy: listing {src:?}: {e}"));
        let ft = entry
            .file_type()
            .unwrap_or_else(|e| panic!("scratch copy: {:?}: {e}", entry.path()));
        let to = dst.join(entry.file_name());
        if ft.is_dir() {
            if recursive {
                copy_dir(&entry.path(), &to, true);
            }
        } else if ft.is_file() {
            copy_file(&entry.path(), &to);
        }
    }
}

/// `p` below `root`, case-insensitively (the members come from the deck's own
/// spellings, which need not match the directory entries' case).
fn rel(p: &Path, root: &Path) -> PathBuf {
    let n = root.components().count();
    assert!(
        is_under(p, root),
        "scratch copy: {p:?} is not under its closure root {root:?}"
    );
    p.components().skip(n).collect()
}

/// A process-unique run nonce: the process tag (pid + start time, so a stale
/// directory left by an earlier process with a recycled pid cannot collide)
/// plus a counter.
fn run_nonce() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    static TAG: OnceLock<u32> = OnceLock::new();
    let tag = *TAG.get_or_init(|| {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        (u64::from(std::process::id()).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ nanos) as u32
    });
    format!("{tag:08x}-{:x}", COUNTER.fetch_add(1, Ordering::Relaxed))
}

/// One producer's run of one case, in its own fresh copy.
pub(crate) struct ScratchCopy {
    run_dir: PathBuf,
    deck: String,
    producer: &'static str,
    removed: bool,
}

impl ScratchCopy {
    /// Create `<scratch>/<case key>/<producer>/<nonce>/` (which must not exist
    /// yet — `create_dir`, never `create_dir_all`, on the last level) and copy
    /// the case's closure into it. The case key is the 16-hex digest of the
    /// folded vendored deck path (`engines::case_digest`): a readable key
    /// would push the deepest decks' copies past `MAX_PATH`, which neither
    /// oracle engine opts out of.
    pub(crate) fn new(case_path: &str, producer: &'static str) -> ScratchCopy {
        let closure = cached_closure(case_path);
        let parent = scratch_root()
            .join(format!(
                "{:016x}",
                crate::engines::case_digest(&fold(&closure.deck))
            ))
            .join(producer);
        std::fs::create_dir_all(&parent)
            .unwrap_or_else(|e| panic!("scratch copy: cannot create {parent:?}: {e}"));
        let run_dir = parent.join(run_nonce());
        std::fs::create_dir(&run_dir).unwrap_or_else(|e| {
            panic!(
                "scratch copy: the run directory {run_dir:?} for {case_path} ({producer}) \
                 could not be created fresh: {e}. A run directory is never reused — a \
                 pre-existing one is a nonce collision or a leftover, not a copy to \
                 run in."
            )
        });
        let root = &closure.root;
        copy_dir(&closure.tree, &run_dir.join(rel(&closure.tree, root)), true);
        for d in &closure.flat {
            copy_dir(d, &run_dir.join(rel(d, root)), false);
        }
        for f in &closure.files {
            copy_file(f, &run_dir.join(rel(f, root)));
        }
        let deck = run_dir
            .join(rel(&closure.deck, root))
            .to_string_lossy()
            .replace('\\', "/");
        ScratchCopy {
            run_dir,
            deck,
            producer,
            removed: false,
        }
    }

    /// The deck path the producer compiles (forward-slashed, like
    /// `manifest::corpus_file`).
    pub(crate) fn deck(&self) -> &str {
        &self.deck
    }

    /// The run directory (the copy's root).
    pub(crate) fn run_dir(&self) -> &Path {
        &self.run_dir
    }

    /// Remove the copy within the budget. `Err` names the producer and what is
    /// still there — the case fails with it.
    pub(crate) fn remove(mut self) -> Result<(), String> {
        self.removed = true;
        remove_run_dir(&self.run_dir).map_err(|left| {
            format!(
                "the `{}` producer's scratch copy {:?} survived {} removal attempts \
                 {:?} apart; still there: {}. A handle held for the producer's \
                 lifetime (an unclosed trace/report file, a working directory \
                 left inside the copy) blocks the removal — fix the producer, \
                 never widen the budget (RETRO_FIXES RF-I00-01, the D32(2) rail).",
                self.producer,
                self.run_dir,
                COPY_REMOVE_ATTEMPTS + 1,
                COPY_REMOVE_PAUSE,
                left.join(", "),
            )
        })
    }

    /// [`Self::remove`] for callers that fail by panicking.
    pub(crate) fn finish(self) {
        if let Err(e) = self.remove() {
            panic!("{e}");
        }
    }
}

impl Drop for ScratchCopy {
    /// An early exit (a failed case, a panic) still removes the copy, quietly:
    /// the case already fails for its own reason, and a panic here during an
    /// unwind would abort the process.
    fn drop(&mut self) {
        if !self.removed && remove_run_dir(&self.run_dir).is_err() {
            eprintln!(
                "scratch copy: {:?} ({}) left behind after a failed run",
                self.run_dir, self.producer
            );
        }
    }
}

/// Remove `dir` with the bounded retry; `Err` lists (up to 10) entries still
/// present. "Gone" is proven by a FRESH listing of the parent plus a
/// `NotFound` lookup, never by `Path::exists` alone: a delete-pending entry is
/// still enumerated while a handle is open on it.
pub(crate) fn remove_run_dir(dir: &Path) -> Result<(), Vec<String>> {
    let gone = || {
        let name = dir.file_name().map(|n| n.to_os_string());
        let listed = dir
            .parent()
            .and_then(|p| std::fs::read_dir(p).ok())
            .map(|rd| rd.flatten().any(|e| Some(e.file_name()) == name))
            .unwrap_or(true);
        !listed
            && matches!(
                std::fs::symlink_metadata(dir),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound
            )
    };
    for attempt in 0..=COPY_REMOVE_ATTEMPTS {
        if attempt > 0 {
            std::thread::sleep(COPY_REMOVE_PAUSE);
        }
        let _ = std::fs::remove_dir_all(dir);
        if gone() {
            return Ok(());
        }
    }
    let mut left = Vec::new();
    list_rel(dir, dir, &mut left);
    if left.is_empty() {
        left.push(".".to_string());
    }
    left.truncate(10);
    Err(left)
}

fn list_rel(dir: &Path, root: &Path, out: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        out.push(
            p.strip_prefix(root)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/"),
        );
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            list_rel(&p, root, out);
        }
    }
}

/// Run `f` on a fresh copy of `case_path` made for `producer`, then remove the
/// copy — panicking, naming the producer, when it survives the budget. A panic
/// inside `f` still removes the copy (quietly) before it propagates.
pub(crate) fn in_copy<T>(case_path: &str, producer: &'static str, f: impl FnOnce(&str) -> T) -> T {
    let copy = ScratchCopy::new(case_path, producer);
    let out = f(copy.deck());
    copy.finish();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_root(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "dss_scratch_{tag}_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    /// Every run gets its own fresh directory: two copies of one case for one
    /// producer never share a directory, both hold the deck, and each is gone
    /// after its removal.
    #[test]
    fn two_runs_of_one_case_get_two_fresh_copies_and_both_are_removed() {
        let deck = crate::manifest::corpus_file("Test/AutoTrans/Auto3bus.dss");
        let a = ScratchCopy::new(&deck, PORT);
        let b = ScratchCopy::new(&deck, PORT);
        assert_ne!(a.run_dir(), b.run_dir(), "a run directory is never reused");
        for c in [&a, &b] {
            assert!(
                Path::new(c.deck()).is_file(),
                "{:?} holds the deck",
                c.run_dir()
            );
            assert!(
                c.run_dir().starts_with(scratch_root()),
                "the copy lives under the gate's scratch root, never in tests/corpus"
            );
        }
        let (da, db) = (a.run_dir().to_path_buf(), b.run_dir().to_path_buf());
        a.finish();
        b.finish();
        assert!(!da.exists() && !db.exists());
    }

    /// A run directory must not pre-exist: `create_dir` on the last level is
    /// what makes a stale or colliding directory loud instead of a copy that
    /// silently carries another run's files.
    #[test]
    fn a_run_directory_that_already_exists_is_refused() {
        let root = fixture_root("fresh");
        let dir = root.join("nonce");
        std::fs::create_dir(&dir).unwrap();
        let again = std::fs::create_dir(&dir).expect_err("create_dir refuses an existing dir");
        assert_eq!(again.kind(), std::io::ErrorKind::AlreadyExists);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The D32(2) rail on its new footing: a created file the producer still
    /// holds open the FPC/Delphi way (`FILE_SHARE_READ` only — deletion NOT
    /// shared, winnt.h) outlives the whole budget and fails the case naming
    /// the producer and the file; released afterwards, the copy goes.
    #[cfg(windows)]
    #[test]
    fn a_copy_a_producer_still_holds_fails_naming_the_producer() {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 0x0000_0001;
        let deck = crate::manifest::corpus_file("Test/AutoTrans/Auto3bus.dss");
        let copy = ScratchCopy::new(&deck, "capi_v0145");
        let dir = copy.run_dir().to_path_buf();
        let trace = Path::new(copy.deck()).parent().unwrap().join("STOR_s1.CSV");
        std::fs::write(&trace, b"hour,t\n").unwrap();
        let held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(&trace)
            .expect("open the created file the way an engine holds its trace");
        let err = copy.remove().expect_err("a held file blocks the removal");
        assert!(
            err.contains("`capi_v0145` producer") && err.contains("STOR_s1.CSV"),
            "the failure names the producer and the survivor: {err}"
        );
        drop(held);
        remove_run_dir(&dir).expect("released, the copy goes");
    }

    /// A holder that lets go INSIDE the budget (a scan of a just-closed file,
    /// a worker exiting after its reply) is waited out, not reported.
    #[cfg(windows)]
    #[test]
    fn a_holder_released_within_the_budget_is_waited_out() {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 0x0000_0001;
        let root = fixture_root("transient");
        let dir = root.join("run");
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("EXP_Y.CSV"), b"y\n").unwrap();
        let held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(dir.join("EXP_Y.CSV"))
            .unwrap();
        let releaser = std::thread::spawn(move || {
            std::thread::sleep(COPY_REMOVE_PAUSE * 3);
            drop(held);
        });
        let res = remove_run_dir(&dir);
        releaser.join().unwrap();
        assert_eq!(
            res,
            Ok(()),
            "released inside the budget, nothing is reported"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A deck whose `../` references leave its folder carries them into the
    /// copy at the same relative place, and the copy compiles like the tree
    /// (no port error): `IEEE13Nodeckt.dss` redirects `../IEEELineCodes.DSS`.
    #[test]
    fn a_parent_reference_resolves_inside_the_copy() {
        let deck =
            crate::manifest::corpus_file("Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss");
        let c = cached_closure(&deck);
        assert!(
            fold(&c.root).ends_with("/version8/distrib/ieeetestcases"),
            "closure root {:?}",
            c.root
        );
        in_copy(&deck, PORT, |copied| {
            let codes = Path::new(copied)
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("IEEELineCodes.DSS");
            assert!(codes.is_file(), "{codes:?} is carried into the copy");
            let mut dss = dss_core::exec::Dss::new();
            dss.command(&format!("compile \"{copied}\""));
            assert!(
                dss.errors().is_empty() && dss.circuit().is_some(),
                "the copy compiles clean: {:?}",
                dss.errors()
            );
        });
    }

    /// The closures that reach outside their case folder, pinned by count and
    /// by their largest root: a new deck that climbs further (or a scanner
    /// change that stops seeing a reference) moves this population and must be
    /// looked at, not absorbed.
    #[test]
    fn the_external_closures_are_the_pinned_population() {
        let mut decks: Vec<String> = crate::manifest::load_solvable()
            .into_iter()
            .map(|c| crate::manifest::corpus_file(&c.path))
            .collect();
        for fam in crate::manifest::FAMILIES {
            for c in crate::manifest::load_family(fam.name) {
                decks.push(crate::manifest::family_file(fam.name, &c.path));
            }
        }
        let corpus = corpus_root();
        let mut roots: BTreeMap<String, usize> = BTreeMap::new();
        let mut outside: BTreeSet<String> = BTreeSet::new();
        for d in &decks {
            let c = cached_closure(d);
            outside.extend(c.outside.iter().cloned());
            if !c.flat.is_empty() || !c.files.is_empty() {
                *roots
                    .entry(fold(rel(&c.root, &corpus).as_path()))
                    .or_default() += 1;
            }
        }
        assert_eq!(decks.len(), 526, "the manifest population");
        assert_eq!(
            roots
                .iter()
                .map(|(r, n)| (r.as_str(), *n))
                .collect::<Vec<_>>(),
            EXTERNAL_CLOSURE_ROOTS,
            "decks whose closure leaves their folder, per closure root"
        );
        assert_eq!(
            outside.into_iter().collect::<Vec<_>>(),
            OUTSIDE_REFERENCES,
            "references that leave tests/corpus/ (kept verbatim in the copy)"
        );
    }

    /// Measured 2026-09-26 on `e5f48d53` (RF-I00-01 part 1, `part_1.md`).
    const EXTERNAL_CLOSURE_ROOTS: &[(&str, usize)] = &[
        ("electricdss-tst", 1),
        ("electricdss-tst/version8/distrib", 39),
        (
            "electricdss-tst/version8/distrib/examples/invertermodels/pvsystem/invcontrol",
            11,
        ),
        ("electricdss-tst/version8/distrib/ieeetestcases", 14),
    ];
    const OUTSIDE_REFERENCES: &[&str] = &["C:\\Users\\prdu001\\OpenDSS\\Source\\DESS1\\Dess1.DLL"];
}

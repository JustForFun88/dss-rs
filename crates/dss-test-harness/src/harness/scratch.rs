//! Per-run scratch copies.
//!
//! The corpus gate never runs a producer inside `tests/corpus/`: every
//! (case, producer) run gets a FRESH directory
//! `<target>/corpus-scratch/<case key>/<producer>/<run nonce>/`, the case's
//! directory closure ([`Closure`]) is copied into it, the producer compiles the
//! COPY's deck, and the copy is removed afterwards within one bounded budget
//! ([`COPY_REMOVE_ATTEMPTS`] x [`COPY_REMOVE_PAUSE`]). A copy that outlives the
//! budget fails the case naming its producer. For the port, whose engine lives
//! in the gate's own process, that is the engine-leak rail: a handle the engine
//! still holds blocks the removal. An oracle producer's process is gone before its copy
//! is removed (a pooled worker is recycled after every case unless
//! `DSS_GATE_RECYCLE_AFTER` > 1, a one-shot exits), so its lifetime handles,
//! the dss_capi Storage `DebugTrace` class among them, are released by then:
//! for the two oracle channels the leak rail stays the transport's
//! `sweep_failed` report (`runner::assert_swept_clean`), and
//! `engines::transport_cwd_tests` keep a worker alive to prove the step back
//! out of the copy.
//!
//! Why: case directories nest (`Test/` holds `Test/AutoTrans/`) and cases run
//! concurrently, so in ONE shared tree a recursive snapshot would read a
//! sibling case's report without `FILE_SHARE_DELETE` and red as a "leaked
//! dropping". Three producers of one case get three copies, so an engine leak
//! cannot poison the next producer and no order-coupling can hide a gap.
//!
//! Every surface is read from the copy the producer ran in: the transports'
//! created-file set is the listing diff against the copy's initial listing
//! (their guards snapshot the copy before the run), the CONTENTS and the
//! demand-interval tree are copied out to the gate's sidecars before the reply,
//! and the sidecars stay keyed by the VENDORED deck path
//! (`engines::build_run_request`'s `key`), so their location and bytes do not
//! depend on the copy.
//!
//! Both oracle engines leave the process working directory in the deck's
//! folder after a compile (r4133 `Executive/ExecHelper.pas:752-754`,
//! `SetCurrentDir(CurrDir)`; dss_capi `SetCurrentDSSDir`), and Windows refuses
//! to remove a directory that is some process's working directory, so both
//! transports step back to their startup directory before they reply
//! (`tools/oracle/oracle_server.py::main`, `dss-epri`'s `epri-worker`).
//!
//! Shared: the corpus gate re-exports this module (`corpus_gate/scratch.rs`,
//! which keeps the rails), every test binary whose decks write into the tree
//! compiles a copy made here (`di_pins`, `run_files_pins`,
//! `run_file_contents_pins`, `props_r4133_pins`), and `dss-epri`'s IEEE13 smoke
//! and mode walk make theirs with `dss_epri::smoke::Ieee13Copy`. All of them remove
//! their copies within the one budget of `dss_epri::guard`
//! ([`COPY_REMOVE_ATTEMPTS`] x [`COPY_REMOVE_PAUSE`]). "No test writes under
//! `tests/corpus/`" is a MEASURED property, not a structural one: the lib unit
//! tests and several pin binaries compile vendored decks that carry no writing
//! verb in place, read-only, nothing refuses a plain in-place compile, and
//! [`TreePhoto`] sees a writer only while it overlaps the corpus gate's own
//! walk.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// The producer tag of the port's own runs (the two oracle channels use their
/// channel tags, `capi_v0145` and `r4133`).
pub const PORT: &str = "port";

/// The copy-removal budget, 25 x 200 ms: `dss_epri::guard`'s one pair of
/// named constants (documented there), shared with `dss-epri`'s own IEEE13
/// copies so the two crates cannot drift apart.
pub use dss_epri::guard::{COPY_REMOVE_ATTEMPTS, COPY_REMOVE_PAUSE};

/// `<target>/corpus-scratch`, derived from the test binary's own path
/// (`<target>/<profile>/deps/<bin>`) like `engines::gate_scratch_root`, so it
/// follows `CARGO_TARGET_DIR` and a lane's `target` junction. Never under
/// `tests/corpus/`.
pub fn scratch_root() -> PathBuf {
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
pub fn corpus_root() -> PathBuf {
    lexical(
        &[env!("CARGO_MANIFEST_DIR"), "..", "..", "tests", "corpus"]
            .iter()
            .collect::<PathBuf>(),
    )
}

/// `.` and `..` resolved on the path's own components, never through the
/// filesystem: `canonicalize` answers a `\\?\` verbatim path, in which the two
/// DSS engines would stop resolving a deck's own `..` references.
pub fn lexical(p: &Path) -> PathBuf {
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
pub fn fold(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/").to_lowercase()
}

pub fn is_under(p: &Path, dir: &Path) -> bool {
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
/// the sparse closures move under a gigabyte per lane, and a member a closure missed would show as a
/// port compile error that no manifest `expect_warnings` covers
/// (`runner::assert_expected_warnings`), never as a silent change.
#[derive(Debug, Clone)]
pub struct Closure {
    pub root: PathBuf,
    pub deck: PathBuf,
    pub tree: PathBuf,
    pub flat: BTreeSet<PathBuf>,
    pub files: BTreeSet<PathBuf>,
    pub outside: BTreeSet<String>,
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
pub fn closure_of(deck: &Path) -> Closure {
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
                if !listings.is_file(&p) {
                    continue;
                }
                if !is_under(&p, &corpus) {
                    // A RELATIVE reference that climbs out of `tests/corpus/`
                    // cannot be carried into a copy (there it would resolve
                    // under the scratch root, not at its vendored target), so
                    // it is listed like an absolute one, for the pinned
                    // population to see, instead of being skipped in silence.
                    if !absolute {
                        outside.insert(tok.clone());
                    }
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
pub fn cached_closure(deck: &str) -> Arc<Closure> {
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
pub fn rel(p: &Path, root: &Path) -> PathBuf {
    let n = root.components().count();
    assert!(
        is_under(p, root),
        "scratch copy: {p:?} is not under its closure root {root:?}"
    );
    p.components().skip(n).collect()
}

/// FNV-1a 64 of a (folded) case path, spelled out: the key must be stable
/// across processes, which `DefaultHasher` does not promise. The scratch
/// copies' directory key, and (through `corpus_gate/engines.rs`'s
/// `case_digest`) the corpus gate's `case_key`.
pub fn case_digest(folded: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in folded.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
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
pub struct ScratchCopy {
    run_dir: PathBuf,
    deck: String,
    /// The vendored deck the copy was made of, below `tests/corpus/`: every
    /// message about the copy names it next to the full scratch path (a digest
    /// is never inverted by hand).
    case: String,
    producer: &'static str,
    removed: bool,
    loud: bool,
}

impl ScratchCopy {
    /// Create `<scratch>/<case key>/<producer>/<nonce>/` (which must not exist
    /// yet — `create_dir`, never `create_dir_all`, on the last level) and copy
    /// the case's closure into it. The case key is the 16-hex digest of the
    /// folded vendored deck path ([`case_digest`]): a readable key
    /// would push the deepest decks' copies past `MAX_PATH`, which neither
    /// oracle engine opts out of.
    pub fn new(case_path: &str, producer: &'static str) -> ScratchCopy {
        Self::new_in(case_path, producer, run_nonce())
    }

    /// [`Self::new`] with the run nonce given: the seam the fresh-directory
    /// rail (`scratch::tests::a_run_directory_that_already_exists_is_refused`)
    /// drives with a nonce it has already used, so a pre-existing run directory
    /// is refused by this very code, not by a stand-in.
    pub fn new_in(case_path: &str, producer: &'static str, nonce: String) -> ScratchCopy {
        let closure = cached_closure(case_path);
        let parent = scratch_root()
            .join(format!("{:016x}", case_digest(&fold(&closure.deck))))
            .join(producer);
        std::fs::create_dir_all(&parent)
            .unwrap_or_else(|e| panic!("scratch copy: cannot create {parent:?}: {e}"));
        let run_dir = parent.join(nonce);
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
        // The case as the manifest spells it (below `tests/corpus/`), so a
        // message names it readably next to the digest-keyed copy.
        let corpus = corpus_root();
        let case = if is_under(&closure.deck, &corpus) {
            rel(&closure.deck, &corpus)
        } else {
            closure.deck.clone()
        };
        ScratchCopy {
            run_dir,
            deck,
            case: case.to_string_lossy().replace('\\', "/"),
            producer,
            removed: false,
            loud: false,
        }
    }

    /// The deck path the producer compiles (forward-slashed, like
    /// `manifest::corpus_file`).
    pub fn deck(&self) -> &str {
        &self.deck
    }

    /// The run directory (the copy's root).
    pub fn run_dir(&self) -> &Path {
        &self.run_dir
    }

    /// Remove the copy within the budget. `Err` names the producer, the case,
    /// the copy and what is still there — the case fails with it.
    pub fn remove(mut self) -> Result<(), String> {
        self.removed = true;
        remove_run_dir(&self.run_dir).map_err(|left| self.survived(&left))
    }

    /// Make the drop LOUD, for a copy that lives in a struct field next to the
    /// engine it feeds (declared after it, so it drops after it): outside an
    /// unwind, a copy that survives its budget panics exactly as
    /// [`Self::finish`] would; during one it only reports.
    pub fn loud(mut self) -> Self {
        self.loud = true;
        self
    }

    fn survived(&self, left: &[String]) -> String {
        format!(
            "the `{}` producer's scratch copy {:?} of {} survived {} removal \
             attempts {:?} apart; still there: {}. A handle held for the \
             producer's lifetime (an unclosed trace/report file, a working \
             directory left inside the copy) blocks the removal — fix the \
             producer, never widen the budget (the D32(2) rail).",
            self.producer,
            self.run_dir,
            self.case,
            COPY_REMOVE_ATTEMPTS + 1,
            COPY_REMOVE_PAUSE,
            left.join(", "),
        )
    }

    /// [`Self::remove`] for callers that fail by panicking.
    pub fn finish(self) {
        if let Err(e) = self.remove() {
            panic!("{e}");
        }
    }
}

impl Drop for ScratchCopy {
    /// An early exit (a failed case, a panic) still removes the copy, quietly:
    /// the case already fails for its own reason, and a panic here during an
    /// unwind would abort the process. A [`ScratchCopy::loud`] copy dropped
    /// outside an unwind panics instead.
    fn drop(&mut self) {
        if self.removed {
            return;
        }
        if let Err(left) = remove_run_dir(&self.run_dir) {
            let msg = self.survived(&left);
            if self.loud && !std::thread::panicking() {
                panic!("{msg}");
            }
            eprintln!("scratch copy left behind after a failed run: {msg}");
        }
    }
}

/// Remove `dir` within the shared budget; `Err` lists (up to 10) entries
/// still present (`dss_epri::guard::remove_dir_within_budget`, one
/// implementation for every copy).
pub fn remove_run_dir(dir: &Path) -> Result<(), Vec<String>> {
    dss_epri::guard::remove_dir_within_budget(dir)
}

/// Run `f` on a fresh copy of `case_path` made for `producer`, then remove the
/// copy — panicking, naming the producer, when it survives the budget. A panic
/// inside `f` still removes the copy (quietly) before it propagates.
pub fn in_copy<T>(case_path: &str, producer: &'static str, f: impl FnOnce(&str) -> T) -> T {
    let copy = ScratchCopy::new(case_path, producer);
    let out = f(copy.deck());
    copy.finish();
    out
}

/// `p` itself when it lies OUTSIDE the vendored corpus ([`corpus_root`]);
/// panics naming `who` when it lies inside it.
///
/// Guard reuse on the copy: the three corpus guards, the port's `RunFileProbe` and the two oracle transports keep
/// their snapshot / classify / sweep / `sweep_failed` jobs, but only ever on a
/// scratch copy. The gate-side entry points that hand a directory to one of
/// them call this (`runner::CorpusGuard::new`, `run_files::RunFileProbe::start`,
/// `engines::build_run_request`, which names the transports' guard directory,
/// and `runner::run_and_compare_abort`, which builds its own request), and so
/// does `props_r4133_pins`'s `Deck::compile_inner`, so a vendored path reaching
/// a guard is refused before anything is photographed, run or swept — the
/// structural half of "no producer runs inside `tests/corpus/`" for those
/// entry points. [`TreePhoto`] around the corpus gate's own walk is the
/// measured half. A plain in-place compile elsewhere is refused by neither.
#[track_caller]
pub fn not_vendored<'a>(p: &'a Path, who: &str) -> &'a Path {
    let abs = lexical(&std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf()));
    assert!(
        !is_under(&abs, &corpus_root()),
        "{who}: {} lies inside the vendored corpus {}. Every producer runs in a \
         fresh scratch copy (`harness::scratch::ScratchCopy`) \
         and every corpus guard brackets that copy, never the vendored tree — run \
         the deck through a copy instead of pointing the guard at `tests/corpus/`.",
        p.display(),
        corpus_root().display(),
    );
    p
}

/// A photograph of a directory tree: the root itself (`"."`) and every entry
/// below it (relative, forward-slashed), each with its kind, length and
/// modification time.
///
/// The read-only-tree rail: `scheduler::run_gate` photographs `tests/corpus/` before its first case and
/// after its last, and `GateRun::assert_complete` fails the gate on any
/// difference; a writer in another test process is seen only while it
/// overlaps that window. A producer that swept what it wrote still moves its folder's
/// mtime, and a vendored file rewritten with its own bytes still moves its own,
/// so a writer that cleaned up after itself is caught as surely as one that
/// left a dropping. Each entry is read through its OWN handle
/// (`symlink_metadata`): a `DirEntry`'s metadata on Windows is the parent
/// index's copy of the times, which NTFS updates lazily, so two photographs of
/// an untouched fixture could differ that way. A link is photographed as itself and
/// never followed out of the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreePhoto(BTreeMap<String, (bool, u64, Option<std::time::SystemTime>)>);

impl TreePhoto {
    /// Walk `root`. A listing or metadata failure panics: a photograph with a
    /// hole in it cannot prove "unchanged".
    pub fn take(root: &Path) -> TreePhoto {
        let md = std::fs::metadata(root)
            .unwrap_or_else(|e| panic!("tree photograph: cannot stat {root:?}: {e}"));
        let mut map = BTreeMap::new();
        map.insert(".".to_string(), (true, 0, md.modified().ok()));
        let mut stack = vec![(root.to_path_buf(), String::new())];
        while let Some((dir, prefix)) = stack.pop() {
            let rd = std::fs::read_dir(&dir)
                .unwrap_or_else(|e| panic!("tree photograph: cannot list {dir:?}: {e}"));
            for entry in rd {
                let entry = entry.unwrap_or_else(|e| panic!("tree photograph: {dir:?}: {e}"));
                let name = entry.file_name().to_string_lossy().into_owned();
                let rel = if prefix.is_empty() {
                    name
                } else {
                    format!("{prefix}/{name}")
                };
                let md = std::fs::symlink_metadata(entry.path())
                    .unwrap_or_else(|e| panic!("tree photograph: cannot stat {rel}: {e}"));
                let is_dir = md.is_dir();
                let len = if is_dir { 0 } else { md.len() };
                map.insert(rel.clone(), (is_dir, len, md.modified().ok()));
                if is_dir {
                    stack.push((entry.path(), rel));
                }
            }
        }
        TreePhoto(map)
    }

    /// How many entries (the root included) the photograph holds.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Every entry whose presence, kind, length or mtime differs between this
    /// photograph and `after`, in path order: `+path` appeared, `-path`
    /// vanished, `~path` changed (a folder's `~` = an entry was created or
    /// removed in it).
    pub fn changes(&self, after: &TreePhoto) -> Vec<String> {
        let mut out = Vec::new();
        for (path, was) in &self.0 {
            match after.0.get(path) {
                None => out.push(format!("-{path}")),
                Some(now) if now != was => out.push(format!("~{path}")),
                Some(_) => {}
            }
        }
        for path in after.0.keys() {
            if !self.0.contains_key(path) {
                out.push(format!("+{path}"));
            }
        }
        out.sort_by(|a, b| a[1..].cmp(&b[1..]));
        out
    }
}

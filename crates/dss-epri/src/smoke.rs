//! Oracle-free self-smoke for the r4133 bridge (`UNIFIED_GATE_PLAN.md` §2.4-1,
//! replacing `tools/opendss/smoke.py`): DLL loads, version matches
//! `revisions.json`, IEEE13 compiles + solves + converges, the CSC export is
//! solution-neutral (`YNodeVarray` bit-identical before/after
//! `InitAndGetYparams`/`GetCompressedYMatrix`), and the injection vector has the
//! `2*(NumNodes+1)` shape. Shared by the `epri-worker --smoke` mode and the
//! `smoke_*` integration `#[test]` so `cargo test --workspace` exercises it with
//! no oracle installed.
//!
//! The DLL compiles a fresh scratch copy of the vendored IEEE13 deck
//! ([`Ieee13Copy`], RETRO_FIXES RF-I00-01), never `tests/corpus/` itself.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::dss::{Engine, EngineError};

/// The vendored r4133 DLL. `DSS_EPRI_DLL` overrides; default is relative to the
/// crate (git-tracked at `tools/opendss/bin/r4133/OpenDSSDirect.dll`).
pub fn dll_path() -> PathBuf {
    if let Ok(p) = std::env::var("DSS_EPRI_DLL") {
        return PathBuf::from(p);
    }
    workspace_root().join("tools/opendss/bin/r4133/OpenDSSDirect.dll")
}

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = <root>/crates/dss-epri (baked at build time).
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// The r4133 `expect_version` substring: `DSS_EPRI_EXPECT` overrides, else read
/// from `tools/opendss/revisions.json`.
pub fn expect_version() -> Result<String, String> {
    if let Ok(v) = std::env::var("DSS_EPRI_EXPECT") {
        return Ok(v);
    }
    let p = workspace_root().join("tools/opendss/revisions.json");
    let text = std::fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", p.display()))?;
    v.get("r4133")
        .and_then(|r| r.get("expect_version"))
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("r4133.expect_version missing/empty in {}", p.display()))
}

/// The vendored IEEE13 deck, below the workspace root.
const IEEE13_REL: &str =
    "tests/corpus/electricdss-tst/Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss";

/// A fresh scratch copy of the vendored IEEE13 deck for one r4133 run
/// (RETRO_FIXES RF-I00-01): the DLL compiles the COPY, never `tests/corpus/`.
///
/// The closure is the `13Bus` folder plus the files directly in
/// `IEEETestCases/` (the folder's own `IEEELineCodes.DSS` redirects
/// `../IEEELineCodes.DSS`) — what `closure_of` in
/// `crates/dss-core/tests/harness/scratch.rs` computes for this deck. The
/// engine leaves the process working directory in the copy's deck folder after
/// a compile (r4133 `Executive/ExecHelper.pas:752-754`), and Windows refuses to
/// remove a process's working directory, so the removal first steps back to
/// the directory the copy was made from, then runs within the shared budget
/// ([`crate::guard::remove_dir_within_budget`]).
pub struct Ieee13Copy {
    run_dir: PathBuf,
    deck: String,
    home: PathBuf,
    removed: bool,
}

impl Ieee13Copy {
    /// Copy the closure into a fresh `<temp>/dss-epri-scratch/ieee13-<nonce>/`,
    /// which must not exist yet.
    pub fn new() -> Result<Ieee13Copy, String> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let home = std::env::current_dir()
            .map_err(|e| format!("IEEE13 scratch copy: no working directory: {e}"))?;
        let master = workspace_root().join(IEEE13_REL);
        let bus13 = master.parent().expect("the deck has a folder");
        let cases = bus13.parent().expect("13Bus has a parent");
        let parent = std::env::temp_dir().join("dss-epri-scratch");
        std::fs::create_dir_all(&parent).map_err(|e| {
            format!(
                "IEEE13 scratch copy: cannot create {}: {e}",
                parent.display()
            )
        })?;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let run_dir = parent.join(format!(
            "ieee13-{}-{nanos:x}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&run_dir).map_err(|e| {
            format!(
                "IEEE13 scratch copy: the run directory {} could not be created fresh: {e}",
                run_dir.display()
            )
        })?;
        let deck = run_dir.join("IEEETestCases/13Bus/IEEE13Nodeckt.dss");
        // From here on an early `Err` drops `copy`, which removes the partial copy.
        let copy = Ieee13Copy {
            deck: deck.to_string_lossy().replace('\\', "/"),
            run_dir,
            home,
            removed: false,
        };
        let to_cases = copy.run_dir.join("IEEETestCases");
        copy_dir(cases, &to_cases, false)?;
        copy_dir(bus13, &to_cases.join("13Bus"), true)?;
        if !deck.is_file() {
            return Err(format!("IEEE13 master missing: {}", master.display()));
        }
        Ok(copy)
    }

    /// The deck path the engine compiles (forward-slashed).
    pub fn deck(&self) -> &str {
        &self.deck
    }

    /// Step back to the directory the copy was made from, then remove the copy
    /// within the shared budget; `Err` names the producer, the vendored deck,
    /// the copy and what is still there.
    pub fn remove(mut self) -> Result<(), String> {
        self.removed = true;
        self.remove_now()
    }

    fn remove_now(&self) -> Result<(), String> {
        std::env::set_current_dir(&self.home).map_err(|e| {
            format!(
                "IEEE13 scratch copy: cannot step back to {}: {e}",
                self.home.display()
            )
        })?;
        crate::guard::remove_dir_within_budget(&self.run_dir).map_err(|left| {
            format!(
                "the `r4133` producer's scratch copy {} of {IEEE13_REL} survived {} \
                 removal attempts {:?} apart; still there: {}. A handle the engine \
                 still holds blocks the removal — fix the producer, never widen the \
                 budget (RETRO_FIXES RF-I00-01).",
                self.run_dir.display(),
                crate::guard::COPY_REMOVE_ATTEMPTS + 1,
                crate::guard::COPY_REMOVE_PAUSE,
                left.join(", ")
            )
        })
    }
}

impl Drop for Ieee13Copy {
    /// An early exit still removes the copy, quietly: the run already fails
    /// for its own reason.
    fn drop(&mut self) {
        if self.removed {
            return;
        }
        if let Err(e) = self.remove_now() {
            eprintln!("{e}");
        }
    }
}

/// Copy `src`'s regular files (and, when `recursive`, its subtrees) into `dst`.
fn copy_dir(src: &Path, dst: &Path, recursive: bool) -> Result<(), String> {
    std::fs::create_dir_all(dst)
        .map_err(|e| format!("IEEE13 scratch copy: cannot create {}: {e}", dst.display()))?;
    let rd = std::fs::read_dir(src)
        .map_err(|e| format!("IEEE13 scratch copy: cannot list {}: {e}", src.display()))?;
    for entry in rd {
        let entry =
            entry.map_err(|e| format!("IEEE13 scratch copy: listing {}: {e}", src.display()))?;
        let to = dst.join(entry.file_name());
        match entry.file_type() {
            Ok(t) if t.is_dir() && recursive => copy_dir(&entry.path(), &to, true)?,
            Ok(t) if t.is_file() => {
                std::fs::copy(entry.path(), &to).map_err(|e| {
                    format!(
                        "IEEE13 scratch copy: cannot copy {} -> {}: {e}",
                        entry.path().display(),
                        to.display()
                    )
                })?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// The printable smoke report (lines committed to STATUS).
pub struct SmokeReport {
    pub lines: Vec<String>,
}

/// Run the smoke checks against the r4133 DLL. Returns the report on success,
/// an error string on any failed check.
pub fn run_smoke() -> Result<SmokeReport, String> {
    let dll = dll_path();
    if !dll.is_file() {
        return Err(format!("r4133 DLL not found: {}", dll.display()));
    }
    let engine = Engine::new(&dll).map_err(|e: EngineError| e.to_string())?;
    let mut lines = Vec::new();

    // 1. version pin.
    let ver = engine.version().trim().to_string();
    let expect = expect_version()?;
    if !ver.contains(&expect) {
        return Err(format!(
            "engine version {ver:?} does not contain {expect:?}"
        ));
    }
    lines.push(format!("version OK: {ver}"));

    // 2. IEEE13 compile + solve converges — on its own scratch copy (RF-I00-01).
    let copy = Ieee13Copy::new()?;
    engine.clear().map_err(|e| e.to_string())?;
    engine
        .compile(copy.deck(), false)
        .map_err(|e| e.to_string())?;
    engine.solve(false).map_err(|e| e.to_string())?;
    if !engine.converged() {
        return Err("IEEE13 did not converge".to_string());
    }
    let n_nodes = engine.num_nodes();
    let iters = engine.iterations();
    lines.push(format!(
        "IEEE13 solved: {n_nodes} nodes, {iters} iterations"
    ));

    // 3. CSC export solution-neutral (InitAndGetYparams always factors first).
    let v0 = engine.ynode_varray();
    let ycsc = engine.y_csc().map_err(|e| e.to_string())?;
    if ycsc.n != n_nodes as usize {
        return Err(format!("CSC n={} != NumNodes {n_nodes}", ycsc.n));
    }
    let nnz = ycsc.row_idx.len();
    if nnz == 0 {
        return Err("CSC export empty".to_string());
    }
    let v1 = engine.ynode_varray();
    if v0 != v1 {
        return Err("YNodeVarray CHANGED across the CSC export — not solution-neutral".to_string());
    }
    lines.push(format!(
        "CSC export OK: n={n_nodes}, nnz={nnz}, voltages bit-identical (solution-neutral)"
    ));

    // 4. injection vector shape (getIpointer readable, length 2*(NumNodes+1)).
    let inj = engine.injection_raw(n_nodes);
    let want = 2 * (n_nodes as usize + 1);
    if inj.len() != want {
        return Err(format!("getIpointer length {} != {want}", inj.len()));
    }
    lines.push(format!(
        "getIpointer OK: len={} (= 2*(NumNodes+1))",
        inj.len()
    ));

    // 5. all-properties enumeration is readable + deterministic (§2.2):
    // `DSSElementV` (AllPropertyNames) + `? name.prop` value reads. Re-reading the
    // first element's first property directly must equal the dumped value. This
    // proves the enumeration is non-empty and the getter is deterministic (same
    // code path both times) — it is NOT a value-correctness check against an
    // independent baseline. What checks the values is the corpus gate: since
    // R4133_PROPS RP4.1 (2026-09-03) this capture is a gating channel, compared
    // cell-by-cell against the port on every live non-`large` r4133-gating case.
    // (Written when it was capability-only report tooling — fix-round audit F1.)
    let dump = crate::capture::all_properties_dump(&engine).map_err(|e| e.to_string())?;
    if dump.is_empty() {
        return Err("all_properties dump is empty (no elements enumerated)".to_string());
    }
    let first = &dump[0];
    let (p0, v0) = first
        .props
        .first()
        .ok_or_else(|| format!("all_properties: {} has no properties", first.element))?;
    let direct = engine.raw_command(&format!("? {}.{}", first.element, p0));
    if &direct != v0 {
        return Err(format!(
            "all_properties round-trip mismatch on {}.{p0}: dump {v0:?} != direct {direct:?}",
            first.element
        ));
    }
    let total: usize = dump.iter().map(|p| p.props.len()).sum();
    lines.push(format!(
        "all_properties OK: {} elements, {total} property values, dump non-empty + getter re-read consistent",
        dump.len()
    ));

    // The engine goes first, then its copy: a copy that survives the budget
    // fails the smoke naming the producer.
    drop(engine);
    copy.remove()?;
    Ok(SmokeReport { lines })
}

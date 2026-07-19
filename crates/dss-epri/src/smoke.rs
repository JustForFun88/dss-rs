//! Oracle-free self-smoke for the r4133 bridge (`UNIFIED_GATE_PLAN.md` §2.4-1,
//! replacing `tools/opendss/smoke.py`): DLL loads, version matches
//! `revisions.json`, IEEE13 compiles + solves + converges, the CSC export is
//! solution-neutral (`YNodeVarray` bit-identical before/after
//! `InitAndGetYparams`/`GetCompressedYMatrix`), and the injection vector has the
//! `2*(NumNodes+1)` shape. Shared by the `epri-worker --smoke` mode and the
//! `smoke_*` integration `#[test]` so `cargo test --workspace` exercises it with
//! no oracle installed.

use std::path::PathBuf;

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

fn ieee13_case() -> String {
    workspace_root()
        .join("tests/corpus/electricdss-tst/Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss")
        .to_string_lossy()
        .replace('\\', "/")
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

    // 2. IEEE13 compile + solve converges.
    let ieee13 = ieee13_case();
    if !PathBuf::from(&ieee13).is_file() {
        return Err(format!("IEEE13 master missing: {ieee13}"));
    }
    engine.clear().map_err(|e| e.to_string())?;
    engine.compile(&ieee13, false).map_err(|e| e.to_string())?;
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

    // 5. all-properties enumeration round-trips (§2.2 report-tooling parity):
    // `DSSElementV` (AllPropertyNames) + `? name.prop` value reads. Re-reading the
    // first element's first property directly must equal the dumped value.
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
        "all_properties OK: {} elements, {total} property values, round-trip verified",
        dump.len()
    ));

    Ok(SmokeReport { lines })
}

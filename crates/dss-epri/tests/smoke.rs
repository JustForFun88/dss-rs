//! Oracle-free smoke `#[test]` for the r4133 bridge — wired into
//! `cargo test --workspace` (UNIFIED_GATE_PLAN.md §2.4-1 DONE bar). Loads the
//! git-tracked r4133 DLL, asserts the version pin, compiles + solves IEEE13,
//! proves the CSC export is solution-neutral, and checks the injection shape.
//! Needs no oracle installed.

#[cfg(windows)]
#[test]
fn epri_r4133_smoke() {
    let rep = dss_epri::run_smoke().unwrap_or_else(|e| panic!("r4133 smoke failed: {e}"));
    for l in &rep.lines {
        eprintln!("smoke: {l}");
    }
    assert!(
        rep.lines.iter().any(|l| l.starts_with("version OK")),
        "missing version check: {:?}",
        rep.lines
    );
    assert!(
        rep.lines.iter().any(|l| l.contains("IEEE13 solved")),
        "missing solve check: {:?}",
        rep.lines
    );
    assert!(
        rep.lines.iter().any(|l| l.contains("solution-neutral")),
        "missing CSC-neutrality check: {:?}",
        rep.lines
    );
    assert!(
        rep.lines.iter().any(|l| l.contains("getIpointer OK")),
        "missing injection-shape check: {:?}",
        rep.lines
    );
}

#[cfg(not(windows))]
#[test]
fn epri_r4133_smoke_unavailable() {
    // The EPRI bridge is Windows-only; nothing to smoke on other platforms.
}

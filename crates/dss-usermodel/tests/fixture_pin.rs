//! Hash-vs-PIN scaffold for the committed `.wasm` fixtures (plan §2.6):
//! the fixture is a golden-class artifact regenerated MANUALLY ONLY with the
//! pinned toolchain; this test asserts the committed file's SHA-256 matches
//! the hash recorded in `tools/wasm_usermodel/PIN.txt` — drift is red.
//!
//! Originally a self-activating scaffold (WP-WM.1 item 3) with a dormant
//! pre-WM.2 arm (no fixture + no pin = pass). WM.2 committed the fixture, so
//! the artifact is now PERMANENT: both the fixture and its PIN line must
//! exist — a simultaneous deletion of both is red, not a silent pass
//! (audit finding WM-T4, settled 2026-07-19).
//!
//! PIN line format (WM.2):
//! `sha256(tests/fixtures/wasm/indmach012a.wasm)=<64 hex digits>`

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Repo root relative to this crate (`crates/dss-usermodel`).
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// Extract the pinned hash for `fixture_name` from PIN.txt, if present.
fn pinned_hash(pin_text: &str, fixture_name: &str) -> Option<String> {
    for line in pin_text.lines() {
        let line = line.trim();
        if line.starts_with('#') || !line.contains(fixture_name) || !line.contains("sha256") {
            continue;
        }
        let (_, value) = line.rsplit_once('=')?;
        let value = value.trim();
        if value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Some(value.to_ascii_lowercase());
        }
    }
    None
}

/// Assert a committed `.wasm` fixture's SHA-256 matches its `PIN.txt` line
/// (both halves must exist — WM-T4). A permanent pinned artifact (plan §2.6).
fn assert_fixture_matches_pin(fixture_name: &str) {
    let root = repo_root();
    let fixture = root
        .join("tests")
        .join("fixtures")
        .join("wasm")
        .join(fixture_name);
    let pin_path = root.join("tools").join("wasm_usermodel").join("PIN.txt");

    let pin_text = std::fs::read_to_string(&pin_path)
        .unwrap_or_else(|e| panic!("PIN.txt must exist at {}: {e}", pin_path.display()));

    let want = pinned_hash(&pin_text, fixture_name).unwrap_or_else(|| {
        panic!(
            "PIN.txt has no sha256 line for {fixture_name} — the fixture is a \
             permanent pinned artifact; restore the pin"
        )
    });
    let bytes = std::fs::read(&fixture)
        .unwrap_or_else(|e| panic!("committed fixture {} must exist: {e}", fixture.display()));
    let got = format!("{:x}", Sha256::digest(&bytes));
    assert_eq!(
        got,
        want,
        "committed fixture {} does not match tools/wasm_usermodel/PIN.txt — \
         fixtures regenerate MANUALLY ONLY with the pinned toolchain (plan §2.6/§2.9-4)",
        fixture.display()
    );
}

#[test]
fn committed_fixture_hash_matches_pin() {
    // WM.2 IndMach012a (Generator user model).
    assert_fixture_matches_pin("indmach012a.wasm");
}

#[test]
fn committed_wm4model_fixture_hash_matches_pin() {
    // WM.4 wm4model (Storage DynaDLL + PVSystem UserModel).
    assert_fixture_matches_pin("wm4model.wasm");
}

#[test]
fn committed_capuserctl_fixture_hash_matches_pin() {
    // WM.5 capuserctl (CapControl UserModel — deadband voltage control).
    assert_fixture_matches_pin("capuserctl.wasm");
}

#[test]
fn pinned_hash_parser_reads_the_wm2_line_format() {
    let text = "\
# comment sha256(tests/fixtures/wasm/indmach012a.wasm)=0000000000000000000000000000000000000000000000000000000000000000
wasmi==1.0.9
sha256(tests/fixtures/wasm/indmach012a.wasm)=A1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90
";
    assert_eq!(
        pinned_hash(text, "indmach012a.wasm").as_deref(),
        Some("a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90")
    );
    assert_eq!(pinned_hash("wasmi==1.0.9\n", "indmach012a.wasm"), None);
}

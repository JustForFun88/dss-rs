//! Hash-vs-PIN scaffold for the committed `.wasm` fixtures (plan §2.6):
//! the fixture is a golden-class artifact regenerated MANUALLY ONLY with the
//! pinned toolchain; this test asserts the committed file's SHA-256 matches
//! the hash recorded in `tools/wasm_usermodel/PIN.txt` — drift is red.
//!
//! Self-activating scaffold (WP-WM.1 item 3): WM.2 commits the fixture and
//! appends its hash line to the PIN; until then neither exists and the test
//! verifies exactly that dormant state (no `#[ignore]` — an inconsistent
//! half-state, fixture without pin or pin without fixture, fails loudly).
//!
//! Expected PIN line format (WM.2):
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

#[test]
fn committed_fixture_hash_matches_pin() {
    const FIXTURE_NAME: &str = "indmach012a.wasm";
    let root = repo_root();
    let fixture = root
        .join("tests")
        .join("fixtures")
        .join("wasm")
        .join(FIXTURE_NAME);
    let pin_path = root.join("tools").join("wasm_usermodel").join("PIN.txt");

    let pin_text = std::fs::read_to_string(&pin_path)
        .unwrap_or_else(|e| panic!("PIN.txt must exist at {}: {e}", pin_path.display()));
    let pinned = pinned_hash(&pin_text, FIXTURE_NAME);

    match (fixture.exists(), pinned) {
        // Dormant state (pre-WM.2): no fixture, no pin — consistent.
        (false, None) => {}
        // Active state: byte hash must match the PIN exactly.
        (true, Some(want)) => {
            let bytes = std::fs::read(&fixture).expect("read committed fixture");
            let got = format!("{:x}", Sha256::digest(&bytes));
            assert_eq!(
                got,
                want,
                "committed fixture {} does not match tools/wasm_usermodel/PIN.txt — \
                 fixtures regenerate MANUALLY ONLY with the pinned toolchain (plan §2.6/§2.9-4)",
                fixture.display()
            );
        }
        (true, None) => panic!(
            "fixture {} is committed but has no sha256 line in PIN.txt — pin it",
            fixture.display()
        ),
        (false, Some(_)) => panic!(
            "PIN.txt pins {FIXTURE_NAME} but the fixture is missing at {}",
            fixture.display()
        ),
    }
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

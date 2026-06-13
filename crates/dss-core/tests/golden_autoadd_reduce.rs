//! WP6.8 (3/4 + 4/4) gate: replay `tests/golden/autoadd_reduce.json` (generated
//! by `tools/golden/gen_autoadd_reduce.py` against the pinned oracle) and match
//! the AutoAdd / circuit-reduction option surface byte-for-byte:
//!
//!   - every `Get` echo (float `%-g`, `IntArrayToString`, AddType device word);
//!   - `Set addtype=<unknown>` -> CAPADD default, no error;
//!   - `Set ueregs=(10 abc 13)` -> `[10, 0, 0]` + a logged conversion error
//!     (oracle #303), the audit-hardened `parseIntArray` behavior;
//!   - `13.7 -> 14` decimal rounding;
//!   - `Reduce` #1890 (no meter) and #262 (named meter missing, uppercased).
//!
//! `Bus.Keep` (`MarkCapandReactorBuses`) and the reduction work itself are not
//! exposed by the dss-python COM API, so they stay pinned by the `exec` unit
//! tests; this gate covers everything the oracle *does* expose.

use std::path::PathBuf;

use dss_core::exec::Dss;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Golden {
    schema: u32,
    scenarios: Vec<Scenario>,
}

#[derive(Debug, Deserialize)]
struct Scenario {
    name: String,
    commands: Vec<String>,
    gets: Vec<GetExpect>,
    /// The oracle's `DoSimpleMsg` error number (for documentation; the Rust port
    /// surfaces the message text, not the number).
    #[allow(dead_code)]
    error_number: Option<i32>,
    /// Substring the Rust port must surface in its error log; `None` means the
    /// scenario must run clean.
    error_contains: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetExpect {
    query: String,
    result: String,
}

fn load_golden() -> Golden {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "golden",
        "autoadd_reduce.json",
    ]
    .iter()
    .collect();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let g: Golden = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("cannot parse {}: {e}", path.display()));
    assert_eq!(g.schema, 1, "autoadd_reduce golden schema mismatch");
    g
}

fn run_scenario(sc: &Scenario) {
    let mut dss = Dss::new();
    dss.command("clear");
    for cmd in &sc.commands {
        dss.command(cmd);
    }

    match &sc.error_contains {
        None => assert!(
            dss.errors().is_empty(),
            "{}: expected a clean run, got {:?}",
            sc.name,
            dss.errors()
        ),
        Some(needle) => assert!(
            dss.errors().iter().any(|m| m.contains(needle)),
            "{}: expected an error containing {:?}, got {:?}",
            sc.name,
            needle,
            dss.errors()
        ),
    }

    for g in &sc.gets {
        dss.command(&format!("Get {}", g.query));
        assert_eq!(
            dss.result(),
            g.result,
            "{}: Get {} mismatch",
            sc.name,
            g.query
        );
    }
}

#[test]
fn autoadd_reduce_surface_matches_oracle() {
    let golden = load_golden();
    assert_eq!(golden.scenarios.len(), 10, "expected 10 scenarios");
    for sc in &golden.scenarios {
        run_scenario(sc);
    }
}

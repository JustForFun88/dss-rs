//! NCIM report goldens (UPGRADE_PLAN WP-U1.7 tail): pin `Export Jacobian` /
//! `Export deltaF` / `Export deltaZ` / `Show PV2PQ_Conversions` after a
//! `Set Algorithm=NCIM` solve against the **capi015** oracle (dss_capi 0.15.0b4;
//! the pinned 0.15.7 gate oracle has no NCIM). Goldens + decks:
//! `tools/golden/gen_ncim_reports.py` on the capi015 venv → `tests/golden/ncim/`.
//!
//! Gate design (the report-gating convention):
//!  * **Jacobian** — numeric-token compare: `Row,Col` exact, `Value` within a
//!    tight abs tol (the Jacobian is built from node voltages pinned to <5e-11 in
//!    `exec/tests/ncim.rs`, so its entries match capi015 to ~1e-9);
//!  * **deltaF / deltaZ** — structural: the vectors are the converged
//!    mismatch/correction at the ~1e-11 faer-vs-KLU floor, so their per-line
//!    *values* are noise and cannot be value-pinned. We assert the exact line
//!    count (`2·NumNodes` + PV-phase rows), the six leading swing zeros, and that
//!    every entry parses finite and sits at the converged magnitude;
//!  * **PV2PQ** — byte-exact (CRLF→LF normalized) text.

use std::path::PathBuf;

use dss_core::exec::Dss;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Meta {
    deck: Vec<String>,
    n_nodes: usize,
    delta_len: usize,
}

fn golden_dir() -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "golden",
        "ncim",
    ]
    .iter()
    .collect()
}

fn scratch_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("dss_ncimrep_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap_or_else(|e| panic!("mkdir {}: {e}", d.display()));
    d
}

/// A `Row,Col,Value` Jacobian CSV → `(row, col, value)` triplets (skips header).
fn parse_jacobian(text: &str) -> Vec<(u64, u64, f64)> {
    text.lines()
        .filter(|l| !l.is_empty() && !l.starts_with("Row,"))
        .map(|l| {
            let mut it = l.split(',');
            let r = it.next().unwrap().trim().parse().unwrap();
            let c = it.next().unwrap().trim().parse().unwrap();
            let v = it.next().unwrap().trim().parse().unwrap();
            (r, c, v)
        })
        .collect()
}

fn run(stem: &str) {
    let dir = golden_dir();
    let meta: Meta = {
        let p = dir.join(format!("{stem}.meta.json"));
        let t = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&t).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };

    let scratch = scratch_dir(stem);
    let mut dss = Dss::new();
    for c in &meta.deck {
        dss.command(c);
        assert!(
            dss.errors().is_empty(),
            "{stem}: `{c}` -> {:?}",
            dss.errors()
        );
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));

    // --- Export Jacobian: numeric-token compare (row/col exact, value tol). ---
    dss.command("Export Jacobian");
    assert!(
        dss.errors().is_empty(),
        "{stem}: Export Jacobian {:?}",
        dss.errors()
    );
    let produced = dss.last_result_file().to_string();
    assert!(
        produced.to_lowercase().ends_with("jacobian.csv"),
        "{stem}: unexpected Jacobian path {produced:?}"
    );
    let rust_jac = std::fs::read_to_string(&produced)
        .unwrap_or_else(|e| panic!("{stem}: read {produced}: {e}"));
    let golden_jac = std::fs::read_to_string(dir.join(format!("{stem}_Jacobian.csv"))).unwrap();
    assert_eq!(
        rust_jac.lines().next(),
        Some("Row,Col,Value"),
        "{stem}: Jacobian header"
    );
    let (rj, gj) = (parse_jacobian(&rust_jac), parse_jacobian(&golden_jac));
    assert_eq!(
        rj.len(),
        gj.len(),
        "{stem}: Jacobian nnz (rust {}, capi015 {})",
        rj.len(),
        gj.len()
    );
    for (i, ((rr, rc, rv), (gr, gc, gv))) in rj.iter().zip(gj.iter()).enumerate() {
        assert_eq!((rr, rc), (gr, gc), "{stem}: Jacobian entry {i} row/col");
        // Value: abs-or-rel 1e-6 (entries are O(1..3); node-V floor is ~5e-11).
        let tol = 1e-6_f64.max(1e-7 * gv.abs());
        assert!(
            (rv - gv).abs() <= tol,
            "{stem}: Jacobian[{gr},{gc}] rust {rv} vs capi015 {gv} (tol {tol:.1e})"
        );
    }

    // --- Export deltaF / deltaZ: structural gate. ---
    for (rep, want_end) in [("deltaF", "deltaf.csv"), ("deltaZ", "deltaz.csv")] {
        dss.command(&format!("Export {rep}"));
        assert!(
            dss.errors().is_empty(),
            "{stem}: Export {rep} {:?}",
            dss.errors()
        );
        let p = dss.last_result_file().to_string();
        assert!(
            p.to_lowercase().ends_with(want_end),
            "{stem}: unexpected {rep} path {p:?}"
        );
        let body = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{stem}: read {p}: {e}"));
        let vals: Vec<f64> = body
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| {
                l.trim()
                    .parse()
                    .unwrap_or_else(|_| panic!("{stem}: {rep} non-numeric {l:?}"))
            })
            .collect();
        assert_eq!(
            vals.len(),
            meta.delta_len,
            "{stem}: {rep} line count (2·NumNodes+PVrows = {})",
            meta.delta_len
        );
        // The first six rows are the swing bus — forced to exactly zero.
        for (i, &v) in vals.iter().take(6).enumerate() {
            assert_eq!(v, 0.0, "{stem}: {rep} swing row {i} must be 0, got {v}");
        }
        // Converged: every entry is at the mismatch/correction floor. Empirically
        // the observed max over both decks is ~2.2e-10 (deltaZ, pq); the 1e-8 bound
        // keeps ~45x headroom over that faer floor yet tightens 100x from the old
        // 1e-6 so a systematic ~1e-8-scale offset (wrong-ordering/sign-away-from-
        // swing bugs stay at the ~1e-10 noise floor and are inherently invisible to
        // a magnitude gate on genuine cancellation noise — the values cannot be
        // value-pinned per the CLAUDE.md cancellation-floor rule) is caught.
        let mx = vals.iter().fold(0.0_f64, |m, &v| m.max(v.abs()));
        assert!(
            mx < 1e-8,
            "{stem}: {rep} not at converged floor (max {mx:.2e})"
        );
    }

    // --- Show PV2PQ_Conversions: byte-exact (CRLF→LF normalized). ---
    dss.command("Show PV2PQ_Conversions");
    assert!(
        dss.errors().is_empty(),
        "{stem}: Show PV2PQ {:?}",
        dss.errors()
    );
    let p = dss.last_result_file().to_string();
    assert!(
        p.to_lowercase().ends_with("pv2pq_generators.csv"),
        "{stem}: unexpected PV2PQ path {p:?}"
    );
    let rust_pv = std::fs::read_to_string(&p).unwrap().replace("\r\n", "\n");
    let golden_pv = std::fs::read_to_string(dir.join(format!("{stem}_PV2PQ.txt")))
        .unwrap()
        .replace("\r\n", "\n");
    assert_eq!(rust_pv, golden_pv, "{stem}: Show PV2PQ text");

    // Sanity: n_nodes recorded in the meta matches the solved model.
    let solved = dss.circuit().map(|c| c.num_nodes).unwrap_or(0);
    assert_eq!(solved, meta.n_nodes, "{stem}: node count");

    std::fs::remove_dir_all(&scratch).ok();
}

/// PQ-only micro feeder (9 nodes, 3 iters): all model-1 loads, no PV bus, so the
/// PV2PQ list is empty (banner only).
#[test]
fn ncim_reports_pq() {
    run("pq");
}

/// PV-bus generator hitting its +Q limit (6 nodes, 8 iters): `Generator.g1`
/// converts PV→PQ, so it appears in the PV2PQ list.
#[test]
fn ncim_reports_pv_qlimit() {
    run("pv_qlimit");
}

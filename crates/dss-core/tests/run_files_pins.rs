//! **GOLDEN_REBASE G1.10a — the expected-value pins behind the run-file SET
//! surface** (`compare_run_files`: every filesystem entry a run creates under
//! the case directory, compared as an exact discrete set at `rel = abs = 0`).
//!
//! Three facts about that surface are settled by something other than "the two
//! sides matched", and CLAUDE.md wants each of them pinned by an expected-value
//! test that names BOTH numbers:
//!
//! * the **`Visualize` DSSView pair** — the one `tests/corpus/ledger.json` row
//!   this sub-step lands (`r4133-visualize-writes-a-dssview-file-pair`), a
//!   deliberate product divergence rather than an upstream defect;
//! * the **D25/Q2 engine-scratch split** — a structural normalization applied
//!   symmetrically to both channels, with **zero** ledger rows and a
//!   fail-on-stale population (`scheduler::SCRATCH_FILE_DECLINES`);
//! * the **cross-oracle case fold** — the normalization that makes the two
//!   oracles' spellings one member, without which every second deck would
//!   "diverge" on capitalization alone.
//!
//! Each pin drives the **gate-side** comparator
//! (`harness::run_files::compare_run_files`) against the oracle sets measured on
//! the named channel, and takes the port's side from the port itself — the very
//! `RunFileProbe` the corpus gate uses, over the vendored corpus deck (never
//! `.inputs/`, CLAUDE.md). Every oracle number below was measured by the G1.10a
//! probes and re-measured by the live gate: `tmp/g110a/oracle_diff.txt` (the
//! 14-deck capi↔r4133 diff), `tmp/g110a/f_F1_capi_files.json` (pinned
//! dss-python 0.15.7 / dss_capi 0.14.5), `tmp/g110a/f_F0_r4133_files.json` (the
//! vendored EPRI r4133 DLL through `crates/dss-epri`) and
//! `tmp/g110a/f_F3_full_drive.log` (the full 526-case default-lane drive).
//!
//! **Windows-only**, like the surface: the created-file classification the three
//! producers share lives in `dss_epri::guard`, and `dss-epri` is
//! `#[cfg(windows)]` (the vendored EPRI binary is a Win64 DLL).
#![cfg(windows)]

mod harness;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use dss_core::exec::Dss;
use harness::run_files::{RunFileProbe, compare_run_files};

/// The vendored corpus path of a deck, exactly as the corpus gate addresses it
/// (`corpus_gate::scheduler`'s `abs`): the deck FILE, whose parent is the case
/// directory the guard snapshots.
fn corpus_deck(rel: &str) -> PathBuf {
    let deck: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        rel,
    ]
    .iter()
    .collect();
    assert!(deck.is_file(), "vendored corpus deck missing: {deck:?}");
    deck
}

/// Run one corpus deck the way `corpus_gate::runner::run_rust_capture` does
/// (`clear` → `compile`, then one `solve` per checkpoint) inside the gate's own
/// run-file probe, and return `(created set, the Visualize payloads the deck
/// fired)`.
///
/// The probe classifies what the run created and then **removes it**, so the
/// vendored corpus directory is left exactly as it was found — the same
/// contract `RunFileProbe` keeps inside the gate.
fn port_run_files(rel: &str, n_steps: usize) -> (Vec<String>, Vec<String>) {
    let deck = corpus_deck(rel);
    let case_path = deck.to_string_lossy().to_string();
    let plots: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let probe = RunFileProbe::start(&case_path);
    let mut dss = Dss::new();
    {
        let sink = Arc::clone(&plots);
        dss.register_plot_callback(move |json: &str| {
            sink.lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(json.to_string());
            0
        });
    }
    dss.command("clear");
    dss.command(&format!("compile \"{}\"", deck.display()));
    assert!(dss.errors().is_empty(), "{rel}: {:?}", dss.errors());
    for _ in 0..n_steps {
        dss.command("solve");
    }
    assert!(dss.errors().is_empty(), "{rel}: {:?}", dss.errors());
    // Drop the engine BEFORE the probe reads, exactly as the runner does: a file
    // the engine still held open would fail its removal.
    drop(dss);
    let created = probe.finish_and_clean(&format!("run_files_pins:{rel}"));
    let fired = plots.lock().unwrap_or_else(|e| e.into_inner()).clone();
    (created, fired)
}

fn v(names: &[&str]) -> Vec<String> {
    names.iter().map(|s| (*s).to_string()).collect()
}

fn nothing_excluded(_: &str) -> bool {
    false
}

/// Run `f` and return its panic message — a pin that claims "this would fail"
/// must show the failure it means, not merely that something failed.
fn panic_message(f: impl FnOnce()) -> String {
    let payload =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).expect_err("the arm must panic");
    if let Some(m) = payload.downcast_ref::<&str>() {
        (*m).to_string()
    } else if let Some(m) = payload.downcast_ref::<String>() {
        m.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

/// **The `Visualize` ledger row's both-numbers pin — r4133 creates SIX entries
/// on `Test/YgD-Test.dss`, the port and dss_capi 0.14.5 create FOUR, and the two
/// extra ones are a DSSView viewer file pair the port answers with a JSON
/// callback payload instead.**
///
/// The deck's last-but-one line is `Visualize powers Transformer.TR1`
/// (`tests/corpus/electricdss-tst/Test/YgD-Test.dss:25`).
///
/// * **r4133** dispatches `DoVisualizeCmd`
///   (`Version8/Source/Executive/ExecHelper.pas:3672`) to
///   `TDSSPlot.DoVisualizationPlot` (`Version8/Source/Plot/DSSPlot.pas:3642`),
///   which names `<OutputDirectory><CircuitName_><Class>_<Name>_PQ.DSV`
///   (`:3728` + `:3746`) and calls `MakeNewGraph`
///   (`:3758` → `Version8/Source/Plot/DSSGraph.pas:109`), which `Rewrite`s that
///   `.DSV` (`:114-115`) and `FileCreate`s its binary twin
///   `ChangeFileExt(…, '.dbl')` (`:125`, `:128`) — hence a PAIR.
/// * **dss_capi 0.14.5** (`src/Executive/ExecHelper.pas:4099`) builds a
///   `{PlotType, ElementName, ElementType, Quantity}` JSON object
///   (`:4184-4189`) and hands it to `DSSPlotCallback`, opening no file.
/// * **the port** follows the JSON payload
///   (`crates/dss-core/src/exec/command.rs:865-968` →
///   `exec/plot.rs:405-421`) — asserted here, so "the port writes no pair"
///   can never quietly become "the port ignores `Visualize`".
///
/// Both numbers, measured: oracle (r4133) **6**, port **4**, capi_v0145 **4**.
/// The gate compares the four common names on both channels and excludes only
/// the two `_pq` ones, on `r4133` only
/// (`tests/corpus/ledger.json` → `r4133-visualize-writes-a-dssview-file-pair`).
#[test]
fn visualize_writes_a_dssview_pair_on_r4133_and_a_json_payload_in_the_port() {
    // Measured on the vendored EPRI r4133 DLL (`tmp/g110a/f_F0_r4133_files.json`,
    // raw spellings `testYgD_Curr_Elem.Txt`, `testYgD_Transformer_tr1_PQ.DSV`, …
    // — the comparator's ASCII fold is what makes them these members).
    let r4133 = v(&[
        "testygd_curr_elem.txt",
        "testygd_curr_seq.txt",
        "testygd_transformer_tr1_pq.dbl",
        "testygd_transformer_tr1_pq.dsv",
        "testygd_vll.txt",
        "testygd_vln_node.txt",
    ]);
    // Measured on the pinned dss-python 0.15.7 / dss_capi 0.14.5
    // (`tmp/g110a/f_F1_capi_files.json`).
    let capi = v(&[
        "testygd_curr_elem.txt",
        "testygd_curr_seq.txt",
        "testygd_vll.txt",
        "testygd_vln_node.txt",
    ]);
    let (port, plots) = port_run_files("electricdss-tst/Test/YgD-Test.dss", 1);

    assert_eq!(r4133.len(), 6, "the r4133 created-file SET is six entries");
    assert_eq!(capi.len(), 4, "the capi_v0145 created-file SET is four");
    assert_eq!(
        port, capi,
        "the port's created-file SET on YgD-Test.dss must be exactly the four \
         Show outputs — the same set dss_capi 0.14.5 creates. It is the port \
         side of the ledger row's both-numbers record (r4133 6, port 4)."
    );

    // The port answers `Visualize` with the capi JSON payload, not with silence.
    assert_eq!(
        plots.len(),
        1,
        "the deck's one `Visualize powers Transformer.TR1` must fire exactly one \
         plot payload; fired {plots:?}"
    );
    let payload = &plots[0];
    for want in [
        "\"PlotType\":\"Visualize\"",
        // Lowercased, as upstream stores it (`Name := AnsiLowerCase(TransfName)`,
        // dss_capi `src/PDElements/Transformer.pas:827`), although the deck
        // writes `Transformer.TR1`.
        "\"ElementName\":\"tr1\"",
        "\"ElementType\":\"Transformer\"",
        "\"Quantity\":\"Power\"",
    ] {
        assert!(
            payload.contains(want),
            "the Visualize payload must carry {want}; got {payload}"
        );
    }

    // The capi channel needs no ledger row: the sets are equal there.
    compare_run_files(
        "capi_v0145",
        Some(&capi),
        &port,
        &nothing_excluded,
        "pin:YgD-Test.dss",
    );

    // The r4133 channel WITHOUT the exclusion fails, naming both missing names —
    // the divergence the ledger row records is real and is what the row buys.
    let msg = panic_message(|| {
        compare_run_files(
            "r4133",
            Some(&r4133),
            &port,
            &nothing_excluded,
            "pin:YgD-Test.dss",
        );
    });
    assert!(
        msg.contains("testygd_transformer_tr1_pq.dbl")
            && msg.contains("testygd_transformer_tr1_pq.dsv")
            && msg.contains("missing (the oracle created it, the port did not)"),
        "the un-excluded r4133 compare must name the DSSView pair: {msg}"
    );

    // …and WITH the row's `name_re` it is clean — while the four Show outputs
    // stay compared, which is the whole point of a field-by-field exclusion.
    let pair = |name: &str| name.starts_with("testygd_transformer_tr1_pq.");
    compare_run_files("r4133", Some(&r4133), &port, &pair, "pin:YgD-Test.dss");
    let msg = panic_message(|| {
        let mut short = port.clone();
        short.retain(|n| n != "testygd_vll.txt");
        compare_run_files("r4133", Some(&r4133), &short, &pair, "pin:YgD-Test.dss");
    });
    assert!(
        msg.contains("testygd_vll.txt"),
        "the exclusion must not widen past the `_pq` pair: {msg}"
    );
}

/// **The D25/Q2 engine-scratch pin — on `Run_NEV.dss` an oracle reports SEVEN
/// created entries and the port SIX, and the seventh is the Pascal engines'
/// harmonics disk round-trip, split off BOTH sides instead of being triaged.**
///
/// `SavePresentVoltages` writes `<OutputDirectory><CircuitName_>SavedVoltages.dbl`
/// (r4133 `Version8/Source/Common/Utilities.pas:1512-1521`, reached only from
/// `InitializeForHarmonics` `:1599-1608`, read back by `RetrieveSavedVoltages`
/// `:1554-1564`); dss_capi 0.14.5 is the twin (`src/Common/Utilities.pas:883`,
/// `:914-923`). The port keeps that vector in memory
/// (`crates/dss-core/src/solution/solution/state.rs`,
/// `solution/solution/harmonics.rs`) and writes no such file.
///
/// Both numbers, measured (`tmp/g110a/f_F1_capi_files.json`): the capi_v0145
/// channel creates **7** — `line_quad-1_yprim.txt`, `nev_exp_y.csv`,
/// `nev_exp_yprim.csv`, `nev_faultstudy.txt`, `nev_savedvoltages.dbl`,
/// `nev_systemy.txt`, `nev_vln_node.txt` — of which **1** is the scratch name,
/// so **6** are compared; the r4133 channel creates the same seven
/// (`tmp/g110a/f_F0_r4133_files.json`, raw `NEV_SavedVoltages.dbl`,
/// `NEV_EXP_Y.CSV`, …); the port creates **6**. Zero ledger rows: the split is
/// symmetric and the population it declines is pinned instead
/// (`corpus_gate::scheduler::SCRATCH_FILE_DECLINES`, fail-on-stale in both
/// directions).
#[test]
fn the_harmonics_scratch_file_is_declined_on_the_nev_deck() {
    let oracle = v(&[
        "line_quad-1_yprim.txt",
        "nev_exp_y.csv",
        "nev_exp_yprim.csv",
        "nev_faultstudy.txt",
        "nev_savedvoltages.dbl",
        "nev_systemy.txt",
        "nev_vln_node.txt",
    ]);
    let (port, _) = port_run_files(
        "electricdss-tst/Version8/Distrib/IEEETestCases/NEVTestCase/Run_NEV.dss",
        1,
    );
    assert_eq!(oracle.len(), 7, "both oracle channels create seven entries");
    assert_eq!(
        port,
        v(&[
            "line_quad-1_yprim.txt",
            "nev_exp_y.csv",
            "nev_exp_yprim.csv",
            "nev_faultstudy.txt",
            "nev_systemy.txt",
            "nev_vln_node.txt",
        ]),
        "the port creates the oracle's seven minus the harmonics scratch file — \
         six entries. If it ever writes `nev_savedvoltages.dbl`, the D25/Q2 \
         split stops being a normalization and becomes a real change."
    );
    assert_eq!(
        port.len(),
        6,
        "port 6 vs oracle 7 — the both-numbers record"
    );

    // The split is what makes the two sides equal; it needs no ledger row on
    // either channel.
    for channel in ["capi_v0145", "r4133"] {
        compare_run_files(
            channel,
            Some(&oracle),
            &port,
            &nothing_excluded,
            "pin:Run_NEV.dss",
        );
    }

    // And it is a SPLIT, not a blanket: drop a compared name and the case still
    // fails.
    let msg = panic_message(|| {
        let mut short = port.clone();
        short.retain(|n| n != "nev_exp_y.csv");
        compare_run_files(
            "capi_v0145",
            Some(&oracle),
            &short,
            &nothing_excluded,
            "pin:Run_NEV.dss",
        );
    });
    assert!(
        msg.contains("nev_exp_y.csv"),
        "the scratch split must not silence the deck's real outputs: {msg}"
    );
}

/// **The cross-oracle case-fold pin — on `Test/AutoTrans/Auto1bus.dss` the two
/// gating oracles spell the SAME nine created files differently, and the fold is
/// what makes them one set.**
///
/// The two engines disagree with each other, so there is no single "upstream
/// spelling" the port could adopt: r4133
/// `Version8/Source/Executive/ExportOptions.pas:333-356` writes `'…CSV'` upper
/// and lowercases the whole deck-supplied stem, dss_capi 0.14.5
/// `src/Executive/ExportOptions.pas:314,343,345,381,437` writes `'…csv'` lower
/// and preserves the stem. Measured raw, on this deck
/// (`tmp/g110a/oracle_diff.txt`, all nine names): capi
/// `Auto1bus_HL_current.txt` vs r4133 `auto1bus_hl_current.txt`; capi
/// `Auto1bus_LT_losses.txt` vs r4133 `auto1bus_lt_losses.txt`; and so on. The
/// port keeps capi's spelling
/// (`crates/dss-core/src/exec/report.rs`, `"EXP_VOLTAGES.csv"` etc.) —
/// recorded in `docs/upgrade/DIVERGENCES.md`, never a ledger row, because the
/// comparator's ASCII fold makes all three sides one member. NTFS is
/// case-insensitive, so nothing observable depends on it.
///
/// This deck is `kind=large_near_ideal_source` and therefore OUTSIDE the forced
/// run-file population — it is here as the measured statement of the fold, so a
/// future normalizer change cannot silently drop the case.
#[test]
fn the_two_oracle_spellings_of_auto1bus_fold_to_one_member() {
    // Raw, as each engine wrote them (`tmp/g110a/oracle_diff.txt`).
    let capi_raw = v(&[
        "Auto1bus_HL_current.txt",
        "Auto1bus_HL_losses.txt",
        "Auto1bus_HT_current.txt",
        "Auto1bus_HT_losses.txt",
        "Auto1bus_LT_current.txt",
        "Auto1bus_LT_losses.txt",
        "Auto1bus_Load_power.txt",
        "Auto1bus_Load_voltage.txt",
        "Auto1bus_noload_power.txt",
    ]);
    let r4133_raw = v(&[
        "auto1bus_hl_current.txt",
        "auto1bus_hl_losses.txt",
        "auto1bus_ht_current.txt",
        "auto1bus_ht_losses.txt",
        "auto1bus_lt_current.txt",
        "auto1bus_lt_losses.txt",
        "auto1bus_load_power.txt",
        "auto1bus_load_voltage.txt",
        "auto1bus_noload_power.txt",
    ]);
    assert_ne!(
        capi_raw, r4133_raw,
        "the two oracles' RAW spellings differ on this deck — that is the fact \
         the fold exists for"
    );
    assert_eq!(capi_raw.len(), 9);
    assert_eq!(r4133_raw.len(), 9);

    let (port, _) = port_run_files("electricdss-tst/Test/AutoTrans/Auto1bus.dss", 1);
    assert_eq!(
        port.len(),
        9,
        "the port creates the same nine reports: {port:?}"
    );

    // All three sides land on one set, and the comparator says so on both
    // channels with no exclusion at all.
    compare_run_files(
        "capi_v0145",
        Some(&capi_raw),
        &port,
        &nothing_excluded,
        "pin:Auto1bus.dss",
    );
    compare_run_files(
        "r4133",
        Some(&r4133_raw),
        &port,
        &nothing_excluded,
        "pin:Auto1bus.dss",
    );

    // The fold is a CASE fold and nothing more: a name that differs by anything
    // else still fails.
    let msg = panic_message(|| {
        let mut renamed = port.clone();
        renamed[0] = format!("{}x", renamed[0]);
        compare_run_files(
            "capi_v0145",
            Some(&capi_raw),
            &renamed,
            &nothing_excluded,
            "pin:Auto1bus.dss",
        );
    });
    assert!(
        msg.contains("extra   (the port created it, the oracle did not)"),
        "a one-character difference beyond case must still fail: {msg}"
    );
}

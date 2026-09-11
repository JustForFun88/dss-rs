//! `InShowResults` — the report-command bracket and what it suppresses
//! (GOLDEN_REBASE G1.10b F0, from the G1.10a audit finding AC-5).
//!
//! r4133 keeps the flag in its DSS-instance globals (`Common/DSSGlobals.pas:242`),
//! raises it inside `DoShowCmd` (`Executive/ShowOptions.pas:204`) and
//! `DoExportCmd` (`Executive/ExportOptions.pas:328`), lowers it at the end of both
//! (`:393` / `:512`), and raises it in `DoSaveCmd` (`Executive/ExecHelper.pas:935`)
//! without ever lowering it again. Its one ported consumer is
//! `TStorageObj.WriteTraceRecord` (`PCElements/Storage.pas:2408`), which skips the
//! record while the flag is up — so a report that reads element currents never
//! lands in the element's own debug trace.
//!
//! **Why the tests below insert a `CalcVoltageBases`.** r4133's report writers call
//! `pElem.GetCurrents(cBuffer)` *directly*, on every element, every time
//! (`Common/ExportResults.pas:404`/`:416`/`:428`/`:440`/`:563`,
//! `Common/ShowResults.pas:492`/`:572`/`:594`/`:616`), which is why the guard has
//! to exist there at all; the port's writers read the cache-aware
//! `compute_iterminal` instead (`report/export/currents.rs:58`,
//! `report/export/seq_currents.rs:32`), which recomputes only when `Iterminal` is
//! stale for this `SolutionCount`. A report straight after a `solve` therefore
//! touches no element model at all. `CalcVoltageBases` runs a zero-load snapshot
//! (`solution/solution/dispatch.rs::set_voltage_bases`, Pascal `SetVoltageBases`)
//! that advances `SolutionCount` without refreshing the Storage's terminal
//! current, so the next report *does* reach `GetTerminalCurrents` — the trace
//! site — and the guard becomes observable in the port too.

use crate::exec::Dss;

use super::storage::trace_scratch;

/// A minimal 3-phase circuit whose one Storage traces (the `Storage_price.dss`
/// skeleton of [`super::storage`], trimmed to a single bus).
fn traced_storage(scratch: &std::path::Path) -> Dss {
    let mut dss = Dss::new();
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("New circuit.t basekv=0.48 phases=3 bus1=a pu=1");
    dss.command(
        "New Storage.storage1 phases=3 bus1=a kv=0.48 pf=1 kWrated=50 %reserve=20 \
         kWhrated=500 %stored=50 state=idling debugtrace=yes model=1",
    );
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

/// Records (data lines) in `STOR_storage1.CSV` — the header is line 1.
fn trace_records(scratch: &std::path::Path) -> usize {
    let text = std::fs::read_to_string(scratch.join("STOR_storage1.CSV"))
        .expect("the trace file the `debugtrace=yes` edit created");
    text.lines().count() - 1
}

/// The files the run created, sorted, minus the trace itself.
fn other_files(scratch: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(scratch)
        .expect("scratch readable")
        .map(|e| {
            e.expect("dir entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|n| n != "STOR_storage1.CSV")
        .collect();
    v.sort();
    v
}

/// The flag the executive raises, read back through the circuit.
fn flag(dss: &Dss) -> bool {
    dss.circuit
        .as_ref()
        .expect("the circuit the commands ran on")
        .in_show_results
}

/// **Both numbers.** Three report commands that read terminal currents —
/// `Show Currents`, `Export Currents`, `Show Powers` — each preceded by the
/// `CalcVoltageBases` that makes the port's `Iterminal` cache stale (see the
/// module doc). With the `InShowResults` guard (r4133 `Storage.pas:2408`) they
/// append **0** records and the trace stays at the **2** the snapshot solve
/// wrote. Without it — measured by deleting the `sys.in_show_results` term from
/// `elements/pc/storage/trace.rs::write_trace_record` and re-running this exact
/// sequence — each of the three appends **one** `TotalCurrent` record: 2 → 3 → 4
/// → **5**.
///
/// The guard is not a mute button, and this test is not vacuous: all three
/// reports still write their files, and the `solve` at the end still appends its
/// two records.
#[test]
fn a_report_does_not_grow_a_storage_debug_trace() {
    let scratch = trace_scratch("inshowresults_report");
    let mut dss = traced_storage(&scratch);

    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let solved = trace_records(&scratch);
    assert_eq!(solved, 2, "the snapshot solve's own records");
    assert!(!flag(&dss), "no report command is running");

    for (cmd, file) in [
        ("show currents", "t_Curr_Seq.txt"),
        ("export currents", "t_EXP_CURRENTS.csv"),
        ("show powers", "t_Power_seq_kVA.txt"),
    ] {
        dss.command("calcvoltagebases");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        assert_eq!(
            trace_records(&scratch),
            solved,
            "the zero-load snapshot that stales `{cmd}`'s cache traces nothing itself"
        );

        dss.command(cmd);
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        assert_eq!(
            trace_records(&scratch),
            solved,
            "`{cmd}` reaches `GetTerminalCurrents` on the stale cache; r4133 \
             `Storage.pas:2408` keeps that read out of the trace (without the \
             guard it appends 1 record)"
        );
        assert!(
            !flag(&dss),
            "`{cmd}` must lower the flag again (`ShowOptions.pas:393`, \
             `ExportOptions.pas:512`)"
        );
        assert!(
            scratch.join(file).is_file(),
            "`{cmd}` must still write {file} — the guard suppresses the trace \
             record, never the report"
        );
    }

    assert_eq!(
        other_files(&scratch),
        vec![
            "t_Curr_Seq.txt".to_string(),
            "t_EXP_CURRENTS.csv".to_string(),
            "t_Power_seq_kVA.txt".to_string()
        ],
        "the three report files, and nothing else"
    );

    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(
        trace_records(&scratch),
        4,
        "a solve outside a report command still traces — 2 records per solve"
    );

    let _ = std::fs::remove_dir_all(&scratch);
}

/// A report that leaves through one of its early returns must lower the flag
/// too. `Show Y` before any Y matrix exists errors (#222) and returns from the
/// middle of the dispatch, where r4133's `ShowY` `Exit`s inside its own nested
/// procedure and `ShowOptions.pas:393` still runs — which is why the port's
/// bracket sits in [`Dss::do_show_cmd`] around the whole dispatch instead of at
/// Pascal's two statement positions.
///
/// **Both numbers.** The aborted `Show` writes no file and appends **0**
/// records; the `solve` after it appends the usual **2**, which a flag left
/// latched would have suppressed (the trace would have stayed at **0**).
#[test]
fn a_show_that_aborts_mid_dispatch_still_lowers_the_flag() {
    let scratch = trace_scratch("inshowresults_abort");
    let mut dss = traced_storage(&scratch);

    // Nothing has solved, so there is no Y matrix: `Show Y` errors from inside
    // the arm (`exec/report.rs`, Pascal #222 `'Y Matrix not Built.'`).
    dss.command("show y");
    assert!(
        format!("{:?}", dss.errors()).contains("Y Matrix not Built."),
        "expected the #222 abort, got {:?}",
        dss.errors()
    );
    assert!(!flag(&dss), "the aborted Show must not leave the flag up");
    assert_eq!(trace_records(&scratch), 0, "nothing has solved yet");
    assert!(
        other_files(&scratch).is_empty(),
        "the aborted Show wrote no file"
    );

    // The #222 above stays in the log, so the solve is checked for *new* errors.
    let logged = dss.errors().len();
    dss.command("solve");
    assert_eq!(dss.errors().len(), logged, "{:?}", dss.errors());
    assert_eq!(
        trace_records(&scratch),
        2,
        "the solve after the aborted Show traces normally"
    );

    let _ = std::fs::remove_dir_all(&scratch);
}

/// **The one deliberate divergence.** r4133's `DoSaveCmd` raises the flag
/// (`ExecHelper.pas:935`) and leaves through four paths (`Exit` at `:953`,
/// `:957`, `:961`, and the class-file tail `:980-981`) without ever lowering it;
/// the flag is a global seeded only in `DSSGlobals`' `initialization` (`:2044`),
/// which not even `clear` runs. So after a single `Save` the authority suppresses
/// **every** element debug-trace record for the rest of the session — a latch
/// that contradicts the balanced brackets of `Show`/`Export` and the flag's own
/// meaning ("a report is being assembled"). CLAUDE.md (2026-08-02) forbids
/// reproducing an upstream defect in any lane, so the port scopes the flag to the
/// command.
///
/// **Both numbers.** After `Save meters` the port's next solve appends its usual
/// **2** records (trace 2 → **4**); r4133 appends **0** and leaves it at **2**,
/// for that solve and every one after it. No vendored corpus deck issues `Save`
/// (GOLDEN_REBASE G1.10b §1: 0 occurrences in the reachable corpus), so neither
/// gating channel can see the difference.
#[test]
fn save_scopes_the_flag_instead_of_latching_it() {
    let scratch = trace_scratch("inshowresults_save");
    let mut dss = traced_storage(&scratch);

    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(trace_records(&scratch), 2);

    dss.command("save meters");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!(
        !flag(&dss),
        "r4133 latches the flag here (`ExecHelper.pas:935`, no clear on any of \
         its four exits); the port scopes it to the command"
    );

    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(
        trace_records(&scratch),
        4,
        "the port keeps tracing after a Save; r4133 would stay at 2 forever"
    );

    let _ = std::fs::remove_dir_all(&scratch);
}

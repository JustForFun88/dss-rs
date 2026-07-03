"""Generate the Phase-8 targeted report goldens from the pinned oracle.

Phase 8 is the reporting/output layer (Export/Show/Save/Dump). Unlike the
command-replay goldens (phase5/6/7), these pin the **report file the oracle
writes**: `gen_phase8.py` runs a small fixture on the pinned engine, issues the
`Export`/`Show`/... command, and captures the produced file's exact bytes into
`tests/golden/phase8/<report>.txt`. The Rust side (`golden_phase8.rs`) replays
the same fixture, writes its own report, and diffs the two **after parsing
numbers out** via the `compare_export` harness (PHASE8_PLAN §2.3) — never a raw
float-string diff.

WP8.1 self-test: `Export Counts` (Pascal `ExportCounts`) — a text dump of every
DSS class and its instance count. The Rust class registry is a *proper subset*
of the oracle's (only a subset of classes is ported), so the Rust file is
compared as a `RustSubsetByKey` subset of the oracle file: every ported class's
count is pinned against the oracle, the classes we do not yet register are
ignored (documented in `tests/TOLERANCE_NOTES.md`).

Usage:
    python tools/golden/gen_phase8.py            # regenerate all phase-8 goldens
Regeneration is manual and must use the exact versions in tools/golden/PIN.txt.
"""

from __future__ import annotations

import json
import shutil
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gen_checkpoints import check_pin  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = REPO_ROOT / "tests" / "golden" / "phase8"

# The Counts fixture: a tiny circuit exercising a few class counts (Line=2,
# Load=1, Vsource=1) on top of the default DSS items. Counts depend only on
# *instance counts*, not on bus names or a solve. The Rust golden test
# (`golden_phase8.rs`) reads this same deck back from the meta file, so the two
# can never drift.
COUNTS_FIXTURE = "counts8"
COUNTS_DECK = [
    f"new circuit.{COUNTS_FIXTURE} basekv=12.47 bus1=src",
    "new line.l1 bus1=src bus2=b",
    "new line.l2 bus1=b bus2=c",
    "new load.ld1 bus1=c kv=12.47 kw=100",
]


def gen_counts(d) -> None:
    """Capture the oracle's `Export Counts` for the fixture."""
    tmp = tempfile.mkdtemp(prefix="dss_gen_phase8_")
    try:
        d.Text.Command = "clear"
        for c in COUNTS_DECK:
            d.Text.Command = c
        d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
        d.Text.Command = "export counts"
        produced = Path(tmp) / f"{COUNTS_FIXTURE}_EXP_Counts.csv"
        content = produced.read_text()  # universal newlines -> LF
    finally:
        # `set datapath` moved the engine's cwd into tmp; move it out before
        # removing the dir, else Windows refuses to delete the locked cwd.
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp, ignore_errors=True)
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    (OUT_DIR / "export_counts.txt").write_text(content, newline="\n")
    meta = {"report": "Counts", "fixture": COUNTS_FIXTURE, "deck": COUNTS_DECK}
    (OUT_DIR / "export_counts.meta.json").write_text(
        json.dumps(meta, indent=2) + "\n", newline="\n"
    )
    print(f"wrote export_counts.txt ({len(content)} bytes), {content.count(chr(10))} lines")


# The solution-export fixture: the unmodified IEEE13 feeder, compiled + solved,
# then each report captured from the file the oracle writes. The Rust golden
# (`golden_phase8.rs`) compiles the same master from `tests/corpus`, replays the
# same post commands, and diffs each report via `compare_export` — so the two
# fixtures can never drift. Each tuple: (export keyword, oracle default-filename
# suffix, golden stem).
FEEDER_MASTER = "Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss"
FEEDER_POST = ["solve"]
FEEDER_REPORTS = [
    ("voltages", "EXP_VOLTAGES.csv", "export_voltages"),
    ("buscoords", "EXP_BUSCOORDS.csv", "export_buscoords"),
    ("nodenames", "EXP_NodeNames.csv", "export_nodenames"),
    ("ynodelist", "EXP_YNodeList.csv", "export_ynodelist"),
    # WP8.2 sub-step 2a — the aggregate PD/PC power exports (kW/kvar/W, no angles).
    ("powers", "EXP_POWERS.csv", "export_powers"),
    ("losses", "EXP_LOSSES.csv", "export_losses"),
    ("p_byphase", "EXP_P_BYPHASE.csv", "export_p_byphase"),
    # The MVA option (`opt=1`): the `m…` Parm2 flag → MW/Mvar headers + the extra
    # 0.001 scaling. Same default filename per ptr, so each overwrites its kVA
    # twin's file (read immediately, so no clash).
    ("powers mva", "EXP_POWERS.csv", "export_powers_mva"),
    ("p_byphase mva", "EXP_P_BYPHASE.csv", "export_p_byphase_mva"),
    # WP8.2 sub-step 2b — the symmetrical-component family (V1/V2/V0, I1/I2/I0,
    # seq powers) + %NEMA + PD ratings. All magnitude/ratio columns, no angles.
    ("seqvoltages", "EXP_SEQVOLTAGES.csv", "export_seqvoltages"),
    ("seqcurrents", "EXP_SEQCURRENTS.csv", "export_seqcurrents"),
    ("seqpowers", "EXP_SEQPOWERS.csv", "export_seqpowers"),
    # WP8.2 sub-step 2c — the per-terminal/per-conductor element exports (mag +
    # angle) + NodeOrder + Taps.
    ("currents", "EXP_CURRENTS.csv", "export_currents"),
    ("nodeorder", "EXP_NodeOrder.csv", "export_nodeorder"),
    ("elemcurrents", "EXP_ElemCurrents.csv", "export_elemcurrents"),
    ("elemvoltages", "EXP_ElemVoltages.csv", "export_elemvoltages"),
    ("elempowers", "EXP_ElemPowers.csv", "export_elempowers"),
    ("taps", "EXP_Taps.csv", "export_taps"),
    # WP8.2 sub-step 3 — the matrix/summary exports. `y triplet` is the sparse
    # `Row,Col,G,B` form (the dense form glues `+j` onto the imaginary tokens, so
    # it is not plain-CSV gate-able; its values are pinned by the checkpoint/live
    # full-Y gates). `Yprims` is the per-element primitive-Y dump. `Summary` is the
    # one-row status line (the `DateTime` col 0 is masked). `Result` dumps the
    # `@result` parser var (always `null` in the pinned PM-build oracle).
    ("y triplet", "EXP_Y.csv", "export_y_triplet"),
    ("yprims", "EXP_YPRIM.csv", "export_yprims"),
    ("summary", "EXP_Summary.csv", "export_summary"),
    ("result", "EXP_Result.csv", "export_result"),
    # WP8.2 follow-up — the remaining solution-family exports: VoltagesElements
    # (per-element terminal voltages, the by-element companion to Voltages) and the
    # raw Y-ordered node vectors YVoltages (NodeV) / YCurrents (Solution.Currents).
    ("voltageselements", "EXP_VOLTAGES_ELEM.csv", "export_voltageselements"),
    ("yvoltages", "EXP_YVoltages.csv", "export_yvoltages"),
    ("ycurrents", "EXP_YCurrents.csv", "export_ycurrents"),
]

# The IEEE 8500-Node bus/summary exports (PHASE8_PLAN §WP8.2 step 4): pin the
# same `Voltages`/`Summary`/`Counts` reports at scale (8531 nodes, 6103 devices).
# The 8500 master needs `Set Maxiterations=20` to converge (exactly as the
# `golden_ieee8500.rs` snapshot gate does). The per-element/matrix dumps are
# omitted as enormous (the established 8500-golden discipline). `Counts` sets no
# `GlobalResult` in the oracle, so its path is built from the CaseName + suffix.
IEEE8500_MASTER = "Version8/Distrib/IEEETestCases/8500-Node/Master.dss"
IEEE8500_POST = ["Set Maxiterations=20", "solve"]
IEEE8500_REPORTS = [
    ("voltages", "EXP_VOLTAGES.csv", "export8500_voltages"),
    ("summary", "EXP_Summary.csv", "export8500_summary"),
    ("counts", "EXP_Counts.csv", "export8500_counts"),
]


def gen_ieee8500_reports(d) -> None:
    """Capture the oracle's Voltages/Summary/Counts on the solved IEEE 8500 feeder."""
    master_abs = (REPO_ROOT / "tests" / "corpus" / "electricdss-tst" / IEEE8500_MASTER).resolve()
    if not master_abs.is_file():
        sys.exit(f"master not found: {master_abs}")
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tmp = tempfile.mkdtemp(prefix="dss_gen_phase8_")
    try:
        d.Text.Command = "clear"
        d.Text.Command = f'compile "{master_abs}"'
        for c in IEEE8500_POST:
            d.Text.Command = c
        case = d.ActiveCircuit.Name
        d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
        for keyword, suffix, stem in IEEE8500_REPORTS:
            d.Text.Command = f"export {keyword}"
            result = d.Text.Result  # GlobalResult; empty for `Counts`
            produced = Path(result) if result else Path(tmp) / f"{case}_{suffix}"
            content = produced.read_text()  # universal newlines -> LF
            (OUT_DIR / f"{stem}.txt").write_text(content, newline="\n")
            meta = {
                "report": keyword,
                "master": IEEE8500_MASTER,
                "post": IEEE8500_POST,
                "fixture": case,
                "suffix": suffix,
            }
            (OUT_DIR / f"{stem}.meta.json").write_text(
                json.dumps(meta, indent=2) + "\n", newline="\n"
            )
            print(f"wrote {stem}.txt ({len(content)} bytes), {content.count(chr(10))} lines")
    finally:
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp, ignore_errors=True)


# The monitor-export fixture (PHASE8_PLAN §WP8.3 step 1): the IEEE13 feeder with
# three monitors covering the distinct CSV shapes — mode 0 (general V/I: paired
# magnitude/angle, unquoted header), mode 1 (power: `S (kVA)`/`Ang` header, whose
# spaces force `CommaText` quoting), and mode 2 (transformer tap: the single
# quoted `Tap (pu)` channel). A short 3-step daily solve gives a multi-row buffer
# (exercising the record stride). `Export Monitors <name>` writes each monitor's
# own `<case>_Mon_<name>_1.csv` (the `_1` is the PM-build primary-context
# `DSS._Name`); the returned `GlobalResult` gives the exact suffix, recorded into
# the meta so the Rust filename (`export.rs`) is pinned against the oracle's.
MONITOR_POST = [
    "new monitor.m_vi element=Line.650632 terminal=1 mode=0",
    "new monitor.m_pow element=Line.650632 terminal=1 mode=1",
    "new monitor.m_tap element=Transformer.Reg1 terminal=2 mode=2",
    "set mode=daily number=3 stepsize=1h",
    "solve",
]
MONITOR_REPORTS = [
    ("monitors m_vi", "export_mon_vi"),
    ("monitors m_pow", "export_mon_pow"),
    ("monitors m_tap", "export_mon_tap"),
]


def gen_monitor_reports(d) -> None:
    """Capture the oracle's `Export Monitors` CSVs on the daily-solved IEEE13 feeder."""
    master_abs = (REPO_ROOT / "tests" / "corpus" / "electricdss-tst" / FEEDER_MASTER).resolve()
    if not master_abs.is_file():
        sys.exit(f"master not found: {master_abs}")
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tmp = tempfile.mkdtemp(prefix="dss_gen_phase8_")
    try:
        d.Text.Command = "clear"
        d.Text.Command = f'compile "{master_abs}"'
        for c in MONITOR_POST:
            d.Text.Command = c
        case = d.ActiveCircuit.Name
        d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
        for keyword, stem in MONITOR_REPORTS:
            d.Text.Command = f"export {keyword}"
            produced = Path(d.Text.Result)  # GlobalResult = the monitor's CSV path
            suffix = produced.name[len(case) + 1 :]  # strip the `<case>_` prefix
            content = produced.read_text()  # universal newlines -> LF
            (OUT_DIR / f"{stem}.txt").write_text(content, newline="\n")
            meta = {
                "report": keyword,
                "master": FEEDER_MASTER,
                "post": MONITOR_POST,
                "fixture": case,
                "suffix": suffix,
            }
            (OUT_DIR / f"{stem}.meta.json").write_text(
                json.dumps(meta, indent=2) + "\n", newline="\n"
            )
            print(f"wrote {stem}.txt ({len(content)} bytes), {content.count(chr(10))} lines")
    finally:
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp, ignore_errors=True)


# The register-dump fixtures (PHASE8_PLAN §WP8.3 step 2). No corpus deck uses these
# keywords, so they are synthesized (PHASE8_PLAN §1). Two fixtures, both daily-solved
# 3 steps so every register accumulates:
#   A) plain IEEE13 + an EnergyMeter (on Line.650632) → `Meters` + `Loads`. The meter
#      registers match the oracle to ~1e-8 (the same daily meter path `corpus_live.rs`
#      pins), so `%10.0f` is identical; `Loads` is a static field dump.
#   B) IEEE13 + a Generator + a PVSystem + a Storage on bus 675 → the DER register
#      dumps. Kept **off** the metered element: adding DER inside the metered,
#      regulated zone shifts the metered-element local power ~3e-5 rel (a real small
#      solve interaction, above the `3358.5` rounding boundary of the meter's Max kW),
#      which would straddle `%10.0f`. The DER's *own* registers (round kWh/kW) stay
#      clean, so B pins them without that meter coupling.
REGISTER_A_POST = [
    "new energymeter.em1 element=Line.650632 terminal=1",
    "set mode=daily number=3 stepsize=1h",
    "solve",
]
REGISTER_A_REPORTS = [
    ("meters", "EXP_METERS.csv", "export_meters"),
    ("loads", "EXP_LOADS.csv", "export_loads"),
]
REGISTER_B_POST = [
    # Two enabled generators (creation order g1→g2 pins the register-dump row
    # order) + a disabled g3 (pins the enabled-filter: it must NOT appear).
    "new generator.g1 bus1=675 phases=3 kv=4.16 kw=100 pf=0.9 model=1",
    "new generator.g2 bus1=675 phases=3 kv=4.16 kw=100 pf=0.9 model=1",
    "new generator.g3 bus1=675 phases=3 kv=4.16 kw=100 pf=0.9 model=1 enabled=no",
    "new pvsystem.pv1 bus1=675 phases=3 kv=4.16 kva=120 pmpp=100 irradiance=1",
    "new storage.st1 bus1=675 phases=3 kv=4.16 kwrated=50 kwhrated=100 %stored=50 state=discharging",
    "set mode=daily number=3 stepsize=1h",
    "solve",
]
REGISTER_B_REPORTS = [
    ("generators", "EXP_GENMETERS.csv", "export_generators"),
    ("pvsystem_meters", "EXP_PVMeters.csv", "export_pvsystem_meters"),
    ("storage_meters", "EXP_STORAGEMeters.csv", "export_storage_meters"),
]


def _gen_register_group(d, post, reports) -> None:
    master_abs = (REPO_ROOT / "tests" / "corpus" / "electricdss-tst" / FEEDER_MASTER).resolve()
    if not master_abs.is_file():
        sys.exit(f"master not found: {master_abs}")
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tmp = tempfile.mkdtemp(prefix="dss_gen_phase8_")
    try:
        d.Text.Command = "clear"
        d.Text.Command = f'compile "{master_abs}"'
        for c in post:
            d.Text.Command = c
        case = d.ActiveCircuit.Name
        d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
        for keyword, suffix, stem in reports:
            d.Text.Command = f"export {keyword}"
            produced = Path(d.Text.Result)  # GlobalResult = produced path
            content = produced.read_text()  # universal newlines -> LF
            (OUT_DIR / f"{stem}.txt").write_text(content, newline="\n")
            meta = {
                "report": keyword,
                "master": FEEDER_MASTER,
                "post": post,
                "fixture": case,
                "suffix": suffix,
            }
            (OUT_DIR / f"{stem}.meta.json").write_text(
                json.dumps(meta, indent=2) + "\n", newline="\n"
            )
            print(f"wrote {stem}.txt ({len(content)} bytes), {content.count(chr(10))} lines")
    finally:
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp, ignore_errors=True)


def gen_register_reports(d) -> None:
    """Capture the oracle's register/load exports on the daily-solved IEEE13 feeder."""
    _gen_register_group(d, REGISTER_A_POST, REGISTER_A_REPORTS)
    _gen_register_group(d, REGISTER_B_POST, REGISTER_B_REPORTS)


# The event/error-log dumps (PHASE8_PLAN §WP8.3 step 3). No corpus deck exports
# these, so they are synthesized (PHASE8_PLAN §1), both on the IEEE13 feeder:
#   EventLog: a daily-3 solve with `Set Log=yes` (ckt.LogEvents) + per-RegControl
#     event logging, driving a **swinging load** (a 1200 kW load on the regulated
#     bus 675 with a 1×/2×/0.5× day shape) so the regulators actually **move
#     taps** each step. The log is then the **full** LogThisEvent solve-marker
#     stream (bus-def reprocess, meter-zone reset, Yprim recalc, Y build,
#     per-iteration + control markers, Solution Done) **plus** every regulator
#     `AppendToEventLog` tap-change line (`CHANGED n TAPS TO <pu>` — non-integer
#     tap values, so the numeric-tolerance compare path is genuinely exercised).
#     Our engine wires all these LogThisEvent call sites (the Circuit-build ones
#     landed in WP8.3 step 3a) and matches the oracle's tap decisions line-for-line
#     (the phase5 `daily_ieee13` gate pins the same tap logic). This golden pins
#     the **export** (SaveToFile → `EXP_EventLog.csv` naming + the `%g` render
#     into the file) on top of the marker + tap-change stream.
#   ErrorLog: a clean solve → an empty `ErrorStrings` dump — pins the plumbing +
#     the `EXP_ErrorLog.txt` naming (a clean IEEE13 run logs no DoSimpleMsg). The
#     non-empty content path is gated Rust-side (`golden_phase8.rs`), since
#     cross-engine error *message text* is not a Phase-8 correctness axis.
EVENTLOG_POST = [
    "RegControl.reg1.eventlog=yes",
    "RegControl.reg2.eventlog=yes",
    "RegControl.reg3.eventlog=yes",
    "new loadshape.es npts=3 interval=1 mult=(1 2 0.5)",
    "new load.swing bus1=675 phases=3 kv=4.16 kw=1200 daily=es",
    "set log=yes",
    "set mode=daily number=3 stepsize=1h",
    "solve",
]
ERRORLOG_POST = ["solve"]


def gen_log_reports(d) -> None:
    """Capture the oracle's EventLog/ErrorLog dumps on the IEEE13 feeder."""
    _gen_register_group(
        d, EVENTLOG_POST, [("eventlog", "EXP_EventLog.csv", "export_eventlog")]
    )
    _gen_register_group(
        d, ERRORLOG_POST, [("errorlog", "EXP_ErrorLog.txt", "export_errorlog")]
    )


# SeqZ reads the per-bus short-circuit impedances (`Zsc1`/`Zsc0`), which are only
# populated by a FaultStudy solve — a plain snapshot leaves them zero (a
# degenerate all-zero report). So it gets its own fixture with a `faultstudy`
# solve, generated from a fresh compile (the study mutates NodeV, so it can't
# share the snapshot loop above).
SEQZ_POST = ["solve mode=faultstudy"]


def gen_feeder_reports(d) -> None:
    """Capture the oracle's solution exports on the solved IEEE13 feeder."""
    master_abs = (REPO_ROOT / "tests" / "corpus" / "electricdss-tst" / FEEDER_MASTER).resolve()
    if not master_abs.is_file():
        sys.exit(f"master not found: {master_abs}")
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tmp = tempfile.mkdtemp(prefix="dss_gen_phase8_")
    try:
        d.Text.Command = "clear"
        d.Text.Command = f'compile "{master_abs}"'
        for c in FEEDER_POST:
            d.Text.Command = c
        case = d.ActiveCircuit.Name  # CaseName defaults to the circuit name
        d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
        for keyword, suffix, stem in FEEDER_REPORTS:
            d.Text.Command = f"export {keyword}"
            produced = Path(d.Text.Result)  # GlobalResult = produced path
            content = produced.read_text()  # universal newlines -> LF
            (OUT_DIR / f"{stem}.txt").write_text(content, newline="\n")
            meta = {
                "report": keyword,
                "master": FEEDER_MASTER,
                "post": FEEDER_POST,
                "fixture": case,
                "suffix": suffix,
            }
            (OUT_DIR / f"{stem}.meta.json").write_text(
                json.dumps(meta, indent=2) + "\n", newline="\n"
            )
            print(f"wrote {stem}.txt ({len(content)} bytes), {content.count(chr(10))} lines")
    finally:
        # `set datapath` moved the engine's cwd into tmp; move it out before
        # removing the dir, else Windows refuses to delete the locked cwd.
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp, ignore_errors=True)


def gen_seqz(d) -> None:
    """Capture the oracle's `Export SeqZ` on the FaultStudy-solved IEEE13 feeder."""
    master_abs = (REPO_ROOT / "tests" / "corpus" / "electricdss-tst" / FEEDER_MASTER).resolve()
    if not master_abs.is_file():
        sys.exit(f"master not found: {master_abs}")
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tmp = tempfile.mkdtemp(prefix="dss_gen_phase8_")
    try:
        d.Text.Command = "clear"
        d.Text.Command = f'compile "{master_abs}"'
        for c in SEQZ_POST:
            d.Text.Command = c
        case = d.ActiveCircuit.Name
        d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
        d.Text.Command = "export seqz"
        produced = Path(d.Text.Result)
        content = produced.read_text()
        (OUT_DIR / "export_seqz.txt").write_text(content, newline="\n")
        meta = {
            "report": "seqz",
            "master": FEEDER_MASTER,
            "post": SEQZ_POST,
            "fixture": case,
            "suffix": "EXP_SEQZ.csv",
        }
        (OUT_DIR / "export_seqz.meta.json").write_text(
            json.dumps(meta, indent=2) + "\n", newline="\n"
        )
        print(f"wrote export_seqz.txt ({len(content)} bytes), {content.count(chr(10))} lines")
    finally:
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp, ignore_errors=True)


# FaultStudy reads the same per-bus short-circuit state (`Ysc`/`BusCurrent`) a
# FaultStudy solve populates, so it shares SeqZ's fixture shape (a `solve
# mode=faultstudy` on IEEE13). `run_feeder_export` replays `meta.post`, so the
# Rust side runs the identical faultstudy solve before exporting.
FAULTSTUDY_POST = ["solve mode=faultstudy"]


def gen_faultstudy(d) -> None:
    """Capture the oracle's `Export Faultstudy` on the FaultStudy-solved IEEE13 feeder."""
    _gen_register_group(
        d, FAULTSTUDY_POST, [("faultstudy", "EXP_FAULTS.csv", "export_faultstudy")]
    )


# The reliability/capacity fixture (PHASE8_PLAN §WP8.3 step 3c). No corpus deck
# exports these, so it is synthesized (PHASE8_PLAN §1) as a self-contained deck (no
# master, like the Counts fixture): a two-section radial feeder (src→b1→b2) with
# per-line fault data, a load with `numcust` on each section, a **recloser** (the OCP
# device `RelCalc` needs so the forward interruption sweep completes) and an
# EnergyMeter on the head line. After `solve` + `relcalc` all the reliability
# accumulators (`Bus*`/`Branch*`/`Accumulated*`) are populated, so:
#   * `BusReliability`   — per-bus Lambda/Num-Interruptions/Num-Customers/…
#   * `BranchReliability`— per-branch Lambda/Accumulated-Lambda/customers/SAIFI/…
#   * `Capacity`         — per-PDElement Imax/%normal/%emergency/kW/kvar/customers/kVBase
# This is exactly the `relcalc_head_recloser_matches_oracle` feeder (the reliability
# unit test proves both engines' `RelCalc` arithmetic agrees), so the report values
# match tightly. The Rust golden (`golden_phase8.rs`) replays the same deck.
RELIABILITY_FIXTURE = "rel"
RELIABILITY_DECK = [
    f"new circuit.{RELIABILITY_FIXTURE} basekv=12.47 bus1=src phases=3",
    "new line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1 faultrate=0.2 pctperm=80 repair=4",
    "new line.l2 bus1=b1 bus2=b2 length=1 units=mi r1=0.1 x1=0.1 faultrate=0.3 pctperm=90 repair=5",
    "new load.ld1 bus1=b1 phases=3 kv=12.47 kw=100 numcust=10",
    "new load.ld2 bus1=b2 phases=3 kv=12.47 kw=200 numcust=25",
    "new recloser.r1 monitoredobj=line.l1 monitoredterm=1 switchedobj=line.l1 switchedterm=1 "
    "phasetrip=100000 groundtrip=100000",
    "new energymeter.m1 element=line.l1 terminal=1",
    "set voltagebases=[12.47]",
    "calcvoltagebases",
    "solve mode=snap",
    "relcalc",
]
RELIABILITY_REPORTS = [
    ("busreliability", "EXP_BusReliability.csv", "export_busreliability"),
    ("branchreliability", "EXP_BranchReliability.csv", "export_branchreliability"),
    ("capacity", "EXP_CAPACITY.csv", "export_capacity"),
]


def gen_reliability(d) -> None:
    """Capture the oracle's BusReliability/BranchReliability/Capacity reports."""
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tmp = tempfile.mkdtemp(prefix="dss_gen_phase8_")
    try:
        d.Text.Command = "clear"
        for c in RELIABILITY_DECK:
            d.Text.Command = c
        case = d.ActiveCircuit.Name
        d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
        for keyword, suffix, stem in RELIABILITY_REPORTS:
            d.Text.Command = f"export {keyword}"
            produced = Path(d.Text.Result)  # GlobalResult = produced path
            content = produced.read_text()  # universal newlines -> LF
            (OUT_DIR / f"{stem}.txt").write_text(content, newline="\n")
            meta = {
                "report": keyword,
                "fixture": case,
                "suffix": suffix,
                "deck": RELIABILITY_DECK,
            }
            (OUT_DIR / f"{stem}.meta.json").write_text(
                json.dumps(meta, indent=2) + "\n", newline="\n"
            )
            print(f"wrote {stem}.txt ({len(content)} bytes), {content.count(chr(10))} lines")
    finally:
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp, ignore_errors=True)


def main() -> None:
    pin = check_pin()
    print(f"oracle: dss-python {pin['dss_python']}, engine {pin['engine']}")
    import dss  # noqa: F401
    from dss import DSS as d

    gen_counts(d)
    gen_feeder_reports(d)
    gen_monitor_reports(d)
    gen_register_reports(d)
    gen_log_reports(d)
    gen_ieee8500_reports(d)
    gen_seqz(d)
    gen_faultstudy(d)
    gen_reliability(d)


if __name__ == "__main__":
    main()

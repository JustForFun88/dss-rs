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


def _gen_show_group(d, post, reports) -> None:
    """Like `_gen_register_group` but for the fixed-width `Show` register tables:
    `Show` writes `<OutputDir>/<CircuitName_><suffix>` and sets no `GlobalResult`,
    so the produced file is found by its fixed suffix glob (not `Text.Result`)."""
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
            d.Text.Command = f"show {keyword}"
            matches = list(Path(tmp).glob(f"*_{suffix}"))
            if len(matches) != 1:
                sys.exit(f"show {keyword}: expected 1 *_{suffix}, found {matches}")
            content = matches[0].read_text()  # universal newlines -> LF
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


# The `Show` register-table fixtures (PHASE8_PLAN §WP8.4). Reuse the WP8.3 register
# fixtures so the accumulated `%10.0f` register values are the same daily-solved
# meter/generator paths `corpus_live.rs` + the `Export Meters`/`Export Generators`
# goldens already pin — `Show Meters` (`EMout.txt`) on the metered IEEE13 (fixture A),
# `Show Generators` (`GenMeterOut.txt`) on the generator fixture (B, whose disabled
# `g3` pins the enabled-filter: it must NOT appear).
SHOW_METER_REPORTS_A = [("meters", "EMout.txt", "show_meters")]
SHOW_METER_REPORTS_B = [("generators", "GenMeterOut.txt", "show_generators")]


def gen_show_meter_reports(d) -> None:
    """Capture the oracle's `Show Meters`/`Show Generators` register tables."""
    d.AllowEditor = False
    _gen_show_group(d, REGISTER_A_POST, SHOW_METER_REPORTS_A)
    _gen_show_group(d, REGISTER_B_POST, SHOW_METER_REPORTS_B)


# `Show Meters`/`Show Generators` edge branches (WP8.4 step-8 audit follow-up):
#   MULTI: two EnergyMeters (em1 on the feeder head, em2 on a lateral) → the
#     legend-uses-FIRST-meter path + two data rows with distinct per-zone registers
#     (a bug printing the legend per-meter, dropping a row, or using meter[0]'s
#     registers for both rows fails `ExactOrdered`).
#   NONE (meters): plain solved IEEE13 has no EnergyMeter → the
#     `No Energymeter Elements Defined.` banner branch.
#   NONE (generators): plain solved IEEE13 has no Generator → the banner-only branch.
SHOW_MULTI_METER_POST = [
    "new energymeter.em1 element=Line.650632 terminal=1",
    "new energymeter.em2 element=Line.632645 terminal=1",
    "set mode=daily number=3 stepsize=1h",
    "solve",
]
SHOW_MULTI_METER_REPORTS = [("meters", "EMout.txt", "show_meters_multi")]
SHOW_NONE_REPORTS = [
    ("meters", "EMout.txt", "show_meters_none"),
    ("generators", "GenMeterOut.txt", "show_generators_none"),
]


def gen_show_meter_edgecases(d) -> None:
    """Capture the oracle's `Show Meters`/`Show Generators` edge branches."""
    d.AllowEditor = False
    _gen_show_group(d, SHOW_MULTI_METER_POST, SHOW_MULTI_METER_REPORTS)
    _gen_show_group(d, ["solve"], SHOW_NONE_REPORTS)


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


# The Show-report fixture (PHASE8_PLAN §WP8.4): the same solved IEEE13 feeder, then
# each `Show <keyword>` captured from the fixed-name file it writes. Unlike Export,
# `Show` sets no `GlobalResult`, so the produced file is found by its fixed suffix
# glob (`<case>_<name>.txt`, original-case circuit name). Each tuple:
# (show keyword, oracle default-filename suffix, golden stem).
SHOW_REPORTS = [
    ("losses", "Losses.txt", "show_losses"),
    ("buses", "Buses.txt", "show_buses"),
    ("taps", "RegTaps.txt", "show_taps"),
    # `Show Voltages` default (ShowOptionCode 0): the symmetrical-component form,
    # file `<case>_VLN.txt` (L-N; bare `show voltages`, no LL/node/elem selector).
    ("voltages", "VLN.txt", "show_voltages"),
    # `Show Currents`/`Powers` default (code 0): the per-element sequence forms.
    ("currents", "Curr_Seq.txt", "show_currents"),
    ("powers", "Power_seq_kVA.txt", "show_powers"),
    # `Show Voltages` code 1 (node form, `WriteBusVoltages`) and code 2 (element
    # form, `WriteElementVoltages`). First param `LN` (non-`LL` → L-N, `FilName=VLN`);
    # second param `Node`/`Elem` selects code 1/2 (`_Node`/`_elem` suffix).
    ("voltages LN node", "VLN_Node.txt", "show_voltages_node"),
    ("voltages LN elem", "VLN_elem.txt", "show_voltages_elem"),
    # `Show Currents` code 1 (element form, `WriteTerminalCurrents`) WITH residual:
    # first param `Y` → ShowResidual=TRUE, second `Elem` → code 1 (`Curr_Elem.txt`).
    ("currents Y elem", "Curr_Elem.txt", "show_currents_elem"),
    # `Show Elements` (`ShowElements`, default PD/PC form): the element↔bus listing.
    ("elements", "Elements.txt", "show_elements"),
    # `Show Voltages LL Node` (code 1, `LL`): the **line-line** node form — different
    # header ('LINE-LINE VOLTAGES BY BUS & NODE') + the `/√3` pu scaling. Exercises
    # the `ll=true` WriteBusVoltages branch (file `<case>_VLL_Node.txt`).
    ("voltages LL node", "VLL_Node.txt", "show_voltages_ll_node"),
    # `Show Elements Line` (`ShowElements` class-filter form): the active elements of
    # class Line, uppercased — exercises the class-filter path (`SetObjectClass` +
    # per-object routing). Same `<case>_Elements.txt` file (written after the default).
    ("elements line", "Elements.txt", "show_elements_class"),
    # `Show Powers e` (code 1, element form, `ShowPowers` case 1): per-terminal
    # branch power flow + per-terminal totals (incl. the 1-phase/2-terminal PD case).
    ("powers e", "Power_elem_kVA.txt", "show_powers_elem"),
    # `Show Ratings` (`ShowRatings`): each PD element's normal/emergency amp ratings.
    ("ratings", "RatingsOut.txt", "show_ratings"),
    # (`Show EventLog` is generated separately — see `gen_show_eventlog` — under the
    # event-logging daily fixture so it has real content, not an empty snapshot log.)
    # `Show Mismatch` (`ShowNodeCurrentSum`): per-node KCL current-sum mismatch. The
    # test pins the `Max Current` column (a 1e-8-pinned terminal current) + node/name
    # and gates the `Current Sum`/`%error` residual columns (faer-vs-KLU cancellation).
    ("mismatch", "NodeMismatch.txt", "show_mismatch"),
    # `Show Result` (`ShowResult`): the `@result` parser var (always `null` in the
    # pinned PM-build oracle). Pins the `<case>_Result.csv` filename (NOT `.txt`) +
    # the `GlobalResult` side-effect via the `Show` dispatch.
    ("result", "Result.csv", "show_result"),
    # `Show Convergence` (`Solution.WriteConvergenceReport`): per-node saved error /
    # |V| (`Str(v:14)` scientific) / Vbase + the Max Error footer.
    ("convergence", "Convergence.txt", "show_convergence"),
    # `Show Y` (`ShowY`): the assembled system Y, lower triangle by columns, one
    # `[row,col] = G + jB` line per stored entry (`%13.10g`).
    ("y", "SystemY.txt", "show_y"),
    # `Show controlqueue` (`ControlQueue.WriteQueue`): the pending control-action
    # queue — drained to the header alone after a converged snapshot solve.
    ("controlqueue", "ControlQueue.csv", "show_controlqueue"),
    # `Show kvbasemismatch` (`ShowkVBaseMismatch`): IEEE13 has loads but no >10%
    # mismatch, so the report is the `!!!  LOAD VOLTAGE BASE MISMATCHES` header
    # block alone (proves the family-header logic; the value/generator branches are
    # exercised by the synthesized `gen_show_kvbasemismatch`).
    ("kvbasemismatch", "kVBaseMismatch.txt", "show_kvbasemismatch"),
]


def gen_show_reports(d) -> None:
    """Capture the oracle's `Show` fixed-width text reports on the solved IEEE13 feeder."""
    d.AllowEditor = False  # don't spawn a Notepad per Show report (see main())
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
        case = d.ActiveCircuit.Name  # lowercased; the file prefix is original-case
        d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
        for keyword, suffix, stem in SHOW_REPORTS:
            d.Text.Command = f"show {keyword}"
            # Show writes `<OutputDir>/<CircuitName_><suffix>` but sets no
            # GlobalResult; find it by suffix (original-case prefix).
            matches = list(Path(tmp).glob(f"*_{suffix}"))
            if len(matches) != 1:
                sys.exit(f"show {keyword}: expected 1 *_{suffix}, found {matches}")
            content = matches[0].read_text()  # universal newlines -> LF
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
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp, ignore_errors=True)


def gen_show_eventlog(d) -> None:
    """Capture the oracle's `Show EventLog` under the event-logging daily fixture
    (`EVENTLOG_POST`), so it has real tap-change/marker content, not the empty
    snapshot log. `Show` sets no GlobalResult, so the file is found by suffix."""
    d.AllowEditor = False
    master_abs = (REPO_ROOT / "tests" / "corpus" / "electricdss-tst" / FEEDER_MASTER).resolve()
    if not master_abs.is_file():
        sys.exit(f"master not found: {master_abs}")
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tmp = tempfile.mkdtemp(prefix="dss_gen_phase8_")
    try:
        d.Text.Command = "clear"
        d.Text.Command = f'compile "{master_abs}"'
        for c in EVENTLOG_POST:
            d.Text.Command = c
        case = d.ActiveCircuit.Name
        d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
        d.Text.Command = "show eventlog"
        matches = list(Path(tmp).glob("*_EventLog.txt"))
        if len(matches) != 1:
            sys.exit(f"show eventlog: expected 1 *_EventLog.txt, found {matches}")
        content = matches[0].read_text()
        (OUT_DIR / "show_eventlog.txt").write_text(content, newline="\n")
        meta = {
            "report": "eventlog",
            "master": FEEDER_MASTER,
            "post": EVENTLOG_POST,
            "fixture": case,
            "suffix": "EventLog.txt",
        }
        (OUT_DIR / "show_eventlog.meta.json").write_text(
            json.dumps(meta, indent=2) + "\n", newline="\n"
        )
        print(f"wrote show_eventlog.txt ({len(content)} bytes), {content.count(chr(10))} lines")
    finally:
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp, ignore_errors=True)


def gen_show_yprim(d) -> None:
    """Capture the oracle's `Show Yprim` (`ShowYPrim`) for an active element on the
    solved IEEE13 feeder. `Select line.650632` makes the line the active circuit
    element; the report writes `<ParentClass.Name>_<Name>_Yprim.txt`
    (`Line_650632_Yprim.txt`, NO `CircuitName_` prefix), found by the `*_Yprim.txt`
    suffix glob. The IEEE13 lines are LineCode-based, so the primitive Y is
    bit-exact Rust↔oracle (→ exact-equality golden)."""
    d.AllowEditor = False
    _gen_show_group(
        d,
        ["solve", "select line.650632"],
        [("yprim", "Yprim.txt", "show_yprim")],
    )


# `Show LineConstants` fixture (PHASE8_PLAN §WP8.4). A self-contained geometry deck
# (no corpus deck is small + geometry-only): `g3` is a 3-conductor overhead line
# (order 3 → exercises the R/jX/susceptance/L/C matrices AND the equivalent
# symmetrical-component summary Z1/Z0/C1/C0/surge/velocity); `g1` is a 1-conductor
# line (order 1 → the non-order-3 branch, no seq summary). No solve is needed
# (`Show LineConstants` reads the LineGeometry catalog + recomputes Carson).
SHOW_LC_DECK = [
    "clear",
    "new circuit.lcdemo basekv=12.47 bus1=sourcebus",
    "new wiredata.acsr336 NormAmps=530 DIAM=0.721 GMRac=0.29280 Rdc=0.057954545 Runits=kft Radunits=in gmrunits=in",
    "new linegeometry.g3 nconds=3 nphases=3 reduce=n",
    "~ cond=1 wire=acsr336 x=-1.25 h=28 units=ft",
    "~ cond=2 wire=acsr336 x=0 h=28 units=ft",
    "~ cond=3 wire=acsr336 x=1.25 h=28 units=ft",
    "new linegeometry.g1 nconds=1 nphases=1 reduce=n",
    "~ cond=1 wire=acsr336 x=0 h=28 units=ft",
]


# A geometry-less deck (circuit + WireData, NO LineGeometry) → `Show LineConstants`
# writes only the file headers (empty geometry list). Pins that header-only path.
SHOW_LC_EMPTY_DECK = [
    "clear",
    "new circuit.lcempty basekv=12.47 bus1=sourcebus",
    "new wiredata.acsr336 NormAmps=530 DIAM=0.721 GMRac=0.29280 Rdc=0.057954545 Runits=kft Radunits=in gmrunits=in",
]


def _gen_lineconstants(d, deck, args: str, stem_prefix: str) -> None:
    """Capture `Show LineConstants <args>` on `deck` into two goldens:
    `<stem_prefix>.txt` (`<case>_LineConstants.txt`) and `<stem_prefix>_code.txt`
    (`LineConstantsCode.dss`, no `<case>_` prefix). `args` is the trailing
    `[freq] [units] [rho]` (empty = the defaults freq=DefaultBaseFreq/kft/100)."""
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tmp = tempfile.mkdtemp(prefix="dss_gen_phase8_")
    try:
        d.Text.Command = "clear"
        for c in deck:
            d.Text.Command = c
        case = d.ActiveCircuit.Name
        d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
        d.Text.Command = f"show lineconstants {args}".rstrip()
        for suffix, stem, glob_pat in [
            ("LineConstants.txt", stem_prefix, "*_LineConstants.txt"),
            ("LineConstantsCode.dss", f"{stem_prefix}_code", "LineConstantsCode.dss"),
        ]:
            matches = list(Path(tmp).glob(glob_pat))
            if len(matches) != 1:
                sys.exit(f"show lineconstants {args}: expected 1 {glob_pat}, found {matches}")
            content = matches[0].read_text()  # universal newlines -> LF
            (OUT_DIR / f"{stem}.txt").write_text(content, newline="\n")
            meta = {
                "report": f"lineconstants {args}".rstrip(),
                "fixture": case,
                "suffix": suffix,
                "deck": deck,
            }
            (OUT_DIR / f"{stem}.meta.json").write_text(
                json.dumps(meta, indent=2) + "\n", newline="\n"
            )
            print(f"wrote {stem}.txt ({len(content)} bytes), {content.count(chr(10))} lines")
    finally:
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp, ignore_errors=True)


def gen_show_lineconstants(d) -> None:
    """Capture the oracle's `Show LineConstants` (`ShowLineConstants`). Each case
    writes TWO files: `<case>_LineConstants.txt` (the R/jX/susceptance/L/C matrices +
    the order-3 symmetrical-component summary) and `LineConstantsCode.dss` (a
    LineCode script, same folder, NO `<case>_` prefix). Two cases: **defaults**
    (`show_lineconstants`, no args = freq=DefaultBaseFreq/kft/rho=100) and
    **non-default args** (`show_lineconstants_mi250` = `60 mi 250` — pins the
    freq/units/rho arg-parse AND the rho=250 earth-return + units=mi scaling
    propagating into the Carson recompute and the `ohms per mi` labels /
    `To_per_Meter` velocity, none of which the default case exercises), and the
    **empty geometry list** (`show_lineconstants_empty` — a geometry-less deck →
    header-only files, pinning the no-`LineGeometry` early-return path)."""
    d.AllowEditor = False
    _gen_lineconstants(d, SHOW_LC_DECK, "", "show_lineconstants")
    _gen_lineconstants(d, SHOW_LC_DECK, "60 mi 250", "show_lineconstants_mi250")
    # freq != DefaultBaseFreq (audit-code follow-up): pins that a non-default
    # frequency propagates into the Carson recompute (the freq-parse arm is already
    # hit by mi250's non-empty "60", but its value equals the default there).
    _gen_lineconstants(d, SHOW_LC_DECK, "50", "show_lineconstants_f50")
    _gen_lineconstants(d, SHOW_LC_EMPTY_DECK, "", "show_lineconstants_empty")


def gen_show_variables(d) -> None:
    """Capture the oracle's `Show Variables` on IEEE13 + a Generator — a PC element
    with dynamic state variables (6: Frequency/Theta/Vd/PShaft/dSpeed/dTheta), so the
    per-variable `%-.6g` value-formatting path is exercised (plain IEEE13 has no PC
    element with variables). `Show` sets no GlobalResult; find the file by suffix."""
    d.AllowEditor = False
    master_abs = (REPO_ROOT / "tests" / "corpus" / "electricdss-tst" / FEEDER_MASTER).resolve()
    if not master_abs.is_file():
        sys.exit(f"master not found: {master_abs}")
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    post = ["new generator.g1 bus1=675 phases=3 kv=4.16 kw=100 model=1", "solve"]
    tmp = tempfile.mkdtemp(prefix="dss_gen_phase8_")
    try:
        d.Text.Command = "clear"
        d.Text.Command = f'compile "{master_abs}"'
        for c in post:
            d.Text.Command = c
        case = d.ActiveCircuit.Name
        d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
        d.Text.Command = "show variables"
        matches = list(Path(tmp).glob("*_Variables.txt"))
        if len(matches) != 1:
            sys.exit(f"show variables: expected 1 *_Variables.txt, found {matches}")
        content = matches[0].read_text()
        (OUT_DIR / "show_variables.txt").write_text(content, newline="\n")
        meta = {
            "report": "variables",
            "master": FEEDER_MASTER,
            "post": post,
            "fixture": case,
            "suffix": "Variables.txt",
        }
        (OUT_DIR / "show_variables.meta.json").write_text(
            json.dumps(meta, indent=2) + "\n", newline="\n"
        )
        print(f"wrote show_variables.txt ({len(content)} bytes), {content.count(chr(10))} lines")
    finally:
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp, ignore_errors=True)


# Synthesized deck (PHASE8_PLAN §1) for `Show kvbasemismatch`'s *value* branches —
# plain IEEE13 has no >10% kV-base mismatch, so add four small (electrically
# negligible, kw=1) elements with deliberately-off kV bases: a 3-phase and a
# 1-phase load, and a 3-phase and a 1-phase generator, exercising both the
# line-line (`kVBase·√3`) and the 1-phase line-neutral comparison forms plus the
# GENERATOR family header.
KVBASE_POST = [
    "new load.mismll bus1=675 phases=3 conn=wye kv=5.0 kw=1 pf=1",
    "new load.mismln bus1=634.1 phases=1 conn=wye kv=0.35 kw=1 pf=1",
    "new generator.gmismll bus1=675 phases=3 kv=5.0 kw=1",
    "new generator.gmismln bus1=634.1 phases=1 conn=wye kv=0.35 kw=1",
    "solve",
]


def gen_show_kvbasemismatch(d) -> None:
    """Capture the oracle's `Show kvbasemismatch` on IEEE13 + four kV-base-mismatched
    elements (`KVBASE_POST`), so the mismatch-line formatting + the generator block
    are exercised. `Show` sets no GlobalResult; find the file by suffix."""
    d.AllowEditor = False
    master_abs = (REPO_ROOT / "tests" / "corpus" / "electricdss-tst" / FEEDER_MASTER).resolve()
    if not master_abs.is_file():
        sys.exit(f"master not found: {master_abs}")
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tmp = tempfile.mkdtemp(prefix="dss_gen_phase8_")
    try:
        d.Text.Command = "clear"
        d.Text.Command = f'compile "{master_abs}"'
        for c in KVBASE_POST:
            d.Text.Command = c
        case = d.ActiveCircuit.Name
        d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
        d.Text.Command = "show kvbasemismatch"
        matches = list(Path(tmp).glob("*_kVBaseMismatch.txt"))
        if len(matches) != 1:
            sys.exit(f"show kvbasemismatch: expected 1 *_kVBaseMismatch.txt, found {matches}")
        content = matches[0].read_text()
        (OUT_DIR / "show_kvbasemismatch_vals.txt").write_text(content, newline="\n")
        meta = {
            "report": "kvbasemismatch",
            "master": FEEDER_MASTER,
            "post": KVBASE_POST,
            "fixture": case,
            "suffix": "kVBaseMismatch.txt",
        }
        (OUT_DIR / "show_kvbasemismatch_vals.meta.json").write_text(
            json.dumps(meta, indent=2) + "\n", newline="\n"
        )
        print(
            f"wrote show_kvbasemismatch_vals.txt ({len(content)} bytes), "
            f"{content.count(chr(10))} lines"
        )
    finally:
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp, ignore_errors=True)


def gen_show_monitor(d) -> None:
    """Capture the oracle's `Show monitor m_vi` (`TranslateToCSV`) under the monitor
    daily fixture — the same CSV the `Export Monitors` path produces, driven via the
    `Show` dispatcher. `Show` sets no GlobalResult; find the file by suffix."""
    d.AllowEditor = False
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
        d.Text.Command = "show monitor m_vi"
        matches = list(Path(tmp).glob("*_Mon_m_vi_1.csv"))
        if len(matches) != 1:
            sys.exit(f"show monitor: expected 1 *_Mon_m_vi_1.csv, found {matches}")
        content = matches[0].read_text()
        (OUT_DIR / "show_monitor.txt").write_text(content, newline="\n")
        meta = {
            "report": "monitor m_vi",
            "master": FEEDER_MASTER,
            "post": MONITOR_POST,
            "fixture": case,
            "suffix": "Mon_m_vi_1.csv",
        }
        (OUT_DIR / "show_monitor.meta.json").write_text(
            json.dumps(meta, indent=2) + "\n", newline="\n"
        )
        print(f"wrote show_monitor.txt ({len(content)} bytes), {content.count(chr(10))} lines")
    finally:
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


# FaultStudy on an UNBASED circuit (WP8.4 step-10 audit-tests follow-up): no
# `Set Voltagebases` / `CalcVoltageBases`, so every bus keeps `kVBase = 0` and
# sections 2 & 3 render the `%10.1f` "L-N Volts if no base" branch instead of the
# `%10.3f` per-unit form (the based IEEE13 golden only exercises pu). A prior
# `solve mode=snap` is required so FaultStudy runs and allocates `Zsc`/`Ysc` — a
# **cold** `solve mode=faultstudy` (no prior solve) access-violates the oracle
# (unallocated `Zsc`), which the port guards instead (see `fault_study::tests`).
SHOW_FS_UNBASED_DECK = [
    "new circuit.fscov basekv=12.47 bus1=src phases=3",
    "new line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1",
    "new line.lat bus1=b1.1 bus2=b2.1 phases=1 length=1 units=mi r1=0.3 x1=0.3",
    "new load.ld1 bus1=b1 phases=3 kv=12.47 kw=100",
    "new load.ld2 bus1=b2.1 phases=1 kv=7.2 kw=50",
    "solve mode=snap",
    "solve mode=faultstudy",
]


def gen_show_faultstudy(d) -> None:
    """Capture the oracle's `Show Faults` (`ShowFaultStudy`): the FaultStudy-solved
    IEEE13 feeder (`solve mode=faultstudy`, the `export_faultstudy`/`SeqZ` fixture
    shape, all buses based → the pu form) plus an **unbased** deck (`kVBase = 0` →
    the `%10.1f` L-N-Volts branch). `Show` sets no `GlobalResult`, so both are found
    by the `<case>_FaultStudy.txt` suffix glob."""
    d.AllowEditor = False
    _gen_show_group(
        d, FAULTSTUDY_POST, [("faults", "FaultStudy.txt", "show_faultstudy")]
    )
    _gen_show_deck_group(
        d, SHOW_FS_UNBASED_DECK, [("faults", "FaultStudy.txt", "show_faultstudy_unbased")]
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


# The Sections fixture (PHASE8_PLAN §WP8.3 step 3c part 3). No corpus deck exports
# it, so it is synthesized (PHASE8_PLAN §1): a two-feeder circuit with TWO meters —
# m1's zone has two recloser-headed sections (src→b1→b2), m2's zone one
# fuse-headed section (src→c1, pinning the FUSE branch of getOCPDeviceTypeString) —
# solved + `relcalc`'d so `SectionCount`/`FeederSections` persist on each meter.
# Two reports: `sections` (all meters) and `sections meter=m2` (the named-meter
# pre-parse — its rows must be m2's only).
SECTIONS_DECK = [
    "new circuit.sect basekv=12.47 bus1=src phases=3",
    "new line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1 faultrate=0.2 pctperm=80 repair=4",
    "new line.l2 bus1=b1 bus2=b2 length=1 units=mi r1=0.1 x1=0.1 faultrate=0.3 pctperm=90 repair=5",
    "new line.l3 bus1=src bus2=c1 length=2 units=mi r1=0.2 x1=0.2 faultrate=0.4 pctperm=70 repair=6",
    "new load.ld1 bus1=b1 phases=3 kv=12.47 kw=100 numcust=10",
    "new load.ld2 bus1=b2 phases=3 kv=12.47 kw=200 numcust=25",
    "new load.ld3 bus1=c1 phases=3 kv=12.47 kw=150 numcust=8",
    "new recloser.r1 monitoredobj=line.l1 monitoredterm=1 switchedobj=line.l1 switchedterm=1 "
    "phasetrip=100000 groundtrip=100000",
    "new recloser.r2 monitoredobj=line.l2 monitoredterm=1 switchedobj=line.l2 switchedterm=1 "
    "phasetrip=100000 groundtrip=100000",
    "new fuse.f1 monitoredobj=line.l3 monitoredterm=1 switchedobj=line.l3 switchedterm=1 "
    "ratedcurrent=100000",
    "new energymeter.m1 element=line.l1 terminal=1",
    "new energymeter.m2 element=line.l3 terminal=1",
    "set voltagebases=[12.47]",
    "calcvoltagebases",
    "solve mode=snap",
    "relcalc",
]
SECTIONS_REPORTS = [
    ("sections", "EXP_SECTIONS.csv", "export_sections"),
    ("sections meter=m2", "EXP_SECTIONS.csv", "export_sections_meter"),
]


def gen_sections(d) -> None:
    """Capture the oracle's `Export Sections` reports (all meters + named meter)."""
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tmp = tempfile.mkdtemp(prefix="dss_gen_phase8_")
    try:
        d.Text.Command = "clear"
        for c in SECTIONS_DECK:
            d.Text.Command = c
        case = d.ActiveCircuit.Name
        d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
        for keyword, suffix, stem in SECTIONS_REPORTS:
            d.Text.Command = f"export {keyword}"
            produced = Path(d.Text.Result)  # GlobalResult = produced path
            content = produced.read_text()  # universal newlines -> LF
            (OUT_DIR / f"{stem}.txt").write_text(content, newline="\n")
            meta = {
                "report": keyword,
                "fixture": case,
                "suffix": suffix,
                "deck": SECTIONS_DECK,
            }
            (OUT_DIR / f"{stem}.meta.json").write_text(
                json.dumps(meta, indent=2) + "\n", newline="\n"
            )
            print(f"wrote {stem}.txt ({len(content)} bytes), {content.count(chr(10))} lines")
    finally:
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp, ignore_errors=True)


# The Profile fixture (PHASE8_PLAN §WP8.3 step 3c part 3): IEEE13 + an EnergyMeter
# (Profile walks each meter's branch list with the zone-build `DistFromMeter`),
# solved snapshot. Seven variants pin every `PhasesToPlot` selector branch: the
# default (3-phase primary), `all`/`primary` (per-present-phase L-N, incl. the
# < 1 kV Linetype=2 rows), the three L-L forms, and an explicit single phase
# (`2` — the single-character `Parser.IntValue` re-read).
PROFILE_POST = [
    "new energymeter.em1 element=Line.650632 terminal=1",
    "solve",
]
PROFILE_REPORTS = [
    ("profile", "EXP_Profile.csv", "export_profile"),
    ("profile all", "EXP_Profile.csv", "export_profile_all"),
    ("profile primary", "EXP_Profile.csv", "export_profile_primary"),
    ("profile ll3ph", "EXP_Profile.csv", "export_profile_ll3ph"),
    ("profile llall", "EXP_Profile.csv", "export_profile_llall"),
    ("profile llprimary", "EXP_Profile.csv", "export_profile_llprimary"),
    ("profile 2", "EXP_Profile.csv", "export_profile_ph2"),
]


def gen_profile(d) -> None:
    """Capture the oracle's `Export Profile` variants on the metered IEEE13 feeder."""
    _gen_register_group(d, PROFILE_POST, PROFILE_REPORTS)


# The demand-interval fixture (PHASE8_PLAN §WP8.3 step 4 / §2.6). The DI files are
# written DURING the time-series solve (opened by SolveDaily, one row per SampleAll,
# closed by its finally), not by an Export command — so this generator sets the
# datapath BEFORE the post commands and collects the produced files from
# `<datapath>/<case>/DI_yr_0/` afterwards. The fixture: IEEE13 + an EnergyMeter with
# PhaseVoltageReport=yes, all four DI switches on, daily-3. The IEEE13 trunk lines
# carry ≈490 A against the 400 A default NormAmps, so DI_Overloads has real rows;
# the 0.48 kV bus 634 gives the voltage report a live LV section and the PHV file a
# second voltage base.
# The leading snapshot `solve` builds the meter zone (VBaseList + the vbase
# register names) BEFORE the DI files open: without it the oracle's per-meter DI
# header carries the ctor-default empty register names and the PHV header renders
# *uninitialized heap garbage* as vbase labels (`1.72E-311kV_Phs_1_Max` — Pascal
# reads the never-initialized VBaseList at open time; unpinnable, like the
# FeederSections OOB read). With the zone pre-built both engines' headers are
# deterministic and identical. Note the zone's voltage-base list collects each
# branch's FROM bus only, so IEEE13 has a single 4.16 kV base (bus 634's 0.48 kV
# is nobody's from-bus) — one PHV group.
DI_POST = [
    "new energymeter.em1 element=Line.650632 terminal=1 phasevoltagereport=yes",
    "solve",
    "set demandinterval=yes",
    "set diverbose=yes",
    "set overloadreport=yes",
    "set voltexceptionreport=yes",
    "set mode=daily number=3 stepsize=1h",
    "solve",
]
DI_FILES = [
    ("em1_1.csv", "di_em1"),
    ("em1_PhaseVoltageReport_1.csv", "di_em1_phv"),
    ("DI_SystemMeter_1.csv", "di_systemmeter"),
    ("DI_Totals_1.csv", "di_totals"),
    ("DI_Overloads_1.csv", "di_overloads"),
    ("DI_VoltExceptions_1.csv", "di_voltexceptions"),
    ("EnergyMeterTotals_1.csv", "di_energymetertotals"),
    ("Totals_1.csv", "di_grand_totals"),
    ("SystemMeter_1.csv", "di_systemmeter_registers"),
]


def gen_demand_interval(d) -> None:
    """Capture the oracle's demand-interval (DI_) files from a daily IEEE13 run."""
    master_abs = (REPO_ROOT / "tests" / "corpus" / "electricdss-tst" / FEEDER_MASTER).resolve()
    if not master_abs.is_file():
        sys.exit(f"master not found: {master_abs}")
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tmp = tempfile.mkdtemp(prefix="dss_gen_phase8_")
    try:
        d.Text.Command = "clear"
        d.Text.Command = f'compile "{master_abs}"'
        d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
        for c in DI_POST:
            d.Text.Command = c
        case = d.ActiveCircuit.Name
        di_dir = Path(tmp) / case / "DI_yr_0"
        for fname, stem in DI_FILES:
            produced = di_dir / fname
            content = produced.read_text()  # universal newlines -> LF
            (OUT_DIR / f"{stem}.txt").write_text(content, newline="\n")
            meta = {
                "master": FEEDER_MASTER,
                "post": DI_POST,
                "fixture": case,
                "relpath": f"{case}/DI_yr_0/{fname}",
            }
            (OUT_DIR / f"{stem}.meta.json").write_text(
                json.dumps(meta, indent=2) + "\n", newline="\n"
            )
            print(f"wrote {stem}.txt ({len(content)} bytes), {content.count(chr(10))} lines")
    finally:
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp, ignore_errors=True)


# Single-phase-overload fixture (WP8.3 step 4 audit-tests follow-up). The daily
# IEEE13 DI fixture only overloads 3-phase trunk lines, so `WriteOverloadReport`'s
# `NPhases < 3` phase-mapping branch (the per-conductor `MapNodeToBus.node_num`
# lookup that replaces Pascal's `FirstBus` string-parse) is unexercised. This deck
# overloads a phase-2-only lateral (`normamps=5` vs ≈32 A load current), so its
# current must land in the I2 column — I1/I3 zero — pinning the mapping. No corpus
# deck exercises this, so it is synthesized (PHASE8_PLAN §1).
OV1PH_DECK = [
    "new circuit.ov2 basekv=12.47 bus1=src phases=3",
    "new line.main bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1 c1=0",
    "new line.lat bus1=b1.2 bus2=b2.2 phases=1 length=0.5 units=mi r1=0.3 x1=0.3 c1=0 "
    "normamps=5 emergamps=6",
    "new load.l1 bus1=b2.2 phases=1 kv=7.2 kw=200 model=1",
    "new energymeter.m element=line.main terminal=1",
    "set voltagebases=[12.47]",
    "calcvoltagebases",
]
OV1PH_POST = [
    "solve mode=snap",
    "set demandinterval=yes",
    "set overloadreport=yes",
    "set mode=daily number=1 stepsize=1h",
    "solve",
]


def gen_di_overloads_1ph(d) -> None:
    """Capture DI_Overloads for a single-phase (phase-2) overloaded lateral."""
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tmp = tempfile.mkdtemp(prefix="dss_gen_phase8_")
    try:
        d.Text.Command = "clear"
        for c in OV1PH_DECK:
            d.Text.Command = c
        d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
        for c in OV1PH_POST:
            d.Text.Command = c
        case = d.ActiveCircuit.Name
        produced = Path(tmp) / case / "DI_yr_0" / "DI_Overloads_1.csv"
        content = produced.read_text()  # universal newlines -> LF
        (OUT_DIR / "di_overloads_1ph.txt").write_text(content, newline="\n")
        meta = {
            "deck": OV1PH_DECK,
            "post": OV1PH_POST,
            "fixture": case,
            "relpath": f"{case}/DI_yr_0/DI_Overloads_1.csv",
        }
        (OUT_DIR / "di_overloads_1ph.meta.json").write_text(
            json.dumps(meta, indent=2) + "\n", newline="\n"
        )
        print(f"wrote di_overloads_1ph.txt ({len(content)} bytes), {content.count(chr(10))} lines")
    finally:
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp, ignore_errors=True)


# Multi-meter BusReliability fixture (audit follow-up — the multi-meter
# `Bus_Int_Duration` cross-zone bug, see
# `investigations/reliability_bus_int_duration_oob_bug_report.md`). Two metered
# feeders off one source, EACH with two OCP sections, so the second meter's
# duration loop reads the first meter's zone buses at section ids 1 and 2 — both
# IN RANGE for its own 2-section array. That triggers the *deterministic*
# cross-zone contamination (a bus's Bus_Int_Duration is overwritten from another
# feeder's section) with NO out-of-bounds read, so both engines agree bus-for-bus
# and the port's reproduction of the upstream bug is pinnable. (The OOB regime —
# a later meter with FEWER sections — is proven-nondeterministic UB in the report
# and is deliberately not gated.) Distinct per-line repair times make the
# contamination observable in the Duration column.
RELIABILITY_MULTIMETER_DECK = [
    "new circuit.sect basekv=12.47 bus1=src phases=3",
    "new line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1 faultrate=0.2 pctperm=80 repair=4",
    "new line.l2 bus1=b1 bus2=b2 length=1 units=mi r1=0.1 x1=0.1 faultrate=0.3 pctperm=90 repair=5",
    "new line.l3 bus1=src bus2=c1 length=2 units=mi r1=0.2 x1=0.2 faultrate=0.4 pctperm=70 repair=6",
    "new line.l4 bus1=c1 bus2=c2 length=1 units=mi r1=0.2 x1=0.2 faultrate=0.5 pctperm=60 repair=9",
    "new load.ld1 bus1=b1 phases=3 kv=12.47 kw=100 numcust=10",
    "new load.ld2 bus1=b2 phases=3 kv=12.47 kw=200 numcust=25",
    "new load.ld3 bus1=c1 phases=3 kv=12.47 kw=150 numcust=8",
    "new load.ld4 bus1=c2 phases=3 kv=12.47 kw=90 numcust=5",
    "new recloser.r1 monitoredobj=line.l1 monitoredterm=1 switchedobj=line.l1 switchedterm=1 "
    "phasetrip=1e5 groundtrip=1e5",
    "new recloser.r2 monitoredobj=line.l2 monitoredterm=1 switchedobj=line.l2 switchedterm=1 "
    "phasetrip=1e5 groundtrip=1e5",
    "new recloser.r3 monitoredobj=line.l3 monitoredterm=1 switchedobj=line.l3 switchedterm=1 "
    "phasetrip=1e5 groundtrip=1e5",
    "new recloser.r4 monitoredobj=line.l4 monitoredterm=1 switchedobj=line.l4 switchedterm=1 "
    "phasetrip=1e5 groundtrip=1e5",
    "new energymeter.m1 element=line.l1 terminal=1",
    "new energymeter.m2 element=line.l3 terminal=1",
    "set voltagebases=[12.47]",
    "calcvoltagebases",
    "solve mode=snap",
    "relcalc",
]


def gen_reliability_multimeter(d) -> None:
    """Capture the oracle's multi-meter `Export BusReliability` (cross-zone contamination)."""
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tmp = tempfile.mkdtemp(prefix="dss_gen_phase8_")
    try:
        d.Text.Command = "clear"
        for c in RELIABILITY_MULTIMETER_DECK:
            d.Text.Command = c
        case = d.ActiveCircuit.Name
        d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
        d.Text.Command = "export busreliability"
        content = Path(d.Text.Result).read_text()  # universal newlines -> LF
        (OUT_DIR / "export_busreliability_multimeter.txt").write_text(content, newline="\n")
        meta = {
            "report": "busreliability",
            "fixture": case,
            "suffix": "EXP_BusReliability.csv",
            "deck": RELIABILITY_MULTIMETER_DECK,
        }
        (OUT_DIR / "export_busreliability_multimeter.meta.json").write_text(
            json.dumps(meta, indent=2) + "\n", newline="\n"
        )
        print(f"wrote export_busreliability_multimeter.txt ({len(content)} bytes)")
    finally:
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp, ignore_errors=True)


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


# The overload / unserved / allocation-factor fixtures (PHASE8_PLAN §WP8.3 step 3c
# part 2). No corpus deck exports these either, so each is synthesized as a small
# self-contained deck (PHASE8_PLAN §1) tailored to make the report non-empty:
#   * Overloads — a line with a deliberately small `normamps`/`emergamps` under a
#     heavy load so its terminal-1 current exceeds both ratings (the only rows
#     `ExportOverloads` writes).
#   * Unserved  — a long high-impedance feeder that sags the load bus below
#     `NormalMinVolts` (0.95 pu), so `ExceedsNormal` latches a nonzero `EEN_Factor`.
#   * AllocationFactors — one connected-kVA-spec load (`xfkva=`) and one kWh-spec
#     load (`kwh=`); only these two spec types emit a line.
# Each is a deck fixture (DeckMeta with the oracle default-filename suffix), the
# Rust golden (`golden_phase8.rs`) replays the same deck.
DECK_GROUPS = [
    (
        "ovl",
        [
            "new circuit.ovl basekv=12.47 bus1=src phases=3",
            "new line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1 normamps=10 emergamps=15",
            "new load.ld1 bus1=b1 phases=3 kv=12.47 kw=500",
            "set voltagebases=[12.47]",
            "calcvoltagebases",
            "solve mode=snap",
        ],
        [("overloads", "EXP_OVERLOADS.csv", "export_overloads")],
    ),
    (
        # Overloads, unbalanced: a single-phase load on a 3-phase line drives
        # nonzero I2/I0 (pins the `phase_to_sym` decomposition, not just the
        # balanced all-zero columns) + a `normamps=0` second line that forces the
        # degenerate `NormAmps<=0` column-shift row (an empty AmpsOver field).
        "ovl2",
        [
            "new circuit.ovl2 basekv=12.47 bus1=src phases=3",
            "new line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1 normamps=5 emergamps=8",
            "new line.l2 bus1=b1 bus2=b2 length=1 units=mi r1=0.1 x1=0.1 normamps=0 emergamps=6",
            "new load.ld1 bus1=b1.1 phases=1 kv=7.2 kw=200",
            "new load.ld2 bus1=b2.1 phases=1 kv=7.2 kw=150",
            "set voltagebases=[12.47]",
            "calcvoltagebases",
            "solve mode=snap",
        ],
        [("overloads", "EXP_OVERLOADS.csv", "export_overloads_unbal")],
    ),
    (
        "uns",
        [
            "new circuit.uns basekv=12.47 bus1=src phases=3",
            "new line.l1 bus1=src bus2=b1 length=8 units=mi r1=0.3 x1=0.6",
            "new load.ld1 bus1=b1 phases=3 kv=12.47 kw=3000 pf=0.9",
            "set voltagebases=[12.47]",
            "calcvoltagebases",
            "solve mode=snap",
        ],
        [("unserved", "EXP_UNSERVED.csv", "export_unserved")],
    ),
    (
        # Unserved, UE (emergency) criterion: a deep-sag load below `EmergMinVolts`
        # (nonzero UE_Factor via the `Unserved` path) + a healthy load that must be
        # excluded (pins both the `ue_only` branch and the criterion filter).
        "uns2",
        [
            "new circuit.uns2 basekv=12.47 bus1=src phases=3",
            "new line.l1 bus1=src bus2=b1 length=12 units=mi r1=0.4 x1=0.8",
            "new load.ld1 bus1=b1 phases=3 kv=12.47 kw=4000 pf=0.9",
            "new line.l2 bus1=src bus2=b2 length=0.1 units=mi r1=0.05 x1=0.05",
            "new load.ld2 bus1=b2 phases=3 kv=12.47 kw=50",
            "set voltagebases=[12.47]",
            "calcvoltagebases",
            "solve mode=snap",
        ],
        [("unserved ue", "EXP_UNSERVED.csv", "export_unserved_ue")],
    ),
    (
        # AllocationFactors: a connected-kVA-spec load (`la`), a kWh-spec load
        # (`lb`), a plain-kW load (`lc`, must emit NO line — the no-emit branch),
        # and a **disabled** connected-kVA-spec load (`ld`, must STILL emit — Pascal
        # `DumpAllocationFactors` has no `Enabled` filter).
        "alloc",
        [
            "new circuit.alloc basekv=12.47 bus1=src phases=3",
            "new line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1",
            "new load.la bus1=b1 phases=3 kv=12.47 xfkva=500 allocationfactor=0.75",
            "new load.lb bus1=b1 phases=3 kv=12.47 kwh=1000 cfactor=0.9",
            "new load.lc bus1=b1 phases=3 kv=12.47 kw=500",
            "new load.ld bus1=b1 phases=3 kv=12.47 xfkva=300 allocationfactor=0.6 enabled=no",
            "set voltagebases=[12.47]",
            "calcvoltagebases",
            "solve mode=snap",
        ],
        [("allocationfactors", "AllocationFactors.txt", "export_allocationfactors")],
    ),
]


def _gen_show_deck_group(d, deck, reports) -> None:
    """Like `_gen_show_group` but for a self-contained `New`-circuit deck (no master
    compile): replay the deck, `show <keyword>`, and capture the fixed-name file the
    report writes (found by its suffix glob — `Show` sets no `GlobalResult`)."""
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tmp = tempfile.mkdtemp(prefix="dss_gen_phase8_")
    try:
        d.Text.Command = "clear"
        for c in deck:
            d.Text.Command = c
        case = d.ActiveCircuit.Name
        d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
        for keyword, suffix, stem in reports:
            d.Text.Command = f"show {keyword}"
            matches = list(Path(tmp).glob(f"*_{suffix}"))
            if len(matches) != 1:
                sys.exit(f"show {keyword}: expected 1 *_{suffix}, found {matches}")
            content = matches[0].read_text()  # universal newlines -> LF
            (OUT_DIR / f"{stem}.txt").write_text(content, newline="\n")
            meta = {
                "report": keyword,
                "fixture": case,
                "suffix": suffix,
                "deck": deck,
            }
            (OUT_DIR / f"{stem}.meta.json").write_text(
                json.dumps(meta, indent=2) + "\n", newline="\n"
            )
            print(f"wrote {stem}.txt ({len(content)} bytes), {content.count(chr(10))} lines")
    finally:
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp, ignore_errors=True)


# `Show Overloads` niche-branch coverage (audit-tests step-9 follow-up). The
# reused `ovl`/`ovl2` decks (3-phase lines, positive `emergamps`, no capacitors)
# never hit three real `show_overloads` branches. This dedicated deck does: a
# **1-phase** overloaded line (`normamps=5`, `emergamps=0`) exercises BOTH the
# `Nphases < 3` symmetrical-component fallback (`I0=I2=0`, `I1=|I|`, `Cmax:=I1`)
# AND the `EmergAmps <= 0` degenerate `%Emerg` literal (`     0.0`); a small
# overloaded shunt **capacitor** (`normamps=1` under 600 kvar ≈ 27.8 A) pins the
# capacitor-skip — it must NOT appear (`(CLASSMASK and DSSObjType) <> CAP_ELEMENT`).
# No corpus deck exercises these, so it is synthesized (PHASE8_PLAN §1).
SHOW_OVL_COV_DECK = [
    "new circuit.ovlcov basekv=12.47 bus1=src phases=3",
    "new line.lat bus1=src.1 bus2=b1.1 phases=1 length=1 units=mi r1=0.3 x1=0.3 normamps=5 emergamps=0",
    "new load.ld1 bus1=b1.1 phases=1 kv=7.2 kw=150",
    "new capacitor.c1 bus1=src phases=3 kvar=600 kv=12.47 normamps=1",
    "set voltagebases=[12.47]",
    "calcvoltagebases",
    "solve mode=snap",
]


def gen_show_overload_unserved(d) -> None:
    """Capture the oracle's `Show Overloads`/`Show Unserved` (PHASE8_PLAN §WP8.4).
    Reuses the `DECK_GROUPS` export decks (index 0=ovl, 1=ovl2, 2=uns, 3=uns2) so the
    `Show` and `Export` forms share fixtures and can never drift: ovl/ovl2 overload a
    small-`normamps` line (ovl2 adds the unbalanced I2/I0 + the `normamps=0`
    degenerate-column row); uns/uns2 sag a load below the normal / emergency voltage
    minimum (`unserved ue` selects the UE_Only criterion). Two follow-up coverage
    goldens (audit-tests step 9): `show_overloads_1ph` (the 1-phase fallback +
    `emergamps=0` + capacitor-skip niche branches) and `show_unserved_normal` (the
    **normal**-criterion healthy-load exclusion — `uns2`'s healthy `ld2` must be
    dropped by `ExceedsNormal`, not just by the UE `Unserved` path)."""
    d.AllowEditor = False
    ovl, ovl2, uns, uns2 = (DECK_GROUPS[i][1] for i in range(4))
    _gen_show_deck_group(d, ovl, [("overloads", "Overload.txt", "show_overloads")])
    _gen_show_deck_group(d, ovl2, [("overloads", "Overload.txt", "show_overloads_unbal")])
    _gen_show_deck_group(d, uns, [("unserved", "Unserved.txt", "show_unserved")])
    _gen_show_deck_group(d, uns2, [("unserved ue", "Unserved.txt", "show_unserved_ue")])
    _gen_show_deck_group(
        d, SHOW_OVL_COV_DECK, [("overloads", "Overload.txt", "show_overloads_1ph")]
    )
    _gen_show_deck_group(d, uns2, [("unserved", "Unserved.txt", "show_unserved_normal")])


# `Show Loops` / `Show Zone` (PHASE8_PLAN §WP8.4): the two EnergyMeter zone-tree
# reports. The radial IEEE13 (fixture `em1` on Line.650632) pins the header/empty
# path (`show_loops`, no loops → two header lines) and the radial branch/shunt
# tree (`show_zone`). A synthesized **meshed** deck — a 3-line loop (b1-b2-b3-b1)
# plus a line parallel to `la` (b1-b2) inside a metered zone — exercises the
# PARALLEL/LOOP branches of BOTH reports (`ShowLoops` one line per parallel/looped
# branch; `ShowMeterZone` the inline `(PARALLEL:Name)`/`(LOOP:FullName)`
# annotations). No corpus deck has a looped metered zone, so it is synthesized
# (PHASE8_PLAN §1). Element names are lowercase — the reports emit them native
# case (LoopLineObj / zone branch), and the port's names are lowercase-normalized.
SHOW_MESH_DECK = [
    "new circuit.meshtest basekv=12.47 bus1=sourcebus phases=3",
    "new line.feed  bus1=sourcebus bus2=b1 phases=3 length=1 units=km r1=0.1 x1=0.1 c1=0",
    "new line.la    bus1=b1 bus2=b2 phases=3 length=1 units=km r1=0.1 x1=0.1 c1=0",
    "new line.lb    bus1=b2 bus2=b3 phases=3 length=1 units=km r1=0.1 x1=0.1 c1=0",
    "new line.lc    bus1=b3 bus2=b1 phases=3 length=1 units=km r1=0.1 x1=0.1 c1=0",
    "new line.lpar  bus1=b1 bus2=b2 phases=3 length=1 units=km r1=0.1 x1=0.1 c1=0",
    "new load.ld    bus1=b3 phases=3 kv=12.47 kw=100 pf=0.95",
    "new energymeter.m1 element=line.feed terminal=1",
    "set voltagebases=[12.47]",
    "calcvoltagebases",
    "solve",
]


# Two metered feeders off one source (audit-tests step-12 follow-up): zone A (m1)
# has a loop a1-a2-a3-a1, zone B (m2) a parallel line b_la||b_lpar. So BOTH meters
# contribute rows to `Show Loops` — exercising the multi-meter outer loop that the
# single-meter `show_loops`/`show_loops_mesh` goldens don't (a regression that broke
# after the first meter, duplicated the header, or mis-attributed the `(mtr)` prefix
# would slip past them). Synthesized — no corpus deck has a looped metered zone.
SHOW_MESH2_DECK = [
    "new circuit.mesh2 basekv=12.47 bus1=sourcebus phases=3",
    "new line.feeda bus1=sourcebus bus2=a1 phases=3 length=1 units=km r1=0.1 x1=0.1 c1=0",
    "new line.a_la  bus1=a1 bus2=a2 phases=3 length=1 units=km r1=0.1 x1=0.1 c1=0",
    "new line.a_lb  bus1=a2 bus2=a3 phases=3 length=1 units=km r1=0.1 x1=0.1 c1=0",
    "new line.a_lc  bus1=a3 bus2=a1 phases=3 length=1 units=km r1=0.1 x1=0.1 c1=0",
    "new load.lda   bus1=a3 phases=3 kv=12.47 kw=100 pf=0.95",
    "new energymeter.m1 element=line.feeda terminal=1",
    "new line.feedb bus1=sourcebus bus2=b1 phases=3 length=1 units=km r1=0.1 x1=0.1 c1=0",
    "new line.b_la   bus1=b1 bus2=b2 phases=3 length=1 units=km r1=0.1 x1=0.1 c1=0",
    "new line.b_lpar bus1=b1 bus2=b2 phases=3 length=1 units=km r1=0.1 x1=0.1 c1=0",
    "new load.ldb    bus1=b2 phases=3 kv=12.47 kw=100 pf=0.95",
    "new energymeter.m2 element=line.feedb terminal=1",
    "set voltagebases=[12.47]",
    "calcvoltagebases",
    "solve",
]


def gen_show_zone_loops(d) -> None:
    """Capture the oracle's `Show Loops`/`Show Zone` (PHASE8_PLAN §WP8.4)."""
    d.AllowEditor = False
    # Radial IEEE13 (fixture A: em1 on Line.650632) — header-only loops + the
    # radial zone tree.
    _gen_show_group(d, REGISTER_A_POST, [("loops", "Loops.txt", "show_loops")])
    _gen_show_group(d, REGISTER_A_POST, [("zone em1", "ZoneOut_em1.txt", "show_zone")])
    # Meshed synthetic deck — the PARALLEL/LOOP paths of both reports.
    _gen_show_deck_group(d, SHOW_MESH_DECK, [("loops", "Loops.txt", "show_loops_mesh")])
    _gen_show_deck_group(
        d, SHOW_MESH_DECK, [("zone m1", "ZoneOut_m1.txt", "show_zone_mesh")]
    )
    # Two-meter deck — the multi-meter `Show Loops` outer loop (both meters emit rows).
    _gen_show_deck_group(
        d, SHOW_MESH2_DECK, [("loops", "Loops.txt", "show_loops_multi")]
    )


# `Show Controlled` multi-control coverage (audit follow-up, step 13). The feeder
# golden (`show_controlled`) only exercises one control per PD element
# (RegControl→Transformer) and only the `RegControl` `controlled_element()`
# override. This synthesized deck (PHASE8_PLAN §1) pins: (a) the multiple-controls
# `, %s , %s ` loop + its creation-order ordering — Line.l1 carries a Recloser then
# a Relay, Line.l2 two SwtControls; (b) all FIVE PD-targeting overrides —
# `swt_control`/`recloser`/`relay`/`fuse` (→Line) and `cap_control` (→Capacitor).
# The `fuse` case is the audit-code Major regression guard (Fuse was the one
# `TControlElem` subclass missing its override, so a fuse-switched line silently
# vanished). The fuse sits on its own healthy lateral (l3) so it stays quiescent
# (a fuse on the recloser/relay line churns → #485). No solve is required by
# `Show Controlled` (arm 33 is not solve-guarded), but a clean snap solve keeps the
# deck realistic.
SHOW_CTRL_DECK = [
    "clear",
    "new circuit.ctrldemo basekv=12.47 bus1=sourcebus phases=3",
    "new line.l1 bus1=sourcebus bus2=b1 phases=3 length=1 units=mi r1=0.1 x1=0.3 c1=0 normamps=400 emergamps=600",
    "new line.l2 bus1=b1 bus2=b2 phases=3 length=1 units=mi r1=0.1 x1=0.3 c1=0 switch=y",
    "new line.l3 bus1=b2 bus2=b3 phases=3 length=1 units=mi r1=0.1 x1=0.3 c1=0 normamps=200",
    "new capacitor.cap1 bus1=b2 phases=3 kvar=600 kv=12.47",
    "new load.ld1 bus1=b2 phases=3 kv=12.47 kw=500",
    "new load.ld3 bus1=b3 phases=3 kv=12.47 kw=100",
    "new recloser.rec1 monitoredobj=line.l1 monitoredterm=1 switchedobj=line.l1 switchedterm=1",
    "new relay.rel1 monitoredobj=line.l1 monitoredterm=1 switchedobj=line.l1 switchedterm=1 type=current",
    "new swtcontrol.sw1 switchedobj=line.l2 switchedterm=1 action=close",
    "new swtcontrol.sw2 switchedobj=line.l2 switchedterm=1 action=close normal=closed",
    "new capcontrol.cc1 element=line.l2 terminal=1 capacitor=cap1 type=current ONsetting=10 OFFsetting=5",
    "new fuse.fu1 monitoredobj=line.l3 monitoredterm=1 switchedobj=line.l3 switchedterm=1",
    "set voltagebases=[12.47]",
    "calcvoltagebases",
    "solve",
]


SHOW_BUSFLOW_REPORTS = [
    # `Show busflow <bus>` (code 0 seq) and `<bus> e` (code 1 elem). Bus 675 is a
    # fully-energised 3-phase leaf (Line.692675 + Capacitor.Cap1 + a 3-phase load) →
    # exercises the seq-V header, per-element seq I (all terminals) + seq P (matched
    # terminal), and the element-form node voltages + WriteTerminalCurrents (PD
    # residual) + WriteTerminalPower. Chosen over a richer junction (671) that lands a
    # `%10.5g` kvar cell on a 5-sig rounding boundary (a print straddle) — 675 is
    # fully byte-exact bar the capacitor's ~0-kW / PF faer-vs-KLU residual.
    ("busflow 675", "675_seq_kVA.txt", "show_busflow"),
    ("busflow 675 e", "675_elem_kVA.txt", "show_busflow_elem"),
    # MVA form (audit-tests step-15 follow-up): the ×0.001 scaling + MW/Mvar/MVA
    # headers — the only MVA coverage in the whole `Show` power path.
    ("busflow 675 m", "675_seq_MVA.txt", "show_busflow_mva"),
    ("busflow 675 m e", "675_elem_MVA.txt", "show_busflow_mva_elem"),
    # 1-phase bus 611 (audit-tests follow-up): the `<3`-phase seq path
    # (WriteSeqVoltages<3 nodes → V2/V0=0, WriteTerminalPowerSeq S1). Bus 611 has a
    # 1-phase load + Capacitor.Cap2.
    ("busflow 611", "611_seq_kVA.txt", "show_busflow_1ph"),
    ("busflow 611 e", "611_elem_kVA.txt", "show_busflow_1ph_elem"),
]


# Coverage deck for the `Show Isolated`/`Show Topology` non-empty branches: a
# PARALLEL line pair (la ‖ la2), a switched line + SwtControl (lb/sw1), and a fully
# ISOLATED island (isoa-isob with a load) unreachable from the source. Not solved
# (the island makes Y singular) — `Show Isolated`/`Topology` need only connectivity.
SHOW_TOPO_DECK = [
    "clear",
    "new circuit.topo basekv=12.47 bus1=src phases=3",
    "new line.la bus1=src bus2=b1 phases=3 length=1 units=mi r1=0.1 x1=0.3 c1=0",
    "new line.la2 bus1=src bus2=b1 phases=3 length=1 units=mi r1=0.1 x1=0.3 c1=0",
    "new line.lb bus1=b1 bus2=b2 phases=3 length=1 units=mi r1=0.1 x1=0.3 c1=0 switch=y",
    "new swtcontrol.sw1 switchedobj=line.lb switchedterm=1 action=close",
    "new load.ld bus1=b2 phases=3 kv=12.47 kw=500",
    "new line.iso bus1=isoa bus2=isob phases=3 length=1 units=mi r1=0.1 x1=0.3 c1=0",
    "new load.isold bus1=isob phases=3 kv=12.47 kw=100",
    "set voltagebases=[12.47]",
    "calcvoltagebases",
]


# `Show Isolated` orphan coverage (audit-tests step-16 follow-up): enabled elements
# on PD-less, source-disconnected buses — an orphan Load + Generator — so the
# "ENABLED ELEMENTS ARE ISOLATED" list (the `"FullName"  Buses:  "bus"` + 1-based
# `get_bus(j)` walk) and the "BUSES NOT CONNECTED TO ANY POWER DELIVERY ELEMENT" list
# are both non-empty (they are empty on IEEE13 + the mesh deck). Not solved (the
# orphan buses make Y singular).
SHOW_ISO_ORPHAN_DECK = [
    "clear",
    "new circuit.orph basekv=12.47 bus1=src phases=3",
    "new line.la bus1=src bus2=b1 phases=3 length=1 units=mi r1=0.1 x1=0.3 c1=0",
    "new load.ld bus1=b1 phases=3 kv=12.47 kw=500",
    "new load.orphan bus1=orphanbus phases=3 kv=12.47 kw=50",
    "new generator.og bus1=genbus phases=3 kv=12.47 kw=50 model=1",
    "set voltagebases=[12.47]",
    "calcvoltagebases",
]


def gen_show_isolated_orphan(d) -> None:
    """Capture `Show Isolated` on the orphan-bus deck (audit-tests step-16 follow-up)."""
    d.AllowEditor = False
    _gen_show_deck_group(
        d, SHOW_ISO_ORPHAN_DECK, [("isolated", "Isolated.txt", "show_isolated_orphan")]
    )


def gen_show_topo_coverage(d) -> None:
    """Capture `Show Isolated`/`Show Topology` on `SHOW_TOPO_DECK` — the non-empty
    branches the IEEE13 goldens miss: isolated buses + an isolated sub-network
    (`show_isolated_iso`), and the PARALLEL / Controlled-Switch / Isolated-PD counts +
    the `(PARALLEL:…)`/`(Control:…)`/`Isolated: …` tree annotations
    (`show_topology_mesh` summary + `show_topology_mesh_tree`). Byte-exact."""
    d.AllowEditor = False
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tmp = tempfile.mkdtemp(prefix="dss_gen_phase8_")
    try:
        d.Text.Command = "clear"
        for c in SHOW_TOPO_DECK:
            d.Text.Command = c
        case = d.ActiveCircuit.Name
        d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
        d.Text.Command = "show isolated"
        d.Text.Command = "show topology"
        for suffix, stem in [
            ("Isolated.txt", "show_isolated_iso"),
            ("TopoSumm.txt", "show_topology_mesh"),
            ("TopoTree.txt", "show_topology_mesh_tree"),
        ]:
            matches = list(Path(tmp).glob(f"*_{suffix}"))
            if len(matches) != 1:
                sys.exit(f"show topo coverage: expected 1 *_{suffix}, found {matches}")
            content = matches[0].read_text()
            (OUT_DIR / f"{stem}.txt").write_text(content, newline="\n")
            report = "isolated" if suffix == "Isolated.txt" else "topology"
            meta = {"report": report, "fixture": case, "suffix": suffix, "deck": SHOW_TOPO_DECK}
            (OUT_DIR / f"{stem}.meta.json").write_text(
                json.dumps(meta, indent=2) + "\n", newline="\n"
            )
            print(f"wrote {stem}.txt ({len(content)} bytes), {content.count(chr(10))} lines")
    finally:
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp, ignore_errors=True)


def gen_show_isolated(d) -> None:
    """Capture the oracle's `Show Isolated` (`ShowIsolated`) on the metered IEEE13:
    the circuit is fully connected, so the isolated sections are empty and the report
    is the connected element tree (`(Level) FullName` + `[SHUNT], FullName`). Pure
    text → byte-exact."""
    d.AllowEditor = False
    _gen_show_group(d, REGISTER_A_POST, [("isolated", "Isolated.txt", "show_isolated")])


def gen_show_topology(d) -> None:
    """Capture the oracle's `Show Topology` (`ShowTopology`) on the metered IEEE13.
    Writes TWO files: `<case>_TopoSumm.txt` (the level/loop/parallel/isolated/switch
    counts) and `<case>_TopoTree.txt` (the TABCHAR-indented branch/shunt tree with the
    inline `(LOOP:…)`/`(Control:…)`/`(Meter:…)` annotations). Both captured (stems
    `show_topology` / `show_topology_tree`), byte-exact."""
    d.AllowEditor = False
    master_abs = (REPO_ROOT / "tests" / "corpus" / "electricdss-tst" / FEEDER_MASTER).resolve()
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tmp = tempfile.mkdtemp(prefix="dss_gen_phase8_")
    try:
        d.Text.Command = "clear"
        d.Text.Command = f'compile "{master_abs}"'
        for c in REGISTER_A_POST:
            d.Text.Command = c
        case = d.ActiveCircuit.Name
        d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
        d.Text.Command = "show topology"
        for suffix, stem in [
            ("TopoSumm.txt", "show_topology"),
            ("TopoTree.txt", "show_topology_tree"),
        ]:
            matches = list(Path(tmp).glob(f"*_{suffix}"))
            if len(matches) != 1:
                sys.exit(f"show topology: expected 1 *_{suffix}, found {matches}")
            content = matches[0].read_text()
            (OUT_DIR / f"{stem}.txt").write_text(content, newline="\n")
            meta = {
                "report": "topology",
                "master": FEEDER_MASTER,
                "post": REGISTER_A_POST,
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


def gen_show_busflow(d) -> None:
    """Capture the oracle's `Show busflow` (`ShowBusPowers`) on solved IEEE13, bus
    675 — both the seq form (`show_busflow`) and the element form
    (`show_busflow_elem`). `Show` sets no GlobalResult; find by suffix."""
    d.AllowEditor = False
    _gen_show_group(d, FEEDER_POST, SHOW_BUSFLOW_REPORTS)


def gen_show_controlled(d) -> None:
    """Capture the oracle's `Show Controlled` (`ShowControlledElements`). Two goldens:
    the feeder case (`show_controlled`) — solved IEEE13's three voltage-regulator
    `RegControl`s each control a `Transformer`, so the report lists three
    `Transformer.regN, RegControl.regN ` lines (single control per PD, `RegControl`
    override) — and the synthesized multi-control deck (`show_controlled_multi`,
    audit-tests follow-up) pinning the repeated-control loop + ordering and the
    `swt_control`/`recloser`/`relay`/`cap_control` overrides. Pure text (names only,
    no numbers, trailing space per control) → diffed **byte-exact**."""
    d.AllowEditor = False
    _gen_show_group(
        d, FEEDER_POST, [("controlled", "ControlledElements.csv", "show_controlled")]
    )
    _gen_show_deck_group(
        d,
        SHOW_CTRL_DECK,
        [("controlled", "ControlledElements.csv", "show_controlled_multi")],
    )


def gen_deck_groups(d) -> None:
    """Capture the oracle's Overloads/Unserved/AllocationFactors reports."""
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for _case, deck, reports in DECK_GROUPS:
        tmp = tempfile.mkdtemp(prefix="dss_gen_phase8_")
        try:
            d.Text.Command = "clear"
            for c in deck:
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
                    "fixture": case,
                    "suffix": suffix,
                    "deck": deck,
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

    # Suppress the report editor auto-open (`FireOffEditor`): the `Show` commands
    # call it per report and would spawn a Notepad window for every golden file.
    # Report *content* is unaffected — the file is always written; only the GUI
    # editor launch is disabled (headless-faithful, PHASE8_PLAN §2.2).
    d.AllowEditor = False

    gen_counts(d)
    gen_feeder_reports(d)
    gen_show_reports(d)
    gen_show_eventlog(d)
    gen_show_yprim(d)
    gen_show_variables(d)
    gen_show_kvbasemismatch(d)
    gen_show_monitor(d)
    gen_monitor_reports(d)
    gen_register_reports(d)
    gen_show_meter_reports(d)
    gen_show_meter_edgecases(d)
    gen_log_reports(d)
    gen_ieee8500_reports(d)
    gen_seqz(d)
    gen_faultstudy(d)
    gen_show_faultstudy(d)
    gen_reliability(d)
    gen_deck_groups(d)
    gen_show_overload_unserved(d)
    gen_show_zone_loops(d)
    gen_show_controlled(d)
    gen_show_busflow(d)
    gen_show_isolated(d)
    gen_show_topology(d)
    gen_show_topo_coverage(d)
    gen_show_isolated_orphan(d)
    gen_show_lineconstants(d)
    gen_sections(d)
    gen_profile(d)
    gen_demand_interval(d)
    gen_di_overloads_1ph(d)
    gen_reliability_multimeter(d)


if __name__ == "__main__":
    main()

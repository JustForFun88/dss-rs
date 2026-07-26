"""Generate the targeted report goldens from the pinned oracle (historically
"Phase 8", PHASE8_PLAN.md).

The reporting/output layer (Export/Show/Save/Dump). Unlike the command-replay
goldens (timeseries_controls / metering_monitors / der_controls), these pin the
**report file the oracle writes**: `gen_reports.py` runs a small fixture on the
pinned engine, issues the
`Export`/`Show`/... command, and captures the produced file's exact bytes into
`tests/golden/reports/<report>.txt`. The Rust side (`golden_reports.rs`) replays
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
    python tools/golden/gen_reports.py            # regenerate all report goldens
Regeneration is manual and must use the exact versions in tools/golden/PIN.txt.
"""

from __future__ import annotations

import json
import os
import shutil
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gen_checkpoints import check_pin  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = REPO_ROOT / "tests" / "golden" / "reports"

# The Counts fixture: a tiny circuit exercising a few class counts (Line=2,
# Load=1, Vsource=1) on top of the default DSS items. Counts depend only on
# *instance counts*, not on bus names or a solve. The Rust golden test
# (`golden_reports.rs`) reads this same deck back from the meta file, so the two
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
    tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
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
# (`golden_reports.rs`) compiles the same master from `tests/corpus`, replays the
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
    tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
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


# The IEEE 34/37/123 core solution exports (PORTING_PLAN §6 literal acceptance).
# §6 requires the export-diff suite green on IEEE 13/34/37/123/8500: 13 is pinned
# by `gen_feeder_reports` (the full export family) and 8500 by
# `gen_ieee8500_reports`; these three feeders complete the letter with the core
# solution exports Voltages/Currents/Powers (mirroring the IEEE13 core set — the
# physics is already pinned to 1e-8 by `corpus_live.rs`, so this gates the report
# layout/scaling at scale on each canonical feeder). IEEE34's `ieee34Mod1.dss` is
# a `not_an_entry_point` fragment (no embedded `Solve`), so all three compile +
# `solve` uniformly. Golden stems: `export_<feeder>_<report>`.
EXTRA_FEEDERS = [
    ("ieee34", "Version8/Distrib/IEEETestCases/34Bus/ieee34Mod1.dss"),
    ("ieee37", "Version8/Distrib/IEEETestCases/37Bus/ieee37.dss"),
    ("ieee123", "Version8/Distrib/IEEETestCases/123Bus/IEEE123Master.dss"),
]
EXTRA_FEEDER_POST = ["solve"]
EXTRA_FEEDER_REPORTS = [
    ("voltages", "EXP_VOLTAGES.csv", "voltages"),
    ("currents", "EXP_CURRENTS.csv", "currents"),
    ("powers", "EXP_POWERS.csv", "powers"),
]


def gen_extra_feeder_reports(d) -> None:
    """Capture the oracle's Voltages/Currents/Powers on the solved IEEE34/37/123
    feeders (PORTING_PLAN §6 export-diff coverage)."""
    for prefix, master in EXTRA_FEEDERS:
        master_abs = (REPO_ROOT / "tests" / "corpus" / "electricdss-tst" / master).resolve()
        if not master_abs.is_file():
            sys.exit(f"master not found: {master_abs}")
        OUT_DIR.mkdir(parents=True, exist_ok=True)
        tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
        try:
            d.Text.Command = "clear"
            d.Text.Command = f'compile "{master_abs}"'
            for c in EXTRA_FEEDER_POST:
                d.Text.Command = c
            case = d.ActiveCircuit.Name
            d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
            for keyword, suffix, name in EXTRA_FEEDER_REPORTS:
                stem = f"export_{prefix}_{name}"
                d.Text.Command = f"export {keyword}"
                produced = Path(d.Text.Result)  # GlobalResult = produced path
                content = produced.read_text()  # universal newlines -> LF
                (OUT_DIR / f"{stem}.txt").write_text(content, newline="\n")
                meta = {
                    "report": keyword,
                    "master": master,
                    "post": EXTRA_FEEDER_POST,
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
    tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
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
    tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
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
    tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
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
#     (the timeseries_controls `daily_ieee13` gate pins the same tap logic). This golden pins
#     the **export** (SaveToFile → `EXP_EventLog.csv` naming + the `%g` render
#     into the file) on top of the marker + tap-change stream.
#   ErrorLog: a clean solve → an empty `ErrorStrings` dump — pins the plumbing +
#     the `EXP_ErrorLog.txt` naming (a clean IEEE13 run logs no DoSimpleMsg). The
#     non-empty content path is gated Rust-side (`golden_reports.rs`), since
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
    tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
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
    # `Show DeltaV` (`ShowDeltaV`): the voltage across each enabled 2-terminal element
    # (per conductor `NodeV[term1] - NodeV[term2]`); `Transformer.SUB` etc.
    ("deltaV", "DeltaV.txt", "show_deltav"),
]


def gen_show_reports(d) -> None:
    """Capture the oracle's `Show` fixed-width text reports on the solved IEEE13 feeder."""
    d.AllowEditor = False  # don't spawn a Notepad per Show report (see main())
    master_abs = (REPO_ROOT / "tests" / "corpus" / "electricdss-tst" / FEEDER_MASTER).resolve()
    if not master_abs.is_file():
        sys.exit(f"master not found: {master_abs}")
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
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
    tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
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
    tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
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
    tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
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
    tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
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
    tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
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
    tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
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
# match tightly. The Rust golden (`golden_reports.rs`) replays the same deck.
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
    tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
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
    tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
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
    tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
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
    tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
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


# The GICMvars fixture (ORPHANED_GAPS §1.1). No corpus deck *exports* it, so the
# GAPS `gictransformer_gic` deck is reused as a self-contained fixture: a quasi-DC
# (Set frequency=0.1) GIC study with all three GICTransformer types — GSU (tg1) +
# YY (tg2) on the K-factor Mvar path, and an Auto (tg3) on the VarCurve path
# (varcurve=vgic → the FVarCurveObj.GetYValue branch). `Export GICMvars` writes
# one `Bus, Mvar, GIC Amps per phase` row per GICTransformer (Pascal
# `ExportGICMvar` + `WriteVarOutputRecord`).
GIC_MVAR_FIXTURE = "gaps_gict"
GIC_MVAR_DECK = [
    "set defaultbasefrequency=60",
    f"new circuit.{GIC_MVAR_FIXTURE} basekv=345 phases=3 bus1=b1 mvasc3=2000000 2000000",
    "new gicline.gl12 bus1=b1 bus2=b2 R=3.5 Volts=100 Angle=0",
    "new gicline.gl23 bus1=b2 bus2=b3 R=2.9 EN=1.0 EE=1.0",
    "~ Lat1=33.613499 Lon1=-87.373673 Lat2=33.547885 Lon2=-86.074605",
    "new gictransformer.tg1 busH=b1 busNH=b1.4.4.4 R1=0.12 type=GSU",
    "new gictransformer.tg2 busH=b2 busNH=b2.4.4.4 busX=b2x busNX=b2.4.4.4 R1=0.2 R2=0.1 type=YY",
    "new xycurve.vgic npts=3 xarray=(0 1 2) yarray=(0 0.6 1.0)",
    "new gictransformer.tg3 busH=b3 busX=b3x busNX=b3.4.4.4 %R1=0.2 %R2=0.15",
    "~ kvll1=345 kvll2=138 mva=300 varcurve=vgic type=Auto",
    "new reactor.gg1 phases=1 bus1=b1.4 r=0.20 x=0",
    "new reactor.gg2 phases=1 bus1=b2.4 r=0.15 x=0",
    "new reactor.gg3 phases=1 bus1=b3.4 r=0.25 x=0",
    "set voltagebases=[345 138]",
    "calcvoltagebases",
    "set frequency=0.1",
    "solve",
]
GIC_MVAR_REPORTS = [("gicmvars", "EXP_GIC_Mvar.csv", "export_gicmvars")]


def gen_gic_mvars(d) -> None:
    """Capture the oracle's `Export GICMvars` on the GIC-study fixture."""
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
    try:
        d.Text.Command = "clear"
        for c in GIC_MVAR_DECK:
            d.Text.Command = c
        case = d.ActiveCircuit.Name
        d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
        for keyword, suffix, stem in GIC_MVAR_REPORTS:
            d.Text.Command = f"export {keyword}"
            produced = Path(d.Text.Result)  # GlobalResult = produced path
            content = produced.read_text()  # universal newlines -> LF
            (OUT_DIR / f"{stem}.txt").write_text(content, newline="\n")
            meta = {
                "report": keyword,
                "fixture": case,
                "suffix": suffix,
                "deck": GIC_MVAR_DECK,
            }
            (OUT_DIR / f"{stem}.meta.json").write_text(
                json.dumps(meta, indent=2) + "\n", newline="\n"
            )
            print(f"wrote {stem}.txt ({len(content)} bytes), {content.count(chr(10))} lines")
    finally:
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp, ignore_errors=True)


def gen_reliability(d) -> None:
    """Capture the oracle's BusReliability/BranchReliability/Capacity reports."""
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
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
# Rust golden (`golden_reports.rs`) replays the same deck.
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
    tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
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


# `Show Isolated` disabled-PD coverage (audit-code step-16 follow-up): a **disabled**
# line (`enabled=no`) must NOT emit a `*** START SUBAREA ***` block (Pascal's
# `if TestElement.Enabled` sub-area guard). A disabled PD element is in `ckt_elements`
# but never in the adjacency lists.
SHOW_ISO_DISABLED_DECK = [
    "clear",
    "new circuit.dis basekv=12.47 bus1=src phases=3",
    "new line.la bus1=src bus2=b1 phases=3 length=1 units=mi r1=0.1 x1=0.3 c1=0",
    "new load.ld bus1=b1 phases=3 kv=12.47 kw=500",
    "new line.dead bus1=deada bus2=deadb phases=3 length=1 units=mi r1=0.1 x1=0.3 c1=0 enabled=no",
    "set voltagebases=[12.47]",
    "calcvoltagebases",
]
# `Show Isolated` **unsolved** coverage (audit-code step-16 follow-up): a compiled but
# never-solved circuit (no `calcvoltagebases`/`solve`) — `Show Isolated` reprocesses
# bus defs itself, so the connected tree is still correct (without the reprocess the
# port would give a degenerate one-source tree).
SHOW_ISO_UNSOLVED_DECK = [
    "clear",
    "new circuit.uns basekv=12.47 bus1=src phases=3",
    "new line.la bus1=src bus2=b1 phases=3 length=1 units=mi r1=0.1 x1=0.3 c1=0",
    "new load.ld bus1=b1 phases=3 kv=12.47 kw=500",
]


def gen_show_isolated_orphan(d) -> None:
    """Capture `Show Isolated` on the orphan-bus deck (audit-tests step-16 follow-up)
    + the disabled-PD + unsolved decks (audit-code step-16 follow-up)."""
    d.AllowEditor = False
    _gen_show_deck_group(
        d, SHOW_ISO_ORPHAN_DECK, [("isolated", "Isolated.txt", "show_isolated_orphan")]
    )
    _gen_show_deck_group(
        d, SHOW_ISO_DISABLED_DECK, [("isolated", "Isolated.txt", "show_isolated_disabled")]
    )
    _gen_show_deck_group(
        d, SHOW_ISO_UNSOLVED_DECK, [("isolated", "Isolated.txt", "show_isolated_unsolved")]
    )


def gen_show_topo_coverage(d) -> None:
    """Capture `Show Isolated`/`Show Topology` on `SHOW_TOPO_DECK` — the non-empty
    branches the IEEE13 goldens miss: isolated buses + an isolated sub-network
    (`show_isolated_iso`), and the PARALLEL / Controlled-Switch / Isolated-PD counts +
    the `(PARALLEL:…)`/`(Control:…)`/`Isolated: …` tree annotations
    (`show_topology_mesh` summary + `show_topology_mesh_tree`). Byte-exact."""
    d.AllowEditor = False
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
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
    tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
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


# WP8.8 exit-sweep fixture: the AutoTrans special cases in the element-form Show
# reports (`ShowResults.pas` — `WriteTerminalCurrents:604`, `ShowPowers` case
# 1 `:1190`, `ShowNodeCurrentSum:3636`: `Ntimes = Nphases` instead of `NConds`,
# with the per-terminal `Inc(k, Ntimes)` block-skip in the first/last and the
# DEAD post-loop `Inc` in `ShowPowers`). A well-conditioned physical-source
# snapshot (the WPG.15 `autotrans_reg.dss` deck minus the daily/control arm) —
# NOT the vendored `AutoAuto.dss`, whose near-ideal source sits on the proven
# faer-vs-KLU conditioning floor and cannot pin report digits.
SHOW_AUTOTRANS_DECK = [
    "Set DefaultBaseFrequency=60",
    "New Circuit.show_autotrans basekv=115 phases=3 bus1=src mvasc3=15000 mvasc1=12000",
    "New Line.lsrc bus1=src bus2=high phases=3 r1=0.5 x1=2.0 r0=1.5 x0=6.0 c1=0 c0=0 length=1",
    "New AutoTrans.at phases=3 windings=2 xhx=9 "
    "wdg=1 bus=high conn=s kV=115 kVA=40000 %r=0.10 "
    "wdg=2 bus=low conn=w kV=34.5 kVA=40000 %r=0.10",
    "New Line.lf bus1=low bus2=fdr phases=3 r1=0.4 x1=1.1 r0=1.2 x0=3.3 c1=0 c0=0 length=1",
    "New Load.ld1 bus1=fdr phases=3 kv=34.5 kw=26000 pf=0.9 model=1",
    "Set voltagebases=[115 34.5]",
    "Calcvoltagebases",
    "Solve",
]

SHOW_AUTOTRANS_REPORTS = [
    ("powers e", "Power_elem_kVA.txt", "show_powers_elem_autotrans"),
    ("currents Y elem", "Curr_Elem.txt", "show_currents_elem_autotrans"),
    ("mismatch", "NodeMismatch.txt", "show_mismatch_autotrans"),
]


def gen_show_autotrans(d) -> None:
    """Capture the element-form `Show` reports on the AutoTrans snapshot deck —
    pins the `Ntimes = Nphases` AutoTrans arms against the oracle."""
    d.AllowEditor = False
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
    try:
        d.Text.Command = "clear"
        for c in SHOW_AUTOTRANS_DECK:
            d.Text.Command = c
        case = d.ActiveCircuit.Name
        d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
        for keyword, suffix, stem in SHOW_AUTOTRANS_REPORTS:
            d.Text.Command = f"show {keyword}"
            matches = list(Path(tmp).glob(f"*_{suffix}"))
            if len(matches) != 1:
                sys.exit(f"show {keyword}: expected 1 *_{suffix}, found {matches}")
            content = matches[0].read_text()
            (OUT_DIR / f"{stem}.txt").write_text(content, newline="\n")
            matches[0].unlink()  # the three reports share the tmp dir
            meta = {
                "report": keyword,
                "fixture": case,
                "suffix": suffix,
                "deck": SHOW_AUTOTRANS_DECK,
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
        tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
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


# Synthesized Dump fixture (PHASE8_PLAN §WP8.5). Two reactors exercise the
# `TReactorObj.DumpProperties` override: `r1` (series R+X, no matrices → the Z/LmH
# custom formats + no RMatrix line) and `rz` (R/X matrices → the un-`~` `RMatrix=
# (…)` / `XMatrix= (…)` lines). `debug` adds the CktElement Complete block
# (NPhases/…/NodeRef/Terminal Status/Bus Ref + the `%13.10g` YPrim G/B matrices).
# A load + solve so YPrim is built (the debug dump reads it).
DUMP_R_FIXTURE = "dumpr8"
DUMP_R_DECK = [
    f"new circuit.{DUMP_R_FIXTURE} basekv=12.47 bus1=src",
    "new reactor.r1 bus1=b1 bus2=b2 phases=3 R=1.1 X=2.2",
    "new reactor.rz bus1=b4 phases=3 Rmatrix=(1 | 0.2 1 | 0.2 0.2 1) "
    "Xmatrix=(3 | 0.5 3 | 0.5 0.5 3)",
    "new load.ld1 bus1=b4 kv=12.47 kw=100",
    "solve",
]

# Symmetrical-components reactor (`SpecType=4`) — the corpus `REACTORTest.DSS`
# `KerstingMotor` shape (WP8.5 audit-tests #1): NON-zero `Z1/Z2/Z0` through the
# `%-.8g` complex path the r1/rz fixtures never touch (both had `Z*=[0,0]`).
# Dumped single-object (`reactor.rk debug`) — also covers the non-glob form.
DUMP_SC_DECK = [
    "new circuit.dumpsc basekv=12.47 bus1=src",
    "new transformer.t1 xhl=2 kvas=25 buses=[src lb] kvs=[12.47 0.24] conns=[y y]",
    "new reactor.rk phases=3 bus1=lb.1.2.3 bus2=lb2.4.4.4 "
    "Z1=[1.9775 1.3431] Z2=[0.1203 0.3623] Z0=[1 0]",
    "set voltagebases=[12.47 0.24]",
    "calcv",
    "solve",
]

# A **disabled** reactor (WP8.5 audit-code Finding 1 + audit-tests #4): `dump
# reactor.* debug` walks enabled AND disabled objects — the disabled one prints
# `! DISABLED`, `NodeRef = "nil"` (bus refs resolve only for enabled elements) and
# `Terminal Bus Ref: [-1 …]` (Pascal `Terminal.BusRef = -1` "not set", NOT 0).
DUMP_DIS_DECK = [
    "new circuit.dumpdis basekv=12.47 bus1=src",
    "new reactor.ren bus1=b1 bus2=b2 phases=3 R=1.1 X=2.2",
    "new reactor.rdis bus1=b3 bus2=b4 phases=3 R=2.2 X=3.3 enabled=no",
    "solve",
]

# The generic base (no Pascal `DumpProperties` override): LoadShape is a plain
# `TDSSObject` (header + props, no `! ENABLED`) whose port property names already
# carry the oracle display case, so it pins the plain-object generic path for free
# (WP8.5 audit-tests #2). The PC / non-PC-CktElement generic paths need those
# classes' names corrected first (tracked for the systematic name pass).
DUMP_LS_DECK = [
    "new circuit.dumpls basekv=12.47 bus1=src",
    "new loadshape.ls1 npts=3 interval=1 mult=(1 2 3)",
]

# --- WP8.5 step 2: per-winding / matrix DumpProperties overrides ---
# Transformer (`TTransfObj.DumpProperties`): the per-winding block + `XHL…X23` +
# `Xscmatrix` + thermal/loss scalars, and — with `debug` — the `ZB`/`Y_OneVolt`/
# `Y_Terminal`/`TermRef` complex lower-triangle dumps. A 2-winding and a
# 3-winding (the latter exercises the `Xsc` off-diagonal + the order-2 `ZB`).
DUMP_XF2_DECK = [
    "new circuit.dumpxf2 basekv=115 bus1=src",
    "new transformer.t1 phases=3 windings=2 xhl=6 buses=[src mid] "
    "conns=[delta wye] kvs=[115 4.16] kvas=[5000 5000] %loadloss=1",
    # A balanced load so the winding currents are physical (not near-zero
    # no-load noise, which is faer-vs-KLU-unreproducible at 1e-12).
    "new load.ld1 bus1=mid kv=4.16 kw=2000 pf=0.9 conn=wye",
    "set voltagebases=[115 4.16]",
    "calcv",
    "solve",
]
DUMP_XF3_DECK = [
    "new circuit.dumpxf3 basekv=115 bus1=src",
    "new transformer.t3 phases=3 windings=3 xhl=6 xht=12 xlt=8 "
    "buses=[src mid low] conns=[delta wye wye] kvs=[115 12.47 4.16] "
    "kvas=[5000 3000 2000]",
    "new load.ld2 bus1=mid kv=12.47 kw=1500 pf=0.9 conn=wye",
    "new load.ld3 bus1=low kv=4.16 kw=1000 pf=0.95 conn=wye",
    "set voltagebases=[115 12.47 4.16]",
    "calcv",
    "solve",
]

# A **disabled** transformer (audit-code follow-up): after a solve, `enabled=no`
# leaves NodeRef populated, so the `WdgCurrents` result must go to `0` only via
# the `not Enabled` guard (Pascal `Transformer.pas:1530`) — pins that guard.
DUMP_XFDIS_DECK = [
    "new circuit.dumpxfd basekv=115 bus1=src",
    "new transformer.t1 phases=3 windings=2 xhl=6 buses=[src mid] "
    "conns=[delta wye] kvs=[115 4.16] kvas=[5000 5000]",
    "new load.ld1 bus1=mid kv=4.16 kw=2000 pf=0.9 conn=wye",
    "set voltagebases=[115 4.16]",
    "calcv",
    "solve",
    "transformer.t1.enabled=no",
]

# Line (`TLineObj.DumpProperties`): a sym-components line (the `%-.7g` R1/X1/R0/X0/
# C1/C0 + the `RMatrix`/`XMatrix`/`CMatrix` folded out of `Z`/`Yc`), a
# LineCode-driven line (the matrix model → the `----` sequence-parameter path), a
# **geometry** line (the `LengthMult = Len` matrix-fold branch, `length≠1`), and a
# **switch** line (`Switch=Yes`).
DUMP_LINE_GEO_DECK = [
    "new circuit.dumplgeo basekv=12.47 bus1=src",
    "new wiredata.w1 diam=0.5 gmrac=0.2 rac=0.1 runits=mi radunits=in "
    "gmrunits=ft normamps=600",
    "new linegeometry.geo1 nconds=3 nphases=3 reduce=no",
    "~ cond=1 wire=w1 x=-4 h=28 units=ft",
    "~ cond=2 wire=w1 x=-1.5 h=28.5 units=ft",
    "~ cond=3 wire=w1 x=3 h=28 units=ft",
    "new line.lg bus1=src bus2=b2 geometry=geo1 length=2 units=km",
    "set voltagebases=[12.47]",
    "calcv",
    "solve",
]
DUMP_LINE_SW_DECK = [
    "new circuit.dumplsw basekv=12.47 bus1=src",
    "new line.sw bus1=src bus2=b2 phases=3 switch=yes",
    "set voltagebases=[12.47]",
    "calcv",
    "solve",
]
DUMP_LINE_SYM_DECK = [
    "new circuit.dumplsym basekv=12.47 bus1=src",
    "new line.l1 bus1=src bus2=b2 phases=3 r1=0.1 x1=0.2 r0=0.3 x0=0.6 "
    "c1=3.4 c0=1.6 length=2 units=km",
    "set voltagebases=[12.47]",
    "calcv",
    "solve",
]
DUMP_LINE_LC_DECK = [
    "new circuit.dumpllc basekv=12.47 bus1=src",
    "new linecode.lcm nphases=3 rmatrix=(0.1 | 0.03 0.1 | 0.03 0.03 0.1) "
    "xmatrix=(0.2 | 0.05 0.2 | 0.05 0.05 0.2) "
    "cmatrix=(3 | -1 3 | -1 -1 3) units=km",
    "new line.l2 bus1=src bus2=b2 linecode=lcm length=1.5 units=km",
    "set voltagebases=[12.47]",
    "calcv",
    "solve",
]

# LineCode (`TLineCodeObj.DumpProperties`): sym + matrix models (a plain
# `TDSSObject`, no solve needed).
DUMP_LC_SYM_DECK = [
    "new circuit.dumplcs basekv=12.47 bus1=src",
    "new linecode.lc1 nphases=3 r1=0.058 x1=0.1206 r0=0.1784 x0=0.4047 "
    "c1=3.4 c0=1.6 units=kft",
]
DUMP_LC_MAT_DECK = [
    "new circuit.dumplcm basekv=12.47 bus1=src",
    "new linecode.lc2 nphases=3 rmatrix=(0.1 | 0.03 0.1 | 0.03 0.03 0.1) "
    "xmatrix=(0.2 | 0.05 0.2 | 0.05 0.05 0.2) "
    "cmatrix=(3 | -1 3 | -1 -1 3) units=km",
]

# LineGeometry (`TLineGeometryObj.DumpProperties`): the `! WARNING` banner + the
# per-conductor `Cond`/`Wire`/`X`/`H`/`Units` block (the `ActiveCond` walk).
DUMP_GEO_DECK = [
    "new circuit.dumpgeo basekv=12.47 bus1=src",
    "new wiredata.w1 diam=0.5 gmrac=0.2 rac=0.1 runits=mi radunits=in "
    "gmrunits=ft normamps=600",
    "new linegeometry.geo1 nconds=3 nphases=3 reduce=no",
    "~ cond=1 wire=w1 x=-4 h=28 units=ft",
    "~ cond=2 wire=w1 x=-1.5 h=28.5 units=ft",
    "~ cond=3 wire=w1 x=3 h=28 units=ft",
]

# XfmrCode (`TXfmrCodeObj.DumpProperties`): the transformer per-winding block
# without buses / Complete matrix block.
DUMP_XC_DECK = [
    "new circuit.dumpxc basekv=115 bus1=src",
    "new xfmrcode.xc1 phases=3 windings=2 xhl=6 conns=[delta wye] "
    "kvs=[115 4.16] kvas=[5000 5000]",
]

# --- WP8.5 step 3a: the 8 remaining leaf `DumpProperties` overrides ---
# `tools/golden/report_decks/dump3.dss` (see its header comment): every
# override except Capacitor (Fault ×2 spec types, Vsource — the implicit
# `circuit.dmp3` source, UPFC, RegControl, Monitor, EnergyMeter, Spectrum) in
# one micro deck, `clear` dropped (issued by the runner loop below).
DUMP3_DECK = [
    "Set DefaultBaseFrequency=60",
    "new circuit.dmp3 basekv=12.47 pu=1.0 phases=3 bus1=src",
    "~ r1=0.4 x1=1.6 r0=1.2 x0=4.2",
    "new linecode.lc nphases=3 r1=0.301 x1=0.667 r0=0.882 x0=2.041 c1=3.4 c0=1.6",
    "~ units=km",
    "new line.l1 bus1=src bus2=b1 linecode=lc length=1.0 units=km",
    "new transformer.tr1 phases=3 windings=2 buses=(b1, blv) conns=(delta, wye)",
    "~ kvs=(12.47, 0.48) kvas=(500, 500) xhl=5 %rs=(0.6, 0.6)",
    "new regcontrol.rc1 transformer=tr1 winding=2 vreg=115 band=3 ptratio=2.4",
    "new fault.f1 bus1=b1.1 bus2=b1.0 phases=1 r=20 ontime=0.2 temporary=yes",
    "~ minamps=3",
    "new fault.fg bus1=b1 phases=3",
    "~ gmatrix=[0.05 -0.01 -0.01 | -0.01 0.05 -0.01 | -0.01 -0.01 0.05]",
    "new spectrum.sp5 numharm=3 harmonic=[1 5 7] %mag=[100 20 12] angle=[0 30 60]",
    "new load.ldk bus1=blv phases=3 conn=wye model=1 kv=0.48 kw=150 pf=0.92",
    "~ spectrum=sp5",
    "new load.ldx bus1=b1.1 phases=1 conn=wye kv=7.2 xfkva=300 allocationfactor=0.55",
    "new load.ldc bus1=b1.2 phases=1 conn=wye kv=7.2 kwh=12000 cfactor=3.5",
    "new monitor.mon1 element=line.l1 terminal=1 mode=0",
    "new energymeter.em1 element=line.l1 terminal=1",
    "new xycurve.losses npts=3 xarray=[0.9 1 1.1] yarray=[1.0143 1.008 1.0143]",
    "new transformer.tup phases=1 windings=2 buses=(b1.1, ui.1) kvs=(7.2, 0.24)",
    "~ kvas=(50, 50) xhl=2 ppm=0",
    "new upfc.u1 phases=1 bus1=ui.1 bus2=uo.1 refkv=0.242 mode=1 losscurve=losses",
    "~ tol1=0.001 xs=0.02",
    "new upfccontrol.uc1",
    "new load.ldu phases=1 bus1=uo.1 kv=0.24 kw=10 pf=0.95",
    "set voltagebases=[12.47 0.48 0.24]",
    "calcvoltagebases",
    "Set maxiterations=100",
    "Set maxcontroliter=100",
    "solve",
]

# WPG.2: a `mode=Time` (SolveGeneralTime) monitor dump. SolveGeneralTime is the
# one solve mode that never calls `MonitorClass.SaveAll`, so at dump time the
# monitor's `MonBuffer` still holds every sample taken (BufPtr != 0) — the
# NON-empty `// Bufptr=`/`// Buffer=` case the ordinary (snapshot/daily) dump
# fixtures can never reach (they flush). `Set LoadShapeClass=Daily` makes the
# load follow `d4`, so the four buffered records carry DIFFERENT V/I per step —
# an off-by-one in the port's `flushed_records*stride` pending slice would drop
# or duplicate a record and mismatch byte-for-byte.
DUMP_MONTIME_DECK = [
    "Set DefaultBaseFrequency=60",
    "new circuit.montime basekv=12.47 pu=1.0 phases=3 bus1=src",
    "~ r1=0.4 x1=1.6 r0=1.2 x0=4.2",
    "new line.l1 bus1=src bus2=b1 phases=3 r1=0.3 x1=0.9 r0=0.9 x0=2.7 c1=3 c0=1.5 length=1 units=km",
    "new loadshape.d4 npts=4 interval=1 mult=(0.5 0.75 1.0 0.8)",
    "new load.ld1 bus1=b1 phases=3 kv=12.47 kw=600 pf=0.95 model=1 daily=d4",
    "new monitor.m1 element=line.l1 terminal=1 mode=0",
    "set voltagebases=[12.47]",
    "calcvoltagebases",
    "Set LoadShapeClass=Daily",
    "Set mode=Time stepsize=1h number=4",
    "solve",
]

# `tools/golden/report_decks/dump_capacitor.dss`: the Capacitor override,
# isolated because of the proven upstream ASLR-garbage bug in `CMatrix`/
# `FaultRate`/`pctPerm` (see the mask below).
DUMP_CAP_DECK = [
    "Set DefaultBaseFrequency=60",
    "new circuit.dmpc basekv=12.47 pu=1.0 phases=3 bus1=src",
    "~ r1=0.4 x1=1.6 r0=1.2 x0=4.2",
    "new linecode.lc nphases=3 r1=0.301 x1=0.667 r0=0.882 x0=2.041 c1=3.4 c0=1.6",
    "~ units=km",
    "new line.l1 bus1=src bus2=b1 linecode=lc length=1.0 units=km",
    "new capacitor.cm1 bus1=b1 phases=3",
    "~ cmatrix=[3.2 -0.9 -0.3 | -0.5 3.5 -0.7 | -0.4 -1.1 3.1]",
    "new capacitor.cs1 bus1=b1 phases=3 kvar=100 kv=12.47 numsteps=2",
    "~ states=[1 0]",
    "new load.ld1 bus1=b1 phases=3 conn=wye model=1 kv=12.47 kw=300 pf=0.92",
    "set voltagebases=[12.47]",
    "calcvoltagebases",
    "Set maxiterations=100",
    "solve",
]

# WPG.14 — Isource has no Pascal `DumpProperties` override, so it dumps via
# the ancestor `TPCElement.DumpProperties` ordering (ENABLED + Y-block +
# VARIABLES + props). Two units: `i1` (3-phase, full spec — daily/spectrum
# refs, ScanType=zero/Sequence=neg) and `iz` (1-phase, disabled — the
# `! DISABLED` / unresolved-NodeRef path, same coverage as `DUMP_DIS_DECK`
# for Reactor). A load + solve so the debug Y-block/NodeRef render live
# values (Isource's own Yprim is always zero regardless).
DUMP_ISRC_DECK = [
    "Set DefaultBaseFrequency=60",
    "new circuit.dmpisrc basekv=12.47 pu=1.0 phases=3 bus1=src",
    "~ r1=0.4 x1=1.6 r0=1.2 x0=4.2",
    "new loadshape.ds1 npts=2 interval=1 mult=(0.5 1.5)",
    "new spectrum.sp1 numharm=2 harmonic=[1 3] %mag=[100 30] angle=[0 15]",
    "new isource.i1 bus1=src.1.2.3 phases=3 amps=25 angle=45 frequency=60 "
    "scantype=zero sequence=neg daily=ds1 spectrum=sp1",
    "new isource.iz bus1=src.2 phases=1 amps=6 angle=-120 enabled=no",
    "new load.ld1 bus1=src phases=3 conn=wye model=1 kv=12.47 kw=300 pf=0.92",
    "set voltagebases=[12.47]",
    "calcvoltagebases",
    "solve",
]

# AutoTrans DumpProperties (WPG.15 Stage A): the bare `dump autotrans.<obj>`
# form — the per-winding block (conn=Series, kv/kVA/tap/%r/Rdcohms as %.7g),
# XHX/XHT/XXT (no X12/X13/X23), the flat Xscmatrix, and the generic tail. NO
# solve (the auto YPrim/solve path is Stage B; DumpProperties is solve-
# independent — ZB/Y_Term come from edit-time RecalcElementData). The elements
# sit on buses ISOLATED from the source so the pre-solve `WdgCurrents` readout
# is a deterministic zero on both engines (the source bus's initialized NodeV
# would otherwise leak a tiny anti-float current into the oracle side only). A
# 2-winding 115/34.5 and a 3-winding 345/161/13.8 (delta tertiary). The
# debug/Complete form (ZB/Y_Terminal matrices) is added in Stage B once solve
# works.
DUMP_AT_DECK = [
    "new circuit.dumpat basekv=115 bus1=src",
    "new autotrans.t1 phases=3 windings=2 xhx=6 buses=[h mid] "
    "conns=[s w] kvs=[115 34.5] kvas=[40000 40000] %loadloss=0.2",
    "new autotrans.t3 phases=3 windings=3 xhx=7.23 xht=24.45 xxt=28.45 "
    "buses=[h3 low3 tert3] conns=[s w d] kvs=[345 161 13.8] "
    "kvas=[330000 330000 72000] %imag=0.0329 %noloadloss=0.024",
    # No `calcv`/`solve`: both build the system Y, and the auto YPrim path is
    # Stage B (would abort here). DumpProperties is edit-time-only, so the dump
    # is unchanged, and `WdgCurrents` stays a deterministic zero (no NodeV).
]

# GIC family DumpProperties (WPG.16): a small 0.1-Hz GIC net exercising all
# three classes. GICLine has a real `DumpProperties` override (the Complete-only
# BaseFrequency/Volts/VMag/VE/VN block + the series Z matrix); GICTransformer
# (shunt PD) and GICsource (NON_PCPD source) fall through to the generic base
# dumps. The geodesy GICLine (gl2) pins VE/VN; the YY GICTransformer (tg2) pins
# the 4-terminal generic block; the spliced GICsource (seg) pins the
# NON_PCPD/TPCElement dump ordering.
DUMP_GIC_DECK = [
    "new circuit.dumpgic basekv=345 phases=3 bus1=b1 mvasc3=2000000 2000000",
    "new gicline.gl1 bus1=b1 bus2=b2 R=3.5 Volts=120 Angle=0",
    "new gicline.gl2 bus1=b2 bus2=b3 R=2.8 EN=1.0 EE=1.0 "
    "Lat1=33.613499 Lon1=-87.373673 Lat2=33.547885 Lon2=-86.074605",
    "new gictransformer.tg1 busH=b1 busNH=b1.4.4.4 R1=0.12 type=GSU",
    "new gictransformer.tg2 busH=b2 busNH=b2.4.4.4 busX=b2x busNX=b2.4.4.4 "
    "R1=0.2 R2=0.1 type=YY",
    "new line.seg bus1=b3 bus2=b4 phases=3 r1=2.7 x1=0.1 r0=2.7 x0=0.1 "
    "c1=0 c0=0 length=1",
    "new gicsource.seg Volts=110 Angle=0 Frequency=0.1",
    "new reactor.g1 phases=3 bus1=b1 r=0.20 x=0",
    "new reactor.gg1 phases=1 bus1=b1.4 r=0.20 x=0",
    "new reactor.gg2 phases=1 bus1=b2.4 r=0.15 x=0",
    "new reactor.g4 phases=3 bus1=b4 r=0.25 x=0",
    "set voltagebases=[345]",
    "calcvoltagebases",
    "set frequency=0.1",
    "solve",
]

# (stem, deck, report). Each captures `<case>_PropertyDump.txt` (Dump sets
# GlobalResult), read back by the Rust golden via `dss.last_result_file()`.
DUMP_DECKS = [
    ("dump_gicline", DUMP_GIC_DECK, "gicline.gl2"),
    ("dump_gicline_debug", DUMP_GIC_DECK, "gicline.gl2 debug"),
    ("dump_gictransformer", DUMP_GIC_DECK, "gictransformer.tg2"),
    ("dump_gictransformer_debug", DUMP_GIC_DECK, "gictransformer.tg2 debug"),
    ("dump_gicsource", DUMP_GIC_DECK, "gicsource.seg"),
    ("dump_gicsource_debug", DUMP_GIC_DECK, "gicsource.seg debug"),
    ("dump_autotrans", DUMP_AT_DECK, "autotrans.t1"),
    ("dump_autotrans3", DUMP_AT_DECK, "autotrans.t3"),
    ("dump_reactor", DUMP_R_DECK, "reactor.*"),
    ("dump_reactor_debug", DUMP_R_DECK, "reactor.* debug"),
    ("dump_reactor_symcomp", DUMP_SC_DECK, "reactor.rk debug"),
    ("dump_reactor_disabled", DUMP_DIS_DECK, "reactor.* debug"),
    ("dump_loadshape", DUMP_LS_DECK, "loadshape.ls1"),
    ("dump_transformer", DUMP_XF2_DECK, "transformer.t1 debug"),
    ("dump_transformer3", DUMP_XF3_DECK, "transformer.t3 debug"),
    ("dump_transformer_disabled", DUMP_XFDIS_DECK, "transformer.t1"),
    ("dump_line_sym", DUMP_LINE_SYM_DECK, "line.l1"),
    ("dump_line_lc", DUMP_LINE_LC_DECK, "line.l2"),
    ("dump_line_geo", DUMP_LINE_GEO_DECK, "line.lg"),
    ("dump_line_switch", DUMP_LINE_SW_DECK, "line.sw"),
    ("dump_linecode_sym", DUMP_LC_SYM_DECK, "linecode.lc1"),
    ("dump_linecode_matrix", DUMP_LC_MAT_DECK, "linecode.lc2"),
    ("dump_linegeometry", DUMP_GEO_DECK, "linegeometry.geo1"),
    ("dump_xfmrcode", DUMP_XC_DECK, "xfmrcode.xc1"),
    ("dump_vsource", DUMP3_DECK, "vsource.source debug"),
    ("dump_upfc", DUMP3_DECK, "upfc.u1 debug"),
    ("dump_regcontrol", DUMP3_DECK, "regcontrol.rc1 debug"),
    ("dump_monitor", DUMP3_DECK, "monitor.mon1 debug"),
    ("dump_monitor_montime", DUMP_MONTIME_DECK, "monitor.m1 debug"),
    ("dump_energymeter", DUMP3_DECK, "energymeter.em1 debug"),
    ("dump_spectrum", DUMP3_DECK, "spectrum.sp5 debug"),
    ("dump_fault", DUMP3_DECK, "fault.f1 debug"),
    ("dump_fault_gmatrix", DUMP3_DECK, "fault.fg debug"),
    ("dump_capacitor_cmatrix", DUMP_CAP_DECK, "capacitor.cm1 debug"),
    ("dump_capacitor_steps", DUMP_CAP_DECK, "capacitor.cs1 debug"),
    ("dump_isource", DUMP_ISRC_DECK, "isource.*"),
    ("dump_isource_debug", DUMP_ISRC_DECK, "isource.i1 debug"),
]

# WP8.5 step 3b — the whole-circuit / aux Dump forms, all over the same
# `dump3.dss` fixture: bare `dump` / `dump debug` (every CktElement + every
# general DSSObj + the Solution options; debug adds the `Circuit.DebugDump`
# header, the per-element Complete blocks and the system-Y dump),
# `dump solution`, the two hash-list dumps, `DumpAllDSSCommands` and
# `DumpAllocationFactors`. Every form sets `GlobalResult` to the produced
# path, so the same capture loop applies. (stem, report, suffix).
DUMP3_AUX = [
    ("dump3_bare", "", "PropertyDump.txt"),
    ("dump3_debug", "debug", "PropertyDump.txt"),
    ("dump3_solution", "solution", "PropertyDump.txt"),
    ("dump3_buslist", "buslist", "Bus_Hash_List.txt"),
    ("dump3_devicelist", "devicelist", "Device_Hash_List.txt"),
    ("dump3_commands", "commands", "DSSCommandsDump.txt"),
    ("dump3_alloc", "alloc", "AllocationFactors.txt"),
]

# Line prefixes dropped from BOTH sides of `dump_capacitor_*` (probe-proven
# ASLR-garbage — see the report_decks README and `investigations/`): the
# oracle's captured golden never contains them (stripped at capture time
# below); the Rust replay drops the same prefixes from its own output before
# the byte-exact compare (`golden_reports.rs::run_deck_dump_exact_masked`).
DUMP_CAPACITOR_GARBAGE_PREFIXES = ("~ CMatrix=(", "~ FaultRate=", "~ pctPerm=")

# `[<ClassName>]` sections dropped from the `dump3_commands` golden at
# capture: these classes are NOT_PORTED, so the Rust registry — and therefore
# its byte-exact `Dump commands` output — has no section for them yet. Prune
# this tuple (and regenerate) as each class lands; every *ported* class's
# section stays byte-pinned (order, property names, help). Isource landed in
# WPG.14; AutoTrans in WPG.15; GICsource/GICLine/GICTransformer in WPG.16 —
# all pruned. The list is now empty (every registered class has a section).
DUMP_COMMANDS_UNPORTED_SECTIONS: tuple[str, ...] = ()


def strip_unported_class_sections(content: str) -> str:
    """Drop each `[<name>]` section in DUMP_COMMANDS_UNPORTED_SECTIONS (header
    line through the line before the next `[` header)."""
    out, skipping = [], False
    for ln in content.splitlines(keepends=True):
        if ln.startswith("["):
            skipping = ln.rstrip("\r\n") in [
                f"[{s}]" for s in DUMP_COMMANDS_UNPORTED_SECTIONS
            ]
        if not skipping:
            out.append(ln)
    return "".join(out)


def gen_dump_decks(d) -> None:
    """Capture the oracle's `Dump` on the synthesized decks. `Dump` sets
    `GlobalResult` to the produced file's path (`<case>_PropertyDump.txt` or
    the aux-form fixed name), so the Rust golden (`golden_reports.rs`) reads
    `dss.last_result_file()`."""
    d.AllowEditor = False
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    all_decks = [(stem, deck, report, "PropertyDump.txt") for stem, deck, report in DUMP_DECKS]
    all_decks += [(stem, DUMP3_DECK, report, suffix) for stem, report, suffix in DUMP3_AUX]
    for stem, deck, report, suffix in all_decks:
        tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
        try:
            d.Text.Command = "clear"
            for c in deck:
                d.Text.Command = c
            case = d.ActiveCircuit.Name
            d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
            d.Text.Command = f"dump {report}".rstrip()
            produced = Path(d.Text.Result)  # GlobalResult = produced path
            content = produced.read_text()
            if stem.startswith("dump_capacitor"):
                lines = content.splitlines(keepends=True)
                lines = [
                    ln
                    for ln in lines
                    if not ln.startswith(DUMP_CAPACITOR_GARBAGE_PREFIXES)
                ]
                content = "".join(lines)
            if stem == "dump3_commands":
                content = strip_unported_class_sections(content)
            (OUT_DIR / f"{stem}.txt").write_text(content, newline="\n")
            meta = {
                "report": report,
                "fixture": case,
                "suffix": suffix,
                "deck": deck,
            }
            (OUT_DIR / f"{stem}.meta.json").write_text(
                json.dumps(meta, indent=2) + "\n", newline="\n"
            )
            print(f"wrote {stem}.txt ({len(content)} bytes)")
        finally:
            d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
            shutil.rmtree(tmp, ignore_errors=True)


# --- WP8.5 step 4: the `Save` forms (Pascal `DoSaveCmd`) ---
# `tools/golden/report_decks/save_forms.dss` (see its header comment): a 4-step
# daily deck with a sampled EnergyMeter + Monitor, so `save` (the meters default
# branch) writes real `MTR_em1.csv` registers, `save voltages` a solved
# `svf_SavedVoltages.txt`, and `save load` two explicitly-set-props load lines.
SAVE_FORMS_DECK = [
    "Set DefaultBaseFrequency=60",
    "new circuit.svf basekv=12.47 pu=1.0 phases=3 bus1=src",
    "~ r1=0.4 x1=1.6 r0=1.2 x0=4.2",
    "new linecode.lc nphases=3 r1=0.301 x1=0.667 r0=0.882 x0=2.041 c1=3.4 c0=1.6",
    "~ units=km",
    "new loadshape.day npts=4 interval=1",
    "~ mult=(0.6 0.9 1.0 0.7)",
    "new line.l1 bus1=src bus2=b1 linecode=lc length=1.0 units=km",
    "new line.l2 bus1=b1 bus2=b2 linecode=lc length=0.7 units=km",
    "new load.ld1 bus1=b1 phases=3 conn=wye model=1 kv=12.47 kw=400 pf=0.92",
    "~ daily=day",
    "new load.ld2 bus1=b2 phases=3 conn=wye model=1 kv=12.47 kw=300 pf=0.95",
    "~ daily=day",
    "new capacitor.c2 bus1=b2 phases=3 kvar=150 kv=12.47",
    "new monitor.mon1 element=line.l1 terminal=1 mode=1",
    "new energymeter.em1 element=line.l1 terminal=1",
    "set voltagebases=[12.47]",
    "calcvoltagebases",
    "Set maxiterations=100",
    "set mode=daily stepsize=1h number=4",
    "solve",
]

# (stem, full save command, produced filename). Each stem replays the deck
# FRESH: `Flg.HasBeenSaved` persists across `save` commands within a session
# (probe-proven 2026-07-07: a second `save load` writes 0 records and DELETES
# the file), so goldens must be first-save captures. `save` sets `GlobalResult`
# to the RELATIVE `MTR_<name>.csv`; `save load`'s default filename is the bare
# class name with NO extension; `save voltages` names `<case>_SavedVoltages.txt`
# — all probe-proven, so the produced file is read by its fixed name.
SAVE_DECKS = [
    ("save_mtr", "save", "MTR_em1.csv"),
    ("save_voltages", "save voltages", "svf_SavedVoltages.txt"),
    ("save_class_load", "save load", "load"),
]


def gen_save_decks(d) -> None:
    """Capture the oracle's `Save` outputs on the save_forms deck (WP8.5 step 4)."""
    d.AllowEditor = False
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for stem, command, produced_name in SAVE_DECKS:
        tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
        try:
            d.Text.Command = "clear"
            for c in SAVE_FORMS_DECK:
                d.Text.Command = c
            case = d.ActiveCircuit.Name
            d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
            d.Text.Command = command
            produced = Path(tmp) / produced_name
            content = produced.read_text()  # universal newlines -> LF
            (OUT_DIR / f"{stem}.txt").write_text(content, newline="\n")
            meta = {
                "report": command,
                "fixture": case,
                "suffix": produced_name,
                "deck": SAVE_FORMS_DECK,
            }
            (OUT_DIR / f"{stem}.meta.json").write_text(
                json.dumps(meta, indent=2) + "\n", newline="\n"
            )
            print(f"wrote {stem}.txt ({len(content)} bytes)")
        finally:
            d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
            shutil.rmtree(tmp, ignore_errors=True)


# --- WP8.6 step 4: Interpolate, gated via `export buscoords` -----------------
# The pre-validated fixture deck (tools/golden/report_decks/interp.dss, see its
# header + the report_decks README): an EnergyMeter zone where only the anchor
# buses src/b1/b5/c2 carry coordinates (SetBusXY); `interpolate` fills b2/b3/b4
# evenly between b1 and b5 and c1 between c2 and b3
# (TEnergyMeterObj.InterpolateCoordinates + CalcBusCoordinates). The golden is
# the `export buscoords` CSV afterwards — pure f64 anchor arithmetic on both
# engines, byte-exact.


def load_report_deck(name: str) -> list[str]:
    """Read a fixture deck from tools/golden/report_decks (the single source),
    dropping comments, blank lines, and the leading `clear` (the runner issues
    its own). The filtered command list goes into the golden `.meta.json`,
    which both engines replay."""
    p = Path(__file__).resolve().parent / "report_decks" / name
    deck = []
    for raw in p.read_text().splitlines():
        line = raw.strip()
        if not line or line.startswith("//"):
            continue
        if line.lower() == "clear":
            continue
        deck.append(line)
    return deck


def gen_interp(d) -> None:
    """Capture the oracle's `export buscoords` after `interpolate` (WP8.6)."""
    deck = load_report_deck("interp.dss")
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
    try:
        d.Text.Command = "clear"
        for c in deck:
            d.Text.Command = c
        case = d.ActiveCircuit.Name
        d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
        d.Text.Command = "export buscoords"
        produced = Path(d.Text.Result)  # GlobalResult = produced path
        content = produced.read_text()  # universal newlines -> LF
        (OUT_DIR / "export_buscoords_interp.txt").write_text(content, newline="\n")
        meta = {
            "report": "buscoords",
            "fixture": case,
            "suffix": "EXP_BUSCOORDS.csv",
            "deck": deck,
        }
        (OUT_DIR / "export_buscoords_interp.meta.json").write_text(
            json.dumps(meta, indent=2) + "\n", newline="\n"
        )
        print(
            f"wrote export_buscoords_interp.txt ({len(content)} bytes), "
            f"{content.count(chr(10))} lines"
        )
    finally:
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp, ignore_errors=True)


# --- WP8.6 step 5: the Distribute command (PHASE8_PLAN §WP8.6) ---
# `tools/golden/report_decks/distrib.dss`: loads with three different kW bases
# and specs (kW/PF, xfkva+allocationfactor, kwh+cfactor) so the Proportional
# weights differ per load, plus a disabled load (pins the enabled filter AND
# Uniform's count — which includes disabled loads, probe-proven
# `Utilities.pas:1349`). Five deterministic variants (incl. `mw=`, which is
# `kW := value*1000`); `how=Random` is RNG-carried upstream (`randomize`,
# `Utilities.pas:1400`) and never golden-gated. The `what=Load` variant passes an explicit `file=` that the
# oracle overrides to `DistLoads.dss` (probe-proven). `Distribute` writes the
# file relative to the engine cwd (= datapath) and sets `GlobalResult` to the
# bare filename.
DISTRIB_DECK = [
    "Set DefaultBaseFrequency=60",
    "new circuit.dst basekv=12.47 pu=1.0 phases=3 bus1=src",
    "~ r1=0.4 x1=1.6 r0=1.2 x0=4.2",
    "new linecode.lc nphases=3 r1=0.301 x1=0.667 r0=0.882 x0=2.041 c1=3.4 c0=1.6",
    "~ units=km",
    "new line.l1 bus1=src bus2=b1 linecode=lc length=1.0 units=km",
    "new line.l2 bus1=b1 bus2=b2 linecode=lc length=0.8 units=km",
    "new load.ld_kw bus1=b2 phases=3 conn=wye model=1 kv=12.47 kw=800 pf=0.92",
    "new load.ld_xf bus1=b1.1 phases=1 conn=wye kv=7.2 xfkva=300",
    "~ allocationfactor=0.55",
    "new load.ld_ac bus1=b1.2 phases=1 conn=wye kv=7.2 kwh=12000 cfactor=3.5",
    "new load.ld_off bus1=b2.3 phases=1 conn=wye kv=7.2 kw=120 pf=0.9 enabled=no",
    "set voltagebases=[12.47]",
    "calcvoltagebases",
    "Set maxiterations=100",
    "solve",
]
DISTRIB_VARIANTS = [
    ("distribute kw=1500 pf=0.95", "DistGenerators.dss", "distrib_proportional"),
    ("distribute kw=1200 how=Uniform pf=0.9", "DistGenerators.dss", "distrib_uniform"),
    ("distribute kw=900 how=Skip skip=1 pf=0.85", "DistGenerators.dss", "distrib_skip"),
    ("distribute kw=750 what=Load file=Explicit.dss", "DistLoads.dss", "distrib_load"),
    # MW= is `kW := value*1000` (`DoDistributeCmd` ordinal 6): mw=1.5 must
    # reproduce the kw=1500 proportional output exactly.
    ("distribute mw=1.5 pf=0.95", "DistGenerators.dss", "distrib_mw"),
]


def gen_distribute(d) -> None:
    """Capture the oracle's `Distribute` output scripts (five deterministic
    variants). Each runs in a fresh dir (the command refuses to overwrite an
    existing file — error 721); `Text.Result` (GlobalResult) is the bare
    produced filename, pinned into the meta."""
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for command, fname, stem in DISTRIB_VARIANTS:
        tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
        try:
            d.Text.Command = "clear"
            for c in DISTRIB_DECK:
                d.Text.Command = c
            case = d.ActiveCircuit.Name
            d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
            d.Text.Command = command
            if d.Text.Result != fname:
                sys.exit(f"{stem}: expected GlobalResult {fname!r}, got {d.Text.Result!r}")
            content = (Path(tmp) / fname).read_text()  # universal newlines -> LF
            (OUT_DIR / f"{stem}.txt").write_text(content, newline="\n")
            meta = {
                "command": command,
                "fixture": case,
                "file": fname,
                "deck": DISTRIB_DECK,
            }
            (OUT_DIR / f"{stem}.meta.json").write_text(
                json.dumps(meta, indent=2) + "\n", newline="\n"
            )
            print(f"wrote {stem}.txt ({len(content)} bytes), {content.count(chr(10))} lines")
        finally:
            d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
            shutil.rmtree(tmp, ignore_errors=True)


# --- WP8.6 step 6: Uuids + `Export Uuids` (PHASE8_PLAN §WP8.6) ---
# `tools/golden/report_decks/uuids.dss` + `uuids_pre.csv`: the deck preloads a
# UUID for the circuit, EVERY bus, EVERY ckt element, the library linecode AND
# the three hashed keys the CIM exporter auto-creates (`DefaultCircuitUUIDs` →
# Station=Station=1 / GeoRgn=GeoRgn=1 / SubGeoRgn=SubGeoRgn=1) — any object
# left out gets a RANDOM v4 at export time (`NamedObject.pas` CreateUUID4),
# which can never be oracle-pinned. The `@FIXTURES@` token resolves to the
# absolute `report_decks` dir on BOTH engines (here and `golden_reports.rs`);
# the meta stores the token form so the golden is machine-portable. The
# produced `<case>_EXP_UUIDS.csv` is byte-exact; probe-proven quirk pinned at
# capture time: `Text.Result` (GlobalResult) stays EMPTY after `export uuids`,
# unlike every other export.
FIXTURES_DIR = Path(__file__).resolve().parent / "report_decks"
UUIDS_DECK = [
    "Set DefaultBaseFrequency=60",
    "new circuit.uid basekv=12.47 pu=1.0 phases=3 bus1=src",
    "~ r1=0.4 x1=1.6 r0=1.2 x0=4.2",
    "new linecode.lc nphases=3 r1=0.301 x1=0.667 r0=0.882 x0=2.041 c1=3.4 c0=1.6",
    "~ units=km",
    "new line.l1 bus1=src bus2=b1 linecode=lc length=1.0 units=km",
    "new load.ld1 bus1=b1 phases=3 conn=wye model=1 kv=12.47 kw=300 pf=0.92",
    "new monitor.mon1 element=line.l1 terminal=1 mode=0",
    "set voltagebases=[12.47]",
    "calcvoltagebases",
    "Set maxiterations=100",
    "solve",
    "uuids file=@FIXTURES@/uuids_pre.csv",
]


def gen_uuids(d) -> None:
    """Capture the oracle's `Export Uuids` on the fully-preloaded uuids fixture."""
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
    try:
        d.Text.Command = "clear"
        for c in UUIDS_DECK:
            d.Text.Command = c.replace("@FIXTURES@", FIXTURES_DIR.as_posix())
        case = d.ActiveCircuit.Name
        d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
        d.Text.Command = "export uuids"
        if d.Text.Result != "":
            sys.exit(f"export uuids: expected EMPTY GlobalResult, got {d.Text.Result!r}")
        matches = list(Path(tmp).glob("*_EXP_UUIDS.csv"))
        if len(matches) != 1:
            sys.exit(f"export uuids: expected 1 *_EXP_UUIDS.csv, found {matches}")
        content = matches[0].read_text()  # universal newlines -> LF
        (OUT_DIR / "export_uuids.txt").write_text(content, newline="\n")
        meta = {
            "report": "uuids",
            "fixture": case,
            "suffix": "EXP_UUIDS.csv",
            "deck": UUIDS_DECK,  # token form; both engines resolve @FIXTURES@
        }
        (OUT_DIR / "export_uuids.meta.json").write_text(
            json.dumps(meta, indent=2) + "\n", newline="\n"
        )
        print(f"wrote export_uuids.txt ({len(content)} bytes), {content.count(chr(10))} lines")
    finally:
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp, ignore_errors=True)


# --- WPG.17: LoadShape/TShape/PriceShape SngSave/DblSave binary writers -------
# `Action=SngSave/DblSave` writes the multiplier arrays as packed little-endian
# IEEE-754 streams (Pascal `SaveToDblFile`/`SaveToSngFile`, LoadShape.pas:1880/
# 1939, TempShape.pas:528/548, PriceShape.pas:547/568). LoadShape splits into
# `<name>_P`/`<name>_Q`; TShape/PriceShape write the bare `<name>`. The goldens
# are the RAW BYTES (no newline normalization) — the strongest possible pin for a
# pure-binary format. The Rust runner (`golden_reports.rs::binsave_matches_oracle`)
# replays the same deck+actions and asserts `Vec<u8>` byte equality.
BINSAVE_DECK = [
    "new circuit.binsave basekv=12.47 bus1=src",
    "new loadshape.bs npts=4 interval=1 mult=(0.5 0.75 1.0 0.8) qmult=(0.1 0.2 0.3 0.4)",
    "new tshape.ts npts=3 interval=1 temp=(10 20 30)",
    "new priceshape.ps npts=3 interval=1 price=(1.5 2.5 3.5)",
]
BINSAVE_ACTIONS = [
    "edit loadshape.bs action=sngsave",
    "edit loadshape.bs action=dblsave",
    "edit tshape.ts action=sngsave",
    "edit tshape.ts action=dblsave",
    "edit priceshape.ps action=sngsave",
    "edit priceshape.ps action=dblsave",
]
# (produced filename in the datapath, golden basename under tests/golden/reports)
BINSAVE_FILES = [
    ("bs_P.sng", "loadshape_binsave_p_sng.bin"),
    ("bs_Q.sng", "loadshape_binsave_q_sng.bin"),
    ("bs_P.dbl", "loadshape_binsave_p_dbl.bin"),
    ("bs_Q.dbl", "loadshape_binsave_q_dbl.bin"),
    ("ts.sng", "tshape_binsave_sng.bin"),
    ("ts.dbl", "tshape_binsave_dbl.bin"),
    ("ps.sng", "priceshape_binsave_sng.bin"),
    ("ps.dbl", "priceshape_binsave_dbl.bin"),
]


# --- WPG.20: MMF-backed (MemoryMapping=Yes) SngSave/DblSave --------------------
# Under `MemoryMapping=Yes` the P/Q multipliers live in a memory-mapped file, not
# a script array; `SaveToDblFile`/`SaveToSngFile` re-read each value through
# `InterpretDblArrayMMF` (LoadShape.pas:1898-1905/1956-1963 P; 1921-1927/1982-1988
# Q). The Q file is written iff `Assigned(dQ)` — i.e. a `qmult=` MMF directive was
# given (`CustomSetRaw` :791-802 allocates a 2-elem `dQ` sentinel so `Assigned` is
# true; the actual bytes come from re-reading `mmViewQ`). The MMF source fixtures
# (`@FIXTURES@/binsave_mmf_*.sng/.dbl`) resolve on BOTH engines. `mp` = P sng-src +
# Q sng-src (both `_P`/`_Q` written); `md` = P dbl-src, no `qmult` (only `_P`, no
# `_Q` — the `Assigned(dQ)=false` case). Oracle-probed 2026-07-11: bytes equal the
# f32-narrowed (sng-src) / raw-f64 (dbl-src) values, and no `md_Q.*` is emitted.
BINSAVE_MMF_DECK = [
    "new circuit.binsavemmf basekv=12.47 bus1=src",
    "new loadshape.mp npts=4 interval=1 MemoryMapping=Yes "
    "mult=(sngfile=@FIXTURES@/binsave_mmf_p.sng) "
    "qmult=(sngfile=@FIXTURES@/binsave_mmf_q.sng)",
    "new loadshape.md npts=4 interval=1 MemoryMapping=Yes "
    "mult=(dblfile=@FIXTURES@/binsave_mmf_p.dbl)",
]
BINSAVE_MMF_ACTIONS = [
    "edit loadshape.mp action=sngsave",
    "edit loadshape.mp action=dblsave",
    "edit loadshape.md action=sngsave",
    "edit loadshape.md action=dblsave",
]
# (produced filename in the datapath, golden basename under tests/golden/reports)
BINSAVE_MMF_FILES = [
    ("mp_P.sng", "loadshape_binsave_mmf_mp_p_sng.bin"),
    ("mp_Q.sng", "loadshape_binsave_mmf_mp_q_sng.bin"),
    ("mp_P.dbl", "loadshape_binsave_mmf_mp_p_dbl.bin"),
    ("mp_Q.dbl", "loadshape_binsave_mmf_mp_q_dbl.bin"),
    ("md_P.sng", "loadshape_binsave_mmf_md_p_sng.bin"),
    ("md_P.dbl", "loadshape_binsave_mmf_md_p_dbl.bin"),
]
# Files that must NOT be written (Assigned(dQ)=false for `md` — no qmult).
BINSAVE_MMF_ABSENT = ["md_Q.sng", "md_Q.dbl"]
# Per-action `GlobalResult` decomposition (datapath-relative filenames + their
# `<obj>=[<ftag>=…]` tags), single-sourced here for the meta AND the generator's
# self-check against the captured oracle string (audit settlement). The Rust
# runner (`golden_reports.rs::binsave_mmf_matches_oracle`) rebuilds the identical
# form against its own scratch dir; the Q clause is joined by `AppendGlobalResult`'s
# `', '` + the clause's leading space -> `,  ` (comma + two spaces).
BINSAVE_MMF_RESULT_FILES = [
    ["mp_P.sng", "mp_Q.sng"],
    ["mp_P.dbl", "mp_Q.dbl"],
    ["md_P.sng"],
    ["md_P.dbl"],
]
BINSAVE_MMF_RESULT_TAGS = [
    ["mult", "sngfile", "Qmult", "sngfile"],
    ["mult", "dblfile", "Qmult", "dblfile"],
    ["mult", "sngfile"],
    ["mult", "dblfile"],
]


def gen_loadshape_binsave(d) -> None:
    """Capture the oracle's SngSave/DblSave binary outputs for LoadShape,
    TShape and PriceShape as raw bytes."""
    tmp = tempfile.mkdtemp(prefix="dss_gen_binsave_")
    produced_bytes: dict[str, bytes] = {}
    try:
        d.Text.Command = "clear"
        for c in BINSAVE_DECK:
            d.Text.Command = c
        d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
        for c in BINSAVE_ACTIONS:
            d.Text.Command = c
        for produced, golden in BINSAVE_FILES:
            produced_bytes[golden] = (Path(tmp) / produced).read_bytes()
    finally:
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp, ignore_errors=True)

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for _produced, golden in BINSAVE_FILES:
        (OUT_DIR / golden).write_bytes(produced_bytes[golden])
    meta = {
        "report": "loadshape_binsave",
        "deck": BINSAVE_DECK,
        "actions": BINSAVE_ACTIONS,
        "files": [{"produced": p, "golden": g} for p, g in BINSAVE_FILES],
    }
    (OUT_DIR / "loadshape_binsave.meta.json").write_text(
        json.dumps(meta, indent=2) + "\n", newline="\n"
    )
    total = sum(len(b) for b in produced_bytes.values())
    print(f"wrote {len(BINSAVE_FILES)} binsave goldens ({total} bytes total)")

    gen_loadshape_binsave_mmf(d)


def gen_loadshape_binsave_mmf(d) -> None:
    """WPG.20: capture the MMF-backed (MemoryMapping=Yes) SngSave/DblSave bytes,
    including the `Assigned(dQ)`-gated Q-file emission."""
    tmp = tempfile.mkdtemp(prefix="dss_gen_binsave_mmf_")
    fixtures = FIXTURES_DIR.as_posix()
    produced_bytes: dict[str, bytes] = {}
    results: list[str] = []
    try:
        d.Text.Command = "clear"
        for c in BINSAVE_MMF_DECK:
            d.Text.Command = c.replace("@FIXTURES@", fixtures)
        d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
        for c in BINSAVE_MMF_ACTIONS:
            d.Text.Command = c
            results.append(d.Text.Result)
        for produced, golden in BINSAVE_MMF_FILES:
            produced_bytes[golden] = (Path(tmp) / produced).read_bytes()
        for absent in BINSAVE_MMF_ABSENT:
            if (Path(tmp) / absent).exists():
                sys.exit(f"binsave_mmf: expected NO {absent} (Assigned(dQ) false)")
        # Audit settlement: verify the hand-written result_files/result_tags
        # (which the Rust test rebuilds the expected GlobalResult from) actually
        # reproduce the TRUE oracle GlobalResult captured above — otherwise a
        # wrong tag/joiner would pass unchecked, making the Rust clause
        # Rust-vs-handwritten instead of Rust-vs-oracle. The oracle emits
        # `<datapath>\<file>` where <datapath> is the forward-slashed tmp we set.
        dp = tmp.replace(chr(92), "/")
        for i, res in enumerate(results):
            files = BINSAVE_MMF_RESULT_FILES[i]
            tags = BINSAVE_MMF_RESULT_TAGS[i]
            if len(tags) != 2 * len(files):
                sys.exit(f"binsave_mmf: malformed result_tags for action {i}")
            exp = ""
            for k, fname in enumerate(files):
                obj_tag, ftag = tags[2 * k], tags[2 * k + 1]
                clause = f"{obj_tag}=[{ftag}={dp}\\{fname}]"
                exp += clause if k == 0 else f",  {clause}"
            if exp != res:
                sys.exit(
                    f"binsave_mmf: reconstructed GlobalResult {exp!r} != "
                    f"oracle {res!r} (action {i}: {BINSAVE_MMF_ACTIONS[i]!r})"
                )
    finally:
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp, ignore_errors=True)

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for _produced, golden in BINSAVE_MMF_FILES:
        (OUT_DIR / golden).write_bytes(produced_bytes[golden])
    meta = {
        "report": "loadshape_binsave_mmf",
        "deck": BINSAVE_MMF_DECK,  # token form; both engines resolve @FIXTURES@
        "actions": BINSAVE_MMF_ACTIONS,
        "files": [{"produced": p, "golden": g} for p, g in BINSAVE_MMF_FILES],
        "absent": BINSAVE_MMF_ABSENT,
        # GlobalResult per action, captured from the datapath-relative oracle run;
        # the Rust runner rebuilds the paths against its own scratch dir.
        "result_files": BINSAVE_MMF_RESULT_FILES,
        "result_tags": BINSAVE_MMF_RESULT_TAGS,
    }
    (OUT_DIR / "loadshape_binsave_mmf.meta.json").write_text(
        json.dumps(meta, indent=2) + "\n", newline="\n"
    )
    total = sum(len(b) for b in produced_bytes.values())
    print(
        f"wrote {len(BINSAVE_MMF_FILES)} binsave-mmf goldens ({total} bytes); "
        f"GlobalResults: {results}"
    )


# WP-U1.5 E2 (dss_capi 0.15.x `55400a29`): seasonal ratings. A snapshot circuit
# with three overloaded PDElements — an overhead Line, a Transformer, and a CN
# cable Line — each carrying `Seasons=4 Ratings=[...]`. `SeasonRating=yes` +
# `SeasonSignal=season` + `set hour=13` -> `SeasonalRatingIdx = trunc(GetYValue(
# 13)) = 2`, so `Export Overloads`/`Capacity` apply `AmpRatings[2]` to ANY
# PDElement with `NumAmpRatings>1` (0.14.5 restricted the `DI_Overloads` override
# to lines and did NOT apply it in `Export Overloads` at all — this golden is
# revision-SENSITIVE: the default 0.14.5 oracle reports base ratings, capi015 ==
# r4133 report the seasonal ones). Validated bit-identical on capi015 and
# oddie:r4133 (§1.7). GATED ON `capi015` — regenerate with
# `DSS_ORACLE_ENGINE=capi015 <oddie-venv>/python tools/golden/gen_reports.py`.
SEASONAL_FIXTURE = "seasov"
SEASONAL_DECK = [
    f"new circuit.{SEASONAL_FIXTURE} basekv=12.47 pu=1.0 phases=3 bus1=sourcebus",
    "new xycurve.season npts=4 xarray=[0 6 12 18] yarray=[0 1 2 3]",
    "new linecode.lc nphases=3 r1=0.1 x1=0.2 r0=0.3 x0=0.6 c1=0 c0=0 normamps=100 emergamps=120",
    "new CNData.CN_250 NormAmps=260 DIAM=0.567 GMRac=0.20520 Rac=0.41 Runits=mi "
    "Radunits=in gmrunits=in EpsR=2.3 Ins=0.220 DiaIns=1.06 DiaCable=1.29 k=13 "
    "DiaStrand=0.0641 GmrStrand=0.02496 Rstrand=14.8722",
    "new LineGeometry.cabgeo nconds=3 nphases=3 reduce=y",
    "~ cond=1 cncable=CN_250 x=-0.5 h=-4 units=ft",
    "~ cond=2 cncable=CN_250 x=0.0 h=-4 units=ft",
    "~ cond=3 cncable=CN_250 x=0.5 h=-4 units=ft",
    "new line.l1 bus1=sourcebus bus2=b1 linecode=lc length=1 seasons=4 ratings=[100 50 40 30]",
    "new transformer.t1 phases=3 windings=2 buses=[b1 b2] conns=[wye wye] "
    "kvs=[12.47 12.47] kvas=[500 500] xhl=5 seasons=4 ratings=[60 25 20 15]",
    "new line.lc1 bus1=b2 bus2=b3 geometry=cabgeo length=0.5 units=km seasons=4 ratings=[260 45 35 25]",
    "new load.ld bus1=b3 phases=3 kv=12.47 kw=3000 pf=0.95 model=1",
    "set voltagebases=[12.47]",
    "calcvoltagebases",
    "set seasonrating=yes",
    "set seasonsignal=season",
    "set mode=snap",
    "set hour=13",
    "solve",
]
SEASONAL_REPORTS = [
    ("overloads", "EXP_OVERLOADS.CSV", "export_overloads_seasonal"),
    ("capacity", "EXP_CAPACITY.CSV", "export_capacity_seasonal"),
]


def gen_seasonal_overloads(d, engine_spec: str) -> None:
    """Capture the capi015 seasonal `Export Overloads`/`Capacity` goldens (E2)."""
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
    try:
        for c in SEASONAL_DECK:
            d.Text.Command = c
        case = d.ActiveCircuit.Name
        d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
        for keyword, suffix, stem in SEASONAL_REPORTS:
            d.Text.Command = f"export {keyword}"
            produced = Path(d.Text.Result)  # GlobalResult = produced path
            content = produced.read_text()  # universal newlines -> LF
            (OUT_DIR / f"{stem}.txt").write_text(content, newline="\n")
            meta = {
                "report": keyword,
                "fixture": case,
                "suffix": suffix,
                "deck": SEASONAL_DECK,
                "oracle": engine_spec,  # §1.5 provenance stamp
            }
            (OUT_DIR / f"{stem}.meta.json").write_text(
                json.dumps(meta, indent=2) + "\n", newline="\n"
            )
            print(f"wrote {stem}.txt ({len(content)} bytes), {content.count(chr(10))} lines")
    finally:
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp, ignore_errors=True)


# `Export Estimation` (Pascal `ExportEstimation`, ORPHANED_GAPS §1.10): the
# EnergyMeter/Sensor state-estimation error report. No corpus deck uses the
# keyword, so three fixtures are synthesized (PHASE8_PLAN §1):
#
#   est8   — the allocated path. Two feeders off the source so both EnergyMeters
#            are at a feeder head: a 3-phase one (`m1`, unequal `peakcurrent=`
#            targets) and a **1-phase** one (`m2`, which leaves `TempX[2..3]` at
#            the zero the calculated pass wrote — the sub-3-phase column shape).
#            Three Sensors cover every spec: current-spec (`s1`), P/Q-spec (`s2`,
#            whose `Get_WLSCurrentError` re-derives `SensorCurrent` from
#            `kWs`/`kvars`, and `weight=2` scaling both WLS residuals), and a
#            1-phase voltage+current sensor (`s3` — the only nonzero `V… Target`
#            columns and `WLSVoltageError`). A disabled `s4` must NOT appear.
#            `allocateloads` fills `CalculatedCurrent` (the meter/sensor
#            allocation loop), so the `I… Calc` / `%Err` columns are live.
#            A **disabled EnergyMeter** is deliberately absent: the pinned 0.14.5
#            oracle access-violates in `allocateloads` on one (no `Enabled` guard
#            before `TEnergyMeterObj.AllocateLoad`'s zone walk — DIVERGENCES §D9,
#            fixed upstream in SVN r4115), so it is not gate-able *on the allocated
#            path*; the meter-side `Enabled` filter is pinned by `estns` instead
#            (that AV is specific to `allocateloads`, which `estns` never runs).
#   estns  — solved but **not** allocated: nonzero targets against an all-zero
#            `CalculatedCurrent`/`CalculatedVoltage`, so every `%Err` takes the
#            `(1 - 0/target)*100 = 100` form and the WLS residuals are the pure
#            `-Weight * sum(target^2)` term. (The sensor values are set by a
#            separate `Edit` — `RecalcElementData` runs `ZeroSensorArrays` at the
#            end of the `New`, so same-command values would be wiped.) Carries the
#            **disabled EnergyMeter** `mdis` on its own feeder (`line.l2`): the
#            oracle lists it in `Meters.AllNames` but omits it from the report, so
#            dropping the port's meter-side `Enabled` filter adds an
#            `"Energymeter.MDIS"` row and fails the row count.
#   estem  — no EnergyMeters and no Sensors: pins the two section headers with
#            both bodies empty.
ESTIMATION_GROUPS = [
    (
        "est8",
        [
            "new circuit.est8 basekv=12.47 bus1=src phases=3",
            "new line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.3 x1=0.6",
            "new line.l2 bus1=b1 bus2=b2 length=1 units=mi r1=0.3 x1=0.6",
            "new line.l3 bus1=b1 bus2=b3 length=1 units=mi r1=0.3 x1=0.6",
            "new line.l4 bus1=src bus2=c1.1 phases=1 length=1 units=mi r1=0.3 x1=0.6",
            "new load.ld1 bus1=b1 phases=3 kv=12.47 xfkva=500 allocationfactor=0.5 pf=0.9",
            "new load.ld2 bus1=b2 phases=3 kv=12.47 xfkva=800 allocationfactor=0.5 pf=0.9",
            "new load.ld3 bus1=b3 phases=3 kv=12.47 xfkva=600 allocationfactor=0.5 pf=0.9",
            # A fixed kW load inside m1's zone: not allocatable, so the loop can
            # never drive the metered current exactly onto the target and the
            # `%Err` columns stay well clear of a cancellation floor.
            "new load.ldf bus1=b2 phases=3 kv=12.47 kw=200 pf=0.9",
            "new load.ld4 bus1=c1.1 phases=1 kv=7.2 xfkva=150 allocationfactor=0.5 pf=0.9",
            "new energymeter.m1 element=line.l1 terminal=1 peakcurrent=(100,110,120)",
            "new energymeter.m2 element=line.l4 terminal=1 peakcurrent=(50)",
            "new sensor.s1 element=line.l2 terminal=1 kvbase=12.47",
            "edit sensor.s1 currents=[20,22,24]",
            "new sensor.s2 element=line.l3 terminal=1 kvbase=12.47 weight=2",
            "edit sensor.s2 kWs=[400,300,200] kvars=[200,150,100]",
            "new sensor.s3 element=line.l4 terminal=1 kvbase=7.2",
            "edit sensor.s3 kvs=[7.1] currents=[15]",
            "new sensor.s4 element=line.l2 terminal=1 kvbase=12.47 enabled=no",
            "set voltagebases=[12.47,7.2]",
            "calcvoltagebases",
            "solve mode=snap",
            "allocateloads",
        ],
        [("estimation", "EXP_ESTIMATION.csv", "export_estimation")],
    ),
    (
        "estns",
        [
            "new circuit.estns basekv=12.47 bus1=src phases=3",
            "new line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.3 x1=0.6",
            "new line.l2 bus1=src bus2=d1 length=1 units=mi r1=0.3 x1=0.6",
            "new load.ld1 bus1=b1 phases=3 kv=12.47 xfkva=500 allocationfactor=0.5 pf=0.9",
            "new load.ld2 bus1=d1 phases=3 kv=12.47 xfkva=300 allocationfactor=0.5 pf=0.9",
            "new energymeter.m1 element=line.l1 terminal=1 peakcurrent=(100,110,120)",
            # Disabled: must be absent from the report (the meter-side `Enabled`
            # filter). Its own feeder head so it is otherwise a legal meter.
            "new energymeter.mdis element=line.l2 terminal=1 peakcurrent=(70,70,70) enabled=no",
            "new sensor.s1 element=line.l1 terminal=1 kvbase=12.47",
            "edit sensor.s1 kvs=[7.2,7.2,7.2] currents=[20,22,24]",
            "set voltagebases=[12.47]",
            "calcvoltagebases",
            "solve mode=snap",
        ],
        [("estimation", "EXP_ESTIMATION.csv", "export_estimation_noalloc")],
    ),
    (
        "estem",
        [
            "new circuit.estem basekv=12.47 bus1=src phases=3",
            "new line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.3 x1=0.6",
            "new load.ld1 bus1=b1 phases=3 kv=12.47 kw=100",
            "set voltagebases=[12.47]",
            "calcvoltagebases",
            "solve mode=snap",
        ],
        [("estimation", "EXP_ESTIMATION.csv", "export_estimation_empty")],
    ),
]


def gen_estimation(d) -> None:
    """Capture the oracle's `Export Estimation` for the three fixtures above."""
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for _case, deck, reports in ESTIMATION_GROUPS:
        tmp = tempfile.mkdtemp(prefix="dss_gen_reports_")
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

    # WP-U1.5 E2: the seasonal goldens are gated on `capi015` (0.14.5 does not
    # apply seasonal ratings in these reports). Running the generator under
    # `DSS_ORACLE_ENGINE=capi015` regenerates ONLY those files (§1.5); the default
    # (0.14.5) run leaves them untouched and regenerates the rest.
    engine_spec = pin.get("engine_spec", "capi")
    if os.environ.get("DSS_ORACLE_ENGINE", "capi") == "capi015":
        gen_seasonal_overloads(d, engine_spec)
        return

    # Each generator is selectable by name on the command line (the suffix
    # after `gen_`), so a new golden can be produced without regenerating (and
    # thereby re-pinning) every existing one:
    #     python tools/golden/gen_reports.py estimation
    # No arguments = the full regeneration.
    generators = {
        "counts": gen_counts,
        "feeder_reports": gen_feeder_reports,
        "show_reports": gen_show_reports,
        "show_eventlog": gen_show_eventlog,
        "show_yprim": gen_show_yprim,
        "show_variables": gen_show_variables,
        "show_kvbasemismatch": gen_show_kvbasemismatch,
        "show_monitor": gen_show_monitor,
        "monitor_reports": gen_monitor_reports,
        "register_reports": gen_register_reports,
        "show_meter_reports": gen_show_meter_reports,
        "show_meter_edgecases": gen_show_meter_edgecases,
        "log_reports": gen_log_reports,
        "ieee8500_reports": gen_ieee8500_reports,
        "extra_feeder_reports": gen_extra_feeder_reports,
        "seqz": gen_seqz,
        "faultstudy": gen_faultstudy,
        "show_faultstudy": gen_show_faultstudy,
        "reliability": gen_reliability,
        "gic_mvars": gen_gic_mvars,
        "deck_groups": gen_deck_groups,
        "show_overload_unserved": gen_show_overload_unserved,
        "show_zone_loops": gen_show_zone_loops,
        "show_controlled": gen_show_controlled,
        "show_autotrans": gen_show_autotrans,
        "show_busflow": gen_show_busflow,
        "show_isolated": gen_show_isolated,
        "show_topology": gen_show_topology,
        "show_topo_coverage": gen_show_topo_coverage,
        "show_isolated_orphan": gen_show_isolated_orphan,
        "show_lineconstants": gen_show_lineconstants,
        "sections": gen_sections,
        "profile": gen_profile,
        "demand_interval": gen_demand_interval,
        "di_overloads_1ph": gen_di_overloads_1ph,
        "reliability_multimeter": gen_reliability_multimeter,
        "dump_decks": gen_dump_decks,
        "save_decks": gen_save_decks,
        "loadshape_binsave": gen_loadshape_binsave,
        "interp": gen_interp,
        "distribute": gen_distribute,
        "uuids": gen_uuids,
        "estimation": gen_estimation,
    }
    wanted = set(sys.argv[1:])
    unknown = wanted - set(generators)
    if unknown:
        sys.exit(f"unknown generator(s): {sorted(unknown)}; known: {sorted(generators)}")
    for name, fn in generators.items():
        if not wanted or name in wanted:
            fn(d)


if __name__ == "__main__":
    main()

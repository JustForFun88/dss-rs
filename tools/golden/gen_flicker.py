"""Generate the Monitor mode-4 flicker golden (WP-PF.2).

UNUSUAL ORACLE — read this before regenerating. The pinned dss_capi oracle
(dss-python 0.15.7) **cannot** produce a mode-4 flicker result: its
`DoFlickerCalculations` (`Monitor.pas:1655`) indexes `Terminals[MeteredTerminal]`
with the 1-based terminal number, but the dss_capi rewrite made `Terminals` a
0-based array, so `Terminals[1]` on a 1-terminal element reads one entry past the
array -> an **unconditional access violation** on any `export monitor` / `Monitors.
Process()` of a mode-4 monitor (probe-verified 2026-07-11: hard segfault). The
**official** Delphi engine keeps `Terminals` 1-based (`pTerminalList`), so it runs
the flicker calc correctly.

The committed golden (`tests/golden/flicker/pst_demo.json`) is the FROZEN
capture from the official EPRI **r3723** binary via the retired Oddie bridge
(its `oracle` block records that provenance). Both the r3723 binary and the
Oddie/dss-python channel are gone (UNIFIED_GATE Phase E); the regeneration path
now drives the official EPRI **r4133** binary — the only vendored official
revision — through the in-house Rust bridge (`epri-worker`, crates/dss-epri,
auto-built if missing):

    python tools/golden/gen_flicker.py

Cross-revision parity is byte-proven (STATUS "EPRI bridge parity round",
2026-07-19): the r4133 regen reproduces the committed r3723 payload — raw f32
magnitudes, flicker + Pst channels, kvbase — byte-identically (the engine's
flicker meter and this trivial deck's power flow are revision-stable), so only
the `oracle` provenance block differs. `DSS_GOLDEN_OUT` redirects the output
dir (scratch parity runs).

The Rust replay (`crates/dss-core/tests/golden_flicker.rs`) is solve-decoupled: it
feeds the captured raw per-phase magnitudes straight into `flicker_meter` and
compares the flicker + Pst channels **f32-exact**, so no power-flow floor enters.
"""

import json
import os
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
DEMO = REPO / "tests/corpus/electricdss-tst/Version8/Distrib/Examples/Matlab"
OUT_DIR = Path(os.environ.get("DSS_GOLDEN_OUT", REPO / "tests" / "golden" / "flicker"))
OUT = OUT_DIR / "pst_demo.json"

# The upstream flicker demo (Examples/Matlab/pst.dss): a stiff source + isource +
# a model=2 load driven by the vendored WindRmsV.csv duty shape, monitored mode 4.
DECK = [
    "clear",
    "Set DefaultBaseFrequency=60",
    "new circuit.pst bus1=EQSRC basekv=1.73205 pu=1 r1=0.00001 x1=0.00001 r0=0.00001 x0=0.00001",
    "New LoadShape.pst npts=8640 interval=(10.0 3600 /)",
    "~   mult=(File=WindRmsV.csv) Action=Normalize",
    "New isource.pst bus1=PCC amps=1 angle=0",
    "New load.pst    bus1=PCC kV=1.73205 kW=3.0 pf=1.0 conn=wye duty=pst vminpu=0.01  Model=2",
    "new Monitor.pst element=load.pst terminal=1 mode=4",
    'set voltagebases="1.73205"',
    "calcvoltagebases",
    "solve mode=duty stepsize=10 number=8640",
    "wait",  # PM/async solve barrier (D9 rule) before reading.
]


def main() -> None:
    sys.path.insert(0, str(REPO / "tools" / "opendss"))
    from epri_worker import EpriWorker

    w = EpriWorker()  # loads bin/r4133, asserts the revisions.json pin

    w.chdir(DEMO)  # File=WindRmsV.csv is relative to the deck
    for c in DECK:
        w.exec(c)

    w.read("monitor_select", name="pst")
    n = int(w.read("monitor_sample_count"))
    nph = int(w.read("monitor_num_channels")) // 2

    # Raw per-phase RMS magnitudes (odd data channels) BEFORE export — the exact
    # f32 inputs DoFlickerCalculations feeds into FlickerMeter.
    raw_mag = [w.read("monitor_channel", index=2 * p + 1) for p in range(nph)]

    kvbase = float(w.read("bus_kvbase", name="PCC"))

    # Export triggers the official DoFlickerCalculations, rewriting the stream in
    # place; read the post-processed channels back at full f32 precision.
    #
    # The export ALSO writes `pst_Mon_pst_1.csv` into the engine's
    # OutputDirectory. The DLL initializes that from the registry-persisted
    # DataPath (`HKCU\Software\OpenDSS\MainSect`, r4133 `ReadDSS_Registry`) =
    # the directory of the last deck ANY local run Compiled — typically inside
    # tests/corpus (empirically `Examples/Scripts/` after a probe_59n run; this
    # deck is exec'd line-by-line, so nothing here re-points it). Redirect
    # DataPath to a stable scratch dir first so the CSV never lands in the
    # corpus. `Set DataPath` also ChDirs the worker there (r4133
    # `DSSGlobals.SetDataPath`) — fine, WindRmsV.csv was already resolved
    # during the deck run — and a stable (never-deleted) dir keeps the
    # registry-persisted path valid for later engine loads. The CSV path does
    # not enter the golden; the channels are read from the in-memory stream.
    export_dir = Path(tempfile.gettempdir()) / "dss_rs_flicker_export"
    export_dir.mkdir(exist_ok=True)
    w.exec(f'set DataPath="{export_dir}"')
    w.exec("export monitor pst")
    flk = [w.read("monitor_channel", index=2 * p + 1) for p in range(nph)]
    pst = [w.read("monitor_channel", index=2 * p + 2) for p in range(nph)]

    # Absolute sample times (uniform 10 s duty step): t[i] = 10*(i+1) s.
    times = [10.0 * (i + 1) for i in range(n)]

    data = {
        "schema": 1,
        "oracle": {
            "engine": "official-EPRI-r4133 (epri-worker)",
            "version": w.version,
            "note": "pinned dss_capi oracle crashes on mode-4 export (Terminals OOB); "
            "the official engine is the correct reference. Originally captured on "
            "r3723 via the retired Oddie bridge; the r4133/epri-worker regen is "
            "payload-byte-identical (see gen_flicker.py).",
        },
        "deck": DECK,
        "n": n,
        "nphases": nph,
        "fbase": 60.0,
        "kvbase": kvbase,
        "times": times,
        "raw_mag": raw_mag,
        "flk": flk,
        "pst": pst,
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(data))
    print(f"wrote {OUT} (N={n}, nphases={nph}, engine={w.version})")
    w.close()


if __name__ == "__main__":
    main()

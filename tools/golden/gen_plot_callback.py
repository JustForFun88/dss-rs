"""Generate the Plot/Visualize callback goldens from the pinned oracle (WPG.17).

The plot plumbing (`DoPlotCmd` / `DoVisualizeCmd`, `AddBusMarker`) does not touch
the solve or write a report file — it assembles a `plotParams` JSON object and
hands it to `DSS.DSSPlotCallback`. So these goldens pin the **callback JSON
payload**: this generator registers a capturing plot callback on the pinned
engine (the exact `dss_capi` build the Pascal source at `.inputs/dss_capi`
compiles to), replays a tiny self-contained fixture, issues each plot/visualize
variant, and saves the captured JSON string to
`tests/golden/plot_callback/<variant>.json`. The Rust driver
(`crates/dss-core/tests/golden_plot_callback.rs`) replays the SAME deck +
variants (read back from `meta.json`, so the two can never drift), captures its
own payload, and compares structurally after parsing numbers out — fpjson emits
`2.0000000000000000E+003` for floats, which is byte-incomparable with any Rust
serializer (PORTING_PLAN §4 tolerance policy).

Registration order matters: the message callback must be registered BEFORE
`DSS_Set_AllowForms(True)` (else it is a no-op, error 5096), then the plot
callback (`dss/plot.py::enable`).

Usage:
    python tools/golden/gen_plot_callback.py     # regenerate the plot goldens
Regeneration is manual and must use the exact versions in tools/golden/PIN.txt.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gen_checkpoints import check_pin  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = REPO_ROOT / "tests" / "golden" / "plot_callback"

# A tiny, self-contained fixture (no master): circuit + 2 lines + a transformer
# (for the Visualize element reference) + a load, solved. Names are lowercase so
# the captured `ElementName` is unambiguous (the engine lowercases stored names).
DECK = [
    "clear",
    "new circuit.plotfix basekv=12.47 bus1=src",
    "new line.l1 bus1=src bus2=b",
    "new line.l2 bus1=b bus2=c",
    "new transformer.tr1 phases=3 windings=2 buses=(c, d) conns=(wye,wye) "
    "kvs=(12.47,4.16) kvas=(1000,1000) xhl=5",
    "new load.ld1 bus1=d phases=3 kv=4.16 kw=100",
    "solve",
]

# Each variant is (stem, [commands run after the deck]); the LAST command emits
# the captured payload. AddBusMarker (a variant prefix) does not itself fire the
# callback — only Plot/Visualize does — so exactly one payload is captured.
VARIANTS = [
    ("circuit_power", [
        "plot type=circuit quantity=Power Max=2000 dots=n labels=n subs=y C1=Blue 1phlinestyle=3",
    ]),
    ("circuit_voltage", ["plot type=circuit quantity=Voltage Max=0.95"]),
    # Positional form (type=circuit, quantity=Losses) + a `$`-hex color.
    ("circuit_losses_hex", ["plot circuit Losses Max=.02 dots=y C1=$00FF00FF"]),
    ("profile_default", ["plot type=profile phases=default"]),
    ("profile_all", ["plot type=profile phases=all"]),
    ("profile_primary", ["plot type=profile phases=primary"]),
    ("monitor", ["plot type=monitor object=m1 channels=(1,3,5) base=[7200 7200 7200]"]),
    ("daisy", ["plot type=daisy"]),
    # The `type=Losses -> LoadShape` quirk (letter L maps unconditionally).
    ("losses_loadshape", ["plot type=Losses"]),
    ("busmarkers", [
        "AddBusMarker Bus=b code=5 color=Red size=3",
        "plot type=circuit quantity=Power Max=2000 subs=y C1=Blue 1phlinestyle=3",
    ]),
    ("visualize_power", ["visualize powers Transformer.tr1"]),
    ("visualize_current", ["visualize current Line.l1"]),
    ("visualize_voltage", ["visualize voltage Line.l1"]),
]


def main() -> None:
    pin = check_pin()
    print(f"oracle: dss-python {pin['dss_python']}, engine {pin['engine']}")
    import dss

    d = dss.DSS
    api = dss.api_util
    cap: list[str] = []

    @api.ffi.callback("int32_t(void *, char *)")
    def plot_cb(ctx, p):  # noqa: ANN001, ANN202
        cap.append(api.ffi.string(p).decode())
        return 0

    @api.ffi.callback("int32_t(void *, char *, int32_t, int64_t, int32_t)")
    def msg_cb(ctx, m, t, sz, st):  # noqa: ANN001, ANN202
        return 0

    api.lib.DSS_RegisterMessageCallback(msg_cb)  # required before AllowForms
    api.lib.DSS_RegisterPlotCallback(plot_cb)
    api.lib.DSS_Set_AllowForms(True)
    if not api.lib.DSS_Get_AllowForms():
        sys.exit("DSS_Set_AllowForms(True) did not take — message callback missing?")

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for stem, cmds in VARIANTS:
        cap.clear()
        for c in DECK:
            d.Text.Command = c
        for c in cmds:
            d.Text.Command = c
        if len(cap) != 1:
            sys.exit(f"variant {stem}: expected 1 captured payload, got {len(cap)}")
        # Pretty-print so the golden is human-diffable; the Rust comparator parses
        # it back to a Value regardless of formatting.
        payload = json.loads(cap[0])
        (OUT_DIR / f"{stem}.json").write_text(
            json.dumps(payload, indent=1) + "\n", newline="\n"
        )
        print(f"wrote {stem}.json ({len(payload)} top-level keys)")

    meta = {"deck": DECK, "variants": [{"stem": s, "cmds": c} for s, c in VARIANTS]}
    (OUT_DIR / "meta.json").write_text(json.dumps(meta, indent=2) + "\n", newline="\n")
    print(f"wrote meta.json ({len(VARIANTS)} variants)")


if __name__ == "__main__":
    main()

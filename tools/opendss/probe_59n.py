"""Reproducible 59NRelayDemo probe on the official EPRI r4133 engine (Oddie).

Backs the WP-U2.6 `relay_voltage_dynamics_chaos_floor` classification
(`tests/corpus/manifests/skipped_needs_investigation.json`, STATUS.md) with a
CHECKED-IN, re-runnable oracle read instead of prose. It proves two claims on
the r4133 engine itself — the only channel with r4133 voltage-relay semantics
(the pinned dss-python oracle is 0.14.5, pre-r4133):

  A. NO-TRIP across the broken-delta open point. The `type=voltage` relay
     monitors the 1-phase PT3 term-2 (Delta.3.2 broken-delta 3V0) and switches
     the 3-phase Line.line1. On r4133 the engine's own `Relay.State` reads
     `[closed, closed, closed, ]` and Line.line1 still carries full current — the
     relay does NOT trip. This is byte-identical to the Rust port's
     `render_state_array()` and the `no_trip` outcome asserted by the unit test
     `voltage_relay_open_point_sizes_state_by_controlled_nphases_59n`, so the
     port's encoded outcome reflects oddie:r4133, not a self-pinned value.

  B. Post-fix residual is a chaotic pole-slip, not a maskable floor. With the
     relay correctly not tripping, the un-damped generator (D=1) pole-slips under
     the sustained t=0.3 s Genbus fault: frequency leaves 60 Hz (~79 Hz by t=1.0)
     and wanders unboundedly (67–115 Hz over the next 15 s). A positive-Lyapunov
     trajectory has no fixed node-V tolerance that honestly bounds the Rust-vs-
     oracle last-ulp (faer-vs-KLU) difference, so the deck stays parked
     (CLAUDE.md: never mask a divergence with a tolerance). This probe reads the
     live-f64 generator state (`AllVariableValues`, not the f32 monitor channel).

Run:  tools/opendss/.venv/Scripts/python tools/opendss/probe_59n.py
Exit 0 iff claim A holds (relay all-closed / no trip); prints the claim-B
frequency trajectory for eyeballing the pole-slip.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parents[1]
DLL = (REPO_ROOT / "tools/opendss/bin/r4133/OpenDSSDirect.dll").resolve()
DECK = (
    REPO_ROOT
    / "tests/corpus/electricdss-tst/Version8/Distrib/Examples/Scripts/59NRelayDemo.dss"
).resolve()

sys.path.insert(0, str(REPO_ROOT / "tools" / "oracle"))

EXPECT_STATE = "[closed, closed, closed, ]"


def line1_max_current(ckt) -> float:
    ckt.SetActiveElement("Line.Line1")
    mags = list(ckt.ActiveCktElement.CurrentsMagAng)[0::2]
    return max(mags)


def gen_vars(ckt) -> dict[str, float]:
    ckt.SetActiveElement("Generator.gen1")
    el = ckt.ActiveCktElement
    return dict(
        zip(
            (str(s) for s in el.AllVariableNames),
            (float(v) for v in el.AllVariableValues),
        )
    )


def main() -> None:
    if not DLL.is_file():
        sys.exit(f"r4133 DLL not vendored: {DLL} (run tools/opendss/vendor_binaries.py)")
    if not DECK.is_file():
        sys.exit(f"59NRelayDemo deck not found: {DECK}")

    from dss import IOddieDSS

    from oracle_server import _CorpusGuard  # tools/oracle on sys.path

    cwd0 = os.getcwd()  # the EPRI engine chdirs the process on Compile
    d = IOddieDSS(library_path=str(DLL))
    print(f"engine: {d.Version}", flush=True)
    d.AllowForms = False
    try:
        d.AllowEditor = False
    except Exception:
        pass

    try:
        with _CorpusGuard(str(DECK)):
            d.Text.Command = "clear"
            d.Text.Command = f'Compile "{DECK}"'  # inline: dynamics stepsize=0.1 number=10 -> t=1.0
            ckt = d.ActiveCircuit
            sol = ckt.Solution
            assert sol.Converged, "59NRelayDemo did not converge on r4133"
            t = sol.dblHour * 3600.0
            print(f"after-compile: converged, t={t:.3f} s", flush=True)

            # --- Claim A: relay did NOT trip -------------------------------
            d.Text.Command = "? Relay.mfrov/uv.State"
            state = str(d.Text.Result)
            i1 = line1_max_current(ckt)
            print(f"[A] Relay.State (r4133) = {state!r}", flush=True)
            print(f"[A] Line.line1 max |I| = {i1:.3f} A (nonzero => closed)", flush=True)
            ok_state = state == EXPECT_STATE
            ok_closed = i1 > 1.0
            print(
                f"[A] no-trip: state matches port render {EXPECT_STATE!r} = {ok_state}; "
                f"line energized = {ok_closed}",
                flush=True,
            )

            # --- Claim B: chaotic pole-slip trajectory ---------------------
            gv = gen_vars(ckt)
            f0 = gv["Frequency"]
            print(
                f"[B] gen @t=1.0: Frequency={f0:.4f} Hz "
                f"(base 60 Hz => pole-slipping), PShaft={gv['PShaft']:.3f} W",
                flush=True,
            )
            print("[B] continued 1 s steps (Frequency Hz, Line1 max |I| A):", flush=True)
            fmin, fmax = f0, f0
            for _ in range(15):
                sol.Solve()
                gv = gen_vars(ckt)
                f = gv["Frequency"]
                fmin, fmax = min(fmin, f), max(fmax, f)
                print(
                    f"    t={sol.dblHour * 3600.0:5.2f}  f={f:8.4f}  |I|max={line1_max_current(ckt):9.3f}",
                    flush=True,
                )
            print(
                f"[B] frequency wandered over [{fmin:.2f}, {fmax:.2f}] Hz "
                "=> unbounded pole-slip, not a bandable floor",
                flush=True,
            )
    finally:
        os.chdir(cwd0)

    if not (ok_state and ok_closed):
        sys.exit(
            "PROBE FAILED: r4133 relay outcome is not the encoded no-trip "
            f"(state={state!r}, line |I|max={i1:.3f})"
        )
    print("PROBE OK: r4133 confirms no-trip [closed,closed,closed] + chaotic pole-slip")


if __name__ == "__main__":
    main()

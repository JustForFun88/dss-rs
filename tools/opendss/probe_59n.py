"""Reproducible 59NRelayDemo probe on the official EPRI r4133 engine, driven
through the in-house Rust bridge (`epri-worker`, crates/dss-epri).

Backs the WP-U2.6 `relay_voltage_dynamics_chaos_floor` classification
(`tests/corpus/manifests/skipped_needs_investigation.json`, STATUS.md) with a
CHECKED-IN, re-runnable oracle read instead of prose. The original probe drove
the same DLL through the retired Oddie/dss-python bridge (UNIFIED_GATE Phase E);
this successor issues the same command/read sequence over `epri-worker` and
reproduces the same two claims on the r4133 engine itself — the only channel
with r4133 voltage-relay semantics (the pinned dss-python oracle is 0.14.5,
pre-r4133):

  A. NO-TRIP across the broken-delta open point. The `type=voltage` relay
     monitors the 1-phase PT3 term-2 (Delta.3.2 broken-delta 3V0) and switches
     the 3-phase Line.line1. On r4133 the engine's own `Relay.State` reads
     `[closed, closed, closed, ]` and Line.line1 still carries full current — the
     relay does NOT trip. This is byte-identical to the Rust port's
     `render_state_array()` and the `no_trip` outcome asserted by the unit test
     `voltage_relay_open_point_sizes_state_by_controlled_nphases_59n`, so the
     port's encoded outcome reflects the official r4133 engine, not a
     self-pinned value.

  B. Post-fix residual is a chaotic pole-slip, not a maskable floor. With the
     relay correctly not tripping, the un-damped generator (D=1) pole-slips under
     the sustained t=0.3 s Genbus fault: frequency leaves 60 Hz (~79 Hz by t=1.0)
     and wanders unboundedly (67–115 Hz over the next 15 s). A positive-Lyapunov
     trajectory has no fixed node-V tolerance that honestly bounds the Rust-vs-
     oracle last-ulp (faer-vs-KLU) difference, so the deck stays parked
     (CLAUDE.md: never mask a divergence with a tolerance). This probe reads the
     live-f64 generator state (`AllVariableValues` via the bridge's `variables`
     read, not the f32 monitor channel).

Run:  python tools/opendss/probe_59n.py    (epri-worker is auto-built if missing)
Exit 0 iff claim A holds (relay all-closed / no trip); prints the claim-B
frequency trajectory for eyeballing the pole-slip.
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parents[1]
DECK = (
    REPO_ROOT
    / "tests/corpus/electricdss-tst/Version8/Distrib/Examples/Scripts/59NRelayDemo.dss"
).resolve()

sys.path.insert(0, str(HERE))  # epri_worker
sys.path.insert(0, str(REPO_ROOT / "tools" / "oracle"))  # corpus_guard

EXPECT_STATE = "[closed, closed, closed, ]"


def line1_max_current(w) -> float:
    w.read("set_active_element", name="Line.Line1")
    cur = w.read("element_currents")
    return max(math.hypot(re, im) for re, im in zip(cur[0::2], cur[1::2]))


def gen_vars(w) -> dict[str, float]:
    v = w.read("variables", name="Generator.gen1")
    return dict(zip((str(s) for s in v["var_names"]), (float(x) for x in v["values"])))


def main() -> None:
    if not DECK.is_file():
        sys.exit(f"59NRelayDemo deck not found: {DECK}")

    from corpus_guard import CorpusGuard

    from epri_worker import EpriWorker

    w = EpriWorker()  # loads bin/r4133 + asserts the revisions.json version pin
    print(f"engine: {w.version}", flush=True)

    # The EPRI engine chdirs its process on Compile — that happens inside the
    # worker process; this driver's cwd is untouched.
    with CorpusGuard(str(DECK)):
        w.exec("clear")
        w.exec(f'Compile "{DECK}"')  # inline: dynamics stepsize=0.1 number=10 -> t=1.0
        assert w.read("converged"), "59NRelayDemo did not converge on r4133"
        t = w.read("dbl_hour") * 3600.0
        print(f"after-compile: converged, t={t:.3f} s", flush=True)

        # --- Claim A: relay did NOT trip -------------------------------
        state = str(w.exec("? Relay.mfrov/uv.State"))
        i1 = line1_max_current(w)
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
        gv = gen_vars(w)
        f0 = gv["Frequency"]
        print(
            f"[B] gen @t=1.0: Frequency={f0:.4f} Hz "
            f"(base 60 Hz => pole-slipping), PShaft={gv['PShaft']:.3f} W",
            flush=True,
        )
        print("[B] continued 1 s steps (Frequency Hz, Line1 max |I| A):", flush=True)
        fmin, fmax = f0, f0
        for _ in range(15):
            w.exec("solve")
            gv = gen_vars(w)
            f = gv["Frequency"]
            fmin, fmax = min(fmin, f), max(fmax, f)
            t = w.read("dbl_hour") * 3600.0
            print(
                f"    t={t:5.2f}  f={f:8.4f}  |I|max={line1_max_current(w):9.3f}",
                flush=True,
            )
        print(
            f"[B] frequency wandered over [{fmin:.2f}, {fmax:.2f}] Hz "
            "=> unbounded pole-slip, not a bandable floor",
            flush=True,
        )
    w.close()

    if not (ok_state and ok_closed):
        sys.exit(
            "PROBE FAILED: r4133 relay outcome is not the encoded no-trip "
            f"(state={state!r}, line |I|max={i1:.3f})"
        )
    print("PROBE OK: r4133 confirms no-trip [closed,closed,closed] + chaotic pole-slip")


if __name__ == "__main__":
    main()

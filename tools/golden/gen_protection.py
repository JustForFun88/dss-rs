"""Generate the **protection** targeted goldens from the pinned oracle
(PHASE7_PLAN WP7.2 step-4 / §1 focused gate 3: "a fault + protection sequence
(recloser/relay/fuse trip + reclose), event logs equal (normalized, line-for-line,
like the time-series control gate), final switch/recloser states exact";
historically part of "Phase 7").

Command-replay like the time-series goldens (per-`solve`-step captures + the
event log), but the
scenarios are protection trip/reclose sequences driven through the **ported**
`mode=duty controlmode=time` control sweep (dynamics mode — which the corpus relay
demos use — is unported until WP7.7). Each scenario pins, against the oracle:

  - per-step dblHour + iteration count + converged + node voltages (the feeder
    voltage collapses to ~0 on every step the protection has the line open, so the
    per-step voltage trajectory already encodes the discrete open/closed state);
  - the **event log line-for-line** (normalized) — the trip/reclose/blow sequence
    (FAST / DELAYED / LOCKED OUT / CLOSED / PHASE TARGET / BLOWN / RESETTING);
  - every element's terminal currents/powers at the **final** step — the
    controlled line carries ~0 A when the device left it open, full load current
    when it reclosed, so the final state is pinned exactly.

A scenario may interleave `edit` commands before chosen steps via `pre_solve`
(step-index -> [commands]), mirroring the corpus `edit swtcontrol.x action=o`
manual-switch usage.

Scenarios (one file each under tests/golden/protection/):
  - recloser_temp:  temporary fault -> Recloser trips FAST, fault self-clears
                    below MinAmps, Recloser recloses (ends CLOSED).
  - recloser_perm:  permanent fault -> Recloser trips FAST -> reclose -> DELAYED
                    -> reclose -> LOCKED OUT (ends OPEN). Exercises NumFast,
                    RecloseIntervals, Shots, lockout.
  - relay_current:  definite-time overcurrent Relay (Type=Current, eventlog=yes)
                    -> RESETTING at init + OPENED ON PH & LOCKED OUT (Shots=1).
  - fuse_blow:      per-phase Fuse on the default tlink curve -> PHASE 3/2/1 BLOWN.
  - swt_manual:     manual SwtControl opened by a mid-run `edit ... action=open`
                    (the corpus civanlar pattern) after its delay.

Usage:
    python tools/golden/gen_protection.py              # regenerate all
    python tools/golden/gen_protection.py recloser_perm  # one scenario
Regeneration is manual and must use the exact versions in tools/golden/PIN.txt.
"""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gen_checkpoints import capture_element, check_pin  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = REPO_ROOT / "tests" / "golden" / "protection"
SCHEMA = 1


def make_engine() -> tuple[object, dict]:
    """Return the golden-generation engine + its provenance dict.

    `DSS_ORACLE_ENGINE` selects it: `capi` (default, pinned dss-python 0.14.5) /
    `capi015` via `check_pin`, or `oddie` — an official EPRI `OpenDSSDirect.dll`
    (`DSS_OPENDSS_REV` -> tools/opendss/revisions.json) driven through the AltDSS
    Oddie bridge from the Oddie venv. The Oddie path is used to pin an r4133-only
    protection behavior (WP-U2.4 SwtControl D6: `swt_manual` regenerated on
    r4133), self-describing via the `engine_spec` key in a mixed golden tree
    (UPGRADE_PLAN §1.5). Run this from the Oddie venv python for `oddie`.
    """
    engine = os.environ.get("DSS_ORACLE_ENGINE", "capi")
    if engine != "oddie":
        from dss import DSS as d

        return d, check_pin()

    rev = os.environ.get("DSS_OPENDSS_REV", "")
    revs = json.loads((REPO_ROOT / "tools" / "opendss" / "revisions.json").read_text())
    if rev not in revs:
        sys.exit(f"DSS_OPENDSS_REV={rev!r} not in revisions.json ({sorted(revs)})")
    dll = str((REPO_ROOT / revs[rev]["dll"]).resolve())
    expect = revs[rev].get("expect_version", "")
    os.add_dll_directory(str(Path(dll).parent))
    from dss import IOddieDSS

    d = IOddieDSS(library_path=dll)
    ver = str(d.Version)
    if expect and expect not in ver:
        sys.exit(f"engine {ver!r} does not contain pinned {expect!r} (rev={rev!r})")
    d.AllowForms = False
    d.Text.Command = "Set RegistryUpdate=No"
    d.Text.Command = "Set Editor=rundll32.exe"
    return d, {"engine_spec": f"oddie:{rev}", "oddie": True, "rev": rev, "engine": ver}

# A small radial feeder: source -> line.feed -> line.lat -> 3-phase load. The
# protection device monitors and switches line.feed; the fault is at the load bus.
CKT = [
    "Set DefaultBaseFrequency=60",
    "new circuit.prot basekv=12.47 phases=3 bus1=src mvasc3=20000 mvasc1=21000",
    "new linecode.lc nphases=3 r1=0.3 x1=0.6 r0=0.7 x0=1.9 c1=0 c0=0 units=mi",
    "new line.feed bus1=src bus2=mid linecode=lc length=1 units=mi",
    "new line.lat  bus1=mid bus2=loadb linecode=lc length=1 units=mi",
    "new load.l bus1=loadb phases=3 kv=12.47 kw=500 pf=0.95 model=1",
]
TAIL = ["set voltagebases=[12.47]", "calcvoltagebases"]
DUTY = "set mode=duty stepsize=0.1 number=1 controlmode=time"


def deck_recloser_temp() -> dict:
    cmds = [
        *CKT,
        "new recloser.r monitoredobj=line.feed monitoredterm=1 "
        "switchedobj=line.feed switchedterm=1 numfast=1 shots=2 "
        "phasetrip=80 groundtrip=40 recloseintervals=(0.5, 1.0)",
        "new fault.f bus1=loadb phases=3 ontime=0.1 r=1 temporary=yes",
        *TAIL,
        DUTY,
    ]
    return {"name": "recloser_temp", "commands": cmds, "n_steps": 12}


def deck_recloser_perm() -> dict:
    cmds = [
        *CKT,
        "new recloser.r monitoredobj=line.feed monitoredterm=1 "
        "switchedobj=line.feed switchedterm=1 numfast=1 shots=2 "
        "phasetrip=80 groundtrip=40 recloseintervals=(0.5, 1.0)",
        "new fault.f bus1=loadb phases=3 ontime=0.1 r=1 temporary=no",
        *TAIL,
        DUTY,
    ]
    return {"name": "recloser_perm", "commands": cmds, "n_steps": 24}


def deck_relay_current() -> dict:
    cmds = [
        *CKT,
        "new relay.rl type=current monitoredobj=line.feed monitoredterm=1 "
        "switchedobj=line.feed switchedterm=1 phasetrip=80 groundtrip=40 "
        "delay=0.1 shots=1 recloseintervals=None eventlog=yes",
        "new fault.f bus1=loadb phases=3 ontime=0.1 r=1 temporary=no",
        *TAIL,
        DUTY,
    ]
    return {"name": "relay_current", "commands": cmds, "n_steps": 8}


def deck_fuse_blow() -> dict:
    cmds = [
        *CKT,
        "new fuse.fz monitoredobj=line.feed monitoredterm=1 "
        "switchedobj=line.feed switchedterm=1 ratedcurrent=40",
        "new fault.f bus1=loadb phases=3 ontime=0.1 r=1 temporary=no",
        *TAIL,
        DUTY,
    ]
    return {"name": "fuse_blow", "commands": cmds, "n_steps": 8}


def deck_swt_manual() -> dict:
    cmds = [
        *CKT,
        "new swtcontrol.sw switchedobj=line.feed switchedterm=1 normal=closed delay=0.25",
        *TAIL,
        DUTY,
    ]
    # Mirror the corpus `edit swtcontrol.x action=o`: a mid-run open before step 5.
    # Regenerate on r4133 (WP-U2.4 D6): `DSS_ORACLE_ENGINE=oddie DSS_OPENDSS_REV=r4133`.
    # Under r4133 the `Action` forces the switch open immediately (no `delay`
    # queue), so the switch is open from step 5 and the event log is empty.
    return {
        "name": "swt_manual",
        "commands": cmds,
        "n_steps": 12,
        "pre_solve": {"5": ["edit swtcontrol.sw action=open"]},
    }


SCENARIOS = {
    "recloser_temp": deck_recloser_temp,
    "recloser_perm": deck_recloser_perm,
    "relay_current": deck_relay_current,
    "fuse_blow": deck_fuse_blow,
    "swt_manual": deck_swt_manual,
}


def build(d, spec: dict) -> dict:
    cmds = spec["commands"]
    pre = spec.get("pre_solve", {})
    d.Text.Command = "clear"
    for c in cmds:
        d.Text.Command = c
    ckt = d.ActiveCircuit
    sol = ckt.Solution

    steps = []
    for s in range(spec["n_steps"]):
        for c in pre.get(str(s), []):
            d.Text.Command = c
        d.Text.Command = "solve"
        if not bool(sol.Converged):
            sys.exit(f"{spec['name']} step {s}: oracle did not converge — fix the deck")
        varray = list(ckt.YNodeVarray)
        steps.append(
            {
                "dbl_hour": float(sol.dblHour),
                "iterations": int(sol.Iterations),
                "converged": True,
                "v_re": varray[0::2],
                "v_im": varray[1::2],
            }
        )

    final_elements = [capture_element(ckt, nm) for nm in ckt.AllElementNames]
    return {
        "name": spec["name"],
        "commands": cmds,
        "pre_solve": pre,
        "n_steps": spec["n_steps"],
        "node_order": list(ckt.YNodeOrder),
        "steps": steps,
        "event_log": list(sol.EventLog),
        "final_elements": final_elements,
    }


def main() -> None:
    d, oracle = make_engine()

    wanted = set(sys.argv[1:])
    unknown = wanted - set(SCENARIOS)
    if unknown:
        sys.exit(f"unknown scenario(s): {sorted(unknown)}; known: {sorted(SCENARIOS)}")
    names = [n for n in SCENARIOS if not wanted or n in wanted]

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for name in names:
        sc = build(d, SCENARIOS[name]())
        path = OUT_DIR / f"{name}.json"
        path.write_text(
            json.dumps({"schema": SCHEMA, "oracle": oracle, "scenario": sc}, indent=1) + "\n"
        )
        print(f"wrote {path.relative_to(REPO_ROOT)} ({len(sc['event_log'])} event-log lines)")

    if not wanted:
        for p in OUT_DIR.glob("*.json"):
            if p.stem not in SCENARIOS:
                p.unlink()
                print(f"removed stale {p.relative_to(REPO_ROOT)}")


if __name__ == "__main__":
    main()

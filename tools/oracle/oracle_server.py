"""Live oracle server for the corpus comparison gate (CORPUS_TEST_PLAN.md §3).

A persistent process the Rust harness (`corpus_live.rs`) drives at test time:
read one JSON request per line on stdin, run it through the pinned dss-python
oracle, write one JSON response per line on stdout. This is the *live* oracle —
no goldens are written; both engines compile the **same** copied `.dss` file and
the harness compares their results in memory.

Reuses the `capture_*` helpers from `tools/golden/gen_checkpoints.py` so the
oracle side of the live gate and the checkpoint goldens stay byte-for-byte
consistent (same unfactored-Y CSC, same column-major YPrim, same injection
slicing, ...).

Protocol — one compact JSON object per line, no embedded newlines:

    request : {"cmd":"run","case_path":<abs path>,"post":[...],"n_steps":N,
               "selected_elements":[...],"full_csc":true}
              {"cmd":"ping"} | {"cmd":"quit"}
    response: {"ok":true,"result":{...}} | {"ok":false,"error":"..."}

stdout carries responses ONLY; all diagnostics go to stderr, and the Rust side
ignores any stdout line that is not a JSON object with an "ok" field, so stray
engine output cannot corrupt the stream. One failing case returns
`{"ok":false,...}` rather than killing the server; a hard crash is handled by the
Rust side restarting the process and recording the case as a failure.

Startup hard-asserts the pin in tools/golden/PIN.txt (dss-python 0.15.7); a wrong
oracle version exits non-zero, never a silent pass.

Engine selection (`make_engine`): `DSS_ORACLE_ENGINE=capi` (default, the pinned
dss-python oracle above) or `oddie` — an official EPRI `OpenDSSDirect.dll`
(`DSS_OPENDSS_REV` -> tools/opendss/revisions.json) driven through the AltDSS
Oddie bridge from the separate venv pinned in tools/opendss/PIN_OPENDSS.txt.
See tools/opendss/README.md. The capture surface is identical; `ping` echoes
`{"oddie":true,"rev":...}` so the caller can verify which engine answered.

Usage (normally spawned by the Rust gate; manual smoke test):
    echo {"cmd":"ping"} | python tools/oracle/oracle_server.py
"""

from __future__ import annotations

import json
import os
import sys
import traceback
from math import isqrt
from pathlib import Path

# Reuse the exact capture helpers the checkpoint goldens use, so both sides of
# the live gate agree by construction.
GOLDEN = Path(__file__).resolve().parent.parent / "golden"
sys.path.insert(0, str(GOLDEN))
import gen_checkpoints as gc  # noqa: E402

# Resolved at import time — the EPRI engine chdirs the process on Compile
# (`AllowChangeDir` is not settable through Oddie), so nothing may rely on
# relative paths after the first `run` request.
OPENDSS_DIR = Path(__file__).resolve().parent.parent / "opendss"
REPO_ROOT = Path(__file__).resolve().parents[2]


def log(msg: str) -> None:
    print(msg, file=sys.stderr, flush=True)


def reply(obj: dict) -> None:
    """Write one response line to stdout and flush (compact, single line)."""
    sys.stdout.write(json.dumps(obj, separators=(",", ":")) + "\n")
    sys.stdout.flush()


def capture_all_elements(ckt) -> list:
    """Every circuit element's terminal currents (A), powers (kW/kvar), and
    losses (W/var — `CktElement.Losses`, the engine's own Get_Losses path).

    The plan mandates comparing *all* element currents/powers/losses (not just
    the selected set), so the live gate captures the whole element list here.
    """
    out = []
    for name in ckt.AllElementNames:
        cap = gc.capture_element(ckt, name)
        # capture_element leaves the element active; Losses reads it.
        loss = ckt.ActiveCktElement.Losses
        cap["loss_w"] = [float(loss[0]), float(loss[1])]
        out.append(cap)
    return out


def capture_probes(ckt, probes: list) -> list:
    """Element-specific state via the generic property surface: for each
    `{element, props: [...]}` spec, `Properties(p).Val` of the active element —
    the same value string the Rust `?` query renders (CONTROL_COVERAGE_PLAN.md).
    Compared by the harness with a numeric skeleton (numbers by value, text
    case-insensitively), so display-format drift is not load-bearing here."""
    out = []
    for spec in probes:
        name = spec["element"]
        ckt.SetActiveElement(name)
        el = ckt.ActiveCktElement
        for p in spec.get("props") or []:
            out.append({"element": name, "prop": p, "value": str(el.Properties(p).Val)})
    return out


def capture_variables(ckt, names: list) -> list:
    """PC-element state variables (`AllVariableNames`/`AllVariableValues`) —
    the live f64 state read (CLAUDE.md: the f32 monitor channel hides it)."""
    out = []
    for name in names:
        ckt.SetActiveElement(name)
        el = ckt.ActiveCktElement
        out.append(
            {
                "name": name,
                "var_names": [str(s) for s in el.AllVariableNames],
                "values": [float(v) for v in el.AllVariableValues],
            }
        )
    return out


def capture_ctrlqueue(ckt) -> list:
    """Pending control actions (`CtrlQueue.Queue` = `TControlQueue.QueueItem`
    rows). Normalized here: the constant header row and the empty-queue
    placeholder `'No events'` are dropped, so the result is exactly the pending
    rows (`Handle, Hour, Sec, ActionCode, ProxyDevRef, Device`)."""
    rows = [str(s) for s in ckt.CtrlQueue.Queue]
    return [
        r
        for r in rows
        if r.strip() and r.strip() != "No events" and not r.startswith("Handle,")
    ]


def capture_all_monitors(ckt) -> list:
    """Every monitor's data-channel header, sample count, and channel arrays.

    Mirrors the phase6 monitor golden capture (`Header`/`SampleCount`/
    `Channel(i)`); empty when the case defines no monitors. Channels are the
    growing per-sample arrays — compared per step by `corpus_live.rs`.
    """
    out = []
    mon = ckt.Monitors
    i = mon.First
    while i:
        nch = mon.NumChannels
        out.append(
            {
                "name": mon.Name,
                "header": list(mon.Header),
                "sample_count": int(mon.SampleCount),
                "channels": [[float(x) for x in mon.Channel(c)] for c in range(1, nch + 1)],
            }
        )
        i = mon.Next
    return out


def capture_all_meters(ckt) -> list:
    """Every EnergyMeter's register names/values and zone branch/end/PCE counts.

    Mirrors the phase6 meter golden capture; empty when the case defines no
    meters. Registers are integration results (compared at 1e-4 rel by the gate).
    """
    out = []
    m = ckt.Meters
    i = m.First
    while i:
        # An EMPTY string-array comes back as the C-API placeholder ['NONE']
        # (DefaultResult, like CtrlQueue's 'No events') — filter it so an empty
        # zone list compares as empty, not as a phantom one-element list.
        def _lst(v):
            xs = [str(s) for s in v]
            return [] if xs == ["NONE"] else xs

        branches = _lst(m.AllBranchesInZone)
        ends = _lst(m.AllEndElements)
        pce = _lst(m.ZonePCE)
        out.append(
            {
                "name": m.Name,
                "register_names": list(m.RegisterNames),
                "register_values": list(m.RegisterValues),
                "n_branches": len(branches),
                "n_ends": len(ends),
                "n_pce": len(pce),
                # Zone member name lists (compared as a case-insensitive set by
                # the gate — stronger than the counts above).
                "branches": branches,
                "ends": ends,
                "pce": pce,
            }
        )
        i = m.Next
    return out


# OpenDSS `Show`/`Export`/`Save` write report files into the compiled case's
# directory (`OutputDirectory := DataDirectory := <case dir>` in
# `DSSGlobals.SetDataPath`, which `Compile` calls). The live gate only compares
# the in-memory model, so those files are pure pollution of the vendored corpus.
# Setting `DataPath` before `Compile` does NOT help — `Compile` resets it to the
# case dir. So snapshot the case dir and restore it after each run instead.
_RESTORE_MAX = 2 * 1024 * 1024  # buffer files up to 2 MiB for overwrite-restore


class _CorpusGuard:
    """Restore the case's directory after a run: delete any file the run
    created, and rewrite any small pre-existing file it overwrote. Large files
    (> `_RESTORE_MAX`) are not buffered — OpenDSS only writes small text reports,
    never the multi-MiB data files (loadshape CSVs, etc.).

    `_snapshot_ok` mirrors the Rust `CorpusGuard` (corpus_live.rs): if the
    pre-run snapshot fails or is cut short, `__exit__` must not delete anything —
    a truncated `names` set would classify pre-existing corpus files as
    run-created and delete them (empirically demonstrated: a transient lock on
    one file mid-snapshot used to abort the whole listing via the old
    whole-loop `except OSError`, and the exit pass then deleted every corpus
    file that sorted after it, `YgD-Test.dss` included). Per-file failures now
    only skip that file's overwrite-restore buffer."""

    def __init__(self, case_path: str):
        self.dir = os.path.dirname(os.path.abspath(case_path))
        self.names: set[str] = set()
        self.buf: dict[str, bytes] = {}
        self._snapshot_ok = False

    def __enter__(self) -> "_CorpusGuard":
        try:
            listing = os.listdir(self.dir)
        except OSError:
            return self  # snapshot failed -> deletion stays disabled
        for name in listing:
            p = os.path.join(self.dir, name)
            try:
                if not os.path.isfile(p):
                    continue
                self.names.add(name)
                if os.path.getsize(p) <= _RESTORE_MAX:
                    with open(p, "rb") as fh:
                        self.buf[name] = fh.read()
            except OSError:
                # Unreadable (e.g. transiently locked): it is still a
                # pre-existing file — keep it in `names` so it is never
                # deleted; only its overwrite-restore is unavailable.
                self.names.add(name)
        self._snapshot_ok = True
        return self

    def __exit__(self, *exc) -> bool:
        if not self._snapshot_ok:
            return False
        try:
            current = set(os.listdir(self.dir))
        except OSError:
            return False
        for name in current - self.names:  # created by the run
            try:
                os.remove(os.path.join(self.dir, name))
            except OSError:
                pass
        for name, data in self.buf.items():  # overwritten by the run
            p = os.path.join(self.dir, name)
            try:
                with open(p, "rb") as fh:
                    if fh.read() == data:
                        continue
                with open(p, "wb") as fh:
                    fh.write(data)
            except OSError:
                pass
        return False


# The pinned engine (dss_capi 0.14.5) has a per-process nondeterminism: on a
# fresh process's FIRST compile of a deck that rewires a transformer winding to
# a new node mid-deck (`Test/YgD-Test.dss`, `Transformer.tr1.wdg=1
# bus=HV.1.2.4`), the post-rewire solve sometimes (~16% of processes,
# empirically; load-independent, PYTHONHASHSEED-independent) hits MaxIterations
# and reports `Converged=false` with NO DSS error raised — an
# uninitialized-memory-style bistability: both outcomes are bit-deterministic,
# and a `clear` + recompile IN THE SAME PROCESS heals it (never observed to
# persist past the 2nd recompile in 75 trials). The healed result is the one
# deterministic fixpoint the Rust engine matches. So: when any checkpoint
# reports non-convergence, retry the whole case in-process (loudly) before
# returning. A case that legitimately fails to converge still fails after
# `_RUN_ATTEMPTS` identical attempts — nothing is masked, only the engine's
# fresh-process misfire is absorbed. (WP8.2 Issue-2 root cause; STATUS.md §1f.)
_RUN_ATTEMPTS = 3


def run_case(d, req: dict) -> dict:
    """Compile one copied `.dss` case, run `n_steps` solves, return the full
    per-step model (the shape `harness::*` / corpus_live.rs deserialize)."""
    case_path = req["case_path"]
    post = req.get("post") or []
    n_steps = int(req.get("n_steps", 1))
    selected = req.get("selected_elements") or []
    full_csc = bool(req.get("full_csc", True))
    probes = req.get("probes") or []
    variables = req.get("variables") or []
    want_eventlog = bool(req.get("eventlog", False))
    want_ctrlqueue = bool(req.get("ctrlqueue", False))
    # Monitors/meters are compared only for cases that deliberately define them in
    # deterministic modes (the daily IEEE13 case). Capturing every master's
    # incidental monitors would surface ill-defined snapshot-sampling edge cases
    # (e.g. a monitor defined after the master's only Solve) unrelated to the gate.
    check_mm = bool(req.get("check_meters_monitors", False))

    with _CorpusGuard(case_path):
        for attempt in range(1, _RUN_ATTEMPTS + 1):
            node_order = None
            checkpoints = []
            d.Text.Command = "clear"
            d.Text.Command = f'Compile "{case_path}"'
            for c in post:
                d.Text.Command = c

            ckt = d.ActiveCircuit
            # `selected_elements=["*"]` -> every element's YPrim (small decks;
            # the Rust side then asserts the returned name set covers ALL
            # YPrim-bearing elements instead of the fixed count). Control /
            # meter elements have no YPrim (the API returns a 1-float stub) —
            # skip them, mirroring the Rust `element_yprim() == None`.
            if selected == ["*"]:
                sel = []
                for nm in ckt.AllElementNames:
                    ckt.SetActiveElement(nm)
                    flat = ckt.ActiveCktElement.Yprim
                    n = isqrt(len(flat) // 2) if flat is not None else 0
                    if n > 0 and 2 * n * n == len(flat):
                        sel.append(nm)
            else:
                sel = selected
            for _ in range(n_steps):
                d.Text.Command = "solve"
                sol = ckt.Solution
                if node_order is None:
                    node_order = list(ckt.YNodeOrder)
                varray = list(ckt.YNodeVarray)
                disc = gc.capture_discrete(ckt)
                checkpoints.append(
                    {
                        "dbl_hour": float(sol.dblHour),
                        "iterations": int(sol.Iterations),
                        "converged": bool(sol.Converged),
                        "v_re": varray[0::2],
                        "v_im": varray[1::2],
                        "y": gc.capture_system_y(d) if full_csc else None,
                        "y_fingerprint": gc.capture_fingerprint(d),
                        "yprims": [gc.capture_yprim(ckt, nm) for nm in sel],
                        "elements": capture_all_elements(ckt),
                        "injection": gc.capture_injection(d),
                        "transformers": disc["transformers"],
                        "regcontrols": disc["regcontrols"],
                        "capacitors": disc["capacitors"],
                        "monitors": capture_all_monitors(ckt) if check_mm else [],
                        "meters": capture_all_meters(ckt) if check_mm else [],
                        "probes": capture_probes(ckt, probes),
                        "variables": capture_variables(ckt, variables),
                        "eventlog": (
                            [str(s) for s in sol.EventLog] if want_eventlog else []
                        ),
                        "ctrlqueue": capture_ctrlqueue(ckt) if want_ctrlqueue else [],
                    }
                )
            bad = [i for i, cp in enumerate(checkpoints) if not cp["converged"]]
            if not bad:
                break
            log(
                f"oracle retry: {case_path} attempt {attempt}/{_RUN_ATTEMPTS} "
                f"non-converged step(s) {bad} (pinned-engine fresh-process "
                f"misfire, see run_case doc / STATUS.md §1f); "
                + ("recompiling in-process" if attempt < _RUN_ATTEMPTS else "returning as-is")
            )
    return {"node_order": node_order, "n_steps": n_steps, "checkpoints": checkpoints}


def _oddie_get_y_sparse(d):
    """Oddie-mode replacement for `gc._get_y_sparse`: the fastdss dss-python
    (0.16.0b2) `getYSparse()` takes no `factor` argument (Oddie ignores the
    flag; EPRI's `InitAndGetYparams` ALWAYS factors before the CSC export —
    proven solution-neutral by `tools/opendss/smoke.py`'s bit-identical
    YNodeVarray check). Same `BuildY` retry as the capi original."""
    r = d.YMatrix.getYSparse()
    if r is None:
        d.Text.Command = "BuildY"
        r = d.YMatrix.getYSparse()
    if r is None:
        raise RuntimeError("YMatrix.getYSparse returned None even after a BuildY retry")
    return r


def _read_pin_opendss() -> dict:
    pins = {}
    for line in (OPENDSS_DIR / "PIN_OPENDSS.txt").read_text().splitlines():
        line = line.split("#", 1)[0].strip()
        if "==" in line:
            k, v = line.split("==", 1)
            pins[k.strip()] = v.strip()
    return pins


def make_engine():
    """Bind the oracle engine from `DSS_ORACLE_ENGINE`:

    - "capi" (default) — the pinned dss-python 0.15.7 / dss_capi 0.14.5 oracle,
      exactly as before (`gc.check_pin()` + the `dss.DSS` singleton);
    - "oddie" — an OFFICIAL EPRI `OpenDSSDirect.dll` loaded by absolute path
      through the AltDSS Oddie bridge (dss-python 0.16.0b2 `IOddieDSS`, the
      separate venv pinned in tools/opendss/PIN_OPENDSS.txt). The revision
      comes from `DSS_OPENDSS_REV` (looked up in tools/opendss/revisions.json,
      whose `expect_version` must be non-empty — no silent pass), or a direct
      `DSS_OPENDSS_DLL` path override (+ optional `DSS_OPENDSS_EXPECT`
      version-substring check).
    """
    engine = os.environ.get("DSS_ORACLE_ENGINE", "capi")
    if engine == "capi":
        oracle = gc.check_pin()  # hard-asserts dss-python 0.15.7 / engine 0.14.5
        from dss import DSS as d

        return d, oracle
    if engine != "oddie":
        sys.exit(f"unknown DSS_ORACLE_ENGINE={engine!r} (expected 'capi' or 'oddie')")

    import dss

    pin = _read_pin_opendss()
    if dss.__version__ != pin["dss-python"]:
        sys.exit(
            f"dss-python {dss.__version__} != pinned {pin['dss-python']} "
            "(tools/opendss/PIN_OPENDSS.txt — is DSS_ORACLE_PYTHON the Oddie venv?)"
        )
    rev = os.environ.get("DSS_OPENDSS_REV", "")
    dll = os.environ.get("DSS_OPENDSS_DLL", "")
    expect = os.environ.get("DSS_OPENDSS_EXPECT", "")
    if not dll:
        revs = json.loads((OPENDSS_DIR / "revisions.json").read_text())
        if rev not in revs:
            sys.exit(f"DSS_OPENDSS_REV={rev!r} not in revisions.json ({sorted(revs)})")
        dll = str((REPO_ROOT / revs[rev]["dll"]).resolve())
        expect = expect or revs[rev].get("expect_version", "")
        if not expect:
            sys.exit(
                f"revisions.json expect_version for {rev} is empty — "
                "run tools/opendss/smoke.py and pin it (no silent pass)"
            )
    from dss import IOddieDSS

    d = IOddieDSS(library_path=dll)
    ver = str(d.Version)
    if expect and expect not in ver:
        sys.exit(f"engine {ver!r} does not contain pinned {expect!r} (rev={rev!r})")
    # Suppress dialogs BEFORE any Text command — an engine error message while
    # forms are still allowed pops a modal dialog (main() sets this again;
    # harmless).
    d.AllowForms = False
    # EPRI's Delphi `FireOffEditor` (Utilities.pas) ShellExecutes `DefaultEditor`
    # on every `Show`/`Export` UNCONDITIONALLY — it has no NoFormsAllowed check,
    # and `AllowEditor` is not settable through Oddie, so a corpus sweep would
    # open hundreds of Notepads (empirically did). Point the editor at
    # rundll32.exe — a GUI-subsystem no-op (no DLL entry point given -> exits
    # silently, no window) — and stop the engine from persisting that override
    # into the user's OpenDSS registry settings on dispose (`Set RegistryUpdate`,
    # ExecOption[102] — the option name differs from the Pascal variable
    # `UpdateRegistry`; identical in r3723/r4088/r4133).
    d.Text.Command = "Set RegistryUpdate=No"
    d.Text.Command = "Set Editor=rundll32.exe"
    # fastdss getYSparse() signature differs; capture_system_y/capture_fingerprint
    # route through gc._get_y_sparse, so rebind it for this process.
    gc._get_y_sparse = _oddie_get_y_sparse
    return d, {"engine": ver, "oddie": True, "rev": rev, "dll": dll}


def main() -> None:
    d, oracle = make_engine()

    d.AllowForms = False
    # `Show`/`Export`/`FileEdit` call `FireOffEditor`, which opens the report in
    # the OS editor (notepad on Windows) — disruptive when the gate runs cases
    # that contain `Show`. `AllowForms=False` does NOT suppress it; `AllowEditor`
    # does. (Pair with `_CorpusGuard`, which deletes the report files themselves.)
    try:
        d.AllowEditor = False
    except Exception:  # older dss-python without the attribute
        pass
    log(f"oracle_server ready: {oracle}")

    # NB: read with readline(), not `for line in sys.stdin` — the latter reads
    # ahead in large blocks and would deadlock the request/response protocol
    # (the server would not see a request until its read buffer filled).
    while True:
        raw = sys.stdin.readline()
        if not raw:  # EOF: stdin closed
            break
        line = raw.strip()
        if not line:
            continue
        try:
            req = json.loads(line)
        except json.JSONDecodeError as e:
            reply({"ok": False, "error": f"bad request json: {e}"})
            continue
        cmd = req.get("cmd")
        if cmd == "quit":
            break
        if cmd == "ping":
            reply({"ok": True, "result": {"pong": True, "oracle": oracle}})
            continue
        if cmd != "run":
            reply({"ok": False, "error": f"unknown cmd {cmd!r}"})
            continue
        try:
            reply({"ok": True, "result": run_case(d, req)})
        except Exception as e:  # one bad case must not kill the server
            log("case failed:\n" + traceback.format_exc())
            reply({"ok": False, "error": f"{type(e).__name__}: {e}"})


if __name__ == "__main__":
    main()

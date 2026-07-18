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


def capture_all_elements(ckt, tolerate_user_model: bool = False) -> list:
    """Every circuit element's terminal currents (A), powers (kW/kvar), and
    losses (W/var — `CktElement.Losses`, the engine's own Get_Losses path).

    The plan mandates comparing *all* element currents/powers/losses (not just
    the selected set), so the live gate captures the whole element list here.

    `tolerate_user_model` (CF-C Port 2): a Generator model=6 whose user-written
    model is not loaded fires DoSimpleMsg #567 the FIRST time its terminal
    currents are recomputed after a solve; the recompute still produces the
    correct (Yprim-only) currents and clears the error, so a second read returns
    them cleanly (verified: read 1 raises #567 + zeroes Error.Number, read 2 OK).
    We absorb that single priming raise (retry once) exactly as the official
    Direct DLL warns-and-continues; any other errno re-raises.
    """
    import dss as _dss

    def _read(fn):
        try:
            return fn()
        except _dss.DSSException as e:
            errno = e.args[0] if e.args else None
            if not (tolerate_user_model and errno in _USER_MODEL_ERRNOS):
                raise
            return fn()  # priming read fired the warning + cleared it; retry is cached

    out = []
    for name in ckt.AllElementNames:
        cap = _read(lambda: gc.capture_element(ckt, name))
        # capture_element leaves the element active; Losses reads it.
        loss = _read(lambda: ckt.ActiveCktElement.Losses)
        cap["loss_w"] = [float(loss[0]), float(loss[1])]
        out.append(cap)
    return out


def capture_eventlog(d, ckt) -> list:
    """The cumulative event log (`DSS.EventStrings`), as a list of lines.

    The AltDSS **Oddie** bridge (official EPRI DLL) does NOT populate the
    `Solution.EventLog` COM accessor — it always returns empty — even though the
    engine records events (protection trips/recloses/etc.) and `export eventlog`
    writes the real CSV. So for the Oddie engine we capture via that export and
    read the CSV back; the pinned dss-python (`capi*`) engines keep the direct
    `Solution.EventLog` read (bit-identical lines). WP-U2.2.
    """
    if type(d).__name__ == "IOddieDSS":
        d.Text.Command = "export eventlog"
        path = str(d.Text.Result).strip().lstrip("﻿")
        try:
            # `utf-8-sig` strips a leading UTF-8 BOM: the official r4133 DLL writes
            # its `export eventlog` CSV with a BOM, so a plain-`utf-8` read would
            # (a) glue `﻿` onto the first event line and (b) turn an EMPTY log
            # into a spurious one-line `['﻿']` (the BOM is not `str.isspace()`,
            # so `if ln.strip()` keeps it). WP-U2.5.
            with open(path, encoding="utf-8-sig", errors="replace") as f:
                # The CSV has no header row — each line is a full event record
                # ("Hour=…, Sec=…, ControlIter=…, Element=…, Action=…"). Delphi's
                # file writer prefixes the file with a UTF-8 BOM (U+FEFF); strip
                # it per line so (a) an EMPTY event log (which exports as a lone
                # BOM) reads back as [] rather than ["﻿"], and (b) the first
                # record is not BOM-glued — matching the Rust `event_log()` and
                # the capi engines' `Solution.EventLog`, both BOM-free. Without
                # this an empty step trips the length assert (0 vs 1) and a
                # non-empty first line trips the numeric-skeleton comparator on
                # the multi-byte BOM. WP-U2.1 restore.
                out = []
                for ln in f:
                    ln = ln.lstrip("﻿").rstrip("\r\n")
                    if ln.strip():
                        out.append(ln)
                return out
        except OSError:
            return []
    return [str(s) for s in ckt.Solution.EventLog]


def capture_probes(d, probes: list) -> list:
    """Element-specific state via the generic property surface: for each
    `{element, props: [...]}` spec, the `? element.prop` executive query
    (CONTROL_COVERAGE_PLAN.md) — the same value string the Rust `?` query
    renders, and the same read path `GetPropertyValue` backs `Properties(p).Val`
    with for a `TDSSCktElement`. Routed through the query (not
    `ActiveCktElement.Properties(p).Val`) because `SetActiveElement` only finds
    `TDSSCktElement`s: it silently no-ops (returns -1, active element
    unchanged) for a `DSS_OBJECT` class with no terminals — LoadShape/TShape/
    PriceShape/GrowthShape/… (WPG.1) — which would otherwise read back
    whatever CktElement happened to be active. Verified bit-identical to
    `Properties(p).Val` for a CktElement probe."""
    out = []
    for spec in probes:
        name = spec["element"]
        for p in spec.get("props") or []:
            d.Text.Command = f"? {name}.{p}"
            out.append({"element": name, "prop": p, "value": str(d.Text.Result)})
    return out


def capture_all_properties(d, ckt) -> list:
    """WP8.5b: EVERY circuit element's EVERY property value, as an ordered
    `[[prop, str(Val)]]` list over the class's `AllPropertyNames` (property-index
    order is the contract compared against the Rust `?`-surface).

    Read exactly like `capture_probes`: the `? element.prop` executive query
    (not `ActiveCktElement.Properties(p).Val`), which backs the same
    `GetPropertyValue` path AND — the WPG.1 harness bug — activates the object
    for BOTH `DSS_OBJECT` and `TDSSCktElement` classes, whereas `SetActiveElement`
    silently no-ops for a terminal-less `DSS_OBJECT`. `AllElementNames` is only
    circuit elements, but `? name.Like` (a read that activates the object without
    needing to know a property name yet; the exact trick `gen_props.py` uses)
    keeps the enumeration on the identical WPG.1-safe path so the property-name
    list read matches the value reads. Runs AFTER `capture_all_elements`, so the
    established element read order is preserved."""
    out = []
    for name in ckt.AllElementNames:
        # Activate via the query path, then read the class property-name list off
        # the now-active DSS object (ActiveDSSElement, not ActiveCktElement —
        # covers DSS_OBJECT classes too).
        d.Text.Command = f"? {name}.Like"
        prop_names = list(ckt.ActiveDSSElement.AllPropertyNames)
        props = []
        for p in prop_names:
            d.Text.Command = f"? {name}.{p}"
            props.append([p, str(d.Text.Result)])
        out.append({"element": name, "props": props})
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
        # zone list compares as empty, not as a phantom one-element list. The
        # official-EPRI (Oddie) engine additionally renders these string arrays
        # with a trailing empty element (`['load.a', …, '']` — the same Delphi
        # trailing-separator artifact as the monitor CSV header), so drop
        # empty/whitespace-only entries too; an element name is never empty.
        def _lst(v):
            xs = [s for s in (str(s).strip() for s in v) if s]
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
# Lifted move-only into corpus_guard.py (2026-07-07) so the DSS-Python
# validation harness (tools/opendss/dsspy_validation/) shares the identical,
# empirically-hardened implementation.
from corpus_guard import CorpusGuard as _CorpusGuard  # noqa: E402


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

# Compile-time DoSimpleMsg error numbers the OFFICIAL OpenDSS engine treats as
# NON-fatal warnings and solves through, but which dss-python's binding raises on
# (because it checks `Error.Number` after every `Text.Command`). We tolerate them
# during the deck `Compile` — clear the raised error and continue — matching the
# official Direct DLL's "warn and continue" behavior AND the Rust engine, which
# reports the same conditions on `GlobalResult` rather than as a hard error.
#   250 — `Export monitor <name>` where <name> is undefined (a typo'd monitor
#         name; the EPRI ckt5/ckt7 Run scripts export a mis-typed monitor as
#         their LAST command, after the full solve). See report/report.rs
#         `export_monitors` for the matching Rust side.
# Kept minimal + greppable: add a number here only with the same 1:1-with-official
# justification. A non-listed error still re-raises (real failures never masked).
_TOLERATED_COMPILE_ERRNOS = {250}

# User-written-model DoSimpleMsg numbers the official Direct DLL treats as
# NON-fatal (warn and continue) but dss-python raises on. Tolerated at BOTH
# compile and every solve ONLY for a case that opts in via `warn_and_continue`
# (the Rust harness sets it for a deck carrying `expect_warnings` — CF-C Port 2:
# a Generator model=6 / Storage user-dynamics DLL that safe Rust cannot load, so
# both engines must warn-and-solve, exactly the official Direct DLL).
#   567  — Generator model designated to use a user-written model that is not
#          defined (fired every solve iteration by the model=6 Yprim-only path);
#   570  — Generator user-written model <name> Not Loaded (fired at compile);
#   1570 — Storage user-written dynamics model Not Loaded.
# These also require DSS.Error.EarlyAbort=False so the compile is not aborted
# mid-redirect (else the circuit truncates — the user-model line sits inside a
# Redirect chain); EarlyAbort is toggled per request in main() and restored.
_USER_MODEL_ERRNOS = {567, 570, 1570}


def _set_early_abort(d, val) -> bool:
    """Best-effort `DSS.Error.EarlyAbort = val`. Returns True on success. The
    Oddie bridge does not implement `Error_Set_EarlyAbort` (raises #2), but the
    raw Direct DLL already warns-and-continues, so a failure here is harmless."""
    try:
        d.Error.EarlyAbort = val
        return True
    except Exception:
        return False


def _get_early_abort(d):
    """Current `DSS.Error.EarlyAbort`, or None if the engine does not expose it
    (Oddie) — in which case there is nothing to save/restore."""
    try:
        return bool(d.Error.EarlyAbort)
    except Exception:
        return None


def run_case(d, req: dict) -> dict:
    """Compile one copied `.dss` case, run `n_steps` solves, return the full
    per-step model (the shape `harness::*` / corpus_live.rs deserialize)."""
    import dss as _dss  # module already loaded by make_engine; for DSSException

    case_path = req["case_path"]
    post = req.get("post") or []
    n_steps = int(req.get("n_steps", 1))
    selected = req.get("selected_elements") or []
    full_csc = bool(req.get("full_csc", True))
    probes = req.get("probes") or []
    variables = req.get("variables") or []
    want_eventlog = bool(req.get("eventlog", False))
    want_ctrlqueue = bool(req.get("ctrlqueue", False))
    # WP8.5b corpus property parity: the full per-element property dump. Opt-in
    # (heavy: elements x props x steps queries) — the Rust property gate and the
    # env-gated `corpus_live_properties` pilot force it.
    want_all_props = bool(req.get("all_properties", False))
    # WPG.5 (AutoAdd): `DSS.GlobalResult` after each solve (the winner + figure)
    # and the `<CircuitName_>AutoAddLog.csv` the search writes. `Text.Result` is
    # captured IMMEDIATELY after `solve`, before any `?`-query capture overwrites
    # it. The AutoAdd solve segfaults dss-python AT PROCESS EXIT (GAPS_PLAN.md
    # 2.2); this one-shot server has already flushed its JSON reply by then, so
    # the crash never loses the capture.
    want_global_result = bool(req.get("global_result", False))
    want_autoadd_log = bool(req.get("autoadd_log", False))
    # Monitors/meters are compared only for cases that deliberately define them in
    # deterministic modes (the daily IEEE13 case). Capturing every master's
    # incidental monitors would surface ill-defined snapshot-sampling edge cases
    # (e.g. a monitor defined after the master's only Solve) unrelated to the gate.
    check_mm = bool(req.get("check_meters_monitors", False))
    # CF-C Port 2 user-model decks: tolerate the `_USER_MODEL_ERRNOS` at compile
    # AND at every solve (EarlyAbort is turned off around this call in main()).
    warn_and_continue = bool(req.get("warn_and_continue", False))
    tolerated_compile = _TOLERATED_COMPILE_ERRNOS | (
        _USER_MODEL_ERRNOS if warn_and_continue else set()
    )

    with _CorpusGuard(case_path):
        for attempt in range(1, _RUN_ATTEMPTS + 1):
            node_order = None
            checkpoints = []
            d.Text.Command = "clear"
            try:
                d.Text.Command = f'Compile "{case_path}"'
            except _dss.DSSException as e:
                # `e.args == (errno, message)`. Tolerate only the warning-class
                # numbers the official engine solves through (see
                # `_TOLERATED_COMPILE_ERRNOS` / `_USER_MODEL_ERRNOS`); the whole
                # deck has already run, so the circuit is intact and `Error.Number`
                # is cleared on catch. Anything else re-raises — a real compile
                # failure is never swallowed.
                errno = e.args[0] if e.args else None
                if errno not in tolerated_compile:
                    raise
                log(f"oracle: tolerated non-fatal compile warning #{errno} on {case_path}")
            for c in post:
                d.Text.Command = c

            ckt = d.ActiveCircuit
            for _ in range(n_steps):
                try:
                    d.Text.Command = "solve"
                except _dss.DSSException as e:
                    # A user-model deck (`warn_and_continue`) fires its non-fatal
                    # DoSimpleMsg EACH solve; with EarlyAbort off the solve
                    # completes, but dss-python still raises at the command
                    # boundary — clear it and read the finished solution. Any
                    # other errno re-raises.
                    errno = e.args[0] if e.args else None
                    if not (warn_and_continue and errno in _USER_MODEL_ERRNOS):
                        raise
                    log(f"oracle: tolerated non-fatal solve warning #{errno} on {case_path}")
                # WPG.5: read GlobalResult right after the solve, before any
                # `?`-query capture below overwrites `Text.Result`.
                global_result = str(d.Text.Result) if want_global_result else ""
                # `selected_elements=["*"]` -> every element's YPrim (small decks;
                # the Rust side then asserts the returned name set covers ALL
                # YPrim-bearing elements instead of the fixed count). Control /
                # meter elements have no YPrim (the API returns a 1-float stub) —
                # skip them, mirroring the Rust `element_yprim() == None`.
                # Rebuilt AFTER each solve so a deck that adds an element during
                # the solve (WPG.5 AutoAdd appends `Generator.Gadd1`) is covered;
                # for every other deck the pre/post-solve element set is identical.
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
                        "elements": capture_all_elements(ckt, warn_and_continue),
                        "injection": gc.capture_injection(d),
                        "transformers": disc["transformers"],
                        "regcontrols": disc["regcontrols"],
                        "capacitors": disc["capacitors"],
                        "monitors": capture_all_monitors(ckt) if check_mm else [],
                        "meters": capture_all_meters(ckt) if check_mm else [],
                        "probes": capture_probes(d, probes),
                        "variables": capture_variables(ckt, variables),
                        "eventlog": (capture_eventlog(d, ckt) if want_eventlog else []),
                        "ctrlqueue": capture_ctrlqueue(ckt) if want_ctrlqueue else [],
                        # WP8.5b: read LAST, after every established capture above,
                        # so the property sweep's `?` queries never perturb any
                        # other read's active-element state.
                        "all_properties": (
                            capture_all_properties(d, ckt) if want_all_props else []
                        ),
                        "global_result": global_result,
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

        # WPG.5: read the `<CircuitName_>AutoAddLog.csv` written to the case
        # dir (OutputDirectory := <case dir> after Compile). Read inside the
        # `_CorpusGuard` scope, before it removes the file on exit.
        autoadd_log = None
        if want_autoadd_log:
            log_path = os.path.join(
                os.path.dirname(os.path.abspath(case_path)),
                f"{ckt.Name}_AutoAddLog.csv",
            )
            if os.path.exists(log_path):
                with open(log_path, "r", encoding="utf-8", errors="replace") as fh:
                    autoadd_log = fh.read()

    return {
        "node_order": node_order,
        "n_steps": n_steps,
        "checkpoints": checkpoints,
        "autoadd_log": autoadd_log,
    }


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
    - "capi015" — the dss_capi 0.15.x-line oracle: dss-python 0.16.0b2 (fastdss)
      from the SAME separate venv the Oddie bridge uses (pinned in
      tools/opendss/PIN_OPENDSS.txt), driving its own bundled dss_capi
      0.15.0b4 backend (based on OpenDSS SVN r4103 — the 0.15.x/r4088 line).
      This is the scriptable r4088-line oracle for the UPGRADE_PLAN target-rev
      gates; `ping` echoes `{"capi015": true}` so the caller can verify.
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
    if engine == "capi015":
        import dss

        pin = _read_pin_opendss()
        if dss.__version__ != pin["dss-python"]:
            sys.exit(
                f"dss-python {dss.__version__} != pinned {pin['dss-python']} "
                "(tools/opendss/PIN_OPENDSS.txt — is DSS_ORACLE_PYTHON the Oddie venv?)"
            )
        from dss import DSS as d

        ver = str(d.Version)
        backend = pin.get("dss-python-backend", "")
        if not backend:
            sys.exit("PIN_OPENDSS.txt has no dss-python-backend pin (no silent pass)")
        if backend not in ver:
            sys.exit(f"engine {ver!r} does not contain pinned backend {backend!r}")
        # fastdss getYSparse() drops the `factor` argument (same as Oddie).
        gc._get_y_sparse = _oddie_get_y_sparse
        return d, {"engine": ver, "capi015": True}
    if engine != "oddie":
        sys.exit(
            f"unknown DSS_ORACLE_ENGINE={engine!r} (expected 'capi', 'capi015' or 'oddie')"
        )

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
        if cmd == "clear":
            # Release the active circuit (and any held loadshape memory-mapped
            # file handle) so another process can compile the same case without a
            # concurrent-mapping conflict. Used only by tools/opendss/xcheck_bridge.py.
            try:
                d.Text.Command = "clear"
                reply({"ok": True, "result": {"cleared": True}})
            except Exception as e:  # never kill the server on a clear
                reply({"ok": False, "error": f"{type(e).__name__}: {e}"})
            continue
        if cmd != "run":
            reply({"ok": False, "error": f"unknown cmd {cmd!r}"})
            continue
        # CF-C Port 2: a `warn_and_continue` deck needs EarlyAbort off for the
        # WHOLE run (compile + solves) so a mid-redirect user-model warning does
        # not truncate the circuit. Toggle it per request and always restore, so
        # the setting never leaks into the next (persistent-server) case.
        warn = bool(req.get("warn_and_continue", False))
        prev_ea = _get_early_abort(d) if warn else None
        if warn:
            _set_early_abort(d, False)
        try:
            reply({"ok": True, "result": run_case(d, req)})
        except Exception as e:  # one bad case must not kill the server
            log("case failed:\n" + traceback.format_exc())
            reply({"ok": False, "error": f"{type(e).__name__}: {e}"})
        finally:
            if warn and prev_ea is not None:
                _set_early_abort(d, prev_ea)


if __name__ == "__main__":
    main()

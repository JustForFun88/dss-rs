"""Live oracle server for the corpus comparison gate (CORPUS_TEST_PLAN.md §3).

A persistent process the Rust harness (`corpus_gate.rs`) drives at test time:
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

This server drives the pinned dss-python oracle only (the `capi_v0145` channel of
the unified corpus gate). The official EPRI `OpenDSSDirect.dll` (r4133) channel is
driven by the in-house `crates/dss-epri` Rust bridge (`epri-worker`), not this
server.

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

def log(msg: str) -> None:
    print(msg, file=sys.stderr, flush=True)


def reply(obj: dict) -> None:
    """Write one response line to stdout and flush (compact, single line)."""
    sys.stdout.write(json.dumps(obj, separators=(",", ":")) + "\n")
    sys.stdout.flush()


def _tolerant_read(fn, tolerate_user_model: bool):
    """Run `fn`, absorbing the ONE priming `_USER_MODEL_ERRNOS` raise a
    user-written-model deck fires the first time its terminal currents are
    recomputed after a solve (the measurement is in `capture_all_elements`'s
    doc: read 1 raises #567 and zeroes `Error.Number`, read 2 returns the
    correct Yprim-only values). Any other errno re-raises — a real failure is
    never masked.

    `capture_all_elements` calls this one definition too (its inline copy was
    deduped into this helper at the D7 lane merge, 2026-09-05); the absorbed
    errno set and the single retry are therefore identical on both paths.
    """
    import dss as _dss

    try:
        return fn()
    except _dss.DSSException as e:
        errno = e.args[0] if e.args else None
        if not (tolerate_user_model and errno in _USER_MODEL_ERRNOS):
            raise
        return fn()  # priming read fired the warning + cleared it; retry is cached


def capture_aggregates(ckt, tolerate_user_model: bool = False) -> dict:
    """The five `Circuit` aggregates of GOLDEN_REBASE_PLAN.md G1.9, with their
    units spelled in the key names because the engine does NOT scale them
    uniformly:

    * `Circuit.Losses` (r4133 `DDLL/DCircuit.pas:294` -> `Common/Circuit.pas:2436-2443`)
      is **W/var** — the raw sum over the enabled, non-shunt PD elements, with
      no `x 0.001`;
    * `LineLosses` (`DCircuit.pas:305-325`), `SubstationLosses`
      (`:327-347`, `IsSubstation` transformers only — AutoTrans lives on its own
      list, `Common/Circuit.pas:2272-2273`, and never contributes),
      `TotalPower` (`:349-368`, terminal 1 of every Source) and
      `AllElementLosses` (`:458-479`, one complex per element in
      `AllElementNames` order) all carry the `cmulreal(..., 0.001)` => kW/kvar.

    Every one of them is a `Get_Losses`/`Get_Power` read, i.e. a
    `ComputeIterminal` (`Common/CktElement.pas:743` / `:677-680`) over the
    elements it walks — a **group-A** read in the §1.1(a)/D3 partition, so the
    call site puts it ahead of every group-B read. See `run_case`.

    `tolerate_user_model` mirrors `capture_all_elements`: since this is now the
    first post-solve read that recomputes `Iterminal`, a `warn_and_continue`
    deck fires its single priming #567 here.
    """

    def _read():
        losses = ckt.Losses  # W/var
        line_losses = ckt.LineLosses  # kW/kvar
        sub_losses = ckt.SubstationLosses  # kW/kvar
        total_power = ckt.TotalPower  # kW/kvar
        ael = ckt.AllElementLosses  # kW/kvar, 2 * NumDevices flat
        return {
            "losses_w": [float(losses[0]), float(losses[1])],
            "line_losses_kw": [float(line_losses[0]), float(line_losses[1])],
            "substation_losses_kw": [float(sub_losses[0]), float(sub_losses[1])],
            "total_power_kw": [float(total_power[0]), float(total_power[1])],
            "all_element_losses_kw": [float(x) for x in ael],
        }

    return _tolerant_read(_read, tolerate_user_model)


def capture_solution_scalars(sol) -> dict:
    """The ten `Solution` scalars of G1.9 — `DDLL/DSolution.pas:29` (Mode),
    `:37` (Hour), `:47` (Year), `:113` (ControlIterations), `:218`
    (Totaliterations), `:222` (MostIterationsDone), `:226`
    (ControlActionsDone), `:192` (SystemYChanged), `:312` (Seconds), `:336`
    (LoadMult).

    All ten are **order-free** (group C): plain field reads that touch no
    cursor and no `Iterminal` cache, so their position in the capture is free.
    They are read here anyway, beside the aggregates, so the whole G1.9 surface
    is one block.

    `Iterations` (`:54`) and `dblHour` (`:400`) are deliberately absent — the
    checkpoint already carries and compares them. `Totaliterations` IS carried:
    r4133 returns `Solution.Iteration` for it verbatim (`:218-220`), and the
    equality is pinned in-engine rather than compared twice (TESTING.md).

    `ControlActionsDone` and `SystemYChanged` are emitted as JSON **booleans**;
    the r4133 transport normalizes its `0|1` ints to the same shape
    (`dss-epri/src/capture.rs::capture_solution_scalars`).
    """
    return {
        "mode": int(sol.Mode),
        "hour": int(sol.Hour),
        "year": int(sol.Year),
        "control_iterations": int(sol.ControlIterations),
        "total_iterations": int(sol.Totaliterations),
        "most_iterations_done": int(sol.MostIterationsDone),
        "control_actions_done": bool(sol.ControlActionsDone),
        "system_y_changed": bool(sol.SystemYChanged),
        "seconds": float(sol.Seconds),
        "load_mult": float(sol.LoadMult),
    }


def _polar_pair(flat) -> tuple:
    """De-interleave a Pascal `[mag, ang, mag, ang, ...]` polar array into a
    `(mags, angs)` pair — the `i_re`/`i_im`, `p_kw`/`p_kvar` convention
    `gen_checkpoints.capture_element` already uses, so the Rust comparator
    never does stride-2 index arithmetic. Angles are degrees on the
    `(-180, 180]` branch cut (`Ctopolardeg` -> `CDang`, r4133
    `Shared/Ucomplex.pas:118`)."""
    vals = list(flat)
    return [float(x) for x in vals[0::2]], [float(x) for x in vals[1::2]]


def _re_im_pair(flat) -> tuple:
    """De-interleave a Pascal complex `[re, im, re, im, ...]` array into a
    `(res, ims)` pair — the same `i_re`/`i_im`, `p_kw`/`p_kvar` convention
    `gen_checkpoints.capture_element` uses, so the Rust comparator never does
    stride-2 index arithmetic.

    Identical arithmetic to `_polar_pair`, deliberately NOT the same function:
    the name is what tells the reader (and the comparator author) that the two
    halves are rectangular components, not `(magnitude, angle)`."""
    vals = list(flat)
    return [float(x) for x in vals[0::2]], [float(x) for x in vals[1::2]]


def capture_all_elements(
    ckt,
    tolerate_user_model: bool = False,
    derived: bool = False,
    element_extras: bool = False,
) -> list:
    """Every circuit element's terminal currents (A), powers (kW/kvar), and
    losses (W/var — `CktElement.Losses`, the engine's own Get_Losses path);
    under `derived` also `Enabled`, the three polar channels
    `CurrentsMagAng` / `VoltagesMagAng` / `Residuals` (GOLDEN_REBASE G1.3a) and
    the three sequence channels `SeqPowers` / `SeqCurrents` / `SeqVoltages`
    (GOLDEN_REBASE G1.3b) and the complex-sequence + per-terminal-total trio
    `TotalPowers` / `CplxSeqCurrents` / `CplxSeqVoltages` (GOLDEN_REBASE G1.3c);
    under `element_extras` also `Enabled` and the discrete index/name scalars
    `NumTerminals` / `NumConductors` / `NumPhases` / `EnergyMeter` / `NodeOrder`
    (GOLDEN_REBASE G1.3d(i)).

    The plan mandates comparing *all* element currents/powers/losses (not just
    the selected set), so the live gate captures the whole element list here.

    Capture order is contractual (GOLDEN_REBASE_PLAN.md §1.1(a), D3) and is
    asserted from the `capture-order: NAME (A|B|C)` markers below by
    `crates/dss-core/tests/capture_order.rs` — a marker sits on the read line
    itself, or on the comment line immediately above it when the read does not
    fit; a call into another capture helper declares the reads that helper
    performs, in its order:

      A  cache-aware reads answered through `ComputeIterminal` — `Powers`,
         `Losses` (r4133 `Common/CktElement.pas:707` `Get_Losses`, capi
         `Common/CktElement.pas:601`), `TotalPowers`, `PhaseLosses`;
      B  reads that run `GetCurrents` into a scratch buffer — `Currents`,
         `CurrentsMagAng` (capi `CAPI/CAPI_Alt.pas:1043`), `Residuals` (capi
         `CAPI/CAPI_CktElement.pas:541`, r4133 `DDLL/DCktElement.pas:827`,
         both carrying the `(i-1)*Nconds` terminal offset), `SeqCurrents`,
         `CplxSeqCurrents`, `SeqPowers`;
      C  order-free reads — the element selectors, discrete state, and the
         voltages (`VoltagesMagAng` reads `NodeV[NodeRef[i]]` only, capi
         `CAPI/CAPI_Alt.pas:1072`).

    Every A read must precede every B read: `TPCElement.GetTerminalCurrents`
    fills the CALLER's buffer yet still stamps `IterminalSolutionCount` (r4133
    `PCElements/PCElement.pas:247`, stamp at `:265`; capi `:107`, stamp at
    `:126`), leaving `Iterminal` itself stale while the cache reads as fresh,
    so a cache-aware read that follows one can answer from that stale cache
    (CLAUDE.md upstream bug 4, harmonics `Powers`-after-`Currents`).
    `Losses` is therefore read BEFORE `gc.capture_element` rather than after
    it — a reordering of two group-A reads, so no captured value moves
    (proven byte-for-byte on IEEE13, two harmonics decks and the two
    user-model decks; G1.3a record).

    `derived` (request key `"derived"`, manifest flag `compare_derived`): the
    three polar channels are read for `Enabled` elements ONLY. r4133's
    `CktElementV(19)` (`VoltagesMagAng`, `DDLL/DCktElement.pas:1099`)
    dereferences `NodeRef^[i]` with no nil guard and kills the worker on a
    never-enabled element, where capi returns its 1-element `DefaultResult`
    (`CAPI/CAPI_Alt.pas:1081` guards `elem.NodeRef = NIL`); capturing enabled
    elements only removes that crash class AND makes the two channels' shapes
    identical, so no sentinel normalization is owed. `enabled` itself is
    captured for every element and compared exactly.

    The same `derived` flag also carries the three GOLDEN_REBASE G1.3b sequence
    channels — `SeqPowers` (`CktElementV(9)`, r4133
    `DDLL/DCktElement.pas:739`; capi `Alt_CE_Get_SeqPowers`
    `CAPI/CAPI_Alt.pas:594` -> `Alt_CE_Get_SeqPowers_` `:529`), `SeqCurrents`
    (`CktElementV(8)`, `:700`; capi `:490`) and `SeqVoltages`
    (`CktElementV(7)`, `:660`; capi `:620`) — read for `Enabled` elements ONLY.
    Here that rule is load-bearing on BOTH engines, not just a shape
    normalization: r4133's mode 9 has neither an `Enabled` nor a `NodeRef`
    guard and dereferences `NodeRef^[k+1]` (`:765`) on a never-enabled element,
    while capi's outer `Alt_CE_Get_SeqPowers` deliberately skips the `Enabled`
    test (`:604`, commented out) yet still resizes the result buffer to
    `3 * NTerms` complex slots at `:608` **before** the helper's own
    `(not Enabled) or (NodeRef = NIL)` guard exits at `:544` — so a disabled
    element with a live `NodeRef` returns uninitialized memory. (The two
    magnitude reads do guard: capi `:501` / `:633`, r4133 `:711` / `:671`.)

    `SeqCurrents` and `SeqVoltages` are magnitudes only (`Cabs`, r4133 `:719` /
    `:680`), `3 * NTerms` doubles each, so they are captured flat. `SeqPowers`
    is complex and de-interleaved by `_re_im_pair` like `p_kw`/`p_kvar`; its
    unit is **kW/kvar** because both engines apply `* 0.003` inside the arm
    (capi `:561` and `:588-589`, r4133 `:767` and `:788`) — a fixed 3-phase kVA
    conversion, unconditional, and NOT the `PositiveSequence` x3 that `Powers`
    applies at the API boundary.

    The same `derived` flag carries the three GOLDEN_REBASE G1.3c channels,
    also `Enabled`-only. `TotalPowers` (`Alt_CE_Get_TotalPowers`,
    `CAPI/CAPI_Alt.pas:1108`, facade `CAPI/CAPI_CktElement.pas:1043`; r4133
    `CktElementV(20)`, `DDLL/DCktElement.pas:1109`) is the per-terminal sum of
    `GetPhasePower`'s conductor block with the **total** scaled by `0.001` once
    (capi `:1138-1139`, r4133 `:1132`), so it is kW/kvar, `NTerms` complex; it
    runs `GetPhasePower`'s own `ComputeIterminal` (`Common/CktElement.pas:1049`)
    and is therefore group **A** — issued at the head of the element, beside
    `PhaseLosses`, never from this derived block. `CplxSeqCurrents`
    (`:898` / `CktElementV(14)`, `:931`) and `CplxSeqVoltages` (`:872` /
    `CktElementV(13)`, `:885`) are the un-`Cabs`'d output of the very helpers
    `SeqCurrents`/`SeqVoltages` take the modulus of, `3 * NTerms` complex each,
    de-interleaved like `seq_p_kw`/`seq_p_kvar`.

    Enabled-only is again what keeps the two channels' shapes equal rather than
    a crash guard here: capi's extra `NodeRef = NIL` tests (`:1119` on
    `TotalPowers`, `:878` on `CplxSeqVoltages`) return the 1-element
    `DefaultResult` / a 2-double zero where r4133 answers `NTerms` zeros
    (mode 20 has no such guard) or a 1-element `CZero` seeded before its
    `If Enabled` (`:887-888`, `:933-934`) — measured on
    `controls/fuse/midi_fuse.dss`'s never-enabled `Line.tie`
    (capi `TotalPowers = [0.0, 0.0]`, `CplxSeq* = [0.0]`). The one shape gap
    the rule does NOT close is the 0-terminal `UPFCControl` (`nt = 0`, enabled):
    capi answers `CplxSeqCurrents = []` but `CplxSeqVoltages = [0.0]` from that
    `NodeRef` guard, and `TotalPowers = [0.0, 0.0]`; that is the comparator's
    business, as with `SeqVoltages`/`SeqPowers` already.

    `element_extras` (request key `"element_extras"`, manifest flag
    `compare_element_extras`): the discrete index/name scalars (G1.3d(i)), the
    five control-derived scalars and `PhaseLosses` (G1.3d(ii)). Nine of those
    ten are read for EVERY element; only `NodeOrder` is conditional (below).

    The four G1.3d(i) scalars are pure field reads
    (`CAPI/CAPI_CktElement.pas:182-211` for the counts, `:672-687` for
    `EnergyMeter`; r4133 `DDLL/DCktElement.pas:139`/`:144`/`:149`/`:442`).

    The five G1.3d(ii) control-derived scalars — `NumControls`
    (`CAPI/CAPI_CktElement.pas:939`, r4133 `DDLL/DCktElement.pas:237`),
    `OCPDevIndex` (`:951` / `:242`), `OCPDevType` (`:978` / `:259`),
    `HasVoltControl` (`:689` / `:222`) and `HasSwitchControl` (`:713` / `:207`)
    — all answer from the element's `ControlElementList` and never touch
    `Iterminal`, so they are order-free (group C). The two `Has*` reads do move
    that list's cursor (`ModeEffect::Impure`), which is unobservable: every
    consumer restarts the walk with `First`/`Get(i)`.

    `PhaseLosses` (`CAPI/CAPI_CktElement.pas:327` -> `CAPI/CAPI_Alt.pas:449`,
    r4133 `DDLL/DCktElement.pas:637`) is the exception: it is
    `TDSSCktElement.GetPhaseLosses` (capi `Common/CktElement.pas:879`, r4133
    `Common/CktElement.pas:1075`), whose first act on an enabled element is
    `ComputeIterminal` (capi `:896`, r4133 `:1090`) — a cache-aware group-**A**
    read, and therefore the FIRST element read this body issues, ahead of
    `Losses` and of the group-B `Currents` inside `gc.capture_element`. It is
    read for EVERY element, enabled or not: both engines zero-fill a disabled
    one (capi guards `(not FEnabled) or (NodeRef = NIL)` at
    `Common/CktElement.pas:890`, r4133 takes the `Else … CZERO` branch at
    `Common/CktElement.pas:1118-1119`), so — unlike `NodeOrder` — no capture
    predicate is owed. Units are **kW/kvar**, both transports scaling by
    `0.001` (capi `CAPI/CAPI_Alt.pas:464`, r4133 `DDLL/DCktElement.pas:651`) —
    unlike `Losses`, which is W/var.

    `NodeOrder` is read only for an element that is `Enabled` **and** has
    `NumTerminals > 0`:
      * a never-enabled element never got `SetNodeRef`, so `NodeRef` is nil:
        capi raises 15013 (`CAPI/CAPI_CktElement.pas:900-906`) and r4133 dereferences
        the nil pointer at `DDLL/DCktElement.pas:1048` with no guard;
      * a 0-terminal element is legitimate (`UPFCControl` never assigns
        `Nterms` — r4133 `Controls/UPFCControl.pas:230-246`), and there the two
        transports disagree in *shape*: r4133 mode 17 returns a 0-length array
        (`setlength(myIntArray, NTerms*Nconds)`, `:1043`) while capi takes the
        same nil-`NodeRef` branch and raises 15013.
    Not issuing the read removes the asymmetry instead of normalizing it (the
    `derived` precedent); the comparator asserts both sides are empty there, so
    the skip cannot hide a payload. `enabled` is emitted under this flag too —
    the predicate must be visible to the comparator, never assumed.

    `tolerate_user_model` (CF-C Port 2): a Generator model=6 whose user-written
    model is not loaded fires DoSimpleMsg #567 the FIRST time its terminal
    currents are recomputed after a solve; the recompute still produces the
    correct (Yprim-only) currents and clears the error, so a second read returns
    them cleanly (verified: read 1 raises #567 + zeroes Error.Number, read 2 OK).
    We absorb that single priming raise (retry once) exactly as the official
    Direct DLL warns-and-continues; any other errno re-raises. Under the D3
    order that priming raise lands on the first cache-aware read of the element
    rather than on `Powers`: `PhaseLosses` when `element_extras` is on, `Losses`
    otherwise. `_read` wraps each read individually, so either absorber is
    correct — it is the same recompute, and it clears the warning either way.
    """
    def _read(fn):
        # One definition of the priming retry, shared with `capture_aggregates`
        # (the dedup its doc promised at the D7 lane merge, 2026-09-05).
        return _tolerant_read(fn, tolerate_user_model)

    out = []
    el = ckt.ActiveCktElement  # capture-order: ActiveCktElement (C)
    for name in ckt.AllElementNames:  # capture-order: AllElementNames (C)
        ckt.SetActiveElement(name)  # capture-order: SetActiveElement (C)
        enabled = bool(el.Enabled)  # capture-order: Enabled (C)
        pl = None
        if element_extras:
            pl = _read(lambda: el.PhaseLosses)  # capture-order: PhaseLosses (A)
        tp = None
        if derived and enabled:
            tp = _read(lambda: el.TotalPowers)  # capture-order: TotalPowers (A)
        loss = _read(lambda: el.Losses)  # capture-order: Losses (A)
        # capture-order: Powers (A), Currents (B)
        cap = _read(lambda: gc.capture_element(ckt, name))
        cap["loss_w"] = [float(loss[0]), float(loss[1])]
        if derived or element_extras:
            cap["enabled"] = enabled
        if derived and enabled:
            cma = _read(lambda: el.CurrentsMagAng)  # capture-order: CurrentsMagAng (B)
            res = _read(lambda: el.Residuals)  # capture-order: Residuals (B)
            vma = _read(lambda: el.VoltagesMagAng)  # capture-order: VoltagesMagAng (C)
            cap["cma_mag"], cap["cma_ang"] = _polar_pair(cma)
            cap["res_mag"], cap["res_ang"] = _polar_pair(res)
            cap["vma_mag"], cap["vma_ang"] = _polar_pair(vma)
            seq_p = _read(lambda: el.SeqPowers)  # capture-order: SeqPowers (B)
            seq_i = _read(lambda: el.SeqCurrents)  # capture-order: SeqCurrents (B)
            seq_v = _read(lambda: el.SeqVoltages)  # capture-order: SeqVoltages (C)
            cap["seq_p_kw"], cap["seq_p_kvar"] = _re_im_pair(seq_p)
            cap["seq_i"] = [float(x) for x in seq_i]
            cap["seq_v"] = [float(x) for x in seq_v]
            cseq_i = _read(lambda: el.CplxSeqCurrents)  # capture-order: CplxSeqCurrents (B)
            cseq_v = _read(lambda: el.CplxSeqVoltages)  # capture-order: CplxSeqVoltages (C)
            cap["tp_kw"], cap["tp_kvar"] = _re_im_pair(tp)
            cap["cseq_i_re"], cap["cseq_i_im"] = _re_im_pair(cseq_i)
            cap["cseq_v_re"], cap["cseq_v_im"] = _re_im_pair(cseq_v)
        if element_extras:
            n_terms = int(el.NumTerminals)  # capture-order: NumTerminals (C)
            cap["n_terms"] = n_terms
            cap["n_conds"] = int(el.NumConductors)  # capture-order: NumConductors (C)
            cap["n_phases"] = int(el.NumPhases)  # capture-order: NumPhases (C)
            # The RAW oracle spelling: `''` here (capi returns NIL, which
            # dss-python's `_get_string` maps to the empty string) is the "no
            # meter" sentinel, and the r4133 channel spells the same state `'0'`
            # (`CktElementS`'s pre-`case` default, `DDLL/DCktElement.pas:421`).
            # Normalizing the two is the comparator's job, not the capture's.
            cap["energy_meter"] = str(el.EnergyMeter)  # capture-order: EnergyMeter (C)
            cap["pl_kw"], cap["pl_kvar"] = _re_im_pair(pl)
            cap["num_controls"] = int(el.NumControls)  # capture-order: NumControls (C)
            cap["ocp_dev_index"] = int(el.OCPDevIndex)  # capture-order: OCPDevIndex (C)
            cap["ocp_dev_type"] = int(el.OCPDevType)  # capture-order: OCPDevType (C)
            # capture-order: HasVoltControl (C)
            cap["has_volt_control"] = bool(el.HasVoltControl)
            # capture-order: HasSwitchControl (C)
            cap["has_switch_control"] = bool(el.HasSwitchControl)
            if enabled and n_terms > 0:
                order = el.NodeOrder  # capture-order: NodeOrder (C)
                cap["node_order"] = [int(v) for v in order]
        out.append(cap)
    return out


def capture_eventlog(d, ckt) -> list:
    """The cumulative event log (`DSS.EventStrings`), as a list of lines — the
    pinned dss-python `Solution.EventLog` read (BOM-free)."""
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
    growing per-sample arrays — compared per step by `corpus_gate.rs`.
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
        # zone list compares as empty, not as a phantom one-element list. Also
        # drop any empty/whitespace-only entries (a Delphi trailing-separator
        # artifact); an element name is never empty.
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


def capture_reliability(ckt, aborted: bool, message: str) -> dict:
    """The `Meters` reliability surface, read AFTER the executive `RelCalc`
    (`GOLDEN_REBASE_PLAN.md` §1.1, sub-step G1.6(i)).

    `RelCalc` (`Executive/ExecCommands.pas:154` -> `TExecHelper.DoLambdaCalcs`,
    r4133 `Executive/ExecHelper.pas:4404-4440`; capi `ExecCommands.pas:631` ->
    `ExecHelper.pas:4847`) is the only thing that ever fills these fields, and
    **no live corpus deck runs it** — so without driving it the whole
    reliability half of the meter surface would compare `0 == 0`. The gate
    therefore drives it itself, exactly once per case, right after the LAST
    solve: it is NOT idempotent (`Bus.TotalMiles` accumulates —
    `13.825757575757578 -> 22.348484848484844` on
    `modes/time/midi_duty_ctrl.dss`, measured on both oracle channels), so a
    per-step drive would be semantically garbage.

    READ-ORDER CONTRACT — three rules, all asserted statically by
    `crates/dss-core/tests/reliability_pins.rs`:

    1. per meter, the non-section fields in `IMeters._columns` order (the
       fastdss harness's own record order, `dss/IMeters.py:13-42`); `ZonePCE`
       is not in `_columns` and is appended right after `AllBranchesInZone`;
    2. a section field is read ONLY after `SetActiveSection(k)`. The selection
       lives on the METER (`COM_ActiveSection`, capi `CAPI_Meters.pas:729-740`;
       `pMeter.ActiveSection`, r4133 `DDLL/DMeters.pas:254-262`) and the meter
       walk never resets it (capi `Meters_Get_First`/`_Next` `:153-167` ->
       `Generic_CktElement_Get_First`/`_Next`; r4133 `DMeters.pas:38-71`), so a
       section read without a preceding `SetActiveSection` returns the
       previously selected section. With none selected — or an out-of-range
       index — `InvalidActiveSection` (`CAPI_Meters.pas:122-134`) yields 0/0.0
       and, under `DSS_CAPI_EXT_ERRORS`, raises: measured on the pinned oracle,
       both cases raise `(#5055, 'Invalid active section. Has SetActiveSection
       been called?')`, so this rule is a hard requirement here, not just a
       stale-value hazard. The loop below selects every `k in 1..=NumSections`,
       so a meter whose calc aborted (`NumSections == 0`) reads no section
       field at all;
    3. `Meters.Totals` is read LAST, after the `First`/`Next` walk has
       finished. It calls `TotalizeMeters` (`CAPI_Meters.pas:279-290`, `:287`
       -> `Common/Circuit.pas:2347-2360`; r4133 `DMeters.pas:566` ->
       `Circuit.pas:2520-2538`), which walks `EnergyMeters` itself and destroys
       the meter cursor. The fastdss harness says so verbatim
       (`tests/save_outputs.py:332-333`, *"This breaks the iteration"*), and it
       is measured on BOTH channels: on
       `controls/energymeter/midi_energymeter.dss` (meters `em`, `em2`) a
       mid-walk `Totals` read makes the very next `Meters.Next` return 0, so the
       clean walk `['em', 'em2']` silently truncates to `['em']`.

    This surface is group **C** of the §1.1(a) capture-order partition: none of
    its reads calls `GetCurrents` into a scratch buffer — `CalcCurrent` returns
    `Cabs` of the STORED `CalculatedCurrent` array (`CAPI_Meters.pas:335-350`),
    `AllocFactors` a `Move` of `PhsAllocationFactor` (`:379-392`) — so it
    neither imposes anything on the element capture order nor inherits anything
    from it.

    Fastdss parity, both directions, deliberately: we are STRICTLY STRONGER on
    the sections (fastdss captures the first section only,
    `tests/save_outputs.py:284-291`; we capture every one), and we deliberately
    do NOT re-read `SeqListSize` / `CountBranches` / `CountEndElements` (the
    lengths of the three ordered lists compared outright below) or
    `MeteredElement` / `MeteredTerminal` / `Peakcurrent` (EnergyMeter properties
    #0 / #1 / #6 — `EnergyMeter.pas:480-486` — already live-compared by
    `capture_all_properties`). `CountEndElements` is additionally a do-not-call
    on the r4133 DDLL arm, which dereferences `BranchList.ZoneEndsList` with no
    nil guard (`DMeters.pas:156-164`, against capi's `CheckBranchList(5500)` --
    `CAPI_Meters.pas:543`; 5501 is `AllBranchesInZone`, 5502 `AllEndElements`).

    `aborted` / `message` are NOT reads: they carry the outcome of the `RelCalc`
    command itself (errno 52902, `Meters/EnergyMeter.pas:2456` capi == `:2502`
    r4133 == the port's `solution/meters/reliability.rs` text). The error COUNT
    is deliberately not reported — the port raises once per failing meter while
    dss-python raises once per command.
    """
    meters = []
    m = ckt.Meters

    # Same placeholder/artifact filter as `capture_all_meters` (the C-API
    # `['NONE']` DefaultResult and the Delphi trailing-separator empty entry).
    def _lst(v):
        xs = [s for s in (str(s).strip() for s in v) if s]
        return [] if xs == ["NONE"] else xs

    i = m.First
    while i:
        # --- rule 1: reads in `IMeters._columns` order (index in the comment).
        name = str(m.Name)  # _columns[0]
        alloc_factors = [float(x) for x in m.AllocFactors]  # _columns[6]
        ends = _lst(m.AllEndElements)  # _columns[7]
        saifikw = float(m.SAIFIKW)  # _columns[8]
        saidi = float(m.SAIDI)  # _columns[10]
        total_customers = int(m.TotalCustomers)  # _columns[11]
        saifi = float(m.SAIFI)  # _columns[13]
        cust_interrupts = float(m.CustInterrupts)  # _columns[14]
        calc_current = [float(x) for x in m.CalcCurrent]  # _columns[16]
        branches = _lst(m.AllBranchesInZone)  # _columns[17]
        pce = _lst(m.ZonePCE)  # (not in _columns)
        num_sections = int(m.NumSections)  # _columns[18]
        # --- rule 2: still inside the per-meter walk, one selection per section.
        sections = []
        for k in range(1, num_sections + 1):
            m.SetActiveSection(k)
            sections.append(
                {
                    "idx": k,
                    # discrete (compared exactly)
                    "num_section_customers": int(m.NumSectionCustomers),
                    "num_section_branches": int(m.NumSectionBranches),
                    "sect_seq_idx": int(m.SectSeqIdx),
                    "sect_total_cust": int(m.SectTotalCust),
                    "ocp_device_type": int(m.OCPDeviceType),
                    # continuous
                    "sum_branch_flt_rates": float(m.SumBranchFltRates),
                    "avg_repair_time": float(m.AvgRepairTime),
                    "fault_rate_x_repair_hrs": float(m.FaultRateXRepairHrs),
                }
            )
        meters.append(
            {
                "name": name,
                "total_customers": total_customers,
                "saifi": saifi,
                "saifikw": saifikw,
                "saidi": saidi,
                "cust_interrupts": cust_interrupts,
                "calc_current": calc_current,
                "alloc_factors": alloc_factors,
                # ORDERED zone lists — own reads. `capture_all_meters` compares
                # the same three lists as a case-insensitive SET (deliberately,
                # see its comment); the ordered assertion lives on these copies.
                "branches": branches,
                "ends": ends,
                "pce": pce,
                "num_sections": num_sections,
                "sections": sections,
            }
        )
        i = m.Next

    # --- rule 3: LAST, after the walk. `TotalizeMeters` destroys the cursor.
    totals = [float(x) for x in m.Totals]
    # G1.6(ii): the per-bus half of the same post-`RelCalc` surface, nested
    # under this payload's own "buses" key (NOT the checkpoint's top-level
    # `buses`, which is G1.4a's voltage capture). Read through its own function
    # (see there) so rule 3 stays literally true -- the meter handle is never
    # touched again -- and after `totals` so the payload reads
    # meters-then-buses; the bus reads themselves are order-free (group C) and
    # move only `ActiveBusIndex`.
    buses = capture_bus_reliability(ckt)
    return {
        "aborted": bool(aborted),
        "message": str(message),
        "meters": meters,
        "totals": totals,
        "buses": buses,
    }


def capture_bus_reliability(ckt) -> list:
    """Every bus's eight reliability columns, read AFTER the executive `RelCalc`
    (`GOLDEN_REBASE_PLAN.md` §1.1, sub-step G1.6(ii)).

    The parity target is `origin/fastdss` `dss/IBus.py:19-53` `_columns`, which
    the fastdss harness archives for every bus through the iterable
    `dss.ActiveCircuit.ActiveBus` (`tests/save_outputs.py:351`); of its 33
    entries these eight are the reliability half. They are read in that
    `_columns` order — `Cust_Duration` (`:25`), `Cust_Interrupts` (`:26`),
    `Int_Duration` (`:29`), `Lambda` (`:31`), `N_Customers` (`:32`),
    `N_interrupts` (`:33`), `SectionID` (`:34`), `TotalMiles` (`:36`) — with the
    list's DUPLICATE `Cust_Interrupts` entry (`dss/IBus.py:27`) collapsed to a
    single read: reading a pure field twice proves nothing and would make the
    read-order pin ambiguous.

    Every one of the eight is a plain field read off the active `TDSSBus`,
    guarded only by "is a bus active": capi `CAPI_Bus.pas:462-526` (`_activeObj`
    then `Bus_Int_Duration` / `BusFltRate` / `BusCustDurations` /
    `BusCustInterrupts` / `BusTotalNumCustomers` / `Bus_Num_Interrupt`) and
    `:604-622` (`BusTotalMiles`, `BusSectionID`); r4133 serves the same fields
    through `BUSF(6..11)` (`Version8/Source/DDLL/DBus.pas:129-170`) and
    `BUSI(4..5)` (`:60-73`). The port's mirror is `circuit/bus.rs:48-63`.

    Capture-order class: **group C, order-free** (`GOLDEN_REBASE_PLAN.md`
    §1.1(a), coordinator decision D3). No arm calls `ComputeIterminal` or
    `GetCurrents`, none writes engine state, and the only cursor that moves is
    `ActiveBusIndex` — which [`capture_all_buses`], the one later reader of it,
    re-selects per bus anyway. This is a SEPARATE function from
    [`capture_reliability`] on purpose: the read-order pins scan that body for
    its `Meters`-handle reads and require `Totals` to be the last of them
    (`crates/dss-core/tests/reliability_pins.rs`), so the bus block gets its own
    scanned body and its own order test instead of perturbing that rule.

    READ-ORDER CONTRACT: the bus is SELECTED before every read of it, and the
    selection is verified — `SetActiveBus` returns the 0-based `BusList` index
    and a failed lookup leaves the PREVIOUS bus active, which would silently
    attribute one bus's reliability row to another. [`capture_all_buses`] makes
    the same assertion for the same reason. With no bus active the capi arms
    return 0/0.0 (`CAPI_Bus.pas:466`) and the r4133 integer arms -1
    (`DBus.pas:73-75`) — plausible-looking values that must never be captured by
    accident.

    Wire spelling: `Lambda` travels as `lambda_`, because `lambda` is a keyword
    in both Python and Rust; the rename is identical on both sides
    (`BusReliabilityCap`). Do not "fix" it.
    """
    rows = []
    for i, name in enumerate(ckt.AllBusNames):
        idx = ckt.SetActiveBus(name)
        if idx != i:
            raise RuntimeError(
                f"bus reliability capture: SetActiveBus({name!r}) returned {idx}, "
                f"expected {i} (AllBusNames must be the engine's BusList order)"
            )
        b = ckt.ActiveBus
        rows.append(
            {
                "name": str(b.Name),
                # `IBus._columns` order, the duplicate collapsed.
                "cust_duration": float(b.Cust_Duration),
                "cust_interrupts": float(b.Cust_Interrupts),
                "int_duration": float(b.Int_Duration),
                "lambda_": float(b.Lambda),
                "n_customers": int(b.N_Customers),
                "n_interrupts": float(b.N_interrupts),
                "section_id": int(b.SectionID),
                "total_miles": float(b.TotalMiles),
            }
        )
    return rows


def capture_pd_elements(ckt) -> list:
    """Every ENABLED PD element's `PDElements` interface record — the thirteen
    `IPDElements._columns` fields of the fastdss parity target plus the parent's
    full name (`GOLDEN_REBASE_PLAN.md` §1.1 row 10, sub-step G1.6b).

    Membership and order are the circuit's `PDElements` pointer list, walked by
    `PDElements_Get_First`/`_Get_Next` (`CAPI/CAPI_PDElements.pas:129-143` ->
    `Generic_CktElement_Get_First`/`_Next`, `CAPI/CAPI_Utils.pas:721-758`), which
    skip `not Enabled` and assign `ActiveCktElement` from the list. `Fault`
    objects are `FAULTOBJECT + NON_PCPD_ELEM` (`PDElements/Fault.pas:114`) and so
    never appear; the six classes that do are Line / Transformer / AutoTrans /
    Capacitor / Reactor / GICTransformer (`Common/Circuit.pas:2242-2248`).

    READ-ORDER CONTRACT — `ParentPDElement` is read LAST, and nothing but the
    parent-name read may follow it. `PDElements_Get_ParentPDElement`
    (`CAPI/CAPI_PDElements.pas:245-257`; the r4133 DDLL arm
    `Version8/Source/DDLL/DPDELements.pas:88-97` is identical) does
    `ActiveCircuit.ActiveCktElement := elem.ParentPDElement` and never restores
    it, so every field read after it returns the *parent's* value. fastdss reads
    it second (`_columns` order) and so contaminates its own records: measured,
    215 cells of the 138-element IEEE123 walk move — identically on both channels
    (e.g. `Line.l1.Totalcustomers` 1 -> 91, `Line.l2.Numcustomers` 0 -> 1).
    Reading it last makes the mutation free and lets us read the parent's FULL
    name off `ActiveCktElement` (`CktElement_Get_Name` returns `elem.FullName`,
    `CAPI/CAPI_CktElement.pas:172-180`) — the strictly stronger half of the
    comparison (88 distinct values on IEEE123 against the `ClassIndex`'s 85).
    The iteration itself is immune: `Get_Next` advances the pointer list, not
    `ActiveCktElement` (`CAPI/CAPI_Utils.pas:740-758`). Asserted statically by
    `crates/dss-core/tests/pd_elements_pins.rs`.

    `parent_name` is read ONLY when `parent_class_index` is non-zero: with a NIL
    parent the getter leaves the active element alone ("leaves ActiveCktElement
    as is", `CAPI/CAPI_PDElements.pas:251`) and a name read would echo the
    element's own name instead of the empty string.
    """
    out = []
    pde = ckt.PDElements
    i = pde.First
    while i:
        # The twelve order-free fields, in the record order. `IsShunt` is a bool
        # here and 0/1 on the r4133 DDLL — both capture sides emit a bool.
        rec = {
            "name": str(pde.Name),
            "accumulated_l": float(pde.AccumulatedL),
            "from_terminal": int(pde.FromTerminal),
            "is_shunt": bool(pde.IsShunt),
            "num_customers": int(pde.Numcustomers),
            "section_id": int(pde.SectionID),
            "fault_rate": float(pde.FaultRate),
            "repair_time": float(pde.RepairTime),
            "total_miles": float(pde.TotalMiles),
            "total_customers": int(pde.Totalcustomers),
            "pct_permanent": float(pde.pctPermanent),
            "lambda": float(pde.Lambda),
        }
        # LAST field read of the record (the contract above); it moves
        # `ActiveCktElement` to the parent, so the parent's full name is read
        # immediately after it and nothing else may come between.
        rec["parent_class_index"] = int(pde.ParentPDElement)
        rec["parent_name"] = (
            str(ckt.ActiveCktElement.Name) if rec["parent_class_index"] else ""
        )
        out.append(rec)
        i = pde.Next
    return out


#: The six G1.5 short-circuit arms, in the order [`capture_all_buses`] reads
#: them — the same order the r4133 transport uses
#: (`crates/dss-epri/src/capture.rs::capture_all_buses`). Always present in the
#: wire shape, empty when the request did not ask for them.
_SC_KEYS = ("zsc1", "zsc0", "zsc", "ysc", "isc", "voc")

#: What THIS transport publishes for `Bus.ZscMatrix`/`Bus.YscMatrix` when the
#: bus has no short-circuit matrix (`pBus.Zsc = NIL`): `DefaultResult`'s single
#: `0.0` (`CAPI/CAPI_Utils.pas:212-221`, the `DSS_CAPI_COM_DEFAULTS` branch —
#: on in the pinned 0.15.7 build). r4133's arm publishes one `CZero` instead,
#: i.e. 2 doubles (`DDLL/DBus.pas:433-434`); the two sentinels are normalized to
#: "no matrix" by the comparator, never compared as values.
_CAPI_SC_SENTINEL_LEN = 1

#: What the two G1.4c line-to-line arms publish when the bus has at most ONE
#: node: the two-double `[-99999.0, 0.0]` "for 1-phase buses, do not attempt to
#: compute" sentinel (`CAPI/CAPI_Alt.pas:2486-2491` == r4133
#: `DDLL/DBus.pas:594-596`, one `cmplx(-99999.0, 0)`). Both gating channels
#: agree on this one; the comparator recognizes it from the bus's NODE SET, not
#: from the length — a 2-phase bus's single L-L pair is also 2 doubles.
_CAPI_VLL_ONE_PHASE_LEN = 2

#: `DefaultResult`'s length on the pinned build (`CAPI/CAPI_Utils.pas:212-221`
#: with `DSS_CAPI_COM_DEFAULTS` on): one `0.0`. The L-L arms fall back to it
#: when the partner-node scan finds nothing (`CAPI_Alt.pas:2525-2529`) —
#: measured live on `NEVMASTER` `ckt1-1-1`, a `[1, 10]` bus. r4133 has no such
#: branch: its unbounded scan wraps onto the node itself and reports `[0, 0]`.
_CAPI_VLL_DEFAULT_LEN = 1


def capture_all_buses(ckt, want_sc: bool) -> list:
    """Every bus's node set, kV base, the three per-node voltage surfaces and —
    when `want_sc` — the six short-circuit arms.

    GOLDEN_REBASE_PLAN.md WP-G1 G1.4a (the `compare_bus` surface) + G1.5 (the
    `compare_zsc` surface, appended to this same walk). Walked in
    `ckt.AllBusNames` order — the engine's own `BusList` order, which
    `SetActiveBus`'s returned 0-based index re-asserts per bus (a failed lookup
    leaves the previous bus active, which would otherwise silently attribute one
    bus's voltages to another).

    Per bus (`origin/fastdss` `dss/IBus.py:19-53` `_columns` — the parity
    target):

    * `nodes` — `Bus.Nodes`: the bus's node NUMBERS in **ascending** order, not
      the bus's internal insertion order (`CAPI_Alt.pas:2143-2163` == r4133
      `DBus.pas:319-345`; both walk `repeat NodeIdx := FindIdx(jj); inc(jj)
      until NodeIdx > 0` and report `GetNum(NodeIdx)`). Verified live on a
      `.2.1.3`-declared bus: `AllNodeNames` is `b1.2, b1.1, b1.3` while `Nodes`
      is `[1, 2, 3]`.
    * `kv_base` — `Bus.kVBase` in kV. Both engines derive the per-unit divisor
      as `BaseFactor = 1000·kVBase` when positive, else `1.0` — the branch
      11 480 of the corpus's 209 211 buses take.
    * `distance` — `Bus.Distance`, i.e. `TDSSBus.DistFromMeter` in km, published
      verbatim (`CAPI_Bus.pas:419-427` -> `CAPI_Alt.pas:2071-2074` == r4133
      `DBus.pas:122-128`, `BUSF` 5). G1.4b. A zone-build output, not a solve
      output: `MakeMeterZoneLists` writes it (`Meters/EnergyMeter.pas:1833-1838`)
      and a circuit with no EnergyMeter — or a bus no meter's zone reaches —
      reports the untouched `0.0`. There is no "no meter" sentinel on either
      channel, so the all-zero vector is a real assertion about the port.
    * `pu_voltages` — `NodeV[GetRef]/BaseFactor`, `2·NumNodes` interleaved
      (re, im), in that same ascending-node-number order
      (`CAPI_Alt.pas:2251-2280` == r4133 `DBus.pas:399-430`).
    * `vmag_angle` — `2·NumNodes` interleaved (magnitude in V, angle in degrees)
      (`CAPI_Alt.pas:2573-2597` == r4133 `DBus.pas:659-689`).
    * `pu_vmag_angle` — the same pairs with only the magnitude divided by
      `BaseFactor` (`CAPI_Alt.pas:2540-2571` == r4133 `DBus.pas:690-723`).

    All four value surfaces are the identical algorithm on both gating channels
    (re-read at HEAD).

    G1.4c appends FOUR more arms to this same walk — read after the voltage
    surfaces and before the (conditional) short-circuit arms. Unlike everything
    above them these four diverge between the channels *structurally*, by
    construction, which is why the comparator judges them against the port's own
    node set (a positive assertion of each engine's own walk) instead of
    value-for-value (coordinator decision D21):

    * `seq_voltages` — `Bus.SeqVoltages`, `|V012|`, ALWAYS 3 doubles
      (`CAPI/CAPI_Bus.pas:142` -> `CAPI_Alt.pas:2165-2200`). THIS transport
      clamps `Nvalues > 3` to 3 and so answers a real V012 on any bus carrying
      three phase nodes; r4133 (`DDLL/DBus.pas:284-317`) has no clamp and
      answers the `-1.0` x3 "n/A" sentinel whenever `NumNodesThisBus <> 3`.
      Both arms substitute GROUND for a missing phase (`Find(i) = 0` ->
      `NodeV[0]`), so a `[1, 2, 10]` bus is answered here from a fabricated 0 V
      phase C while the port declines (S-SEQ).
    * `cplx_seq_voltages` — `Bus.CplxSeqVoltages`, the same V012 as re/im pairs,
      ALWAYS 6 doubles (`CAPI_Bus.pas:440` -> `CAPI_Alt.pas:2367-2398`,
      `Alt_Bus_Get_ComplexSeqVoltages`): same clamp, sentinel `-1.0` x6
      (r4133 `DBus.pas:520-548`, three `cmplx(-1, -1)` — the same six doubles).
    * `vll` / `pu_vll` — `Bus.VLL` / `Bus.puVLL` (`CAPI_Bus.pas:547` / `:528` ->
      `CAPI_Alt.pas:2473-2537` / `:2400-2470`), the L-L pairs, `2*Nvalues`
      doubles with `Nvalues = min(NumNodesThisBus, 3)` and `2 -> 1`. Three
      shapes, which the LENGTH alone does not name — the node set does:
      `n <= 1` -> the `_CAPI_VLL_ONE_PHASE_LEN` sentinel; no partner node found
      -> `DefaultResult`, `_CAPI_VLL_DEFAULT_LEN`; otherwise 2 doubles (2
      phases) or 6 (3). Both engines probe node `jj` BEFORE the `jj > 3 ->
      jj := 1` wrap, so a 4-node bus's third pair is `V3 - V4`, not `V3 - V1`
      (live: `Test/indmachtest/Master.DSS` `sourcebus`) — the shared upstream
      pairing defect of D21, which r4133's own sibling report path
      (`Common/ShowResults.pas:193-194`) and its own commented-out original
      (`DBus.pas:586-587`) both contradict.
      This transport cannot hang: its partner scan is bounded (`for k := 1 to
      3`, the 2020-03-01 C-API fix quoted at `CAPI_Alt.pas:2512-2513`) and the
      preceding unbounded `repeat` is entered only with `n >= 2` distinct
      positive node numbers, so a node number `>= i` exists for every
      `i <= Nvalues <= 3`. r4133's second loop is an unbounded `repeat`
      (`DBus.pas:580-584` / `:636-640`) and DOES hang (measured TIMEOUT on the
      NEV `double-*` buses), so ITS transport refuses the call per bus;
      `vll_declined` is hard-`False` here and exists only so both transports
      ship one wire shape.
    * `pu_vll` divides by `BaseFactor_LL = 1000*kVBase*sqrt3` when
      `kVBase > 0`, else `1.0` (`CAPI_Alt.pas:2427-2430` == r4133
      `DBus.pas:622-623`) — a different divisor from `pu_voltages`' above.

    G1.5's six short-circuit arms are appended to THIS walk (never a second
    `SetActiveBus` pass), in the fixed order `zsc1, zsc0, zsc, ysc, isc, voc`,
    and only when `want_sc`; the keys are always emitted, empty when it is off,
    so the wire shape does not depend on the request:

    * `zsc1` / `zsc0` — `Bus.Zsc1` = `Zs - Zm` / `Bus.Zsc0` = `Zs + 2*Zm`, one
      complex each, ALWAYS 2 doubles (`CAPI_Alt.pas:2294-2303` / `:2283-2292`
      write `2` unconditionally; the `Zsc = NIL` case is `cZERO` from
      capi `Common/Bus.pas:216-232` == r4133 `Bus.pas:215-229`). `AvgOffDiagonal`
      divides only `If Ntimes > 0` (capi `Shared/Ucmatrix.pas:372-387` ==
      r4133 `:369-383`), so on a 1-node bus `Zm = 0` and
      `Zsc1 == Zsc0 == Zsc[0,0]`.
    * `zsc` / `ysc` — `Bus.ZscMatrix` / `Bus.YscMatrix`, row-major (`i` outer,
      `j` inner) `2*n*n` doubles (`CAPI_Alt.pas:2305-2334` / `:2336-2365` ==
      r4133 `DBus.pas:431-459` / `:491-518`). Both matrices exist only after a
      fault study / `ZscRefresh` built them (`SolveFaultStudy`, capi
      `SolutionAlgs.pas:852-894` == r4133 `SolutionAlgs.pas:875-912`); until
      then this transport publishes its `DefaultResult` SENTINEL of
      `_CAPI_SC_SENTINEL_LEN` double(s) (`CAPI_Utils.pas:212-221` with
      `DSS_CAPI_COM_DEFAULTS` on in the pinned build) while r4133 publishes one
      `CZero`, i.e. 2 — a shape difference the comparator normalizes.
    * `isc` / `voc` — `Bus.Isc` (`BusCurrent`) / `Bus.Voc` (`VBus`), `2*n`
      doubles (`CAPI_Alt.pas:2202-2224` / `:2227-2249`). `VBus`/`BusCurrent`
      are never NIL on THIS transport: `TDSSBus.AllocateBusState`
      (`Common/Bus.pas:250-256`) uses `AllocMem`, which returns a non-nil block
      even at `FNumNodesThisBus = 0`, so the corpus's two 0-node buses report
      `2*0 = 0` doubles here — where r4133's `Reallocmem(VBus, 0)`
      (`Bus.pas:246-260`) frees the pointer and its arm falls back to the
      2-double `CZero` sentinel. Measured on `Test/REACTORTest.DSS` and
      `Test/Source012Test.dss` (`loadbus2`).

    Every SC array is indexed by the bus's INTERNAL (insertion) node index, not
    by ascending node number: `Zsc`/`Ysc` are built column by column over
    the bus's internal index (`pBus.RefNo[i]` in capi `ComputeYsc`,
    `SolutionAlgs.pas:788-816`; `GetRef(i)` in r4133 `:800-832`) and `VBus`/`BusCurrent` are stored per internal
    index — unlike the three voltage surfaces above, whose `FindIdx` walk sorts
    ascending. Measured on a `bus2=b2.2.1.3` deck: the odd `Zsc` diagonal sits
    at index 1 (the node the 1-phase shunt is on), which ascending order would
    have put at index 0.

    G1.4d appends the LAST two arms of this same walk — read after the
    short-circuit block on both transports:

    * `all_pce_at_bus` / `all_pde_at_bus` — `Bus.AllPCEatBus` / `Bus.AllPDEatBus`
      (`CAPI/CAPI_Bus.pas:773-788` / `:790-805`, both with `useNone = False`,
      -> `Common/Circuit.pas:1797-1870` / `:1712-1794` == r4133
      `DDLL/DBus.pas:840-866` / `:867-898` -> `Common/Circuit.pas:1540-1583` /
      `:1493-1536`). The fastdss harness has both in `dss/IBus.py:51-52`
      `_columns` but DROPS them in the Oddie configuration
      (`origin/fastdss` `tests/save_outputs.py:208-209`, under
      `COM_VLL_BROKEN`), so this surface is strictly stronger than the parity
      target.

      Shipped **raw**, and the wire shape here is NOT dss_capi's: the C API
      returns `[]` for an empty answer and appends nothing, while the pinned
      dss-python facade substitutes `['None']` and appends one `''` to a
      non-empty reply — `dss/IBus.py`, `result.append('')` under
      `# TODO: remove this -- added for full compatibility with COM`. r4133's
      DDLL emits its own `'None'` (`DBus.pas:862-863`, `:894-895`) and filters
      the empty trailing slot `getP*atBus` appends (`:853`, `:880`), so the two
      channels' conventions differ by construction. Normalizing here would hide
      exactly the fact the comparator asserts per channel, so this transport
      does not touch the reply.

    Capture-order class: **group C, order-free** (GOLDEN_REBASE_PLAN.md §1.1(a),
    coordinator decision D3). Every read goes straight to `Solution.NodeV`
    (`CAPI_Alt.pas:2276`) and touches neither `ComputeIterminal` nor
    `ActiveCktElement`; only `ActiveBusIndex` moves. That holds for the two
    at-bus arms too on THIS channel: `for elem in DSS_Class` is a
    `TDSSPointerEnumerator` (`Shared/DSSPointerList.pas:17-27`) carrying its own
    index, so no global cursor moves — unlike r4133, whose `DSS_Class.First`/
    `Next` walk is `ModeEffect::Impure` (`dss_epri::modes::BUS_ALL_PCE_AT_BUS`).
    The fixed per-bus read order below is therefore a contract the capture-order
    test asserts, not a staleness hazard.

    Shapes are asserted, never assumed: a `2·len(nodes)` mismatch fails the case
    loudly instead of shipping a short row the comparator would misread as a
    length divergence. That also catches a process running with
    `DSS.AdvancedTypes = True`, where these accessors return complex arrays of
    half the length (the pinned oracle runs with the default `False`). A 0-node
    bus (2 in the corpus: `loadbus2` of `Test/REACTORTest.DSS` and
    `Test/Source012Test.dss`) returns empty arrays and passes the same assert.
    """
    out = []
    for i, name in enumerate(ckt.AllBusNames):
        idx = ckt.SetActiveBus(name)
        if idx != i:
            raise RuntimeError(
                f"bus capture: SetActiveBus({name!r}) returned {idx}, expected {i} "
                "(AllBusNames must be the engine's BusList order)"
            )
        b = ckt.ActiveBus
        nodes = [int(x) for x in b.Nodes]
        cap = {
            "name": str(b.Name),
            "kv_base": float(b.kVBase),
            # G1.4b, read here — with the bus's other scalar attribute, ahead of
            # the value arrays — so both transports share one per-bus read order
            # (`crates/dss-epri/src/capture.rs::capture_all_buses`). Group C:
            # `Alt_Bus_Get_Distance` returns a stored field and touches nothing.
            "distance": float(b.Distance),
            "nodes": nodes,
            "pu_voltages": [float(x) for x in b.puVoltages],
            "vmag_angle": [float(x) for x in b.VMagAngle],
            "pu_vmag_angle": [float(x) for x in b.puVmagAngle],
        }
        for key in ("pu_voltages", "vmag_angle", "pu_vmag_angle"):
            if len(cap[key]) != 2 * len(nodes):
                raise RuntimeError(
                    f"bus capture: {name}.{key} returned {len(cap[key])} values, "
                    f"expected 2*{len(nodes)} for nodes {nodes}"
                )
        cap["seq_voltages"] = [float(x) for x in b.SeqVoltages]
        cap["cplx_seq_voltages"] = [float(x) for x in b.CplxSeqVoltages]
        cap["vll"] = [float(x) for x in b.VLL]
        cap["pu_vll"] = [float(x) for x in b.puVLL]
        # This transport's partner scan is bounded, so it never refuses; the key
        # is emitted so both transports ship one wire shape (see the docstring).
        cap["vll_declined"] = False
        nv = min(len(nodes), 3)
        if nv <= 1:
            want_ll = (_CAPI_VLL_ONE_PHASE_LEN,)
        else:
            want_ll = (_CAPI_VLL_DEFAULT_LEN, 2 if nv == 2 else 6)
        for key, want in (
            ("seq_voltages", (3,)),
            ("cplx_seq_voltages", (6,)),
            ("vll", want_ll),
            ("pu_vll", want_ll),
        ):
            if len(cap[key]) not in want:
                raise RuntimeError(
                    f"bus capture: {name}.{key} returned {len(cap[key])} values, "
                    f"expected one of {want} for nodes {nodes} (see "
                    "capture_all_buses' docstring for each arm's Pascal shape)"
                )
        if len(cap["vll"]) != len(cap["pu_vll"]):
            raise RuntimeError(
                f"bus capture: {name}.vll ({len(cap['vll'])} values) and "
                f"{name}.pu_vll ({len(cap['pu_vll'])}) disagree in length for "
                f"nodes {nodes} — the two arms walk the identical loop over the "
                "identical node set, so they cannot"
            )
        cap.update({k: [] for k in _SC_KEYS})
        if want_sc:
            cap["zsc1"] = [float(x) for x in b.Zsc1]
            cap["zsc0"] = [float(x) for x in b.Zsc0]
            cap["zsc"] = [float(x) for x in b.ZscMatrix]
            cap["ysc"] = [float(x) for x in b.YscMatrix]
            cap["isc"] = [float(x) for x in b.Isc]
            cap["voc"] = [float(x) for x in b.Voc]
            n = len(nodes)
            for key, want in (
                ("zsc1", (2,)),
                ("zsc0", (2,)),
                ("zsc", (_CAPI_SC_SENTINEL_LEN, 2 * n * n)),
                ("ysc", (_CAPI_SC_SENTINEL_LEN, 2 * n * n)),
                ("isc", (2 * n,)),
                ("voc", (2 * n,)),
            ):
                if len(cap[key]) not in want:
                    raise RuntimeError(
                        f"bus capture: {name}.{key} returned {len(cap[key])} values, "
                        f"expected one of {want} for nodes {nodes} (see "
                        "capture_all_buses' docstring for each arm's Pascal shape)"
                    )
        # G1.4d — the two at-bus lists, read LAST in the per-bus walk on BOTH
        # transports (`crates/dss-epri/src/capture.rs::capture_all_buses` reads
        # them in the same slot, in the same order). Shipped RAW: the trailing
        # `''` and the `['None']` substitution are the pinned dss-python
        # facade's, not dss_capi's, and the comparator asserts that convention
        # rather than letting a transport quietly erase it (see the docstring).
        cap["all_pce_at_bus"] = [str(x) for x in b.AllPCEatBus]
        cap["all_pde_at_bus"] = [str(x) for x in b.AllPDEatBus]
        out.append(cap)
    return out


def capture_all_bus_vmag_pu(ckt) -> list:
    """`Circuit.AllBusVmagPu` — every NODE's per-unit voltage magnitude.

    Ordered bus-list order x the bus's INTERNAL node index (`GetRef(j)` for
    `j = 1..NumNodesThisBus`, i.e. the `AllNodeNames` order) — a different
    permutation from the ascending-node-number order of the per-bus arrays in
    [`capture_all_buses`] AND from the gated `YNodeOrder`
    (`CAPI_Circuit.pas:521-548` == r4133 `Circuit.AllBusMagPu`,
    `DCircuit.pas:481-500`, which share the `BaseFactor` rule). Read once per
    checkpoint; order-free (group C).
    """
    return [float(x) for x in ckt.AllBusVmagPu]


def capture_all_bus_distances(ckt) -> list:
    """`Circuit.AllBusDistances` — each bus's `DistFromMeter` (km), BusList order.

    GOLDEN_REBASE_PLAN.md WP-G1 G1.4b. `for i := 0 to NumBuses-1 do Result[i] :=
    Buses[i+1].DistFromMeter` (`CAPI_Circuit.pas:671-688` == r4133
    `DCircuit.pas:566-580`, `CircuitV` 12) — capi's own comment: *"in an array
    that aligns with the buslist"*, i.e. the same sequence
    [`capture_all_buses`] walks. Length = `NumBuses`. Order-free (group C).
    """
    return [float(x) for x in ckt.AllBusDistances]


def capture_all_node_distances(ckt) -> list:
    """`Circuit.AllNodeDistances` — the owning bus's `DistFromMeter` per node.

    GOLDEN_REBASE_PLAN.md WP-G1 G1.4b. Walked bus x the bus's INTERNAL node
    index (`for i := 1 to NumBuses do for j := 1 to NumNodesThisBus`,
    `CAPI_Circuit.pas:697-722` == r4133 `DCircuit.pas:582-604`, `CircuitV` 13) —
    the `AllNodeNames` permutation, which capi's own comment names (*"Array
    sequence is same as all bus Vmag and Vmagpu"*): the same order as
    [`capture_all_bus_vmag_pu`], NOT the ascending-node-number order of the
    per-bus arrays. Length = `NumNodes`. Order-free (group C).
    """
    return [float(x) for x in ckt.AllNodeDistances]


def _topo_names(v) -> list:
    """Normalize one `ITopology` string array to its comparable shape.

    Exactly two transport-side normalizations, both measured over the whole
    corpus (GOLDEN_REBASE_PLAN.md §G1.7):

    * the **empty sentinel** — an empty array comes back as the single entry
      `NONE`: `DefaultResult(..., 'NONE')` on the capi channel
      (`.inputs/dss_capi/src/CAPI/CAPI_Topology.pas:139-142`, `:196-199`,
      `:392-395` via `CAPI_Utils.pas:115`), and the same word on r4133, whose
      `TStr` is pre-seeded `'NONE'` (`DDLL/DTopology.pas:274-275`). Mapped to
      `[]`, so an empty list compares as empty and not as a phantom
      one-element list (the `capture_all_meters` precedent above).
    * **one trailing empty entry** — capi grows its array with
      `SetLength(Result, k + 1)` after each hit and then copies
      `Length(Result)` entries, so a NON-EMPTY `AllIsolatedBranches` /
      `AllIsolatedLoads` carries exactly one trailing `''`
      (`CAPI_Topology.pas:127-134` + `:145-149`; `:379-387` + `:399-403`),
      while r4133 filters empties (`DTopology.pas:341-347`, `if TStr[i] <> ''`).
      `AllLoopedPairs` starts at `k := -1` on both channels and lands exactly on
      `2 * npairs`, so it carries NO trailing slot (`CAPI_Topology.pas:164`,
      `:198-203`; `DTopology.pas:276`) — the asymmetry is real, and exactly ONE
      trailing `''` is ever dropped.

    Anything else empty — an interior entry, a second trailing one, a
    whitespace-only name — is a shape change and RAISES: this normalization
    must never quietly swallow a missing element name.
    """
    xs = [str(s) for s in v]
    if xs == ["NONE"]:
        return []
    if xs and xs[-1] == "":
        xs = xs[:-1]
    blank = [i for i, s in enumerate(xs) if not s.strip()]
    if blank:
        raise ValueError(
            f"topology name array has unexpected empty entries at {blank}: {xs!r} "
            "(only ONE trailing '' — the capi SetLength artifact — is dropped)"
        )
    return xs


def capture_topology(ckt) -> dict:
    """The six order-free `ITopology` quantities of GOLDEN_REBASE G1.7.

    Surface: `dss/ITopology.py:41/50/59/157/166/175` on `origin/fastdss`
    (`.inputs/DSS-Python`). Engine side: `NumLoops` walks the topology tree and
    halves the `IsLoopedHere` count (capi
    `.inputs/dss_capi/src/CAPI/CAPI_Topology.pas:81-97`; r4133
    `Version8/Source/DDLL/DTopology.pas:67-77`), `NumIsolatedBranches` /
    `NumIsolatedLoads` count `IsIsolated` over `PDElements` / `PCElements`
    (capi `:302-334`, `:448-480`; r4133 `:79-88`, `:89-98`), and the three
    lists emit those same elements' `FullName` (capi `:114-151`, `:160-215`,
    `:369-405`) / `QualifiedName` (r4133 `:271-321`, `:322-356`, `:357-391`) —
    measured byte-identical on all 336 both-gated corpus cases.

    The other three fields of the class's nine `_columns` (`ITopology.py:10-20`)
    — `ActiveLevel`, `BranchName`, `ActiveBranch` — are deliberately NOT read,
    and neither is any cursor mode (`First`, `Next`, `ForwardBranch`,
    `BackwardBranch`, `LoopedBranch`, `ParallelBranch`, `FirstLoad`, `NextLoad`,
    `BusName`): every one of them reassigns `ActiveCircuit.ActiveCktElement`
    (capi `CAPI_Topology.pas:98-110`; r4133 `DTopology.pas:28-40` and `:42-53`,
    reached by modes 3-12, plus `:187-233`) and would poison the per-element
    capture. The omission is asserted by
    `crates/dss-core/tests/capture_order.rs`, not just by this comment.

    The FIRST of these six reads is what BUILDS the memoized `Branch_List` and
    rewrites `Checked` / `IsIsolated` / `BusChecked` on every element (r4133
    `Common/Circuit.pas:2932-2950`) — which is why the call site is strictly
    last in the per-step capture, the `all_properties` argument made
    independent of every future addition.
    """
    t = ckt.Topology
    return {
        "num_loops": int(t.NumLoops),
        "num_isolated_branches": int(t.NumIsolatedBranches),
        "num_isolated_loads": int(t.NumIsolatedLoads),
        "looped_pairs": _topo_names(t.AllLoopedPairs),
        "isolated_branches": _topo_names(t.AllIsolatedBranches),
        "isolated_loads": _topo_names(t.AllIsolatedLoads),
    }


def _inc_ints(v, what: str) -> list:
    """Normalize one capi incidence / Laplacian integer array (G1.8 rule N1).

    `Solution_Get_IncMatrix` / `Solution_Get_Laplacian` allocate `NZero * 3 + 1`
    integers and fill only the first `NZero * 3`
    (`.inputs/dss_capi/src/CAPI/CAPI_Solution.pas:910` and `:873`, both carrying
    the upstream `//TODO: remove the +1`), so the wire always carries exactly ONE
    trailing cell that is not part of the triple stream; a NIL matrix answers
    `DefaultResult(ResultPtr, ResultCount)`, which is the same single zero
    (`CAPI_Utils.pas:201-210`). r4133's DDLL has no such slot — it returns
    `NZero * 3`, or the one-element `[0]` when the matrix is NIL
    (`Version8/Source/DDLL/DSolution.pas:542-568`, `:640-667`) — so dropping the
    cell HERE, in the transport, is what makes the two channels byte-identical
    (measured: 358 both-gated cases, 0 disagreements on all four quantities).

    The pinned dss-python (0.15.7, `dss/ISolution.py:616-631` / `:651-668`) hands
    the C array through untouched. Its `origin/fastdss` successor appends a COM
    compatibility zero when `len % 3 == 0` (`dss/ISolution.py:609-631`,
    `:652-673`) — inert against a backend that already allocates the `+1`, and the
    reason this rule is written as "drop exactly one trailing zero" rather than
    "drop the last cell".

    The shape is this sub-step's KILL CRITERION (GOLDEN_REBASE_PLAN.md §G1.8), so
    anything else RAISES: a length that is not `3 * k + 1`, or a trailing cell
    that is not 0. Measured over the whole live capi population, both arrays:
    1733 / 1733 steps `len % 3 == 1` with the trailing cell 0.
    """
    xs = [int(x) for x in v]
    if len(xs) % 3 != 1:
        raise ValueError(
            f"capi {what}: length {len(xs)} is not `3*NZero + 1` — the trailing-cell "
            f"contract of CAPI_Solution.pas:873 / :910 changed (head {xs[:6]!r})"
        )
    if xs[-1] != 0:
        raise ValueError(
            f"capi {what}: the trailing cell is {xs[-1]}, not 0 — it is NOT the "
            f"unwritten `+1` slot and must not be dropped (length {len(xs)})"
        )
    return xs[:-1]


def _inc_names(v, sentinel_ok: bool, what: str) -> list:
    """Normalize one capi incidence row / column name array (G1.8 rule N3).

    capi answers an absent list with `DefaultResult(..., '')` — a ONE-element
    array holding the empty string, not an empty array (`CAPI_Solution.pas:961`
    for `Inc_Mat_Rows = NIL`; `:988`, `:995` and `:1010` for the three
    `IncMatrixCols` exits; `CAPI_Utils.pas:234-243`). r4133 writes the word
    `None` in the same places (`DDLL/DSolution.pas:605`, `:636`). Both mean "no
    names", and both map to `[]`.

    The sentinel is accepted ONLY where the engine can actually reach it —
    `IncMatrixRows` when the incidence matrix carries no triple, `IncMatrixCols`
    when the circuit has no buses. A one-element `''` anywhere else, or any blank
    entry inside a real list, is a shape change and RAISES: this normalization
    must never quietly swallow a missing row or column name (the `_topo_names`
    precedent above). Measured: 104 of 1733 live capi steps take the rows
    sentinel, 0 take the cols sentinel (`IncMatrixCols` was `AllBusNames` on
    1733 / 1733 steps).

    Case is left alone: the comparator matches each entry case-insensitively,
    because the port's bus names are `HashList`-lowercased while its row names
    are `Class.name` with a capitalized class.
    """
    xs = [str(s) for s in v]
    if len(xs) == 1 and xs[0].strip() == "":
        if not sentinel_ok:
            raise ValueError(
                f"capi {what}: the empty sentinel {xs!r} came back where the engine "
                "cannot reach it (a non-empty incidence matrix / a circuit with buses)"
            )
        return []
    blank = [i for i, s in enumerate(xs) if not s.strip()]
    if blank:
        raise ValueError(
            f"capi {what}: unexpected empty entries at {blank}: {xs!r} "
            "(only the one-element empty sentinel is ever dropped)"
        )
    return xs


def capture_inc_matrix(d, ckt) -> dict:
    """The flat incidence-matrix / Laplacian surface of GOLDEN_REBASE G1.8.

    Issues the executive pair `CalcIncMatrix` (ordinal 108) then `CalcLaplacian`
    (111) — `.inputs/dss_capi/src/Executive/ExecCommands.pas:406-409` and
    `:421-433`; r4133 `Version8/Source/Executive/ExecCommands.pas:911-917` — and
    reads the four flat quantities back in the order below:
    `Solution.IncMatrix`, `Solution.Laplacian`, `Solution.IncMatrixRows`,
    `Solution.IncMatrixCols` (`dss/ISolution.py:609` / `:652` / `:643` / `:634`
    on `origin/fastdss`; `:618` / `:653` / `:644` / `:635` in the pinned 0.15.7).

    NEVER the ordered builder `CalcIncMatrix_O` (109) and never
    `Solution.BusLevels`: the first calls `GetTopology`, which builds the very
    tree G1.7's census is defined on, and the second walks one element past its
    own array on r4133 (`DDLL/DSolution.pas:578-582`; it sits on the bridge's
    `DO_NOT_CALL` register). Both omissions are asserted by
    `crates/dss-core/tests/capture_order.rs`, not only by this comment.

    **Read strictly last in the step — after `all_properties` AND after
    `topology`.** Two reasons:

    * the pair is not `ActiveCktElement`-neutral on the r4133 channel:
      `AddSeriesReac2IncMatrix` re-points `LastClassReferenced` /
      `ActiveDSSClass` and then calls `ActiveDSSClass.First`
      (r4133 `Common/Solution.pas:3007-3010`), which reassigns the active
      element. The pinned capi walks the same reactors with a typed class
      iterator and leaves the active element alone
      (`.inputs/dss_capi/src/Common/Solution.pas:1464-1473`), but both transports
      capture in the same order by construction, so the stronger channel sets the
      rule for both;
    * `Calc_Inc_Matrix` is a solution-state write — it recreates or resets
      `IncMat`, refills `Inc_Mat_Rows` and clears `IncMat_Ordered`
      (`Common/Solution.pas:1507-1526`) — and it must follow the topology read,
      which is the one that builds and memoizes `Branch_List` (see
      `capture_topology`).

    `IncMatrixCols` is therefore always read after a FLAT build, where
    `IncMat_Ordered` is false and both engines answer every bus in `BusList`
    order instead of `Inc_Mat_Cols` (capi `CAPI_Solution.pas:991` + `:1008-1018`;
    r4133 `DSolution.pas:616` + `:627-629`) — measured equal to `AllBusNames` on
    1733 / 1733 live capi steps.

    The two transport normalizations are `_inc_ints` (N1) and `_inc_names` (N3)
    above; nothing is normalized in the comparator. The returned dict is the
    shape the r4133 bridge returns and the Rust side deserializes: four keys,
    with the two integer arrays FLAT (`row, col, value` triples in insertion
    order) and already normalized.
    """
    sol = ckt.Solution
    d.Text.Command = "CalcIncMatrix"
    d.Text.Command = "CalcLaplacian"
    inc = _inc_ints(sol.IncMatrix, "IncMatrix")
    lap = _inc_ints(sol.Laplacian, "Laplacian")
    return {
        "inc_matrix": inc,
        "laplacian": lap,
        "rows": _inc_names(sol.IncMatrixRows, not inc, "IncMatrixRows"),
        "cols": _inc_names(sol.IncMatrixCols, int(ckt.NumBuses) == 0, "IncMatrixCols"),
    }



# OpenDSS `Show`/`Export`/`Save` write report files into the compiled case's
# directory (`OutputDirectory := DataDirectory := <case dir>` in
# `DSSGlobals.SetDataPath`, which `Compile` calls). The live gate only compares
# the in-memory model, so those files are pure pollution of the vendored corpus.
# Setting `DataPath` before `Compile` does NOT help — `Compile` resets it to the
# case dir. So snapshot the case dir and restore it after each run instead.
# Lifted move-only into corpus_guard.py (2026-07-07) so any consumer shares the
# identical, empirically-hardened implementation.
from corpus_guard import CorpusGuard as _CorpusGuard  # noqa: E402
from corpus_guard import copy_selected_contents as _copy_selected_contents  # noqa: E402


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

# G1.6(i). A SEPARATE, deliberately narrow scope: the ONE DoSimpleMsg number the
# executive `RelCalc` raises as its own by-design abort, tolerated ONLY around
# that command and never at compile or solve (hence not folded into
# `_TOLERATED_COMPILE_ERRNOS` above).
#   52902 — "Error: No Overcurrent Protection device (Relay, Recloser, or Fuse)
#           defined. Aborting Reliability calc." — raised per meter whose zone
#           carries no OCP device (`Meters/EnergyMeter.pas:2456` capi ==
#           r4133 `:2502`). The reliability calc then leaves that meter at
#           `SectionCount = 0`; the Rust engine reports the identical text on
#           `Dss::errors()` (`solution/meters/reliability.rs`) and the r4133
#           bridge tolerates the same single number (`crates/dss-epri`), so the
#           three engines' abort is compared, not masked (`aborted`/`message`
#           of `capture_reliability`).
# Any other errno out of `RelCalc` re-raises — a real failure is never swallowed.
_RELCALC_TOLERATED_ERRNOS = {52902}


def _set_early_abort(d, val) -> bool:
    """Best-effort `DSS.Error.EarlyAbort = val`. Returns True on success; a
    failure is harmless (the engine warns-and-continues regardless)."""
    try:
        d.Error.EarlyAbort = val
        return True
    except Exception:
        return False


def _get_early_abort(d):
    """Current `DSS.Error.EarlyAbort`, or None if the engine does not expose it —
    in which case there is nothing to save/restore."""
    try:
        return bool(d.Error.EarlyAbort)
    except Exception:
        return None


def run_case(d, req: dict) -> dict:
    """Compile one copied `.dss` case, run `n_steps` solves, return the full
    per-step model (the shape `harness::*` / corpus_gate.rs deserialize)."""
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
    # GOLDEN_REBASE G1.3a (manifest flag `compare_derived`): the per-element
    # polar channels `CurrentsMagAng` / `VoltagesMagAng` / `Residuals` plus
    # `Enabled`. Opt-in because the three extra reads roughly double the
    # per-element payload; the same request key reaches the r4133 worker
    # unchanged (`corpus_gate::engines::build_run_request`).
    want_derived = bool(req.get("derived", False))
    # G1.4a bus surface (`compare_bus`): the per-bus voltage arrays plus the
    # circuit-level `AllBusVmagPu`. Opt-in — cheap (41 ms for the 4 876-bus
    # 8500-Node deck) but it doubles a large deck's JSON payload.
    want_buses = bool(req.get("buses", False))
    # GOLDEN_REBASE G1.3d(i) (manifest flag `compare_element_extras`): the
    # per-element discrete index/name scalars `NumTerminals`/`NumConductors`/
    # `NumPhases`/`EnergyMeter`/`NodeOrder` plus `Enabled`. Same one-key-for-the
    # -whole-surface shape as `derived`, honored by both transports.
    want_element_extras = bool(req.get("element_extras", False))
    # GOLDEN_REBASE G1.7 (`compare_topology`): the six order-free `ITopology`
    # reads, captured strictly LAST in the step (see `capture_topology`).
    want_topology = bool(req.get("topology", False))
    # G1.5 bus short-circuit surface (`compare_zsc`): the six `Zsc1`/`Zsc0`/
    # `ZscMatrix`/`YscMatrix`/`Isc`/`Voc` arms, appended to the SAME per-bus walk
    # `buses` drives — so `zsc` without `buses` would silently ship nothing.
    # The gate expresses the implication in the request builder
    # (`corpus_gate::engines::build_run_request`); this transport refuses the
    # malformed request loudly rather than returning an empty surface.
    want_zsc = bool(req.get("zsc", False))
    if want_zsc and not want_buses:
        raise RuntimeError(
            "request asks for the bus short-circuit surface (zsc) without the "
            "bus surface it is appended to — the six SC arms share the one "
            "per-bus walk (GOLDEN_REBASE_PLAN.md WP-G1 G1.5 §2.a)"
        )
    # GOLDEN_REBASE G1.8 (`compare_inc_matrix`): the flat `CalcIncMatrix` +
    # `CalcLaplacian` pair and its four reads, captured after `topology`, i.e.
    # strictly last of all (see `capture_inc_matrix`).
    want_inc_matrix = bool(req.get("inc_matrix", False))
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
    # G1.6b: the per-PD-element `PDElements` interface walk. Opt-in (the Rust
    # scheduler forces it on every live non-`large` case); when off the checkpoint
    # carries `None`, not `[]`, so the gate can tell "not requested" apart from
    # "requested, and this circuit simply has no PD element" (96 of the 372 walked
    # live cases have none) — `harness::capture_guard::require_capture_opt`.
    want_pde = bool(req.get("pd_elements", False))
    # G1.6(i): the `Meters` reliability surface (manifest flag
    # `compare_reliability`). The ONLY request flag that DRIVES an executive
    # command — `RelCalc`, once, after the last solve — because no live corpus
    # deck runs it and the whole surface would otherwise compare `0 == 0`. Off
    # -> every checkpoint carries `None`; on -> only the LAST one carries the
    # payload (`harness::capture_guard::require_capture_opt`).
    want_rel = bool(req.get("reliability", False))
    # G1.10a: the run-produced FILE SET under the case's DataPath
    # (`OutputDirectory := DataDirectory := <case dir>`, set by `Compile` ->
    # `DSSGlobals.SetDataPath`). Not a model read at all: it is the run's
    # filesystem effect, classified by the SAME `_CorpusGuard` pass that sweeps
    # the corpus clean (`corpus_guard.CorpusGuard.created`), so the reported set
    # can never disagree with the swept set. Read STRICTLY LAST of the whole run
    # — after every step's model read and after `autoadd_log`, which reads a file
    # off disk — and inside the guard scope, because the guard's exit deletes
    # exactly those files. Off -> the response carries `None`, not `[]`, so
    # `harness::capture_guard::require_capture_opt` can tell "not requested"
    # apart from "requested, and this deck creates nothing" (most decks do not).
    want_run_files = bool(req.get("run_files", False))
    # G1.10b: the CONTENTS half. The gate's selection of created-file names
    # whose BYTES this transport hands back, as patterns over a normalized
    # member (`corpus_guard.selects_contents`); the list is declared ONCE on
    # the gate side (`crates/dss-epri/src/guard.rs::RUN_FILE_CONTENTS_PATTERNS`)
    # and travels in the request, so neither transport re-derives it. The bytes
    # go through the gate-owned sidecar directory of decision D40(6) (never the
    # case dir, never `tests/corpus/`) because the selection reaches ~19 MB per
    # channel per drive and would otherwise cross the worker pipes inline. Empty
    # -> the reply omits the key entirely (`None`), which is what lets the gate's
    # presence rail tell that apart from `[]` ("asked, and this deck wrote none
    # of the selected reports").
    contents_patterns = list(req.get("run_file_contents", []) or [])
    contents_dir = req.get("run_file_contents_dir") or ""
    # CF-C Port 2 user-model decks: tolerate the `_USER_MODEL_ERRNOS` at compile
    # AND at every solve (EarlyAbort is turned off around this call in main()).
    warn_and_continue = bool(req.get("warn_and_continue", False))
    tolerated_compile = _TOLERATED_COMPILE_ERRNOS | (
        _USER_MODEL_ERRNOS if warn_and_continue else set()
    )

    with _CorpusGuard(case_path) as guard:
        for attempt in range(1, _RUN_ATTEMPTS + 1):
            node_order = None
            checkpoints = []
            d.Text.Command = "clear"
            # D13: `clear` does NOT reset `DefaultBaseFreq` on this channel
            # either. dss_capi keeps it per `TDSSContext`, initialized once from
            # `GlobalDefaultBaseFreq` = 60.0 (`Common/DSSClass.pas:1278`,
            # `Common/DSSGlobals.pas:143`); `TExecutive.Clear`
            # (`Executive/Executive.pas:268-318`) never touches it, and the only
            # assignments are `Set DefaultBaseFrequency`
            # (`Executive/ExecOptions.pas:257` with no circuit, `:611` with one)
            # and the JSON circuit loader (`CAPI/CAPI_Obj.pas:2919`). This server
            # is long-lived, so a 50 Hz deck would otherwise leak its base
            # frequency into every later deck of the sweep, while the Rust engine
            # starts each case at 60 Hz (`crates/dss-core/src/exec/construct.rs:173`).
            # Restore that starting point before the compile; a deck that wants
            # 50 Hz still sets it itself. (No registry counterpart here: unlike
            # r4133, dss_capi has none and rejects `Set RegistryUpdate`
            # outright — `Executive/ExecOptions.pas:259-260`, error 302.)
            d.Text.Command = "Set DefaultBaseFrequency=60"
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
            for step in range(n_steps):
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
                # G1.6(i): drive the executive `RelCalc` ONCE, on the LAST step
                # only, HERE — after `Text.Result` has been read (`RelCalc`
                # overwrites it with its own reply) and before every capture of
                # this checkpoint, so the reliability payload and the fields it
                # feeds (`PDElements.{AccumulatedL,Lambda,TotalMiles,SectionID}`,
                # `Bus.*`, EnergyMeter properties #19-23) are read post-calc on
                # all three engines at the same point. Never per step: the calc
                # is not idempotent (see `capture_reliability`).
                rel_aborted = False
                rel_message = ""
                if want_rel and step == n_steps - 1:
                    # The exception IS the detection, on every case: dss-python
                    # raises whenever the error pointer is set and exceptions
                    # are enabled (`dss/_cffi_api_util.py`, `using_exceptions`),
                    # and `DoSimpleMsg` sets `DSS.ErrorNumber` unconditionally
                    # -- `DSS_CAPI_EARLY_ABORT` only decides `Redirect_Abort`
                    # (`Common/DSSGlobals.pas:259-265`). So a case that opted
                    # into `warn_and_continue` (EarlyAbort False) still lands
                    # here; nothing can take the 52902 silently. (G1.6(i) audit
                    # settlement, finding AT-9: measured claim, not an
                    # assumption -- the whole `_USER_MODEL_ERRNOS` block above
                    # exists for the same reason.)
                    try:
                        d.Text.Command = "RelCalc"
                    except _dss.DSSException as e:
                        # Only 52902, the by-design "no OCP device in the zone"
                        # abort; anything else is a real failure and re-raises.
                        errno = e.args[0] if e.args else None
                        if errno not in _RELCALC_TOLERATED_ERRNOS:
                            raise
                        rel_aborted = True
                        rel_message = str(e.args[1]) if len(e.args) > 1 else ""
                        log(
                            f"oracle: tolerated RelCalc abort #{errno} on {case_path}"
                        )
                # G1.9 (GOLDEN_REBASE_PLAN.md, §1.1(a) + decision D3) — the
                # circuit aggregates and the solution scalars, read HERE and
                # nowhere later, for two independent reasons:
                #  * group A before group B. Every aggregate is a
                #    `Get_Losses`/`Get_Power` read, i.e. a `ComputeIterminal`
                #    over the elements it walks, while `capture_all_elements`
                #    below issues Powers *then* `Currents` per element — and
                #    `Currents` is the read that fills a scratch buffer. Group A
                #    therefore runs first, ahead of every group-B read.
                #  * cursor hygiene. `Losses` walks PDElements, `LineLosses`
                #    walks Lines, `SubstationLosses` walks Transformers,
                #    `TotalPower` walks Sources and `AllElementLosses` walks
                #    CktElements (r4133 `DDLL/DCircuit.pas:294/313/335/356/468`),
                #    each leaving that `TPointerList` cursor at the end — and
                #    `gc.capture_discrete` below drives `Transformers.First/Next`.
                #    Reading before any First/Next walk removes the interaction
                #    by construction.
                # `global_result` above still comes first: it reads `Text.Result`,
                # which any later `?` query would overwrite, and G1.6(i)'s
                # once-per-case `RelCalc` sits between the two — a
                # state-changing command, so it precedes every read of this
                # checkpoint (both transports place it identically).
                # The source order is asserted by
                # `crates/dss-core/tests/capture_order.rs`.
                aggregates = capture_aggregates(ckt, warn_and_continue)
                solution_scalars = capture_solution_scalars(ckt.Solution)
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
                        "elements": capture_all_elements(
                            ckt, warn_and_continue, want_derived, want_element_extras
                        ),
                        "injection": gc.capture_injection(d),
                        "transformers": disc["transformers"],
                        "regcontrols": disc["regcontrols"],
                        "capacitors": disc["capacitors"],
                        "monitors": capture_all_monitors(ckt) if check_mm else [],
                        "meters": capture_all_meters(ckt) if check_mm else [],
                        # G1.6(i): immediately AFTER the meters — the two meter
                        # walks stay adjacent — and BEFORE `pd_elements`, whose
                        # `ParentPDElement` hijack must remain the checkpoint's
                        # last active-element mutation. `Meters.Totals` (the
                        # last read inside) destroys the meter cursor, so it
                        # must not precede `capture_all_meters`. `None` on every
                        # step but the last, and whenever the flag is off, so
                        # the gate can tell "not requested / not this step" from
                        # "requested, and this circuit simply has no meter";
                        # `crates/dss-epri`'s `run_case` uses the identical slot.
                        "reliability": (
                            capture_reliability(ckt, rel_aborted, rel_message)
                            if want_rel and step == n_steps - 1
                            else None
                        ),
                        # G1.6b: AFTER the meters, BEFORE the probes. The walk
                        # mutates the active element (`ParentPDElement`), and
                        # every reader below re-selects its own element
                        # (`? el.prop` / `SetActiveElement`), so this is the one
                        # slot where it perturbs nothing; `crates/dss-epri`'s
                        # `run_case` uses the identical slot.
                        "pd_elements": capture_pd_elements(ckt) if want_pde else None,
                        "probes": capture_probes(d, probes),
                        "variables": capture_variables(ckt, variables),
                        "eventlog": (capture_eventlog(d, ckt) if want_eventlog else []),
                        "ctrlqueue": capture_ctrlqueue(ckt) if want_ctrlqueue else [],
                        # G1.4a + G1.5: order-free (group C) bus reads — they move
                        # only `ActiveBusIndex`, so their slot is free; kept here,
                        # ahead of the property sweep, so the `?` queries below stay
                        # the last reads of the step.
                        "buses": (
                            capture_all_buses(ckt, want_zsc) if want_buses else []
                        ),
                        "all_bus_vmag_pu": (
                            capture_all_bus_vmag_pu(ckt) if want_buses else []
                        ),
                        # G1.4b: the two circuit-level distance arrays, read in
                        # the same order-free slot and behind the same
                        # `compare_bus` flag as the per-bus `distance` above —
                        # one surface, three views of `DistFromMeter`.
                        "all_bus_distances": (
                            capture_all_bus_distances(ckt) if want_buses else []
                        ),
                        "all_node_distances": (
                            capture_all_node_distances(ckt) if want_buses else []
                        ),
                        # WP8.5b: read after every established capture above, so
                        # the property sweep's `?` queries never perturb any
                        # other read's active-element state. Only G1.7's
                        # topology read (below) comes later, for the same
                        # reason applied to itself.
                        "all_properties": (
                            capture_all_properties(d, ckt) if want_all_props else []
                        ),
                        # G1.7: read STRICTLY LAST, after `all_properties` —
                        # the first `Topology` read builds the tree and rewrites
                        # `Checked`/`IsIsolated`/`BusChecked` on every element
                        # (r4133 `Common/Circuit.pas:2932-2950`). `None` (not
                        # `[]`) when the case does not request it, so the Rust
                        # side's `Option<TopologyCap>` tells "not captured" from
                        # "captured empty".
                        "topology": (capture_topology(ckt) if want_topology else None),
                        # G1.8: read after `topology`, i.e. STRICTLY LAST of
                        # the whole step — the pair rewrites solution state
                        # and (on r4133) moves `ActiveCktElement`, and it must
                        # not precede the topology read that memoizes
                        # `Branch_List`. `None` (not an empty dict) when the
                        # case does not request it, so the Rust side's
                        # `Option<..>` tells "not captured" from "captured
                        # empty".
                        "inc_matrix": (
                            capture_inc_matrix(d, ckt) if want_inc_matrix else None
                        ),
                        "global_result": global_result,
                        # G1.9 — read at the top of the step (see the block
                        # above); listed last only because the dict is
                        # serialization order, not read order.
                        "aggregates": aggregates,
                        "solution_scalars": solution_scalars,
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

        # G1.10a: STRICTLY LAST inside the guard scope — every file the run
        # wrote (including the AutoAddLog just read) is still on disk, and the
        # `__exit__` below removes exactly what this call classifies.
        # `created()` returns None when it cannot report honestly (incomplete
        # pre-run snapshot / failed listing); the gate's presence rail turns that
        # into a failed case rather than "nothing was created".
        run_files = guard.created() if want_run_files else None

        # D32(2)(a): release the circuit BEFORE the guard sweeps. dss_capi opens
        # a Storage `debugtrace` file at edit time and NEVER closes the stream
        # for the life of the object (`src/PCElements/Storage.pas:872`;
        # `FreeAndNil(TraceFile)` only at `:871` on a re-edit and `:1199` in
        # `TStorageObj.Destroy`), so with the circuit still alive the guard's
        # `os.remove` raises, the file survives, and the NEXT producer of the
        # same case snapshots it as pre-existing and reports an empty created
        # set — the artifact G1.10a F4 measured (`tmp/g110a/probe_f4e.py`). One
        # `clear` runs every element's destructor, which closes those handles.
        # It is a TEARDOWN, not a read: it comes after `created()`, so the
        # reported surface is still exactly the run
        # `clear -> compile -> post -> n x solve` on both transports, and the
        # r4133 bridge (whose engines close their trace files immediately, r4133
        # `Version8/Source/PCElements/Storage.pas:1085`) needs no counterpart.
        #
        # D33(1): the teardown is GUARDED. The pinned dss_capi 0.14.5 raises on
        # this second `clear` on the two AutoAdd decks (`modes:autoadd/autoadd.dss`
        # and `autoadd_cap.dss`: `DSSException (#303) ... clear ... Access
        # violation` - the same backend whose AutoAdd solve already segfaults at
        # process exit, `GAPS_PLAN.md` 2.2). The compared surface is captured
        # ABOVE this point, so the fault cannot corrupt it: the exception is
        # recorded as `teardown_error` (reported in the reply and surfaced by
        # `corpus_gate::runner::compare_with_result` without failing the case),
        # the guard below still sweeps and still reports `sweep_failed`, and
        # `main` exits a PERSISTENT worker after replying so the pool respawns it
        # - a raised access violation may have poisoned the process. Recorded,
        # never swallowed.
        teardown_error = None
        try:
            d.Text.Command = "clear"
        except Exception as e:
            teardown_error = f"{type(e).__name__}: {e}"
            log(f"teardown clear raised for {case_path}: {teardown_error}")

        # G1.10b / decision D40(6): the CONTENTS of the files the gate selected,
        # copied out of the case directory into the gate-owned sidecar while the
        # guard still holds them. It consumes the set classified above - never a
        # second classification, never a second listing rule.
        #
        # It runs AFTER the D32(2)(a) teardown on THIS channel, and that slot is
        # MEASURED, not cosmetic (`tmp/g110b/probe_f1_stor_handle.py`, G1.10b F1):
        # dss_capi 0.14.5 holds a Storage `debugtrace` stream open for the life of
        # the object (`src/PCElements/Storage.pas:872`, freed only at `:871` on a
        # re-edit and `:1199` in `TStorageObj.Destroy`) with a share mode that
        # denies READ, so before the teardown `STOR_<name>.csv` cannot be opened at
        # all (`PermissionError: [Errno 13]`) - the same handle whose `os.remove`
        # D32(2)(a) had to move the teardown for. After the `clear` it reads whole
        # (28 777 bytes on `Storage_price.dss`). The ordinary export reports are
        # unaffected either way: `ExportVoltages` and its siblings close their file
        # as they return, and `fbs_EXP_VOLTAGES.csv` is byte-identical before and
        # after the teardown (349 bytes, sha256:0e8329e54fab84ee, measured). The
        # r4133 transport needs no such move - it has no teardown at all, because
        # r4133 closes its own trace file as it writes each record
        # (`Version8/Source/PCElements/Storage.pas:1085`).
        #
        # `crates/dss-core/tests/capture_order.rs::check_run_file_contents_read_with_the_set`
        # asserts this slot from the source text of both transports.
        run_file_contents = _copy_selected_contents(
            guard.dir, run_files, contents_patterns, contents_dir
        )

    # D32(2): whatever the guard could not remove. Read AFTER the `with` block
    # (`__exit__` fills it) and reported unconditionally — the hygiene contract
    # does not depend on the run-file request flag. The gate's runner fails the
    # case on a non-empty list
    # (`crates/dss-core/tests/corpus_gate/runner.rs::compare_with_result`); the
    # r4133 twin is `CaseResult::sweep_failed` in `crates/dss-epri/src/capture.rs`.
    # A raise inside the block skips this line — `__exit__` still sweeps, and that
    # case has already failed on the error itself (same as the Rust `?` path).
    sweep_failed = guard.sweep_failed

    return {
        "node_order": node_order,
        "n_steps": n_steps,
        "checkpoints": checkpoints,
        "autoadd_log": autoadd_log,
        "run_files": run_files,
        "run_file_contents": run_file_contents,
        "sweep_failed": sweep_failed,
        "teardown_error": teardown_error,
    }


def make_engine():
    """Bind the pinned dss-python 0.15.7 / dss_capi 0.14.5 oracle (`gc.check_pin()`
    hard-asserts the pin) and return `(dss.DSS singleton, provenance)`. This is the
    only engine this server drives — the `capi_v0145` channel of the unified gate.

    `DSS_ORACLE_ENGINE` is honored only as an explicit `"capi"` (the default); any
    other value exits non-zero (the retired `capi015`/`oddie` EPRI arms were removed
    with the Python EPRI stack — the r4133 channel now runs through the in-house
    `crates/dss-epri` Rust bridge). Never a silent pass to the wrong engine."""
    engine = os.environ.get("DSS_ORACLE_ENGINE", "capi")
    if engine != "capi":
        sys.exit(
            f"unknown DSS_ORACLE_ENGINE={engine!r} (only 'capi' is supported; "
            "the EPRI r4133 channel runs through crates/dss-epri, not this server)"
        )
    oracle = gc.check_pin()  # hard-asserts dss-python 0.15.7 / engine 0.14.5
    from dss import DSS as d

    return d, oracle


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
        # CF-C Port 2: a `warn_and_continue` deck needs EarlyAbort off for the
        # WHOLE run (compile + solves) so a mid-redirect user-model warning does
        # not truncate the circuit. Toggle it per request and always restore, so
        # the setting never leaks into the next (persistent-server) case.
        warn = bool(req.get("warn_and_continue", False))
        prev_ea = _get_early_abort(d) if warn else None
        if warn:
            _set_early_abort(d, False)
        try:
            result = run_case(d, req)
            reply({"ok": True, "result": result})
        except Exception as e:  # one bad case must not kill the server
            log("case failed:\n" + traceback.format_exc())
            reply({"ok": False, "error": f"{type(e).__name__}: {e}"})
        else:
            # D33(1): this case's teardown `clear` raised inside the pinned
            # oracle (see `run_case`). The reply is already written and flushed,
            # so the case keeps the surface it captured before the teardown -
            # but a raised access violation may have left this process in an
            # undefined state, and a PERSISTENT worker must not serve another
            # case on it. Exit instead: the pool kills, respawns and retries
            # once on a worker whose pipe EOFs
            # (`corpus_gate::engines::WorkerPool::call` / `checkin`). A one-shot
            # request already has stdin closed and would exit on the next
            # `readline()` anyway.
            if result.get("teardown_error"):
                log(
                    "exiting after a failed teardown clear "
                    f"({result['teardown_error']}) - the pool respawns this worker"
                )
                break
        finally:
            if warn and prev_ea is not None:
                _set_early_abort(d, prev_ea)


if __name__ == "__main__":
    main()

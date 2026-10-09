# UPGRADE divergence ledger (`docs/upgrade/DIVERGENCES.md`)

Per `UPGRADE_PLAN.md` §1.4. The plan's end-target is **official OpenDSS 11.0.0.1
(r4133)**, so the default decision is **EPRI r4133 behavior wins**. dss_capi
0.15.x deliberately diverges from EPRI in a handful of places (its
`docs/known_differences.md`); each such divergence is settled here — adopted or
reproduced — with: the observable, both behaviors, the probe evidence, the
decision, and the gate consequence (which oracle gates it + the `known_diffs.json`
entry). No divergence decision lives only in a commit message.

Format per entry: **Observable · dss_capi 0.15.x · EPRI r4088/r4133 · Probe ·
Decision · Gate consequence.** Seed rows L1–L4 are in the plan's §1.4 table.

---

## L2 — zero `kW`/`kVA` handling (`DblValueNZ` clamp) — SETTLED (WP-U1.1, adopt EPRI clamp)

**Observable.** A `Load`/`Generator`/`Storage`/`PVSystem` (and `WindGen`, U1.8)
essential-sizing property (`kW`, `kVA`, `MVA`) parsed as `0` (more precisely, any
value in the open band `(-1e-8, 1e-8)`).

**dss_capi 0.15.x (capi015).** Two overlapping mechanisms, both **off by
default**:
- `TPropertyFlag.ReplaceZero` on these props replaces exactly-`0` with `1e-8`
  **only when the `PermissiveProperties` compat flag is set**
  (`DSSObjectHelper.pas:2984` — `if ReplaceZero and Value=0 and (COMPAT and
  PermissiveProperties)<>0 then Value:=1e-8`).
- `TPropertyFlag.NonZero` on some of the same props (`Load.kVA`, `Generator`,
  `Storage`, `PVSystem`) raises a **strict error** by default
  (`DSSObjectHelper.pas:3013`, unconditional). `Load.kW` had `NonZero` *removed*
  (handled by `NonZeroSpecSets` spec-set tracking instead).

So capi015 **default**: `Load.kW=0` (with `kvar` given) is accepted and kept as
literal `0`; `…kVA=0` (or the strict spec-sets) **errors**.

**EPRI r4088 / r4133.** `Parser.DblValueNZ` (`ParserDel.pas:912`,
`MakeDoubleNZ`): the essential-sizing props read the parsed value through a
band-clamp — `if (Result < 1e-8) and (Result > -1e-8) then Result := 1e-8` —
**unconditionally, by default, no error**. `Load.pas` prop 4/23, `generator.pas`
prop 4/26, `Storage.pas` `kW`/`kVArating`, `PVsystem.pas` `kVArating`,
`WindGen.pas` `kW`/`kVA`/`MVA`.

**Probe** (`scratch_probe_l2.py`, 2026-07-12; deck = 3-phase `Load.z kW=<x> kvar=1`):

| engine | `kW=0` default | `kW=0` + `CompatFlags=0x200` |
|---|---|---|
| capi015 (0.15.0b4) | `powers[0]≈6.8e-24`, `?kw → 0` (kept 0) | `powers[0]=3.33e-9`, `?kw → 1E-8` (clamps) |
| oddie r4088 (10.2.0.1) | `powers[0]=3.33e-9`, `?kw → 1E-008` (clamps) | (0x200 rejected by Oddie — only `MonitorHeader`) |
| oddie r4133 (11.0.0.1) | `powers[0]=3.33e-9`, `?kw → 1E-008` (clamps) | (same) |

So the `PermissiveProperties` bit is `0x200`; capi015-with-`0x200` reproduces the
EPRI **default** clamp. `r4088 == r4133` here (parser untouched across the delta).

**Decision — adopt the EPRI r4133 clamp as the dss-rs default.** Implement the
`DblValueNZ` band-clamp on {Load `kW`/`kVA`, Generator `kW`/`kVA`, Storage
`kW`/`kVA`, PVSystem `kVA`} — `PropFlags::REPLACE_ZERO`, applied unconditionally
(EPRI default), pre-scale, in `obj/props/setters.rs::set_obj_double`. Do **NOT**
adopt dss_capi's strict-`NonZero`-error default nor the compat-flag gating — that
strict surface is dss-extensions-only (`known_differences.md` §2). WindGen's
props get the flag when WP-U1.8 lands the class.

**r4133 asymmetry — Generator `MVA` does NOT clamp.** `generator.pas:663` reads
`kVA` (prop 26) through `DblValueNZ` (clamps) but `MVA` (prop 27, line 664)
through plain `DblValue * 1000` — so `Generator … MVA=0` sets `kVArating=0`
un-clamped, while `kVA=0` clamps to `1e-8`. Reproduced 1:1 (only `kVA` carries
`REPLACE_ZERO`). WindGen differs: its `MVA` (prop, `WindGen.pas:638`) IS
`DblValueNZ * 1000` — so WindGen's `kW`/`kVA`/`MVA` all clamp (U1.8).

**Gate consequence.**
- **The `V`/`I`/`Y`/`losses` numeric channels do not move at gate resolution.**
  The clamp value is `1e-8` (kW/kvar), which contributes `~1e-8` power — below
  every live-compare `i_abs` floor (micro `1e-6`; §TOLERANCE_NOTES). The
  `capi↔capi015` U0.2 sweep saw no numeric witness for zero-kW (capi015 default
  also keeps `0`).
- **BUT the clamp DOES move a default-oracle *property readback* — corrected from
  the first draft of this row, which wrongly claimed "no default-oracle observable
  moves."** A `Load` with **both `kW=0` and `kvar=0`** ends in the `KwKvar` spec
  (the trailing `pf=…` sets `PFNominal` but not the spec type). Un-clamped
  (0.14.5): `kVA=√(0²+0²)=0`, so `RecalcElementData`'s `if kVA>0` guard skips the
  PF recompute and `PFNominal` keeps the parsed `pf`. Clamped (Rust≡r4133):
  `kW=1e-8`, `kVA=1e-8>0`, so `PF := kW/kVA = 1`. So `? load.pf` reads **`1`** post-
  clamp vs the parsed `0.9` un-clamped — a discrete-magnitude property move.
  Probe (`scratch_probe_pf.py`, 2026-07-12; deck `Load kW=0 kvar=0 pf=0.9`):
  `0.14.5 → pf=0.9`; `capi015 → ERROR #2025111` (strict NonZero, the surface we
  reject); `r4088/r4133 → pf=1, kw=1E-008, kva=1E-008`. Rust matches r4088/r4133.
- **Corpus impact = exactly one mandatory-gate deck.** A full scan
  (`/tmp/scan_zero.py`) of the vendored corpus for the breaking pattern (Load
  `kW=0 ∧ kvar=0`; Generator/Storage/PVSystem `kW=0 ∨ kVA=0`) finds it in only two
  files: `epri_dpv/M1/Loads_Only.dss` (8 loads) and `ieee9500dss` (6 gens, kva=775
  un-affecting, and **not in solvable_now**). Only **`epri_dpv/M1/Master_NoPV.dss`**
  (kind `feeder`, so property-parity-compared) is in the mandatory gate. Per §1.2
  it is **flipped to `oracle: "r4133"` in this same commit**; the whole-model live
  compare against r4133 passes green (`corpus_live_solvable_cases_match_oracle`,
  172s), and target-rev cases drop `compare_all_properties` (§1.3-2), so the PF
  readback is no longer compared against the 0.14.5 oracle it deliberately
  mismatches. The 748 zero-`kW`/`kVA` corpus lines that end in a `KwPf`/`KvaPf`
  spec (or are on `large`-kind decks, property compare off) stay green.
- The exact behavior is pinned three ways: (1) **exact per-class** Rust unit
  tests — `zero_kw_kva_clamp_dblvaluenz` on `load`, `generator` (also pins the
  `MVA`-NOT-clamped asymmetry: `MVA=0 → kVA rating 0`), and `storage` (`kW=0`
  clamp witnessed via `Set_kW` resolving DISCHARGING not IDLING), plus
  `zero_kva_clamp_dblvaluenz` on `pvsystem`; each fails when the clamp is disabled.
  (2) the live `epri_dpv/M1` feeder now on `oracle: "r4133"` — a real corpus
  witness whose loads clamp on r4133 too, so a broken clamp would diverge.
  (3) the synthesized `modes/upgrade_parser_zerokw.dss` (also `oracle: "r4133"`),
  made **feature-sensitive per §1.7-3** by `Load.zpf` (`kW=0 ∧ kvar=0`) + a `?pf`
  probe: the clamp recomputes `PF 0.9→1` (kW=kVA=1e-8), so the probe diverges
  `0.9` vs `1` (|Δ|=0.1 » the 1e-8 numeric-channel floor) the moment the clamp
  no-ops. (Settle 2026-07-12: coverage-gap findings — (1) and the deck's
  feature-sensitivity were added then; the item-1 draft had only the Load unit
  test and numeric-only probes below floor.)
- `known_diffs.json`: no Rust↔EPRI entry existed for zero-kW at r3723 (0.14.5 and
  the port both kept `0`, matching each other); adopting the clamp now makes
  Rust match r4133 — nothing to retire, nothing newly diverges vs r4133.

---

## ParseAsSymMatrix incomplete matrix — SETTLED (WP-U1.1 item 2, adopt EPRI reject)

**Observable.** A symmetric-matrix property (`rmatrix`/`xmatrix`/`cmatrix` on
Line/LineCode/Reactor, LineGeometry `Zmatrix`, …) whose value supplies **fewer
rows than the element's order** (`NPhases`/`NConds`) — e.g. a 3-row `cmatrix` on
an `nphases=4` linecode.

**dss_capi 0.14.5 (default oracle) AND 0.15.x (capi015).** The FPC
`TDSSParser.ParseAsSymMatrix` (`ParserDel.pas:736`) has **no `OrderFound`
check**: it zero-fills the buffer, writes the rows that were supplied, and always
returns `ExpectedOrder`. So a missing row silently stays **zero** and the element
is built from that partly-zero matrix. (The FPC guard that *does* error is
`subpos > maxpos` — too *many* elements on one row — code 65534.)

**EPRI r4088 / r4133.** Delphi `TParser.ParseAsSymMatrix` (`ParserDel.pas:741`)
increments `OrderFound` per row with `ElementsFound > 0` and, after the loop,
`if OrderFound < ExpectedOrder then DSSMessageDlg('The matrix entered does not
match with the expected order, review the entered parameters and try again.',
TRUE); Result := 0`. `Result = 0` makes the caller **reject** the matrix — the
property keeps its prior value — with a log-and-continue error (the object
survives). The r4088→r4133 parser is byte-identical, so both behave the same.

**Probe** (`probe_symmatrix.py`, 2026-07-12; `new linecode.lc nphases=3
rmatrix=(1|2 3) xmatrix=(complete) cmatrix=(complete)`, then `? …rmatrix`):

| engine | `rmatrix=(1)` (1 row) | `rmatrix=(1|2 3)` (2 rows) |
|---|---|---|
| capi 0.14.5 (default) | `[1 \|0 0 \|0 0 0]` (zero-fill) | `[1 \|2 3 \|0 0 0]` (zero-fill) |
| capi015 (0.15.0b4) | `[1 \|0 0 \|0 0 0]` | `[1 \|2 3 \|0 0 0]` |
| oddie r4088 (10.2.0.1) | rmatrix = **default** (rejected) | rmatrix = **default** (rejected) |
| oddie r4133 (11.0.0.1) | rmatrix = **default** (rejected) | rmatrix = **default** (rejected) |

So r4088/r4133 reject and revert; both capi lines zero-fill.

**Decision — adopt the EPRI r4133 reject as the dss-rs default.**
`Parser::parse_as_sym_matrix` now returns `OrderFound` (rows with ≥1 value); the
caller `ClassProps::parse_into` (both `SymMatrix*` and `DoubleSymMatrix` arms)
rejects when `OrderFound < order`: it pushes the r4133 message to `eng.errors`
and skips the commit, leaving the property at its prior value — the exact
DoSimpleMsg-and-continue semantics. The too-many-per-row `subpos>maxpos` path
stays an `Err` (unchanged; both oracles also error on it). This diverges from
capi015 (the Rung-1 primary oracle) — a deliberate ledger exception favoring the
plan's r4133 end-target (§1.4 default "EPRI r4133 wins").

**Gate consequence.**
- **No mandatory-gate (solvable_now) deck moves.** A full corpus scan
  (`scan_incomplete_matrix.py`) for a sym-matrix with fewer rows than its phase
  count finds exactly one witness: `IEEETestCases/4wire-Delta/
  Kersting4wireIndMotor.dss` (linecode `556MCM`, `nphases=4`, a **3-row
  cmatrix**). All three `Kersting4wire*` decks are already in
  `skipped_oracle_issue.json` (the oracle cannot load their `IndMach012a`
  user-model, #570 — unrelated to matrices), so none is a live gate case. The
  default-oracle gate stays green because no compared case supplies an
  incomplete matrix.
- **No live deck is possible for this behavior.** The adopted reject cannot be
  gated live: **capi015 zero-fills** (comparing Rust-reject against it would be a
  deliberate mismatch, forbidden by §1.2), and **oddie r4133 hangs** the moment
  an incomplete rmatrix leaves a linecode's series Zmatrix inconsistent — probed
  2026-07-12: `new linecode.lc nphases=3 rmatrix=(0.4|0.1 0.4)` (rmatrix-only,
  no xmatrix/cmatrix) never returns under Oddie, and any deck that reaches
  `solve` after such a reject times out. So r4133 can neither compile nor solve
  the reject feature, and §1.7's "compiles/solves on target oracle" is
  unattainable here. (The one corpus witness keeps a *complete* rmatrix+xmatrix,
  so its series Z is fine, but it is oracle-skipped for the user-model reason
  above.)
- **Pinned by feature-sensitive Rust unit tests instead** — the honest gate for
  a behavior neither oracle can drive: `dss-parser`
  `sym_matrix_returns_order_found_for_incomplete_input` (OrderFound 2 for a
  2-of-3-row matrix; 3 for a complete one) and `dss-core` line_code
  `incomplete_sym_matrix_rejected_keeps_default` (asserts BOTH the r4133 reject
  message is logged AND rmatrix reverts to the default symmetric-component matrix
  — not the zero-filled `[1|2 3|0 0 0]`) + `complete_sym_matrix_still_accepted`
  (guards against over-rejection). Each flips if the reject is removed — a
  discrete structural change, far above any numeric floor.
- `known_diffs.json`: no Rust↔EPRI entry existed for this (0.14.5 and the port
  both zero-filled, matching each other at r3723); adopting the reject makes Rust
  match r4133 — nothing to retire.

## AllowNoneItem — `none` in conductor lists — SETTLED (WP-U1.1 plumbing; scope + compaction re-decided to r4133 by the 0.15.x-adoption sweep)

**Observable.** A `none` entry inside a `DSSObjectReferenceArrayProperty` — the
conductor lists `Wires`/`CNCables`/`TSCables` on Line and LineGeometry (SVN
r3902/r3913, `TPropertyFlag.AllowNoneItem`).

**Where `none` is legal — Line-level ONLY (re-verified against r4133 + both
oracles).** r4133 `Line.pas` FetchWireList/FetchCNCableList/FetchTSCableList
(:1866-1975) treat `CompareText(…,'None')=0` as a **NIL slot**; `LineGeometry.pas`
:346-396 (the direct `wires`/`cncables`/`tscables` arms, props 12/15/16) have **no
`none` branch** — the token drives `WireDataClass.Code := 'none'` → #10103. So a
**geometry-level** `none` is rejected by BOTH gating oracles, while a
**Line-level** `none` is accepted and (after compaction) SOLVES.

**Own probes** (epri-worker r4133 + pinned 0.14.5):

| deck | 0.14.5 | r4133 | port |
|---|---|---|---|
| `new linegeometry.g nconds=2 nphases=2 wires=(w none)` | #40303 reject | #10103 reject | **reject** (ALLOW_NONE_ITEM dropped) |
| `line.l1 phases=1 spacing=sp(nc=2,np=1) wires=(w none)` + load | (feature n/a) | **converged, I1=(21.802597,−0.001427)** | **converged, matches to faer-vs-KLU floor** |

**Decision — Line-level accept + compact (r4133), geometry-level reject
(r4133).** `PropFlags::ALLOW_NONE_ITEM` stays on the three **Line** lists +
`Conductors`, and is **dropped** from the three **LineGeometry** lists
(`line_geometry/mod.rs`). The `none` NIL slot is now consumed: `LoadSpacingAndWires`
(`line_geometry/matrix.rs`, r4133 LineGeometry.pas:1190-1262) recounts the
conductors actually present (`actualNConds`/`actualNPhases`), sizes the throwaway
geometry to the compacted count, and copies the non-NIL wires into contiguous
positions with their ORIGINAL spacing coordinates; `FMakeZFromSpacing`
(`line/solve.rs`, r4133 Line.pas:2213-2219) raises **#181021** and aborts when a
`none` at a PHASE position drops the phase count below the Line's `phases=` (both
oracles do). A `none` in a NEUTRAL position leaves the phases intact and solves.

**The earlier "no solvable standalone `none` deck" claim is DISPROVEN.** The
capi015-side probe that `#303`'d was a standalone LineGeometry (no consumer); the
Line+spacing `wires=(w none)` deck solves on r4133 (own probe above), and now on
the port — the WP-U1.4 mixed-list compaction that "closed without landing" is
landed here.

**Gate consequence.**
- **Gate-safe:** no corpus deck puts `none` in a conductor list (scanned — 0
  hits); the compaction is byte-identical to the old index-aligned copy for any
  list with no NIL (`actualNConds == NWires`, `j == i`), so every spacing line
  (IEEE13 …) is unchanged — full line/line_geometry/line_constants unit suites +
  the 514-case corpus gate stay green.
- **Pinned by** `line_fetch::conductor_none_geometry_rejects_but_line_accepts_and_solves`
  (geometry-level `none` rejects; Line-level `none` compacts + solves, currents
  pinned to the own r4133 probe; a non-`none` missing name — `nope` — still
  errors), + `dss-parser` `is_quoted_reflects_the_last_token_quote_state`.
- known_diffs: nothing to retire (both oracles + port now reject the geometry
  case and accept+solve the Line case — no Rust↔EPRI divergence, no ledger row).

## TCC_Curve `none` (rejection + AllowNone-single-ref) — SETTLED (WP-U1.1 item 4)

**Observable.** (a) `new TCC_Curve.none …`; (b) a single TCC_Curve reference on a
Recloser (`PhaseFast`/`PhaseDelayed`/`GroundFast`/`GroundDelayed`) or Fuse
(`FuseCurve`) set to the literal value `none`.

**Spec.** dss_capi `fd034bb0` ("port SVN r4119"): `TTCC_Curve.NewObject` rejects
name `none` (DoErrorMsg 423, returns NIL) + `TPropertyFlag.AllowNone` added to the
five curve refs so `none` clears them (`DSSObjectHelper.pas:862`).

**Probe** (`probe_tcc_none.py` / `probe_r4133_tcc.py`, 2026-07-12):

| observable | capi 0.14.5 | capi015 | r4133 |
|---|---|---|---|
| `new TCC_Curve.none` | (no reject) created | **#423 reject**, not created | **#423 reject**, not created |
| `fusecurve=none` readback | `''` (cleared) | `''` (cleared) | **`none`** (literal stored) |
| `fusecurve=none` error | #401 not-found | **#401 not-found** | none |

**(a) Rejection — adopt.** `new TCC_Curve.none` now errors ("…\"none\" is a
reserved name…") and creates no object, in `command.rs::add_object` (the
DSS_OBJECT branch, keyed on class name). capi015 == r4133 here. (0.14.5, the
default oracle, did NOT reject — but no corpus deck defines `TCC_Curve.none`, so
no gate case moves.) Gated by the `tcc_curve_none_is_reserved` unit test
(feature-sensitive: the object stays absent).

**(b) AllowNone-single-ref — SUPERSEDED by WP-U2.1 (now silent-clear, r4133-aligned).**
The WP-U1.1 decision recorded below reproduced the capi015 no-op quirk
(`fusecurve=none` clears the ref AND logs #401, since capi015's AllowNone branch
sets `otherObj := NIL` at l.862 but the unconditional `if otherObj = NIL then
DoSimpleMsg(… 401)` at l.867 fires anyway). **WP-U2.1 later re-decided this to the
r4133 behavior:** `fusecurve=none` **clears the ref SILENTLY** (no #401), pinned by
`lifecycle::fuse_curve_none_clears_silently_like_r4133` (the old
`_clears_with_error_like_capi015` name is retired). So the paragraph below is the
historical WP-U1.1 record; the live behavior is r4133 silent-clear.
- **(historical WP-U1.1 record.)** The C7 feature is observably a no-op in capi015
  (clears + logs #401, the plain not-found path); r4133 clears silently. WP-U1.1
  first kept the capi015 #401; WP-U2.1 flipped to the r4133 silent clear (no live
  oracle gates (b): capi015 raises the #401 in dss-python early-abort, r4133 renders
  the silent clear — unit-test pinned, same rationale as §ParseAsSymMatrix).

## Class-command activation (C11 / SVN r3875) — SETTLED (WP-U1.1 item 5)

**Spec.** dss_capi `7457fc0b` ("port part of SVN r3875"): `SetObjectClass` now
sets `DSS.ActiveDSSClass` (not only `LastClassReferenced`) — "always activate the
selected class, if any" — and two callers (ShowResults, DoSelectCmd) drop their
now-redundant `ActiveDSSClass := …`.

**Port mapping.** The port collapses Pascal `LastClassReferenced` **and**
`ActiveDSSClass` into a single `active_class` field, so every already-ported
SetObjectClass-equivalent (`do_select_cmd`, `add_object`, `do_edit_cmd`,
`do_batch_edit_cmd`, `do_enable_disable_cmd`) *already* reproduces the fixed
behavior — no stale-`ActiveDSSClass` divergence is representable. The only Rust
paths where the r3875 fix introduces behavior over 0.14.5 were the `Set Class=`
and `Set Object=` SET-options, which were **NOT_PORTED** (emitted "not ported
yet"). Per "port gaps immediately", this WP ports them:
- `Set Class=`/`Set Type=` (opts 12/1) → `set_object_class`: activate the named
  class (unified `active_class` — carries the r3875 `ActiveDSSClass` set); unknown
  class logs #903 and keeps the previous class.
- `Set Object=`/`Set Element=` (opts 13/2) → `set_object`: select the object
  (class-qualified or bare against the active class), now also making a circuit
  element the `ActiveCktElement` (Pascal `SetActive` — was missing, so a bare
  `? prop`/`~` couldn't reach it).

**Probe transcript (settle 2026-07-12 — witnessing the SetObject semantics that
were previously derived from Pascal only; `probe_setobj.py`).** capi015 (0.16.0b2)
and 0.14.5 are IDENTICAL on every case (`SetObject`/`SetObjectClass` are
byte-identical in `dss_capi_with_git` across the delta):

| sequence | capi015 & 0.14.5 → active ckt element / error |
|---|---|
| `Set Class=Line; Set Object=l2` (bare resolve) | `Line.l2` selected, no error |
| `Set Class=Line; Set Object=badclass.l1` (unknown qualifier) | logs **#903** "Object Class badclass not found" **yet still selects `Line.l1`** (fall-back to `LastClassReferenced`) |
| `Set Object=line.l1` (valid qualifier) | `Line.l1`, no error |

**Fall-back FIX (settle 2026-07-12).** The first draft of `set_object` aborted on
an unknown class qualifier (`class_by_name.get→None→return false`), diverging from
the transcript above. Pascal `SetObject` (`DSSGlobals.pas:303-353`) instead calls
`SetObjectClass` (logs #903, its FALSE return **discarded**, `LastClassReferenced`
unchanged), then resolves the name against the *previous* class. `set_object` now
mirrors this (calls `set_object_class`, keeps `active_class`, falls back) exactly
like the already-correct `do_select_cmd`; a NIL active class emits #905 "Active
object type/class is not set." Pinned by the new
`set_object_unknown_class_qualifier_falls_back` (bad qualifier → #903 logged AND
`Line.l1` still selected).

**Gate.** No corpus deck uses `Set Class=`/`Set Object=` (scanned — 0 hits), so
zero mandatory-gate movement; the port is gate-safe. Also pinned by
`set_class_activates_and_set_object_selects` + `set_class_unknown_errors_keeps_previous`.
No oracle flip needed (capi015 == 0.14.5 on every case above; the r3875
`ActiveDSSClass` refresh is unobservable — `Set Object` bare-resolution reads
`LastClassReferenced`, set in both revs).

**Before a circuit exists (settle 2026-07-12 — refuting the "silent no-op"
concern).** `Set Class=`/`Set Object=` before any circuit route through
`DoSetCmd_NoCircuit`, whose `else` arm (`ExecOptions.pas:303`) emits **#301** "You
must create a new circuit object first" and `Result := FALSE` for any option it
does not explicitly handle — it is NOT a silent no-op (the Pascal `case` DOES have
an `else`). Probed 2026-07-12 (`probe_905.py`): `Clear; Set Class=Line` and
`Clear; Set Object=l2` both raise #301 on capi015 AND 0.14.5. The port's
`do_set_cmd_no_circuit` catch-all `_ =>` arm emits exactly this #301 message, so
it already matches the oracle — no change needed.

Follow-up (separate `?`-command gap, NOT r3875): a bare `? prop` querying the
ActiveCktElement is still unported in `do_query_cmd` — noted in STATUS, an owner
for a later WP.

## B2/D1 — SimpleCarson earth-return De constant `658.5 → 658.8530451057239` — SETTLED (WP-U1.2, adopt capi015; reproduce the upstream Line.Kxg inconsistency)

**Observable.** The series impedance `Zmatrix` of any Line whose Z is built from
a `LineGeometry`/`LineSpacing`/cable under `EarthModel=Carson` (the SimpleCarson
earth model). `LineConstants.GetZearth`'s SimpleCarson branch computes the
earth-return reactance from `ln(De·√(ρ/f))` with the De constant.

**dss_capi 0.14.5 (default oracle).** `De = 658.5` (`LineConstants.pas`
`Get_Ze`/SIMPLECARSON). The port matched this at r3723.

**dss_capi 0.15.x (capi015) / EPRI r4088+.** `De = 658.8530451057239` — the
precise `De ≈ 658.87·√(ρ/f)` reference constant, corrected in the r3913-era
line/conductor rework (`LineConstants.pas:474`, `GetZearth`). r4088/r4133 carry
the same corrected value (byte-identical LineConstants across the deltas).

**Upstream INCONSISTENCY reproduced 1:1.** dss_capi 0.15.x corrected the
constant ONLY in `LineConstants`; **`Line.pas`'s own `Kxg`** — used for the
frequency-dependent ground-reactance adjustment `Xgmod = 0.5·Kxg·ln(FreqMult)`
— **keeps `658.5`** (`Line.pas:531/741/1077`, unchanged across the delta). So
within one Line the geometry-derived Z uses 658.85 while the Kxg frequency
correction uses 658.5. This is a genuine upstream inexactness, reproduced:
`line_constants/mod.rs::get_ze` uses `658.8530451057239`; the three `kxg` sites
(`elements/pd/line/{accessors,code,mod}.rs`) keep `658.5` under `TODO(compat)`
notes citing this ledger row. The §6 marker sweep unifies both to the corrected
constant, regenerating goldens deliberately.

**Probe** (`/tmp/probe_carson.py`, 2026-07-12; 3-phase overhead geometry,
`earthmodel=carson`, `? Line.l1.Xmatrix`):

| engine | `Xmatrix[0]` (ohm/km) |
|---|---|
| capi 0.14.5 (default) | `9.080731425743580e-01` |
| capi015 (0.15.0b4) | `9.081135553904781e-01` |

The ~4.0e-5 (rel ~3e-5) reactance shift is far above the live Y floor (1e-6) —
this is the plan's revision-**sensitive** first flip (WP-U1.2 gate note: a
numeric-routing regression now fails on the numbers, not only the ping).

**Decision — adopt the capi015 (=r4133) 658.8530451057239 in `LineConstants`;
reproduce the `Line.Kxg` 658.5 inconsistency.** Consistent with the plan's
"EPRI r4133 wins" default.

**Gate consequence.**
- **18 mandatory-gate decks flipped to `oracle: "capi015"`** in this same
  commit (§1.2): the two `Test/Cable*` cable-geometry decks, the twelve
  `InverterModels/.../MonitoredVoltage/{Local,Mon}_voltage_*-2` volt-var decks
  (Carson `LineGeometry.Poste`; no Cmatrix capacitor, so B1 does not touch them
  — B2 is the sole mover, confirmed by the empirical gate), and the four
  `IEEETestCases/4Bus-*`/`YYD-Master-step1` cable/geometry decks. Each compiles,
  solves, and its full assembled system Y matches capi015 (the corpus_live gate
  compares the entire Y entry-by-entry, so every Carson-geometry deck moves).
- **New capi015 golden** `tests/golden/line_constants/line_geometry_carson.json`
  (`oracle.engine_spec == "capi015"`), the offline revision-sensitive twin;
  registered in `golden_line_constants.rs`. This WP builds the generator engine
  switch (`gen_checkpoints.py::check_pin` honours `DSS_ORACLE_ENGINE=capi015`
  and stamps `engine_spec`).
- Three `line_constants`/`line_geometry` **Rust unit tests** re-referenced to
  capi015 (SimpleCarson Z probed on the Oddie venv; only the earth-return
  reactance — and, for the CN/TS-reduced cases, the coupled reduced resistance
  — move; FullCarson/DERI references untouched).
- `known_diffs.json`: no Rust↔EPRI entry existed (0.14.5 and the port both used
  658.5, matching each other at r3723) — nothing to retire.

## B5 — GFM `Isc1` factor-1000 removal — SETTLED (GFM WP, adopt capi015; no Rust op-point gap remained)

**Observable.** The equivalent short-circuit admittance (`CalcGFMYprim`) of any
PVSystem/Storage operating in grid-forming (GFM) mode — its Norton YPrim.

**dss_capi 0.14.5 (default).** `Isc1 = mKVArating·1000 / (√3·RatedkVLL) /
NPhases`. **0.15.x (capi015) / EPRI r4088+.** `Isc1 = mKVArating / (√3·
RatedkVLL) / NPhases` — the `·1000` DROPPED (`InvDynamics.pas:263`, commit
`de6a5a42`, port of SVN r3865, "preventing oversizing the model"). `Isc1` feeds
only `c = 4·(R1²+X1²) − (√3·RatedkVLL·1000/Isc1)²` in the R0 quadratic; the
separate `·1000` there (kV→V of RatedkVLL) is unchanged.

**Decision — adopt the capi015 (=r4133) `Isc1` (drop the `·1000`)** in
`inv_based_pce.rs::calc_gfm_yprim`. Consistent with the plan's "EPRI r4133 wins".

**The "pre-existing injection-vs-YPrim gap" the WP-U1.2 draft feared does NOT
exist at the GFM-WP base (`d99f0f3`, post U1.1/U1.2/CF2-G sync).** Re-probed
2026-07-12 (dss-python live `ActiveCktElement` f64 reads, both engines, +
toggled the Rust `Isc1` factor in-tree): the Rust GFM op-point is **already
Isc1-invariant**. On the islanded `gfm_micro` deck, `Load.isl` and
`Storage.batt` power and `islbus` voltage are **bit-identical** with the old and
the new `Isc1` (`Load.isl P = 399.999986 W`, `islbus = 0.99777 pu` both ways),
and both equal the bit-identical value 0.14.5 **and** capi015 report. The
YPrim itself moves to the capi015 value (`Y[0,0]: 613.68−2402.46j →
561.47−2245.82j`, matching the capi015 live probe). The U1.2 draft's "400→368
gap" was an artifact of that earlier code state (the CF2-G PVSystem dynamics
load-shape/`iMaxPPhase` fixes landed between U1.2 and this WP resolved it); the
`127094.3908 W/φ` figure was from a different probe deck.

**Why the op-point is invariant — the exact mechanism (decomposition, not a
tolerance sweep).** `Isc1` feeds ONLY the R0-quadratic, i.e. the **zero-sequence**
impedance `Z0 = Zs + 2·Zm`. The **positive-sequence** impedance `Zs − Zm`
collapses algebraically to `R1 + jX1 = Z1`, a function of `X1` alone
(`X1 = RatedkVLL²/mKVArating/√1.0625`) — **`Isc1`-free**. A balanced /
delta-fed GFM load excites only the positive sequence, so its terminal voltage —
hence delivered power — is invariant to the ~1000× `Z0` (Norton virtual-impedance
scale) move. Verified numerically: the balanced Norton eigenvalue `Y·v_pos/v_pos
= 842.14 − 3368.55j = 1/Z1` is bit-identical on 0.14.5 and capi015, while the
zero-sequence eigenvalue moves ~1000×. Pinned by
`inv_based_pce::tests::gfm_norton_positive_seq_admittance_is_isc1_invariant`,
`gfm_calc_yprim_matches_capi015_isc1_no_1000`, and
`storage::storage_gfm_micro_op_point_isc1_invariant`.

**Gate consequence.**
- **All GFM decks whose gated state has a discharging-GFM inverter move to
  `oracle: "capi015"`** in this same commit (the B5 `Isc1` change moves the
  assembled system Y at the StoBus/PVBus mutuals — witnessed `Y[STOBUS.1,STOBUS.2]`
  0.14.5→capi015 rel `1.209e-1` on IEEE123, `1.172e-1` on 8500-Node; the op-point
  channels V/I/P do NOT move, only the Norton YPrim). Determined empirically by
  diffing the capi015-vs-0.14.5 assembled Y per deck: **8 vendored decks** (the 2
  re-promoted `IBRDynamics_Cases/GFM_IEEE123{,_AmpLimit}` GFMDaily decks +
  `CannotPickUpLoad` + `Microgrid/.../{GFM_IEEE123/GFMSnap-C,GFM_IEEE123/GFMDailySwapRef,
  GFM_AmpsLimit_123/GFMSnap,GFM_AmpsLimit_123/GFMDailySwapRef,GFM_IEEE8500/GFMSnap}`)
  and **4 controls decks** (`controls/gfm/{gfm_micro,gfm_invcontrol,gfm_dynamics,
  pv_gfm_dynamics}`). Whole-model live compare (system Y + V/I/P per step) green
  vs capi015.
- **GFM decks whose gated state ends non-discharging stay 0.14.5-green** (their
  YPrim never reaches `CalcGFMYprim`): the `Microgrid/GFM_IEEE123/GFMDaily`,
  `GFMSnap`, `GFMSnap-A/B`, `GFMWholeDaily`, `GFM_AmpsLimit_123/GFMDaily`,
  `GFM_IEEE8500/{GFMDaily,GFMDailySmallerPV,Unbal}` decks all end
  Charging/GFL/IDLING. Confirmed by the same per-deck Y diff (rel 0.0).
- `known_diffs.json`: no Rust↔EPRI entry existed for GFM `Isc1` at r3723 (0.14.5
  and the port shared the `·1000`, matching each other) — nothing to retire;
  adopting the change makes Rust match r4133.

## capi015 daily `CktElement.Losses` staleness — SETTLED (GFM WP, capi015 quirk NOT reproduced)

**Observable.** `CktElement.Losses` (the engine `Get_Losses` path) of ANY element
in a multi-step **daily** (time-series) run, read after the first solve.

**dss_capi 0.14.5 (default) / EPRI r4133.** `Get_Losses` recomputes per solve;
for a daily-shaped Load it tracks the scaled power each step. **0.15.x (capi015).**
`CktElement.Losses` **freezes at the step-0 value** across subsequent daily steps
while `CktElement.Powers` scales correctly. Probed 2026-07-12 (a plain
grid-connected 3-phase daily `Load kW=400 daily=dl`, `mult=[1.0 0.92 0.85 …]`):

| step | capi015 `Powers` | capi015 `Losses` | 0.14.5 `Losses` |
|---|---|---|---|
| 0 | 400.00 kW | 400000 W | 400000 W |
| 1 | 368.00 kW | **400000 W** (stale) | 368000 W |
| 2 | 340.00 kW | **400000 W** (stale) | 340000 W |

General (not GFM/islanding-specific), an unrelated 0.15.x engine caching change.

**Decision — do NOT reproduce (capi015 quirk).** Rust — like 0.14.5 and r4133 —
recomputes losses fresh, matching `Powers`. Since `Get_Losses = Σ_k S_k` is
**mathematically the sum of the per-conductor `Powers`** the live gate already
compares (and which match), the `loss_w` channel is redundant; comparing Rust's
correct losses against capi015's stale ones would be a §1.2-forbidden deliberate
mismatch. `harness::compare_element` now **skips the `loss_w` cross-engine
compare only when the oracle's own captured `Losses` disagrees with the sum of
its own captured `Powers`** (the stale-oracle condition) — a self-validating
guard: a self-consistent oracle (0.14.5, r4133, and capi015 at step 0) still
fully gates the `Get_Losses` path, and no real coverage is lost because the
per-conductor Powers pin the same physics. `known_diffs`: nothing to retire (no
prior Rust↔EPRI entry).

## D7 — IBR dynamics current-limit base `PanelkW → FkVArating` (PVSystem) — SETTLED (WP-U1.2, adopt capi015)

**Observable.** `dynVars.iMaxPPhase` in `TPVsystemObj.IntegrateStates` — the
per-phase current limit that caps the GFL `ISP` and bounds the GFM droop
`ISPDelta`; also the `Max. Amps (phase)` mode-3 state variable (index 21).

**dss_capi 0.14.5 (default).** `iMaxPPhase = PanelkW / BasekV / NumPhases`
(`PVsystem.pas:2309`). **0.15.x (capi015) / EPRI r4088+.** `iMaxPPhase =
FkVArating / BasekV / NumPhases` (`PVsystem.pas:2275`, commit `32db066f`, port
of SVN r3868 — "IBR operational range": the limit tracks the inverter kVA
rating, not the instantaneous DC panel power). **Storage did NOT change** —
`Storage.pas` `InitStateVars` already used `FkVArating` in BOTH revs (r3582
predates 0.14.5); reproduced as-is. PVSystem's `InitStateVars` iMaxPPhase also
already used `FkVArating`; only `IntegrateStates` moved.

**Decision — adopt for PVSystem `IntegrateStates`** (`pvsystem/dynamics.rs`
`integrate_states_impl`, `panel_kw → self.f_kva_rating`).

**Gate consequence.**
- **`pvsystem_dynamics_mode3_matches_oracle`**: ch21 `Max. Amps` 23.149570 →
  **27.779484** (= ×kVA/PanelkW = ×600/500), matches capi015 exactly
  (`/tmp/probe_pvdyn.py`). The limit is non-binding (settles at 6.17 ≪ 27.78) so
  channels 13-20 are unchanged.
- **`pvsystem_dynexp_dynamics_mode3_matches_oracle`**: this deck's `isp ≈ 23.1457`
  sat right at the OLD clamp boundary; the new 600-base boundary (27.78)
  RELEASES it (isp now uncapped, matching capi015's clamp logic), shifting the
  single deterministic startup derivative `dit@0` 1055150.8 → 1054757.2
  (~0.035%). The BINDING settled pins (`it`=23.14571, sample 100+) are unchanged
  and match BOTH engines. The startup *transient* is 0.14.5-shaped, NOT capi015's
  (capi015 seeds the DynExp at the fixpoint — a separate, out-of-scope 0.15.x
  DynExp-init change; recorded, not adopted here).
- `pvsystem_gfm_dynamics_mode3_matches_oracle` is UNCHANGED (its GFM trajectory
  does not reach the iMaxPPhase clamp).
- **No live corpus deck moves under D7.** The GFM PVSystem deck `pv_gfm_dynamics.dss`
  has `kVA = Pmpp = 800` (`irradiance=1` ⇒ `PanelkW = FkVArating = 800`), so D7 is
  a numeric NO-OP there — probed bit-identical on 0.14.5 and capi015 (`/tmp/probe_pvgfm.py`).
  (It later flipped to `oracle:capi015` in the **GFM WP** — for B5, not D7: the
  GFM PVSystem reaches `CalcGFMYprim`, whose YPrim the B5 `Isc1` change moves;
  D7 stays a no-op there, so capi015 == 0.14.5 for the op-point.) No other corpus
  PVSystem-dynamics deck captures the current-limit under `kVA ≠ Pmpp`. D7 therefore
  lands as a unit-test-only same-commit package (no manifest flip), decoupled from B5.
- `known_diffs`: none matched — nothing to retire.

## D8 — Transformer X13/X23 `TrapZero` — SETTLED (WP-U1.2, no code change: not an observable delta; Rust already traps)

**Observable.** The 3-winding transformer reactances `X13`/`X23` (and `X12`) when
parsed as `0`.

**Spec.** The `TrapZero` FLAG on `X12`/`X13`/`X23` (commit `69fca934`,
`Transformer.pas` DefineProperties) and the `NonZero` flag on `XSCArray` are
BOTH already present in the 0.14.5 baseline — commit `69fca934` landed *before*
the 0.14.5 tag, not in the 0.15.x window. Verified against the vendored sources:
the `PropertyTrapZero` values (7/35/30 %) AND the `[TrapZero, ...]`
`PropertyFlags` block are byte-identical between `.inputs/dss_capi` (0.14.5,
`Transformer.pas:584-595` values+flags) and `.inputs/dss_capi_with_git` (0.15.x,
`:586-597`), and `NonZero` on `XSCArray` is present in both (0.14.5 `:413`, 0.15.x
`:401`). So this is **not a 0.14.5 → 0.15.x delta at all** — the source is
identical across our baseline and target (empirically reconfirmed by the
0.14.5 == capi015 probe below).

**Probe** (`/tmp/probe_d8.py`, `/tmp/probe_d8b.py`, 2026-07-12; 3-winding
delta/wye/wye, `XHT=0`):

| observable | capi 0.14.5 | capi015 |
|---|---|---|
| `? Transformer.t.XHT` (XHT=0) | `3500` | `3500` |
| `AllBusVmagPu` Vmin (XHT=0, solved) | `0.124819` | `0.124819` |
| `AllBusVmagPu` Vmin (XHT=35, solved) | `0.928196` | `0.928196` |

**Decision — no code change; NOT an observable delta for our port.** 0.14.5 and
capi015 are **bit-identical** for the reachable scalar `X13`/`X23`=0 path (both
reach the same trapped default via the Xsc build). The Rust port ALREADY traps
these: the `PropertyTrapZero` 7/35/30 were ported onto `XHL`/`XHT`/`XLT`
(`transformer/mod.rs`) and `setters.rs` applies `trap_zero` unconditionally, so
`XHT=0 → xht=35.0`, matching both engines. The `XSCArray` `NonZero` **strict
error** (present in both baselines, above) is the same C2/PermissiveProperties
strict surface the plan's L2 decision deliberately does NOT adopt (dss-ext-only).
Mirrors WP-U1.1's D5 / D8-r3723 "not a delta for us" records.

**Gate consequence.** No case/golden moves (no corpus deck sets X13/X23=0). Pinned
by the feature-sensitive unit test
`transformer::tests::three_winding_x13_x23_trap_zero_to_default` (XHT=0 → 35,
XLT=0 → 30). `known_diffs`: nothing to retire.

## B1 — Capacitor Cmatrix YPrim diagonal ×1.000001 before inversion — SETTLED (WP-U1.2; keep code on physics — the perturbation is capi015-ONLY, NOT r4088/r4133; attribution corrected by the 0.15.x-adoption sweep)

**Observable.** The YPrim of a `Cmatrix` (SpecType=3) capacitor that also carries
a **series filter reactance** (`R`/`XL` > 0 ⇒ `has_zl`) — the only config that
reaches `MakeYprimWork`'s SpecType-3 inversion path.

**dss_capi 0.14.5 (default) AND EPRI r4088 AND r4133.** The SpecType-3 branch
inverts the C-admittance work matrix directly, with NO diagonal perturbation —
**re-verified against the r4133 source** (`Capacitor.pas` `MakeYprimWork`: the
`×1.000001` "add a little bit so it will invert" loop is in the `1,2` Line-Line
`HasZL` arm ONLY; the `3:` CMatrix arm is bare `Invert; add ZL; Invert`). r4088 is
byte-identical. **The `×1.000001` on SpecType-3 is a capi015-ONLY change** (added
in 0.15.0b4, `git show 0.15.0b4:src/PDElements/Capacitor.pas` `3:` arm) — the
earlier "EPRI r4088+" attribution here was FALSE (the regcontrol_idle false-EPRI
class).

**Own live r4133 probe** (epri-worker, `Capacitor.f1 conn=wye
cmatrix=(1.5|0.2 1.5|0.2 0.2 1.5) R=0.5 XL=3` on a stiff 12.47 kV source,
`ActiveCktElement.Powers`): with `R/XL` r4133 reports **Q ≈ +1.4e-15 kvar** (the
bank vanishes — a structurally-singular invert, garbage) == 0.14.5; WITHOUT `R/XL`
r4133 reports **-25.4 kvar/phase (-76.2 total)**. Physics: series `ZL = 0.5+j3 Ω`
against `1/ωC ≈ 1768 Ω` must leave `Q ≈ -76 kvar` nearly unchanged, so capi015/
port (`Y[0,0] = (1.661774199e-07, 5.664824416e-04)`, real ≈ R/|Z|²) is physically
correct; both gating oracles' ≈0 is not.

**Decision — KEEP the capi015 behavior on physics + the VSConverter precedent**
(`capacitor/solve.rs`, the SpecType-3 `_ =>` arm keeps the ×1.000001 diagonal loop
before `invert()`). Adopting r4133 here would mean reproducing a
structurally-singular-matrix garbage invert — the exact class the project does NOT
reproduce (VSConverter `GetCurrents`). Only the false EPRI attribution is
corrected; the code stands. (The 0.14.5/capi015 `Y[0,0]` probe table — 0.14.5
garbage `(-5.29e-23, -1.08e-19)` vs capi015 finite `(1.661774199e-07,
5.664824416e-04)` — is retained in the unit-test pin below.)

**Gate consequence.**
- **No corpus/live witness.** No vendored deck defines a Cmatrix capacitor with a
  series reactance (the corpus caps are simple shunt-kvar); the only `cmatrix`
  hits in cap-bearing decks are LineCode matrices. So no corpus_live case and no
  golden moves.
- **Pinned by an oracle-validated unit test**
  (`capacitor::tests::cmatrix_with_series_reactance_yprim_matches_capi015`): the
  Rust YPrim phase block equals the capi015 probe reference to 1e-11/1e-12; the
  0.14.5 garbage (~1e-23) fails the `5.66e-4` diagonal assertion, so it is
  feature-sensitive to the ×1.000001.
- **Pre-registered ledger policy (no witness today).** The port's finite physical
  YPrim diverges from BOTH gating channels (`capi_v0145` garbage ≈0 kvar AND r4133
  garbage ≈0 kvar). If a future deck ever introduces a SpecType-3 + `R/XL`
  capacitor, it must carry a divergence ledger row **per channel** (`capi_v0145`
  and `r4133`) with exact-pair pins citing this entry (the port is the physically
  correct side; both oracles' singular-invert garbage is not reproduced) — NOT a
  tolerance loosening.
- `known_diffs`: none matched — nothing to retire.

## D6 — Transformer seasonal AmpRatings drop the `1.1 *` factor — SETTLED (WP-U1.2, adopt capi015; unit-pinned, no live witness yet)

**Observable.** A Transformer's per-season current ratings array `AmpRatings[i]`
(used by the seasonal overload-report override, `PDElement.NumAmpRatings>1`).

**dss_capi 0.14.5 (default).** `AmpRatings[i] := 1.1 * kVARatings[i] / Fnphases /
Vfactor`. **0.15.x (capi015) / EPRI r4088+.** `AmpRatings[i] := kVARatings[i] /
Fnphases / Vfactor` — the spurious `1.1 *` DROPPED (`Transformer.pas:1058`,
commit `4ed59416`, SVN r4033). The separate `NormMaxHkVA = 1.1 * Winding[1].kVA`
(the 110% default norm rating) is a DIFFERENT quantity, unchanged upstream.

**Decision — adopt** (`transformer/yterminal.rs`, `1.1 * r` → `r`).

**Gate consequence.**
- **No live/golden witness in the current port.** `amp_ratings` is consumed
  ONLY by the seasonal overload-report override, which is `NOT_PORTED`
  (`report/export/capacity.rs`, `solution/meters/demand_interval.rs` — WPG.11
  deferred to WP-U1.5 E2); the non-seasonal overload reports use `norm_amps`
  (which keeps its own 1.1 via `norm_max_hkva`), and the `Ratings` property
  readback returns `kVARatings`, not `AmpRatings`. So no corpus_live case and no
  report golden moves. (U0.2 audit already flagged D6 as report-only and
  unwitnessable by `ab_compare` — needs a synthesized seasonal overload deck,
  which WP-U1.5 owns.)
- **Pinned by a feature-sensitive unit test**
  (`transformer::tests::seasonal_amp_ratings_drop_the_1_1_factor`): with
  `kVARatings=[1000 1200]` it asserts `amp_ratings[i] == kVARatings[i] ·
  norm_amps / norm_max_hkva` (reconstructing `np·vfactor` from the unchanged
  NormAmps relation) — pre-D6 each value was 1.1× this, so the assertion flips.
- `known_diffs`: none matched — nothing to retire. **When WP-U1.5 ports the
  seasonal override, its overload-report deck becomes D6's live witness.**

## L1 — InvControl `InvControlDeltaV` per-control 2-slot buffer — SETTLED (WP-U1.3, adopt the fix; r4133 keeps the 9-year bug)

**Observable.** The `voltagechangesolution` (the change in the DER's per-unit
solution voltage across control passes) that drives the volt-var hysteresis
curve-1/curve-2 state machine, when a circuit has **two or more InvControl
objects** (each controlling a DER with a non-zero `hysteresis_offset`).

**dss_capi 0.15.x (capi015).** `FVpuSolution` is a per-control 2-slot buffer,
`FVpuSolutionIdx` init `-1`, toggled `0↔1` **unconditionally** once per
`UpdateInvControl` pass (`InvControl.pas` l.2524-2531, default `CompatFlags &
InvControlDeltaV == 0`). Every InvControl advances its own cursor, so
`voltagechangesolution = present − prior` is correct for all controls.

**EPRI r4133 (and 0.14.5).** A 9-year-old bug: `TInvControl.UpdateAll`
(l.3563-3575) calls `obj.UpdateInvControl(i)` with `i` = the InvControl's
**element-list index**, and the cursor bump is gated on `(j=1) and (i=1)`
(l.2537). So **only the FIRST InvControl (i=1) ever advances its cursor**;
every other control's `FVpuSolutionIdx` stays `0`, its buffer read matches
neither `idx=1` nor `idx=2`, and `voltagechangesolution` latches at 0 — its
hysteresis never sees the voltage move. `InvControlDeltaV=0x100` restores this
bug on capi015 (it is dss_capi's default-OFF compat flag).

**Probe** (`scratch_probe_l1.py`, 2026-07-12; a 2-bus feeder, one InvControl +
wye PVSystem per bus, VOLTVAR `hysteresis_offset=-0.05`, daily voltage swing):

| step | capi015 pv2 kvar | r4133 pv2 kvar |
|---|---|---|
| 1 | −14.013 | −14.013 |
| 2 | −14.013 (settled) | −9.200 |
| 3 | −14.013 | −12.816 |
| 4 | −14.013 | −9.070 |
| 5 | −14.013 | −12.142 |

pv1 (the FIRST control, i=1) is bit-identical on both engines; **pv2 (the second
control) OSCILLATES on r4133** (its `voltagechangesolution` is stuck at 0) but
settles on capi015. A ~kvar-scale divergence, far above any floor.

**Decision — adopt the capi015 fix; catalog the EPRI r4133 behavior as a
known upstream bug (a report entry, NOT a gate).** The Rust r3723 port already
computed `voltagechangesolution` the fixed way (it toggles each InvControl
object's own cursor unconditionally in `update_all_inv_controls` — it never had
the `i=1` element-list gating, which the per-element control dispatch cannot
represent). This WP makes the **buffer form** faithful to the 0.15.x source:
`f_vpu_solution: [f64; 2]` (was `[f64; 3]`), `f_vpu_solution_idx` init `-1` (was
`0`), toggle `0↔1` (was `1↔2`), read `fv[0]-fv[1]`/`fv[1]-fv[0]`
(`compute.rs::update_inv_control` + `calc_qvv_curve_desiredpu`). This refactor
is **numerically identical** to the pre-refactor Rust for `voltagechangesolution`
(proven by a slot relabelling: idx {1,2}↔{0,1}) — so no existing InvControl deck
or golden moves (the whole inv_control unit suite + `der_controls` goldens stay
green).

**Gate consequence.**
- **No live multi-step gate is possible for the D1 divergence.** It needs a
  time-series (the hysteresis history), and Rust matches ONLY capi015 (0.14.5 and
  r4133 share the bug). But the **capi015 oracle cannot gate a multi-step deck**:
  its `run_case` capture (`capture_fingerprint`/`getYSparse` via the fastdss
  `_oddie_get_y_sparse` rebind, plus the element capture) **re-nominalizes
  time-varying elements**, so every step of a daily/loadshape deck reports the
  nominal (step-0) state — witnessed 2026-07-12 (`scratch_probe_srv3.py`:
  `dbl_hour` advances 1→8 but `Load.ld1`/PVSystem output stays constant on
  capi015, while the default 0.14.5 oracle steps correctly). All 18 pre-existing
  capi015 corpus cases are `n_steps=1` for this reason. Recorded in STATUS as an
  oracle-infra follow-up.
- **Pinned by the feature-sensitive Rust unit test**
  `inv_control::tests::dispatch::d1_buffer_2slot_tracks_last_two_pu_voltages`
  (drives `update_inv_control` three passes with changing monitored voltages;
  asserts `idx` −1→0→1→0 and the 2-slot buffer holds the last two per-unit
  voltages) — flips if the buffer/init/toggle regresses.
- **New snapshot capi015 deck** `controls/inv_control/invcontrol_multi_vv_wye.dss`
  (`oracle: "capi015"`, `n_steps=1`) gates the multi-InvControl fleet build +
  independent per-control dispatch that the fix operates over (the hysteresis
  history itself is snapshot-invisible — `voltagechangesolution=0` at hour 1).
- `known_diffs.json`: no Rust↔EPRI entry existed (the r3723 port already matched
  the fix; it diverged from 0.14.5/r4133 all along on this path but no deck
  witnessed it). Adopting the buffer form changes nothing observable. The r4133
  bug is now catalogued here (this row) per the CLAUDE.md known-bugs discipline.

## D2 — InvControl per-DER base-voltage cross-leak — SETTLED (WP-U1.3, adopt capi015)

**Observable.** In `UpdateInvControl`, the base voltage used to renormalize a
DER's `MonBus`-monitored voltage into the rolling-average / `FVpuSolution`
history — for an InvControl controlling **multiple DERs on different base
voltages** with an explicit `MonBus`.

**dss_capi 0.14.5.** `BasekV := CtrlVars[i].FVBase` where `i` is the
InvControl's element-list index (its own `//TODO: check (i, j)`), so for a
single control it collapses to `CtrlVars[1]` = the **first** DER's base for
**every** DER. **0.15.x (capi015) / r4133.** `BasekV := FVBase` inside `with
CtrlVars[j]` — the **per-DER** base (`InvControl.pas` l.2550).

**Decision — adopt** (`compute.rs::update_inv_control`, `ctrl_vars[0].f_vbase →
ctrl_vars[j].f_vbase`). A no-op for a homogeneous-base fleet or a non-`MonBus`
control (the self-monitoring path ignores `BasekV`), so no existing deck moves
(every corpus InvControl is self-monitored, 0 `MonBus` uses — scanned).

**Gate consequence.** No corpus/live witness (needs `MonBus` + a
heterogeneous-base fleet). Pinned by the feature-sensitive unit test
`d2_update_uses_per_der_basekv_not_first_der` (two DERs on 7200/4160 V bases
monitoring a 4160 V bus: DER#2 reads 4160 with the fix, 7200 without).
`known_diffs`: nothing to retire.

## D3 — InvControl watt-priority `Sqrt(kVA²−kW²)` guard — SETTLED (WP-U1.3, adopt capi015)

**Observable.** `Q_Ppriority` in `Check_Qlimits`'s watt-priority arm
(`FPPriority && (VARMAX || WATTPF)`): a near-cancellation `SQR(kVArating) −
SQR(presentkW)` that can go tiny-**negative** in f64 when `kW ≈ kVA`.

**dss_capi 0.14.5.** `Q_Ppriority := Sqrt(SQR(FkVArating) − SQR(FpresentkW)) /
QHeadRoom` — `Sqrt(<0) = NaN`. **0.15.x (capi015) / r4133.** Guard first
(`InvControl.pas` l.3442-3446, r4056): `Qavailable_sqr := SQR(..)−SQR(..); if
abs(Qavailable_sqr) < epsilon then Qavailable_sqr := 0.0;` then `Sqrt`.
`epsilon = DSSGlobals.EPSILON = 1e-12`.

**Decision — adopt** (`compute.rs::check_qlimits`: the `qavailable_sqr` guard
with `crate::util::EPSILON`). The same identifier also corrected the
neighbouring `|Q_Ppriority| < epsilon` guard, which the r3723 port had
mis-mapped to `f64::EPSILON` (~2.2e-16) — now `1e-12` (a faithfulness fix in the
same hunk, cited at the site).

**Gate consequence.** No corpus witness (the guarded band is a sub-ULP
near-cancellation). Pinned by `d3_watt_priority_sqrt_guard_zeroes_tiny_negative_radicand`
(`kVA=10`, `kW = 10 + 1 ULP` → radicand ~−3.6e-14, `|.|<1e-12`: the guard yields
`Q_Ppriority=0` → `QDesireLimitedpu=0`; without it, `NaN` skips the clamp block
and leaves the unclamped `1.0`). `known_diffs`: nothing to retire.

## D4 — InvControl delta-DER monitored voltage is line-to-line — SETTLED (WP-U1.3, adopt capi015 = r4133)

**Observable.** `GetMonVoltage`'s self-monitoring path for a **delta-connected**
controlled DER (PVSystem/Storage `conn=delta`).

**dss_capi 0.14.5.** `cBuffer[j] := DERElem.Vterminal[j]` — the line-neutral
magnitudes, even for a delta DER. **0.15.x (capi015) / r4133.** `case
DERElem.Connection of Delta: cBuffer[j] := Vterminal[j] −
Vterminal[NextDeltaPhase(j)]` — **line-to-line** (capi015 `InvControl.pas`
l.1647-1652, r3822; **re-verified in EPRI r4133 `InvControl.pas:2184`** — the
line-cite label above is capi015's, the substance is r4133's;
`NextDeltaPhase(iphs)=iphs+1`, wraps to 1 past `NCondsDER`).

**Probe** (`scratch_probe_d4.py`, 2026-07-12; one delta PVSystem, VOLTVAR):

| engine | delta PV Q |
|---|---|
| capi 0.14.5 (LN) | **−31.11 kvar** |
| capi015 (LL) | **+520.28 kvar** |
| oddie r4133 (LL) | **+520.28 kvar** |

The LL-vs-LN per-unit gap flips the var **sign** — decisive. capi015 == r4133
(a clean adoption aligned with the plan's end-target).

**Decision — adopt** (`compute.rs::get_mon_voltage`: build the per-node complex
`cBuffer` and, for a delta DER, take the LL difference before reducing by
`MonVoltageCalc`; new env methods `der_vterminal` (complex phasors) +
`der_is_delta`).

**Gate consequence.**
- **New snapshot capi015 deck** `controls/inv_control/invcontrol_vv_delta.dss`
  (`oracle: "capi015"`, `n_steps=1`): a delta PVSystem under VOLTVAR; the LL
  monitored voltage is a snapshot property (not time-dependent), so one solve is
  a decisive feature-sensitive gate — a regression to the wye (LN) reading flips
  pv1's kvar sign and diverges from capi015.
- **Also unit-pinned** by `d4_delta_der_monitors_line_to_line_voltage` (a
  MockEnv delta DER returns a balanced 120°-spaced phasor set → the LL path
  yields √3·pu; a regression to LN lands at 1.0 pu) and its wye control
  `d4_wye_der_monitors_line_neutral_voltage` (offline coverage of the branch, so
  D4 does not rest solely on Oddie-venv availability at gate time).
- **Two default-oracle corpus decks provably moved and are handled in-commit:**
  `midi_controls.dss` and `midi_invcontrol.dss` each carried a delta `pvsystem.pv3`
  under an auto-populated InvControl. These are **multi-step** decks whose other
  controls (CapControl/RegControl/StorageController) carry unported U1.5/U1.6
  deltas, so they **cannot** flip to capi015 (which would entangle unported
  behavior) — and the capi015 oracle cannot gate a multi-step deck anyway (L1
  above). `pv3` is changed to `conn=wye` in both (a documented one-token edit at
  the site), moving the delta-DER-under-InvControl coverage to the dedicated
  `invcontrol_vv_delta` deck while the midi decks stay 0.14.5-gated for their
  combo content. `gfm_invcontrol` (delta Storage under `mode=GFM`) is **not**
  affected — the GFM arm never reads the monitored voltage (verified).
- `known_diffs`: no prior Rust↔EPRI entry (0.14.5 and the r3723 port both used
  LN); adopting LL makes Rust match r4133 — nothing to retire.

## D5 — InvControl9611 — SETTLED (WP-U1.3, not a delta for us)

The `InvControl9611` compat flag (the 9.6.1.1 volt-var regression) exists in
**both** 0.14.5 and 0.15.x, **OFF by default in both** — the fixed behavior is
the default. The r3723 port already reproduced the fixed side: `update_deltaq_factor`
(`compute.rs`) is `if delta_q_factor == FLAGDELTAQ { change_deltaq_factor(j) }
else { f_delta_q_factor = delta_q_factor }` — exactly the `CompatFlags &
InvControl9611 == 0` branch (`InvControl.pas` l.2687-2688 / 2728-2729 etc.). The
0.14.5↔0.15.x branch text is byte-identical (grep-verified). **No code change;
not an observable delta.** Mirrors the D5/D8 "not a delta for us" records.

## C8 — InvControl surface: `VV_RefReactivePower` removal + MonBus validations — SETTLED (WP-U1.3)

**(a) `VV_RefReactivePower` removal — NOT adopted (kept, r4133-aligned).**
dss_capi 0.15.x fully removed the property (`c44e5873`); capi015 rejects
`VV_RefReactivePower=…` with `#110 Unknown parameter` and reports 36 properties.
But **EPRI r4133 KEEPS it** as a real property (probe `scratch_probe_c8.py`,
2026-07-12: r4133 accepts the write, readback `varmax`, 37 properties), matching
0.14.5 (which logs a `#2020030` deprecation, readback `''`, 37 properties). Per
the plan's §1.4 default (**r4133 wins**), the port **keeps** `VV_RefReactivePower`
(the existing `DeprecatedAndRemoved`-style placeholder that renders `''`,
aligned with r4133 AND 0.14.5). Adopting capi015's removal would drop the InvControl
property count 37→36 and, via the controls-family `compare_all_properties` (which
demands Rust count == oracle count), **force every default-oracle InvControl deck
to flip to capi015** — including the combo/midi decks whose CapControl/RegControl/
StorageController carry **unported U1.5/U1.6 deltas** (and which the multi-step
capi015 oracle cannot gate). That is out of proportion for the InvControl WP and
would entangle other WPs' behavior; the capi015-only removal is recorded here as
a **divergence NOT adopted** (a Rung-1 dss-ext API cleanup, not a bug), a
candidate for a coordinated flip once U1.5/U1.6 land. No deck moves; the
`props/invcontrol.json` golden keeps its `VV_RefReactivePower` line.

**(b) MonBus validation errors — adopted.** Two new capi015 guards on malformed
`MonBus` input (probe `scratch_probe_c8.py`):
- **#2024111** (`InvControl.pas` l.694-698, `e6607efd`): a `MonBus` entry with
  no node numbers (`MonBus=(bus)` without `.node`) is a hard parse error that
  aborts the side effect. Ported in `accessors.rs::side_effects` (MonBus arm) —
  capi015 emits it, 0.14.5/r4133 silently accept.
- **#2024112** (`InvControl.pas` l.1596-1601): a `MonBus` name that never
  resolves to a real bus aborts the solve at `GetMonVoltage` (instead of silently
  reading the ground node's zero voltage — the previous Rust behavior). Ported
  via `get_mon_voltage` + the new `mon_bus_unresolved`/`request_solution_abort`
  env hooks; capi015 emits the specific message, 0.14.5/r4133 abort with the
  generic `#482`.

These are capi015-specific messages (they diverge from r4133's silent-accept /
generic-abort), but they only fire on **malformed** input, never in a valid deck;
the *sequence* (both engines abort on an invalid bus) is preserved. (Corpus note,
0.15.x-adoption sweep: `controls/invcontrol/invcontrol_monbus.dss` DOES use a
`MonBus` — a single-DER, **valid** deck — so the earlier "0 corpus decks use
MonBus" claim is stale; but its input is well-formed, so it exercises neither the
#2024111 nor the #2024112 malformed-input guard, and no observable is witnessed on
either channel by that deck.) Pinned by the feature-sensitive unit tests
`c8_monbus_missing_nodes_errors_2024111` and `c8_monbus_invalid_bus_aborts_2024112`.
`known_diffs`: nothing to retire.

## D14 — DynamicExp RPN evaluator "index-bug fix" (a no-op evaluator) — REVERTED (BUG WP DynExp; NOT adopted — both gating oracles integrate)

> **REVERTED 2026-07-19 (BUG WP DynExp).** The earlier "adopt capi015" decision
> below was a mistake: it pointed the port at the retired **non-gating** capi015
> (dss_capi 0.15.x) reference, and the port then matched **neither** surviving
> gating oracle. Both gating channels — the pinned **dss_capi 0.14.5** backend
> AND **EPRI r4133** — run the *full* evaluator and integrate (the DynExp rotor
> swings). `solve_eq` is restored to that full evaluator; the historical analysis
> is kept below for context.

**Observable.** `TDynamicExpObj.SolveEq` — the per-step evaluator that a
`DynEqPCE` (Generator / PVSystem / Storage / WindGen with `DynamicEq=`) calls
each dynamics substep to compute the state-variable derivatives. The result is
visible in the mode-3 monitor DynamicExp slots and, downstream, in the whole
dynamics trajectory (rotor swing, inverter current ramp) and the solved node
voltages a DynExp-driven element injects.

**Both gating oracles (pinned dss_capi 0.14.5 `SolveEq`, `DynamicExp.pas:377`;
EPRI r4133 `SolveEq`, `DynamicExp.pas:497`) — byte-identical in structure.**
`SolveEq` walks the compiled `Cmds` automation array (`for idx := 0 to
High(Cmds)`), evaluating the RPN right-hand side and writing each output's
derivative into `MemSpace[OutIdx][1]`. (It reads `Cmds[idx+1]` one past the end
at the final index — a benign OOB read that happens not to alias `-50`; the port
reproduces it with the safe `cmds.get(idx+1)`.) The rotor genuinely swings.

**dss_capi 0.15.x (retired capi015) / upstream `2a8bdb78` ("DynamicExp: reuse
RPN, fix index bug") — NOT a gating oracle.** Two coupled edits: the loop bound
becomes `High(Cmds) - 1`, **and an `Exit` is added right after the first
equation's output index is latched**. A well-formed compiled stream always
starts `[outIdx, -50, ...]`, so the loop hits that marker at idx 0 and returns
immediately — the RHS is never evaluated, `SolveEq` is a no-op, and the state
variable freezes at its `InitStateVars` seed. This is an upstream **regression**
introduced by the "fix"; it survives only in the 0.15.x line, which the project
does not gate against. The port must NOT adopt it.

**First-divergence measurement (BUG WP DynExp, 2026-07-19).** Kundur DynExp deck,
one dynamics substep from the seeded operating point:
- both oracles (0.14.5 and r4133, agree): `dspeed` = **-1.6169543e-6** (=
  -1/Mass·(Pterm-Pshaft) = -66.66/41.22e6), `speed` = -8.085e-10, `theta`
  = 0.72907153 → then swings; by the deck's 5 s endpoint `theta` = 2.036 rad,
  `speed` = 0.626 (the two oracles' node V agree to ~4.8e-10).
- the D14 no-op port: `dspeed` = **0** exactly (slot never written), `speed` = 0,
  `theta` frozen at 0.72907156 for the whole run.
- the reverted (full-evaluator) port: `dspeed` = -1.6169543e-6, matching both
  oracles to the f32 monitor floor.
The frozen derivative compounds through the trapezoidal integrator: by step 5000
the port's node voltages diverge catastrophically at the deep nodes (HT.1: oracle
122713 V @ 69.1° vs D14-frozen 193725 V @ 24.8°), while the near-invariant
quasi-ideal source bus barely moves (~2.6 V, 1.5e-5 — the misleadingly small
"entry 0" the first re-measurement latched onto).

**Decision — REVERT; port the full 0.14.5/r4133 evaluator** (`dynamic_exp.rs::solve_eq`:
full `0..cmds.len()` loop, safe `cmds.get(idx+1)` for the benign OOB read, no
early return, final upload after the loop; `get_out_idx` restored to the guarded
`0..cmds.len()` form). Cited to `DynamicExp.pas:377` (0.14.5) / `:497` (r4133).

**Gate consequence.**
- **`Dynamic_KundurDynExp.dss`** gates on **both** channels at the feeder tier
  floor (no ledger entry): the port swings and matches both oracles (rotor
  `theta` 2.036 rad / `speed` 0.626). Its `-steady-state-only` sibling stays on
  `both` (never enters dynamics).
- **`Run_IEEE123Bus_GFLDaily_DynExp.DSS`** gates on **r4133** (the DynExp state
  now integrates and matches r4133 at the floor); it stays off `capi_v0145` for
  the *separate* PVSystem-dynamics reason (D7, #d7) that also keeps its non-DynExp
  sibling on r4133 — unrelated to `SolveEq`.
- **The `exec/tests/dynamics.rs` DynExp gates + the `dynamic_exp` unit tests** are
  restored to their pre-D14 swinging-oracle pins (generator mode3/fault/swing;
  PVSystem mode3/safe-fault; Storage mode3/trip-fault; the `SolveEq` RPN-evaluator
  unit tests). They now guard against a *re*-introduction of the no-op.
- The pre-staged `dynexp-d14` ledger cause (which anticipated an r4133 envelope)
  is **removed** — no envelope is needed; the port matches both oracles at the
  floor.
- `known_diffs`: none matched — nothing to retire.

## A3/A5 — PCE force hooks (`Set`/`Get` InjCurrent/ITerminal/YPrim/StateVar/…) — SETTLED (WP-U1.9, adopt capi015)

**Observable.** The `Set`/`Get` options `InjCurrent`/`ITerminal`/`YPrim`/
`StateVar`/`IterNumber`/`CtrlIterNumber`/`IntegrationFlag` and the element flags
`Flg.ForceInjCurrents`/`Flg.ForceYPrim` — the pyControl co-simulation engine
hooks (the `pyControl` component + `Set PyPath=` stay `NOT_PORTED`, §0).

**Source-tree note (corrected by the 0.15.x-adoption sweep — the hooks EXIST in
EPRI).** The force hooks originated at **EPRI**: EPRI r4088 AND r4133 Version8
carry them (`ExecOptions.pas` `ExecOption[141..148]` StateVar/pyPath/IterNumber/
CtrlIterNumber/InjCurrent/ITerminal/Yprim/IntegrationFlag; `TPCElement.
ForceInjCurr`/`ForceY`; honored in `Ymatrix.pas` ReCalc skips and in each of the
six flag-checking PCE `InjCurrents`/`GetTerminalCurrents`), and the **gating
r4133 DLL is built from Version8**, so the hooks are live in the r4133 channel —
`modes/upgrade_forcehooks.dss` is manifest-gated `engines=r4133` and passes. They
are absent only from the vendored dss_capi **working tree** `.inputs/
dss_capi_with_git` (checked out at `master` `f5728aec`, a commit *after* `0.15.0b4`
where dss_capi *removed* them), so the capi015 spec was read via `git show
0.15.0b4:` (tag `e936d210`); the pinned `capi_v0145` oracle (backend 0.14.5)
predates the hooks and cannot gate them (`Set InjCurrent` → `#130 Unknown
parameter`) — `engines=r4133` is the correct channel. `delta_capi_0145_015x.md`
A3 also wrongly listed `SampleControlDevices` as a new hook — it is present
already in `0.14.5` (`Solution.pas:1974`) and was ported long ago
(`solution/controls/sampling.rs`); not a delta.

**Decision — adopt the behavior (confirmed against 0.15.0b4 AND EPRI r4088/r4133
AND a live r4133 probe).** `ElemFlags::FORCE_YPRIM`/`FORCE_INJ_CURRENTS`, honored
per the Pascal per-class shape — the `ForceInjCurr` check lives **inside** each
flag-checking class's `inj_currents` (`if not ForceInjCurr then
CalcInjCurrentArray`), skipping only the model recompute while the unconditional
set-nominal preamble and the inherited add-into-Currents still run
(`solution/solution/power_flow.rs::get_pc_inj_curr_filtered` just calls each
element's `inj_currents`) — and in `ReCalcAllYPrims` (`solution/ymatrix.rs` skips
`CalcYPrim` for a `ForceYPrim` element). The **six** flag-checking PCE
`GetTerminalCurrents`/`InjCurrents` skip the recompute when forced (Load/
Generator/PVsystem/Storage/IndMach012/**WindGen**); the non-flag-checking
overrides VCCS/UPFC/VSConverter recompute unconditionally, so a force on those is
inert on both the port and both oracles (0 `ForceInjCurr` hits in the r4133
sources). The 0.15.x-adoption sweep found and fixed three port fidelity gaps: the
WindGen `get_currents` guard was missing — the only one of the six flag-checking
classes without it — so its reported terminal currents recomputed at the forced
operating point where r4133 freezes. **Own r4133 probe** (epri-worker, weak
source + `set InjCurrent=[400 0 400 0 400 0]` + re-solve, `ActiveCktElement.
Currents`): r4133 reports the frozen `[-89.231224, -11.202775, 34.913725,
82.877894, 54.317500, -71.675120]`; the port matches it to a faer-vs-KLU floor
with the guard, but recomputes `[-86.039, -58.979, …]` (imag −11.20 → −58.98,
a ≈48 A miss) without it — pinned + guard-toggle-verified by
`windgen_force_inj_freezes_iterminal` (reads the currents through the corpus
`snapshot_elements`/`GetCurrents` path, not the guard-blind `Get ITerminal`
cache). An earlier central force-branch in the injection loop dropped the
set-nominal preamble (a forced element's Yeq would freeze in a varying-loadshape
time series), and it honored the flag for every PCE (freezing VCCS/UPFC/
VSConverter where both oracles recompute). The option set/get is in `exec/set_cmd.rs`/
`exec/get_cmd.rs`; the parser gained `make_complex`/`parse_as_complex_vector`/
`parse_as_complex_matrix` (`ParserDel.pas`). `Set IterNumber`/`CtrlIterNumber`/
`IntegrationFlag` are read-only (error 25040103); `Set PyPath=` is a loud
NOT_PORTED.

**Audit follow-ups (WP-U1.9, verified against the capi015 0.15.0b4 oracle).**
- **`Set StateVar` via text is upstream-broken — reproduced as an error, not a
  write.** `DoSetCmd` matches bare tokens *positionally* (only `name=value`
  pairs are looked up by name — `ExecOptions.pas:247-255`), so
  `set StateVar generator.g1 Frequency 55` never reaches the StateVar arm: the
  tokens land on options 1/3/4 and the integer `hour` option rejects
  `Frequency` (capi015 `#303`; the port errors the same way and leaves the
  variable unchanged). `Get StateVar` **does** work (`DoGetCmd` name-matches
  every token — `:951-957`) and is the pinned/read path. The earlier
  "Set/Get StateVar covered" claim was corrected to reflect this: the unit
  suite now pins the natural-syntax error + the read path + both 7103 guards.
- **7103 `is TPCElement` guard.** `Set/Get StateVar` on a non-PCE now errors
  `Object "<Class>.<name>" is not a valid PC element.` (Pascal 7103, checked
  *before* the NumVariables 7101 check), matching capi015
  (`get StateVar line.ln Frequency` → `#7103`).
- **`Set/Get AllowForms`/`AllowProgressBar`** are accepted headless no-ops
  (`NoFormsAllowed`/`NoProgressBarFormAllowed` stored for `Set`/`Get`
  round-trip, unread; default `No`). capi015 silently accepts them — the
  pre-fix "not ported yet" error diverged.
- **Force-hook error arms `Exit`** the whole `Set`/`Get` command in Pascal
  (`DoSimpleMsg(...); Exit`); the port now breaks the option loop on those
  errors instead of continuing.
- **`Set YPrim` size-mismatch is only the oversize-row case.** capi015's
  `ParseAsComplexMatrix` returns `ExpectedOrder` (accept) for a too-*few*-row
  matrix (`[5 0 | 0 5]` on a 4-cond PCE → zero-padded, no error, both engines);
  the `#3004` size-mismatch fires only when a single row exceeds `NConds²`
  (>16). **Not reproduced (deliberate):** on that `#3004` capi015 has already
  zeroed the live YPrim (its own source comments this is a known error-state
  imperfection — "we'd need to keep a copy of the old matrix"), whereas the
  port parses into a scratch buffer and leaves the real YPrim intact. The
  difference is transient — neither engine sets `ForceYPrim` on the error, so
  the next `ReCalcAllYPrims` recomputes the matrix on both — and it is an
  error state, so the safer preserve-on-error is kept.

**Gate consequence.**
- **Live capi015 deck** `modes/upgrade_forcehooks.dss` (`oracle: "capi015"`,
  validated bit-identical across two capi015 processes): `Set InjCurrent=[80 0
  80 0 80 0]` on the b2 load shifts b2 Vmag 7187.45 → 7224.14 V; whole-model
  live compare green. Feature-sensitive (a broken injection-loop honor → 7187 vs
  capi015 7224, ≫ floor).
- **capi015-pinned Rust unit suite** `exec/tests/force_hooks.rs` (13 tests):
  forced Vmag 7224.143523 (1e-6), frozen `Get InjCurrent`/`ITerminal` (Load) +
  frozen Generator `ITerminal` (2nd PCE force-skip) + frozen WindGen `ITerminal`
  (`windgen_force_inj_freezes_iterminal`, the 6th flag-checking class the sweep
  fixed), `Set ITerminal` freeze
  (base 23.175527 → forced `[10,0,10]`), `Get IterNumber`/`IntegrationFlag`,
  read-only `Set` (aborts the loop), `Set PyPath` NOT_PORTED, `Set YPrim`
  survives a rebuild + oversize-row `#3004`, `Get StateVar` read + natural-syntax
  `Set StateVar` upstream-broken error + 7103 non-PCE guard, `AllowForms`/
  `AllowProgressBar` round-trip, and `Clear` resets the force flags.
- `known_diffs.json`: no Rust↔EPRI entry existed for the force hooks at r3723
  (the options did not exist) — nothing to retire.

## B3/C1 — LineSpacing equivalent-spacing model — SETTLED (WP-U1.4, adopt capi015; default-off, no golden movement)

**Observable.** The series `Z` / shunt `Yc` of a Line whose `LineGeometry`
references a `LineSpacing` with `Detailed=No` (`EquivalentSpacing = not detailed`):
the four equivalent distances `EqDistPhPh`/`EqDistPhN`/`AvgPhaseHeight`/
`AvgNeutralHeight` replace the per-conductor `X`/`H` coordinates in the Carson
`D_ij` / image-distance / earth-return terms.

**dss_capi 0.15.x (capi015) / EPRI r4088+.** New `LineSpacing` properties
(`LineSpacing.pas`): `Detailed` (bool, default `true`), `EqDistPhPh`, `EqDistPhN`,
`AvgPhaseHeight`, `AvgNeutralHeight` (double, default `0.0`). `TLineConstants`
(`LineConstants.pas`, SVN r3913-era) grows an `equivalentSpacing` flag + the four
distances; `Calc`/`GetZearth`/`ConductorsInSameSpace` branch on it. `LineGeometry`
copies the spacing's equivalent state (`UpdateLineGeometryData` converts the
distances to meters via `To_Meters(FLastUnit)`), and skips `SetX`/`SetY`.

**Decision — adopt (default-off preserves 0.14.5 numerics).** `Detailed=true` ⇒
`equivalentSpacing=false` ⇒ the legacy per-conductor-coordinate path, bit-identical
to 0.14.5 (`eps_r_medium=1.0` ⇒ `E0*1.0 == E0` exactly; `height_offset=0.0`).
Ported 1:1 in `support/line_constants/mod.rs` (the `equivalent_spacing` branches in
`calc_overhead`/`get_ze`/`cisp_overhead`, plus `set_equivalent_spacing`/
`set_equivalent_distances`/`set_eps_r_medium`/`set_height_offset` setters),
`elements/general/line_spacing/mod.rs` (the five props + `equivalent_spacing()`
accessor + the `Detailed` prop-tracking side effect), and the
`elements/general/line_geometry` handoff (`apply_spacing`/`load_spacing_and_wires`/
`update_line_geometry_data`).

**Probe / gate.** capi015 (0.15.0b4), a 4-conductor (3φ+N) overhead line under an
equivalent-spacing spacing, DERI, reduce=y, ohms/mi (probe 2026-07-16): reduced
`rmatrix` diag `0.410565535096229` / off `0.109545787814619`, `xmatrix` diag
`0.988334662246234` / off `0.426587892673967`. Rust matches to 1e-8 rel
(`line_geometry::tests::matrices_equivalent_spacing_match_capi015`).
- **No existing golden/live case moves** (`Detailed` defaults true; no corpus deck
  sets the equivalent-spacing props — scanned). The whole `line_constants`/
  `line_geometry`/`line` suites + `props_roundtrip` stay green untouched.
- **New capi015 corpus deck** `modes/upgrade/upgrade_linecs_eqspacing.dss`
  (`oracle: "capi015"`, YPrim-focused on `line.l1`; §1.7-validated: converges in 2
  iters, bit-identical fingerprint across two capi015 processes).
- **New capi015 props golden** `tests/golden/props/linespacing_eqspacing.json`
  (per-file `engine_spec: capi015` provenance) pins the property surface
  (`Detailed`/`EqDistPhPh`/… readbacks).
- `known_diffs`: no Rust↔EPRI entry existed (0.14.5 had no equivalent-spacing
  path); adopting it makes Rust match r4133 for the new surface — nothing to retire.

**WP-U1.4 property tail — LANDED (wt-u14props, adopt capi015).** `Line.EpsRMedium`/
`HeightOffset`/`HeightUnit` Line-level props wired to the engine fields (the
`compare_all_properties` block is now resolved by the `PROPS_015X` allowlist landed
on wt-h015 — 0.15.x-only trailing props are excluded from the 0.14.5 shape check);
`LineCode` FaultRate/PctPerm/Repair deprecation (schema-metadata Deprecated/Unused
flags, no runtime observable); **LineType enum width 4->5** (5-char `swt_*`
abbreviations now disambiguate instead of falling back to `oh`; capi015 deck +
unit test, the 0.14.5 oracle renders them `oh`); **WP-U1.2 D3 spacing ratings**
(min over phase conductors, not conductor 1 — `line_spacing_asym` flipped to
capi015, YPrim bit-identical). See STATUS §WP-U1.4 property tail.

**WP-U1.4 is now COMPLETE (wt-u14cond).** The last row, the `Line.Conductors` /
`LineGeometry.Conductors` *property* (Line prop 34, LineGeometry prop 20; the
3-class `(WireData|CNData|TSData)` proxy), is ported — see
§"Line/LineGeometry Conductors (text upstream-broken)" below and STATUS
§WP-U1.4 (wt-u14cond).

## Merged TCableConstants (per-conductor CN/TS) + CNData.SemiconLayer — SETTLED (WP-U1.4, adopt capi015; pure paths byte-green)

**Observable.** The series `Z` / shunt `Yc` of a Line whose `LineGeometry`
carries **mixed** conductor kinds (CN and TS cables plus bare wires on one
geometry), and the shunt capacitance of a CN cable with `SemiconLayer=no`.

**dss_capi 0.15.x (capi015) / EPRI r4088+.** The separate
`TCNLineConstants`/`TTSLineConstants` classes are **merged** into a single
`TCableConstants` (`CableConstants.pas`): the CN-vs-TS choice moves from the
engine kind to a per-conductor `FCondType[i]` (`SetCondType(i, CN|TS)`), so one
engine carries mixed conductors (Kersting mixed-conductor model). `Calc` branches
per conductor on `FCondType`; the mutual/`ConductorsInSameSpace` distance uses a
new `GetDij` helper (equivalent-spacing aware). `CNData` gains `SemiconLayer`
(prop 5, LongBool default `true`): `true` = the classic `Denom = ln(RadOut/RadIn)`
coaxial capacitance; `false` = the Synergi / Kersting no-semicon formula
`Denom = ln(RadCN/RadIn) - (1/k)·ln(k·RadStrand/RadCN)`.

**Decision — adopt (default preserves 0.14.5 numerics).** A pure-CN (all
`FCondType=CN`) or pure-TS geometry reproduces the former class `Calc`
bit-for-bit — the merged `Calc` restricted to one conductor type IS the old
`Calc`, and `GetDij` equals the old raw `sqrt` distance in the default
(non-equivalent-spacing) path; `SemiconLayer` defaults `true` = the old formula.
Ported in `support/line_constants/{mod,cable,cn,ts}.rs` (kind collapses to
`{Overhead, Cable}`, per-conductor `ConductorType` + `semicon_layer`,
`set_cond_type`/`set_semicon_layer`, merged `calc_cable`), the
`line_geometry` handoff (`change_line_constants_type` → one cable engine,
`update_line_geometry_data` sets `SetCondType`/`SetSemiconLayer` per conductor),
and `conductor_data/cn_data.rs` (`SemiconLayer` prop + field + make_like).

**Probe / gate.** capi015 (0.15.0b4), DERI, ohm/m and nF/m (probe 2026-07-16):
a mixed geometry (phase 1 CN, phase 2 TS, phase 3 CN, bare-wire neutral,
reduce=y) → reduced `Z` asymmetric (rmatrix diag `2.043e-4`/`1.649e-4`/`1.937e-4`
ohm/m), `C` diag `283.089` nF/km; a CN cable `SemiconLayer=no` → `C` diag
`167.168` nF/km (vs `283.089` for the default). Rust matches to 1e-8
(`line_constants::tests::cn_cable_no_semicon_capacitance`,
`line_geometry::tests::matrices_mixed_cn_ts_wire_match_capi015`).
- **No existing golden/live case moves** — the whole `line_constants`/
  `line_geometry` golden family + `props_roundtrip` stay green untouched; the
  merged `Calc` is bit-identical on every pure-CN/TS deck.
- **New capi015 corpus decks** `modes/upgrade/upgrade_linecs_mixed.dss`
  (mixed CN/TS/wire) and `modes/upgrade/upgrade_linecs_semicon.dss`
  (`SemiconLayer=no`), both `oracle: "capi015"`, YPrim-focused on `line.l1`,
  §1.7-validated (2 iters, two-process bit-identical fingerprint).
- **Harness allowlist** `PROPS_015X += ("CNData", ["SemiconLayer"])` — the
  inserted prop is excluded from the 0.14.5-oracle property-table walk.
- `known_diffs`: no Rust↔EPRI entry existed (0.14.5 had no mixed-conductor or
  semicon-branch path); adopting matches r4088+ for the new surface — nothing to
  retire.
- **`Line.Conductors` / `LineGeometry.Conductors` (the 0.15.x mixed-conductor
  *property*) — LANDED (wt-u14cond).** See §"Line/LineGeometry Conductors (text
  upstream-broken)" below.

## Line/LineGeometry Conductors — SETTLED (WP-U1.4 plumbing; text parse re-decided to r4133 by the 0.15.x-adoption sweep — capi015-broken, r4133-working)

**Observable.** The `Conductors` property — Line prop 34 (`Line.pas:62`),
LineGeometry prop 20 (`LineGeometry.pas:80`) — a mixed
`WireData|CNData|TSData` object-reference-array. Replaces `Spacing, Wires` with
`Spacing, Conductors` in the spacing spec-set; `Wires`/`CNCables`/`TSCables`
become `RedundantWith(Conductors)`.

**The text parse is capi015-BROKEN, EPRI r4133-WORKING (0.15.x-adoption sweep,
own probes).** dss_capi 0.15.x routed `conductors=` through a `TProxyClass` whose
`GetDSSClass` (`DSSClass.pas:2644`) compared the parser's `AnsiLowerCase`d class
token against the *original-case* `TargetClassNames` (`'WireData'`…) — never
satisfiable, so every class-prefixed item errored #10103 "Invalid class". EPRI
r4133 has NO `TProxyClass` (0 Pascal-source hits): it parses `conductors=`
NATIVELY (LineGeometry.pas prop 20 :410-540 / Line.pas prop 34) with a
CASE-INSENSITIVE `LowerCase(CondClass) = 'wiredata'/'cndata'/'tsdata'` dispatch
AND solves. The port reproduced the capi015 breakage; the sweep re-decided to
r4133 (CLAUDE.md 0.15.x rule). **Own r4133 probes (epri-worker):**
- Class-prefixed (any case) `Conductors=[WireData.w wiredata.w]` on a 2-wire/
  1-phase spacing → **converged**, Line.l1 I1=(21.801759, 0.027069); the port now
  resolves + solves, matching to a faer-vs-KLU floor.
- Bare item → r4133 rejects too (`dotpos = 0`: LineGeometry #10103, Line #181023);
  the port keeps its single generic-list #10103 for both (behaviour matches =
  reject; per-class code/wording is a cosmetic difference).
- `Conductors=` before the spacing → the port's clean #402; **r4133 #303 Access
  Violation** (UB) — NOT reproduced.
- All-`none` → **Line** parses on both (degenerate, non-converging); **LineGeometry**
  keeps the port's clean #10103 "At least one valid conductor" — **r4133 #303
  Access Violation** on that input (own probe), UB, NOT reproduced.
- `? <elem>.Conductors` (text getter) → #303 AV in capi015 — getter UB, NOT
  reproduced (safe name list).

**Decision — adopt r4133 case-insensitive class match; keep the JSON masquerade +
HIDE_015X.**
- `parse_conductor_proxy` (`obj/props/class_props/parse.rs`) now resolves a
  class-prefixed item by `eq_ignore_ascii_case` against the target class names
  (r4133 `LowerCase(CondClass)` dispatch), routing the resolved refs into the
  already-verified `set_conductors`/`apply_conductors` storage path; the NIL
  slots a `none` leaves are compacted out at solve time by `LoadSpacingAndWires`
  (see §AllowNoneItem). The reproduced capi015 `GetDSSClass` case-bug `TODO(compat)`
  is dropped. The bare-name #10103, the clean count-`<1` #402, and the not-reproduced
  #303 getter/all-none/before-spacing AVs stay.
- **JSON export is unchanged.** dss_capi already emits `"Conductors":[FullName…]`
  with each conductor's *actual* class; the Rust port has emitted the same bytes
  since wt-u14props via the `Line.Wires → "Conductors"` `json_name` masquerade
  (default set-order sweep). The real `Conductors` prop is added with `HIDE_015X`,
  so it is invisible to the byte-exact 0.14.5 Dump / FULL-JSON / `Dump commands`
  goldens (the catalog's running counter already skips `HIDE_015X`), and the
  `Wires` masquerade continues to own the `"Conductors"` JSON key. **The
  masquerade + HIDE_015X are retained deliberately** rather than flipping the
  Line/LineGeometry Dump/JSON golden surface to capi015: `gen_json.py` is
  hard-pinned to the 0.14.5 oracle (no capi015 engine switch, unlike
  `gen_bh_capi015.py`/`gen_regcontrol_capi015.py`), so flipping would require
  teaching the shared multi-element JSON generator the engine switch and
  re-verifying every captured element's FULL sweep — disproportionate for this
  tail row (UPGRADE_PLAN §1.4 fallback: "if the flip is disproportionate, keep
  HIDE_015X and document why"). Residual, latent, untested: the `Wires`
  masquerade renders a *mixed* conductor list (e.g. `cncables=cn1 wires=wn`) with
  a single `WireData.` prefix, where capi015 renders each conductor's real class;
  no golden/deck exercises a mixed-conductor Line's JSON, so this is inert until
  a later sweep flips the surface and drops the masquerade.

**Gate.** `PROPS_015X += ("Line", …+"Conductors")` and `("LineGeometry",
["Conductors"])` (the inserted props excluded from the 0.14.5 property-table
walk); `tests/upgrade_conductors.rs` now pins the r4133 semantics — a
class-prefixed item resolves + solves (`conductors_full_name_items_resolve_and_solve`,
pinned to the own r4133 probe currents), bare-name/#402/all-none-geometry stay
rejected (per-channel-scoped: the port's clean errors vs r4133's #181023/#303-AV).
The **resolved-ref** fill (the path the text parser AND a JSON-import round-trip
now take) is gated by whitebox
equivalence tests that drive `set_object_ref_array(CONDUCTORS)` + the side
effect directly — `line::tests::conductors_array_matches_buried_neutral_and_oracle`
/ `conductors_array_overhead_matches_wires_and_oracle` /
`conductors_all_none_after_wires_clears_wires_seq` (Line
`set_conductors`/`conductors_phase_choice`/`conductor_choice_of` + last-writer
`clear_seq`), and `line_geometry::tests::conductors_array_matches_mixed_capi015`
/ `conductors_array_defaults_ratings_from_first_valid` (LineGeometry
`apply_conductors`/per-conductor `change_line_constants_type`/`default_amps_from`),
each pinned to the same capi015 Z/Yc/ratings the traditional `wires=`/`cncables=`
paths pin. No solvable-corpus / byte-golden case moves (HIDE_015X + the masquerade
keep them byte-identical); whole workspace green.

## WP-U1.6 partial — B3-r3723 / D10 / D12 / D15 / A7-r3723 — SETTLED (plain adoptions)

These Rung-1 rows are straight adoptions of the 0.15.x (= r4088/r4133) behavior
with no new dss_capi↔EPRI divergence to catalog; detail in STATUS §WP-U1.6.

**Version note (settle 2026-07-16, audit-U1.6).** The vendored
`.inputs/dss_capi_with_git` **working tree** is checked out at `f5728aec`
(`0.14.6a1-8`, dated 2024-07-11) — which PREDATES the B3/D10 fixes, so a plain
`grep` of the checkout shows the *old* code and the fix commits carry a
re-vendor **commit-date** of 2026-02-16. Both are misleading: the fixes'
**author-dates** are 2025-06, and — decisively — `git merge-base --is-ancestor`
proves `d…`/`a14c3f1f`/`1b3123ce` are all **ancestors of tag `0.15.0b4`**, the
actual capi015 oracle backend (`ab_compare.py`: 0.15.0b4 = OpenDSS **SVN r4103**).
Read the target via `git -C .inputs/dss_capi_with_git show 0.15.0b4:src/…`, NOT
the working-tree checkout. **Both B3 and D10 therefore live in the capi015
oracle** and are directly oracle-validatable (probed below) — they do NOT
"diverge from both pinned oracles."

- **B3-r3723** `Load.GrowthFactor` Year=0 hourly progression — adopt (in capi015
  0.15.0b4 = r4103; `git show 0.15.0b4:src/PCElements/Load.pas` has the
  `calcYear := dblHour/8760` rewrite verbatim; 0.14.5 = flat 1.0). **capi015
  probe** (`/tmp/probe_b3*.py`, growthshape `year=(0,1,2) mult=(1.2,1.5,2.0)`,
  100 kW pf 0.9 load): snapshot Year=0 → **120 kW** (factor `GetMultIdx(1)=1.2`)
  at every hour (snapshot never advances `dblHour` past 8760); daily Year=0 with
  `dblHour≈8760` → **180 kW** (`GetMult(2)=1.8`), `≈17520` → 360, `≈8759` → 120.
  0.14.5 gives **100 kW** (flat 1.0) throughout. **Now deck-gated:**
  `modes/upgrade/upgrade_growth_year0.dss` (`oracle: "capi015"`, snapshot,
  feature-sensitive 120-vs-100 kW / B1 |V| 7198.16 vs 7198.40 V; §1.7 two-process
  determinism confirmed) — the whole-model live compare fails on any regression to
  the flat factor. Per-branch values also unit-pinned
  (`growth_factor_year0_tracks_simulated_hours_with_growthshape`).
- **D10** StorageController `FpctkWBandLow` typo (`a14c3f1f`) + first-iter
  `StorekWChanged` (`1b3123ce`) — adopt (SVN r4058, in capi015 0.15.0b4 = r4103;
  `git show 0.15.0b4:src/Controls/StorageController.pas:547` has
  `FpctkWBandLow := FkWBandLow/FkWTargetLow*100`). The typo fix retired a
  `TODO(compat)`. **capi015 probe** (`/tmp/probe_d10.py`,
  `kWTarget=300 kWTargetLow=100 kWBand=50 kWBandLow=20`): capi015 `%kWBand=16.667`
  / `%kWBandLow=20`; 0.14.5 `%kWBand=6.667` / `%kWBandLow=2` (the typo overwrites
  `%kWBand` from `kWBandLow/kWTarget` and never syncs `%kWBandLow`). Both halves
  **unit-pinned** and oracle-validated: `kw_band_low_side_effect_syncs_the_low_pct_pair`
  (property sync, 16.667/20 vs the typo's 6.667/2 discriminator) and
  `d10_discharge_transition_forces_resolve_on_first_iteration` (force-resolve).
  No corpus witness: the property sync is property-only (target-rev cases don't
  property-compare, §1.3-2) and the force-resolve needs a multi-step control-
  iteration run (capi015 re-nominalizes multi-step captures, L1 note) — so unit-
  test gating is the honest gate here (precedent: B1/D6/D7).
- **D12** SwtControl `Normal`/`State` field mapping (`bb9c9785`) — **RE-LANDED**
  (WP-U1.6 tail), then **superseded by the per-phase model** (R4133_PROPS RP3.7 —
  the settlement at the end of this row). `Normal`→`NormalState`, `State`→
  `PresentState`, `Action`→`CurrentAction` (distinct offsets,
  `SwtControl.pas:156-166`); the side effects sync `CurrentAction :=
  NormalState`/`PresentState` (were the reverse). All three were **scalar** fields
  until RP3.7 replaced the first two with r4133's per-phase arrays. 0.14.5
  mapped all three onto the single `CurrentAction`, so a write to any changed the
  others' readback and `State` reported the *armed* action rather than the live
  switch. The props golden `swtcontrol.json` is re-baselined to capi015 (probed
  2026-07-16: `Normal=''` default/lock, `State=Closed` after `action=/normal=`
  since the switch has not operated; `swtcontrol_locked_then_action` dropped — see
  below). **Entangled decks resolved** (the earlier revert's blocker): the moved
  `state`/`normal` readback moves three default-oracle SwtControl decks —
  `swtcontrol_time.dss` + `midi_swtcontrol.dss` **flipped to capi015** (the armed
  `action=open` opens the switch at its delay; the port, matching capi015
  `GetState`=live element, reads `State=Closed` until step 3 then `Open`; 0.14.5
  read `Open` from the arm). Multi-step capi015 capture is valid for these
  (constant loads / no loadshapes → the L1 re-nominalization touches only the
  getYSparse element-state re-read, NOT the per-step Monitor/probe/eventlog
  channels — verified 2026-07-16 by a stepwise capi015 probe). `civanlar.dss`
  (snapshot) **flipped to capi015**: `SwtControl.5_11` is `Action=c` then `edit
  action=o`, so D12/capi015 read `Normal=NormalState=closed` while 0.14.5 read the
  edited `CurrentAction=open`; the whole-model physics is capi015==0.14.5 (no
  loadshapes) so only the property readback moves and target-rev cases do not
  property-compare. `swtcontrol_lock.dss` **unaffected** (stays 0.14.5): the locked
  switch never operates, so both `State` and `Normal` read `closed` on every engine
  — and it *cannot* flip to capi015 anyway, since the strict PermissiveProperties
  read-only #2024106 rejects the locked `Action=` post-command there (the
  not-adopted L2/C2 dss-ext surface; the port silently ignores it, matching
  0.14.5/r4133 **for `Action=` and `State=`**, unit-pinned
  `locked_ignores_action_write` / `locked_ignores_normal_and_state_writes` — the
  latter renamed `locked_normal_applies_locked_state_and_action_do_not` by the
  RP3.7 settlement below).
  **Correction (2026-08-23, R4133_PROPS RP2.2 audit settlement): the `Normal`
  half of that "matching 0.14.5/r4133" claim is FALSE.** r4133's
  `InterpretSwitchState` exits early only when the *property name* starts with
  `'a'` or `'s'` — `if Locked and ((LowerCase(property_name[1]) = 'a') or
  (LowerCase(property_name[1]) = 's')) Then Exit`, under the comment "Only
  allowed to change normal state if locked"
  (`Version8/Source/Controls/SwtControl.pas:416-417`) — and property 6 is
  `'Normal'` (`:128`), so arm 6 (`:201-204`) reaches `set_NormalStates`
  (`:556-561`), which has no lock guard. Probed on the vendored r4133 DLL: with
  `lock=yes`, `normal=open` moves `Normal` to `[open, open, open, ]` while
  `state=`/`action=` move nothing. The port refused all three (pre-RP3.7)
  (`swt_control/accessors.rs:151-155`), following 0.14.5's `ConditionalReadOnly`
  flag on `Normal` (`SwtControl.pas:159-160`) — and its own **Relay** already
  implements the r4133 rule and documents it (`relay/accessors.rs:416-420`). By
  the 2026-08-02 policy r4133 is the authority, so this is a port bug: the fix
  is owned by `R4133_PROPS_PLAN.md` §RP3.7 (a2) (both lanes, with a pin), and
  `locked_ignores_normal_and_state_writes` is re-pointed there (RP3.7 re-pointed
  it by renaming, to
  `locked_normal_applies_locked_state_and_action_do_not`). No corpus deck
  writes `normal=` under lock, so nothing gates on it today.
  **Settlement (2026-09-02, R4133_PROPS RP3.7 — landed in BOTH lanes).** The
  scalar model went with the fix. `Normal` and `State` are now r4133's per-phase
  arrays (`FNormalState`/`FPresentState : pStateArray`, `SwtControl.pas:37-38`,
  allocated `:299-305`, driven per conductor by `set_States` `:532-549` →
  `ControlledElement.Closed[Idx]`), written either **ganged** from a bare token
  (`:433-451`, every slot) or **per phase** from any quoted list (`:453-480`,
  first-character match, at most five tokens honored, unlisted slots unchanged),
  and rendered as `[tok, tok, … ]` — one token per **controlled-element** phase
  (`GetPropertyValue` `:589-599` Normal / `:600-610` State; `[]` when there is no
  controlled element). `Create` initializes BOTH arrays all-CLOSED, so a fresh
  control renders `[closed, closed, closed, ]` for `Normal` where 0.14.5/capi015
  render the unset `''` / the `closed` scalar. The lock rule is r4133's, verbatim:
  the guard is on the **property name** (`if Locked and ((LowerCase(param)[1] =
  'a') or (… = 's')) Then Exit`, `:416-417`) and property 6 is `'Normal'`
  (`:128`), so a locked `normal=` write reaches arm 6 (`:201-204`) →
  `set_NormalStates` (`:556-561`) and APPLIES — ganged and quoted alike — while a
  locked `state=` / `action=` writes nothing. The `{Supplemental Actions}` block
  (`:220-228`) sits OUTSIDE the arm, so `NormalStateSet` latches even on a write
  the guard refused (measured on the vendored r4133 DLL, RP3.7 A2a). Pins
  (`elements/control/swt_control/tests.rs`):
  `locked_normal_applies_locked_state_and_action_do_not` — the renamed
  `locked_ignores_normal_and_state_writes`, now the full probe byte sequence
  through the executive —
  `a_locked_state_write_still_runs_the_normal_defaults_supplemental`,
  `render_is_one_token_per_controlled_element_phase`,
  `a_quoted_single_token_is_per_phase_a_bare_one_is_ganged`,
  `per_phase_write_renders_the_r4133_bytes_through_both_seams`. Consequences
  recorded elsewhere: the ten `Normal`/`State` cells of the capi015 props golden
  `props/swtcontrol.json` are overlaid with the r4133 DLL's own bytes (the
  artifact's own oracle block says so), and the AltDSS schema now spells both
  properties `type: array` + `items: $ref` — the shape Relay already had
  (`json/schema_full_port.json`, cause prose in `json/schema_divergences.json`).
  Against the **capi** channel the render is a deliberate, pinned divergence:
  0.14.5 has no per-phase model at all (scalar render; a quoted list is silently
  dropped through the enum default), so the capi-gated cells are ledgered
  (RP3.7 B2, 2026-09-02) and never re-baselined — **five** per-case
  `capi_v0145` entries under one new cause
  `swtcontrol-per-phase-state-render`, on `controls:swtcontrol/
  swtcontrol_lock.dss` (its `state`/`normal` **probes** *and* its property
  compare, 12 steps ⇒ 48 hits), `modes:makeposseq/makeposseq_ctrl.dss`
  (2 property cells, the 1-token `[closed, ]` render) and the three
  `IEEE_519.DSS` copies (4 each: two controls × two properties). Those five
  cases carry the pair's 19 out-of-scope cells; the **40 in-scope** cells per
  property sit on `midi_swtcontrol`, `swtcontrol_time` and `civanlar`, which are
  `engines=r4133` and now render r4133's bytes exactly — so **no r4133 property
  entry is staged or owed**, and the port's value there is held by
  `exec::tests::controls::swtcontrol_state_renders_per_phase_on_the_r4133_only_decks`
  (plan §1.1(c): no oracle channel can witness those cells before the RP4.1
  unmask).
  Feature-sensitivity: unit
  `d12_normal_and_state_readbacks_are_independent` (0.14.5 conflated both onto
  `CurrentAction`). **Gating note (0.15.x-adoption sweep):** the "flipped to
  capi015" phrasing above is historical — all three moved decks (`swtcontrol_time`,
  `midi_swtcontrol`, `civanlar`) are now gated **`engines=r4133`** with empty
  ledgers, and the D12 Normal/State mapping was live-verified against r4133 (which
  agrees with capi015 here) per WP-U2.4 D6 — whose `Action` is, on r4133, exactly
  a **ganged `State` write**: arms 3 and 7 share one body (`:205-207`), calling the same
  `InterpretSwitchState` with the property name `'Action'`, so the same lock guard
  applies and the ganged path always runs (`WasQuoted` is never consulted for it).
  The re-baselined capi015 props golden stays the offline pin, with its ten
  per-phase cells overlaid by RP3.7.
- **D15** `LookupVariable` case-insensitivity (`4366b126`) — not-a-delta: the
  port's only equivalent (relay) already matches the fixed side.
- **A7-r3723** GenController deregistration — not-a-delta: never registered in the
  r3723 port; `New GenController` already errors "not found".
- **D11 (part 2)** CapControl TIMECONTROL monitored-element requirement
  (`b9bc87b8`) — adopt. **0.14.5:** TIMECONTROL (like FOLLOWCONTROL) used the
  *controlled* capacitor as `effElement` and forced `ElementTerminal := 1`.
  **0.15.x (capi015 0.15.0b4):** the `<> TIMECONTROL` guard was dropped, so only
  FOLLOWCONTROL falls back to the capacitor; TIME now **requires** a monitored
  element and uses it as `effElement` with the specified terminal
  (`CapControl.pas:581`). **capi015 probe** (0.15.0b4): `type=time` with no
  `element=` errors `CapControl.cc1: "Element" is not set, aborting.`;
  `type=time element=line.l1 terminal=2` keeps `Terminal=2` (0.14.5 forces →1)
  and binds to the monitored element. Part 1 (PT/CTPhase validation scope) was
  already aligned. **Unit-pinned** (`time_control_requires_monitored_element` +
  `time_control_uses_monitored_element_terminal`, both citing the capi015 probe).
  The existing multi-step `controls/capcontrol/capcontrol_time.dss` schedule deck
  cannot flip to capi015 (it has a `daily=` load → multi-step re-nominalization,
  L1) and its only moved observable is the static `Terminal` readback; reworked to
  `terminal=1` so the readback is engine-agnostic (0.14.5 forces→1, port keeps 1)
  while the clock-based switching feature is unchanged.
- **C4** `SolveAll` command (ordinal 123, a `DSS_CAPI_PM`-only command word) —
  now dispatched. Single-actor semantics = plain `Solve` (`ExecCommands.pas:346`
  iterates `DoSetCmd(child,1)` over the one actor; `IsSolveAll` only steers the
  parallel/A-Diakoptics path we do not have). Oracle-confirmed (dss-python
  0.15.7): `SolveAll` solves like `Solve`; the spaced `Solve all` errors
  `Object Class "all" not found`. Unit-pinned (`solve_all_alias_matches_plain_solve`).
  **0.15.x spaced-form delta (not adopted, ungated).** The 0.15.x parser
  (`ExecCommands.pas:505-521`) added a `Solve`-modifier that also maps the
  *spaced* `Solve all` to `SolveAll` (a plain solve, no error). The port keeps
  the 0.14.5 behavior (`Solve` + unknown token `all` → `Object Class "all" not
  found`), matching the default 0.14.5 oracle; C4's scope is the one-word
  `SolveAll` command word only. No corpus deck exercises spaced `Solve all`, so
  the delta is ungated either way — tracked here for a later parser-parity pass.
- **D13** LoadShape MMF fixes (`c4590d16`) — **not-a-delta for the port** (already
  matches the fix). The three Pascal hunks are: (1) the single-column `csvfile=`
  `CreateMMF` guard's missing `not` (`LoadShape.pas ~:1032`) — 0.14.5 exits on
  CreateMMF **success**, so a single-column MMF shape never loads its P data and
  the daily solve raises `#482 Division by zero`; (2) `mmDataSizeQ := mmDataSize`
  (a debug-only field, no observable); (3) the Linux `fpMUnMap` disposal
  `mmFileSize`/`mmFileSizeQ` swap. The port is `#![forbid(unsafe_code)]` with no
  memory-mapping — it eager-reads the whole file into `p_mult`/`q_mult`
  (`read_csv_file` MMF branch), so hunks 2/3 have no equivalent and hunk 1's data
  ALREADY loads. **Probe** (single-column `npts=8 MemoryMapping=Yes csvfile=`, 8
  daily steps): 0.14.5 aborts `#482 Division by zero` (Pmult=`[0.0]`); capi015
  (0.15.0b4) drives the load to P/phase `[20 40 70 110 160 130 90 50]` kW
  (= 200·`[0.10 0.20 0.35 0.55 0.80 0.65 0.45 0.25]`). **Gate:** live deck
  `modes/upgrade/mmf_singlecol/mmf_singlecol.dss` (now gated **`engines=r4133`**
  in the manifest — the "`oracle:"capi015"`" wording here is stale; the single-
  column-MMF fix is present in r4133 too, `n_steps=8`, whole-model per-step
  compare; cannot gate 0.14.5 — the deck is the bug the fix removes, §1.2; §1.7
  two-process determinism confirmed, fingerprint `0ead40d7199b0781`) + unit
  `mmf_single_column_csvfile_loads_like_capi015`. The
  existing `inputformat/shape_mmf` deck stays 0.14.5 (it uses `sngfile`/`dblfile`/
  `pqcsvfile`, not the single-column path). **Vendored-spec caveat.** The `#482`
  claim above is against the pinned 0.14.5 oracle **binary** (tag `0.14.5`, which
  ships the buggy `if CreateMMF(...)`), settled empirically — NOT by reading the
  vendored source. The vendored `.inputs/dss_capi/src/General/LoadShape.pas:1035`
  already reads the FIXED `if not CreateMMF(...)` (the c4590d16 fix, dated after
  the 0.14.5 tag), so a `grep` of `.inputs/dss_capi` for this hunk shows the fix,
  not the bug — a hole in the "`.inputs/dss_capi` == the 0.14.5 backend" invariant
  at this one line. Gating was correctly settled against the oracle binary, so the
  outcome is unaffected.

`known_diffs.json`: none of these had a prior Rust↔EPRI entry — nothing to retire.

## WP-U1.6 C5 — RegControl signed thresholds + idle zones — SETTLED (signed-threshold framework = r4086; abs fallback re-decided to r4133 + idle no-load zone bounded-AND, both by the 0.15.x-adoption sweep fix round)

`8a898cba` (SVN r4086, in capi015 0.15.0b4) reworks RegControl's reverse-power
surface and adds an idle-zone family:

- `RevThreshold` becomes a **signed W** field with a kW→W property scale
  (default **−100 kW**, was +100 kW), and a new `FwdThreshold` (+100 kW) splits
  the forward edge. The reverse-power detection sign moved from the *comparison*
  into the *stored value* (`FwdPower < RevPowerThreshold`, no unary `−`), so a
  legacy deck that sets only `revThreshold=X (X>0)` is **behavior-identical**:
  `EndEdit`'s rev-only fall-back restores the old band around 0 kW. **EPRI r4133
  RegControl.pas:499-507 is sign-preserving — `Fwd:=Rev; Rev:=−Rev` (no abs).**
  dss_capi 0.15.x `8a898cba:428-435` added an `abs` (`Fwd:=abs(Rev); Rev:=−Fwd`)
  with its own comment calling it a "fix" — a capi015-only deviation, NOT in
  r4086/r4133. For a positive `X>0` the two are identical (the symmetric ±X band);
  for a **negative** rev-only edit the abs inverts the band, diverging from BOTH
  gating oracles (**own live probe, revThreshold=−500 rev-only, reversible=yes,
  50 kW forward load**: r4133 → Rev=+500 kW/Fwd=−500 kW → reverse ping-pong →
  `#485 Max Control Iterations Exceeded`; pinned 0.14.5 → identical `#485`; port
  with the abs → converged) — the D14 signature. The port now follows r4133
  (dropping the abs simultaneously restores 0.14.5-legacy equivalence) — after the
  fix the port likewise hits `Max Control Iterations Exceeded` on that deck,
  matching both oracles — pinned by
  `reg_control::tests::rev_only_edit_fallback_is_sign_preserving`. The fallback is
  per-edit (tracked via a new `PrpSequence` BeginEdit boundary), so a later
  rev-only edit re-derives the band and clobbers an earlier `FwdThreshold` —
  reproduced 1:1.
- New `Idle`/`IdleReverse`/`IdleForward` flags suppress a pending tap when the
  through-power sits in a dead-band. The no-load test is EPRI r4133/r4088's
  **bounded AND** — `(FwdPower ≤ FwdPowerThreshold) and (FwdPower ≥
  RevPowerThreshold)` (`control_loop.rs:319`): with the default −100/+100 kW band
  an idling reversible reg idles only inside the finite ±100 kW no-load zone and
  still taps once the through-power leaves it. dss_capi 0.15.x wrote this as an
  **OR** (`(FwdPower≥Rev) or (FwdPower≤Fwd)`), a tautology under the default band
  that makes the reg never tap — proven wrong on 2026-07-19 vs r4088/r4133 +
  physics and re-decided to the bounded-AND (CLAUDE.md 0.15.x rule; pinned by
  `idle_no_load_zone_{suppresses_out_of_band_tap,still_taps_when_power_out_of_band}`).

Gate: capi015 props golden re-baseline (`tests/golden/props/regcontrol.json`,
`gen_regcontrol_capi015.py`) pinning the signed defaults + the two-edit fallback;
harness `PROPS_015X` row; capi015 deck `regcontrol_idle.dss` (idle suppresses a
tap that would otherwise reach tapnum 15 / tap 1.09375; |ΔV|≈0.075 pu, two-process
deterministic); RegControl unit tests. Legacy equivalence is covered by the
existing default-oracle `regcontrol_reverse.dss` (unchanged trajectory).

## WP-U1.6 C6 — Transformer/AutoTrans BH-curve `Unused` props — SETTLED (adopt capi015 = r4064)

`90962ae8` (SVN r4064) adds three GICharm data props — `BHpoints` (int),
`BHcurrent`/`BHflux` (double arrays sized by BHpoints) — to **both** transformer
classes, flagged `Unused` (parsed + stored, never consumed by a solve; the port
does not implement GICharm). Ported: the props, the `BHpoints` realloc side
effect (zeroes both arrays), and the MakeLike copy.

**Upstream crash NOT reproduced (UB, per CLAUDE.md).** On capi015, *parsing* a
non-empty `BHcurrent=(…)` **segfaults** the backend, and *reading* the array with
`BHpoints>0` errors — the `Unused` DoubleVArray getter reads its element count
from an **unset `PropertyOffset2`** (BHcurrent only wires `Offset3`), so `Norder`
is garbage. Only the empty default is well-defined: `GetDSSArray` guards
`ptr=NIL → ''` first (`Utilities.pas:1857`), so a default (BHpoints=0, NIL arrays)
dumps `''`. The port matches that (empty Vec ⇒ `get_f64_array` returns `None`
⇒ `''`) and renders the set-state safely as `[ … ]` instead of crashing.

Gate: capi015 default-state props goldens (`transformer_bh.json`/
`autotrans_bh.json`, `gen_bh_capi015.py`) — the only oracle-probable state; the
set-state (parse/store/realloc/dump) is unit-pinned (`bh_curve_props_parse_and_store`
+ `auto_trans::…::bh_curve_default_and_realloc`); harness `PROPS_015X` rows for
both classes. No `known_diffs.json` entry existed.

## WP-U1.6 C5-r3723 — LoadShape `Mode` prop — SETTLED (NOT a delta for us; dss_capi 0.15.x declines it)

The EPRI SVN r40xx line inserts a LoadShape `Mode` property at index 22, shifting
`Interpolation` 22→23. **dss_capi 0.15.x explicitly declines to port it** — the
`TLoadShapeProp` enum carries `// Mode = 22, -- not useful to implement this yet`
with `Interpolation = 22` in **both** 0.14.5 and 0.15.0b4
(`git show 0.15.0b4:src/General/LoadShape.pas`). The capi015 oracle therefore has
**23 properties, `Interpolation` at 22, no `Mode`** (probed 2026-07-16) — identical
to 0.14.5. The port already matches this exactly, so there is **nothing to port**:
adding `Mode` would break every LoadShape deck's property-count parity against the
binding oracle. No allowlist row (the tables are equal), no golden change. Pinned
by the guard `no_mode_prop_interpolation_stays_at_22` (fails if a stray `Mode`
ever lands). No `known_diffs.json` entry existed.

**Forward-note (0.15.x-adoption sweep) — r4133 DOES add `Mode@22`.** Unlike
dss_capi (which declines it), EPRI **r4133 `LoadShape.pas:246-247`** carries
`PropertyName[22] := 'Mode'` with `Interpolation` shifted to **23** (parse arm at
:624). So against the r4133 channel the port is one property short (23 vs r4133's
24, `Interpolation` at a different index). It is a real **UPGRADE Rung-2 parity
gap** (LoadShape `Mode` property + its sparse-shape interpolation-mode effect),
correctly deferred — added to the Rung-2 list for a later WP; no Rung-1 action
(the pinned `capi_v0145` oracle has no `Mode`, so no gate moves today).

## L4, E2 — SeasonalRating reimplementation (global `SeasonalRatingIdx`) — SETTLED (WP-U1.5 multi-season core = capi015 = r4133; single-season guard re-decided to r4133 by the 0.15.x-adoption sweep fix round)

**Observable.** The per-PDElement norm/emerg current ratings used by the overload
report paths — `Export Overloads` (`ExportOverloads`), `Export Capacity`
(`CalcAndWriteMaxCurrents`), and `DI_Overloads` (`WriteOverloadReport`) — when
`SeasonRating=yes` + a `SeasonSignal` XYCurve are active and the element carries
`Seasons>1` (`NumAmpRatings>1`).

**dss_capi 0.14.5 (default oracle).** Each report re-read the XYCurve per element
and **state-mutated** `DSS.SeasonalRating := FALSE` on a miss; `DI_Overloads`
applied the override only to `ClassName='line'`; `Export Overloads` did **not**
apply the seasonal rating at all (it read base `NormAmps`).

**dss_capi 0.15.x (capi015) / EPRI r4133.** `55400a29`: a global
`DSS.SeasonalRatingIdx` is precomputed once (`SyncSeasonalRatingIdx`:
`trunc(SeasonSignalObj.GetYValue(intHour))`, `-1` when inactive) on every solve
and the season/hour `Set` commands; `TPDElement.GetRatings` centralizes the
override (`if (idx>=0) and (idx<NumAmpRatings) then Norm/Emerg := AmpRatings[idx]`)
and it applies to **any** PDElement (Line, Transformer, cable), not just lines.
The state-mutating XYCurve re-read is gone.

**Probe** (`scratch_seasdeck.py`, 2026-07-16; overhead Line + Transformer + CN
cable, each `Seasons=4 Ratings=[...]`, `SeasonSignal` maps hour→index,
`set hour=13` → `SeasonalRatingIdx = trunc(GetYValue(13)) = 2`, `Export
Overloads`):

| engine | `Line.L1 %Normal` | applies to Transformer/cable? |
|---|---|---|
| capi 0.14.5 (default) | `134.5` (base `NormAmps=100`) | **no** — base ratings |
| capi015 (0.15.0b4) | `336.3` (`AmpRatings[2]=40`) | **yes** |
| oddie r4133 (11.0.0.1) | `336.3` | **yes** — bit-identical to capi015 |

capi015 == r4133 bit-identical (ledger L4 confirmed). A >200-percentage-point,
revision-**sensitive** move.

**Decision — adopt the capi015 global-index form.** New
`Circuit::seasonal_rating_idx` (`-1` init) synced by
`solution::meters::sync_seasonal_rating_idx` on **every solve AND on the `Set
Hour`/`SeasonRating`/`SeasonSignal` commands** (`55400a29` calls
`SyncSeasonalRatingIdx` at ExecOptions params 3/114/115 + CAPI `Set_Hour`/
`Set_dblHour` — the union of the Pascal sync sites). The set-command sync is
**not** optional: a `solve; set hour=X; export overloads` (no re-solve) reads the
new index on capi015, verified empirically (`solve@hour0; set hour=18; export`
reports `AmpRatings[3]`, not the stale `AmpRatings[0]`) — pinned by
`golden_reports.rs::set_commands_resync_seasonal_rating_idx`. New
`CktElement::get_ratings(seasonal_idx)` trait method (Pascal `55400a29`
`TPDElement.GetRatings`) overridden by Line + Transformer's `num_amp_ratings`/
`amp_ratings` accessors; wired into `export_capacity`, `export_overloads`, and
`write_overload_report` (the DI path keeps the BASE-rating entry gate, then uses
the seasonal ratings for the overload test + reported values, exactly as 0.15.x
`EnergyMeter.pas::WriteOverloadReport` — pinned by
`di_overloads_applies_seasonal_rating`). The 0.14.5 state-mutating
`DSS.SeasonalRating := FALSE`-on-miss read is NOT reproduced (CLAUDE.md known-bug
policy) — the precomputed index removes it.

**capi015 ≠ r4133 on SINGLE-season elements — the port follows r4133 (fixed by
the 0.15.x-adoption sweep, 2026-07-19).** The `55400a29` `GetRatings` guard is
`(idx >= 0) and (idx < NumAmpRatings)` — it **dropped** the pre-refactor/r4133
`(RatingIdx <= NumAmpRatings) and (NumAmpRatings > 1)` guard. So under an active
signal at idx 0 a **single-season** PDElement (`NumAmpRatings = 1`, the default)
takes the stale constructor default `AmpRatings[0]` for BOTH norm and emerg on
capi015 — silently discarding a user-set `normamps`/`emergamps` in the overload/
capacity reports — whereas r4133 keeps the base `(NormAmps, EmergAmps)`. Under
the new CLAUDE.md rule (0.15.x is not an authority) the sweep proved capi015 wrong
here vs BOTH r4133 (source read: PDElement.pas l.351 guard present) AND physics
(the drop hides real overloads), and the earlier "adopt capi015 the binding
oracle" rationale was self-referential — the seasonal goldens contain only
`Seasons=4` fixtures, so nothing bound the single-season path. **Own r4133 vs
port probe** (single-season `Line.L1 normamps=100 emergamps=120`, I1≈139.77 A at
idx 0, `Export Overloads`): r4133 `%Normal=139.8 %Emergency=116.5` (overload row
present); port BEFORE the fix — **no overload row at all** (rated at the stale
`AmpRatings[0]`); port AFTER the fix (add `&& num_amp_ratings() > 1` at
`traits.rs::get_ratings`, keeping the memory-safe `0 <= idx < NumAmpRatings`
bound — r4133's own `<= NumAmpRatings` off-end read is UB, not reproduced) —
`%Normal=139.8 %Emergency=116.5`, **bit-identical to r4133**. Zero goldens move
(all `Seasons=4`); the earlier "capi015 == r4133 bit-identical" claim holds only
for those multi-season fixtures.

**Gate consequence.**
- **New capi015 goldens** `tests/golden/reports/export_overloads_seasonal.txt` +
  `export_capacity_seasonal.txt` (`.meta.json` `"oracle":"capi015"`), regenerated
  with `DSS_ORACLE_ENGINE=capi015 <oddie-venv>/python tools/golden/gen_reports.py`
  (the generator gained a `capi015` branch that regenerates ONLY these files, §1.5).
  Driven by `golden_reports.rs::export_{overloads,capacity}_seasonal_matches_capi015`
  (`compare_export`, small numeric floor for the CN-cable sparse solve). Deck
  validated bit-identical on capi015 and oddie:r4133 (§1.7).
- **Feature-sensitive unit tests** `line::tests::get_ratings_applies_seasonal_index`
  + `transformer::tests::get_ratings_applies_seasonal_index_on_transformer` (pin the
  `AmpRatings[idx]` override + the `NumAmpRatings > 1` and `idx<NumAmpRatings`/`-1`
  guards — a single-season element keeps its base ratings, per r4133), plus
  `golden_reports.rs::{set_commands_resync_seasonal_rating_idx,
  di_overloads_applies_seasonal_rating}` (the set-command re-sync and the DI-path
  seasonal wiring).
- `known_diffs.json`: no seasonal Rust↔EPRI entry existed (the override was
  NOT_PORTED, so it never produced a cataloged divergence); porting it now makes
  Rust match r4133. Nothing to retire; a latent Rust↔r4133 gap is resolved.

## D9 — AllocateLoad/CalcAllocationFactors ignore disabled meters/sensors — SETTLED (WP-U1.5, adopt capi015; 0.14.5 UB not reproduced)

**Observable.** `AllocateLoads` when a metered zone's EnergyMeter (or a Sensor)
is disabled.

**dss_capi 0.14.5 (default).** No `Enabled` guard: walking a disabled meter's
(un-built / stale) `BranchList` raises an **Access Violation** (probed
2026-07-16: `edit energymeter.m1 enabled=no; allocateloads` → AV). **0.15.x
(capi015) / EPRI r4133.** `fb728364` (SVN r4115): `TEnergyMeterObj.AllocateLoad`
and `TMeterElement.CalcAllocationFactors` each open with `if not Enabled then
Exit` — a disabled meter/sensor is skipped; its zone loads keep their factors.

**Probe** (`scratch_alloc4.py`, 2026-07-16; two `xfkva` loads, meter enabled at
zone-build then `enabled=no` before `allocateloads`): capi015 → both loads' factor
stays `0.5` (skipped); 0.14.5 → Access Violation. Enabled contrast → factor
`6.3725`.

**Decision — adopt** (the two `if !enabled` guards in
`solution/meters/sampling/allocate.rs`, `calc_allocation_factors_all` +
`allocate_load_all`). The 0.14.5 AV is UB → not reproduced (the Rust port is
memory-safe regardless; the guards make the r4115 skip faithful and cover the
disable-after-zone-build case).

**Gate consequence.** `allocation.rs::allocateloads_ignores_disabled_meter`
(feature-sensitive: the meter is enabled at zone-build so the zone is populated,
then disabled — the guard keeps the factors at `0.5`; without it the populated
zone drives them to `6.3725`). No corpus deck moves (none disables a meter with an
allocating zone). `known_diffs.json`: no entry — nothing to retire.

## D16 — zone-list counter skips disabled + non-PD — SETTLED (WP-U1.5, NOT a delta for us; already ported)

The manual-`ZoneList` build's "ignore disabled devices and non-PD elements" skip
(`MakeMeterZoneLists`, `690e02f9` flattened it) is **already present in the 0.14.5
baseline** (`.inputs/dss_capi` and `.inputs/dss_capi_with_git` `MakeMeterZoneLists`
carry the same `if not TestElement.Enabled ... Inc; if (DSSObjType and BaseClassMask)
<> PD_ELEMENT ... Inc` logic; 0.15.x only refactored it to a single
`if (not Enabled) or (... <> PD_ELEMENT)` guard). The Rust port already reproduces
it (`solution/meters/zones/build.rs`: `if !enabled || !is_pd_element(...) {
zone_list_counter += 1 }`). **No code change; not an observable delta** — mirrors
the D5/D8 "not a delta for us" records. `known_diffs.json`: the `meter-zonepce-count`
entry documents an EPRI-r3723-only ZonePCE off-by-one this WP does not change (it
stays for a Rung-2 r4133 re-check).

## D8-r3723 — manual-ZoneList child from-bus/terminal — SETTLED (WP-U1.5, adopt r4133; effect masked in our path)

**Observable.** The `FromBusReference`/`FromTerminal` of a branch added to a meter
zone from a manual `ZoneList`.

**dss_capi 0.14.5.** `BranchList.AddNewChild(TestElement, 0, 0)` — from-bus unset,
from-terminal 0; the broken tree AVs downstream in the oracle (`calcvoltagebases`,
probed 2026-07-16 — the manual-zone deck raises an Access Violation on 0.14.5).
**0.15.x (capi015) / r4133.** `AddNewChild(TestElement, TestCE.Terminals[0].BusRef,
1)` — the element's terminal-1 bus and terminal 1.

**Verify verdict (per the WP's instruction).** Our Phase-6 port used the 0.14.5
form (`add_new_child(NO_BUS, 0)`), so it did NOT build the tree the r4133 way — it
**is** a code delta. Applied the fix
(`solution/meters/zones/build.rs`: `add_new_child(terminals[0].bus_ref, 1)`). Its
observable effect is **masked** in the current manual-zone path: the corrected
from-bus feeds the volt-base-list (the bus is already listed from the metered
branch) and the `DistFromMeter` base (not propagated in the manual branch), and
the zone's branch/PCE lists + load collection are unchanged — so no report or
`meter_zone` channel moves. The 0.14.5 side is UB (AV) → no oracle golden is
possible; the existing memory-safe `energymeter_manual_zonelist` test guards the
build (still green with the fix).

## E1 / L3 — Monitor CSV header — SETTLED (WP-U1.5, keep the dss_capi form; tokens match capi015)

**Observable.** The monitor CSV header channel-name tokens.

**Spec.** dss_capi 0.15.x `a6d3aa2c`/`6b54aba5`: the header omits quotes
(`CommaText`→`DelimitedText`) and a `MonitorHeader` compat flag (0x80, **off by
default**) restores EPRI's extra leading spaces + trailing comma. Both are CSV
**file-rendering** changes; the parsed `Monitors.Header` **token list** is
unchanged (the flag is off by default; the port stores bare tokens, not a rendered
CSV).

**Probe** (`scratch_monhdr.py`, 2026-07-16; mode-0 V/I monitor `Monitors.Header`):
capi015 and 0.14.5 both return `['V1','VAngle1',…,'IAngle3']` — **identical**.

**Decision — keep the dss_capi-ported form (no quotes, no extra spaces); no code
change** (ledger L3). The port's monitor header is compared token-wise
(`harness::compare_monitor`) and already matches the default oracle; capi015 ==
0.14.5 on tokens, so no case moves. `known_diffs.json`: the
`monitor-header-whitespace` entry (Rust/dss_capi strip EPRI's leading-space
padding — a deliberate KEPT divergence vs the EPRI oddie binaries) **stays** — E1
confirms keeping it, it is not retired.

## NCIM PV→PQ Q-limit iteration count — SETTLED (WP-U1.7 cross-check, capi015 pinned; r4088 differs, report-only) → RE-GATED to r4133 (NCIM RE-GATE WP, 2026-07-20) → **ADOPTED r4133 cadence (ORPHANED_GAPS §1.6, 2026-07-26)**

**Observable.** The NCIM (`Set algorithm=NCIM`) iteration count to converge a deck
that hits a generator Q-limit → PV→PQ conversion (`NCIM_UpdateGenQ` switching).

**dss_capi 0.15.x (capi015, r4103).** The PV→PQ decks converge in **8 iterations**
(`ncim_pv_pq`, `ncim_midi`). PQ-only (`ncim_pq`) converges in **3**.

**EPRI r4088 (OpenDSS 10.2.0.1 "Columbus").** NCIM IS supported (accepts `Set
algorithm=NCIM`, `get algorithm → ncim`, swing bus at the ideal EMF). The PV→PQ
decks converge in **4 iterations** — half of capi015's 8 — while PQ-only matches
(3). The r4088→r4103 window changed the PV→PQ switching/relaxation cadence.

**Probe** (`ab_ncim.py`, 2026-07-16; capi015 vs oddie:r4088 on the three
`modes/ncim/` decks, comparing `YNodeVarray`):

| deck | capi015 conv/iters | r4088 conv/iters | max\|dV\| |
|---|---|---|---|
| ncim_pq | True / 3 | True / 3 | 7.1e-12 V |
| ncim_pv_pq | True / 8 | True / 4 | 1.8e-12 V |
| ncim_midi | True / 8 | True / 4 | 7.2e-11 V |

The **converged node voltages are bit-identical** across the two engines (max\|dV\|
at the KLU-vs-KLU last-ulp floor); only the iteration COUNT to reach the same
fixpoint differs on the PV→PQ decks. No physics divergence.

**Decision — pin capi015 (the Rung-1 primary oracle); the r4088 iteration count is
report-only, NOT gated.** The port reproduces capi015's NCIM solver loop-for-loop
(`NCIMSolutionHelper.pas`, r4103) and pins its 8-iteration PV→PQ convergence
(`exec/tests/ncim.rs`, unit; `modes/ncim/*` live). The corpus iteration policy is
already `<=` for `oracle: "capi015"` cases (§1.3-1), so a future engine that
converges the same fixpoint in fewer passes would not fail the gate.

**Gate consequence.** None — this is an inventory note for the eventual r4133
end-target (§1.4). No `known_diffs.json` entry (NCIM is a 0.15.x-line feature with
no 0.14.5 baseline; the port matches capi015, its spec). If a later rung retargets
NCIM to r4133, the switching cadence (not the fixpoint) is the item to revisit.

### UPDATE — oracle-of-record flip capi015→r4133 (NCIM RE-GATE WP, 2026-07-20)

**Decision (user-approved 2026-07-20): r4133 is the oracle-of-record for NCIM.**
The capi015 0.15.0b4 probe venv is retired (gone from disk), so capi015 can never
again be a live oracle; the only live NCIM-capable channel is r4133 via
`epri-worker`. The 4 NCIM corpus decks (`modes/ncim/ncim_pq`, `ncim_pv_pq`,
`ncim_midi` + `solvable_now Xmission_System_Kundur2Area`) are flipped from
`defer_ledger` Rust-smoke to **live r4133 gating** with **NO ledger entry** — the
whole-model compare (node V, system Y, the Vsource swing current/power/loss, the
NCIM generators, and the warm-resolve iteration count) is within the tier floor on
every case.

**Swing-source current — the capi015 off-by-one is FIXED in r4133 (source
evidence).** `TVsourceObj.CalcInjCurrAtBus` (r4133 `PCElements/VSource.pas` l.1085,
reached from `GetCurrents` l.1194-1195 under `Algorithm=NCIMSOLVE`) fills a
`Yorder+1` `ElmCurrents` with an **offset write** `GetCurrents(@(ElmCurrents[1]))`
(l.1123 PD / l.1158 PC) — conductor 1 → `ElmCurrents[1]` — then reads
`ElmCurrents[(myTerm*stride)+j]`, `j:=1..NPhases` (l.1135 / l.1169): a 1-based read
of the offset-written array is **unshifted**. capi015 0.15.0b4 (e936d210) instead
wrote `ce.GetCurrents(ElmCurrents)` at index 0 and read `ElmCurrents[j]` 1-based — a
one-conductor shift. The port dropped the `+1` in
`exec/view.rs::ncim_swing_source_currents` (the `TODO(compat)` removed) and now
reproduces r4133's unshifted read. *(**Correction, 2026-09-03,
R4133_PROPS_PLAN §RP3.13.** That code no longer lives in `exec/view.rs`: the
override was deleted and the same unshifted read now sits in
`solution::solution::ncim::ncim_stamp_swing_source_currents`, stamped into the
swing source's `Iterminal` at convergence and echoed by
`TVsourceObj.GetCurrents`' ported NCIM arm in
`elements/pc/vsource/solve.rs`, so the element path and the corpus gate read one
live state. The values below are unchanged.)*

**Live r4133 probe evidence** (own `epri-worker` run, r4133 = `Version 11.0.0.1
(64-bit build) - Charlottesville`, 2026-07-20; `ncim_pq` = the `pq_circuit(1)` unit
deck). `Vsource.source` reported terminal current, conductors 0..2 (A):

| conductor | capi015 (shifted, old pin) | r4133 (unshifted, new pin) |
|---|---|---|
| 0 | `70.71692 + 55.78569i` | `-83.67029 + 33.34980i` |
| 1 | `12.95337 - 89.13549i` | ` 70.71692 + 55.78569i` |
| 2 | `83.67022 - 33.36820i` | ` 12.95337 - 89.13549i` |

r4133 per-conductor power = `-602.38906 - 240.10383i` kVA (×3), losses
`-1807167.18 - 720311.50i` (W/var). The Rust port matches these to the micro tier
floor (unit-pinned, `exec/tests/ncim.rs::ncim_vsource_reported_currents_match_oracle`).

**Iterations + node V.** Converged node V is DIGIT-IDENTICAL r4133-vs-port (~1e-12,
faer-vs-KLU). The gate compares the **warm re-solve** count (compile runs the deck's
own solve, the gate re-solves once); r4133's warm-resolve counts are `ncim_pq=2`,
`ncim_pv_pq=2`, `ncim_midi=2`, `Kundur2Area=1`, and the port matches each **exactly**
(no `rust<oracle` NOTE fired). The cold-solve cadence divergence documented above
(capi015 8 vs r4088 4 on the PV→PQ decks) is never observed by the gate and needs no
ledger row.

**Gate consequence.** The 4 decks now gate LIVE on r4133 (`engines:"r4133"`,
`defer_ledger` removed, population-lock `defer=1→0`); no ledger entry, no tolerance
change. The `ncim-oppoint` ledger cause is rewritten to the resolved reality
(documentary, referenced by no entry). The frozen goldens `tests/golden/ncim/`
(capi015-captured solver internals) are UNREGENERABLE (retired venv) and untouched —
they pin internals unaffected by the swing-report path.

### UPDATE — the cadence itself is ADOPTED (ORPHANED_GAPS §1.6, 2026-07-26)

**Decision: port r4133's PV↔PQ switching cadence; the capi015 r4103 cadence is
retired.** The re-gate above flipped the *oracle*; this step flips the *code*.
The report-only status ends here — there is no remaining capi015-vs-r4133 NCIM
cadence divergence to report, because the port now implements r4133's.

**Source evidence (r4133, the oracle-of-record).** NCIM lives inline in
`Version8/Source/Common/Solution.pas` in the EPRI tree (there is no
`NCIMSolutionHelper.pas` — that file is the retired capi015 refactor, so the
earlier §1.6 "spec" pointer was wrong). `TSolutionObj.UpdateGenQ` (l.1993-2322)
is written

```
if (pGen.GenModel = 3) then          // l.2059 — PV: Q update + PV→PQ demotion
  … myPVOK … GenModel := 4 …         //          (l.2117-2163)
else                                 // l.2166 — ELSE arm
begin
  if pGen.GenModel = 4 then          // l.2169 — PQ→PV promotion test
    … myPQOK … GenModel := 3 …       //          (l.2226-2295)
  for j := 1 to pGen.NPhases do …    // l.2301 — Iterminal for the other models
end;
```

so the two conversion tests are **mutually exclusive within one Newton pass**: a
generator demoted PV→PQ at l.2120 is not re-examined by the PQ→PV test until the
*next* pass. r4088's `Solution.pas` is byte-identical here (diffed: the only
r4088→r4133 delta in this routine is commented-out debug file I/O at l.2018-2031 /
l.2109 / l.2318), which is exactly why r4088 and r4133 share the 4-iteration
count. The port's ported form ran the PQ→PV block as an unconditional second
`if`, so a just-demoted generator could be promoted straight back in the same
pass — a PV↔PQ limit cycle around `|V| = VTarget`.

**Live r4133 probe** (own `epri-worker` run, `Version 11.0.0.1 (64-bit build) -
Charlottesville`, 2026-07-26; `Solution.Iterations`/`Converged`, `YNodeVarray`,
`GeneratorsF(4)` = `Presentkvar`), against the port after the adoption:

| deck | r4133 | port (before) | port (after) |
|---|---|---|---|
| `pq_circuit(1)` (PQ only) | True / 3 | True / 3 | True / 3 |
| `pq_circuit(2)` (ConstZ) | True / 3 | True / 3 | True / 3 |
| `pv_circuit(1.0)` (regulating) | True / 3 | True / 3 | True / 3 |
| `pv_circuit(1.01)` (Q-limit → PQ) | True / **4** | True / 8 | True / **4** |
| `pv_circuit(1.02)` (target unreachable) | True / **4** | **False** / 15 | True / **4** |
| `IEEE118Bus/master_file.dss` | True / **9** cold, 2 warm | **False** / 100 | True / **9** cold, 2 warm |

Converged node voltages are unchanged wherever both converged, and IEEE118Bus's
converged point is *exactly* the voltage vector the port used to stall on
(`89_CLINCHRV.1 = 80072.708834`, `1_RIVERSDE.1 = 76088.991977`,
`4_NWCARLSL.1 = 79514.988474`) — the fixpoint was always right; only the
convergence test never fired while the generators chattered.

**Reported generator Q follows r4133 too.** After a PV→PQ conversion r4133
reports `Generators.kvar = 0.0` (probe: `vpu=1.01` and `1.02` both), because
`GetNCIMPowers` writes `Qnominalperphase := deltaQNom[j]` only on its model-3 arm
(l.1308) — the last write is iteration 1's zero. The port matched the old capi015
cadence's 1500 only because its generator was model-3 again on the final pass;
with the r4133 cadence it reports 0.0 like r4133, while the terminal powers still
carry the real clamp (−266.667 kW, −500 kvar per conductor, both engines).

**`PV2PQList` is deliberately NOT ported.** r4133 tracks converted generators in
`PV2PQList` (l.346, appended l.1938/2158, removed l.2260-2291; cleared at
construction l.645 and on the `InitGenQ` pass l.1119). It has no effect on the
solve: its only live consumer is `Show PV2PQGen` (`ShowResults.pas` l.3617,
routed as Show verb 35 at `ShowOptions.pas:388`), and `ReversePQ2PV` (l.1743) +
`DistGenClusters` (l.1687) are **dead code** in r4133 (declared, defined, never
called — grepped). The port's per-generator `Generator.ncim_expv` flag is the
equivalent and already drives its `Show PV2PQ_Conversions`; it is mutated at the
same **three** sites — the `GetNumGenerators` zero-Q-limit demotion (l.1938 ↔
`ncim.rs` `ncim_get_num_generators`), the `UpdateGenQ` PV→PQ conversion (l.2158 ↔
`ncim_update_gen_q`), and the PQ→PV reversal (l.2260-2291 ↔ the same fn's clear).

*Settler fix (2026-07-26).* The first of those three did not match: r4133 gates
the append on `if InitQ then` (l.1936-1939) while the port set `ncim_expv`
unconditionally, so a *warm* NCIM re-solve of a generator edited back to
`model=3` with `kvarmax=kvarmin=0` listed a generator in `Show
PV2PQ_Conversions` that r4133's `Show PV2PQGen` would not. Report-only (the
demotion to model 4 itself is unconditional in both), unreachable from the
solver's own state (the PQ→PV promotion at l.2216 requires nonzero limits, so it
can never re-create the zero-limit model-3 shape), and not probeable through the
DLL bridge — `Show` is the only consumer and it is a file+editor path — so this
one is settled on the r4133 source lines alone. Port now gates on `init_q`. The
same pass dropped a retired-capi015 leftover in the `Add2Limits` `else`
(`GenModel = 3 and NCIM_ExPV`, unreachable inside that arm; r4133 l.1972-1973 is
plain `Add2Limits := pGen.GenModel = 4`).

**Gate consequence.** `IEEE118Bus/master_file.dss` is promoted out of
`skipped_needs_investigation.json` (tag `ncim_pv_pq_switching_divergence`) into
`solvable_now.json` with `engines:"r4133"`, no ledger entry, no tolerance change;
the 4 existing NCIM cases keep their exact iteration counts and stay green. The
frozen capi015 goldens `tests/golden/ncim/` still pass **unchanged** (both decks:
Jacobian nnz + values, deltaF/deltaZ shape, and the byte-exact PV2PQ list) — the
cadence changes how the fixpoint is reached, not the converged Jacobian or which
generator ends up converted.

---

### UPDATE — three r4133 NCIM defects, proven and NOT reproduced (R4133_PROPS_PLAN §RP3.13, 2026-09-03)

**Correction to the two UPDATEs above, dated 2026-09-03.** They say "the 4 NCIM
corpus decks"; there are **five** r4133-gated cases that run `Set algorithm=NCIM` —
the three `modes/ncim/*` decks, `Xmission_System_Kundur2Area` and
`IEEETestCases/IEEE118Bus/master_file.dss` (`population.lock.json:538`,
`engines: "r4133"`, `kind: "large"`). The same "4 NCIM decks" wording stands in the
`ncim-oppoint` cause string of `tests/corpus/ledger.json` (a 2026-07-20 record kept
as history; the cause is documentary and referenced by no entry). Nothing else in
those UPDATEs changes: no ledger entry, no tolerance, no golden.

**Four r4133 defects on the NCIM path, all measured on the live DLL and none
reproduced** (the fourth added by the RP3.13 audit settlement) (CLAUDE.md 2026-08-02 policy; port-side fixes and pins in
`R4133_PROPS_PLAN.md` §RP3.13 / STATUS §RP3.13, upstream reports in the gitignored
`investigations/to_opendss/51..54`):

1. **`UpdateGenQ` writes `deltaQNom[j]` over a length-1 array.** `InitPQGen`
   (`R4133:Common/Solution.pas:1678-1679`) sizes `deltaQNom` to **1** for every
   non-PV machine, but all three writers index it per phase — the PV arm's stamp
   (`:2107`), the PV→PQ clamp (`:2152-2154`) and the PQ→PV promotion (`:2254-2256`), each
   `j := 0 to NPhases-1`. A generator born `model=4` that the promotion arm later
   flips to PV therefore writes past the end (unchecked in FPC). Measured
   2026-09-03 on an 8-line deck (`tmp/rp313/repro_pq2pv.dss`): the **solve** still
   answers — `converged=True`, 5 iterations, and node voltages the port matches to
   the digit (`GENBUS.1 = 7065.195045516548 - 20.00602724009179j`) — and the DLL
   then **hangs on the first element access after it** (`set_active_element
   Line.l1` never returns; the run killed at 150 s had burned 0.12 s of worker
   CPU: blocked, not spinning). (The first RP3.13 measurement read elements and so
   recorded the whole deck as unanswerable; corrected by the audit settlement, own
   re-probe 2026-09-03.) The port sizes it per phase, as r4133's own model-3 path
   does at `:1928-1930` (`solution/solution/ncim.rs:566`).
2. **`DOForceFlatStart` writes `NodeV[1..3]` on any circuit**
   (`Solution.pas:1650-1654`). On a circuit with fewer than three nodes that runs
   past the `NumNodes+1` allocation and **corrupts the DLL's heap**: measured on a
   1-phase 8-line deck (`tmp/rp313/repro_1ph_nogen.dss`) r4133 still answers
   (`converged=False`, 15 iterations, `SOURCEBUS.1 = 7199.557856794634 + 0j`,
   `LOADBUS.1 = -2432.428875690357 - 4868.236987187187j`) and then cannot `quit`.
   The port clamps to the node count (`ncim.rs:244`); on every circuit with ≥ 3
   nodes the two are identical, and the port reproduces r4133's two node values
   bit for bit, which is what proves the overrun did not perturb its own answer.
3. **`TGeneratorObj.GetCurrents`' NCIM arm fills only conductors `1..NPhases`**
   (`R4133:PCElements/generator.pas:1408-1409`), leaving the rest of the caller's
   buffer untouched — visible as shared-`cBuffer` leftovers in r4133's own
   `Export Currents`: `5.32907E-015` in conductor 4 of `modes/ncim/ncim_pv_pq`'s
   generator and `853.417 A` in conductor 4 of all three Kundur generators. Its API
   path zero-fills and returns `0.0`, which is what the corpus gate compares. Not
   reproduced: the port zeroes the tail (`elements/pc/generator/accessors.rs:363`),
   which is also what it emitted before the port gained the arm.
4. **`TVsourceObj.CalcInjCurrAtBus` adds PC-element terminal currents where it
   subtracts the PD ones**, so the swing source's NCIM-reported current violates
   KCL (found by the RP3.13 audit settlement, 2026-09-03). The PD loop is
   `csub(Curr[j], …)` (`R4133:PCElements/VSource.pas:1135`), the PC loop
   `cadd(Curr[j], …)` (`:1169`). Every OpenDSS `GetCurrents` returns the current
   flowing *into* the element — `TPCElement.GetCurrents`' own header says so, and
   it is what makes a load report `+P` and a generator `−P`; the NCIM generator
   stamp `Iterminal[j+1] := cnegate(conjg(cdiv(cmplx(Pnominalperphase,
   deltaQNom[j]), Volt)))` (`Common/Solution.pas:2108`) is the same convention —
   so KCL at the bus is `I(source) + Σ I(others) = 0` and **both** loops must
   subtract. Measured live on a deck with a 1000 kW / 400 kvar load bonded onto
   the swing bus (`tmp/rp313/settle/swing_pc.dss`; both engines diverge
   identically there — any PC element on the slack node breaks NCIM's slack
   constraint — and agree on every node voltage and on the `Line`/`Load` terminal
   currents to the digit): r4133 reports `Vsource.source I1 = -12.910456091137 +
   52.625721242876j A` (`54.1862 ∠103.78°`), which is exactly
   `-I(Line.l1 t1) + I(Load.ldswing)` and leaves a KCL residual of
   `85.035098 - 43.071927j A` = precisely `2·I(Load.ldswing)`. Not reproduced: the
   port subtracts both loops (`solution/solution/ncim.rs`), reads
   `-97.945554 + 95.697649j A` (`Export Currents` prints `136.936 ∠135.67`) and
   closes KCL to `< 1e-9 A` — pinned by
   `exec::tests::ncim::ncim_swing_sum_subtracts_pc_terminals_and_closes_kcl`,
   which names both engines' rows. Zero gated exposure: the divergence needs a PC
   element other than the source on the slack node and no gated NCIM case has one
   (`exec::tests::ncim::ncim_swing_bus_carries_no_pc_element_on_the_gated_decks`),
   so no ledger entry and no golden byte moves. The same routine carries two
   further defects the port also refuses — the PC loop's `myTerm` is reset once
   before the loop rather than per element (`:1146` vs the PD loop's `:1119`), so
   its terminal-finder accumulates across PC elements and can index past
   `SetLength(ElmCurrents, Yorder+1)`; and its per-terminal stride is `NPhases`
   where the PD loop uses `Round(Yorder/2)`. Upstream report
   `investigations/to_opendss/54-ncim-calcinjcurratbus-pc-sign.md`.

**Where the port is being corrected, not r4133.** The same sub-step fixed two port
bugs of its own — the missing NCIM reporting arm (the port reported the *declared*
`kvar` through `varBase`/`YQFixed` instead of the dispatched `deltaQNom`:
`Generator.G1` on `modes/ncim/ncim_pv_pq` read `-800.0, -431.8` kW/kvar /
`42.0103 A ∠151.09°` against r4133's `-800.0, -1500.0` / `78.5593 A ∠117.52°`, a
1068.2 kvar KCL miss at `genbus`) and the two panics the overruns above map to.
**A third port gap, first recorded open and then closed inside the same sub-step
(2026-09-03):** the port had no counterpart of `TVsourceObj.GetCurrents`' NCIM arm
(`R4133:VSource.pas:1194`), so the ordinary `Export Currents` reported
`YPrim·V − Iinj`, which cancels at the ideal-EMF swing bus: `Vsource.SOURCE` phase-A
magnitude was `3.24074e-05` / `3.24074e-05` / `4.42577e-05` / `0.0106809` A on
`ncim_pq` / `ncim_pv_pq` / `ncim_midi` / `Kundur2Area` against r4133's `90.0718` /
`64.2127` / `124.964` / `20295.6`. RP3.13 ported `CalcInjCurrAtBus`
(`R4133:VSource.pas:1085`) as `solution::solution::ncim::ncim_stamp_swing_source_currents`,
which `do_ncim_solution` calls once after the Newton loop and stamps into the
source's `Iterminal`, and `elements/pc/vsource/solve.rs`'s new NCIM arm echoes that
stamp — so `Export Currents` now prints `90.0718 ∠158.27` / `64.2127 ∠-150.28` /
`124.964 ∠161.49`, digit-identical to r4133's own `EXP_CURRENTS.CSV` rows (pin
`exec::tests::ncim::ncim_vsource_export_currents_match_oracle`). With that the
`exec/view.rs` override `ncim_swing_source_currents` — the gate's private reader,
pinned by `exec::tests::ncim::ncim_vsource_reported_currents_match_oracle` (the
table further up this section) — is **deleted**: its values are unchanged bit for
bit and are now produced by the element path itself. No gated channel moved, no
ledger entry, no golden byte.

---

## Rung-1 EXITED — `known_diffs.json` burn-down (WP-U1.10)

The Rung-1 exit swept the port against official EPRI **r4088** (and r3723 for the
prune criterion) with `DSS_LIVE_OPENDSS_ASSERT=1` and drove **both** green: every
remaining Rust↔EPRI divergence is a justified `known_diffs.json` entry — a
cross-solver FPC(0.14.5)↔Delphi floor, Delphi property-display precision
(the Delphi/FPC `Format`/`Str` last-digit rendering §1.3-2 relaxes to
numeric-token comparison), or an oracle-can't-run `skip` — or a documented
Rung-2 item. None is a
divergence DECISION in this ledger's sense (no adopt/reproduce arbitration); they
are the report-only inventory. Proof that none is a Rung-1 regression: every
swept case is also in the mandatory gate vs the pinned 0.14.5 oracle (green), so
the port equals the FPC oracle and the r4088 gap is purely the FPC↔Delphi layer,
corroborated by `sweeps/capi015_vs_r4088.md`.

The full entry-by-entry burn-down (11→22 entries: pruned
`epri-gendispatcher-propname`, +5 extended to r4088, +7 new numeric floors, +1
r3723-only, +4 skip) is in **`docs/upgrade/known_diffs_burndown.md`**.

## Rung-2 EXITED — the r4133 parity claim + `known_diffs.json` burn-down (WP-U2.6)

The Rung-2 exit swept the port against official EPRI **r4133** (11.0.0.1
"Charlottesville") with `DSS_LIVE_OPENDSS_ASSERT=1` and drove it **GREEN**, and
re-ran **r4088** as the direction sanity-check. Every remaining Rust↔r4133
divergence is a documented `known_diffs.json` class — an FPC(0.14.5)↔Delphi
last-ulp/display-precision floor, a dss_capi property-system format difference, or
an oracle-can't-run `skip` — and every case the Rung-2 protection overhaul moved
is gated in the **mandatory** gate against its own `oracle: "r4133"` target
(excluded from the sweep). **Zero unexplained, zero "not yet ported."**

### r4133 ASSERT sweep — GREEN

`326 matched · 70 known-diverged · 4 known-skipped · 0 NEW` (of 400; **103**
target-rev cases excluded — the Rung-2 protection flips + the Rung-1 capi015
flips + the L2 clamp deck, all gated in the mandatory gate). Of the 70
known-diverged, 16 are the two format classes and 54 are the FPC↔Delphi
last-ulp / display-precision floors now cataloged for r4133 (44 via entries
extended to r4133 + 10 via the 3 new entries; the 4 EPRI-DLL crash cases are the
separate known-skipped tally — see below). The two
format classes: `property-format-brackets` ×10 (dss_capi's bracketed
numeric-array PropertyValue render, e.g. sensor `[ 100 90 80]` vs EPRI
`100,90,80` — the class STATUS §WP-U2.5 flagged as still-live on r4133; **this is
its closure**) and `eventlog-trailing-space` ×6 (EPRI's trailing space after
InvControl event text — the class WP-U2.3 deferred here; it still has 6 r4133
witnesses on non-protection InvControl decks, so it **keeps** its r4133 tag, not
retired). Known-skipped: the four EPRI-DLL crash decks (`shape_binfiles`,
`IEEE13_LineSpacing`, `IEEE13_LineAndCableSpacing`, `CapControlFollow` — all
#303 access violations on the r4133 binary, same as r4088).

### Direction check — r4088 re-run

`329 matched · 67 known-diverged · 4 known-skipped · 0 NEW` (103 excluded, same
set; ASSERT green). The port (now r4133-behavior on the flipped cases, shared
0.14.5=r4088=r4133 behavior elsewhere) diverges from r4088 on exactly the same
FPC↔Delphi floor classes as it does from r4133, minus the r4133-only
**IEEE_519 harmonics** move and plus the r4088-only `harmonics-yfingerprint-drift`
(Y-trace) witness — precisely the r4088→r4133 delta this rung owns, confirming
the direction. The Rung-2 protection deltas themselves are invisible to both
sweeps (excluded), gated in the mandatory gate. Summary committed at
`docs/upgrade/sweeps/` and `known_diffs_burndown.md`.

### The 58 r4133 NEW divergences — all dispositioned (nothing masked)

Every one was proven a legitimate class, corroborated by the **mandatory gate
being green** (the port equals the pinned 0.14.5 oracle on all 58 → the r4133 gap
is purely the FPC↔Delphi layer, never a Rung-2 regression):

- **48** (44 `diff` + 4 `skip` cases) map to an existing r4088-tagged floor/skip
  entry on a path that is **behaviorally identical r4088=r4133** (source-verified:
  `PCElements/`, `Meters/`, injection assembly, reduction, ckt24 feeder all
  byte-identical; the solver `Common/Solution.pas` differs only in inert
  progress-form plumbing + a commented-out debug `WriteLn`, and `PDElements/AutoTrans.pas`
  only in two read-only PropertyHelp strings — numerically inert) → the entry's `revs` extended to include
  `r4133` (14 entries): `iteration-count-delta`, `injection-fpc-delphi-ulp`,
  `regcontrol-autotrans-typecast` (renamed 2026-09-03 from
  `autotrans-regcontrol-tap`; see the correction below),
  `pvsystem-kvar-display-precision`,
  `storage-kwhstored-drift`, `storage-kw-display-precision`,
  `makeposseq-fpc-delphi`, `reduce-fpc-delphi`, `ckt24-regcontrol-conditioning`,
  `vsource-nearzero-power`, `epri-binaryshape-crash`, `epri-linespacing-r4088-crash`,
  `epri-linecablespacing-r4088-crash`, `epri-capcontrolfollow-r4088`.
- **10 cases → 3 new entries** for classes first witnessed this sweep (all present
  on r4088 too — byte-identical source — so tagged `r4088`+`r4133`, except the
  r4133-only IEEE_519 move):
  - `storage-pctstored-display-precision` — Storage `%stored` rendered to 6 sf by
    Delphi (`75.089575→75.0896`); the display-precision class (§1.3-2), sibling to
    the `.kw`/`kvar` entries.
  - `monitor-seq-magnitude-drift` — the sequence-magnitude monitor channel (V2)
    FPC↔Delphi Fortescue-transform last-ulp (rel ~1.1e-5); `Meters/Monitor.pas`
    byte-identical r4088=r4133.
  - `harmonics-ieee519-r4133` — the r4088→r4133 harmonics voltage move on IEEE_519
    (see below). **r4133-only.**
- **2 entries narrowed** (empirically 0 hits on r4088 **and** r4133, so their
  `revs` dropped to `r3723`): `monitor-header-whitespace` (the harness
  `compare_monitor` now normalizes the Delphi leading-space header directly —
  WP-U2.1 audit fix — so it never surfaces on the EPRI channel) and
  `meter-zonepce-count` (its six witness decks now MATCH both EPRI revs).

### Correction (2026-09-03, RP3.12) — `autotrans-regcontrol-tap` was not a floor

The AutoTrans+RegControl entry in the 48 above was filed as an FPC↔Delphi
last-ulp floor ("the reg-tap resolves on a different discrete step"). The
*measurement* stands, the *class* does not, and "conditioning" was never proven by
decomposition — which CLAUDE.md requires before that label is accepted. RP3.12
decomposed it on the live r4133 DLL: **EPRI's `RegControl` never taps an
`AutoTrans` at all.** It reads the controlled element through an unchecked
`TTransfObj(ControlledElement)` typecast (`Version8/Source/Controls/RegControl.pas:926`,
`:1026`, `:1296`, `:1370`, `:1479`) while `TAutoTransObj = class(TPDElement)`
(`Version8/Source/PDElements/AutoTrans.pas:88`) is not one and `TAutoWinding`
(`AutoTrans.pas:59`) stops matching `TWinding` (`Transformer.pas:62`) after
`Rdcohms`, so `TapIncrement` reads the winding's `MaxTap` (1.1 pu) and
`PendingTapChange := Round(BoostNeeded/Increment)*Increment` (`:1249-1250`) zeroes
every realistic boost. Evidence: 0 event-log lines on all four
`controls:autotrans/*` decks vs 10–13 on the port and the 0.14.5 oracle; the
regulated bus left BELOW its band — `LOW.1`/166 = 118.04 V against `vreg=120 band=2`
(0.96 V under the 119 V edge, 1.96 V under the setpoint) and `AT69.1`/332 = 119.25 V
against `vreg=123 band=1.5` (3.00 V under the 122.25 V edge, 3.75 V under the
setpoint) — with 5–11 taps unused; `? RegControl.rat.tapnum`
tracks `Round(puTap/maxtap)` exactly when `maxtap` is edited; `tapnum=0` renders
`taps=[1, 1.58101E-322]`, the winding's `NumTaps = 32` reinterpreted as a Double.
The same four decks with the RegControl disabled make the port print EPRI's
census literals **byte for byte**.

Consequences: the entry's cause key is renamed **`regcontrol-autotrans-typecast`**
and rewritten (`tests/corpus/ledger.json`); the class is `UPSTREAM_BUG`, present
identically in r3723/r4088/r4133 and absent from dss_capi 0.14.5 (shared
`TControlledTransformerObj` base); per CLAUDE.md (2026-08-02) it is **not
reproduced in any lane** — the port keeps the regulated answer, pinned by
`props_r4133_pins::autotrans_wdgcurrents_stay_regulated_where_r4133_never_taps_the_autotrans`.
The four decks are `engines: "capi_v0145"`, so nothing is gated on the r4133
channel today and the case-level `skip` entries are drafted, not landed. Upstream
report: `investigations/to_opendss/50-regcontrol-autotrans-ttransfobj-typecast.md`.
The AutoTrans share of `iteration-count-delta` (6-vs-3 on `autotrans_both`/`_reg`)
is the same defect seen from the solver: r4133 iterates three times because no
control ever arms.

### IEEE_519 harmonics — source-confirmed "nothing to port"

The WP-U0.2 sweep flagged an r4088→r4133 harmonics-mode voltage move on IEEE_519
(V ~4.3e-4 @ pcc, assembled Y bit-identical) that WP-U2.6 had to "source-confirm
or catalog." **Source-confirmed:** `Common/SolutionAlgs.pas` (harmonics driver),
`PCElements/Load.pas`, `General/Spectrum.pas` and `Common/YMatrix.pas` are all
**byte-identical r4088=r4133** (`Common/Solution.pas` differs only in progress-form
plumbing + commented-out debug `WriteLn`), so **no harmonics/injection algorithm
changed** — there is nothing to port. The move is determinism-proven per engine
(r4088↔r4088 and r4133↔r4133 both match, `sweeps/r4088_vs_r4133.md` §Surprises)
⇒ a build-to-build Delphi last-ulp drift amplified by the near-resonance of the
519 filter (an ill-conditioned harmonics fixpoint). The port matches the pinned
FPC 0.14.5 oracle (mandatory gate green) and its own physically-correct harmonics
solution; cataloged as `harmonics-ieee519-r4133` (documented upstream, no port
action). The InductionMachine converged-flip surprise was already resolved by
WP-U2.1 (`InductionMachine/{Master,Run}` moved to
`skipped_needs_investigation` — the port reproduces the r4133 non-convergence).

### Net catalog state at Rung-2 exit (25 entries)

r4133-applicable: 19 (15 `diff` + 4 `skip`). r4088-applicable: 19. r3723-applicable:
19. No divergence decision lives only in a commit message; the entry-by-entry
Rung-2 burn-down is in `docs/upgrade/known_diffs_burndown.md`. **Engine behavior =
OpenDSS 11.0.0.1 (r4133) except this documented ledger.**

---

## L5 — Generator swing-damping default (`Dpu`) — MEASURED (WASM-UM WP-WM.3, 2026-07-19)

**Observable.** A dynamics-mode `Generator` (`model=6` or the classic
swing-integrated models) with no explicit `D=` property: the swing-equation
damping term `D*Speed` in `dSpeed := (Pshaft + TracePower.re − D*Speed)/Mmass`
(`generator.pas` `IntegrateStates`).

**dss_capi 0.14.5 (dss-rs spec / pinned oracle).** The constructor sets
`GenVars.Dpu := 1.0` (`generator.pas:1006`); `InitStateVars` derives
`D := Dpu*kVArating*1000/w0` (`:2437`) ⇒ for `kVA=5000, w0=2π·60`, **D≈13263**
(heavy damping). dss-rs reproduces this (default `Dpu=1.0`).

**EPRI r4133.** The constructor sets `D := 1.0` **directly** (`generator.pas:968`)
but **never initialises `Dpu`**, so the managed record field defaults to `0`; the
same `InitStateVars` line (`:2710`) then recomputes `D := Dpu*kVArating*1000/w0 =
0` ⇒ **D=0** (undamped). The `:968` `D:=1.0` is dead (immediately overwritten).

**Probe** (WM.3 dyn deck, native twin via the r4133 bridge; oracle `DebugTrace`
`GEN_g1.CSV`): first predictor `dSpeed`: **r4133 default = +0.0025** (D≈0) vs
**Rust/dss_capi = −1.165** (D≈13263 dominating). Setting `D=1` explicitly on both
makes r4133 report the damped `dSpeed=−71.3` matching Rust's class.

**Decision — record, do not "fix".** dss-rs is a 1:1 port of dss_capi 0.14.5 and
gates on the pinned dss-python 0.14.5 oracle everywhere else; changing the
Generator default to r4133's `Dpu=0` would regress that entire (green) gate. The
WM.3 dyn deck pins `D=1` explicitly so both engines agree (removing the confound).

**Gate consequence.** WASM-UM `wasm_gen_dyn` gates the version-independent
state-variable surface + convergence (not the damping-dependent trajectory —
`wasm_usermodels.rs` `gate_deck(.., numeric=false)`; STATUS §WASM-UM WM.3). A
future UPGRADE-to-r4133-dynamics rung must revisit this default (and the residual
~5e-4 flux-transient gap D2 that survives even with D matched — see STATUS).

---

## L6 — a designated-but-unloadable `UserModel=` suppresses #567/#5671 — SETTLED (WASM-UM convention; recorded by the R4133_PROPS RP1.3 audit settlement, 2026-08-23)

**Observable.** A PC element whose model selector names the user-written model
(`Generator`/`WindGen` `Model=6`, `PVSystem`/`Storage` `Model=3`) AND whose
`UserModel=` names something the wasm host cannot load — in practice a native
`.dll` (permanently out of reach under `#![forbid(unsafe_code)]`) or a missing
file. Two diagnostics and, in dynamics, the abort that follows them.

**EPRI r4133.** `Set_Name`'s `LoadLibrary` fails ⇒ `DoSimpleMsg('… Not Loaded …',
570)` once (`WindGenUserModel.pas:187`, `GenUserModel.pas` twin) and
`UserModel.Exists` stays FALSE. Every later call site then takes its
missing-model arm: `DoUserModel` emits **#567 per power-flow iteration**
(`WindGen.pas:1895`, `generator.pas:1834`) and `DoDynamicMode` emits **#5671 +
`SolutionAbort := TRUE`** (`WindGen.pas:1996-1997`, `generator.pas:1940-1943`),
i.e. the dynamics solve stops.

**dss-rs.** The #570 (or #569 for a bad export set) is emitted once at load time,
exactly as upstream. After that the port **suppresses** the repeat #567 and the
#5671 abort whenever the name is non-empty — `windgen/solve.rs::do_user_model`,
`windgen/dynamics.rs::do_dynamic_mode` and the WM.3/WM.4 twins all gate their
missing-model arm on `user_model_name.is_empty()`. A `Model=6` element naming a
native DLL therefore runs on its Yprim contribution in power flow and does not
abort in dynamics. Where NO model is named at all, both diagnostics fire exactly
as upstream (pinned:
`exec::tests::windgen_usermodel::model_6_without_a_user_model_logs_567_and_keeps_solving`
and `…_dynamics_without_a_user_model_logs_5671_and_aborts`).

**Evidence.** r4133 source lines above; the port side is pinned by
`exec::tests::windgen_usermodel::a_missing_wasm_warns_570_and_leaves_the_slot_absent`
(exactly one #570, non-abort, no #567, slot absent). There is no probe to run
against the oracle: no native WindGen user model exists anywhere upstream (r4133
ships only the loader), so the r4133 bridge cannot exhibit the WindGen half at
all.

**Decision — keep the suppression; record it here.** The native-DLL case is
structural for this engine, not a modelling choice: the name can never load, so
#570 is the one true, actionable diagnostic and repeating #567 every iteration
would be a port-specific artifact in the error stream. The dynamics half matters
more: the vendored corpus contains decks that name native DLLs, and following
r4133 there would abort them wholesale (a solve the oracle completes with its
DLL), destroying the rest of the deck's comparison for a model we cannot run
either way. The related convention rides with it: `? …UserModel` echoes the name
that was **attempted**, whereas r4133 assigns `FName` only on success
(`WindGenUserModel.pas:190`) and would answer `''`; the suppression is keyed on
exactly that stored name.

**Gate consequence.** None today: no corpus deck sets `UserModel=` on a WindGen,
and neither oracle channel is exposed on any of the five `modes:windgen/*` decks
(all `engines: "r4133"`, all `model=1`). No ledger entry, no allowlist row. If a
deck that names a native DLL is ever gated on a channel where the oracle DOES
load it, the divergence is a whole-element one (their model runs, ours cannot)
and must be excluded case-by-case rather than papered over here.

## L7 — WindGen `QMode=0` dispatches `kvarBase` (r4133 zero-var bug, not reproduced) — R4133_PROPS RP3.10, 2026-09-04

**Observable.** A `WindGen` that does not type `QMode=` — i.e. every default
one — injects **zero vars** in power flow on r4133 however its `kvar=`, `pf=` or
`kVA=` reads. The port dispatches the machine's `kvarBase` instead, so the two
engines diverge across the solved model of any deck that declares a WindGen with
a non-zero base.

**EPRI r4133.** `TWindGenObj.SetNominalGeneration`'s steady-state
`case WindModelDyn.QMode` (`Version8/Source/PCElements/WindGen.pas:1276-1322`)
implements arm 1 (PF, `:1277-1288`) and arm 2 (volt-var, `:1289-1319`) and has
**no arm 0**, so mode 0 falls through to `Else kvarCalc := 0` (`:1320-1321`) and
`:1325` then stores `Qnominalperphase := 1e3 * 0 * … = 0`. `QMode` *defaults* to
0 (`Create`, `:1020`), the property help documents it as
`'Q control mode (0:Q, 1:PF, 2:VV).'` (`:429-430`), and the dynamics model
spells the same mode `QMode := 0; // 0 -> Constant Q` (`WTG3_Model.pas:252`),
implemented as `Qord := Qref` (`:1059-1061`). It is an omission, not a design:
arm 1's own saturation fallback **is** `kvarBase` (`:1284`), arm 2 is `kvarBase`
scaled by the VV curve and saturated at `|kvarBase|` (`:1313-1316`), models 4/5
inject `varBase = 1000*kvarBase/Fnphases` unconditionally (`:1361`, `:1797`,
comment `:1775` *"Q is always kvarBase"*), and the parent class writes the
dispatch outright (`Generator.pas:1163`) — which WindGen could not transcribe
because its `ShapeFactor` carries the wind **speed**, not a pu multiplier
(`:1241`). A complete 20-hit `Qnominalperphase` write census leaves no other
filler for mode 0: `:1245` (turbine off → 0), `:1325` (the dispatch) and the two
seeds `:3002`/`:3025`, both overwritten by `:1325` before any solve;
`InitDQDVCalc`/`BumpUpQ`/`ResetStartPoint` have no WindGen caller
(`Common/Solution.pas:963-1000` walks `Generators` only).

**Live measurement** (EPRI r4133 DLL 11.0.0.1 through `epri-worker`, RP3.10's
probe and its `DSS_GATE_ONLY=windgen` gate run). Under `QMode=0` r4133
dispatches **exactly 0** on all five `modes:windgen/*` corpus decks and in every
configuration probed, including decks that type `kvar=`, `pf=` **and** `kVA=`
(`kW=1000 pf=0.9`, `kW=1000 kvar=±400` all give
`P = -1000.0000016773234, Q = -8.437050548309344e-06`); `edit … QMode=0` is
inert and re-entrant. The same engine dispatches the base the moment the `case`
is bypassed or an arm exists — `model=4`/`DoFixedQGen` gives
`-726.4287342535065` on `windgen_snap_delta` and `-985.9891404764404` on
`windgen_daily`, and arm 2 with a flat `y=+1` volt-var curve gives exactly
`kvarBase`. Per deck, port against r4133:

| deck | `kvar_base` (kvar) | port `q_nominal_per_phase` (VAr) | port terminal Q (kvar) | r4133 terminal Q (kvar) |
|---|---|---|---|---|
| `modes:windgen/windgen_snap_delta.dss` | 726.4831572567788 | 242161.05241892627 | −726.4838 | −4.216133426461965e-05 |
| `modes:windgen/windgen_daily.dss` | 986.0523155365896 | 328684.1051788632 | −986.0531 | −2.1275018134247148e-05 |
| `modes:windgen/windgen_dyn.dss` | 854.95263026673 | 284984.21008891 | −37077.425 | −37087.759 |
| `modes:windgen/windgen_dyn_fault.dss` | 854.95263026673 | 284984.21008891 | −29209.383 | −29216.667 |
| `modes:windgen/windgen_snap.dss` | 0 (`pf=1.0`) | 0 | — | — (no divergence) |

`WindGen.pas:1254` skips the whole P/Q block in dynamics, so the two dynamics
decks move only through the snapshot `solve` their own deck line performs before
`Set mode=dynamic`; `windgen_snap` types `pf=1.0`, so its base is 0 and the arm
is a literal no-op there.

**Decision — fix it in both lanes, never reproduce it** (CLAUDE.md's 2026-08-02
policy). `crates/dss-core/src/elements/pc/windgen/nominal.rs` gains one
unconditional arm — `kvar_base` read raw where `kVA=` is unset (it already
carries the `pf` sign there), `|kvar_base|` with `lead_lag = -1` for
`PFNominal < 0` where a typed `kVA=` has stripped it, which is arm 1's own sign
discipline (`:1286-1287`) — with no `cfg`, no
`compat::` alias and no `kVArating` clamp: arm 1's saturation test cannot fire,
because `kVATmp = sqrt(Pg² + kvarCalc²)` is `Pg/|PF|` while `Pg` is capped at
`kWBase` (`:1268-1269`), so `kVATmp <= kWBase/|PF| == kVArating` bit-for-bit
(measured on `3000/0.95`, `1584/0.88` and `1080/0.9`). Taking `kvar_base` raw
instead would dispatch the opposite sign whenever a deck types `kVA=`:
`RecalcElementData`'s kVA-set branch re-derives `kvarBase := sqrt(kVArating² −
kWBase²)` (`:1377-1378`), a non-negative root that strips the sign a typed
`pf<0`/`kvar<0` put there. Probed live on r4133 for `kW=1000 kVA=1200 pf=-0.9`:
arm 1 gives terminal `Q = +523.0678462465884` kvar (the machine **absorbs** —
the documented meaning of `pf<0`) while upstream's own `model=4`, reading the
stripped base through `varBase`, gives `-523.0475290915948` (injecting); this
engine's mode 0 follows arm 1, pinned by
`::qmode0_dispatch_carries_the_sign_and_scales_with_genmult`. `Factor`
(`GenMultiplier`) still applies because `:1325` sits outside the `case`. The `_`
arm keeps upstream's `Else` for out-of-range modes.

The dispatch feeds `Yeq := (Pnominalperphase − j·Qnominalperphase)/Vbase²`
(`:1338`, with `Yeq95`/`Yeq105` derived from it), so the arm moves **every**
WindGen `GenModel` that reads `Yeq` — not only the constant-P/Q model 1 the
corpus exercises: model 2 (constant Z) takes its whole current from `Yeq`, and
model 6 (user model) seeds `InjCurrent` from the same `YPrim`. Models 4/5
(`DoFixedQGen`/`DoFixedQZGen`) read `varBase` instead and are untouched; models
3/7 are out of scope in the port and fall into its constant-PQ arm. Every
`modes:windgen` deck declares `model=1`, so nothing beyond the four gated decks
moves in the measured population — the exposure table above is complete for what
the gate compares, not a claim that mode 0 only touches model 1. `lane_diff` measured `max |Δ| = 0` on every gated kind, so
both lanes compute the identical `q_nominal_per_phase`.

**Exclusions and pins.** The four decks' divergence is excluded field-by-field
in `tests/corpus/ledger.json` — `windgen-qmode0-constant-q-{snapdelta,daily,dyn,dynfault}-r4133`,
`kind: exclusion`, channel `r4133`, cause `windgen-qmode0-no-arm` — and pinned by
`elements::pc::windgen::tests::qmode0_dispatches_the_base_kvar`,
`::qmode0_dispatch_carries_the_sign_and_scales_with_genmult`,
`::qmode0_zero_only_when_the_base_is_zero` and
`::dynamics_variables_match_the_qmode0_dispatch`, each naming both engines'
numbers, plus the re-centred
`exec::tests::force_hooks::windgen_force_inj_freezes_iterminal`;
`every_rp310_windgen_pin_exists_and_is_cited`
(`crates/dss-core/tests/props_r4133_replay.rs`) keeps those citations from
drifting. `modes:windgen/windgen_snap.dss` gets **no** entry — nothing moves
there. The property side is untouched: r4133's `kvar` getter renders the
*dispatched* Q (`:2896`, RP3.2's finding) while the port renders `kvar_base`,
which this dispatch never writes. Upstream report:
`investigations/to_opendss/55-windgen-qmode0-zero-var-dispatch.md` (local-only).
Full record: `docs/phase-records/r4133-props-rp3.md` §RP3.10.

## L8 — CapControl TIMECONTROL binds its bus to the MONITORED element's terminal (capi 0.14.5 uses the capacitor's bus) — GOLDEN_REBASE G1.3a, 2026-09-04

**Observable.** A `type=time` CapControl that names an `element=` reports its own
terminal-1 voltages at the *monitored* element's terminal. The pinned dss_capi
0.14.5 reports them at the *controlled capacitor's* bus instead. On
`tests/corpus/controls/capcontrol/capcontrol_time.dss`
(`line.lf bus1=src bus2=b`, both capacitors on `b`, both banks
`element=line.lf terminal=1`) `CapControl.cc1.VoltagesMagAng[0]` is
**7342.020904321447 V** (bus `src`) on the port and on r4133, and
**7276.216225426737 V** (bus `b`) on capi 0.14.5 — 65.80467889471038 V, 0.90 %
apart, i.e. a different bus, not a numeric gap.

**EPRI r4133 (the authority).** `Version8/Source/Controls/CapControl.pas:605`
computes `ElmReq := ElmReq and (ControlType <> FOLLOWCONTROL)`, so **only**
FOLLOWCONTROL skips the monitored element; with one present the control binds
`Setbus(1, MonitoredElement.GetBus(ElementTerminal))` (`:622`) and sizes
`cBuffer`/`CondOffset` off that element (`:624`/`:625`). The
`ControlledElement.GetBus(1)` arm at `:633` is the no-monitored-element branch
only. **capi 0.14.5** (`.inputs/dss_capi/src/Controls/CapControl.pas:597-608`)
still carries the pre-`b9bc87b8` form: for TIMECONTROL *and* FOLLOWCONTROL it
sets `effElement := ControlledElement` and forces `ElementTerminal := 1`, then
`Setbus(1, effElement.GetBus(ElementTerminal))` at `:619`.

**Decision — port follows r4133; not adopted from the 0.15.x side alone.** This
is the bus half of the split whose *readback* half (`effElement`/`Terminal`) was
already adopted at UPGRADE WP-U1.6 as **D11 (part 2)** above, on r4133 +
capi015-probe evidence; the port's `control_type != Follow` arm
(`crates/dss-core/src/elements/control/cap_control/mod.rs`) implements both
halves at once, so no engine code moved here. What is new in G1.3a is the
*gate*: `CktElement.VoltagesMagAng` reads `NodeV` through the element's own
`NodeRef`, so it is the first live channel that can see which bus a control sat
down on. Nothing is masked silently — the `capi_v0145` divergence is excluded
field-by-field by `tests/corpus/ledger.json` entry
`capi-capcontrol-time-bus-is-the-capacitors` (`element` sub-channel
`voltages_mag_ang`, `name_re` the two CapControls only, cause
`capcontrol-time-bus-is-the-capacitors`), the **r4133 channel of the same case
needs no entry at all**, and both numbers are pinned by
`dss_core::exec::tests::derived_polar::capcontrol_time_voltages_follow_the_monitored_elements_terminal`.
No EPRI report is owed — r4133 is the side that is right.

## D12/D14 — GICTransformer decks gate on **r4133 only**; the pinned capi 0.14.5 oracle is nondeterministic on them — GOLDEN_REBASE G1.4a, 2026-09-04

*(`D12`/`D14` here are the **coordinator session decisions** of the 2026-09-04 GOLDEN_REBASE
run, not this file's own upstream-divergence rows `D12` (SwtControl `Normal`/`State`, §WP-U1.6)
and `D14` (DynamicExp RPN evaluator) above. Every citation elsewhere written
"`DIVERGENCES.md` §D12/D14" means this section.)*

**Not an engine divergence: an oracle-channel decision.** No port behavior
changes here, no tolerance moves, no golden byte moves.

**The finding (D12).** The pinned dss-python 0.15.7 / dss_capi 0.14.5 oracle
disagrees with **itself** across fresh processes on any deck that instantiates a
`GICTransformer`. Measured on lane `lane-b` with fresh one-shot
`tools/oracle/oracle_server.py` processes over the same deck text:
**7 bad runs of 60** with the `new gictransformer.…` line present, **0 of 40**
with that one line deleted. A bad run is not noise in the last ulp — the whole
no-load solve lands elsewhere (`src` at 2332 V instead of ~7199 V) and
`Bus.kVBase` is punted to 0. EPRI r4133 through `epri-worker` is deterministic
over the same experiment: **80/80** bit-identical solved node voltages plus every
`(bus, kVBase)` (20 fresh one-shot runs × the four decks), and **20/20** on the
new deck below. An oracle channel that disagrees with itself cannot gate, and
CLAUDE.md's policy says so: *"where r4133 does not share the bug, prefer gating
the affected case on the r4133 channel"*. The decision does not depend on the
root cause — the measurement alone disqualifies the channel — but the cause was
**measured** afterwards (2026-09-04, same sub-step): the element's `YPrim` and
the assembled system `Y` are bit-identical across processes, and the divergence
sits in `SetVoltageBases`' zero-load snapshot
(`.inputs/dss_capi/src/Common/Solution.pas:1083` → `:1025`-`:1051` → `:1103`),
where `NodeV` is `ReAllocMem`'d and only `NodeV[0]` zeroed
(`Common/YMatrix.pas:416`), `SolveSystem`'s return code is discarded, and
whatever a node happens to hold is read straight into `nearestBasekV` — which
punts `kVBase` to 0. r4133 carries the identical code
(`Version8/Source/Common/Solution.pas:2486-2514`, `:2541`;
`Common/YMatrix.pas:242-245`), so this is a latent upstream defect whose
realization is capi-process-specific (`investigations/issue-37-…`, local-only).

**Why the shunt deck is SPLIT, not flipped (D14).** Three of the four affected
decks (`asymmetric:gic/gictransformer_gic.dss`, `asymmetric:gic/gic_midi.dss`,
`solvable_now:Version8/Distrib/Examples/GICExample/GIC_Example.dss`) were `both`
and flip to `r4133` at zero cost. The fourth,
`modes:makeposseq/makeposseq_shunt.dss`, is a WPG.21 `MakePosSequence` deck gated
on capi alone, and moving it to r4133 would have quantized the very outputs it
exists to check: **every r4133 `MakePosSequence` override builds a command STRING
with `Format('%-.5g')` and re-parses it** — `Version8/Source/PDElements/
Capacitor.pas:801`, `:806`, `:829` into the parser at `:834-835`, and the same
shape in `PCElements/Vsource.pas:1396-1402`, `PCElements/Load.pas:2305-2332`,
`PDElements/Line.pas:1591-1596` — where the pinned dss_capi 0.14.5 and the port
aim typed setters at the same properties and keep full f64. Measured images on
that deck: reduced `kV` reads `7.1996` against the port's `7.19955785679463`
(rel 5.85e-06), `load.kw` `133.33` vs `133.333333333333` (2.50e-05), `reactor.r`
`0.26667` vs `0.266666666666667` (1.25e-05); the quantized Vsource base kV alone
moves the injection RHS by `3.407076218201843e-02` (rel 5.854e-06) with the
cmatrix capacitor deleted, while **without** the trailing `makeposseq` the two
oracles agree to f64-ulp (max `9.094947e-13`, rel `1.563e-16`). The r4133
property census on that one case returned **13 unclaimed cells / 12 pairs** plus
the model artifacts and the deck's own manifest probes — more than ten new ledger
rows for one deck, so the sub-step's kill criterion fired and the coordinator
settled on the split.

**What landed.** `makeposseq_shunt.dss` loses its single
`new gictransformer.gt busH=b1 busNH=b1.4.4.4 R1=0.1 type=GSU` line and keeps its
full-precision capi gating (re-measured: **20/20** identical capi runs without the
element, and its two ledger entries `makeposseq-cuf-applied-capi{,-props}` are
still hit). The GICTransformer's `MakePosSequence` override — `Phases=1` plus the
inherited bus strip, r4133 `PDElements/GICTransformer.pas:736-747` == pinned capi
0.14.5 `src/PDElements/GICTransformer.pas:588-593`, base `CktElement.pas:1352-1363`
— moves to the new `r4133`-gated micro deck
`tests/corpus/modes/makeposseq/makeposseq_gic.dss`, whose every **other** reduced
parameter is chosen so the 5-digit round trip is exact (`basekv = 7.2·√3` ⇒
line-neutral `7.2`, `kw=900` ⇒ `300`/phase, `pf=1` ⇒ `kvar 0`, `r1/x1/c1/normamps`
already 5-digit). It needs **zero** ledger rows: the live compare against r4133
passes clean. The four capi-side entries
`gic-pct-r2-honoured-{gictransformer,midi}-capi{,-props}` are deleted (ledger
**57 → 53**); their r4133 twins keep every port-side assertion, and no pin loses
one (`compat_quirks::gic_transformer_pct_r2_drives_winding_two`,
`props_r4133_pins::gictransformer_r2_honours_the_x_winding_percentage{,_on_the_ring}`,
`golden_reports::export_gicmvars_matches_the_equivalent_ohms_spec` read no capi
oracle). The unreferenced ledger cause `makeposseq-fpc-delphi` claimed
"transcendental last-ulp drift"; the census above shows the mechanism is the 5-sf
string round trip, so the cause text is **rewritten**, not deleted. A guard test
`corpus_gate::manifest::no_capi_gated_case_instantiates_a_gictransformer` pins
both halves — the closed set of corpus decks that `new` one, and that each gated
case naming one declares `engines: "r4133"` — with its refusal driven
non-vacuously by `…::the_gictransformer_channel_guard_refuses_a_capi_gated_deck`.
`FORCED_PROPS_POPULATION` / `FORCED_BUS_POPULATION` move
`(440, 313, 83, 44) → (441, 310, 87, 44)` (three flips + one new case), both
re-derived from the manifests on every run.

## L9 — a control re-attaches to the END of its element's `ControlElementList` on every Edit (capi 0.14.5 only on a `SwitchedObj` write) — GOLDEN_REBASE G1.3d(ii), 2026-09-05

**Observable.** `CktElement.OCPDevIndex` / `OCPDevType` / `NumControls` /
`HasVoltControl` / `HasSwitchControl` all answer from the element's
`ControlElementList`, so their answer depends on the list's ORDER whenever an
element carries controls of more than one class. Probed on a 3-phase line
carrying `Relay.r`, `Fuse.f` and `SwtControl.s` (attached in that order): all
three engines report `OCPDevType = 3` (Relay) after the build; after
`edit relay.r delay=0.05` + `solve`, **EPRI r4133 answers 1 (the Fuse)** because
the relay moved to the end of the list, while **dss_capi 0.14.5 still answers 3**.

**EPRI r4133 (the authority).** `TControlElem.Set_ControlledElement`
(`Version8/Source/Controls/ControlElem.pas:113-131`) is a remove-then-append:
`RemoveSelfFromControlElementList` (`:81-99`) rebuilds the list omitting self,
then `ControlElementList.Add(Self)` appends at the end. r4133 re-assigns
`ControlledElement := ActiveCircuit[ActorID].CktElements.Get(DevIndex)` inside
**`RecalcElementData`** — i.e. on **every** edit — for each of the six control
classes that join a list (`Controls/Relay.pas:955`, reached from `:626`;
`Recloser.pas:702`, `SwtControl.pas:332`, `CapControl.pas:580`,
`RegControl.pas:693`, `fuse.pas:470`). **capi 0.14.5** instead makes
`ControlledElement` a property-write target
(`.inputs/dss_capi/src/Controls/Relay.pas:439-441`: `PropertyOffset[SwitchedObj]
:= @obj.FControlledElement`, `PropertyWriteFunction := @SetControlledElement`),
so an edit that does not write `SwitchedObj` leaves the list order untouched.

**Decision — port follows r4133.** The port materialised no `ControlElementList`
at all (`report/show/controlled.rs` scanned `Circuit::controls`, i.e. *creation*
order, and `CktElementData::ocp_device_type` was a latch written once at
registration). G1.3d(ii) adds the list as a circuit-wide attach order
(`Circuit::control_attach_order`, maintained by `Circuit::reattach_control` at
the port's `RecalcElementData` moment) bucketed per element by
`circuit::controls::derive_control_lists`, which `Show Controlled`, the five
`CktElement` scalars and the reliability sweep's live `GetOCPDeviceType` all
share. `Circuit::controls` — the *sampling* order, which no oracle exposes and
which a reorder would move control-action event logs — is deliberately untouched.

**Nothing is masked: the divergence costs 0 ledger rows because it is not
observable on the corpus.** It needs an element carrying controls of two classes
*and* a later re-edit. **Measured over the whole gated population** (G1.3d(ii)
audit settlement, 2026-09-05, after the first census — a live capi walk of
`tests/corpus/controls/**` only — proved too narrow): of 298 565 compared
(case, channel, step, element) rows, 3 184 carry a control and **18** carry two or
more; all 18 are **Relay-only** lists — `Line.thev` under `Relay.21src` +
`Relay.21rev` in the eight Distance/TD21 relay decks, `Line.motorleads` under
`Relay.{mfrov/uv,mfr46,mfr47}` in `controls:fuse/indmach_r4133/
indmach_{snap,dyn}.dss` — so every permutation answers the same `OCPDevIndex = 1`
and `OCPDevType = 3`, and the remaining three scalars are order-free by
construction. The census is re-derived on every full run and fails on stale in
both directions (`harness::assert_no_multi_control_element`, called from the
corpus gate's epilogue). The engine numbers are pinned by
`dss_core::exec::tests::element_extras::ocp_dev_type_follows_the_last_attach_order`
(port and r4133 `1`, capi 0.14.5 `3`), with
`the_control_sampling_order_is_not_reordered_by_a_re_edit` guarding the sampling
order and `only_six_control_classes_join_an_elements_control_list` guarding the
class set. No EPRI report is owed — r4133 is the side that is right; capi 0.14.5
is a numeric oracle only.

**A second, non-divergent fact settled with it.** `GetOCPDeviceType`
(r4133 `Common/Utilities.pas:3165-3184`) has **no `Enabled` test**, so a
*disabled* OCP control still holds its slot and still wins the scan; both
channels agree (`Line.l2` with a disabled `Fuse.fd` ahead of an enabled
`Relay.rd` reads `OCPDevType = 1` on capi and on r4133). The port's registration
latch answered `3` there, so the accessors recompute from the derived list with
no `Enabled` filter anywhere — pinned by
`a_disabled_ocp_control_still_wins_the_ocp_scan`, and the same live scan replaced
the latch in the reliability sweep (`Meters/EnergyMeter.pas:2538`).
## D22 — `CalcReliabilityIndices` recomputes `TotalUpDownstreamCustomers`; dss_capi 0.14.5 dropped the call — GOLDEN_REBASE G1.6(ii), 2026-09-05

**Observable.** Only under a restoration flag: `RelCalc <restore>` (`Relcalc yes`,
`RelCalc restore=y`). With a bare `RelCalc` — the form the live gate and every
corpus deck use — all three engines are bit-identical, so this divergence is
**unreachable on the corpus: 0 ledger rows, 0 golden bytes, `population.lock.json`
untouched** (measured; the full unscoped gate is green over all 525 cases).

**Source, all four revisions.** `Version8/Source/Meters/EnergyMeter.pas`
`CalcReliabilityIndices` in **r3723, r4088 and r4133** runs, between the
`Assigned(SequenceList)` check and `// Zero reliability accumulators`, the two
statements `AssumeRestoration := AssumeRestoration_input;` and
`TotalUpDownstreamCustomers;` (r4133 `:2466-2468`, routine `:2449`, roll-up
`:1629`; the caller passes the flag as an argument and never touches the field,
`Executive/ExecHelper.pas:4440`). The pinned **dss_capi 0.14.5**
(`.inputs/dss_capi/src/Meters/EnergyMeter.pas:2411-2427`) has **neither** line —
it goes straight to the zeroing loop and assigns the flag in the caller instead
(`ExecHelper.pas:4884`) — although its own banner says it is based on SVN 3723,
the revision that carries the call. It is a **capi-only regression of the dss_capi
refactor**, not an r4133-era feature.

**Measured** (`tmp/g16ii/probe_f7_*.py`, worktree quiet, both oracles live).
Fixture A, the in-engine auto-recloser feeder under `Relcalc yes`: port **==**
r4133 `SAIDI 2.1583333333333337`, `src.N_Customers 35`, `b1.N_Customers 25`,
`b1.Cust_Interrupts 10.750000000000002`, `b1.Cust_Duration 69.64999999999999`,
section 1 `SectTotalCust 35`; dss_capi 0.14.5 returns `2.4899999999999998`, `42`,
`32`, `13.760000000000002`, `83.58`, `42`. Under `Relcalc no` all three engines
agree bit-for-bit. Fixture B, the gate's own
`tests/corpus/controls/energymeter/midi_relcalc.dss` under `RelCalc restore=y`:
port == r4133 `src.N_Customers 0`, `SectTotalCust 0`, `SAIDI 0.27623590504451034`
vs dss_capi `30`, `30`, `0.5496409495548961`; SAIFI, SAIFIkW, CustInterrupts and
every other bus column agree on all three engines under both flags.

**Decision — the port follows r4133 (CLAUDE.md: r4133 is the behavioral
authority; capi 0.14.5 is a numeric oracle only).** The two statements are
restored in `crates/dss-core/src/solution/meters/reliability.rs`
(`calc_reliability_indices`, `:107`), with the `AssumeRestoration` assignment moved
from the caller into the callee exactly as r4133 places it — which also makes the
port faithful on the error path (a meter whose zone was never built keeps the flag
it had). Nothing is masked: the one in-engine literal that pinned the capi answer
is **re-pinned with both numbers, never relaxed**, by
`dss_core::exec::tests::reliability::relcalc_assume_restoration_changes_auto_ocp_interruptions`
(whose doc carries the six-row port==r4133 vs dss_capi table), and the restored
call is exercised — so it cannot rot into dead code — by
`dss_core::exec::tests::reliability::relcalc_recomputes_the_customer_totals_it_depends_on`,
which drives the gate's own deck under both `RelCalc` and `RelCalc restore=y`.
**No EPRI report is owed** — r4133 is the side that is right; this is a capi-only
finding. Coordinator decisions **D20** (take the fix in-sub-step, "port gaps
immediately") and **D22** (ratified as measured: zero on goldens, corpus, ledger
and locks, non-zero on one dss_capi-pinned in-engine literal).

## G1.4c — Bus `SeqVoltages`/`CplxSeqVoltages` and `VLL`/`puVLL`: the port's semantics differ from **both** oracles — GOLDEN_REBASE G1.4c, 2026-09-05

*(`G1.4c` is the GOLDEN_REBASE sub-step; the coordinator decisions cited are that
session's **D4**, **D8**, **D15**/**D16** and **D21**.)*

**Three upstream defects, one surface.** On these four `IBus` arms the port, the
pinned capi 0.14.5 and EPRI r4133 give three different answers, and neither
oracle answer is defensible:

1. **Sequence quantities are gated on the node COUNT, not on the phases
   present.** r4133 `DDLL/DBus.pas:298-299` returns `-1, -1, -1` whenever
   `NumNodesThisBus <> 3` — its own comment says *"Signify seq voltages n/A for
   less then 3 phases"*, so a 4-node bus that carries phases 1, 2 and 3 is
   declined although its V012 is perfectly defined. capi 0.14.5 clamps
   `Nvalues > 3` to 3 first (`CAPI/CAPI_Alt.pas:2172-2186`) and therefore
   answers there — the two oracles split on `n > 3`.
2. **Both substitute GROUND for a missing phase.** `Vph[i] := NodeV[Find(i)]`
   with `Find` returning `0` (the ground slot) for an absent node
   (`DBus.pas:305` == `CAPI_Alt.pas:2190`), so a `[1,2,10]` bus gets a V012
   computed over a fabricated 0 V phase. Physically meaningless, and it
   contradicts the same comment.
3. **The line-to-line pairing polls the node BEFORE it wraps.** Both arms walk
   `jj` with `NodeIdxj := FindIdx(jj); if jj > 3 then jj := 1 else inc(jj)`
   (r4133 `DBus.pas:580-584`, capi `CAPI_Alt.pas:2500-2523`), so on a
   `[1,2,3,4]` bus the third pair is `V3 - V4` instead of `V3 - V1`, on
   `[1,2,10]` the second pair is the exact negative of the first, and on
   `[1,10]` r4133 pairs node 1 **with itself**. r4133's own report path wraps
   *first* (`Common/ShowResults.pas:193-194`: `k := jj; IF k > 3 Then k := 1;
   kk := FindIdx(k)`), and the commented-out original two lines below the loop
   (`DBus.pas:586-587`) are that same wrap-first code — intent contradicts
   behaviour inside one engine. r4133 additionally **hangs**: its second loop is
   an unbounded `repeat`, so a bus whose node numbers miss `{jj₀} ∪ {1,2,3,4}`
   (`jj₀` = the node the first loop found **plus one**, the spelling
   `modes::bus_vll_would_hang` uses)
   spins forever (measured 20.011 s TIMEOUT on `NEVTestCase` `double-1`); capi
   bounded the same loop to three tries in 2020 (`CAPI_Alt.pas:2512-2514`) and
   says so in its own comment.

**What the port does (S-SEQ / S-VLL, D4 chain → D8 trigger 3 → D21).**
`Bus.SeqVoltages` / `CplxSeqVoltages` are published **iff the bus carries nodes
1, 2 and 3** (r4133's stated intent, and the only case where symmetrical
components are defined); `Bus.VLL` / `puVLL` are the line-to-line voltages over
the phase nodes actually present — three pairs, the single pair, or nothing below
two phases — the pairing ORDER `ShowResults.pas:193-194` uses (that report path
is not S-VLL on every bus: with a phase missing it still pairs against ground).
No upstream walk is reproduced anywhere in the engine's API surface, in either
lane. The **report** paths are untouched here for two different reasons:
`report/show/voltages.rs` keeps the wrap-first pairing, which is r4133's own
correct order and no defect at all, while `report/export/seq_voltages.rs` keeps
the ground substitution, which **is** the defect. That one is not held in place
by golden bytes (measured 2026-09-05: every voltage-report golden runs
`IEEE13Nodeckt.dss`, whose buses carry node numbers 1-3 only, so the substituting
branch is never reached and fixing it would move zero bytes) but by scope — what
an export should print instead is a report-semantics decision outside WP-G1. It
is tracked as `ORPHANED_GAPS.md` §1.19. The split between the report convention
and the API semantics is pinned on both sides.

**How the divergence is gated — 0 ledger rows.** Instead of excluding the
divergent buses, `harness::compare_bus_seq_and_vll` classifies every bus from its
node set alone and closes the four divergent classes with a **positive assertion
of the upstream mechanism over the port's own state** (`oracle ==
upstream_walk(port node voltages)`, the D15/D16 settlement shape): the decline
rules are asserted in both directions against the `-1` sentinel, and the two
Pascal pairing loops are transcribed literally in the harness. The r4133 hang is
refused before any FFI by a **state-dependent** register in the bridge
(`modes::bus_vll_would_hang`, `STATE_DEPENDENT_REFUSALS`, `Engine::bus_vll_pair`),
whose verdict is cross-checked against the harness' independent replay. Four
run-wide populations fail on stale in both directions:
`R4133_SEQ_SENTINEL_POPULATION` (10, 129), `SEQ_GROUND_SUBSTITUTION_POPULATION`
(4, 54), `VLL_UPSTREAM_PAIRING_DECLINES` (16, 196), `R4133_VLL_HANG_POPULATION`
(2, 12). One tolerance constant carries the r4133 arm, `SEQ_C012 =
5.229590094302253e-10` (r4133's truncated `sin60` in `Ap2s`;
`tests/TOLERANCE_NOTES.md` §"Bus sequence and line-to-line voltages"), shared
with G1.3b and deduped to one definition at the 2026-09-06 landing (D21 — the
tight row sum, never the `5.30e-10` this sub-step had rounded it up to), and no
band moves.

**Pins** (the oracle-facing ones naming both numbers; the two `pu`/`node_v`
identities below are port-internal and name no oracle number):
`bus_seq_voltages_need_all_three_phase_nodes`,
`bus_vll_pairs_only_the_phase_nodes_the_bus_carries`,
`bus_pu_vll_divides_by_the_line_to_line_base`,
`bus_node_v_is_the_raw_voltage_behind_the_per_unit_arrays` (`exec/view.rs`);
`the_upstream_vll_walk_reproduces_the_ten_measured_bus_classes`,
`the_port_and_the_two_channels_state_their_availability_differently`,
`the_wrap_before_probe_pairing_reds_the_four_node_bus`,
`a_payload_that_contradicts_its_own_bus_class_reds_the_comparator`,
`the_seq_vll_population_rules_fail_in_both_directions`,
`the_seq_and_line_to_line_bands_are_the_documented_images` (harness);
`bus_vll_would_hang_refuses_exactly_the_measured_corpus_class`,
`the_two_refusal_registers_are_disjoint_and_state_dependent_rows_are_read_modes`
(`crates/dss-epri`). Upstream-ready reports:
`investigations/to_opendss/64-bus-vll-infinite-loop-no-node-1-to-4.md`,
`65-bus-vll-pairing-probes-before-wrapping.md`,
`66-bus-seqvoltages-node-count-and-ground-substitution.md`.

## R-18 — the two gating oracles spell a created file's name differently; the port keeps capi's spelling and the gate folds ASCII case — GOLDEN_REBASE G1.10a, 2026-09-06

**Observable.** G1.10a compares the SET of files a run creates under the case
directory. Un-case-folded, the two oracles disagree on **most** decks that write
anything: capi `NEV_EXP_Y.csv` against r4133 `NEV_EXP_Y.CSV`, capi
`..._VLN_Node.txt` against r4133 `..._VLN_Node.Txt`, capi
`IEEE13Nodeckt_CIM100x.xml` against r4133 `...CIM100x.XML`; and on
`Test/AutoTrans/Auto1bus.dss` r4133 lowercases the **whole deck-supplied stem**
(`auto1bus_hl_current.txt` against capi's `Auto1bus_HL_current.txt`).

**Sources.** r4133 `Version8/Source/Executive/ExportOptions.pas:333-356` writes
`FileName := 'EXP_VOLTAGES.CSV'` and its siblings in upper case; dss_capi 0.14.5
writes the same switch in lower case,
`.inputs/dss_capi/src/Executive/ExportOptions.pas:314,343,345,381,437`. The port
follows capi (`crates/dss-core/src/exec/report.rs:314,370,399,1411`).

**Decision — the port keeps its (capi) spelling; the comparator folds ASCII
case.** Four reasons, none of them "it does not matter". (i) There is no single
"r4133 spelling" to adopt: the two gating channels disagree with each other, so
matching one is diverging from the other. (ii) Matching r4133 fully would mean
*destroying* deck-supplied case (the `Auto1bus` stem), i.e. losing information the
user wrote. (iii) NTFS is case-insensitive, so no observable behaviour anywhere
depends on the choice — this is a spelling, not a semantic. (iv) The executive
echoes the filename into `GlobalResult`, which the gate already compares on the
`compare_global_result` cases, so flipping the spelling would move that string for
zero behavioural gain. The fold is therefore a documented cross-oracle
*normalization* — not a tolerance, `tests/TOLERANCE_NOTES.md` §G1.10a says so —
applied identically to all three producers, ASCII-only so a non-ASCII name is
refused loudly instead of being folded by one language's locale rule, and pinned
literally (both spellings written out) by
`run_files_pins::the_two_oracle_spellings_of_auto1bus_fold_to_one_member`. It
admits case and nothing else. 0 ledger rows. Not an upstream defect and not an
upstream report: two engines chose two conventions.

## `Visualize` writes a DSSView `.DSV`/`.dbl` file pair on r4133; the port emits a JSON plot payload — GOLDEN_REBASE G1.10a, 2026-09-06

**Observable.** On `solvable_now:Test/YgD-Test.dss` (`Visualize powers
Transformer.TR1`, line 25) the r4133 channel's created-file set is 6 names and
includes `testYgD_Transformer_tr1_PQ.DSV` + `testYgD_Transformer_tr1_PQ.dbl`; the
capi channel's is 4 and the port's is 4.

**Sources.** r4133 `Version8/Source/Executive/ExecHelper.pas:3672`
(`DoVisualizeCmd`) dispatches to `Plot/DSSPlot.pas:3642`
(`TDSSPlot.DoVisualizationPlot`), which builds the `…_PQ.DSV` name (`:3746`)
and calls `MakeNewGraph` (`:3758`); `Plot/DSSGraph.pas:109` rewrites that
`.DSV` text file (`:114-115`) and creates its binary companion
`ChangeFileExt(…, '.dbl')` (`:125`, `:128`) — the PAIR the viewer reads
(the chain the ledger cause `visualize-dssview-file-pair` carries; `:4071`
is an unrelated `FireOffEditor`, corrected by the G1.10a audit settlement). dss_capi 0.14.5 has no viewer: it fires
a plot callback and writes nothing. The port builds a JSON plot payload
(`crates/dss-core/src/exec/command.rs`) — one payload per `Visualize`, measured
`{"ElementName":"tr1","ElementType":"Transformer","PlotType":"Visualize","Quantity":"Power"}`.

**Decision — this is a PRODUCT divergence, not an upstream defect.** Nothing is
wrong with r4133 here; we deliberately do not write a Windows-viewer file format.
So no `investigations/to_opendss/` report is owed, and it is handled the way every
deliberate divergence is: one `r4133` ledger `exclusion`
(`r4133-visualize-writes-a-dssview-file-pair`, cause `visualize-dssview-file-pair`)
scoped by `name_re` to the two `_pq` names — never the whole case, never the whole
channel — plus the both-numbers pin
`run_files_pins::visualize_writes_a_dssview_pair_on_r4133_and_a_json_payload_in_the_port`,
which asserts r4133's 6 against the port's 4 and the payload non-empty, and proves
the exclusion does not widen (dropping an unrelated name still fails).

## The pinned dss_capi 0.14.5 faults on a `clear` after an AutoAdd solve — recorded, guarded, never reproduced — GOLDEN_REBASE G1.10a (D33(1)), 2026-09-06

**Observable.** G1.10a's capi transport issues one `clear` as the last statement of
its hygiene-guard scope, to run the element destructors that release dss_capi's
never-closed Storage `DebugTrace` stream (`src/PCElements/Storage.pas:868-885`,
freed only at `:871`/`:1199`) before the guard sweeps. On the two AutoAdd decks —
`modes:autoadd/autoadd.dss` and `modes:autoadd/autoadd_cap.dss` — that `clear`
raises, deterministically: `DSSException (#303) Error 303 Reported From OpenDSS
Intrinsic Function: ProcessCommand: Exception Raised While Processing DSS Command:
clear Error Description: Access violation`, and the one-shot process then exits
`0xC0000005`. r4133 is unaffected (it needs no teardown `clear` at all — it closes
its own trace file as it writes the header,
`Version8/Source/PCElements/Storage.pas:1085`). It is the same
process-exit fault family the AutoAdd work already recorded (`GAPS_PLAN.md` §2.2)
and it belongs to the **outdated numeric oracle**, not to the behavioural
authority.

**Decision — record and guard, never reproduce, never a ledger row.** The teardown
stays (it is what closes the leak), but it is wrapped: the exception is caught and
reported in a run-level `teardown_error` reply key that the runner prints without
failing the case — every compared surface is already captured when it raises,
since the classification is the statement before it — the sweep still runs and
still reports a leaked dropping, and a persistent worker whose teardown raised
replies in full and then exits so the pool respawns it. Pinned in both directions
by `engines::a_capi_worker_whose_teardown_clear_raises_replies_in_full_then_exits_for_respawn`:
if a future dss_capi stops faulting, the test says the guard can be retired. Not
reported upstream — the pinned 0.14.5 is four releases old and is a numeric oracle
only.

## A negative Transformer or AutoTrans rating is still a rating: every loading takes its magnitude — user decision 2026-10-03

**Observable.** The Transformer and the AutoTrans derive `NormAmps`/`EmergAmps`
from their normal and emergency kVA and the winding 1 voltage base, so a
negative base or a negative kVA rating gives a negative pair. An AutoTrans whose
winding 1 (series) kV sits below its winding 2 (common) kV has a negative series
base: `AutoTrans.at` of the gated deck `modes/makeposseq/makeposseq_xfmr.dss`
derives −152.848446716868/−208.429700068457. A Transformer gets a negative pair
from a negative `normhkva`/`emerghkva`, and on r4133 also from a negative
winding 1 `kV`. Wherever a loading is measured against such a rating, both
oracles read it as a number with the wrong sign or as no rating at all.

**Sources.** The lines are r4133's, and the pinned dss_capi 0.14.5 does the
same at every site. The ratings derive at `Version8/Source/PDElements/Transformer.pas:1129-1130`
and `PDElements/AutoTrans.pas:1164-1165`. The loading sites:

- `Export SeqCurrents` seeds `iNormal := NormAmps` and divides only when it is
  `> 0` (`Common/ExportResults.pas:355-358`), so it prints the rating itself.
- `Export Capacity` guards only `= 0` (`ExportResults.pas:509-512`), so it
  prints a negative percentage.
- `Export Overloads` and `Show Overloads` report only an element with a positive
  rating (`ExportResults.pas:2756`, `Common/ShowResults.pas:2286`), and
  `DI_Overloads` likewise (`Meters/EnergyMeter.pas:3213`), so the overloaded
  element is never reported.
- `Show Currents` prints a loading only for a positive rating
  (`ShowResults.pas:460-461`), so it prints `0.00`.
- `Get_ExcesskVANorm`/`Get_ExcesskVAEmerg` divide the terminal-1 current by the
  signed rating (`PDElements/PDElement.pas:257`, `:284`), so the overload factor
  is negative and the excess kVA is zero. That empties the excess columns of
  `Export Powers`, `Show Powers` and `Export SeqPowers`, the EnergyMeter
  `Overload kWh Normal`/`Emerg` registers (`EnergyMeter.pas:1372-1385`) and,
  through `Overload_EEN`/`Overload_UE`, the overload share of the zone loads'
  `Load EEN`/`Load UE` (`:1414-1419`).
- Those load factors also decide `Export Unserved` and `Show Unserved`
  (`ExportResults.pas:2799`, `ShowResults.pas:2318`): a load is unserved when
  its factor is positive (`PCElements/Load.pas:2010`, `:2069`), so the zone
  load of the overloaded element is listed only for its voltage, or not at all.
- AutoAdd scores each candidate bus by the EnergyMeter registers named in
  `UEregs` (`Common/AutoAdd.pas:740-765`, weighted at `:248-250`), by default
  `Overload kWh Emerg` (`Common/Circuit.pas:512`), so the overload never enters
  its objective, its log or its choice of bus.

r4133 stores a negative winding `kV` unchecked (`Transformer.pas:504`,
`AutoTrans.pas:487`). dss_capi 0.14.5 flags it `NonNegative`
(`.inputs/dss_capi/src/PDElements/Transformer.pas:457`) and refuses it with
error #2020032, and so does the port. That refusal is a deliberate divergence
from r4133 (user decision 2026-10-03): a winding voltage entered with a minus
sign is an input error for a Transformer and an AutoTrans, unlike windings
entered in reversed order. `negative_winding_kv_is_refused_and_the_rating_stays_positive`
pins it.

**Measured 2026-10-03** on the pinned dss-python and on the r4133 DLL through
`epri-worker`: a 500 kVA Transformer feeding 900 kW with
`normhkva=-550 emerghkva=-750` set after its windings, its positive twin, the
same pair at 100 kW (under the rating magnitude), `(-550, 0)` and `(0, -750)`
with their positive twins, the Transformer with a winding 1 `kV` of −12.47, an
AutoTrans with winding 1 at 4.16 kV under winding 2 at 12.47 kV feeding
2000 kW, the same AutoTrans with winding 1 at −4.16 kV, and a Line rated −1/−2.
Both oracles accept the negative `normhkva`/`emerghkva`, read the ratings back
as −25.4645267084438/−34.7243546024234, and show the readings above at every
site, while on the positive twin both print the loadings the port now prints
for the negative deck (`Export SeqCurrents` 181.1/132.8, `Export Capacity`
181.08/132.79, an `Export Overloads` row, excess 405.6/185.4/223.7/102.3 kW/kvar,
`Overload kWh Normal` 405.5516011426772, the zone load unserved at 0.811/0.328,
AutoAdd `kW UE` 223.672400411608 at `src` and `lv` chosen at
0.758933540057766). On the negative deck both list the load at the voltage-only
0.022/0.000 (and not at all for `UEonly`) and score AutoAdd at 0.0133736745189343
with `kW UE` 0, choosing the same bus. r4133 accepts the −12.47 kV winding and
derives −25.465/−34.724, and the −4.16 kV AutoTrans winding with −76.378/−104.15.
The Line reads as unrated on both oracles at every site but `Export Capacity`,
which guards only a zero rating (`ExportResults.pas:509-512`) and divides by
the signed one: both print −2438.39/−1219.20 for the Line.

**Decision (user, 2026-10-03).** A negative current is still a current. The
element solves as on both oracles, and its stored rating, the property
read-back, `Dump`, `Show Ratings`, the CIM limits and the rating columns of
`DI_Overloads` stay exactly as the oracles have them. Every loading of a
Transformer or an AutoTrans measures against the magnitude of its rating
(`CktElement::loading_rating`, the same in both lanes): the six report sites,
the excess kVA, the overload registers, the zone loads' EEN/UE factors and with
them `Export Unserved`, `Show Unserved` and the AutoAdd objective. A zero rating
stays unrated. Every other class keeps the oracles' reading of a non-positive
rating, which is "no rating" everywhere but `Export Capacity`. A Line built
from a WireData without a rating takes −1/−1, whose magnitude means nothing.
Whether `Export Capacity` should print 0/0 for such a rating instead of
dividing by it is outside this decision and open, since it would be a new
divergence from both oracles.

**Pins** (`crates/dss-core/tests/golden_reports.rs`).
`export_seqcurrents_negative_autotrans_rating_loads_against_its_magnitude`
derives the `AutoTrans.at` cells of the gated deck from its `I1` and the rating
magnitudes (28.79/21.11, where both oracles print −152.8/−208.4). On the
overloaded Transformer, `export_seqcurrents_negative_transformer_rating_loads_against_its_magnitude`,
`show_currents_negative_transformer_rating_loads_against_its_magnitude`,
`export_capacity_negative_transformer_rating_loads_against_its_magnitude`,
`export_overloads_negative_transformer_rating_reports_the_overload`,
`show_overloads_negative_transformer_rating_reports_the_overload`,
`export_powers_negative_transformer_rating_carries_the_excess_kva`,
`energymeter_negative_transformer_rating_registers_the_overload`,
`di_overloads_negative_transformer_rating_reports_the_overload`,
`unserved_negative_transformer_rating_flags_the_overloaded_load` and
`autoadd_negative_transformer_rating_scores_the_overload` assert the loading
literally, quote both oracles' reading, and prove the negative deck reports as
its positive twin (`DI_Overloads` apart from its rating columns, which print
the stored rating). `di_overloads_one_negative_transformer_rating_opens_the_entry_gate`
and `negative_transformer_rating_under_its_magnitude_reports_no_overload` pin
the entry gate and the overload test against their twins, and
`nonpositive_line_rating_keeps_the_oracle_reading_at_every_loading_site` with
`export_seqcurrents_prints_zero_for_an_undefined_rating` holds the Line at the
oracles' reading, the signed `Export Capacity` division included. No
`ledger.json` entry and no golden byte: the one gated deck with a negative
transformer rating (`makeposseq_xfmr.dss`,
`AutoTrans.at` loaded to 28.8 %) has no overload, no EnergyMeter, no AutoAdd
and no compared report, so no gated channel moves.

## TotalPower adds every terminal of every source, both oracles add terminal 1 alone — user decision 2026-10-03

**Observable.** `Circuit.TotalPower`, the `Summary` power lines and loss
percentage, the `Export Summary` `TotalMW`/`TotalMvar`/`pctLosses` columns and
the system meter (its demand-interval kW/kvar row and its kWh, kvarh, peak kW
and peak kVA registers) report the power the sources supply. Both oracles add
the power at terminal 1 of every Vsource, Isource and GICsource and drop
terminal 2. Where terminal 2 is earthed, the default, the two readings are the
same number. Where it sits on a live bus (a Vsource or Isource with an explicit
`bus2`, and every GICsource, which is spliced in series into a line) the
terminal-1 reading is wrong by the terminal-2 power, in either direction.

**Physics.** With KCL at every node the complex power into all conductors of
all elements sums to zero (Tellegen), so the power the sources supply to the
rest of the circuit is minus the sum over all of their conductors. Two
measurements decide it without presupposing either definition. Writing a
two-ended source with `bus1` and `bus2` swapped and its phasor turned by 180°
changes no current and no voltage, yet moves the terminal-1 total (below), and
the all-terminals sum stays put. Raising every potential by U changes the power
of a set of conductors by U·conj(ΣI), which is zero over a whole element but not
over one terminal of a two-ended source, so the terminal-1 total depends on
where the circuit touches earth. For a two-terminal source the all-terminals sum
is Σ (V1 − V2)·conj(I1), the voltage across the source times the current
through it, which is what the r4133 `Circuit` interface documentation promises
("the total power in kW and kvar supplied to the circuit by all Vsource and
Isource objects").

**Sources.** r4133 `Version8/Source/Common/Utilities.pas:2297-2309`
(`GetTotalPowerFromSources`, `Caccum(Result, Cnegate(CktElem.power[1,ActorID]))`
at `:2306`), read by the Summary (`Executive/ExecHelper.pas:3417`, loss
percentage `:3421`), `Export Summary` (`Common/ExportResults.pas:3309`, `:3313`)
and `TSystemMeter.TakeSample` (`Meters/EnergyMeter.pas:3490`, registers
`:3492-3497`). The API property repeats the walk: `DDLL/DCircuit.pas:359`
(`CircuitV` mode 3) and `DLL/ImplCircuit.pas:382`. The pinned dss_capi 0.14.5
carries the same walk at `src/Common/Utilities.pas:1330` and
`src/CAPI/CAPI_Circuit.pas:332`, read at `src/Executive/ExecHelper.pas:3486`,
`src/Common/ExportResults.pas:3043` and `src/Meters/EnergyMeter.pas:3495`.

**Measured 2026-10-03** with the pinned dss-python and with epri-worker on the
r4133 DLL, which agree to 1e-10 kW or better (kW, TotalPower against the sum of
the sources' `Powers` over every terminal): the paper circuit (100 V at A, 1 Ω
A–B, 9 Ω B–earth, a 5 A Isource `bus1=A bus2=B`) −1.4499998 against −1.0224998,
written end for end −0.5224999 against the same −1.0224998, where the network
absorbs 1022.5 W by hand. Two 12.47 kV islands tied by
`Isource.itie bus1=b1 bus2=b2` −1472.906666 against −1303.468626, end for end
−1138.845018 against −1303.468626. A 5 % series booster on a 900 kW feeder
−1767.611241 against −905.177254, end for end −3.451251 against −905.177254.
The corpus cases `asymmetric/isource/isource_snap.dss` −1553.393 against
−1389.463, `asymmetric/gic/gicsource_gic.dss` −20.287 against −22.134 and
`asymmetric/gic/gic_midi.dss` −4.676 against −7.056. On the tie deck both
oracles' Summary prints `1.47291 MW` and a loss share of `0.2361 %` (`0.3054 %`
end for end), where 3.4776 kW of 1303.47 kW is 0.2668 %.

**Decision (user, 2026-10-03, on a physics verdict).** The terminal-1 walk is
an upstream bug shared by both oracles and is not reproduced. The engine adds
every conductor of every terminal of every enabled source
(`Circuit::source_power`, ×3 in a positive-sequence circuit), and
`Dss::total_power` and the system meter's `total_power_from_sources` read it,
so the Summary, `Export Summary` and the SystemMeter registers follow. A circuit
whose sources all keep terminal 2 earthed is bit-identical to before, since
terminal 2 adds an exact zero. Upstream report
`investigations/to_opendss/81-totalpower-reads-terminal-one-of-two-ended-sources.md`
(local-only).

**Exclusion and pins.** The corpus gate's `Circuit.TotalPower` arm compares the
three cases above against the oracle's reading plus its sources' accepted
terminal-2 powers, `harness::lane::total_power_counts_every_source_terminal`,
and asserts the list both ways (an unlisted case with material terminal-2 power
fails, and so does a listed case without it). Its membership arm rebuilds each
oracle's `TotalPower` from that oracle's own terminal-1 powers on every case.
In-engine and oracle-free (`exec::tests::aggregates`):
`total_power_sums_every_terminal_of_every_source`,
`total_power_closes_the_complete_ledger`,
`total_power_does_not_move_when_a_source_is_written_end_for_end`,
`total_power_of_the_paper_circuit_is_its_network_power`,
`the_source_power_readers_report_every_terminal`, and one complete-ledger pin
per listed case: `isource_snap_total_power_closes_the_complete_ledger`,
`gicsource_gic_total_power_closes_the_complete_ledger` and
`gic_midi_total_power_closes_the_complete_ledger`. No `ledger.json` entry, golden
byte or tolerance moved. `Circuit.Losses` leaving shunt elements out is a
separate upstream choice this row does not touch, which is why the pins use the
complete ledger and not loads plus `Circuit.Losses`.

## A `DynamicExp` without the memory its host integrates refuses the dynamics solve — 2026-10-04

**Observable.** A Generator, WindGen, PVSystem or Storage linked by
`DynamicEq=` integrates the equation's memory, one row per declared variable
(`NVariables`, default 20, see the row "`DynamicExp.NVariables` defaults to 20"
below). When that memory has no row, when `DynOut` names fewer outputs than
the host integrates, or when the outputs, the calculated values or the
equations reach past its last row, the integration has nothing to read or
write. The port refuses the dynamics `Solve` before its first step with error
482, `Error Encountered in Solve: Generator.e1: DynamicExp.de declares no state
variables (NVariables=0). Set NVariables to the number of its VarNames.` (or the
`declares NVariables=1 but its equations use 2 state variables` form, which
reads `holds the default NVariables=20` when the count was not written, or the
`DynOut` forms of the row "A `DynOut` list with a name that is not an output is
refused" below), and the solution aborts. The memory is checked before
`DynOut`. Entering dynamics stays silent. A `DynOut` read of an equation
without `VarNames` answers the empty value, and a memory row past `VarNames` is
named by the empty string.

**Measured 2026-10-04** with the epri-worker on the r4133 DLL and the pinned
dss-python 0.15.7, on the pins' deck for each host (dss_capi 0.14.5 has no
WindGen):

- No state and no `DynOut`, `nvariables=0` with two `VarNames` and `DynOut`, and
  a two-variable equation without `DynOut`: both oracles fail the `Solve` with
  482 `Error Encountered in Solve: Access violation` (r4133 adds the address in
  `OpenDSSDirect.dll`).
- `nvariables=1` for an equation that uses two variables: neither oracle
  reports an error. The second row is written past the memory
  (`Version8/Source/PCElements/generator.pas:2843-2849`,
  `PCElements/WindGen.pas:2697-2703`), and one r4133 PVSystem run of that deck
  hung in the DLL.
- `nvariables=1` with `DynOut=[a]` inside the memory, for an equation that reads
  a second variable (`a dt = b`) or for a calculated value on it
  (`vac=Vmag` under `a dt = a`): neither oracle reports an error on any host.
  The step reads the second row past the memory or writes `Vmag` there
  (`PCElements/generator.pas:2836`, `PCElements/PVsystem.pas:2509`,
  `PCElements/Storage.pas:3643`). The r4133 WindGen raises 303 on the one-name
  `DynOut`, and with `DynOut=[a a]` it solves without an error.
- Initial values `theta=Edp pm=P0` on two variables past a one-row memory (the
  init pins' deck): dss_capi 0.14.5 solves the Generator without an error, and
  r4133 fails its `Solve` with 482 `Access violation`. The r4133 WindGen raises
  303 on the deck's one-name `DynOut=[Speed]`, since it reads a second name
  (`PCElements/WindGen.pas:1098-1101`), and with `DynOut=[Speed Speed]` it
  solves without an error.
- Two `VarNames` with `NVariables` omitted: r4133 integrates, because its
  `NumVars` defaults to 20 (`General/DynamicExp.pas:305`). dss_capi 0.14.5
  defaults `NVariables` to 0 (`src/General/DynamicExp.pas:218`) and fails the
  `Solve` with 482 `Access violation`. The port defaults to 20 and integrates
  as r4133 does (the row "`DynamicExp.NVariables` defaults to 20" below).
- `DynOut` read back after an unresolved `DynOut=` on an equation without
  `VarNames`: both oracles raise 303 `List index out of bounds (0)`
  (`PCElements/PCElement.pas:205`).
- A WindGen linked to an equation without variables reads its 22 native
  variables on r4133, as the port does.

**Decision.** An equation whose memory cannot hold what its host integrates
defines no dynamics for that host. The oracles' access violation, out-of-range
write and hang are accidents of their implementation, not a rule. The port
refuses with a message naming the element and the property to set, and never
falls back to the built-in model in place of the equation the user linked. For
no state and no `DynOut` that is the oracles' outcome class (a failed `Solve`,
482, aborted). For an `NVariables` too small for what the equations use, the
default 20 included, the port refuses where r4133 writes or reads past the
memory without an error. An omitted `NVariables` holds 20 rows and integrates.

**Pins** (`crates/dss-core/src/exec/tests/dynamic_eq_memory.rs`, both lanes):
`windgen_`, `generator_`, `pvsystem_` and
`storage_dynamic_eq_without_state_variables_refuses_the_dynamics_solve` (an
explicit `nvariables=0`), `an_omitted_nvariables_holds_twenty_rows`,
`a_dynamic_eq_without_dynout_refuses_the_dynamics_solve`,
`a_dynamic_eq_with_too_few_rows_refuses_the_dynamics_solve`,
`generator_init_values_without_a_memory_row_refuse_the_dynamics_solve`,
`windgen_init_values_without_a_memory_row_refuse_the_dynamics_solve`,
`every_host_refuses_an_equation_that_reads_past_its_memory`,
`every_host_refuses_a_calculated_value_past_its_memory`,
`windgen_dynamic_eq_without_variables_keeps_the_classic_layout`,
`every_host_names_an_unnamed_memory_row_by_the_empty_string` and
`every_host_reads_back_no_dynout_from_an_equation_without_var_names`, with
`a_dynamic_eq_with_its_memory_integrates` as the control. No ledger entry,
exclusion, golden byte or tolerance moved: the corpus decks that link a
`DynamicExp` give it a row per variable and set `DynOut`.

## `DynOut` with more names than its two output slots is refused — 2026-10-04

**Observable.** `DynOut` fills two output slots: speed and angle on a
Generator or a WindGen, and a PVSystem or a Storage reads the first as its
current. A `DynOut=` list of three or more names is refused whole with error
50009, `DynOut takes at most 2 output variables, but 3 were given: [a, b, c].
DynOut is unchanged.`, and the element keeps the outputs it had. The length is
checked before any name is resolved, so `DynOut=[b c nosuch]` gets the same
refusal and no 50008. The code is the port's own, the next number after the
`DynamicExp` messages 50001-50008.

**Measured 2026-10-04** with the epri-worker on the r4133 DLL and the pinned
dss-python 0.15.7, on the pin's deck (`nvariables=3 varnames=[a b c]`, after a
valid `DynOut=[c a]` where noted):

- `DynOut=[a b c]` on a Generator, PVSystem or Storage writes past the two
  slots on both oracles (dss_capi `src/PCElements/DynEqPCE.pas:218-228`, r4133
  `Version8/Source/PCElements/PCElement.pas:223-236`). dss_capi 0.14.5 dies a
  few commands later (0xC0000409, 0xC0000374 for one Storage run). r4133
  reports no error and reads `DynOut` back as `[a,da,]` (a Storage reads back
  `''`, as for any list), then its worker does not quit (killed after 30 s),
  and the edit itself hung on a Storage that had `DynOut=[c a]`.
- The same list on an r4133 WindGen: no error. Its `SetDynOutput` reads exactly
  the first two names (`PCElements/WindGen.pas:1098`) and drops the third
  without a word.
- `DynOut=[b c nosuch]` after `DynOut=[c a]`: both oracles raise 50008 for
  `nosuch` and take `b` and `c` (dss_capi reads back `[b, c]`). The r4133
  WindGen raises nothing.

**Decision.** A third name has no slot and no output to drive: every host
integrates at most the first two. The oracles' write past the array, the crash
and the hang that follow are accidents, and the r4133 WindGen's silent
truncation hides the input error. The port refuses the list where it is
written, names the property, and keeps the outputs last set, so the next
dynamics solve integrates those or refuses with `DynOut is not set`. The length
decides whatever the names resolve to, so a long list with an unknown name is
refused the same way instead of taking its first two names as both oracles do.

**Pin** (`crates/dss-core/src/exec/tests/dynamic_eq_memory.rs`, both lanes):
`every_host_refuses_a_dynout_longer_than_two_outputs`. No corpus deck, golden
or test deck names more than two outputs (the corpus writes `[Speed theta]` and
`[it]`), so no ledger entry, exclusion, golden byte or tolerance moved.

## `DynamicExp.NVariables` defaults to 20 — 2026-10-04

**Observable.** A `DynamicExp` that never writes `NVariables` holds 20 memory
rows. `? DynamicExp.de.NVariables` reads `20` for `New DynamicExp.de`, for an
equation with `VarNames` only and for a `like=` copy (which copies nothing),
and a written count reads back as written, `0` included. An equation that
names its `VarNames` and omits the count integrates on a Generator, WindGen,
PVSystem or Storage exactly like the same equation with the count written. Its
host reports the 20 rows, the rows past `VarNames` as unnamed zeros. An
equation without `VarNames` (`New DynamicExp.de`, a `like=` copy) holds the 20
rows but no output: its `DynOut=` never resolves, a re-link to it empties the
`DynOut` its host had, and the host refuses the dynamics `Solve` (the row "A
`DynOut` list with a name that is not an output is refused" below). An
equation whose equations, outputs or calculated values reach past row 20
(21 `VarNames` and `v1 dt = v21`) is refused with 482, `DynamicExp.de holds
the default NVariables=20 but its equations use 21 state variables. Set
NVariables to the number of its VarNames.`. `Save` writes `NVariables` only
when the deck wrote it, so a saved equation reloads with the count it had. The
JSON schema gives the property `"default" : 20`.

**Measured 2026-10-04** with the epri-worker on the r4133 DLL and the pinned
dss-python 0.15.7:

- Readback: r4133 answers `20` for `New DynamicExp.de`, for `varnames=[a b]`
  only and for `like=base` (its `MakeLike` copies nothing and reports no
  error), and `0` for `nvariables=0`. dss_capi 0.14.5 answers `0` for all four
  and refuses the `like=` with 50099.
- Integration, on each host's deck (a swing equation started at `Speed=0.05`
  for the machines, the filter current of a grid-following inverter for
  Storage at 0.69 kV and PVSystem at 12.47 kV, 51 steps): r4133 runs the
  omitted and the written count without an error, and the host powers, the
  node voltages and its variable reads are bit-identical between the two on
  all four hosts. dss_capi 0.14.5 fails the omitted count's `Solve` with 482
  `Access violation` on Generator, PVSystem and Storage, and the machine deck's
  initial values with 303.
- More than 20 names: 21 `VarNames` with `NVariables` omitted run on r4133
  without an error whether the equations stay in the first 20 rows or read
  the 21st, which reads past the memory.
- Variables: r4133's `AllVariableNames` of a linked host stays the classic
  list (6 Generator, 22 WindGen and PVSystem, 34 Storage names) for either
  count. A mode-3 monitor on a host whose equation omits the count fails with
  670 `List index out of bounds (6). TStringList range is 0..5` (it names the
  20 rows through `VarNames`), and the monitor then reports a garbage channel
  count.
- `Save`: r4133 writes no `nvariables` for the omitted count and
  `nvariables=0` for the written one, as dss_capi 0.14.5 does.

**Decision** (user, 2026-10-04). The default is a script-language choice:
physics is indifferent to it and r4133's 20 is logical, since a user who
writes `VarNames` and forgets the count gets a working equation. The port
takes it in both lanes. r4133's read past row 20, its monitor failure and its
integration of an equation without `VarNames` out of a zero row are not
copied: a reach past the memory is refused, an unnamed row is named by the
empty string (the first row above), and a host without an output refuses the
solve (the row below). The pinned 0.14.5 oracle prints `0`,
and regenerating its capture would print `0` again, so the two capture cells
are excluded instead: `dynamicexp_default` / `NVariables` and
`dynamicexp_makelike` / `NVariables` in `props_roundtrip.rs`'s
`LANE_SKIP_SCENARIO_PROPS`. The schema default is a `port_changed_line` in
`tests/golden/json/schema_divergences.json` and moves
`tests/golden/json/schema_full_port.json`. No corpus deck omits `NVariables`,
so no ledger entry moved.

**Pins** (`crates/dss-core/src/exec/tests/dynamic_eq_memory.rs`, both lanes):
`an_omitted_nvariables_reads_back_twenty` (it replays both excluded
scenarios), `every_host_integrates_an_omitted_nvariables_like_the_written_count`,
`an_omitted_nvariables_holds_twenty_rows` and
`save_reloads_an_omitted_nvariables_as_the_default`, with the four
`<host>_dynamic_eq_without_state_variables_refuses_the_dynamics_solve` pins
for an explicit `nvariables=0`.

## A `DynOut` list with a name that is not an output is refused, and a machine needs both outputs — 2026-10-04

**Observable.** `DynOut=` names the outputs a host integrates, in slot order:
a Generator or a WindGen integrates the first as its speed and the second as
its angle, a PVSystem or a Storage the first as its current. A name resolves
when it is an output of the linked `DynamicExp`, a variable with a
`<name> dt =` equation. A list with a name that does not resolve is refused
whole, like a list of three names (the row "`DynOut` with more names than its
two output slots is refused" above): each such name records error 50008,
`DynamicExp variable "zzz" not found or not defined as an output. DynOut is
unchanged.`, and the element keeps the outputs it had. `DynOut` so holds only
outputs a list named, and reads back as written: `[it]` for `DynOut=[it]`. A
host refuses the dynamics `Solve` with 482 when `DynOut` names fewer outputs
than it integrates: `DynOut is not set. Name the output variables of
DynamicExp.de with DynOut=[...].` when it names none (no list, `DynOut=[]`, or
only lists that did not resolve), and `DynOut names one output, but a machine
integrates two, its speed and its angle. Name both output variables of
DynamicExp.de with DynOut=[...].` for a machine given one name. A re-link,
`DynamicEq=` set again, resolves the outputs `DynOut` reads back against the
new equation by name: they stay when every one is an output of it, and
`DynOut` is emptied otherwise, also when `DynamicEq=` names an equation that
does not exist. A `DynOut=` list after `DynamicEq=` on the same
line names outputs of the new equation. An equation without `VarNames`, such as
the bare `New DynamicExp.de` or a `like=` copy, has no output to name, so its
host refuses the solve whatever its memory holds, also after a re-link to it.

**Measured 2026-10-04** with the epri-worker on the r4133 DLL and the pinned
dss-python 0.15.7, on the pins' 0.69 kV deck for each host, 21 dynamics steps
(dss_capi 0.14.5 has no WindGen):

- `New DynamicExp.de` with `DynOut=[Speed theta]`, or a `like=` copy with
  `DynOut=[a b]` (50008 for the names): r4133 reports no error at the `Solve`
  and integrates both slots out of row 0 of its 20 zero rows. The Generator
  falls from -500.0 kW / -269.8 kvar to -34.2 kW / -344.3 kvar per phase, the
  WindGen ends at 2.5 kW / -11997 kvar and the Storage output at 0, and the
  PVSystem diverges, as its built-in model does on this deck. dss_capi 0.14.5
  fails the `Solve` with 482 `Access violation`, its default count being 0
  (after 50099 on the `like=`).
- `nvariables=2 varnames=[a b] expression=[a dt = b]` with `DynOut=[zzz]`:
  both oracles record 50008 (the r4133 WindGen 303) and integrate both slots
  out of row 0 without an error, with the same -34.2 kW on the Generator.
  dss_capi reads `DynOut` back as `[a, a]`.
- `nvariables=2 varnames=[Speed theta] expression=[Speed dt = theta; theta dt =
  Speed]` with `DynOut=[Speed]`: the Generator integrates its angle out of the
  speed row on both oracles without an error (-34.2 kW again) and reads back
  `[speed, speed]`. The r4133 WindGen raises 303 on the one-name list, since it
  reads a second name (`PCElements/WindGen.pas:1098-1101`), and integrates the
  same way. With the count omitted r4133 does the same and dss_capi fails the
  `Solve` with 482.
- `DynOut=[]` on the bare equation: both oracles fail the `Solve` with 482.
- After `DynOut=[a b]`, `DynOut=[b zzz]` takes `b` and leaves the slot whose
  name failed at its old index on both oracles (dss_capi reads back `[b, b]`).
- A re-link, `Edit Generator.e1 DynamicEq=b`, after `DynOut=[Speed theta]`
  (`[it]` on an inverter) resolved on the six-name (four-name) equation `a`:
  both oracles keep the slots' indices into `a`, and their memory. To a copy of
  `a` they run bit-identically to the deck that never re-linked, where the port
  allocates a fresh memory. To the bare `New DynamicExp.b`, a `like=` copy, a
  written count without names, or `a`'s names with no equation for the last
  output (`theta`, `it`), r4133 integrates without an error at the `Solve`:
  the Generator holds -499.99 kW with its angle frozen, the WindGen ends at
  2.5 kW / -11997 kvar, the PVSystem diverges to 9.2e65 and the Storage output
  falls to 0. dss_capi integrates the written count and the missing equation
  the same way, and fails the bare equation and the `like=` copy with 303 or
  482. Both raise 303 on the `DynOut` readback of an equation without names.
  `DynamicEq=b DynOut=[w th]` on one line reads back `[w, th]` on dss_capi.

**Decision.** A slot holds an output its host integrates. A slot that no list
filled holds none: the oracles' row 0 in its place is the zero of a fresh index
array, and a list that fails part-way leaves them a mix of two lists. Either
makes a machine integrate its speed and its angle out of one row, or out of a
row of zeros, so its rotor angle collapses and its output with it. That is not
dynamics. The port takes a list whole or not at all, keeping the outputs last
set as for three names, and refuses a dynamics solve whose host lacks an
output. The 20 default rows made the case reachable from an equation without
`VarNames`: refused before for its empty memory, it now meets the `DynOut`
refusal where r4133 integrates the collapsed state. A re-link replaces the
equation a slot's index points into, so the port resolves the outputs again by
the names `DynOut` reads back, whole or not at all. An equation that defines
the same outputs keeps them, as the oracles' indices do for a copy, and any
other empties `DynOut`.

**Pins** (`crates/dss-core/src/exec/tests/dynamic_eq_memory.rs`, both lanes):
`every_host_refuses_a_dynout_that_names_no_output_of_its_equation` (the bare
equation, a `like=` copy and a written count),
`every_host_refuses_an_empty_dynout`, `a_machine_refuses_a_dynout_with_one_output`,
`every_host_keeps_its_dynout_through_a_list_with_an_unresolved_name`,
`every_host_reads_back_no_dynout_from_an_equation_without_var_names`, which now
covers the bare equation too,
`every_host_refuses_a_relink_to_an_equation_without_its_outputs` (the bare
equation, a `like=` copy, a written count without names, a missing equation for
the last output and a smaller equation) and
`every_host_keeps_its_dynout_through_a_relink_to_an_equation_with_its_outputs`
(a copy, and `DynamicEq=` with `DynOut=` on one line). No corpus deck, golden
or test deck names an output that does not resolve, gives a machine one name or
re-links a host (the corpus writes `[Speed theta]` on machines and `[it]` on
inverters), so no ledger entry, exclusion, golden byte or tolerance moved.

## A matrix-defined Capacitor, Reactor, Line or Fault reduces to its positive-sequence self term under MakePosSequence — user decision 2026-10-04

**Observable.** `MakePosSequence` writes, for an element given by a matrix of
N conductors, the positive-sequence self term `S` of the matrix it stamps.
For N = 2 or 3 that is `S(M) = (Σ M_ii − Σ_{i<j} M_ij) / 3` on the symmetric
part of `M`, for N > 3 the mean of the diagonal minus the mean over all
off-diagonal pairs (the user's ruling of 2026-10-04). A `cmatrix` bank writes
`Cuf = S(C)` on every step: 12 µF for `[10|-2 10|-2 -2 10]` and for
`[12|-3 10|-1 -2 8]`, 22/3 µF for the two-phase `[10|-2 10]`. A series matrix
Reactor writes `R + jX = 1 / S((R + jX)⁻¹)`: 0.8 + j9 Ω for
`rmatrix=[1|.2 1|.2 .2 1] xmatrix=[10|1 10|1 1 10]` and for the corpus deck's
`rx_mat`, 1.31826664 + j14.14381841 Ω for the two-phase `[1|.2 1]`/`[10|1 10]`.
A `Parallel=yes` matrix Reactor writes `r=0`, `x = −1/S(B)`, `rp = 1/S(G)` from
the `G = R⁻¹`, `B = −X⁻¹` it stamps (`r=0 x=10 rp=100` for diagonal R 100,
X 10), and a matrix reduction that writes no `rp` of its own writes `rp=0` over
a given one, which the matrix stamp never used. `rp=0` stands for no parallel
branch, the reading the R + jX stamp gives it (the documentation says only
that `Rp` is "Assumed infinite if not specified"), and `Save circuit` writes
it as `Rp=0`, which the reload reads the same way. A matrix Line writes C1 by
the same rule (22/3 nF for the two-phase `[10|-2 10]`, 12 nF for the balanced
4×4), with Z1 the mean diagonal minus the mean off-diagonal term at every N
and the three-phase reduction unchanged to the bit. A `gmatrix` Fault writes
`R = 1 / S(G)` ohm: 8.3333 Ω for `[0.1|-0.02 0.1|-0.02 -0.02 0.1]`, and
12.5 Ω for the two-phase `[0.12|0 0.12]`, which keeps its 12440.1 kW, where
its `r=8.3333` twin keeps its per-phase `r` and draws 18660.1 kW after the
reduction (open question 1). On the
same path: a closed three-phase delta `cuf` Capacitor writes three times its
leg value, a closed three-phase delta R + jX Reactor a third of its leg `r`,
`x` and `rp`, a closed three-phase delta `kvar` Reactor a third of its leg
`r` and `rp`, and a
one-phase symmetrical-component Reactor stamps `Z1` (three-phase stamps keep
`(Z0 + Z1 + Z2)/3` on the diagonal). A delta-connected `cmatrix` Capacitor or
matrix or `z1` Reactor of two or more phases, a series matrix Reactor given
only one of `rmatrix`/`xmatrix` or whose impedance `rmatrix + j xmatrix` does
not invert, a `cmatrix`, `rmatrix` or `xmatrix` of another order than the
element's phases, and a `gmatrix` of another order than a fault's two or more
phases (the last two as a `phases=` edit after the matrix leaves them) are
refused by the solve with a message naming the element and the remedy (`…
cannot be connected in delta. Specify it with conn=wye.`, `… xmatrix is
missing. …`, `… the impedance rmatrix + j xmatrix is singular and has no
admittance. …`, `Capacitor.c has 4 phases but its cmatrix has 3 x 3 entries.
Specify cmatrix for 4 phases.`), followed by 482 `Error Encountered in Solve:
an element has no defined primitive admittance`, and `makeposseq` leaves them
as they are. The failed Y build still sizes the solution arrays, so
`summary`, `show currents`, `export summary` and `sample` after the refusal
read the circuit as not solved, and the next solve builds again and refuses
again. A disabled element is out of the model and refuses nothing, so the rest
of the circuit solves as it does without it. Before, the port panicked on the
delta forms (disabled or not), on the half-given reactor and on a phase count
above the matrix's order, read a matrix of an order above the phase count by
its first entries, a matrix nobody gave, and stamped a singular series matrix as a tiny
conductance in silence. The matrix reduction of a Reactor drops its `RCurve`
and `LCurve`, which the matrix stamp never reads and the R + jX stamp it
leaves would apply. Above three conductors a zero self term takes the stamp's
open fallback, as at three (`x = 1/EPSILON` in the parallel form, `r =
1/EPSILON` with `x = 0` in the series form), where `r = x = 0` turned the
solve NaN with no error. A shunt Reactor's no-load loss (`Export Losses`, the
meters' loss registers) is `V²/Rp` where the stamp adds `Rp` (the `kvar` and
R + jX forms). A matrix or Z1 reactor carrying `rp` reports no no-load loss
its stamp does not draw, and a `Parallel=yes` matrix reactor reports its whole
loss as I²R, as both oracles do, until `makeposseq` writes its resistance
branch as `rp` and the same loss turns no-load (open question 7). `?`, `Dump`
and the JSON export show a
matrix at the order it holds, and `Save circuit` writes it behind `Phases=`
at that order, so the reload holds the same element and its solve refuses it
again. A refused build drops the Y matrix it cleared, so `Export Y` and
`Show Y` answer 222 `Y Matrix not Built.` Before, the JSON export panicked on
a matrix short of the phases, `?` and `Save circuit` padded it with zeros to
the new order, which the reload took for a matrix of that order and solved in
silence, and `Export Y` wrote the emptied matrix as the system Y. The
energized steps of a bank are capacitors in parallel, so a closed delta bank
of two or more energized steps draws the sum of its steps at 12.47 kV: 600
kvar for `kvar=[300 300]`, and for `cuf=[4 3]` the 1231.1 kvar of its `cuf=7`
twin. Before, step k stamped steps 1 to k, a running sum (900 and 1934.5 kvar).
A `Parallel=yes` matrix Reactor whose given `rmatrix` or `xmatrix` does not
invert is refused by the solve (`Reactor.p: xmatrix is singular, so the
parallel reactor has no admittance. …`). Before, it solved with that branch
zeroed, an open branch where all-equal entries short every voltage pattern
orthogonal to (1, 1, 1), the positive sequence included.

**Physics.** Under a balanced positive-sequence voltage `V1 [1, a², a]` an
element of nodal matrix `Y` draws exactly `S = 3 |V1|² conj((1/3) v1ᴴ Y v1)`,
for any `Y`, and every off-diagonal pair of a symmetric `Y` contributes
`−Y_ij`. The positive-sequence circuit reports every element's power three
times the one phase, so a one-phase element carrying `S(Y)` reports the
multi-phase element's power exactly. That fixes the shunt values above. For a
balanced impedance matrix `1/S(Y)` equals the textbook `Zs − Zm`. Measured on
the port in a stiff 12.47 kV circuit, the reduced elements keep their power to
1.6e-16 .. 2e-11 (balanced and unbalanced banks, shunt, series, parallel,
two-phase and delta reactors, the fault), and the corpus deck
`modes/makeposseq/makeposseq_shunt.dss`, which holds only balanced three-phase
elements, reproduces its three-phase solution to 2.7e-8 in |V|. The rest is
the truncated `CALPHA` of its three-phase Z1/Z0 reactor and the solver's
convergence floor: with that reactor declared `r=1 x=12` and the solve at
`tolerance=1e-13` it agrees to 5.8e-12 in |V| and 2.5e-14 in power.

**Measured 2026-10-04** with the epri-worker on the r4133 DLL and the pinned
dss-python 0.15.7 (the same decks as the pins):

- Both oracles average the mutual terms over rows 2..N with the diagonals 2..N
  in, so a balanced matrix reduces to (N−2)/N of its value: `cmatrix`
  `[10|-2 10|-2 -2 10]` gives 4 (read back `[ 4E-006]` on r4133, which scales
  the farads by 1e-6 a second time, and the 20.47 µF creation default on
  dss_capi, which drops the scalar write onto the array property), the two-phase
  bank 0, the reactor `0.26667 + j3` (r4133) and `0.266666666666667 + j3`
  (dss_capi). The reduced balanced shunt reactor draws exactly three times its
  power on both (4571.3 + j51427.3 kVA against 1523.77 + j17142.43), the
  balanced two-phase matrix reactor turns into `R = X = 0` and a NaN solution,
  an unequal one (`rmatrix=[1|0.2 1.3] xmatrix=[10|1 12]`) into `R = −0.15,
  X = −1`, the `(M11 − M22)/2` the loop gives at two phases, an element that
  delivers 22.8 MW where the matrix element draws 972.5 + j9922.3 kVA, and a
  balanced series matrix reactor feeding a 40 + j30 Ω wye load drops 2.6 times
  too little voltage (the load gets 17.5 % too much power).
- A `Parallel=yes` matrix reactor becomes a series R + jX on both (4618.8 +
  j461.9 kVA against 1555.0 + j15550.1).
- A one-phase `z1=[1, 12]` reactor draws 3.000000 times its `r=1 x=12` twin on
  both.
- A three-phase delta `cuf=4` bank and delta `r=2.4 x=27` reactor (with or
  without `rp=3000`) draw a third of their power after `makeposseq` on both
  (703.47 → 234.49 kvar, 1523.77 + j17142.43 → 507.92 + j5714.14 kVA).
- A three-phase delta `kvar=17277.89 kv=12.47 r=2.4` reactor gets its wye X
  but keeps its leg R after `makeposseq` on both, so its power moves from
  1523.77 + j17142.44 to 4301.5 + j16130.8 kVA.
- A three-phase `gmatrix` fault keeps `G11` on both (863.95 → 719.96 A per
  phase).
- A two-phase `gmatrix=[0.12|0 0.12]` fault and its `r=8.333333333333334`
  twin each draw 12440.072 kW (863.947 A per phase) before `makeposseq` on
  both. After it both keep `G11` per phase for the matrix and the per-phase
  `r` for the twin, so each draws 18660.108 kW on dss_capi and 18660.326 kW
  on r4133 (re-measured 2026-10-05).
- The Line C1 divisor `N(N−1)/2` is right only at three phases: 22 nF for the
  two-phase line, 8.6667 for the balanced 4×4, on both.
- A delta `cmatrix` bank: both solve it to an active element that delivers
  67.7 kW (−67.7 kW and −664.4 kvar into its terminal), and the dss_capi
  process then dies with an access violation at exit, where its wye twin exits
  cleanly. A series matrix reactor given only `xmatrix` or only `rmatrix`: both
  stop with an access violation.
- The same delta `cmatrix` bank with `enabled=no` beside a 100 kW load: both
  solve the load at 100 + j53.9743 kVA, the value of the circuit without the
  bank, and the dss_capi process then dies with an access violation at exit
  (re-measured 2026-10-05, two runs of two).
- A 3 × 3 matrix after `phases=4` on a stiff 12.47 kV bus: the `cmatrix` bank
  and the `gmatrix` fault solve to NaN on dss_capi. On r4133, whose stamps
  read past the nine-entry arrays, both are reported converged: the fault
  with zero volts at every node, the source bus included, and the bank with
  the source voltage held on nodes 1 to 3 (7199.56 V, 0 V on node 4) and
  −33.846 kW − j19.541 kvar into its terminal, real power from a capacitor.
  The series matrix reactor stops dss_capi's Y build with error 234 `Matrix
  Inversion Error for Reactor "r"`. On r4133 its stamp reads past the matrix
  with a result that depends on the deck: error 303 (an access violation) and
  482 at `Solve` on this bus, a converged 1.573e21 + j2.581e21 kVA behind
  1e10 MVA. After `phases=2` both read the first four entries of the old
  array as a 2 × 2 matrix: the bank draws −195.408 kvar and the reactor
  11491.8 + j66982.5 kVA.
- A balanced series or `Parallel=yes` matrix reactor with flat curves
  `LCurve` 2 and `RCurve` 3 draws its curve-free power before `makeposseq` on
  both, and after it 3395.2 + j25464.1 kVA (dss_capi) and 3395.3 + j25464.4
  (r4133) against 4571.3 + j51427.3 and 4571.4 + j51427.9 without the curves:
  both apply the curves to the R + jX element they write.
- A shunt matrix reactor `rmatrix=[1|.2 1|.2 .2 1] xmatrix=[10|1 10|1 1 10]`
  and its `rp=500` twin each draw 1523.772 + j17142.431 kVA before
  `makeposseq` on both, the matrix stamp reading no `rp`. After it r4133
  gives 4571.425 and 4882.430 kW, dss_capi 4571.315 and 4882.317: both keep
  `rp=500` on the R + jX element they write, which stamps it, so the twin
  gains 311.0 kW (re-measured 2026-10-05).
- A series matrix reactor with all-equal entries (`rmatrix` all 1, `xmatrix`
  all 10): dss_capi aborts the Y build with error 234 `Matrix Inversion Error
  for Reactor "s3"` ("Invalid impedance specified. Replaced with tiny
  conductance."), r4133 stops with an access violation, and both solve to NaN
  after `makeposseq`. A four-conductor `Parallel=yes` reactor given only
  `rmatrix` stops both with an access violation at its `new`.
- `Export Losses` of a shunt matrix reactor and of a `z1=[1, 12] z0=[3, 20]`
  reactor, each with `rp=500`: both report a no-load loss of 311001.8 W that
  their stamps do not draw (the totals equal the `rp`-free twins', 1523772 W
  and 1072420 W). Both report the whole loss of a `Parallel=yes` matrix
  reactor as I²R.
- A closed delta bank of two energized steps on the stiff 12.47 kV bus
  (measured 2026-10-09): `kvar=[300 300]` draws 900.0 kvar on both, where its
  `kvar=[600]` twin and the wye `kvar=[300 300]` draw 600.0. `cuf=[4 3]` and
  `cuf=[1 2 4]` draw 1934.5 kvar (11 µF per leg), where `cuf=7` draws 1231.1.
  With `xl=[5 5]` the bank draws 917.8 kvar, where two one-step `xl=5` banks
  draw 302.9 each. One energized step (`states=[1 0]` or `[0 1]`) draws 300.0.
- A `Parallel=yes` reactor with `rmatrix` diagonal 100 and `xmatrix` all 10:
  both report error 233 `Error inverting X Matrix for Reactor.p - B is
  zeroed.` at its `new` and solve it to 1555.0 + j0 kVA, with the reactance
  branch open. With `rmatrix` all 100 and `xmatrix` diagonal 10: error 232
  (`G is zeroed`) and 0 + j15550.1 kVA (measured 2026-10-09).
- A `kvar=[200 400]` bank on the stiff 12.47 kV bus (measured 2026-10-09):
  both draw 400.0 kvar in wye and 600.0 in delta, and with `states=[0 1]`
  200.0 in either connection, where its rated steps give 600, 600 and 400.
  Every step takes the capacitance of the first step's 200 kvar (`Cuf` reads
  `[3.41166 3.41166]` in wye and `[1.13722 1.13722]` in delta), and the delta
  bank reaches 600 only through the running sum of its two steps. After
  `makeposseq` both write `kvar=[66.667 133.33]` with the first step's
  capacitance and the reduced banks draw the same 400, 600 and 200.
  A wye bank given `numsteps=2` and then `kvar=300` holds `kvar=[300 0]` on
  both and draws 600.0 kvar with `Cuf=[5.11749 5.11749]`, where its rated 300
  kvar is what the port draws. The wye `kvar=[200 400] harm=[5 5]` bank draws
  416.667 kvar on both, each step tuned at the first step's capacitance (`xl`
  reads `[31.1002 31.1002]`), where each step tuned to its own, `xl = X_C,i /
  25` (31.1002 and 15.5501 Ω), draws (25/24) × 600 = 625.0 kvar (measured
  2026-10-10).
- `Export Losses` of a closed delta reactor `kvar=17277.89 kv=12.47 r=2.4
  rp=3000` and of its `r=2.4 x=27 rp=3000` twin (measured 2026-10-09): both
  report a no-load loss of 51833.63 W, `3 |V1|² / 3000` node to ground, where
  the `rp` across the legs loses `3 |V_LL|² / 3000` = 155500.9 W, the no-load
  loss of their wye twin `r=0.8 x=9 rp=1000`. The two-phase delta `r=2.4 x=27
  rp=3000` on `b.1.2` (legs 1-2 and 2 to ground) reports 34555.76 W,
  `2 |V1|² / 3000`, where its legs lose `4 |V1|² / 3000` = 69111.5 W. The
  totals (1679275, 1679273 and 746343.4 W) are right on both, so the I²R
  column carries the difference. A wye reactor from node 1 to node 2 of the
  bus, `bus1=b.1 bus2=b.2 phases=1 r=0.8 x=9 rp=1000`, a shunt by its bus
  names, reports 51833.63 W on both, `|V1|² / 1000` node 1 to ground, where
  its `rp` between the terminals loses `|V1 − V2|² / 1000` = 155500.9 W, with
  the same total of 1679273 W (measured 2026-10-10).

**Sources.** r4133 `Version8/Source/PDElements/Capacitor.pas:817-829` and
`Reactor.pas:1169-1191` (the averaging loops), `Line.pas:1579-1585` (the C1
divisor), `Reactor.pas:927-933` (the one-phase `Z1` divided by 3),
`Reactor.pas:771-780` (SpecType 1/2 never reads `Parallel`), the delta arms
`Capacitor.pas:814-816` and `Reactor.pas:1157-1168`, `Fault.pas:720-731`. A
matrix is allocated at the phase count of its parse (`Capacitor.pas:249-255`,
`Fault.pas:203-209`), a `phases=` edit changes only the conductor count
(`Capacitor.pas:400-408`, `Fault.pas:303-309`), and the stamps read the old
array at the new count (`Capacitor.pas:1033-1040`, `Reactor.pas:830-832`,
`Fault.pas:539-546`). `Reactor.pas:238-240` (the `RCurve`/`LCurve` help: R or
Z, and X, LmH, Z or kvar), `:836-838` and `:968-970` (error 234 on a series
matrix that does not invert), `:1049-1078` (`GetLosses`: the `Rp` branch for
every spec type, summed node to ground where the delta stamp at
`:782-795` puts `Gp` across the legs), `:686` (`Gp = 0` for `Rp = 0`), `:938` (the sequence
matrix gets off-diagonal terms at three phases only), `:698` and `:706`
(errors 232 and 233, the parallel matrix zeroed). `Capacitor.pas:682-686` (one
work matrix for every step, added after each) and `:1016-1017` (`AddElement`
in the delta arm, where the wye arm at `:1026` uses `SetElement`), `:611`
(every step's `FC` from `FkvarRating^[1]`). The same
procedures in
the vendored dss_capi 0.14.5 (`src/PDElements/Capacitor.pas:768-819` and `:642`, `Reactor.pas:1052-1115` and its
`CalcYPrim` at `:880-881`, `Line.pas:1594`, `Fault.pas:604-609`).

**Decision.** Physics decides the value, and no engine computes it: the port
writes `S` of the stamped matrix in both lanes and reproduces neither the
averaging nor the one-phase `Z1/3`. Above three conductors the ruling's mean
diagonal minus mean off-diagonal term applies to `cmatrix`, the Line C1 and,
read literally, to `rmatrix` and `xmatrix` (open question 3 below). The delta
matrix forms and the half-given matrix reactor have no stamp the port can
defend, so it refuses them (open questions 2 and 4). A matrix of another order
than the phases describes no element of that phase count, so it is refused
too, and what a `phases=` edit does to the matrix stays open (question 5). A
one-phase fault still reads the first entry of a larger `gmatrix`, the `G11`
that the bare `Phases=1` reduction of a fault with no positive-sequence
conductance (`S(G) <= 0`) leaves (part of question 5). A series matrix whose
impedance does not invert has a current pattern that meets no impedance and so no admittance:
it is refused, where dss_capi stops with error 234 and r4133 crashes, and the
open circuit the tiny conductance stood for is wrong in every sequence. The
curves and the `rp` a matrix stamp never reads are not carried into the R + jX
element (question 6), and a zero self term above three conductors takes the
open fallback the stamp takes at three. A `Parallel=yes` matrix reactor
stamps `R⁻¹` and `−X⁻¹`, so a given matrix that does not invert has no
admittance and is refused like the series form. The no-load loss is the loss
of the `rp` branch the stamp holds, at the voltage the stamp puts across it:
the leg voltages of a delta, terminal to terminal of a wye. A matrix or Z1
reactor carrying `rp` reports none. A `Parallel=yes` matrix reactor keeps the I²R label both oracles give
its whole loss, and the reduction, which writes that branch as `rp`, moves it
to no-load (question 7). The steps of a bank are capacitors in parallel, each
of the capacitance of its own rating and tuned by `harm` to that capacitance,
so a bank draws the sum of its energized steps and a delta bank stamps each step once, in both lanes. A disabled
element is out of the model, as on r4133, so its refusal is not asked. One-
and two-phase `cuf`, R/X and Z1 elements, a Fault given by `r` and the
one-phase `cmatrix` bank keep their per-phase value as before, which
overstates them by 3/N (open question 1), and the Line's two-phase Z1 stays
per phase beside its shunt-exact C1.

**Exclusion.** One corpus case moves, `modes:makeposseq/makeposseq_shunt.dss`
on `capi_v0145` (the r4133 channel runs no matrix reduction):
`makeposseq-matrix-average-capi` (`yprim` of `Reactor.rx_mat`),
`reactor-one-phase-z1-capi` (`yprim` of `Reactor.rx_z1`),
`makeposseq-matrix-average-capi-props` (the `rx_mat` probes `r`, `x` and
properties `r`, `x`, `z`, `lmh`, exact pairs), the `cuf`/`normamps`/`emergamps`
pairs of `makeposseq-cuf-applied-capi-props` re-pinned (12 µF), and the
`injection` field added to `makeposseq-cuf-applied-capi`. No golden byte, no
tolerance and no other case moves. No corpus case holds a delta bank of two or
more steps, a `kvar` bank of unequal steps or of a `kvar` array shorter than
its steps, a Capacitor with `harm`, a shunt reactor with `rp` or a
`Parallel=yes` reactor, and no golden or ledger entry holds the unequal or
short steps or the `rp` reactor.

**Pins** (both lanes): `elements::pd::capacitor::tests::make_pos_sequence_cmatrix`,
`make_pos_sequence_cmatrix_applies_the_positive_sequence_cuf`,
`make_pos_sequence_unbalanced_cmatrix_keeps_the_banks_power`,
`make_pos_sequence_two_phase_cmatrix_keeps_the_banks_power`,
`make_pos_sequence_four_conductor_cmatrix_takes_mean_self_minus_mean_mutual`,
`make_pos_sequence_delta_cuf_bank_keeps_its_power`,
`a_delta_cmatrix_bank_refuses_the_solve`,
`a_disabled_delta_cmatrix_bank_leaves_the_solve_to_the_rest`,
`a_cmatrix_of_another_phase_count_refuses_the_solve`,
`a_multistep_delta_bank_draws_the_sum_of_its_steps`,
`a_kvar_bank_of_unequal_steps_draws_its_rating`,
`derived_cuf_multistep_takes_each_steps_own_rating`;
`elements::pd::reactor::tests::make_pos_sequence_matrix`,
`make_pos_sequence_matrix_shunt_keeps_the_reactors_power`,
`make_pos_sequence_matrix_series_keeps_loss_and_load`,
`make_pos_sequence_unbalanced_matrix_shunt_keeps_the_reactors_power`,
`make_pos_sequence_parallel_matrix_keeps_the_reactors_power`,
`make_pos_sequence_parallel_matrix_takes_the_admittance_self_terms`,
`make_pos_sequence_two_phase_matrix_keeps_the_reactors_power`,
`make_pos_sequence_four_conductor_matrix_takes_mean_self_minus_mean_mutual`,
`make_pos_sequence_four_conductor_parallel_matrix_takes_mean_self_minus_mean_mutual`,
`a_single_phase_z1_reactor_stamps_z1`,
`make_pos_sequence_delta_reactor_keeps_its_power`,
`a_delta_matrix_reactor_refuses_the_solve`,
`a_matrix_reactor_without_both_matrices_refuses_the_solve`,
`a_refused_reactor_set_aside_by_disable_leaves_the_solve_to_the_rest`,
`a_matrix_of_another_phase_count_refuses_the_solve`,
`a_singular_series_matrix_refuses_the_solve`,
`a_singular_parallel_matrix_refuses_the_solve`,
`make_pos_sequence_four_conductor_zero_self_term_stays_finite`,
`make_pos_sequence_matrix_drops_the_curves_it_never_read`,
`the_no_load_loss_is_the_parallel_branch_the_stamp_holds`,
`a_delta_reactor_loses_its_rp_across_the_legs`,
`a_wye_reactor_loses_its_rp_between_its_terminals`,
`dump_prints_the_matrices_a_reactor_holds`;
`elements::pd::line::tests::make_pos_sequence_matrix_branch` (the three-phase
reduction to the bit), `make_pos_sequence_two_phase_line_c1_is_the_shunt_self_term`,
`make_pos_sequence_four_conductor_line_takes_mean_self_minus_mean_mutual`,
`make_pos_sequence_four_wire_geometry_line_takes_mean_self_minus_mean_mutual_per_length`;
`elements::pd::fault::tests::make_pos_sequence_gmatrix_fault_keeps_its_current`,
`make_pos_sequence_two_phase_gmatrix_fault_keeps_its_power`,
`make_pos_sequence_gmatrix_fault_with_no_conductance_draws_nothing`,
`a_gmatrix_short_of_the_phases_refuses_the_solve`,
`a_gmatrix_larger_than_the_phases_refuses_the_solve`,
`a_refused_matrix_element_saves_and_reloads_refused`;
`exec::tests::make_pos_seq::makeposseq_shunt_reproduces_its_three_phase_solution`;
`solution::ymatrix::tests::a_refused_build_leaves_no_y_and_logs_no_build_event`,
`a_refused_build_keeps_each_bus_voltage_across_a_renumbering`;
the helper's own tests in `elements::pos_seq::tests`.

**Open, for the user.** (1) Whether one- and two-phase elements of every class
should keep their total under `makeposseq`. The `kvar` arms and the Load do,
bar a two-phase delta `kvar` Reactor, the `cuf`, R/X, Z1 and Fault `r` arms
do not: two two-phase banks with one YPrim reduce to 20/3 µF
(`cmatrix=[10|0 10]`) and 10 µF (`cuf=10`), the two-phase
`gmatrix=[0.12|0 0.12]` fault keeps 12440.1 kW where its `r=` twin draws
18660.1 kW, and a two-phase Line writes its shunt total as C1 (22/3 nF for
`[10|-2 10]`) beside a Z1 per phase. A two-phase delta Reactor (legs 1-2 and
2 to ground) given by `kvar=11518.59 kv=12.47 r=2.4 rp=3000` draws 746.3 +
j7618.9 kVA before `makeposseq` and 2036.8 + j11165.7 after, its
`r=2.4 x=27` twin with the same `rp` 559.8 + j5714.1 after, on the port and
dss_capi and within 0.1 kvar on r4133 (measured 2026-10-09): the `kvar` arm
keeps the nameplate kvar, which the leg to ground does not draw. (2) Whether a delta matrix element of
two or more phases is refused (taken) or read with a meaning. The Capacitor
page reads "Cmatrix Alternate method of defining a capacitor bank. Enter nodal
capacitance matrix in uf. Can be used to define either series or shunt banks."
and gives `Conn` as "{delta | ll} for delta (line-line) connected banks". A
nodal matrix supports one reading, the same nodal block on the bus whatever
`conn` says (`conn=delta` only drops the second terminal, so the bank draws
what its wye twin with a grounded `bus2` draws), and the refusal of a delta
bank narrows input the documentation accepts. The Reactor page gives a delta
matrix no meaning. (3) Whether a reactor above three conductors reduces
`rmatrix` and `xmatrix` each (taken) or its admittance (0.825 + j8.25 against
0.7939 + j7.9391 for an unbalanced 4×4, equal for a balanced one). (4) Whether
a missing `rmatrix` or `xmatrix` is refused or read as an absent branch. The
series form refuses it (taken). The parallel form reads it as an open branch,
with an error at the edit (taken), which
`make_pos_sequence_four_conductor_zero_self_term_stays_finite` pins. The
inversion the parallel refusal shares with the stamp pivots on the diagonal,
so it also refuses an invertible matrix with a zero diagonal entry, such as
`rmatrix=[0|1 0|0 0 1]`, and calls it singular. Such a matrix is not passive. (5) What
a `phases=` edit does to a matrix given before it: drop it, resize it, or keep
it and the solve refuses the element (taken, and the kept matrix reads back
and saves at its own order), and what a one-phase fault reads from a larger
`gmatrix` (its first entry, taken). The order itself is closed by the
documentation, which is the language specification (user ruling 2026-10-04):
the Reactor and Line pages say "Order of the matrix is the number of phases",
and the Capacitor and Fault pages call their matrices nodal, which fixes the
order to the terminal's node count (an inference). (6) Whether `rp`, `RCurve`
and `LCurve` on a matrix reactor, which its stamp never reads, are refused,
stamped with the whole matrix branch, or ignored (ignored, taken), and with
them the `rp=0` the reduction writes for no parallel branch. (7) Whether the
resistive loss of a `Parallel=yes` matrix reactor is no-load like that of `rp`,
or I²R as both oracles report it (taken). `makeposseq` writes that branch as
`rp`, so its loss moves from I²R to no-load. (8) Whether a line above three
conductors with its neutral retained, the documentation's own example of such
a line ("a 4x4 Z matrix with the neutral retained", and LineGeometry `reduce`
defaults to no), reduces over its phase conductors alone, the neutral
Kron-reduced in `Z` and struck in `C`. With the neutral on node 0 at both ends
that gives the exact values, and the ruling's mean over all four conductors
misses them: a 5-mile four-wire line feeding a 3000 kW, 1500 kvar
constant-impedance load at 12.47 kV delivers 2683.6 + j1341.8 kVA with 99.0 kW
of line loss, and after `makeposseq` (R1 0.3775, X1 0.63075, C1 17.5)
2631.9 + j1316.0 kVA with 119.7 kW, where the phase values 0.306, 0.5618 and
18 give the unreduced solution exactly (both oracles write the same R1 and X1
with C1 12.75, measured 2026-10-05). A plain matrix line above three
conductors marks no neutral, so what it means is part of the question.
(9) Whether a two-phase `z1` Reactor, which stamps only the diagonal
`(Z0 + Z1 + Z2)/3` in the port and on both oracles, takes the 2 × 2 block of
the three-phase sequence matrix, keeps the diagonal (kept) or is refused: the
documentation gives no rule for two phases. Left for later on the same path:
the property order `conn=delta phases=3` giving a three-phase delta element
four conductors (only guarded here, the delta factors require three), as on
dss_capi, where r4133 keeps three and the `r=2.4 x=27 rp=3000` reactor draws
its closed-delta 1679273 W against 1306101 W in the port and on dss_capi
(measured 2026-10-10), a second
`makeposseq` dividing a Vsource's base kV by √3 again, and the default
`NormAmps` and `EmergAmps` of a `Cuf` Capacitor, which the reduced bank
inherits: `1e6 × phases` too small on both oracles and in the port (43.9695 A
for a 703.47 kvar bank at 12.47 kV given by `kvar`, 1.46565e-5 A for its
`cuf=12` twin). So `Export Powers` shows every `Cuf` bank, shunt banks
included, over its normal and emergency ratings by its whole power (703.5
kvar for `cuf=12`, 0 for its `kvar` twin, on r4133 and in the port), and
`makeposseq` gives a `cmatrix` bank that reading (0 before, 703.5 after). On
the same stamp, on all three engines: the default `NormAmps` of a delta bank is
its `kvar` per phase over the line-to-line kV, a leg current held against the
line current, so a 600 kvar delta bank at its rated 12.47 kV shows 132.3 kvar
over its normal rating. Off the path, still stamped as
a tiny conductance in silence: a
Z1 Reactor with a zero sequence impedance, and a series matrix reactor in a GIC
solve, which stamps `rmatrix` alone, when that matrix does not invert.

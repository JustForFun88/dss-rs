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

## AllowNoneItem — `none` in conductor lists — SETTLED (WP-U1.1 item 3, adopt capi015)

**Observable.** A `none` entry inside a `DSSObjectReferenceArrayProperty` — the
conductor lists `Wires`/`CNCables`/`TSCables` on Line and LineGeometry (SVN
r3902/r3913, `TPropertyFlag.AllowNoneItem`).

**Probe** (`probe_nonewire.py`, 2026-07-12; `new linegeometry.g nconds=2
nphases=2 ~ wires=(w none)`):

| engine | `wires=(w none)` |
|---|---|
| capi 0.14.5 (default) | **error #40303** "WireData object `none` not found" |
| capi015 (0.15.0b4) | **accepted** — NIL slot, no error |

**Decision — adopt the capi015 accept-`none`.** Added `PropFlags::ALLOW_NONE_ITEM`
(Pascal `AllowNoneItem`, distinct from the single-ref/`DoubleVArray` `AllowNone`),
set on Line + LineGeometry `Wires`/`CNCables`/`TSCables`. The `ObjectRefArray`
parse arm now resolves a `none` token (when the flag is set) to a **`None`
slot** instead of the "not found" error; the storage chain
(`set_object_ref_array` → `set_wires`/`set_cables`) threads
`ObjectRefArrayItem = Option<(name, ElemRef, view)>` and leaves a `None` slot NIL
(both `line_wire_data`/`fwiredata` are already `Vec<Option<…>>`). Also exposed
`Parser::is_quoted()` (item 3 "WasQuoted plumbing" — the parser already tracked
`IsQuotedString`; WP-U2's per-phase state arrays will consume it).

**Scope split with WP-U1.4.** This item is the **parser/storage plumbing** only.
The mixed-conductor-list *numerics* that actually consume a NIL conductor (the
new `Conductors` property, EqDist spacing, CN/TS mixing) are **WP-U1.4**. A
`none` conductor in isolation is degenerate — capi015 accepts the parse but then
`#303`s on the incomplete geometry (probed), so there is no solvable standalone
`none` deck; §1.7's "solves on target oracle" is unattainable until U1.4.

**Gate consequence.**
- **Gate-safe:** no corpus deck puts `none` in a conductor list (scanned — 0
  hits), so no default-oracle case moves; the `Option`-threading kept the
  non-`none` path byte-identical (the whole line/line_geometry unit suites +
  full workspace gate stay green).
- **Pinned by feature-sensitive unit tests** (no live oracle, per above):
  `dss-core` `line_fetch::conductor_list_accepts_none_entry` (`wires=(w none)`
  yields a NIL slot with no error, while a non-`none` missing name — `nope` —
  still errors "not found"), + `dss-parser`
  `is_quoted_reflects_the_last_token_quote_state`.
- **Readback of a NIL-slot list is UB on the oracle (settle 2026-07-12,
  `probe_wires2.py`) — not reproduced.** `? linegeometry.g.wires` on a list
  containing a `none` slot raises a capi015 **Access Violation** (#303 "Access
  violation": the FPC readback dereferences the NIL wire pointer). There is thus
  no defined oracle readback STRING to pin against; per the project UB rule the
  port does NOT reproduce the crash — it renders `[w, ]` deterministically (NIL →
  `""`, consistent with the probed single-ref cleared-ref `""` rendering). The
  `conductor_list_accepts_none_entry` assertion documents this.
- known_diffs: nothing to retire (0.14.5 errored, the port errored — no prior
  Rust↔EPRI entry).

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

**(b) AllowNone-single-ref — NOT ported (capi015 no-op quirk).** The C7 feature
is **observably a no-op in capi015**: its AllowNone branch sets `otherObj := NIL`
(l.862) but the *unconditional* `if otherObj = NIL then DoSimpleMsg(… 401)`
immediately below (l.867) fires the "not found" error anyway — so `fusecurve=none`
on capi015 clears the ref AND logs #401, bit-identical to the plain not-found path
the port ALREADY reproduces. Adding a silent AllowNone clear in Rust would
**diverge** from capi015. So the port sets NO `ALLOW_NONE` flag on these refs;
`fusecurve=none` clears + logs #401 through the existing path. Pinned by
`fuse_curve_none_clears_with_error_like_capi015`.
- **r4133 divergence (Rung-2 note, not adopted here):** r4133 stores the literal
  `none` as the ref name (readback `none`, no #401) instead of clearing to `''`.
  The functional effect (no curve at runtime) is the same across all three; only
  the readback + error-log differ. Recorded for a Rung-2 revisit; no Rung-1
  action. No live oracle gates (b): capi015 raises the #401 in dss-python
  (early-abort), r4133 renders a different readback — the divergence is unit-test
  pinned, same rationale as §ParseAsSymMatrix.

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

## B1 — Capacitor Cmatrix YPrim diagonal ×1.000001 before inversion — SETTLED (WP-U1.2, adopt capi015)

**Observable.** The YPrim of a `Cmatrix` (SpecType=3) capacitor that also carries
a **series filter reactance** (`R`/`XL` > 0 ⇒ `has_zl`) — the only config that
reaches `MakeYprimWork`'s SpecType-3 inversion path.

**dss_capi 0.14.5 (default).** The SpecType-3 branch inverts the C-admittance
work matrix directly. **0.15.x (capi015) / EPRI r4088+.** Each work-matrix
diagonal is first multiplied by `1.000001` ("Add a little bit to each phase so it
will invert", `Capacitor.pas` `MakeYprimWork`) — the same perturbation the Delta
1|2 branch already used — so a (near-)singular C matrix still inverts.

**Decision — adopt** (`capacitor/solve.rs`, the SpecType-3 `_ =>` arm gains the
×1.000001 diagonal loop before `invert()`).

**Probe** (`/tmp/probe_capfull.py`, 2026-07-12; `Capacitor.f1 conn=wye
cmatrix=(1.5|0.2 1.5|0.2 0.2 1.5) R=0.5 XL=3`, `? Yprim`):

| engine | `Y[0,0]` (phase self) |
|---|---|
| capi 0.14.5 (default) | `(-5.29e-23, -1.08e-19)` — **garbage** (singular invert) |
| capi015 (0.15.0b4) | `(1.661774199e-07, 5.664824416e-04)` — **finite** |

Strongly revision-**sensitive** (garbage → finite) and feature-sensitive (without
`R`/`XL` the SpecType-3 invert path is never reached).

**Gate consequence.**
- **No corpus/live witness.** No vendored deck defines a Cmatrix capacitor with a
  series reactance (the corpus caps are simple shunt-kvar); the only `cmatrix`
  hits in cap-bearing decks are LineCode matrices. So no corpus_live case and no
  golden moves.
- **Pinned by an oracle-validated unit test**
  (`capacitor::tests::cmatrix_with_series_reactance_yprim_matches_capi015`): the
  Rust YPrim phase block equals the capi015 probe reference to 1e-11/1e-12; the
  0.14.5 garbage (~1e-23) fails the `5.66e-4` diagonal assertion, so it is
  feature-sensitive to the ×1.000001. (A live capi015 deck was prepared but the
  modes manifest's mixed manual unicode-escaping/CRLF blocks a clean append; the
  unit test carries the exact capi015 numbers instead — same oracle, offline.)
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
Vterminal[NextDeltaPhase(j)]` — **line-to-line** (`InvControl.pas` l.1647-1652,
r3822; `NextDeltaPhase(iphs)=iphs+1`, wraps to 1 past `NCondsDER`).

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
generic-abort), but they only fire on **malformed** input (0 corpus decks use
`MonBus` — scanned), never in a valid deck; the *sequence* (both engines abort on
an invalid bus) is preserved. Pinned by the feature-sensitive unit tests
`c8_monbus_missing_nodes_errors_2024111` and `c8_monbus_invalid_bus_aborts_2024112`.
`known_diffs`: nothing to retire.

## D14 — DynamicExp RPN evaluator "index-bug fix" (a no-op evaluator) — SETTLED (DYNEXP WP, pulled ahead of WP-U1.6; adopt capi015)

**Observable.** `TDynamicExpObj.SolveEq` — the per-step evaluator that a
`DynEqPCE` (Generator / PVSystem / Storage / WindGen with `DynamicEq=`) calls
each dynamics substep to compute the state-variable derivatives. The result is
visible in the mode-3 monitor DynamicExp slots and, downstream, in the whole
dynamics trajectory (rotor swing, inverter current ramp) and the solved node
voltages a DynExp-driven element injects.

**dss_capi 0.14.5 (default).** `SolveEq` walks the compiled `Cmds` automation
array (`for idx := 0 to High(Cmds)`), evaluating the RPN right-hand side and
writing each output's derivative into `MemSpace[OutIdx][1]`. (It reads
`Cmds[idx+1]` one past the end at the final index — a benign OOB read that
happens not to alias `-50`.) **0.15.x (capi015) / upstream `2a8bdb78`
("DynamicExp: reuse RPN, fix index bug").** Two coupled edits: the loop bound
becomes `High(Cmds) - 1`, **and an `Exit` is added right after the first
equation's output index is latched**. A well-formed compiled stream always
starts `[outIdx, -50, ...]`, so the loop hits that marker at idx 0 and returns
immediately — **the RHS is never evaluated**. `SolveEq` is now a no-op: every
derivative slot is left exactly as the host set it, so the state variable stays
frozen at its `InitStateVars` seed value. (The RPN calculator also becomes a
reused member field; with the `Exit` it is never stepped, so that part is
form-only.) This is an upstream regression introduced by the "fix", but it is
deterministic and defined — capi015 is the port target for the DynExp path.

**Probe (Oddie/capi015 vs pinned capi 0.14.5, `probe_fault.py`/`probe_repin.py`).**
`probe_fault.py` runs a reduced `SimpleDemo` Kundur DynExp generator (1000
pre-fault substeps, an 86-step bolted fault at HT, then a 500-step post-clear
window):
- **0.14.5** — rotor swings: `speed` → 0.487, `theta` → 2.054 rad, `dspeed`
  → -4.10 (real dynamics). NOTE: these are the endpoint of an *undamped*
  (non-decaying) swing, so the exact values are scenario-dependent — a different
  step count or the vendored `Dynamic_KundurDynExp.dss` deck lands elsewhere on
  the same oscillation (e.g. that deck → speed 0.626, theta 2.036, dspeed -4.58).
  The load-bearing fact is qualitative: 0.14.5 **swings**.
- **capi015** — fully frozen on every scenario: `speed` = 0, `dspeed` = 0,
  `theta` = 0.72907156 (the seed angle), `dtheta` = 0 — no swing at all.
The Rust port with D14 reproduces capi015 to the f32 monitor floor on every
channel (verified live via `probe_repin.py` on both engines: generator
`speed`/`theta` frozen; PV `it` held at its seed fixpoint 23.14571 with `dit`=0;
Storage `it` held at its 0 seed, `modul` still host-evolved 0.9000948→0.9463813).

**Decision — adopt for the DynExp path** (`dynamic_exp.rs::solve_eq`: loop bound
`0..cmds.len()-1` + early `return` at the first output marker; `get_out_idx`
inner loop bound shortened, form-only). Cited to the 0.15.x Pascal + commit
`2a8bdb78` in the doc comment.

**Gate consequence.**
- **`Run_IEEE123Bus_GFLDaily_DynExp.DSS`** re-promoted from
  `skipped_needs_investigation` to `solvable_now` under `oracle:capi015`
  (`large_floating_delta`): D7 (landed) + D14 (this WP) together take it off both
  mid-rung engines and onto capi015 (was 8.31e-3 > 7.40e-4 at entry 0 vs capi015
  pre-D14).
- **`Dynamic_KundurDynExp.dss`** flipped to `oracle:capi015` (runs a full DynExp
  swing; the frozen trajectory matches capi015, not 0.14.5). Its
  `-steady-state-only` sibling stays on the default oracle (never enters
  dynamics, so `SolveEq` is never called).
- **7 `exec/tests/dynamics.rs` DynExp unit gates** re-pinned from the 0.14.5
  swing values to the capi015 frozen values (generator mode3/fault/swing = 3;
  PVSystem mode3/safe-fault = 2; Storage mode3/trip-fault = 2). They are now D14
  regression guards: un-doing the no-op makes the state ring again and diverges
  from capi015. The `dynamic_exp` unit tests keep the InterpretDiffEq `cmds`
  compilation pins (unchanged) and pin the `SolveEq` no-op (derivative slot left
  untouched), with an index-bug witness (Kundur multi-eq) and a single-output
  no-op witness.
- `known_diffs`: none matched — nothing to retire.

## A3/A5 — PCE force hooks (`Set`/`Get` InjCurrent/ITerminal/YPrim/StateVar/…) — SETTLED (WP-U1.9, adopt capi015)

**Observable.** The `Set`/`Get` options `InjCurrent`/`ITerminal`/`YPrim`/
`StateVar`/`IterNumber`/`CtrlIterNumber`/`IntegrationFlag` and the element flags
`Flg.ForceInjCurrents`/`Flg.ForceYPrim` — the pyControl co-simulation engine
hooks (the `pyControl` component + `Set PyPath=` stay `NOT_PORTED`, §0).

**Source-tree note (important for future WPs).** These options do **not** exist
in the vendored working tree `.inputs/dss_capi_with_git` — it is checked out at
`master` (`f5728aec`), a commit *after* `0.15.0b4` where the pyControl hooks were
**removed** upstream. But the **capi015 oracle** the plan pins to is
`0.15.0b4` (tag `e936d210`), which **does** carry them (`get InjCurrent` returns
a value; `git show 0.15.0b4:src/Executive/ExecOptions.pas` shows the enum tail
`…StateVar, PyPath, IterNumber, CtrlIterNumber, InjCurrent, ITerminal, YPrim,
IntegrationFlag, …`). The spec for this WP was therefore read via
`git show 0.15.0b4:`, not the working tree. `delta_capi_0145_015x.md` A3 also
wrongly listed `SampleControlDevices` as a new hook — it is present already in
`0.14.5` (`Solution.pas:1974`) and was ported long ago
(`solution/controls/sampling.rs`); not a delta.

**Decision — adopt the capi015 (=0.15.0b4) behavior.** `ElemFlags::FORCE_YPRIM`/
`FORCE_INJ_CURRENTS`, honored in the injection loop
(`solution/solution/power_flow.rs::get_pc_inj_curr_filtered` injects the stored
`InjCurrent` directly, per `TPCElement.InjCurrents`) and in `ReCalcAllYPrims`
(`solution/ymatrix.rs` skips `CalcYPrim` for a `ForceYPrim` element); the five
PCE `GetTerminalCurrents` skip the model recompute when forced (Load/Generator/
PVsystem/Storage/IndMach012). The option set/get is in `exec/set_cmd.rs`/
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
- **capi015-pinned Rust unit suite** `exec/tests/force_hooks.rs` (12 tests):
  forced Vmag 7224.143523 (1e-6), frozen `Get InjCurrent`/`ITerminal` (Load) +
  frozen Generator `ITerminal` (2nd PCE force-skip), `Set ITerminal` freeze
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

## Line/LineGeometry Conductors (text upstream-broken) — SETTLED (WP-U1.4 wt-u14cond, reproduce 1:1)

**Observable.** The 0.15.x `Conductors` property — Line prop 34 (`Line.pas:62`),
LineGeometry prop 20 (`LineGeometry.pas:80`) — a mixed
`WireData|CNData|TSData` object-reference-array over a `TProxyClass` created with
`fullNames=True` and `.Name = "Conductor"` (`DSSClass.pas:2603`;
`LineGeometry.pas:159`). Replaces `Spacing, Wires` with `Spacing, Conductors` in
the spacing spec-set; `Wires`/`CNCables`/`TSCables` become `RedundantWith(Conductors)`.

**Empirical capi015 behavior (0.15.0b4, probed 2026-07-17) — the text property is
BROKEN.** Every `Conductors=[…]` with a real item errors and never populates the
array:
- A class-prefixed item (`Conductors=[WireData.w1, …]`, ANY case) → `#10103
  "…Conductors: Invalid class (wiredata) for item. Valid classes:
  (WireData|CNData|TSData)"`. **Root cause: a deterministic upstream bug** —
  `TProxyClass.GetDSSClass` (`DSSClass.pas:2644`) compares the parser's
  `AnsiLowerCase`d class token (`ValidateObjectItem`, `DSSObjectHelper.pas:6462`)
  against the *original-case* `TargetClassNames` (`'WireData'`…), never satisfiable
  (the parallel `TargetClassNamesLower` array is never consulted).
- A bare item (`Conductors=[w1, …]`) → `#10103 "…Conductors: You must define the
  Conductor class for all the valid items in the array."` (`FullNameAsArray`
  requires a class prefix).
- `Conductors=` before the spacing / `NConds` (array count `< 1`) → `#402
  "…Conductors: No objects are expected! …"` (checked before item validation).
- All-`none` → **Line** parses (all NIL slots, model → spacing, `phaseChoice =
  Overhead`; err#0); **LineGeometry** rejects it → `#10103 "…Conductors: At least
  one valid conductor must be provided."`.
- `? <elem>.Conductors` (the text getter) → **Access Violation (#303)** in capi015
  — a getter UB, NOT reproduced (safe name list instead).

So text `Conductors=` can only ever be all-`none` (a no-op) or an error; the
property is otherwise reachable only through the JSON export/import round-trip.

**Decision — reproduce 1:1 (`TODO(compat)`), keep the JSON masquerade + HIDE_015X.**
- The proxy resolution + the four diagnostics are reproduced exactly in
  `parse_conductor_proxy` (`obj/props/class_props/parse.rs`), with a
  `TODO(compat)` on the `GetDSSClass` case bug (the clean fix — compare the
  lowercased token against lowercased class names — lands in the §6 shim sweep;
  the golden/unit pins hold it until then). The `#303` getter crash is UB → not
  reproduced.
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
  the §6 sweep flips the surface and drops the masquerade.

**Gate.** `PROPS_015X += ("Line", …+"Conductors")` and `("LineGeometry",
["Conductors"])` (the inserted props excluded from the 0.14.5 property-table
walk); `tests/upgrade_conductors.rs` pins all four capi015 diagnostics + the
all-`none` split (Line parses / LineGeometry rejects). The net-new
**resolved-ref** fill (unreachable via the broken text parse; the path the
§6-fixed parser and a JSON-import round-trip take) is gated by whitebox
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
  (WP-U1.6 tail). `Normal`→`NormalState`, `State`→`PresentState`, `Action`→
  `CurrentAction` (distinct offsets, `SwtControl.pas:156-166`); the side effects
  sync `CurrentAction := NormalState`/`PresentState` (were the reverse). 0.14.5
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
  0.14.5/r4133, unit-pinned `locked_ignores_action_write` /
  `locked_ignores_normal_and_state_writes`). Feature-sensitivity: unit
  `d12_normal_and_state_readbacks_are_independent` (0.14.5 conflated both onto
  `CurrentAction`).
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
  (= 200·`[0.10 0.20 0.35 0.55 0.80 0.65 0.45 0.25]`). **Gate:** new capi015 live
  deck `modes/upgrade/mmf_singlecol/mmf_singlecol.dss` (`oracle:"capi015"`,
  `n_steps=8`, whole-model per-step compare; cannot gate 0.14.5 — the deck is the
  bug the fix removes, §1.2; §1.7 two-process determinism confirmed, fingerprint
  `0ead40d7199b0781`) + unit `mmf_single_column_csvfile_loads_like_capi015`. The
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

## WP-U1.6 C5 — RegControl signed thresholds + idle zones — SETTLED (adopt capi015 = r4086)

`8a898cba` (SVN r4086, in capi015 0.15.0b4) reworks RegControl's reverse-power
surface and adds an idle-zone family:

- `RevThreshold` becomes a **signed W** field with a kW→W property scale
  (default **−100 kW**, was +100 kW), and a new `FwdThreshold` (+100 kW) splits
  the forward edge. The reverse-power detection sign moved from the *comparison*
  into the *stored value* (`FwdPower < RevPowerThreshold`, no unary `−`), so a
  legacy deck that sets only `revThreshold=X (X>0)` is **behavior-identical**:
  `EndEdit`'s compat fallback sets `Fwd:=abs(Rev); Rev:=−Fwd`, restoring the old
  symmetric ±X band. The fallback is per-edit (tracked via a new `PrpSequence`
  BeginEdit boundary), so a later rev-only edit re-symmetrizes and clobbers an
  earlier `FwdThreshold` — reproduced 1:1 (capi015-probed, 2026-07-16).
- New `Idle`/`IdleReverse`/`IdleForward` flags suppress a pending tap when the
  through-power sits in a dead-band. Ported verbatim, **including** the no-load
  test's `(FwdPower>=Rev) or (FwdPower<=Fwd)` — with the default −100/+100 kW band
  that OR spans the whole axis, so an idling reversible reg never taps. Not
  "corrected" to AND (would diverge from the oracle).

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

## L4, E2 — SeasonalRating reimplementation (global `SeasonalRatingIdx`) — SETTLED (WP-U1.5, adopt capi015 = r4133)

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

**capi015 ≠ r4133 on SINGLE-season elements (adopt capi015, the binding
oracle).** The `55400a29` `GetRatings` guard is `(idx >= 0) and (idx <
NumAmpRatings)` — it **dropped** the pre-refactor/r4133 `(RatingIdx <=
NumAmpRatings) and (NumAmpRatings > 1)` guard. So under an active signal at idx 0
a **single-season** PDElement (`NumAmpRatings = 1`, the default) takes
`AmpRatings[0]` for BOTH norm and emerg on capi015, whereas r4133 keeps the base
`(NormAmps, EmergAmps)`. Verified on the pinned capi015 oracle (0.15.0b4 / SVN
4103, newer than `55400a29`): a default single-season Line under `SeasonRating`
at idx 0 reports `%Normal == %Emergency` (both use `AmpRatings[0]`), i.e. the
no-`>1`-guard behavior. The port follows **capi015** (the goldens' oracle);
the earlier "capi015 == r4133 bit-identical" claim above holds only for the
multi-season fixtures (`Seasons=4`), which is all the goldens exercise.

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
  `AmpRatings[idx]` override + the `idx<NumAmpRatings`/`-1` guard, no `>1`), plus
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

## NCIM PV→PQ Q-limit iteration count — SETTLED (WP-U1.7 cross-check, capi015 pinned; r4088 differs, report-only)

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
  `autotrans-regcontrol-tap`, `pvsystem-kvar-display-precision`,
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

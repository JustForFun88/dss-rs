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

## L1, L3, L4 — pending later WPs

- **L1** InvControl `InvControlDeltaV` buffer — WP-U1.3.
- **L3** Monitor CSV header — WP-U1.5 (report-format, numeric-token gated).
- **L4** SeasonalRating application — WP-U1.5.

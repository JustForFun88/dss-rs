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

**Gate.** No corpus deck uses `Set Class=`/`Set Object=` (scanned — 0 hits), so
zero mandatory-gate movement; the port is gate-safe. Pinned by
`set_class_activates_and_set_object_selects` (Set Class activates the class; a
bare `Set Object` resolves against it and sets `ActiveCktElement`; `Type`/`Element`
aliases) + `set_class_unknown_errors_keeps_previous`. The observable is identical
on capi015 and 0.14.5 (the r3875 `ActiveDSSClass` refresh is unobservable in any
constructible sequence — `Set Object` bare-resolution reads `LastClassReferenced`,
set in both revs), so no oracle flip is needed. Follow-up (separate `?`-command
gap, NOT r3875): a bare `? prop` querying the ActiveCktElement is still unported
in `do_query_cmd` — noted in STATUS, an owner for a later WP.

## L1, L3, L4 — pending later WPs

- **L1** InvControl `InvControlDeltaV` buffer — WP-U1.3.
- **L3** Monitor CSV header — WP-U1.5 (report-format, numeric-token gated).
- **L4** SeasonalRating application — WP-U1.5.

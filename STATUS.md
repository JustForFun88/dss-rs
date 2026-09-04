# dss-rs — Project Status & Session Handoff

> **Purpose of this file:** a living snapshot so a fresh session can resume
> without re-deriving context. It records *what is done*, *what was decided*,
> and **why**. It is **not** authoritative for the plan itself — that is
> `PORTING_PLAN.md` (roadmap + binding decisions) and `CLAUDE.md` (conventions
> + the green-gate rule). Read those two first; then read this for the current
> frontier.

> **History archived, round 2 (2026-09-03).** This file went from **9 311** to
> **576** lines; every historical record moved **verbatim** into
> [`docs/phase-records/`](docs/phase-records/), leaving the live frontier, the
> escape register, the standing follow-ups and sections 5-7. New this round:
> `r4133-props-frontier-log.md`, `r4133-props-rp0-rp1.md`, `r4133-props-rp2.md`,
> `r4133-props-rp3.md`, `r4133-props-rp4.md`, `follow-ups-carried.md`; appended
> to `golden-rebase.md` (section 1's `### GOLDEN_REBASE` condensed blocks) and
> `phase-index.md` (round 1's own archive note). Section 7's table says what each
> holds and forwards the `STATUS §X` citations. Zero loss is proven by script,
> not asserted. **Correction carried forward:** round 1's note said "no test
> parses this file", which stopped being true at RP3.11 (2026-09-02) — but all
> **three** `props_r4133_replay.rs` pin-citation guards (RP3.10, RP3.11, RP3.13)
> now read [`r4133-props-rp3.md`](docs/phase-records/r4133-props-rp3.md)
> instead, every assertion unchanged, so the claim holds again here.

> **Round 1 (2026-08-05)** archived 16 368 -> 415 lines and created the sixteen
> `docs/phase-records/` files it lists; that note is kept verbatim at the foot of
> [`phase-index.md`](docs/phase-records/phase-index.md).

## 1. Where we are

**Era.** Post-acceptance. The 1:1 behavioral port is finished and
referee-certified (2026-07-11); `PORTING_PLAN.md` is history. The order of all
active work is `PLAN_SEQUENCE.md`; the binding conventions and the five-command
gate are `CLAUDE.md`. The 2026-08-02 user policy is in force: **EPRI r4133 is
the behavioral authority, the pinned dss_capi 0.14.5 is a numeric oracle only,
and upstream bugs are never reproduced in any lane** — the `oracle-parity` lane
has shrunk to a precision-compat lane and is scheduled for full teardown.

**In flight.** `GOLDEN_REBASE_PLAN.md` **WP-G1**, on branch **`update`** (the
R4133_PROPS branch `r4133-props` was merged and deleted 2026-09-04). **`R4133_PROPS_PLAN.md` is COMPLETE**
(2026-09-04, §RP5.2) — all six WPs gate-green in both lanes over 26 sub-steps /
**67** RP-titled commits (64 through RP5.1's `64474762`, plus RP5.2's
`5a110653`, its settlement `bc16430b` and this record), plan archived to
`docs/plans-archive/`, `PLAN_SEQUENCE.md` row 5b COMPLETE with the final
counters, record in
[`era-summaries.md`](docs/phase-records/era-summaries.md) §1a, **G1.1 handed back
satisfied**. Close-out 2026-09-04: `update` fast-forwarded to `r4133-props`
@ `2724a139` (32 commits) and pushed to `origin/update`. Since D7 (2026-09-04)
WP-G1 executes in per-chain lanes merged back into `update`, which the merge
agent regenerates `population.lock.json` on; that lock, `ledger.json` and
`golden.lock.json` stay fail-on-stale.

**Record placement (from 2026-09-03).** Every sub-step's **full** record is
appended to its per-WP file under `docs/phase-records/` — WP-RP3 →
[`r4133-props-rp3.md`](docs/phase-records/r4133-props-rp3.md), where §RP3.10's
went; a later WP → a new `r4133-props-<wp>.md` (WP-RP5 →
[`r4133-props-rp5.md`](docs/phase-records/r4133-props-rp5.md)). The pin-citation guards
in `crates/dss-core/tests/props_r4133_replay.rs` read that file, and section 7
forwards the `§RPx.y` citations to it. Section 1 gets a **3–6 line** landed
paragraph per sub-step — verdict, date, commit, pointer — and never grows a full
record again; the ritual's "update `STATUS.md`" / "read `STATUS.md` end to end"
steps therefore mean STATUS **plus** the sub-step's phase-records file.

**WP-RP0 – WP-RP2 — COMPLETE** (RP0.1–RP0.2 2026-08-22; RP1.1–RP1.4 2026-08-22
and 2026-08-23; RP2.1–RP2.4 2026-08-23). The evidence base and census rails (the vendored
G1.1 extracts, the permanent `DSS_PROPS_CENSUS` knob); the property-table shape
closure — two real ports (AutoTrans `XfmrCode`; WindGen `UserModel`/`UserData`
and the `Model=6` behavior they feed, over the sandboxed WASM host), two
`UPSTREAM_STUB` rows and one `PROPS_015X` allowlist row, taking **r4133 shape
classes 5 → 0**; and the channel-aware value comparator — the normalization
table (**168** rows at close), the echo-exclusion table (**82** rows at close)
and the measure-first `R4133_DISPLAY_FLOOR = 2e-4`, together taking in-scope
`UNCLAIMED` cells **521 841 → 889**, every one attributed to an RP3.x sub-step
(WP-RP2 was zero engine change). Full records:
[`r4133-props-rp0-rp1.md`](docs/phase-records/r4133-props-rp0-rp1.md) and
[`r4133-props-rp2.md`](docs/phase-records/r4133-props-rp2.md).

**WP-RP3 (genuine-jump closure) — COMPLETE**: all thirteen sub-steps landed and
settled, §RP3.10 last (2026-09-04, settled the same day). Nothing in the WP
blocked the unmask, and §RP3.10 — §RP5.2's last precondition — was discharged by
execution (verdict `FIX`), not by the `ORPHANED_GAPS.md` fallback. Full records:
[`r4133-props-rp3.md`](docs/phase-records/r4133-props-rp3.md) — section
"R4133_PROPS WP-RP3 — condensed records" (RP3.1–RP3.5) and section "Full
sub-step records RP3.6 – RP3.13 and RP3.10 (moved from STATUS §1)".

- **RP3.1–RP3.4** (all 2026-08-24, one commit each, zero product-crate and zero
  `ledger.json` bytes) — the four bin-7 root causes, each excluded and pinned
  with no engine change owed: `swtcontrol.delay` (r4133 never wires property 5),
  `windgen.kvar` (its getter renders the dispatched value), `generator.model`
  (the first ECHO outcome, the 82nd `PROPS_ECHO_R4133` row) and
  `gictransformer.r2` (LEDGER); their staged entries landed at RP4.1.
- **The six `FIX`-in-both-lanes port fixes** — RP3.5 `line.units` (2026-08-28,
  five `TLineObj.MergeWith` defects, the first sub-step to move product-crate
  bytes), RP3.6 `line.linecode` (2026-08-29, the `switch=yes` arm plus the
  `FLineCodeSpecified`/`CondCode` split and the CIM units back-fill), RP3.7
  per-phase switch and relay state (2026-09-02, both control classes rebuilt on
  r4133's `pStateArray` model — the widest sub-step, 50 files), RP3.8 the five
  read-only text surfaces r4133 renders live (2026-09-02; its settlement caught
  `Save` as a fifth un-refreshed `get_value` reader), RP3.10 the WindGen
  `QMode=0` dispatch (2026-09-04, `9f55095b` + `9f067c19`, `lane_diff` Δ = 0;
  its settlement caught the dispatched sign under a typed `kVA=` and opened the
  AT-1 follow-up below) and RP3.13 the two NCIM port bugs (2026-09-03,
  `2ce1a66e` + `217355da`, zero ledger entries and zero golden bytes; its
  settlement stopped reproducing a fourth r4133 defect, `CalcInjCurrAtBus`'
  PC-element sign).
- **The three recorded-but-never-reproduced sub-steps** — RP3.9's 27
  `PRECISION_ROUNDTRIP` pairs with `OPEN_RP39 = (0, 0, 0)` (2026-09-02), RP3.11's
  `KEEP_LIVE_PINNED` `Save`/`Dump` re-serialization surface (2026-09-03, whose P0
  findings opened RP3.13) and RP3.12's `autotrans.wdgcurrents` `UPSTREAM_BUG`
  (2026-09-03) — zero product-crate lines between them. Per-sub-step shas,
  verdicts and settlements: the condensed table in `r4133-props-rp5.md` §RP5.2.

**WP-RP4 (the unmask) — COMPLETE** (RP4.1, 2026-09-03, `59e521e5`, 23 files
+2 576 / −536; zero product-crate lines, zero golden bytes, zero tolerances
moved). `all_properties` is compared on the r4133 channel for every live
non-`large` case: the **83 r4133-only** non-large cases get a property check for
the first time and the **313 non-`large` `both`** cases get their r4133 property
table compared — **1 670** gating property walks over **151 782** elements per
full run, identically in both lanes. G1.1's re-armed kill criterion did **not**
fire (zero ledger entries and zero pins from RP4.1's own residual triage). Full
record: [`r4133-props-rp4.md`](docs/phase-records/r4133-props-rp4.md).

**WP-RP5 (operational docs + closing record) — COMPLETE.** **RP5.1 landed
2026-09-04** (`dd0b9e5b` + `8802fb6a`), docs only: the r4133 claim chain
(normalize → echo → floor → assert), the property-divergence triage procedure and
**46** `file.rs:LINE` citations, which its settlement turned into an executable
walk over all **58** (its two handed-forward wording items were both **refuted**
and are recorded as deliberately not edited). **RP5.2 landed 2026-09-04**
(`5a110653` + settlement `bc16430b` + this record) — the closing record and the
archive move, with the final counters published as measured: normalization
**168** / echo **82** rows, ledger **36 → 57** entries over **23 → 30** causes,
one new tolerance (the 2e-4 display floor), **4 499 / 0 / 5** per lane (4 498 at
the closing commit; the +1 is the thirteenth `oracle_parity_cfg_gate` test its
settlement added), and the gating-case outcome as the lock's **464** r4133-gating
cases (**313** non-`large` `both` compared), not the plan's stale 462. Its
settlement (14 findings — 12 fixed, 1 recorded, 1 superseded, 0 refuted) closed
both citation-guard gaps — an unanchored `file.rs:LINE` citation now **fails**
instead of being checked for existence only (58 anchored, six re-spelled), and a
thirteenth test resolves the seven `record.md:LINE` citations Rust comments carry
— and corrected two live cross-doc counts (`GOLDEN_REBASE_PLAN.md`'s stale 96
r4133-only cases → 97; 25 → 26 sub-steps) plus four self-description defects.
Full records — including the condensed table of all 26 sub-steps and the 13+
`max |Δ| = 0` `lane_diff` runs:
[`r4133-props-rp5.md`](docs/phase-records/r4133-props-rp5.md) §RP5.1 / §RP5.2.

**GOLDEN_REBASE WP-G0 (rails) + WP-G2 (bug-kernel teardown) — COMPLETE**, merged
to `update` (`6e7ee691` / `77e1799a` / `4d3fc2d7`, all pushed): G2.0, G2.1a–h,
G2.2a–d, G2.3, G2.4, G2.5 and G2.6 landed, `SPLIT_ALIAS_POPULATION` **31 → 11**,
`Escape::WholeCase` **4 → 1**, zero golden bytes over the whole WP, and none of
the six CLAUDE.md §"Known upstream bugs" reproduced in any lane. Full record:
[`golden-rebase.md`](docs/phase-records/golden-rebase.md) section "GOLDEN_REBASE
WP-G0 / WP-G2 — condensed records" (full session records precede it there).

**GOLDEN_REBASE WP-G1 (live gate to fastdss parity) — OPEN** (opened
2026-08-08; since 2026-09-04 its independent chains run in parallel **lanes** —
worktrees `.claude/worktrees/lane-*`, D7 — and `update` takes merges only, one
lane sub-step at a time). Landed: **G1.1**, killed on day one, handed to
`R4133_PROPS_PLAN.md` and delivered by its RP4.1 2026-09-03 — **G1.1
satisfied**, **G3.4**/**G3.5** unblocked; **G1.2** (the ESPVLControl deck)
2026-08-29; **G1.0**, the rails ahead of G1.3a (ten-flag vocabulary in one lock
regen, exclusion `channels`, the capture-presence guard, the r4133 bridge rails
whose 96-mode probe **also discharges G1.11's mode-capability acceptance for
the whole WP**) 2026-09-04; **G1.9** (lane `lane-s`, 2026-09-04, `9757d26c` + its audit settlement), the
five `Circuit` aggregates and ten `Solution` scalars on all 519 live cases of
both channels, unflagged and universal — no lock regen, **0 ledger entries / 0
new `LEDGER_FIELDS` / 0 golden bytes**, no new floor, all three kill criteria
not met. Full record: the same file, section "GOLDEN_REBASE WP-G1 — records".

**Next.** Lane `lane-s` takes **G1.7** (topology: `NumLoops`, `NumIsolated*`,
`AllLoopedPairs`), then G1.8 and G1.10a–c; the element lane runs G1.3a→3d→3b→3c,
the bus lane G1.4a→G1.5→G1.4c→G1.4b, the PD/meter lane G1.6b→G1.6(i)→G1.6(ii),
each merged into `update` one sub-step at a time. Then WP-G3–G5, inside which
**G3.4**/**G3.5**, blocked since 2026-08-08, are runnable. Queued behind
GOLDEN_REBASE: `WASM_USERMODELS` follow-ups, RESONANCE, MULTITHREADING, UPGRADE.

**Sequenced after / parked.** DIAKOPTICS Part II WP-AD.6 (threaded children,
needs MULTITHREADING M2); the IEEE118Bus NCIM switching-cadence rung; the
`UpgradeRung` escape rows below. Details in *Standing open follow-ups*.

### Live escape register — the 15 surviving `TODO(compat)` markers

The register itself is executable: `oracle_parity_cfg_gate.rs::ESCAPE_REGISTER`
(+ `EXIT_POPULATION`) checks it **both ways** — an unregistered marker fails, a
registered marker that no longer exists fails. Rows below are the prose index;
the site comment carries each row's measured cost.

| Owner | Site | Row |
| --- | --- | --- |
| `UpgradeRung` | `support/line_constants/mod.rs` | truncated `mu0` / `Twopi` (35/520 corpus cases) |
| `UpgradeRung` | `support/line_constants/cable.rs` | truncated `1/pi` = `0.3183` in the tape-shield resistance (8/520) |
| `UpgradeRung` | `util.rs` | Pascal `CALPHA = (-0.5, -0.866025)` (33/520 + `dump_reactor_symcomp`) |
| `UpgradeRung` | `elements/pd/reactor/solve.rs` | the same truncated `CALPHA`, second site |
| `UpgradeRung` | `support/complexutil/mod.rs` | truncated constants from `DSSUcomplex.pas` (`pi`) |
| `UpgradeRung` | `support/complexutil/mod.rs` | rad→deg `57.29577951` — "replace with `f64::atan2`" |
| `UpgradeRung` | `elements/pd/line/mod.rs` | Carson earth-return depth `658.5` (not `658.8530451057239`) |
| `UpgradeRung` | `elements/pd/line/code.rs` | the same `658.5` |
| `UpgradeRung` | `elements/pd/line/accessors.rs` | the same `658.5` |
| `UpgradeRung` | `elements/control/exp_control/accessors.rs` | `FOpenTau := Tresponse / 2.3026` (documented r4133 model constant) |
| `UpgradeRung` | `cim/ieee1547.rs` | `LPFTau * 2.3026` |
| `WholeCase` | `elements/pc/generator/user_model.rs` | the Model=6 dynamics-entry `E1` seed (`wasm_gen_dyn`) — the last of four; the GICTransformer `%R2`, Capacitor `Cuf` and LoadShape MMF rows were torn down by G2.5 |
| `WasmGuest` | `tools/wasm_usermodel/models/indmach012a/src/symcomp.rs` | truncated `sqrt(3)/2` = `0.866025403` |
| `WasmGuest` | `.../indmach012a/src/model.rs` | truncated `sqrt(3)` = `1.732` |
| `WasmGuest` | `.../indmach012a/src/model.rs` | FPC single-precision folding of `3.0/746.0` |

> **Reading note (2026-09-03).** The two legacy lists that used to follow this
> point — the carried-forward handoffs and the residual floors / parked items —
> moved **verbatim**, closed rows included, into
> [`follow-ups-carried.md`](docs/phase-records/follow-ups-carried.md), with the
> 2026-08-05 reading note that introduced them. Two of those handoffs are
> superseded and must not be re-opened from the old text: "`TODO(compat)`
> bug-for-bug sweep → Stage F" and "`HIDE_015X` → Stage F" were executed by
> DE_PASCALIZE Stage F and finished by GOLDEN_REBASE WP-G2/WP-G4 — the live
> marker population is the 15-row register above, not section 5's 2026-07-17
> count. The rows still open are listed after the standing follow-ups below.

### Standing open follow-ups (actionable)

- **The two WindGen power-flow decks keep almost no oracle-compared solved state
  — OPEN, recorded by the R4133_PROPS §RP3.10 audit settlement (AT-1,
  2026-09-04).** `modes:windgen/windgen_snap_delta.dss` and
  `modes:windgen/windgen_daily.dss` gate on `r4133` only, and RP3.10's
  `windgen-qmode0-constant-q-{snapdelta,daily}-r4133` entries exclude
  `voltages`, `injection`, `element`, `y`, `y_fingerprint` and the WindGen
  `yprim` on them — deliberately, because the port dispatches `kvarBase` where
  r4133 dispatches 0 and there is no envelope to re-assert. What still runs
  against the oracle is the iteration count (2 == 2), the forced property surface
  and the two non-WindGen YPrims, so a regression in the delta-YPrim/L-N-Vmag
  path or the daily wind-speed dispatch — the behaviours those two cases were
  written to gate — would now pass. **The fix is a sibling deck per case with
  `QMode=1`** (or `QMode=2` plus a flat `y=+1` volt-var curve),
  `engines: "r4133"`, same delta/daily paths: both engines then take the same
  arm, so node V, the RHS, the elements, Y and YPrim are compared again with no
  ledger entry. Not built inside the settlement because two new manifest cases
  are a measured population change (the lock's anti-shrink accounting, the
  523-case count in `CLAUDE.md`, `TESTING.md` and `corpus_gate/scheduler.rs`),
  i.e. its own sub-step with its own audit pair, owing the usual live measurement
  that the new decks compare clean on every channel. Full text: the §RP3.10
  record.

- **The generator's NCIM reporting arm is keyed on the *global* algorithm —
  OPEN, recorded by the R4133_PROPS §RP3.13 audit settlement (AC-3,
  2026-09-03).** `SysCtx.ncim` mirrors r4133's global `Algorithm`, not a
  per-solve flag, so `TGeneratorObj.GetCurrents`' new NCIM arm — and its
  precedence over the `LastSolutionWasDirect` shortcut — also governs a
  `direct`/`dynamics`/`harmonics` solve run while `Set algorithm=NCIM` is still
  in force: the machine then reports the last `UpdateGenQ` stamp evaluated at the
  new voltages. **Not a divergence** — the auditor measured live r4133 returning
  the identical numbers in the same sequence (`ncim_pv_pq` + `Set mode=direct;
  Solve` → `Generator.G1 78.5593 ∠117.52°`, `(-783.9 kW, -1504.8 kvar)`, KCL at
  `genbus` off by `(1216.1, -704.8)`) — and unreachable from every gated case,
  which is why RP3.13 recorded it instead of inventing an answer: unlike the
  PC-sign finding it settled, there is no measured "correct" value to move to,
  and guessing one would leave the sole live NCIM oracle on an untested surface.
  Documented at the arm (`elements/pc/generator/accessors.rs`); the candidate fix
  is a `ncim_stamped_at` marker mirroring `VSource::ncim_swing_stamped_at`.
  Whoever takes it owes a probe of what a generator *should* report in that
  sequence before the arm moves.

- **`UNIFIED_GATE_PLAN.md` still reads `Status: PLANNED` while its execution is
  recorded as finished — OPEN (found by the R4133_PROPS RP5.2 audit settlement,
  2026-09-04).** The plan sits at the repo root, `TESTING.md` cites it as the live
  gate's design record, and `docs/phase-records/unified-gate.md` records Phases 0
  and A–F plus §6 final acceptance (2026-07-19). RP5.2's settlement added it to
  the `docs/plans-archive/README.md` root-plan list, which had omitted it, and
  stopped there: flipping another plan's lifecycle banner — and deciding whether
  it archives — belongs to whoever owns it.

- **54 `kind=large*` `engines: both` cases have no property compare on EITHER
  channel — DECIDED at RP5.2 (2026-09-04): accepted permanently** (raised by the
  R4133_PROPS RP4.1 audit settlement, 2026-09-03). `scheduler::force_properties`
  keeps the plan's §1.3 cost guard (`!kind.starts_with("large")`), so of the 367
  `both` cases **313** compare their property table; the same guard also leaves
  14 r4133-only and 11 capi-only `large` decks out, but those two never had one.
  The forced population is pinned (`FORCED_PROPS_POPULATION` = (440, 313, 83,
  44), asserted by `the_property_forcing_rule_is_every_live_non_large_case`), so
  the gap is measured, bounded and locked. Pricing a `large`-deck property sweep
  stays available to GOLDEN_REBASE, owed by nothing: `r4133-props-rp5.md`
  §RP5.2.

- **`DECLARED_RP35`'s four remaining declared pairs owe a per-pair disposition —
  OPEN (R4133_PROPS RP4.1, 2026-09-03).** RP4.1 retired only the two pairs its
  unmask measured (`swtcontrol.normal`/`.state` → `RP37_SUPERSEDED`); `line.units`
  (RP3.5), `line.linecode` (RP3.6), `relay.normal` and `relay.state` keep their
  declared rows (`DECLARED_RP35 = (5, 4, 2)`). The HEAD census shows no divergent
  cell for any of the four either, but a superseded row owes a **per-pair live
  disposition, a cited r4133 getter arm and a pin**, and nobody has produced that
  trio for them. Not RP4.1 work (the same item is recorded in the §RP4.1 record).
  **RP5.2 (2026-09-04) could not close it** — the trio is test logic, outside a
  documentation-only sub-step — so it is handed on with its evidence to whoever
  closes the WP-RP3 accounting inside GOLDEN_REBASE WP-G1
  (`r4133-props-rp5.md` §RP5.2).

- **r4133 `New espvlcontrol.*` access violation — upstream-report candidate, OPEN
  (GOLDEN_REBASE G1.2, 2026-08-29).** The official EPRI r4133 DLL cannot
  instantiate class `ESPVLControl` at all (`#303`, read of `0x0`; offsets `15440`
  / `8F3F2D` — see the G1.2 record). Measured, isolated to the constructor path,
  and ledgered as `r4133-espvlcontrol-uninstantiable` so the corpus deck gates on
  `capi_v0145`. No port action: the port and the pinned 0.14.5 oracle both build
  and sample the class. What is owed is an English write-up in
  `investigations/to_opendss/` (local-only folder) — out of G1.2's scope.
- **`ESPVLControl.Forecast` — r4133 property 12 absent from the port, OPEN
  (GOLDEN_REBASE G1.2 audit settlement, 2026-08-29).** r4133 declares **12**
  class properties where the pinned dss_capi 0.14.5 declares 11:
  `.inputs/electricdss-code-r4133-trunk/Version8/Source/Controls/ESPVLControl.pas:133`
  (`NumPropsThisClass = 12`) and `:178` (`PropertyName^[12] := 'Forecast'` —
  "Loadshape object containing daily forecast"). The port mirrors capi
  (`elements/control/espvl_control/mod.rs`, `NUM_PROPS = 14` incl. the
  `TCktElementClass` tail + `Like`), so the property is **not implemented**. It is
  also **invisible to the R4133_PROPS census machinery**: the r4133 DLL cannot
  instantiate ESPVLControl at all (the follow-up above), so no census case will
  ever surface it. Whoever picks up the r4133 property line must add it by hand
  from the source; the corpus deck deliberately does not use it (it would not
  parse on 0.14.5).
- **`kind=skip` ledger entries self-hit, so an r4133 blackout can never go stale
  on its own — OPEN infrastructure limitation (surfaced by the GOLDEN_REBASE G1.2
  audit, 2026-08-29; pre-existing).** `corpus_gate/ledger.rs:272-283`
  (`channel_is_skipped`) sets `applied`/`exceeded_floor` and bumps `hits`
  unconditionally whenever the case dispatches, so the fail-on-stale discipline is
  satisfied trivially for `kind=skip` — an entry keeps reporting "1 hit" whether
  or not the upstream defect still exists. This affects all five skip entries
  today (`r4133-espvlcontrol-uninstantiable` + the four `r4133-*-303`). No
  automatic fix is possible without actually running the crashing case, so the
  procedure is manual; it is spelled out in `r4133-espvlcontrol-uninstantiable`'s
  `source` (the four older entries still lack it — adding it there is a separate,
  digest-moving edit): whenever the r4133 binary is re-vendored
  (TESTING.md §"Re-vendor the r4133 binary"),
  re-probe the skip-bearing cases with `DSS_GATE_SEED_LEDGER=1
  DSS_GATE_SEED_ONLY=<case>` and delete any entry whose cause upstream has fixed,
  so the r4133 channel re-lights instead of staying dark forever.
- **`CorpusGuard` can leak deck-written artifacts under concurrency —
  OPEN, out-of-scope observation (first seen at GOLDEN_REBASE G1.2,
  2026-08-29).** Unfiltered `cargo test --workspace` runs intermittently leave
  untracked deck-written exports inside the tracked corpus tree — nearly always
  `tests/corpus/electricdss-tst/Test/AutoTrans/` (`Auto3bus_*` / `AutoHLT_*`
  `.txt`, written by the vendored decks' own `export … file=` lines) —
  contradicting TESTING.md's "keep `tests/corpus` pristine afterwards".
  Consistent with an overlapping-guard snapshot race (cf. the unit test
  `corpus_guard_overlapping_guards_still_sweep`) plus the case-insensitive
  collision `corpus_gate/runner.rs:42-55` (the same file appears both
  `Auto3bus_HL_current.txt` and `auto3bus_hl_current.txt`). **Twelve sightings**
  2026-08-29 … 2026-09-04 (G1.2 ×2, §RP3.12 ×2, §RP4.1 ×2, §RP3.13 ×2,
  §RP3.10 ×3, §RP5.1 ×1), the set varying in size (1 … 36 files) and in case
  between successive runs of the *same* tree — the nondeterminism itself was
  measured at §RP3.13 — and once accompanied by an unreproducible `corpus_gate`
  `137 passed; 1 failed` whose most likely cause is this race. Every set was
  removed before the commit — by exact name, `git clean -fd` scoped to the deck
  tree (§RP3.10), or by hand (§RP5.1) — and both lanes were green with the files
  present, no tracked corpus or golden byte ever moving, so the leak costs
  hygiene only. Per-run detail is in the per-WP records (`r4133-props-rp3.md`,
  `-rp4.md`, `golden-rebase.md`). Twelve sightings make it a pattern, not a
  fluke: whoever picks it up should start with `DSS_GATE_JOBS=1` per G2.2d.

**Carried-forward and residual-floor items — the rows still open.** Full text,
closed rows and all:
[`follow-ups-carried.md`](docs/phase-records/follow-ups-carried.md).

- **IEEE118Bus NCIM PV→PQ switching cadence → a future UPGRADE rung** — matches
  capi015 loop-for-loop but not r4133's newer cadence; parked
  `skipped_needs_investigation`, report-only in `DIVERGENCES.md`.
- **DER user-model FILENAME render** (`UserModel`/`ShaftModel`/`DynaDLL`)
  unpinned — do it with the WASM loader's error-path gating, not by weakening
  `gen_json.py`.
- **WindGen `Spectrum` FullNames render ungated** — the class is absent from the
  pinned 0.14.5 oracle; needs an r4133-side JSON channel or a UPGRADE rung.
- **A-Diakoptics `AggregateProfiles` → DIAKOPTICS Part II WP-AD.5, partial**
  (`exec/command.rs` `NOT_PORTED`); WP-AD.6 threaded children needs M2.
- **The `like=` / `clone_ckt` user-model tail** — the WM.3/WM.4 slots keep the
  older lazy shape, unreachable in the corpus; all the RP1.3 settlement left.
- **The file-backed-loadshape ORACLE flake is not extinct** — last recurrence
  2026-08-23 on RP1.3's parity-lane gate run; next suspect is the case-directory
  guard restoring `mm8.csv` while a one-shot worker still has it mapped.
- **ckt24 RegControl/LDC `SubXFMR`** ~4.7e-5 rel tap current — an ultra-switch
  conditioning floor (CF-D); watch on re-touch.
- **UTF-8-BOM edge cases** — GAPS follow-up.

## 5. `TODO(compat)` / deferrals

Grep `rg "TODO\(compat\)"` for the full marker list (**123 sites across 71 files** as of 2026-07-17 — Phase 7/8/GAPS/UPGRADE added many; the whole set is wiped in one DE_PASCALIZE Stage F pass, not yet run). Notable:
truncated `CALPHA`/`pi`/`0.001732`/`57.29577951` constants, FPC banker's
`Round` shims, LineCode `Repair`=0 default, the `DoubleSymMatrix` zero-matrix
getter.

`NOT_PORTED` (hard parse error; every site points at its phase):
- Line `geometry` — **ported (WP7.1 step 3a)**: resolves a `LineGeometry`, runs
  `FetchGeometryCode` + `FMakeZFromGeometry` (the Carson `Zmatrix`/`YCmatrix`).
  Line `spacing`/`wires`/`cncables`/`tscables` stay `NOT_PORTED` until step 3b
  (the `FetchLineSpacing`/`SetWires`/`FMakeZFromSpacing` path, PORTING_PLAN
  §Phase 7 sub-block 1).
- Reactor `RCurve`/`LCurve` — Phase 5 (XYcurve) — XYcurve is now ported; the
  fetch is still `NOT_PORTED` (only the harmonic `CalcYPrim` consumes it, Phase 7).
- CapControl `ControlSignal` — Phase 5 (LoadShape); still `NOT_PORTED` (the
  `Follow` control type that consumes it has no corpus case — WP5.6's `Sample`
  records the Pascal abort error if reached); `UserModel`/`UserData` — never
  (no DLL loading in safe Rust).
- LoadShape `CSVFile` — **ported (WP5.2b)** via the deferred-`FileLoad` path.
  `SngFile`/`DblFile`/`PQCSVFile` (binary/2-col input) stay `NOT_PORTED` until a
  gate needs them. Single-precision arrays + `MemoryMapping` (MMF) and
  `Action=DblSave`/`SngSave` (binary output) — not ported (no corpus case).
- TempShape (`TShape`)/PriceShape `CSVFile` — **ported (WP5.2c)** via the same
  deferred-`FileLoad` path. `SngFile`/`DblFile` (binary input) and
  `Action=DblSave`/`SngSave` (binary output) stay `NOT_PORTED`.
- GrowthShape `CSVFile`/`SngFile`/`DblFile` — file-input machinery, when a
  gate needs it.

Other deferrals: Transformer GIC path (<0.51 Hz) + harmonics interplay
(the frequency-scaled Y + the <0.51 Hz branch are **exercised by WP7.6
harmonics**; the GIC *elements* stay Phase 9); RegControl/CapControl
`Sample`/`DoPendingAction` **wired into the
control loop (WP5.7)**; RegControl/ControlQueue debug-trace files (flag
stored, no file — port with Monitors, Phase 6+); `MakePosSequence` everywhere
(Phase 6+); `BusCoords` **ported (WP5.8)**; Monitors/EnergyMeters
`sample_all`/`EndOfTimeStepCleanup` are no-op hook stubs at the SolveDaily/
Yearly/Duty call sites (Phase 6); the **dynamics** (WP7.7), **harmonics/harmonicT**
(WP7.6) and **faultstudy** (WP7.9) solve modes are **ported**; the Newton algorithm
and the Monte-Carlo/load-duration/AutoAdd/`SolveGeneralTime` solve modes keep the
"Unknown solution mode" error (no corpus case — WP7.9 empirical decision);
the report verbs are **routed (Phase 8)**: `Export Counts` (WP8.1) and the
bus/node solution exports `Voltages`/`BusCoords`/`NodeNames`/`YNodeList` (WP8.2
sub-step 1) are **real**, the remaining `Export` keywords + `Save`/`Dump` record a
scoped `NOT_PORTED` (real formatters in WP8.2–8.5), `Show` is a faithful silent
no-op (real `ShowResults` in WP8.4), `Plot`/`Visualize` are headless no-ops
(§2.5); `Select`/... remaining executive verbs still record "not ported"
(Phase 8 WP8.6).

**2026-09-03 correction.** The 2026-07-17 count above is history and must not be
re-derived from it. The live population is the 15-row escape register in section
1, enforced both ways by `oracle_parity_cfg_gate.rs::ESCAPE_REGISTER` (+
`EXIT_POPULATION`); re-measure with `rg "TODO\(compat\)"`. The Stage-F and
`HIDE_015X` handoffs that sentence assumes were executed by DE_PASCALIZE Stage F
and finished by GOLDEN_REBASE WP-G2/WP-G4. Since the 2026-08-02 policy the tag
covers **precision/convention sites only** — logic bugs are fixed outright in
both lanes and never tagged.

---

## 6. How to run / regenerate

```bash
# Gate (must be green before any commit)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Run a script
cargo run -p dss-cli -- path\to\script.dss

# Regenerate goldens (MANUAL ONLY, pinned versions in tools/golden/PIN.txt)
python tools/golden/gen_props.py       # -> tests/golden/props/<class>.json   (Phase 2+)
python tools/golden/gen_slice.py       # -> tests/golden/slice.json           (Phase 3)
python tools/golden/gen_phase4.py      # -> tests/golden/phase4/*.dss + phase4.json
python tools/golden/gen_phase5.py      # -> tests/golden/phase5/<scenario>.json
python tools/golden/gen_phase6.py      # -> tests/golden/phase6/<scenario>.json
python tools/golden/gen_checkpoints.py # -> tests/golden/checkpoints/<scenario>.json
```
Oracle pin: Python 3.12.4, dss-python 0.15.7, dss-python-backend 0.14.5
(the same dss_capi release vendored in `.inputs/dss_capi`). `python` works in
this environment; the `py` launcher is broken — use `python` directly.

**Official-EPRI-OpenDSS oracle (opt-in, `tools/opendss/` — 2026-07-07):**
vendored EPRI `OpenDSSDirect.dll` r3723 (9.8.0.1) / r4088 (10.2.0.1) / r4133
(11.0.0.1) driven through the AltDSS Oddie bridge (dss-python 0.16.0b2 in a
separate venv, `PIN_OPENDSS.txt`), reusing `oracle_server.py` unchanged via
`DSS_ORACLE_ENGINE=oddie`. For inventorying upstream changes ahead of porting
them; the mandatory gate is untouched. Workflows (see `tools/opendss/README.md`):
`DSS_LIVE_OPENDSS=<rev> cargo test ... corpus_live_opendss` → report
`tmp/opendss_report_<rev>.json`, now partitioned against the triage catalog
`tools/opendss/known_diffs.json` (2026-07-07, modeled on DSS-Python
`KNOWN_COM_DIFF`; substring match on case label + first-failure reason, every
entry states its cause, zero-hit entries warn). r3723: 150 matched / 82
known-diverged / **0 new** of 232 — all 82 triaged into 11 classes (19 EPRI
InvControl max-iter failures, 25 InvControl fixpoint drift, 10 iteration
deltas, 8 monitor-header whitespace, 4 property-format brackets, 4
injection FPC-vs-Delphi ulp, 3 storage kWhStored drift, 3 meter ZonePCE
count, 3 event-log trailing space, 2 GenDispatcher prop-name, 1 harmonics
Y-fingerprint) — so `DSS_LIVE_OPENDSS_ASSERT=1` (fails only on NEW) is green
for r3723. Caveat: comparison stops at a case's first divergence — a known
first divergence masks later ones in that case (accepted for inventory).
`ab_compare.py --a oddie:r3723 --b oddie:r4133` → upstream-change inventory
(baseline: 109/168 match; deltas in distance relays, harmonics, InvControl
iteration behavior); `--known-diffs tools/opendss/known_diffs.json` relabels
fully-triaged cases `known_diverged` (entries carry `ab_contains` where this
tool's issue wording differs) and exits 0 when only known diffs remain.
Two operational gotchas, both handled: (1) EPRI's Delphi `FireOffEditor`
ShellExecutes the editor on every `Show`/`Export` with NO `NoFormsAllowed`
check and Oddie can't set `AllowEditor` — a corpus sweep opened hundreds of
Notepads; `make_engine()` now issues `Set RegistryUpdate=No` + `Set
Editor=rundll32.exe` (silent no-op; registry write suppressed so the user's
OpenDSS editor setting is untouched) — verified on all 3 revisions with a
`Show` deck, zero spawns. (2) `.inputs/electricdss-tst` is now a re-checkout
with different EOLs: `tools/corpus/vendor.py --force` produces a ~1544-file
EOL-only diff — clean run pollution with `git restore tests/corpus` instead;
re-vendor only deliberately.

**DSS-Python validation harness, vendored (`tools/opendss/dsspy_validation/`
— 2026-07-07):** copy of DSS-Python `fastdss` `tests/`
`_settings`/`save_outputs`/`compare_outputs` (BSD-3, attribution headers,
local edits marked `# dss-rs:`): full-API-state dumps (~40 collections/case,
206 upstream-curated cases, all present in our corpus) zipped per engine +
offline tolerant diff (their `KNOWN_COM_DIFF` catalog kept as upstream) —
broad-surface upstream inventory complementing `ab_compare.py`. Adaptations:
corpus → vendored copy, engine spec `DSS_EXTENSIONS_TEST_ODDIE=oddie:<rev>`
via `revisions.json` (+ expect_version hard check), COM branch dropped, our
`RegistryUpdate=No`+`Editor=rundll32.exe` suppression, per-case `CorpusGuard`
(lifted move-only into `tools/oracle/corpus_guard.py`, shared with
oracle_server), results → `tmp/dsspy_validation/`, and `(Oddie)`-prefixed
DSSException skips for API exports absent from older official DLLs (r3723
lacks `Transformers_Get_LossesByType`, `StoragesI`, ...). **Its `capi` side
is dss_capi 0.15.0b4 — NOT the pinned 0.14.5 oracle; inventory only, never
feeds goldens/gate.** pandas+xmldiff pinned into the Oddie venv
(`PIN_OPENDSS.txt`). Sweeps must end with `git status tests/corpus` (guard is
non-recursive; a sweep-created *subdirectory* — 123Bus `Run_YearlySim` makes
`16Nov2011/` — escapes it: `git clean -fd` that path). Full-sweep baseline
2026-07-07: capi 199/206 captured, oddie:r3723 189/206 (its 19 misses = the
`epri-invcontrol-maxiter` #485 class, 1:1 with known_diffs), compare
processes 3885 zip entries. The two beta packages are vendored as wheels in
`tools/opendss/wheels/` (+SHA256SUMS; offline `--find-links` install proven)
— setup no longer depends on the pre-releases staying on PyPI.

**DSS-Python corpus cross-check (`tools/corpus/dsspy_crosscheck.py` —
2026-07-07):** diffs DSS-Python's own 206-case validation list
(`.inputs/DSS-Python/tests/_settings.py::test_filenames`, extracted textually
— importing that module binds a DSS engine) against our six classifier
manifests → `tmp/dsspy_crosscheck.{json,md}`. Measured split: 133
solvable_now / 59 skipped_unsupported / 8 needs_investigation / 5
oracle_issue / 1 not_an_entry_point = **73 promotion candidates** (35
unblock at WP8.6 BatchEdit alone); all 206 exist in the vendored corpus.
`L!`-prefixed cases (55) are run line-by-line upstream with interactive
commands filtered — recorded per case so promotion work doesn't naively
`Compile` them.

---


## 7. Archived history — index of `docs/phase-records/`

Frozen history, superseded only by the code and tests. Every file is verbatim
STATUS.md text; nothing here is a summary.

**Forwarding rule for `STATUS §X` citations.** Code comments, plans and manifests
across the repo cite this file by section (~70 such references outside
`docs/phase-records/` at archiving time). A citation is history — it names the
section it was written against, and the call sites were deliberately **not**
rewritten. Resolve it in whichever file below holds §X; `rg '§X'
docs/phase-records/` finds it. The moved anchors most often cited: §1a →
`era-summaries.md`; §1b–1f and §2 → `phase-index.md`; §3/§3.4 and §4 →
`design-decisions.md`; §OG-1.x → `orphaned-gaps.md`; §7 (**caution** — the old
"Phase 7 — inherited deferrals" section, unrelated to *this* §7) →
`phase-7.md`; the per-WP records (§WP7.5, §WP8.3, §WP-AD.*, §WM.*, §W3.*,
§R2/R3, …) → the per-plan file named in the table.

**2026-09-03 (round 2).** Section 1's chronological frontier narrative and the
`R4133_PROPS` / `GOLDEN_REBASE` per-WP record blocks moved out of this file.
Resolve **section 1's WP-RP narrative** → `r4133-props-frontier-log.md`; the
**per-sub-step records §RPx.y** → `r4133-props-rpN.md` — `§WP-RP0`, `§RP0.x`,
`§WP-RP1`, `§RP1.x` → `r4133-props-rp0-rp1.md`; `§WP-RP2`, `§RP2.x` →
`r4133-props-rp2.md`; `§WP-RP3`, `§RP3.1` … `§RP3.13` → `r4133-props-rp3.md`;
`§WP-RP4`, `§RP4.1` → `r4133-props-rp4.md`; `§WP-RP5`, `§RP5.x` →
`r4133-props-rp5.md` (created 2026-09-04); the **GOLDEN_REBASE condensed
records** (`§WP-G0`, `§WP-G1`, `§WP-G2`) → `golden-rebase.md`; the
carried-forward and residual-floor follow-ups → `follow-ups-carried.md`.
`rg '§RP3.7' docs/phase-records/` finds any of them. Measured at this round:
**118** `STATUS §X` citations across **37** files outside `docs/phase-records/`
and this file; none was rewritten.

| File | What it holds |
| --- | --- |
| `r4133-props-frontier-log.md` | section 1's frontier narrative RP0 → RP3.5, the landed-recap / next / parked paragraphs, and round 1's closing archive note |
| `r4133-props-rp0-rp1.md` | R4133_PROPS WP-RP0 (RP0.1, RP0.2) and WP-RP1 (RP1.1–RP1.4) condensed records |
| `r4133-props-rp2.md` | R4133_PROPS WP-RP2 (RP2.1–RP2.4 and the RP2.4 audit settlement) |
| `r4133-props-rp3.md` | R4133_PROPS WP-RP3 — the RP3.1–RP3.5 condensed records **and** the full RP3.6–RP3.13 records with their audit settlements; read by the three `props_r4133_replay.rs` pin-citation guards (RP3.10, RP3.11, RP3.13) |
| `r4133-props-rp4.md` | R4133_PROPS WP-RP4 — the RP4.1 `all_properties` unmask record and its audit settlement |
| `r4133-props-rp5.md` | R4133_PROPS WP-RP5 — the RP5.1 operational-docs record and its audit settlement, and §RP5.2, the plan's closing record (2026-09-04): final counters, the `lane_diff` table, the condensed per-sub-step record of all 26 sub-steps, the RP5.2 audit settlement and its settlement record |
| `follow-ups-carried.md` | the carried-forward handoffs and the residual-floor / parked items, open and closed rows alike |
| `golden-rebase.md` (appended 2026-09-03) | section 1's `### GOLDEN_REBASE` WP-G0 / WP-G2 and WP-G1 condensed record blocks |
| `phase-index.md` (appended 2026-09-03) | round 1's own 2026-08-05 archive note, listing the files that round created |
| `golden-rebase.md` | GOLDEN_REBASE WP-G0 (G0.1/G0.2) and WP-G2 (G2.0, G2.1a–h) full records — the source of §1's condensations |
| `depascalize-stagef.md` | DE_PASCALIZE Stage F: the `oracle-parity` lane split, F.1 → F.5 and the wave-4 settlement (incl. the escape-register steps F.3, F.3aa–F.3ag) |
| `depascalize-w3.md` | DE_PASCALIZE wave 3 (W3.1–W3.5), the wave-2a settler, R3iv, P3 |
| `depascalize-r2b-r3.md` | R2b + R3: the `ElemRef → ElemId` store flip, typed accessors, the `Any`/`as_ckt_element` removal |
| `depascalize-r1-r2.md` | R1 typed arenas (`Idx<T>`/`ElemId`/`Elements`) and the R2 injection seam |
| `depascalize-p1.md` | P1 + the whole P1-tail series (1/n–5/n) and its escape record (all three rows closed by W3.1–W3.3) |
| `depascalize-p-series.md` | P1b, P5a/b/c (miette diagnostics), P8, P9, P10, P11, P12, P13, P14, P15 |
| `depascalize-p2.md`, `-p6.md`, `-p12.md`, `-p13.md`, `-r0.md` | the earlier standalone DE_PASCALIZE records |
| `unified-gate.md` | UNIFIED_GATE Phase 0 and A–F, the pre-E/F cross-phase audit and the post-audit fix round |
| `epri-bridge.md` | the `dss-epri` bridge parity round and capability round (Oddie retirement) |
| `wasm-usermodels.md` | WASM_USERMODELS WM.0–WM.7, the ABI re-freeze to r4133, the D2 sub-bug trace and its fix |
| `orphaned-gaps.md` | OG-1.1 … OG-1.10 (GICMvars, AltDSS JSON tails, `CAPI_Schema` walks, UPFC modes, NCIM cadence, `Export Estimation`) |
| `bug-wps.md` | the standalone BUG work packages: livectx and DynExp |
| `adopt-015x.md` | the 0.15.x adoption sweep fix round and its settle |
| `upgrade-rung1.md` | UPGRADE Rung 1/2 records + the NCIM oracle-of-record re-gate |
| `corpus-rounds.md` | the corpus classification/coverage rounds |
| `part2-adiakoptics.md` | DIAKOPTICS/PSTCALC Part II (AD.1–AD.5) |
| `gaps.md` | the GAPS plan (WPG.1–WPG.21) |
| `final-acceptance.md` | the 1:1 final-acceptance round (§6) and the referee certification |
| `era-summaries.md` | §1a archived completed-plan records — including the **R4133_PROPS** archived-plan record appended 2026-09-04 by its RP5.2 — plus the condensed era summaries |
| `gate-history.md` | the historical gate-state snapshots (pre-`corpus_gate.rs`) |
| `design-decisions.md` | §3 key design decisions & rationale + §4 empirical oracle facts |
| `phase-index.md` | the old §1b–1f/§2 index of the completed phases |
| `phase-3.md` … `phase-8.md`, `phase-7-wp1..7.md` | the per-phase and per-work-package logs |
| `test-triage-*.md` | the test-triage rounds (AD classify, IndMach, infra audit, monitor windings, promotions) |

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

**In flight.** `GOLDEN_REBASE_PLAN.md` **WP-G1** on **`update`**, its sub-step chains
running since 2026-09-04 in parallel `lane-*` worktree branches that a merge agent lands
one at a time, regenerating `population.lock.json` on the merged tree and unioning
`ledger.json` (**D7**); that lock, `ledger.json` and `golden.lock.json` stay
fail-on-stale. **`R4133_PROPS_PLAN.md` is COMPLETE** (2026-09-04, §RP5.2) — all six WPs
gate-green in both lanes over 26 sub-steps / **67** RP-titled commits (64 through RP5.1's
`64474762`, plus RP5.2's `5a110653`, its settlement `bc16430b` and this record), plan
archived to `docs/plans-archive/`, `PLAN_SEQUENCE.md` row 5b COMPLETE with the final
counters, record in [`era-summaries.md`](docs/phase-records/era-summaries.md) §1a,
**G1.1 handed back satisfied**. Close-out 2026-09-04: `update` fast-forwarded to
`r4133-props` @ `2724a139` (32 commits, the branch then deleted) and pushed to
`origin/update`.

**Record placement (from 2026-09-03).** Every sub-step's **full** record is appended to
its per-WP file under `docs/phase-records/` — R4133_PROPS → `r4133-props-rp<N>.md` (the
pin-citation guards in `crates/dss-core/tests/props_r4133_replay.rs` read `-rp3.md`;
section 7 forwards the `§RPx.y` citations), GOLDEN_REBASE →
[`golden-rebase.md`](docs/phase-records/golden-rebase.md). Section 1 gets a **3–6 line**
landed paragraph per sub-step — verdict, date, commits, pointer — and never grows a full
record again; the ritual's "update `STATUS.md`" / "read `STATUS.md` end to end" steps mean
STATUS **plus** that records file.

**WP-RP0 – WP-RP2 — COMPLETE** (RP0.1–RP0.2 2026-08-22; RP1.1–RP1.4 2026-08-22 and
2026-08-23; RP2.1–RP2.4 2026-08-23). The evidence base and census rails (the vendored G1.1
extracts, the permanent `DSS_PROPS_CENSUS` knob); the property-table shape closure — two
real ports (AutoTrans `XfmrCode`; WindGen `UserModel`/`UserData` and the `Model=6` behavior
they feed, over the sandboxed WASM host), two `UPSTREAM_STUB` rows and one `PROPS_015X`
allowlist row, taking **r4133 shape classes 5 → 0**; and the channel-aware value comparator
— normalization **168** rows, echo-exclusion **82**, the measure-first `R4133_DISPLAY_FLOOR
= 2e-4` — together taking in-scope `UNCLAIMED` cells **521 841 → 889**, every one attributed
to an RP3.x sub-step (WP-RP2 was zero engine change). Full records:
[`r4133-props-rp0-rp1.md`](docs/phase-records/r4133-props-rp0-rp1.md) and
[`r4133-props-rp2.md`](docs/phase-records/r4133-props-rp2.md).

**WP-RP3 (genuine-jump closure) — COMPLETE**: all thirteen sub-steps landed and settled,
§RP3.10 last (2026-09-04, settled the same day). Nothing in the WP blocked the unmask, and
§RP3.10 — §RP5.2's last precondition — was discharged by execution (verdict `FIX`), not by
the `ORPHANED_GAPS.md` fallback. Full records:
[`r4133-props-rp3.md`](docs/phase-records/r4133-props-rp3.md), sections "R4133_PROPS WP-RP3
— condensed records" (RP3.1–RP3.5) and "Full sub-step records RP3.6 – RP3.13 and RP3.10
(moved from STATUS §1)".

- **RP3.1–RP3.4** (all 2026-08-24, one commit each, zero product-crate and zero
  `ledger.json` bytes) — the four bin-7 root causes (`swtcontrol.delay`, `windgen.kvar`,
  `generator.model` — the first ECHO outcome — and `gictransformer.r2`, LEDGER), each
  excluded and pinned with no engine change owed; their staged entries landed at RP4.1.
- **The six `FIX`-in-both-lanes port fixes** — RP3.5 `line.units` (2026-08-28, five
  `TLineObj.MergeWith` defects, the first sub-step to move product-crate bytes), RP3.6
  `line.linecode` (2026-08-29, the `switch=yes` arm, the `FLineCodeSpecified`/`CondCode`
  split, the CIM units back-fill), RP3.7 per-phase switch and relay state (2026-09-02, both
  classes rebuilt on r4133's `pStateArray` model — 50 files), RP3.8 the five read-only text
  surfaces r4133 renders live (2026-09-02; its settlement caught `Save` as a fifth
  un-refreshed `get_value` reader), RP3.10 the WindGen `QMode=0` dispatch (2026-09-04,
  `9f55095b` + `9f067c19`; its settlement opened the WindGen follow-up below) and RP3.13 the
  two NCIM port bugs (2026-09-03, `2ce1a66e` + `217355da`; its settlement stopped
  reproducing a fourth r4133 defect, `CalcInjCurrAtBus`' PC-element sign).
- **The three recorded-but-never-reproduced sub-steps** — RP3.9's 27 `PRECISION_ROUNDTRIP`
  pairs (`OPEN_RP39 = (0, 0, 0)`), RP3.11's `KEEP_LIVE_PINNED` `Save`/`Dump` surface (whose
  P0 findings opened RP3.13) and RP3.12's `autotrans.wdgcurrents` `UPSTREAM_BUG` — zero
  product-crate lines between them. Per-sub-step shas, verdicts and settlements:
  `r4133-props-rp5.md` §RP5.2.

**WP-RP4 (the unmask) — COMPLETE** (RP4.1, 2026-09-03, `59e521e5`, 23 files +2 576 / −536;
zero product-crate lines, zero golden bytes, zero tolerances moved). `all_properties` is
compared on the r4133 channel for every live non-`large` case: at RP4.1 the **83
r4133-only** non-large cases got a first property check and the **313 non-`large` `both`**
cases their r4133 property table — **1 670** gating walks over **151 782** elements per run
(87/311 and 1 672 walks / 151 798 elements since G1.4a's flip and G1.6(i)'s deck), both
lanes. G1.1's re-armed kill criterion did **not** fire (zero ledger entries and zero pins
from RP4.1's residual triage). Full record:
[`r4133-props-rp4.md`](docs/phase-records/r4133-props-rp4.md).

**WP-RP5 (operational docs + closing record) — COMPLETE.** **RP5.1** (2026-09-04, `dd0b9e5b`
+ `8802fb6a`), docs only: the r4133 claim chain (normalize → echo → floor → assert), the
property-divergence triage procedure and **46** `file.rs:LINE` citations, which its
settlement turned into an executable walk over all **58**. **RP5.2** (2026-09-04, `5a110653`
+ settlement `bc16430b`) — the closing record and the archive move, final counters as
measured: normalization **168** / echo **82** rows, ledger **36 → 57** entries over **23 →
30** causes, one new tolerance (the 2e-4 display floor), **4 499 / 0 / 5** per lane, and the
lock's **464** r4133-gating cases (**313** non-`large` `both` compared), not the plan's
stale 462. Its settlement (14 findings — 12 fixed, 1 recorded, 1 superseded, 0 refuted)
closed both citation-guard gaps — an unanchored `file.rs:LINE` citation now **fails**
instead of being checked for existence only, and a thirteenth test resolves the seven
`record.md:LINE` citations Rust comments carry — and corrected two live cross-doc counts
plus four self-description defects. Full records, including the condensed table of all 26
sub-steps and the 13+ `max |Δ| = 0` `lane_diff` runs:
[`r4133-props-rp5.md`](docs/phase-records/r4133-props-rp5.md) §RP5.1 / §RP5.2.

**GOLDEN_REBASE WP-G0 (rails) + WP-G2 (bug-kernel teardown) — COMPLETE**, merged to `update`
(`6e7ee691` / `77e1799a` / `4d3fc2d7`, all pushed): G2.0, G2.1a–h, G2.2a–d, G2.3, G2.4, G2.5
and G2.6 landed, `SPLIT_ALIAS_POPULATION` **31 → 11**, `Escape::WholeCase` **4 → 1**, zero
golden bytes over the whole WP, and none of the six CLAUDE.md §"Known upstream bugs"
reproduced in any lane. Full record:
[`golden-rebase.md`](docs/phase-records/golden-rebase.md) section "GOLDEN_REBASE WP-G0 /
WP-G2 — condensed records" (full session records precede it there).

**GOLDEN_REBASE WP-G1 (live gate to fastdss parity) — OPEN** (opened 2026-08-08; since
2026-09-04 its chains run in parallel **lanes**, D7, merged into `update` one sub-step at a
time). Landed: **G1.1**, killed on day one and delivered by `R4133_PROPS_PLAN.md` RP4.1
2026-09-03 (**G3.4**/**G3.5** unblocked); **G1.2** (the ESPVLControl deck) 2026-08-29; and,
2026-09-04/05, the rails plus six surfaces — **G1.0** (`c4b67a6e` + `42454b64`), the flag
vocabulary, exclusion `channels`, the capture guard and the r4133 bridge whose mode probe
**discharges G1.11**; **G1.9** (`lane-s`, `9757d26c` + `f27f9598`), `Circuit` aggregates +
`Solution` scalars; **G1.6b** (`lane-m`, `06808a6d` + `e1e18367` + `c6a3c0a8`), the
`PDElements` walk + the **D9** engine fix; **G1.3a** (`lane-e`, `d8e71991` + `588e0bfe` +
`9f9c723d`), `Enabled` + the three polar channels (**1** new entry, `DIVERGENCES.md` L8, +
**13** widenings); **G1.4a** (`lane-b`, `6b0dbd32` + `be01e413` + `10417d99`), the bus
surface's divergence-free half (**0** new; **D8** defers the rest to G1.4c), with **D11(1)**
`float_roundtrip`, **D12**/**D14** the four `GICTransformer` decks onto `r4133` (ledger −4,
corpus 523 → 524) and **D13** the worker's registry leak; **G1.3d(i)** (`lane-e`, `e4d99806`
+ `b7d7da2a` + `c9c4ac09`), the per-element counts, `NodeOrder` and `EnergyMeter` on both
channels, compared exactly (**0** new, no floor; **D19′** folded the duplicate D9 pick away
at the merge); and **G1.6(i)** (2026-09-05, `lane-m`, `e343d9e8` + `96d7540a` + `bcc835b6`),
meter extras and **the run protocol** — the gate drives the executive `RelCalc` once per
case and six flagged cases compare the indices, every section, `CalcCurrent`/`AllocFactors`,
`Meters.Totals` and the **ordered** zone lists, exact but for three cells banded from
existing tiers (**D17a**), **0** new entries, one pinned skip closed on the corpus's only
`AllocateLoads` deck (524 → **525** cases), G1.6b's two deferrals discharged (**D11/D18**);
and **G1.6(ii)** (2026-09-05, `lane-m`, `3e65ae2d` + `572954e6` + settlement), the eight per-bus
reliability columns inside that same payload — 6 cases / 50 capi + 84 r4133 buses, compared
exactly, keys `bus:<bus>:<field>` on the existing `reliability` field, **0** new entries, plus
the **D20/D22** engine fix (`calc_reliability_indices` recomputes `TotalUpDownstreamCustomers`,
r4133 `EnergyMeter.pas:2466-2468`) whose only footprint is a measured capi divergence under
`RelCalc <restore>`, unreachable on the corpus (`DIVERGENCES.md` §D22).
No golden byte moved, |Δ| = 0 throughout; `WP_G1_MODES` **111**, ledger **54** / 31 causes;
G1.3d(ii), G1.3b/c, G1.4b/c, G1.5, G1.7, G1.8, G1.10–G1.11c and WP-G3–G5 remain.

**Next.** Element lane **G1.3d(ii)** → G1.3b → G1.3c; `lane-s` **G1.7** → G1.8 →
G1.10a–c; bus lane **G1.5** → G1.4c (D8) → G1.4b, then WP-G3–G5; the PD/meter lane's chain is
complete with G1.6(ii); queued: `WASM_USERMODELS`, RESONANCE, MULTITHREADING, UPGRADE.

**Sequenced after / parked.** DIAKOPTICS Part II WP-AD.6 (threaded children, needs
MULTITHREADING M2); the IEEE118Bus NCIM switching-cadence rung; the `UpgradeRung` escape
rows below. Details in *Standing open follow-ups*.

### Live escape register — the 15 surviving `TODO(compat)` markers

The register itself is executable: `oracle_parity_cfg_gate.rs::ESCAPE_REGISTER` (+
`EXIT_POPULATION`) checks it **both ways** — an unregistered marker fails, a registered
marker that no longer exists fails. Rows below are the prose index; the site comment carries
each row's measured cost.

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

- **The two WindGen power-flow decks keep almost no oracle-compared solved state — OPEN,
  recorded by the R4133_PROPS §RP3.10 audit settlement (AT-1, 2026-09-04).**
  `modes:windgen/windgen_snap_delta.dss` and `modes:windgen/windgen_daily.dss` gate on
  `r4133` only, and RP3.10's `windgen-qmode0-constant-q-{snapdelta,daily}-r4133` entries
  exclude `voltages`, `injection`, `element`, `y`, `y_fingerprint` and the WindGen `yprim`
  on them — deliberately, because the port dispatches `kvarBase` where r4133 dispatches 0
  and there is no envelope to re-assert. What still runs against the oracle is the iteration
  count (2 == 2), the forced property surface and the two non-WindGen YPrims, so a
  regression in the delta-YPrim/L-N-Vmag path or the daily wind-speed dispatch — the
  behaviours those two cases were written to gate — would now pass. **The fix is a sibling
  deck per case with `QMode=1`** (or `QMode=2` plus a flat `y=+1` volt-var curve), `engines:
  "r4133"`, same delta/daily paths: both engines then take the same arm, so node V, the RHS,
  the elements, Y and YPrim are compared again with no ledger entry. Not built inside the
  settlement because two new manifest cases are a measured population change (the lock's
  anti-shrink accounting, the 523-case count in `CLAUDE.md`, `TESTING.md` and
  `corpus_gate/scheduler.rs`), i.e. its own sub-step with its own audit pair, owing the
  usual live measurement that the new decks compare clean on every channel. Full text: the
  §RP3.10 record.

- **The generator's NCIM reporting arm is keyed on the *global* algorithm — OPEN, recorded
  by the R4133_PROPS §RP3.13 audit settlement (AC-3, 2026-09-03).** `SysCtx.ncim` mirrors
  r4133's global `Algorithm`, not a per-solve flag, so `TGeneratorObj.GetCurrents`' new NCIM
  arm — and its precedence over the `LastSolutionWasDirect` shortcut — also governs a
  `direct`/`dynamics`/`harmonics` solve run while `Set algorithm=NCIM` is still in force:
  the machine then reports the last `UpdateGenQ` stamp evaluated at the new voltages. **Not
  a divergence** — the auditor measured live r4133 returning the identical numbers in the
  same sequence (`ncim_pv_pq` + `Set mode=direct; Solve` → `Generator.G1 78.5593 ∠117.52°`,
  `(-783.9 kW, -1504.8 kvar)`, KCL at `genbus` off by `(1216.1, -704.8)`) — and unreachable
  from every gated case, which is why RP3.13 recorded it instead of inventing an answer:
  unlike the PC-sign finding it settled, there is no measured "correct" value to move to,
  and guessing one would leave the sole live NCIM oracle on an untested surface. Documented
  at the arm (`elements/pc/generator/accessors.rs`); the candidate fix is a
  `ncim_stamped_at` marker mirroring `VSource::ncim_swing_stamped_at`. Whoever takes it owes
  a probe of what a generator *should* report in that sequence before the arm moves.

- **`UNIFIED_GATE_PLAN.md` still reads `Status: PLANNED` while its execution is recorded as
  finished — OPEN (found by the R4133_PROPS RP5.2 audit settlement, 2026-09-04).** The plan
  sits at the repo root, `TESTING.md` cites it as the live gate's design record, and
  `docs/phase-records/unified-gate.md` records Phases 0 and A–F plus §6 final acceptance
  (2026-07-19). RP5.2's settlement added it to the `docs/plans-archive/README.md` root-plan
  list, which had omitted it, and stopped there: flipping another plan's lifecycle banner —
  and deciding whether it archives — belongs to whoever owns it.

- **54 `kind=large*` `engines: both` cases have no property compare on EITHER channel —
  DECIDED at RP5.2 (2026-09-04): accepted permanently** (raised by the R4133_PROPS RP4.1
  audit settlement, 2026-09-03). `scheduler::force_properties` keeps the plan's §1.3 cost
  guard (`!kind.starts_with("large")`), so of the 365 `both` cases **311** compare their
  property table; the same guard also leaves 14 r4133-only and 11 capi-only `large` decks
  out, but those two never had one. The forced population is pinned
  (`FORCED_PROPS_POPULATION` = (442, 311, 87, 44) since G1.4a's D12/D14 flip and G1.6(i)'s
  deck — the gap itself unchanged at 54 — asserted by
  `the_property_forcing_rule_is_every_live_non_large_case`), so it is measured, bounded and
  locked. Pricing a `large`-deck property sweep stays available to GOLDEN_REBASE, owed by
  nothing: `r4133-props-rp5.md` §RP5.2.

- **`DECLARED_RP35`'s four remaining declared pairs owe a per-pair disposition — OPEN
  (R4133_PROPS RP4.1, 2026-09-03).** RP4.1 retired only the two pairs its unmask measured
  (`swtcontrol.normal`/`.state` → `RP37_SUPERSEDED`); `line.units` (RP3.5), `line.linecode`
  (RP3.6), `relay.normal` and `relay.state` keep their declared rows (`DECLARED_RP35 = (5,
  4, 2)`). The HEAD census shows no divergent cell for any of the four either, but a
  superseded row owes a **per-pair live disposition, a cited r4133 getter arm and a pin**,
  and nobody has produced that trio for them. Not RP4.1 work (the same item is recorded in
  the §RP4.1 record). **RP5.2 (2026-09-04) could not close it** — the trio is test logic,
  outside a documentation-only sub-step — so it is handed on with its evidence to whoever
  closes the WP-RP3 accounting inside GOLDEN_REBASE WP-G1 (`r4133-props-rp5.md` §RP5.2).

- **r4133 `New espvlcontrol.*` access violation — upstream-report candidate, OPEN
  (GOLDEN_REBASE G1.2, 2026-08-29).** The official EPRI r4133 DLL cannot instantiate class
  `ESPVLControl` at all (`#303`, read of `0x0`; offsets `15440` / `8F3F2D` — see the G1.2
  record). Measured, isolated to the constructor path, and ledgered as
  `r4133-espvlcontrol-uninstantiable` so the corpus deck gates on `capi_v0145`. No port
  action: the port and the pinned 0.14.5 oracle both build and sample the class. What is
  owed is an English write-up in `investigations/to_opendss/` (local-only folder) — out of
  G1.2's scope.
- **`ESPVLControl.Forecast` — r4133 property 12 absent from the port, OPEN (GOLDEN_REBASE
  G1.2 audit settlement, 2026-08-29).** r4133 declares **12** class properties where the
  pinned dss_capi 0.14.5 declares 11:
  `.inputs/electricdss-code-r4133-trunk/Version8/Source/Controls/ESPVLControl.pas:133`
  (`NumPropsThisClass = 12`) and `:178` (`PropertyName^[12] := 'Forecast'` — "Loadshape
  object containing daily forecast"). The port mirrors capi
  (`elements/control/espvl_control/mod.rs`, `NUM_PROPS = 14` incl. the `TCktElementClass`
  tail + `Like`), so the property is **not implemented**. It is also **invisible to the
  R4133_PROPS census machinery**: the r4133 DLL cannot instantiate ESPVLControl at all (the
  follow-up above), so no census case will ever surface it. Whoever picks up the r4133
  property line must add it by hand from the source; the corpus deck deliberately does not
  use it (it would not parse on 0.14.5).
- **`kind=skip` ledger entries self-hit, so an r4133 blackout can never go stale on its own
  — OPEN infrastructure limitation (surfaced by the GOLDEN_REBASE G1.2 audit, 2026-08-29;
  pre-existing).** `corpus_gate/ledger.rs:272-283` (`channel_is_skipped`) sets
  `applied`/`exceeded_floor` and bumps `hits` unconditionally whenever the case dispatches,
  so the fail-on-stale discipline is satisfied trivially for `kind=skip` — an entry keeps
  reporting "1 hit" whether or not the upstream defect still exists. This affects all five
  skip entries today (`r4133-espvlcontrol-uninstantiable` + the four `r4133-*-303`). No
  automatic fix is possible without actually running the crashing case, so the procedure is
  manual; it is spelled out in `r4133-espvlcontrol-uninstantiable`'s `source` (the four
  older entries still lack it — adding it there is a separate, digest-moving edit): whenever
  the r4133 binary is re-vendored (TESTING.md §"Re-vendor the r4133 binary"), re-probe the
  skip-bearing cases with `DSS_GATE_SEED_LEDGER=1 DSS_GATE_SEED_ONLY=<case>` and delete any
  entry whose cause upstream has fixed, so the r4133 channel re-lights instead of staying
  dark forever.
- **`CorpusGuard` leaks deck-written artifacts under concurrency — mechanism MEASURED at
  GOLDEN_REBASE G1.6b (2026-09-04, audit settlement T5); still OPEN, owed a hygiene
  sub-step** (first seen at G1.2, 2026-08-29). Unfiltered `cargo test --workspace` runs
  intermittently leave untracked deck-written exports in the tracked corpus tree — nearly
  always `tests/corpus/electricdss-tst/Test/AutoTrans/` (`Auto3bus_*` / `AutoHLT_*` `.txt`,
  from the decks' own `export … file=` lines). **Nineteen sightings** 2026-08-29 …
  2026-09-05 (G1.2 ×2, §RP3.12 ×2, §RP4.1 ×2, §RP3.13 ×2, §RP3.10 ×3, §RP5.1 ×1, G1.0 ×2,
  G1.9 ×2, G1.6b ×1, G1.6(i) ×2 — the last, after a settlement gate + `lane_diff`, was 22
  `.txt` plus a 0-byte `controls/gfm/DA3ABD.tmp`, a new shape), 1 … 36 files, varying
  between runs of the *same* tree (measured at §RP3.13) and once with an unreproducible
  `corpus_gate` `137 passed; 1 failed`; every set was removed before its commit and both
  lanes were green with the files present, no tracked corpus or golden byte ever moving, so
  the leak costs hygiene only. **It is a drop-order race, not a missing sweep:** `impl Drop
  for CorpusGuard` (`corpus_gate/runner.rs:161-190`) releases the directory lock *before*
  `sweep_created` and the restore loop run, so a sibling case starting in that window
  (`Test/AutoTrans` holds five cases in one directory) photographs the outgoing case's
  exports as "vendored" and its own drop rewrites them — only files under `RESTORE_MAX`;
  scoped or single-binary runs leave it clean, and the case-insensitive collision at
  `runner.rs:42-55` compounds it. The fix (hold the lock across sweep + restore) owes a
  gate-contention measurement in a file every lane is editing (**D7**); start with
  `DSS_GATE_JOBS=1` per G2.2d.
- **`RelCalc` leaks reliability accumulators across meter zones — engine finding, OPEN
  (GOLDEN_REBASE G1.6(i) audit settlement AT-1, 2026-09-05).** `DoLambdaCalcs` zeroes only
  `BusFltRate`/`Bus_Num_Interrupt` circuit-wide (`ExecHelper.pas:4432-4441`);
  `BusTotalMiles` and its siblings are zeroed per meter on the FROM bus of that meter's
  `SequenceList` (`EnergyMeter.pas:2471-2472` → `PDElement.pas:313-328`) and read on the TO
  bus (`:105-111`), so a zone-boundary bus is zeroed by the inner meter and read by the
  outer one: nested pair, `Bus.TotalMiles(src)` = `2.0 → 3.0 → 3.0` declared outer-first but
  `3.0` from run 1 inner-first — **measured identical on both oracles**, so the port mirrors
  them. Intent contradicts behaviour (r4133's own "Zero reliability accumulators" comment) →
  upstream defect `to_opendss/61-relcalc-cross-zone-accumulator-leak.md`, pinned with its
  run-count and declaration-order arms. Unfixable inside G1.6(i) (R-14(d)); the correct
  value for a nested head bus needs a semantics decision (3.0 vs 2.0 at the zone boundary —
  circuit-wide zeroing fixes idempotence, not order-independence). **G1.6(ii) HANDED IT ON**
  (2026-09-05): its six-case flagged population holds exactly one two-meter deck
  (`midi_energymeter`) and both of its meters abort at 52902 before any section is allocated, so
  no nested zone-boundary bus is gated live and the semantics decision has no witness to settle
  it against; the four `DOCTechNote` decks that would supply one are out of the population by
  D-i-2. The pin and the `to_opendss` report stand; **owner: `ORPHANED_GAPS.md` §1.19** (named by the
  G1.6(ii) audit settlement, 2026-09-05: no WP-G1 sub-step's population holds a witness deck).

**Carried-forward and residual-floor items — the rows still open.** Full text, closed rows
and all: [`follow-ups-carried.md`](docs/phase-records/follow-ups-carried.md).

- **IEEE118Bus NCIM PV→PQ switching cadence → a future UPGRADE rung** — matches capi015
  loop-for-loop but not r4133's newer cadence; parked `skipped_needs_investigation`,
  report-only in `DIVERGENCES.md`.
- **DER user-model FILENAME render** (`UserModel`/`ShaftModel`/`DynaDLL`) unpinned — do it
  with the WASM loader's error-path gating, not by weakening `gen_json.py`.
- **WindGen `Spectrum` FullNames render ungated** — the class is absent from the pinned
  0.14.5 oracle; needs an r4133-side JSON channel or a UPGRADE rung.
- **A-Diakoptics `AggregateProfiles` → DIAKOPTICS Part II WP-AD.5, partial**
  (`exec/command.rs` `NOT_PORTED`); WP-AD.6 threaded children needs M2.
- **The `like=` / `clone_ckt` user-model tail** — the WM.3/WM.4 slots keep the older lazy
  shape, unreachable in the corpus; all the RP1.3 settlement left.
- **The file-backed-loadshape ORACLE flake is not extinct** — last recurrence 2026-08-23 on
  RP1.3's parity-lane gate run; next suspect is the case-directory guard restoring `mm8.csv`
  while a one-shot worker still has it mapped.
- **ckt24 RegControl/LDC `SubXFMR`** ~4.7e-5 rel tap current — an ultra-switch conditioning
  floor (CF-D); watch on re-touch.
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

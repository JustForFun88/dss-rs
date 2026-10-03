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
active work is `PLAN_SEQUENCE.md`; the binding conventions and the seven-command
gate are `CLAUDE.md`. The 2026-08-02 user policy is in force: **EPRI r4133 is
the behavioral authority, the pinned dss_capi 0.14.5 is a numeric oracle only,
and upstream bugs are never reproduced in any lane** — the `oracle-parity` lane
has shrunk to a precision-compat lane and is scheduled for full teardown.

**In flight.** `GOLDEN_REBASE_PLAN.md` **WP-G1** on **`update`**, its sub-step chains running since 2026-09-04 in parallel `lane-*` worktree branches that a
merge agent lands one at a time, regenerating `population.lock.json` on the merged tree and unioning `ledger.json` (**D7**); that lock, `ledger.json` and
`golden.lock.json` stay fail-on-stale. **`R4133_PROPS_PLAN.md` is COMPLETE** (2026-09-04, §RP5.2) — all six WPs gate-green in both lanes over 26 sub-steps /
**67** RP-titled commits (64 through RP5.1's `1af27153`, plus RP5.2's `6f5c7206`, its settlement `609e07ea` and this record), plan archived to
`docs/plans-archive/`, `PLAN_SEQUENCE.md` row 5b COMPLETE with the final counters, record in [`era-summaries.md`](docs/phase-records/era-summaries.md) §1a,
**G1.1 handed back satisfied**. Close-out 2026-09-04: `update` fast-forwarded to `r4133-props` @ `b5fe5016` (32 commits, the branch then deleted) and pushed to
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
  `8c22e898` + `34591f24`; its settlement opened the WindGen follow-up below) and RP3.13 the
  two NCIM port bugs (2026-09-03, `a029b5df` + `ccaecfba`; its settlement stopped
  reproducing a fourth r4133 defect, `CalcInjCurrAtBus`' PC-element sign).
- **The three recorded-but-never-reproduced sub-steps** — RP3.9's 27 `PRECISION_ROUNDTRIP`
  pairs (`OPEN_RP39 = (0, 0, 0)`), RP3.11's `KEEP_LIVE_PINNED` `Save`/`Dump` surface (whose
  P0 findings opened RP3.13) and RP3.12's `autotrans.wdgcurrents` `UPSTREAM_BUG` — zero
  product-crate lines between them. Per-sub-step shas, verdicts and settlements:
  `r4133-props-rp5.md` §RP5.2.

**WP-RP4 (the unmask) — COMPLETE** (RP4.1, 2026-09-03, `c82d4fdc`, 23 files +2 576 / −536;
zero product-crate lines, zero golden bytes, zero tolerances moved). `all_properties` is
compared on the r4133 channel for every live non-`large` case: at RP4.1 the **83
r4133-only** non-large cases got a first property check and the **313 non-`large` `both`**
cases their r4133 property table — **1 670** gating walks over **151 782** elements per run
(87/311 and 1 672 walks / 151 798 elements since G1.4a's flip and G1.6(i)'s deck), both
lanes. G1.1's re-armed kill criterion did **not** fire (zero ledger entries and zero pins
from RP4.1's residual triage). Full record:
[`r4133-props-rp4.md`](docs/phase-records/r4133-props-rp4.md).

**WP-RP5 (operational docs + closing record) — COMPLETE.** **RP5.1** (2026-09-04, `5acd0a49`
+ `d04f9d14`), docs only: the r4133 claim chain (normalize → echo → floor → assert), the
property-divergence triage procedure and **46** `file.rs:LINE` citations, which its
settlement turned into an executable walk over all **58**. **RP5.2** (2026-09-04, `6f5c7206`
+ settlement `609e07ea`) — the closing record and the archive move, final counters as
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
reproduced in any lane (the Newton one only on the gate's snapshot reader until
RETRO_FIXES RF-D00-01 moved the repair into the solver). Full record:
[`golden-rebase.md`](docs/phase-records/golden-rebase.md) section "GOLDEN_REBASE WP-G0 /
WP-G2 — condensed records" (full session records precede it there).

**GOLDEN_REBASE WP-G1 (live gate to fastdss parity) — OPEN** (opened 2026-08-08; since 2026-09-04 its chains run in parallel **lanes**, D7, merged one
sub-step at a time). Landed: **G1.1**, killed and delivered by `R4133_PROPS_PLAN.md` RP4.1 2026-09-03 (**G3.4**/**G3.5** unblocked); **G1.2** (the
ESPVLControl deck) 2026-08-29; and, 2026-09-04/06, the rails plus seventeen surfaces, each with its full record in
[`golden-rebase.md`](docs/phase-records/golden-rebase.md) — **G1.0** (`abc6ee9d` + `0297e2c0`), the flag vocabulary, exclusion `channels`, the capture guard
and the r4133 bridge (its mode probe **discharges G1.11**); **G1.9** (`lane-s`, `8817f82a` + `ef932106`), `Circuit` aggregates + `Solution` scalars;
**G1.6b** (`lane-m`, `0a1f2d6c` + `855f55ee` + `4c2b9fab`), the `PDElements` walk + the **D9** engine fix; **G1.3a** (`lane-e`, `a8b556d0` + `e6e1d4db` +
`844cbd60`), `Enabled` + the three polar channels (**1** new entry, `DIVERGENCES.md` L8, + **13** widenings); **G1.4a** (`lane-b`, `2158707a` + `ac192be6` +
`07cd0fea`), the bus surface's divergence-free half (**0** new; **D8** defers the rest to G1.4c), with **D11(1)** `float_roundtrip`, **D12**/**D14** the
four `GICTransformer` decks onto `r4133` (ledger −4) and **D13** the worker's registry leak; **G1.3d(i)** (`lane-e`, `f6a714c0` + `d8ad88f5` + `badad4f9`),
the per-element counts, `NodeOrder` and `EnergyMeter`, exact (**0** new); **G1.7** (`lane-s`, `9e891306` + `82d3f44f` + `eade703c` + `fa4b9593`), the six
`Topology` rows, memoized-tree and window-dedup defects **asserted, not excluded** (**D15**/**D16**, `(16, 135)` / `(8, 96)`; two port gaps fixed en route,
**0** new); **G1.6(i)** (`lane-m`, `799fa7b8` + `03689a00` + `04b55771`), meter extras and **the run protocol** — `RelCalc` driven once per case, the
indices, sections, `CalcCurrent`/`AllocFactors`, `Meters.Totals` and the **ordered** zone lists exact but for three tier-banded cells (**D17a**,
**D11/D18**, **0** new); **G1.5** (`lane-b`, `13a76b02` + `7409c11c` + docs), the bus short-circuit surface read as **precomputed state**, sentinels
normalized (**D4**), D11(2) narrowed to `Voc`/`Isc`, a `ReduceAlgs` port gap closed in step, `SC_STUDY_POPULATION` (**0** new); and **G1.3d(ii)** (`lane-e`,
`41d975ec` + `df6e7222` + `68a924e6`), `PhaseLosses` + the five control-derived scalars — **§G1.3d complete** — on an `assert_power_close` floor, first to
join `LANE_SKIP_ELEM_POWERS`, with the per-edit control re-attach (`DIVERGENCES.md` L9), the disabled-OCP scan and defect A-1 fixed behind a multi-control
census (**0** new; ten `phase_losses` widenings land as **eight**); and **G1.6(ii)** (`lane-m`, `7f5566ec` + `019ab9ee` + `94797302`), the eight per-bus
reliability columns on G1.6(i)'s payload — 6 cases / 50 capi + 84 r4133 buses, exact, keys `bus:<bus>:<field>`, plus the **D20/D22** engine fix
(`calc_reliability_indices` recomputes `TotalUpDownstreamCustomers`, r4133 `EnergyMeter.pas:2466-2468`; its only footprint is a corpus-unreachable capi
divergence under `RelCalc <restore>`, `DIVERGENCES.md` §D22) — **the PD/meter chain closes here** (**0** new); and **G1.3b** (`lane-e`, `9dd79d74` +
`7c3e35d5` + `64d03c6c`), per-element `SeqCurrents`/`SeqVoltages`/`SeqPowers` — **31** widenings on 11 `element` scopes, the n/A power sentinel a
channel-scoped fold, `SEQ_C012 = 5.229590094302253e-10` the r4133-only 012-matrix term — under **D31**: `makeposseq_gic.dss` alone reaches the 1φ-posseq arm
on `r4133`, so r4133's slot/stride defect costs **1** entry + a pin, census `(297 867, 78, 4)`; **G1.4c** (`lane-b`, `e11f960d` + `a6034934` +
`617df67e`/`6da6fd24`), the bus **sequence** and **line-to-line** arms, where port, capi and r4133 all differ: the port publishes the physically correct
answer (S-SEQ/S-VLL, **D21**), each oracle's own walk is **asserted** (**D15**/**D16**, four populations) and r4133's `VLL` **hang** is refused per bus
(**D2**; `DIVERGENCES.md` §G1.4c, `to_opendss/` 64-66) — **0** new; and **G1.4b** (2026-09-06, `lane-b`, `85557db4` + `69ba809e` + `349b6b8e`/`bf34640f`),
the bus **distance** surface, compared **exactly** (`rel = abs = 0`) behind `DISTANCE_POPULATION` **(867, 79 137)** with D9's `MakeBusList` fix pinned live
— **D26** split its at-bus half into **G1.4d** (*a plan amendment the user has not seen*), **D29** refused its own step 1 (r4133's `MergeWith` renames a
line without updating `DeviceList`, `to_opendss/68`), so `modes:reduce/midi_reduce.dss` stays capi-gated behind the new **`distance`** ledger field
(**+1**); the 012-matrix term was deduped here to one `SEQ_C012` (**D21**); and **G1.3c** (2026-09-06, `lane-e`, `23fdc5f9` + `5b9a8645` +
`ab8ed883`/`aabb5f05`), per-element `CplxSeqCurrents`/`CplxSeqVoltages`/`TotalPowers` — **the element chain closes here**, `compare_derived` at **thirteen**
sub-channels: **0** new entries, **25** widenings on 9 `element` scopes, `TotalPowers` the fourth `LANE_SKIP_ELEM_POWERS` channel on the two `newton*`
decks, D31's widening measured `seq_powers`-only; **G1.8** (2026-09-05, `lane-s`, `7653e933` + `047b0837` + `cdf29e6e` + `860741ac`/`d621c522`), the four
flat incidence quantities (`IncMatrix`, `Laplacian`, `IncMatrixRows`, `IncMatrixCols`), read **strictly last** (they move `ActiveCktElement`), the reactor
row cursor **asserted, not excluded** (S-INC, `INC_UPSTREAM_ROW_DECLINES = (4, 5)`; **0** new, no floor); and **G1.10a** (2026-09-06, `lane-s`, `e83f3558` +
`487abef9` + `4205433f` + `10b18523` + `dc3458d0` + `dfa55fe4` + `1fe2fbc9`), the run's **created-file SET** — one classification for all three producers,
the oracles' spellings ASCII-folded (`DIVERGENCES.md` §R-18), the harmonics scratch counted off (`SCRATCH_FILE_DECLINES` **(9, 9)**), **1** new entry (the
r4133 `Visualize` `.DSV`/`.dbl` pair, a *product* divergence) — with **D25** editor suppression, **D30(1)** the in-memory event log, **D32(1)** the Storage
`DebugTrace` **port gap**, **D33** the loud leak report + one producer per case dir, **D35** (trace-header loops 0-based; P14's ceiling stays 106); its
settlement also fixed the parent-guard resurrect behind the `Test/AutoTrans` residue below. **+ F0′** (2026-09-11, `lane-m`, **D39**/**D41**;
`9f0fb8b0` + `6664a73c` + docs): the
bridge gags report auto-display with r4133's own `AllowForms`/`ShowReports`/`ShowExport`, `Set Editor=` covering only the **12** unguarded
`FireOffEditor` sites (`to_opendss/73`); no report is suppressed — 56 = 56 created entries over 14 decks, **0** new entries; its audit
settlement adds the per-case re-assertion of both switches in `Engine::clear` (five live decks set `ShowExport` themselves). And **G1.4d** (2026-09-06,
`lane-b`, `dd60d1a8` + `572e58f2` + `0e3ea8a2` + `b9417cdc`/`d675a9f5`), the bus **at-bus lists** — **the bus chain closes here**: the port answers
**S4**, neither oracle's criterion (D26's disabled-drop premise refuted — 15 of 223 pairs ARE listed); each channel's own walk is **asserted** over the
port's raw terminal facts (**D15**/**D16**/**D21**) and the residue COUNTED into four fail-on-stale populations — (279, 279) / (18, 147) / (8, 11) / (0,
0) — the completeness direction added by its settlement (`assert_port_at_bus_is_s4`); the two r4133 mode rows go `Pure` → **Impure**, **0** new entries,
no flag and no lock cell; **D34**'s unported executive commands sit at `ORPHANED_GAPS.md` §1.21. And **G1.10b** (2026-09-12, `lane-s`, `3c5fc74f` +
`b9a4abfa` + `02a4b3a9` + `fd1dcd50`/`42c86549` + docs), the run files' **CONTENTS** — nine report kinds through the SAME `ExportPolicy`
their goldens use (lifted into `harness::export_policies`), on **D40**'s *case floor + print ulp* rule, a derivation with **no new constant**: 7 cases / 32 file comparisons /
**3 035 190** cells per drive, sidecar-transported, three fail-on-stale censuses per channel; classes C1–C5 all inside the rule (**0** new, 0 golden bytes, lock identical), the
Storage trace's **36** `%-.g` columns declined on BOTH channels (the oracles disagree → **G4.1**) and its row count the reader's own footprint (**D43(1)**, 102 vs 98); two port
gaps landed ahead of it — the `InShowResults` bracket round Show/Export/Save (`DoSaveCmd`'s latch NOT reproduced, `to_opendss/72`) and nine PC `node_ref` guards (`to_opendss/74`). And **G1.10c** (2026-09-12, `lane-e`, `1f77b145` + `015d2c4f` + `ac7aab89` + docs), the **demand-interval tree's contents** — five
decks, 36 files per channel, **9 793 064** cells compared at each column's own calibrated tier (**D42(1)** S-CLASS: the brief's `di_policy()` fixture floor is refuted — the two ORACLES fail it against each other on
46 130 ckt7 + 2 884 123Bus cells — so **no** `Tolerances` value, constant or golden byte moved), forced on **every** live case, captured into a per-channel sidecar before the capi teardown `clear` (**D43(2)** orders the merged tail
against G1.10b); **+4** entries / 2 causes on the new exclusion-only **`di`** field, both ckt7 columns (`kvarh`, a cross-engine indeterminate whose pin asserts the measured decomposition after **D44** withdrew
D42(2)'s two universals; `min lv bus`, an argmin over a bit-identical tie), `DI_TREE_CENSUS` **(5, 883, 72, 9 793 064)**, `lane_diff` not owed (dss-epri + tests only); its audit settlement (8 reports, **34** findings — 27 fixed / 7 recorded / 0 refuted) pinned the masked `kvarh` column whole (row-scoping REFUSED as measured), keyed the census by (case, **channel**), dropped the scanner's non-`InterpretYesNo` `=1` reading and corrected six citation/count sites; a round-2 settlement of that settlement's own audit (6 findings — 4 fixed / 2 recorded) recorded all three tied buses' oracle magnitudes and restated the whole-column pin's sensitivity at its measured scope, leaving SA-6 (those 8 736 `kvarh` rows are watched port-side only — neither narrowing exists in the tree) for the coordinator. The lanes' micro decks met here: corpus **526** cases /
522 live; `FORCED_{PROPS,ELEMENT_EXTRAS,PDELEMENTS,BUS,ZSC,TOPOLOGY,INC_MATRIX,RUN_FILES}_POPULATION` = (443, 312, 87, 44), `FORCED_DERIVED_POPULATION`
= (445, 314, 87, 44), `FORCED_DI_POPULATION` = (522, 366, 101, 55) — the one forced surface outside that equality; no golden byte, |Δ| = 0 throughout; `WP_G1_MODES` **111** (**20** `Impure`), ledger **61** / 35 causes; the **G1.11′**
close-out (G1.11a–c were absorbed sub-step by sub-step, **D2**/**D36**) and WP-G3–G5 remain.

**RETRO_FIXES (settle the live findings of the 2026-09 retro audits) — OPEN**, run in waves of up to three lanes per
[`RETRO_FIXES_PLAN.md`](RETRO_FIXES_PLAN.md), one record block per step in
[`retro-fixes.md`](docs/phase-records/retro-fixes.md). Landed:

- RF-I00-01 `257b8603`.
- RF-I00-03 `0278ed0a`.
- RF-I00-04 `e45d5abf`.
- RF-I00-05 `967a3fef`.
- RF-D07-07 `c41148f6`.
- RF-D01-01 `24cd6f37`.
- RF-D00-05 `5d934d40`.
- RF-D08-06 `540cba7d`.
- RF-D00-01 `16455d01` (Newton stale `Iterminal` repaired at the solver for every reader, no ledger/golden/lock change).
- RF-D01-04 `f8327474` (`Save` sizing-property hoist derived from the property tables for every class, 0 golden bytes, 0 ledger rows, its leftovers are listed in its record).
- wave 2 turned its items into rulings R14-R17 and five R12 follow-up steps (`86e5191a`) and linked MSVC builds with rust-lld (`05e2539e`).
- RF-D02-01 `9f76d027` (AutoTrans `XfmrCode` fetch: phases guard, `BusNameRedefined`, honesty guards).
- RF-D00-16 `7a026fe6` (`props_r4133` evidence-lock holes closed).
- RF-D07-01 `c8822163` (the `LINE_CITED_DOCS` walk became the R18 symbol rail, `TESTING.md` and `TOLERANCE_NOTES.md` cite code by item).
- RF-I00-02 `3df57bcf` (the capi one-shot names its exit status, the `espvlcontrol` deck keeps the type-confused capi redispatch silent).
- RF-I00-06 `38823ea7` (the comments and texts the harness move made false re-worded, ruling R19).
- RF-I00-09 `fdc23919` (the build/test queue tracked as `tools/gate/gatelock.py`).
- RF-D03-01 (the `Export SeqCurrents` zero-footprint claim corrected by a census of negative ratings, the derived negative AutoTrans rating pinned).
- RF-D01-13 (`capture_order.rs` scans the run tail to the end of `run_case`, counts a read on any handed receiver and an `Engine` command through any path).
- RF-D08-01 (the corpus gate pins its deck-wide bus-array suppressions, the ledger's sequence and complex envelope bands are driven by asymmetric cases).
- `capture_order.rs` and `tools/oracle/oracle_server.py` state the oracle behaviour in plain words, with no Pascal source citation (user decision 2026-10-03, follow-up of RF-D01-13).
- RF-I00-10 (the harness texts state the present, the scratch and DI census asserts read manifest rows only, the D24 pin reads all three census counters, `lane_dump` reads the lane register from the harness, the locked ledger `source` names the harness crate).
- RF-D02-15 (Generator, PVSystem and Storage keep their user-model variable tails when a `DynamicExp` is linked, an out-of-range `VariableName` follows r4133).
- RF-D08-07 (`golden_lock.rs` reads `.gitattributes` in any ASCII case, fails an ambiguous provenance declaration and pins the declared-provenance census).
- dss-epri exit hang (an r4133 DLL bug: a process that exits before the circuit's solver thread has started spins forever in the DLL's exit cleanup). `Engine`'s drop frees every circuit, every wait of the `protocol` tests is bounded and kills the worker, the registry tests restore under a cross-process lock, the guard tests clear their fixture sidecar, and the nextest profile terminates a test at 1800 s (`terminate-after = 3`, user decision 2026-10-03).

**Next.** (**D41**/**D27**, 2026-09-12) WP-G1’s last three landings are in `update`, first-finished-first-landed, one at a time, each
pushed: **F0′** (`lane-m`), **G1.10b** (`lane-s`) and **G1.10c** (`lane-e`, the three merges above). No WP-G1 surface is in flight; `lane-b` has been
idle since G1.4d. Next comes the **G1.11′** docs close-out in the main tree (**D37(1)**, `lane-e tmp/g111/brief.md` — G1.11a–c were absorbed
sub-step by sub-step, **D2**/**D36**), then WP-G3 with **G3.1** alone first (**D37(9)**, `lane-e tmp/g30/`); both wait on the user’s go (**HOLD** on
new plan steps, 2026-09-12). Queued: `WASM_USERMODELS`, RESONANCE, MULTITHREADING, UPGRADE.

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

- **The port never clears `HAS_OCP_DEVICE`/`HAS_AUTO_OCP_DEVICE` — OPEN** (adjacent defect A-2, GOLDEN_REBASE G1.3d(ii), lane
  `lane-e`, 2026-09-05). r4133 clears both at the head of every control `RecalcElementData` and re-sets them only `If Enabled`
  (`Controls/Relay.pas:946-964`); the port's `RefAction::SetOcpDevice` only *includes* them, so the two `DG_Prot_Fdr.dss` decks
  (`engines: "both"`) keep a feeder-section head the oracles dropped. Blast radius = the whole reliability sweep, so **G1.6 /
  G1.6b own it**. Sibling **A-1 is fixed** (live `GetOCPDeviceType`, pinned by
  `section_device_type_is_the_live_ocp_scan_not_the_registration_latch`); retiring the now test-only
  `CktElementData::ocp_device_type` latch is A-1's tail.

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
  anti-shrink accounting, the 526-case count in `CLAUDE.md`, `TESTING.md` and
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
  (`FORCED_PROPS_POPULATION` = (443, 312, 87, 44) since G1.4a's D12/D14 flip and the G1.6(i)/G1.5
  decks — the gap itself unchanged at 54 — asserted by
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
  action for the constructor: the port and the pinned 0.14.5 oracle both build the class.
  What is owed is an English write-up in `investigations/to_opendss/` (local-only folder),
  which also carries two r4133 source-level ESPVLControl bugs that the class's constructor
  crash keeps from running live (RETRO_FIXES RF-I00-02, 2026-10-01): `TESPVLControlObj.Sample`
  stores `Gen.kWBase` through a `TGeneratorObj` cast of another ESPVLControl
  (`Version8/Source/Controls/ESPVLControl.pas:531-535`), the store that corrupts the pinned
  capi 0.14.5 process whenever a SystemController redispatches (37 one-shot deaths in 9951
  concurrent runs, `#8888` on a later compile). And a `kWBand` edit never moves the dead
  band (`:253` sets `FkWBand` alone, `HalfkWBand` is assigned only in `Create`, `:384`,
  where `GenDispatcher.pas:214` recomputes it on the same edit). The port reproduces the
  inert `kWBand` until an R12 follow-up of RF-I00-02 lands.
- **`ESPVLControl.Forecast` — r4133 property 12 absent from the port, OPEN (GOLDEN_REBASE
  G1.2 audit settlement, 2026-08-29).** r4133 declares **12** class properties where the
  pinned dss_capi 0.14.5 declares 11:
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
- **`CorpusGuard` leaks deck-written artifacts under concurrency — root cause FOUND and FIXED at GOLDEN_REBASE G1.10a (2026-09-06); CLOSED by RETRO_FIXES RF-I00-01 (2026-09-26), bar the `espvlcontrol` sentences at its end** (first seen G1.2 2026-08-29; drop-order mechanism
  measured at G1.6b). Unfiltered `cargo test --workspace` runs intermittently leave untracked deck-written exports in the tracked corpus tree — nearly always `tests/corpus/electricdss-tst/Test/AutoTrans/`
  (`Auto3bus_*` / `AutoHLT_*` `.txt`, from the decks' own `export … file=` lines). **Forty-two sightings** 2026-08-29 … 2026-09-06 (G1.10a ×8, the RP rounds ×10, four merges ×7, G1.7 ×4, G1.8 and G1.6(i)
  ×3 each, the rest across G1.0–G1.6(ii); the `lane-b` chain retired its tally — nearly every unfiltered run left a set), 1 … 36 files per run; every set was removed before its commit and no tracked corpus
  or golden byte ever moved — hygiene only. **Two mechanisms fixed.** The drop-order race (**D33(2)**): `runner::CorpusGuard` now holds an exclusive claim on the *canonicalized* case dir from before the
  pre-run photograph until after the sweep **and** the restore, and the contention it owed measured negative. The parent-guard **resurrect** (`dfa55fe4`): `Test/` holds 36 manifest cases and
  `Test/AutoTrans/` five — two claim keys, concurrent by design — so the parent photographed the child's live output and its `restore` wrote back a file the child's own guard had swept; both Rust guards
  now leave a GONE entry gone (an overwritten one is still restored; `corpus_guard.py` always did), pinned by `a_parent_guard_does_not_resurrect_a_sibling_cases_swept_output`, 9 leaked before / 0 after.
  **Residual, narrowed at G1.10c (2026-09-12):** `kind=large*` decks are still outside `force_run_files`, but `force_di` arms every live case and the port's `RunFileProbe` bracket is built on
  `compare_run_files || compare_di`, so the ~79 live `large*` decks now get a snapshot, classification and sweep too and a leak there fails the case loudly through `sweep_failed`; what stays open is the
  non-live population and G1.10a's file-set COMPARISON on `large` decks (ckt5's capi `EarlyAbort` costs that channel one member, **D42(8)**). G1.10c's settlement recorded one more shape for that surface's
  owner: the presence re-check after the sweep has no bounded re-list, so a delete-pending entry (a scanner still holding a just-written export) is reported as a leak — four parity reds in
  `Test/AutoTrans/` under concurrent load, every file gone from disk afterwards, the cases green scoped and on the quiet machine (**D23**). The same shape hides in **gitignored** droppings: two stale
  `*_SavedVoltages.dbl` in the main tree (one dating from 2026-09-04) read as pre-existing and shrank `SCRATCH_FILE_DECLINES` to (8, 8) until this merge deleted them by name. The single-case `espvlcontrol`
  "You must create a new circuit object first" flake stays open, with a new rail: `run_rust_capture` asserts the compile produced a circuit and prints the deck's size on disk. **Closed by RETRO_FIXES RF-I00-01 (2026-09-26):** no test writes under `tests/corpus/` any more - measured per test binary (72 targets x 0 changes), not enforced for plain in-place compiles of decks with no writing verb; every former tree writer and every corpus-gate producer runs a per-run scratch copy (`harness::scratch::ScratchCopy`, `b354cf26` / `0cffd47b` / `9ace2e25`) removed within 25 x 200 ms, and the corpus gate fails on a change of the tree's listing or mtimes during its own walk (`scheduler::GateRun::assert_complete`, `71dd09ac`); 0 tree changes in every gate run from part 1's run 5 on, both lanes. The `espvlcontrol` compiled-to-no-circuit shape stays with R11 / RF-D09-06; the capi one-shot's native death on that deck is RF-I00-02. **Closed by RETRO_FIXES RF-I00-02 (2026-10-01):** the cause is the pinned capi engine's SystemController redispatch, which stores `Gen.kWBase` through a `TGeneratorObj` cast of another ESPVLControl (`TESPVLControlObj.Sample`) and corrupts the process; `kWBand` cannot silence it (`HalfkWBand` is fixed in `TESPVLControlObj.Create` on both oracle revisions). The deck's `sys`+`scan` now monitor a branch to a constant 8000 kW load, so the redispatch never fires on the oracle (0 deaths in 3000 concurrent one-shot runs under a full corpus-gate run, against 37 of 9951 with it firing); the port's named-list redispatch is pinned by `crates/dss-core/src/exec/tests/espvl_control.rs::system_controller_redispatch_writes_the_weighted_deficit` and the deck's silence by `crates/dss-core/src/exec/tests/espvl_control.rs::system_controller_inside_the_band_writes_nothing`. A no-JSON death now prints the one-shot's exit status and `oracle_server.py` runs with `faulthandler`.
- **`RelCalc` leaks reliability accumulators across meter zones — engine finding, OPEN
  (GOLDEN_REBASE G1.6(i) audit settlement AT-1, 2026-09-05).** `BusTotalMiles` and its siblings are
  zeroed per meter on its `SequenceList`'s FROM bus and read on the TO bus, so a zone-boundary bus
  is zeroed by the inner meter and read by the outer one — measured identical on both oracles, so
  the port mirrors them; upstream defect `to_opendss/61-relcalc-cross-zone-accumulator-leak.md`,
  pinned with its run-count and declaration-order arms. What is open is the *correct* value at a
  nested head bus; G1.6(ii) **handed it on** (2026-09-05) — no WP-G1 population holds a witness
  deck. **Owner: `ORPHANED_GAPS.md` §1.19** (full statement, citations and priority there).

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
# Gate (must be green before any commit; the seven commands of CLAUDE.md "## Gate")
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --features dss-core/oracle-parity -- -D warnings
cargo nextest run --workspace
cargo nextest run --workspace --features dss-core/oracle-parity
cargo test --workspace --doc
cargo test --workspace --doc --features dss-core/oracle-parity

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

**Official-EPRI-OpenDSS oracle — the in-house bridge (`crates/dss-epri`).** The
gating r4133 channel is the git-tracked `tools/opendss/bin/r4133/` DLL driven by
`epri-worker`; `TESTING.md` and `tools/opendss/README.md` hold its operating
rules, among them the three-layer UI suppression F0′ landed 2026-09-11
(`Set AllowForms=No` → `Set ShowReports=No`/`Set ShowExport=No`, re-asserted per
`clear`, with `Set Editor=rundll32.exe` as the safety net for the 12 unguarded
`FireOffEditor` sites) — **no report is suppressed, only the viewer launch**.
The opt-in Oddie/dss-python tooling §6 used to describe (`ab_compare.py`,
`known_diffs.json`, `dsspy_validation/`, `wheels/`, `dsspy_crosscheck.py`, the
r3723/r4088 binaries) was retired with the bridge round and is no longer in the
tree; its per-consumer disposition and that text verbatim are in
[`epri-bridge.md`](docs/phase-records/epri-bridge.md).

**Re-vendoring the corpus still bites:** `.inputs/electricdss-tst` is a
re-checkout with different EOLs, so `tools/corpus/vendor.py --force` produces a
~1544-file EOL-only diff — clean run pollution with `git restore tests/corpus`
instead; re-vendor only deliberately.

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
| `epri-bridge.md` | the `dss-epri` bridge parity round and capability round (Oddie retirement) — plus §6's three retired-Oddie tooling paragraphs, moved verbatim 2026-09-11 |
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

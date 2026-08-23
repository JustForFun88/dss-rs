# R4133_PROPS — property parity on the r4133 channel (the GOLDEN_REBASE G1.1 successor)

## Source-integrity gate — ritual step 0 (before the model-tier check)

The Pascal at `.inputs/dss_capi` (186 `.pas` files) is the *spec*; oracle/live work
also needs `.inputs/electricdss-tst`; every r4133 citation in this plan needs
`.inputs/electricdss-code-r4133-trunk` (`Version8/Source/...` — the gating DLL is
built from `Version8/Source/DDLL/OpenDSSDirect.dpr`); the live r4133 channel needs
the git-tracked DLL at `tools/opendss/bin/r4133/` and the pinned dss-python of
`tools/golden/PIN.txt`. Before doing anything, and re-checked continuously (not only
at kickoff), confirm those folders exist and are non-empty. If one has vanished —
missing or empty — at **any** point in the work, **STOP immediately**: make no
edits, run no gate, and do **not** reconstruct, guess, or "port" a source you cannot
read. Tell the user the vendored source is gone and must be re-vendored, then wait.
Reply exactly:
**«Исходник (`.inputs/...`) отсутствует или пуст — работа остановлена. Восстанови
vendored-исходник (re-vendor) и повтори команду.»**
This gate runs **ahead of the tier/refuse check** (`PLAN_SEQUENCE.md` §Model-tier
protocol). Also verify `(Get-Command cargo).Source` is under `.cargo\bin` before
trusting any gate result (the mingw shadow trap).

Plan-specific evidence trap: the G1.1 census artifacts live in the **gitignored,
local-only** `investigations/g1_1_r4133_props/` (five small extracts, 24 944 bytes
total, plus the 270 MiB `r4133_props_census.json`). RP0.1 vendors the extracts into
the repo; until that lands, treat the local folder as load-bearing evidence — if it
is gone before RP0.1, do not guess the numbers: rebuild the census first with the
RP0.2 knob (order RP0.2 before RP0.1 in that case, and say so in STATUS).

> **Decision context (binding, user 2026-08-22):** GOLDEN_REBASE G1.1 fired its
> kill criterion on 2026-08-08 (`GOLDEN_REBASE_PLAN.md:394-396`; STATUS.md §1) —
> unmasking `all_properties` on the r4133 channel diverges on **433 of ~512 live
> cases** (209 structural pairs / 960 129 cells, 94 numeric pairs / 95 317 cells,
> 5 property-table shape gaps; restricted to the 462 r4133-gating cases: 390
> cases, 198 / 920 954 structural, 53 / 90 627 numeric of which 16 pairs are
> genuine value jumps). The user's decision: author this dedicated plan now.
> **G1.1 is superseded by this plan**: its deliverable (the unmask) lands as
> RP4.1; GOLDEN_REBASE WP-G3 sub-steps that presume live props on both channels
> (G3.4 `GOLDEN_REBASE_PLAN.md:928`, G3.5 `:941`) wait for RP4.1.
>
> The construction has three parts, in this order: **1)** make the evidence
> durable and the property tables complete (WP-RP0 evidence + census rails,
> WP-RP1 shape closure — two real ports, two upstream-stub rows, one upstream-bug
> allowlist row); **2)** build the channel-aware value comparator (WP-RP2:
> spelling-preserving normalization rules, the echo-exclusion table with pins,
> the r4133 display floor; WP-RP3: root-cause the four genuine jumps that are
> not echo); **3)** flip the mask (WP-RP4) and close the docs (WP-RP5).
>
> **Ordering is mandatory.** The unmask (RP4.1) is last-but-docs because every
> earlier WP removes a whole category from its residual: without RP1 the shape
> assert fails on 149 in-scope element rows across 3 classes (429 across 5 in
> the full census); without RP2/RP3 the value assert fails on ~1 million cells.
> Measured basis: the census bin counts in §1.1 (from
> `investigations/g1_1_r4133_props/`, method in its triage report); the
> precedent for a channel-keyed mechanism table paired with per-case ledger
> entries is the eventlog-mask + ledger pair (`harness/mod.rs:1643-1689`,
> `tests/corpus/ledger.json` — the mask table ships with an empty r4133 row
> set, its emptiness itself the documented proof none was needed, so the
> keeps-the-gate-green precedent is carried by the ledger half); the precedent
> commit for landing an exclusion with its expected-value pin is `4f977d9e`
> (GOLDEN_REBASE G2 procedure).
>
> **Standing rules of engagement** (CLAUDE.md is authoritative): r4133 is the
> behavioral authority and upstream bugs are NEVER reproduced in any lane; every
> deliberate divergence from an oracle channel is excluded field-by-field and
> pinned by an expected-value test (`tests/corpus/ledger.json` / harness tables /
> golden fix-ups); tolerances are never loosened to make a comparison pass; a
> Rust↔oracle gap above its floor is a bug until proven otherwise; `TODO(compat)`
> stays precision-only. **The r4133 property strings are NOT engine state**: for
> every property index a class's `GetPropertyValue` override does not cover,
> r4133 echoes the `PropertyValue[]` string store (last parse or
> `InitPropertyValues` default) — `Version8/Source/General/DSSObject.pas:112-115`
> — while dss_capi 0.14.5 and the port always render the live typed value.
> Matching those echoes would mean emulating a parse-string store, i.e.
> reproducing upstream architecture warts and bugs (`swtcontrol.delay` renders a
> default the deck overrode) — **forbidden**. Echo-sourced cells are excluded and
> pinned, never imitated. Branch off `update` (branch `r4133-props`), merge back
> into `update`. Commit messages short.
>
> **Stop-and-confirm cadence:** after each sub-step run the per-sub-step ritual
> below **autonomously, without pausing between its stages**; the single stop
> point is at the very end of the sub-step — then wait for the user's explicit
> confirmation (unless the user authorized several sub-steps in one pass).

## Per-sub-step ritual (do EVERY sub-step, in order, without being told)

Every sub-step below — regardless of how trivial it looks — runs this full ritual.
No batching of audits across sub-steps, no shared fix agent across sub-steps.

0. **Tier check** (protocol: `PLAN_SEQUENCE.md` §Model-tier protocol). Look up the
   sub-step's row in the tier table (§0 below). The check is **mandatory and
   explicit** for all three agent roles: the **implementation agent** at the
   sub-step's exec tier or higher; both **auditors** at the audit tiers listed in
   §0, spawned with an explicit model/effort override, never "whatever the session
   runs"; the **fix agent** at the exec tier or higher. If the session/agent is
   below the required tier, do NOT execute; reply exactly:
   «Этот шаг требует <tier>. Переключи сессию (/model + reasoning effort) и повтори
   команду.» and stop. If the session's reasoning effort is not visible, ask the
   user to confirm it before executing — **mandatory on the `opus-xhigh` rows**
   (RP1.2, RP1.3, RP2.1, RP2.4), per `PLAN_SEQUENCE.md` §Model-tier protocol.
   Fable is used only with explicit user approval (standing rule since 2026-07-19).
1. **Implement** — one dedicated implementation agent (or the session itself at
   tier) executes exactly this sub-step's scope. Scope creep = a finding, not a
   favor; a gap discovered mid-step that belongs to the sub-step is fixed in the
   sub-step (the "port gaps immediately" rule), a gap that belongs elsewhere is
   recorded in STATUS.
2. **Gate green** — the five mandatory commands:
   `cargo fmt --all --check`;
   `cargo clippy --workspace --all-targets -- -D warnings`;
   `cargo clippy --workspace --all-targets --features dss-core/oracle-parity -- -D warnings`;
   `cargo test --workspace`;
   `cargo test --workspace --features dss-core/oracle-parity`.
   Plus `pwsh -File tools/lanes/lane_diff.ps1` for any sub-step touching solved
   state or a compat kernel — in this plan that is RP1.2 and RP1.3 (both change
   engine behavior: Y-invalidation via `XfmrCode`, WindGen model dispatch). No
   `#[ignore]`, no name-filter that can green on zero matches; a red test blocks
   the commit.
3. **Update `STATUS.md`** (frontier + the plan record), **commit** (code + STATUS
   together).
4. **`/audit-code` + `/audit-tests` in parallel** — two **fresh independent
   agents, never forks**, spawned with the explicit audit-tier override. Each gets
   a self-contained brief: the sub-step's commit range (`<sha>^..HEAD`), the diff,
   the authoritative sources (the r4133 `Version8/Source` unit:lines named in the
   sub-step, the vendored census extracts of RP0.1, and/or the epri-worker live
   probe procedure), the plan section, and the binding rules above. Auditors are
   **read-only** and return findings only.
5. **Dedicated fix agent** — a **fresh agent, one per sub-step** (exec tier),
   receives both auditors' findings plus the same brief, settles each finding
   against evidence (r4133 source, a live oracle probe — never "sounds
   plausible"), fixes what is real, re-runs the full gate, commits. A finding
   deliberately not fixed is **recorded in STATUS with its reason**, never
   dropped. If both audits return nothing, the fix agent is skipped (no empty
   commits) — record "audits clean" in STATUS instead.
6. **STATUS review** — read `STATUS.md` end to end; sync whatever the sub-step
   made stale (no two places disagreeing), dedup restated paragraphs; if anything
   changed, re-run the five-command gate and land a `docs:` commit, so the
   sub-step ends on a **clean tree**. Then stop and report **in Russian** (code,
   identifiers, commit messages and STATUS stay English): what landed, what the
   audits found and how the fix agent settled it, gate status, next sub-step.

## 0. Scope, ordering, tier table

Six work packages, strictly ordered as WPs: RP0 → {RP1, RP2, RP3} → RP4 → RP5.
Inside the middle group, sub-steps may interleave **with these hard constraints**:
RP2.1 (the normalization engine and the replay harness) lands before RP2.2, RP2.3
and RP2.4 (they add rows to its tables — `PROPS_ECHO_R4133` is created **empty**
in RP2.1; its rows land in RP2.3); RP2.3 lands after RP2.2 (its row set is "bin 5
+ bin-7 echoes minus what RP2.2 routed here" — undefined before RP2.2); RP3.2
lands after RP2.3 whenever its outcome is an echo row; RP3.4 lands after RP2.3
(its ledger twins must not duplicate echo rows). RP4.1 starts only after
**every** RP1–RP3 sub-step is landed, including any RP3.5+ sub-step RP2.2's
triage opens, **§RP3.8, which RP2.3's kill criterion opened** (its 1 064 cells
are re-routed, not claimed — see `RP38_ROUTING` in the replay) **and §RP3.9,
which RP2.4's audit settlement opened** (55 spellings / 27 pairs whose r4133
value is no `%.Ng` render of ours — `RP39_ROUTING`; none of them in scope today,
which is why the block is a discipline and not a gate failure), and after its own
in-sub-step precondition, the **per-cell narrowing of the 20 mixed echo rows**
(RP2.3's audit settlement, §RP4.1's first paragraph); RP5 is last.
Execution is on a **single branch only — never in parallel
worktrees**: `tests/corpus/ledger.json`, `tests/corpus/manifests/population.lock.json`
and `tests/golden/golden.lock.json` are fail-on-stale and are rewritten by this
plan **and** by the still-open GOLDEN_REBASE WP-G1 sub-steps (G1.2, G1.3d run
independently per STATUS §1) — a sub-step of either plan always rebases onto the
latest `update` before regenerating them. Branch: `r4133-props` off `update`.

| WP | What | Why it is ordered here |
|---|---|---|
| WP-RP0 | Vendor the census evidence + a permanent census knob | every later table row cites this evidence; re-measurement must be a knob, not a scratch test |
| WP-RP1 | Property-table shape closure (2 ports, 2 stubs, 1 allowlist row) | the shape assert (`compare_prop_lists` count/order, `harness/mod.rs:1501-1519`) fails before any value is even compared; shape rows also move `props/` goldens, which must settle before the comparator work reads them |
| WP-RP2 | The channel-aware value comparator (normalization + echo exclusions + display floor) | converts ~99 % of the divergent cells into either a proven-value-preserving compare or a cited, pinned exclusion — the categorical mass must be gone before per-case triage means anything |
| WP-RP3 | Root-cause the four genuine jumps that are not echo | per CLAUDE.md a gap above floor is a bug until proven otherwise; these four must be fixed or pinned before the unmask, not ledgered blind |
| WP-RP4 | The unmask (G1.1's deliverable) + residual ledger triage | only defensible when the residual is small; its kill criterion re-arms G1.1's |
| WP-RP5 | Docs + the restated property argument + closing record | closes the plan |

**Per-sub-step model tiers.** Audit-code and audit-tests are separate agents; the
fix agent runs at the exec tier. Per `PLAN_SEQUENCE.md` §Model-tier protocol,
audits on `opus-xhigh` exec rows are themselves `opus-xhigh`.

| Sub-step | Exec | Audit-code | Audit-tests | Why |
|---|---|---|---|---|
| RP0.1 | `opus-high+` | `opus-high+` | `opus-high+` | evidence vendoring with count cross-checks against a local-only source |
| RP0.2 | `opus-high+` | `opus-high+` | `opus-high+` | census knob cloned from `DSS_LIVE_PROPS` (`corpus_gate.rs:328-416`) |
| RP1.1 | `opus-high+` | `opus-high+` | `opus-high+` | stub rows + one new `PropFlags` bit; parse/JSON semantics patterned on existing flags |
| RP1.2 | `opus-xhigh` | `opus-xhigh` | `opus-xhigh` | real behavioral port (`TAutoTransObj.FetchXfmrCode`, Y-invalidation) + an ordinal shift across a PD class + a golden-lock interaction |
| RP1.3 | `opus-xhigh` | `opus-xhigh` | `opus-xhigh` | real behavioral port (WindGen user-model surface: power flow, dynamics, variables) + a new `dss-usermodel` shuttle + enum widening |
| RP1.4 | `opus-high+` | `opus-high+` | `opus-high+` | allowlist row + upstream report; investigation bounded by a decided outcome set |
| RP2.1 | `opus-xhigh` | `opus-xhigh` | `opus-xhigh` | the design centerpiece: channel threading, value-preserving rule engine, replay accounting, non-loosening argument |
| RP2.2 | `opus-high+` | `opus-high+` | `opus-high+` | per-pair source dossier over an enumerated, closed pair list |
| RP2.3 | `opus-high+` | `opus-high+` | `opus-high+` | echo-exclusion rows, each mechanically citable to an r4133 echo site, plus pins |
| RP2.4 | `opus-xhigh` | `opus-xhigh` | `opus-xhigh` | a new channel-scoped numeric floor — calibration discipline (the eight required components are enumerated in RP2.4 itself) |
| RP3.1–RP3.4 (+ any RP3.5+ opened by RP2.2 or RP2.3) | `opus-high+` | `opus-high+` | `opus-high+` | one root-cause each, bounded surface, live-probe procedure prescribed |
| RP4.1 | `opus-high+` | `opus-high+` | `opus-high+` | flag flip + residual triage (G1.1's own tier) |
| RP5.1, RP5.2 | `opus-high+` | `opus-high+` | `opus-high+` | doc surgery validated by `oracle_parity_cfg_gate.rs` doc tests |

## 1. Plan-wide design decisions

### 1.1 The target, the census, and the treatment map

The target: `compare_all_properties` (`crates/dss-core/tests/harness/mod.rs:1549-1601`)
runs on the **r4133 channel** for every r4133-gating live case — today it is
masked off in both the gate path (`corpus_gate/scheduler.rs:357-363`) and the
seeding path (`scheduler.rs:710-717`), and `force_properties`
(`scheduler.rs:97-114`) opts in `gates_capi()` cases only. Population
(`tests/corpus/manifests/population.lock.json`): 366 `both` + 96 `r4133` + 59
`capi_v0145` — so the unmask gives 462 cases an r4133 property check, of which
the 96 r4133-only cases (WindGen, flicker, the r4133 arms of `protection/`)
currently have **no property check of any kind**.

The evidence is the 2026-08-08 G1.1 census (1 055 880 comparison rows over 438
cases; method and control in `investigations/g1_1_r4133_props/g1_1_r4133_props_triage.md`
— the capi channel was verified unaffected on `controls:regcontrol/regcontrol_sym.dss`).
Restricted to r4133-gating cases: 390 divergent cases, 198 structural pairs
(920 954 cells), 53 numeric pairs (90 627 cells), 149 shape rows over 3 classes
(generator 137, autotrans 7, windgen 5 — Sensor's 61 and GenDispatcher's 48
shape rows sit entirely on capi-only cases; the full-census shape total is 429
over 5 classes).
**RP0.2 re-census correction (2026-08-22, STATUS §RP0.2):** the 2026-08-08 walk
itself was incomplete in two places — `regcontrol.fwdthreshold` (a whole bin-5
echo pair, 888 cells; r4133 `''` echo like its three `idle*` siblings, port
renders `'100'`/`'800'`) is missing, and the two cursor-disagreement
transformers were dropped whole-element (+2 cells on 7 structural + 4 numeric
`transformer.*` pairs, no new pair). Corrected full-census totals: structural
**210 pairs / 961 031 cells**; the vendored extracts stay frozen at 209 —
corrections ride as notes on the affected rows below and in the vendored
`README.md` §"Corrections measured after freezing".
**WP-RP1 shape-closure correction (structural, not an error in the evidence):**
while a class carried a `shape_count` row its property walk stopped at the
name-list disagreement, so the 2026-08-08 census could not record value pairs
for the props behind it. Every RP1 sub-step therefore **creates** pairs no
frozen row can contain, and must re-measure with the RP0.2 knob and record its
own new pairs in the vendored `README.md` §"Pairs the WP-RP1 shape closures make
live" (RP2.1 provisions `examples_supplement.txt` from those records; RP2.2's
"closed pair list" is `bins.tsv` **plus** that section). As executed for
**RP1.1** (2026-08-22, full re-run, 438 cases): structural 218, numeric 98
(against the frozen 209 / 94, or the knob's pre-RP1.1 210 / 94), shape classes
5 → 3 — **12 new pairs**, eleven of them in bins 1/2/5/6 which the existing
machinery covers, plus `generator.d` (`'1'` vs `'0'`, rel 1.00e+00) in **bin 7**:
the in-scope bin-7 population grows 16 → 17 and this section's enumerated
four-root-cause list is no longer closed. Its cause is already read off the
Pascal and is an **echo**, not a jump
(`Create` sets `GenVars.D := 1.0` and never `Dpu`, `generator.pas:955-971`;
`InitPropertyValues` echoes `Format('%-g', [GenVars.Dpu])`, `:2585`), so it is
RP2.2 triage → an RP2.3 echo row, not an RP3 root-cause sub-step.
**CORRECTION (2026-08-23, RP2.3 part A finding F1; the kill ruling's R2,
user-approved).** The two sentences above are right about the routing and wrong
about the mechanism, and the difference matters. The property's field is
`GenVars.Dpu` — what `Edit` writes (`generator.pas:669`) and what
`InitPropertyValues` snapshots (`:2585`) — while `Create` initializes
`GenVars.D` (`:969`) and **never** `Dpu`. So the frozen `'0'` is not a stale
store at all: it is r4133's own live value, and `InitStateVars` then recomputes
`D := Dpu*kVArating*1000/w0 = 0` (`:2710`), discarding `Create`'s 1.0 and running
generator dynamics **undamped** against the property's documented default
("Default is 1.0", `:467`). That is an upstream initialisation bug, not an echo;
dss_capi 0.14.5 fixed it (`Dpu := 1.0`, `src/PCElements/Generator.pas:1006`) and
the port follows. The pair still lands in RP2.3, but as an
`EchoCategory::LiveSemanticsDiffer` row with an expected-value pin
(`generator_d_renders_the_documented_damping_default`) and an upstream report
(`investigations/to_opendss/42-generator-dpu-never-initialized.md`). **Closed out
2026-08-23** across all four sub-steps: 12 (RP1.1) + 9 (RP1.2) + 2 (RP1.3) +
1 (RP1.4) = **24 new pairs**, structural 210 → 225 and numeric 94 → 103, shape
classes 5 → 0.
The full-census bins below are re-derived in-scope by RP0.1 (its `bins.tsv`
assigns every pair to its bin); the treatment map binds every bin to the
sub-step that closes it:

| # | census bin (pairs / cells, full census) | root cause | treatment | lands in |
|---|---|---|---|---|
| 1 | boolean rendering, 75 / 297 593 | FPC `Yes`/`No` vs eleven non-empty Delphi spellings (`true/True/false/False/YES/yes/no/NO/n/y/Y` — `False` alone is 76 492 cells); the `''` renders in this bin are echo-defaults, not booleans | `BoolFold` rule; echo cells go to the echo table. **RP0.1 census correction (2026-08-22, STATUS §RP0.1):** nine bin-1 pairs answer with an echo, not three — four genuinely mixed (`recloser.eventlog`, `regcontrol.idle`, `relay.distreverse` with `''`; `relay.reset` with the non-empty parse string `'0.20'`) and five pure-echo with no foldable cell at all (`regcontrol.idleforward`, `regcontrol.idlereverse`, `capcontrol.reset`, `recloser.debugtrace`, `upfccontrol.enabled`), so only 70 pairs take a `BoolFold` row — §1.2 replay bullet | RP2.1 / RP2.3 |
| 2 | case-only + trailing space, 59 / 72 007 + 2 / 21 206 | THashList lowercasing (port = dss_capi) vs Delphi as-declared case; literal `'wye '`/`'Delta '` (`Transformer.pas:1762-1763`, `AutoTrans.pas:1818-1819`) | `CaseFold` + trim | RP2.1 |
| 3 | enum spelling + singletons, 8 / 4 400 | per-pair enum spellings (`Positive`/`Pos`) and per-pair semantics. **RP2.2 correction (2026-08-23):** `monitor.mode`'s `'1 16 +'` is NOT a "decomposition render" — it is the deck's own RPN **source text** (`mode=(1 16 +)`) echoed back with the parser's parens stripped (`Meters/Monitor.pas:359`, no `GetPropertyValue` override), i.e. an `EchoParse`. And the bin's 8 pairs split 4/4: only the source sequence selectors are synonyms | `EnumSynonym` rows + the S6 dossier (S6 = the triage's per-pair singleton list, enumerated exhaustively in RP2.2) | RP2.2 |
| 4 | array form, 21 / 122 554 | dss_capi `GetDSSArray` `[ 400]` vs Delphi comma/paren/bare forms | `ArrayForm` tokenizing compare | RP2.1 |
| 5 | empty-vs-value + display defaults, 44 / 442 369 (**45 / 443 257** after the RP0.2 correction above — the 45th pair is `regcontrol.fwdthreshold`, 888 cells, absent from every vendored extract; RP2.3 provisions its row from the RP0.2 record) | `PropertyValue[]` echo: un-overridden `GetPropertyValue` returns the parse store / `InitPropertyValues` default (`DSSObject.pas:112-115`; e.g. `Reactor.pas:1087-1140`, `Transformer.pas:1914-1919`; `RegControl.pas:1423-1459` initializes only `PropertyValue[1..32]`, so props 33–36 all echo `''`) | `PROPS_ECHO_R4133` exclusion rows + pins | RP2.3 |
| 6 | numeric display precision, 61 pairs full / 37 in-scope | Delphi `%-.5g`/`%-.6g`/`%-.8g` getters (`Vsource.pas:1327-1343`); measured worst rel 6.43e-5 (`load.pf`) — RP2.4 re-derived the family as `Format('%[-].Ng')`, N ∈ {4,5,6,7,8}, the worst cell being a `%-.4g` (`Load.pas:2345`), and the in-scope pair count as 43; see its as-executed note and `tests/TOLERANCE_NOTES.md` | the r4133 props display floor | RP2.4 |
| 7 | genuine value jumps, 33 pairs full / 16 in-scope | 12 of the 16 are echo (frozen defaults: `transformer.pctperm/repair`, `fault.pctperm`, `gictransformer.pctperm`, `reactor.kvar`, `pvsystem/storage.%pminnovars/%pminkvarmax`, `invcontrol.lpftau/risefalllimit`, `regcontrol.remoteptratio`); 4 need root-cause (`swtcontrol.delay`, `windgen.kvar`, `generator.model`, `gictransformer.r2`) | echo rows / root-cause | RP2.3 / RP3 |
| 8 | property-table shape, 5 classes / 429 rows | 2 real port gaps (AutoTrans `XfmrCode`, WindGen `UserModel`/`UserData`), 2 upstream stubs (Generator `Rneut`/`Xneut`, Sensor `action`), 1 r4133 registration bug (GenDispatcher `weights`) | ports / stub rows / allowlist row | RP1 |

A load-bearing fact the whole design rests on (re-derived from the census over
the **in-scope** pairs — the numbers come from the census itself, not from a
triage section): the display-precision population and the genuine-jump
population are separated by an **empty band** — the worst in-scope display pair
peaks at rel 6.43e-5 and the smallest in-scope genuine jump starts at 1.00e-3;
no in-scope pair has its maximum inside (6.43e-5, 1e-3). It is an in-scope
statement: out of scope, capi-only `storagecontroller.kwneed` peaks at 1.38e-3
yet is display-class — exactly why RP2.4 derives the floor from the in-scope
extract only. The floor sits inside the band, so it absorbs display truncation
while every known genuine jump still fails.

Where our gate is already stronger — nothing changes: the capi_v0145 property
compare stays byte/skeleton-exact at the case tier floors (`mod.rs:202-234`,
`:1523-1528` "case-exact"), `PROPS_015X`/`SKIP_PROPS`/`LANE_SKIP_PROPS` keep
their exact semantics, and the 366 `both` cases keep their full-precision capi
property witness — the r4133 compare is **additive**.

Mechanics for every sub-step: **(a) evidence-first** — every row of every table
this plan introduces (normalization rule, echo exclusion, enum synonym, shape
allowlist) traces to a vendored census extract row (RP0.1) and, where the row
asserts an upstream mechanism, to an r4133 `Version8/Source` unit:line; no row
without evidence. Live r4133 probes throughout this plan follow the epri-worker
procedure: the bridge and its knobs are documented in `tools/opendss/README.md`
and TESTING.md §Procedures; the `epri-worker` binary is built by `cargo test`
itself (`crates/dss-epri`, Windows-only). **(b) capi-invariance** — no
mechanism of this plan may alter the capi_v0145 property compare; every new
behavior is gated on the r4133 channel at the comparator seam (the
`EngineChannel`-visibility trap is §1.2's first bullet), and RP2.1 proves
invariance with an A/B run. **(c) value-preserving normalization only** — a normalization rule
may change how a value is spelled, never which value it is (the
`lane::expected_rerounded` discipline, `lane.rs:546-592`); anything that cannot
satisfy that is an **exclusion**, and every exclusion whose ours-value has no
capi witness (r4133-only classes/cases) carries its own expected-value pin.
**(d) liveness both ways** — every table row must be exercised: offline by the
RP2.1 replay-accounting test (value rows over the vendored in-scope pair
extracts; shape-allowlist rows over the full `shape.txt`, so capi-only classes
count), and live (after RP4.1) by per-row hit accounting with a fail-on-stale
assertion (the `REROUND_VISITS`/`REROUND_HITS` pattern, `lane.rs:281-317`) —
the live accounting covers the two new tables
(`PROPS_NORM_R4133`/`PROPS_ECHO_R4133`) only; `PROPS_015X` shape rows carry no
live counters (none exist today, `mod.rs:1425-1468`) and rely on the replay's
offline arm — plus a count lock per table (the `props_roundtrip.rs:66-68,238`
pattern). **(e) the ledger stays small** — per-case `ledger.json` entries only
for genuine one-off divergences no table covers, each with r4133 evidence and
its pin (`property` scopes are divergence-only and per-case by design,
`ledger.rs:439-449`, `:974-1105`). The entry shape to draft is the existing
`makeposseq-cuf-applied-capi-props` entry in `tests/corpus/ledger.json` —
`match: [{field: "property", name_re, rust, oracle}]` rows + `cause_ref` +
`source`; a new cause needs its own `causes` row. **Staging rule:** a `property`-scoped entry
on the `r4133` channel is applied only by the r4133 props compare
(`property_handled_keys`, `ledger.rs:974-1105`, reached from the props path,
`runner.rs:629`), which stays masked until RP4.1 — landing such an entry
earlier fails `assert_all_hit` (NEVER APPLIED, `ledger.rs:308-367`) in plain
`cargo test`. Therefore every RP1–RP3 sub-step **drafts** its r4133 property
entries (exact JSON recorded verbatim in the sub-step's STATUS record) and
lands its pins immediately; the entries themselves land in RP4.1's unmask
commit, where fail-on-stale validates them at once.
**(f) acceptance, every sub-step**: the touched comparator path is proven
non-vacuous (a deliberately corrupted Rust value still fails, run once in a
scratch tree), `population.lock.json` and/or `golden.lock.json` are regenerated
in the same commit whenever the ledger, a manifest key or a locked golden moved,
every new floor has its derivation in `tests/TOLERANCE_NOTES.md`, and each
sub-step's own numeric stop-and-report threshold holds.

### 1.2 The machinery this plan introduces

- **Channel threading.** `compare_all_properties` grows a channel parameter
  (it is channel-blind today, `mod.rs:1549`; the channel is already in scope at
  both gate call sites, `runner.rs:340,629,654`, and the `DSS_LIVE_PROPS` pilot
  passes capi, `corpus_gate.rs:416`). Visibility trap: `EngineChannel` is
  `pub(crate)` to the `corpus_gate` binary (`corpus_gate/manifest.rs:151`)
  while `harness/` compiles into ~20 other test binaries that lack that module,
  so the harness-side parameter is a **harness-local** channel type
  (`PropsChannel` — the G1.1 scrap's own solution) or the `compare_eventlog`
  pattern (`oracle_spec: Option<&str>`, `mod.rs:1697`); the corpus_gate call
  sites map `EngineChannel` into it. Wherever this plan says "`EngineChannel`"
  at a harness seam, read "the channel". The normalization
  hook sits in `compare_prop_lists` immediately before
  `assert_value_matches_tol` (`mod.rs:1529`) — the function already takes the
  shape allowlist as a parameter for testability (`mod.rs:1470-1477`), and the
  new policy travels the same way. The two **whole-element skips** inside
  `compare_all_properties` — Recloser and Relay, justified today by their
  r4133-shaped Rust tables vs the 0.14.5 capture (`mod.rs:1552-1573`) — become
  **channel-scoped**: skip on the capi channel only; on r4133 both classes
  compare fully (the justification inverts — the tables match r4133).
  Otherwise the census's 22 relay/recloser pairs would be dead on the live path
  and the RP4.1 closure unreachable. The channel audit does not stop there:
  `skip_prop` (`mod.rs:1344-1353`, consulted at `:1520`) is channel-blind, so
  after RP4.1 every `SKIP_PROPS` row (`mod.rs:1261-1332`) would value-mask the
  r4133 channel too. RP2.1 therefore disposes **every** `SKIP_PROPS` /
  `LANE_SKIP_PROPS` row for r4133: the changed-default rows — (Fuse,
  FuseCurve), (Fuse, RatedCurrent), (RegControl, RevThreshold) — must
  **compare** on r4133 (`tests/TOLERANCE_NOTES.md:951-956` pins the r4133-side
  values and forbids masking them there); rows justified by channel-independent
  facts (heap-garbage matrix reads) stay skipped on both channels;
  `LANE_SKIP_PROPS`'s (Monitor, BaseFreq) stays deliberately channel-blind —
  r4133 shares the bug (`Monitor.pas` r4133:552). Each disposition lands in the
  row's comment. These cells are invisible in the census (its walk ran with the
  skips active), so the rows unmasked for r4133 are validated by the RP4.1 live
  run, not by the replay.
- **`PROPS_NORM_R4133`** (new harness module `tests/harness/props_norm.rs`) — the
  channel-scoped value-normalization table, rows keyed `(class, prop)` with a
  typed rule: `BoolFold` (fold `{yes,y,true}` / `{no,n,false}`,
  case-insensitive; `''` is NOT a boolean), `CaseFold` (case-insensitive compare
  + leading/trailing whitespace trim), `ArrayForm` (tokenize both sides —
  strip `[]()`, split on commas/whitespace — numbers compared at the channel
  floor, tokens case-folded), `EnumSynonym` (an explicit per-row spelling map,
  each mapping citing the r4133 source line that prints it). Applied **only**
  when the channel is `R4133`. Modeled on `EVENTLOG_MASKS`
  (`mod.rs:1643-1689`) for channel keying and per-row documentation, on
  `expected_rerounded` (`lane.rs:546-592`) for the value-preservation contract.
- **`PROPS_ECHO_R4133`** (same module) — the echo-exclusion table: `(class,
  prop)` rows, value-only skip on the r4133 channel (name/order still checked —
  exactly `SKIP_PROPS`'s shape, `mod.rs:1261-1317`, but channel-scoped), each
  row carrying a category tag (`EchoDefault` / `EchoParse` /
  `LiveSemanticsDiffer`) and the r4133 citation that proves it (the missing
  `GetPropertyValue` arm and/or the `InitPropertyValues` line). Rows whose
  ours-value has **no** capi witness (the class or every affected case is
  r4133-only) must name their expected-value pin test in the row comment.
  Since RP2.3's audit settlement two riders make the pair-scoped shape honest:
  `ECHO_CARVE_OUTS` takes a measured cell back out of a row whose citation does
  not explain it (never a shape heuristic — the `(rust, r4133)` spelling is
  matched exactly, and the cell is re-declared to whoever owns it), and the
  "r4133-only **cases**" half of the pin rule is enforced from the measured
  `ECHO_ROWS_ON_R4133_ONLY_CASES` rather than applied by hand.
- **The replay-accounting test** (new integration test
  `crates/dss-core/tests/props_r4133_replay.rs`) — reads the vendored census
  extracts (RP0.1) and pushes every **example row** of `examples_full.txt` —
  plus the RP2.1-vendored `examples_supplement.txt` (the two
  `regcontrol.fwdthreshold` spellings the 2026-08-08 census missed, RP0.2
  correction, **and every pair the WP-RP1 shape closures made live** — the
  spellings recorded per sub-step in the vendored `README.md` §"Pairs the WP-RP1
  shape closures make live", which WP-RP1 closed out at **24 pairs**: 12 from
  RP1.1 (generator/sensor), 9 from RP1.2 (autotrans), 2 from RP1.3 (windgen),
  1 from RP1.4 (gendispatcher) — matching the census deltas, structural
  210 → 225 and numeric 94 → 103. RP1.4's is `gendispatcher.enabled`, whose
  class is reached through a `PROPS_015X` row rather than a port, so a replay
  that accounts only for `oracle_only` closures would miss it. Without these the
  replay would report full offline coverage over a population that excludes the
  very classes RP1 opened) —
  through the full r4133 policy in the documented chain order (shape allowlist
  → normalization → echo table → display floor). `examples_full.txt` carries
  one row per **distinct (rust, r4133) spelling** per pair, untruncated (the
  pair files' own example columns are cut at ~34 chars and cannot feed a
  tokenizing or numeric compare; a single example per pair would hide the
  **mixed pairs** — measured by RP0.1: four bin-1 pairs mix foldable boolean
  spellings with echo cells (`recloser.eventlog`, `regcontrol.idle`,
  `relay.distreverse` with `''`; `relay.reset` with the parse string `'0.20'`),
  and 17 structural pairs overall are bin-heterogeneous, 15 in scope — the
  vendored `README.md` carries the full table). The test asserts every example
  row is claimed by the **first matching mechanism in the chain**, and that
  each example row's claim is admissible for the row's OWN class under the
  chain — never merely for the pair's `bins.tsv` label, which is the
  representative-first-cell summary and under-describes the 17 heterogeneous
  pairs (a bin-1 mixed pair legitimately holds a `BoolFold` row and an echo
  row; the five pure-echo bin-1 pairs hold only an echo row; single-claim is
  per example row, never per pair), and that every
  table row claims **at least one** example row (both-ways liveness, offline —
  no oracle needed; shape-allowlist rows are exercised against the full
  `shape.txt`, capi-only classes included — for the shared `PROPS_015X` table
  only the rows **this plan adds** are in the accounting, the pre-existing
  0.15.x rows answer to `props_roundtrip`/goldens, not to `shape.txt`). The
  contract is **per-example-row offline, per-cell live**: the replay proves
  spelling-level completeness before the unmask; cell-level closure is proven
  at RP4.1 by the census knob's disposition mode (RP0.2/RP2.1) over the live
  capture. This is the completeness proof that precedes the unmask, and the
  anti-rot guard afterwards.
- **The r4133 props display floor** — a named, channel-scoped relative floor
  applied by `compare_prop_lists` to numeric cells on the r4133 channel only
  (**`2e-4`, derived and landed by RP2.4** inside the measured empty band
  `(6.431124e-05, 1.374769e-03)`; `props_norm::R4133_DISPLAY_FLOOR`, derivation
  in `tests/TOLERANCE_NOTES.md`). It is **not** a `tol_for` tier change and
  touches no `Tolerances`
  field — the capi channel keeps `tol.i_rel/i_abs` (`mod.rs:1596-1597`)
  untouched.
- **Shape allowlisting reuses `PROPS_015X`** (`mod.rs:1400-1468`). The G1.1 text
  expected "a `PROPS_R4133` shape-allowlist sibling"
  (`GOLDEN_REBASE_PLAN.md:390-392`); that expectation is **superseded** here in
  favor of the standing TOLERANCE_NOTES doctrine ("Rather than mint parallel
  `PROPS_R4133` / `HIDE_R4133` mechanisms, the *identical* Rung-1 machinery is
  reused", `tests/TOLERANCE_NOTES.md:943-956`). Mechanically safe: `prop_015x`
  drops a Rust-side prop only when the oracle's own name list lacks it
  (`mod.rs:1495-1500`), so a `("GenDispatcher", &["weights"])` row is inert on
  the capi channel (whose list contains `weights`) and active on r4133 (whose
  list omits it — the registration bug, RP1.4). The row's doc comment records
  the r4133-bug provenance, distinct from the 0.15.x rows.
- **Golden-lock interaction rule for RP1.** The `props/` class goldens are
  committed byte artifacts under `tests/golden/golden.lock.json` anchors; RP1's
  table additions move `props/{generator,sensor,autotrans}.json` (and possibly
  `json/` schema artifacts). WindGen has **no** `props/` golden and none is
  created — the class has no capi channel to capture from; its coverage is
  RP1.3's unit pins plus the r4133 channel after RP4.1. The procedure is
  GOLDEN_REBASE §1.2 R2: fix → pin the intended new rows with an expected-value
  test → regenerate per-family with `DSS_UPDATE_GOLDENS=1` → the diff moves
  ONLY the predicted cells; the lock edit (anchor/reason update for each moved
  artifact) **and** the `props_roundtrip.rs` count-lock moves the new rows
  force (`PROPS_PROPERTY_CELLS`/`PROPS_SCENARIOS`, `props_roundtrip.rs:62-68`)
  are part of the same commit and named in the commit body. No other golden
  byte moves in this plan.

### 1.3 What is deliberately OUT of scope

- **The 59 capi-only cases' r4133 property behavior.** Sensor value rows, the
  GenDispatcher deck family, `modes:makeposseq/*` and the other
  `engines: "capi_v0145"` cases stay uncompared on r4133 (their `engines` key is
  the owner of that decision, per-case, in the manifests). Nothing is lost: the
  capi channel gates them fully. RP1.4 may flip one GenDispatcher deck to
  `both` — that single flip is in scope; a broader `engines` re-audit is not.
- **WindGen models 3 and 7.** r4133 dispatches models 1–7
  (`WindGen.pas:2109-2118`); the port admits 1/2/4/5 plus (after RP1.3) 6. No
  corpus deck sets 3 or 7 and no plan owns them — RP1.3 records the deferral as
  a named row in the existing root `ORPHANED_GAPS.md` (verified 2026-08-22: no
  WindGen row exists yet — add one, do not create a new file).
- **Report/Dump/Save rendering.** This plan touches only the live
  `all_properties` compare; the `Dump`/`Save` text paths and their goldens stay
  GOLDEN_REBASE territory (WP-G3/G4).
- **The `props/` golden migration to self-anchors.** Stays GOLDEN_REBASE G3.5;
  this plan only edits locks where RP1 shape changes force it (§1.2 last
  bullet). G3.4/G3.5 unblock when RP4.1 lands — that dependency is recorded in
  both plans and in PLAN_SEQUENCE (rows added by this plan's authoring commit;
  RP5.2 flips the state to COMPLETE).
- **No tolerance tier moves.** `tests/TOLERANCE_NOTES.md` tiers and every
  `Tolerances` field are untouchable; the RP2.4 floor is a new named
  channel-scoped parameter with its own derivation, not an edit to any existing
  floor, and it is *never* consulted on the capi channel.

---

## WP-RP0 — Evidence base + census rails

Bookkeeping shared by both sub-steps: the census source of truth until RP0.1
lands is local-only (`investigations/g1_1_r4133_props/`, five extracts +
270 MiB full census); nothing in `tests/corpus/props_r4133/` may disagree with
the numbers already recorded in STATUS.md §1 and this plan §1.1 — a mismatch is
a stop-and-report, not a silent re-derivation. **Acceptance criterion for the
whole WP:** the vendored extracts' data-row counts equal the §1.1 pair/shape
counts and `bins.tsv`'s per-bin totals equal the §1.1 bin counts, the in-scope
re-derivations carry their filter definition in-file, and the census knob
reproduces the vendored rows for a spot-checked family on both channels (in the
family-filtered sense RP0.2's acceptance defines).

### RP0.1 — vendor the census extracts

Copy the five small artifacts (24 944 bytes total) from the local
`investigations/g1_1_r4133_props/` (source files carry a `g1_1_` prefix; strip
it) into `tests/corpus/props_r4133/`: `triage.md` (the 146-line report
verbatim), `summary.json`, `structural_pairs.txt` (209 data rows + 1 header),
`numeric_pairs.txt` (94 data rows + 1 header), `shape.txt` (5 rows); derive a
sixth, `examples_full.txt` — one row per **distinct (rust, r4133) spelling**
per pair (≥ 209 + 94 rows; pipe-separated like the pair files:
`class.prop | rust | r4133 | count` — a pair's cells may collapse to several
spellings, and the mixed pairs of §1.2 need every spelling represented — **as
executed:** 3 378 rows, and the mixed-pair set measured NINE bin-1 echo pairs,
four of them mixed, not three; see the §1.2 correction),
with the **untruncated** rust/r4133 values from the local census (the pair
files cut examples at ~34 chars; the RP2.1 replay needs whole values); derive
a seventh, `bins.tsv` — one row per census pair (209 + 94), assigning it to
exactly one §1.1 bin 1–7 (bin 8 is the shape file), the assignment rule
spelled out in `README.md` — this is the file the §1.1 sentence "re-derived
in-scope by RP0.1" and the RP2.1 replay's admissible-claim assert consume; add
`README.md` with provenance (census of 2026-08-08 at
`golden-g1` 67a0910e's parent state, method = the triage §Method, the full
270 MiB census stays local-only and re-derivable via RP0.2) and the **in-scope
re-derivations** `structural_pairs_in_scope.txt` / `numeric_pairs_in_scope.txt`
(filter: cases whose manifest `engines ∈ {both, r4133}`; expected 198 and 53
rows — derive from the local full census, record the exact filter, the census
case-label → manifest `path` join rule, and the counts in README). Known data traps to preserve in README: element names may contain `|`
(`Line.b1||b2` in `modes:reduce/reduce_mergeparallel.dss` — naive pipe-splits
break), and `numeric_pairs.txt` example columns are truncated to ~34 chars
(full values only in the census). **Acceptance:** data-row counts and
`bins.tsv` per-bin totals match §1.1 exactly; the files are plain data (no
compat aliases — keeps the `oracle_parity_cfg_gate` corpus walk quiet).
Outcome: the plan's evidence is in-repo and reviewable, 209 + 94 + 5 rows plus
the untruncated example set and the bin assignment.

### RP0.2 — the permanent census knob

Make the G1.1 scratch census a permanent opt-in diagnostic: `DSS_PROPS_CENSUS=1`
on the corpus gate walks every live case (respecting `DSS_GATE_ONLY`), captures
`all_properties` on **both** channels regardless of the §1.1 masks, compares with
the plain (un-normalized) comparator in collect-don't-panic mode, and writes
`tmp/props_census.json` + the four pair/shape extracts in the RP0.1 format
(**as executed:** every live non-`large` case; the census is its own armed
`#[test]`, the extracts are per-channel and `examples_full.txt` is emitted too,
plus a `run.json` stamping any `DSS_GATE_ONLY` filter — `bins.tsv`/`*_in_scope`
stay RP2.1's, needing the bin policy; full record in STATUS §RP0.2). This
plain mode is the knob's baseline forever (it is what reproduces RP0.1); RP2.1
later adds a second, **disposition** mode (`DSS_PROPS_CENSUS=claims`) that runs
the same walk through the full r4133 policy and annotates every divergent cell
`normalized-by-<rule> / echo-row / under-floor / ledger-hit / UNCLAIMED` — the
per-cell accounting RP4.1's acceptance reads. The claims mode calls the **same
policy seam** the live gate uses (`compare_prop_lists`' normalize → echo →
floor → ledger chain) in collect mode — never a parallel reimplementation (a
drifting copy would corrupt RP4.1's zero-UNCLAIMED read silently).
Implementation seam: `DSS_LIVE_PROPS` (`corpus_gate.rs:328-416`) contributes
the capture/report shape only — that path is capi-only today
(`corpus_gate.rs:345-360`) and stays untouched; the two-channel walk itself
lives in the scheduler (`scheduler.rs:655` iterates both channels per case,
`ctx.channel(uc, ch)` `:256` builds each; `DSS_GATE_ONLY` filtering lives
there too, `scheduler.rs:448`), which is where the knob hooks. Seeding stays
blind to r4133 props until RP4.1 (`scheduler.rs:714-717`) — this knob is the
measurement tool in the meantime. TESTING.md §Environment variables gains the
row. **Acceptance:** a bounded run (`DSS_GATE_ONLY` on one family) writes
per-cell rows that byte-match the local full census's rows for that family
(compared **order-invariantly**, as cell multisets: the census's
representative-cell artifacts — a pair extract's `example`, a `bins.tsv`
label — are case-order-dependent on the 17 heterogeneous pairs the vendored
`README.md` tables, so a moved label there under a different walk order is
expected, not a regression; what must match is the cell population);
the vendored extracts (which aggregate across families) are cross-checked
where a pair's population lies entirely inside the chosen family — pick a
family where some do, e.g. `modes:windgen` — and the knob asserts nothing (a
divergence never fails it). If RP0.2 runs **before** RP0.1 (the reorder
branch — local evidence lost), its acceptance is instead two-run
byte-reproducibility of the knob's own output. Outcome: census re-measurement
is one env var, not a scratch test.

---

## WP-RP1 — Property-table shape closure

Every sub-step here regenerates the affected `props/` golden(s) under the §1.2
golden-lock interaction rule (predicted-cells-only diff, lock edit in the same
commit) and regenerates `population.lock.json` when a manifest key moves.
**Acceptance criterion for the whole WP:** a census re-run (RP0.2 knob) shows
**zero** `shape_count` rows on r4133-gating cases (in-scope 149 → 0; the full
census closes 429 → 0 modulo the RP1.4 allowlist row, Sensor's 61 and
GenDispatcher's 48 rows sitting on capi-only cases), and the capi channel's
property compare is bit-for-bit unchanged (A/B run on one family). **Per
sub-step, one more obligation** (§1.1 "WP-RP1 shape-closure correction", opened
by RP1.1's audit round): re-run the knob over the **full** population and record
the value pairs the closure made live — pair, both spellings, cells, bin — in
the vendored `README.md` §"Pairs the WP-RP1 shape closures make live", so RP2.1's
supplement and RP2.2's pair list stay complete.

### RP1.1 — Generator `Rneut`/`Xneut` + Sensor `action` upstream-stub rows

Add the three property rows the port lacks, with r4133's exact semantics:
Generator `Rneut`/`Xneut` at display slots 16/17 (after `Conn` — r4133 registers
them via `AddProperty('Rneut', 14, 'Removed due to causing confusion …')`,
`Version8/Source/PCElements/generator.pas:441-442`; the display position
follows registration order — `Conn` is the 15th name, Rneut/Xneut the
16th/17th; the binding order authority is the census `shape.txt` name list,
pinned by this sub-step's unit test; the write path stores the
string and logs soft errors 5611/5612, `generator.pas:625,651-652`; no field, no
Y effect — the neutral stamping is dead text, `generator.pas:1296-1303`;
defaults `'0'`, `:2567-2568`); Sensor `action` at slot 13
(`Version8/Source/Meters/Sensor.pas:183`, help says "NOT IMPLEMENTED"; empty
setter `:850-854`, silent store `:253`, default `''`, `:807`). Neither
`PropFlags::NOT_PORTED` fits (it hard-errors and hides the row from JSON,
`obj/props/prop_flags.rs:31`, `class_props/parse.rs:32-37`, `json.rs:45-51`) —
add a new flag (working name `UPSTREAM_STUB`): the row is listed, the value is
stored and echoed, parsing never fails; a per-class side-effect hook lets
Generator log its two soft messages while Sensor stays silent (no such
mechanism exists today — design a minimal one, e.g. an optional callback
carried by the flag row; keep it unit-covered). Ordinal shifts:
Generator `NUM_PROPS` 48 → 50 (`elements/pc/generator/mod.rs:145,151-237`),
Sensor 15 → 16 (`elements/meter/sensor/mod.rs:39-54,62-86`); walk every
`prop::` consumer, `dump.rs`/`save.rs`, and the `debug_assert_eq!(defs.len(),
NUM_PROPS - 1)` locks. The three new rows join `PROPS_015X` (the capi oracle's
tables lack them — same relief as the existing 0.15.x rows, `mod.rs:1495-1500`).
**Acceptance:** parse/echo/log semantics pinned by unit tests per class
(including "write does not change behavior" — Y and solution untouched by a
`rneut=` edit, proven on a micro deck); affected `props/` goldens regenerated
under the §1.2 rule. Outcome: 273 + 61 full-census shape rows (generator +
sensor) close — in-scope that is generator's 137; Sensor's 61 sit entirely on
capi-only cases (their closure is witnessed by the capi channel and the full
census, not the r4133 gate).

### RP1.2 — AutoTrans `XfmrCode` (real port)

Port property 39 `XfmrCode` (`Version8/Source/PDElements/AutoTrans.pas:329`,
help `:414`) and `TAutoTransObj.FetchXfmrCode` (`AutoTrans.pas:2339-2396`): on
edit, resolve the named `XfmrCode` (miss → soft message 100180, nothing else)
and copy the electrical model — per winding the **autotrans-specific connection
override** (winding 1 → `SERIES`, winding 2 → `WYE`, else the code's
connection), `kVLL`/`VBase`/`kVA`/`puTAP`/`Rpu`, `RdcOhms` **with
`RdcSpecified := TRUE`**, tap limits/counts; then `SetTermRef`, the reactance
name remap `XHL/XHT/XLT → puXHX/puXHT/puXXT`, `puXSC` copy, thermal/loss/ppm
fields, `Yorder` recompute, `YPrimInvalid := True`, `RecalcElementData`
(`:2360-2395`; Edit arm `:520`, Edit tail recalc `:584`). The Transformer analog
exists (`pd/transformer/windings.rs:357-402`, accessors
`transformer/accessors.rs:594-609`, prop row `transformer/mod.rs:81,159`) but **cannot be reused verbatim** — it copies
windings wholesale and copies 0.15.x kVA-ratings (`windings.rs:362,398-399`),
both wrong for the AutoTrans r4133 form; write the AutoTrans variant
field-by-field. Ordinal shift: insert at 39, `NUM_PROPS` 52 → 53
(`elements/pd/auto_trans/mod.rs:88-106`), fix the module docs that declare the
absence (`auto_trans/mod.rs:11-13,47-49`, `dump.rs:4`). **Do not reproduce**
r4133's contradictory second arm — `'XFmrCode Property not used with AutoTrans
object.'` (`AutoTrans.pas:567`) fires even though arm `:520` applied the code;
that message is an upstream bug (report it, `investigations/to_opendss/`); if
its absence is observable on a gated surface (error/event log), exclude + pin
per the standing rules. The new row also joins `PROPS_015X` (absent from the
0.14.5 table, `.inputs/dss_capi/src/PDElements/AutoTrans.pas:76,125`).
**Measure-first:** no corpus deck sets `xfmrcode=` on an AutoTrans today — add
one (extend an existing `asymmetric:autotrans/*` deck or a sibling; new-deck
procedure per TESTING.md §Procedures — manifest entry,
`corpus_manifest.rs::every_dss_is_accounted_for_exactly_once`,
`population.lock.json` regen in the same commit), validate it on the r4133
channel live (epri-worker, §1.1(a)), and prove the port against the oracle's
solved state on that deck. `lane_diff` run (solved state moves). **Acceptance:**
the new deck's Y/voltages/currents match r4133 at tier floors; a unit test pins
the connection override (winding 1 forced SERIES even when the code says
otherwise); `props/autotrans` golden regenerated under §1.2. Outcome: 42 full-census /
7 in-scope shape rows close; AutoTrans gains a working `XfmrCode` for the
first time.

### RP1.3 — WindGen `UserModel`/`UserData` (real port over the WASM host)

Port properties 18/19 (`Version8/Source/PCElements/WindGen.pas:391-394`; Edit
arms `:641-642`; `MakeLike :829`; `GetPropertyValue 18 :2903`; defaults `''`
`:2453-2454`) and the model-6 behavior they feed: power flow `DoUserModel`
(`WindGen.pas:1875-1898` — missing model → soft message 567), dynamics
(`:1990-2000` — missing model → message 5671 + `SolutionAbort`), `InitStateVars`
(`:2564-2574`), `IntegrateStates` (`:2663`), `RecalcElementData`'s
`FUpdateModel` (`:1418`), and the variable surface (`:2735-2876`). Build on the
WASM user-model host: `generator/user_model.rs` (`GenUserModelSlot`, WM.3) over
`crates/dss-usermodel` exposes exactly the needed call surface; WindGen needs
its own shuttle (`WindGenVars.pas` is a different record shape from
`TGeneratorVars`) and a test guest fixture (the WASM_USERMODELS guest-build
pattern — the existing fixtures and drivers live in
`crates/dss-usermodel/tests/` (`fixture_pin.rs`, `fixture_self_gate.rs`) and
`crates/dss-epri/tests/gen_wasm_usermodels*.rs`; clone that arrangement for
the WindGen shuttle). Deliberately keep `ShaftModel`/`ShaftData` absent — r4133 has the
field but registers no property for it (`WindGen.pas:100`, no `PropertyName`
row). Widen `windgen_model` to admit 6 (`obj/dss_enum/registry/pc.rs:205-218`
`&[1,2,4,5]`; dispatch `windgen/solve.rs:306-312`); models 3/7 stay out
(§1.3, ORPHANED_GAPS row). Fix the module doc that cites the un-vendored
"0.15.x cleaned" source (`windgen/mod.rs:1-2` — re-derive the narrowed-model
statement from r4133 + this plan instead). Ordinal shift: insert at 18/19,
tail +2, `NUM_PROPS` 46 → 48 (`windgen/mod.rs:56-102`). WindGen has **no capi
channel** (the class does not exist in 0.14.5) — the property rows join no
allowlist; the behavior is gated by unit tests + the wasm guest, the shape by
the r4133 channel after RP4.1. `lane_diff` run (dispatch arm added).
**Acceptance:** with no user model set, all `modes:windgen/*` decks are
bit-identical to before (the new arms are dormant — prove with an A/B state
dump via `crates/dss-core/examples/lane_dump.rs`, the same dump the
`lane_diff` job drives: diff the two checkpoint streams); the guest fixture exercises calc/init/integrate/variables round-trip;
no `props/` golden exists for WindGen and none is created (§1.2 — no capi
channel to capture from). Outcome: 5 census shape rows close; the port's
WindGen surface matches r4133's table.

### RP1.4 — GenDispatcher `weights` — allowlist row + upstream report

Zero engine change (the Rust side is correct and matches capi:
`gen_dispatcher/mod.rs:63-67,90`, dispatch `compute.rs:28-79`). r4133 loses the
`Weights` NAME to a registration off-by-one — `NumPropsThisClass = 6` but
`PropertyName^[7] := 'Weights'` (`Version8/Source/Controls/GenDispatcher.pas:92,133`),
so `TCktElementClass.DefineProperties` overwrites slot 7 with `basefreq`
(`Common/CktElementClass.pas:93-106`) and `AllPropertyNames` returns 9 names
without `weights`; worse, the command list is built from the overwritten names
(`GenDispatcher.pas:105`), so on r4133 `basefreq=` parses as a weights vector.
Write the upstream report (`investigations/to_opendss/` — the fix is
`NumPropsThisClass = 7`), add the `("GenDispatcher", &["weights"])` row to
`PROPS_015X` with the r4133-bug provenance comment (§1.2), and **decide the
liveness question by measurement**: all GenDispatcher decks are
`engines: "capi_v0145"` today, so the row would be dead on the live gate. Probe
the three decks (`controls:gendispatcher/*`) on the r4133 channel (epri-worker):
if the known `generator.kw/kvar` dispatch deltas (census tail rows 7/10)
root-cause to a pinnable r4133 behavior difference, flip **one** deck to
`engines: "both"` (pins land here; any r4133 `property`-scoped ledger entries
are drafted here and land at RP4.1 per the §1.1(e) staging rule; lock regen in
the flip commit); if they are unpinnable, keep the decks capi-only and mark the
allowlist row's comment "dormant until a gendispatcher deck gates r4133" — no
live-accounting exemption mechanism is needed, `PROPS_015X` rows carry no live
counters (§1.1(d)); either way the replay test exercises the row offline via
the full `shape.txt`. **Acceptance:** the report exists; the row exists with
either outcome recorded in the comment and STATUS; no silent third state. Outcome: the census's one `rust_only` shape row is explained and
guarded.

---

## WP-RP2 — The channel-aware value comparator

**Acceptance criterion for the whole WP:** the replay-accounting test passes
with every in-scope example row claimed by exactly one mechanism — the first
match in the chain (§1.2; a mixed pair may hold a norm row and an echo row;
shape rows are RP1's; the 16 genuine-jump pairs may be claimed by
declared-pending RP3 markers until RP3 lands), every table row live in the
offline accounting, and the capi channel proven untouched (A/B run, §1.1(b)).

### RP2.1 — channel threading + the normalization engine + replay accounting

Thread the channel into `compare_all_properties` and `compare_prop_lists`
(seams, call sites and the `EngineChannel`-visibility trap in §1.2 — the
harness-side type is harness-local; the capi call sites pass the capi channel
and compile-time-trivially keep today's behavior), make the Recloser/Relay
whole-element skips channel-scoped per §1.2 (capi-only; on r4133 both classes
compare fully), and land the §1.2 row-by-row r4133 disposition of
`SKIP_PROPS`/`LANE_SKIP_PROPS` (changed-default rows compare on r4133;
channel-independent rows stay skipped on both; each row's comment records its
disposition). Introduce
`tests/harness/props_norm.rs` with the four rule kinds (§1.2) and the initial
row set for bins 1/2/4 of §1.1: `BoolFold` (the 70 bin-1 pairs with foldable
cells — the five pure-echo bin-1 pairs take NO `BoolFold` row and go to RP2.3
wholesale; RP0.1 census correction, §1.1 bin-1 row),
`CaseFold` (+trim; the 59 case-only pairs + the 2 trailing-space pairs),
`ArrayForm` (the 21 array-form pairs — note `sensor.kvs` is out of scope,
capi-only, and its value delta is real; the in-scope extract governs). Every row
cites its census line; rules are typed, not regexes. Implement the per-row hit
accounting (dormant until RP4.1 flips the live path on) and the **count locks**
(both-ways equalities, `props_roundtrip.rs:238` pattern). Land the
replay-accounting test (`props_r4133_replay.rs`, §1.2): it is the sub-step's
own proof of completeness for bins 1/2/4 — example rows it cannot claim yet
(bins 3/5/6/7) are asserted to match the vendored `bins.tsv` assignment (the
pair sets RP2.2/RP2.3/RP2.4/RP3 own), so the accounting is total from day one
and later sub-steps only move pairs between mechanisms, never invent them.
**RP0.2 correction — the replay's input is `examples_full.txt` PLUS a
supplement.** The vendored extracts have no `regcontrol.fwdthreshold` rows
(the pair is missing from the 2026-08-08 census; RP0.2 STATUS record, finding
1), so RP2.1 vendors `tests/corpus/props_r4133/examples_supplement.txt` — same
row format, the two knob-measured spellings `regcontrol.fwdthreshold | '100' |
'' | 864` and `| '800' | '' | 24` (Σ 888), provenance header citing the RP0.2
record — extends the evidence lock with its row count, and feeds the replay
from both files. The pair carries no `bins.tsv` row either: the supplement's
provenance IS its bin-5 assignment (declared for RP2.3's echo row, like the
other bin-5 pairs); the both-ways liveness assert counts supplement rows
exactly like `examples_full.txt` rows, so RP2.3's `fwdthreshold` echo row is
live offline, not exempted.
Claim semantics are per example row, first-match-in-chain (§1.2) — the four
mixed pairs keep their echo example rows unclaimed here (declared for RP2.3's
echo rows: `''` on three of them, `'0.20'` on `relay.reset`) while their
foldable rows are claimed by `BoolFold`; the five pure-echo bin-1 pairs'
example rows are declared for RP2.3 wholesale. Extend the RP0.2 census knob with the
**disposition mode** (`DSS_PROPS_CENSUS=claims`, spec in RP0.2): the same live
walk annotated with the policy claim chain — the per-cell accounting tool
RP4.1's acceptance reads. **Non-vacuity:** a corrupted boolean
(`Yes` vs `false` where the oracle says `true`), a corrupted token inside an
array, and a wrong number at 1e-2 must all still fail through the normalized
path (scratch probe). **Capi-invariance:** run one family's gate before/after on
the capi channel — bit-identical outcomes. **Kill criterion:** any census row in
bins 1/2/4 that the typed rules cannot claim without weakening (c) — stop and
report; that row belongs to another bin or to a new category this plan must
name. **Acceptance:** the non-vacuity and capi-invariance probes above pass,
the replay-accounting test is green with bins 1/2/4 fully claimed, and the
count locks hold. Outcome: ~491 000 in-scope cells (bins 1/2/4) become a
value-preserving compare instead of a mask.

### RP2.2 — enum synonyms + the S6 dossier

Close bin 3 (the eight pairs, all enumerated in the census: `vsource.scantype/
sequence` `Positive`/`Pos`, `isource.scantype/sequence` `Positive`/`pos`,
`swtcontrol.action` `close`/`open`, `monitor.mode` `17`/`1 16 +`, `line.units`
`none`/`kft`, `storagecontroller.modedischarge` `Schedule`/`UNKNOWN`) plus the
S6 singletons the triage flagged for individual source investigation
(`relay.normal/state` and `swtcontrol.state/normal` per-phase array renders,
`invcontrol.monbus/monbusesvbase/pvsystemlist/vsetpoint/monvoltagecalc`,
`isource.bus2/yearly`, `reactor.bus2`, `load.yearly`,
`line.linecode/spacing/units`, `expcontrol.derlist`). For each pair, read the
r4133 getter/echo site and classify into exactly one of: `EnumSynonym` row
(spelling of the same value — cite the r4133 line that prints it),
`PROPS_ECHO_R4133` row (echo/default — hand to RP2.3's table with the citation),
or **suspected engine/behavior divergence** (a hand-off opens a numbered
RP3.5+ sub-step at the RP3.1–RP3.4 tier row, recorded in STATUS; RP4.1 does
not start until it closes, §0 — `swtcontrol.action` `close`/`open` and
`line.units` `none`/`kft` are the two candidates). **Kill criterion:** any pair
that fits none of the three after reading its source — stop and report (a new
category or a port bug). **Acceptance:** the replay accounting claims bin 3
fully; every row cites its source line. Outcome: the eight bin-3 pairs and the
S6 list each have a cited, single classification.

**As executed (2026-08-23, STATUS §WP-RP2).** The kill criterion did not fire.
Bin 3 split 4/4: `vsource`/`isource` × `scantype`/`sequence` took `EnumSynonym`
rows, while `swtcontrol.action` (an `EchoParse` stored before the CASE and left
stale by the `Locked` refusal — the plan's first RP3.5 candidate, **disproven**),
`monitor.mode` (the RPN source-text echo — see the §1.1 bin-3 correction) and
`storagecontroller.modedischarge` (`'UNKNOWN'` is the non-injective catch-all of
`GetModeString`) went to RP2.3, and `line.units` (the plan's second candidate,
**confirmed**) opened RP3.5. Of the S6 list, 10 pairs went to RP2.3, 4 to RP3.7,
1 (`line.linecode`) opened RP3.6, and 2 (`invcontrol.monbus`/`monbusesvbase`)
were already fully claimed by RP2.1's `ArrayForm` rows. **Closing bin 3 also
meant closing its cells:** three bin-2-labelled pairs carry enum-spelling cells
(vendored `README.md` §"A pair's bin is a label"), which the pair list does not
name — `capcontrol.type` and `fault.bus2` are echoes (RP2.3) and
`invcontrol.voltage_curvex_ref` is a live enum getter, so its whole pair was
re-typed from `CaseFold` to a fifth `EnumSynonym` row. Two RP2.1 hand-offs also
closed here: `generator.dynout` is RP2.3's (`LiveSemanticsDiffer` + pin, upstream
report written) and `Fault.GMatrix` stays value-skipped on both channels with no
sub-step.

**Audit settlement (2026-08-23, one follow-up commit).** Its two audits raised 8
findings, 6 after dedup (2 major, 4 minor); 5 fixed, 1 recorded. The one
substantive addition: reading the whole `swtcontrol.normal`/`state` Edit arm — as
this sub-step's brief required — surfaced a **live** r4133 divergence the dossier
had not recorded, the port refusing a `normal=` write while `Locked` where r4133
applies it (§RP3.7 (a2), probe-confirmed). It is an engine fix, so RP2.2 recorded
it into RP3.7's scope instead of landing it; nothing else in the routing moved.

### RP2.3 — the echo-exclusion table + pins

Land the `PROPS_ECHO_R4133` rows for bin 5 (the 44 empty-vs-value pairs — echo
defaults and never-parsed stores — **plus the 45th, `regcontrol.fwdthreshold`**:
missing from the vendored census, measured by RP0.2's re-census at 888 cells,
`EchoDefault` like its three `idle*` siblings — r4133 `RegControl.pas:1423-1459`
initializes only `PropertyValue[1..32]` and `GetPropertyValue` overrides only
index 28, so props 33–36 all echo `''`; its example rows come from
`examples_supplement.txt`, vendored by RP2.1; its in-scope cell split is not
in the frozen extracts — re-derive it with the knob when writing the pin)
and the 12 echo-rooted genuine-jump pairs of
bin 7 (§1.1 table; the census grounding proves each against its r4133 site:
`Transformer.pas:1914-1919` pctperm/repair, `Reactor.pas:1087-1140` kvar echo,
`RegControl.pas:1452` remoteptratio frozen default (`PropertyValue[27]`; the
live value re-inits at `:484` while the store stays `'60'`), the `%pmin*`/`lpftau`/
`risefalllimit`/`pctperm` `InitPropertyValues` defaults), minus whatever RP2.2
already routed here (RP2.3 runs after RP2.2, §0), plus the echo rows of the
**nine** bin-1 echo pairs (RP0.1 census correction, §1.1 bin-1 row / §1.2
replay bullet): the four mixed pairs — `recloser.eventlog`, `regcontrol.idle`,
`relay.distreverse` (`''` echoes) and `relay.reset` (the non-empty stale parse
string `'0.20'` — an `EchoParse` row, not `EchoDefault`) — where the chain
order does the per-cell **attribution** (foldable cells are claimed by
`BoolFold` before the echo row is consulted, so only the echo cells are credited
to the echo row; the shipped exclusion is still pair-scoped — see the
as-executed correction below and §RP4.1's precondition), and the five pure-echo
pairs `regcontrol.idleforward`,
`regcontrol.idlereverse`, `capcontrol.reset`, `recloser.debugtrace`,
`upfccontrol.enabled` (no foldable cell at all — the six pairs beyond the
originally-named three carry 2 125 in-scope echo cells that would otherwise
reach the RP2.1 liveness assert unclaimed). Every row: category tag + r4133 citation + the witness
statement — either "capi channel pins the live value on N `both` cases" or, for
r4133-only coverage, the named expected-value pin test this sub-step adds
(pattern: `newton_powers_match_the_normal_algorithm`-style pins in the exec
tests — assert the port's live value on the exact deck the census flagged).
Expected magnitude: ~50–70 rows; the replay accounting fixes the exact count as
the table's count lock. **Kill criterion:** any candidate row that cannot be
cited to a concrete r4133 echo site (a missing `GetPropertyValue` arm — the
`DSSObject.pas:112-115` fallthrough — or an `InitPropertyValues` line) — stop
and report; it may be a port bug wearing an echo costume. **Acceptance:** every
row cited; every no-capi-witness row has its pin; replay accounting green.
Outcome: ~443 000 in-scope cells become documented, pinned exclusions instead
of silence.

**As executed (2026-08-23, STATUS §WP-RP2).** **The kill criterion FIRED**, on
six of the 86 bucket pairs, and the user ruled both halves
(AskUserQuestion, 2026-08-23): the five dss_capi-0.14.5 `SilentReadOnly`
surfaces (`indmach012.pf`, `storagecontroller.kwhtotal`/`kwtotal`/`kwhactual`/
`kwactual`, **1 064 cells / 772 in scope**) take **no echo row** — r4133 renders
a live computed read-only value there and the port renders `''` only by a
capi-lineage convention, so under the 2026-08-02 policy the fix is an engine
change and they are re-routed to the new **§RP3.8** below (`RP38_ROUTING` /
`DECLARED_RP38 = (181, 5, 181)` in the replay, blocking RP4.1 per §0); and
`generator.d` lands as `LiveSemanticsDiffer` + pin + upstream report rather than
the echo this section's §1.1 note called it (the mechanism correction is at that
note).

**81 rows landed** (86 − 5), split 50 `EchoDefault` / 7 `EchoParse` / 14
`EmptyCollectionRender` / 10 `LiveSemanticsDiffer`. The count is above the
~50–70 estimate for a measured reason, not scope creep: the bucket the RP2.1
replay had already *declared by name* holds 86 pairs — bin 5's 43 (of 44;
`line.linecode` went to RP3.6) + bin 7's 12 + bin 1's 9 + six bin-2-labelled
pairs whose `''`-echo cells the chain routes here + bin 3's 3 + bin 4's 2 + the
supplement's 11 — and RP2.3's job was to close exactly that set. **One new
category** was needed and added, `EmptyCollectionRender`: 14 pairs where r4133's
getter arm is LIVE and merely renders the other empty-collection convention
(`'[]'`/`'()'`/`''` for an unset array), so all three original tags would have
been false.

**Nine new `PROPS_NORM_R4133` rows landed FIRST** (part A finding F4, the
ruling's R3): `load.yearly`/`reactor.bus2`/`invcontrol.monvoltagecalc`
`CaseFold` and `line.wires`/`load.zipv`/`generator.userdata`/`storage.dynadata`/
`storagecontroller.seasontargets`+`seasontargetslow` `ArrayForm`, **6 446 cells
(6 370 in scope)**. The tenth measured pair, `swtcontrol.action`, deliberately
took none (6 foldable cells, **0 in scope**, and the row would have loosened a
live RP2.2 pin for zero live coverage).
**CORRECTION (audit settlement, 2026-08-23):** this paragraph originally said
those nine rows keep their 6 446 cells "inside the value compare that a
pair-scoped exclusion would otherwise have swallowed". They do not, and the
audit proved it through the real comparator: the chain order decides which link
*claims* a cell — the census disposition, and the hit that keeps each of the
nine rows provably live — but the seam's exclusion is asked per PAIR, so on the
20 mixed pairs it drops the value assert for the cells the rule refused as well.
A genuine regression there (a wrong resolved loadshape name, a wrong ZIPV
vector) is caught on the capi channel only. Narrowing those rows per cell is
now an explicit RP4.1 precondition (§RP4.1); the mechanism exists —
`props_norm::ECHO_CARVE_OUTS`, added by the same settlement — and the shipped
behaviour is pinned by
`harness::props_policy_tests::a_mixed_pairs_echo_row_masks_the_cells_its_rule_refuses`.

**Witnesses**: 20 expected-value pins in the new
`crates/dss-core/tests/props_r4133_pins.rs`, covering the 32 rows whose value the
capi channel does not (or must not only) hold. 21 are pin-only — the seven
Recloser/Relay pairs (capi skips those elements whole), the ten
`PROPS_015X`/`SKIP_PROPS` RegControl/Line/Transformer/AutoTrans pairs,
`windgen.dynout` (no capi case holds a WindGen) and the three whose echo cells
are all on capi-only cases; the other 11 carry both witnesses — every
`LiveSemanticsDiffer` row plus `energymeter.peakcurrent`, which this section
names. Each compiles the deck the claims census flagged and
asserts the port's live render literally, with a discriminating second reading;
`props_r4133_replay::every_echo_row_pin_is_a_test_that_exists` reads the names
back both ways.

**Measured (full claims census at HEAD, 439 cases × 2 channels, error baselines
5 r4133 / 22 capi):** `echo-row` claims **488 017 cells (468 044 in scope)** over
169 spellings and all 81 pairs; `UNCLAIMED` falls 545 568 → **51 105**
(521 841 → **47 427** in scope), leaving the RP2.4 display class and RP3's
residue. The capi channel still claims **zero** cells — the exclusion is r4133-only.
(The pre-settlement run measured 488 018 / 170 spellings / 51 104 unclaimed; the
one cell that moved is the `reactor.kvar` carve-out below, which is out of
r4133 scope, so no in-scope number changes.)

**As settled (audit round, 2026-08-23).** Seven findings from the two auditors
(one raised by both), five fixed, one recorded, one refuted — STATUS §WP-RP2
carries the arithmetic. Three things changed beyond wording:

* **`props_norm::ECHO_CARVE_OUTS`**, a per-cell narrowing valve, with one entry:
  `reactor.kvar`'s 607th census cell is not that row's `'1200'` echo but r4133's
  own live `kvarRating` after `MakePosSequence` round-trips it through
  `Format(' kvar=%-.5g')` and the parser (`Reactor.pas:1145-1201`) — a 5.0e-06
  divergence of two live values, i.e. **RP2.4's** display class, where the same
  round-trip's `reactor.kv` already sits (`bins.tsv` bin 6, 5.85e-06). Declared
  to RP2.4 (`ECHO_CARVE_OUT_ROUTING`), so `DECLARED_RP24` is `(2101, 71, 2021)`
  and `CLAIMED_ECHO` 169.
* **Nine more expected-value pins (20 → 29, covering 63 rows)** for mechanic
  (c)'s "r4133-only **cases**" half, which part B2 had applied to
  `swtcontrol.action` alone. Measured: 57 of the 81 rows mask cells on
  `engines: "r4133"` cases (34 969 cells), where the capi channel never runs;
  31 of them had only a `Capi(n)` witness. `props_norm::
  ECHO_ROWS_ON_R4133_ONLY_CASES` records the population and a test enforces the
  rule.
* **The order of the two seams is now asserted at the shipped comparator**
  (`the_normalization_seam_runs_before_the_exclusion_on_a_mixed_pair`), not only
  in the two offline copies of the chain; the two exemption lists
  (`ECHO_ROWS_WITH_NO_IN_SCOPE_CELL`, `NOT_A_PIN`) are pinned literally.

### RP2.4 — the r4133 props display floor

Derive the channel-scoped numeric floor for r4133 property values from the
vendored in-scope numeric extract: the display population's measured worst
(rel 6.43e-5, `load.pf`; the `%-.5g` class ceiling argument per
`Vsource.pas:1327-1343`, with the `%-.8g` sub-family of `puZ*`/`Z*` noted
separately) against the genuine-jump population's floor (1.00e-3,
`invcontrol.lpftau`) — the empty band (6.43e-5, 1e-3) is the calibration's
safety margin; the provisional target is 2e-4 (≈3.1× measured worst, 5× under
the smallest genuine jump). Write the `tests/TOLERANCE_NOTES.md` section with
all eight required components (named exception + exact scope; measured worst +
ratio; mechanism with the Pascal citation; the decomposition argument — the
census itself, two disjoint populations; why no real coverage is lost — the
capi channel keeps full-precision on every `both` case and every known genuine
jump exceeds the floor by ≥5×; scope justification — r4133 channel only, by
construction; fix owner — none, the floor reflects Delphi display formatting,
not a defect; the "what it relaxes / what it never relaxes" pair). Wire it into
`compare_prop_lists` (r4133 branch only) and into `ArrayForm`'s numeric
element compare. **Measure-first:** the final value comes from the extract, not
this paragraph — if the re-derived worst display rel differs from 6.43e-5,
recalibrate and record. **Acceptance:** TOLERANCE_NOTES section complete; a
scratch probe proves a 1e-3 numeric error still fails on the r4133 channel;
capi unchanged. Outcome: the 37 in-scope display pairs (~ the numeric mass
minus the tail) compare within a derived, documented floor.

> **AS EXECUTED (2026-08-23, one commit — WP-RP2 closes with it).** Floor
> **`2e-4`**, the provisional target confirmed by measurement, but two of this
> paragraph's inputs were **recalibrated** and one was incomplete:
>
> * *worst display rel* — **confirmed exactly**: 6.431124e-05, `load.pf`
>   `'0.747651914485831'` vs `'0.7477'`, 33 cells, in scope. Re-measured live
>   over the full 439-case census: same cell, same number.
> * *smallest genuine jump* — the "1.00e-3, `invcontrol.lpftau`" above is a
>   number in the **census's** metric, which reports the ABSOLUTE difference when
>   the expected side is 0. Under the floor's symmetric metric that cell
>   (`'0.001'` vs `'0.0'`) is rel **1.0**; a 0-vs-nonzero pair can never be
>   claimed. The real neighbours are 1.374769e-03 (`storagecontroller.kwneed`,
>   6.874× over the floor), 4.404256e-03 (`generator.kvar`, the nearest genuine
>   value jump, 22.02×) and 5.524501e-02 (`regcontrol.remoteptratio`, the
>   smallest **in-scope** jump, 276.2×). The safety band is therefore
>   `(6.431124e-05, 1.374769e-03)` — **empty, 21.38× wide**, not `(6.43e-5, 1e-3)`.
> * *mechanism* — "`%-.5g` per `Vsource.pas:1327-1343` with a `%-.8g` sub-family"
>   is **incomplete, not wrong**: the family is `Format('%[-].Ng')` with
>   N ∈ {4,5,6,7,8}, and the worst cell is a **`%-.4g`** (`Load.pas:2345`), which
>   this paragraph does not name. All five formatters and their sites are tabled
>   in `tests/TOLERANCE_NOTES.md` §"r4133 props display floor". Its `%-.4g`
>   class ceiling (5e-4) does **not** fit the band, so the floor is derived from
>   the measured population (293 `load.pf` spellings, min mantissa 5.653 →
>   8.845e-05) with the residual stated, not absorbed.
> * *outcome* — **43** in-scope display pairs offline, not 37: the 37 in-scope
>   bin-6 pairs of `bins.tsv` **plus** 5 WP-RP1 supplement bin-6 pairs
>   (`generator.kva`/`maxkvar`/`minkvar`, `autotrans.normamps`/`emergamps`) and
>   the `reactor.kvar` carve-out RP2.3 handed over — i.e. this number plus the
>   two post-freeze sources. Live the floor claims **79 pairs / 2 012 spellings
>   / 49 451 cells (46 538 in scope)**, with the in-scope cells landing on **41**
>   of those pairs: `reactor.kvar` and `generator.kva` are claimed only on
>   spellings that sit on `capi_v0145` cases, so they contribute nothing in
>   scope. Per-pair split in the vendored `README.md` §"What RP2.4 moved".
> * *acceptance* — met, and the "scratch probe" was **landed permanently**
>   instead: `props_policy_tests::the_display_floor_drops_only_the_cell_it_claims_only_on_r4133`
>   drives the real `compare_prop_lists`. capi unchanged (census: 0 on every
>   r4133 disposition). `DECLARED_RP24` `(2101, 71, 2021)` → `(0, 0, 0)`, its
>   105-row residual re-declared `OutOfScope` by the cited ceiling proof
>   `RP24_OUT_OF_SCOPE`. STATUS §WP-RP2 carries the full record.
>
> **AUDIT SETTLEMENT (same day, same branch).** Both auditors landed the same
> major: the *mechanism* above was attributed by naming the formatter family, not
> by checking each claimed render against it, and **55 of the 2 006 claimed
> spellings (70 cells, 27 pairs) are gaps no `%.Ng` rounding of our value can
> produce**. The floor's value did not move; its predicate grew the missing
> clause, `props_norm::display_is_render`, so "every claimed cell is a `%.Ng`
> render of ours" is now enforced rather than surveyed. The 55 are declared to
> the new **§RP3.9**, which is an RP4.1 precondition (§0). Consequently the
> numbers in this note read, post-settlement: **1 951** claimed vendored
> spellings, live **69 pairs / 1 957 spellings / 49 381 cells (46 538 in scope —
> unchanged, since not one of the 55 has an in-scope cell)**. The same round
> corrected the `Vsource.pas` line ranges and the `basekv` attribution in the
> mechanism table, replaced "the capi property compare is exact" with the tier
> floors it really runs at, and added six missing guards. STATUS §WP-RP2 carries
> the itemised record.

---

## WP-RP3 — Genuine-jump closure

Each sub-step follows the CLAUDE.md divergence discipline: prove cause before
concluding (live probe via epri-worker / pinned dss-python, decomposition, the
r4133 source), then exactly one outcome — a port bug **fixed in both lanes**, or
an upstream/echo divergence **excluded + pinned** (ledger entry per §1.1(e) or
echo row per RP2.3), or an upstream bug **reported** (`investigations/to_opendss/`)
with its exclusion + pin. r4133 `property`-scoped ledger entries obey the
§1.1(e) **staging rule**: pins land in the sub-step, the entries themselves in
RP4.1's unmask commit — earlier they would fail `assert_all_hit` as NEVER
APPLIED, the r4133 props compare being still masked. **Acceptance criterion for
the whole WP:** none of the four pairs remains unclassified; every outcome has
its artifact (fix commit / drafted ledger entry + landed pin / report + row)
named in STATUS. **RP3.5–RP3.7 were opened by RP2.2's dossier** (§0's "any
RP3.5+ sub-step RP2.2's triage opens"; they are structural/live-state
divergences rather than numeric jumps, so the "four pairs" above stays the
bin-7 statement) and are subject to the same criterion.

### RP3.1 — `swtcontrol.delay` (proven r4133 Edit bug)

r4133 never wires property 5 `Delay` — the Edit `CASE` has no arm 5
(`Version8/Source/Controls/SwtControl.pas:194-217`), so `delay=` lands only in
the echo store while `TimeDelay` stays 120.0 (`:310`) and the **live** getter
renders 120 (`:588`). The divergence is **render-only**: the delay is never
consumed behaviorally in r4133 — `TSwtControlObj.Sample`'s queue-pushing body
is commented out wholesale (`:484-507`, "Removing because action … and lock are
instantaenous"; the pushes at `:492,498` are dead code), `DoPendingAction`'s
likewise (`:396-408`), and `set_States` acts immediately (`:532-549`); the
repo's own manifest validation agrees (control queue and event log empty under
r4133). dss_capi 0.14.5 wires the property
(`.inputs/dss_capi/src/Controls/SwtControl.pas:185`), the port follows capi, and
the manifest already notes the r4133 render on `swtcontrol_time.dss`
(`tests/corpus/controls/manifest.json:967`; the case's `path` at `:952`).
Deliverables: the upstream report (`investigations/to_opendss/` — fix is one
added case arm; describe a silently-ignored property, NOT a queue-delay bug —
the queue sites are dead code), and the property-surface exclusion (a
`PROPS_ECHO_R4133` row is WRONG here — the getter is live; use a per-case
ledger `property` divergence entry with exact pins on the affected r4133-gating
decks, in-scope cells: 24 across 2 cases — `swtcontrol_time.dss` +
`midi_swtcontrol.dss`, 12 cells each — **drafted here, landed at RP4.1**
per the §1.1(e) staging rule). **Acceptance:** pins green in this sub-step; the
drafted entries recorded verbatim in STATUS; the report cites
`:194-217/:310/:588/:484-507`. Outcome: the one live-getter render bug in the
census tail is reported and pinned, not imitated.

### RP3.2 — `windgen.kvar` (r4133-only class, no second oracle)

The census shows ours 986.05 vs r4133 `0` on four of the five `modes:windgen/*`
decks (`windgen_snap.dss` alone does not carry it — use its non-divergence as a
root-cause data point; engines `r4133` — in scope, and no capi witness exists
for this class).
Root-cause with the full discipline: read the r4133 kvar getter/echo site in
`WindGen.pas` (is prop kvar echoed or live?), probe the live r4133 engine state
via a solved-state observable that is *not* the property string (element powers
on the WindGen terminal), and decompose. If r4133's rendered `0` is an echo
(kvar never re-parsed, live Q actually ≈986) → `PROPS_ECHO_R4133` row + the pin
test asserting the port's live kvar on `windgen_daily.dss`. If r4133's live Q
really is 0 while ours is 986 → a behavior divergence in the WindGen port —
**fix it** (both lanes) or, if r4133 is provably wrong against physics +
`WindGen.pas`, report + exclude + pin. **Kill criterion:** the probe cannot
distinguish echo from live (e.g. the DLL exposes no independent observable) —
stop and report with the evidence gathered. **Acceptance:** one of the three
outcomes, with the probe transcript summarized in STATUS. Outcome: the largest
in-scope numeric jump (rel 9.86e+2) is explained.

### RP3.3 — `generator.model` on the NCIM decks

`4` vs `3`, 2 cells — the two NCIM decks that hold a generator
(`modes:ncim/ncim_pv_pq.dss`, `modes:ncim/ncim_midi.dss`; `ncim_pq.dss` is
all-loads, engines all `r4133`). Both decks drive the NCIM PV→PQ conversion:
r4133 switches a Q-limited model-3 generator to model 4
(`Version8/Source/Common/Solution.pas:1935`, `:2120`) and later **takes it
back to 3** (`Solution.pas:1760` — `// Takes it back to model 3`); the port's
NCIM path is `solution/solution/ncim.rs` (model-3/4 participation and the
demote sites, `:275-313`) with pins in `exec/tests/ncim.rs`. Root-cause with
the RP3.1/RP3.2 decision rule: probe the live r4133 model state (epri-worker,
a solved-state observable independent of the property string) and read the
render site — if r4133's live model at render time really is 3 (the `:1760`
restore) while ours stays 4, that is a port behavior divergence — **fix it in
both lanes** (r4133 is the authority); if r4133's live model is 4 and only the
property render echoes the parse store `'3'`, that is an echo —
`PROPS_ECHO_R4133` row (or a drafted ledger entry per §1.1(e)) + the pin
asserting the port's live model on the two decks. **Acceptance:**
classification with citations; artifact landed. Outcome: the one
discrete-state pair in the tail is explained.

### RP3.4 — r4133 twins of the already-pinned capi divergences

The G2.5 fixes are already pinned against the capi channel (5 ledger entries
carry all 11 existing `property` scopes, all `channel: capi_v0145` —
`makeposseq-cuf-applied-capi-props`, `gic-pct-r2-honoured-*`,
`capi-linespacing-normamps`, `capi-generator-makeposseq-rating`). Of those
surfaces, exactly one is in-scope on r4133: `gictransformer.r2` (`0.09522` vs
`0.12696`, 2 cells on `asymmetric:gic/*` — the census shows r4133 renders the
same un-honoured `%R2` echo as capi 0.14.5). Draft the r4133-channel twin
entries with the same cause_ref and land the pins (the `makeposseq`/
`linespacing` surfaces stay capi-only — their decks do not gate r4133; verify
and record); the twins themselves land at RP4.1 per the §1.1(e) staging rule.
**Acceptance:** pins green; the drafted twins recorded verbatim in STATUS; no
duplicate coverage with RP2.3 rows (the replay accounting enforces
single-claim); hit + non-stale is asserted at RP4.1. Outcome: the shared-bug
pins hold on both channels where both channels look.

### RP3.5 — Line length units lost by the matrix-branch merge (opened by RP2.2)

`line.units` `'none'` vs `'kft'` (3 cells, **0 in scope** — the affected
`modes:reduce` cases are `engines: "capi_v0145"`). r4133 renders index 20 from
the live field (`LineUnitsStr(LengthUnits)`,
`Version8/Source/PDElements/Line.pas:1404`), so the two engines hold **different
`LengthUnits`** after a matrix-form `MergeWith` — a live-state divergence, not a
render. r4133 saves the units (`:1627`) and re-applies them in a **separate**
`Length=/Units=` edit *after* the impedance edit, in both merge branches
(`:1721-1726` symmetrical-components, `:1791-1796` matrix), and `MakePosSequence`
re-appends `Units=` for the same reason (`:1596`, "Repeat the Length Units to
compensate for unexpected reset"). The port does that in the sym-components
branch (`exec/reduce.rs:245,348-353`) but **inverts the order in the
matrix-series branch** (`:396-409`: it writes `l.length_units = len_units_saved`
and only then runs the `RMATRIX/XMATRIX/CMATRIX` side effects, which call
`reset_length_units`, `elements/pd/line/accessors.rs:478-486` →
`code.rs:24-28`). Second, independent divergence in the same routine: the port's
`reset_length_units` also clears `user_length_units`, where r4133 deliberately
keeps `FUserLengthUnits` (`Line.pas:2330`, "but do not erase FUserLengthUnits,
in case of CIM export") — a CIM-export-visible difference with no census cell.
**Do first:** a live probe on `modes:reduce/reduce_mergeparallel` +
`reduce/midi_reduce.dss` (epri-worker) — RP2.2 read this off the two sources and
did not build. Then one outcome per the WP rule: a port fix in **both lanes**
with an expected-value pin on the merged line's `length_units` /
`user_length_units`, or a cited exclusion if the probe overturns the reading.
**Acceptance:** the probe recorded; `line.units` either compares or is excluded
with a pin; the `user_length_units` half decided explicitly (it is not
observable in the census, so it needs its own statement). Tier: `opus-high+`.

### RP3.6 — `switch=yes` must not clear the linecode flag (opened by RP2.2)

`line.linecode` `''` vs `'99'`/`'98'` — **5 cells, all 5 in scope**, so this is
the one RP3.5+ pair the RP4.1 unmask will actually compare, and RP4.1 breaks on
it if this sub-step does not land. r4133 renders index 3 live
(`3: If FLineCodeSpecified Then Result := CondCode else Result := ''`,
`Line.pas:1357`); the flag is set by `FetchLineCode` (`:413`) and cleared by the
impedance side effects `6..11, 26..27` (`:685`) and `12..14` (`:691`) — but
**not** by `switch=yes`, whose arm assigns r1/x1/r0/x0/c1/c0/len as fields,
kills geometry and spacing and resets the length units while leaving
`FLineCodeSpecified` TRUE (`:694-700`). The port clears it there
(`elements/pd/line/accessors.rs:488-511`, `SWITCH => { … kill_line_code_
specified(); … }`, following dss_capi 0.14.5's `KillLineCodeSpecified`). Not
cosmetic: the flag selects the `FUnitsConvert` formula on a later `units=`
(`Line.pas:626-627`), and the affected decks
(`Version8/Distrib/Examples/StoCtrl_Current_PeakShave/Line.DSS`) put `units=m`
**after** `Switch=True`, so the two engines take different branches there.
r4133 is the behavioral authority (CLAUDE.md 2026-08-02), so the default
expectation is a port fix in both lanes plus a pin on the resulting
`FUnitsConvert`/impedance; the alternative (keep the kill, pin the r4133-side
delta) must be argued from a probe, not from "capi does it". **Do first:** an
epri-worker probe of `linecode=… Switch=True units=m` reading back `linecode`,
`units` and `r1` on both engines. **Acceptance:** the 5 in-scope cells are
either compared or excluded-with-a-pin; the `FUnitsConvert` consequence measured
either way; the capi channel proven unmoved (0.14.5 keeps its own behavior —
if the port changes, the capi-side delta needs its own ledger/pin decision).
Tier: `opus-high+`.

### RP3.7 — Per-phase switch and relay state (opened by RP2.2)

Two classes, one modelling question. **(a) SwtControl.** r4133 keeps per-phase
state — `FPresentState`/`FNormalState : pStateArray`
(`Version8/Source/Controls/SwtControl.pas:37-38`), allocated per phase
(`:299-305`), settable phase-by-phase from a quoted list (`InterpretSwitchState`,
`:453-480`) or ganged from a bare token (`:433-451`), each phase driving its own
conductor (`set_States`, `:532-549` → `ControlledElement.Closed[Idx]`), applied
per phase by `RecalcElementData` (`:347-355`) — and its getters render one token
per **controlled-element** phase (`:589-599` Normal, `:600-610` State). The port
holds a **single scalar** per field (`elements/control/swt_control/
accessors.rs:126-163`) applied to the whole terminal (`RefAction::
SetSwitchClosed`, `:270-276`). Every r4133 render the census saw is homogeneous,
so no *value* differs today (`swtcontrol.normal`/`state`, 59 cells each, 40 in
scope); a deck writing `state=(open, closed, closed)` would diverge in Y.

**(a2) A second, independent SwtControl defect on the same pair — added by the
RP2.2 audit settlement (2026-08-23) and confirmed by a live r4133 probe.** The
port refuses a `normal=` write while `Locked`
(`elements/control/swt_control/accessors.rs:151-155`, side effect `:244-249`,
pinned by `swt_control/tests.rs::locked_ignores_normal_and_state_writes`),
following the 0.14.5 `ConditionalReadOnly` flag
(`.inputs/dss_capi/src/Controls/SwtControl.pas:159-160`). **r4133 applies it.**
`InterpretSwitchState`'s guard is property-name-conditional — `if Locked and
((LowerCase(property_name[1]) = 'a') or (LowerCase(property_name[1]) = 's'))
Then Exit`, under the comment *"Only allowed to change normal state if locked"*
(`Version8/Source/Controls/SwtControl.pas:416-417`) — and property 6 is
`'Normal'` (`:128`), so Edit arm 6 (`:201-204`) reaches the ganged/quoted writer
and `set_NormalStates` (`:556-561`), which has no lock guard. Probe (vendored
r4133 DLL, `epri-worker`, 3-phase switched line): with `lock=yes`,
`normal=open` moves `Normal` to `[open, open, open, ]` while `state=open` and
`action=open` leave both fields untouched — the guard discriminates exactly as
the source reads. The port's own **Relay** already implements the r4133 rule and
documents it (`elements/control/relay/accessors.rs:416-420`: "`State` writes are
blocked while `Locked`; `Normal` writes are NOT"), which makes the SwtControl
side a port bug rather than a decision. No census cell exposes it (no corpus deck
writes `normal=` under lock), and `docs/upgrade/DIVERGENCES.md` §D12 asserted the
opposite until this settlement corrected it. RP3.7 fixes it in **both** lanes
with the rest of the per-phase work: drop the `Locked` gate on `Normal` in
`set_i32`/`side_effects`, re-point
`locked_ignores_normal_and_state_writes` at the r4133 rule (Normal applies,
State/Action do not), and keep the D12 record honest.

**(b) Relay.** The mirror image: `TRelayObj.GetPropertyValue` 39/40 loops the
**live** `ControlledElement.NPhases` (`Controls/Relay.pas:1407-1428`), while the
port renders its own per-phase array — which it has — without resyncing it to
the controlled element after `MakePosSequence` (1 cell each, 0 in scope,
`modes/makeposseq/makeposseq_ctrl.dss:44`). Decide, with a probe on a
quoted-list `state=` deck: port the per-phase switch state (which also fixes the
render, and fixes Relay's resync), or record the gap in `ORPHANED_GAPS.md` and
carry the render as a cited exclusion + pin. **Acceptance:** the probe recorded
(does r4133 really hold three independent conductor states on a `state=(…)`
deck?); a decision with its artifact; the 80 in-scope `swtcontrol` cells either
compare or are excluded with an expected-value pin on the port's live switch
state; **and (a2) landed in both lanes with its own pin** (a locked `normal=`
applies, a locked `state=`/`action=` does not), with the corrected §D12 record —
(a2) is a fix, not a decision, so it may not be deferred to `ORPHANED_GAPS.md`
even if (a) is. Tier: `opus-high+`.

### RP3.8 — the five read-only text surfaces r4133 renders live (opened by RP2.3)

**Why it exists.** RP2.3's kill criterion fired on five pairs — `indmach012.pf`
and `storagecontroller.kwhtotal`/`kwtotal`/`kwhactual`/`kwactual`, **1 064 cells
/ 772 in scope** — because none of them can be cited to an r4133 echo site.
r4133 renders each one **live**: `IndMach012.pas:1790`
(`Format('%.6g', [PowerFactor(Power[1, ActiveActor])])`) and
`StorageController.pas:991-994`, which call `GetkWhTotal`/`GetkWTotal`/
`GetkWhActual`/`GetkWActual` (declared `:136-139`). The port answers `''`, and
only because dss_capi 0.14.5 flags the properties `[SilentReadOnly,
ReadByFunction]` and leaves their `PropertyOffset` at `-1`, so its
`GetObjPropertyValue` short-circuits before the read function runs — a
convention the port reproduces deliberately at one class-agnostic gate,
`crates/dss-core/src/obj/props/class_props/value.rs:21-27`, over
`PropFlags::SILENT_READ_ONLY` (`obj/props/prop_flags.rs:55-64`; the PropDefs are
`elements/pc/ind_mach012/mod.rs:88` and
`elements/control/storage_controller/mod.rs:231-234`). Under the standing
2026-08-02 policy r4133 is the behavioral authority and a 0.14.5 convention is
not, so the correct endpoint is the engine rendering the value — which RP2.3
could not do (zero product-crate bytes) and therefore re-routed here, loudly:
`props_r4133_replay::RP38_ROUTING` + the `DECLARED_RP38 = (181, 5, 181)` count
lock. **Blocks RP4.1** (§0): those 772 in-scope cells would otherwise land in the
unmask's residual with no owner. User-approved 2026-08-23 (the RP2.3 kill
ruling, R1).

**Scope.** Render the live computed value for the five surfaces, matching r4133's
formatting (`%.6g` for `indmach012.pf`, `IndMach012.pas:1790`; `%-.8g` for the
four StorageController ones, `StorageController.pas:1160-1197` — read all four
bodies rather than assuming one format). **One of them has a side effect worth
naming**: `GetkWhTotal`/`GetkWTotal` take a `Var Sum` and the getter passes the
object's own `TotalkWhCapacity`/`TotalkWCapacity` (`:991-992`), so in r4133
*reading the property writes state* — the port must render the same number
without inheriting that (it is the `VSConverter.GetCurrents` family of hazard,
CLAUDE.md §"Known upstream bugs"). **Decide explicitly
whether the fix is the text surface or the flag**: `SILENT_READ_ONLY` is read in
three places — the `?`/`DumpProperties` render (`class_props/value.rs:25`), the
JSON *export* omission (`class_props/json.rs:44-48`) and the JSON *set* refusal
(`class_props/json_set.rs:76`) — and only the first is what r4133 disagrees with.
The default expectation is therefore a narrowed marker (or a second flag) that
keeps the JSON behavior exactly as it is, so no JSON golden moves; anything wider
must be argued from measurement, not convenience. `SILENT_READ_ONLY`'s other
holders are out of scope and must be shown unmoved.

**Do first:** an epri-worker probe reading all five properties on a solved
`controls:storagecontroller/*` deck and on `controls:fuse/indmach_r4133/
indmach_snap.dss`, so the expected strings come from the r4133 DLL and not from
reading the Pascal alone (the census only measured the port's `''` against
r4133's text; the *numbers* have never been compared).

**Capi side.** 0.14.5 will keep answering `''`, so the change creates a
capi-channel divergence on every `both` case that holds one of these elements:
exclude it field-by-field (a `SKIP_PROPS_CAPI_ONLY` row per pair, with the 0.14.5
`SilentReadOnly` citation) and pin the port's live value with its own
expected-value test — the same shape RP2.3's pins use
(`crates/dss-core/tests/props_r4133_pins.rs`). **Measure golden exposure in this
sub-step**: `?`/`Dump`/`Show`/JSON-export bytes for the two classes, plus
`props_roundtrip`, and regenerate whatever legitimately moves in the same commit
(`golden.lock.json` with it).

**Kill criterion:** if the probe shows r4133's five renders are *not* reproducible
from the port's own state (e.g. a quantity the port does not track, or one that
depends on r4133's fleet-iteration order), stop and report — the outcome is then
a cited exclusion + pin on the r4133 channel too, not a guessed formula.
**Acceptance:** the probe recorded; the five surfaces render the live value in
**both lanes**; the capi-side divergence excluded field-by-field and pinned; the
JSON surfaces proven unmoved (or their goldens regenerated with the argument);
`RP38_ROUTING`'s five pairs re-routed out of `DECLARED_RP38` into the ordinary
claimed accounting (or into a cited exclusion), and the replay's count locks
moved with their deltas stated. Tier: `opus-high+`.
Outcome: the last population RP4.1 has no owner for is owned.

### RP3.9 — the r4133 round-trip residue (opened by the RP2.4 audit settlement)

**Why it exists.** RP2.4's display floor claims a numeric cell only when its
r4133 side is the port's value rounded to the significant digits r4133 printed
(`props_norm::display_is_render`, the mechanism clause the audit round added).
**55 vendored spellings over 27 pairs — 70 cells** — sit inside the floor and are
NOT such a render: the two engines hold doubles further apart than one `%.Ng`
print can account for. Two shapes, both measured:

* a round trip **upstream of a derived quantity** — r4133's `load.kva`
  recomputed from an already round-tripped `pf` (`Load.pas:2352` prints `%-g`,
  i.e. full precision, so both sides carry 15 digits and differ at the 6th),
  `vsource.puz*`/`mvasc*`/`isc3` from a round-tripped Z, `line.b0`/`b1` from a
  round-tripped C, `transformer.normamps`/`emergamps` from a round-tripped kVA;
* a **full-precision getter** where the two engines' state simply differs —
  `capacitor.normamps`/`emergamps` (`Capacitor.pas:1108-1109`, `%g`),
  `reactor.normamps`/`emergamps` (`Reactor.pas:1099-1100`).

The pairs, their row counts, their in-scope split and their r4133 sites are
`props_r4133_replay::RP39_ROUTING`, with the count lock
`DECLARED_RP39 = (55, 27, 19)` and the both-ways proof
`the_display_floors_round_trip_residue_is_owned_by_rp39`. **No cell of the 55 is
in scope today** — the full claims census measures `count_in_scope = 0` on every
one of them — so nothing the gate compares is blocked; what the sub-step
guarantees is that they are on the WP-RP3 work list
(`claims_unclaimed_pairs.txt`) instead of tagged "not a defect, no fix owner",
which is how RP2.4 first filed them.

**Scope.** Per pair (27, but they collapse into ~6 chains — `load.*`,
`vsource.*`, `line.b*`, `reactor.*`, the two amps families, the four
`generator.*`): read the r4133 round-trip chain off the Pascal, decide the one
outcome the WP-RP3 discipline allows, and record it. Expect most to end as
"upstream precision round trip, port is exact" → an expected-value pin naming
both numbers (the `props_r4133_pins.rs` shape) plus, where a cell ever becomes
in-scope, a `property` ledger entry staged per §1.1(e). A pair whose gap turns
out to be a **port** bug is fixed in both lanes like any other.

**Do first:** re-run `DSS_PROPS_CENSUS=claims` and confirm the 55 are still
exactly the residue (the routing asserts it offline; the census says whether any
has become in-scope). Any that has → it is now gate material and leads.

**Kill criterion:** a pair whose r4133 value cannot be reproduced from the port's
own state by any round-trip chain (i.e. the divergence is not display-rooted at
all) — stop and report; it is a numeric divergence of its own class, not RP2.4's
residue, and it needs its own root-cause sub-step.

**Acceptance:** every one of the 27 pairs carries a recorded verdict; the ones
that stay divergent have a landed pin (and a staged ledger entry where a cell is
in scope); `RP39_ROUTING`/`DECLARED_RP39` shrink with their deltas stated in
STATUS. **Blocks RP4.1** (§0). Tier: `opus-high+`.
Outcome: the population RP2.4's mechanism clause refuses has an owner and a
verdict, not a floor.

---

## WP-RP4 — The unmask

### RP4.1 — `all_properties` on the r4133 channel (G1.1's deliverable)

**Precondition added by RP2.3's audit settlement (2026-08-23) — narrow the 20
mixed echo rows per cell before the unmask.** `PROPS_ECHO_R4133` is pair-scoped
(the shape §1.2 prescribes), and on the 20 pairs that also hold a
`PROPS_NORM_R4133` row that is wider than each row's citation: the typed rule's
refusal of a cell — a wrong resolved loadshape name, a wrong ZIPV vector, a
wrong `Bus2` terminal spelling — is a genuine divergence, and the moment this
sub-step unmasks the path it will pass silently on r4133 (the capi channel is
the only live witness meanwhile, and eight of the nine pairs the settlement
looked at carry a `Capi(n)` witness that says nothing about `engines: "r4133"`
cases). Two admissible fixes, both cited-and-measured: extend
`props_norm::ECHO_CARVE_OUTS` with the cells a row must not cover, or give each
mixed row its measured echo spellings and match on them. Landing either moves
`CLAIMED_ECHO`/`MULTI_LINK_ROWS` and flips the first assertion of
`harness::props_policy_tests::a_mixed_pairs_echo_row_masks_the_cells_its_rule_refuses`,
which is written to be replaced rather than deleted. Do it **before** the flip:
afterwards the same gap is a green gate that proves less than it says.

Stop masking properties on r4133: remove the per-channel clear in the gate path
(`corpus_gate/scheduler.rs:357-363`) and the seeding path (`scheduler.rs:710-717`),
extend `force_properties` to r4133-gating cases (`scheduler.rs:97-114` — keep
the `!c.kind.starts_with("large")` cost guard; note the request now costs an
extra per-case `?`-sweep on the r4133 worker), and fix the stale doc claims the
masks leave behind — at minimum four (`scheduler.rs:97-101` "the r4133 bridge
has no all-properties capture" — false since `capture.rs:619-664`; TESTING.md
§PROPS_015X "runs on the capi_v0145 channel only"; the capture doc
`crates/dss-epri/src/capture.rs:626-629` "NOT compared for gating"; the
`PROPS_015X` SwtControl row comment `harness/mod.rs:1452-1458` "r4133-oracle
cases do not property-compare") plus an `rg` sweep for stragglers. **Land the
ledger entries staged by RP1.4/RP3 per the §1.1(e) staging rule** —
fail-on-stale validates them in this very commit. Then run the seeding report
(`DSS_GATE_SEED_LEDGER=1`) and the census knob at HEAD: triage the **residual**
per-case divergences (whatever RP1–RP3 did not absorb) into `ledger.json` with
pins per §1.1(e); regenerate `population.lock.json` in the same commit; the
live hit accounting for `PROPS_NORM_R4133`/`PROPS_ECHO_R4133` (dormant since
RP2.1) turns on with the unmask, and it asserts **globally** that the r4133
props compare visited at least one case per full-gate run (`visits > 0`, not
the visits-gated silent form; the assert lives beside the tables in
`props_norm.rs` and is invoked once from the corpus gate's epilogue, the way
`lane.rs`'s reround accounting is checked) — `population.lock.json`
fingerprints manifest flags and per-case ledger tags but no scheduler code
(`population_lock.rs:86,143` the flag; `:130,:151` the ledger tag), so a
scheduler-side re-mask is invisible to the lock itself; this assertion plus
the landed property entries (which would scream NEVER APPLIED — and whose
removal would trip a lock diff via the ledger tag) are what make a wholesale
re-mask loud. **Kill criterion (G1.1's, re-armed):** more
than ~15 residual ledger entries — the entries staged by RP1.4/RP3 do **not**
count (they are pre-triaged, cited and pinned; the criterion counts entries
born from this sub-step's own residual triage) — or any residual that cannot
be pinned → stop and report; that magnitude means WP-RP2/RP3 missed a category and the plan
needs a revision, not a bigger ledger. **Acceptance:** full five-command gate
green in both lanes; the census knob's **disposition mode** (RP0.2/RP2.1,
`DSS_PROPS_CENSUS=claims`) at HEAD reports **zero UNCLAIMED cells** (every cell
passes, normalizes, hits an echo row, sits under the floor, or hits a ledger
entry — the per-cell accounting closes; cost: one extra two-channel props walk
over the 462 gating cases, same order as a normal corpus-gate run — if a
single run is impractical, run per-family under `DSS_GATE_ONLY` and aggregate
the disposition files, recording the aggregation in STATUS); the capi channel
A/B is bit-identical.
Outcome: the 96 r4133-only cases get a property check for the first time, and
the 366 `both` cases get their r4133 property table checked — G1.1's outcome,
delivered.

---

## WP-RP5 — Documentation and the restated property argument

### RP5.1 — operational docs

TESTING.md: a new section beside §PROPS_015X documenting the r4133 property
policy (the normalization table, the echo table, the display floor, the census
knob, the replay test, the `SKIP_PROPS` r4133 dispositions of §1.2 — each with
its liveness/count-lock guarantee), the
corrected §PROPS_015X wording (RP4.1), the new env var rows (RP0.2), and the
"triage a property divergence" delta (normalize → echo → floor → ledger, in
that order). `tests/TOLERANCE_NOTES.md`: cross-check the RP2.4 section against
the final landed values. CLAUDE.md: no new section — the standing policy block
already covers this plan; verify the gate section needs no change. The
`oracle_parity_cfg_gate.rs` operational-docs tests must stay green
(`operational_docs_cite_the_compat_machinery_accurately` and the corpus/tools
walks — the new tables are harness code, not compat aliases). **Acceptance:**
every doc claim added here cites the landed code line; doc tests green.
Outcome: the machinery is discoverable without reading this plan.

### RP5.2 — closing record

`PLAN_SEQUENCE.md`: flip this plan's entry to COMPLETE with date and final
counters (tables' row counts, ledger delta, the 462-case outcome); confirm the
GOLDEN_REBASE entry's G1.1 note points here. `STATUS.md`: final counters, the
recorded `lane_diff` runs (RP1.2, RP1.3), the condensed per-sub-step record
(the GOLDEN_REBASE WP-G2 condensed-record shape), and the frontier hand-back to
GOLDEN_REBASE WP-G1 (G3.4/G3.5 unblocked). Verify the §1.3 deferrals landed
their rows (`ORPHANED_GAPS.md` WindGen 3/7). Move this plan to
`docs/plans-archive/` (the 2026-07 convention). **Acceptance:** no doc
disagrees with any other on counts or state; clean tree. Outcome: the plan
closes and GOLDEN_REBASE resumes with G1.1 satisfied.

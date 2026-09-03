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
> *(**RP4.1 landed 2026-09-03** — G1.1's deliverable shipped and its kill
> criterion did not fire, so G3.4/G3.5 are unblocked; §RP4.1's as-executed notes
> and `STATUS.md` §RP4.1.)*
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
triage opens, **§RP3.8, which RP2.3's kill criterion opened** (landed 2026-09-02: the
engine renders all five live and its 1 064 frozen cells are accounted
`RP38_SUPERSEDED` / `SUPERSEDED_RP38` in the replay) **and §RP3.9,
which RP2.4's audit settlement opened** (landed 2026-09-02: all 27 pairs settled
`PRECISION_ROUNDTRIP` and pinned, `OPEN_RP39 = (0, 0, 0)`) (55 spellings / 27
pairs whose r4133 value is no `%.Ng` render of ours — `RP39_ROUTING`; none of
them in scope today, which is why the block was a discipline and not a gate
failure) **and §RP3.12, which RP3.9's own P0 open item opened** (landed
2026-09-03: the 34 `controls:autotrans/*` `wdgcurrents` cells settled
`UPSTREAM_BUG` — r4133 never taps an AutoTrans — never reproduced, zero
product-crate lines; it added no precondition to the flip either, all four of
its decks being capi-only), and after its own
in-sub-step precondition, the **per-cell narrowing of the 20 mixed echo rows**
(RP2.3's audit settlement, §RP4.1's first paragraph); RP5 is last.
*(**All of that ordering is discharged: RP4.1 landed 2026-09-03** with both
preconditions met — the 20 mixed echo rows narrowed per cell, the eight staged
`property` entries landed — and every RP1–RP3 sub-step it waited on landed
before it. §RP3.11 followed on 2026-09-03 (`KEEP_LIVE_PINNED`, both surfaces);
§RP3.10 remains, outside this rule, as the next paragraph says.)*
**Two RP3 sub-steps are deliberately outside that rule** — three since
2026-09-03, when the plan owner accepted §RP3.13 (last sentence of this
block). **§RP3.10** (the
reproduced `QMode=0` dispatch, opened by RP3.2's audit settlement) is a
solve-side fix with no property cell of its own — our `kvar` render reads
`kvar_base`, which the dispatch never writes — so it blocks **§RP5.2**, the
closing record, and not the unmask; it also runs only on the user's go-ahead
(§RP3.10). **§RP3.11** (the `Save`/`Dump` re-serialization surface, opened by
RP3.3's audit settlement) is the mirror image: no *compared* channel reads it at
all, so it cannot block a gate flip — it blocks **§RP5.2** too, and it ran after
RP4.1 had fixed which pairs are echoes, because that list is exactly the list of
properties where the two serializers disagree. *(**Executed 2026-09-03** —
`KEEP_LIVE_PINNED` on both surfaces, `97107e54` + `0194b086`; see the dated
lines in §RP3.11. That §RP5.2 precondition is therefore **discharged**, leaving
§RP3.10 as the closing record's only open blocker.)* **§RP3.13** (the two NCIM
port bugs, opened by RP3.11's own P0 findings) is the third: it moves no
property cell at all — both defects are in the NCIM solve and its reporting arms
— so it blocks neither the unmask nor §RP5.2, and it ran on the user's go-ahead
straight after RP3.11. *(**Executed 2026-09-03** — verdict `PORT_BUG` × 2, fixed
in both lanes with zero ledger entries and zero golden bytes; see §RP3.13.)*
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
| RP3.12 | `opus-high+` | `opus-high+` | `opus-high+` | as executed: a verdict-only sub-step over one already-decomposed root cause — no product line, no numerics of its own, the judgement calls being the count-lock re-derivation and the ledger-cause rewrite |
| RP3.10 | `opus-xhigh` | `opus-xhigh` | `opus-xhigh` | a behavioral port change in both lanes (the reproduced `QMode=0` dispatch) that moves solved powers on four r4133-gating decks — live probe + per-case power-channel ledger work |
| RP3.11 | `opus-xhigh` | `opus-xhigh` | `opus-xhigh` | a serialization-semantics decision (store vs live) over every class at once, with `Save`/`Dump`/`props_roundtrip` golden exposure in both lanes |
| RP3.13 | `opus-xhigh` | `opus-xhigh` | `opus-xhigh` | as executed: a behavioral engine change in both lanes — two NCIM port bugs, one of them a panic in a `#![forbid(unsafe_code)]` crate — where r4133 is the only live oracle (0.14.5 has no NCIM) and three upstream overruns had to be proven and *not* reproduced |
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
  **compare** on r4133 (`tests/TOLERANCE_NOTES.md:987-993` pins the r4133-side
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
  reused", `tests/TOLERANCE_NOTES.md:980-993`). Mechanically safe: `prop_015x`
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

> **As executed (2026-08-24, COMPLETE — zero product-crate bytes, zero ledger
> byte).** Every factual claim above verified against the vendored r4133 source;
> the kill criterion did **not** fire. One citation drifted: the `Edit` `CASE` is
> at **`:195-218`**, not `:194-217` (arms 0, 1, 2, 4, 6, `3,7`, 8, 9 — no 5); the
> other five (`:310`, `:588`, `:484-507`, `:396-408`, `:532-549`) are exact, and
> the report cites the corrected range.
> Sharper than the plan knew: `LockCommand` is commented out of the **class
> declaration** too (`:39`), and `ActionCommand`/`PresentState`/`Armed` exist in
> neither `TSwtControlObj` nor `TControlElem` — the dead queue code would not
> compile, so "render-only" is provable rather than inferred. The census
> decomposes exactly as stated: 24 in-scope cells = 12 + 12 over
> `swtcontrol_time.dss` + `midi_swtcontrol.dss`, both `engines: r4133`, ours
> `0.25` vs r4133 `120`; of the 18 out-of-scope cells, 12 are
> `swtcontrol_lock.dss` (`capi_v0145`, the third deck of the same `'0.25'`
> spelling — whose frozen row is therefore 36 cells) and 6 are three
> `capi_v0145` copies of the vendored `IEEE_519.DSS` (`Delay=0.0`), and neither
> group needs an entry.
> Landed: the report (`investigations/to_opendss/43-*`, local), the two drafted
> entries (STATUS §WP-RP3, verbatim, landing at RP4.1), the two pins
> `swtcontrol_delay_wires_the_property{,_on_the_midi_tie}`, and the per-pair work
> list `props_r4133_replay::RP3_ROUTING` — `DECLARED_RP3` stays `(7, 4, 7)`
> deliberately, since nothing in the tree claims those rows until the entries
> land.
>
> **Audit-settled the same day** (ten minor findings, one commit, still zero
> product and zero ledger bytes). Records: the "remaining 6" arithmetic above
> (it is 18) and STATUS's RP2.4 frontier sentence corrected; the local report's
> COM/DDLL paragraph fixed — the DDLL `Delay` **write** goes through
> `Set_Parameter` → `DSSExecutive.Command` (`DDLL/DSwtControls.pas:135-136`,
> `:20-28`) and is therefore dropped by the same missing arm, so only the COM
> setter (`DLL/ImplSwtControls.pas:183`) can set what the script cannot. Guards:
> the settled-verdict contract is now typed by the three outcomes §WP-RP3
> sanctions (`RP3_SETTLED_SHAPES`) instead of hard-coding RP3.1's; a settled
> verdict must cite the **r4133** unit (`Version8/Source/…`) and name each pin as
> a whole identifier; the 24 = 12 + 12 decomposition is derived from the corpus
> and the population lock (`the_rp31_census_decomposition_is_read_off_the_corpus`)
> and tied one-for-one to the drafted entries; and the hand-move RP4.1 owes is
> armed by `the_staged_r4133_property_entries_have_not_landed_yet` (§RP4.1
> precondition 2).

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

> **As executed (2026-08-24, COMPLETE — zero product-crate bytes, zero ledger
> byte).** Outcome **LEDGER**; the kill criterion did **not** fire — the probe
> separated echo from live cleanly, and the census re-read holds (exactly 4 of
> the 5 decks, `windgen_snap.dss` clean).
> **The plan's own framing was the one thing that drifted.** "Echoed or live?" is
> the wrong axis here: r4133's `Edit` arm 11 **exists**
> (`Version8/Source/PCElements/WindGen.pas:629`) and its getter is **live**
> (`:2896`) — it simply reads the *dispatched* Q (`Get_Presentkvar`,
> `:2297-2300`, `Qnominalperphase*0.001*Fnphases`) where the property documents
> "the base kvar" (`:365`). Both engines read live fields; they read **different**
> ones (ours is `kvar_base`, `elements/pc/windgen/accessors.rs:431`). The echo
> store would have printed `'60'` (`:2446`) and is unreachable
> (`DSSObject.pas:117-120`), so ECHO was refuted by the engine as well as by the
> source: probed, a typed `kvar=500` renders `0`, and `Edit kvar=777` still
> renders `0` while `? PF` moves to `0.968058`.
> **FIX was refuted by measurement**, which is why the plan's "if r4133's live Q
> really is 0 while ours is 986" branch does not apply: *our* live Q is 0 too.
> The port ports `SetNominalGeneration` loop-for-loop including `Else kvarCalc
> := 0` (`:1320-1321` → `windgen/nominal.rs:223-225`), and the probed terminal
> powers agree on all five decks (power-flow Q −2.1e-05/−4.2e-05 kvar on both;
> dynamics −37 087.81 vs −37 087.76 and −29 216.72 vs −29 216.67). Adopting
> `presentkvar` as our render would *reproduce* an upstream defect — barred.
> **"r4133 is provably wrong" carries five legs**, four of them empirical: the
> help says "base kvar"; the sibling `Generator` has the identical getter
> (`Generator.pas:2402-2405`) and identical help (`:396`) yet renders the base
> (`:3018`); with `QMode=1` the render is `363.54` for a typed `kvar=500`; in
> dynamics it is a stale `777` against a base of `792.718441186736` and a
> terminal Q of −37 087.76 kvar; and `Save Circuit` writes `kvar=0` (file
> produced), corrupting the model on reload (`:3001-3008`).
> Landed: report `investigations/to_opendss/44-*` (local), **four** drafted
> per-case entries (STATUS §WP-RP3, verbatim, landing at RP4.1 per §1.1(e)), four
> pins `windgen_kvar_renders_the_base_on_the_{daily_deck,delta_snapshot,dynamics_deck,fault_ride_through_deck}`,
> and `the_rp32_census_decomposition_is_read_off_the_corpus` (RP3.1's precedent),
> which derives the 4 = 1+1+1+1 split from the decks' own tokens and closes the
> corpus over **seven** WindGen decks — the five cases plus two held out by
> `skipped_oracle_issue.json`. `DECLARED_RP3` stays `(7, 4, 7)`; no echo row.
> **Flagged, out of scope, coordinator/user call: D2** — the steady-state
> `case WindModelDyn.QMode` (`:1276-1322`) has arms 1 and 2 and **no arm 0**
> though `QMode` defaults to 0 (`:1020`) and both the help (`:429-430`) and
> `WTG3_Model.pas:252` document `0 -> Constant Q`. The port reproduces it, so a
> default WindGen dispatches zero vars in power flow on both engines. Fixing it
> moves solved powers on the r4133-gated windgen decks (directly on the two
> power-flow ones) — a separate decision; it does not disturb this landing (the
> render reads `kvar_base`, which the dispatch never touches). **Since the audit
> settlement it is owned, not merely flagged: §RP3.10.**
>
> **Audit-settled the same day** (ten minor findings, one commit, still zero
> product-crate and zero ledger bytes; nothing touched the sub-step's premise).
> Corrections: the local report 44 attributed the `QMode=1` flip of the *daily*
> deck (−985.69 kvar) to its own reproduction deck (−363.54 kvar) and cited
> `Set_Presentkvar`'s "init to something reasonable" as `WindGen.pas:2998`
> (a `Var` declaration — the line is `:3002`) and the volt-var arm as `:1300`
> (it is `:1289`); the pins' and STATUS's claim that the four decks "end at
> `Set mode=…` and carry no solve of their own" was false for three of them
> (`windgen_snap_delta.dss` ends at `Calcvoltagebases`; both dynamics decks carry
> a snapshot `solve`) — the pins' solve is nonetheless the gate's own
> (`corpus_gate/runner.rs:348-349`); the held-out pair's "~569.97 **against
> r4133's `0`**" was never measured and does not even share the mechanism (both
> type `QMode=2` with a real `VV_Curve=`, i.e. the volt-var arm), and a promotion
> would add `WindGens × steps` cells, not "a cell"; and the commit message
> `c46bca42` welded two probe steps (`kvar=500` renders PF `0.986394`; PF
> `0.968058` is the later `Edit kvar=777`) — the tree's own records were already
> right, so the correction is recorded in STATUS. Guards: `element_scope` now
> accepts the quoted declaration form and treats `Edit`/`BatchEdit` as the same
> element scope, so "no deck types `kvar=`" is swept over the lines that can type
> it (the audit's mutation — `Edit WindGen.w1 kvar=500` into `windgen_daily.dss`,
> previously green — is red), and the census derivation now reads each deck's
> `QMode=`/`VV_Curve=` too: the five census decks must select **no** arm (that is
> where r4133's `0` comes from) and the two held-out decks must be the volt-var
> pair the doc argues from.

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

> **As executed, 2026-08-24 — ECHO (`EchoParse`), zero product-crate bytes, zero
> ledger bytes.** Line numbers below are re-measured in
> `.inputs/electricdss-code-r4133-trunk/Version8/Source/`.
> **The plan's own premise was wrong, and disproving it was the sub-step.** The
> `:1760` restore is real as a *line* — `ReversePQ2PV` (`Common/Solution.pas:
> 1743-1768`, declared `:372`) does carry `// Takes it back to model 3` — but the
> procedure **has no caller anywhere in the trunk**. The only call is
> `VersionC/Common/Solution.cpp:827`, commented out (`ReversePQ2PV(ActorID); -
> not needed for now (04/01/2024)`), and `DoNCIMSolution` (`:1095-1161`) ends at
> its `Until` with nothing after it. So r4133 leaves a converted generator at
> model **4**, exactly as the port does; the only live reversion is the in-loop
> `:2229`, which needs `not myPQOK` and does not fire on these decks (both clamp
> *upward*, so |V| lands below the target).
> **The divergence is the render, and it is a parse-store echo — proven in both
> directions on the live r4133 DLL, without the solver.**
> `TGeneratorObj.GetPropertyValue` (`PCElements/generator.pas:3007-3038`) has no
> arm 6, so index 6 falls to `General/DSSObject.pas:112-115`
> `Result := FPropertyValue[Index]` — the deck's own token, written at
> `generator.pas:625` before the CASE assigns the live field at `:643`. Probed:
> `GeneratorsI(9)` (the field itself, `DDLL/DGenerators.pas:125-134`) reads **4**
> after the solve on both decks while `? Generator.g1.model` renders `'3'`;
> `GeneratorsI(10, 4)` moves the field alone and the render **stays** `'3'`;
> `Edit model=4` moves the store and the render **follows**; `Dump` and
> `Save Circuit` both write `model=3`. A second `Solve` leaves it at 4 — the
> behavioural confirmation that the restore is dead.
> **FIX was refuted by measurement**: the two engines' live state agrees digit
> for digit — present kvar `431.79425771046976` / `323.84569328285227` before the
> conversion, `0` after, 4 iterations, on both decks (the port leg read through
> the public API). LEDGER was refuted by the `GeneratorsI(10, 4)` probe: the
> getter demonstrably does not read `GenModel`, so "live but wrong" is false.
> `EchoParse`, not `EchoDefault`: the store holds the deck's `'3'`, where
> `InitPropertyValues` would have left `'1'` (`:2559`).
> Landed: the echo row `generator.model` (`EchoParse`, 2 cells, witness
> `Pin`, since **both** cells are on `engines: "r4133"` cases and no capi witness
> could exist), the pin
> `generator_model_renders_the_live_pv2pq_conversion` (both decks —
> `ncim_midi.dss` had no unit pin at all — with the `maxkvar` reading that names
> the `:2120` promote and an edit-then-re-solve round trip), and
> `the_rp33_census_decomposition_is_read_off_the_corpus`, which sweeps the corpus
> for `Set algorithm=NCIM`, reads each survivor's `Generator` count and `model=`
> token, and derives 2 = 1 + 1 while naming the control (`ncim_pq.dss`, NCIM with
> no generator) and the two held-out `kind: large` NCIM decks
> (`IEEE118Bus/master_file.dss` 53 generators, `NCIM/Xmission_System_Kundur2Area`
> 3, both `model=3`, both outside the census population per `triage.md` §Method).
> Accounting: `PROPS_ECHO_R4133` 81 → **82**, `ECHO_PARSE_ROWS` 7 → **8**,
> `ECHO_ROWS_ON_R4133_ONLY_CASES` 57/34 969 → **58/34 971**, `CLAIMED_ECHO`
> 169 → **170** (`CLAIMED_TOTAL` 2 974 → 2 975 derived), `CLAIMED_SPELLINGS_LIVE`
> 2 981 → **2 982** (re-measured: `DSS_PROPS_CENSUS=claims`, 439 cases × 2
> channels, `echo-row` = 170 spellings over 82 pairs), and — the first time a
> WP-RP3 sub-step moves it — `DECLARED_RP3` `(7, 4, 7)` → **`(6, 3, 6)`**, because
> an ECHO outcome ships its exclusion in the sub-step's own commit and the chain
> really claims the row. The routing guard was extended for that: its middle term
> is now the entries that still declare rows, and a `0, 0` entry must prove the
> positive half — the pair's rows still exist in the corpus and **every** one is
> claimed by `Link::Echo`.
>
> **Audit settlement, 2026-08-24 — one thing this sub-step could NOT close.**
> "No behavioural divergence to fix" is true of every channel any lane compares,
> and the audit round bounded that claim: r4133's `Save`/`Dump` print the same
> store, so its round trip re-creates the model-3 generator, while the port's
> `Save Circuit` writes the live `Model=4` (measured: `report/save/save.rs:34-53`
> renders through `ClassProps::get_value`, where Pascal `SaveWrite` reads
> `PropertyValue[iProp]`, `General/DSSObject.pas:145-165`) — a re-compiled deck
> is then a PQ generator instead of a Q-limited PV one, and the same line also
> carries a `PF=0.88` the deck never typed. No oracle channel compares that
> surface, on these decks or any other, so it is neither this commit's regression
> nor part of the echo classification: it is **§RP3.11**, which the settlement
> opened for it. *(Executed 2026-09-03: verdict `KEEP_LIVE_PINNED` on both
> surfaces — and the premise above is corrected there, `PropertyValue[iProp]`
> being the **virtual** getter that 49 r4133 units answer live; see the dated
> line in §RP3.11.)* The settlement also closed two holes in the census
> decomposition's own sweep (abbreviated `Set` option spellings and the
> `.dss`-only file universe), tied the
> `ECHO_ROWS_ON_R4133_ONLY_CASES` entry to the derivation that claims to produce
> it, and removed a pre-existing flake in the seam-counter tests.

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

> **As executed, 2026-08-24 — COMPLETE, outcome LEDGER, and the section above is
> wrong on one word.** The premise held: r4133 carries the slip byte for byte
> (`Version8/Source/PDElements/GICTransformer.pas:495`
> `G2 := 100.0 / (FZBase2 * FPctR1);`, the twin of pinned dss_capi 0.14.5
> `src/PDElements/GICTransformer.pas:441`), and the census decomposes exactly as
> stated — **2 cells, both in scope**, one per `%R`-specified GICTransformer on
> `asymmetric:gic/gictransformer_gic.dss` (`tg3`) and `asymmetric:gic/gic_midi.dss`
> (`tg5`), with the corpus's other **20** declarations (19 of them on
> r4133-gating cases) ohms-specified and cell-free. The word that is wrong is
> "**echo**": property 8 (`R2`, `:130`) is rendered by `GetPropertyValue` arm 8
> as `Format('%.8g', [1.0/G2])` (`:723`, and `DumpProperties` `:663`) — a **live
> computation** off the mis-derived conductance, not a parse-store echo, while
> property 14 (`%R2`, `:136`) *does* echo `FpctR2` (`:729`) and agrees with the
> port. A `PROPS_ECHO_R4133` row would therefore have misnamed the mechanism,
> so the outcome stayed the section's own named shape, `LEDGER`: two drafted
> per-case entries (`gic-pct-r2-honoured-gictransformer-r4133-props`,
> `gic-pct-r2-honoured-midi-r4133-props`) reusing the existing
> `gic-pct-r2-ignored` cause unchanged, recorded verbatim in STATUS §WP-RP3 and
> landing at RP4.1 per §1.1(e), witnessed meanwhile by
> `gictransformer_r2_honours_the_x_winding_percentage` and
> `…_on_the_ring`. Zero product-crate bytes (G2.5 fixed the engine on
> 2026-08-06), zero `ledger.json` bytes, no report — number **45** stays free
> because `investigations/to_opendss/07-gictransformer-g2-uses-pctr1.md` is
> already written against r4133 and reproduces this very surface.
>
> **The one record this section states imprecisely.** "The `makeposseq`/
> `linespacing` surfaces stay capi-only — their decks do not gate r4133" is true
> of `modes:makeposseq/makeposseq_shunt.dss` and `makeposseq_pc.dss`
> (`engines: "capi_v0145"`), but **`asymmetric:line/line_spacing_asym.dss` is
> `engines: "both"`**. It stays capi-only for two independent reasons measured
> here instead: the ledger holds `r4133-linespacing-asym-303` with
> `kind: "skip"` (EPRI #303 access violation while compiling the deck's
> `tscables=`/`wires=` spacing), so `ledger.rs::channel_is_skipped` makes
> `scheduler.rs:355-356` `continue` past the channel; and r4133's own
> `General/LineGeometry.pas:1235-1239` implements the min-over-phase rating rule
> the port follows, so there would be nothing to twin even if it ran — the census
> carries no `line.normamps`/`line.emergamps` row at all. Successors must quote
> the mechanism, not the shorthand — and since the audit settlement the *guards*
> quote it too: `props_r4133_replay.rs::r4133_skipped_cases` reads the ledger's
> `kind: "skip"` entries, and all four RP3 census derivations now take "in scope"
> to mean `engines=` **and** an undropped channel. Note also that this deck's
> `engines: "both"` touches the sub-step's stated kill criterion ("a
> `makeposseq`/`linespacing` deck DOES gate r4133"); the sub-step ruled it
> not-the-kill-criterion in-lane and the settlement escalates that reading to the
> user, the substance being independently confirmed twice.
>
> **Audit settlement (2026-08-24, one commit over `cab2e667`).** Seven minor
> findings, all settled, no classification or count moved (STATUS §WP-RP3 carries
> the per-finding record): the ring pin gained the `R1 = 1.587` control it was
> missing (the audit proved a no-op `%R9=0.4` edit left it green); the census's
> `%R1 == %R2` branch became a counted third class with its own assertion, beside
> the new "every diverging element sits on an r4133-gating case" one; the scope
> predicate became skip-aware in all four derivations; and three citations were
> corrected (`scheduler.rs:355-356`, three syntax-highlight files, the routing
> guard's and tripwire's stale staged-set docs — the tripwire now names all eight
> staged ids). Zero product-crate, `ledger.json`, golden, manifest,
> `population.lock` and frozen-extract bytes; 4 204 tests per lane, unchanged.
>
> With this sub-step **all four bin-7 root-cause pairs are settled**; RP4.1 now
> waits on RP3.5–RP3.7, RP3.8 and RP3.9 only. `DECLARED_RP3` stays `(6, 3, 6)` —
> a LEDGER outcome stages its exclusion, so three of the four rows are still
> declared even with every sub-step run.

### RP3.5 — Line length units lost by the matrix-branch merge (opened by RP2.2)

`line.units` `'none'` vs `'kft'` (3 cells, **0 in scope** — the affected
`modes:reduce` case is `engines: "capi_v0145"`). r4133 renders index 20 from
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

**Three corrections to the paragraph above, measured 2026-08-28 and recorded
here because the sub-step's population and its cost both depend on them.**

1. **`reduce_mergeparallel` contributes ZERO `line.units` cells** — the deck
   list above is wrong. All 3 cells are on `modes:reduce/midi_reduce.dss`
   (`Line.l2a~l2b`, `Line.l3a~l3b`, `Line.bb14_15~l9a`), read out of
   `investigations/g1_1_r4133_props/r4133_props_census.json` and confirmed live:
   `Line.b1||b2.units` renders `km` on the r4133 DLL, on the pinned dss_capi
   0.14.5 and on the port. Mechanically `reduce_mergeparallel`'s lines are
   3-phase symmetrical-components, so its merge takes the **sym** branch. (The
   `'kft'` spelling in the census is itself the giveaway: that deck types
   `units=km` throughout.) What `reduce_mergeparallel` *does* expose is a
   different r4133 defect in the same routine — see (3).
2. **The sym-components branch is NOT already correct**, only correct on the
   path the corpus walks. The port's `Length=`/`Units=` re-apply sits **inside**
   `if let Some(v) = rxc` (`exec/reduce.rs:343`), whereas both oracles run it
   unconditionally (r4133 `:1724-1726`, capi 0.14.5
   `src/PDElements/Line.pas:1764-1768`, outside `if UseRXC`). The two arms that
   produce no impedance values — the parallel `IsSwitch` (`:1708`) and
   `OtherLine.IsSwitch` (`:1709`) arms — therefore kept a pre-merge `Len`. And
   the second of them was a silent no-op besides: the port emitted the **text**
   `Switch=1`, a transliteration of capi's *typed*
   `SetInteger(ord(TProp.Switch), 1, [])` (`:1736`), which `InterpretYesNo`
   rejects on both engines, so the merged branch kept the partner's real
   impedance where both oracles give it dummy z. Both halves are inside this
   sub-step ("port gaps immediately"), and both are fixed here.
3. **The plan's only stated costs — "a port fix in both lanes with a pin" or "a
   cited exclusion" — miss the capi channel.** `modes:reduce/midi_reduce.dss` is
   `engines: "capi_v0145"`, `MODES.compare_all_properties` is `true`
   (`corpus_gate/manifest.rs`) and `force_properties` ORs it in for every
   `gates_capi()` family case (`corpus_gate/scheduler.rs`), so the capi channel
   **does** compare `all_properties` on this deck today and the fix reds 3 cells
   on it. The §1.1(e) staging rule is written for **r4133** `property` entries
   (staged because the r4133 props compare is masked until RP4.1); the capi
   compare is live, so a staged capi entry would leave the gate red. The
   `capi_v0145` entry therefore lands **in this sub-step's commit**, with its
   cause and the `population.lock.json` rewrite the rigor fingerprint forces.

**As executed (2026-08-28) — outcome `FIX` (both lanes) + one live `capi_v0145`
ledger entry.** The probe ran first, on all three engines
(r4133 `OpenDSSDirect.dll` `Version 11.0.0.1`, pinned dss-python 0.15.7 /
dss_capi 0.14.5, and the port), over the two vendored decks plus six
purpose-built micro-decks; no decision-table kill criterion fired.

* **A — the matrix-series branch.** r4133 renders `kft` on all three merged
  lines where the port and capi 0.14.5 render `none`, and the value is
  deck-dependent (`mi` and `cm` on two micro-decks whose surviving line carries
  those units, `none` on a switch), so the getter is live state. Fixed in both
  lanes: `exec/reduce.rs` re-applies the units through `red_set_units` **after**
  the `RMATRIX/XMATRIX/CMATRIX` side effects. `length`, `rmatrix`, `xmatrix`,
  node count (88) and iteration count (5) already agreed with r4133 digit for
  digit, and `ConvertLineUnits` returns 1.0 whenever either side is
  `UNITS_NONE`, so `FUnitsConvert` — and with it YPrim, Y, V, I, S and losses —
  is unmoved: `units` is the only cell that moves. Ledgered on the capi channel
  as `reduce-merge-units-restored-midi-capi-props` (cause
  `line-merge-length-units-reset`, 3 hits), pinned by
  `exec::tests::reduce::merged_matrix_line_keeps_the_surviving_lines_length_units`.
* **B — `reset_length_units` clearing `user_length_units`.** Neither oracle does
  (r4133 `:2330`, capi 0.14.5 `:2084`, identical comment), so this was a
  port-authored line with no authority question. It **is** observable, through
  the one consumer either tree has: on a deck typing `units=kft` before its
  matrices, `Export CIM100` writes `<cim:Conductor.length>609.6</…>` on r4133
  **and** on capi 0.14.5 against `2` on the port. The line is deleted; pinned in
  direct state by
  `elements::pd::line::tests::reset_length_units_keeps_the_users_units` (all
  three callers) and observably by
  `golden_cim::cim_conductor_length_uses_the_users_length_units`. No CIM golden
  byte moves — in every golden CIM deck (`cim_lines`, `cim_load`, `cim_shunt`,
  `cim_xfmr`, `cim_der`, `IEEE13Nodeckt`, `IEEE123Master`) `units=` is the last
  impedance-relevant token or absent, so `reset_length_units` never runs after a
  `units=` write; verified by a sweep of every `Line` declaration in those decks.
* **C — the sym branch's two switch arms.** Fixed as described in correction (2):
  the `Length=`/`Units=` re-apply is hoisted out of the `rxc` arm and the
  `Switch=1` token becomes r4133's own `switch=yes`. No vendored corpus deck
  reaches either arm (0 census cells), so nothing was red and nothing on a gated
  case moves; pinned by
  `exec::tests::reduce::parallel_merge_with_a_switch_restores_length_and_dummy_z`
  over three decks (partner-is-switch, self-is-switch, and a non-switch control).
* **D — a new r4133 defect in the same routine, reported not reproduced.**
  `Line.pas:1715` is `S := ' R0=' + …` where every neighbouring statement appends
  (`S := S + …`), so the parallel symmetrical-components edit string loses its
  `R1=`/`X1=` half: on `reduce_mergeparallel` r4133 renders the *un-merged*
  `r1 = 0.301` / `x1 = 0.667` while `r0`, `x0`, `c1`, `c0`, `length` and `units`
  all match the port and capi exactly — the precise asymmetry an
  assignment-instead-of-append predicts, and the four measured relative gaps
  reproduce the frozen census cells to the last digit. capi 0.14.5 does not carry
  it (`:1740-1761` fills `RXC[1..6]` and sets all six) and neither does the port.
  The only exposing deck is `engines: "capi_v0145"` ⇒ 0 in-scope cells ⇒ report
  only, no exclusion: `investigations/to_opendss/45-line-mergewith-parallel-drops-r1-x1.md`.
* **Not moved, deliberately.** No `PROPS_ECHO_R4133` row (the r4133 getter at
  `:1404` is live — an echo row would be a false statement about the mechanism);
  no r4133 ledger entry (0 in-scope cells, and the r4133 props compare is masked
  until RP4.1, so `assert_all_hit` would fail it as NEVER APPLIED); no
  `props_roundtrip` scenario (that gate replays 0.14.5 values and would pin the
  capi bug as expected); `DECLARED_RP35`/`DECLARED_RP3` and the frozen
  `tests/corpus/props_r4133/` extracts unchanged — the extracts are a **data**
  lock recording the 2026-08-08 measurement, so their `rust='none'` column is
  historical after this sub-step and is not edited (RP0.1 evidence lock).
  `lane_diff.ps1` is not owed by the ritual's list (RP1.2/RP1.3 only) and the
  change carries no `#[cfg(feature = …)]`, but it was run rather than argued:
  **VERDICT PASS, `max |Δ| = 0` exactly on conv/cur/errs/iter/loss/pow/v/y over
  3 220 247 records / 522 cases, 0 drifted iteration counts.**
* **Audit settlement (2026-08-29).** Both audits confirmed the classification
  and the mechanism on live oracles; nine claims, seven settled, one refuted, one
  handed to §RP3.6. Two more `MergeWith` defects were fixed here — one raised by
  the code audit, one found while settling the refuted claim: the routine
  re-pointed only the **partner's** controls
  (r4133 does both, `:1682-1683`, with `NewName` before `:1684`'s rename — the
  survivor half is the one the reduce strategies actually use), and the sym
  branch's `RecalcElementData` (`:1730`) had been deferred to `CalcYPrim`, which
  is not equivalent because the call clears `SymComponentsChanged` and that flag
  gates `CalcYPrim`'s `C1 /= ConvertLineUnits(UNITS_KFT, LengthUnits)` fix-up
  (`:1031-1038`) — the partial-C arms this sub-step had just made reachable came
  out at `c1 = 3.6089` where both oracles render `1.1`. Also added:
  `the_rp35_census_decomposition_is_read_off_the_corpus` (the "3 cells, 0 in
  scope" split derived from the corpus, with `reduce_mergeparallel`'s zero
  contribution asserted), a pin on the real `midi_reduce` deck, and pins on the
  two length-derived quantities the restored units correct. **Refuted:** the
  claim that r4133 renders `linecode = ''` on a partner-is-switch merge — it
  renders `lc`, so §RP3.6's premise is now measured rather than read. Full record
  in STATUS §RP3.5.

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
(`Line.pas:626-627`), and the decks that carry the five cells
(`Examples/ADiakoptics/EPRI_Ckt7-G/Torn_Circuit/zone_2/Branches.dss:93,:95,:479`
and `zone_3/Branches.dss:161,:165`) put `units=m` **after** `Switch=True`, so
the two engines take different branches there. (`StoCtrl_Current_PeakShave/
Line.DSS` has the same shape on nine lines and contributes **zero** cells: its
case is `kind: large`, which `force_properties` never property-compares.)
**Two measurements RP3.5's audit settlement hands to this sub-step
(2026-08-29), both live on the r4133 DLL.** (1) The premise is **confirmed**:
after a parallel merge whose partner is a switch, r4133 renders the survivor's
`linecode` as `lc` while the port renders `''` — the arm RP3.5's item (C) made
reachable, so the divergence is live on a path no corpus deck walks (0 cells)
until this sub-step lands. (2) The `FLineCodeSpecified`/`CondCode` split this
sub-step must build also owns a **CIM** divergence RP3.5 measured and could not
fix in `MergeWith`: r4133 back-fills a LineCode's units from `FUserLengthUnits`
by matching the `CondCode` **string**, which survives the flag being cleared
(`Common/ExportCIMXML.pas:3877`), and writes
`PerLengthSequenceImpedance.r = 0.301/304.8 = 0.00098753281` where capi 0.14.5
and the port — which match by the live object, `cim/export.rs::
find_line_units_for_linecode` — write `0.301`. The port's
`kill_line_code_specified` clears `line_code_name` along with the reference, so
it has no `CondCode` to match on; resolve both with the same split.

r4133 is the behavioral authority (CLAUDE.md 2026-08-02), so the default
expectation is a port fix in both lanes plus a pin on the resulting
`FUnitsConvert`/impedance; the alternative (keep the kill, pin the r4133-side
delta) must be argued from a probe, not from "capi does it". **Do first:** an
epri-worker probe of `linecode=… Switch=True units=m` reading back `linecode`,
`units` and `r1` on both engines. **Acceptance:** the 5 in-scope cells are
either compared or excluded-with-a-pin; the `FUnitsConvert` consequence measured
either way; and the capi *oracle* proven unmoved while the capi *comparison*
is re-settled (0.14.5 keeps its own behavior, so a port fix owes the capi-side
delta its own ledger/pin decision on the cases that compare properties today).
Tier: `opus-high+`.

**Part (a) — the switch arm — SETTLED 2026-08-29 (`FIX`, both lanes).** The
probe ran on all three engines and the kill criterion did not fire: r4133
renders `'99'`/`'98'` on every switched, linecode-bearing corpus line, the flag
really selects the `FUnitsConvert` branch (a code in kft with the line in m
gives `r1 = 1/304.8` on r4133 against `1` on capi and the pre-fix port, and a
later `units=kft` flips it back), and deck A moves nothing else — every
impedance, length, units and matrix cell agrees on all three engines and the
solved state is inside the faer-vs-KLU floor. The one call was deleted from the
`SWITCH` arm of `elements/pd/line/accessors.rs`; the five cells are excluded on
the live capi channel by `line-switch-keeps-linecode-zone{2,3}-capi-props`
(landed, not staged) and pinned by
`exec::tests::line_fetch::switch_yes_keeps_the_linecode_and_its_units_conversion`;
no golden byte moved and `lane_diff` was re-run as a measurement: PASS,
`max |Δ| = 0` on every kind.

**Part (b) — the `FLineCodeSpecified`/`CondCode` split — SETTLED 2026-08-29
(`FIX`, both lanes).** The port now carries the flag (`line_code_specified`)
beside the name (`line_code_name` = `CondCode`), which `FetchLineCode` writes
(`Line.pas:387`) and only the constructor clears (`:825`), so it outlives every
`KillLineCodeSpecified`; the typed handle `line_code_ref` (r4133 keeps none —
its `LineCodeObj` is a local, `:376`) travels with the flag. The property render
and the `units=` branch stay flag-gated (`:1357`, `:626-627`), `Dump` prints the
raw `CondCode` unconditionally (`:1273`, taken here rather than deferred to
RP3.11: a one-line render difference from 0.14.5 with no golden behind it), and
`cim/export.rs::find_line_units_for_linecode` matches the **name**
(`Common/ExportCIMXML.pas:3872-3884`), so an overridden or switched line donates
its `FUserLengthUnits` to a `Units = UNITS_NONE` LineCode exactly as r4133 does:
`PerLengthSequenceImpedance.r = 0.301/304.8 = 0.00098753281` against 0.14.5's
`0.301`. Pinned by
`exec::tests::line_fetch::linecode_name_survives_the_flag_that_gates_its_render`
and `golden_cim::cim_linecode_units_backfill_matches_the_condcode_string`, both
oracle-free and non-vacuity-proven. **No golden byte moved** (measured deck by
deck across the CIM decks, the `Dump` report goldens, `props/*.json` and the
`Save` goldens, then re-measured by running them) and **no ledger or lock line
moved**: the gate compares properties through the flag-gated getter and reads
neither `Dump` text nor CIM XML, and the full 521-case gate is green in both
lanes; `lane_diff` was re-run as a measurement even though the solved state is
out of reach: PASS, `max |Δ| = 0` on every kind. Four adjacent divergences are recorded with owners rather than chased
(the object-ref miss path, `MakeLike`'s copy set, the switch-unreachable
`Conductor.length` branch, name casing). Full record in STATUS §RP3.6.

**Audit settlement — 2026-08-29 (one commit over `4b146ab9`).** Both auditors
re-derived the mechanism on the live oracles and confirmed the outcome: `FIX` in
both lanes, 5 cells / 5 in scope, both ledger entries and their cause unchanged,
no bug reproduced, no golden regenerated. Three real defects were fixed here, all
in the same statement family the sub-step had opened, all under "port gaps
immediately". **(i)** r4133's eighth flag-clear site, `FetchConductorList`
(`:1853-1854`), had no port counterpart, and part (b)'s written justification for
skipping it — "r4133 reaches the statement with a nil `FLineSpacingObj`" — cited
**dss_capi**'s `FetchLineCode` tail (`src/PDElements/Line.pas:581-582`) as if it
were r4133's; r4133's arm-3 side effect is a **plain** `SpacingSpecified := False`
(`:663`), so the spacing object survives and `? Line.<x>.linecode` really does
answer `''` behind a `conductors=[..]` (measured). **(ii)** That plain assignment
is the general rule the port could not express: `SpacingSpecified` is a Boolean
field (`:107`) and the `switch=` arm drops it the same way (`:696`), where the
port called the full `KillSpacingSpecified`. Measured as an A/B on identical
decks: after `switch=yes` r4133 still runs a following `conductors=`, after `r1=`
it faults on the nil pointer. The port now carries `Line::spacing_specified`
beside the objects, raised where r4133 raises it (`:704-713`) and guarded where
r4133 guards it (`:2268`). **(iii)** The `FUnitsConvert` consequence was pinned
only through the rendered `r1`; the pin now **solves** the discriminating deck
and compares its node voltages against the r4133 DLL's own `YNodeVarray`, plus
the `Save Circuit` emission the fix newly reaches. `MakeLike`'s copy set — listed
above without an owner — is now `ORPHANED_GAPS.md` §1.12. Gate green in both
lanes (4 219 tests), `lane_diff` re-run: PASS, `max |Δ| = 0` on all eight kinds.

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

**As executed (2026-09-02) — outcome `FIX` (both lanes) for all three parts,
plus five live `capi_v0145` ledger entries. Full record: STATUS §RP3.7.** The
probe ran on all three engines and no kill criterion fired. Four points where the
code corrected this section's premises or numbers:

* **The acceptance criterion resolves as "compare", with zero r4133 entries.**
  The 80 in-scope cells sit on `midi_swtcontrol` (12), `swtcontrol_time` (12) and
  `civanlar` (16) — all `engines=r4133`, i.e. **not** the decks the fix reds — and
  after the port they are **byte-identical to the r4133 DLL**, measured directly
  and confirmed by a census that finds **no** `Normal`/`State` row left on that
  channel. So **80 / 80 COMPARE**, no exclusion, and no r4133 `property` entry is
  staged, landed or drafted. What the fix does red is the **capi** channel's 19
  out-of-scope cells on five cases, ledgered under one new cause.
* **The corpus carries eight SwtControl decks, not five.** A `--include=*.dss`
  sweep misses the uppercase `IEEE_519.DSS` copies, which declare two SwtControls
  each (`:45-46`) and carry 6 of those 19 cells.
* **Two source facts this section did not call out, both load-bearing.** (i) The
  per-phase branch honors at most **five** tokens (`i < SWTCONTROLMAXDIM`,
  `SwtControl.pas:461`; `Relay.pas:1286`) while the getters render up to
  `min(6, NPhases)` — parse cap and render bound are different numbers. (ii) A
  **quoted single token** is per-phase where a bare one is ganged (`state=(open)`
  → `[open, closed, closed, ]` vs `state=open` → `[open, open, open, ]`,
  measured), which is why `Parser.WasQuoted` had to be plumbed to the property
  seam and reconstructed on the seams that have no outer parser.
* **(b) is not render-only.** One frozen `ctrl_snap` produces **two** symptoms —
  the three-token render *and* a `Sample` resyncing past the end of a 1-conductor
  element, which is what makes the frozen census cell `[closed, open, open, ]` —
  and one refresh in `make_pos_sequence` settles both. The same seam carried four
  further r4133 mismatches on Relay (the quoted single token ×2, the `Action`
  supplemental ×2, the five-token cap), all fixed here under "port gaps
  immediately". The fallback this section offered (record the gap in
  `ORPHANED_GAPS.md` and carry an exclusion + pin) was **not** taken: the probe
  confirmed the authority reading, so the default applied and no exclusion exists.

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

**As executed (2026-09-02) — outcome `FIX` (both lanes), zero golden bytes, zero
ledger entries. Full record: STATUS §RP3.8.** The probe ran first and the kill
criterion did **not** fire: all five renders are reproducible from the port's own
state, measured cell-for-cell on 9 decks / 100 solved steps + 9 golden scenarios
+ 2 pin decks. Seven points where the code corrected this section's premises or
numbers:

* **`PowerFactor` is `Version8/Source/Common/Utilities.pas:1821`, not
  `mathutil`.** There is no `PowerFactor` in `Shared/mathutil.pas` at all; the
  pointer above was off by a unit. Its `Else Result := 1.0` arm is why an
  unsolved or disabled machine reports unity.
* **The port renders full precision, not the Delphi `%.6g`/`%-.8g` this section
  asked for.** Every other double in the port renders through `float_to_str_ex`;
  emitting a Delphi width from these five alone would put a lossy string on
  `Dump`/`Save`/export where every sibling is exact. The r4133 channel absorbs
  the digit difference through RP2.4's `R4133_DISPLAY_FLOOR = 2e-4` +
  `display_is_render` — measured worst gated cell **4.029e-08** rel, four orders
  under the floor — and the pins carry r4133's own bytes through `fmt_g`.
* **The `Var Sum` write-back is a dead store, so there was nothing to weigh.**
  A whole-tree grep for `TotalkWhCapacity`/`TotalkWCapacity` returns six lines —
  two declarations, the two property arms, two dead `RecalcElementData` calls
  (`:1107-1108`) — and nothing reads them (0.14.5 commented the fields out). The
  getters re-sum the fleet on every call, so the store feeds no render either:
  not reproduced, and not even a pinnable divergence.
* **Exposure: 22 StorageController + 6 IndMach012 cases; 24 red on capi / 84
  cells / 1 006 comparisons.** Three corrections to the exposure list this plan
  worked from: `Test/indmachtest/Master.DSS` and `Kersting4wire_{Lagging,
  Leading}.dss` hold **no** IndMach012 (their hit is `UserModel=IndMach012a` on a
  *Generator*), `StoCtrl_SeasonTarget/IEEE13NodecktMOD.dss` holds no
  StorageController, and three cases were missing (`controls/combo/
  midi_controls.dss`, `modes/makeposseq/makeposseq_ctrl.dss`,
  `controls/relay/relay_generic.dss`). `StoCtrl_Current_PeakShave/master.dss` is
  `kind: large`, hence never property-compared — which is why 22 holders produce
  only 20 red cases.
* **A pre-solve read of `indmach012.pf` mutates in r4133 and must not here.**
  The recompute on an unstamped `Iterminal` cache runs `CalcPFlow` and advances
  the slip-Newton; r4133 keeps that advance (the `VSConverter.GetCurrents`
  family), and an in-place port of it moved a committed JSON golden
  (`spectrum_refs.json`, `Slip`). The recompute is required — it is how both
  engines get the number — so it runs on a throwaway `self.clone()` and only the
  f64 is kept.
* **The accounting took a third state, not either of the two this section
  offered.** The frozen example rows record `rust = ''` **by capture** and cannot
  be re-frozen, so neither "the ordinary claimed accounting" nor a cited
  exclusion can hold them honestly — replaying a counterfactual spelling would
  prove nothing about the comparator, and writing the port's new spelling into
  that column would be a fabricated measurement. `DECLARED_RP38` still goes
  `(181, 5, 181) → (0, 0, 0)`, but its old value **moves** into a new
  `SUPERSEDED_RP38` bucket (`RP38_ROUTING` → `RP38_SUPERSEDED`), with
  `claimed + declared + superseded == rows` and the shipped `skip_prop`
  disposition asserted per pair.
* **Two upstream bugs found and reported** (`investigations/to_opendss/`, 48 and
  49): `? IndMach012.<n>.PF` before `NodeRef` is assigned access-violates in
  r4133, and `TStorageObj.MakePosSequence` writes `kWrating=` for the property
  `kWrated` (`PCElements/Storage.pas:3979-3985` vs `:647`), which is the one
  measured r4133-side divergence these renders expose — see §RP4.1.

**Audit settlement (2026-09-02).** The two lenses raised nine findings (eight
after dedup); one was major. `Save` is a **fifth** `ClassProps::get_value`
reader, and it never ran the refresh choke point: a deck that writes one of these
read-only properties still marks it *set*, so `Save` emitted the render cache —
the construction default (`PF=1`, `kWhTotal=0`), or the live value if a `?`
happened to come first, i.e. an output that depended on the session's read
history. Both engines were measured on the same decks (r4133 writes
`pf=0.886059`, `kWhTotal=6000`), and the same latency was found — pre-dating
RP3.8 — on `READS_VTERMINAL` (`Transformer.WdgCurrents` saved all-zero where
r4133 saves the solved currents). Fixed with one up-front pass over the store,
`Dss::refresh_render_caches_for_save`, in both `Save` entry points; pinned by
`exec::tests::report::save_renders_the_live_result_properties` with r4133's own
bytes. No corpus deck or golden writes any of the six properties, so no committed
byte moved. The rest of the settlement (a registry-wide holder guard for the
flag, a discriminating leg for the two JSON-load tests, the marker-vocabulary
check, the root-anchored `tmp` skip, and the two records that stay records) is in
STATUS §RP3.8.


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

**As executed (2026-09-02, audit settled 2026-09-03) — all 27 pairs
`PRECISION_ROUNDTRIP`, held by ten expected-value pins, no `PORT_BUG` /
`UPSTREAM_BUG` / `KILL`, zero product-crate lines and zero ledger entries
(`count_in_scope = 0` on all 70 cells); `OPEN_RP39 = (0, 0, 0)` while
`DECLARED_RP39` stays `(55, 27, 19)` as a measurement. Full record: STATUS
§RP3.9.**

### RP3.10 — the reproduced `QMode=0` dispatch (opened by the RP3.2 audit settlement)

**Why it exists.** RP3.2 found, and its audit round refused to leave merely
flagged, a **reproduced upstream bug** — the one thing the 2026-08-02 policy
(CLAUDE.md) says may not stand in any lane. `SetNominalGeneration`'s steady-state
`case WindModelDyn.QMode` (`Version8/Source/PCElements/WindGen.pas:1276-1322`)
implements arm 1 (PF) and arm 2 (Volt-Var) and **no arm 0**, so `Else kvarCalc :=
0` (`:1320-1321`) zeroes the reactive dispatch — while `QMode` *defaults* to 0
(`:1020`) and both the property help (`:429-430`, `'Q control mode (0:Q, 1:PF,
2:VV).'`) and the dynamics model (`WTG3_Model.pas:252`, `QMode := 0; // 0 ->
Constant Q`, implemented at `:1059-1061`, `Qord := Qref`) document 0 as
constant-Q. A default-configured WindGen therefore injects **zero vars** in power
flow no matter what `kvar=`/`pf=` says. The port ports the arm verbatim
(`crates/dss-core/src/elements/pc/windgen/nominal.rs:223-225`, `_ => kvar_calc =
0.0`), which is why the gate's power channel is green on the four windgen decks.
RP3.2 could not fix it: its outcome was LEDGER, i.e. **zero product-crate bytes**
by the plan's own rule, so the item carries no note at its site and lives only
here and in STATUS §WP-RP3 — this section is what keeps it from being lost.

**Scope.** Decide the correct steady-state behavior for `QMode=0` from
`WindGen.pas` + `WTG3_Model.pas` + physics (the obvious candidate is `kvarCalc :=
kvarBase`, the constant-Q reading of both documents, but it is **not** proven —
`kvarBase`'s own saturation/LeadLag handling and the `kVArating` clamp of arm 1
have to be read before adopting it), probe the live r4133 engine for what it
actually dispatches under each `QMode` (epri-worker; RP3.2's probe transcript in
STATUS is the pattern), then fix it in **both lanes**. Everything the fix moves
is measured and excluded per the standing discipline: on the two power-flow decks
(`modes:windgen/windgen_daily.dss`, `windgen_snap_delta.dss`) the port's solved Q
would move from ≈ −2e-05 kvar to the deck's base (≈ 986 / 726 kvar) while r4133
stays at 0, so the exclusion is a **power**-channel ledger entry per case
(`kind: divergence`, the powers field), plus the pins the policy requires. The
two dynamics decks are the ones to *measure* rather than predict: `:1254` skips
the Q block in dynamics, so their divergence, if any, arrives through the
snapshot solve their own `solve` line performs before `Set mode=dynamic`. Whether the deck comments that misdescribe the mechanism
(`modes/windgen/windgen_snap_delta.dss:3` and its manifest note claim a "QMode=0
(constant-Q via PF) reactive dispatch" that does not exist) are corrected in the
same commit is that sub-step's call — corpus bytes were off-limits in RP3.2.

**Precondition (explicit, and it is the reason this is not scheduled here).**
It is a **product-crate behavior change** that moves solved powers on four
r4133-gating decks, i.e. exactly the kind of change the plan's own §1.1(e)
staging rule and the corpus gate make expensive mid-plan — so it runs **only on
the user's go-ahead**, as its own sub-step with its own audit pair, never folded
into another sub-step's commit.

**Blocks §RP5.2** (the plan's closing record), **not RP4.1** — measured: the
unmask compares *properties*, our `kvar` render reads `kvar_base`
(`elements/pc/windgen/accessors.rs:431`), which the dispatch never writes, so the
four RP3.2 entries' `rust` values survive this fix unchanged and no other
property cell depends on `Qnominalperphase`. Tier: `opus-xhigh` (a behavioral
port change in both lanes, with a live probe and per-case ledger work).

**Kill criterion:** the probe shows r4133's dispatch under `QMode=0` is a
deliberate design (e.g. some other site fills `Qnominalperphase` for mode 0 that
the port already reproduces) — then there is no reproduced bug, and the finding
is closed as refuted with the evidence, in STATUS, rather than "fixed".

**Acceptance:** either the arm is implemented in both lanes with its divergences
excluded per case and pinned, or it is refuted with a cited probe; either way
STATUS §WP-RP3 carries the verdict and this section is marked as executed.
Outcome: the last reproduced upstream bug this plan uncovered stops living in
prose.

### RP3.11 — the `Save`/`Dump` re-serialization surface (opened by the RP3.3 audit settlement)

**Why it exists.** Every echo row in this plan says the same thing about a
*compare*: r4133's getter answers the parse store, ours answers the live field,
and the cell is excluded. RP3.3 measured, for the first time, what that same
difference does on a surface **no oracle channel compares** — the re-serializers.
Pascal `TDSSObject.SaveWrite` (`Version8/Source/General/DSSObject.pas:145-165`)
writes `PropertyValue[iProp]`; the port's `save_write`
(`crates/dss-core/src/report/save/save.rs`) renders each property through
`ClassProps::get_value`, the live field. *(**Corrected at execution, 2026-09-03,
and the correction is half the verdict:** `PropertyValue[iProp]` is
`Get_PropertyValue` → the **virtual** `GetPropertyValue` (`:45`, `:117-120`),
which **49** r4133 units override to answer *live* on a hand-picked index set —
so "the store" is not a meaning of `Save` in r4133 at all, and `model=3` is
printed only because `TGeneratorObj.GetPropertyValue` happens to have no arm 6
(`PCElements/generator.pas:3007-3038`) while `TStorageObj`'s arm list carries
`propMODEL` (`PCElements/Storage.pas:1525-1596`). The port's doc comment was
re-written accordingly.)* On `modes:ncim/ncim_pv_pq.dss`, after the converged NCIM
solve that moves the generator PV→PQ, the two `Save Circuit` outputs are
(both measured 2026-08-24):

* r4133 (RP3.3 part A2, live DLL): `New "Generator.g1" bus1=genbus phases=3
  kv=12.47 kW=800 model=3 maxkvar=1500 minkvar=-1500 Vpu=1.01`
* the port (`cargo run -p dss-cli`): `New "Generator.g1" PF=0.88 Bus1=genbus
  Phases=3 kV=12.47 kW=800 Model=4 Maxkvar=1500 Minkvar=-1500 Vpu=1.01`

Re-compiling the port's line yields a **PQ generator with no Q limits in play**
instead of the authored Q-limited PV one — a different problem, not a different
spelling. The second difference in the same line is of another kind again: the
port emits `PF=0.88`, a property the deck never typed and r4133 does not print,
i.e. a **property-sequence** divergence (`PrpSequence` / `next_property_set`),
not a render one.

**Scope.** Two questions, both decided once for every class rather than per pair.
(1) *What is `Save` for?* — re-creating the **authored** circuit (the store, what
r4133 does) or dumping the **current state** (the live field, what we do). r4133
is the behavioral authority, but "match r4133" here is not automatic: the store
it prints is stale by construction, and the no-bug-reproduction policy
(CLAUDE.md, 2026-08-02) forbids reproducing a *bug* in the product — so the
sub-step must decide whether the echo is a bug at all on this surface or the
documented meaning of `Save`. (2) The same question for `Dump`
(`report/save/dump.rs`), whose Pascal twin goes through `GetPropertyValue` and
therefore echoes exactly where the census says it echoes. The `PF=0.88`-class
sequence difference is settled in the same sub-step, from `PrpSequence`'s own
Pascal semantics.

**Do first:** measure the exposure before deciding anything — which `save*` /
`dump*` / `props_roundtrip` goldens would move under each answer, and an
epri-worker probe of r4133's `Save Circuit` on one deck per echo *category*
(`EchoParse`, `EchoDefault`, `EmptyCollectionRender`, `LiveSemanticsDiffer`), so
the decision is taken against measured r4133 bytes and not against the Pascal
read alone.

**Precondition.** It runs **after RP4.1**: the echo table is the list of pairs
where the two serializers can disagree at all, and it is not final until the
unmask has settled the residual triage. It is a product-crate behavior change
with golden exposure in both lanes, so it is its own sub-step with its own audit
pair, never folded into another commit.

**Kill criterion:** if matching r4133 on this surface requires the product to
print a value the engine knows to be stale — i.e. reproducing the echo as
*behavior* rather than tolerating it as a compare exclusion — stop and report:
the 2026-08-02 policy forbids it, and the outcome is then a recorded, pinned
divergence from r4133's serializer instead of a port change.

**Blocks §RP5.2** (the closing record), **not RP4.1** — the unmask compares
properties through `compare_all_properties`, which never reads a `Save` or
`Dump` byte on the r4133 channel. Tier: `opus-xhigh`. *(**Discharged
2026-09-03** — the sub-step landed, so §RP5.2 waits on §RP3.10 alone; see the
dated lines below.)*
**Acceptance:** both surfaces carry a recorded verdict; whatever stays divergent
from r4133 is pinned by an expected-value test naming both serializations; the
`PF=0.88`-class sequence difference is explained or fixed; goldens that
legitimately move are regenerated with the argument in the same commit; STATUS
§WP-RP3 carries the record.
Outcome: the last surface where this plan's echo mechanism has a behavioral
consequence stops being unowned.

**Executed 2026-09-03 — verdict `KEEP_LIVE_PINNED` on BOTH surfaces.** The kill
criterion **fired**: matching r4133's `Save` on the deciding cell means printing
`model=3` while `gen_model == 4` in the same process, which the 2026-08-02
policy forbids — so the outcome is a recorded, pinned divergence from r4133's
serializer, not a port change. `Dump` shares the one `get_value` with `Save`,
`?`, the property API and batchedit (as both Pascals share their one virtual
getter), so the same cell settles it, with all 88 committed `dump*` artifacts
behind the alternative. The `PF=0.88`-class sequence difference is **explained,
not fixed** (0.14.5's property-tracking seeds, which r4133 has no counterpart
for and which the AltDSS JSON export — a surface with no r4133 counterpart
either — depends on; no `set_as_next_seq`/`clear_seq` site moved). What did
change is structural only, lane-unconditional, and taken as the *union* of both
upstreams' `SaveWrite` guards, every one of which exists to keep the emitted
deck re-compilable: P1 the unconditional `CalcVoltageBases`
(`Common/Circuit.pas:2716-2740`; 0.14.5's `! CalcVoltageBases` cost every saved
circuit its bus `kVBase`s), P2 the LoadShape `npts`-first branch
(`General/DSSObject.pas:144-173`), P3 `TXYcurveObj.SaveWrite`
(`General/XYcurve.pas:978-1003`), P4 `TRegcontrolObj.SaveWrite`
(`Controls/RegControl.pas:1399-1421`). Exposure: **1** golden content line +
**1** lock digest (a self-golden, so no `tools/golden/*.py` run and no ledger
row), **0** literal tests moved, **8** pins added — four re-compilability, four
divergence pins naming both serializations. Gate green in both lanes;
`lane_diff` **Δ = 0**. Tier as executed: `opus-xhigh`. Full record:
STATUS §RP3.11.
*Audit settlement (same day):* the union statement above was true of the policy
but not yet of the tree — both auditors found it shipped for 6 of the 8 upstream
overrides. Settled with three more lane-unconditional guards, all of them
re-compilability: **P5** `TXfmrCodeObj.SaveWrite`
(`CAPI:General/XfmrCode.pas:667-745` — r4133's own save keeps the active winding
only), **P6** `TDynEqPCE.SaveWrite` (`CAPI:PCElements/DynEqPCE.pas:252-273`, the
`UserDynInit` tail r4133 has no counterpart for) and **P7** the sizing-property
hoist for the five curve classes neither upstream guards (`TCC_Curve`,
`GrowthShape`, `PriceShape`, `TShape`, `Spectrum`). Exposure stays **0** golden
bytes and **0** ledger rows; pins 8 → **11**, all registered in
`props_r4133_replay.rs::RP311_SERIALIZATION_PINS`. Every finding's disposition is
in STATUS §RP3.11 ("audit settlement").
**Landed 2026-09-03** — `97107e54` (sub-step: P1–P4, eight pins, one golden
content line + one lock digest), `0194b086` (audit settlement: 14 raw findings,
12 distinct — 10 fixed, 2 recorded, 0 refuted; P5–P7 and three more pins) and
this docs commit. Gate green in both lanes, each exit code read individually —
**4 439 passed / 0 failed / 5 ignored** per lane over 74 binaries, the corpus
gate unfiltered over the full 523-case population with every ledger entry hit
and none stale; `lane_diff` owed (product `src/` moved) and **PASS, Δ = 0** on
every gated kind. Verdict as executed: `KEEP_LIVE_PINNED` on both surfaces.
Full record: STATUS §RP3.11.

### RP3.12 — `autotrans.wdgcurrents` on the regulator decks (opened by RP3.9's P0 open item)

**Why it exists.** RP3.9 closed with one item it refused to absorb: the **34**
`autotrans.wdgcurrents` cells on `controls:autotrans/{autotrans_both,
autotrans_reg,midi_autotrans,midi_autotrans_both}` — 3–7.5 % apart, port ==
`capi_v0145`, r4133 alone differing — had no owner and no root cause, only
RP2.4's display-class `OutOfScope` filing, which a multi-percent gap in a solved
current cannot be. A read-only investigation decomposed it before the sub-step
was written.

**Verdict: `UPSTREAM_BUG` in r4133, never reproduced; the port is correct; no
product-crate line changes.** r4133's `RegControl` reaches its controlled
element through five unchecked `TTransfObj(ControlledElement)` casts
(`RegControl.pas:926`, `:1026`, `:1296`, `:1370`, `:1479`) although
`TAutoTransObj = class(TPDElement)` (`AutoTrans.pas:88`) is not one and
`TAutoWinding` lays its fields after `Rdcohms` at other offsets than `TWinding`,
so `Increment := TapIncrement[TapWinding]` (`:1249`) reads the winding's
`MaxTap` = 1.1 pu and `PendingTapChange := Round(BoostNeeded / Increment) *
Increment` (`:1250`) zeroes every realistic boost. r4133 therefore never taps an
AutoTrans (0 event-log lines on all four decks) and its numbers are the
*unregulated* circuit; the same defect is in r3723 and r4088, and the
DSS-Extensions fork fixed it while refactoring. Upstream report (gitignored):
`investigations/to_opendss/50-regcontrol-autotrans-ttransfobj-typecast.md`.

**Obligations, as landed.** The 34 cells are re-filed out of RP2.4's
display-class bucket into their own `UPSTREAM_BUG` disposition
(`props_r4133_replay::RP312_UPSTREAM_BUG` / `Owner::Rp312`, `DECLARED_RP312 =
(8, 1, 0)`, `DECLARED_OUT_OF_SCOPE (229, 22, 0) → (221, 21, 0)`, every count
re-derived from the census artifacts); one two-leg expected-value pin names both
engines' numbers and reproduces r4133's census literal byte for byte from the
port's own state with the RegControl disabled
(`props_r4133_pins::autotrans_wdgcurrents_stay_regulated_where_r4133_never_taps_the_autotrans`);
the four case-level `r4133` `kind: "skip"` entries are **drafted only** — all
four decks are `engines: "capi_v0145"`, so an entry would be stale on arrival —
and a both-ways tripwire reds the day a deck gains the channel without one; and
the false ledger cause `autotrans-regcontrol-tap` ("a last-ulp voltage nudges the
tap … FPC-vs-Delphi"), exactly the conditioning excuse CLAUDE.md forbids, is
renamed to `regcontrol-autotrans-typecast` and rewritten, with dated corrections
in `docs/upgrade/DIVERGENCES.md`, `docs/upgrade/sweeps/capi015_vs_r4088.md`,
`docs/upgrade/known_diffs_burndown.md` and the vendored census README.

**Blocks nothing** — all four decks are capi-only, so the r4133 channel gates
none of them and RP4.1 gains no precondition. Tier as executed: `opus-high+`
(exec and both audits).

**Landed 2026-09-03** — `09e70233` (sub-step), `c2a8b68c` (audit settlement:
12 findings, 11 distinct — 9 fixed, 2 recorded, 0 refuted) and this docs
commit. Gate green in both lanes, 4 290 tests per lane; `lane_diff` not owed (no
`src/` line moved). Full record: STATUS §RP3.12.

### RP3.13 — the two NCIM port bugs (opened by RP3.11's P0 findings)

**Why it exists.** RP3.11's mandated round-trip measurement of `Save` walked
`modes:ncim/ncim_pv_pq.dss` on both engines and turned up two engine-side
defects that have nothing to do with serialization and no owner anywhere in this
plan. Recorded with their evidence in STATUS §RP3.11 ("Open, recorded not
chased", items (a) and (b)). RP3.11 drafted this section as a **proposal** it
was not authorized to act on; the plan owner **accepted and scheduled it on
2026-09-03**, and it ran the same day as its own sub-step with its own audit
pair. The two bullets below are RP3.11's evidence as written, kept verbatim as
the sub-step's starting point; what the investigation actually found follows
them.

* **(a) A plain user script panics a `#![forbid(unsafe_code)]` product crate.**
  `crates/dss-core/src/solution/solution/ncim.rs:683` — *index out of bounds: the
  len is 1 but the index is 1*. The arm writes `gobj.delta_q_nom[j]` for
  `j < nphases`, but the vector is still the length-1 one built at `:540`
  (`vec![gobj.q_nominal_per_phase]`); the per-phase sizing (`:277`,
  `vec![0.0; nphases]`) sits in a branch a model-4-at-birth generator never
  enters. An 11-line repro with no `Save` involved was measured, and r4133 on
  the identical deck does not crash. **Release-blocking**: a supported input
  aborts the engine.
* **(b) NCIM PV→PQ reporting violates KCL.** On the unmodified deck the port
  reports `Generator.G1` −800 kW / −431.8 kvar (42.0103 A) while the solve
  injects the clamped −1500 kvar (78.5593 A on r4133); node voltages and the
  Line/Load rows match r4133 digit for digit, so KCL at `genbus` is off by
  1068.2 kvar. Same family as the Newton stale-`Iterminal` bug (CLAUDE.md
  §"Known upstream bugs"), and it is what makes RP3.11's saved `Model=4` line
  look self-consistent while re-compiling to a different problem.

**Do first:** decide whether (a) and (b) are one defect or two — (b) is a
reporting read of the same PV→PQ arm — then fix in **both** lanes, since neither
is an upstream behaviour to reproduce. **Kill criterion:** if (b) turns out to
be r4133's own defect rather than the port's, it stops being a fix and becomes a
recorded, pinned divergence (the port already reports the physical current), per
the 2026-08-02 policy. **Acceptance:** the panic is impossible from any deck
(pinned by the repro), the reported generator power satisfies KCL at the bus on
the ncim decks, and the `modes:ncim/*` golden/corpus exposure is measured before
either change lands. **Blocks nothing in this plan** (no property cell moves —
the `Save`/`Dump` verdict is independent of it), so it can run after §RP5.2.
Tier: `opus-xhigh` (a solve-side engine change in both lanes with golden and
corpus exposure). Runs **only on the user's go-ahead**, as its own sub-step with
its own audit pair.

**Verdict: `PORT_BUG` × 2 — the kill criterion did not fire.** (b) is the port's
defect, not r4133's: r4133's `Generator.G1` numbers close KCL at `genbus` to the
printed digit while the port's missed it by 1068.2 kvar, and the port's own
`exec/view.rs` NCIM override — the reader the corpus gate uses — already agreed
with r4133, so the engine disagreed with itself. (a) and (b) are **two** defects,
not one: (a) is a sizing overrun in `ncim_init_pq_gen`, (b) a missing *reporting*
arm; (a) makes the promotion path unreachable, which is why (b) was only ever
seen on the PV→PQ side. The read-only investigation found **two more** of the
same family, both fixed here: **Bug C**, the flat-start slack override writing
`node_v[1..3]` on a circuit with fewer than three nodes (a second panic), and
**Bug B′**, `TVsourceObj.GetCurrents`' NCIM arm (`R4133:VSource.pas:1194`),
which the port also lacked. B′ was first recorded as measured-and-open; the
coordinator then had it fixed **inside this sub-step, before its commit** (a
measured gap is never parked), so all four defects land together.

**Obligations, as landed.** All lane-unconditional (no `cfg`, no `compat::`;
both lanes get the same values): `deltaQNom` is sized per phase for every NCIM
generator (`solution/solution/ncim.rs:566`, r4133 `Common/Solution.pas:1678-1679`
vs its three per-phase writers `:2107` / `:2152-2154` / `:2254-2256`, and
`:1928-1930` for the model-3 sizing it already had); the
flat-start slack override is clamped to the node count (`ncim.rs:244`,
`Solution.pas:1650-1654`); a new `SysCtx.ncim` (`elements/traits.rs:542`,
`solution/solution/state.rs:704`) carries `Algorithm = NCIMSOLVE` to the
reporting arms, and `TGeneratorObj.GetCurrents`' NCIM arm
(`R4133:PCElements/generator.pas:1406-1410`) is ported at
`elements/pc/generator/accessors.rs:363`, ahead of the `LastSolutionWasDirect`
shortcut exactly as in Pascal; the gate-only override `ncim_generator_currents`
is **deleted** from `exec/view.rs` so one live state feeds every reader (the
RP3.8 principle). The swing `VSource` follows the same rule and needs one extra
step, because `TVsourceObj.CalcInjCurrAtBus` (`R4133:VSource.pas:1085`, reached
from `GetCurrents` at `:1194`) sums *every* element at the swing bus, which one
element cannot do from inside its own `get_currents`: the sum is ported as
`ncim_stamp_swing_source_currents` (`solution/solution/ncim.rs:772`), called once
as the last statement of `do_ncim_solution` (`ncim.rs:963`) and stamped into the
source's `Iterminal`, and the NCIM arm of `TVsourceObj.GetCurrents`
(`elements/pc/vsource/solve.rs:315`, guarded `sys.ncim && node_ref.first() ==
Some(&1)` as Pascal's `NodeRef^[1] = 1`, **plus** the marker
`VSource::ncim_swing_stamped_at` that says the stamp is this element's and current
for this `SolutionCount`) returns that stamp — after which
the second gate-only override, `ncim_swing_source_currents`, is **deleted** from
`exec/view.rs` too and `snapshot_elements` has no NCIM special case left at all.
**Nine** expected-value pins in `crate::exec::tests::ncim` (eight in the sub-step
and a ninth from its audit settlement), registered in `RP313_NCIM_PINS` +
`every_rp313_ncim_pin_exists_and_is_cited`
(`crates/dss-core/tests/props_r4133_replay.rs`) so a rename cannot orphan the
record
(`ncim_pq2pv_promotion_does_not_panic_and_closes_kcl`,
`ncim_missing_voltage_bases_does_not_panic`,
`ncim_below_three_nodes_does_not_panic`,
`ncim_generator_reports_the_dispatched_q_not_the_declared_kvar`,
`ncim_gate_reader_and_ordinary_reader_agree`,
`ncim_swing_bus_carries_no_pc_element_on_the_gated_decks` — widened to red on a
second slack-node *source* as well —,
`ncim_vsource_export_currents_match_oracle`,
`ncim_second_slack_node_vsource_reports_its_own_current`,
`ncim_swing_sum_subtracts_pc_terminals_and_closes_kcl`), each naming both
engines' numbers where an oracle exists: on the two-source deck r4133 has none —
its per-read `CalcInjCurrAtBus` recursion overflows the DLL's stack (own probe
2026-09-03) — so that pin asserts the Thevenin physics `|E2 - V| / |Z1| =
1388.97 A` and plain KCL at the bus instead. **Zero `ledger.json` entries, zero golden bytes, zero
tolerances**: the five gated NCIM cases are `engines: "r4133"` with no ledger
entry, the gate's element channel already read the deleted overrides' formulas,
and `tests/golden/ncim/` is solve-side only — so no oracle observable moved and
the field-by-field exclusion obligation is vacuous rather than waived. **Four**
r4133 defects are proven and **not reproduced** (the `deltaQNom[j]` write over
the length-1 `InitPQGen` array, which corrupts the r4133 DLL — its solve still
answers, and the DLL then hangs on the first element access after it (own
re-probe in the audit settlement, 2026-09-03); `DOForceFlatStart`'s
`NodeV[1..3]` write, which corrupts its heap on a sub-3-node circuit; and
`GetCurrents`' partial fill, which leaves shared-`cBuffer` garbage in conductor
`NPhases+1` of r4133's own `Export Currents`); the upstream reports are the
gitignored `investigations/to_opendss/51-ncim-updategenq-deltaqnom-overrun.md`,
`52-ncim-doforceflatstart-nodev-overrun.md` and
`53-ncim-generator-getcurrents-partial-fill.md`. The **fourth** was found by the
audit settlement and is the only one that changes a reported number:
`TVsourceObj.CalcInjCurrAtBus` **adds** PC-element terminal currents (`cadd`,
`VSource.pas` l.1169) where it subtracts the PD ones (l.1135), so the swing
source's reported current violates KCL by exactly twice the PC current at its bus
— measured live on a deck with a load bonded onto the swing bus (r4133
`Vsource.source I1 = -12.910456 + 52.625721j A` = `-I(Line) + I(Load)`, residual
`2·I(Load)`; the port subtracts both loops and prints `136.936 ∠135.67`, KCL
closed). Unreachable on every gated case (the widened tripwire is the proof);
report `54-ncim-calcinjcurratbus-pc-sign.md`.

**Blocks nothing** — no property cell moves, so neither the unmask (landed) nor
§RP5.2 gains a precondition. Tier as executed: `opus-xhigh` (exec and both
audits). **Executed 2026-09-03**; the sub-step, its audit settlement and this
docs commit are named by sha in STATUS §RP3.13, which is the full record. **Bug
B′ landed here too, not after it:** `Export Currents`' `Vsource.SOURCE` phase-A
magnitude read `3.24074e-05` / `3.24074e-05` / `4.42577e-05` / `0.0106809` A on
`ncim_pq` / `ncim_pv_pq` / `ncim_midi` / `Xmission_System_Kundur2Area` against
r4133's `90.0718` / `64.2127` / `124.964` / `20295.6`; the first three now print
`90.0718 ∠158.27` / `64.2127 ∠-150.28` / `124.964 ∠161.49`, digit-identical to
r4133's own `EXP_CURRENTS.CSV` rows (pin
`ncim_vsource_export_currents_match_oracle`), and `Xmission_System_Kundur2Area`
stays 4/4 green on its live r4133 whole-model compare, which includes the swing
`Vsource`'s current, power and loss. No gated channel moved (the gate reads
`snapshot_elements`, whose values are unchanged bit for bit and are now `==` the
element path), and the tripwire pin above still reds the day a gated NCIM deck
puts a generator on the swing bus. **Open after it:** nothing on the NCIM path —
what is left is RP3.11's AC-5 round-trip question and the two `Dump`
store-vs-live cells (`capacitor.faultrate`, `autotrans.tap`), both record-only
and both listed in STATUS §RP3.13's own open items.

---

## WP-RP4 — The unmask

### RP4.1 — `all_properties` on the r4133 channel (G1.1's deliverable)

**Precondition 1, added by RP2.3's audit settlement (2026-08-23) — narrow the 20
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

**Precondition 2, added by RP3.1's audit settlement (2026-08-24) — landing the
staged entries is also an accounting commit, and nothing does it for you.** The
replay accounting (`props_r4133_replay.rs`) has no link that reads
`tests/corpus/ledger.json`: the chain is `Link::ORDER`'s four links and
`declare` routes every bin-7 row on `BIN7_ROOT_CAUSE` to `Owner::Rp3`
unconditionally, so `DECLARED_RP3` does **not** shrink when the entries land and
no test would notice the omission. In the same commit that lands them: retire
each settled `RP3_ROUTING` row whose entry landed (its rows are excluded now,
not merely declared), shrink `DECLARED_RP3` by exactly those rows, and re-state
`the_staged_r4133_property_entries_have_not_landed_yet` against whatever is
still staged — that tripwire goes red the moment the first `property`-scoped
`r4133` entry appears, which is how this precondition announces itself. The same
applies to RP1.4's staged entries — but **not to RP3.3**, which
closed `ECHO` (2026-08-24): its exclusion is a `PROPS_ECHO_R4133` row that
shipped in its own commit, it staged no ledger entry, and its `RP3_ROUTING` row
is already retired to `0, 0` with `DECLARED_RP3` down to `(6, 3, 6)`, so RP4.1
has nothing to retire for it — and to **RP3.2's four**
(`r4133-windgen-kvar-dispatched-daily` / `-delta` / `-dyn` / `-dynfault`), which
are already drafted in STATUS §WP-RP3 and whose census derivation
(`the_rp32_census_decomposition_is_read_off_the_corpus`) fixes the count at four,
and to **RP3.4's two** (`gic-pct-r2-honoured-gictransformer-r4133-props` /
`gic-pct-r2-honoured-midi-r4133-props`, drafted verbatim in the same STATUS
block on 2026-08-24), whose census derivation
(`the_rp34_census_decomposition_is_read_off_the_corpus`) fixes the count at two —
one per `%R`-specified GICTransformer, the corpus's other twenty being
ohms-specified and cell-free. With RP3.4 the staged set is complete at
**RP3.1's two** (`r4133-swtcontrol-delay-ignored-time` /
`r4133-swtcontrol-delay-ignored-midi`, named here since the RP3.4 audit
settlement — they were carried only by the arithmetic) **+ RP3.2's four +
RP3.4's two = eight**, plus RP1.4's, and all
three sub-steps keep their `RP3_ROUTING` rows and their share of
`DECLARED_RP3` `(6, 3, 6)` until this commit retires them. The tripwire
`the_staged_r4133_property_entries_have_not_landed_yet` lists the same eight.

**Pre-measured by RP3.8 (2026-09-02), so this sub-step need not rediscover it.**
With the mask bypassed (`DSS_PROPS_CENSUS=claims`) the five surfaces RP3.8 made
live produce **105 divergent r4133 cells / 89 in scope**, of which **103 are
RP2.4's display class** (the port prints `float_to_str_ex`, r4133 its
`%.6g`/`%-.8g` of the same double; worst rel `4.029e-08`, four orders under
`R4133_DISPLAY_FLOOR`) and therefore need nothing here, while `indmach012.pf`
and `storagecontroller.kwhtotal` produce **zero**. The remaining **2** are the
one r4133-side exclusion candidate this sub-step inherits:
`modes:makeposseq/makeposseq_ctrl.dss` `StorageController.kWTotal`
(`33.3333333333333` vs `100`) and `.kWActual` (`-0.333333333333333` vs `-1`) —
a factor of `Fnphases`, because r4133's `TStorageObj.MakePosSequence` writes
`' kWrating=%-.5g'` (`Version8/Source/PCElements/Storage.pas:3979-3985`) where
the class's property is `kWrated` (`:647`), so its own edit is an unknown
parameter and the rating is never scaled; 0.14.5 fixed it by ordinal
(`src/PCElements/Storage.pas:3340`/`:3349`) and the port follows. The case is
`capi_v0145`-only today, so the r4133 channel does not gate it — if its
`engines` key ever changes it needs a cited exclusion + pin, never a
reproduction (upstream report `investigations/to_opendss/49-storage-makeposseq-
writes-kwrating.md`).

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

**As executed, part 1 of 2 (2026-09-03, RP4.1 P2 — the staged entries landed and
the accounting moved; the section above is the pre-execution text and stays).**

- **"plus RP1.4's" (¶2 above, and "the ledger entries staged by RP1.4/RP3" and
  the kill criterion's "staged by RP1.4/RP3") is wrong, and RP4.1 landed
  **eight** entries, not eight-plus-RP1.4's.** RP1.4 staged **no** ledger entry:
  its `gendispatcher.weights` divergence is whole-solution, no pin could cover a
  flip to `engines: "both"`, its decks therefore stay `capi_v0145`, and its
  artifact is a `PROPS_015X` allowlist row (STATUS §WP-RP1, RP1.4's own record).
  `LEDGER_ENTRY_PINS` has carried exactly eight rows since RP3.4 and none of them
  is RP1.4's. Coordinator ruling of 2026-09-02, binding for this sub-step; the
  same phrase was corrected in the tripwire's doc, which is live code.
- **The eight landed verbatim from their STATUS drafts** (RP3.1 ×2, RP3.2 ×4,
  RP3.4 ×2) into `tests/corpus/ledger.json`, with the two new `causes` keys
  `swtcontrol-delay-not-wired` and `windgen-kvar-renders-dispatched-q`; RP3.4's
  two reuse the existing `gic-pct-r2-ignored`. The ledger is now **53 entries /
  29 causes** (`ledger_is_structurally_valid`).
- **Precondition 2's accounting move needed a mechanism, not just a hand edit.**
  The plan (and `DECLARED_RP3`'s own note) said the shrink is a hand edit because
  no chain link reads `ledger.json` — but `DECLARED_RP3` is *asserted against the
  measured walk*, so editing the constant alone would have reded
  `every_example_row_is_claimed_or_declared_exactly_once`. RP4.1 therefore landed
  `RP3_LEDGERED` (`props_r4133_replay.rs`), an interception in the shape
  `RP38_SUPERSEDED` already uses — between the chain and `declare` — with its own
  bucket lock `LEDGERED_RP3 = (6, 3, 6)` and a both-ways guard
  (`the_ledgered_rows_are_excluded_by_entries_that_are_really_in_the_ledger`)
  that reads the entry ids back out of the live ledger. `DECLARED_RP3` is now
  `(0, 0, 0)` as a measurement, and `RP3_ROUTING`'s three `LEDGER` rows are
  retired to `0, 0` with their verdicts amended to "landed at RP4.1".
- **The tripwire was renamed**, its doc rewritten:
  `the_staged_r4133_property_entries_have_not_landed_yet` →
  `the_staged_r4133_property_entries_landed_at_rp41`. It now asserts the positive
  form (the landed `property`-scoped `r4133` set is exactly
  `LEDGER_ENTRY_PINS`' eight, each on the case its pin cites); the "no NEW
  un-reviewed entry" half moved to the `RP3_LEDGERED` guard next door.
- **Stale plan line numbers found while executing** (reported, never edited into
  the historical text above): `scheduler.rs:357-363` → `:359-365`;
  `scheduler.rs:710-717` → `:717-721`; `scheduler.rs:97-114` → `:97-116`;
  `harness/mod.rs:1452-1458` → `:3043-3049`; `capture.rs:619-664` → fn `:630-662`,
  doc `:618-628`; the population figures "96 r4133-only / 366 both / 462 gating"
  → the lock reads 97 / 367 / 464 (523 cases, 80 `large`, 396 non-large
  r4133-gating) and must be re-derived, not transcribed.
- **P2 alone leaves the tree knowingly red** and is therefore not a commit of its
  own: a landed `r4133` `property` entry is NEVER APPLIED while the scheduler
  mask stands, so `ledger.assert_all_hit` fails until P3 removes it (coordinator
  decision 4 — the whole sub-step is one commit). `population_lock` is red here
  too, by design: the `ledger=` component of the eight cases moved and P5
  regenerates the lock.

**As executed, part 2 of 2 (2026-09-03, RP4.1 P1/P3/P4/P5 — the flip, the
acceptance and the alarm; the section above is the pre-execution text and
stays).** Landed in the single RP4.1 commit; the full record is `STATUS.md`
§RP4.1.

- **The flip.** The per-channel `compare_all_properties = false` clears are gone
  from the gate path (`corpus_gate/scheduler.rs`) and the seeding path, and
  `force_properties` forces the property request on **every live non-`large`
  case** — `gates_capi ∪ gates_r4133` is every live case, so the two-arm test was
  replaced by that honest spelling; the `large` cost guard stays. 83 r4133-only
  non-large cases property-checked for the first time, **313** non-`large`
  `both` cases comparing their r4133 table (the audit settlement corrected this
  line: the other **54** `both` cases are `kind=large*` and the cost guard leaves
  them property-unchecked on both channels — an open item for the RP5.2 closing
  record, not a regression); 1 670 gating walks over 151 782 elements per full
  run. The forced population is pinned by
  `corpus_gate::scheduler::the_property_forcing_rule_is_every_live_non_large_case`
  (`FORCED_PROPS_POPULATION = (440, 313, 83, 44)`), added by the same settlement.
- **Precondition A, mechanism (b)** (coordinator decision 3): `ECHO_NARROWED`
  gives each of the 20 mixed echo rows its 66 measured `(rust, oracle)`
  spellings; the 62 pure echo rows keep the pair scope; `ECHO_CARVE_OUTS`' doc
  owns the inversion. `MULTI_LINK_ROWS` 135 → 0, restated as
  `MIXED_PAIR_NORM_ROWS = 135`.
- **Deviation, recorded (coordinator decision 1): acceptance is read as "zero
  UNCLAIMED cells IN SCOPE"** — the gate never compares an out-of-scope cell, so
  the plan's "per-cell accounting closes" applies to compared cells. No new
  census disposition was added. Measured at HEAD over one full-population run
  (440 cases × 2 channels, 1 059 178 rows): r4133 `UNCLAIMED` in scope **0**,
  `ledger-hit` in scope **30** (the eight entries, cell for cell), and the 504
  out-of-scope `UNCLAIMED` cells accounted per owner in the STATUS record. A
  second reading rule: acceptance is read off `props_census.json`, never off
  `claims_summary.json`, which collapses a mixed-disposition spelling to its
  weakest cell.
- **Kill criterion did not fire:** 0 ledger entries and 0 pins born from RP4.1's
  own residual triage, 0 residuals that could not be pinned. The one finding was
  a **stale exclusion** — the `ArrayForm` rows `swtcontrol.normal`/`.state`,
  dead since RP3.7(a) — dropped and re-owned by `RP37_SUPERSEDED`.
- **Deviation, recorded: the global assert counts the gating call site, not the
  tables' `visits`.** `assert_r4133_props_compare_ran()` is invoked once from the
  corpus gate's epilogue and self-silences under `DSS_GATE_ONLY`; the plan's
  literal "`visits > 0` summed over the tables" is unsound here, because sibling
  unit tests in the same binary drive the comparator and moved those statics
  through the whole masked era.
- **Gate:** 4 382 passed / 0 failed / 5 ignored per lane over 74 binaries
  (**4 427** on the settled tree, `corpus_gate` 135 → 138 tests);
  `corpus_gate` 523/523 unfiltered, ledger 53 entries / 1 534 hits, none stale.
  `corpus_gate` wall 141.78 s default / 142.94 s parity against 142.90 s /
  139.68 s before the unmask. `lane_diff` run anyway (no product line moved):
  PASS, `max |Δ| = 0` on all eight kinds.
- **Audit settlement (2026-09-03).** Nine findings, eight fixed, one recorded;
  the full per-finding disposition is in the STATUS §RP4.1 record. The three
  that changed what this section claims: the forced population is **313**
  non-`large` `both` cases (not 367) and is now pinned by
  `the_property_forcing_rule_is_every_live_non_large_case`, which is also the
  only guard that catches a *partial* re-mask; `check_echo_rows_are_live` grew
  the inverted staleness arm the per-cell narrowing had disabled for the 20
  rows; and `RP3_LEDGERED`'s retirement is now checked per **spelling**
  (`RP3_LEDGERED_UNNAMED`), since an entry pins one exact `(rust, oracle)` while
  the interception retires the pair.
- **As-executed line-number corrections.** The citations in the pre-execution
  text above drifted (`scheduler.rs:357-363` → `:359-365`, `:710-717` →
  `:717-721`, `:97-114` → `:97-116`; `harness/mod.rs:1452-1458` → `:3043-3049`;
  `capture.rs:619-664` → fn `:630-662`, doc `:618-628`; the population figures
  "96 r4133-only / 366 both / 462 gating" → 97 / 367 / 464 on the lock). They are
  recorded in `STATUS.md` §RP4.1 and deliberately **not** edited into the
  historical text here (coordinator decision 7, 2026-09-02).

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
`docs/plans-archive/` (the 2026-07 convention).
**Precondition, added by RP3.2's audit settlement (2026-08-24): §RP3.10 is
closed** — executed or refuted with evidence. It is the one reproduced upstream
bug this plan uncovered, and the 2026-08-02 policy does not let it be archived as
a note; if the user has not sanctioned the fix by then, it moves to
`ORPHANED_GAPS.md` with its evidence instead of vanishing with the plan.
**Acceptance:** no doc
disagrees with any other on counts or state; clean tree. Outcome: the plan
closes and GOLDEN_REBASE resumes with G1.1 satisfied.

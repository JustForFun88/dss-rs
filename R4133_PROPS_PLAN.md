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
triage opens; RP5 is last. Execution is on a **single branch only — never in parallel
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
| RP3.1–RP3.4 (+ any RP3.5+ opened by RP2.2) | `opus-high+` | `opus-high+` | `opus-high+` | one root-cause each, bounded surface, live-probe procedure prescribed |
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
The full-census bins below are re-derived in-scope by RP0.1 (its `bins.tsv`
assigns every pair to its bin); the treatment map binds every bin to the
sub-step that closes it:

| # | census bin (pairs / cells, full census) | root cause | treatment | lands in |
|---|---|---|---|---|
| 1 | boolean rendering, 75 / 297 593 | FPC `Yes`/`No` vs eleven non-empty Delphi spellings (`true/True/false/False/YES/yes/no/NO/n/y/Y` — `False` alone is 76 492 cells); the `''` renders in this bin are echo-defaults, not booleans | `BoolFold` rule; `''` cells go to the echo table (three pairs mix both — §1.2 replay bullet) | RP2.1 / RP2.3 |
| 2 | case-only + trailing space, 59 / 72 007 + 2 / 21 206 | THashList lowercasing (port = dss_capi) vs Delphi as-declared case; literal `'wye '`/`'Delta '` (`Transformer.pas:1762-1763`, `AutoTrans.pas:1818-1819`) | `CaseFold` + trim | RP2.1 |
| 3 | enum spelling + singletons, 8 / 4 400 | per-pair enum spellings (`Positive`/`Pos`) and per-pair semantics (`monitor.mode` decomposition render) | `EnumSynonym` rows + the S6 dossier (S6 = the triage's per-pair singleton list, enumerated exhaustively in RP2.2) | RP2.2 |
| 4 | array form, 21 / 122 554 | dss_capi `GetDSSArray` `[ 400]` vs Delphi comma/paren/bare forms | `ArrayForm` tokenizing compare | RP2.1 |
| 5 | empty-vs-value + display defaults, 44 / 442 369 | `PropertyValue[]` echo: un-overridden `GetPropertyValue` returns the parse store / `InitPropertyValues` default (`DSSObject.pas:112-115`; e.g. `Reactor.pas:1087-1140`, `Transformer.pas:1914-1919`) | `PROPS_ECHO_R4133` exclusion rows + pins | RP2.3 |
| 6 | numeric display precision, 61 pairs full / 37 in-scope | Delphi `%-.5g`/`%-.6g`/`%-.8g` getters (`Vsource.pas:1327-1343`); measured worst rel 6.43e-5 (`load.pf`) | the r4133 props display floor | RP2.4 |
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
- **The replay-accounting test** (new integration test
  `crates/dss-core/tests/props_r4133_replay.rs`) — reads the vendored census
  extracts (RP0.1) and pushes every **example row** of `examples_full.txt`
  through the full r4133 policy in the documented chain order (shape allowlist
  → normalization → echo table → display floor). `examples_full.txt` carries
  one row per **distinct (rust, r4133) spelling** per pair, untruncated (the
  pair files' own example columns are cut at ~34 chars and cannot feed a
  tokenizing or numeric compare; a single example per pair would hide the three
  in-scope **mixed pairs** — `recloser.eventlog`, `regcontrol.idle`,
  `relay.distreverse` — whose cells mix foldable boolean spellings with
  `''`-echo cells). The test asserts every example row is claimed by the
  **first matching mechanism in the chain**, that each pair's claim set is
  admissible for its `bins.tsv` bin (bin 1 admits `BoolFold` for foldable
  spellings **and** an echo row for its `''` cells — a mixed pair legitimately
  holds both; single-claim is per example row, never per pair), and that every
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
  (provisionally ≈2e-4, inside the measured empty band; RP2.4 derives the final
  value). It is **not** a `tol_for` tier change and touches no `Tolerances`
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
spellings, and the three mixed pairs of §1.2 need every spelling represented),
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
`tmp/props_census.json` + the four pair/shape extracts in the RP0.1 format. This
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
per-cell rows that byte-match the local full census's rows for that family;
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
property compare is bit-for-bit unchanged (A/B run on one family).

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
row set for bins 1/2/4 of §1.1: `BoolFold` (~75 pairs minus the `''` echo rows),
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
Claim semantics are per example row, first-match-in-chain (§1.2) — the three
mixed pairs keep their `''` example rows unclaimed here (declared for RP2.3's
echo rows) while their foldable rows are claimed by `BoolFold`. Extend the RP0.2 census knob with the
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

### RP2.3 — the echo-exclusion table + pins

Land the `PROPS_ECHO_R4133` rows for bin 5 (the 44 empty-vs-value pairs — echo
defaults and never-parsed stores) and the 12 echo-rooted genuine-jump pairs of
bin 7 (§1.1 table; the census grounding proves each against its r4133 site:
`Transformer.pas:1914-1919` pctperm/repair, `Reactor.pas:1087-1140` kvar echo,
`RegControl.pas:1452` remoteptratio frozen default (`PropertyValue[27]`; the
live value re-inits at `:484` while the store stays `'60'`), the `%pmin*`/`lpftau`/
`risefalllimit`/`pctperm` `InitPropertyValues` defaults), minus whatever RP2.2
already routed here (RP2.3 runs after RP2.2, §0), plus the `''`-cell echo rows
of the three mixed bin-1 pairs (`recloser.eventlog`, `regcontrol.idle`,
`relay.distreverse` — §1.2 replay bullet; the chain order does the per-cell
discrimination: foldable cells are claimed by `BoolFold` before the echo row
is consulted, so the echo row masks only the `''` cells). Every row: category tag + r4133 citation + the witness
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
named in STATUS.

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

---

## WP-RP4 — The unmask

### RP4.1 — `all_properties` on the r4133 channel (G1.1's deliverable)

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

# UPGRADE_PLAN — porting dss-rs from the r3723 baseline to OpenDSS 11.0.0.1 (r4133)

> Companion to `PORTING_PLAN.md`; same rules of engagement as `PHASE8_PLAN.md §0`
> / `GAPS_PLAN.md §0` (the upstream source is the spec; probe the target oracle,
> never guess semantics; `TODO(compat)` / `NOT_PORTED` discipline; no fudging —
> tolerances change only with empirical proof). **Prerequisite: FINAL ACCEPTANCE
> of the 1:1 r3723 port** (PORTING_PLAN §6 — Phase 8 + GAPS complete, all
> markers swept). Position in `PLAN_SEQUENCE.md`: **immediately after final
> acceptance, before DE_PASCALIZE** — (a) the Pascal-porting context is
> freshest here, (b) DE_PASCALIZE would otherwise refactor code this plan
> rewrites (double work), (c) DE_PASCALIZE Stage F's `oracle-parity` lane then
> pins the FINAL upstream target (r4133-parity), not an already-obsolete
> r3723-parity.
>
> **What this plan is.** The stable dss_capi line the port is calibrated to
> (0.14.5 = OpenDSS SVN r3723) is behind official OpenDSS. This plan moves the
> engine's behavior to **OpenDSS 11.0.0.1 (SVN r4133, "Charlottesville")** in
> two rungs, each independently gated:
>
> - **Rung 1 (WP-U1.\*) — the 0.15.x / r4088-line parity.** Spec = the FPC
>   source of dss_capi branch `0.15.x`
>   (`.inputs/dss_capi_with_git`, HEAD `e936d210` ≈ 0.15.0b4, "based on
>   OpenDSS SVN 4103") — the same codebase lineage the whole port came from,
>   diffed against the vendored 0.14.5. Primary oracle = **capi015** (see §1).
> - **Rung 2 (WP-U2.\*) — the r4133 parity.** Spec = the **Delphi** diff
>   `.inputs/electricdss-code-r4088-trunk` → `.inputs/electricdss-code-r4133-trunk`
>   (`Version8/Source`). No dss_capi exists for this delta yet; the oracle is
>   the official EPRI binary **oddie:r4133**. The delta is compact: a
>   protection-device overhaul (Relay/Recloser/Fuse/SwtControl) + a handful of
>   executive fixes; `PCElements/`, `Meters/`, `Parser/` and the whole solver
>   are byte-identical between r4088 and r4133.
>
> **Scoping inventories (committed, the WPs cite them):**
> `docs/upgrade/delta_capi_0145_015x.md` (Rung 1's item list),
> `docs/upgrade/delta_r3723_r4088.md` (the raw-Delphi cross-check for Rung 1 —
> what EPRI changed that dss_capi did/didn't adopt),
> `docs/upgrade/delta_r4088_r4133.md` (Rung 2's item list). Surveyed
> 2026-07-07; each WP re-verifies its items against the actual source at WP
> open (the PHASE7/8 "line refs confirmed at WP open" convention).
>
> **Stop-and-confirm cadence (same as PHASE8_PLAN §0):** after each WP (or a
> self-contained stage of one) run the §2 ritual autonomously, without pausing
> between its sub-steps; the single stop point is at the very end — then wait
> for the user's explicit confirmation (unless the user authorized several WPs
> in one pass).

## 0. Inputs — the source trees and what each is for

| Input | Role |
|---|---|
| `.inputs/dss_capi` (vendored, 0.14.5 = r3723) | the CURRENT spec — stays authoritative for everything not yet upgraded |
| `.inputs/dss_capi_with_git` @ `0.15.x` (0.15.0b4, ≈r4103) | **Rung 1's spec** (FPC, 1:1 with our porting lineage); diff base = tag `0.14.5` |
| `.inputs/DSS-Python` @ `fastdss` (0.16.0b2) | the Python API over 0.15.0b4 — the **capi015** oracle's bridge (vendored wheels in `tools/opendss/wheels/`) |
| `.inputs/electricdss-code-r3723-trunk` | the Delphi baseline (cross-checks only) |
| `.inputs/electricdss-code-r4088-trunk` | Rung 1's Delphi cross-check + Rung 2's diff base |
| `.inputs/electricdss-code-r4133-trunk` | **Rung 2's spec** (Delphi, `Version8/Source`) + the target binary |
| `tools/opendss/bin/{r3723,r4088,r4133}` | vendored official `OpenDSSDirect.dll` binaries (Oddie bridge) + `kmetis.exe`/`pmetis.exe` (A-Diakoptics tearing, since 2026-07-11) |

**Explicitly NOT in scope** (each stays `NOT_PORTED` with a loud error, or is
skipped as non-engine):

- **pyControl component + `Set pyPath=`** (named-pipe Python co-simulation,
  Windows `ShellExecute`) — same class as user-model DLLs: never. The
  engine-side **force hooks** (`ForceInjCurr`/`ForceY` + the
  `InjCurrent`/`ITerminal`/`Yprim`/`StateVar` options) ARE in scope (WP-U1.9).
- **Generic5OrderMach + FMonitor** — disabled upstream in 0.15.x (constructing
  one errors); nothing reachable to port.
- **A-Diakoptics** — behind a build ifdef in the dss_capi lineage (commands not
  exposed by either capi oracle), so it is not an *upgrade* item on any rung. It
  is ported separately per **`DIAKOPTICS_PSTCALC_PLAN.md` Part II**, and that work
  is **upgrade-neutral by evidence** (verified 2026-07-11): the official
  `Version8/Source/Common/Diakoptics.pas` is **byte-identical across
  r3723/r4088/r4133** (sha256-equal in all three vendored trunks), and the
  dss_capi `0.14.5 → 0.15.x` delta for the unit is mechanical API renames only
  (`SendCmd2Actors→SendADCommandToActors`, getter/setter forms, the flag moving
  from `Solution` to the DSS context — zero numeric change). Rung 1/2 owe it
  nothing; if a later official rev ever touches the file, the sha check in that
  plan's D10 flags it.
- **Pstcalc** — same verdict, opposite side of the fence: it IS compiled into
  every oracle, lands pre-acceptance as `DIAKOPTICS_PSTCALC_PLAN.md` Part I
  (WP-PF.1/2), and is upgrade-neutral by the same evidence (official
  `Shared/Pstcalc.pas` byte-identical r3723=r4088=r4133; capi `0.14.5 → 0.15.x`
  delta = a `uses` swap + `()` cosmetics). The Rung 1/2 gates just keep its
  Part-I goldens green — no WP-U work item exists for it.
- All C-API / COM / AltDSS / Oddie **API-layer** growth (buckets F/G of the
  inventories) — dss-rs has no C API by design (PORTING_PLAN).
- GUI/progress-bar/editor plumbing (the established no-op class).
- `GenController` — deregistered upstream in r4088 (dead class; if the r3723
  port registered it, WP-U1.6 deregisters with a loud "Object Class not found"
  parity error).

## 1. Oracle & gate policy — the multi-oracle era (binding for every WP)

The WP-U0 infrastructure (landed on branch `upgrade-test-infra`) makes the
oracle a **per-case property**, so upgraded and legacy behavior coexist in one
green gate while WPs land subsystem by subsystem, in parallel branches.

### 1.1 The four oracles

| Engine spec | What it is | Used as |
|---|---|---|
| *(default, no `oracle` field)* | pinned dss-python 0.15.7 / dss_capi **0.14.5** (`tools/golden/PIN.txt`) | the oracle for every observable NOT yet upgraded — unchanged |
| `capi015` | dss-python 0.16.0b2 (Oddie venv) driving its bundled dss_capi **0.15.0b4** (≈r4103) | **Rung 1's primary gate oracle** (scriptable, rich API, FPC like our lineage) |
| `r4088` | official EPRI `OpenDSSDirect.dll` 10.2.0.1 via Oddie | Rung 1 diagnostic cross-check (is a quirk dss_capi-specific?) |
| `r4133` | official EPRI `OpenDSSDirect.dll` **11.0.0.1** via Oddie | **Rung 2's gate oracle** + the plan's end-target |

Selection: manifest field `"oracle": "capi015"|"r3723"|"r4088"|"r4133"` on any
`SolvableCase` (vendored `solvable_now` or family manifests). The gate builds
one `Oracle` per spec (ping-verified against its own pin — a lost env var can
never silently compare the wrong engine). `tools/oracle/oracle_server.py`
carries the `capi015` engine binding; `tools/opendss/README.md` §3b documents
the mechanics. The Oddie venv + `bin/` are **mandatory `cargo test`
prerequisites** since WP-U0 (the `modes/upgrade_pilot.dss` case keeps the
machinery permanently exercised).

### 1.2 The same-commit rule (how behavior moves without ever being red)

A WP that adopts a newer upstream behavior lands, **in one commit**: the code
change + the `oracle` flip of every case whose observables move + the golden
migration for affected goldens (§1.5) + the `known_diffs.json` retirement of
the now-ported divergence entries (§1.6) + the `docs/upgrade/DIVERGENCES.md`
ledger entry (§1.4). The gate is green before and after; there is no
intermediate state where a case compares against an engine it deliberately
mismatches.

New-feature decks (NCIM, WindGen, per-phase protection …) follow the GAPS §3.1
lifecycle **plus** the target: committed `pending: true` with `wp: "WP-U…"`
AND `oracle: "<target>"` declared up front; the porting WP flips `pending:
false` and proves the live compare green against the declared target.

### 1.3 Relaxations — exactly three, and what is NOT relaxed

1. **Iteration counts: `Rust <= oracle`** for every target-rev case (`oracle`
   set); exact equality remains mandatory for default-oracle cases. Rationale:
   RESONANCE_PLAN's refinement will legitimately shorten the fixed point, and
   Delphi-vs-FPC/Rust drift makes exact parity vs EPRI binaries brittle.
   Discipline: while RESONANCE has not landed, a strict `<` is *suspicious* —
   the gate prints it loudly, and the WP records each `<` case in STATUS with
   a one-line cause (a systematic `<` before RESONANCE is a bug until proven
   otherwise, CLAUDE.md prove-it rules). NB (audit WP-U0): libtest captures
   stderr on passing tests, so the note is visible only under `--nocapture` —
   an upgrade WP's ritual step 1 runs its touched live gates with
   `--nocapture` and reads the notes before declaring the step done.
2. **No byte-exact text/print-format gates against EPRI engines.** Delphi
   `Format`/`Str` differ from FPC in last-digit rendering; upgraded report
   behavior is gated with the WP8.1 `compare_export` **numeric-token**
   comparator (headers exact as tokens, numbers by tolerance, identifiers
   case-insensitive) or by live-model probes — never raw text diffs.
   Byte-exact goldens remain ONLY for observables still pinned to the default
   0.14.5 oracle; when their subsystem upgrades, they migrate per §1.5.
3. **Event logs against EPRI:** compared as exact strings only after masking
   documented per-rev format deltas (the r4133 wording overhaul) — masks live
   in the comparator with the same proof discipline as
   `tests/TOLERANCE_NOTES.md` entries; the *sequence* (order, hours, devices,
   actions) is never relaxed.

**Not relaxed:** every numeric tolerance class (`tol_for`,
TOLERANCE_NOTES.md) is shared with the mandatory gate unchanged; discrete
state exact; node order exact; element name sets exact; no new tolerance
classes without empirical decomposition proof; never loosen a band to hide a
divergence. The iteration relaxation is a *policy* decision (documented
here), not a tolerance fudge.

### 1.4 The divergence ledger — `docs/upgrade/DIVERGENCES.md` (created by the first WP that needs it)

dss_capi deliberately diverges from EPRI (its `docs/known_differences.md`:
InvControlDeltaV fix, strict `PermissiveProperties` default, Monitor header,
SeasonalRating reimplementation …). Since the plan's end-target is **official
OpenDSS 11.0.0.1**, the default decision is **EPRI r4133 behavior wins**. Every
exception (keeping a dss_capi-side fix because EPRI's behavior is a proven
bug) is recorded in the ledger with: the observable, both behaviors, the
probe evidence, the decision, and the gate consequence (which oracle gates it
+ the `known_diffs.json` entry vs the other engine). No divergence decision
lives only in a commit message. Seed entries the WPs must settle:

| # | Observable | dss_capi 0.15.x | EPRI r4133 | Leaning |
|---|---|---|---|---|
| L1 | InvControl ΔV buffer (`InvControlDeltaV`) | fixed (per-control 2-slot) | 9-year-old shared-buffer bug | **EPRI-compat is a proven upstream bug**: adopt the fix, catalog the r4133 divergence (WP-U1.3 probes and decides) |
| L2 | zero `kW`/`kVA` handling | strict error (PermissiveProperties off) | silent clamp to `1e-8` (`DblValueNZ`) | **EPRI**: clamp (r4133 semantics); the strict mode is dss-ext-only surface (WP-U1.1) |
| L3 | Monitor CSV header | no quotes/extra spaces | quoted + extra spaces | keep 0.14.5-ported behavior until a consumer gate needs a decision; report-format gates are numeric-token anyway (WP-U1.5 note) |
| L4 | SeasonalRating application | global idx, any PDElement | r4088+: `GetRatings` centralization | probe both — they converged upstream; adopt the r4133 form (WP-U1.5) |

### 1.5 Golden migration mechanics

Goldens are pinned to the 0.14.5 oracle. When a WP moves an observable:

- **Live-gate cases** (vendored + families): flip `oracle` (§1.2). This is the
  PREFERRED gate for upgraded behavior — live compares don't need format
  parity.
- **Command-replay goldens** (`tests/golden/*` + `golden_*.rs`): regenerate
  ONLY the affected files with the target engine — the `gen_*.py` generators
  run under the Oddie venv with `DSS_ORACLE_ENGINE=capi015` (the generators
  import the same `dss` package surface; the first WP that regenerates —
  WP-U1.2 — teaches `gen_checkpoints.py`/`check_pin` the engine switch and
  stamps the golden's `.meta.json` with an `"oracle"` provenance field, so a
  mixed golden tree is self-describing). EPRI-rev targets get **no byte-exact
  goldens** (§1.3-2): where a byte-exact golden covered a report whose
  behavior moves in Rung 2, the gate converts to `compare_export`
  numeric-token form in the same commit.
- The golden regeneration rule stays **manual, deliberate, pinned** — same as
  today, with `PIN_OPENDSS.txt` joining `PIN.txt` as the pin set.

### 1.6 `known_diffs.json` lifecycle — the burn-down meter

Today the catalog explains Rust↔EPRI divergences (82 triaged @ r3723). Under
this plan it becomes the **progress inventory in reverse**: every Rung-1/2 WP
that ports a delta RETIRES the catalog entries that delta explains (the
zero-hit warning already polices stale entries), and the opt-in sweeps
(`DSS_LIVE_OPENDSS=r4088|r4133 … DSS_LIVE_OPENDSS_ASSERT=1`) must go green
per rung exit (WP-U1.10 / WP-U2.6). Cases whose `oracle` field moved are
auto-excluded from the sweep (they're gated in the mandatory gate already).
Retiring a first-divergence entry re-exposes whatever it masked — expected;
triage the newly visible layer honestly (new entry or new WP item, never a
tolerance bump).

### 1.7 Deck validation protocol on a target oracle

Same as GAPS §3 with the oracle swapped: every synthesized deck is validated
on **its target engine** before commit — (1) compiles/solves/converges; (2)
**bit-identical fingerprint across two separate oracle processes** (one-shot
`oracle_server.py` runs with `DSS_ORACLE_ENGINE=capi015` or
`=oddie`+`DSS_OPENDSS_REV`); (3) feature-sensitive where the feature could
silently no-op. RNG-carried scenarios follow GAPS §2.1 unchanged (never a
statistical gate). Record the validation transcript facts in the manifest
`note`.

## Source-integrity gate — ritual step 0 (before the model-tier check)

The Pascal we port FROM — `.inputs/dss_capi` (186 `.pas` files), plus
`.inputs/electricdss-tst` for oracle/live work — is the **specification**. Before doing
anything, and re-checked continuously (not only at kickoff), confirm that folder exists
and is non-empty. If it has vanished — missing or empty — at **any** point in the work,
**STOP immediately**: make no edits, run no gate, and do **not** reconstruct, guess, or
"port" a source you cannot read. Tell the user the vendored source is gone and must be
re-vendored, then wait. Reply exactly:
**«Исходник порта (`.inputs/dss_capi`) отсутствует или пуст — работа остановлена. Восстанови
vendored-исходник (re-vendor) и повтори команду.»**
No spec → nothing to port; fabricating one from memory is a silent, unverifiable
divergence — far worse than stopping. This gate runs **ahead of the tier/refuse check**
(`PLAN_SEQUENCE.md` §Model-tier protocol).

## 2. Workflow — the per-step ritual (do every step, in order, without being told)

0. **Tier check** (protocol: `PLAN_SEQUENCE.md` §Model-tier protocol). Look up
   the WP's **exec tier** in §3; compare against the session. If the session
   is below tier, do NOT execute; reply exactly: «Этот шаг требует <exec
   tier>. Переключи сессию (/model + reasoning effort) и повтори команду.»
   and stop. The audit tiers in §3 bind the spawned auditors (step 3).
1. **Gate green** — `cargo fmt --all --check`; `cargo clippy --workspace
   --all-targets -- -D warnings`; `cargo test --workspace` (all goldens + the
   always-on live corpus gates, INCLUDING every target-rev case). No
   `#[ignore]`, no name-filter that could green on zero matches. For an
   upgrade WP this includes its §1.2 same-commit package: cases flipped,
   goldens migrated, known_diffs retired, ledger updated — a red test blocks
   the commit. Keep `tests/corpus` pristine (`git status tests/corpus`).
2. **Update `STATUS.md`** (the §1 frontier + the UPGRADE record; each
   iteration-`<` case noted per §1.3-1), **commit** (code + STATUS together).
3. **`/audit-code` + `/audit-tests` in parallel** — two **fresh independent
   agents, never forks**, **spawned with an explicit model/effort override
   matching the WP's audit tiers in §3** (never "whatever the session runs").
   Each gets a self-contained brief: the commit range (`<sha>^..HEAD`), the
   diff, the authoritative upstream units (FPC for Rung 1, Delphi for Rung 2)
   + the inventory row(s) + plan/STATUS sections, and the binding rules (§1
   policy, the PIN set, `TODO(compat)`/`NOT_PORTED`, the same-commit rule,
   the target oracle is the spec). Auditors are read-only and return findings
   only; **you** settle each finding against the target oracle (probe, don't
   argue), fix what is real, note follow-ups in `STATUS.md`, re-run the gate,
   commit. A finding deliberately not fixed is recorded in STATUS, never
   dropped; if an audit finds nothing, skip its commit.
4. **`STATUS.md` full review** — read end to end; sync stale spots, archive
   dead weight to `docs/phase-records/`, dedup; `docs:` commit if anything
   changed (gate re-run first).
5. **Only now stop** and report **in Russian** (code, identifiers, commit
   messages and STATUS stay English): what landed, which oracle now gates
   what, what the audits found and how settled, known_diffs burn-down delta,
   gate status, next step.

**Upgrade-specific ritual additions:**

- **Probe protocol:** every semantic question is settled empirically against
  the WP's **target** oracle (`tools/golden/probe_val.py` pattern with
  `DSS_ORACLE_ENGINE`/`DSS_OPENDSS_REV` set), and cross-checked against the
  neighbor rev when the ledger needs a decision (§1.4).
- **Branch discipline:** this plan is built for **parallel porting**. Base
  branch = `upgrade-test-infra` (merged to the integration branch at U0 exit).
  Each WP runs on its own branch off the integration point; WPs touching the
  same units (the U2 protection trio) serialize. `known_diffs.json` and
  `DIVERGENCES.md` are append/retire-only files — merge conflicts are
  resolvable by union.
- **Pascal/Delphi citation:** Rung 1 code comments cite the FPC unit +
  identifier (as today); Rung 2 cites the Delphi unit + identifier and the
  `delta_r4088_r4133.md` row.

## 3. Model tiers (binding; vocabulary per `PLAN_SEQUENCE.md` §Model-tier protocol)

Exec = who may execute the WP. Audit-code / audit-tests = the explicit
model/effort override for the two spawned auditors (step 3 of the ritual).

| WP | Exec tier | Audit-code | Audit-tests | Why this exec tier |
|---|---|---|---|---|
| U0 (infra) | — landed (this branch) | `opus-high+` (post-hoc) | `opus-high+` (post-hoc) | done |
| U0.2 (inventory sweeps) | `sonnet-high+` | `opus-high+` | `opus-high+` | mechanical opt-in runs + triage bookkeeping |
| U1.1 (parser/property semantics) | **`opus-medium+`** | `opus-high+` | `opus-high+` | error-surface decisions ripple through every class; ledger L2 |
| U1.2 (numeric long tail) | `sonnet-high+` | `opus-high+` | **`opus-high+`** | each item is a cited one-line constant/formula with a pre-wired gate; the risk is in the golden migration, which the tests auditor owns |
| U1.3 (InvControl cluster) | **`opus-medium+`** | `opus-high+` | `opus-high+` | InvControl has twice hidden real port bugs behind "conditioning" (STATUS §WP7.5); ledger L1 |
| U1.4 (line/cable-constants cluster) | **`opus-medium+`** | `opus-high+` | `opus-high+` | the 1-ULP line-impedance history: numerics here are treacherous; new CNTS class |
| U1.5 (meter/seasonal/allocation) | `sonnet-high+` | `opus-high+` | `opus-high+` | table-driven reimplementation with probe-pinned decks |
| U1.6 (controls & misc fixes) | `sonnet-high+` | `opus-high+` | `opus-high+` | a list of S-size cited fixes, each deck-gated |
| U1.7 (NCIM solver) | **`opus-high+`** | `opus-high+` | `opus-high+` | a full third solution algorithm + dss-sparse real-matrix extension |
| U1.8 (WindGen + WTG3) | **`opus-high+`** | `opus-high+` | `opus-high+` | new element with 22-state dynamics and a 50 µs sub-cycle integrator |
| U1.9 (force hooks) | `sonnet-high+` | `opus-high+` | `opus-high+` | plumbing with explicit option surface |
| U1.10 (rung exit) | `sonnet-high+` | `opus-high+` | `opus-high+` | sweep + triage + docs |
| U2.1 (Fuse overhaul) | `sonnet-high+` | `opus-high+` | `opus-high+` | small, fully spec'd, breaking-defaults matrix pre-probed |
| U2.2 (Recloser per-phase) | **`opus-medium+`** | `opus-high+` | `opus-high+` | per-phase state machine + sequencing timing |
| U2.3 (Relay per-phase) | **`opus-medium+`** | `opus-high+` | `opus-high+` | same machinery, more relay types interacting |
| U2.4 (SwtControl + executive) | `sonnet-high+` | `opus-high+` | `opus-high+` | small cited fixes + `batchedit where` grammar |
| U2.5 (protection reports/logs) | `sonnet-high+` | `opus-high+` | `opus-high+` | masks + numeric-token conversions |
| U2.6 (rung exit / 11.0 parity claim) | **`opus-medium+`** | `opus-high+` | `opus-high+` | final triage judgment across the whole r4133 sweep |

## 4. Work packages

> Effort % ≈ share of this plan. Line refs confirmed at WP open. Every WP ends
> gate-green with its §1.2 same-commit package complete.

---

### WP-U0 — Multi-oracle test infrastructure [8%] — ✅ LANDED (branch `upgrade-test-infra`)

Built ahead of the plan so parallel porting branches inherit it:

1. **`capi015` oracle engine** (`tools/oracle/oracle_server.py::make_engine`):
   dss-python 0.16.0b2 from the Oddie venv, backend pin `0.15.0b4` asserted
   from `PIN_OPENDSS.txt` (no silent pass), fastdss `getYSparse` rebind;
   `ping` echoes `{"capi015": true}`.
2. **Per-case target oracle** (`corpus_live.rs`): `SolvableCase.oracle` field
   (`ORACLE_SPECS = capi015|r3723|r4088|r4133`), `Oracle::capi015()` /
   `Oracle::for_spec()` (ping-verified per spec), `OraclePool` (one engine
   binding per spec per run), structural validation of the spec value in
   `family_manifest_is_complete` (typo fails oracle-free).
3. **Iteration policy** (§1.3-1): exact for default-oracle cases; `<=` with a
   loud note for target-rev cases.
4. **Sweep hygiene:** `corpus_live_opendss` excludes `oracle`-flipped cases
   (reported count), so ASSERT mode stays meaningful as cases migrate.
5. **Pilot** `tests/corpus/modes/upgrade_pilot.dss` (`oracle: "r4133"`,
   validated per §1.7 — bit-identical across two r4133 processes): the
   mandatory gate now exercises the Oddie channel on every run.
6. Docs: `TESTING.md`, `tools/opendss/README.md` §3b, this plan, the three
   `docs/upgrade/delta_*.md` inventories, `PLAN_SEQUENCE.md` ordering update.

**Follow-up step U0.2 (first execution step of the plan, `sonnet-high+`):**
run the three pairwise `ab_compare.py` sweeps (`capi↔capi015`,
`oddie:r4088↔oddie:r4133`, `capi015↔oddie:r4088`) over the full case universe
+ the `dsspy_validation` broad-surface sweep; commit the reports' summaries
under `docs/upgrade/sweeps/` and reconcile them against the three inventory
documents (an inventory item with no observable case → note "no corpus
witness, deck must be synthesized"; an observed diff with no inventory row →
add the row). This is the empirical completeness check on the survey-based
scoping before any porting WP opens.

---

## Rung 1 — dss_capi 0.15.x / r4088-line parity

> Spec = `.inputs/dss_capi_with_git` @ `0.15.x` diffed against tag `0.14.5`
> (FPC-to-FPC, same lineage as the whole port — port loop-for-loop exactly as
> in Phases 3–8). Primary gate oracle = `capi015`; `oddie:r4088` for
> ledger/diagnostic cross-checks. Inventory: `delta_capi_0145_015x.md`
> (+ `delta_r3723_r4088.md` cross-check).

### WP-U1.1 — Parser & property-system semantics (PermissiveProperties era) [7%]

**Spec:** inventory C2/B7/C7-capi (TCC "none" is r4119, also in capi015);
`General/DSSObjectHelper.pas` (TrapZero/ReplaceZero/NonZero flags),
`ParserDel.pas` (+480: `ParseAsSymMatrix` incomplete-matrix error,
`AllowNoneItem`, `WasQuoted`, `MakeDoubleNZ`), per-element zero-handling.

1. **Ledger L2 decision first** (probe capi015 with/without the compat flag,
   and oddie:r4088/r4133): target behavior = EPRI's `DblValueNZ` clamp
   (`kW=0` → `1e-8` on Load/Generator/Storage/PVSystem/WindGen) — r4133
   semantics; implement the clamp machinery; do NOT adopt dss_capi's strict
   default (it is dss-ext-only surface; record in the ledger).
2. `ParseAsSymMatrix` incomplete-matrix validation (error instead of silent
   zero-fill) — probe which engines error (r4088 Delphi does, B7): adopt.
3. `AllowNoneItem` list parsing (`none` entries in conductor lists),
   `WasQuoted` plumbing (needed by U2's per-phase state arrays too).
4. TCC_Curve name `none` reserved + curve-ref `none` clears (C7; identical in
   r4133 — port once here).
5. Class-command activation fix (C11, r3875).

**Gate:** synthesized `modes/upgrade_parser.dss` micro-decks (zero-kW load,
incomplete Zmatrix, `none` conductor entries, `TCC_Curve.none` rejection),
`oracle: "capi015"`, validated per §1.7; existing corpus decks that newly
error/pass re-classified honestly (`DSS_LIVE_CLASSIFY=1`). Ledger updated.

### WP-U1.2 — The numeric long tail (constants & one-line formula fixes) [8%]

**Spec rows:** B1 (Capacitor Cmatrix diagonal ×1.000001 before inversion —
every Cmatrix capacitor's YPrim moves ~1e-6), B2/D1 (SimpleCarson
`658.5 → 658.8530451057239` in `LineConstants.Get_Ze` ONLY — `Line.pas Kxg`
keeps 658.5 upstream: reproduce the inconsistency, `TODO(compat)`-style note
citing the ledger), D6 (Transformer seasonal AmpRatings drop `1.1*`), D8
(X23/X13 trap-zero actually triggering), D7 (IBR dynamics
`IMaxPPhase = kVArating/BasekV/NPhases`), B5 (GFM `Isc1` ×1000 removal), D3
(spacing ratings: phase conductors only, minimum; CN/TS cables get ratings),
B3-r3723 (`Load.GrowthFactor` at Year=0 from `dblHour/8760`).

Each item: port the cited hunk → flip the live cases whose observables move
(asymmetric capacitor decks, line-constants cases with `EarthModel=Carson`,
GFM/dynamics decks…) to `oracle: "capi015"` → regenerate ONLY the affected
goldens with the capi015 engine (**this WP builds the generator engine
switch**, §1.5: `gen_checkpoints.py` env-driven engine + `.meta.json`
`"oracle"` provenance) → retire the matching known_diffs entries.

**Gate:** per-item targeted live cases + regenerated goldens; the
`line_constants` golden family gets a capi015-regenerated Carson subset with
provenance stamps; full gate green. The FIRST case this WP flips is by
construction revision-*sensitive* (its observable differs between 0.14.5 and
capi015), which closes the audit-WP-U0 note that the U0 pilot proves engine
identity (ping) but not numeric routing — from here on, a routing regression
fails on the numbers too.

### WP-U1.3 — InvControl cluster [7%]

**Spec rows:** D1 (InvControlDeltaV per-control 2-slot buffer — **ledger L1
decision**: probe capi015 vs r4133 on a multi-DER volt-var deck; adopt the
fix, catalog the r4133 divergence as a known-upstream-bug entry, same
discipline as CLAUDE.md's known-bugs list), D2 (per-DER basekV cross-leak
fix), D3 (sqrt-guard on `kVA²−kW²`), D4 (delta-DER LL monitored voltages), D5
(InvControl9611 note — verify our WP7.5 port already matches the fixed
side), C8 (`VV_RefReactivePower` property removed; MonBus validation errors).

**Gate:** the existing controls-family InvControl decks + two new synthesized
multi-DER decks (wye + delta monitored, `oracle: "capi015"`, event-log +
per-step Q trajectories exact); `inv_control` unit pins extended. The WP7.5
prove-don't-rationalize discipline applies to every divergence found here.

### WP-U1.4 — Line/cable-constants property cluster [10%]

**Spec rows:** C1 + B3 + A6-r3723: `Line.EpsRMedium/HeightOffset/HeightUnit/
Conductors` (mixed wire/CN/TS lists + `none`), `LineSpacing.Detailed/
EqDistPhPh/EqDistPhN/AvgPhaseHeight/AvgNeutralHeight` (equivalent-spacing
model), `CNData.SemiconLayer` (default preserves the old capacitance
formula), the merged `CNTSLineConstants` class (mixed-conductor Kersting),
`LineCode` FaultRate/PctPerm/Repair deprecation, LineType enum width fix.

Defaults preserve 0.14.5 numerics (probe-proven upstream claim — verify!);
the new paths are new numeric surface. Port with the `dss-complex`/FPC-forms
discipline from the line-impedance investigation (memory: `cdiv_fpc`,
`csqrt_fpc`, `cln_fpc` are already engine-wide — the new code uses the same
`DssComplex64` ops).

**Gate:** (a) default-path no-change proof — the existing line_constants
goldens stay green UNTOUCHED; (b) new-path decks: `modes/upgrade_linecs_*.dss`
(equivalent spacing, mixed conductors, SemiconLayer=no, EpsRMedium≠1,
HeightOffset) with `oracle: "capi015"`, YPrim-focused live compare (the
asymmetric-family convention); props round-trip for the new surface.

### WP-U1.5 — EnergyMeter seasonal / allocation / monitor-header [7%]

**Spec rows:** E2 (SeasonalRating reimplementation: globally synced
`SeasonalRatingIdx`, applies to any PDElement with `NumAmpRatings>1`; ledger
L4 — probe that capi015 and r4133 agree, adopt), D9 (`AllocateLoad` ignores
disabled meters/sensors — same fix exists in r4133, port once), D16 (zone
counters skip disabled + non-PD), E1 (monitor header quote removal — ledger
L3: keep numeric-token gating, decide the rendered form empirically), D8-r3723
(manual `zonelist` from-bus/terminal fix — verify whether our Phase-6 port
already built the tree correctly; if it did, record "not a delta for us").

**Gate:** seasonal-rating deck matrix (Line + Transformer + cable ratings,
`SeasonRating=yes`, DI/Overloads exports via `compare_export`), an
allocation deck with a disabled meter (`oracle: "capi015"`, allocation
factors probed), zonelist deck. Retire the seasonal/meter known_diffs
entries.

### WP-U1.6 — Controls & misc long tail [8%]

**Spec rows:** C5 (RegControl `FwdThreshold` + signed rev/fwd threshold pair
+ legacy fallback + idle properties — r3723 cross-check B4/C7), D10
(StorageController `FpctkWBandLow` typo + first-iteration `StorekWChanged`),
D11 (CapControl PT/CTPhase validation scope + TIMECONTROL monitored-element
requirement), D12 (SwtControl Normal/State field mapping fix — note: U2
supersedes SwtControl semantics wholesale; port the 0.15.x mapping only if
U2.4 has not landed first, else skip with a cross-ref), D13 (LoadShape MM
fixes — only the parts our WPG-era port shares), D14 (DynamicExp RPN index),
D15 (`LookupVariable` case-insensitivity), C4 (`Solve all`/`Clear all`
aliases; actor semantics = single-actor no-op), C6 (Transformer/AutoTrans
BHpoints/BHcurrent/BHflux data props), A7-r3723 (GenController
deregistration), C5-r3723 (LoadShape `Mode` prop insertion shifting
`Interpolation` 22→23 — property-index parity check), B4-capi (harmonics
init-failure abort).

**Gate:** per-item micro-decks in their §3.1 families with
`oracle: "capi015"` (each feature-sensitive, §1.7); props-roundtrip
re-baselined for the touched classes; RegControl reverse-power deck matrix
(legacy-input equivalence + new-property divergence both pinned).

### WP-U1.7 — NCIM solver [16%]

**Spec:** A1 — `Common/NCIMSolutionHelper.pas` (1048, FPC form) + the
`Solution.pas` NCIM state/hooks, `Ymatrix` `PDE_ONLY` build, Generator
PV-bus participation (`NCIM_Idx`, `InitPVBusJac`, Q-limits/`UpdateGenQ`,
PV→PQ switching), `VSource.CalcInjCurrAtBus`, Load Z/PQ classification,
options `NCIMQGain`/`IgnoreGenQLimits`, `Export Jacobian/deltaF/deltaZ`,
`Show PV2PQ_Conversions` (numeric-token gates).

1. **dss-sparse extension first** (its own commit): a real-valued
   KLU-shaped path (`SetMatrixElement` semantics) behind the existing
   forbid-unsafe faer wrapper; unit tests transcribed from small hand
   Jacobians.
2. The solver port loop-for-loop; `Set Algorithm=NCIM` dispatch (prefix
   `'nc'`); `Converged()` NCIM branch; `CheckControls`'s `NCIMRdy` reset.
3. Deck matrix: micro NCIM snapshot (PQ-only), PV-bus generator deck
   (Q-limit hit → PV→PQ conversion pinned via `Show PV2PQ_Conversions`
   token compare + iteration count ≤), midi IEEE123-class NCIM re-solve;
   all `oracle: "capi015"`, `pending: true` from day one (this WP flips).
4. Cross-check one deck on `oddie:r4088` (report-only) to catch
   capi015-specific drift.

### WP-U1.8 — WindGen + WTG3 dynamics [16%]

**Spec:** A2 — `PCElements/WindGen.pas` (2272, the dss_capi-cleaned form:
no user-model, no Xd/puXd), `WTG3_Model.pas` (1335: PLL, seq-current PI
regulators, current limiting |I1|≤1.25, LVPL/LVQL, fault detection, Cp 5×5
polynomial, MPPT/pitch/inertia, one-mass swing, odd-substep 50 µs
trapezoidal sub-cycle), `WindGenVars.pas`. Harmonics for WindGen is
**disabled upstream in 0.15.x** — reproduce the loud disable, do not invent.

1. Stage A: class skeleton, props (enum order + flags from
   `DefineProperties`), power-flow models (const-PQ from YPrim currents per
   `082900eb`), aerodynamic P(v³,Cp) steady-state path. Gate A:
   props-roundtrip + snapshot deck.
2. Stage B: WTG3 dynamics (22 state vars; the sub-cycle integrator is
   deterministic — gate per §1.7 with `compare_variables` on all 22).
3. Deck matrix {snap × wind-shape daily × both} at micro + midi scale,
   `oracle: "capi015"`; dynamics deck with a fault ride-through
   (LVPL-sensitive, probe-proven).
4. The CLAUDE.md cancellation-floor decomposition rule applies to any
   residual gap in the sub-cycle states (this is WPG.13-class numerics).

### WP-U1.9 — PCE force hooks (`ForceInjCurr`/`ForceY` + options) [4%]

**Spec:** A3/A5 — `Set`/`Get` `InjCurrent`/`ITerminal`/`Yprim`/`StateVar`/
`IterNumber`/`CtrlIterNumber`/`IntegrationFlag`; element flags honored in
the injection loop and `ReCalcAllYPrims` (skip `CalcYPrim` when forced);
`SampleControlDevices` event hook. The pyControl *component* stays
`NOT_PORTED` (loud), §0.

**Gate:** micro deck driving the options from script (set a load's forced
injection, solve, read back) vs capi015; a Rust-only unit pinning that
`clear` resets the force flags.

### WP-U1.10 — Rung 1 exit [3%]

1. Full `ab_compare.py --a capi015 --b <rust-via-manifest>`-equivalent: run
   the mandatory gate + `DSS_LIVE_OPENDSS=r4088 DSS_LIVE_OPENDSS_ASSERT=1`
   — every remaining r4088 divergence is either a ledger entry (dss-ext
   deliberate) or a Rung-2 item; zero unexplained.
2. known_diffs burn-down documented in `docs/upgrade/`; STATUS UPGRADE
   record; `tests/corpus/COVERAGE.md` refresh; marker sweep
   (`NOT_PORTED(U1…)` none left).
3. Merge to the integration branch (`--no-ff`, explicit user request — the
   established convention).

---

## Rung 2 — OpenDSS r4133 (11.0.0.1) parity

> Spec = Delphi diff r4088→r4133 (`Version8/Source`); oracle = `oddie:r4133`
> exclusively (no capi exists). Inventory: `delta_r4088_r4133.md`. The delta
> is the protection overhaul + executive fixes; solver/PC-elements/meters are
> untouched upstream — any observed divergence outside protection is a Rung-1
> regression, not new scope.

### WP-U2.1 — Fuse overhaul [3%]

**Spec:** `Controls/fuse.pas` (moved from PDElements — file org only; our
module layout keeps its current home, note the upstream move): `RatedCurrent`
repurposed to informational (default 1.0→0.0), new `CurveMultiplier`
(default 1.0) as the TCC divisor `GetTCCTime(Cmag/CurveMultiplier)`, default
`FuseCurve` `Tlink`→`none` (**a default-constructed fuse never blows**),
`InterruptingRating` informational prop.

**Gate:** fuse deck matrix `oracle: "r4133"`: legacy-style deck (RatedCurrent
only — pins the never-blows breaking default), explicit-curve deck
(CurveMultiplier scaling pinned via event log + trip times), props
round-trip. Existing fuse controls-family decks flip `oracle` in the same
commit with re-probed expectations; retire fuse known_diffs entries.

### WP-U2.2 — Recloser per-phase rewrite [8%]

**Spec:** `Controls/Recloser.pas` (1432): per-phase `StateArray[1..6]` +
per-phase OperationCount/lockout/armed, `SinglePhTrip`/`SinglePhLockout`,
fast/slow pickup split (`PhFastPickup`/`PhSlowPickup` etc.; legacy
`PhaseTrip` sets both), default A/D curves removed (**default-constructed
recloser is inert**), inst-trip delay no longer double-counted (fires one
`MechanicalDelay` earlier), `MaxOperatingCount` curve selection over
non-locked-out phases, sampling continues while ≥1 phase closed, state
resync from `ControlledElement.Closed[i]`, property renames 24→46 with
deprecated aliases, `Lock`/`Reset` actions, `EventLog`/`DebugTrace` props.

**Gate:** deck matrix `oracle: "r4133"`, all per §1.7 on the r4133 engine:
ganged legacy sequence (temp + perm fault — pins the inst-delay change and
the removed defaults), single-phase trip/lockout sequence (per-phase event
log with §1.3-3 masks; final per-phase states exact), pickup-split deck,
alias-parsing props round-trip. The existing recloser decks
(`recloser_temp/perm`, midi twins) flip with re-probed event logs.

### WP-U2.3 — Relay per-phase rewrite [8%]

**Spec:** `Controls/Relay.pas` (2211): the same per-phase machinery for
`type=current` + `SinglePhTrip`/`SinglePhLockout`; `VoltageLogic` OV/UV over
**closed phases only** (reclose check still all-phase); `CTRL_RESET` no
longer runs full `Reset` (only OperationCount for closed phases + TD21 quiet
window); renames 50→71 with aliases (`PhCurve`, `OC_GndPickup`,
`DefiniteTimeDelay`, `MechanicalDelay`, `Generic_*`, `Voltage_OVCurve`…);
`Lock`/`Reset`/`RatedCurrent`/`InterruptingRating`; the upstream
reset-logs-as-"Recloser." bug — **decide via the CLAUDE.md known-bugs rule**
(deterministic + defined → reproduce with `TODO(compat)`; probe first).

**Gate:** overcurrent ganged + single-phase matrices, voltage-relay deck with
a partially open element (pins closed-phase OV/UV), TD21 deck re-run under
r4133 (WPG.12 machinery), reset-semantics deck (CTRL_RESET behavior pinned
via event log + states). All `oracle: "r4133"` per §1.7.

### WP-U2.4 — SwtControl + TCC + executive verbs [4%]

**Spec:** SwtControl r4133 form (instant actions per r4088 B5 + r4133
`Action` fix: deprecated `Action` sets the ACTUAL state; new informational
`RatedCurrent`; per-phase `State`/`Normal` arrays; first-set side effect);
`batchedit … where` conditionals (`DoCheckConditionals`/`DoEvalConditionals`:
`>,<,>=,<=,=,!=` with `and/or/xor`) + `GlobalResult = 'Elements edited: N'`;
`DoNewCmd` aborts on NewObject failure; AllocateLoad disabled-check —
verify U1.5 already covers it (r4115 == r4133 form), record.

**Gate:** SwtControl deck matrix (Action-deprecated path + per-phase arrays)
`oracle: "r4133"`; batchedit-where decks extending the WP8.6 batchedit
family (result-string pinned as text token; where-filtered edit sets pinned
via live model); props round-trip.

### WP-U2.5 — Protection report/log surface [3%]

**Spec:** E-bucket of `delta_r4088_r4133.md`: event-log wording overhaul
(per-phase messages), property-value render `[closed, closed, closed]` in
`?`/Dump/Save for the four protection classes, batchedit result text.

1. Event-log comparator masks for the r4133 wording (§1.3-3) — masks are
   per-rev, documented, and do NOT apply to default-oracle cases.
2. Save/Dump for the renamed/array properties: gate by **round-trip through
   our own parser** (the WP8.5 convention) + numeric-token compare vs
   r4133's `?` output — never byte-exact vs Delphi.

**Gate:** the U2.1–U2.4 decks re-run with `compare_eventlog` enabled; a
Save round-trip case over a protection-heavy deck.

### WP-U2.6 — Rung 2 exit — the 11.0.0.1 parity claim [3%]

1. `DSS_LIVE_OPENDSS=r4133 DSS_LIVE_OPENDSS_ASSERT=1` **green**: every
   surviving catalog entry is a ledger-documented deliberate divergence
   (upstream-bug class) — zero unexplained, zero "not yet ported".
2. `DSS_LIVE_OPENDSS=r4088` re-run: divergences = exactly the r4088→r4133
   deltas this rung ported (sanity direction check).
3. `dsspy_validation` sweep vs r4133 re-run; summary committed.
4. Marker sweep (`NOT_PORTED(U2…)`, pending upgrade decks — none left);
   `DIVERGENCES.md` final review; STATUS; `PLAN_SEQUENCE.md` marks this plan
   complete; version/parity statement in README/STATUS ("engine behavior =
   OpenDSS 11.0.0.1 (r4133) except the documented ledger").
5. Merge (`--no-ff`, explicit request).

## 5. Plan-wide exit criteria

- The three-command gate green with the mixed-oracle manifest state final:
  default-oracle cases = observables where 0.14.5 == r4133 behavior;
  `oracle`-flipped cases = everything the upgrade moved; zero `pending`
  upgrade decks.
- Both opt-in EPRI sweeps assert-green (r4133 fully, r4088 explained).
- `docs/upgrade/DIVERGENCES.md` complete: every dss_capi↔EPRI↔dss-rs
  difference has an owner (adopted / reproduced-with-TODO(compat) /
  documented-upstream-bug).
- The golden tree is provenance-stamped (`oracle` field in every regenerated
  `.meta.json`); `TESTING.md` reflects the final oracle map.
- `PropFlags::HIDE_015X` retired: the Line/LineGeometry Dump/JSON golden
  surface flips to capi015 (`gen_json.py` re-pinned from 0.14.5), the
  `Line.Wires → "Conductors"` `json_name` masquerade drops, and the real
  `Conductors` prop owns the JSON key (mixed-class lists then render with
  per-item class prefixes — currently untested by any byte golden). Deferred
  from WP-U1.4 wt-u14cond; full decision in `docs/upgrade/DIVERGENCES.md`
  §"Line/LineGeometry Conductors (text upstream-broken)". `rg HIDE_015X`
  must be empty at plan exit.
- Handoff notes for DE_PASCALIZE Stage F: the `oracle-parity` lane's target
  is r4133-parity (this plan's end state), and RESONANCE's iteration-count
  freedom is already structurally supported by the §1.3-1 policy.

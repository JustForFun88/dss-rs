# GOLDEN_REBASE — Self-goldens + a live gate at fastdss parity

## Source-integrity gate — ritual step 0 (before the model-tier check)

The Pascal at `.inputs/dss_capi` (186 `.pas` files) is the *spec*; oracle/live work
also needs `.inputs/electricdss-tst`; r4133 citations need
`.inputs/electricdss-code-r4133-trunk`; this plan's reference harness needs
`.inputs/DSS-Python` (branch `origin/fastdss`). Before doing anything, and re-checked
continuously (not only at kickoff), confirm those folders exist and are non-empty. If
one has vanished — missing or empty — at **any** point in the work, **STOP
immediately**: make no edits, run no gate, and do **not** reconstruct, guess, or
"port" a source you cannot read. Tell the user the vendored source is gone and must be
re-vendored, then wait. Reply exactly:
**«Исходник (`.inputs/...`) отсутствует или пуст — работа остановлена. Восстанови
vendored-исходник (re-vendor) и повтори команду.»**
This gate runs **ahead of the tier/refuse check** (`PLAN_SEQUENCE.md` §Model-tier
protocol). Also verify `(Get-Command cargo).Source` is under `.cargo\bin` before
trusting any gate result (the mingw shadow trap).

**`.inputs/DSS-Python` working-tree trap.** The checkout sits on `master`
(`85783cf`), while the reference harness is `origin/fastdss` — 243 changed lines in
`tests/save_outputs.py` alone. Never read `.inputs/DSS-Python/tests/*.py` (or
`dss/I<Class>.py` `_columns` lists) from the working tree: read them as
`git -C .inputs/DSS-Python show origin/fastdss:tests/save_outputs.py` (same for
`compare_outputs.py` and every `dss/I*.py`). Before starting any G1 sub-step verify
`git -C .inputs/DSS-Python rev-parse origin/fastdss` resolves.

> **Decision context (binding, user 2026-08-02, same session as the r4133 policy):**
> the 1:1 port is finished; third-party-generated goldens are no longer the anchor.
> The construction has three parts, in this order:
>
> 1. **The live corpus gate is extended to "no worse than fastdss"** — every derived
>    quantity that DSS-Python's own validation harness
>    (`.inputs/DSS-Python` @ `origin/fastdss`, `tests/save_outputs.py` +
>    `tests/compare_outputs.py`) compares as API fields, but which we today verify
>    only through report-byte goldens, moves into the live numeric comparison
>    against the oracles (`capi_v0145` always; `r4133` where the channel is capable).
> 2. **Report FORM is compared against nothing external** — every rendered artifact
>    becomes a self-golden of our own engine (pure anti-regression), protected by a
>    provenance lock and a regeneration discipline that replaces "never regenerate".
> 3. **The FPC print-emulation kernels die (user decision 2026-08-02, this plan's
>    WP-G4).** After the self-snapshot migration, both lanes move to native Rust
>    number/string rendering: `fmt_g`, `fixed_w_script`, `json_float`,
>    `JSON_LINE_BREAK`, `CONTROL_QUEUE_SEC_DIGITS`, `render_rows` are torn down
>    kernel-by-kernel with a reviewed regen of the affected (by then self-anchored)
>    families. What stays in `oracle-parity` is **numeric** precision-compat only:
>    truncated constants (`PI`, `kv_base_search_scale`, `profile_ll_pu_divisor`) and
>    FPC `Round` semantics in computation (`round_f64`, `round_i32`). Census
>    trajectory: `SPLIT_ALIAS_POPULATION` 31 → **11** (after WP-G2) → **5** (after
>    WP-G4); `Escape::WholeCase` 4 → 1.
>
> Ordering is mandatory: live-gate expansion (WP-G1) and bug-kernel teardown (WP-G2)
> land **before** any self-snapshot (WP-G3) — otherwise bugs freeze into the
> "reference"; the print-kernel teardown (WP-G4) lands only **after** WP-G3, because
> regenerating an artifact is legal only once it is self-anchored (flipping a print
> kernel while goldens are still oracle-anchored reds the gate mid-flight). Measured
> basis: the live gate today compares zero report text (only `GlobalResult` ×2
> cases, `runner.rs:575-582`, and `AutoAddLog.csv` ×2, `runner.rs:631-653`); the
> precedent for removing bug kernels without touching a golden byte is commit
> `4f977d9e`.
>
> **Standing rules of engagement** (CLAUDE.md is authoritative): r4133 is the
> behavioral authority and upstream bugs are NEVER reproduced in any lane; every
> deliberate divergence from an oracle channel is excluded field-by-field and pinned
> by an expected-value test (`tests/corpus/ledger.json` / lane.rs / golden fix-ups);
> tolerances are never loosened to make a comparison pass; a Rust↔oracle gap above
> its floor is a bug until proven otherwise; `TODO(compat)` stays precision-only.
> Branch off `update`, merge back into `update`. Commit messages short.
>
> **Stop-and-confirm cadence:** after each sub-step run the per-sub-step ritual below
> **autonomously, without pausing between its stages**; the single stop point is at
> the very end of the sub-step — then wait for the user's explicit confirmation
> (unless the user authorized several sub-steps in one pass).

## Per-sub-step ritual (do EVERY sub-step, in order, without being told)

Every sub-step below — regardless of how trivial it looks — runs this full ritual.
No batching of audits across sub-steps, no shared fix agent across sub-steps (the
DE_PASCALIZE Stage-F group-fixer pattern is explicitly rescinded for this plan).

0. **Tier check** (protocol: `PLAN_SEQUENCE.md` §Model-tier protocol). Look up the
   sub-step's row in the tier table (§0 below). The check is **mandatory and
   explicit** for all three agent roles:
   - the **implementation agent** must run at the sub-step's exec tier or higher;
   - both **auditors** must run at the sub-step's audit tiers **as listed in §0**
     (never a single plan-wide default) — spawned with an explicit model/effort
     override, never "whatever the session runs";
   - the **fix agent** must run at the sub-step's exec tier or higher.
   If the session/agent is below the required tier, do NOT execute; reply exactly:
   «Этот шаг требует <tier>. Переключи сессию (/model + reasoning effort) и повтори
   команду.» and stop. If the session's reasoning effort is not visible, ask the
   user to confirm it before executing — **mandatory on the `opus-xhigh` rows**
   (G1.0, G1.3a–c, G1.5, G1.11a–c, G2.5, G4.1), per `PLAN_SEQUENCE.md` §Model-tier
   protocol. Fable is used only with explicit user approval (standing rule since
   2026-07-19).
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
   Plus `pwsh -File tools/lanes/lane_diff.ps1` for any sub-step touching a compat
   kernel, a lane alias, or the solver. No `#[ignore]`, no name-filter that can
   green on zero matches; a red test blocks the commit.
3. **Update `STATUS.md`** (frontier + the plan record), **commit** (code + STATUS
   together). *(**Since 2026-09-03** this means the sub-step's **full** record
   goes to its `docs/phase-records/` file — GOLDEN_REBASE → `golden-rebase.md` —
   while `STATUS.md` section 1 gets only a **3–6 line** landed paragraph; the
   rule itself is STATUS section 1 "Record placement".)*
4. **`/audit-code` + `/audit-tests` in parallel** — two **fresh independent agents,
   never forks**, spawned with the explicit audit-tier override. Each gets a
   self-contained brief: the sub-step's commit range (`<sha>^..HEAD`), the diff, the
   authoritative sources (r4133 unit:lines and/or the fastdss harness lines and/or
   the oracle probe procedure), the plan section, and the binding rules above.
   Auditors are **read-only** and return findings only.
5. **Dedicated fix agent** — a **fresh agent, one per sub-step** (exec tier),
   receives both auditors' findings plus the same brief, settles each finding
   against evidence (r4133 source, a live oracle probe — never "sounds plausible"),
   fixes what is real, re-runs the full gate, commits. A finding deliberately not
   fixed is **recorded in STATUS with its reason**, never dropped. If both audits
   return nothing, the fix agent is skipped (no empty commits) — record "audits
   clean" in STATUS instead.
6. **STATUS review** — read `STATUS.md` end to end; sync whatever the sub-step made
   stale (no two places disagreeing), dedup restated paragraphs; if anything
   changed, re-run the five-command gate and land a `docs:` commit, so the sub-step
   ends on a **clean tree** (§1.2 regen rule R1 depends on it). *(**Since
   2026-09-03** "end to end" covers `STATUS.md` **and** the sub-step's
   `docs/phase-records/` file, which is where its full record lives — STATUS
   section 1 keeps only its 3–6 line paragraph; the rule itself is STATUS
   section 1 "Record placement".)* Then stop and
   report **in Russian** (code, identifiers, commit messages and STATUS stay
   English): what landed, what the audits found and how the fix agent settled it,
   gate status, next sub-step.

## 0. Scope, ordering, tier table

Six work packages, strictly ordered. WP-G1 and WP-G2 may interleave at sub-step
granularity **with these hard constraints**: G2.2a (`BUS_INT_DURATION_WALKS_ALL_BUSES`)
lands **before** G1.6 (it moves the very quantities G1.6 gates), and G2.4 (monitor
padding) lands before any G1 sub-step that re-touches `harness::compare_monitor`.
Any other G1 sub-step landed before a G2 row that moves its surface must re-run its
`DSS_GATE_SEED_LEDGER=1` triage in that G2 commit. Interleaving happens on a
**single branch only — never in parallel worktrees**: `tests/corpus/ledger.json`
and `population.lock.json` are fail-on-stale and are rewritten by both WPs, so a
sub-step always rebases onto the latest `update` before regenerating them. Nothing
in WP-G3 starts before **both** G1 and G2 are complete; WP-G4 starts only after
WP-G3 is fully landed; WP-G5 is last.

> **2026-09-04 amendment — parallel lanes (user decision; coordinator decision D7).**
> The "single branch only — never in parallel worktrees" rule above is relaxed for
> the rest of WP-G1 and for WP-G3/WP-G4. Sub-steps that share no accessor,
> comparator or exclusion list (the 2026-08-29 synthesis chains: element
> G1.3a → G1.3d(i) → G1.3d(ii) → G1.3b → G1.3c; bus G1.4a → G1.5 → G1.4b
> (**2026-09-04**, D8: now G1.4a → G1.5 → G1.4c → G1.4b; **2026-09-05**, D26 splits
> the last one again: G1.4a → G1.5 → G1.4c → G1.4b → G1.4d — §G1.4's as-executed notes);
> PD/meter G1.6b → G1.6(i) → G1.6(ii); the singles G1.7 / G1.8 / G1.9 /
> G1.10a–c) run as **lanes** in per-lane git worktrees (`.claude/worktrees/lane-*`,
> branches `lane-*`) branched from `update`. `update` stays the integration branch
> and its main working tree receives **merges only**, one lane sub-step at a
> time: a merge agent resolves conflicts semantically; `population.lock.json` is
> **regenerated** on the merged tree (never hand-merged); `ledger.json` is the
> union of both sides' entries; the full five-command gate (both lanes) — plus
> `lane_diff.ps1` when product code moved on both sides since the merge base —
> runs on the merged tree before the merge commit is kept. A lane fast-forwards
> onto `update` before starting its next sub-step. This is safe precisely because
> the locks are fail-on-stale: a merged lock or ledger that does not match the
> merged tree fails loudly on the merged-tree gate, so no stale state survives a
> merge. The per-sub-step ritual, the audit pair and the records are unchanged;
> each record names its lane branch and the merge commit.

| WP | What | Why it is ordered here |
|---|---|---|
| WP-G0 | Golden provenance lock + regen rails | pure instrumentation; makes every later byte movement a loud, reviewed event |
| WP-G1 | Live-gate expansion to fastdss parity | the math currently witnessed only by golden bytes gets its live oracle witness BEFORE the golden anchor is dropped |
| WP-G2 | Teardown of the remaining bug kernels (shared with r4133, plus the one capi-only row the `4f977d9e` pass missed) | bugs must be fixed before self-snapshots, else they freeze into the reference; runs against the EXISTING goldens (zero golden bytes move — the `4f977d9e` precedent) |
| WP-G3 | Golden migration: delete the redundant, self-snapshot the rest, freeze the named exceptions | only after G1+G2 |
| WP-G4 | Rendering de-FPC: the six print-emulation kernels die in both lanes | regen is legal only on self-anchored artifacts, so only after G3 |
| WP-G5 | Docs + the restated oracle argument | closes the plan |

**Per-sub-step model tiers.** Audit-code and audit-tests are separate agents; the
fix agent runs at the exec tier. Per `PLAN_SEQUENCE.md` §Model-tier protocol,
audits on `opus-xhigh` exec rows are themselves `opus-xhigh`.

| Sub-step | Exec | Audit-code | Audit-tests | Why |
|---|---|---|---|---|
| G0.1, G0.2 | `opus-high+` | `opus-high+` | `opus-high+` | new test infra cloned from a proven pattern (`population_lock.rs`) |
| G1.0 | `opus-xhigh` | `opus-xhigh` | `opus-xhigh` | rails over the two fail-on-stale artifacts (`population.lock.json`, `ledger.json`) + the unsafe-adjacent r4133 bridge crate |
| G1.1 | `opus-high+` | `opus-high+` | `opus-high+` | flag flip + ledger triage; capture already built (`dss-epri/src/capture.rs:208-367`) |
| G1.2 | `opus-high+` | `opus-high+` | `opus-high+` | one new deck for a class with known oracle-abort paths (`makeposseq_ctrl.dss:9-13`) |
| G1.3a, G1.3b, G1.3c | `opus-xhigh` | `opus-xhigh` | `opus-xhigh` | new engine accessors over solver state + cross-engine floor calibration |
| G1.3d | `opus-high+` | `opus-high+` | `opus-high+` | discrete element extras on established capture patterns |
| G1.4, G1.6, G1.6b, G1.7, G1.8, G1.9 | `opus-high+` | `opus-high+` | `opus-high+` | capture+comparator extensions on established patterns |
| G1.5 | `opus-xhigh` | `opus-xhigh` | `opus-xhigh` | Zsc surface: precomputed-state semantics + floor calibration |
| G1.10 | `opus-high+` | `opus-high+` | `opus-high+` | file-artifact comparison, existing `compare_export` machinery |
| G1.11a, G1.11b, G1.11c | `opus-xhigh` | `opus-xhigh` | `opus-xhigh` | FFI family additions in `crates/dss-epri` (unsafe-adjacent bridge crate) |
| G2.0 | `opus-high+` | `opus-high+` | `opus-high+` | doc-citation re-anchor + the `TORN_DOWN_ROWS` register |
| G2.1a–G2.1h | `opus-high+` | `opus-high+` | `opus-high+` | one bug-kernel teardown each, `4f977d9e` procedure, each row cited |
| G2.2a–G2.2d | `opus-high+` | `opus-high+` | `opus-high+` | teardown + one exclusion mechanism each |
| G2.3, G2.4 | `opus-high+` | `opus-high+` | `opus-high+` | Newton kernel / channel-scoped monitor normalization |
| G2.5 | `opus-xhigh` | `opus-xhigh` | `opus-xhigh` | three engine bug fixes with live-gate divergence triage |
| G2.6 | `opus-high+` | `opus-high+` | `opus-high+` | the capi-only Show width row missed by `4f977d9e` |
| G3.1 | `opus-high+` | `opus-high+` | `opus-high+` | twin-audit table + gap decks |
| G3.2a–c | `opus-high+` | `opus-high+` | `opus-high+` | deletions gated on the twin audit |
| G3.3a–G3.3e, G3.4, G3.5 | `opus-high+` | `opus-high+` | `opus-high+` | snapshot conversions over the G0 rails |
| G3.6 | `opus-medium+` | `opus-high+` | `opus-high+` | lock-anchor bookkeeping |
| G4.1 | `opus-xhigh` | `opus-xhigh` | `opus-xhigh` | `fmt_g` — the widest blast radius (all report text + event-log fold) |
| G4.2–G4.5 | `opus-high+` | `opus-high+` | `opus-high+` | one print kernel each (G4.3: two), categorical regen diff |
| G4.6 | `opus-high+` | `opus-high+` | `opus-high+` | closure: `fmt_battery` retirement + census |
| G5.1, G5.2 | `opus-high+` | `opus-high+` | `opus-high+` | doc surgery validated by `oracle_parity_cfg_gate.rs:1310` |

*(**2026-09-04**, coordinator decision **D8**: **G1.4a** ran at `opus-xhigh`, not the
`G1.4` row's `opus-high+` — three engine-semantics triggers, a new engine accessor and
the per-bus capture struct G1.5/G1.6(ii) inherit — and its spin-off **G1.4c** is
`opus-xhigh` too, for those triggers plus the `crates/dss-epri` do-not-call guard the
r4133 `VLL` hang needs. The `G1.4` row above now stands for **G1.4b** alone.
**2026-09-05**, coordinator decision **D26**: G1.4b is split once more — the `G1.4` row's
`opus-high+` is what **G1.4b** (the distances) actually ran at, and the spun-off
**G1.4d** (the at-bus lists `AllPCEatBus`/`AllPDEatBus`) is `opus-xhigh`: new engine code
for a criterion that is neither oracle's, three measured oracle mechanisms and a
two-sided assertion against an artifact that is not a function of port state.)*

## 1. Plan-wide design decisions

### 1.1 The fastdss parity target (what "no worse" means, precisely)

The reference is `.inputs/DSS-Python` @ `origin/fastdss` (read via `git show`, see
the source-integrity gate), `tests/save_outputs.py` (captures the full API facade
per deck into a zip) and `tests/compare_outputs.py` (recursive JSON-tree diff;
floats via `np.isclose(rtol=atol=1e-5)`; skip-list `KNOWN_COM_DIFF`). The groups it
compares that our live gate does not, mapped to the golden families that carry them
today:

| # | fastdss surface | our only witness today | lands in |
|---|---|---|---|
| 1 | per-element SeqCurrents / SeqVoltages / SeqPowers / CplxSeq* / Residuals / *MagAng / TotalPowers | `tests/golden/reports/export_seq*` | G1.3a–c |
| 2 | Bus Zsc1 / Zsc0 / ZscMatrix / YscMatrix / Isc / Voc | `fault_study` golden | G1.5 |
| 3 | meter extras: CalcCurrent, AllocFactors, Totals, SAIFI/SAIFIKW/SAIDI/CustInterrupts, active-section fields (fastdss captures the FIRST section only — parity = at least that), **ordered** zone vectors (the live gate already set-compares `AllBranchesInZone`/`AllEndElements`/`ZonePCE` — `harness/mod.rs:2022-2041`, `oracle_server.py:226-246`; the gap is order + the extras, NOT the lists' existence), **plus the per-bus reliability columns** `Bus.Lambda / N_interrupts / N_Customers / Cust_Interrupts / Cust_Duration / Int_Duration / TotalMiles / SectionID` (`IBus._columns` on `origin/fastdss` — the very columns `Export BusReliability` renders and the `BUS_INT_DURATION` surface, hence the G2.2a-before-G1.6 ordering). CAIDI does **not** exist in the DSS-Python API on either branch — it is gated only if the r4133 DLL exposes it (measure in G1.11c), else documented as not-comparable (**as executed 2026-09-05, part (i)**: the set compare is `compare_meter`'s `cmp_members`, `harness/mod.rs:6064-6079` — **not** `:2022-2041`, a stale citation — and the ordered arm is `compare_reliability`, `harness/mod.rs:8755-8790`; CAIDI **is** compared, as EnergyMeter property #22 through `compare_all_properties` on both channels, so "not-comparable" was never written; see §G1.6) | `reliability`/`export_busreliability*` goldens | G1.6 |
| 4 | Topology interface: NumLoops, NumIsolatedBranches/Loads, AllLoopedPairs, AllIsolatedBranches/Loads | `show_topology`/`show_isolated` goldens | G1.7 |
| 5 | Bus.Distance, AllBusDistances, AllNodeDistances | `profile` goldens | G1.4 (**landed 2026-09-05 as G1.4b**; §G1.4's as-executed note) |
| 6 | Solution.IncMatrix/IncMatrixCols/IncMatrixRows/Laplacian (fastdss-branch-only additions — exactly why the reference is `origin/fastdss`) | `inc_matrix/` goldens | G1.8 |
| 7 | circuit aggregates: TotalPower, Losses, LineLosses, SubstationLosses, **AllElementLosses**, plus the Solution scalars ControlIterations / Totaliterations / MostIterationsDone / ControlActionsDone / SystemYChanged / Seconds / LoadMult / Year / Hour / Mode | `export_losses`/`summary` goldens | G1.9 |
| 8 | run-produced files: **every** `*.csv` the deck emits under DataPath (fastdss archives and compares them all, incl. the forced `export profile phases=all` — DI CSVs are just the closedi subset); `save circuit` output **file set** (fastdss archives it but never compares — `compare_outputs.py:426-529` has no `.dss` branch — so our file-set + round-trip check is strictly stronger; state that, don't claim parity) | `di_*`/`save_*` goldens | G1.10 |
| 9 | CktElement discrete extras: PhaseLosses, NodeOrder, EnergyMeter, OCPDevType, OCPDevIndex, HasVoltControl, HasSwitchControl, NumControls, NumTerminals/NumPhases/NumConductors; LineGeometries.Rmatrix/Xmatrix/Zmatrix (measure-first); Lines.Yprim (verify it is already witnessed by the per-element YPrim live compare, record in TESTING.md) | scattered `props/`/report goldens | G1.3d |
| 10 | PDElements interface: AccumulatedL, ParentPDElement, FromTerminal, IsShunt, Numcustomers, SectionID, RepairTime, Totalcustomers, Lambda (**as executed 2026-09-04: 13 columns** — also FaultRate, TotalMiles, pctPermanent — plus `parent_name`; see §G1.6b) | `reliability` goldens (partially) | G1.6b |
| 11 | Bus extras: VLL/puVLL, VMagAngle, AllPCEatBus/AllPDEatBus | `export_seq*`/`profile` goldens | G1.4 (VMagAngle: G1.4a; VLL/puVLL: G1.4c, landed 2026-09-05; AllPCEatBus/AllPDEatBus: **G1.4d**, split out of G1.4b by D26) |

Where our gate is already stronger than fastdss (monitor channels, event log,
control queue, full Y/YPrim/injection, discrete state, two channels at once,
fail-on-stale ledger vs their passive skip-list, calibrated floors vs their blanket
1e-5, the named zone lists as live set-compares, CIM byte goldens in both lanes vs
their captured-but-never-compared XML, the `save circuit` round-trip) — nothing
changes.

Mechanics for every G1 sub-step: (a) capi channel first — the pinned dss-python
exposes every field natively via `tools/oracle/oracle_server.py` capture additions;
**capture order is contractual on the capi channel**: for every element,
`Powers`/`SeqPowers`/`TotalPowers`/`Losses` are read BEFORE any `Currents`-family
read (CLAUDE.md upstream bug 4 — harmonics stale-`Iterminal`; today the rule lives
in `gen_checkpoints.py::capture_element`), carried as a comment at the capture site
plus a test asserting the request order — *(**2026-09-04**, D3: restated as the
A/B/C partition in the WP-G1 preamble note; `SeqPowers` belongs to the
`GetCurrents`-into-a-scratch-buffer group that must be read **last**, and
`PhaseLosses` to the cache-aware group that must be read first)*; (b) Rust side
gets a read-only accessor
(`exec/view.rs` pattern) that computes the quantity from solved state, reusing the
shared math the reports call — e.g. the 012 transform `SymComp::phase_to_sym`
(`support/mathutil/mod.rs:105`, shared by `report/export/*` and `report/show/*`;
note the per-terminal/rating/Iresidual logic of `seq_currents.rs` itself is NOT
exercised by the live path — the goldens keep covering that until G3); (c) a
comparator in `tests/harness/mod.rs` with floors derived per
`tests/TOLERANCE_NOTES.md` discipline (documented, never guessed); (d) manifest
opt-in flags — the whole vocabulary, same shape as `compare_all_properties`,
declared **once** (G1.0, 2026-09-04) in `corpus_gate/manifest.rs` and mirrored in
`population_lock.rs::rigor()`: `compare_derived` (G1.3a–c), `compare_element_extras`
(G1.3d), `compare_bus` (G1.4), `compare_zsc` (G1.5), `compare_reliability` (G1.6),
`compare_pdelements` (G1.6b), `compare_topology` (G1.7), `compare_inc_matrix` (G1.8),
`compare_run_files` (G1.10a), `compare_di` (G1.10b). *(**2026-09-04**, G1.0: this
supersedes the six names originally written here — `compare_derived` now means the
per-element row only, `compare_bus` carries G1.4's half, and the other three names
were added; **G1.9 gets no flag**, its aggregates and solution scalars being
universal and cheap, so nobody adds an eleventh flag and a second lock regen. A flag
may be **set** by a manifest only once its own sub-step has flipped
`G1_SURFACE_FLAGS`' `wired` in the same commit as its request field + comparator.)*
Force-enabled by the scheduler for the suitable families (the `force_properties`
pattern in `corpus_gate/scheduler.rs`) — and since G1.0 a force rule ships in the
same commit as its own `FORCED_<FLAG>_POPULATION` pin + re-derivation test, because
the lock records the **manifest** flag, not the effective one;
`population.lock.json` regenerated in the same commit; (e) every known upstream
quirk triaged into `ledger.json` with r4133
evidence (do NOT assume the export-side bug shapes — measure the API path), **and
where the divergence is ours-is-right (an upstream defect, not a floor), an
expected-value pin naming the value our engine must produce — the ledger entry's
note cites the pin**; (f) **acceptance, every G1 sub-step**: the new comparator is
proven non-vacuous (a deliberately corrupted Rust value fails it, run once in a
scratch tree), the flag is set on ≥ 1 case per gating channel,
`population.lock.json` is regenerated in the same commit, every new floor has its
derivation in `tests/TOLERANCE_NOTES.md`, and more than ~10 new ledger entries (or
any entry that cannot be given a pin) is a stop-and-report. Seeding:
`DSS_GATE_SEED_LEDGER=1`.

### 1.2 Self-goldens: the regen discipline that replaces "never regenerate"

- `tests/golden/golden.lock.json` — `{path, sha256, anchor, reason, produced_by}`
  per artifact; anchors: `self` | `fpc_3.2.2` | `r4133` | `r3723` | `capi_v0145`
  (initially all pinned-oracle goldens; "frozen" = only the post-G3 residue) |
  `capi015` (frozen dead path). Lock scope: **all committed golden artifacts** =
  `tests/golden/**` **plus the registered out-of-tree witness**
  `crates/dss-core/tests/data/adiakoptics/r3723_ref/`. Test
  `crates/dss-core/tests/golden_lock.rs` (clone of `population_lock.rs` design),
  fail-on-stale in both directions + digest match + "anchor=self requires a
  registered reason". Regen: `DSS_UPDATE_GOLDEN_LOCK=1`.
- **Which lane writes a self-golden.** Until WP-G4 the two lanes render different
  report/json bytes (the print kernels). A self-golden is rendered by the lane
  holding the family's STRICTEST contract: for every family whose comparator has a
  parity-only byte arm — `lane::compare_report` (parity `assert_bytes_eq`, default
  `compare_export`; `harness/lane.rs:623-637`) and `lane::compare_json` (parity raw
  `assert_eq!`, no CRLF normalization; `lane.rs:477-481`) — the producing lane is
  the **parity** lane (`DSS_UPDATE_GOLDENS=1` + `--features dss-core/oracle-parity`):
  parity-rendered bytes are what the parity byte arm already passes, and the
  default lane's `exact_value_policy` (rel=abs=0) passes against that same
  rendering today, so both gates stay green with zero comparator surgery. The lock
  records each family's producing lane / lane-invariance; `snapshot_*` refuses to
  write from the non-producing lane; lane-invariance (`cim/` after G2 — Show text
  still passes through `fmt_g`/`render_rows` until G4.1/G4.5, so Show families
  are parity-produced too) is **verified** by a cross-lane regen into a scratch
  dir, never assumed.
  This asymmetry is temporary: WP-G4 makes both lanes render identically, after
  which every family is lane-invariant by construction.
- `harness::snapshot_*` helpers honor `DSS_UPDATE_GOLDENS=1` and **refuse to write
  any artifact whose lock anchor ≠ `self`** — de-anchoring is a reviewable lock
  edit, never a side effect of a regen run.
- Regen rules (written into TESTING.md by G0.2, enforced culturally + by the lock):
  R1 clean tree only; R2 never to fix a red gate — the sequence is fix → pin the
  intended new value with an expected-value test → regen → the diff moves ONLY the
  predicted cells; R3 per-family only (no bare "regenerate everything"); R4 the
  lock digest diff is the review artifact and the commit body names every moved
  artifact and its cause.
- Frozen (never regenerated, `snapshot_*` hard-refuses; compared **value-wise**
  after WP-G4 — see the WP-G4 preamble):
  `json/schema_full_oracle.json` + `schema_divergences.json` (the external half of
  the schema pair; `schema_full_port.json` is already self via `REGEN_SCHEMA_PORT`),
  `crates/dss-core/tests/data/adiakoptics/r3723_ref/` (the only external
  A-Diakoptics witness, anchor `r3723`), `wasm_usermodels/` (stays r4133-anchored,
  `crates/dss-epri/tests/gen_wasm_usermodels*.rs`), and — because they have **no
  live value twin** (see G3.1/G3.5) — `pstcalc/` (the only Pst-value oracle gate:
  the live case cannot carry `compare_global_result`, `tests/corpus/modes/manifest.json:860`),
  `plot_callback/` (captured through the oracle's `DSS_RegisterPlotCallback`,
  TESTING.md — no corpus surface), `ncim/` (Jacobian/deltaF/deltaZ/PV2PQ report
  numbers; the ncim corpus cases live-compare only V/Y/injection/iterations).
  `fmt_battery.csv` is NOT on this list anymore: it retires with the print kernels
  in WP-G4 (G4.6), measure-first. This is the **initial** frozen set — G3.1/G3.2a
  may add rows (`autoadd_reduce.json`/`parser.json` if their twin audit fails),
  each with a reason, mirrored in G3.6. (`protection/` and `flicker/` are not
  frozen-`capi_v0145` — they stay externally anchored `r4133`; disposition
  recorded in G0.1/G3.6.)

### 1.3 What is deliberately OUT of scope

- **No parity-lane teardown here — but the lane shrinks to numeric-only.** After
  WP-G2 exactly 11 split aliases survive; WP-G4 (this plan) then removes the six
  rendering rows (`fmt_g`, `fixed_w_script`, `json_float`, `JSON_LINE_BREAK`,
  `CONTROL_QUEUE_SEC_DIGITS`, `render_rows`), leaving **5 numeric rows**: `PI`,
  `round_f64`, `round_i32` (dss-parser), `kv_base_search_scale`,
  `profile_ll_pu_divisor` (dss-core) — truncated constants and FPC `Round`
  computation semantics, owned by the UPGRADE line. Dismantling the parity lane
  itself remains a later, dedicated WP — by then a purely numeric one.
- **No tolerance floor moves.** `tests/TOLERANCE_NOTES.md` tiers are untouchable;
  new G1 floors are added with their derivations, existing ones never relaxed.
- **Generator model-6 `FInit` stale `Vterminal`** (`user_model.rs:729`) is deferred
  to `WASM_USERMODELS_PLAN.md` — it needs its own decision about diverging from an
  r4133-anchored dynamics trajectory; it must not ride this plan. **That plan
  carries no such item today**, so G5.2 must (a) add a named WP row to
  `WASM_USERMODELS_PLAN.md` (the model-6 FInit seeding decision, with the
  wasm_gen_dyn golden bill measured) and (b) record it in `ORPHANED_GAPS.md` until
  that row exists; the closing record reports it as the last reproduced upstream
  bug (`Escape::WholeCase` survivor).
- **`tools/golden/gen_*.py`** generators become historical after G3 (kept in-tree
  for archaeology; `check_pin` stays importable while anything still uses it).

---

## WP-G0 — Safety rails (before any regen capability exists)

### G0.1 — `golden.lock.json` + `golden_lock.rs`

New `tests/golden/golden.lock.json` fingerprinting all committed golden artifacts
(~727 files under `tests/golden/` plus the registered out-of-tree
`crates/dss-core/tests/data/adiakoptics/r3723_ref/` tree) as
`{path, sha256, anchor, reason, produced_by}` (§1.2), and new test
`crates/dss-core/tests/golden_lock.rs`
modeled on `crates/dss-core/tests/population_lock.rs`: (1) every file has a row;
(2) every row has a file (closes the `props_roundtrip.rs:209-211` hole — today it
asserts only non-emptiness); (3) every digest matches; (4) every `anchor: "self"`
row has a non-empty reason and the `self` set equals a `DEANCHORED` register const
in the test (the `oracle_parity_cfg_gate.rs::ESCAPE_REGISTER` both-ways
discipline). The `DEANCHORED` register is kept per **family/glob** with one
reason per family (a per-file list would be a second copy of the lock); its two
initial entries (`adiakoptics/`, `json/schema_full_port.json`) are recorded as
born-`self`, never de-anchored. G3.6 extends the reason requirement to every
non-`self` row. Initial anchors: `capi_v0145` for the pinned-oracle families; `r4133`
for `flicker/`, `wasm_usermodels/`, the `protection/` r4133 arms
(`gen_protection.py:217`); `fpc_3.2.2` for `fmt_battery.csv`; `r3723` for the
`r3723_ref/` tree; `capi015` for the 11 artifacts whose provenance block declares
`engine_spec: "capi015"` — `props/{linemedium,autotrans_bh,linespacing_eqspacing,`
`regcontrol,swtcontrol,transformer_bh}.json`, `ncim/{pq,pv_qlimit}.meta.json`,
`line_constants/line_geometry_carson.json`,
`reports/{export_capacity_seasonal,export_overloads_seasonal}.meta.json` (their
generator environment no longer exists, so `snapshot_*` hard-refuses them); `self`
for `adiakoptics/` (the in-tree `midi_torn_tree.txt`) and
`json/schema_full_port.json`. Knob: `DSS_UPDATE_GOLDEN_LOCK=1`.

Verification: negative probes in a scratch copy — touch one golden byte → digest
assertion fails; delete one `props/` class file → missing-file assertion fails;
flip an anchor to `self` without a register row → fails. Gate green both lanes.

### G0.2 — snapshot helpers + regen procedure docs

`harness::regen()` — the shared `DSS_UPDATE_GOLDENS` + lock-update plumbing —
with `harness::snapshot_text()/snapshot_bytes()` routed through it, honoring the
anchor-guard AND the producing-lane guard of §1.2 (no driver calls them yet —
that is G3). **Acceptance:** unit tests over a scratch fixture prove all three
guard outcomes — write refused when `anchor ≠ self`; refused from the
non-producing lane; accepted for `self` + producing lane. `props_roundtrip.rs`
gains the class/scenario count lock. TESTING.md: new "Golden provenance lock" section beside
the population-lock section; R1–R4 in §Procedures; `DSS_UPDATE_GOLDENS` /
`DSS_UPDATE_GOLDEN_LOCK` in §Environment variables. `tools/golden/README.md` regen
paragraph replaced by a pointer.

---

## WP-G1 — Live gate to fastdss parity

Each sub-step lands capi-channel capture + Rust accessor + comparator + floors +
manifest flags + ledger triage (+ pins for ours-is-right divergences), per §1.1
mechanics (a)–(f). The r4133 channel joins in G1.11a–c for the groups the DLL
exposes.

> **2026-09-04 — three amendments (coordinator decisions D1/D2/D3), all landed by
> the new rails sub-step G1.0** (branch `update`; full record:
> `docs/phase-records/golden-rebase.md`, §"GOLDEN_REBASE WP-G1 — records").
>
> **D1 — G1.0 added, ahead of G1.3a.** Three rails every later G1 sub-step would
> otherwise re-pay or silently skip, landed once with **zero** comparator, zero
> engine change, zero ledger entry and zero golden byte:
> (1) the **whole** manifest flag vocabulary is declared once in
> `corpus_gate/manifest.rs` and mirrored in `population_lock.rs::rigor()` in a
> **single** `population.lock.json` regen — a flag absent from that format string
> is invisible to the anti-shrink guard, and ten sub-steps each rewriting all 523
> rigor rows is ten unreviewable diffs instead of one; a `G1_SURFACE_FLAGS` table
> refuses any manifest that sets a flag whose capture request + comparator do not
> exist yet, so a surface flag can never go live vacuous.
> (2) The **ten** committed bare `element` ledger exclusions (8 cases) now spell
> their `channels` explicitly: `Scope::channels` empty means *all*, so the element
> sub-channels G1.3a–c adds would have widened reviewed entries with no ledger
> diff and no lock trip. Three load-time rules keep it that way, including the
> typo that today loads cleanly and selects nothing.
> (3) `harness::capture_guard` — the rail a flag-gated comparator calls first, so
> a flag that is ON while that channel's capture is absent/empty **fails the
> case** instead of comparing 0 == 0.
>
> **D2 — G1.11a/b/c re-scoped.** The vendored r4133 DLL is the **grouped DDLL**
> API — 42 families / 147 entry points (`CktElementI/F/S/V`, `BUSI/F/S/V`, …),
> every one bound at load — and has **no** `*_Get_*` symbols, so those sub-steps'
> `GetProcAddress`-miss acceptance clause is not executable as written. It is
> replaced by a **mode-probe** clause: an unknown property falls through its
> family's Pascal `case` into an `else` that returns a **sentinel**, and
> `crates/dss-epri/src/modes.rs` classifies the four ABI shapes' sentinels into
> `Served` / `UnknownMode` / `DoNotCall`. G1.0 landed the rails (the probe, the
> typed mode accessors, the two-double `CircuitF`/`CmathLibF` ABI fix, the
> do-not-call register) **and executed this acceptance once for all of WP-G1**:
> `crates/dss-epri/tests/modes.rs::r4133_mode_capability_is_complete_for_wp_g1`
> proves all **96** modes WP-G1 needs classify `Served` on a solved deck, so the
> expected-miss list is **empty**. Each surface sub-step therefore wires **both**
> channels in the same commit (capi via `tools/oracle/oracle_server.py`, r4133 via
> the typed accessors); a group the DLL cannot serve is recorded in `TESTING.md`
> with its mode and sentinel, never masked capi-only. WP-G1 still closes with the
> `TESTING.md` mode-capability record (**G1.11′**).
>
> **D3 — §1.1(a)'s capture-order rule, restated as an A/B/C partition.** On the
> capi channel, per element: **(A)** the cache-aware quantities that go through
> `ComputeIterminal` — `Powers`, `TotalPowers`, `Losses`, `PhaseLosses` — are read
> **before** **(B)** every read that calls `GetCurrents` into a scratch buffer —
> `SeqPowers`, `SeqCurrents`, `CplxSeqCurrents`, `Residuals`, `CurrentsMagAng`,
> `Currents`; **(C)** order-free reads (voltages, discrete state) go anywhere.
> `SeqPowers` is a *poisoner*, not a victim, and `PhaseLosses` belongs to the
> must-read-first group — neither follows from the pre-G1.0 wording. G1.3a's
> capture test asserts the request order; G1.0 adds no capture read on either
> transport, so it owes no order test (stated so the absence does not read as a
> gap).
>
> **As executed (2026-09-04).** G1.0 landed on `update` exactly as scoped — no
> comparator, no `crates/dss-core/src` change, **0** ledger entries, 0 golden bytes,
> no floor, no `lane_diff` owed — with three corrections settled in-part against the
> vendored r4133 source and now enforced by code: `Cdang(0,1)` cannot return the
> 90.0 this plan's draft expected (`Ucomplex.pas:96-121`, two truncated constants),
> the WP-G1 mode table is **96** rows and not 98 (`PDElements` `F:1`/`F:3` are
> *write* arms, `DPDELements.pas:143`/`:162`, held in `modes::EXCLUDED_WRITE_MODES`),
> and the acceptance is a single `#[test]` because the DDLL is a process-global
> singleton. One unplanned edit: 29 `file.rs:LINE` citations in `TESTING.md` and
> `tests/TOLERANCE_NOTES.md` were re-pointed after the rails moved their targets (no
> floor, tier or verdict changed). Full record: `docs/phase-records/golden-rebase.md`
> §"GOLDEN_REBASE WP-G1 — records".

### G1.1 — `all_properties` on the r4133 channel

Stop masking properties on r4133: `corpus_gate/scheduler.rs:358-363` (and the
seeding path) + force-enable for r4133-gating cases (`scheduler.rs:102-114`); the
capture is already built (`crates/dss-epri/src/capture.rs:208-367`, masked by
policy not capability). Seed with `DSS_GATE_SEED_LEDGER=1`, hand-triage the
candidates into `tests/corpus/ledger.json` (expect a `PROPS_R4133` shape-allowlist
sibling to `PROPS_015X`, `harness/mod.rs:1404-1439`); every ours-is-right entry
gets its expected-value pin (§1.1(e)). Regenerate `population.lock.json` same
commit. **Kill criterion:** more than ~15 new ledger entries, or any entry that
cannot be pinned → stop and report; that magnitude means the r4133 property
surface diverges materially and needs its own plan.
Outcome: the r4133-only cases get a property check for the first time.
*(**2026-09-04**, RP5.2 audit settlement: **97**, not the 96 counted when this
sub-step was authored — `tests/corpus/manifests/population.lock.json` carries 523
cases = 367 `both` + 97 `r4133` + 59 `capi_v0145`, i.e. **464** r4133-gating.)*

> **2026-08-22 — superseded by `R4133_PROPS_PLAN.md`** (user decision; the kill
> criterion fired 2026-08-08 — census: 433 of ~512 live cases diverge, 209
> structural pairs / 94 numeric pairs / 5 property-table shape gaps, STATUS §1).
> The dedicated plan delivers this sub-step's outcome as its RP4.1, with this
> kill criterion re-armed there; the `PROPS_R4133` shape-allowlist expectation
> above is superseded by the TOLERANCE_NOTES reuse doctrine (R4133_PROPS §1.2 —
> `PROPS_015X` is reused). G3.4/G3.5 wait for RP4.1.
>
> **2026-09-04 — SATISFIED.** RP4.1 delivered the unmask 2026-09-03 (the re-armed
> kill criterion did not fire) and `R4133_PROPS_PLAN.md` COMPLETED 2026-09-04,
> archived at `docs/plans-archive/R4133_PROPS_PLAN.md`; its closing record is
> `docs/phase-records/r4133-props-rp5.md` §RP5.2. **G1.1 is closed and G3.4/G3.5
> are unblocked.**

### G1.2 — corpus deck for the one zero-coverage class

`ESPVLControl` is instantiated by **no** corpus deck (verified;
`makeposseq_ctrl.dss:10` documents its absence as deliberate for *that* deck — do
not perturb it). Author one deck under the controls family, manifest row with
probes, validate on the pinned oracle first, gate on both channels. **Caution:**
ESPVLControl is one of the five controls whose MakePosSequence override
dereferences NIL and aborts the oracle (`makeposseq_ctrl.dss:9-13`, manifest note)
— the new deck must avoid that verb and be oracle-validated in a scratch process
before the manifest row lands, else it is classified `expect_solve_abort` with a
reason. (`linemedium` is a props-golden *scenario* name, not a class; its subject
matter — `Line.l1` with `EpsRMedium`/`HeightOffset`/`HeightUnit` — is already
live-gated by `tests/corpus/modes/upgrade/upgrade_linecs_epsrmedium.dss` and
`upgrade_linecs_heightoffset.dss`; no new deck — record the mapping in STATUS
now, G3.1 carries it into TWINS.md.) **Acceptance (replaces §1.1(f), which
presumes a new comparator):** the deck solves on both channels (or is classified
`expect_solve_abort` with a reason), an ESPVLControl object is actually
instantiated and covered by probes, `population.lock.json` regenerated in the
same commit.

> **2026-08-29 — DONE, by a third route the acceptance did not enumerate**
> (branch `r4133-props`; record + audit settlement in STATUS §"GOLDEN_REBASE
> WP-G1 — records"). The deck is
> `tests/corpus/controls/espvlcontrol/espvlcontrol.dss` — six ESPVLControls in
> all four instantiation shapes over a 12-step daily ramp, 8 probes, no
> `MakePosSequence`, `isolate: true`. It solves cleanly on the **pinned oracle**
> and is fully gated there, so `expect_solve_abort` would have been a false
> statement; what it cannot do is gate r4133, because that DLL raises #303
> (access violation, read of `0x0`) on **every** `New espvlcontrol.<name>`
> before a property is parsed — measured on a minimal deck, so it is the class
> constructor and not this deck. Landed as `engines: "both"` with the r4133
> channel `kind: "skip"`ped (`r4133-espvlcontrol-uninstantiable`), the shape the
> four `r4133-*-303` skips already use. Population 522 → 523 cases;
> `population.lock.json` regenerated in the same commit. Two standing follow-ups
> came out of it (the upstream write-up, and `ESPVLControl.Forecast` — r4133
> property 12, absent from the port and invisible to the R4133_PROPS census
> because the class cannot be built there).

### G1.3a — per-element polar channels

`CurrentsMagAng`, `VoltagesMagAng`, `Residuals`. Polar-rendering floors derived
from the already-gated I/V tier, documented in TOLERANCE_NOTES. Ledger triage per
§1.1(e) — do NOT assume the export-side Iresidual bug shape; measure the API path.

> **2026-09-04 — AS EXECUTED (lane `lane-e`).** Landed with `Enabled` alongside the
> three polar channels, forced on **442** cases (`FORCED_DERIVED_POPULATION`
> `(442, 315, 83, 44)`), both oracle channels.
>
> * **Q-1 settled by measurement, 0 rows.** Both API paths carry the `(i-1)*Nconds`
>   terminal offset (r4133 `DDLL/DCktElement.pas:842`, capi
>   `CAPI/CAPI_CktElement.pas:562`); the `Export SeqCurrents` `Iresidual` defect is
>   confined to the report path, so it costs no ledger entry — as the plan
>   suspected but did not assume.
> * **Enabled-only capture.** r4133's `CktElementV(19)` dereferences a nil
>   `NodeRef` (`DCktElement.pas:1099`, no guard) and **kills the worker** on a
>   never-enabled element, where capi answers a one-element `DefaultResult`
>   sentinel (`CAPI_Alt.pas:1081`). Reading the three channels for `Enabled`
>   elements only removes the crash class and makes both transports' shapes
>   identical — no sentinel normalization owed, no ledger row.
> * **One mode taken here:** `CktElement.Enabled` (`CktElementI(12)`,
>   `DCktElement.pas:263`), so `WP_G1_MODES` is **97**, not 96 — it is the safety
>   predicate the enabled-only capture needs, and a fastdss `_columns` surface in
>   its own right.
> * **Two manifest opt-ins** — `Test/AutoTrans/{Auto3bus,AutoHLT}.dss`, the exact
>   two decks fastdss *skips* `Residuals` on (`origin/fastdss`
>   `tests/compare_outputs.py:56-59`, "Close enough for the system"): gating them
>   is where this gate is strictly stronger than the harness it reaches parity with.
> * **No new channel joins `LANE_SKIP_ELEM_POWERS`.** The Newton staleness is
>   confined to the cache-aware `Get_Powers`/`Get_Losses` path
>   (`Common/CktElement.pas:632-640`); all three of these come from a fresh
>   `GetCurrents` or from `NodeV`, so the two `newton*` decks gain three compared
>   channels rather than an exclusion.
> * **Ledger: §3.4's "0 new entries" forecast was wrong in one place.** The final
>   count is **1 new entry + 13 measured widenings** of committed `element` scopes
>   (the kill criterion, "> ~10 **new** entries", did not fire). The new entry is
>   `capi-capcontrol-time-bus-is-the-capacitors`: a TIMECONTROL CapControl binds
>   its bus 1 to the **monitored element's** terminal (r4133
>   `Controls/CapControl.pas:605` + `:622`), while capi 0.14.5 still uses the
>   controlled capacitor's bus (`:597-608` → `:619`) — the port follows r4133, the
>   r4133 channel needs no entry, `docs/upgrade/DIVERGENCES.md` L8 records it and
>   `capcontrol_time_voltages_follow_the_monitored_elements_terminal` pins both
>   numbers. The 13 widenings are per-sub-channel and measured (fixpoint iteration;
>   `r4133-indmachmidi-injection-ulp` did not fail and was left alone).
> * **Two comparator-shape findings the spec did not predict**, both settled without
>   a tolerance: a 0-terminal element (`UPFCControl`, r4133
>   `Controls/UPFCControl.pas:230-246` never sets `Nterms`) is accepted two-sidedly
>   when neither side carries a payload, up to the capi `DefaultResult` sentinel
>   (coordinator decision D4); and the spec's √2 rectangular-band correction (D10)
>   is **refuted** — `harness::assert_complex_close_c` bands the *modulus*, so the
>   inherited set is a disc and every derivation was already its image. No band moved.
> * The first launch of this sub-step collided with G1.0's audit settlement in the
>   main tree; F1 backed out cleanly and the sub-step was relaunched on `lane-e`
>   (`tmp/g13a/spec_amendment.md`).

### G1.3b — per-element sequence transform

`SeqCurrents`, `SeqVoltages`, `SeqPowers`: one shared accessor over
`SymComp::phase_to_sym` (§1.1(b)), one floor derivation for the whole linear
transform.

### G1.3c — per-element complex sequence + totals

`CplxSeqCurrents`, `CplxSeqVoltages`, `TotalPowers`.

### G1.3d — per-element discrete extras

`PhaseLosses`, `NodeOrder`, `EnergyMeter`, `OCPDevType`, `OCPDevIndex`,
`HasVoltControl`, `HasSwitchControl`, `NumControls`,
`NumTerminals`/`NumPhases`/`NumConductors` (discrete → zero tolerance);
`LineGeometries.Rmatrix/Xmatrix/Zmatrix` (measure-first: floors only if a
divergence is proven, per the no-guessing rule); `Lines.Yprim` — verify it is
already witnessed by the per-element YPrim live compare and record the conclusion
in TESTING.md instead of double-capturing.

> **2026-09-04/05 — AS EXECUTED (lane `lane-e`), part (i).** The sub-step is split:
> **(i)** the index/name scalars `NumTerminals`/`NumConductors`/`NumPhases`, `NodeOrder`,
> `EnergyMeter` and the two documentation verdicts; **(ii)** `PhaseLosses` and the
> control-derived extras (`OCPDevType`/`OCPDevIndex`, `HasVoltControl`/`HasSwitchControl`,
> `NumControls`). `CktElement.Enabled` is not a NEW channel here — G1.3a landed it — but it
> is re-emitted under the `element_extras` request (the two manifest flags are independent)
> and re-asserted by this comparator, since the `NodeOrder` capture predicate rests on it.
> Forced on **441** cases
> (`FORCED_ELEMENT_EXTRAS_POPULATION` `(441, 310, 87, 44)` after the merge into
> `update`; `(440, 313, 83, 44)` on the lane, before G1.4a's D12/D14 flips), both oracle channels,
> **0 new ledger entries and 0 widenings** — the §3 forecast held exactly, and no tolerance
> is introduced or consulted (every field is discrete, compared exactly).
>
> * **`Lines.Yprim` — verified already witnessed, not double-captured.** `Lines_Get_Yprim`
>   (capi `CAPI/CAPI_Lines.pas:777-796`) and `CktElement_Get_Yprim`
>   (`CAPI/CAPI_CktElement.pas:583-599`) are the same two statements —
>   `GetYprimValues(ALL_YPRIM)` plus a bulk `Move` of `2·Yorder²` doubles — and r4133's
>   `LinesV` mode 7 (`DDLL/DLines.pas:771-796`) and `CktElementV` mode 12
>   (`DDLL/DCktElement.pas:856-883`) likewise copy `SQR(Yorder)` complexes from one such
>   call; on capi the two are exactly equivalent modulo the `Lines` path's `IsLine()` type
>   filter, and on r4133 they differ only outside the payload (mode 12 `Exit`s on a nil
>   `cValues`, `DCktElement.pas:869-872`, before assigning `myPointer`/`mySize` at `:882-883`;
>   mode 7 assigns them regardless, `DLines.pas:794-795`). The gate already
>   compares `CktElement.Yprim` live on both channels, so the conclusion and its honest
>   per-case residual are recorded in TESTING.md instead of a second capture.
> * **`LineGeometries.Rmatrix/Xmatrix/Zmatrix` dropped from the parity claim**, on two
>   independent kills: they are computing **methods** taking `(Frequency, Length, Units)`
>   (`origin/fastdss` `dss/ILineGeometries.py:84`/`:88`/`:92`), so fastdss's own harness
>   raises `StopIteration` on them (`tests/save_outputs.py:140-141`) and the caller swallows
>   it (`:277-279`) — they are skipped in *every* fastdss run, i.e. they are not part of the
>   parity target; and the r4133 DLL has no `LineGeometr*` family at all (no
>   `DDLL/DLineGeometries.pas`, and `OpenDSSDirect.dpr`'s `exports` clause carries only
>   `LinesI/F/S/V`), so the surface is capi-only by capability. No capture, no ledger row.
> * **The `NodeOrder` capture predicate is source-derived, not defensive.** It is read only
>   for an element that is `Enabled` **and** has `NumTerminals > 0`: r4133's `CktElementV(17)`
>   dereferences `NodeRef^[j]` with no nil guard (`DDLL/DCktElement.pas:1048`) and kills the
>   worker on a never-enabled element, while capi raises 15013 (`CAPI/CAPI_CktElement.pas:900-906`);
>   and on a 0-terminal element (`UPFCControl` never assigns `Nterms`,
>   `Controls/UPFCControl.pas:230-246`) r4133 answers a 0-length array where capi raises. Not
>   issuing the read removes that shape asymmetry instead of normalizing it. The comparator's
>   `!enabled` branch then asserts the **oracle** side is silent only: an element disabled
>   *after* a solve legitimately keeps its mapping in the port, and so would both oracles
>   (neither mode-17 arm has an `Enabled` guard).
> * **One channel normalization, 0 ledger rows (coordinator decision D4):** "no meter" is
>   spelled `''` on capi (`Result := NIL`, `CAPI/CAPI_CktElement.pas:672-687`) and `'0'` on
>   r4133 (the `CktElementS` pre-`case` default, `DDLL/DCktElement.pas:421`; arm 4 at
>   `:442-449`, guarded by `HasEnergyMeter` at `:444`). Each channel's OWN spelling is folded
>   at the capture boundary and pinned (audit settlement 2026-09-05: the fold takes the channel,
>   so on capi a meter *named* `0` is a name and reds if the port loses it, while on r4133 the
>   collision is undecidable in both directions) — what keeps it unreachable is the corpus
>   census (1310 decks, 92 distinct meter names, none of them `0`), not the fold.
> * **D19 (coordinator, 2026-09-05):** lane-m's D9 engine commit — `MakeBusList` must reset
>   the meter zones (`Common/Circuit.pas:2411`) — was cherry-picked onto `lane-e`, because this
>   is the first surface that makes D9 observable in the gate; **amended (D19′): the pin's
>   PDElements half is read directly on lane-e** (`Dss::pd_elements` arrives only with G1.6b,
>   so `exec/tests/energymeter_zones.rs` reads the customers/parent numbers through the file's
>   own `branch_customers`/`branch_parent` helpers — same oracle numbers, engine hunks
>   byte-identical), which leaves exactly one predictable merge conflict in that file, resolved
>   toward `update`'s PDElements-walk form.
> * **F5 STOP-1:** the spec's D9-independence claim was false — it was measured per *file*,
>   while `Test/indmachtest/Master.DSS` redirects its meter and only then calls `MakeBusList`,
>   so `Line.l1` reported no meter until the D19 cherry-pick (never a ledger row, per D9).
> * **F5 STOP-2:** the corpus meter-name census raced the live gate's own deck-written
>   exports (a Windows sharing violation on a file the gate owns), so it moved verbatim from
>   `corpus_gate/runner.rs` into the oracle-free `corpus_manifest.rs` binary — the race is
>   removed structurally, with no weakened assertion, no retry and no skip.

> **2026-09-05 — AS EXECUTED (lane `lane-e`), part (ii).** `PhaseLosses` and the five
> control-derived scalars `NumControls`, `OCPDevIndex`, `OCPDevType`, `HasVoltControl`,
> `HasSwitchControl` compare live on **both** channels over the same **440** cases
> (`FORCED_ELEMENT_EXTRAS_POPULATION` `(440, 313, 83, 44)`, unchanged — this sub-step widens the
> flag's *fields*, not its population), which completes the `compare_element_extras` surface and
> §G1.3d as a whole. **0 new ledger entries, 0 new causes** (58 / 31 unchanged on the lane), **10** measured
> widenings of committed `element` scopes, 0 golden bytes, no existing band moved.
> *(On `update` after the merge: the population is **443** = (443, 312, 87, 44) and the ledger **54** / 31 —
> `gic-pct-r2-honoured-{gictransformer,midi}-capi` were already deleted by G1.4a's **D12**/**D14**, so two of
> the ten widenings went with their entries and **eight** land; `makeposseq-cuf-applied-capi`'s sample was
> re-measured on the moved deck and still fails. Phase record, merge note.)*
>
> * **`PhaseLosses` is ported, not derived from `Losses`.** The port had no `GetPhaseLosses`;
>   `CktElement::phase_losses` (`elements/traits.rs`) is r4133
>   `Common/CktElement.pas:1078-1120` loop-for-loop — `ComputeIterminal` at `:1090`, the phase
>   loop `:1093-1112` summing `NodeV[NodeRef[(j-1)*FNconds+i]]·conj(Iterminal[…])` over the
>   element's terminals and skipping `n = 0`, ×3 under positive sequence, `CZERO` zero-fill on
>   `!FEnabled` (`:1118-1119`). It returns **W/var**; the oracles' ×0.001 (r4133
>   `DDLL/DCktElement.pas:637` mode 6, the `cmulreal(…, 0.001)` at `:651`; capi
>   `CAPI/CAPI_Alt.pas:449-467`, the multiply at `:466`) is a capture-boundary encoding applied
>   at exactly one site, the comparator.
> * **The band is a derivation, not a new class** (`tests/TOLERANCE_NOTES.md` §G1.3d(ii)):
>   `harness::phase_loss_band` sums `assert_power_close`'s per-conductor floor over exactly the
>   conductors `GetPhaseLosses` sums, i.e. the construction `Get_Losses` already applies over
>   *all* of them, restricted to one phase — strictly tighter, and no `Tolerances` field is read
>   or written. The `Get_Losses` oracle-self-consistency trust escape is deliberately **not**
>   copied (it exists for a capi015 quirk on a rev this gate does not run).
> * **`PhaseLosses` DOES join `LANE_SKIP_ELEM_POWERS`** on the two `newton*` decks, unlike G1.3a's
>   three polar channels: it opens with the same cache-aware `ComputeIterminal` as
>   `Get_Powers`/`Get_Losses`, so no oracle reports it at the converged `NodeV`
>   (CLAUDE.md bug 5 / G2.3). **Measured before the bit was added**, both decks red on
>   `Vsource.source` phase 0 at 55.5× / 34.4× the band (`|Δ| = 4.8559011331706704e-4` and
>   `2.460687672864992e-3` kVA) — bit-identical to the `Powers` figures the row already records,
>   which is the mechanism, not a coincidence. The five discrete scalars stay compared there.
>   *(Audit settlement, 2026-09-05: that first measurement stopped at the `capi_v0145` channel,
>   because a case aborts on its first failing channel. Re-measured on the **`r4133`** channel by
>   flipping the two cases to `engines: "r4133"` in a scratch copy: same element, same phase,
>   `|Δ| = 4.855901044093186e-4 > 8.753018514278278e-6` and
>   `2.4606876731535624e-3 > 7.144000397412528e-5` kVA — i.e. both gating channels carry the
>   staleness, as CLAUDE.md bug 5 says of every official rev. Nothing was committed from that copy.)*
> * **D-ii-1 — the control list is attach-ordered, and r4133 re-attaches on every edit.** Pascal's
>   per-element `ControlElementList` is remove-then-append (`Controls/ControlElem.pas:113-131`,
>   `RemoveSelfFromControlElementList` at `:81-99`); r4133 re-runs
>   `ControlledElement := CktElements.Get(DevIndex)` inside **`RecalcElementData`**, i.e. on every
>   Edit (`Relay.pas:955` from `:626`; `Recloser.pas:702`, `SwtControl.pas:332`,
>   `CapControl.pas:580`, `RegControl.pas:693`), while capi 0.14.5 makes `ControlledElement` a
>   property-write target only (`Controls/Relay.pas:439-441`). Measured: after
>   `edit relay.r delay=0.05`, r4133 answers `OCPDevType` **1** (the fuse, the relay having moved
>   to the end) and capi **3**. The port follows r4133 — a new `Circuit::control_attach_order`
>   maintained at the port's `RecalcElementData` moment, derived per element by
>   `circuit::controls::derive_control_lists`, which `Show Controlled` now shares so the report and
>   the API cannot drift. **0 ledger rows:** no corpus deck carries a *heterogeneous*
>   multi-control element, so the divergence is not observable in the gate; it is pinned by
>   `ocp_dev_type_follows_the_last_attach_order` (port 1, r4133 1, capi 3) and recorded in
>   `docs/upgrade/DIVERGENCES.md` **L9**.
>   *(Audit settlement, 2026-09-05. The census quoted here — 60 controlled elements,
>   `max NumControls = 1` — was a live capi walk of `tests/corpus/controls/**` only, and does NOT
>   hold over the gated population: the whole-gate census now re-derived on every run finds
>   **18** (case, channel, step, element) rows with ≥ 2 controls, out of 3 184 controlled rows in
>   298 565. All 18 are Relay-ONLY lists — `Line.thev` under `Relay.21src` + `Relay.21rev` in the
>   eight Distance/TD21 relay decks, `Line.motorleads` under `Relay.{mfrov/uv,mfr46,mfr47}` in
>   `controls:fuse/indmach_r4133/indmach_{snap,dyn}.dss` — so every permutation answers the same
>   `OCPDevIndex = 1` / `OCPDevType = 3` and the conclusion stands, now on the right premise. The
>   counts are pinned fail-on-stale in both directions by
>   `harness::assert_no_multi_control_element`, called from the gate epilogue.)*
> * **D-ii-2 — a *disabled* OCP control still holds its slot and still wins the scan** (r4133
>   `Common/Utilities.pas:3165-3184` has no `Enabled` test; both channels agree). The accessors
>   therefore recompute from the derived list with no `Enabled` filter anywhere, instead of reading
>   the port's registration latch `CktElementData::ocp_device_type`, which is written for the first
>   *enabled* OCP control and answers `3` where both oracles answer `1`. Pinned by
>   `a_disabled_ocp_control_still_wins_the_ocp_scan` (names all three numbers).
> * **Ledger: one new sub-channel, ten measured widenings, no new entry.** `phase_losses` joins
>   `SUBCHANNEL_FIELDS`' `element` list (seven names now) in the same commit, and both ledger
>   handlers honour it — `rewrite_element_selected` writes the port's kW/kvar back and
>   `envelope_element` bands each phase with the same derived floor. Ten committed `element` scopes
>   that already select `powers`/`losses` were widened onto it, each **only** after the live gate
>   printed its own failing sample on its own channel (`measured.g13d2_phase_losses_first_failure`):
>   `mmf-accept-set-honoured-capi`, `makeposseq-cuf-applied-capi`,
>   `gic-pct-r2-honoured-{gictransformer,midi}-{capi,r4133}` and
>   `windgen-qmode0-constant-q-{daily,snapdelta,dyn,dynfault}-r4133`. The four
>   `r4133-*-injection-ulp` entries (which select `losses`) were measured **not** to fail and keep
>   their committed lists; `capi-capcontrol-time-bus-is-the-capacitors` likewise (a CapControl has
>   no `Iterminal`, so its `PhaseLosses` is zero on both sides). The staleness half is proved live:
>   a deliberate eleventh widening of `r4133-indmach-injection-ulp` was reported STALE by
>   `Scope::dead_channels` on an unfiltered run, then reverted.
> * **No mode, no capability gap.** All six r4133 modes already existed and were proven `Served` by
>   G1.0, so `WP_G1_MODES` stays **97** and `crates/dss-epri/src/modes.rs` is byte-untouched.
>   Measured on a 0-phase element (`UPFCControl`, `controls/upfc/upfc_dual.dss`): `CktElementV(6)`
>   returns a 0-length array and the worker survives, capi returns `[]` — no capture predicate, no
>   sentinel normalization and **no `DoNotCall` row** is owed.
> * **Adjacent defect A-1 fixed here; A-2 recorded.** The reliability sweep's
>   `pSection.OCPDeviceType` was the same registration latch where Pascal calls
>   `GetOCPDeviceType` live (r4133 `Meters/EnergyMeter.pas:2538`); it now calls the shared live
>   scan (`solution/meters/reliability.rs::live_ocp_device_type`), measured at **zero** golden and
>   corpus movement and pinned by
>   `section_device_type_is_the_live_ocp_scan_not_the_registration_latch`. **A-2** — the port never
>   clears `HAS_OCP_DEVICE`/`HAS_AUTO_OCP_DEVICE` where r4133 clears them at the head of every
>   control `RecalcElementData` (`Controls/Relay.pas:946-964`) — is deliberately untouched (its
>   blast radius is the whole reliability sweep) and is recorded in STATUS as owned by G1.6/G1.6b.

### G1.4 — bus surface: pu-voltages, seq voltages, distances, extras

`Bus.puVmagAngle`/`puVoltages`/`AllBusVmagPu`, `Bus.SeqVoltages`/`CplxSeqVoltages`,
`Bus.VLL`/`puVLL`, `Bus.VMagAngle`, `AllPCEatBus`/`AllPDEatBus`, `Bus.Distance`,
`AllBusDistances`, `AllNodeDistances` (meter-zone distances — cases with meters
only; manifest-flagged).

> **As executed (2026-09-04, lane `lane-b`).** G1.4a **STOPPED at spec
> time** and was re-scoped by coordinator decision **D8**: three measured
> triggers put the surface over the §1.1(f) kill threshold at once — r4133's
> `-1` sentinel on `SeqVoltages`/`CplxSeqVoltages` for every `NumNodes != 3`
> bus (18 cases / 138 buses, 10 after the `large*` force guard: the
> threshold exactly), an r4133 **hang** in `BUSV(11)`/`BUSV(12)` on the two
> NEV decks (the unbounded `jj>3 ⇒ jj:=1` pairing loop,
> `DDLL/DBus.pas:549-602`, bounded to three tries in capi), and an unsettled
> third question (a ≥3-node bus with no node 1/2/3). So G1.4a lands the four
> **divergence-free** quantities (`puVoltages`, `puVmagAngle`, `VMagAngle`,
> `AllBusVmagPu`, plus `Nodes`/`kVBase`) and the shared per-bus capture
> struct + comparator G1.5 and G1.6(ii) reuse; the sequence quantities and
> `VLL`/`puVLL` move to a new sub-step **G1.4c** (one structural
> normalization + pins, never per-case rows; a state-dependent do-not-call
> guard in `crates/dss-epri` for the hang), chain order G1.4a → G1.5 → G1.4c
> → G1.4b. Two settlements were confirmed mid-execution as **D11**: (1) the
> first EXACT float compare the gate has ever run (`Bus.kVBase`) reddened 4
> of the then 523 cases at 1 ULP with the port and **both** transports agreeing
> bit-for-bit — the wire was wrong, so the workspace `Cargo.toml` gains
> `serde_json`'s `float_roundtrip` (a strict strengthening: no band, zero
> golden bytes, and every earlier floor can only re-measure tighter); (2) on
> a case whose `voltages` field is ledger-excluded the three continuous
> per-bus arrays are suppressed — 0 new rows instead of 10 for one
> already-pinned cause — while bus count, name sequence, `nodes`, `kv_base`
> and lengths stay compared, printed by the gate next to the entry that
> caused it. Two predictions of this plan are corrected:
> `population.lock.json` does **not** move *for the bus surface*
> (`population_lock.rs::rigor` fingerprints the *manifest* flag and no case sets
> `compare_bus` — the guard is the pinned `FORCED_BUS_POPULATION` + its
> re-derivation test; the lock does move for D12/D14 below), and
> the WP-G1 mode table is **98** rows on this lane, not the 96 the G1.0 note
> above records (`Bus.Nodes` `BUSV(2)` and `Circuit.AllBusNames`
> `CircuitV(7)` were ported here under "port gaps immediately"). The bus surface
> itself adds **0** ledger entries and **0** golden bytes. Two further settlements
> landed inside the sub-step. **D12/D14:** the same exact `kv_base` compare caught
> the pinned capi 0.14.5 oracle disagreeing with *itself* across fresh processes on
> every deck that instantiates a `GICTransformer` (7 bad runs of 60 with one, 0 of
> 40 without; r4133 80/80 bit-identical; root cause = `SetVoltageBases`' zero-load
> snapshot reading un-zeroed `NodeV`), so those four decks gate on `r4133` alone,
> `makeposseq_shunt`'s GICTransformer moved into the new r4133-gated micro deck
> `modes/makeposseq/makeposseq_gic.dss` (flipping the shunt deck itself would have
> quantized its own coverage through r4133's 5-significant-digit `MakePosSequence`
> round trip), the four `gic-…-capi{,-props}` entries are deleted (57 → **53**) and
> a manifest guard test forbids a capi-gated GICTransformer deck; the corpus is
> **524** cases / 520 live. (On `update` after the merge the ledger is **54** entries
> — 58 on the merged base minus those four — and `WP_G1_MODES` **102**; see the record's
> merge note.) **D13:** the r4133 worker no longer inherits or persists
> `DefaultBaseFreq` through `HKCU\Software\OpenDSS\MainSect` (its own bridge
> commit) — that machine-wide channel, not the scheduler, was the "21 red, all
> `R4133`" parity run, and the operating rule ("one gate or probe per worktree at a
> time") is in `TESTING.md`. Full record:
> `docs/phase-records/golden-rebase.md` §"WP-G1 — records".

> **As executed — G1.4c (2026-09-05, lane `lane-b`, D7).** Landed whole, on both
> channels, riding G1.4a's per-bus capture, comparator call site and flag: **no**
> new manifest flag, **no** new force rule and **no** `population.lock.json` move
> (regenerated in-step, `git diff` empty). D8's diagnosis held; its *shape* was
> strengthened by **D21**. Six things this section did not say:
> **(1)** The port's own semantics are settled and are neither oracle's:
> **S-SEQ** — `SeqVoltages`/`CplxSeqVoltages` exist iff the bus carries nodes 1,
> 2 and 3 (r4133's stated intent, `DDLL/DBus.pas:299`, against its own node-*count*
> test at `:298` and capi's `Nvalues > 3` clamp at `CAPI_Alt.pas:2172-2186`,
> both of which then substitute **ground** for an absent phase, `DBus.pas:305` ==
> `CAPI_Alt.pas:2190`) — and **S-VLL** — `VLL`/`puVLL` over the phase nodes
> actually present, which is what r4133's own report path computes
> (`Common/ShowResults.pas:193-194`). This settles D8's trigger 3 in the same
> shape for both quantity families.
> **(2)** A **third** upstream defect, shared by both oracles and unseen by the
> STOP note: the L-L pairing loop polls `FindIdx(jj)` *before* the
> `jj > 3 ⇒ jj := 1` wrap (`DBus.pas:575-584` == `CAPI_Alt.pas:2500-2523`), so a
> `[1,2,3,4]` bus pairs phase 3 with node **4** and a `[1,10]` bus pairs node 1
> with **itself** — while the commented-out original right below the loop
> (`DBus.pas:586-587`) and `ShowResults` both wrap first. Three
> `investigations/to_opendss/` reports were written, not the two planned (64, 65,
> 66).
> **(3)** The divergent buses are **not** excluded, as D8 proposed, but closed
> with a *positive* assertion of the upstream walk over the port's own state
> (`oracle == upstream_walk(port)`, the D15/D16 shape) — strictly stronger, and
> no bus is left unwitnessed: **0** ledger rows (53 unchanged), and the ledger-free
> seeding report over both channels on all 521 live cases attributes **0** of its
> 200 non-matches to this surface.
> **(4)** The r4133 hang is refused by a **state-dependent** register in the
> bridge (`modes::bus_vll_would_hang` + `STATE_DEPENDENT_REFUSALS`, disjoint from
> `DO_NOT_CALL`; the one dispatcher `Engine::bus_vll_pair` re-reads `Bus.Nodes`
> itself), cross-checked against the harness' independent replay of the same loop.
> Measured: TIMEOUT at 20.011 s on `NEVTestCase` `double-1` vs 0.000 s on
> `13kvbus`; bridge cost +2.94–3.54 µs/bus (≤ 0.74 s per full run). This is the
> **D2** mode-capability record for `BUSV(11)`/`BUSV(12)` — served, refused per
> bus, never silently capi-only (`TESTING.md` §"The r4133 bridge").
> **(5)** Four run-wide populations fail on stale in both directions and are
> printed as `corpus_gate seq/vll:` — `R4133_SEQ_SENTINEL_POPULATION` **(10, 129)**,
> `SEQ_GROUND_SUBSTITUTION_POPULATION` **(4, 54)**,
> `VLL_UPSTREAM_PAIRING_DECLINES` **(16, 196)**, `R4133_VLL_HANG_POPULATION`
> **(2, 12)**; the offline predictions were wrong for the first and third and the
> live measurement ruled (`modes:makeposseq/makeposseq_gic.dss` reaches none of
> the classes — `makeposseq` leaves its buses single-node).
> **(6)** One tolerance constant is added, `C_012 = 5.30e-10` (D21's shared
> constant with G1.3b): the analytic ceiling `2·Δsin60/3 = 5.229591574599605e-10`
> rounded up, live worst `5.229587392548124e-10` over 390 decks / 13 830
> three-node buses = 0.9999992 of the ceiling. **0** golden bytes;
> `lane_diff` **PASS**, max |Δ| = 0. Full record:
> `docs/phase-records/golden-rebase.md` §"WP-G1 — records".

> **As executed — G1.4b (2026-09-05, lane `lane-b`, D7).** The sub-step **STOPPED at
> spec time** on its at-bus half and was split by coordinator decision **D26**: with the
> brief's criterion (the port implements r4133's terminal-1/2 *name* test) the
> `capi_v0145` channel still diverges on **60** capi-gating cases over **three**
> mechanisms — 3rd-terminal inclusions, disabled elements dropped, and stale node refs
> after `Reduce` — six times the §1.1(f) budget, and the third is not a function of the
> port's state at all. r4133's own header (*"all PDE connected to the bus"*,
> `Common/Circuit.pas:1490-1492`) contradicts its behaviour, so under the D4 chain the
> port must compute a criterion that is **neither** oracle's. G1.4b therefore lands the
> **distances** only; the at-bus lists become **G1.4d** (below).
>
> Distances as executed: `Bus.Distance`, `AllBusDistances` and `AllNodeDistances` are
> three views of the ONE zone-build field `TDSSBus.DistFromMeter`, published **by
> reference** by `exec/view.rs` and compared by a new sibling of `compare_bus`,
> `harness::compare_bus_distances`, on the same per-bus walk — **no** new manifest flag,
> **no** new force rule, **no** `population.lock.json` flag move (the one cell that moved
> is a `ledger=` digest), **0** golden bytes. Floor: **`rel = abs = 0`**, no `Tolerances`
> argument at all — the two oracles are bit-identical to each other and to the port
> (measured on 10 decks incl. a `units=miles` one, which also settles that the shipped
> r4133 build uses `Shared/LineUnits.pas:81`'s `1609.344`), derived in
> `tests/TOLERANCE_NOTES.md` §"Bus distance surface". The comparator asserts lengths,
> the port-internal identity `AllBusDistances[i] == Bus.Distance == AllNodeDistances[k]`,
> the **oracle**-internal form of the same (which is what pins that the oracle's array is
> in `BusList` order), the values exactly, and "the port invents no distance".
>
> Four things this section did not say:
> **(1)** **D29 was executed step 1 first and came back dirty for an unpredicted reason.**
> Re-gating `modes:reduce/midi_reduce.dss` on r4133 fails on `Line.l2a~l2b`'s YPrim by
> exactly ×2, because r4133's `TLineObj.MergeWith` renames the surviving object in place
> (`Version8/Source/PDElements/Line.pas:1684`) and never updates
> `TDSSCircuit.DeviceList`: `SetElementActive` (`Common/Circuit.pas:2195-2214`) finds
> nothing, leaves `ActiveCktElement` where it was, and the DDLL silently captures
> **another element** (measured cursor-by-cursor; capi 0.14.5 does not share it; blast
> radius on today's corpus is zero because no `both`/`r4133`-gated deck renames an
> element). Re-gating would have ledgered a mis-addressed capture, so the settlement is
> D29 **branch 3**: the deck stays `capi_v0145`-gated and the one real divergence — capi
> loses the merged lines' `LengthUnits` and consumes `4 kft` as `4 km`, so `l2e` reads
> `5.524` against the port's and r4133's `2.7432` — is excluded bus-by-bus through a new
> **`distance`** ledger field (exclusion-only, **per-VALUE**, keyed by BUS name, consulted
> only *after* the exact equality has already failed so a hit means *masked a real
> divergence*) by the single entry `reduce-merge-units-lost-midi-capi-distance`
> (`cause_ref: line-merge-length-units-reset`, three `name_re` scopes), pinned by
> `the_reduced_midi_deck_reports_the_merged_lines_kft_distances`. Ledger 54 → **55**.
> `investigations/to_opendss/68-mergewith-rename-leaves-devicelist-stale.md` reports it upstream.
> **(2)** The surface's fail-on-stale is a **run-wide** pair, not a case count:
> `DISTANCE_POPULATION = (867, 79_137)` — gating compares carrying at least one non-zero
> `DistFromMeter`, and the buses that carried one — re-derived on every run, asserted in
> both directions and printed as `corpus_gate distance:`. It exists because the
> comparator is an equality over a field that is `0.0` on the ~370 meterless cases:
> without it a regression that stopped the zone walk writing distances would leave every
> one of them green. **The R part's "40 metered cases" was an undercount** — its probe
> reached only 369 of the 442 cases it meant to cover; a static scan that follows
> `Redirect`/`Compile` over the **443** forced (`compare_bus`) cases finds **70** that
> instantiate an EnergyMeter. Neither number gates anything; the run-wide pair does.
> **(3)** The surface is the live observable of **D9**'s `MakeBusList` fix: its two decks
> report all-zero distances without it, so the comparison would be green over nothing.
> `the_make_bus_list_decks_report_the_zone_distances_both_oracles_measure` pins all 20 of
> their distance literals (9 of them non-zero) against both oracles.
> **(4)** Non-vacuity was driven three ways in a scratch copy (never committed), each red
> on **both** channels: a perturbed distance (`+1e-9` on non-zero buses) reds
> `[CapiV0145]` and `[R4133]`; a distance invented where the zone walk wrote none reds
> both on meterless decks; and a node array one entry short reds the length rails on both.
> **0** golden bytes; gate **526/526 in both lanes**, ledger 55 entries / 1567 hits;
> `lane_diff` owed for `exec/view.rs`, expected max |Δ| = 0. Full record:
> `docs/phase-records/golden-rebase.md` §"WP-G1 — records".

> **G1.4d — the at-bus lists `AllPCEatBus` / `AllPDEatBus`** (new sub-step, split out of
> G1.4b by **D26** on 2026-09-05; tier `opus-xhigh`; bus chain order
> G1.4a → G1.5 → G1.4c → G1.4b → **G1.4d**). Criterion **S4** — the port computes the
> physically correct answer, which is neither oracle's: a PD-class element with **any**
> terminal at the bus under the `bus1 <> bus2` shunt filter; a PC-class element (plus
> Capacitor/Reactor, Vsource/Isource, Fault, per r4133's own class sets) with terminal 1
> at the bus; disabled elements **included**. Each channel is closed by a *positive*
> mechanism assertion over the port's state in the D15/D16/D21 shape — r4133's list ==
> the port's entries whose terminal 1 or 2 is at the bus; capi's == the capi fast-path
> walk of the same state — with four fail-on-stale populations and **0** ledger rows.
> Class C (capi naming a *disabled* element at a foreign bus through stale node refs, an
> artifact that is not a function of port state) is closed two-sidedly with its own
> population `CAPI_STALE_NODEREF_ADDS = (8, 11)` and a both-numbers pin, never an
> exclusion. The `ModeEffect` correction (`crates/dss-epri/src/modes.rs`: `BUSV(18)`/`(19)`
> are Impure — `DSSClass.pas:342-371`) lands there in its own commit ahead of the surface,
> and three `investigations/to_opendss/` reports at the next free numbers. Brief:
> `tmp/g14d/brief.md`. **This split is a plan amendment the user has not seen yet.**

### G1.5 — short-circuit surface

`Bus.Zsc1/Zsc0/ZscMatrix/YscMatrix/Isc/Voc` on cases that ran a fault study (the
precomputed-state semantics of `Export Faultstudy` — the gate reads what the solve
populated, never re-runs the study). Manifest flag on the faultstudy-family cases.

> **As executed (2026-09-05, lane `lane-b`).** Landed whole, on both channels,
> for every live non-`large*` case, riding G1.4a's per-bus capture and
> comparator (`compare_zsc ⇒ compare_bus`, asserted three times — the request
> builder and a loud refusal in each transport — never written as an `||`).
> Four corrections and two additions to this section:
> **(1)** The surface carries a **third** ordering convention: the two matrices
> are row-major over the bus's *internal* (insertion) node index (r4133
> `DDLL/DBus.pas:445-450` == capi `CAPI_Alt.pas:2318-2330`), not the ascending
> node number of G1.4a's arrays, and `CMatrix` stores column-major — so the
> flatten is an explicit `(i, j)` walk with the convention pinned by two live
> witnesses (the micro deck below and `Run_NEV`) rather than by a comment.
> **(2)** The two channels publish **different** not-run sentinels (capi 1
> double, r4133 2) and a second, 0-node split on `Isc`/`Voc` (capi 0, r4133 2);
> both are shape differences of an empty quantity, so per **D4** they are
> comparator-level normalizations **per channel** with one pin — 0 ledger rows —
> and the one genuine collision (r4133 at a 1-node bus) is closed positively by
> asserting the oracle's pair is `CZero`. The comparator's first assertion is the
> discrete "study ran" bit, before any number.
> **(3)** There are **four** vendored decks that run a study, not three:
> `ieee37_SC_Currents.dss` spells it `solve mode=f`, which a `grep faultstudy`
> misses; it is the population worst on `Zsc0`/`ZscMatrix`/`YscMatrix`. All four
> are `kind: feeder`, so the sub-step also authors the corpus's only `micro`-band
> short-circuit deck, `modes/faultstudy/faultstudy_micro.dss` (a new sub-family),
> and — correcting this plan's standing prediction — `population.lock.json`
> **does** move here: one new case row, the only hand-set `compare_zsc` in the
> tree, so the lock fingerprints the surface (`zsc=1`) and a later narrowing is
> visible in it. The scheduler's `force_zsc` turns the surface on for every live
> non-`large*` case regardless (`FORCED_ZSC_POPULATION = (442, 311, 87, 44)` on the
> lane, re-derived, not trusted; **(443, 312, 87, 44)** on `update` after the merge,
> where G1.6(i)'s `midi_relcalc` deck joins this one — corpus 525 → **526** / 522 live —
> and every other `FORCED_*_POPULATION` moves with it).
> **(4)** A port gap found in spec and closed in-step under "port gaps
> immediately": `ReduceAlgs`' `kVBase <= 0` branch skipped Pascal's
> `Solution.UpdateVBus` (r4133 `Meters/ReduceAlgs.pas:500-508`, capi `:487-494`),
> which refreshes the very `VBus` array `Bus.Voc` publishes; the returned kV base
> is unchanged, the side effect is not.
> **Measured:** the whole forced population on both channels — worst **0.61** of
> the allowed band (`Voc`, `IEEE123Master-SC` bus `610`; 0.42 over the five
> impedance/current arms alone, `Zsc0` at `ieee37_SC_Currents` bus `775`), the two conditioning
> outliers (`IEEE123Master-SC:610` κ = 1.10e8, `Run_NEV:tertiary` κ = 9.52e6)
> **below** the predicted `κ·u·‖Ysc‖∞` floor, **0** new ledger entries, **0**
> golden bytes, **no** new tolerance constant (every band is an existing tier).
> The R-5 "singular bus" arm never occurred: no non-finite entry in any
> short-circuit comparison of the run. Full record:
> `docs/phase-records/golden-rebase.md` §"WP-G1 — records".

### G1.6 — meter extras + per-bus reliability

**Ordering: G2.2a lands first (it moves `Bus.Int_Duration` on multi-meter decks);
if that order is ever violated, re-run this sub-step's ledger triage in the G2.2a
commit.** `CalcCurrent`, `AllocFactors`, `Totals`, reliability indices
(`SAIFI/SAIFIKW/SAIDI/CustInterrupts` — cases whose deck ran
`CalcReliabilityIndices`; CAIDI only if the r4133 DLL exposes it, measured in
G1.11c, else documented), active-section fields (fastdss captures the first
section only — parity = at least the first section), the zone lists upgraded from
set-compare to **ordered** name vectors, and the **per-bus** reliability surface
`Bus.Lambda`, `Bus.N_interrupts`, `Bus.N_Customers`, `Bus.Cust_Interrupts`,
`Bus.Cust_Duration`, `Bus.Int_Duration`, `Bus.TotalMiles`, `Bus.SectionID`
(`IBus._columns` on `origin/fastdss`) — the columns `Export BusReliability`
renders.

> **As executed — part (i) (2026-09-05, lane `lane-m`; decisions D7, D17a, D18).** The sub-step is
> split: **(i)** the meter extras + the run protocol (here); **(ii)** the eight per-bus reliability
> columns + `Bus.Int_Duration` (next on this lane). Part (i) is the only WP-G1 sub-step that changes
> **how a case is run**: no live corpus deck ran `CalcReliabilityIndices` (both occurrences are
> comments in `expect_solve_abort` decks), so the whole reliability half would have compared
> `0 == 0`. Three decisions carry it. **D-i-1** — the executive `RelCalc` is driven **once** per
> case, on the last step, after the error assert and the `Text.Result` read (the command overwrites
> it) and before every capture of that checkpoint, on all three engines: it is **not idempotent**
> (a second run re-accumulates `Bus.TotalMiles`, measured `13.825757575757578 →
> 22.348484848484844` on both oracles), so the payload lives on the last checkpoint only and the
> runner asserts exactly that. **D-i-2** — the flag is **manifest-set, never forced**: "has an
> EnergyMeter" is not a manifest field and forcing it would fire `28724 No EnergyMeter Objects
> Defined` on ~340 meterless decks, so no `FORCED_RELIABILITY_POPULATION` pin is owed; the four
> `DOCTechNote` `large*` decks stay out under the same cost guard `force_properties` /
> `force_pdelements` already apply, and the multi-meter `Bus_Int_Duration` divergence therefore
> keeps its existing witness (the `export_busreliability_multimeter` golden + its G2.2a pin) and
> gains no live one. **D-i-3** — `Meters.CalcCurrent` / `AllocFactors` are an **uninitialised read
> in both oracles** on any deck that never ran `AllocateLoads` (`MeterElement.pas:45-52` ReallocMems
> without zeroing; only `CalcAllocationFactors` `:54-72` writes them, driven solely by
> `ExecHelper.pas:2624-2683`), nondeterministic across processes and therefore un-envelopable: a
> field-scoped `RELIABILITY_SKIP_FIELDS` exclusion with a pin, not a ledger row — and **not a
> permanent hole**, because the sub-step adds the corpus's only `AllocateLoads` deck
> (`tests/corpus/controls/energymeter/midi_relcalc.dss`, `both`, three sections) where both fields
> are compared live on both channels. Population **6 cases** (4 capi-gating / 5 r4133-gating,
> including the 52902 abort witness); corpus **524 → 525** cases / 521 live at the merge into
> `update` (523 → 524 on the lane itself, before G1.4a's D12/D14 corpus flips arrived), and the
> `FORCED_*_POPULATION` locks move `(441, 310, 87, 44) → (442, 311, 87, 44)` with it —
> `FORCED_TOPOLOGY_POPULATION` (G1.7, merged first) included.
> Compared **exactly** (`rel = abs = 0`) — the two oracle engines return bit-identical doubles for
> the whole payload — with three banded cells derived from existing tiers, never new ones:
> `Meters.Totals` on the **energy** tier (D17a: it is `Σ registers·Mask`, not a reliability number)
> and `calc_current` / `alloc_factors` on the **current** tier and its image under
> `SensorCurrent/|I|`. **0 ledger entries** (budget 10). Deliberately **not** re-read from
> `IMeters._columns`: `SeqListSize` / `CountBranches` / `CountEndElements` are the lengths of the
> three zone lists we compare in full, and `MeteredElement` / `MeteredTerminal` / `Peakcurrent` are
> EnergyMeter properties #0/#1/#6 already live-compared by `compare_all_properties` — as is
> **CAIDI** (property #22), which the plan's row 3 said would be "documented as not-comparable";
> `CountEndElements` is additionally a **do-not-call** on the r4133 arm (`DMeters.pas:157-163`
> dereferences `pMeter.BranchList.ZoneEndsList` with no `BranchList` guard, where capi guards with
> `CheckBranchList(5500)`), recorded in TESTING.md's bridge section. `WP_G1_MODES` **102 → 103**
> at the merge (**99 → 100** on the lane; the
> one selector `MetersI(22)` `Meters.SetActiveSection`, the table's single declared non-getter).
> One honest deviation from the spec's test recipe: its `Meters.Totals` non-vacuity corruption
> (drop the `TotalsMask` multiply) is **vacuous on this population** — no flagged deck sets `mask=`
> — so the mask arm is pinned in-engine (`meter_totals_is_the_masked_register_sum`) and the
> gate-level demo is a value drive instead. Exact float compares on this surface (and on G1.6b's)
> depend on `serde_json`'s `float_roundtrip`: decision **D11/D18**, committed on this lane.
> Full record: `docs/phase-records/golden-rebase.md` §"GOLDEN_REBASE WP-G1 — records".

### G1.6b — PDElements interface

`AccumulatedL`, `ParentPDElement`, `FromTerminal`, `IsShunt`, `Numcustomers`,
`SectionID`, `RepairTime`, `Totalcustomers`, `Lambda` — the per-PD-element walk
fastdss compares wholesale.

> **As executed (2026-09-04, lane `lane-m`, decisions D7 + D9).** The surface above is
> **incomplete**: `IPDElements._columns` on `DSS-Python@origin/fastdss` carries **13**
> columns — the nine listed plus `FaultRate`, `TotalMiles` (= `AccumulatedMilesDownStream`,
> *not* `Bus.TotalMiles`) and `pctPermanent` — and the sub-step landed all 13 plus
> `parent_name`, on **both** channels, compared **exactly** (`rel = abs = 0`; every value is
> a class default, a deck literal or an untouched `0.0`, so the derivation went to
> `tests/TOLERANCE_NOTES.md` and no floor was added). **0** ledger entries: the only
> measured divergence is an uninitialized read in *both* oracles on in-zone shunt
> Capacitors/Reactors (`EnergyMeter.pas` assigns through `pPCelem: TPCElement` — r4133
> `:1868-1869`, capi `:1927-1929`), nondeterministic and therefore un-envelopable; it is
> excluded per (channel, class, field) **and per element** in `harness::PD_SKIP_FIELDS` (4 capi
> cells `fault_rate`/`pct_permanent`, 4 r4133 cells `lambda`/`accumulated_l`, consulted only where
> the write lands — an in-zone shunt member, `pd_skip_applies`, narrowed by the audit settlement —
> each still visited and hit-accounted) and pinned by
> `pd_elements_shunt_reliability_inputs_survive_the_meter_zone` and
> `pd_elements_shunt_branch_flt_rate_survives_the_meter_zone`. `WP_G1_MODES` **96 → 99**
> (`PDElementsI:1`/`:2`, `PDElementsS:0`) and `EXCLUDED_WRITE_MODES` **2 → 3**. Two
> deviations from the plan's letter, both recorded: `ParentPDElement` is read **last** per
> element (it re-points `ActiveCktElement` at the parent and never restores it), not in the
> fastdss column order; and the sub-step carries a **separate engine-fix commit** (decision
> D9) restoring `DoResetMeterZones` to `ReProcessBusDefs`' tail (r4133 `Circuit.pas:2411`),
> without which `MakeBusList` left every EnergyMeter with an empty zone. `SectionID`,
> `TotalMiles`, `Lambda` and `AccumulatedL` are wired and compared but **0 on every live
> case** at the time (no deck ran `RelCalc`); **G1.6(i) discharged both obligations on
> 2026-09-05** — the four fields are live and oracle-compared on three cases (pin
> `pd_elements_relcalc_fields_are_live_after_relcalc`), and the exactness note is re-derived
> affirmatively at `tests/TOLERANCE_NOTES.md:1011`: the twelve 1-ULP cells the live gate first
> reported were the gate's own JSON decoder (D11/D18 — the wire tokens round-trip bit for bit,
> 15/15), so no floor was added and `compare_pd_elements` keeps `rel = abs = 0`. Full record:
> `docs/phase-records/golden-rebase.md` §"GOLDEN_REBASE WP-G1 — records".

### G1.7 — topology interface

`NumLoops`, `NumIsolatedBranches`, `NumIsolatedLoads`, `AllLoopedPairs`,
`AllIsolatedBranches/Loads` (`ITopology._columns` on `origin/fastdss` lists all of
these, so this sub-step is straight parity, not a bonus; `ActiveLevel`/
`BranchName`/`ActiveBranch` are iteration cursors and are deliberately not
compared).

> **As executed (2026-09-04/05, lane `lane-s`).** The six rows landed on both channels in one
> commit: flag `compare_topology` flipped to `wired: true` and **forced on every live
> non-`large` case** — on the lane 440 = 313 `both` / 83 `r4133` / 44 `capi_v0145`
> (`scheduler::FORCED_TOPOLOGY_POPULATION`, re-derived each run and asserted equal to the
> property surface's population) — with seven decks also *declaring* the flag, because
> `population.lock.json` fingerprints the manifest flag and cannot see scheduler-side forcing
> (`TOPOLOGY_DECLARED_IN_MANIFEST`, one witness per gating channel). The lock moved by exactly
> seven `topo=0 → topo=1` tokens; **0** golden bytes, **0** ledger entries, 0 new
> `LEDGER_FIELDS`, and **no floor** — the surface is fully discrete (counts and identifier
> lists), which `tests/TOLERANCE_NOTES.md` now records as deliberate. **No FFI was added**:
> G1.0 had already bound and classified the six `TopologyI`/`TopologyV` modes `Served`.
> Four corrections to the text above. (1) **Not "iteration cursors".** `ActiveLevel`,
> `BranchName` and `ActiveBranch` are excluded for a stronger reason: each *reassigns*
> `ActiveCircuit.ActiveCktElement` (r4133 `DDLL/DTopology.pas:29-54`, `:96-160`, `:170-186`;
> capi `CAPI/CAPI_Topology.pas:98-110`) and would poison the per-element capture of the same
> step — the brief's B16 gap, now a source-text rail rather than a comment: six new cases in
> `crates/dss-core/tests/capture_order.rs` assert that the surface is read **strictly last**
> (the first `Topology` read builds the memoized tree and rewrites
> `Checked`/`IsIsolated`/`BusChecked`, r4133 `Common/Circuit.pas:2932-2950`) and that exactly
> six of `ITopology`'s eighteen members are touched on either transport — the bridge binds no
> other. (2) **Two shape normalizations**, transport-side and measured corpus-wide: the empty
> sentinel `["NONE"] → []` (r4133 pre-seeds `TStr[0] := 'NONE'`, `DTopology.pas:271-275`; capi
> `DefaultResult(…, 'NONE')`, `CAPI_Utils.pas:115`) and capi's single trailing `''`
> (`CAPI_Topology.pas:126-132`; absent on r4133 and on `AllLoopedPairs`, `k := -1`). The
> comparator **asserts the fixpoint** instead of repeating the repair, so a transport that
> stops normalizing fails rather than comparing a phantom entry as empty. (3) **Two upstream
> defects are asserted, not excluded** (coordinator decisions D15/D16, 0 ledger rows): both
> oracles memoize `Branch_List` and never invalidate it on a conductor open/close, and both
> dedup the looped-pair buffer in overlapping windows (`i := i + 1`,
> `DTopology.pas:286-296`), dropping a genuinely new pair that coincides with a straddling
> window. The port does neither; where the port's fresh answer differs the comparator asserts
> the *mechanism* — `oracle(k) == port(step 0)` for the four isolation fields, and
> `oracle.looped_pairs == window_dedup(port candidates)` for the pair list — with two
> fail-on-stale populations, `TOPOLOGY_STALE_DECLINES = (16 cases, 135 case-steps)` and
> `LOOPED_PAIR_WINDOW_DECLINES = (8, 96)`, over 3 314 compared (case, step, channel) triples
> on the lane (3 312 after the merge into `update`, both constants unmoved;
> `FORCED_TOPOLOGY_POPULATION` re-derived there to `(441, 310, 87, 44)` for G1.4a's D12/D14
> corpus flips).
> (4) **Two PORT gaps the new surface exposed were fixed in-part**, both inside the
> surface commit: topology adjacency routed by `TPDElement.IsShunt` instead of Pascal's
> class-switched `IsShuntElement` (capi `Shared/CktTree.pas:522-528`, r4133
> `Common/Utilities.pas:1262-1274`), which hid every `GICTransformer` loop; and
> `CktElementData::set_nconds` forcing a terminal reallocation that r4133's own guard
> (`Common/CktElement.pas:349-361`, `:386`) would have skipped, which unwired every terminal
> on a no-op `Phases=` re-set — a second `MakePosSequence` left the whole model
> bus-unresolved. Coordinator decisions applied: **D2** (no bridge change owed), **D3**
> (capture order), **D4** (defect ⇒ the port computes the correct value), **D7** (lane
> `lane-s`), **D15**, **D16**.
> **Tier as executed:** §0's `opus-high+` row held for the capture, comparator, flag and
> docs parts; the two settlement parts (the D15 isolation rebase and the D16 window model,
> with the engine-side candidate sequence) ran at `opus-xhigh`, as the sub-step brief
> foresaw for "the memoization question".

### G1.8 — incidence/Laplacian

Run `CalcIncMatrix`/`CalcLaplacian` on flagged cases; compare
`Solution.IncMatrix/IncMatrixCols/IncMatrixRows/Laplacian` as exact integer/index
vectors (discrete → zero tolerance).

### G1.9 — circuit aggregates + solution scalars

`Circuit.TotalPower`, `Losses`, `LineLosses`, `SubstationLosses`,
`AllElementLosses` — the aggregation code paths the reports use, compared as
numbers on every case (cheap, universal). Plus the Solution scalars
`ControlIterations`, `Totaliterations`, `MostIterationsDone`,
`ControlActionsDone`, `SystemYChanged`, `Seconds`/`LoadMult`/`Year`/`Hour`/`Mode`
(discrete/counter → exact; the iteration-count lane policy of `lane.rs` applies
where it already exists).

> **As executed (2026-09-04, lane `lane-s`).** Both channels wired in one commit,
> **unflagged and universal** as G1.0 decided — no `G1_SURFACE_FLAGS` row, no rigor
> token, **no `population.lock.json` regen**, 0 golden bytes, **0** ledger entries and
> 0 new `LEDGER_FIELDS`. Five things were settled in-part and are not what this text
> assumed. (1) `Totaliterations` is *literally* `Solution.Iteration`
> (`DDLL/DSolution.pas:218-220`), so it is captured and asserted as an alias — in
> engine and live on every checkpoint — instead of being oracle-compared twice; the
> same treatment for `Circuit.YCurrents` (`DDLL/DCircuit.pas:777-787` = the
> already-compared `injection` vector), which is therefore not built at all.
> (2) The dossier's cancellation model for `Circuit.Losses` is **refuted by
> measurement** — `Σ|term| / |Σ term|` is 1.000…1.5036 corpus-wide (worst ckt24), the
> summands being each element's own same-signed loss — so no floor was written for it
> and the kill criterion ("looser than 1e-4 rel on a feeder case") is answered by the
> measured feeder maximum `2.719409449622587e-08`. (3) The value arms **inherit** the
> ledger instead of re-pinning it: an aggregate is a linear functional of per-element
> quantities the ledger already partitions, and pinning that echo would have cost ~14
> rows (kill criterion) for divergences already owned — so they consume the runner's
> `LedgerView::element_rewrites`, while the membership/identity arms stay on the raw
> oracle capture on every case. (4) `MostIterationsDone` is per-**step**, not per-run
> (`Common/Solution.pas:2568` zeroes it in `SnapShotInit`), which is what makes it
> comparable checkpoint-by-checkpoint. (5) Five `modes.rs` `Circuit` rows moved
> `ModeEffect::Pure` → `Impure` (each walks a `TPointerList` to exhaustion and calls
> `ComputeIterminal` on what it walks) — `CIRCUIT_LOSSES` moves the very `PDElements`
> cursor G1.6b reads. Also settled: the two open measurements — `SystemYChanged` agrees
> after every solve on both engines (nothing to exclude, the Y-rebuild scheduling matches),
> and `SubstationLosses` already has corpus witnesses (`8500-Node`, `ckt5`, `ckt24`), so **no**
> synthetic deck and no population change was owed — and both transports gained the element
> capture's retry-once tolerance, the aggregates now being the first post-solve read to prime a
> user-model `DoSimpleMsg` (found and fixed in-part, not parked). Coordinator decisions applied:
> **D3** (the A/B/C capture order, asserted by `crates/dss-core/tests/capture_order.rs`),
> **D4** (r4133's `0|1` `SystemYChanged`/`ControlActionsDone` normalized to `bool` at the bridge
> — a shape normalization plus one pin, never a ledger row) and **D7** (lane `lane-s`).
> Full record: `docs/phase-records/golden-rebase.md` §"GOLDEN_REBASE WP-G1 — records".

### G1.10 — run-file artifacts

**Every** CSV the deck emits under DataPath (fastdss archives and compares them
all — DI CSVs are the closedi subset; include the forced
`export profile phases=all` where flagged) through the existing `compare_export`
numeric comparator, manifest-flagged. `save circuit`: compare the emitted **file
set** (names) vs the oracle's and keep our round-trip gate as the content check
(byte-matching the oracle's Save is explicitly not a goal — `PHASE8_PLAN.md §2.4`;
fastdss itself never compares this surface, so this is strictly stronger).

### G1.11a — r4133 channel: CktElement families

Extend `crates/dss-epri/src/families.rs` + `capture.rs` with the CktElement reads
serving G1.3a–d (`CktElement_Get_SeqCurrents/SeqVoltages/SeqPowers/`
`CplxSeqCurrents/CplxSeqVoltages/Residuals/CurrentsMagAng/VoltagesMagAng/`
`TotalPowers`, the discrete extras). `#[cfg(windows)]`, module-level `// SAFETY`
docs, `#![deny(unsafe_op_in_unsafe_fn)]` — the crate's standing rules.
**Acceptance (all three G1.11 sub-steps):** for every symbol, either a working
capture proven on one gated r4133 case, or a recorded `GetProcAddress` miss with
the symbol name in TESTING.md — never a silent capi-only fallback.

*(**2026-09-04**, decision D2, landed by G1.0: the `GetProcAddress`-miss half is
superseded — the DLL has **no** `*_Get_*` symbols. It is the grouped DDLL API, one
entry point per (family, ABI shape) with the property selected by a mode index, so
the acceptance is the **mode probe**, and G1.0 executed it once for all of WP-G1:
all 96 modes these three sub-steps need classify `Served`, the expected-miss list is
empty (`crates/dss-epri/tests/modes.rs`). **2026-09-05:** the table is now **103**
rows — G1.6b added the three PDElements walk arms, G1.3a added
`CktElement.Enabled` (`CktElementI(12)`, `DCktElement.pas:263`), the safety
predicate its enabled-only polar capture needs, G1.4a added `Circuit.AllBusNames`
and `Bus.Nodes`, and G1.6(i) added `Meters.SetActiveSection`, the section cursor
its reliability capture drives; the probe was re-run at 103/103
`Served`, still zero misses. The `X_Get_Y` spellings below and above
name the *properties* to capture, not symbols to bind. See the WP-G1 preamble note
and TESTING.md §"The r4133 bridge — entry points, mode capability, do-not-call".)*

### G1.11b — r4133 channel: Bus families

Serving G1.4 + G1.5: `Bus_Get_puVmagAngle/SeqVoltages/CplxSeqVoltages/Distance/`
`Zsc1/Zsc0/ZscMatrix/YscMatrix/Isc/Voc`,
`Circuit_Get_AllBusDistances/AllNodeDistances/AllBusVmagPu`.

### G1.11c — r4133 channel: Meters/Topology/Solution/Circuit families

Serving G1.6–G1.9. A group the DLL cannot serve stays capi-only with a one-line note in
TESTING.md. (**2026-09-05, G1.6(i):** the CAIDI capability measurement is moot — CAIDI has
no API mode on either channel and is EnergyMeter property #22 of `AllPropertyNames`,
live-compared through `compare_all_properties` on both channels; see §G1.6 as-executed.)

---

## WP-G2 — Bug-kernel teardown (zero golden bytes move)

Repeat the `4f977d9e` procedure per row: delete the alias arm, make the engine
compute the correct value in both lanes, make the existing exclusion/pin
unconditional, adjust the census. **Every row has a deep-dive report** in the
local-only `investigations/` folder (`issue-NN-*.md`, indexed by
`investigations/TODO_COMPAT_REGISTRY.md` §3; English upstream-ready copies in
`investigations/to_opendss/`) — the implementation agent reads its row's report
before touching code, and each teardown updates the registry's «Судьба» entry
(local docs, gitignored — not part of the gate, but never skipped). Exceptions:
G2.4 (a dss-python wrapper artifact — no engine-bug report exists) and G2.6 (its
report does not exist yet and is written inside the sub-step). Bookkeeping
in every commit (all fail-on-stale):

- `SPLIT_ALIAS_POPULATION` (`oracle_parity_cfg_gate.rs:804`, 31 → **11** across the
  WP), `ESCAPE_REGISTER`/`EXIT_POPULATION`, `TAG_PATH_CITATIONS`, `ledger.json` +
  `population.lock.json` where a fix diverges from an oracle channel.
- **`TORN_DOWN_ROWS` register** (created empty in G2.0): every teardown commit adds
  its row **plus the greppable markers of G2.0(b)** at every new unconditional
  exclusion site and pin — every G2 sub-step, not only G2.1.
- **Doc-surface alias references, in the SAME commit as the alias deletion.**
  `oracle_parity_cfg_gate.rs::operational_docs_cite_the_compat_machinery_accurately`
  (`:1310`) has two independent failure modes over CLAUDE.md / TESTING.md /
  `tests/TOLERANCE_NOTES.md` / `tests/corpus/ledger.json` /
  `tests/corpus/**/manifest.json` / `tools/**/*.{md,py}`: (a) `bad_alias`
  (`:1396`, asserted `:1455`) — any `compat::<ident>` no compat module still
  declares; (b) the non-vacuity floor `alias_refs >= 4` (`:1478`). The 7 live
  references all name rows this WP deletes: CLAUDE.md:104 (IRESIDUAL), :111
  (BUS_INT_DURATION), :129 (POWERS_REUSE…), :147 (monitor_base_frequency);
  `tests/TOLERANCE_NOTES.md:643` (IRESIDUAL); `tools/golden/gen_props.py:2795`
  (ISOURCE_BUS2); `tests/corpus/modes/manifest.json:488` (POWERS_REUSE…, note
  text only — `population_lock.rs::Case::rigor` does not fingerprint `note`, so no
  lock field moves). Each sub-step strikes its own citations; G2.0 pre-seeds the
  floor. Precedent: `4f977d9e` struck CLAUDE.md/TOLERANCE_NOTES/gen_json.py
  sentences in the very commit that deleted the arms.
- **Pin-walk non-vacuity re-anchor**: `oracle_parity_cfg_gate.rs:1087-1108` anchors
  `every_lane_split_alias_is_pinned_by_an_expected_value_test` on
  `IRESIDUAL_FROM_TERMINAL_1` — G2.2a must re-point it at one of the five
  **numeric** survivors (`PI`, `round_f64`, `round_i32`, `kv_base_search_scale`,
  `profile_ll_pu_divisor` — they survive WP-G4 too) in the same commit, or the
  test goes red.

**Acceptance criterion for the whole WP:** `git diff --stat -- tests/golden` is
**empty**, `TORN_DOWN_ROWS` names every torn-down row with an existing pin, and
`oracle_parity_cfg_gate` is green at **every intermediate commit** (not just at
the WP's end). Expected final register counts for this WP: 20 `Kind::SplitAlias`
(31 → 11) + 3 `Kind::WholeCase` (4 → 1).

### G2.0 — WP-G2 rails (before any deletion)

(a) **Doc-citation re-anchor.** Add at least six `compat::<alias>` citation
**lines** naming **numeric survivors** — `compat::PI`, `compat::round_f64`,
`compat::round_i32`, `compat::kv_base_search_scale`,
`compat::profile_ll_pu_divisor` (five aliases; a line may repeat an alias — the
floor counts references, not distinct names; they survive WP-G4 too, so these
citations are written once) — to the walked doc surface: a new subsection of TESTING.md
("Precision-compat rows still split by lane") and/or the `TODO(compat)` convention
section of CLAUDE.md. Do **not** cite "the F-FMT section of
tests/TOLERANCE_NOTES.md" — no such section exists. Each added line must be a true
statement about a row that is still lane-split, and must **not** contain the
literal tag `TODO(compat)` together with a `*.rs` path on the same line (that
trips the `unregistered` assert at `:1441` unless a `TAG_PATH_CITATIONS` row is
added). Verify in the same commit:
`cargo test -p dss-core --test oracle_parity_cfg_gate` (both lanes).
(b) **`TORN_DOWN_ROWS` register.** Extend `oracle_parity_cfg_gate.rs` (NOT a new
test file — the needed helpers `repo_root`/`rust_sources`/`test_region`/
`names_token` are private to that binary, and the file already is the
compat-machinery register):
`const TORN_DOWN_ROWS: &[(&str /*former row name*/, Kind /*SplitAlias | WholeCase*/, Evidence /*Site(file, slice) | Ledger(key) | None*/, Option<(&str /*pin file*/, &str /*pin fn*/)> /*Some mandatory for every WP-G2 row*/)]`.
Checks: a `Some` pin's file exists and names the pin fn inside its test region;
an `Evidence::Site` still contains its distinctive slice; an `Evidence::Ledger`
key exists in `tests/corpus/ledger.json`; and two arithmetic ties —
`31 − count(Kind::SplitAlias) == SPLIT_ALIAS_POPULATION` and
`4 − count(Kind::WholeCase) == EXIT_POPULATION[WholeCase]` — so a row cannot
leave either census without landing here. Greppable markers make the both-ways
direction mechanizable: every unconditional exclusion site gains
`// LANE-EXCLUSION(<row>):` and every pin `// EXPECTED-VALUE-PIN(<row>):`, checked
marker↔row both ways exactly like `markers_in_tree`. The register is created
empty here; G2.5's three engine fixes join it carrying their ledger key instead of
a site.

### G2.1a–G2.1h — zero-footprint rows, one sub-step (full ritual) per row

Each row: delete the parity arm, single kernel in both lanes, the row's
expected-value pin becomes unconditional (the pin is the test the pin walk —
`oracle_parity_cfg_gate.rs:1013-1043` — associates with the alias today; copy its
`path::name` into the `TORN_DOWN_ROWS` row), `SPLIT_ALIAS_POPULATION` −1, marker
comments per G2.0(b). None of the eight is cited on the doc surface (measured —
the 7 live references all belong to G2.2/G2.3), so no doc edits. Zero golden
bytes move — these rows have no harness exclusion anywhere. (G2.1g caveat:
measure first whether any committed `cim/` golden observes the grounded flag —
the default-lane byte compare is green today, which says none should; if one
does, the row moves into G2.2c with a `lane_expected_cim` rule, and G3.4's
predicted-diff list gains it. **As executed: measured, and it did not fire** —
no committed `cim/` golden observes the flag, so the row stayed in G2.1g, the
transform kept exactly its two rewrites, and G3.4's list is unchanged. The
transform is `expected_cim` since G2.2c renamed it.)

- **G2.1a** `stddev_single_point` (`compat.rs:561`; issue-11)
- **G2.1b** `CAPCONTROL_MAKELIKE_DROPS_CONTROL_SIGNAL` (`:763`; issue-15)
- **G2.1c** `SEQ_CURRENTS_PRINTS_RAW_NONPOSITIVE_RATING` (`:801`; issue-12)
- **G2.1d** `REDUCE_SCANS_ONLY_THE_FIRST_PARENT_SHUNT` (`:832`; issue-31)
- **G2.1e** `STORAGE_CONTROLLER_IDLE_TEST_COMPLEMENTS_THE_ORDINAL` (`:863`; issue-18)
- **G2.1f** `STORAGE_MULTIFILE_USES_THE_PV_PREFIX` (`:885`; issue-20)
- **G2.1g** `CIM_WYE_GROUNDED_IS_HARDCODED_TRUE` (`:980`; issue-23)
- **G2.1h** `HEIGHT_UNIT_CHANGE_REREADS_THE_METRES_FIELD` (`:1129`; issue-28)

### G2.2a — existing-exclusion rows: numeric column exclusions

- `IRESIDUAL_FROM_TERMINAL_1` (`compat.rs:588`; issue-01; engine
  `report/export/seq_currents.rs:107-109`): the `ColTol` exclusion at
  `golden_reports.rs:2907` becomes unconditional; the derived pin at `:2953` stays
  (it derives truth from `Export Currents`' `Iresid_j` — an independent anchor).
  Move `tests/TOLERANCE_NOTES.md:642-653` (**as executed:** rewritten in place —
  see the G5.1 doc list for why); strike CLAUDE.md:104 and
  TOLERANCE_NOTES.md:643 in the same commit; **re-anchor the pin-walk non-vacuity
  const (`oracle_parity_cfg_gate.rs:1087-1108`) onto one of the five numeric
  survivors (the G2.0(a) list).**
- `BUS_INT_DURATION_WALKS_ALL_BUSES` (`:615`; issue-02): `duration` column mask
  (`golden_reports.rs:4085-4094`) unconditional; literal pin at `:4125-4168` stays;
  strike CLAUDE.md:111. (This sub-step precedes G1.6 — §0 ordering.)

### G2.2b — existing-exclusion rows: property exclusions

- `monitor_base_frequency` (`:653`; issue-06): drop the `!lane::PARITY` guard in `skip_prop`
  (`harness/mod.rs:1326-1332`); surviving pin
  `monitor_basefreq_is_the_lane_kernel` loses its lane branch; strike
  CLAUDE.md:147.
- `ISOURCE_BUS2_NEVER_LATCHES` (`:743`; issue-14): `LANE_SKIP_SCENARIO_PROPS`
  (`props_roundtrip.rs:133`) becomes unconditional by dropping the
  `!ORACLE_PARITY` guard at `:246-252` (the guard is NOT at `:133` — that line is
  the data); its stale-entry guard (`:216-225`) stays; surviving pin
  `elements::pc::isource::tests::bus2_latching_is_the_lane_kernel`; strike
  `tools/golden/gen_props.py:2795`. **`LANE_SKIP_PROP_VALUE_CELLS = 33`
  (`props_roundtrip.rs:174`, asserted `:273-279`) is the unrelated sym-matrix
  exclusion, already both-lane — it must NOT move; if it does, that is a finding,
  not a re-measurement.**

### G2.2c — existing-exclusion rows: text transforms

- `FAULT_DUMP_TAIL_REPRINTS_MINAMPS` (`:1077`; issue-13): positional drop
  (`golden_reports.rs:5610-5626`) unconditional; non-vacuity assert stays.
- `CIM_DELTA_SHUNT_GROUNDED_USES_LINEAR_PREFIX` + `CIM_ACLINESEGMENT_G0CH_WRITTEN_AS_B0CH`
  (`:909`, `:936`; issue-24, issue-25): `golden_cim.rs:64-92 lane_expected_cim`
  rewrite unconditional (**as executed:** renamed `expected_cim`, and its pin
  `cim_lane_divergences_are_pinned` → `cim_writer_divergences_are_pinned`, since
  neither reads the lane any more).

### G2.2d — existing-exclusion rows: event-log transforms

`RELAY_SAMPLE_TRACE_IGNORES_DEBUGTRACE` + `RELAY_RESET_EVENT_IS_LABELLED_RECLOSER`
(`:1014`, `:1046`; issue-29, issue-30): in `harness/lane.rs::expected_eventlog`
(`:176-229`) move
**only the two Relay rewrites** — the `, Element=Debug Sample: Relay.` filter
(`:190`) and the `reset_device_name`/`device_is_relay` `Recloser.<n>` → `Relay.<n>`
relabel (`:191-197`) — above the `if PARITY` early return (`:181-183`), so they
run in both lanes. The `EVENTLOG_REROUNDED` fold (`:184-186` + `:198-207`) and its
`REROUND_VISITS`/`REROUND_HITS` accounting (`:210-227`) **stay behind the parity
guard**: they are the `compat::fmt_g` precision row, alive until G4.1 —
unconditionalizing them hands the parity lane the native `%g` spelling against the
FPC-spelled engine output, a 1e-5 gap vs `compare_eventlog`'s
`assert_value_matches_tol(…, 1e-6, 1e-9)` (`harness/mod.rs:1701`) on the gated
case `controls:invcontrol/midi_invcontrol_drc.dss` = red gate. Model the mixed
shape on `golden_json.rs::lane_expected_json` (`:112-123`). Same commit: update
the `expected_eventlog` doc header (`lane.rs:144-152`) and re-point the in-file
unit tests (`:870-1006`) so the relay assertions hold in both lanes while the
re-round assertions stay parity-conditional.

### G2.3 — Newton

`POWERS_REUSE_STALE_NEWTON_ITERMINAL` (`:703`; issue-05; engine `exec/view.rs:170-198`)
single-kernel; `harness/lane.rs:130-142 LANE_SKIP_ELEM_POWERS` unconditional (both
lanes lose those two decks' powers/losses vs both oracles — record the coverage
note in STATUS); rewrite `exec::tests::newton::newton_powers_are_the_lane_kernel`
into an unconditional expected-value pin (Newton powers == the normal algorithm's
in both lanes) while `newton_dispatch_leaves_a_valid_but_stale_iterminal_cache`
stays untouched; **delete BOTH Newton rows from
`examples/lane_dump.rs:128-131 DOCUMENTED_DIVERGENCES`** (`newton.dss` AND
`newton_feeder.dss` — fail-on-stale, `lane_diff.ps1` breaks otherwise) and run
`lane_diff.ps1` in this sub-step's gate. Strike CLAUDE.md:129 and the deck `note`
at `tests/corpus/modes/manifest.json:488` in the same commit.

### G2.4 — monitor-channel padding reclassify

The `[0.0]` that `MONITOR_CHANNEL_PADS_THE_UNFLUSHED_STREAM` (`:1165`)
reproduced is a **client** artifact, not an engine value. The plan first asked
for a *channel-scoped* normalization firing only for `capi_v0145`, on the
premise that "on `r4133` there is no wrapper, so a `[0.0]` capture is a real
value". **That premise is false, and the sub-step's first attempt returned
blocked with the measurement** (owner resolution 2026-08-06, recorded here): the
pad is a client-layer artifact on **both** gating channels — dss-python pads in
`dss/IMonitors.py`, and the `r4133` channel's captures come from our own bridge,
which replicates that decoder by design (`crates/dss-epri/src/dss.rs:625-634`,
`cnt == 272 -> [0.0]`). Channel-scoping was measured to red three gated `r4133`
cases (`modes:time/generaltime.dss`, `generaltime_yearly.dss`,
`generaltime_duty.dss`).

**The engine half (corrected 2026-08-06 after audit — the first write-up of this
section overstated the Pascal).** Neither authority engine returns an *empty*
channel in the gated state either, so this sub-step is not a pure
reclassification: it also declines an upstream defect, per the 2026-08-02
policy. `Monitors_Get_Channel` keeps its empty `DefaultResult`
(`CAPI_Monitors.pas:304`) only for `SampleCount <= 0` (`:308`) or an invalid
index (`:313-320`); the `generaltime*` decks have `SampleCount > 0` with a
header-only stream (`TakeSample` increments the counter, `Monitor.pas:1195`,
while only `Save` grows the stream, `:1122-1125`), so it allocates `SampleCount`
doubles (`:321`) and fills them from a zero-filled `AllocMem` buffer (`:325`)
whose reads all fail at EOF — i.e. it fabricates `SampleCount` zeros out of
bytes it never wrote. r4133's native accessor is the same: `DMonitors.pas:509-516`
pads `myDBLArray := [0]` only while `SampleCount = 0`, and at `SampleCount > 0`
takes the read branch (`:517-541`) over that same unwritten region. Neither is
reachable through a gating client (both short-circuit at `cnt == 272` and never
call the accessor), so the divergence is **unobservable on either channel** —
which is why it owes no ledger entry, and why an upstream-ready report
(`investigations/to_opendss/`) is the only artifact it produces.

**Resolution (A), as executed.** Drop the alias; the engine's `Monitor::channel`
returns the empty channel in both lanes; `lane::expected_monitor_channel`
(`lane.rs:379-385`) becomes an **unconditional** capture normalization — both
lanes, both channels — documented as a client-decoder artifact of both oracle
read paths, carrying the row's `LANE-EXCLUSION` marker. No `EngineChannel`
parameter is threaded through `harness::compare_monitor` (nothing needs it) and
**no ledger entry is added** — legitimizing a client artifact as an r4133 engine
divergence is exactly what the ledger must not say. Rejected alternatives, with
their *correct* reasons: (B) making the `dss-epri` decoder return the honest
empty channel would not make the bridge engine-faithful either — the r4133
accessor's answer in this state is `SampleCount` zeros, so `[]` merely swaps one
client fabrication for another, while breaking the bridge's design contract of
being a dss-python-shaped reader so both channels' captures stay comparable;
(C) three ledger entries would name a divergence the gate cannot see. ADD the
expected-value pin `monitor_channel_of_an_unflushed_stream_is_empty` (this row
had none today) and register the row with `SPLIT_ALIAS_POPULATION` 13 → 12.

**Fail-on-stale (added at the settle).** Once both lanes report `[]`, a client
that stopped padding would return `[]` too and the normalization would silently
become dead code — the one rot its shape guards cannot see. `lane.rs` therefore
counts placeholder hits vs non-placeholder unflushed captures and
`assert_monitor_pad_is_live` (called from `corpus_gate.rs` beside
`assert_reround_cells_are_live`) fails on any miss.

### G2.5 — the three corpus-blocked WholeCase bug fixes

GIC `%R2` ignored (`elements/pd/gic_transformer/solve.rs:20`; issue-07; cases
`gictransformer_gic.dss`, `gic_midi.dss`), MakePosSequence `Cuf` discarded
(`elements/pd/capacitor/solve.rs:303`; issue-09; `makeposseq_shunt.dss`), LoadShape
MMF plain-text accept-set (`elements/general/load_shape/compute.rs:896`; issue-08;
`shape_mmf.dss`). Fix the engine; per case a ledger entry or field-scoped
exclusion + an expected-value pin, the ledger note citing the pin. `shape_mmf.dss`
exists to observe the quirk — add a sibling deck preserving its sng/dbl MMF-reader
coverage before excluding. Remove the three `Escape::WholeCase` rows
(`oracle_parity_cfg_gate.rs:500-534`; `Escape::WholeCase` 4 → 1, the survivor
being the deferred model-6 item); each fix joins `TORN_DOWN_ROWS` with its ledger
key.

### G2.6 — `Show` device-name column width (capi-only, missed by `4f977d9e`)

`compat::max_device_name_length` (`compat.rs:1419-1437`) reproduces a **defect**,
not a formatting convention: dss_capi's `ShowResults.pas:116` zeroes the unit-level
`MaxDeviceNameLength`, then `:117-121` — inside `with DSS.ActiveCircuit do` —
assigns the *shadowing circuit field* (`Circuit.pas:100`, init `:379 := 30`),
while the glue site `WriteTerminalPowerSeq` (`ShowResults.pas:1375`,
`Pad(...) + IntToStr(j)`) reads the unit var left at 0 — so `Show BusFlow` glues
the terminal number onto the quoted name. **r4133 does not share it**:
`MaxDeviceNameLength` exists there only as a unit var
(`Version8/Source/Common/ShowResults.pas:66`), no circuit field, honest width —
the default kernel already equals the authority. Teardown:

- Delete the parity arm and the zero kernel; `max_device_name_length_measured_impl`
  (`report/show/mod.rs:137`) becomes the sole path in both lanes; update the
  `max_bus_name_length` note (`show/mod.rs:86-92`) and the compat.rs:47 table row.
- Test edits — exactly two lane arms: `golden_reports.rs:2288` (`busflow_expected`,
  body `:2287-2299`) and `:2450` become unconditional. **Do NOT touch
  `golden_reports.rs:2330`/`:2384`** — `terminal_total_expected` and its
  non-vacuity test belong to `compat::render_rows` over
  `PadDots('   TERMINAL TOTAL', …)` (alive until G4.5). The `:2440-2448`
  non-vacuity assert stays as-is (it reads the committed oracle capture, valid in
  both lanes; it is deleted only at G3.3b when the self-snapshot no longer carries
  glue).
- Pin unconditional: `exec::tests::compat_quirks::device_name_column_width_is_the_lane_kernel`
  drops its `parity` branches (`compat_quirks.rs:31`, `:52-56`, `:71-76`) and pins
  the honest width plus the separated terminal column outright.
- Re-MEASURE, do not assume, that no other `Show` golden moves (only the
  `IntToStr` site glues; every other consumer pads with spaces/dots the tokenizer
  drops); anything that does move gets a field-scoped exclusion, never a
  re-baseline. Zero golden bytes move.
- Bookkeeping: `SPLIT_ALIAS_POPULATION` −1 (→ 11 at WP end); the F-FMT narrative
  `oracle_parity_cfg_gate.rs:488-498` ("F.4b took the seventh…"); a tenth row in
  `investigations/to_opendss/NOT-APPLICABLE-TO-R4133.md` with the
  Circuit.pas-field citation; STATUS's "7 F-FMT rendering markers" → 6;
  `TORN_DOWN_ROWS` row; this row has **no** `issue-*` report and no registry
  section — write both (a new `investigations/issue-*` report modeled on the
  series, plus a `TODO_COMPAT_REGISTRY.md` §3 entry) in this sub-step: it is the
  one bug the investigation series missed.

---

## WP-G3 — Golden migration (only after G1 + G2 are fully landed)

**Standing acceptance, every G3 sub-step:** `golden_lock.rs` green; the commit
body names every moved artifact with its cause; the artifact count by anchor is
recorded in STATUS; snapshots are produced in the **parity** lane
(`DSS_UPDATE_GOLDENS=1` + `--features dss-core/oracle-parity`, §1.2) and the gate
is green in BOTH lanes plus the cross-lane lane-invariance check before commit.

### G3.1 — twin audit

New committed table `tests/golden/TWINS.md`: one row per golden scenario that is
**deleted, de-anchored to `self`, or frozen** — columns
`<golden scenario> | <disposition: DELETE|DEANCHOR|FROZEN> | <status: PENDING|LANDED> | <manifest case id(s) / reason> | <comparator>`
(now including the G1 comparators; the `linemedium` mapping recorded in STATUS by
G1.2 lands here as a row). The no-twin rule, in order: if the signal is
expressible in the live gate, a new corpus deck + manifest row is authored here,
not later (`DSS_UPDATE_POPULATION_LOCK=1` regen); if it is not expressible
(pstcalc/plot_callback/ncim), the row goes FROZEN with its reason (§1.2). New
test `golden_twins.rs` **parsing TWINS.md** (the lock gains no bucket/status
field — twin bookkeeping lives here): (a) every named manifest case id exists in
one of the manifests; (b) a `DELETE+LANDED` row's path is absent from
`golden.lock.json`, a `DELETE+PENDING` row's path is still present, and every
DEANCHOR/FROZEN row's path exists — fail-on-stale both ways; each G3.2 commit
flips its rows `PENDING → LANDED`. **Stop condition:** a family whose twin cannot be demonstrated neither gets
deleted NOR de-anchored — it stays `capi_v0145`-frozen with a reason (the §1.2
frozen list: `pstcalc/`, `plot_callback/`, `ncim/` are the known cases); no
schedule pressure justifies deleting or de-anchoring an undemonstrated signal.

### G3.2a — delete: solved-state families

`checkpoints/` (+ `golden_checkpoints.rs`), `ieee13/34mod1/37/123.json`,
`ieee8500.json`, `feeders_controlsoff/` **and the loose
`feeders_controlsoff.json`**, `slice.json`, `reliability.json`,
`gendispatcher.json`, `allocation.json`, and — gated on their twin-audit rows —
`autoadd_reduce.json` and `parser.json` (no twin → they go FROZEN with a reason
instead) (+ drivers). Keep `gen_checkpoints.py::check_pin` importable
(`gen_reports.py` imports it). Deleting `checkpoints/` must not delete the
capture-order rule (§1.1(a)) — verify it is restated at the live capture site
before the driver is removed.

### G3.2b — delete: scenario-replay families

`der_controls/` (45), `harmonics/` (10), `line_constants/` (6),
`metering_monitors/` (5), `timeseries_controls/` (4) + drivers. The required-
scenario lists live in the **drivers being deleted** (`golden_der_controls.rs:21+`,
`golden_harmonics.rs:18-32`, `golden_line_constants.rs:27+`; metering/timeseries
carry their own `load_scenarios` + count asserts) — `harness/scenario.rs` holds
only the generic `check_family` runner and goes dead with its last consumer:
delete it too. Each deletion commit cites the replacing corpus case IDs from
TWINS.md.

### G3.2c — delete: `inc_matrix/`

56 files + driver — superseded by G1.8's live exact compare.

### G3.3a — self-snapshot: `reports/export*` (146 files)

Wire `harness::snapshot_*` into `golden_reports.rs` (the driver already holds the
produced text — `run_deck_export` reads the file back before comparing). First
snapshot = current bytes minus diffs explained by G2 fixes; every moved artifact
named in the commit body with its cause; `golden.lock.json` anchors flip
`capi_v0145 → self` via the `DEANCHORED` register. Same commit: the now-identity
transforms retire — the Iresidual `ColTol` exclusion (`golden_reports.rs:2907`)
and the Bus_Int_Duration duration mask (`:4085-4094`) are deleted; **their
expected-value pins (`:2953`, `:4125-4168`) STAY as the in-repo truth anchors.**
Of the 146 files, the two seasonal `.meta.json` are `capi015` — the anchor guard
skips them (144 snapshots).

### G3.3b — self-snapshot: `reports/show*` (144 files)

Same mechanics. The G2.6 follow-through lands here: the self-snapshot carries no
glued terminal column, so `busflow_expected`/`split_glued_terminal` and the
`glued > 0` non-vacuity assert (`:2440-2448`) are deleted in this commit.

### G3.3c — self-snapshot: `reports/dump*` + `dump3*` (88 files)

Same mechanics (these are `lane::compare_report` byte-contract families — parity
rendering per §1.2).

### G3.3d — self-snapshot: `reports/di_*` + `save*` + `export8500*` (32 files)

Same mechanics; these are the run-file artifacts G1.10 twinned.

### G3.3e — self-snapshot: `reports/loadshape|tshape|priceshape` + `distrib*` (26 files)

Same mechanics.

### G3.4 — self-snapshot: `cim/`, `json/`, `json_import/`

Same mechanics. `json/schema_full_oracle.json` + `schema_divergences.json` stay
frozen (§1.2); `cim/` value semantics are witnessed by G1.1's live props (delivered by `R4133_PROPS_PLAN.md` RP4.1; **that plan completed 2026-09-04, so this sub-step is unblocked**) + the
r4133 CIM bug fixes from G2.2c (and G2.1g, only if its measurement showed a
committed golden observes the flag) land here as predicted diffs. Order inside
the commit: self-snapshot `cim/` **first**, then retire `expected_cim`
(`golden_cim.rs`, `lane_expected_cim` until G2.2c made it lane-independent) and
`cim_writer_divergences_are_pinned` (`cim_lane_divergences_are_pinned` until the
same sub-step) — the transform
becomes the identity only against the new reference (deleting it before the
snapshot reds the gate); the CIM pins stay.

### G3.5 — self-snapshot: `props/`

`props/` value truth is live (`all_properties` on both channels after G1.1 — delivered by `R4133_PROPS_PLAN.md` RP4.1; **that plan completed 2026-09-04, so this sub-step is unblocked**); the
snapshot pins the text rendering only. Of its 51 files, the six `capi015`
scenarios are not snapshotted (the guard refuses them). `pstcalc/`, `plot_callback/`, `ncim/` are
**not** snapshotted — they have no live value twin and stay `capi_v0145`-frozen
(§1.2, G3.1 stop condition); `ncim/`'s dead capi015 meta anchors are recorded in
their lock reasons.

### G3.6 — freeze the exceptions, close the lock

Frozen set of §1.2 pinned with reasons; `snapshot_*` hard-refuse verified by a
negative probe. The two pre-existing self-regen paths that bypass the rails —
`adiakoptics.rs`'s `DSS_REGEN_AD_GOLDEN` and `golden_schema.rs:629`'s
`REGEN_SCHEMA_PORT` (both write their goldens directly) — are either routed
through `harness::snapshot_*` (preferred: one guard, one knob) or given a
`golden_lock.rs` row asserting their targets are anchored `self` and naming the
bypass in the lock reason; the negative probe covers them. Final
`golden.lock.json` state: every artifact `self`, `fpc_3.2.2` (none left after
WP-G4 — see G4.6), `r4133` (wasm/protection/flicker), `r3723` (the A-Diakoptics
witness), `capi015` (10 after G3.2b deleted `line_constants/`; 11 at G0.1), or
`capi_v0145`-frozen (schema pair, pstcalc, plot_callback, ncim, plus any G3.2a
no-twin fallbacks), each non-self with a reason; `golden_lock.rs`
asserts the anchor histogram sums to the committed artifact count — no artifact
may be absent from a G3 disposition.

---

## WP-G4 — Rendering de-FPC (the print-emulation kernels die; user decision 2026-08-02)

Only after WP-G3 (regen is legal only on self-anchored artifacts, R1–R4). One
kernel per sub-step — except G4.3, which flips two (`json_float` +
`JSON_LINE_BREAK` share the fpjson emission path): six kernels across five flip
sub-steps, full ritual each. Every sub-step: flip both cfg arms to the native
impl → delete the kernel and its lane scaffolding → regenerate the affected
(self-anchored) families with the diff **categorically predicted** ("every float
spelling moves to Rust shortest-round-trip" / "line endings LF" / …) and every
moved artifact named per R4 → census (`SPLIT_ALIAS_POPULATION` −1 per kernel, −2
in G4.3), lock, STATUS. Regen runs in the **parity** lane
(`DSS_UPDATE_GOLDENS=1` + `--features dss-core/oracle-parity`) through G4.5 — a
family touched by a not-yet-flipped kernel is still lane-divergent; after each
flip the flipped family's lane-invariance is proven by the §1.2 cross-lane
scratch regen; G4.6 flips every family's `produced_by` to lane-invariant and
retires the producing-lane guard. Each flip also adds its row to
`TORN_DOWN_ROWS` (`Kind::SplitAlias`, `Evidence::None`, pin `None` — the pin
slot is mandatory only for WP-G2 bug rows), keeping the census tie
`31 − count(SplitAlias) == SPLIT_ALIAS_POPULATION` true through G4.6
(31 − 26 = 5). `lane_diff.ps1` runs in every sub-step's gate (a compat kernel
changes; report text is not part of the dump stream, so the `max|Δ| = 0`
expectation is unchanged). Artifacts with anchor ≠ `self` are **never**
regenerated: any such family whose comparator is print-dependent (byte /
raw-string compare) is switched to a value-wise compare (parse + numbers,
rel=abs=0) against the frozen bytes in the sub-step that flips its kernel; G4.1
opens with a measurement pass listing every non-`self` family with a
print-dependent comparator. Doc surface: G2.0's citations name numeric survivors
only, so no doc-citation moves here — each sub-step re-verifies
`oracle_parity_cfg_gate` regardless. The registry entry for the whole family is
`investigations/TODO_COMPAT_REGISTRY.md` §3.31 (11 code sites behind the seven
rendering rows — six rows after G2.6; category «соглашение») — each kernel flip
updates its «Судьба» line (local docs, never skipped).

### G4.1 — `fmt_g`

The `%g` spelling kernel (`compat.rs:1326/1329`) — the widest blast radius: all
report float text. Same commit: delete the `EVENTLOG_REROUNDED` fold and its
`REROUND_VISITS`/`REROUND_HITS` accounting from `harness/lane.rs` (both lanes now
emit native `%g` — the G2.2d parity guard and its cells go); regen the affected
report families.

### G4.2 — `fixed_w_script`

FPC `%8.2f` ties-away kernel (`compat.rs:1332-1347`; consumed at
`report/export/json/circuit.rs`); regen the affected `json*` artifacts.

### G4.3 — `json_float` + `JSON_LINE_BREAK`

fpjson 17-sig literals and CRLF (`compat.rs:1392-1396`, `:1398-1417`); regen
`json/`, `json_import/`. The frozen schema pair is NOT regenerated — its driver
moves to value-wise comparison here (see the preamble); `lane::compare_json`'s
raw parity arm becomes equivalent to the default arm and is simplified.

### G4.4 — `CONTROL_QUEUE_SEC_DIGITS`

The control-queue seconds-digits stand-in (`compat.rs:1350-1375`); regen the
affected report/eventlog artifacts.

### G4.5 — `render_rows`

Pascal `Pad`/`PadDots` table replay vs comfy-table (`compat.rs:1458-1462`): both
lanes move to the native renderer. Show goldens are token-compared, so bytes
should move only in padding — **measure first** that the diff is layout-only;
regen `show*`; delete the `terminal_total_expected` glue machinery
(`golden_reports.rs:2316-2384`) — its non-vacuity tests go with it.

### G4.6 — closure

`fmt_battery.csv` + its driver retire (measure-first: if a consumer outside the
fmt kernels exists — stop and report); every non-`self` family re-verified
print-independent; `SPLIT_ALIAS_POPULATION == 5` (PI, round_f64, round_i32,
kv_base_search_scale, profile_ll_pu_divisor) with the `TORN_DOWN_ROWS` census tie
holding (26 `Kind::SplitAlias` rows); every family's `produced_by` flipped to
lane-invariant, the producing-lane guard retired; TWINS.md + `golden.lock.json`
reflect the deletions; the `fpc_3.2.2` anchor bucket is now empty and is removed
from the enum.

---

## WP-G5 — Documentation and the restated oracle argument

### G5.1 — operational docs

- `CLAUDE.md`: the §Known upstream bugs rows were already struck row-by-row in
  WP-G2 — here only: rewrite the "numeric oracle" bullet (goldens =
  self-snapshots of form, per-artifact anchors in `golden.lock.json`, native
  rendering since WP-G4); rewrite the gate section's transitive
  `|default − oracle|` paragraph — the default lane's oracle standing is now
  **direct**: the live dual-channel gate at fastdss-parity carries the claim;
  `lane_diff` remains the sharper-than-tier kernel tripwire.
- `TESTING.md`: golden-families table gains the anchor column **and loses the
  rows for every family deleted in G3.2**; the two-lane table (`TESTING.md:73`)
  drops "checkpoint Y" (deleted in G3.2a) and "every upstream quirk reproduced"
  (rescinded 2026-08-02 — replace with "precision-compat numeric kernels only");
  regen procedure = R1–R4; env-var section; the transitive-argument rewrite
  (`TESTING.md:89-158`); golden lock beside the population lock; the WP-G4
  outcome (both lanes render identically).
- `tests/TOLERANCE_NOTES.md`: the G1 floor derivations live here; the Iresidual
  note **rewritten in place** by G2.2a (it stayed in §Field-specific exceptions —
  it describes the `SeqCurrents` compare policy, and §Deliberately-reproduced
  upstream inexactnesses, the move target the sub-step text named, is about
  reproductions, which that row no longer is; STATUS records the deviation).
- Every edit keeps `oracle_parity_cfg_gate.rs:1310`
  (`operational_docs_cite_the_compat_machinery_accurately`) green — including its
  `alias_refs >= 4` floor (the G2.0 citations must survive every edit);
  `TAG_PATH_CITATIONS` updated in the same commit.

### G5.2 — closing record

`PLAN_SEQUENCE.md`: the entry already exists (5a, inserted 2026-08-22 by the
R4133_PROPS authoring commit, with the not-a-prerequisite-for-M0–M2 framing
carried by its position) — flip it to COMPLETE, do **not** insert a second
row. `WASM_USERMODELS_PLAN.md`: add the named
model-6 `FInit` row (§1.3); `ORPHANED_GAPS.md`: record it until that row exists.
`STATUS.md` closing record: final `SPLIT_ALIAS_POPULATION` (= 5),
`Escape::WholeCase` (= 1, model-6 — reported as the last reproduced upstream
bug), golden count by anchor, the ledger delta, and a full
`pwsh -File tools/lanes/lane_diff.ps1` run for the record (expect `max|Δ| = 0`,
documented-divergence list shorter by both Newton rows). Final sync of
`investigations/TODO_COMPAT_REGISTRY.md` (every torn-down row's «Судьба» updated,
the arithmetic of its §5 re-checked). Move this plan to `docs/plans-archive/` on
completion (the 2026-07 convention).

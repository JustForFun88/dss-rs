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
| 3 | meter extras: CalcCurrent, AllocFactors, Totals, SAIFI/SAIFIKW/SAIDI/CustInterrupts, active-section fields (fastdss captures the FIRST section only — parity = at least that), **ordered** zone vectors (the live gate already set-compares `AllBranchesInZone`/`AllEndElements`/`ZonePCE` — `harness/mod.rs:2022-2041`, `oracle_server.py:226-246`; the gap is order + the extras, NOT the lists' existence), **plus the per-bus reliability columns** `Bus.Lambda / N_interrupts / N_Customers / Cust_Interrupts / Cust_Duration / Int_Duration / TotalMiles / SectionID` (`IBus._columns` on `origin/fastdss` — the very columns `Export BusReliability` renders and the `BUS_INT_DURATION` surface, hence the G2.2a-before-G1.6 ordering). CAIDI does **not** exist in the DSS-Python API on either branch — it is gated only if the r4133 DLL exposes it (measure in G1.11c), else documented as not-comparable | `reliability`/`export_busreliability*` goldens | G1.6 |
| 4 | Topology interface: NumLoops, NumIsolatedBranches/Loads, AllLoopedPairs, AllIsolatedBranches/Loads | `show_topology`/`show_isolated` goldens | G1.7 |
| 5 | Bus.Distance, AllBusDistances, AllNodeDistances | `profile` goldens | G1.4 |
| 6 | Solution.IncMatrix/IncMatrixCols/IncMatrixRows/Laplacian (fastdss-branch-only additions — exactly why the reference is `origin/fastdss`) | `inc_matrix/` goldens | G1.8 |
| 7 | circuit aggregates: TotalPower, Losses, LineLosses, SubstationLosses, **AllElementLosses**, plus the Solution scalars ControlIterations / Totaliterations / MostIterationsDone / ControlActionsDone / SystemYChanged / Seconds / LoadMult / Year / Hour / Mode | `export_losses`/`summary` goldens | G1.9 |
| 8 | run-produced files: **every** `*.csv` the deck emits under DataPath (fastdss archives and compares them all, incl. the forced `export profile phases=all` — DI CSVs are just the closedi subset); `save circuit` output **file set** (fastdss archives it but never compares — `compare_outputs.py:426-529` has no `.dss` branch — so our file-set + round-trip check is strictly stronger; state that, don't claim parity) | `di_*`/`save_*` goldens | G1.10 |
| 9 | CktElement discrete extras: PhaseLosses, NodeOrder, EnergyMeter, OCPDevType, OCPDevIndex, HasVoltControl, HasSwitchControl, NumControls, NumTerminals/NumPhases/NumConductors; LineGeometries.Rmatrix/Xmatrix/Zmatrix (measure-first); Lines.Yprim (verify it is already witnessed by the per-element YPrim live compare, record in TESTING.md) | scattered `props/`/report goldens | G1.3d |
| 10 | PDElements interface: AccumulatedL, ParentPDElement, FromTerminal, IsShunt, Numcustomers, SectionID, RepairTime, Totalcustomers, Lambda | `reliability` goldens (partially) | G1.6b |
| 11 | Bus extras: VLL/puVLL, VMagAngle, AllPCEatBus/AllPDEatBus | `export_seq*`/`profile` goldens | G1.4 |

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

### G1.4 — bus surface: pu-voltages, seq voltages, distances, extras

`Bus.puVmagAngle`/`puVoltages`/`AllBusVmagPu`, `Bus.SeqVoltages`/`CplxSeqVoltages`,
`Bus.VLL`/`puVLL`, `Bus.VMagAngle`, `AllPCEatBus`/`AllPDEatBus`, `Bus.Distance`,
`AllBusDistances`, `AllNodeDistances` (meter-zone distances — cases with meters
only; manifest-flagged).

### G1.5 — short-circuit surface

`Bus.Zsc1/Zsc0/ZscMatrix/YscMatrix/Isc/Voc` on cases that ran a fault study (the
precomputed-state semantics of `Export Faultstudy` — the gate reads what the solve
populated, never re-runs the study). Manifest flag on the faultstudy-family cases.

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

### G1.6b — PDElements interface

`AccumulatedL`, `ParentPDElement`, `FromTerminal`, `IsShunt`, `Numcustomers`,
`SectionID`, `RepairTime`, `Totalcustomers`, `Lambda` — the per-PD-element walk
fastdss compares wholesale.

### G1.7 — topology interface

`NumLoops`, `NumIsolatedBranches`, `NumIsolatedLoads`, `AllLoopedPairs`,
`AllIsolatedBranches/Loads` (`ITopology._columns` on `origin/fastdss` lists all of
these, so this sub-step is straight parity, not a bonus; `ActiveLevel`/
`BranchName`/`ActiveBranch` are iteration cursors and are deliberately not
compared).

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
empty (`crates/dss-epri/tests/modes.rs`). The `X_Get_Y` spellings below and above
name the *properties* to capture, not symbols to bind. See the WP-G1 preamble note
and TESTING.md §"The r4133 bridge — entry points, mode capability, do-not-call".)*

### G1.11b — r4133 channel: Bus families

Serving G1.4 + G1.5: `Bus_Get_puVmagAngle/SeqVoltages/CplxSeqVoltages/Distance/`
`Zsc1/Zsc0/ZscMatrix/YscMatrix/Isc/Voc`,
`Circuit_Get_AllBusDistances/AllNodeDistances/AllBusVmagPu`.

### G1.11c — r4133 channel: Meters/Topology/Solution/Circuit families

Serving G1.6–G1.9 (incl. the CAIDI capability measurement of G1.6). A group the
DLL cannot serve stays capi-only with a one-line note in TESTING.md.

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

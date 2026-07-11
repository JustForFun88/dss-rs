# DIAKOPTICS_PSTCALC_PLAN — Pstcalc/flicker + A-Diakoptics (the last PORTING_PLAN Phase-9 residue)

Created 2026-07-11. Closes the two Phase-9 items that still have no home:
`Shared/Pstcalc.pas` (IEC-868 flicker: the `Pstcalc` command + Monitor mode 4) and
`Common/Diakoptics.pas` + satellites (the A-Diakoptics spatial-parallelization solve).
`MULTITHREADING_PLAN.md` M2 covers only the *actor* machinery and explicitly leaves
A-Diakoptics out; this plan is where A-Diakoptics actually gets ported.

Two parts with **different oracles, gates and timing** (see §0 for why):

- **Part I (oracle-gated, pre-acceptance):** WP-PF.1 `Pstcalc` command, WP-PF.2 Monitor
  mode-4 flicker, WP-AD.1 incidence matrix + `Sparse_Math`. All three are compiled into
  the pinned oracle → normal golden/live gating. They belong to final-acceptance scope.
- **Part II (no oracle, post-acceptance):** WP-AD.2–WP-AD.6, the A-Diakoptics engine.
  The pinned oracle build has this code **compiled out** (§0.2) — the gate is
  Rust-vs-Rust equivalence (A-Diakoptics solve ≡ normal solve on the same deck) across
  the whole corpus, plus structural invariants and fixture goldens.

Sequencing lives in `PLAN_SEQUENCE.md`: Part I = porting stage 3 (after the GAPS
follow-ups WPG.19/20), Part II = the final stage after `MULTITHREADING_PLAN` M2
(early-start allowed right after M2; it does not need M3/M4).

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

## Per-step ritual (every WP, in order, autonomously — the `PHASE8_PLAN` discipline)

0. **Tier check** — compare the session against the WP's **exec tier** (§4); below tier →
   do NOT execute, reply exactly: «Этот шаг требует <exec tier>. Переключи сессию
   (/model + reasoning effort) и повтори команду.» and stop.
1. **Gate green** — `cargo fmt --all --check` · `cargo clippy --workspace --all-targets
   -- -D warnings` · `cargo test --workspace`. For Part I this includes the new goldens
   and family decks vs the pinned oracle. For Part II "gate green" additionally means the
   **rust-only A-Diakoptics gates** (fixture goldens, invariants, the corpus AD sweep) —
   there is no oracle leg for AD, and that is by design, not an excuse to skip: the
   rust-only gates are mandatory and unconditional in `cargo test`. No `#[ignore]`, no
   name filters that green on zero matches. A red test blocks the commit.
2. **Update `STATUS.md`** (frontier + a per-WP record), **commit** (code + STATUS together).
3. **`/audit-code` + `/audit-tests` in parallel** — two fresh independent agents (never
   forks), spawned with an explicit model/effort override per §4. Brief: commit range,
   diff, this plan's WP section, the authoritative Pascal units (§0.1 table), and the
   binding rules (§1 decision records; the no-fudging tolerance rule; for Part II — the
   D7 comparison contract). Auditors are read-only; **you** settle findings empirically,
   fix what is real, record deliberate no-fixes in STATUS, re-run the gate, commit.
4. **`STATUS.md` full review** — sync stale sections, archive dead weight, dedup.
5. **Only now stop** and report **in Russian** (code/identifiers/commits/STATUS stay
   English): what landed, audit findings + resolution, gate status, next step.

---

## §0 Ground truth — read this before any WP

### §0.1 The Pascal spec surface (file / lines / role)

**Pstcalc (Part I):**

| Source | Lines | What |
|---|---|---|
| `Shared/Pstcalc.pas` | 1–474 | `PstRMS` → `_Pst`: the **f64** IEC-868 path used by the `Pstcalc` command. Filter state `Double6Array`s, `Set_Filter_Coefficients` (120 V vs 230 V bandpass constants), `Get_Pinst` (input adapter → bandpass → weighting → sliding mean), 16000 bins / `bin_ceiling=350`, `CalcPst` percentile weights (0.0314·P0.1 + 0.0525·P1s + 0.0657·P3s + 0.28·P10s + 0.08·P50s), 30 s warm-up at 1 pu + 5 s settle, 600 s intervals, `input_type=6` fixed (`internal_reference=0.008449`), `Tstep = 1/(16·Fbase)`, `DeltaT = NcyclesperSample/Fbase` |
| `Shared/Pstcalc.pas` | 476–687 | `FlickerMeter`: the **f32** (`Single`) RMS-flickermeter used by Monitor mode 4. Filter cascade `Fhp`→`Fw1`→`Fw2`→square→`Flp`, `cf = 1/1.285e-6`, 50/60 Hz coefficient sets, Block-5 Pst via `QuickSort` + `Percentile` (interpolated, `pctExceeded` list) every 600 s. In/out arrays are `pSingleArray`, 1-indexed, `y[1]:=0`/`y[2]:=0` seeds |
| `Executive/ExecHelper.pas` | 4778–4845 | `DoPstCalc`: parser (`Npts`,`Voltages`,`dt`/`cycles`,`freq`,`lamp`), `Npts>10` guard, result string `Format('%.8g, ', …)` into `GlobalResult` |
| `Executive/ExecCommands.pas` | 117, 623 | command `Pstcalc = 96` → `DoPstCalc` |
| `Meters/Monitor.pas` | 156–158, 602–605, 665–668 | `FlickerBuffer: pComplexArray`, allocated `Nphases` on `RecalcElementData`/`MakePosSequence` for mode 4 |
| `Meters/Monitor.pas` | 736–739 | mode-4 header: `Flk<i>`, `Pst<i>` per phase (record size `2·Nphases`) |
| `Meters/Monitor.pas` | 1252–1263, 1479, 1560–1562 | `TakeSample` mode 4: `FlickerBuffer[i] := NodeV[NodeRef[i]]` (complex), `ConvertComplexArrayToPolar`, write `2·Fnphases` doubles (mag, ang) — narrowed to f32 by the buffer |
| `Meters/Monitor.pas` | 1101–1116, 1136–1144 | `CloseMonitorStream` → `PostProcess` → `DoFlickerCalculations` when `mode=4` and stream non-empty; `IsProcessed` latch |
| `Meters/Monitor.pas` | 1602–1688 | `DoFlickerCalculations`: re-reads the stream (time `= s + 3600·hr`, per-phase **odd channel** = magnitude), `Npst = 1 + Trunc(t_N/600)`, `Vbase = 1000·kVBase` of the metered terminal's bus, calls `FlickerMeter` per phase, then **rewrites the stream in place**: channel 2p−1 ← flicker level, channel 2p ← Pst of the current 600 s interval (0 until the first interval completes; `ipst` advances when `t−tpst ≥ 600`) |

**A-Diakoptics (Part II) + incidence matrix (WP-AD.1, Part I):**

| Source | Lines | What |
|---|---|---|
| `Common/Sparse_Math.pas` | whole unit | `Tsparse_matrix` (integer COO, `data: array of array of Integer` rows `[row,col,val]`) and `Tsparse_Complex` (`CData: array of TCmplx_Data`); `insert` (accumulates into an existing cell, else appends — **insertion order is the storage order**), `add`, `Transpose`, `TransposeConj`, `multiply`, `Rank`, `NZero/NRows/NCols` |
| `Common/Solution.pas` | 202–232, 303–315, 1340–1620 | incidence matrix: `IncMat`/`Laplacian: Tsparse_matrix`, `Inc_Mat_Rows/Cols/levels`, `ActiveIncCell`, `Upload2IncMatrix`, `AddLines2IncMatrix`/`AddXfmr2IncMatrix`/`AddSeriesCap2IncMatrix`/`AddSeriesReac2IncMatrix` (series-only filter for cap/reactor), `Calc_Inc_Matrix` (flat), `Calc_Inc_Matrix_Org` (hierarchical/BFS with levels), `get_IncMatrix_Row/Col` |
| `Common/ExportResults.pas` | 3310–3399 | ungated exports 53–57: `ExportIncMatrix`, `ExportIncMatrixRows`, `ExportIncMatrixCols`, `ExportBusLevels`, `ExportLaplacian` (CSV formats — port byte-exact) |
| `Common/ExportResults.pas` | 3401–3485 | AD-gated exports 58–61: `ExportZLL`, `ExportZCC`, `ExportC` (Contours, real part only), `ExportY4` — all no-op unless `Solution.ADiakoptics` |
| `Common/Diakoptics.pas` | whole unit (707 lines) | `Solve_Diakoptics` (45–80), `get_Statistics` (84–110), `SendIdx2Actors` (115–156), `Calc_Y4` (160–193), `Calc_ZCC` (197–252), `Calc_C_Matrix` (257–347), `Calc_ZLL` (351–438), `ADiakoptics_Tearing` (446–473), `ADiakopticsInit` state machine 0–9 (477–704) |
| `Common/Circuit.pas` | 255–301 | AD fields: `VIndex`, `Coverage`/`Actual_Coverage`, `Num_SubCkts` (init `CPU_Cores−1` at 564), `Link_Branches`, `PConn_Names/Voltages`, `Locations`, `BusZones`, `Contours/ContoursT/ZLL/ZCT/ZCC/Y4/Ic: TSparse_Complex`, `MeTISZones` |
| `Common/Circuit.pas` | 702–833 | `get_longest_path`, `Append2PathsArray`, `Normalize_graph`, `Get_paths_4_Coverage` (the `Refine_BusLevels` command body) |
| `Common/Circuit.pas` | 834–1078 | `AppendIsources` (ISource `amps=0.000001 angle=0` per node at a link-branch bus), `Format_SubCircuits` (Master → Master_Interconnected + per-zone Masters + per-zone `VSource.dss` with measured `PConn_Voltages`), `Save_SubCircuits` (`save circuit` into `<output>/Torn_Circuit`, then format) |
| `Common/Circuit.pas` | 1081–1305 | `Create_MeTIS_graph` (incidence → Laplacian → dedup parallel branches → `.graph` file: header `<NCols> <numEdges> 1`, weights = NPhases, Transformer weight 1), `Create_MeTIS_Zones` (runs `kmetis`, parses `<graph>.part.<N>`, swap-first-two-lines quirk at 1258–1260, zone-must-have-≥2-buses rule, fills `Locations`/`BusZones`, then `inc(Locations[j])` for all) |
| `Common/Circuit.pas` | 1306–1334, 1336–1570 | `Disable_All_DER`; `AggregateProfiles` (MeTIS zones → per-zone EnergyMeters → solve → aggregated loadshape model in `Aggregated_model/`) |
| `Common/Circuit.pas` | 1574–1697 | `Tear_Circuit`: graph+zones, disable all EnergyMeters, per-location link branch from `Inc_Mat_Rows`, terminal orientation by comparing node-1 |V| at both ends, `PConn_Voltages` capture (3 phases: mag/1000, angle), places `New EnergyMeter.Zone_<i> element=<PDE> terminal=<j> option=R action=C` |
| `Common/Solution.pas` | 43–68 | `AD_ACTORS` wait flag; `TActorMessage` incl. `INIT_ADIAKOPTICS`, `SOLVE_AD1`, `SOLVE_AD2`, `GETCTRLMODE` |
| `Common/Solution.pas` | 233–255 | solution AD fields: `ADiakoptics`, `ADiak_Init`, `ADiak_PCInj`, `ADiakoptics_ready/Actors`, `LocalBusIdx`, `AD_IBus`, `AD_ISrcIdx` (`Node_dV`/`Ic_Local` — see D5: dead, do not port) |
| `Common/Solution.pas` | 693–702 | `Converged`: children map through `VoltInActor1`; the coordinator (Parent=NIL) uses plain `NodeV` |
| `Common/Solution.pas` | 885–914 | `DoNormalSolution`: per-iteration `Solve_Diakoptics` on the coordinator (with `ADiak_PCInj := TRUE`); children run the normal body |
| `Common/Solution.pas` | 1133–1175 | `CheckControls` AD branch: `SendCmd2Actors(DO_CTRL_ACTIONS)`, `ControlActionsDone = AND` over children |
| `Common/Solution.pas` | 1237–1273, 2409–2429 | `SolveDirect` / `SolveYDirect` AD branches (`ADiak_PCInj := FALSE`); `SolveCircuit` skips the coordinator Y build (1302–1306) |
| `Common/Solution.pas` | 1893–1901 | `VDiff` AD branch (children: `VoltInActor1(i) − VoltInActor1(j)`) |
| `Common/Solution.pas` | 2328–2337 | `SolveSystem` AD branch: children solve **into the parent's V array** at offset `LocalBusIdx[0]` |
| `Common/Solution.pas` | 2593–2611, 2670–2721 | actor-loop handling of INIT/AD1/AD2/GETCTRLMODE; `SolveAD(Initialize)` (phase 1: zero + source inj (+PC inj if `ADiak_PCInj` or dynamic/harmonic) + Y check + solve; phase 2: `UpdateISrc` + solve); `SendCmd2Actors` (barrier: `Wait4Actors(AD_ACTORS)`) |
| `Common/Solution.pas` | 2724–2756, 2777–2813, 2855–2927 | `Start_Diakoptics` (disable the feeder-head PDE in every child but the first; disable artificial `source`/`vph_2`/`vph_3` VSources), `VoltInActor1` (`NodeIdx + LocalBusIdx[0] − 1`), `UpdateISrc` (adds `−Ic[row]` into `Currents[AD_IBus[i]]`), `IndexBuses` (child node → parent node index map; `AD_IBus`/`AD_ISrcIdx` from `Contours` rows) |
| `Common/Ymatrix.pas` | 430–434 | AD-gated `Node_dV`/`Ic_Local` allocation — dead scaffolding (never read; see D5) |
| `Executive/ExecOptions.pas` | 152–158, 742–753, 1100–1112 | options `Coverage`, `Num_SubCircuits`, `ADiakoptics` (set → `ADiakopticsInit` / clear flag), `LinkBranches` (**get-only** in the vendored source — there is NO `set LinkBranches`/`UseMyLinkBranches`; that is a newer official-OpenDSS feature, out of scope) |
| `Executive/ExecCommands.pas` | 147–151, 406–433, 448–450, 667–669 | commands `AggregateProfiles`, `Tear_Circuit` (AD-gated); `CalcIncMatrix`/`CalcIncMatrix_O`/`CalcLaplacian` (ungated), `Refine_BusLevels` (AD-gated); `Solve` resets `AD_Init` |
| `Common/MeTIS_Exec.pas` | whole unit | `RunMeTIS` is `{$IFNDEF FPC}` — **Delphi-only, never compiled in the FPC spec build**; under FPC `Create_MeTIS_Zones` uses `Process.ParseCommand` to run `kmetis`. `GetNumEdges` + `TFileSearchReplace` implement the "wrong edge count → patch the .graph header and retry" repair loop |

### §0.2 The build-flag fact (why Part II has no oracle)

`src/common-release.cfg` / `common-debug.cfg` define `DSS_CAPI_PM` and
`DSS_CAPI_ADIAKOPTICS_DISABLED`; the positive symbol `DSS_CAPI_ADIAKOPTICS` is **never
defined anywhere** in the vendored tree. Every `{$IFDEF DSS_CAPI_ADIAKOPTICS}` block —
the whole of `Diakoptics.pas`'s call surface, the `Tear_Circuit`/`AggregateProfiles`/
`Refine_BusLevels` commands, the `Coverage`/`Num_SubCircuits`/`ADiakoptics`/`LinkBranches`
options, exports 58–61 — is **compiled out of the spec build and out of the pinned
oracle** (dss-python 0.15.7 / dss_capi 0.14.5). Evidence already in-repo: the AD master
decks are classified `skipped_oracle_issue.json` with `oracle_error_#130` («Unknown
parameter "Num_SubCircuits"»), and `help_catalog.rs:871` carries upstream's own note
"Currently not supported on DSS-Extensions" (dss_capi issue #46).

Consequences (binding):

- **Part I is normal porting**: `Pstcalc`, Monitor mode 4, the incidence-matrix layer
  and exports 53–57 are all *in* the oracle → golden + live gates as usual.
- **Part II cannot be oracle-gated, ever, with this PIN.** The Pascal **source** is still
  the spec (it is present and readable); the gates are defined in D1/D7. Implementing a
  surface the reference *binary* refuses is a deliberate, recorded departure — the Rust
  engine behaves like a hypothetical `DSS_CAPI_ADIAKOPTICS` build. The corpus AD master
  decks **stay** in `skipped_oracle_issue.json` for the live oracle gate (the oracle
  still errors on them); their Rust-side coverage comes from the new rust-only gates.
- The upstream `.graph`/`Torn_Circuit` artifacts vendored in
  `tests/corpus/electricdss-tst/Version8/Distrib/Examples/ADiakoptics/ckt24/` were
  produced by *official OpenDSS* (a different lineage than the vendored 0.14.5 source) —
  useful as **diagnostic cross-checks only**, never as byte gates.

### §0.3 Where the Rust tree stands today (inventoried 2026-07-11)

- `Pstcalc` command: registered (`exec/tables.rs:110`), refused via `not_ported_command`.
  `Tear_Circuit`/`AggregateProfiles`: **not registered at all** (→ "Unknown Command").
  `set/get ADiakoptics|Num_SubCircuits|Coverage|LinkBranches`: unknown-parameter error.
  `Export ZLL/ZCC/Contours/Y4`: unknown-export error (deliberately unregistered,
  `report/mod.rs:28–31`). `Export IncMatrix/IncMatrixRows/IncMatrixCols/BusLevels/
  Laplacian`: registered keywords (`report/mod.rs:88–92`) → scoped `NOT_PORTED`.
  `CalcIncMatrix`/`CalcIncMatrix_O`/`CalcLaplacian`/`Refine_BusLevels`: registered →
  `not_ported_command`.
- Monitor: `crates/dss-core/src/elements/meter/monitor/` — mode-4 **header already
  built** (`header.rs:117–122`, `Flk{i}`/`Pst{i}`, record size `2·nphases`); sample body
  is a documented no-op (`sample.rs:192–196`); no `DoFlickerCalculations` port. Stream
  model: single `mon_buffer: Vec<f32>`, records `[hour, sec, ch…]`, `flushed_records`
  mirrors `Save`; `to_csv()` mirrors `TranslateToCSV`.
- Engine: strictly single-context (`MaxCircuits=1`), no children/actors; a second
  `Dss::new()` in-process is trivial (tests do it constantly). `dss-sparse` exposes
  `factor()` (cached) + `solve(b, x)` single-RHS — per-column `Y·z=c` solves are a loop
  reusing the factorization. `CMatrix::invert()` (`support/cmatrix/mod.rs:235`) is the
  `TcMatrix.Invert` port A-Diakoptics needs for `Calc_Y4`. EnergyMeter zones exist
  (`solution/meters/zones/`); **no incidence matrix anywhere**; `save circuit` is ported
  (`exec/save_circuit.rs`).
- Corpus: family gates `modes`(32)/`asymmetric`(36)/`controls`(55) in
  `crates/dss-core/tests/corpus_live.rs` + per-family `manifest.json`
  (`SolvableCase`: `path/kind/post/n_steps/…/pending/wp/oracle/compare_*`); vendored
  corpus classified by bijection across `tests/corpus/manifests/*.json`. Tolerance tiers:
  `tests/harness` `Tolerances`/`tol_for` + `tests/TOLERANCE_NOTES.md`. The pre-torn
  `ADiakoptics/*/Torn_Circuit/**` decks are already `solvable_now` (tier
  `large_ultra_switch` exists for the stitched interconnected masters).
- `Examples/Matlab/pst.dss` (the upstream flicker demo, monitor `mode=4`, 8640-step
  duty; `WindRmsV.csv` is vendored) is currently `skipped_unsupported`
  (`file-backed-arrays`) — unblocked by **WPG.19** (in flight in a parallel session);
  its only `solve` is commented out upstream, so it stays a non-entry-point. WP-PF.2
  recreates it as a driver (see there).

---

## §1 Binding decision records

**D1 — A-Diakoptics gates (no oracle).** Three mandatory legs, all rust-only:
(1) *source fidelity* — loop-for-loop transcription of the `{$IFDEF DSS_CAPI_ADIAKOPTICS}`
code with Pascal citations in doc comments, audited against §0.1; (2) *structural
invariants + fixture goldens* — committed fixtures for `Contours`/`ZLL`/`ZCC`/`Y4` on
hand-checkable feeders, with the invariants recomputed inside the test (Contours columns
have exactly one +1 and one −1; ZLL is block-diagonal 3×3 per link = inverted link
Yprim; `ZCC = ContoursᵀZCT + ZLL` re-derived; `Y4·ZCC ≈ I` residual bound); (3) *solve
equivalence* — A-Diakoptics and the normal solve are two iterations to the **same
fixpoint** under the same `ConvergenceTolerance`, so `AD-solve ≈ normal-solve` within a
calibrated tier (D7) on every eligible deck (WP-AD.4). The CLAUDE.md
don't-rationalize-conditioning rule applies with full force: an AD↔normal gap above the
calibrated tier is a port bug, and the tighten-tolerance experiment (D7) is a permanent
test, not a one-off.

**D2 — METIS substitution.** The spec path shells out to an external `kmetis`
executable that is not vendored, not pure Rust, and (under FPC) not even wrapped by
`RunMeTIS` (Delphi-only). Decision: port `Create_MeTIS_graph` and the `.part.<N>`
**file formats and parsing 1:1**, and replace the external partitioner with a
deterministic pure-Rust one behind the same file interface:
`crates/dss-core/src/support/partition.rs` — greedy BFS k-way partitioning of the
`.graph` adjacency (same vertex order as `Inc_Mat_Cols`, edge weights as written to the
file), growing zones level-contiguously from the feeder head to `⌈NVertices/k⌉`,
deterministic tie-break = ascending vertex index. It writes `<graph>.part.<N>`
(one zone id per line, same format kmetis emits) and `Create_MeTIS_Zones` consumes it
unchanged. Consequences: partitions are *valid but not kmetis-identical* (nothing can
gate kmetis-identity — no oracle); the edge count we write is exact, so the upstream
"repair loop" (`GetNumEdges` + `TFileSearchReplace` header patch + retry) is
**not ported** (document at the call site). Running a real external kmetis is out of
scope permanently (no external binaries in the engine or tests). Mark the substitution
in the module doc as `NOTE(subst-metis)` — greppable, like `TODO(compat)` but permanent.

**D3 — children without threads.** Upstream AD runs on PM actor threads that are
barrier-synchronized after every message (`SendCmd2Actors` → `Wait4Actors`), i.e. the
algorithm is *bulk-synchronous* — thread scheduling can never affect the numbers.
Decision: the port executes children **sequentially**: `AdCoordinator { children:
Vec<Dss>, … }` owned by the main `Dss`; `TActorMessage` is mirrored as a Rust enum
(`AdMsg::InitAdiakoptics | SolveAd1 | SolveAd2 | DoCtrlActions | GetCtrlMode`) and
"sending" = a synchronous method call loop, preserving 1:1 traceability to the Pascal
actor loop (`Solution.pas:2593`). Every place the Pascal child dereferences
`DSS.Parent.…` becomes an explicit argument passed by the coordinator (the parent NodeV
buffer for the offset solve, `Contours`/`Ic` read-only views, `VIndex`/`LocalBusIdx`).
Threading the children (fan out AD1/AD2 via `std::thread::scope`) is the optional
WP-AD.6 stretch **after** MULTITHREADING M2, gated by *bitwise* equality with the
sequential path.

**D4 — numeric fidelity split.** `FlickerMeter` and everything Monitor-mode-4 stays
**f32** exactly where Pascal uses `Single` (filter states, coefficients, `hst`
percentile buffer, the stream itself); `PstRMS`/`_Pst` stays **f64** throughout. Before
porting either, probe FPC semantics empirically (the `tools/golden/probe_val.py`
pattern) for: `power(Tstep, 4)` (FPC `Math.power` — integer-exponent path?), `round(…)`
(banker's rounding — reuse the existing FPC `Round` port), `Trunc`, and the `%.8g`
result formatting (reuse the existing FPC format machinery). Goldens then pin the
outcome; expected: byte-exact `GlobalResult` strings and f32-exact monitor channels
(floors, if any, go to `tests/TOLERANCE_NOTES.md` with the decomposition proof — never
a silent widen).

**D5 — upstream quirk inventory.** Deterministic-and-defined quirks are reproduced 1:1
and pinned by the fixture goldens; UB is not reproduced (CLAUDE.md known-bugs rule).
Since no oracle can pin Part II, reproduced quirks get a greppable
`NOTE(upstream-quirk)` comment (not `TODO(compat)`, which is reserved for
golden-pinned inexactness vs the oracle):

| Site | Quirk | Treatment |
|---|---|---|
| `Diakoptics.pas:188` (`Calc_Y4`) | `(value.re <> 0) and (value.re <> 0)` — `.re` tested twice; Y4 entries with `re=0, im≠0` are dropped | reproduce; fixture-pinned |
| `Diakoptics.pas:237` (`Calc_ZCC`) | ZCT entries kept only if `re≠0 AND im≠0` | reproduce; fixture-pinned |
| `Diakoptics.pas:317` (`Calc_C_Matrix`) | node lookup by **substring** (`ansipos`) — bus-name prefix collisions pick the first superstring match | reproduce; add a fixture comment documenting the hazard |
| `Diakoptics.pas:389–424` (`Calc_ZLL`) | LinkPrim extraction loop only fills a 3×3 from `Yorder=6` two-terminal elements — link branches are effectively required to be **3-phase Lines** (a <3-phase link leaves LinkPrim partially zero → singular `Invert`) | reproduce the loop 1:1; surface the singular-invert as the upstream error path; WP-AD.4 eligibility excludes non-3-phase cut candidates |
| `Diakoptics.pas:139–147` (`SendIdx2Actors`) | `j` read after a completed `for` loop when the bus is not found (FPC loop-var-after-loop) | unreachable by construction (a child's bus 1 always exists in the parent); port as an explicit error instead of reproducing UB; document |
| `Pstcalc.pas:579–592` (`Percentile`) | `nhi = nlo+1` can index one past `ihst` at small `pctExceeded` (reads inside the allocation but past the logical end) | probe first; if it reads uninitialized/stale slots → **do not reproduce** (UB class): clamp + document, like the `Bus_Int_Duration` OOB precedent |
| `Solution.pas` `Node_dV`/`Ic_Local` (+ `Ymatrix.pas:430–434`) | allocated, never read | do not port; note at the `AdCoordinator` definition |
| `Diakoptics.pas:56–79` | `Vpartial` leaked (reassigned without free) | memory bug, no numeric effect — not applicable in Rust |
| `Circuit.pas:1258–1260` (`Create_MeTIS_Zones`) | `TextCmd := MeTISZones[1]; Delete(0); Insert(0, TextCmd)` — replaces line 0 with line 1 (drops the first bus's zone id, duplicates the second) | reproduce 1:1 — it shifts zone boundaries deterministically; fixture-pinned |

**D6 — test determinism.** Every AD test and deck sets `Num_SubCircuits` explicitly
(sweep default: 2). The CPU-derived defaults (`Num_SubCkts = CPU_Cores−1` at
`Circuit.pas:564`; the `CPU_Cores−2` clamp at `Diakoptics.pas:508`) are ported —
`CPU_Cores` := `std::thread::available_parallelism()` — but **no test may depend on
them**. `get_Statistics` output is machine-independent only because tests fix the zone
count.

**D7 — the AD↔normal comparison contract (used by WP-AD.3/AD.4).** Comparing "deck
solved normally" vs "deck + AD preamble" mixes two effects; the gate separates them:

1. *Round-trip leg:* `ADiakopticsInit` rebuilds the coordinator from
   `Torn_Circuit/Master_Interconnected.dss` (a `save circuit` product). First compare
   **saved-interconnected-solved-normally** vs **original-solved-normally**. A gap here
   is a `save circuit` fidelity bug (or a WPG.20-era shape-save issue) — fix there, not
   in AD.
2. *AD leg:* compare **AD solve** vs **saved-interconnected normal solve**. Calibration
   procedure (once, in WP-AD.3, recorded in `tests/TOLERANCE_NOTES.md` §AD): (a) measure
   the gap on the three synthesized fixtures at the default `ConvergenceTolerance`
   (1e-4); (b) re-run both at 1e-10 — the gap **must collapse proportionally** (shared
   fixpoint proof; failure = port bug, stop and fix); (c) pin the tier at measured ×4.
   The tighten-proof (b) stays in the suite as a permanent test. The tier is never
   loosened afterwards (CLAUDE.md no-fudging rule).

**D8 — manifests and lifecycle.** Family decks stay oracle-gated exactly as today —
**no deck in a family manifest ever contains AD commands** (the oracle would error
#130). AD coverage is orthogonal: a mandatory `ad` field on every family-manifest case
plus `tests/corpus/manifests/ad_sweep.json` for the vendored corpus (WP-AD.4). Decks
that must run literal AD commands (tearing fixtures, export formats, statistics) live
as command sequences inside `crates/dss-core/tests/adiakoptics.rs`, not as corpus decks.

---

## §2 Part I — oracle-gated (pre-acceptance)

### WP-PF.1 — the `Pstcalc` command

**Port map.**

| Pascal | Rust target |
|---|---|
| `Pstcalc.pas` `PstRMS`/`_Pst`/`Get_Pinst`/`Set_Filter_Coefficients`/`SB`/`CalcPst`/`Gather_Bins`/`Sample_Shift`/`Init6Array` | new `crates/dss-core/src/support/pstcalc.rs` — a `PstEngine` struct owning what Pascal keeps in unit-level globals (`rms_reference`, filter arrays, bins, …); public `pub fn pst_rms(voltages: &[f64], freq_base: f64, cycles_per_sample: i32, lamp: i32) -> Vec<f64>` |
| `ExecHelper.DoPstCalc` (4778–4845) | `exec/` command handler replacing the `not_ported` arm for pointer 110; parser via the existing `PstCalcCommands`-equivalent sub-command table (`Npts`,`Voltages`,`cycles`(=`dt`),`freq`,`lamp`); `Npts>10` guard message 28723; result via the FPC `%.8g` formatter into `GlobalResult` |

Port notes (all visible in §0.1): unit globals become struct fields; `input_type = 6`
always (the 0/1/3 branches of `internal_reference` are dead but port the constant table
verbatim with a comment); `CyclesPerSample` from `dt` is
`Round(Frequency·dblvalue)` — **solution** frequency, not `DefaultBaseFreq`; the
warm-up loop runs `time < 30.0` then `PST_Start_Time = time + 5.0`; bins are f64.

**Tests.**
- Unit: interval count (`NumPstIntervals = max(1, trunc(N·ΔT/600))`), lamp/freq
  coefficient selection, monotone warm-up decay, a constant-1-pu input → Pst ≈ 0.
- Golden `tests/golden/pstcalc/cmd_results.json` via new `tools/golden/gen_pstcalc.py`:
  drive the pinned oracle's Text interface with a matrix of `Pstcalc` calls
  (npts ∈ {12, 700, 1900}, modulated-sine and step voltage arrays generated in-script,
  lamp ∈ {120, 230}, freq ∈ {60, 50}, dt variants) and capture `GlobalResult` strings;
  Rust test replays identical commands and compares **byte-exact**.
- Family deck `tests/corpus/modes/pstcalc_cmd.dss` (trivial circuit, solve, one
  `Pstcalc npts=… voltages=[…] dt=1 freq=60 lamp=120`) with `compare_global_result:
  true` in `modes/manifest.json` — live oracle gate (follow the WP8.8
  executive-tail deck pattern).

**Exit:** command un-refused; goldens + family deck green; probes from D4 recorded in
STATUS.

### WP-PF.2 — Monitor mode 4 (flicker + Pst channels)

**Port map.**

| Pascal | Rust target |
|---|---|
| `TakeSample` mode-4 body (1252–1263, 1479, 1560–1562) | `monitor/sample.rs`: fill a `flicker_buffer: Vec<Complex64>` from `NodeV[node_ref[i]]`, convert to polar (reuse the existing polar helper), push `2·nphases` values (mag, ang) — f32 narrowing happens in `mon_buffer` as everywhere else |
| `PostProcess`/`IsProcessed` (1136–1144) + `CloseMonitorStream` trigger (1101–1116) | `monitor/mod.rs`: `post_process()` with an `is_processed` latch, invoked wherever the port closes/exports the stream (`to_csv`, `export monitor`, `show monitor` — mirror the Pascal call sites; also the `Process`/`ResetIt` action semantics at 289–304) |
| `DoFlickerCalculations` (1602–1688) | same module: read `flushed` records (time = `sec + 3600·hour`, f32), per phase run `FlickerMeter`, rewrite channels in place: `ch[2p−2] ← flicker`, `ch[2p−1] ← pst[ipst]` with the exact `ipst`/`tpst` stepping (Pst stays `0.0` for rows before the first completed 600 s interval); `Vbase = 1000·kv_base` of the metered terminal's bus |
| `FlickerMeter`/`Fhp`/`Flp`/`Fw1`/`Fw2`/`QuickSort`/`Percentile` (476–687) | `support/pstcalc.rs` (same module as WP-PF.1), **all-f32**, 1-indexed loops translated carefully (`y[1]:=0` seeds; `ts = pT[2]−pT[1]`; `cf = 1/1.285e-6`) |

Port notes: `data[p][i]` reads only the **odd** channel (magnitude) — angle is ignored
and overwritten; `Npst = 1 + Trunc(t_N/600)`; a monitor with <2 samples → `ts`
undefined — probe the oracle for the degenerate deck and mirror (likely garbage-in-
garbage-out but deterministic; if UB → guard per D5). Check the `Percentile` OOB probe
(D5) before transcribing.

**Tests.**
- Unit: each filter section vs hand-computed low-order responses (f32); percentile
  interpolation; a 600 s window count.
- Golden via `tools/golden/gen_pstcalc.py` (part 2): recreate the upstream flicker demo
  as a **driver with an explicit solve** — inline the `Examples/Matlab/pst.dss` circuit
  (vsource + `isource.pst` + `load.pst` model=2 with the `WindRmsV.csv` duty shape —
  the CSV is vendored in the corpus) + `solve mode=duty stepsize=10 number=8640`,
  capture the mode-4 monitor CSV from the oracle; Rust replays and compares channels at
  the harness monitor (f32) tier. Note: the loadshape uses `File=` → generation and the
  Rust replay depend on **WPG.19** (in flight in a parallel session) — if WPG.19 hasn't
  merged when this WP starts, use an inline-`mult` copy of the shape (8640 points is
  fine inline) and add the `File=` variant after the merge.
- Family decks `tests/corpus/controls/monitor_pst.dss` + `controls/midi_monitor_pst.dss`
  (small 3-phase feeder; duty `stepsize=1 number=1300` with a flickering inline shape —
  long enough for 2 complete Pst intervals; monitor `mode=4`; `export monitor`) with
  `check_meters_monitors: true` — live oracle gate.

**Exit:** mode 4 samples + post-processes; goldens + both family decks green; the
mode-4 line removed from the monitor module-doc deferral list.

### WP-AD.1 — incidence matrix + `Sparse_Math` (oracle-gated)

**Port map.**

| Pascal | Rust target |
|---|---|
| `Sparse_Math.pas` `Tsparse_matrix` + `Tsparse_Complex` | new `crates/dss-core/src/support/sparse_math.rs`: `SparseInt` + `SparseComplex` with the exact COO semantics — `insert` accumulates into an existing `(r,c)` cell else appends (insertion order = storage order; `multiply`/`Transpose` output orders must match the Pascal loops), `add`, `TransposeConj`, `Rank` (the row-echelon walk), `NZero/NRows/NCols`. `DssComplex64` throughout |
| `Solution.pas` 1340–1620 (`Upload2IncMatrix`, the four `Add*2IncMatrix`, `Calc_Inc_Matrix`, `Calc_Inc_Matrix_Org`, `get_IncMatrix_Row/Col`) + fields 202–232 | `solution/` new `inc_matrix.rs`: `IncMatrixState { inc_mat: SparseInt, laplacian: Option<SparseInt>, rows: Vec<String>, cols: Vec<String>, levels: Vec<i32>, … }`. Series-only filters for Capacitor/Reactor ports 1:1; `Calc_Inc_Matrix_Org`'s hierarchical ordering and `Inc_Mat_levels` are the delicate part — transcribe loop-for-loop |
| `ExecCommands` `CalcIncMatrix`/`CalcIncMatrix_O`/`CalcLaplacian` (406–433) | replace the three `not_ported` arms; `CalcLaplacian` keeps the NIL-guard message 8877 verbatim |
| `ExportResults.pas` 3310–3399 | `report/export/inc_matrix.rs`: five exports, byte-exact CSV (headers, `floattostr` formatting via the existing FPC-format helpers), wired to keywords 53–57 (drop their scoped `NOT_PORTED`) |

**Tests.**
- Unit: `SparseInt`/`SparseComplex` ops incl. the accumulate-vs-append `insert` and
  `multiply` ordering; a `Rank` case.
- Goldens (byte-exact, `golden_reports` family): `CalcIncMatrix_O` + all five exports
  over 4 corpus decks chosen to hit all four element walks — e.g. IEEE13
  (lines+transformers), a series-capacitor deck, a series-reactor deck, ckt24 (scale);
  plus `CalcIncMatrix` (flat) on one of them. Generated by extending
  `tools/golden/gen_reports.py` (or the family's existing generator) — the pinned
  oracle supports all of it.
- Negative: `CalcLaplacian` before any `CalcIncMatrix` → message 8877 (compare via
  `compare_global_result` or the error-channel assert used by existing report tests).

**Exit:** 4 commands + 5 exports un-refused and golden-pinned. (`Refine_BusLevels` is
AD-gated upstream → stays refused until WP-AD.5.)

---

## §3 Part II — A-Diakoptics (post-acceptance; start any time after MULTITHREADING M2)

### The algorithm in one page (orientation for every WP below)

*Initialization* (`set ADiakoptics=yes` → `ADiakopticsInit`, states 0–9; requires a
prior successful solve — the ckt24 header says so and `Tear_Circuit` reads `NodeV`):

0. `ADiakoptics_Tearing(AddISrc=FALSE)`: snapshot mode + `controlmode=off` →
   `BuildYMatrix` → `Tear_Circuit()` (graph → partition → `Locations`/`Link_Branches` →
   zone EnergyMeters `option=R action=C` + `PConn_Voltages`) → `Save_SubCircuits`
   (`save circuit` → `Torn_Circuit/` → `Format_SubCircuits`) → restore mode.
1. Copy `Link_Branches` locally.
2. `ClearAll`; coordinator compiles `Torn_Circuit/Master_Interconnected.dss`,
   `controlmode=off`, disables `zone_*` EnergyMeters, builds Y, **solves**; for each
   zone k≥2 create child engine, compile `Torn_Circuit/[zone_k/]Master.dss`, child 2
   keeps everything, children >2 disable their inherited link branch, `controlmode=off`,
   solve (abort → error).
3. Coordinator: disable all link branches, rebuild Y (the "torn" interconnected Y).
4. `Calc_C_Matrix` (contours: +1/−1 per phase column per link, node lookup by name
   substring; link must parse as `line.` else error "not lines").
5. `Calc_ZLL` (per link: extract 3×3 LinkPrim from the link's Yprim, `Invert`, place on
   the block diagonal).
6. `Calc_ZCC`: for each contour column c: solve `Y_torn·z = c` (per-column
   `SolveSparseSet` ≡ `dss-sparse` `solve()` with the cached factorization) → `ZCT`;
   `ZCC = Contoursᵀ·ZCT + ZLL`.
7. `Calc_Y4 = ZCC⁻¹` via dense `CMatrix::invert` (with the D5 drop quirk).
8. `SendIdx2Actors`: per-child `VIndex` (offset of the child's bus 1 in the
   interconnected node list); zero-init `Ic`.
9. Statistics + re-enable link branches + rebuild Y + send `INIT_ADIAKOPTICS` to
   children (`Start_Diakoptics`: disable feeder-head PDE for children >2, disable
   artificial VSources; `IndexBuses`: `LocalBusIdx`, `AD_IBus`, `AD_ISrcIdx`) → set
   `Solution.ADiakoptics := TRUE`.
   Any error → flag stays FALSE and the summary reports it (`GlobalResult` = the
   progress string, ported verbatim).

*Per iteration* (coordinator inside `DoNormalSolution`/`SolveDirect`/`SolveYDirect`;
Newton is **not** AD-aware):

- `SOLVE_AD1` to every child: zero inj, source inj (+PC inj iff `ADiak_PCInj` — TRUE
  from DoNormalSolution, FALSE from Direct/YDirect — or dynamic/harmonic), Y check,
  solve **into the parent NodeV at the child's offset**.
- Coordinator: `Vpartial[i] = NodeV[Contours.CData[2i].Row+1] −
  NodeV[Contours.CData[2i+1].Row+1]` (one pair per contour column) → `Vpartial :=
  Y4·Vpartial` → `Ic := Contours·Vpartial`.
- `SOLVE_AD2` to every child: `UpdateISrc` (for each contour row present in this child:
  `Currents[AD_IBus[i]] += −Ic[row]`) and solve again into the parent NodeV.
- Coordinator `Converged` runs over the interconnected NodeV as usual.

### WP-AD.2 — tearing (`Tear_Circuit` + the partitioner + torn files)

**Scope.** Circuit AD fields (§0.1); `Create_MeTIS_graph` 1:1 (needs WP-AD.1's
`Calc_Inc_Matrix_Org` + Laplacian; the parallel-branch dedup loop and the
Transformer-weight-1 rule verbatim); `support/partition.rs` per **D2**;
`Create_MeTIS_Zones` parsing 1:1 (including the D5 line-swap quirk, the ≥2-bus zone
rule, the final `inc(Locations[j])`); `Tear_Circuit` (terminal orientation by |V|
difference, `PConn_Voltages`, zone meters); `Save_SubCircuits`/`Format_SubCircuits`/
`AppendIsources`/`Disable_All_DER`; register the `Tear_Circuit` command (dispatch →
`ADiakoptics_Tearing(DSS, False)` = tearing without ISources).

**Tests** (`crates/dss-core/tests/adiakoptics.rs`, rust-only):
- Two synthesized radial 3-phase feeders (a ~40-bus midi and a ~200-bus macro, inline
  shapes only) + `set Num_SubCircuits=2/3; Tear_Circuit`: assert zone count, link
  branches are 3-phase lines, zones connected and balanced (recompute from the partition
  file), `GlobalResult = "Sub-Circuits Created: N"`.
- Committed fixture golden of the emitted `Torn_Circuit/` tree for the midi feeder
  (byte-stable: fixed partition, fixed `%.8g`-class formatting) — this is the
  self-golden that pins D2's partitioner + `Format_SubCircuits` (incl.
  `Master_Interconnected.dss` filtering rules and per-zone `VSource.dss` values).
- Round trip: compile each `zone_k/Master.dss` and `Master_Interconnected.dss`; the
  interconnected model solves; zones solve standalone.
- Error paths: partition that would cut at a transformer → upstream "link branches are
  not lines" flow (surfaced later by `Calc_C_Matrix`, but the tear-level graph weights
  already bias against it — assert the ckt24-documented behavior); graph file for a
  1-zone request.
- Diagnostic (non-gating, `#[ignore]`d or log-only): diff our `ckt24` `.graph` against
  the vendored `ckt24_.graph` and report — expected to differ (different lineage), the
  report is inventory input only.

### WP-AD.3 — the AD engine (init, solve, options, exports)

**Scope.** `AdCoordinator` per **D3** (owned `children: Vec<Dss>`, message enum,
explicit parent-context arguments); `ADiakopticsInit` state machine 0–9 (incl.
`ClearAll`+recompile semantics, `zone_*` meter disable, link open/close + Y rebuilds,
error accumulation into the progress string, `Parallel_enabled`/`ADiak_Init` flags);
`Calc_C_Matrix`/`Calc_ZLL`/`Calc_ZCC`/`Calc_Y4` (quirks per D5; per-column solves via
`dss-sparse` cached factorization); `SendIdx2Actors`/`Start_Diakoptics`/`IndexBuses`;
`Solve_Diakoptics`/`SolveAD`/`UpdateISrc`/`VoltInActor1`; the solve-path branches
(`DoNormalSolution`, `SolveDirect`, `SolveYDirect`, `SolveCircuit` Y-skip, `Converged`,
`VDiff`, `SolveSystem` offset write, `CheckControls` AD branch + `GetCtrlMode`);
`set/get Coverage|Num_SubCircuits|ADiakoptics|LinkBranches` (get-only for
LinkBranches!); `Export ZLL/ZCC/Contours/Y4` (register 58–61, formats per
`ExportResults.pas:3401–3485`, silent no-op when `ADiakoptics=false` — 1:1);
`get_Statistics`; `Solve` resetting `AD_Init`.

**Tests** (`tests/adiakoptics.rs`):
- Fixture goldens for `Contours`/`ZLL`/`ZCC`/`Y4` on the midi feeder (2 zones), with
  the D1 invariants recomputed in-test (ZCCᵀ reconstruction, `‖Y4·ZCC − I‖` bound
  modulo the D5 drop quirks — assert the dropped-entry pattern explicitly).
- Export format tests for all four (against the committed fixtures).
- **Equivalence gates** per D7 on three synthesized fixtures (midi snap, midi
  daily-24-step, macro yearly-168-step): round-trip leg + AD leg, tier calibrated and
  recorded in `TOLERANCE_NOTES.md` §AD; plus the permanent tighten-tolerance proof test
  (both engines at 1e-10 → gap collapses).
- Options/lifecycle: `set ADiakoptics=yes` without a prior solve; `=no` clears the flag
  only; `get ADiakoptics/LinkBranches/Num_SubCircuits`; a failed init (non-Line link)
  leaves `ADiakoptics=false` and reports the state-machine summary.
- `get_Statistics` string golden (fixed zones → fixed reduction/imbalance numbers,
  `%4.2f` formatting).

### WP-AD.4 — the corpus-wide AD ↔ normal sweep (the user-mandated gate)

**Requirement.** *Every* deck in `tests/corpus/asymmetric`, `tests/corpus/controls`,
`tests/corpus/modes` and every `solvable_now` entry point of
`tests/corpus/electricdss-tst` must carry an explicit A-Diakoptics disposition, and
every eligible one is solved both ways and compared. New unconditional test
`corpus_ad_matches_normal_mode` in `corpus_live.rs` (rust-vs-rust — the oracle is not
involved, so it runs everywhere `cargo test` runs).

**Disposition vocabulary** (`ad` field):
- `"full"` — run the case normally and with the AD preamble; compare node voltages,
  element currents/powers **and** monitors/eventlog at the §AD tier.
- `"pf"` — same, but both runs force `controlmode=off` for the compared solve (the
  physics-only comparison; used where control-action placement vs zone boundaries is
  not zone-local).
- `"off:<reason>"` — not run; reason is mandatory and specific (see catalogue).

**Mechanics.**
- Family manifests: add the mandatory `ad` field to **all** cases (loader errors on a
  missing field — same spirit as the `pending`/`wp` discipline).
- Vendored corpus: new `tests/corpus/manifests/ad_sweep.json` mapping every
  `solvable_now` path → disposition; a bijection test (extend `corpus_manifest.rs`)
  asserts `ad_sweep.json` covers exactly the `solvable_now` set, so future decks can't
  skip classification.
- Runner: reuse the existing per-case runner; the AD arm replays the deck up to its
  final solve, inserts `set Num_SubCircuits=2` + a snapshot solve (if the deck's own
  base solve isn't one) + `set ADiakoptics=yes`, then re-issues the final solve —
  the ckt24 recipe. Output dirs are per-case temps (Torn_Circuit lands there).
- An `ad: full|pf` case whose AD init **fails** at runtime = test failure (the
  disposition is a promise, like `pending`), with the init summary in the message.

**Initial disposition catalogue** (starting point; WP-AD.4's work is largely moving
decks from `off:unclassified` upward, deck by deck, with evidence):
- `off:mode-outside-AD-scope` — dynamics, harmonics (incl. HarmonicT), faultstudy,
  Monte1/2/3, MonteFault, LD1/LD2, AutoAdd, `Reduce`-family decks (topology-mutating),
  `peakday`/duty-with-controls-that-open-links. Upstream AD is a power-flow
  accelerator (snapshot/direct/time-series); this is 1:1 scope, not a shortcut.
- `off:too-small` — micro decks whose graph can't yield 2 connected ≥2-bus zones with
  a 3-phase-Line cut (most `*_asym.dss` micros; the `midi_*` decks mostly qualify for
  `pf`).
- `off:non-3ph-cut-only` — feeders whose only cut candidates are 1/2-phase lines or
  transformers (D5 ZLL constraint).
- `pf` — the default target for `midi_*` and macro power-flow decks in all three
  families and for the electricdss-tst radial feeders (IEEE13/34/37/123, ckt24
  original master, the EPRI DPV J1/K1 if runtime allows — budget check first).
- `full` — control decks whose controlled element + controller + monitored element
  provably land in one zone at `Num_SubCircuits=2` (start with
  `regcontrol_*`/`capcontrol_*` on the synthesized feeders; promote with eventlog
  equality as the proof).
- The pre-torn `ADiakoptics/*/Torn_Circuit/**` decks: `off:already-torn-artifact`.

**Exit:** zero `off:unclassified`; the sweep green in `cargo test`; sweep population
counts recorded in STATUS (like the solvable_now counts).

### WP-AD.5 — `AggregateProfiles`, coverage paths, ckt24, EPRI probe, exit

- Port `Get_paths_4_Coverage`/`get_longest_path`/`Append2PathsArray`/`Normalize_graph`
  + the `Refine_BusLevels` command and `Coverage`/`Actual_Coverage` options;
  `Disable_All_DER`; `AggregateProfiles` (+ its `Aggregate`-command registration),
  fixture-golden for the emitted `Aggregated_model/` on the midi feeder.
- ckt24 driver test (in `adiakoptics.rs`, reading the vendored deck): compile the
  original `master_ckt24.dss` **prefix** (up to the normal yearly block), then
  auto-tear at `Num_SubCircuits=2` and `4`, AD-solve `mode=yearly number=24`, compare
  vs the normal solve per D7 (this is the real-corpus macro gate; 168-step variant
  behind `DSS_EXPENSIVE_TESTS=1` if runtime demands).
- EPRI channel probe (opt-in, report-only, never gating): determine whether the
  official r3723/r4088/r4133 binaries under `tools/opendss/` expose
  `set ADiakoptics` + ship `kmetis.exe`; if yes, add an `ab_compare.py` AD case and
  record findings in `tools/opendss/README.md` + `known_diffs.json`; if no, record
  that. Either way STATUS gets the inventory note.
- Exit sweep: no `NOT_PORTED`/refusal left on the §0.1 surface; `help_catalog.rs`
  Tear_Circuit "not supported" note updated to reflect the Rust-side support;
  `NOTE(subst-metis)`/`NOTE(upstream-quirk)` markers greppable and inventoried in
  STATUS; PORTING_PLAN Phase-9 checklist, MULTITHREADING_PLAN pointer and
  PLAN_SEQUENCE cross-checked.

### WP-AD.6 (stretch, optional) — threaded children over M2

After MULTITHREADING M2: fan `SOLVE_AD1`/`SOLVE_AD2` across scoped threads (children
are already owned, message-driven, barrier-synchronized). Gate: **bitwise** equality of
the full AD sweep vs the sequential path at any thread count + the M2 determinism
rules. If bitwise fails for any reason, children stay sequential (valid outcome — the
escape protocol, not a failure to hide).

---

## §4 Model tiers (protocol: `PLAN_SEQUENCE.md` §Model-tier protocol)

Audit tier applies to **both** spawned auditors (`/audit-code` + `/audit-tests`).

| WP | Exec tier | Audit tier | Why |
|---|---|---|---|
| WP-PF.1 | `opus-medium+` | `opus-high+` | sequence-exact f64 filter cascade + FPC probes; mechanical once probed |
| WP-PF.2 | `opus-medium+` | `opus-high+` | f32 fidelity + in-place stream rewrite semantics |
| WP-AD.1 | `opus-medium+` | `opus-high+` | COO-ordering-sensitive sparse ops + hierarchical incidence ordering, but fully oracle-pinned |
| WP-AD.2 | `opus-high+` | `opus-high+` | partitioner design (D2) + multi-file emission fidelity, self-goldened |
| WP-AD.3 | **`opus-xhigh`** | **`opus-xhigh`** | the design step: ownership inversion (D3), no-oracle numerics, D7 calibration |
| WP-AD.4 | `opus-medium+` | `opus-high+` | sweep mechanics + per-deck triage at fixed contracts |
| WP-AD.5 | `opus-medium+` | `opus-high+` | AggregateProfiles is sizable but fixture-gated; probe work is inventory |
| WP-AD.6 | `opus-high+` | `opus-high+` | threading with a bitwise gate over existing machinery |

## §5 Forbidden moves (any WP)

- No tolerance loosening to admit a failing comparison — oracle-facing (Part I) or
  AD↔normal (Part II). Calibration happens once per D7 and only tightens after.
- No external binaries (kmetis or otherwise) in the engine, tests, or CI. D2 is final.
- No reproduction of UB-class quirks (D5 table is the authority; when in doubt, probe
  first and bring the evidence to the audit).
- No `Arc<Mutex<Dss>>`, no shared engine state between coordinator and children —
  ownership + explicit arguments only (D3; also MULTITHREADING's M2 rule).
- No test that depends on `available_parallelism` (D6).
- Don't "fix" the D5 reproduce-list quirks (Y4/ZCC drops, the zones line swap, the
  substring lookup) — they are the spec; fixtures pin them.
- No family-manifest deck containing AD commands (D8) — the oracle leg must stay green.
- A gap between AD and normal mode is a **bug until proven otherwise** (CLAUDE.md
  discipline); "the partition differs from upstream" is never an accepted explanation
  for a physics mismatch — partitions change *which* zones exist, not the fixpoint.

## §6 Sequencing (mirror of `PLAN_SEQUENCE.md`)

- **Part I** (WP-PF.1 → WP-PF.2 → WP-AD.1; order free, they are independent): porting
  stage 3, after the GAPS follow-ups (WPG.19/20) land — WP-PF.2's corpus-demo golden
  touches WPG.19's `File=` arrays, so sequencing after it avoids churn. Part I is
  **inside final-acceptance scope** (all of it is oracle-visible surface).
- **Part II** (WP-AD.2 → AD.3 → AD.4 → AD.5 → optional AD.6, strictly in order):
  post-acceptance, the last stage; early-start allowed as soon as MULTITHREADING **M2**
  has landed (M3/M4 are not prerequisites). Rationale: no oracle gate ties it to
  acceptance; DE_PASCALIZE/Stage-F/RESONANCE would otherwise double-touch the solve
  paths AD branches into; M2 settles the actor/ownership idioms AD reuses.
- Dependencies into this plan: WP-AD.2 needs WP-AD.1 (incidence + Laplacian);
  WP-AD.3 needs WP-AD.2; WP-AD.4 needs WP-AD.3; WP-AD.6 needs M2.
- Dependencies out: none — no later plan consumes AD. `MULTITHREADING_PLAN` M2's
  "A-Diakoptics out of scope" note points here.

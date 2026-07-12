# FINAL ACCEPTANCE (PORTING_PLAN §6) — detailed record

> **Archived verbatim from `STATUS.md` on 2026-07-12** to keep the living handoff lean. The referee
verdict, the §6 criterion→evidence table, the named non-blocking residuals, the
FA settle findings, and FA fix 1/2/3 are frozen here; `STATUS.md` keeps the
condensed verdict + residual list with a pointer to this file.

**FINAL ACCEPTANCE (PORTING_PLAN §6) EXECUTED 2026-07-11, on explicit user
request.** A max-effort referee round on branch `final-acceptance` (HEAD after the
3-branch fix round + FA settle) returned `criteria_met=true`, `blocking_items=[]`.
Verdict (quoted): *"ACCEPT — PORTING_PLAN section-6 criteria are met; all three
prior blocking items (B1/B2/B3) are resolved with evidence I independently
re-verified and re-ran in `…/worktrees/final-acceptance` (range de9630b..27d73e1)."*
The §6 criteria map to evidence as:

| §6 criterion | evidence (this branch, re-witnessed at settle) |
|---|---|
| all `cmd_coverage.py`-covered electricdss-tst cases through both engines, **zero out-of-tolerance**, **exact discrete state** | `corpus_live` 14/14 pass, 0 failed (~174 s): `corpus_live_solvable_cases_match_oracle` live-compares all **245 solvable_now** decks (full Y/V/I/P + injection + discrete state per step) at the calibrated floors; the 3 family suites (asymmetric/controls/modes) pass; `corpus_manifest` bijection PASS — **every** corpus `.dss` is in exactly one manifest, so nothing hides |
| `save_roundtrip` green on IEEE 13/34/37/123/8500 | `save_roundtrip` **6/6** (13/34/37/123/8500 + structural file-set): discrete reg taps + cap banks and warm-re-solve iteration count **exact** on every deck incl. 8500; 8500 node-V band 3e-4 is an oracle-proven `Save circuit` floor (probe below), not a slack |
| export-diff suites green on IEEE 13/34/37/123/8500 | `golden_reports` **193/0** — pinned-oracle goldens for IEEE 13 (V/I/P + seq + per-element) and 34/37/123/8500 |
| gate | `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -D warnings` clean; `cargo test --workspace` exit 0 (pinned dss-python oracle 0.15.7 / dss_capi 0.14.5) |

Gated population: **245/335 solvable_now (73.1%)**. cmd_coverage's unported tail is
exclusively Phase-9 actor/parallel mode (SolveAll/NewActor/Clone/Abort +
ActiveActor/CPU/Parallel options) + `CapControl.ControlSignal` — explicitly outside
1:1 acceptance (PORTING_PLAN §"stopping before Phase 9 = complete simulator").


---

**Named non-blocking residuals** (all documented, bounded, correctly classified,
**outside** solvable_now — the acceptance names them):
- **RegControl/LDC `SubXFMR` family** (port-side, tagged `live_mismatch` "BUG until
  a floor is proven"): ckt24 `Run_Ckt24`/`master` + 5 MemoryMapping siblings +
  `IEEE13_Assets` (~1.0e-3) + Version8-CIM `IEEE13_CDPSM` (4.6e-6). Owner: root-cause
  post-acceptance (UPGRADE/DE_PASCALIZE era).
- **LVTestCase/Master** — real gap: Rust leaves node entry 0 unenergized. Owner: same.
- **SecondaryTestCircuit_modified** (5.5e-1); **Storage-Quasi Run_Demo1** (1.1e-4);
  **GFM_IEEE8500** daily/snap ×3 (1.4e-4…2.7e-4 above band) + 1 oracle-nonconvergence.
- **Unported optionals** (skipped_unsupported): actor+parallel mode,
  `CapControl.ControlSignal`, UTF-8-BOM strip, 14 deferred IEEE123-GFM-trajectory
  decks. Owners: MULTITHREADING_PLAN M2+ (actor); GAPS follow-up (ControlSignal/BOM).
- **Oracle-side blocks** (oracle_timeout/nonconvergence ×~40) and the known
  **VSConverter GetCurrents self-alias** upstream bug (gated via
  `exec/tests/vs_converter.rs`) — nothing to fix port-side.
- **A-Diakoptics Part II** — deliberately outside final acceptance; early-start was
  user-ordered (2026-07-11), owner DIAKOPTICS_PSTCALC_PLAN Part II.


---

**FA settle — audit findings settled (2026-07-11).** Six Minor findings from the
code/tests audits; none contradicted a §6 criterion. Three fixed, two recorded as
deliberate no-fix, and #6 folded into the #3 fix:
- **#1 (8500 save-floor proof prose-only, dangling citation) — FIXED.** Committed a
  reproducible oracle-side probe `tools/golden/probe_save_roundtrip_8500.py` (pinned
  dss-python) that reproduces the floor from the repo bytes: worst node `SX3312692A.1`
  **2.022253e-4** (< the 3e-4 band), total power −11983.486783 → −11983.420712 kW,
  iterations 2/2 exact, 8354/8531 nodes over 1e-6. Recorded the numbers in a new
  `tests/TOLERANCE_NOTES.md` §"`Save circuit` round-trip floor — IEEE-8500" and fixed
  the `save_roundtrip.rs` doc citation to point at it + the probe.
- **#3/#6 (population_lock fingerprinted membership+counts only) — FIXED.** Extended
  `population_lock.rs` to fingerprint **per-case rigor** for every solvable_now deck
  (kind/tolerance-tier, oracle target, n_steps, and every compare-depth flag —
  selected_elements/meters-monitors/probes/variables/eventlog/ctrlqueue/all-properties/
  global-result/autoadd-log/pending/solve-abort) **and** the three families' **path
  lists** (not just counts). Regenerated `population.lock.json`; verified the guard
  now trips with a precise per-field diff on an in-place kind flip (feeder →
  large_near_ideal_source), closing the "retained deck weakened in place" gap.
- **#5 (CIM Breaker check was a weak substring) — FIXED.** The B1 regression now
  asserts **exactly one** `<cim:Breaker>` element and **no** Fuse/Recloser
  misclassification (was `xml.contains("cim:Breaker")`).
- **#2 (fix1 commit-message over-generalized the +16 parks) — NO-FIX (recorded).**
  Cosmetic; rewriting a merged commit message is not warranted. The per-deck manifest
  tags/notes are individually honest and correctly differentiated (referee-confirmed).
- **#4 (DSS_UPDATE_POPULATION_LOCK regen arm) — NO-FIX (recorded).** Confirmed the var
  is **not** set in `.github/workflows/ci.yml` (only CARGO_TERM_COLOR +
  DSS_ORACLE_TIMEOUT_SECS); this is the documented deliberate-regen path. Keep it out
  of any future automated env block.

Post-acceptance the `TODO(compat)` sweep + goldens regen run in one dedicated pass
(PORTING_PLAN §4.1/§6); the named residuals are root-caused in the upgrade/refactor
eras (PLAN_SEQUENCE stages 4–8).

**FA fix 2 — divergence root-cause: DOCTechNote ×4 + GFMSnap (2026-07-11),
gate-green.** Root-caused the five above-`large` Rust↔oracle divergences the
WP8.8 classify left `needs_investigation` (per CLAUDE.md §"conditioning" +
§"cancellation floor" — decomposition, no tolerance fudging). Per-case verdict:

| case | verdict | evidence |
|---|---|---|
| `DOCTechNote/1_1` | **floor** → solvable_now `large_floating_zeroseq` | common mode 2.39e-3 V; L-L rel 1.36e-10; 0/1170 nodes fail `large` after per-bus shift removal |
| `DOCTechNote/1_2` | **floor** → same | common mode 2.39e-3 V; L-L rel 1.29e-10; 0/1170 fail |
| `DOCTechNote/2_1` | **floor** → same | common mode 1.81e-3 V; L-L rel 1.30e-10; 0/1170 fail |
| `DOCTechNote/2_2` | **floor** → same | common mode 2.85e-3 V; L-L rel 1.54e-10; 0/1170 fail |
| `GFMSnap` | **already floor-proven, gate-green; re-verified** | L-L rel 2.27e-10 confirms §floating-delta's 2.3e-10; 0/284 fail; gap 9.03e-5 V; no manifest change |

Root cause (DOCTechNote ×4): the decks `Redirect` LVTestCaseNorthAmerican
`Master.dss` (delta-delta substation + delta-primary distribution
transformers → the whole 13.8 kV MV system floats in zero-seq, already a proven
`large_floating_zeroseq` member) and add one perturbation each — an LV SLG fault
(1_1/1_2), a network-protector breaker open (2_1), or OC relays + an MV L-L
fault under `controlmode=event` (2_2). None adds an MV zero-seq ground path, so
the MV common mode stays un-pinnable solver junk. Decomposition (full 1170-node
dump both engines): the gap is 100% per-bus zero-sequence common mode
(1.8e-3…2.85e-3 V, well inside the 3e-2 band; ~2.2e-7…3.6e-7 rel at 8 kV) — the
L-L (differential) voltages agree to ≤1.4e-10 rel and removing each bus's mean
shift leaves **0/1170** nodes above `large`. Un-pinnable, not iteration-driven:
at `ConvergenceTolerance 1e-10` the gap is byte-unchanged while Rust can no
longer converge (residual floors out in the near-null direction); at the default
tolerance both engines converge in the identical iteration count. Y
bit-identical, injections match, iterations equal, element currents/powers
within `large` — verified by the `run_and_compare` full compare passing at
`large_floating_zeroseq`. Recorded: tests/TOLERANCE_NOTES.md §floating-zeroseq
(new DOCTechNote bullet), harness `tol_for` comment. No tolerance/tier value
changed (reused the existing `large_floating_zeroseq` band); no `TODO(compat)`
(floors are not compat quirks). solvable_now 226 → **230**;
needs_investigation 18 → 14.

**FA fix 3 (anti-shrink guard + §6 suite completion, 2026-07-11).** Two FINAL
ACCEPTANCE gaps closed. (1) **Anti-shrink guard**: `tests/corpus/manifests/
population.lock.json` (committed fingerprint — per-manifest case counts, the full
sorted `solvable_now` path list, the 3 family case counts) + `population_lock.rs`
(unconditional `cargo test`) that fails on any drift with a diff + the one-command
regen `DSS_UPDATE_POPULATION_LOCK=1 …`, so a silent reclassification of a deck out
of `solvable_now` can no longer stay green. Base counts: solvable_now 226, family
36/57/40. Documented in TESTING.md §Anti-shrink population lock. (2) **PORTING_PLAN
§6 literal suites**: `save_roundtrip.rs` now covers IEEE 34 + 8500 (were 13/37/123);
`golden_reports.rs` adds Voltages/Currents/Powers export-diff on IEEE 34/37/123
(were 13/8500) with 9 pinned-oracle goldens (`gen_reports.gen_extra_feeder_reports`).
Two proven floors (not weakening, decomposition per CLAUDE.md): (a) the **8500 save
round-trip** node-V floor is 3e-4 rel — **inherent to OpenDSS `Save circuit`**, not
the port: the pinned oracle's own Save→recompile→resolve reproduces the identical
worst node (`sx3312692a.1`, 2.022e-4) and pre/post total power to the digit; the
gate stays strong via exact iteration count + exact discrete state (12 reg taps +
10 cap banks); (b) the **Currents export** magnitude/angle floors mirror the
always-on `corpus_live` feeder current tolerance (`i_rel=1e-7`, `i_abs=1e-5` A) —
lightly-loaded phases carry near-cancellation mutual currents (IEEE123 L49 phase-2
= 3.45 mA vs 9–18 A) whose angle is pinned only above `i_abs/sin(0.005°)≈0.12 A`.
Gate: fmt + clippy clean, `cargo test --workspace` exit 0.


---

**FA fix 1 (Relay/CDPSM panic + manifest re-sweep) — 2026-07-11, branch `fa-fix1`.**
FINAL ACCEPTANCE referee items.

- **The bug:** exporting CIM100 for any deck with a **Relay**-controlled switch
  (e.g. `Examples/CIM/IEEE13_CDPSM.dss`) panicked
  `unreachable!("Relay has no double property 6")`. Root cause is caller-side, not
  a Relay accessor gap: `cim/export.rs::parse_switch_class` refactored Pascal's
  per-class `ParseSwitchClass` (`ExportCIMXML.pas:451`) into one closure that read
  `get_f64(6)` for **every** matched control class. Fuse prop 6 is `RatedCurrent`
  (a double — correct), but Relay prop 6 is `PhaseCurve` (a curve reference), so the
  read hit the accessor's `unreachable!`. Pascal reads `RatedCurrent` **only** inside
  the Fuse branch; Relay→`Breaker`/Recloser→`Recloser` are pure class-match checks.
  Fixed 1:1 (the closure now returns the matched `ElemRef`; only the Fuse branch reads
  prop 6). Regression: `cim::tests::parse_switch_class_relay_does_not_read_relay_double`
  drives `export cim100` on a minimal Relay-guarded switch.
- **Why WP8.5b's property-parity sweep missed it:** that sweep checks each class's
  accessor covers its *own* property list — the Relay table is correct (Relay genuinely
  has no double at prop 6). The panic is a **cross-class** caller reading a Fuse property
  index on a Relay object, which a per-class parity sweep cannot catch.
- **Manifest re-sweep:** ran the `DSS_LIVE_CLASSIFY=1` classifier over the 33 stale-tagged
  `skipped_unsupported` candidates (excluded the 14 `deferred=ieee123-gfm-trajectory-scope`
  scope-deferrals + the actor `SolveAll` deck; hands-off decks untouched). **15 converge
  clean + full-model compare green → `solvable_now`** (Dynamic_KundurDynExp ×2, ExpControl,
  InductionMachine ×2, InverterTechNote kWRated, Matlab/pst, StoCtrl_SeasonTarget ×2,
  UPFC_test_3, civinlar regulator, 123Bus SolarRamp ×3, 8500 P174_360kW_PV). **16 above-band
  / real-divergence → `skipped_needs_investigation`** with honest notes (ckt24 + 5
  memory-mapping = the known RegControl/LDC SubXFMR family; IEEE13_Assets + IEEE13_CDPSM =
  RegControl/LDC; GFM_IEEE8500 ×4; Storage-Quasi Demo1; LVTestCase Master = Rust entry-0
  unenergized, a real gap; SecondaryTestCircuit_modified). **2 stay `skipped_unsupported`**
  with refreshed tags (CapControlFollow = `CapControl.ControlSignal` unported;
  Source012Test = UTF-8 BOM not stripped, a parser gap, flagged for follow-up).
  - **IEEE13_CDPSM did NOT go to `solvable_now`**: the task-1 fix removes the panic and the
    deck now compiles/solves/exports CIM100, but the live compare is above-band (step 0
    entry 6 node V |diff|=4.60e-6 > allowed 1.599e-6). Per the no-fudging rule it is a
    divergence-to-investigate, not a green case.
  - `solvable_now` 226→**241 (71.9% of entry points)**; `COVERAGE.md` regenerated.


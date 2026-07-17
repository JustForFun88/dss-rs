# Test-triage: stale-skip promotions (2026-07-17)

Re-triage of corpus skip entries parked when the pinned dss_capi **0.14.5** oracle
could not handle a deck, now that the **capi015** oracle (dss-python 0.16.0b2 /
dss_capi 0.15.0b4, OpenDSS SVN r4103, `tools/opendss/.venv`) and the WP-U1.7 NCIM /
WP-U1.8 WindGen features exist in the port. Each deck was **probed first**, then
moved only if proven per UPGRADE_PLAN.md §1.7.

Probe harness: `tools/oracle/oracle_server.py::run_case` (the exact code the live
gate drives) via `DSS_ORACLE_ENGINE=capi015`; the Rust side via a throwaway
`dss-core` example (compile + warm re-solve, since the harness compares the
**warm** re-solve on both engines). Two-process determinism = the y-fingerprint
across two fresh capi015 processes.

---

## T2 — Kundur two-area (NCIM) — PROMOTED to `solvable_now` (`oracle: capi015`)

`Version8/Distrib/Examples/NCIM/Xmission_System_Kundur2Area/Master.dss`
was `skipped_oracle_issue` tag `oracle_error_#24713` ("Unknown Export command:
deltaf"). **Stale**: the 0.14.5 oracle predates NCIM and its `deltaF/deltaZ/
jacobian` exports.

Probe evidence (2026-07-17):
- **capi015 compiles+solves+converges.** `export deltaF/deltaZ/jacobian` run
  without error. Warm re-solve = **1 iter**, converged, 33 nodes / 20 elements.
- **Two-process bit-identical fingerprint.** Two fresh capi015 processes both
  give `y_fp` nnz=99, frob=4352.558077818025, tr_re=2002.050898338095,
  tr_im=-7402.642529188686, maxdiag=2511.186990142312 — identical.
- **Feature-sensitive.** Forcing `set algorithm=Normal` (post) on capi015
  **diverges** (|V|~8.1e115, 200 iters); NCIM converges. So NCIM cannot silently
  no-op — a fallback would blow up the live compare.
- **Rust matches capi015 to 0.** All 33 node voltages **byte-identical**
  (max abs diff 0.0, max rel diff 0.0), B1.1=11893.415545 (the ideal-EMF swing
  bus at 1.03 pu, no Thevenin droop = the NCIM signature). Rust warm re-solve
  **1 iter** — iteration policy Rust≤capi015 satisfied (1 == 1). Port raises no
  errors on the deck (NCIM exports ported WP-U1.7).

Verdict: **PROMOTE.** kind `large` (transmission NCIM network, stiff source
MVAsc3/1=1e6), n_steps 1, no `post` (deck sets `MaxIterations=200` itself).

## T1 — IEEE118Bus (NCIM) — NOT promotable (stays `skipped_needs_investigation`)

`Version8/Distrib/IEEETestCases/IEEE118Bus/master_file.dss` — the deck sets
`set algorithm=NCIM`; model=3 generators with `maxkvar/minkvar` Q-limits enforced
(`!IgnoreGenQLimits=Yes` commented out).

The orchestrator saw the port converge with "physically sane ~0.98 pu voltages" —
but that iterate is **`is_solved=false`** (a stalled NCIM iterate that merely
looks plausible). Full per-engine matrix (probed 2026-07-17):

| engine | result |
|---|---|
| pinned 0.14.5 | NO (no NCIM) |
| r3723 | NO |
| **capi015 (r4103)** | **NO — 100 iters, not converged** |
| **Rust port** | **NO — 100 iters** |
| official EPRI r4088 | YES (2 iters) |
| official EPRI r4133 | YES (2 iters) |

Proof it is not a port bug: Rust and capi015 stall at **byte-identical** node
voltages — 89_CLINCHRV=80072.708834, 1_RIVERSDE=76088.991977,
4_NWCARLSL=79514.988474. The port reproduces capi015's NCIM solver loop-for-loop
(`NCIMSolutionHelper.pas` r4103, `exec/tests/ncim.rs`), **including** its
non-convergence. This is the documented NCIM PV→PQ Q-limit **switching-cadence**
divergence between capi015 and r4088+ (DIVERGENCES.md "NCIM PV→PQ Q-limit
iteration count", report-only), fatal here at 118-bus scale.

Verdict: **DO NOT PROMOTE.** capi015 (the port's NCIM oracle) does not converge →
no live checkpoint exists → cannot gate on capi015. The old note's premise
("migratable once the 0.15.x solver is adopted") was wrong: the 0.15.x/capi015
solver also fails. Promotable only if the port later adopts r4133's NCIM switching
cadence (a future UPGRADE rung). Tag changed `oracle_nonconvergence` →
`ncim_pv_pq_switching_divergence`; note rewritten with the full matrix.

## T3 — WindGen GFL_Dynamics + QSTS — NOT promotable (stay `skipped_oracle_issue`)

`Version8/Distrib/Examples/WindGenerator/WindGen_GFL_Dynamics/Run_IEEE123Bus_GFLDaily.DSS`
and `.../WindGen_QSTS/Run_IEEE123Bus_GFLDaily.DSS` — were tag `oracle_error_#263`
("Object Type WindGen not found"). **Stale**: WindGen is ported (WP-U1.8) and
capi015 compiles+solves both (probed 2026-07-17: converged, 281 nodes / 244
elements each).

**New blocker: capi015 cannot gate a multi-step deck.** Both decks are inherently
multi-step:
- GFL_Dynamics: snapshot solve → `set mode=dynamics stepsize=0.001 number=200/70/2000`
  (fault at bus 51), plus `BatchEdit Load..* Daily=default`. Harness n_steps=1
  lands mid-dynamics (probed dbl_hour≈0.00119).
- QSTS: snapshot solve → `set mode=daily number=24` over a `WindData` daily
  loadshape. Harness n_steps=1 advances to dbl_hour=48.

`oracle_server.run_case`'s per-step capture **re-nominalizes** time-varying
elements (loads/shapes read step-0/nominal while `dbl_hour` advances) — the
recorded oracle-infra limit (DIVERGENCES.md "No live multi-step gate is
possible"; upgrade-rung1.md "Oracle-infra finding"), which is why all capi015
corpus cases are `n_steps=1`. Additionally WindGen carries the daily
Losses-staleness quirk (WP-U1.8 `windgen_daily` note) that would diverge the
full-model losses channel.

Verdict: **LEAVE SKIPPED, re-noted honestly.** Tag `oracle_error_#263` →
`capi015_multistep_limitation`. WindGen snapshot/daily/dynamics coverage is
already gated by the `modes/windgen` family (`windgen_snap/_delta/_daily/_dyn/
_dyn_fault`) + the `aerodynamic_wind_speed_sweep` unit test. Promotable only once
the oracle-infra multi-step re-nominalization fix lands.

---

## Manifest / lock changes

- `solvable_now.json`: +Kundur2Area (kind large, oracle capi015). count 292→293.
- `skipped_oracle_issue.json`: −Kundur2Area; WindGen ×2 re-tagged/re-noted.
  count 14→13.
- `skipped_needs_investigation.json`: IEEE118Bus re-tagged/re-noted (count
  unchanged).
- `population.lock.json` regenerated (`DSS_UPDATE_POPULATION_LOCK=1 cargo test -p
  dss-core --test population_lock`): the two count deltas + the Kundur rigor
  fingerprint.
- Stale `E:/RustProject/...` machine paths dropped from every note rewritten here.

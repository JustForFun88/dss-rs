# PLAN_SEQUENCE — the master ordering of all plan documents

One page answering "which plan runs after which." `PORTING_PLAN.md` stays authoritative for
the porting phases themselves; this file orders the *documents*, including everything after
final acceptance. Order confirmed by the user 2026-07-06.

```
── PORTING (pre-acceptance) ────────────────────────────────────────────────────────
 1. PHASE8_PLAN.md            ← CURRENT (branch phase-8-reporting; WP8.5 in progress)
 2. GAPS_PLAN.md              WPG.* long tail (incl. WPG.16 GIC, WPG.18 CIM XML) —
                              closes the remaining PORTING_PLAN Phase-9 scope
 ═ FINAL ACCEPTANCE (PORTING_PLAN §6) ═
── POST-ACCEPTANCE (refactor & improvement era) ────────────────────────────────────
 3. DE_PASCALIZE_PLAN.md      Parts I–III [A] (arenas/enums/de-indexing, bit-neutral,
                              proven by the still-stable goldens), then Stage F —
                              the `oracle-parity` feature split (absorbs the
                              TODO(compat) sweep; creates the two CI lanes)
 4. RESONANCE_PLAN.md         WP-R1 iterative refinement (default lane on, parity off —
                              needs Stage F), WP-R2 resonance analysis, WP-R3 diagnostics
 5. MULTITHREADING_PLAN.md    M0–M4 (actor mode, intra-solve rayon, faer parallelism) —
                              last, per PORTING_PLAN; M3 needs DE_PASCALIZE R2 arenas,
                              M3c needs Stage F
```

Early-start exceptions (allowed out of order because they are independent and cheap):
- `MULTITHREADING` **M0** (Send bounds + `assert_send::<Dss>()`) may land with
  DE_PASCALIZE R1 (it *is* DE_PASCALIZE P7).
- `MULTITHREADING` **M1** (criterion benchmark baseline) may land any time — it should
  exist *before* DE_PASCALIZE Part III to catch perf regressions there too.

Universal discipline: the **per-step ritual** (gate green → STATUS+commit → parallel
`/audit-code` + `/audit-tests` → STATUS review → stop and report in Russian) originates in
`PHASE8_PLAN.md` and is adopted verbatim by every subsequent plan (each carries its own
adapted copy; after DE_PASCALIZE Stage F "gate green" means **both CI lanes** + the
parity↔default differential job).

Supporting documents (not stages — referenced throughout):
- `CLAUDE.md` — conventions + the green gate (gets its two-lane update at Stage F).
- `STATUS.md` — living snapshot; `SPLITTING_RULES.md` — module-split protocol.
- `CORPUS_TEST_PLAN.md`, `CONTROL_COVERAGE_PLAN.md`, `tests/TOLERANCE_NOTES.md` — test
  infrastructure, woven through all stages.
- `PHASE4..7_PLAN.md`, `docs/phase-records/` — completed phases (historical).

Key cross-plan dependencies (why the order is what it is):
- **Stage F before RESONANCE & MULTITHREADING-M3c:** both need the lane split — the
  default build takes the improvement (refinement / parallel LU, iteration counts free to
  differ), the `oracle-parity` build keeps every 1:1 gate bitwise-green forever.
- **DE_PASCALIZE Parts I–III before Stage F:** the bit-neutral rewrites use the
  still-stable byte-exact goldens as their free equivalence proof; Stage F is the single
  default-lane re-baseline event.
- **DE_PASCALIZE R2 before MULTITHREADING M3:** typed arenas provide the disjoint `&mut`
  element access `par_iter_mut` needs; R2's riders create the compute/scatter seams M3b
  parallelizes.

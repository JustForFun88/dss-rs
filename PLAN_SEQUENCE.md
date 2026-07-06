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

## Model-tier protocol (binding for every plan below)

Tier vocabulary: **`opus-medium+`** / **`opus-high+`** / **`opus-xhigh`** — meaning "Opus
at that reasoning effort *or anything stronger*" (a stronger model at equal-or-higher
effort always qualifies). `PHASE8_PLAN` additionally uses **`sonnet-high+`** for its
deliberately Sonnet-executable porting WPs — same "or stronger" rule. Every execution plan carries a per-stage tier table
(**exec tier** = who may execute; **audit tier** = the model/effort the spawned
`/audit-code` + `/audit-tests` agents must run at).

Harness reality (do not over-promise): the main session's model **cannot switch itself**
mid-plan — only the user can (`/model` + effort). What IS automatic:
1. **Auditor override** — audits run as spawned subagents, and the ritual requires
   spawning them with an **explicit model/effort override** per the stage's audit tier
   (never "whatever the session runs").
2. **The refuse protocol (ritual step 0)** — before executing any WP, the executor
   compares the session tier against the stage's `exec` tier (the model name is in the
   system prompt; if the effort level is not visible, ask the user to confirm — mandatory
   for `opus-xhigh` stages). If the session is below tier, the executor must NOT attempt
   the work and must reply exactly:
   **«Этот шаг требует <exec tier>. Переключи сессию (/model + reasoning effort) и повтори
   команду.»** — under-tier execution fails safe instead of failing silently.

Tier map at a glance (full tables live in each plan; the audit tier always applies to
**both** spawned auditors, `/audit-code` and `/audit-tests`):
- **`opus-xhigh`**: DE_PASCALIZE **R1**, **Stage F**, **P15 item 2**; MULTITHREADING
  **M2**; RESONANCE **WP-R2**.
- **`opus-high+`**: DE_PASCALIZE R2, P10, P15 (rest); MULTITHREADING M3a/M3b/M3d/M4;
  RESONANCE WP-R1; GAPS **WPG.13** (GFM). Audits everywhere are `opus-high+` minimum,
  `opus-xhigh` on the xhigh-exec stages.
- **`opus-medium+`**: everything else in the post-acceptance plans (R0/R3, Part II,
  P8/P9/P11–P14, P3, M0/M1/M3c, WP-R3) — mechanical-with-guardrails: named pinning
  tests, forbidden-move lists, and the "when stuck: leave green, record in STATUS,
  surface" escape protocol.
- **Porting plans (`PHASE8_PLAN` §0, `GAPS_PLAN` §0-tiers)** carry their own per-WP
  tables: mostly `sonnet-high+` (deliberately Sonnet-executable, pre-validated decks),
  with `opus-medium+` on the non-mechanical spots (WP8.5 step 5, WP8.7; the numeric
  WPGs 4/5/6/9/10/12/15/16) and `opus-high+` on WPG.13.

Supporting documents (not stages — referenced throughout):
- `CLAUDE.md` — conventions + the green gate (gets its two-lane update at Stage F).
- `STATUS.md` — living snapshot; `SPLITTING_RULES.md` — module-split protocol.
- `CORPUS_TEST_PLAN.md`, `CONTROL_COVERAGE_PLAN.md`, `tests/TOLERANCE_NOTES.md` — test
  infrastructure, woven through all stages.
- `tools/opendss/README.md` — the **official-EPRI-binary oracle** (Oddie bridge,
  r3723/r4088/r4133; added 2026-07-07, opt-in, gates nothing). Infrastructure for a
  **future UPGRADE plan** — porting newer upstream OpenDSS behavior (r4088/r4133) after
  final acceptance; its `ab_compare.py` r3723↔r4088↔r4133 inventory is that plan's
  scoping input. The upgrade plan is not yet written and slots after final acceptance,
  ordered against DE_PASCALIZE/RESONANCE/MULTITHREADING when drafted.
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

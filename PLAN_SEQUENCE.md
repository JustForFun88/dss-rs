# PLAN_SEQUENCE — the master ordering of all plan documents

One page answering "which plan runs after which." `PORTING_PLAN.md` stays authoritative for
the porting phases themselves; this file orders the *documents*, including everything after
final acceptance. Order confirmed by the user 2026-07-06; UPGRADE_PLAN inserted as
post-acceptance stage 3 per the user's 2026-07-07 request (its WP-U0 infra pre-landed).

```
── PORTING (pre-acceptance) ────────────────────────────────────────────────────────
 1. PHASE8_PLAN.md            COMPLETE (branch phase-8-reporting; WP8.8 exit done)
 2. GAPS_PLAN.md              WPG.1–18 COMPLETE (2026-07-09); follow-ups WPG.19
                              (non-MM `File=` arrays) + WPG.20 (MMF-shape save)
                              ← CURRENT (in flight)
 3. DIAKOPTICS_PSTCALC_PLAN.md **Part I** (WP-PF.1 `Pstcalc` command, WP-PF.2
                              Monitor mode-4 flicker, WP-AD.1 incidence matrix +
                              Sparse_Math + exports 53–57) — the oracle-visible
                              half of the Phase-9 A-Diakoptics/Pstcalc residue;
                              runs after WPG.19/20 (WP-PF.2's corpus-demo golden
                              touches WPG.19's `File=` arrays). In acceptance scope.
 ═ FINAL ACCEPTANCE (PORTING_PLAN §6) ═
── POST-ACCEPTANCE (upgrade, then refactor & improvement era) ──────────────────────
 4. UPGRADE_PLAN.md           Rung 1 (WP-U1.*: dss_capi 0.15.x / r4088-line parity,
                              spec = .inputs/dss_capi_with_git@0.15.x, oracle capi015)
                              then Rung 2 (WP-U2.*: OpenDSS 11.0.0.1 / r4133 parity,
                              spec = Delphi diff, oracle oddie:r4133). Its WP-U0 test
                              infra (branch upgrade-test-infra: per-case `oracle`
                              manifest field, capi015 engine, iteration policy ≤) is
                              ALREADY LANDED (2026-07-07) so parallel porting branches
                              inherit it. Runs FIRST post-acceptance: freshest porting
                              context, avoids double-touching code DE_PASCALIZE would
                              refactor, and lets Stage F pin r4133-parity (not r3723).
 5. DE_PASCALIZE_PLAN.md      Parts I–III [A] (arenas/enums/de-indexing, bit-neutral,
                              proven by the still-stable goldens), then Stage F —
                              the `oracle-parity` feature split (absorbs the
                              TODO(compat) sweep; creates the two CI lanes; parity
                              target = r4133 per UPGRADE_PLAN §5)
 6. RESONANCE_PLAN.md         WP-R1 iterative refinement (default lane on, parity off —
                              needs Stage F), WP-R2 resonance analysis, WP-R3 diagnostics
                              (UPGRADE_PLAN §1.3-1 already grants target-rev cases the
                              iterations-≤ policy WP-R1 needs)
 7. MULTITHREADING_PLAN.md    M0–M4 (actor mode, intra-solve rayon, faer parallelism);
                              M3 needs DE_PASCALIZE R2 arenas, M3c needs Stage F
 8. DIAKOPTICS_PSTCALC_PLAN.md **Part II** (WP-AD.2–AD.6: A-Diakoptics tearing,
                              solve engine, the corpus-wide AD↔normal sweep,
                              AggregateProfiles, optional threaded children) — last.
                              **No oracle exists for it** (the pinned capi build has
                              `DSS_CAPI_ADIAKOPTICS` compiled out — errors #130), so
                              it gates rust-vs-rust (AD solve ≡ normal solve) and is
                              deliberately outside final acceptance. Early-start:
                              may begin as soon as MULTITHREADING **M2** lands
                              (M3/M4 are not prerequisites).
```

Early-start exceptions (allowed out of order because they are independent and cheap):
- `MULTITHREADING` **M0** (Send bounds + `assert_send::<Dss>()`) may land with
  DE_PASCALIZE R1 (it *is* DE_PASCALIZE P7).
- `MULTITHREADING` **M1** (criterion benchmark baseline) may land any time — it should
  exist *before* DE_PASCALIZE Part III to catch perf regressions there too.
- `DIAKOPTICS_PSTCALC` **Part II** may start right after MULTITHREADING **M2** —
  it does not wait for M3/M4 (its WP-AD.6 threading stretch is the only piece that
  consumes M2, and only optionally).

Universal discipline: the **per-step ritual** (gate green → STATUS+commit → parallel
`/audit-code` + `/audit-tests` → STATUS review → stop and report in Russian) originates in
`PHASE8_PLAN.md` and is adopted verbatim by every subsequent plan (each carries its own
adapted copy; after DE_PASCALIZE Stage F "gate green" means **both CI lanes** + the
parity↔default differential job).

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
divergence — far worse than stopping. This gate runs **ahead of the tier/refuse check
below** — a missing spec halts even a correctly-tiered session.

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
  **M2**; RESONANCE **WP-R2**; DIAKOPTICS_PSTCALC **WP-AD.3** (the no-oracle AD solve
  engine — audits there are `opus-xhigh` too).
- **`opus-high+`**: DE_PASCALIZE R2, P10, P15 (rest); MULTITHREADING M3a/M3b/M3d/M4;
  RESONANCE WP-R1; GAPS **WPG.13** (GFM); DIAKOPTICS_PSTCALC **WP-AD.2** (partitioner +
  torn-file emission) and **WP-AD.6**. Audits everywhere are `opus-high+` minimum,
  `opus-xhigh` on the xhigh-exec stages.
- **`opus-medium+`**: everything else in the post-acceptance plans (R0/R3, Part II,
  P8/P9/P11–P14, P3, M0/M1/M3c, WP-R3) — mechanical-with-guardrails: named pinning
  tests, forbidden-move lists, and the "when stuck: leave green, record in STATUS,
  surface" escape protocol. DIAKOPTICS_PSTCALC's remaining WPs sit here too:
  **WP-PF.1/PF.2, WP-AD.1** (pre-acceptance Part I) and **WP-AD.4/AD.5**.
- **Porting plans (`PHASE8_PLAN` §0, `GAPS_PLAN` §0-tiers)** carry their own per-WP
  tables: mostly `sonnet-high+` (deliberately Sonnet-executable, pre-validated decks),
  with `opus-medium+` on the non-mechanical spots (WP8.5 step 5, WP8.7; the numeric
  WPGs 4/5/6/9/10/12/15/16) and `opus-high+` on WPG.13.
- **`UPGRADE_PLAN` §3** carries the per-WP three-column table (exec / audit-code /
  audit-tests): `opus-high+` on WP-U1.7 (NCIM) and WP-U1.8 (WindGen/WTG3),
  `opus-medium+` on U1.1/U1.3/U1.4/U2.2/U2.3/U2.6, `sonnet-high+` on the rest;
  audits `opus-high+` everywhere.

Supporting documents (not stages — referenced throughout):
- `CLAUDE.md` — conventions + the green gate (gets its two-lane update at Stage F).
- `STATUS.md` — living snapshot; `SPLITTING_RULES.md` — module-split protocol.
- `CORPUS_TEST_PLAN.md`, `CONTROL_COVERAGE_PLAN.md`, `tests/TOLERANCE_NOTES.md` — test
  infrastructure, woven through all stages.
- `tools/opendss/README.md` — the **official-EPRI-binary oracle** (Oddie bridge,
  r3723/r4088/r4133; added 2026-07-07). Two roles since UPGRADE_PLAN WP-U0: the
  opt-in inventory channel (`ab_compare.py`, `corpus_live_opendss`) AND the
  mandatory gate's target-rev cases (manifest `oracle` field) — the Oddie venv +
  `bin/` are `cargo test` prerequisites now.
- `docs/upgrade/delta_*.md` — the three revision-delta inventories (0.14.5→0.15.x,
  r3723→r4088, r4088→r4133; surveyed 2026-07-07) — UPGRADE_PLAN's scoping input.
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

# PLAN_SEQUENCE — the master ordering of all plan documents

One page answering "which plan runs after which." `PORTING_PLAN.md` stays authoritative for
the porting phases themselves; this file orders the *documents*, including everything after
final acceptance. Order confirmed by the user 2026-07-06; UPGRADE_PLAN inserted as
post-acceptance stage 3 per the user's 2026-07-07 request (its WP-U0 infra pre-landed).

> **Plan file locations (2026-07-18).** The completed plans below — `PHASE4–8_PLAN.md`,
> `GAPS_PLAN.md`, `JSON_EXPORT_PLAN.md`, `UPGRADE_PLAN.md`, plus the finished test-infra
> build-plans `CORPUS_TEST_PLAN.md` / `CONTROL_COVERAGE_PLAN.md` — are archived under
> **`docs/plans-archive/`** (their names in the ordering are unchanged). Active/foundational
> plans stay at the repo root: `PORTING_PLAN.md`, `DE_PASCALIZE_PLAN.md`,
> `DIAKOPTICS_PSTCALC_PLAN.md`, `GOLDEN_REBASE_PLAN.md`, `R4133_PROPS_PLAN.md`,
> `RESONANCE_PLAN.md`, `MULTITHREADING_PLAN.md`, `WASM_USERMODELS_PLAN.md`.
> Residual work no plan owns is tracked in `ORPHANED_GAPS.md`.

```
── PORTING (pre-acceptance) ────────────────────────────────────────────────────────
 1. PHASE8_PLAN.md            COMPLETE (branch phase-8-reporting; WP8.8 exit done)
 2. GAPS_PLAN.md              COMPLETE — WPG.1–18 (2026-07-09) + follow-ups
                              WPG.19/20 merged 2026-07-11 (`7860582`)
 3. DIAKOPTICS_PSTCALC_PLAN.md **Part I** COMPLETE (2026-07-11) — WP-PF.1
                              `Pstcalc` command, WP-PF.2 Monitor mode-4 flicker,
                              WP-AD.1 incidence matrix + Sparse_Math + exports
                              53–57; ultracode round, merged `be2cfc7`/`e96caaa`,
                              records in STATUS.md. The porting era's plans are
                              now all executed ⇒ next = FINAL ACCEPTANCE below.
 ═ FINAL ACCEPTANCE (PORTING_PLAN §6) — EXECUTED 2026-07-11 (branch final-acceptance,
   explicit user request; referee criteria_met=true / blocking_items=[]; record in
   STATUS.md §1 + PORTING_PLAN §6). DIAKOPTICS Part II early-start (stage 8) was
   user-ordered and ran in parallel — deliberately outside this acceptance. ═
── POST-ACCEPTANCE (upgrade, then refactor & improvement era) ──────────────────────
 4. UPGRADE_PLAN.md           COMPLETE (2026-07-17) — Rung 1 (WP-U1.*: dss_capi 0.15.x /
                              r4088-line parity, spec = .inputs/dss_capi_with_git@0.15.x,
                              oracle capi015) exited WP-U1.10; **Rung 2 (WP-U2.*: OpenDSS
                              11.0.0.1 / r4133 parity, spec = Delphi diff, oracle
                              oddie:r4133) exited WP-U2.6** — r4133 `DSS_LIVE_OPENDSS_ASSERT=1`
                              sweep GREEN (0 unexplained divergences), r4088 direction check
                              green, `docs/upgrade/DIVERGENCES.md` complete. Engine behavior =
                              OpenDSS 11.0.0.1 (r4133) except the documented ledger. Its WP-U0
                              test infra (branch upgrade-test-infra: per-case `oracle` manifest
                              field, capi015 engine, iteration policy ≤) landed 2026-07-07 so
                              parallel porting branches inherited it. Ran FIRST post-acceptance:
                              freshest porting context, avoided double-touching code DE_PASCALIZE
                              would refactor, and lets Stage F pin r4133-parity (not r3723).
                              OPEN TAIL (recorded 2026-07-29): the §5 exit criterion
                              "`PropFlags::HIDE_015X` retired" (itself deferred from
                              WP-U1.4) was NOT met at the Rung 1/2 exit and remains
                              owned by the UPGRADE line's NEXT rung. Stage F measured
                              why it cannot ride along (F.3aa): the un-hide is atomic
                              with dropping the `Line.Wires → "Conductors"` json_name
                              masquerade + regenerating the 13 Dump/JSON/schema
                              goldens, i.e. it belongs to the next oracle-surface
                              switch, not to any Stage F step (F.3ag re-homing,
                              commit 1936f638). Tracked in ORPHANED_GAPS.md §2; two
                              pins in `crates/dss-core/tests/oracle_parity_cfg_gate.rs`
                              trip on any partial touch of the bundle.
 5. DE_PASCALIZE_PLAN.md      COMPLETE (2026-07-31, branch depas-stagef).
                              Merged: wave 1 2026-07-17 (R0 + P1-partial + P2 + P6,
                              `e7cfc1e`); the v2 reruns of the salvaged wave-2 WIP
                              2026-07-19/20 (P5a, P1b, P12+P13, P15 incl. M1 benches,
                              P9); R1(+P7), P10, P11, P8+P14, P5b/c 2026-07-25;
                              R2 (M3b seam) + R2b (a–e: make_like closed, from_ref
                              bridges, try_ckt_elem tag) 2026-07-25/26; R3 + the P1
                              deferred tail + P3 + wave 3 (depas-final) 2026-07-26…29.
                              All of the above [A]/bit-neutral, parallel worktrees +
                              opus audits; records docs/phase-records/depascalize-*.md
                              + STATUS. **Stage F** (the only [C] stage — the
                              `oracle-parity` feature split, parity target r4133)
                              executed 2026-07-26…31: F.1 seam → F.2 lane test policy
                              → F.3 kernel flips + TODO(compat) sweep (117 → 18, every
                              survivor a registered escape) → F.4 F-FMT → F.5 (the
                              parity↔default differential job, the two-lane gate
                              definition in CLAUDE.md, the success-metric gates). The
                              sanctioned default-lane re-baseline came back EMPTY (no
                              golden/tolerance/ledger/deck moved), and the differential
                              job measures the two lanes' solved-state checkpoint
                              stream bit-identical outside the one deliberate Newton
                              row (max |d| = 0 on every gated kind).
                              Hands forward, none of it open plan work: HIDE_015X →
                              UPGRADE_PLAN §5 (see item 4's open tail), the 4
                              whole-case exclusions → an unowned policy call, the 3
                              wasm-guest markers → WASM_USERMODELS. From here "gate
                              green" means BOTH lanes (CLAUDE.md, TESTING.md).
 5a. GOLDEN_REBASE_PLAN.md    IN FLIGHT (opened 2026-08-02) — retire the golden/
                              corpus debt of the r4133-authority policy switch
                              (not a prerequisite for MULTITHREADING M0–M2):
                              WP-G0 rails + WP-G2 bug-kernel teardown COMPLETE
                              (merged to `update` @ 4d3fc2d7); WP-G1 (live gate
                              to fastdss parity) open — G1.1 fired its kill
                              criterion 2026-08-08 and is handed to
                              R4133_PROPS_PLAN.md (its RP4.1 delivers the
                              unmask; G3.4/G3.5 wait on it), G1.2 (the
                              ESPVLControl corpus deck, the last zero-coverage
                              class) landed 2026-08-29 on `r4133-props`, and
                              G1.3a-d + G1.4-G1.11c remain; WP-G3–G5 queued.
                              Added 2026-08-22 per the user's request.
 5b. R4133_PROPS_PLAN.md      IN FLIGHT (authored + opened 2026-08-22) — property
                              parity on the r4133 channel, the dedicated
                              successor G1.1's kill criterion demanded: census
                              rails (WP-RP0) and
                              property-table shape closure (WP-RP1) COMPLETE
                              (RP1.4, 2026-08-23: r4133 shape classes 5 -> 0),
                              and the channel-aware value comparator (WP-RP2:
                              normalization + echo exclusions + the derived 2e-4
                              display floor) COMPLETE (RP2.4, 2026-08-23:
                              in-scope UNCLAIMED cells 521 841 -> 889, all of
                              them attributed to an open RP3.x sub-step);
                              next the four root-causes plus RP3.5-RP3.8
                              (WP-RP3), then the unmask (RP4.1 =
                              G1.1's deliverable, kill criterion re-armed).
                              RP5.2 flips this row to COMPLETE.
                              Runs inside the GOLDEN_REBASE window on branch
                              `r4133-props` off `update`.
                              Added 2026-08-22 per the user's request.
 6. RESONANCE_PLAN.md         WP-R1 iterative refinement (default lane on, parity off —
                              needs Stage F), WP-R2 resonance analysis, WP-R3 diagnostics
                              (UPGRADE_PLAN §1.3-1 already grants target-rev cases the
                              iterations-≤ policy WP-R1 needs)
 7. MULTITHREADING_PLAN.md    M0–M4 (actor mode, intra-solve rayon, faer parallelism);
                              M3 needs DE_PASCALIZE R2 arenas, M3c needs Stage F
 8. DIAKOPTICS_PSTCALC_PLAN.md **Part II** (WP-AD.2–AD.6: A-Diakoptics tearing,
                              solve engine, the corpus-wide AD↔normal sweep,
                              AggregateProfiles, optional threaded children) — last.
                              **The pinned oracle can't run it** (`DSS_CAPI_ADIAKOPTICS`
                              compiled out — errors #130); behavioral spec = the
                              official Delphi source (r3723 trunk, byte-identical
                              through r4133 — plan D10), gated rust-vs-rust
                              (AD solve ≡ normal solve) + the official-EPRI r3723
                              reference channel (Oddie; A-Diakoptics probe-proven
                              working there 2026-07-11 — plan §0.2/D9), and is
                              deliberately outside final acceptance. Early-start:
                              may begin as soon as MULTITHREADING **M2** lands
                              (M3/M4 are not prerequisites).
 9. WASM_USERMODELS_PLAN.md   COMPLETE (2026-07-25) — WP-WM.0–WM.7 all done
                              (WM.7 exit sweep: markers clean, gate green, five
                              expect_warnings decks byte-identical; branch wasm-wm7).
                              WP-WM.0–WM.7: replace the user-written-DLL mechanism
                              (GenUserModel/StoreUserModel/PVSystemUserModel/
                              CapUserControl — the one surface every prior plan
                              listed as "never: safe Rust") with sandboxed
                              WebAssembly over pure-Rust **wasmi** (the vendored
                              typst plugin host is the implementation template).
                              Same properties, same 15/13/7-function contract,
                              same warn-and-fallback failure semantics; activation
                              is additive (`.wasm` files only), so every existing
                              gate — incl. the five `expect_warnings` user-model
                              decks — stays byte-identical. Numeric gate = the
                              pinned oracle actually loading the native twin of
                              the reference model (FPC-built vendored IndMach012a
                              example), goldens committed; `.wasm` fixtures pinned
                              like goldens (`tools/wasm_usermodel/PIN.txt`).
                              Added 2026-07-12 per the user's request.
```

Early-start exceptions (allowed out of order because they are independent and cheap):
- `MULTITHREADING` **M0** (Send bounds + `assert_send::<Dss>()`) may land with
  DE_PASCALIZE R1 (it *is* DE_PASCALIZE P7).
- `MULTITHREADING` **M1** (criterion benchmark baseline) may land any time — it should
  exist *before* DE_PASCALIZE Part III to catch perf regressions there too.
- `DIAKOPTICS_PSTCALC` **Part II** may start right after MULTITHREADING **M2** —
  it does not wait for M3/M4 (its WP-AD.6 threading stretch is the only piece that
  consumes M2, and only optionally).
- `WASM_USERMODELS` may start **any time post-acceptance** — it is purely additive
  (new leaf crate + hooks at already-NOT_PORTED sites) and depends on no other
  post-acceptance stage. Caveat, not a blocker: landing it before DE_PASCALIZE R2
  means the element-side hooks get re-touched by the arena refactor (cheap — the
  hooks are thin); its per-element-owned instances are already M3-compatible by
  design (plan §2.7). Its extra vendored-source requirements
  (`.inputs/electricdss-code-r3723-trunk`, `.inputs/typst`) join the ritual step-0
  check for its WPs only.

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
  engine — audits there are `opus-xhigh` too); GOLDEN_REBASE **G1.3a–c/G1.5/G1.11a–c/
  G4.1** (G2.5 complete; audits `opus-xhigh` on all of them); R4133_PROPS
  **RP1.2/RP1.3/RP2.1/RP2.4** (audits `opus-xhigh` too; the rest of both plans is
  `opus-high+` per their §0 tables).
- **`opus-high+`**: DE_PASCALIZE R2, P10, P15 (rest); MULTITHREADING M3a/M3b/M3d/M4;
  RESONANCE WP-R1; GAPS **WPG.13** (GFM); DIAKOPTICS_PSTCALC **WP-AD.2** (partitioner +
  torn-file emission) and **WP-AD.6**; WASM_USERMODELS **WP-WM.0** (ABI freeze),
  **WP-WM.2** (reference-model fixture port — audits `opus-xhigh`), **WP-WM.3**
  (Generator + oracle gate — audits `opus-xhigh`; exec escalates to `opus-xhigh` if
  the pinned-oracle DLL channel falls through to r3723, plan §2.5) and **WP-WM.6**.
  Audits everywhere are `opus-high+` minimum, `opus-xhigh` on the xhigh-exec stages.
- **`opus-medium+`**: everything else in the post-acceptance plans (R0/R3, Part II,
  P8/P9/P11–P14, P3, M0/M1/M3c, WP-R3) — mechanical-with-guardrails: named pinning
  tests, forbidden-move lists, and the "when stuck: leave green, record in STATUS,
  surface" escape protocol. DIAKOPTICS_PSTCALC's remaining WPs sit here too:
  **WP-PF.1/PF.2, WP-AD.1** (pre-acceptance Part I) and **WP-AD.4/AD.5**; likewise
  WASM_USERMODELS **WP-WM.1/WM.4/WM.5** (WM.7 exit sweep is `sonnet-high+`).
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
- `docs/wasm/USERMODEL_ABI.md` + `tools/wasm_usermodel/` (PIN.txt, fixture sources,
  native-twin build) — the WASM user-model ABI spec and pinned artifacts, created by
  WASM_USERMODELS WP-WM.0/WM.2.
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

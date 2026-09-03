# Carried-forward handoffs and residual-floor items

> Moved verbatim from `STATUS.md` on 2026-09-03 (STATUS.md archiving round 2);
> order preserved, nothing rewritten. It holds STATUS §1's 2026-08-05 reading note and
> the two lists it introduces - the carried-forward handoffs and the residual
> floors / parked items, closed rows included. STATUS.md §7 forwards them here.

> **Reading note (2026-08-05).** The two lists below are kept **verbatim** from
> the pre-archiving STATUS. Rows closed by a later round say so in place, with
> two exceptions the archiving supersedes: the "`TODO(compat)` bug-for-bug sweep
> → Stage F — NOT started" and "`HIDE_015X` → Stage F — NOT started" handoffs
> were executed by DE_PASCALIZE Stage F (complete, `depascalize-stagef.md`) and
> are now finished by GOLDEN_REBASE WP-G2/WP-G4; the live marker population is
> the 15-row table above, not §5's 2026-07-17 count of 123. Their in-place
> back-references now resolve outside this file: `§OG-1.x … below` in
> [`orphaned-gaps.md`](docs/phase-records/orphaned-gaps.md), `§1a archive` in
> [`era-summaries.md`](docs/phase-records/era-summaries.md). §7 states the
> general forwarding rule.



**Carried-forward handoffs — work a *declared-complete* plan deferred to a
successor plan that has NOT finished it** (audited 2026-07-17; surfaced here so the
open item is not buried in the §1a archive):
- **TODO(compat) bug-for-bug sweep → DE_PASCALIZE Stage F — NOT started.** 123
  `TODO(compat)` shims across 71 files still in-tree (PORTING_PLAN §4.1 rule 4, the
  single dedicated post-acceptance cleanup pass, re-assigned to Stage F). Stage F is
  unbuilt (DE_PASCALIZE paused after wave 1) → cleanup unexecuted.
- **UPGRADE §5 exit criterion `rg HIDE_015X` empty → DE_PASCALIZE Stage F — NOT
  started.** 15 `HIDE_015X` refs in 5 files (line / line_geometry / prop_flags /
  save/dump). UPGRADE was declared COMPLETE having consciously waived this own-§5
  criterion to Stage F (byte-neutral, non-rung-blocking); still unmet.
- **IEEE118Bus NCIM PV→PQ switching-cadence → a future UPGRADE rung — NOT started.**
  Port matches its capi015 oracle loop-for-loop incl. non-convergence, but not
  r4133's newer cadence; parked `skipped_needs_investigation`, report-only in
  DIVERGENCES.md.
- **GICMvars export (verb 36) / GICTransformer `WriteVarOutputRecord` → Phase 9 —
  ✅ PORTED 2026-07-18** (orphaned-gaps round OG-1.1, branch `og11-gicmvars`; see
  §OG-1.1 below). Was GAPS WPG.16's only deferred piece.
- **AltDSS JSON `DynInit` tail + Full-mode Transformer WdgCurrents — DONE
  (og1213, 2026-07-18; see §OG-1.2+1.3).** Capacitor CMatrix = proven UB
  non-port (uninitialized heap, nondeterministic across processes). New
  sub-follow-ups surfaced (below).
- **AutoTrans JSON array-alternative metadata + golden → ✅ CLOSED 2026-07-26**
  (orphaned-gaps round OG-1.3a, branch `depas-og`; see §OG-1.3a below). The
  metadata premise was **stale**: `430d033` (og15c-B6) had already ported the
  singular/plural `array_alternative` + `REDUNDANT` + `ON_ARRAY` block into
  `auto_trans/mod.rs`, so only the golden was missing. Now pinned by
  `autotrans_micro` + `autotrans_solved`.
- **Generator/PVSystem/Storage `ShaftModel`/`ShaftData` under JSON Full → ✅
  CLOSED 2026-07-26** (OG-1.3a). Re-triaged empirically: the WM.3/WM.4
  NOT_PORTED removal did make them render, and the Full JSON matches the pinned
  0.14.5 oracle exactly (all six surfaces `""`). Pinned by `der_usermodel_full`
  (empty default) and `der_usermodel_assigned` (assigned data strings).
- **DER user-model FILENAME render (`UserModel`/`ShaftModel`/`DynaDLL`) still
  unpinned — OPEN (OG-1.3a settle).** Probed: the oracle stores and renders an
  unresolvable name but raises `#570 … Not Loaded` doing it, so a byte golden
  needs `gen_json.py` to tolerate a `DSSException` on selected commands.
  Deliberately not built — do it together with the WASM loader's own
  error-path gating, not by weakening the generator.
- **WindGen `Spectrum` FullNames render ungated — OPEN (OG-1.3a settle).** The
  other twelve Spectrum-bearing classes are pinned (`spectrum_refs` +
  `der_usermodel_full`); WindGen has no capi channel because the class does not
  exist in the pinned 0.14.5 oracle. Needs an r4133-side JSON channel, or a
  UPGRADE-line rung that gives WindGen an oracle.
- **A-Diakoptics `AggregateProfiles` command + D9(d) official-r3723 AD-replay →
  DIAKOPTICS Part II WP-AD.5 — partial.** `exec/command.rs:69` `NOT_PORTED`; WP-AD.6
  threaded children not started (needs MULTITHREADING M2).
- **User-model native DLLs (Gen/PVSystem/Storage/CapControl UserModel) →
  WASM_USERMODELS COMPLETE (WM.0–WM.7 all done, 2026-07-25).** All six properties
  × four elements (Generator `UserModel`/`ShaftModel` WM.3; Storage
  `UserModel`/`DynaDLL` + PVSystem `UserModel` WM.4; CapControl `UserModel` WM.5;
  callback tail WM.6; exit sweep WM.7) follow the plan §2.4 uniform rule (a
  `.wasm` loads through the sandboxed `dss-usermodel` ABI; a native-DLL name warns
  #570/#1570 + falls back to built-in), gated bit-exact vs the r4133 oracle
  (`indmach012a` + `wm4model` + `capuserctl` twin-pinned `.wasm` fixtures). **Zero
  live `PropFlags::NOT_PORTED`** on any user-model property. `PLAN_SEQUENCE.md`
  stage 9 COMPLETE.
- **`like=` dropped a bound user model on Generator / PVSystem / Storage /
  CapControl — FIXED and pinned by the RP1.3 audit settlement (2026-08-23)**,
  after being found (and measured) while fixing the identical defect on WindGen.
  `ClassArena::make_like_within` hands `make_like` an owned `clone()` of the
  donor and the user-model slot's `Clone` deliberately drops the live wasmi
  instance; every call site then guarded on `exists()`, so the copy echoed
  `UserModel=<path>` and silently ran the built-in model with no diagnostic
  (measured: `wasm_gen_pflow` + `New Generator.g2 like=g1` → **g1 20 variables,
  g2 six**). Upstream's `MakeLike` assigns `UserModel.Name`, which is a
  `Set_Name` = free + `LoadLibrary` + `FNew`, i.e. an eager fresh instance at the
  guest's own defaults (`generator.pas:825-826`, `PVsystem.pas:909`,
  `Storage.pas:1210-1211`, `CapControl.pas:452`). All five slots on the four
  classes now queue a real load exactly as WindGen does — Generator
  `UserModel`+`ShaftModel`, PVSystem `UserModel`, Storage `UserModel`+`DynaDLL`,
  CapControl `UserModel` — each with its own regression pin
  (`wasm_usermodels.rs::like_carries_a_live_user_model_on_both_generator_slots`,
  `wasm_usermodels_wm4.rs::like_carries_a_live_user_model_on_the_wm4_classes`,
  `wasm_usermodels_wm5.rs::like_carries_a_live_user_control`), all three measured
  non-vacuous by reverting the fix. The related `ClassArena::clone_ckt` shadow
  (control dispatch with monitored == switched) is closed on WindGen too: every
  engine-side call site now goes through `take_live_user_model`, which revives a
  snapshot's instance from its spec instead of falling back in silence
  (`an_element_snapshot_revives_its_user_model`). The WM.3/WM.4 slots keep the
  older lazy shape there — their `clone_ckt` path is still unreachable in the
  corpus (a Fuse's switched element is a PD element) and is the one part of this
  item left open.

**Residual floors / parked (documented, not bugs):**
- **The file-backed-loadshape ORACLE flake is not extinct — one recurrence
  2026-08-23** (RP1.3's gate run, parity lane): `corpus_gate` failed on
  `modes:upgrade/mmf_singlecol` step 0 with the **r4133** side 2.1e-3 V below
  the port on `SOURCEBUS.1` (allowance 8.2e-6), the exact ~2e-3 class the
  case's `isolate: true` note documents. The port side is bit-stable (the lane
  dump of the same tree carries the failing "actual" value and is byte-identical
  to the pre-sub-step baseline), the case passes standalone, and the re-run of
  the full command is green. UNIFIED_GATE Phase D's fix — a fresh worker per
  case plus `isolate` on this deck — lowered the rate but has not eliminated it,
  and the Phase-D record's "4/4 consecutive green full runs" is therefore an
  under-sample, not a proof. Next suspect if it recurs: the case-directory
  guard restoring `mm8.csv` while a one-shot worker still has it mapped.
- **ckt24 RegControl/LDC `SubXFMR`** ~4.7e-5 rel tap-current — ultra-switch
  conditioning floor (CF-D), watch on re-touch.
- ~~**UPFC modes 2/3/5**~~ **CLOSED 2026-07-18 (OG-1.7)** — see §OG-1.7 record;
  `midi_relay_dist` deferred (budget); Kersting4wire #567
  UserModel decks parked (no oracle channel tolerates the DoSimpleMsg).
- **UTF-8-BOM edge cases** — GAPS follow-up. (`CapControl.ControlSignal` FOLLOW path
  is in fact *ported* and live in `cap_control` — the old "unported" note was stale
  and is retired.)

Retired (done): combo fuse-save restore (wt-combo); WP-U1.2 D3 / WP-U1.6 tail (all
landed pre-rung-exit); Monitor modes 8/10/12 (test-triage wt-t3).


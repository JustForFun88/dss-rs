# RETRO_FIXES — step records (one block per step, appended by its settler, plan `RETRO_FIXES_PLAN.md`)

## RF-D01-01 — Fix Relay/SwtControl per-phase state parse off-by-one and pin the >6-phase rendering (2026-09-26)
- **Render clip kept (R4) and pinned.** r4133's getters loop `ControlledElement.NPhases` uncapped
  (`Relay.pas:1409`/`:1420`, `SwtControl.pas:591`/`:602`) over `Array[1..6]` (`:62`/`:19`), so a 7th token is an
  out-of-bounds read, not reproduced. Pins: `relay::tests::a_seven_phase_controlled_element_renders_six_tokens` and
  its `swt_control` twin (each assert names r4133's seven tokens). `non_debug_lines` filters `Element=Debug Sample:`
  (self-check `non_debug_lines_skips_the_debug_sample_lines_only`). Doc fixes: both `state_size`, `normal_state`,
  `ganged_view`. No ledger row, golden or tolerance moved (zero corpus exposure).
- **uids: 7 fixed / 0 recorded / 0 invalid / 1 blocked.** Blocked: `RP|RP3.7|AC3|AC3-1` (major). The parse
  off-by-one (`Relay.pas:1286`, `SwtControl.pas:461`, `i<MAXDIM`) is still reproduced in both lanes, since the
  ordinal twins in `relay/accessors.rs` / `swt_control/accessors.rs` (outside Files) repeat the cap. It is routed to
  the coordinator, and every reproducing site says so. Upstream report (R1, local-only):
  `investigations/to_opendss/75-per-phase-state-list-drops-the-sixth-phase.md`.
- **Notes left:** `DIVERGENCES.md` (L10 render clip, L11 conditional on AC3-1), `ORPHANED_GAPS.md` (§1.15, §1.16,
  §1.14(a)), `R4133_PROPS_PLAN.md`, `r4133-props-rp3.md`. The epri-worker probe transcripts (r4133 DLL 11.0.0.1)
  are local-only under `tmp/retro_fix/state/RF-D01-01/probes/`, and the pin docs carry the re-derivation recipe.
- **Commits:** `27f666ec`, `1c90699e`, then this record's settlement commit. **Gate:** five commands exit 0,
  default and parity 13 663 / 0 / 5 each (passed / failed / ignored; the parity corpus gate needed its one unmodified
  re-run after a `Test/AutoTrans` leaked dropping), corpus gate green (every ledger entry hit, none stale),
  `lane_diff` not owed. **audit: 10 findings - 7 fixed / 3 recorded / 0 refuted** (AC-1/AT-1 are the blocked
  AC3-1, AT-2 its first-char pin flip).

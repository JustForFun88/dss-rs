# RETRO_FIXES — step records (one block per step, appended by its settler, plan `RETRO_FIXES_PLAN.md`)

## RF-D01-01 — Relay/SwtControl >6-phase render clip pinned; per-phase parse off-by-one fix blocked (2026-09-26)
- **Done (7 of 8 uids):** render clip kept (R4; r4133 getters loop `NPhases` uncapped, `Relay.pas:1409`, `SwtControl.pas:591`),
  pinned by both `a_seven_phase_controlled_element_renders_six_tokens`; `non_debug_lines` fixed (self-check
  `non_debug_lines_skips_the_debug_sample_lines_only`); `state_size`, `normal_state`, `ganged_view` docs corrected; ledger/goldens untouched.
- **Blocked `RP|RP3.7|AC3|AC3-1` (major; coordinator question 1):** the `i<MAXDIM` parse off-by-one (`Relay.pas:1286`,
  `SwtControl.pas:461`; report `investigations/to_opendss/75-per-phase-state-list-drops-the-sixth-phase.md`, R1 local-only) stays
  reproduced. Its ordinal twins in both `accessors.rs` (outside Files) are unmarked; `rg "RF-D01-01 AC3-1" crates/` lists the marked sites.
- **Notes** for all four §4 docs. **Commits** `27f666ec`, `1c90699e`, `a2bb1203`, round 2. **Gate** 5/5 exit 0 (default re-run once: ckt24
  oracle timeout), 13 665 / 0 / 5 per lane, corpus gate green, no `lane_diff` owed. **Audit** 10 findings - 7 fixed / 3 recorded / 0 refuted.
- settlement round 2: 2 findings - 2 fixed / 0 recorded / 0 refuted (both classes' `a_seven_token_list_leaves_no_residue_for_the_next_bare_write`).

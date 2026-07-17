# DE_PASCALIZE P2 — Monitor mode bit-packing → typed decode

Stratum [A], bit-neutral. Branch `wt-p2` off `update`.

## What landed

The one genuine raw-int bitfield on `Monitor` — `mode: i32` with `MODEMASK=15`,
`SEQUENCEMASK=16`, `MAGNITUDEMASK=32`, `POSSEQONLYMASK=64` decoded ad-hoc with
`&`/`==` across `sample.rs`, `header.rs`, `post.rs`, `mod.rs` — is now decoded
**once** into a typed view.

- New submodule `elements/meter/monitor/mode.rs`:
  - `enum MonitorBaseMode` (0..=12 + `Undefined`): `VoltageAndCurrent`, `Power`,
    `Tap`, `StateVars`, `Flicker`, `SolutionVars`, `CapacitorSteps`, `Storage`,
    `TransformerWindingCurrents`, `Losses`, `TransformerWindingVoltages`,
    `AllTerminalVI`, `LineToLineVoltages`, and `Undefined` (base 13/14/15).
  - `struct MonitorModeView { raw, base, sequence, magnitude, posseq_only }` with
    `from_raw(i32)` / `to_raw() -> i32`. The four masks (15/16/32/64) are pinned
    **inside** `from_raw` and **nowhere else** in the crate. `to_raw` returns the
    verbatim `raw` ordinal (Pascal `Get_Mode` reads the whole `Mode: Integer`),
    so the decode is lossless — no information is dropped at the boundary.
  - Round-trip + decode unit tests (4 tests, all green): defined-encoding
    round-trip, base/flag decode, undefined-base raw preservation, high-junk-bit
    round-trip.
- `Monitor` now stores `mode: MonitorModeView` (private) instead of `pub mode: i32`.
  Raw `i32` survives only at the property boundary:
  - `set_i32(MODE, v)` → `MonitorModeView::from_raw(v)`
  - `get_i32(MODE)` → `self.mode.to_raw()`
  - new `pub fn mode_raw(&self) -> i32` for the one external reader
    (`solution/monitors.rs`, the Pascal `SampleAllMode5` `Mode = 5` full-ordinal
    split — uses `to_raw()` so the full raw ordinal is compared, matching Pascal).
- Every ad-hoc mask-decode site rewritten to match on the view:
  - `header.rs`: class-check `match self.mode.base`; the per-mode header `match
    self.mode.base`; general header `is_pos_seq`/`is_power`/`match
    (magnitude, posseq_only)`.
  - `sample.rs`: `match self.mode.base` for the mode body; `is_sequence`,
    `is_power`, the `VoltageAndCurrent` residual branch, and the write-out
    `match (magnitude, posseq_only)`.
  - `post.rs`: mode-4 guard → `self.mode.base == MonitorBaseMode::Flicker`.
  - `mod.rs`: `metered_bus_name` mode-4 guard → `== Flicker`.

## Key decisions

- **`(magnitude, posseq_only)` tuple match** replaces `mode & (32+64)` → the four
  arms `(true,false)=32`, `(false,true)=64`, `(true,true)=96`, `(false,false)=0`
  are exhaustive and reproduce the exact same branches/accumulation order.
- **Undefined base 13/14/15 → distinct `Undefined` variant, raw ordinal preserved.**
  Pascal's `ClearMonitorStream` `case … else` routes 13/14/15 through the general
  (mode-0-style) header, but its `TakeSample` `else Exit` records a *timestamp-only*
  row (NOT a mode-0 V/I record). The `Undefined` variant reproduces both: it falls
  into `header.rs`'s general-header catch-all (byte-identical header) yet is listed
  in `sample.rs`'s no-op arm (timestamp only). The `raw` field preserves the exact
  ordinal so `get_i32`/`mode_raw` report 13/14/15 verbatim — fully bit-neutral even
  on these untested inputs. (Corpus monitor modes seen: 0/1/2/3/4/5 base + modifiers
  16/32/48/64/65/96/112.)
- **High junk bits (≥128) preserved.** `to_raw` returns the verbatim ordinal, so a
  raw such as 133 (`128 + base 5`) round-trips to 133; `get_i32` reports 133 and the
  `Mode = 5` mode-5 split (Pascal `Mon.Mode = 5`, full ordinal) correctly sees
  `133 ≠ 5`. No lossy canonicalization anywhere.
- `bitflags` deliberately not used (low 4 bits are a multi-bit subfield, per the plan).

## Metric counts

- Mask-mention sites before (`rg 'MODEMASK|SEQUENCEMASK|MAGNITUDEMASK|POSSEQONLYMASK'`
  in monitor/): 4 const defs (mod.rs) + 3 import lines + ~11 use sites across
  sample/header/post/mod = the four masks referenced in 4 files.
- After: the four masks appear **only** inside `mode.rs` (`from_raw`/`to_raw`); zero
  mask `&`-decodes remain in `sample.rs`/`header.rs`/`post.rs`/`mod.rs`.
  `rg 'MODEMASK|SEQUENCEMASK|MAGNITUDEMASK|POSSEQONLYMASK' crates/dss-core/src/elements/meter/monitor`
  → matches in `mode.rs` only.
- Files touched: `mode.rs` (new), `mod.rs`, `accessors.rs`, `header.rs`,
  `sample.rs`, `post.rs`, `solution/monitors.rs`.

## Audit settlement

Two auditors reviewed the port. audit-tests: CLEAN (no blockers). audit-code: one
`minor` finding plus two `note`s. Disposition:

- **FIXED — audit-code minor: undefined base 13/14/15 and high junk bits (≥128)
  were lossily canonicalized, breaking [A] bit-neutrality.** Confirmed empirically:
  (a) old `sample.rs` matched raw `mode_mask`, so base 13/14/15 hit `_ => return`
  (timestamp-only), matching Pascal `TakeSample` `else Exit` (Monitor.pas:1414);
  the first port routed them through `from_bits`'s `_ => VoltageAndCurrent` into the
  full V/I arm. (b) `to_raw` re-packed from decoded fields, so `from_raw(133).to_raw()`
  gave 5, losing bit 128 — but Pascal `Get_Mode` returns the whole `Mode: Integer`
  and `SampleAllMode5`/`SampleAll` split on the full ordinal `Mon.Mode = 5`
  (Monitor.pas:393,406), so 133 must stay 133. Fix (auditor's suggested shape):
  `MonitorModeView` now carries the verbatim `raw: i32` (returned unchanged by
  `to_raw`), and `MonitorBaseMode` gains an `Undefined` variant for base 13/14/15
  that (i) falls into `header.rs`'s general-header catch-all — byte-identical header,
  matching Pascal's `ClearMonitorStream` `else` — and (ii) is listed in `sample.rs`'s
  now-exhaustive no-op arm (timestamp-only). New unit tests
  `undefined_base_preserves_raw_ordinal` and `high_junk_bits_survive_round_trip`
  pin both. Defined modes 0..=12 with any modifier are unchanged (`to_raw` == the
  encoded value), so every golden/corpus deck stays byte-identical.
  - Side note surfaced by the fix: `from_bits` needed an explicit `0 => VoltageAndCurrent`
    arm once the wildcard became `Undefined` (base 0 previously relied on the wildcard).
- **NOTE — audit-code: all other transformations bit-neutral, masks confined to
  `mode.rs`.** Acknowledged; unchanged by the fix (masks still live only in `mode.rs`).
- **NOTE — audit-code: no forbidden moves.** Acknowledged; still holds — no golden
  regenerated, no tolerance touched, no `cfg(feature="oracle-parity")`, no
  Rc/RefCell/Mutex/statics; scope still the monitor module + `solution/monitors.rs`.
- **NOTE ×3 — audit-tests (CLEAN).** No action; the round-trip pin is extended (not
  weakened) with the two new lossless-decode tests.

## Deferred / escape-protocol sites

None. All decode sites converted; the audit finding was fixed rather than deferred.

## Gate

- `cargo +stable fmt --all --check` — green.
- `cargo +stable test --workspace` — green (see final report for counts).
- `cargo +stable clippy --workspace --all-targets -- -D warnings` — the P2 code is
  clippy-clean (no error in any monitor file). Three **pre-existing** clippy errors
  in files untouched by P2 fail under the installed stable (clippy 0.1.96):
  `exec/diakoptics/matrices.rs:40` (`nonminimal_bool` on a line already carrying
  `#[allow(clippy::eq_op)]`), `solution/inc_matrix.rs:190`, `elements/pc/windgen/tests.rs:356`.
  These are identical to `update` (empty `git diff` for those files) — a
  toolchain-drift condition on the base branch shared by all worktrees, not
  introduced by P2. Left for central resolution by the orchestrator.

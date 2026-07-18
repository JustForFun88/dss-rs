# DE_PASCALIZE P13 — VCCS delay line → `RingBuf` type

Stratum [A], bit-neutral. Branch `wt-p1213-v2` off `update`@ae4b4ef.

## What landed

The VCCS z-domain filter's two wrap-around history buffers — `z` and `whist`,
tapped with the 1-based circular `MapIdx(iu - k + 1, fl)` inside the filter sum
(`vccs/dynamics.rs`, `TVCCSObj.IntegrateStates`/`IntegratePhasorStates`) —
are now a small `RingBuf` type whose accessor encapsulates the wraparound.

- New `pub struct RingBuf { buf: Vec<f64>, len: i64 }` in `vccs/mod.rs`:
  - `empty()` (nil placeholder, matches the former `Vec::new()`), `alloc(len)`
    (Pascal `Allocmem(len+1)`, slot 0 dead).
  - `tap(idx: i64) -> f64` = `self.buf[map_idx(idx, self.len)]` — the sole reader
    that applies the wraparound, calling the **unchanged** module-level `map_idx`.
  - `Index<usize>`/`IndexMut<usize>` for direct head writes (`self.z[iu_u]`),
    the snapshot/clear loops (`self.z[k] = self.zlast[k]`), and `s5`/`s6` reads.
- `Vccs.z` / `Vccs.whist`: `Vec<f64>` → `RingBuf`. `RingBuf` derives `Debug, Clone`
  (Vccs derives both). Init in `new()` (`empty()`) and `RecalcElementData`
  (`alloc(ffiltlen)`).
- The four filter-tap sites (two in the waveform `IntegrateStates`, two in the
  phasor `IntegratePhasorStates`):
  `self.whist[map_idx(iu - k + 1, fl)]` → `self.whist.tap(iu - k + 1)`, likewise
  `self.z`. `map_idx` dropped from the `dynamics.rs` import (now reached only
  through `tap`; still used module-wide by `offset_idx` and `RingBuf::tap`).
- `y2`/`zlast`/`wlast` stay `Vec<f64>` — they are plain windows / corrector-step
  snapshots, never tapped through `MapIdx` (only direct-indexed or fully summed),
  so wrapping them would add surface without removing any `map_idx` call.

## Tap-order proof (how bit-neutrality was verified)

`RingBuf::tap` calls the identical `map_idx(idx, len)` the inline code did, and
`RingBuf.len` for `z`/`whist` is `ffiltlen` = the former `fl` argument — so the
physical slot each tap reads is unchanged, and the filter sum keeps its
statement order (`k = 1..=ffiltlen` forward, same `filter_y`/`filter_x` operand
pairing). Pinned:

- New unit test `ringbuf_tap_reproduces_pascal_map_idx_order`: fills a len-5 ring
  with `slot k = k`, asserts the `iu - k + 1` taps at head `iu = 1` read slots in
  the exact `[1, 5, 4, 3, 2]` order the existing `offset_idx_steps_and_wraps`
  pins for the raw helper, and that `tap(idx) == self[map_idx(idx, len)]` for
  every index across `-len ..= 2·len` (incl. negative / past-end).
- The 3 oracle-gated dynamics-trajectory tests (`exec::tests::vccs`,
  Monitor mode-3): `vccs_waveform_dynamics_mode3_matches_oracle`,
  `vccs_rmsmode_dynamics_mode3_matches_oracle_1phase` / `_3phase` — all pass
  UNCHANGED. These drive the exact tap loops end-to-end against the pinned
  oracle.

## Audit settlement

_(to be appended after the P13 audits)_

## Deferred / escape-protocol sites

None. Both tapped buffers converted; `y2`/`zlast`/`wlast` deliberately left as
`Vec` (they carry no `MapIdx` tap) — a scope decision, not a blocked rewrite.

## Gate

- `cargo fmt --all --check` — green.
- `cargo clippy --workspace --all-targets -- -D warnings` — green.
- `cargo test --workspace` — green (counts in the final report).
- `tests/corpus` pristine.

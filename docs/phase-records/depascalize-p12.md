# DE_PASCALIZE P12 — `line_constants` parallel arrays → `Vec<Conductor>`

Stratum [A], bit-neutral. Branch `wt-p1213-v2` off `update`@ae4b4ef.

## What landed

The ~20 parallel per-conductor `Vec<f64>`/`Vec<i32>`/`Vec<bool>` arrays on
`LineConstants` (Pascal `TLineConstants`/`TCableConstants`, `LineConstants.pas` /
`CableConstants.pas`) collapse into a single `cond: Vec<Conductor>`.

- New private `struct Conductor { x, y, rdc, rac, gmr, radius, cap_radius,
  cable: Option<CableData> }` — the 7 base geometry/parameter fields, plus the
  cable subclass data as an `Option`.
- New private `struct CableData` gathers the 11 cable-only arrays (`eps_r`,
  `ins_layer`, `dia_ins`, `dia_cable`, `cond_type`, `semicon_layer`, `k_strand`,
  `dia_strand`, `gmr_strand`, `rstrand`, `dia_shield`, `tape_layer`, `tape_lap`).
  `Some(CableData::default())` on every conductor of a cable engine (Pascal
  `Allocmem` zeros), `None` on an overhead conductor — the typed form of the
  Pascal "subclass arrays empty on overhead lines" trick.
- `LineConstants` field block: the ~20 `f*`-prefixed `Vec`s replaced by the one
  `cond: Vec<Conductor>`. Constructor `with_kind` builds it with the Pascal
  "not set" sentinels (`rdc/gmr/radius/cap_radius = -1.0`; `rac/x/y = 0.0`).
- Two private accessors `cable(i) -> &CableData` / `cable_mut(i) -> &mut CableData`
  (panic on an overhead engine — reproduces the Pascal precondition that the
  cable setters/`Calc` run only on a `TCableConstants` engine, formerly an
  empty-array OOB). Plain-private (not `pub(super)`): the four files are all
  descendants of the `line_constants` module, so no visibility leak of the
  private `CableData`.
- Every kernel now reads `self.cond[i].field` / `self.cable(i).field`:
  - `mod.rs`: setters, `get_zint`, `get_ze`, `calc_overhead` (self/mutual
    impedance + capacitance), `cisp_overhead`, `set_height_offset` loop.
  - `cable.rs`: cable setters, `cable_dij`, `calc_cable` (self/mutual CN/TS +
    coaxial capacitance), `cisp_cable`. Per-conductor cable reads bound once as
    `let cab = self.cable(i);` inside the CN/TS match arms.
  - `cn.rs` / `ts.rs`: the CN strand / TS shield setters → `cable_mut(i)`.

## Bit-neutrality

Same arithmetic, same operand order — only the storage layout of the operands
changed (parallel arrays → array of structs). Every read/write targets the exact
same logical `(conductor i, field)` value it did before. The FPC-compat helpers
(`cdiv_fpc`, `csqrt_fpc`, `cln_fpc`, `cabs_fpc`) and all `TWOPI`/`MU0`/`E0`
constants are untouched (Stage F's business, forbidden move 5).

Proof, all UNCHANGED:
- `support::line_constants::tests` — 20 unit tests (overhead / CN / TS / DERI /
  FullCarson / SimpleCarson, reductions, unit+length conversion, rho-recalc).
- `golden_line_constants::line_constants_scenarios_match_oracle` + the harness
  comparator/props tests (11 total).
- `corpus_gate_all_cases_match_engines` (checkpoint YPrims through every line/
  geometry deck) — bit-identical.

## Salvage path

`origin/wt-p1213` WIP commit `70cefbb` (single-file `mod.rs`, +126/−89,
interrupted mid-`calc_overhead`) was cherry-picked cleanly (its parent tree's
`mod.rs` is byte-identical to `update`@ae4b4ef). Reviewed hunk-by-hunk against
the P12 spec and the Pascal, then completed: the remaining `calc_overhead`
capacitance + `cisp_overhead` sites in `mod.rs`, plus all of `cable.rs` /
`cn.rs` / `ts.rs`, which the draft never touched. The WIP commit was folded into
this single P12 commit (`git reset --soft`), so the WP is one commit per the
plan; draft authorship credited in the trailer.

## Audit settlement

_(to be appended after the P12 audits)_

## Deferred / escape-protocol sites

None. Every parallel-array site converted; the rewrite is bit-neutral throughout
(no site required leaving old code in place).

## Gate

- `cargo fmt --all --check` — green.
- `cargo clippy --workspace --all-targets -- -D warnings` — green (the draft's
  `pub(super)` accessors leaked private `CableData`; fixed to plain-private).
- `cargo test --workspace` — green (counts in the final report). One
  non-deterministic `corpus_gate` iteration-count flake under 4-way concurrent
  worktree load did not reproduce on isolated or repeat full runs; a bit-neutral
  refactor (proven by the unchanged goldens) cannot fail non-deterministically —
  it is cross-worktree resource contention, not a regression.
- `tests/corpus` pristine.

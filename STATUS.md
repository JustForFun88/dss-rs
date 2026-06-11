# dss-rs — Project Status & Session Handoff

> **Purpose of this file:** a living snapshot so a fresh session can resume
> without re-deriving context. It records *what is done*, *what was decided*,
> and **why**. It is **not** authoritative for the plan itself — that is
> `PORTING_PLAN.md` (roadmap + binding decisions) and `CLAUDE.md` (conventions
> + the green-gate rule). Read those two first; then read this for the current
> frontier.

Last updated: 2026-06-11, end of **Phase 2**.

---

## 1. Where we are

| Phase | Scope | Status |
|------|-------|--------|
| 0 | Tooling, oracle, faer spike, CI, Phase-0 goldens | ✅ done (committed) |
| 1 | Shared math (`support/`) + full `TDSSParser` port | ✅ done (commit `729eb77`) |
| **2** | **Object model, property engine, executive skeleton** | ✅ **done — committed on branch `phase-2-object-model`** |
| 3 | ★ Vertical slice: parse → Y matrix → solve → voltages | ⬜ next |

**Important:** Phase 2 is committed on the `phase-2-object-model` branch (off
`main`), per the repo rule that commits happen only on explicit request and
never directly on the default branch. Merge to `main` when reviewed.

### Gate state (all green)
```
cargo fmt --all --check        # clean
cargo clippy --workspace --all-targets -- -D warnings   # clean
cargo test --workspace         # lib 83, dss-parser 62, props_roundtrip 1,
                               # golden_smoke 3, + harness/doctests all pass
```

---

## 2. What Phase 2 had to deliver (from PORTING_PLAN.md §Phase 2)

Object model (`NamedObject`/`DSSObject`/`DSSClass`), the `DSSObjectHelper`
generic parse/get engine, `Command.pas` abbreviation matching, and an
`Executive` subset (`New`, `Edit`, `~`, `Set`/`Get`, `?`, `Redirect`/`Compile`,
`Clear`, `like=`). **Gate:** `props_roundtrip` — create each implemented class
with defaults, dump every property, compare to a dss-python all-defaults golden;
plus scripted edits in odd orders and via abbreviations.

All of this is done **except** `Set`/`Get`/`Redirect`/`Compile`, which are
deliberately stubbed (see §4.7).

---

## 3. What was built (file by file)

### dss-parser (made reusable by the engine)
- `crates/dss-parser/src/parser.rs` — exposed internals the engine needs:
  `ParserError::new` is now `pub` (the engine reuses it for
  `EParserProblem`/`Exception` cases), and `val_f64` / `val_i32` /
  `check_for_var_in` are `pub`. No behavior change; Phase-1 golden still green.
- `crates/dss-parser/src/lib.rs` — re-exports `val_f64`, `val_i32`.

### dss-core: shared/util
- `src/support/command_list.rs` — **`CommandList`** = Pascal `TCommandList`.
  Registers all full names first, then every proper prefix that doesn't collide,
  in registration order (first-registered wins). `abbrev_allowed` flag mirrors
  `CommandList.Abbrev` (only `GrowthShape` sets it false). `get_command` returns
  a **0-based** index (Pascal returned 1-based; callers add 1).
- `src/util.rs` — `Utilities.pas` helpers ported on demand: `str_y_or_n`,
  `compare_text_shortest_eq`, `interpret_yes_no`, `parse_object_class_and_name`,
  `check_for_blanks`, `float_to_str`/`float_to_str_ex`, `get_dss_array_f64`/`_i32`,
  and **`interpret_dbl_array`** (the list-of-numbers path of `InterpretDblArray`;
  the `file=`/`dblfile=`/`sngfile=` paths error out — deferred).

### dss-core: object model (`src/obj/`)
- `obj/dss_enum.rs` — **`DssEnum`** = Pascal `TDSSEnum`. `string_to_ordinal`
  (the min/max-char prefix-disambiguation window, exact-match short-circuit,
  hybrid integer fallback, `default_value`/`use_first_found`/`try_exact_first`/
  `allow_longer`), `ordinal_to_string`, `is_ordinal_valid`. **`EnumRegistry`**
  owns the enum instances; an `EnumId` (a `usize`) is the Rust form of the raw
  `TDSSEnum` pointer Pascal stored in `PropertyOffset2`. Only `Length Unit` and
  `Earth Model` are registered so far (added as classes need them).
- `obj/base.rs` — **`DssObjData`** (name + `PrpSequence` set-order tracking:
  `set_as_next_seq`, `next_property_set` for future `SaveWrite`) and the
  **`DssObject` trait** (see §4.1 — the load-bearing design decision).
- `obj/props.rs` — the **generic property engine**:
  - `PropType` (subset: `Double`, `Integer`, `Boolean`, `String`, `MakeLike`,
    `DoubleArray`, `MappedStringEnum`, `MappedIntEnum`).
  - `PropFlags` (a `u64` bitset; behavioral flags `NON_NEGATIVE`/`NON_ZERO`/
    `NON_POSITIVE`/`GREATER_THAN_ONE`/`IGNORE_INVALID`/`INVERSE_VALUE`/
    `APPLY_ROUND`/`VALUE_OFFSET`/`TRANSFORM_LOWERCASE`; metadata-only flags
    `SUPPRESS_JSON`/`REDUNDANT`/`REQUIRED_IN_SPEC_SET`/`IS_FILENAME`/`GLOBAL_COUNT`
    are recorded for fidelity but inert in the text path).
  - `PropDef` (one `DefineProperties` row: name, type, flags, scale, trap_zero,
    value_offset, enum_id, size_prop) with builder constructors.
  - `ClassProps` (per-class property table + `CommandList`; 1-based, slot 0 is a
    dummy; auto-appends `Like`). Methods: `property_index`, `parse_into`,
    `edit_property` (one `TDSSClass.Edit` loop iteration), `get_value`.
  - `PropEngine` (the scratch parser + vars + enums + error sink threaded
    through), and the ports of `SetObjDouble`/`GetObjDouble`/`SetObjInteger`.

### dss-core: guinea-pig classes (`src/elements/general/`)
- `tcc_curve.rs` — **`TccCurveObj`** (`TCC_Curve.pas`). 3 props
  (`NPts`/`C_Array`/`T_Array`) + `Like`. Exercises integer + double-array +
  the npts→array size dependency + side effects (realloc, `CalcLogPoints`).
- `spectrum.rs` — **`SpectrumObj`** (`Spectrum.pas`). Adds the **scaled array**
  path (`%Mag`, scale 0.01) and a **string** prop (`CSVFile`), and reproduces
  Spectrum's distinct `NumHarm` side effect (zero-fills only the angle array,
  leaves the others NIL until set). `MultArray`/`SetMultArray` and `DoCSVFile`
  are deferred (not observable in the property dump).

### dss-core: executive (`src/exec/mod.rs`)
- **`Dss`** = Pascal `TDSSContext`: class registry, main parser, scratch
  (`aux`) parser, parser vars, enum registry, error log, `last_result`.
- `command(&str)` = `ProcessCommand`; implements `New`, `Edit`,
  `~`/`More`/`M`, `Clear`, `?`, and `like=` MakeLike, with the faithful
  `TDSSClass.Edit` loop (positional vs `name=` params, abbreviation lookup,
  `SetAsNextSeq`, `PropertySideEffects`, `EndEdit`). `Set`/`Get` and
  `Redirect`/`Compile` are registered verbs that record a clear
  "not implemented in the Phase 2 executive" message. The verb registration
  order follows the Pascal `TExecCommand` enum so abbreviation ownership among
  the implemented subset matches the oracle.

### Gate
- `tools/golden/gen_props.py` — regenerates `tests/golden/props.json` from the
  pinned oracle. **10 scenarios** across both classes (default, full spec,
  abbreviations, multi-command edit + `~`, npts shrink, MakeLike).
- `tests/golden/props.json` — committed golden (self-describing: each scenario
  carries its `commands`, `target`, and oracle property values).
- `crates/dss-core/tests/props_roundtrip.rs` — replays each scenario through
  `Dss` and compares per property with a **number-aware** comparator
  (skeleton-exact + numbers within tolerance).

---

## 4. Key design decisions & rationale

### 4.1 Property access: a typed-accessor trait, not pointer offsets *(load-bearing)*
Pascal pokes object fields via `ptruint(@obj.Field)` byte offsets stored in
`PropertyOffset[]`. That is impossible in safe Rust. **Decision:** the generic
engine (`obj/props.rs`) does all type dispatch, flag handling, scale, and
side-effect triggering **once**; each concrete class implements the
**`DssObject` trait** — typed getters/setters keyed by the **1-based property
index** (`get_f64(idx)`, `set_i32(idx, v)`, `get_f64_array(idx)`, …). These
`match idx { … }` arms are the 1:1 stand-in for the pointer pokes
(`SetObjDouble`/`GetObjInteger`/…).
- Default trait methods **panic** (`unreachable!`) so a wrong dispatch is an
  obvious bug, mirroring the Pascal base `CustomSetRaw` "base reached" guard.
- *Why not an enum `PropValue` blob?* It would lose the per-type clarity and
  still need a per-class match. The typed-accessor shape mirrors Pascal closest.

### 4.2 1-based indexing with a dummy slot 0
Property indices stay 1-based to match Pascal `TProp` ordinals (the order is
load-bearing for `PrpSequence`/Save). `ClassProps.props[0]` is an unused dummy
so `props[idx]` lines up. The 1-based/0-based boundary is a flagged risk
(PORTING_PLAN §5); keeping props 1-based localizes the only conversion to
`CommandList` (0-based → +1).

### 4.3 `Option<Vec<f64>>` for arrays = NIL semantics
A Pascal array field is a pointer; **NIL** dumps as `''` (not `[]`). So array
fields are `Option<Vec<f64>>`: `None` ⇔ NIL. This is what makes a default
object dump `C_Array=''` while a sized one dumps `[ … ]`.

### 4.4 Goldens test only deterministic states
The oracle's `ReAllocmem` leaves new array memory **uninitialized** — an array
sized by `npts` but never filled dumps heap garbage (`1.14e-311`, …), which is
**non-deterministic UB**, not a reproducible quirk. Project policy: don't try to
match UB. So goldens use only **all-default** or **fully-specified** states. A
fully-parsed array *overwrites every `npts` slot* (zero-filling when the input
string is short, e.g. `(5 6)` with npts=3 → `[ 5 6 0]`), which **is**
deterministic and reproduced exactly.

### 4.5 Number-aware golden comparison (never raw float-string diffs)
FPC `%g` formatting differs from Rust's `Display` (digit count, sci-notation),
so per the §4 tolerance policy the comparator splits each value into a
non-numeric **skeleton** (numbers → `#`) compared exactly, plus the **numbers**
compared with tolerance. This also absorbs scale round-trip float drift
(`%mag=14.3` → ×0.01 → ÷0.01 → `14.299999…` still matches `14.3`).

### 4.6 Property types/flags are ported on demand
`DSSObjectHelper.pas` is ~5000 lines with ~40 property types plus pointer-heavy
struct-array / matrix / complex / object-reference / string-list paths and a
whole JSON layer. **Decision:** port only the `PropType`/`PropFlags` the ported
classes actually use; add the rest as the classes that need them arrive. The
plan explicitly allows later phases to refactor earlier code — passing tests are
the only contract.

### 4.7 Executive scope: what's stubbed and why
- `Set`/`Get` (global options) need an options registry that doesn't exist yet.
- `Redirect`/`Compile` need recursive file execution.
- `circuit`/`solution` pseudo-classes need Phase 3's `Circuit`.
None are needed for the `props_roundtrip` gate, so all four verbs are
registered but record a clear "not implemented in the Phase 2 executive"
message (and `New circuit....` no-ops) rather than faking behavior.
`DSS_OBJECT` classes (TCC_Curve, Spectrum) are creatable without a circuit,
exactly as in Pascal.

### 4.8 MakeLike via `clone_box`
`like=src` copies another object's state. Source and target live in the same
`Vec<Box<dyn DssObject>>`, so we can't borrow both at once. **Decision:**
`DssObject::clone_box()` clones the source out of the arena, then
`target.make_like(src.as_ref())`. `make_like` reads the source through the
typed trait accessors — **no downcast needed**. Each impl starts with
`DssObjData::copy_prp_sequence_from` = the Pascal base `TDSSObject.MakeLike`
(`inherited MakeLike`), which copies the source's `PrpSequence` so a future
`Save` writes the copied properties as explicitly set.

### 4.9 Disjoint-borrow editing
`edit_active` destructures `self` into its disjoint fields (`classes`, `parser`,
`aux_parser`, `vars`, `enums`, `errors`) then destructures the active
`DssClass`, exactly the "context structs of disjoint borrows" pattern from
PORTING_PLAN §2.1. The **scratch `aux_parser`** sub-parses property values so it
never clobbers the main `parser` driving the `Edit` loop (Pascal's
`DSS.PropParser`/`AuxParser` vs `DSS.Parser`).

### 4.10 Modern property names only
`CommandList` is built from **modern** names (`NumHarm`, `%Mag`, …). The oracle
in this config **rejects** legacy names (`pctmag` is unknown), so the engine
matches the modern set. `%Mag` parses fine (`%` is not a delimiter).

---

## 5. Empirical oracle facts (verified, not guessed)

Settled by probing dss-python 0.15.7 directly (the project mandates empirical
resolution over guessing FPC semantics). Also recorded in the auto-memory.
- Default arrays are NIL → dump `''` (not `[]`).
- Size-allocated-but-unfilled arrays = uninitialized garbage = UB → excluded.
- Parsed arrays zero-fill short input → `(5 6)` w/ npts=3 → `[ 5 6 0]`.
- Shrinking the size prop truncates deterministically (keeps the first N).
- `%Mag` stored per-unit (scale 0.01); legacy `pctmag` is **rejected**.
- Base class auto-appends `Like` (`MakeLikeProperty`) — always dumps `''`.
- TCC_Curve props: `NPts`, `C_Array`, `T_Array`, `Like`.
- Spectrum props: `NumHarm`, `Harmonic`, `%Mag`, `Angle`, `CSVFile`, `Like`.

(Phase-1 facts still apply: truncated constants `pi=3.14159265359`,
`rad→deg=57.29577951`; FPC `Val` accepts `.5`/`5.`/`1.e3` and `$`/`0x`/`%`/`&`;
FPC `Round` is ties-to-even. dss-python's `Parser.Matrix`/`SymMatrix` wrappers
crash the process — never use them in golden generators.)

---

## 6. `TODO(compat)` / deferrals introduced this phase

- `obj/props.rs` `APPLY_ROUND`: uses `f64::round_ties_even()`. `TODO(compat)`
  notes FPC `Round`'s integer-indefinite path isn't reproduced for array
  rounding (year/point-count magnitudes are always in range).
- `util.rs` `interpret_dbl_array`: only the list-of-numbers path; `file=`/
  `dblfile=`/`sngfile=` error out (deferred with the rest of file-array I/O).
- `spectrum.rs`: `DoCSVFile` (csvfile side effect) and `SetMultArray`/`MultArray`
  are deferred — neither is observable in the property dump. `end_edit`'s
  zero-harmonic check is currently a silent no-op (Pascal `TSpectrum.EndEdit`
  raises a `DoSimpleMsg`); wire it to the error log when `EndEdit` grows a
  context (the trait method has no error-sink access yet).
- Executive: `Set`/`Get`/`Redirect`/`Compile` stubbed (see §4.7). The verb
  `CommandList` is a subset, so prefix ownership for *unimplemented* commands
  diverges from the oracle (e.g. `cl` → `Clear` here, `Close` there) until the
  full `TExecCommand` list is ported.
- `DSSClassDefs` constants (class-type categories) are not ported — nothing
  needs them until Phase 3's circuit element classes arrive.
- Many `PropType` variants and JSON paths not yet ported (see §4.6).

No new `TODO(compat)` markers were added beyond the `APPLY_ROUND` note — grep
with `rg "TODO\(compat\)"` to find all of them.

---

## 7. How to run / regenerate

```bash
# Gate (must be green before any commit)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Regenerate the Phase 2 golden (MANUAL ONLY, pinned versions in tools/golden/PIN.txt)
python tools/golden/gen_props.py     # -> tests/golden/props.json
```
Oracle pin: Python 3.12.4, dss-python 0.15.7, dss-python-backend 0.14.5
(the same dss_capi release vendored in `.inputs/dss_capi`). `python` works in
this environment; the `py` launcher is broken — use `python` directly.

---

## 8. Next session — start Phase 3 (the vertical slice)

Goal: the thinnest complete path **parse → build circuit → Y matrix → solve →
voltages**, validated to 1e-9/1e-6 rel against the oracle. From PORTING_PLAN
§Phase 3:
- `Bus.pas`, `Terminal.pas`, `Circuit.pas` (subset: AddCktElement, bus/node
  mapping, ProcessBusDefs), `CktElement.pas` (full base), `PDElement`/`PCElement`
  bases, `VSource.pas`, `Line.pas` (R1/X1/R0/X0/C1/C0 + rmatrix), `Load.pas`
  (all 8 models), `Ymatrix.pas` (full rebuild), `Solution.pas` subset
  (SnapShotInit/SolveSnap/DoNormalSolution/Converged/SolveSystem/…),
  `CalcVoltageBases`, and `Set voltagebases/mode/maxiterations/tolerance`.
- Make `dss-sparse` production-ready (faer behind the KLUSolve-shaped API).
- **Gate** `tests/golden_slice.rs`: 2-bus exact to 1e-9; IEEE13-flat to 1e-6;
  iteration counts equal; all 8 load models.

Phase-3 enabling facts:
- The circuit-solve golden harness already exists: `crates/dss-core/tests/harness/mod.rs`
  (schema-1 `Golden` loader + `Tolerances` + `assert_complex_close`), and
  Phase-0 goldens for IEEE13/34/37/123 are committed under `tests/golden/`.
  `tools/golden/generate.py` + `cases.json` generate them.
- The executive's `circuit`/`solution` stubs and the `Set`/`Get` stubs are the
  first things Phase 3 will replace.
- Property types Phase 3 will need that aren't ported yet: `Bus`/`BusOnStructArray`,
  `DoubleSymMatrix`, `Complex`, `DSSObjectReference`, `Enabled`, struct-array
  variants — extend `obj/props.rs` as each element needs them.

---

## 9. Outstanding action

**Phase 2 is committed on `phase-2-object-model`** (branched off `main`); the
gate was green at commit time. Remaining action: review + merge to `main`,
then start Phase 3 (§8).

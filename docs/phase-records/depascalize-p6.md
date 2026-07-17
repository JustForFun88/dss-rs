# DE_PASCALIZE P6 — Case-fidelity: Unicode `to_lowercase` → ASCII

Stratum [A], bit-neutral. Pascal `AnsiLowerCase`/`UpperCase` is byte-based; Rust
`str::to_lowercase`/`to_uppercase` are Unicode-aware (full case folding, locale-free
but e.g. `ß`→`ss`, non-ASCII lowercasing). On identifier paths (HashList keys,
CommandList, DssEnum registry, class/property/element/bus-name matching) this could
mis-key a non-ASCII name. Fix: `to_ascii_lowercase()` / `to_ascii_uppercase()` on
every *identifier* path. Report/display text and error-message strings keep the
Unicode form (they are not lookup keys; leaving them is the judgment rule).

Zero behavior change on the corpus — all corpus identifiers are ASCII, for which
`to_ascii_*` and `to_*` are identical. The full gate proves equivalence.

## Metrics

- Before: **175** `to_lowercase()`/`to_uppercase()` occurrences across 83 files
  (`rg -n "to_lowercase\(\)|to_uppercase\(\)" crates/*/src`). The plan's 113/49
  figure was stale.
- Converted: **125** identifier-path sites across **53 files**
  (`git diff --stat`: 125 insertions / 125 deletions, 1:1 line swaps; `git diff
  --check` clean — no EOL/whitespace churn).
- Left as-is: **50** sites (report/display text + error messages + test assertions).

## Left-as-is sites (report text / non-identifier), by reason

- **Report/Show/Export display** (`to_uppercase` on bus/element names purely for
  printed output — Pascal `UpperCase` at the report boundary): `report/format.rs`
  251/252; `report/show/*` (currents, elements, fault_study, bus_powers, voltages,
  powers, meter_zone); `report/export/*` (bus_coords, fault_study, profile,
  reliability, registers, unserved, loads, seq_z, seq_voltages, voltages,
  voltages_elements, y_matrix, ynode_list); `circuit/circuit.rs:786`.
- **CIM UUID formatting** `cim/mod.rs:58,69` — `to_uppercase` on a hyphenated GUID
  string for RDF `rdf:ID` output (always ASCII hex; display, not a lookup key).
- **Executive/event display**: `solution/event_log.rs:84` (`action.to_uppercase()`
  for the event-log line).
- **Error-message text** (lowercases a name only for a diagnostic string, not a
  lookup): `exec/get_cmd.rs:508` (state-variable not found), `elements/pc/dyneq_pce.rs:145`
  (DynamicExp output-var not found), `exec/report.rs:1975` (bus not found).
- **CSV filename** `exec/report.rs:1264` — `to_uppercase` on element name for the
  default export filename (filesystem output, not an engine lookup key).
- **Test assertions** (compare against ASCII literals / substrings; not production
  identifier paths): `cim/tests.rs:137`, `exec/tests/ncim.rs:400/404`,
  `exec/tests/report.rs` (5), `exec/tests/select.rs` (3),
  `elements/general/xy_curve/tests.rs:130`, `elements/general/temp_shape/tests.rs:245`.

## Notable converted paths

- Core keying: `support/hashlist/mod.rs`, `support/hashlist/thash_dump.rs`,
  `support/command_list/mod.rs`, `obj/dss_enum/enum_def.rs`,
  `obj/props/class_props/parse.rs` (enum `string_to_ordinal`, ObjectRef name,
  StringList lowercasing), `dss-parser/src/vars.rs`, `dss-parser/src/parser/value.rs`.
- Class/element/bus lookups: `exec/command.rs`, `exec/helpers.rs`, `exec/view.rs`,
  `exec/registry.rs`, `exec/batchedit.rs`, `exec/construct.rs`, `exec/solve.rs`,
  `exec/report.rs` (parse params only), `exec/plot.rs`, `exec/distribute.rs`,
  `exec/set_cmd.rs`, `exec/get_cmd`(n/a), `exec/reduce.rs`, `exec/reconductor.rs`,
  `exec/uuids_cmd.rs`, `exec/tearing.rs`, `exec/tearing_save.rs`,
  `exec/diakoptics/{mod,engine,solve,matrices}.rs`.
- Element name normalization at construction (`DssObjData::new(name.to_ascii_lowercase())`):
  all `elements/general/*` shape/curve/geometry/conductor_data classes,
  `elements/ckt.rs`, `circuit/{circuit,bus}.rs`, `elements/general/dynamic_exp.rs`.
- Sub-identifier keys: `cim/power_xfmr.rs`, `cim/export.rs`,
  `report/save/dump/commands.rs` (help-catalog keys),
  `report/export/json/circuit.rs`, `report/show/losses.rs` (class-aggregation match),
  `support/line_units/mod.rs` (unit keyword), control `monitor_variable` /
  `voverride_bus_name` normalization.

## Deferrals / escape-protocol sites

None. Every identifier path was converted; no site required leaving the old code in
place.

## Audit settlement (orchestrator note, 2026-07-17)

Both auditors returned PASS with note-severity observations only; the fix agent
exited before writing the settlement, recorded here instead:

- **(code) `to_ascii_lowercase` is not byte-identical to FPC `AnsiLowerCase` for
  high bytes (>=0x80)** — acknowledged, plan-sanctioned. DE_PASCALIZE P6
  prescribes ASCII folding as the byte-based target; the *Unicode* folding was
  the latent divergence (multi-byte case folds could mis-key lowercase-keyed
  registries). High-byte single-byte folds differ per Windows codepage and are
  not corpus-reachable; permanent semantics, not a bug.
- **(tests) zero golden/corpus/tolerance churn; symmetric 125-add/125-remove
  diff** — confirms bit-neutrality; no action.

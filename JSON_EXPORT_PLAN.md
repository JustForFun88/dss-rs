# JSON_EXPORT_PLAN — porting `Obj_ToJSON` / `Batch_ToJSON` / `Obj_Circuit_ToJSON_`

> **Status: DEFERRED, ready to execute.** Authored 2026-07-09 by the opus spec
> agent of the WPG.17 ultracode closure round (workflow `wpg17-deporting`,
> label `spec:json-output`); every Pascal citation was read from the vendored
> source and every empirical claim probed on the pinned oracle in that
> session. The port itself was deferred by the user ("нет времени — сделаем
> позже"); this file preserves the implementation-ready spec as the plan of
> record. Cross-referenced from `GAPS_PLAN.md` §WPG.17 closure addendum.
>
> **Rules of engagement:** same as every plan — the source-integrity gate
> (`PLAN_SEQUENCE.md` §step 0) runs first; Pascal is the spec; the pinned
> dss-python oracle (`tools/golden/PIN.txt`) settles every behavior question;
> `TODO(compat)`/`NOT_PORTED` discipline; the three-command gate green per
> stage; the per-step ritual (STATUS + commit + independent audit pair) from
> `PHASE8_PLAN.md` §0 applies verbatim. Suggested tiers: exec `opus-high+`
> for Stage A (the fpjson byte-fidelity + the property-selection deferral
> logic), `opus-medium+` for Stage B; audits `opus-high+` minimum.
>
> **Why this exists:** the user is attaching a GUI to the Rust library; the
> JSON surface is dss_capi's machine-readable model dump (single object,
> class batch, whole circuit) and pairs with the already-ported Plot callback
> (`Dss::register_plot_callback`).

---

# Implementation Spec: JSON output for object / batch / circuit (`Obj_ToJSON` family)

## 0. Source-integrity gate — PASSED at authoring
`.inputs/dss_capi` present (186 `.pas`). All Pascal citations below were read from the vendored source at authoring time (2026-07-09; re-verify at WP open). Pinned oracle `dss-python==0.15.7 / backend 0.14.5` used for every empirical claim.

---

## 1. What the Pascal actually does (verified citations)

### 1.1 Enum of options — `DSSObjectHelper.pas:17-32` (`DSSJSONOptions`)
```
Full=1<<0  SkipRedundant=1<<1  EnumAsInt=1<<2  FullNames=1<<3  Pretty=1<<4
ExcludeDisabled=1<<5  IncludeDSSClass=1<<6  LowercaseKeys=1<<7
IncludeDefaultObjs=1<<8  SkipTimestamp=1<<9  SkipBuses=1<<10
State=1<<11  Debug=1<<12  Edit=1<<13
```
The public C header (`include/dss_capi.h:403-417`) exposes only bits 0–10; `State`/`Debug` are commented out as **NOT IMPLEMENTED**, `Edit` is internal (JSON import only). Confirmed empirically: passing `State` (2048) is a silent no-op on the oracle.

### 1.2 Entry points
- `Obj_ToJSONData` — `CAPI_Obj.pas:626-760`: builds the `TJSONObject` for one object. The heart of property selection.
- `Obj_ToJSON_` — `CAPI_Obj.pas:762-784`: serialize. **Pretty → `FormatJSON([],2)`; else `FormatJSON([foSingleLineArray,foSingleLineObject,foSkipWhiteSpace],0)`** (compact).
- `Obj_ToJSON` — `CAPI_Obj.pas:786-789` (CDECL wrapper).
- `Batch_ToJSON` — `CAPI_Obj.pas:1201-1254`: JSON array of `Obj_ToJSONData`; `batchSize=0` → literal `"[]"`; `ExcludeDisabled` skips disabled ckt-elements; `IncludeDefaultObjs` gates `Flg.DefaultAndUnedited`. Same compact/pretty rule.
- `Obj_Circuit_ToJSON_` — `CAPI_Obj.pas:2513-2672`; helper `saveOpenTerminalsJSON` — `CAPI_Obj.pas:2470-2511`. Serializes with **`circ.FormatJSON()` = always pretty (indent 2), no flag** (`CAPI_Obj.pas:2656`).
- Per-property renderer `TDSSClassHelper.GetObjPropertyJSONValue` — `DSSObjectHelper.pas:968-1518` (the full `case ptype of`).
- Array helpers `GetDSSArray_JSON` (int / double+scale) — `DSSObjectHelper.pas:923-966`.
- Enum→JSON `TDSSEnum.OrdinalToJSONValue` — `DSSClass.pas:2374-2410` (used by Bus/Alt JSON, **not** by the object property path — object enums use `OrdinalToString`, `DSSObjectHelper.pas:1131/1145/1158/1388/1422`).
- JSON key names `PopulatePropertyNames` — `DSSClass.pas:2101-2130`: `PropertyNameJSON[i]` = raw enum literal with only `cls→Class`, `typ→Type`, `vr→Var`, taken **before** the modern-name transforms (`__`-strip, `pct→%`, `__→-`). `LowercaseKeys` uses `PropertyNameLowercase` instead (`CAPI_Obj.pas:642-649`).

### 1.3 The two property-selection modes (`Obj_ToJSONData`)
- Root object always starts `{"Name": <name>}` (or `{"DSSClass":cls,"Name":name}` when `IncludeDSSClass`; lowercase key `dssclass` when `LowercaseKeys` — `CAPI_Obj.pas:651-659`).
- **Default (not `Full`)** — `CAPI_Obj.pas:665-733`: walk **filled** properties in set-order via `GetNextPropertySet(-9999999)`; a `done[]` guard; the redundant/array-alternative *deferral* (`iPropNext2`) that prefers the singular original over a redundant array-form (`PropertyRedundantWith`, condition at l.699-722); skip `MakeLike`, `SuppressJSON` (unless also `Redundant`), `AltIndex`, `IntegerStructIndex` (l.724-729); emit via `GetObjPropertyJSONValue(...,preferArray=True)`.
- **`Full`** — `CAPI_Obj.pas:735-751`: `for iProp := 1 to NumProperties`; `SkipRedundant` skips `Redundant`; always skip `SuppressJSON`/`AltIndex`/`IntegerStructIndex`.
- Tail (both modes): if `obj is TDynEqPCE` and `UserDynInit<>NIL`, add `"DynInit"` (`CAPI_Obj.pas:752-759`) — **scoped out**, see §6.

### 1.4 Per-property JSON value — the `case` matrix (`DSSObjectHelper.pas:1009-1516`)
Verbatim behaviors to port (Rust `PropType` in parens):
- `preferArray` + `PropertyArrayAlternative[Index]<>0` → recurse into the array-form property (l.988-998).
- `Double*` (Double/DoubleOnArray/DoubleOnStruct): scalar → `NaN`/`Inf` become `TJSONNull`, else `TJSONFloatNumber(GetObjDouble)` (l.1042-1048); `preferArray` on the on-array variants → `GetDSSArray_JSON` with scale (l.1014-1038).
- `Integer`/`MappedIntEnum`/`IntegerOnStruct`: integer number; struct+preferArray → int array (l.1050-1078).
- `Boolean`/`Enabled`/`BooleanAction`: `TJSONBoolean(GetObjInteger<>0)` (l.1079-1085).
- `ComplexProperty` / `ComplexPartsProperty`: `[re, im]` 2-array (l.1087-1099) — **Complex** in Rust.
- String / Bus / MappedStringEnum families (l.1101-1166): `OnArray`/BusOnStruct/EnumOnStruct → array of strings-or-ints (`enumAsInt`); scalar → `TJSONString(GetObjString)` or int when `enumAsInt`.
- `DSSObjectReference` (l.1167-1206): array form or scalar; **FullName vs Name** chosen by `FullNames` flag **or** `PropertyOffset2=NIL` (any-class ref) **or** `FullNameAsArray`; unset → `TJSONNull`.
- `DoubleArray`/`DoubleDArray`/`DoubleVArray` (l.1207-1250): `AllowNone`+count0 → `null`; `ReadByFunction` path; else `GetDSSArray_JSON`.
- `DoubleFArray` (l.1251-1259); `DoubleSymMatrix` (l.1260-1285, nested `[[..]]`, `/scale`); `ComplexPartSymMatrix` real/imag (l.1286-1328, `/scale`, `ScaledByFunction`); `DoubleArrayOnStruct` (l.1329-1353); `BusesOnStruct` (l.1355-1369); `MappedStringEnumArray` / `...OnStruct` (l.1370-1435); `IntegerArray` (l.1437-1444); `StringList` (l.1445-1461); `DSSObjectReferenceArray` (l.1462-1515, FullName/Name, `FullNameAsJSONArray`, `ReadByFunction`).
- Fallthrough → `Result:=False` (property omitted).

### 1.5 fpjson serialization — the byte-exact contract (**empirically pinned**)
**Floats** (`TJSONFloatNumber` default, no custom format set anywhere — confirmed by grep): FPC `Str(Double)` with leading space trimmed. Exact form:
```
[-]D.DDDDDDDDDDDDDDDDE[+-]DDD     (1 int digit, 16 fraction digits = 17 sig, 3-digit signed exponent)
12.47   -> 1.2470000000000001E+001
0.1     -> 1.0000000000000001E-001
-0.0    -> -0.0000000000000000E+000
1e30    -> 1.0000000000000000E+030
```
Rust reproduction: `format!("{:.16E}", x)` then rewrite the exponent to explicit sign + zero-pad to 3 (`E1` → `E+001`, `E-9` → `E-009`). f64 exponent is always ≤3 digits. **`NaN`/`Inf` → `null`** (never reaches the float formatter — handled at the Double arm).

**Compact mode** (Obj/Batch, no Pretty): `{"k":v,"k2":v2}`, `[v,v]`, **zero whitespace**, nested arrays `[[a,b],[c,d]]`. Insertion order preserved.

**Pretty mode** (Obj/Batch `Pretty`, AND always for Circuit): indent 2, `"key" : value` (**space-colon-space**), every object/array member on its own line, arrays fully expanded (even innermost matrix scalars one-per-line), closing bracket at parent indent. Empty object `{}` / empty array `[]` (confirm the empty-container line layout in the generator; fpjson emits them inline).

**Integers** render bare (`3`), booleans `true`/`false`, unset refs `null`.

### 1.6 JSON key naming (**empirically validated**)
JSON key = modern property name with `%→pct` and `-→__` (the inverse of `PopulatePropertyNames`). Validated by membership test across Load/Line/Transformer/Generator/Capacitor/RegControl/EnergyMeter/LoadShape/XYcurve/Storage/PVSystem: **only** systematic diff is `%X → pctX` (e.g. `%SeriesRL → pctSeriesRL`, `%Mean → pctMean`), plus the single dash property `P-TCurve → P__TCurve` (Relay). `cls/typ/vr` never surface as those literals in ported classes (already stored as `Class`/`Type`/`Var`), and no ported modern name had a stripped leading `__`. `LowercaseKeys` → `AnsiLowerCase(modern)`.

### 1.7 Circuit JSON shape (`Obj_Circuit_ToJSON_`, pinned)
Top-level pretty object: `$schema` (`ALTDSS_SCHEMA_ID` = `https://dss-extensions.org/altdss-schema/2023-12-13.schema.json`), `Name`, `DefaultBaseFreq` (float), `PreCommands` (array of strings: version/timestamp unless `SkipTimestamp`, CktModel/AllowDup/LongLineCorr conditionals, EarthModel, VoltageBases), `Bus` (unless `SkipBuses`; array of `alt_Bus_ToJSON_`), `PostCommands` (~35 `Set …` strings with their own `%-g`/`%8.2f`/`IntToStr`/`StrYorN` formats — each a TODO(compat), see l.2568-2607 incl. `saveOpenTerminalsJSON`), then one key per DSS class → array of `Obj_ToJSONData` (skipping `DefaultAndUnedited` unless `IncludeDefaultObjs`; empty class arrays omitted, l.2649-2653).

---

## 2. Rust design — files & functions to touch

The Rust side already has the mirror of every needed accessor (`get_value` in `obj/props/class_props/value.rs` uses `get_complex`, `get_matrix_part`, `get_f64_array`, `get_i32_array`, `get_struct_f64_array`, `get_struct_i32_array`, `get_struct_buses`, `get_enum_array`, `get_string_list`, `get_points`, `get_object_ref_names`, `array_size`, `get_bus_name`, `get_active_struct_bus`), the set-order tracker (`DssObjData::next_property_set` / `set_as_next_seq`, `obj/base/mod.rs:124,153`), the enum registry (`EnumRegistry::get(id).ordinal_to_string`, `obj/dss_enum/enum_def.rs:69`), and the `Dss` facade with `element_properties` (`exec/view.rs:275`) as the exact lookup+sweep template.

### 2.1 New: internal JSON value + writers — `crates/dss-core/src/report/export/json/mod.rs` (new dir)
Do **not** use `serde_json` for output — its `Number` cannot hold `1.0E+000` (default re-formats to shortest; `arbitrary_precision` is a workspace-wide behavior change) and its pretty printer uses `"k": v` not fpjson's `"k" : v`. Hand-roll a tiny ordered tree + two writers:
```rust
enum Json { Null, Bool(bool), Int(i64), Float(f64), Str(String),
            Arr(Vec<Json>), Obj(Vec<(String, Json)>) }   // Obj preserves insertion order
```
- `fpjson_float(f64) -> String` — §1.5 formatter (unit-tested against the oracle table).
- `write_compact(&Json, &mut String)` — `foSingleLine*+foSkipWhiteSpace`.
- `write_pretty(&Json, indent, &mut String)` — fpjson default `FormatJSON([],2)`.
- String escaping must match fpjson `StringToJSON` (`\"`, `\\`, `\/`? verify — fpjson does **not** escape `/`; control chars `\b\t\n\f\r`, `\uXXXX` for <0x20). Pin with a probe deck containing a name/string with a quote/backslash.

### 2.2 New: per-property JSON renderer — `crates/dss-core/src/obj/props/class_props/json.rs`
`impl ClassProps { pub fn get_json_value(&self, obj:&dyn DssObject, idx:usize, enums:&EnumRegistry, opts:JsonOpts, prefer_array:bool) -> Option<Json> }` — loop-for-loop port of `GetObjPropertyJSONValue` (§1.4), reusing the existing accessors. Declare `mod json;` in `class_props/mod.rs`. Mirror the `CONDITIONAL_VALUE`/`SILENT_READ_ONLY` guards already in `get_value`? — **No**: the JSON path has its own guard (`PropertyOffset<>-1` → maps to `SILENT_READ_ONLY`/`NOT_PORTED` returning `None`); reproduce `GetObjPropertyJSONValue`'s own control flow, not `get_value`'s.

### 2.3 New: object/batch/circuit assembly — `crates/dss-core/src/report/export/json/build.rs`
`fn obj_to_json_data(cls, obj, enums, opts) -> Json` — port `Obj_ToJSONData` §1.3 (Name/DSSClass header; Full vs default sweep using `next_property_set`; redundant/array-alt deferral; skip flags).

### 2.4 New public surface — `impl Dss` in `crates/dss-core/src/exec/view.rs` (next to `element_properties`)
```rust
pub fn obj_to_json(&mut self, full_name:&str, opts:JsonOpts) -> Option<String>   // Obj_ToJSON_
pub fn class_batch_to_json(&mut self, class:&str, opts:JsonOpts) -> Option<String> // Batch over a class (IActiveClass.ToJSON oracle surface)
pub fn circuit_to_json(&mut self, opts:JsonOpts) -> Option<String>               // Stage 2
```
Lookup/activation exactly like `element_properties` (`class_by_name` + `set_active`). `JsonOpts` = a typed struct/bitflags in `report/export/json/mod.rs` exposing bits 0–10 only; `State`/`Debug`/`Edit` are **not** representable (§6). Wire a CLI/executive entry only if the item's downstream needs it; the library methods are the contract.

### 2.5 Metadata gaps to add (PropDef / PropFlags)
These are needed for a faithful **default-mode** sweep and object-ref/array rendering. Currently absent:
- `PropDef.redundant_with: usize` (default 0) — Pascal `PropertyRedundantWith`. **61 assignments across ~13 files.**
- `PropDef.array_alternative: usize` (default 0) — Pascal `PropertyArrayAlternative`. **41 assignments.**
- New `PropFlags`: `ALT_INDEX`, `INTEGER_STRUCT_INDEX`, `ON_ARRAY`, `FULL_NAME_AS_JSON_ARRAY` (bit 32+ metadata band, joining existing `SUPPRESS_JSON`/`REDUNDANT`). `FULL_NAME_AS_ARRAY` if any Obj_* array path needs it.
- `PropDef.json_name: Option<&'static str>` — only where key ≠ `name.replace('%',"pct").replace('-',"__")`. Given §1.6, prefer a **derivation helper** `json_name(&self)` computing that replacement, and add the explicit override field solely for the lone `P-TCurve`→`P__TCurve` (or accept the derivation, which already yields `P__TCurve`). **Recommendation: pure derivation, no per-prop field** — validated to cover 100% of probed classes. Store an override only if a golden later contradicts it.

Populate `redundant_with`/`array_alternative` + the four flags **only for the classes each stage's goldens cover** (staged), not all-at-once.

---

## 3. Test & gate design

### 3.1 Unit tests (inline `#[cfg(test)]`)
- `report/export/json/mod.rs`: `fpjson_float` against the full §1.5 oracle table (0, ±1, 12.47, 0.1, 1e-9, 1e30, 1e-30, -0.0, 123456789.123456789) — byte-exact.
- Compact & pretty writers: nested arrays, empty object/array, string escaping — against small hand-built trees whose expected bytes come from an oracle probe committed as a comment.
- `class_props/json.rs`: one arm per `PropType` on a synthetic object (Null-on-NaN, complex 2-array, matrix nesting, enum string vs `EnumAsInt`, objref Name vs FullName, `AllowNone`→null).

### 3.2 Golden generator — `tools/golden/gen_json.py` (clone `gen_reports.py`)
`check_pin` first. For each micro deck + **one IEEE feeder (IEEE13)**, capture the oracle bytes into `tests/golden/json/`:
- **Single object**: `d.ActiveCircuit.SetActiveElement(name)` → `api.get_string(lib.DSSElement_ToJSON(opts))` (the low-level path proven at authoring; `IDSSElement` is not surfaced as an attribute in 0.15.7).
- **Batch**: `d.SetActiveClass(cls); d.ActiveClass.ToJSON(opts)`.
- **Circuit** (Stage 2): `d.ActiveCircuit.ToJSON(opts)` — always with `SkipTimestamp` for determinism.
- Options combos per capture: `0` (default/compact), `Full`, `Full|Pretty`, `EnumAsInt`, `FullNames`, `Full|IncludeDSSClass`, `LowercaseKeys`.
- **Byte-exact goldens, captured pre-solve** (JSON export is a model dump of parsed input properties, not solved results → no faer-vs-KLU last-ULP exposure; this is stronger than the §2.3 token-compare used for numeric reports and is justified precisely because the values are exact parses). Store raw `.json` bytes + a `meta.json` deck (mirror `gen_reports.py` meta pattern) so the Rust side replays the identical deck.

### 3.3 Golden driver — `crates/dss-core/tests/golden_json.rs` (thin, over the harness)
Replay each deck through `Dss`, call `obj_to_json`/`class_batch_to_json`/`circuit_to_json` with the same opts, and assert **byte-equality** with the committed golden. Add to the mandatory `cargo test --workspace`.

### 3.4 New-deck validation (GAPS_PLAN §3) — only if any deck is a *new corpus deck*
The micro-decks here are golden fixtures (`tests/golden/json/meta`), not live-gate corpus decks, so §3's three-step proof is not triggered. **If** a deck is added under `tests/corpus/{...}` for the live gate, it must: (1) compile+solve+converge on the pin; (2) bit-identical fingerprint across **two** oracle processes; (3) feature-sensitivity (removing the JSON-exercised feature changes oracle output); proof recorded in that family's `manifest.json` note. Not expected for this item.

---

## 4. Staging (the matrix is too large for one stage)

**Stage A — core renderer + single object + batch** (bulk of the value):
- §2.1 JSON tree + float formatter + both writers; §2.2 `get_json_value` full `PropType` matrix; §2.3 `obj_to_json_data`; §2.4 `obj_to_json` + `class_batch_to_json`; §2.5 metadata (`redundant_with`, `array_alternative`, 4 flags) for the covered classes only.
- Gate A: §3.1 unit + §3.2/§3.3 goldens for single-object (all option combos) and batch, over micro-decks + IEEE13 element samples.
- Size: **large** (~the whole `case` matrix + per-class metadata data-entry for covered classes).

**Stage B — whole-circuit sweep** (`circuit_to_json`):
- `saveOpenTerminalsJSON`, the ~35 `PostCommands` with their exact `%-g`/`%8.2f`/`IntToStr`/`StrYorN` formats (**each a TODO(compat)**), `PreCommands`, `Bus` array (needs `alt_Bus_ToJSON_` — a bus-level renderer; check whether a bus JSON path must be ported too — **additional scope**), class sweep with `DefaultAndUnedited` filtering.
- Requires a Rust `DefaultAndUnedited` object flag (absent today — `DssObjData` has no such flag) and confirmation of which default objects the Rust registry auto-creates.
- Gate B: circuit goldens (IEEE13, `SkipTimestamp`, default + `Full`/`Pretty`/`SkipBuses`).
- Size: **medium-large**, with a real dependency on bus JSON + default-object flagging.

---

## 5. Quirks / risks to reproduce as TODO(compat)
- **Float format** `1.xxxE+0dd` (§1.5) — a deliberate fpjson fidelity, tag the formatter `TODO(compat)` (clean fix = shortest round-trip). This is the single biggest byte-compare risk.
- **PostCommands numeric formats** (`%-g`, `%8.2f` → `"    1.00"`, `%-.4g`) — each `TODO(compat)` (Stage B).
- **JSON key = raw enum name** (`pctR` not `%R`) — reproduce via derivation; document.
- **Redundant/array-alternative deferral** (`iPropNext2`) — subtle ordering; port loop-for-loop and pin with a Transformer/AutoTrans golden (the classes that exercise `RdcOhms`/`MaxTap`/`Rneut` redundancy, `CAPI_Obj.pas:696-698`).
- **`OrdinalToJSONValue` out-of-range → null** is *not* on the object path (object enums use `OrdinalToString`); do not wire it here.
- **`-0.0` sign preserved**; **NaN/Inf → null** — both pinned.

## 6. Stays NOT_PORTED (loud errors / absent surface), owners named
- **`Obj_Circuit_FromJSON_`** (JSON **import**, `CAPI_Obj.pas:2674-end`, incl. `loadClassFromJSON`, `busFromJSON`, `FillObjFromJSON`, `SetObjPropertyJSONValue`) — **out of scope**. Follow-up: *"AltDSS JSON import"* — owner: the next WP after this item. No stub; simply not exposed.
- **`CAPI_Schema.pas`** (JSON **schema** export) — **out of scope**. Follow-up: *"CAPI JSON schema export"* — owner: same lead. The `Units_*`/`NoDefault`/`DynamicDefault`/`REQUIRED_IN_SPEC_SET` flags already carried in `PropFlags` are its future inputs; leave inert.
- **`DSSJSONOptions.State` / `Debug`** — TODO upstream, silent no-op on the oracle, commented out of the public C enum. The Rust `JsonOpts` typed surface **omits them** (cannot be passed) — the faithful equivalent of "not implemented". If a raw-bits entry point is ever added, it must **error loudly** (`NOT_PORTED`), never silently ignore.
- **`DSSJSONOptions.Edit`** — import-only internal flag; not on the export surface.
- **`TDynEqPCE` `"DynInit"` tail** (`CAPI_Obj.pas:752-759`) — only fires for Generator/PVSystem/Storage with a `UserDynInit` DynamicExp attached (rare). Defer as a named follow-up *"DynInit JSON tail"* folded into Stage A's owner; until then, emitting it is skipped — acceptable because the goldens will not set `UserDynInit`. If a covered deck ever does, that's a loud golden mismatch, not a silent wrong answer.

## 7. Honest size estimate
Stage A: **large** — the ~25-arm property matrix is mechanical (accessors exist) but the per-class `redundant_with`/`array_alternative`/flag metadata (61+41 assignments) plus the hand-rolled fpjson writers and float format make it a full WP. Stage B: **medium-large**, gated separately, and pulls in bus-JSON + default-object flagging as sub-dependencies. Recommend shipping Stage A (object+batch) first behind its own green gate, then Stage B.

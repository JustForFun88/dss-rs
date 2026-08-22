# G1.1 — all_properties on the r4133 channel: seeding triage (2026-08-08)

**Verdict: KILL CRITERION MET — r4133 property surface diverges materially.
Needs its own plan. G1.1 blocked; all scratch code reverted (clean tree).**

## Method

- Anchors re-measured before editing (all as briefed): mask at
  `corpus_gate/scheduler.rs:361-362`, seeding mask `:715-716`, force-enable
  region `:102-114`; capture machinery present at `dss-epri/src/capture.rs:308/:620`.
- Implemented the unmask (scheduler + channel-aware `harness::compare_all_properties`
  with `PropsChannel` and an empty `PROPS_R4133` allowlist), then measured the
  full divergence surface with a scratch census test (removed before revert):
  every live case (non-large, non-pending/abort/defer), Rust `?`-surface vs the
  r4133 `epri-worker` all_properties capture, collecting EVERY divergent cell
  instead of stopping at the first. Full data: `tmp/r4133_props_census.json`
  (1 055 880 rows), category extracts in `tmp/g1_1_{structural_pairs,numeric_pairs,shape}.txt`.
- Control: the capi_v0145 channel is UNAFFECTED — verified on
  `controls:regcontrol/regcontrol_sym.dss` (capi: match; r4133: diverge).

## Headline numbers

| measure | count |
|---|---|
| cases with ≥1 r4133 property divergence | **433** (of ~512 live) |
| structural value pairs (skeleton mismatch) | **209** (960 129 cells) |
| numeric value pairs (above case tier floor) | **94** (95 317 cells) |
| property-table shape gaps | **5 classes** |
| oracle errors | 5 = the 4 known r4133 `#303` skips + `Test/CapControlFollow.dss` (engines=capi_v0145, never sent to r4133 — benign) |

Kill criterion for G1.1: ">~15 new ledger entries, or any entry that cannot be
pinned". The numeric side alone needs far more than 15 per-case entries, and the
209 structural pairs CANNOT be ledgered at all — they are rendering-convention
deltas, not numeric divergences, and "ours-is-right" pins make no sense for them.

## Divergence classes (what a follow-up plan must bridge)

### S1. Boolean rendering (46 pairs)
Port/dss_capi render `Yes`/`No`; r4133 renders `true`/`True`/`false`/`YES`/`NO`/
`yes`/`no`/`n`/``''`` — inconsistent even within r4133 (per-getter choice).
Examples: `*.enabled` (every class, `Yes` vs `true`), `line.switch` (`Yes`/`True`),
`autotrans.sub` (`No`/`n`), `monitor.ppolar` (`Yes`/`YES`), `reactor.parallel` (`No`/`NO`).

### S2. Enum rendering (case + spelling)
Case: `load.status` `Variable`/`variable`, `autotrans.conn` `delta`/`Delta `,
`storage.state` `Discharging`/`DISCHARGING`, `invcontrol.mode` `Voltvar`/`VOLTVAR`,
`generator.dispmode` `Price`/`price`. Spelling: `vsource.scantype/sequence`
`Positive`/`Pos`, `isource.scantype/sequence` `Positive`/`pos`,
`storagecontroller.modedischarge` `Schedule`/`UNKNOWN`,
`storagecontroller.modecharge` `I-PeakshaveLow`/`I-PeakShaveLow`.

### S3. Identifier case preservation (~103 pairs are case-only diffs)
EPRI preserves the as-declared case of names; the port (THashList semantics,
matching dss_capi) lowercases. Every name-valued prop: `bus1` (`b3.1`/`B3.1`),
`element`/`monitoredobj`/`switchedobj` (`Line.lt`/`line.lt`), shape refs
(`load.daily` `load1`/`Load1`, `pvsystem.daily` `myirrad`/`MyIrrad`,
`vsource.daily` `vshape`/`Vshape`), `xfmrcode` (`regleg`/`RegLeg`),
`capcontrol.capacitor` (`c1`/`C1`).

### S4. Array/empty rendering (28 pairs)
dss_capi `GetDSSArray` form `[ 400]` vs r4133 comma forms `[400,]`, `[1, ]`,
specials `energymeter.option` `[E, R, C]`/`(E, R, C)`, `energymeter.peakcurrent`
`[ 400 400 400]`/`((400, 400, 400))`, `recloser.recloseintervals`
`[ 0.5 1]`/`(0.5, 1, )`, `sensor.currents` `[ 340 130 430]`/`340 130 430`,
`relay.recloseintervals` `[NONE]`/`NONE`. Empty conventions: `''` vs `[]` vs `()`
vs `NONE` (`transformer.bhcurrent/bhflux`, `pvsystem.dynout/userdata`,
`line.cncables/tscables/conductors/wires`, `load.zipv`, `vccs.bp1/bp2`).

### S5. Display-default strings (r4133 `InitPropertyValues` overrides)
r4133 classes override inherited display defaults the 0.14.5 line does not:
Transformer `pctperm` 0/100, `repair` 0/36 (r4133 Transformer.pas:1914-1919),
`gicsource.spectrum` `''`/`default`, `upfc.*limits` (`265`/`''` etc.),
`upfccontrol.basefreq/enabled` (`60`/`Yes` vs `''`), `pvsystem/storage.amplimit`
(`-1`/`''`), `storage.dynadll` (dss_capi's hardcoded default path vs `''`),
`storagecontroller.kwactual/kwhactual/kwtotal/kwhtotal` (`''`/values),
`indmach012.pf` (`''`/`0.909545`), `relay.action`/`energymeter.action`
(`''`/`closed`/`clear`).

### S6. State/convention pairs that need individual source investigation
`swtcontrol.action/state/normal` (scalar vs per-phase array + action `close`/`open`),
`relay.normal/state` (3-phase array vs `[closed, ]` 1-entry), `monitor.mode`
(`17` vs `1 16 +` decomposition render), `invcontrol.monbus/monbusesvbase/
pvsystemlist/vsetpoint/monvoltagecalc`, `isource.bus2/yearly`, `reactor.bus2`,
`load.yearly`, `line.linecode/spacing/units` (`''`/`98`, `''`/`sp`, `none`/`kft`),
`expcontrol.derlist` (`[PVSystem.pv]`/`[pv]`).

### N1. Numeric display precision (Delphi `%-.5g`/`%-.6g` getters)
The storage/pvsystem-display-precision class, generalized: `vsource.isc1/isc3/r1/
x1/r0/x0/mvasc1/mvasc3/angle/basekv/puz*` (Vsource.pas:1327-1343 `%-.5g`),
`transformer.normamps/emergamps/kv/kva/kvs/kvas/normhkva` (21k+ rows),
`load.pf` (5 213 rows), `line.length/b1/c1/cmatrix/...`, `reactor.lmh`,
`generator.kvar/kv`, `storage.kwhstored/%stored/%charge/%discharge`,
`storagecontroller.kwneed/kwtarget`, `capacitor.cuf` (display part),
`autotrans/kv-family`, `windgen.kva/mva`. Rel magnitudes 1e-9..6e-5.

### N2. Genuine value jumps (each needs root-cause before any ledger entry)
- `transformer.pctperm/repair` 0 vs 100/36 — display-default (S5), 21k rows each.
- `fault.pctperm` 100 vs 0 (n=389), `gictransformer.pctperm` 100 vs 0 (n=22).
- `reactor.kvar` 100 vs 1200 on `asymmetric:gic/*` (n=607 subset).
- `pvsystem/storage.%pminnovars/%pminkvarmax` 0 vs −1 (n≈460 each) — default gap.
- `invcontrol.lpptau` 0.001 vs 0.0, `invcontrol.risefalllimit` 1 vs 0 (n=257 each).
- `swtcontrol.delay` 0.25 (deck-set) vs 120 (r4133 ignores/resets? — investigate).
- `regcontrol.remoteptratio` (166 vs 60), `regcontrol.tapnum` (5 vs 1),
  `autotrans.tap/taps` (1.03125 vs 1) — concentrated in `controls:autotrans/*`
  (AutoTrans regulator class — cursor vs real state to determine).
- `generator.model` (2 rows), `generator.kw/kvar` (dispatched-value deltas up to
  ×54), `windgen.kvar` (×1000), `storage.kva/kw/kwrated` (0.667-rel subset),
  `line.r1/x1/rmatrix/xmatrix` (up to 1.5-rel subset), `capacitor.cuf/normamps/
  emergamps` (up to 1e6 subset — includes the G2.5 `makeposseq_shunt` Cuf unit
  divergence now visible on r4133), `gictransformer.r2` (the G2.5 %R2 exact pair
  now visible on r4133: 0.09522 vs 0.12696), `autotrans.kv-family` single rows,
  `storagecontroller.kwneed` (1.38e-3 max — display).

### G. Property-table shape gaps (PROPS_R4133 territory + port gaps)
- `Generator`: r4133 registers `Rneut`/`Xneut` as DELETED-but-listed props
  (generator.pas:441-442, "Removed due to causing confusion"; edit → error 5611).
  Port table lacks them. 273 generator instances.
- `Sensor`: r4133 prop 13 `action` (Sensor.pas:183) absent from port.
- `AutoTrans`: r4133 prop 39 `XfmrCode` (AutoTrans.pas:329) absent from port
  (port's Transformer has it; AutoTrans does not).
- `WindGen`: r4133 props 18/19 `UserModel`/`UserData` (WindGen.pas:391-394)
  absent from port (WASM-usermodel surface).
- `GenDispatcher`: port has `weights`; r4133 registers `Weights` at index 7 but
  `NumPropsThisClass = 6` (GenDispatcher.pas:92,133) so AllPropertyNames omits
  it → the one genuine PROPS_R4133 allowlist row (Rust-side extra).

## Why this cannot be squeezed into G1.1

1. Ledger discipline (plan §1.1(e)): every ours-is-right divergence gets an
   expected-value pin. N1 (display precision) would need per-case num_rel
   property scopes on 90+ pairs across 400+ cases — order(s) of magnitude over
   the ~15-entry limit. N2 needs per-pair root-cause first (some may be port
   bugs to FIX, not ledger).
2. S1–S5 are not ledgerable at all: `property` scopes accept numeric envelopes
   or exact pairs, and there is nothing to "pin" — both renderings are valid
   conventions of their engine line (port matches the pinned capi oracle's FPC
   conventions byte-exactly; r4133's Delphi conventions are its own).
3. A workable r4133 property compare needs a DESIGN decision: a channel-aware
   value-normalization layer (boolean/enum/array/empty/name-case conventions,
   per `(channel, class, prop)` evidence rows, proven non-loosening in
   TOLERANCE_NOTES) plus the four port-gap props and the r4133 display-default
   inventory. That is its own work package.

## Artifacts
- `tmp/r4133_props_census.json` — full census (1 055 880 rows)
- `tmp/g1_1_structural_pairs.txt`, `tmp/g1_1_numeric_pairs.txt`, `tmp/g1_1_shape.txt`

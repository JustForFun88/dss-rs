# tools/golden/report_decks — fixture decks for the report goldens

Fixture decks for the report verbs that gate on **file output**, not on the
live model compare: Dump, the Save forms, Interpolate / Distribute / Uuids.
Validated on the pinned dss-python (2026-07-05, two separate processes,
bit-identical produced files). `tools/golden/gen_reports.py` wires each deck
into its golden (the deck text goes into the golden `.meta.json`, the single
source both engines replay), and `golden_reports.rs` holds the matching test.

| Deck | Feeds | Notes |
|---|---|---|
| `dump3.dss` | `dump <class> [debug]` for Fault/Vsource/UPFC/RegControl/Monitor/EnergyMeter/Spectrum + bare `dump`, `dump debug`, `dump solution`/`buslist`/`devicelist`/`commands`/`alloc` | Capacitor and Reactor deliberately absent (dss_capi garbage reads — see below) |
| `dump_capacitor.dss` | the Capacitor override goldens | compared with the `~ CMatrix=(`/`~ FaultRate=`/`~ pctPerm=` lines dropped on both sides |
| `save_forms.dss` | `save` (meters → `MTR_em1.csv`), `save voltages`, `save <class>`, `save circuit dir=` | 4-step daily so registers are non-trivial |
| `interp.dss` | `interpolate` gated via `export buscoords` | anchors src/b1/b5/c2; b2/b3/b4 + c1 interpolated |
| `distrib.dss` | `distribute` Proportional/Uniform/Skip/what=Load | `how=Random` is RNG-carried in OpenDSS — never golden-gated |
| `uuids.dss` + `uuids_pre.csv` | `uuids file=` + `export uuids` | preloads EVERY object incl. the 3 auto hashed keys; `@FIXTURES@` → this dir at replay |

**Probe-proven (2026-07-07):** `dump buslist` prints only the
`LINEAR LISTING...` section while `dump devicelist` prints the full bucket
dump — a *structural* difference, not an allocation quirk: `BusList` is a
Pascal `TAltHashList` (linear-only `DumpToFile`) while `DeviceList` is the
classic bucketed `THashList` (`Circuit.pas:107-109`; port in
`support/hashlist/thash_dump.rs`). Also probe-proven: FPC renders floats via
Grisu1 with a half-away-from-zero string re-round (`util::fmt_g` port; see the
20k-value battery note there) — `loadshape.default`'s `Mean` prints
`0.825828333333334` where correctly-rounded `%.15g` gives `…333`.

**Probe-proven dss_capi garbage reads (do NOT chase as bugs of this engine):** in this
pinned dss_capi build, `DumpProperties` prints ASLR-dependent uninitialized
memory (denormals ~1e-305, different every process) for **Capacitor**
`CMatrix`/`FaultRate`/`pctPerm` (always) and **Reactor** `FaultRate`/`pctPerm`
(only when an EnergyMeter exists in the circuit). Line/Transformer/Fault/Load
dumps stay deterministic. This engine renders the correct values: a
nondeterministic garbage read is an OpenDSS bug, and OpenDSS bugs are never
reproduced.

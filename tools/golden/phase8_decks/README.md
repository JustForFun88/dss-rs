# tools/golden/phase8_decks — fixture decks for the remaining Phase-8 goldens

Oracle-validated (2026-07-05, two separate oracle processes, bit-identical
produced files) fixture decks for the PHASE8_PLAN.md work packages that gate
on **file output**, not on the live model compare — Dump step 3 (WP8.5),
Save forms (WP8.5), Interpolate / Distribute / Uuids (WP8.6). The WP that
ports each verb wires its deck into `tools/golden/gen_phase8.py` (the deck
text goes into the golden `.meta.json`, the single source both engines
replay) and adds the matching `golden_phase8.rs` test; until then the decks
sit here, pre-validated.

| Deck | Feeds | Notes |
|---|---|---|
| `dump3.dss` | `dump <class> [debug]` for Fault/Vsource/UPFC/RegControl/Monitor/EnergyMeter/Spectrum + bare `dump`, `dump debug`, `dump solution`/`buslist`/`devicelist`/`commands`/`alloc` | Capacitor and Reactor deliberately absent (upstream garbage reads — see below) |
| `dump_capacitor.dss` | the Capacitor override goldens | compared with the `~ CMatrix=(`/`~ FaultRate=`/`~ pctPerm=` lines dropped on both sides |
| `save_forms.dss` | `save` (meters → `MTR_em1.csv`), `save voltages`, `save <class>`, `save circuit dir=` | 4-step daily so registers are non-trivial |
| `interp.dss` | `interpolate` gated via `export buscoords` | anchors src/b1/b5/c2; b2/b3/b4 + c1 interpolated |
| `distrib.dss` | `distribute` Proportional/Uniform/Skip/what=Load | `how=Random` is RNG-carried upstream — never golden-gated |
| `uuids.dss` + `uuids_pre.csv` | `uuids file=` + `export uuids` | preloads EVERY object incl. the 3 auto hashed keys; `@FIXTURES@` → this dir at replay |

**Probe-proven upstream garbage reads (do NOT chase as port bugs):** in this
pinned dss_capi build, `DumpProperties` prints ASLR-dependent uninitialized
memory (denormals ~1e-305, different every process) for **Capacitor**
`CMatrix`/`FaultRate`/`pctPerm` (always) and **Reactor** `FaultRate`/`pctPerm`
(only when an EnergyMeter exists in the circuit). Line/Transformer/Fault/Load
dumps stay deterministic. The Rust port renders the correct values (a
nondeterministic garbage read is never reproduced — CLAUDE.md known-bug rule);
WP8.5 step 3 records the full investigation in `investigations/`.

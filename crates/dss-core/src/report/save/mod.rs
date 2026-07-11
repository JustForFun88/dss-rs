//! `Save` / `Dump` — the OpenDSS *script* / property-dump output layer
//! (Pascal `Executive/ExecHelper.pas` `DoSaveCmd`/`DoPropertyDump`,
//! `Common/Circuit.pas` `Circuit.Save`, `Common/Utilities.pas` `WriteClassFile`,
//! `General/DSSObject.pas` `DumpProperties`/`SaveWrite`).
//!
//! Unlike `Export`/`Show` (CSV / fixed-width tables), `Save`/`Dump` emit DSS
//! *script* text: `New <Class>.<name> ` + each property, assembled straight from
//! the Phase-2 property getters (`ClassProps::get_value`, the same renderer the
//! `?` query + `props_roundtrip` pin). So the formatting here is thin
//! string-joining, not reinvented number formatting (PHASE8_PLAN §2.4).

pub mod dump;
// The plan-mandated path for the Save-side serializer is `report/save/save.rs`
// (PHASE8_PLAN §WP8.5 step 4) — the inception is deliberate.
#[allow(clippy::module_inception)]
pub mod save;

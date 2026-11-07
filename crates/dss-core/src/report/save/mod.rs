//! `Save` / `Dump` — the OpenDSS *script* / property-dump output layer
//! (Pascal `Executive/ExecHelper.pas` `DoSaveCmd`/`DoPropertyDump`,
//! `Common/Circuit.pas` `Circuit.Save`, `Common/Utilities.pas` `WriteClassFile`,
//! `General/DSSObject.pas` `DumpProperties`/`SaveWrite`).
//!
//! Unlike `Export`/`Show` (CSV / fixed-width tables), `Save`/`Dump` emit DSS
//! *script* text: `New <Class>.<name> ` + each property, assembled straight from
//! the property getters (`ClassProps::get_value`, the same renderer the
//! `?` query + `props_roundtrip` pin). So the formatting here is thin
//! string-joining, not reinvented number formatting.
//!
//! That shared getter is also the *decision* both surfaces rest on: `Save` and
//! `Dump` follow one rule for every class, **`KEEP_LIVE_PINNED`** — values are the live field, membership and order are
//! the explicitly-set chain, structure guards are the union of both upstreams'
//! `SaveWrite` overrides, and no branch prints a value the engine knows to be
//! superseded. The reasoning lives in the two submodule docs ([`save`],
//! [`dump`]); the divergences from r4133's own serializer are pinned, with both
//! engines' bytes quoted, in `crate::exec::tests::report`.

pub mod dump;
// The Save-side serializer lives at `report/save/save.rs` — the inception is
// deliberate.
#[allow(clippy::module_inception)]
pub mod save;

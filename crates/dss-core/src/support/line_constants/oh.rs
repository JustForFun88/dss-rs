//! Overhead-line specialization, port of `General/OHLineConstants.pas`
//! (`TOHLineConstants`). The Pascal subclass adds no behavior over the base
//! `TLineConstants` — the overhead Carson model *is* the base — so this is a
//! plain alias. Build it with [`LineConstants::new`]; the concentric-neutral
//! ([`super::cn`]) and tape-shield ([`super::ts`]) kinds override `Calc`.

use super::LineConstants;

/// `TOHLineConstants`: identical to the base [`LineConstants`].
pub type OhLineConstants = LineConstants;

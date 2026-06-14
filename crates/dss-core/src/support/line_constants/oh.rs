//! Overhead-line specialization, port of `General/OHLineConstants.pas`
//! (`TOHLineConstants`). The Pascal subclass adds no behavior over the base
//! `TLineConstants` — the overhead Carson model *is* the base — so this is a
//! plain alias kept for naming parity with the concentric-neutral / tape-shield
//! / cable specializations that do override `Get_Zint`.

use super::LineConstants;

/// `TOHLineConstants`: identical to the base [`LineConstants`].
pub type OhLineConstants = LineConstants;

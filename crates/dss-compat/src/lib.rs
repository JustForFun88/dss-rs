#![forbid(unsafe_code)]
//! The switch of the precision-compatibility lane. Built with the feature
//! `opendss-parity`, the engine selects the kernels that reproduce the
//! precision of the OpenDSS engines (truncated constants, FPC rounding and
//! number formatting); built without it, the engine is the product. This crate
//! alone declares the feature: the other crates read [`OPENDSS_PARITY`] and
//! select a kernel with an ordinary `if`, so both kernels compile in both
//! builds.

/// `true` when the workspace is built with `--features dss-compat/opendss-parity`.
pub const OPENDSS_PARITY: bool = cfg!(feature = "opendss-parity");

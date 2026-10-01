//! The golden/oracle comparison harness of the `dss-core` integration tests:
//! the goldens and scenario loaders, the tolerance floors, the lane policy
//! (`harness::lane`), the live-capture comparators and the corpus scratch
//! copies. Test-only and never published: `dss-core` takes it as a
//! dev-dependency and each driver under `crates/dss-core/tests/` imports it
//! with `use dss_test_harness::harness;`, so the harness compiles once per lane
//! and its self-tests run once, in this crate's own lib test binary. A driver
//! links the library built without `cfg(test)`, so no `#[cfg(test)]` code of
//! the harness, its `unit:` fixtures included, ever runs in a driver's binary
//! or shares a process, a static or a counter with the corpus gate.
#![forbid(unsafe_code)]

// Two lint scopes of a test-only library whose callers are the drivers: a
// `use` that only the self-tests reach (through `use super::*`) is unused in
// this crate's non-test build (`regen.rs`'s `Path`), and `pub mod` makes the
// harness an exported surface that the exported-API lints read as a library
// API (`scratch::TreePhoto::len`), though no API is published. The lib-test
// build still checks unused imports.
#[cfg_attr(not(test), allow(unused_imports))]
#[allow(clippy::len_without_is_empty)]
pub mod harness;

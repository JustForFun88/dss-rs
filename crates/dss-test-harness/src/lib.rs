//! The golden/oracle comparison harness of the `dss-core` integration tests
//! (RETRO_FIXES_PLAN.md RF-I00-04): the goldens and scenario loaders, the
//! tolerance floors, the lane policy (`harness::lane`), the live-capture
//! comparators and the corpus scratch copies. Test-only and never published:
//! `dss-core` takes it as a dev-dependency and each driver under
//! `crates/dss-core/tests/` imports it with `use dss_test_harness::harness;`,
//! so the harness compiles once per lane and its self-tests run once, in this
//! crate's own test binary. The module is the former
//! `crates/dss-core/tests/harness/`, moved byte-for-byte; see `TESTING.md`.
#![forbid(unsafe_code)]

// Two lint scopes the move itself creates, both for code the drivers used to
// compile only with `cfg(test)` on and only as a private `mod`: a `use` that
// just the self-tests reach (through `use super::*`) is unused in this crate's
// non-test build (`regen.rs`'s `Path`), and `pub mod` makes the harness an
// exported surface that the exported-API lints read as a library API
// (`scratch::TreePhoto::len`). The lib-test build still checks unused imports.
#[cfg_attr(not(test), allow(unused_imports))]
#[allow(clippy::len_without_is_empty)]
pub mod harness;

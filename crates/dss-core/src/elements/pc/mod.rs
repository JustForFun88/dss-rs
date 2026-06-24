//! Power conversion elements (Pascal `PCElement.pas` descendants). The
//! `TPCElement` base behavior (InjCurrents into the global array, terminal
//! currents = Yprim·V − InjCurrent) lives on the [`CktElement`] trait defaults
//! and in each element; there is no separate Rust base trait.
//!
//! [`CktElement`]: crate::elements::traits::CktElement

pub mod generator;
pub mod inv_based_pce;
pub mod load;
pub mod pvsystem;
pub mod storage;
pub mod vsource;

pub use generator::Generator;
pub use inv_based_pce::{InvBasedPce, InvBasedPceData};
pub use load::Load;
pub use pvsystem::PVSystem;
pub use storage::Storage;
pub use vsource::VSource;

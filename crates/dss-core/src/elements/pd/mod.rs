//! Power delivery elements (Pascal `PDElement.pas` descendants). The
//! `TPDElement` base behavior (terminal currents = Yprim·V) is the
//! [`CktElement::get_currents`] default; there is no separate Rust base trait.
//!
//! [`CktElement::get_currents`]: crate::elements::traits::CktElement::get_currents

pub mod auto_trans;
pub mod capacitor;
pub mod fault;
pub mod fuse;
pub mod gic_transformer;
pub mod line;
pub mod reactor;
pub mod transformer;
pub mod winding;

pub use auto_trans::AutoTrans;
pub use capacitor::Capacitor;
pub use fault::Fault;
pub use fuse::Fuse;
pub use gic_transformer::GicTransformer;
pub use line::Line;
pub use reactor::Reactor;
pub use transformer::Transformer;

//! Power delivery elements (Pascal `PDElement.pas` descendants). The
//! `TPDElement` base behavior (terminal currents = Yprim·V) is the
//! [`CktElement::get_currents`] default; there is no separate Rust base trait.
//!
//! [`CktElement::get_currents`]: crate::elements::traits::CktElement::get_currents

pub mod line;

pub use line::Line;

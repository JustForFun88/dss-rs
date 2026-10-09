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

/// The solve's refusal of the element `full` when its matrix property `prop`
/// holds `len` entries where its `nphases` phases need `nphases²`, as a
/// `phases=` edit after the matrix leaves it. `None` when the order fits.
/// `respecify` names what the user gives again.
pub(crate) fn matrix_order_refusal(
    full: &str,
    nphases: usize,
    prop: &str,
    len: usize,
    respecify: &str,
) -> Option<String> {
    if len == nphases * nphases {
        return None;
    }
    let order = (len as f64).sqrt().round() as usize;
    let held = if order * order == len {
        format!("{order} x {order}")
    } else {
        len.to_string()
    };
    Some(format!(
        "{full} has {nphases} phases but its {prop} has {held} entries. Specify {respecify} \
         for {nphases} phases. Aborting solution."
    ))
}

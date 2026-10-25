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

pub(crate) use super::ckt::{array_input_text, input_text};

/// The sentence that ends the solve's refusal of an element.
pub(crate) const ABORTING_SOLUTION: &str = " Aborting solution.";

/// `name=(a |b c )`, as `?` reads a symmetric matrix of `nphases` phases
/// stored in units of `scale` (the Capacitor's microfarads), for the message
/// of a refused recalc.
pub(crate) fn sym_matrix_input_text(
    name: &str,
    vals: &[f64],
    nphases: usize,
    scale: f64,
) -> String {
    let order = crate::report::save::dump::held_matrix_order(nphases, vals.len());
    let mut s = format!("{name}=(");
    for i in 0..order {
        if i > 0 {
            s.push('|');
        }
        for v in &vals[i * order..=i * order + i] {
            let v = if scale == 1.0 { *v } else { *v / scale };
            s.push_str(&crate::util::float_to_str_ex(v));
            s.push(' ');
        }
    }
    s.push(')');
    s
}

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
         for {nphases} phases.{ABORTING_SOLUTION}"
    ))
}

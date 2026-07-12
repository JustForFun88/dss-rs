//! A-Diakoptics matrix exports 58–61 (Pascal official `ExportResults.pas:3541–
//! 3627`): `ZLL`/`ZCC`/`Contours`(C)/`Y4` as compressed-coordinate CSV. Each is
//! **silent no-op when `Solution.ADiakoptics` is false** (the Pascal body is
//! wrapped in `if ADiakoptics then …`) — that gate is enforced at the dispatch
//! (`exec/report.rs`), so these formatters only run once the matrices exist.
//!
//! `floattostr` == FPC `FloatToStr` → [`crate::util::float_to_str`].

use crate::circuit::Circuit;
use crate::support::sparse_math::SparseComplex;
use crate::util::float_to_str;

/// Shared body for the complex COO exports: `Row,Col,Value(Real), Value(Imag)`
/// per stored non-zero, in storage (insertion) order.
fn export_complex_coo(m: &SparseComplex) -> String {
    // Pascal header verbatim (note the single space before `Value(Imag)`).
    let mut s = String::from("Row,Col,Value(Real), Value(Imag)\n");
    for cd in &m.cdata {
        s.push_str(&format!(
            "{},{},{},{}\n",
            cd.row,
            cd.col,
            float_to_str(cd.value.re),
            float_to_str(cd.value.im),
        ));
    }
    s
}

/// Pascal `ExportZLL` (ExportResults.pas:3541).
pub(crate) fn export_zll(ckt: &Circuit) -> String {
    export_complex_coo(&ckt.ad.zll)
}

/// Pascal `ExportZCC` (ExportResults.pas:3563).
pub(crate) fn export_zcc(ckt: &Circuit) -> String {
    export_complex_coo(&ckt.ad.zcc)
}

/// Pascal `ExportY4` (ExportResults.pas:3585).
pub(crate) fn export_y4(ckt: &Circuit) -> String {
    export_complex_coo(&ckt.ad.y4)
}

/// Pascal `ExportC` (ExportResults.pas:3607): the Contours matrix, **real part
/// only** (`floattostr(Value.Re)`), header `Row,Col,Value`.
pub(crate) fn export_contours(ckt: &Circuit) -> String {
    let mut s = String::from("Row,Col,Value\n");
    for cd in &ckt.ad.contours.cdata {
        s.push_str(&format!(
            "{},{},{}\n",
            cd.row,
            cd.col,
            float_to_str(cd.value.re)
        ));
    }
    s
}

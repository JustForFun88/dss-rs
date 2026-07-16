//! NCIM solver report exports (Pascal `Common/ExportResults.pas`
//! `ExportJacobian` / `ExportdeltaF` / `ExportdeltaZ`): dump the last-solution
//! NCIM Jacobian and mismatch/correction vectors. All three read state left by
//! [`crate::solution::solution::ncim`] (`Set Algorithm=NCIM`); on a circuit that
//! never ran an NCIM solve the vectors are empty and the Jacobian is `None`.
//!
//! These are numeric dumps of the *last iteration's* solver state. The Jacobian
//! entries are O(1) network/injection derivatives (gated numerically vs
//! capi015); `deltaF`/`deltaZ` are the converged mismatch/correction, i.e. at the
//! ~1e-11 faer-vs-KLU floor — their per-line *values* are noise (structurally the
//! first 6 swing rows are exactly 0 and the length is `2·NumNodes + NumPVphases`),
//! so the gate on them is structural, not value-pinned.

use crate::circuit::Circuit;
use crate::report::format;

/// Pascal `ExportJacobian` (`ExportResults.pas` l.3903): factor the last NCIM
/// Jacobian and dump its triplets as `Row,Col,Value` (0-based, column-major, the
/// real part via `%.10g`). Returns `Err` with the upstream "Jacobian matrix not
/// built." message (#222) when no NCIM solve has populated it.
pub fn export_jacobian(ckt: &mut Circuit) -> Result<String, String> {
    let jac = ckt
        .solution
        .ncim_jacobian
        .as_mut()
        .ok_or_else(|| "Jacobian matrix not built.".to_string())?;
    // `FactorSparseMatrix` in Pascal only compresses duplicate stamps; our
    // `coo_entries` assembles + sums duplicates and emits column-major 0-based
    // (row, col) — the exact content KLU `GetTripletMatrix` returns post-factor.
    let (rows, cols, vals) = jac
        .coo_entries()
        .map_err(|e| format!("Export Jacobian: {e}"))?;
    let mut s = String::from("Row,Col,Value\n");
    for i in 0..vals.len() {
        // Pascal `Format('%d,%d,%.10g', [RowIdx[i], ColPtr[i], cVals[i].re])`.
        s.push_str(&format!(
            "{},{},{}\n",
            rows[i],
            cols[i],
            format::g(vals[i], 10)
        ));
    }
    Ok(s)
}

/// Pascal `ExportdeltaF` (`ExportResults.pas` l.3945): one `%.10g` per line, the
/// last NCIM mismatch vector. Returns `None` (silent exit, no file) when empty —
/// Pascal `if Length(NCIM_deltaF) = 0 then Exit`.
pub fn export_delta_f(ckt: &Circuit) -> Option<String> {
    export_real_vec(&ckt.solution.ncim_delta_f)
}

/// Pascal `ExportdeltaZ` (`ExportResults.pas` l.3967): one `%.10g` per line, the
/// last NCIM correction vector. Silent no-op when empty.
pub fn export_delta_z(ckt: &Circuit) -> Option<String> {
    export_real_vec(&ckt.solution.ncim_delta_z)
}

fn export_real_vec(v: &[f64]) -> Option<String> {
    if v.is_empty() {
        return None;
    }
    let mut s = String::new();
    for &x in v {
        s.push_str(&format!("{}\n", format::g(x, 10)));
    }
    Some(s)
}

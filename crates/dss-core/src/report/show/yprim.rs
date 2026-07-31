//! `Show Yprim` (Pascal `ShowResults.pas` `ShowYPrim` (:3486)): the **active**
//! circuit element's primitive Y matrix, as its `G` (conductance) and `jB`
//! (susceptance) lower triangles. Read-only over the element's already-computed
//! `Yprim` (`GetYprimValues(ALL_YPRIM)`, our `cd.yprim`); a `Nil` Yprim (an element
//! whose Y hasn't been built) prints `Yprim matrix is Nil`.

use crate::report::format;
use crate::report::table::{Cell, Report, Row};
use crate::support::cmatrix::CMatrix;

/// Build the `Show Yprim` text (Pascal `ShowYPrim`) for the active element named
/// `full_name` (`Class.Name`, native case). `yprim` is the element's full primitive
/// Y (column-major); `yorder` its order. Each matrix is printed as the **lower
/// triangle by rows** — `for i in 0..yorder`, `for j in 0..=i` — with each entry
/// `%13.10g ` (10 sig figs, the real part for `G`, the imag for `jB`).
pub(crate) fn show_yprim(full_name: &str, yprim: Option<&CMatrix>, yorder: usize) -> String {
    let mut rep = Report::new();
    rep.line(&format!("Yprim of active circuit element: {full_name}"));
    rep.blank();

    let Some(y) = yprim else {
        rep.line("Yprim matrix is Nil");
        return rep.finish();
    };

    // `G` (real part) then `jB` (imag part), each a blank line + banner + blank
    // line + the lower-triangle rows. Each triangle is one run, so the lane sizes
    // its columns across the whole matrix.
    for (banner, real_part) in [
        ("G matrix (conductance), S", true),
        ("jB matrix (Susceptance), S", false),
    ] {
        rep.blank();
        rep.line(banner);
        rep.blank();
        for i in 0..yorder {
            let mut row = Row::new();
            for j in 0..=i {
                let v = y.get(i, j);
                let val = if real_part { v.re } else { v.im };
                // Pascal `Format('%13.10g ', [val])` — the trailing space is the
                // column's gutter.
                row = row.cell(Cell::right(format::g(val, 10), 13).sep(" "));
            }
            rep.row(row);
        }
    }
    rep.finish()
}

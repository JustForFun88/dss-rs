//! `Show Yprim` (Pascal `ShowResults.pas` `ShowYPrim` (:3486)): the **active**
//! circuit element's primitive Y matrix, as its `G` (conductance) and `jB`
//! (susceptance) lower triangles. Read-only over the element's already-computed
//! `Yprim` (`GetYprimValues(ALL_YPRIM)`, our `cd.yprim`); a `Nil` Yprim (an element
//! whose Y hasn't been built) prints `Yprim matrix is Nil`.

use crate::report::format;
use crate::support::cmatrix::CMatrix;

/// Build the `Show Yprim` text (Pascal `ShowYPrim`) for the active element named
/// `full_name` (`Class.Name`, native case). `yprim` is the element's full primitive
/// Y (column-major); `yorder` its order. Each matrix is printed as the **lower
/// triangle by rows** — `for i in 0..yorder`, `for j in 0..=i` — with each entry
/// `%13.10g ` (10 sig figs, the real part for `G`, the imag for `jB`).
pub(crate) fn show_yprim(full_name: &str, yprim: Option<&CMatrix>, yorder: usize) -> String {
    let mut s = String::new();
    s.push_str("Yprim of active circuit element: ");
    s.push_str(full_name);
    s.push('\n');
    s.push('\n');

    let Some(y) = yprim else {
        s.push_str("Yprim matrix is Nil\n");
        return s;
    };

    // `G` (real part) then `jB` (imag part), each a blank line + banner + blank
    // line + the lower-triangle rows.
    for (banner, real_part) in [
        ("G matrix (conductance), S", true),
        ("jB matrix (Susceptance), S", false),
    ] {
        s.push('\n');
        s.push_str(banner);
        s.push('\n');
        s.push('\n');
        for i in 0..yorder {
            for j in 0..=i {
                let v = y.get(i, j);
                let val = if real_part { v.re } else { v.im };
                // Pascal `Format('%13.10g ', [val])`.
                s.push_str(&format::g_w(val, 13, 10));
                s.push(' ');
            }
            s.push('\n');
        }
    }
    s
}

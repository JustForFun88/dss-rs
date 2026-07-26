//! `Export Yprims` (Pascal `ExportResults.pas` `ExportYprim`): the primitive `Yprim`
//! matrix of every enabled PD/PC circuit element, in device (creation) order.
//!
//! Pascal walks `CktElements.Get(k)` for `k := 1..NumDevices` and emits the block
//! for each element that `is TPDElement or is TPCElement`. In our model that set
//! is `pd_elements ∪ pc_elements ∪ sources ∪ faults`: `TVsourceObj` is a
//! `TPCElement` (our `ElemKind::Source`, tracked in `sources`), and `TFaultObj`
//! is a `TPDElement` (our `ElemKind::Fault`, tracked off `pd_elements` in
//! `faults`). Each block is the element `Class.NAME` header then `Yorder` rows of
//! `re, im,` pairs (`GetYprimValues(ALL_YPRIM)`, our `cd.yprim`, column-major →
//! printed row-major). Read-only, no mutation.

use std::collections::HashSet;

use crate::circuit::Circuit;
use crate::exec::registry::DssClass;
use crate::report::format;

/// Build the `Export Yprims` body (Pascal `ExportYprim`).
pub fn export_yprims(classes: &[DssClass], ckt: &Circuit) -> String {
    // The "is PD or PC" set (Pascal), keyed (cls, idx) since `ElemId` is not
    // `Hash`. Vsource (sources) and Fault (faults) are PC/PD in Pascal.
    let mut members: HashSet<(usize, usize)> = HashSet::new();
    for list in [
        &ckt.pd_elements,
        &ckt.pc_elements,
        &ckt.sources,
        &ckt.faults,
    ] {
        for r in list {
            members.insert((r.class_ord(), r.index()));
        }
    }

    let mut s = String::new();
    for &r in &ckt.ckt_elements {
        if !members.contains(&(r.class_ord(), r.index())) {
            continue;
        }
        let class_name = classes[r.class_ord()].props.class_name();
        let obj = &classes[r.class_ord()].arena[r.index()];
        let Some(elem) = obj.as_ckt_element() else {
            continue;
        };
        if !elem.cd().enabled {
            continue;
        }
        let cd = elem.cd();
        let Some(yprim) = cd.yprim.as_ref() else {
            continue;
        };
        let yorder = cd.yorder;
        // Header: `ParentClass.Name`, `.`, `AnsiUpperCase(Name)`.
        let full = format!("{}.{}", class_name, obj.data().name());
        s.push_str(&format::upper_elem_name(&full));
        s.push('\n');
        // `for i := 1 to Yorder` (row) `for j := 1 to Yorder` (col): the
        // column-major `cValues[i+(j-1)*Yorder]` = entry (row i, col j), printed
        // row by row as `%-13.10g, %-13.10g, ` per column, trailing `, `.
        for row in 0..yorder {
            for col in 0..yorder {
                let v = yprim.get(row, col);
                s.push_str(&format!(
                    "{}, {}, ",
                    format::g(v.re, 10),
                    format::g(v.im, 10)
                ));
            }
            s.push('\n');
        }
    }
    s
}

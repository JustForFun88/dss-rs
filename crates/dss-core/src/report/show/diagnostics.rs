//! Small `Show` diagnostic reports (Pascal `ShowResults.pas`):
//! - `Show Result` ([`show_result`], `ShowResult`) — the `@result` parser var;
//! - `Show EventLog` ([`show_event_log`], `ShowEventLog`) — the accumulated event
//!   strings (same `SaveToFile` dump as `Export EventLog`);
//! - `Show Ratings` ([`show_ratings`], `ShowRatings`) — each PD element's normal /
//!   emergency amp ratings;
//! - `Show Variables` ([`show_variables`], `ShowVariables`) — every PC element's
//!   present dynamic-state-variable values.

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::traits::SysCtx;
use crate::exec::registry::DssClass;
use crate::report::export::for_each_enabled_elem;
use crate::report::format;

/// `Show Result` (Pascal `ShowResult`): the `@result` parser var value on one line.
pub(crate) fn show_result(result: &str) -> String {
    format!("{result}\n")
}

/// `Show EventLog` (Pascal `ShowEventLog` = `EventStrings.SaveToFile`): one entry
/// per terminated line (identical to `Export EventLog`).
pub(crate) fn show_event_log(entries: &[String]) -> String {
    crate::report::export::export_event_log(entries)
}

/// `Show Ratings` (Pascal `ShowRatings`): each PD element's normal/emergency amp
/// ratings, `"FullName", normamps=%-.4g,  %-.4g  !Amps`.
pub(crate) fn show_ratings(classes: &[DssClass], ckt: &Circuit) -> String {
    let mut s = String::from("Power Delivery Elements Normal and Emergency (max) Ratings\n\n");
    for &r in &ckt.pd_elements {
        let class_name = classes[r.cls].props.class_name();
        let obj = &classes[r.cls].objects[r.idx];
        if let Some(elem) = obj.as_ckt_element() {
            let name = format!("{}.{}", class_name, obj.data().name());
            s.push_str(&format!(
                "\"{}\", normamps={},  {}  !Amps\n",
                name,
                format::g(elem.norm_amps(), 4),
                format::g(elem.emerg_amps(), 4),
            ));
        }
    }
    s
}

/// `Show Variables` (Pascal `ShowVariables`): every enabled PC element with
/// dynamic state variables, its `ELEMENT: name` / `No. of variables: n` header and
/// each `  name = value` (`%-.6g`).
pub(crate) fn show_variables(
    classes: &mut [DssClass],
    ckt: &Circuit,
    sys: &SysCtx,
    node_v: &[Complex64],
) -> String {
    let mut s = String::new();
    s.push('\n');
    s.push_str("VARIABLES REPORT\n");
    s.push('\n');
    s.push_str("Present values of all variables in PC Elements in the circuit.\n");
    s.push('\n');

    for_each_enabled_elem(classes, &ckt.pc_elements, |name, elem| {
        let nvar = elem.num_variables();
        if nvar == 0 {
            return;
        }
        let names: Vec<String> = (1..=nvar).map(|i| elem.variable_name(i)).collect();
        let mut states = vec![0.0f64; nvar];
        elem.get_all_variables(sys, node_v, &mut states);
        // Pascal writes `pcElem.FullName` (native case).
        s.push_str(&format!("ELEMENT: {name}\n"));
        s.push_str(&format!("No. of variables: {nvar}\n"));
        for (nm, v) in names.iter().zip(&states) {
            s.push_str(&format!("  {} = {}\n", nm, format::g(*v, 6)));
        }
        s.push('\n');
    });
    s
}

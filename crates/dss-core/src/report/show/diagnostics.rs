//! Small `Show` diagnostic reports (Pascal `ShowResults.pas`):
//! - `Show Result` ([`show_result`], `ShowResult`) — the `@result` parser var;
//! - `Show EventLog` ([`show_event_log`], `ShowEventLog`) — the accumulated event
//!   strings (same `SaveToFile` dump as `Export EventLog`);
//! - `Show Ratings` ([`show_ratings`], `ShowRatings`) — each PD element's normal /
//!   emergency amp ratings;
//! - `Show Variables` ([`show_variables`], `ShowVariables`) — every PC element's
//!   present dynamic-state-variable values;
//! - `Show Mismatch` ([`show_mismatch`], `ShowNodeCurrentSum`) — the per-node
//!   current-sum (KCL) mismatch report.

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

/// `Show Mismatch` (Pascal `ShowNodeCurrentSum`): the per-node current-sum (KCL)
/// mismatch — for every node, Σ of each connected element's terminal current, its
/// `%error` against the largest single current at that node, and that max current.
pub(crate) fn show_mismatch(
    classes: &mut [DssClass],
    ckt: &Circuit,
    sys: &SysCtx,
    node_v: &[Complex64],
) -> String {
    // Accumulate Σ current and max |current| per node (index 0 = ground).
    let n = node_v.len();
    let mut currents = vec![Complex64::ZERO; n];
    let mut max_node = vec![0.0f64; n];
    for_each_enabled_elem(classes, &ckt.ckt_elements, |_name, elem| {
        elem.compute_iterminal(sys, node_v);
        let cd = elem.cd();
        for i in 0..cd.nconds * cd.nterms {
            let nref = cd.node_ref[i];
            let ct = cd.iterminal[i];
            currents[nref] += ct;
            max_node[nref] = max_node[nref].max(ct.norm());
        }
    });

    let mbnl = super::max_bus_name_length(ckt) + 2;
    let mut s = String::new();
    s.push('\n');
    s.push_str("Node Current Mismatch Report\n");
    s.push('\n');
    s.push('\n');
    s.push_str(&format::pad("Bus,", mbnl));
    s.push_str(" Node, \"Current Sum (A)\", \"%error\", \"Max Current (A)\"\n");

    let row = |s: &mut String, bname: &str, node: i32, nref: usize| {
        let dtemp = currents[nref].norm();
        // `%error`: 0 when the node has no current or its sum equals its max
        // (a single-branch node balances trivially), else `sum/max·100`.
        let pcterr = if max_node[nref] == 0.0 || max_node[nref] == dtemp {
            format::fixed_w(0.0, 10, 1)
        } else {
            format::fixed_w(dtemp / max_node[nref] * 100.0, 10, 6)
        };
        // `'%s, %2d, %10.5f,       %s, %10.5f'`.
        s.push_str(&format!(
            "{}, {}, {},       {}, {}\n",
            bname,
            format::fixed_w_int(node as i64, 2),
            format::fixed_w(dtemp, 10, 5),
            pcterr,
            format::fixed_w(max_node[nref], 10, 5),
        ));
    };

    // Ground bus (node ref 0) first.
    row(&mut s, &format::pad("\"System Ground\"", mbnl), 0, 0);
    for i in 0..ckt.buses.len() {
        let bus = &ckt.buses[i];
        for j in 0..bus.num_nodes_this_bus() {
            let bname = if j == 0 {
                format::pad_dots(
                    &format::enclose_quotes(ckt.bus_list.name(i).unwrap_or("")),
                    mbnl,
                )
            } else {
                format::pad("\"   -\"", mbnl)
            };
            row(&mut s, &bname, bus.get_num(j), bus.get_ref(j));
        }
    }
    s
}

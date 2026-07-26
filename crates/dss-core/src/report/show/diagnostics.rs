//! Small `Show` diagnostic reports (Pascal `ShowResults.pas`):
//! - `Show Result` ([`show_result`], `ShowResult`) — the `@result` parser var;
//! - `Show EventLog` ([`show_event_log`], `ShowEventLog`) — the accumulated event
//!   strings (same `SaveToFile` dump as `Export EventLog`);
//! - `Show Ratings` ([`show_ratings`], `ShowRatings`) — each PD element's normal /
//!   emergency amp ratings;
//! - `Show Variables` ([`show_variables`], `ShowVariables`) — every PC element's
//!   present dynamic-state-variable values;
//! - `Show Mismatch` ([`show_mismatch`], `ShowNodeCurrentSum`) — the per-node
//!   current-sum (KCL) mismatch report;
//! - `Show Convergence` ([`show_convergence`], `Solution.WriteConvergenceReport`) —
//!   the per-node saved error / magnitude / base-voltage snapshot;
//! - `Show kvbasemismatch` ([`show_kvbase_mismatch`], `ShowkVBaseMismatch`) — loads
//!   and generators whose declared kV base is >10% off the connected bus's base;
//! - `Show controlqueue` ([`show_control_queue`], `ControlQueue.WriteQueue`) — the
//!   pending control-action queue.

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::traits::{ElemId, SysCtx};
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
        let class_name = classes[r.class_ord()].props.class_name();
        let obj = &classes[r.class_ord()].arena[r.index()];
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
    for_each_enabled_elem(classes, &ckt.ckt_elements, |name, elem| {
        elem.compute_iterminal(sys, node_v);
        let cd = elem.cd();
        if super::is_autotrans(name) {
            // AutoTrans special case (`ShowResults.pas:3636`): per terminal, sum
            // only the first `Nphases` conductor currents, then skip the same
            // count (`Inc(k, Nphases)`; AutoTrans `NConds = 2·Nphases`).
            let mut k = 0usize;
            for _ in 0..cd.nterms {
                for _ in 0..cd.nphases {
                    let nref = cd.node_ref[k];
                    let ct = cd.iterminal[k];
                    currents[nref] += ct;
                    max_node[nref] = max_node[nref].max(ct.norm());
                    k += 1;
                }
                k += cd.nphases;
            }
        } else {
            for i in 0..cd.nconds * cd.nterms {
                let nref = cd.node_ref[i];
                let ct = cd.iterminal[i];
                currents[nref] += ct;
                max_node[nref] = max_node[nref].max(ct.norm());
            }
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

/// `Show Convergence` (Pascal `TSolutionObj.WriteConvergenceReport`): for every
/// node, its `"Bus.Node"` label, the last-iteration saved error (`ErrorSaved`,
/// `%10.5f`), the saved magnitude (`VmagSaved`, `Str(v:14)`) and the node base
/// voltage (`NodeVbase`, `Str(v:14)`), then the `Max Error` footer. The 1-based
/// solution arrays carry a dummy slot 0, so we walk `1..=num_nodes`.
pub(crate) fn show_convergence(ckt: &Circuit) -> String {
    let sol = &ckt.solution;
    let mut s = String::new();
    s.push('\n');
    s.push_str("-------------------\n");
    s.push_str("Convergence Report:\n");
    s.push_str("-------------------\n");
    s.push_str("\"Bus.Node\", \"Error\", \"|V|\",\"Vbase\"\n");
    for i in 1..=ckt.num_nodes {
        let nb = ckt.map_node_to_bus[i];
        let bus_name = ckt.bus_list.name(nb.bus_ref).unwrap_or("");
        // Pascal `'"' + pad(BusName + '.' + NodeNum + '"', 18)` (the trailing
        // quote is *inside* the padded field, the leading one before it).
        let label = format::pad(&format!("{bus_name}.{}\"", nb.node_num), 18);
        s.push_str(&format!(
            "\"{}, {}, {}, {}\n",
            label,
            format::fixed_w(sol.error_saved[i], 10, 5),
            format::fpc_sci_w(sol.vmag_saved[i], 14),
            format::fpc_sci_w(sol.node_vbase[i], 14),
        ));
    }
    s.push('\n');
    s.push_str(&format!(
        "Max Error = {}\n",
        format::fixed_w(sol.max_error, 10, 5)
    ));
    s
}

/// `Show kvbasemismatch` (Pascal `ShowkVBaseMismatch`): the loads (then the
/// generators) whose declared kV base differs by more than 10% from the base of
/// the bus they connect to. A 1-phase wye element is compared line-neutral, all
/// others line-line (`kVBase·√3`); the header block for each device family is
/// written whenever that family is non-empty (matching the oracle even when no
/// mismatch follows). Walks `ckt.loads`/`ckt.generators` in creation order
/// (Pascal `Loads.First`/`Generators.First`).
pub(crate) fn show_kvbase_mismatch(classes: &[DssClass], ckt: &Circuit) -> String {
    use crate::elements::pc::{Generator, Load};
    use crate::elements::pc::{generator::Connection as GConn, load::Connection as LConn};

    let sqrt3 = crate::util::sqrt3();
    let mut s = String::new();

    // Loads.
    if !ckt.loads.is_empty() {
        s.push('\n');
        s.push_str("!!!  LOAD VOLTAGE BASE MISMATCHES\n");
        s.push('\n');
    }
    for &r in &ckt.loads {
        let obj = &classes[r.class_ord()].arena[r.index()];
        let Some(l) = obj.as_any().downcast_ref::<Load>() else {
            continue;
        };
        let full_name = format!(
            "{}.{}",
            classes[r.class_ord()].props.class_name(),
            obj.data().name()
        );
        let bus_ref = l.cd.terminals[0].bus_idx();
        let bus_kv = ckt.buses[bus_ref].kv_base;
        let bus_name = ckt.bus_list.name(bus_ref).unwrap_or("");
        if bus_kv == 0.0 {
            continue;
        }
        if l.cd.nphases == 1 && l.connection == LConn::Wye {
            if (l.kv_load_base - bus_kv).abs() > 0.10 * bus_kv {
                s.push_str(&format!(
                    "!!!!! Voltage Base Mismatch, {}.kV={}, Bus {} LN kvBase = {}\n",
                    full_name,
                    format::g(l.kv_load_base, 6),
                    l.cd.get_bus(1),
                    format::g(bus_kv, 6),
                ));
                s.push_str(&format!(
                    "!setkvbase {} kVLN={}\n",
                    bus_name,
                    format::g(l.kv_load_base, 6)
                ));
                s.push_str(&format!("!{}.kV={}\n", full_name, format::g(bus_kv, 6)));
            }
        } else {
            let bus_kv_ll = bus_kv * sqrt3;
            if (l.kv_load_base - bus_kv_ll).abs() > 0.10 * bus_kv_ll {
                s.push_str(&format!(
                    "!!!!! Voltage Base Mismatch, {}.kV={}, Bus {} kvBase = {}\n",
                    full_name,
                    format::g(l.kv_load_base, 6),
                    l.cd.get_bus(1),
                    format::g(bus_kv_ll, 6),
                ));
                s.push_str(&format!(
                    "!setkvbase {} kVLL={}\n",
                    bus_name,
                    format::g(l.kv_load_base, 6)
                ));
                s.push_str(&format!("!{}.kV={}\n", full_name, format::g(bus_kv_ll, 6)));
            }
        }
    }

    // Generators.
    if !ckt.generators.is_empty() {
        s.push('\n');
        s.push_str("!!!  GENERATOR VOLTAGE BASE MISMATCHES\n");
        s.push('\n');
    }
    for &r in &ckt.generators {
        let obj = &classes[r.class_ord()].arena[r.index()];
        let Some(g) = obj.as_any().downcast_ref::<Generator>() else {
            continue;
        };
        let full_name = format!(
            "{}.{}",
            classes[r.class_ord()].props.class_name(),
            obj.data().name()
        );
        let bus_ref = g.cd.terminals[0].bus_idx();
        let bus_kv = ckt.buses[bus_ref].kv_base;
        let bus_name = ckt.bus_list.name(bus_ref).unwrap_or("");
        if bus_kv == 0.0 {
            continue;
        }
        if g.cd.nphases == 1 && g.connection == GConn::Wye {
            if (g.kv_generator_base - bus_kv).abs() > 0.10 * bus_kv {
                s.push_str(&format!(
                    "!!! Voltage Base Mismatch, {}.kV={}, Bus {} LN kvBase = {}\n",
                    full_name,
                    format::g(g.kv_generator_base, 6),
                    g.cd.get_bus(1),
                    format::g(bus_kv, 6),
                ));
                s.push_str(&format!(
                    "!setkvbase {} kVLN={}\n",
                    bus_name,
                    format::g(g.kv_generator_base, 6)
                ));
                s.push_str(&format!("!{}.kV={}\n", full_name, format::g(bus_kv, 6)));
            }
        } else {
            let bus_kv_ll = bus_kv * sqrt3;
            if (g.kv_generator_base - bus_kv_ll).abs() > 0.10 * bus_kv_ll {
                s.push_str(&format!(
                    "!!! Voltage Base Mismatch, {}.kV={}, Bus {} kvBase = {}\n",
                    full_name,
                    format::g(g.kv_generator_base, 6),
                    g.cd.get_bus(1),
                    format::g(bus_kv_ll, 6),
                ));
                s.push_str(&format!(
                    "!setkvbase {} kVLL={}\n",
                    bus_name,
                    format::g(g.kv_generator_base, 6)
                ));
                s.push_str(&format!("!{}.kV={}\n", full_name, format::g(bus_kv_ll, 6)));
            }
        }
    }
    s
}

/// `Show controlqueue` (Pascal `TControlQueue.WriteQueue`): the pending
/// control-action queue, one CSV row per queued action. After a converged
/// snapshot the queue is drained (`show controlqueue` after `solve` — the only
/// text-interface path — always yields the header alone; the row body cannot be
/// reached via the executive, oracle-probed), so this row path is exercised only
/// by the [`tests`] unit test, against the Pascal `WriteQueue` format.
pub(crate) fn show_control_queue(classes: &[DssClass], ckt: &Circuit) -> String {
    let mut s = String::from("Handle, Hour, Sec, ActionCode, ProxyDevRef, Device\n");
    for (handle, hour, sec, code, proxy, ctrl) in ckt.solution.control_queue.queue_rows() {
        let name = device_name(classes, ctrl);
        s.push_str(&queue_row_line(handle, hour, sec, code, proxy, &name));
    }
    s
}

/// One `Show controlqueue` row (Pascal `Format('%d, %d, %-.g, %d, %d, %s ', …)`,
/// `ControlQueue.pas:496`) — the trailing space after the device name is Pascal's.
///
/// TODO(compat): `%-.g` is FPC `ffGeneral` with an *empty* precision after the
/// dot (`.` → precision 0); its rendered significant-digit count cannot be
/// confirmed against the oracle because the queue is always drained before any
/// text-interface `show controlqueue` (probe-proven). `%.6g` (6 sig) is used as a
/// reasonable stand-in — for the whole-second / simple-fraction times a real queue
/// carries it is indistinguishable — flagged for the WP8.8 byte-faithfulness pass.
fn queue_row_line(handle: i32, hour: i32, sec: f64, code: i32, proxy: i32, name: &str) -> String {
    format!(
        "{handle}, {hour}, {}, {code}, {proxy}, {name} \n",
        format::g(sec, 6),
    )
}

/// The bare object name of a control element (Pascal `ControlElement.Name`).
fn device_name(classes: &[DssClass], r: ElemId) -> String {
    classes[r.class_ord()].arena[r.index()]
        .data()
        .name()
        .to_string()
}

#[cfg(test)]
mod tests {
    //! The `Show controlqueue` row-format path (`queue_row_line`) is unreachable
    //! from the executive — a `show controlqueue` after any `solve` sees a drained
    //! queue (oracle-probed), so no golden reaches it. Pin it here against the
    //! Pascal `WriteQueue` format string instead.
    use super::queue_row_line;

    #[test]
    fn control_queue_row_format() {
        // Pascal `Format('%d, %d, %-.g, %d, %d, %s ', …)`: comma+space separated,
        // a trailing space after the device name, `%-.g` for Sec.
        assert_eq!(
            queue_row_line(3, 1, 15.0, 2, 0, "capcontrol.cc1"),
            "3, 1, 15, 2, 0, capcontrol.cc1 \n"
        );
        // A fractional Sec still renders through the general float.
        assert_eq!(
            queue_row_line(1, 0, 0.5, 1, 7, "swtcontrol.sw1"),
            "1, 0, 0.5, 1, 7, swtcontrol.sw1 \n"
        );
    }
}

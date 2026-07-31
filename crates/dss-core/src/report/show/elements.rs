//! `Show Elements` (Pascal `ShowResults.pas` `ShowElements` + `WriteElementRecord`,
//! `ShowOptions.pas` case 5): the element ↔ bus-connection listing. Two forms:
//! - a **class filter** (`Show Elements <class>`) — every object of that class,
//!   uppercased, split into the enabled (main) and disabled companion files
//!   (non-`CktElement` classes go entirely to the main file);
//! - the **default** (`Show Elements`) — PD then PC elements, each `Element  Bus1
//!   Bus2 …`, again split enabled/disabled.
//!
//! Pascal writes **two** files: the main `Elements.txt` and a sibling
//! `Elements_Disabled.txt`; [`show_elements`] returns `(main, disabled)` and the
//! router writes both.

use crate::circuit::Circuit;
use crate::elements::traits::CktElement;
use crate::exec::registry::DssClass;
use crate::report::format;
use crate::report::table::{Cell, Report, Row};

/// Build the `Show Elements` text for both files (Pascal `ShowElements`).
/// `class_name` is the already-lowercased class filter (empty = the default
/// PD/PC form). Returns `(main, disabled)`. Read-only over the registry.
pub(crate) fn show_elements(
    classes: &[DssClass],
    ckt: &Circuit,
    class_name: &str,
) -> (String, String) {
    let mbnl = super::max_bus_name_length(ckt);
    let mdnl = crate::compat::max_device_name_length(super::device_name_width(classes, ckt));
    let mut main = Report::new();
    let mut disabled = Report::new();

    if !class_name.is_empty() {
        // Class filter (`SetObjectClass` + walk `ActiveDSSClass`). An unknown class
        // leaves both strings empty (Pascal still creates the two files). One name
        // per line — no columns, so no rows.
        if let Some(ci) = classes
            .iter()
            .position(|c| c.props.class_name().eq_ignore_ascii_case(class_name))
        {
            main.text(&format!("All Elements in Class \"{class_name}\"\n\n"));
            disabled.text(&format!(
                "All DISABLED Elements in Class \"{class_name}\"\n\n"
            ));
            let arena = &classes[ci].arena;
            for i in 0..arena.len() {
                let uname = arena.obj(i).data().name().to_uppercase();
                // Pascal `(DSSClassType and BASECLASSMASK) > 0` = a circuit element:
                // route by `Enabled`. A non-CktElement object always goes to `main`.
                match arena.try_ckt_elem(i) {
                    Some(elem) if !elem.cd().enabled => disabled.line(&uname),
                    _ => main.line(&uname),
                }
            }
        }
        return (main.finish(), disabled.finish());
    }

    // Default form: PD then PC elements, each `WriteElementRecord`. The header is
    // the columns the records draw: `Pad(' BusN', mbnl)`'s leading space is the
    // previous column's gutter, so the label sits in a `mbnl - 1` field (`Pad`
    // only appends — the bytes are identical).
    let header = |rep: &mut Report, disabled_prefix: &str| {
        let elem_col = format!("{disabled_prefix}Element");
        let mut row = Row::new().cell(Cell::left(elem_col, mdnl + 2).sep(" "));
        for b in ["Bus1", "Bus2", "Bus3"] {
            row = row.cell(Cell::left(b, mbnl.saturating_sub(1)).sep(" "));
        }
        rep.row(row.cell(Cell::plain("...")));
    };

    main.blank();
    main.line(&format!("Elements in Active Circuit: {}", ckt.name));
    main.blank();
    main.line("Power Delivery Elements");
    main.blank();
    header(&mut main, "");
    main.blank();

    disabled.blank();
    disabled.line(&format!(
        "DISABLED Elements in Active Circuit: {}",
        ckt.name
    ));
    disabled.blank();
    disabled.line("DISABLED Power Delivery Elements");
    disabled.blank();
    header(&mut disabled, "DISABLED ");
    disabled.blank();

    write_records(
        classes,
        ckt,
        &ckt.pd_elements,
        mbnl,
        mdnl,
        &mut main,
        &mut disabled,
    );

    main.blank();
    main.line("Power Conversion Elements");
    main.blank();
    header(&mut main, "");
    main.blank();

    disabled.blank();
    disabled.line("DISABLED Power Conversion Elements");
    disabled.blank();
    header(&mut disabled, "DISABLED ");
    disabled.blank();

    write_records(
        classes,
        ckt,
        &ckt.pc_elements,
        mbnl,
        mdnl,
        &mut main,
        &mut disabled,
    );

    (main.finish(), disabled.finish())
}

/// Walk a circuit list, writing each element's `WriteElementRecord` row to `main`
/// (enabled) or `disabled`.
#[allow(clippy::too_many_arguments)]
fn write_records(
    classes: &[DssClass],
    ckt: &Circuit,
    refs: &[crate::elements::traits::ElemId],
    mbnl: usize,
    mdnl: usize,
    main: &mut Report,
    disabled: &mut Report,
) {
    for &r in refs {
        let class_name = classes[r.class_ord()].props.class_name();
        let obj = &classes[r.class_ord()].arena[r.index()];
        let name = format!("{}.{}", class_name, obj.data().name());
        if let Some(elem) = classes[r.class_ord()].arena.try_ckt_elem(r.index()) {
            let rec = element_record_row(ckt, &name, elem, mbnl, mdnl);
            if elem.cd().enabled {
                main.row(rec);
            } else {
                disabled.row(rec);
            }
        }
    }
}

/// One element's `"Name"  Bus1 Bus2 …` row (Pascal `WriteElementRecord`). The
/// bus names are the terminals' stored (stripped) names, uppercased.
fn element_record_row(
    ckt: &Circuit,
    name: &str,
    elem: &dyn CktElement,
    mbnl: usize,
    mdnl: usize,
) -> Row {
    let cd = elem.cd();
    let mut row = Row::new().cell(Cell::left(format::enclose_quotes(name), mdnl + 2).sep(" "));
    for j in 0..cd.nterms {
        let bus = cd.terminals[j]
            .bus_ref
            .and_then(|b| ckt.buses.get(b))
            .map(|b| b.name.as_str())
            .unwrap_or("");
        row = row.cell(Cell::left(bus.to_uppercase(), mbnl).sep(" "));
    }
    row
}

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

/// Build the `Show Elements` text for both files (Pascal `ShowElements`).
/// `class_name` is the already-lowercased class filter (empty = the default
/// PD/PC form). Returns `(main, disabled)`. Read-only over the registry.
pub(crate) fn show_elements(
    classes: &[DssClass],
    ckt: &Circuit,
    class_name: &str,
) -> (String, String) {
    let mbnl = super::max_bus_name_length(ckt);
    let mdnl = super::max_device_name_length(classes, ckt);
    let mut main = String::new();
    let mut disabled = String::new();

    if !class_name.is_empty() {
        // Class filter (`SetObjectClass` + walk `ActiveDSSClass`). An unknown class
        // leaves both strings empty (Pascal still creates the two files).
        if let Some(ci) = classes
            .iter()
            .position(|c| c.props.class_name().eq_ignore_ascii_case(class_name))
        {
            main.push_str(&format!("All Elements in Class \"{class_name}\"\n\n"));
            disabled.push_str(&format!(
                "All DISABLED Elements in Class \"{class_name}\"\n\n"
            ));
            let arena = &classes[ci].arena;
            for i in 0..arena.len() {
                let uname = arena.obj(i).data().name().to_uppercase();
                // Pascal `(DSSClassType and BASECLASSMASK) > 0` = a circuit element:
                // route by `Enabled`. A non-CktElement object always goes to `main`.
                match arena.try_ckt_elem(i) {
                    Some(elem) if !elem.cd().enabled => {
                        disabled.push_str(&uname);
                        disabled.push('\n');
                    }
                    _ => {
                        main.push_str(&uname);
                        main.push('\n');
                    }
                }
            }
        }
        return (main, disabled);
    }

    // Default form: PD then PC elements, each `WriteElementRecord`.
    let header = |disabled_prefix: &str| {
        let elem_col = if disabled_prefix.is_empty() {
            "Element".to_string()
        } else {
            format!("{disabled_prefix}Element")
        };
        format!(
            "{}{}{}{} ...\n",
            format::pad(&elem_col, mdnl + 2),
            format::pad(" Bus1", mbnl),
            format::pad(" Bus2", mbnl),
            format::pad(" Bus3", mbnl),
        )
    };

    main.push('\n');
    main.push_str(&format!("Elements in Active Circuit: {}\n", ckt.name));
    main.push('\n');
    main.push_str("Power Delivery Elements\n");
    main.push('\n');
    main.push_str(&header(""));
    main.push('\n');

    disabled.push('\n');
    disabled.push_str(&format!(
        "DISABLED Elements in Active Circuit: {}\n",
        ckt.name
    ));
    disabled.push('\n');
    disabled.push_str("DISABLED Power Delivery Elements\n");
    disabled.push('\n');
    disabled.push_str(&header("DISABLED "));
    disabled.push('\n');

    write_records(
        classes,
        ckt,
        &ckt.pd_elements,
        mbnl,
        mdnl,
        &mut main,
        &mut disabled,
    );

    main.push('\n');
    main.push_str("Power Conversion Elements\n");
    main.push('\n');
    main.push_str(&header(""));
    main.push('\n');

    disabled.push('\n');
    disabled.push_str("DISABLED Power Conversion Elements\n");
    disabled.push('\n');
    disabled.push_str(&header("DISABLED "));
    disabled.push('\n');

    write_records(
        classes,
        ckt,
        &ckt.pc_elements,
        mbnl,
        mdnl,
        &mut main,
        &mut disabled,
    );

    (main, disabled)
}

/// Walk a circuit list, writing each element's `WriteElementRecord` to `main`
/// (enabled) or `disabled`.
#[allow(clippy::too_many_arguments)]
fn write_records(
    classes: &[DssClass],
    ckt: &Circuit,
    refs: &[crate::elements::traits::ElemId],
    mbnl: usize,
    mdnl: usize,
    main: &mut String,
    disabled: &mut String,
) {
    for &r in refs {
        let class_name = classes[r.class_ord()].props.class_name();
        let obj = &classes[r.class_ord()].arena[r.index()];
        let name = format!("{}.{}", class_name, obj.data().name());
        if let Some(elem) = classes[r.class_ord()].arena.try_ckt_elem(r.index()) {
            let rec = write_element_record(ckt, &name, elem, mbnl, mdnl);
            if elem.cd().enabled {
                main.push_str(&rec);
            } else {
                disabled.push_str(&rec);
            }
        }
    }
}

/// One element's `"Name"  Bus1 Bus2 …` row (Pascal `WriteElementRecord`). The
/// bus names are the terminals' stored (stripped) names, uppercased.
fn write_element_record(
    ckt: &Circuit,
    name: &str,
    elem: &dyn CktElement,
    mbnl: usize,
    mdnl: usize,
) -> String {
    let cd = elem.cd();
    let mut s = format::pad(&format::enclose_quotes(name), mdnl + 2);
    s.push(' ');
    for j in 0..cd.nterms {
        let bus = cd.terminals[j]
            .bus_ref
            .and_then(|b| ckt.buses.get(b))
            .map(|b| b.name.as_str())
            .unwrap_or("");
        s.push_str(&format::pad(bus, mbnl).to_uppercase());
        s.push(' ');
    }
    s.push('\n');
    s
}

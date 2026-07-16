//! `Dump commands` — Pascal `DumpAllDSSCommands` (`Utilities.pas:821-872`):
//! every executive command, executive option, and registered class property,
//! each with its help string from the gettext catalog
//! ([`crate::report::help_catalog`]).

use std::collections::HashMap;

use crate::exec::registry::DssClass;
use crate::exec::{EXEC_COMMANDS, EXEC_OPTIONS};
use crate::report::help_catalog::dss_help;

/// Pascal `DSSClassDefs.pas` `CreateDSSClasses` registration order — the
/// `DSS.DSSClassList` iteration order `DumpAllDSSCommands` walks. The Rust
/// registry (`exec/construct.rs`) groups the `DSS_OBJECT` classes ahead of
/// the circuit-element classes (an internal layout choice), so the dump walks
/// this table instead. Every class here is registered since WPG.14/15/16
/// (Isource, AutoTrans, the GIC trio) — the skip below is defensive-only and
/// `gen_reports.py::DUMP_COMMANDS_UNPORTED_SECTIONS` is empty (the golden
/// carries every section).
pub(crate) const PASCAL_CLASS_ORDER: &[&str] = &[
    "LineCode",
    "LoadShape",
    "TShape",
    "PriceShape",
    "XYcurve",
    "GrowthShape",
    "TCC_Curve",
    "Spectrum",
    "WireData",
    "CNData",
    "TSData",
    "LineSpacing",
    "LineGeometry",
    "XfmrCode",
    "Line",
    "Vsource",
    "Isource",
    "VCCS",
    "Load",
    "Transformer",
    "RegControl",
    "Capacitor",
    "Reactor",
    "CapControl",
    "Fault",
    "DynamicExp",
    "Generator",
    "WindGen",
    "GenDispatcher",
    "Storage",
    "StorageController",
    "Relay",
    "Recloser",
    "Fuse",
    "SwtControl",
    "PVSystem",
    "UPFC",
    "UPFCControl",
    "ESPVLControl",
    "IndMach012",
    "GICsource",
    "AutoTrans",
    "InvControl",
    "ExpControl",
    "GICLine",
    "GICTransformer",
    "VSConverter",
    "Monitor",
    "EnergyMeter",
    "Sensor",
];

/// Pascal `ReplaceCRLF` (`Utilities.pas:778`): CRLF pairs become the literal
/// two-char sequence `\n`. (Bare LF — what the catalog's multi-line helps
/// actually contain — passes through untouched; asserted at catalog
/// generation.)
fn replace_crlf(s: &str) -> String {
    s.replace("\r\n", "\\n")
}

/// One `i, "name", "help"` line (Pascal `WriteStr(sout, i: 0, ', "', name,
/// '", "', help, '"')`; embedded quotes are written raw, not escaped).
fn entry_line(out: &mut String, i: usize, name: &str, help: &str) {
    out.push_str(&format!("{i}, \"{name}\", \"{}\"\n", replace_crlf(help)));
}

/// Pascal `TDSSClass.GetPropertyHelp` (`DSSClass.pas:2166-2201`): the catalog
/// entry for `<Class>.<prop-lowercase>`, or the key itself on a miss. The
/// ClassParents fallback loop is provably dead against the pinned catalog
/// (no parent-class-prefixed key exists — asserted by
/// `tools/golden/gen_help_catalog.py`), so own-key-or-miss is complete.
fn property_help(class_name: &str, prop_name: &str) -> String {
    let key = format!("{class_name}.{}", prop_name.to_lowercase());
    dss_help(&key).to_string()
}

/// Pascal `DumpAllDSSCommands` body: the `[execcommands]` / `[execoptions]`
/// sections over the executive name tables, then one `[<ClassName>]` section
/// per registered class in `DSSClassList` order.
pub(crate) fn dump_all_dss_commands(
    classes: &[DssClass],
    class_by_name: &HashMap<String, usize>,
) -> String {
    let mut out = String::new();

    out.push_str("[execcommands]\n");
    for (i, name) in EXEC_COMMANDS.iter().enumerate() {
        let key = format!("Command.{}", name.to_lowercase());
        entry_line(&mut out, i + 1, name, dss_help(&key));
    }

    out.push_str("[execoptions]\n");
    for (i, name) in EXEC_OPTIONS.iter().enumerate() {
        let key = format!("Executive.{}", name.to_lowercase());
        entry_line(&mut out, i + 1, name, dss_help(&key));
    }

    for class_name in PASCAL_CLASS_ORDER {
        let Some(&ci) = class_by_name.get(&class_name.to_lowercase()) else {
            continue; // NOT_PORTED class (see PASCAL_CLASS_ORDER above).
        };
        let props = &classes[ci].props;
        out.push_str(&format!("[{}]\n", props.class_name()));
        // Running 1-based index that skips 0.15.x-deferred props (`HIDE_015X`):
        // the catalog is a byte-exact 0.14.5 golden, so a prop inserted at its
        // upstream position (e.g. Line EpsRMedium at 31) must not shift the printed
        // index of the 0.14.5 props after it (NormAmps stays 31). The prop still
        // occupies its real slot in the class; only this help listing renumbers.
        let mut printed = 0usize;
        for i in 1..=props.num_properties() {
            if props
                .prop(i)
                .flags
                .contains(crate::obj::props::PropFlags::HIDE_015X)
            {
                continue;
            }
            printed += 1;
            let name = props.property_name(i);
            entry_line(
                &mut out,
                printed,
                name,
                &property_help(props.class_name(), name),
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every registered class must appear in the Pascal order table — a newly
    /// ported class that forgets to join it would silently vanish from the
    /// `Dump commands` report instead of failing the golden.
    #[test]
    fn pascal_order_covers_every_registered_class() {
        let dss = crate::exec::Dss::new();
        for cls in dss.registered_classes() {
            let name = cls.props.class_name();
            assert!(
                PASCAL_CLASS_ORDER
                    .iter()
                    .any(|c| c.eq_ignore_ascii_case(name)),
                "class {name} missing from PASCAL_CLASS_ORDER"
            );
        }
    }
}

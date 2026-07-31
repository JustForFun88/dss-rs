//! `Show Meters` / `Show Generators` (Pascal `ShowResults.pas` `ShowMeters` /
//! `ShowGenMeters`): each element's accumulated energy-meter registers in Pascal's
//! fixed-width table layout — a register-name legend, a per-register column header,
//! then one `%10.0f`-per-register row per **enabled** element (a disabled element
//! contributes only its trailing newline, i.e. a blank line). Read the same
//! register arrays as `Export Meters`/`Export Generators`, live.

use crate::circuit::Circuit;
use crate::elements::meter::EnergyMeter;
use crate::elements::pc::Generator;
use crate::elements::traits::ElemId;
use crate::exec::registry::DssClass;
use crate::report::export::GEN_REGISTER_NAMES;
use crate::report::format;
use crate::report::table::{Cell, Report, Row};

/// Downcast an `ckt.energy_meters` ref to its concrete [`EnergyMeter`].
fn as_meter(classes: &[DssClass], r: ElemId) -> &EnergyMeter {
    classes[r.class_ord()]
        .arena
        .get::<EnergyMeter>(r.index())
        .expect("energy_meters holds EnergyMeter")
}

/// Build the `Show Meters` text (Pascal `ShowMeters`). Read-only: walks
/// `ckt.energy_meters` (creation order) and reads each meter's registers live.
pub(crate) fn show_meters(classes: &[DssClass], ckt: &Circuit) -> String {
    let mut rep = Report::new();
    rep.blank();
    rep.line("ENERGY METER VALUES");
    rep.blank();
    rep.line("Registers:");

    // Pascal `if MeterClass.ElementCount = 0` → the "no meters" line. (The
    // EnergyMeter class is always registered, so the `MeterClass = NIL` early-exit
    // is unreachable in this port.)
    let meters = &ckt.energy_meters;
    if meters.is_empty() {
        rep.line("No Energymeter Elements Defined.");
        return rep.finish();
    }

    // The register-name legend, from the FIRST meter (Pascal `energyMeters.First`,
    // even if disabled). Every meter carries `NumEMRegisters` names (the same list
    // `Export Meters` pins verbatim — the per-zone voltage-base names plus the
    // `Aux<n>` fillers for unused base slots).
    let first = as_meter(classes, meters[0]);
    let reg_names = first.register_names();
    for (i, name) in reg_names.iter().enumerate() {
        // Pascal `FSWriteln(F, 'Reg ' + IntToStr(i) + ' = ', RegisterNames[i-1])`.
        rep.line(&format!("Reg {} = {name}", i + 1));
    }
    rep.blank();

    // Column header: `'Meter        '` (13-char literal) + `Pad('   Reg i', 11)`
    // per register — as the columns it draws: the three leading spaces of each
    // `'   Reg i'` are the previous column's gutter, so the label is `Reg i` in a
    // width-8 field (`Pad` only appends, so the bytes are identical).
    let mut hdr = Row::new().cell(Cell::left("Meter", 13).sep("   "));
    for i in 0..reg_names.len() {
        let c = Cell::left(format!("Reg {}", i + 1), 8);
        // The gutter precedes each label, so the last one carries none.
        hdr = hdr.cell(if i + 1 == reg_names.len() {
            c
        } else {
            c.sep("   ")
        });
    }
    rep.row(hdr);
    rep.row(Row::blank(reg_names.len() + 1));

    // One `%10.0f`-per-register row per meter; a disabled meter emits only its
    // trailing newline (Pascal writes the newline outside the `if Enabled` guard).
    for &r in meters {
        let m = as_meter(classes, r);
        if !m.enabled() {
            rep.row(Row::blank(reg_names.len() + 1));
            continue;
        }
        let mut row = Row::new().cell(Cell::left(
            classes[r.class_ord()].arena[r.index()].data().name(),
            12,
        ));
        for &v in m.registers() {
            // Pascal `Format('%10.0f ', [Register])` — width 10, 0 decimals,
            // trailing space inside the format literal.
            row = row.cell(Cell::right(format::fixed(v, 0), 10).sep(" "));
        }
        rep.row(row);
    }
    rep.finish()
}

/// Build the `Show Generators` text (Pascal `ShowGenMeters`). Read-only: walks
/// `ckt.generators` (creation order) and reads each generator's registers live.
/// The register names are class-fixed ([`GEN_REGISTER_NAMES`]).
pub(crate) fn show_gen_meters(classes: &[DssClass], ckt: &Circuit) -> String {
    let mut rep = Report::new();
    rep.blank();
    rep.line("GENERATOR ENERGY METER VALUES");
    rep.blank();

    // Pascal guards the whole body on `Generators.First <> NIL` (no generators →
    // just the two-line banner above).
    if ckt.generators.is_empty() {
        return rep.finish();
    }

    // Column header: `'Generator          '` (19-char literal) + `Pad(name, 11)`
    // per register (unlike `ShowMeters`, the actual register names, not `Reg i`).
    let mut hdr = Row::new().cell(Cell::left("Generator", 19));
    for name in GEN_REGISTER_NAMES {
        hdr = hdr.cell(Cell::left(name, 11));
    }
    rep.row(hdr);
    rep.row(Row::blank(GEN_REGISTER_NAMES.len() + 1));

    for &r in &ckt.generators {
        let g = classes[r.class_ord()]
            .arena
            .get::<Generator>(r.index())
            .expect("generators holds Generator");
        if !g.cd.enabled {
            rep.row(Row::blank(GEN_REGISTER_NAMES.len() + 1));
            continue;
        }
        let mut row = Row::new().cell(Cell::left(
            classes[r.class_ord()].arena[r.index()].data().name(),
            12,
        ));
        for &v in &g.registers {
            row = row.cell(Cell::right(format::fixed(v, 0), 10).sep(" "));
        }
        rep.row(row);
    }
    rep.finish()
}

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

/// Downcast an `ckt.energy_meters` ref to its concrete [`EnergyMeter`].
fn as_meter(classes: &[DssClass], r: ElemId) -> &EnergyMeter {
    classes[r.class_ord()].arena[r.index()]
        .as_any()
        .downcast_ref::<EnergyMeter>()
        .expect("energy_meters holds EnergyMeter")
}

/// Build the `Show Meters` text (Pascal `ShowMeters`). Read-only: walks
/// `ckt.energy_meters` (creation order) and reads each meter's registers live.
pub(crate) fn show_meters(classes: &[DssClass], ckt: &Circuit) -> String {
    let mut s = String::new();
    s.push('\n');
    s.push_str("ENERGY METER VALUES\n");
    s.push('\n');
    s.push_str("Registers:\n");

    // Pascal `if MeterClass.ElementCount = 0` → the "no meters" line. (The
    // EnergyMeter class is always registered, so the `MeterClass = NIL` early-exit
    // is unreachable in this port.)
    let meters = &ckt.energy_meters;
    if meters.is_empty() {
        s.push_str("No Energymeter Elements Defined.\n");
        return s;
    }

    // The register-name legend, from the FIRST meter (Pascal `energyMeters.First`,
    // even if disabled). Every meter carries `NumEMRegisters` names (the same list
    // `Export Meters` pins verbatim — the per-zone voltage-base names plus the
    // `Aux<n>` fillers for unused base slots).
    let first = as_meter(classes, meters[0]);
    let reg_names = first.register_names();
    for (i, name) in reg_names.iter().enumerate() {
        // Pascal `FSWriteln(F, 'Reg ' + IntToStr(i) + ' = ', RegisterNames[i-1])`.
        s.push_str(&format!("Reg {} = {name}\n", i + 1));
    }
    s.push('\n');

    // Column header: `'Meter        '` (13-char literal) + `Pad('   Reg i', 11)`
    // per register.
    s.push_str("Meter        ");
    for i in 0..reg_names.len() {
        s.push_str(&format::pad(&format!("   Reg {}", i + 1), 11));
    }
    s.push('\n');
    s.push('\n');

    // One `%10.0f`-per-register row per meter; a disabled meter emits only its
    // trailing newline (Pascal writes the newline outside the `if Enabled` guard).
    for &r in meters {
        let m = as_meter(classes, r);
        if m.enabled() {
            s.push_str(&format::pad(
                classes[r.class_ord()].arena[r.index()].data().name(),
                12,
            ));
            for &v in m.registers() {
                // Pascal `Format('%10.0f ', [Register])` — width 10, 0 decimals,
                // trailing space inside the format literal.
                s.push_str(&format::fixed_w(v, 10, 0));
                s.push(' ');
            }
        }
        s.push('\n');
    }
    s
}

/// Build the `Show Generators` text (Pascal `ShowGenMeters`). Read-only: walks
/// `ckt.generators` (creation order) and reads each generator's registers live.
/// The register names are class-fixed ([`GEN_REGISTER_NAMES`]).
pub(crate) fn show_gen_meters(classes: &[DssClass], ckt: &Circuit) -> String {
    let mut s = String::new();
    s.push('\n');
    s.push_str("GENERATOR ENERGY METER VALUES\n");
    s.push('\n');

    // Pascal guards the whole body on `Generators.First <> NIL` (no generators →
    // just the two-line banner above).
    if ckt.generators.is_empty() {
        return s;
    }

    // Column header: `'Generator          '` (19-char literal) + `Pad(name, 11)`
    // per register (unlike `ShowMeters`, the actual register names, not `Reg i`).
    s.push_str("Generator          ");
    for name in GEN_REGISTER_NAMES {
        s.push_str(&format::pad(name, 11));
    }
    s.push('\n');
    s.push('\n');

    for &r in &ckt.generators {
        let g = classes[r.class_ord()].arena[r.index()]
            .as_any()
            .downcast_ref::<Generator>()
            .expect("generators holds Generator");
        if g.cd.enabled {
            s.push_str(&format::pad(
                classes[r.class_ord()].arena[r.index()].data().name(),
                12,
            ));
            for &v in &g.registers {
                s.push_str(&format::fixed_w(v, 10, 0));
                s.push(' ');
            }
        }
        s.push('\n');
    }
    s
}

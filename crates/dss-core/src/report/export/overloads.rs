//! `Export Overloads` (Pascal `ExportResults.pas` `ExportOverloads` (:2489)):
//! one row per **enabled** PDElement (excluding capacitors) whose terminal-1 max
//! phase current exceeds its normal or emergency rating — the symmetrical-component
//! currents, the amps/kVA over the normal rating, and the `%Normal`/`%Emergency`
//! loading. A non-overloaded element (or one with no rating) writes nothing.

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::traits::SysCtx;
use crate::exec::registry::DssClass;
use crate::report::export::for_each_enabled_elem;
use crate::report::format;
use crate::support::mathutil::SymComp;

/// Build the `Export Overloads` body (Pascal `ExportOverloads`). Walks the
/// PDElements (terminal 1 only) calling the mutating `GetCurrents`/`Power`
/// getters (PHASE8_PLAN §2.1); capacitors are skipped (Pascal
/// `(CLASSMASK and DSSObjType) <> CAP_ELEMENT`).
pub(crate) fn export_overloads(
    classes: &mut [DssClass],
    ckt: &Circuit,
    sys: &SysCtx,
    node_v: &[Complex64],
) -> String {
    let mut s = String::from(
        "Element, Terminal,  I1, AmpsOver, kVAOver, %Normal, %Emergency, I2, %I2/I1, I0, %I0/I1\n",
    );
    let sc = SymComp::default();
    let seasonal_idx = ckt.seasonal_rating_idx;
    for_each_enabled_elem(classes, &ckt.pd_elements, |name, elem| {
        // Pascal `if (CLASSMASK and PDElem.DSSObjType) <> CAP_ELEMENT` — ignore
        // capacitors (they carry current but are not "overload" candidates).
        if name
            .split('.')
            .next()
            .is_some_and(|cls| cls.eq_ignore_ascii_case("Capacitor"))
        {
            return;
        }
        elem.compute_iterminal(sys, node_v);
        // Seasonal ratings (dss_capi 0.15.x `55400a29`, WP-U1.5 E2): Pascal
        // `PdElem.GetRatings(iNormal, iEmerg)` — the globally-synced season
        // index overrides norm/emerg for any PDElement with `NumAmpRatings > 1`.
        let (norm_amps, emerg_amps) = elem.get_ratings(seasonal_idx);
        let nphases = elem.cd().nphases;

        // Terminal-1 max phase current over the first `min(Nphases, 3)` phases
        // (Pascal `Cmax := max(Cmax, Cabs(Iph[i]))`, `Iph[i] = cBuffer^[i]`).
        let nph = nphases.min(3);
        let mut cmax = 0.0f64;
        for i in 0..nph {
            let mag = elem.cd().iterminal[i].norm();
            if mag > cmax {
                cmax = mag;
            }
        }

        // Symmetrical components (>= 3 phases) or the single-phase fallback
        // (`I0 = I2 = 0`, `I1 = |Iph[1]|`, `Cmax := I1`).
        let (i0, i1, i2) = if nphases >= 3 {
            let iph = [
                elem.cd().iterminal[0],
                elem.cd().iterminal[1],
                elem.cd().iterminal[2],
            ];
            let mut i012 = [Complex64::ZERO; 3];
            sc.phase_to_sym(&iph, &mut i012);
            (i012[0].norm(), i012[1].norm(), i012[2].norm())
        } else {
            let i1 = elem.cd().iterminal[0].norm();
            cmax = i1;
            (0.0, i1, 0.0)
        };

        // Only overloaded branches are reported (Pascal guards on a positive
        // rating AND `Cmax` over one of the two ratings).
        if !(norm_amps > 0.0 || emerg_amps > 0.0) {
            return;
        }
        if !(cmax > norm_amps || cmax > emerg_amps) {
            return;
        }

        // `Spower := Cabs(PDElem.Power[1]) * 0.001` (kVA).
        let spower = elem.terminal_power(sys, node_v, 1).norm() * 0.001;

        // Build the row char-for-char as Pascal's `FSWrite` sequence, so the
        // degenerate `NormAmps <= 0` / `EmergAmps <= 0` column shift is byte-exact.
        // The subtlety: Pascal writes I1 as `Format('%8.2f, ', [I1])` — WITH a
        // trailing `, ` — then the `NormAmps > 0` branch emits AmpsOver directly
        // (`Format('%8.2f, %10.2f')`, no leading separator) so I1's trailing comma
        // becomes AmpsOver's separator; but the `NormAmps <= 0` branch emits
        // `Separator + '0.0'`, which DOUBLES with I1's trailing comma to produce an
        // **empty field** before the `0.0` (the observable upstream column shift).
        // `"Class.NAME"` (quoted, name uppercased; Pascal pads to 22 — trailing
        // whitespace the comparator trims).
        let mut row = format!(
            "\"{}\", 1, {}, ",
            format::upper_elem_name(name),
            format::fixed(i1, 2),
        );
        // `iNormal` branch: AmpsOver / kVAOver / %Normal (no leading separator —
        // it rides I1's trailing comma), or a `Separator + '0.0'` that leaves the
        // empty AmpsOver field behind it.
        if norm_amps > 0.0 {
            row.push_str(&format!(
                "{}, {}, {}",
                format::fixed(cmax - norm_amps, 2),
                format::fixed(spower * (cmax - norm_amps) / norm_amps, 2),
                format::fixed(cmax / norm_amps * 100.0, 1),
            ));
        } else {
            row.push_str(", 0.0");
        }
        // `iEmerg` branch: %Emergency, or the literal `0.0` (both `Separator + …`).
        if emerg_amps > 0.0 {
            row.push_str(&format!(
                ", {}",
                format::fixed(cmax / emerg_amps * 100.0, 1)
            ));
        } else {
            row.push_str(", 0.0");
        }
        // I2, then %I2/I1 (`0.0` when `I1 == 0`), then I0, then %I0/I1.
        row.push_str(&format!(", {}", format::fixed(i2, 1)));
        if i1 > 0.0 {
            row.push_str(&format!(", {}", format::fixed(100.0 * i2 / i1, 1)));
        } else {
            row.push_str(", 0.0");
        }
        row.push_str(&format!(", {}", format::fixed(i0, 1)));
        if i1 > 0.0 {
            row.push_str(&format!(", {}", format::fixed(100.0 * i0 / i1, 1)));
        } else {
            row.push_str(", 0.0");
        }
        row.push('\n');
        s.push_str(&row);
    });
    s
}

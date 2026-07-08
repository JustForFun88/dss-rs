//! `Export Capacity` (Pascal `ExportResults.pas` `ExportCapacity` (:2449) +
//! `CalcAndWriteMaxCurrents` (:551)): per-PDElement max phase current, that
//! current as a percentage of the normal/emergency rating, terminal-1 power, and
//! the branch customer counts — "similar to export currents except does only max
//! of the phases and compares that to the Normamps and Emergamps rating".

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::traits::SysCtx;
use crate::exec::registry::DssClass;
use crate::report::export::for_each_enabled_elem;
use crate::report::format;

/// Build the `Export Capacity` body (Pascal `ExportCapacity`). Walks the
/// PDElements calling the mutating `GetCurrents`/`Power` getters (PHASE8_PLAN
/// §2.1).
///
/// NOT_PORTED (seasonal-rating): `DSS.SeasonalRating`/`DSS.SeasonSignal` are now
/// real engine globals (`Set SeasonRating=`/`Set SeasonSignal=`, GAPS_PLAN
/// WPG.11 — see `Circuit::season_rating`/`season_signal`), but
/// `CalcAndWriteMaxCurrents`'s (`ExportResults.pas:567`) seasonal-amp-rating
/// override still isn't modeled here, for two independent reasons: (1) the
/// override branch it guards (`PElem.NumAmpRatings > 1`) is reachable in the
/// port — `Ratings=` (`PDElement.pas` `AmpRatings`) is ported per-element on
/// Line/Transformer — but wiring it through this generic per-element walk is
/// unverified by any corpus case (no live-oracle deck exercises `Export
/// Capacity` with `SeasonalRating=yes`); (2) the Pascal code itself
/// **mutates** `DSS.SeasonalRating := FALSE` on a miss (empty `SeasonSignal`/
/// unregistered curve) — a state-mutating *read* from a report export, the
/// same category CLAUDE.md's known-bug policy says not to reproduce (cf. the
/// VSConverter `GetCurrents` precedent). `SeasonalRating` is false-by-default,
/// and when false Pascal takes the element's own `NormAmps`/`EmergAmps` —
/// exactly what this always does — so every non-seasonal deck matches the
/// oracle bit-for-bit.
pub(crate) fn export_capacity(
    classes: &mut [DssClass],
    ckt: &Circuit,
    sys: &SysCtx,
    node_v: &[Complex64],
) -> String {
    let mut s = String::from(
        "Name, Imax, %normal, %emergency, kW, kvar, NumCustomers, TotalCustomers, NumPhases, kVBase\n",
    );
    for_each_enabled_elem(classes, &ckt.pd_elements, |name, elem| {
        elem.compute_iterminal(sys, node_v);
        let norm_amps = elem.norm_amps();
        let emerg_amps = elem.emerg_amps();

        // Max |I| over the terminal-1 phase conductors (Pascal `for i := 1 to
        // Nphases: Cabs(Cbuffer^[i])`, `Cbuffer` = the full Iterminal buffer).
        let nphases = elem.cd().nphases;
        let mut max_current = 0.0f64;
        for i in 0..nphases {
            let mag = elem.cd().iterminal[i].norm();
            if mag > max_current {
                max_current = mag;
            }
        }

        // `LocalPower := Power[1] * 0.001` (kW/kvar).
        let local_power = elem.terminal_power(sys, node_v, 1) * 0.001;

        // A zero rating prints `0`/`0` percentages (Pascal guards the divide).
        let (pct_norm, pct_emerg) = if norm_amps == 0.0 || emerg_amps == 0.0 {
            (0.0, 0.0)
        } else {
            (
                max_current / norm_amps * 100.0,
                max_current / emerg_amps * 100.0,
            )
        };

        let cd = elem.cd();
        // `Buses^[MapNodeToBus^[NodeRef^[1]].BusRef].kVBase` — the base kV of the
        // terminal-1 conductor-1 bus (= the element's first terminal bus).
        let kv_base = ckt.buses[cd.terminals[0].bus_ref].kv_base;

        s.push_str(&format!(
            "{}, {}, {}, {}, {}, {}, {}, {}, {}, {}\n",
            format::upper_elem_name(name),
            format::g(max_current, 6),
            format::fixed(pct_norm, 2),
            format::fixed(pct_emerg, 2),
            format::g(local_power.re, 6),
            format::g(local_power.im, 6),
            cd.branch_num_customers,
            cd.branch_total_customers,
            cd.nphases,
            format::g(kv_base, 3),
        ));
    });
    s
}

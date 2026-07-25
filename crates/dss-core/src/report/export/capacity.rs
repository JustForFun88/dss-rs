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
/// Seasonal ratings (dss_capi 0.15.x `55400a29`, WP-U1.5 E2): the norm/emerg
/// ratings come from `TPDElement.GetRatings`, which applies the globally-synced
/// `DSS.SeasonalRatingIdx` (`Circuit::seasonal_rating_idx`) to ANY PDElement with
/// `NumAmpRatings > 1`. The 0.14.5 baseline re-read the XYCurve per element AND
/// state-mutated `DSS.SeasonalRating := FALSE` on a miss (a state-mutating read
/// from a report export — not reproduced, per CLAUDE.md's known-bug policy); the
/// 0.15.x form drops that mutation by precomputing the index at solve time
/// (`sync_seasonal_rating_idx`). `seasonal_rating_idx == -1` (feature off) → the
/// element's own `NormAmps`/`EmergAmps`.
pub(crate) fn export_capacity(
    classes: &mut [DssClass],
    ckt: &Circuit,
    sys: &SysCtx,
    node_v: &[Complex64],
) -> String {
    let mut s = String::from(
        "Name, Imax, %normal, %emergency, kW, kvar, NumCustomers, TotalCustomers, NumPhases, kVBase\n",
    );
    let seasonal_idx = ckt.seasonal_rating_idx;
    for_each_enabled_elem(classes, &ckt.pd_elements, |name, elem| {
        elem.compute_iterminal(sys, node_v);
        let (norm_amps, emerg_amps) = elem.get_ratings(seasonal_idx);

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
        let kv_base = ckt.buses[cd.terminals[0].bus_idx()].kv_base;

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

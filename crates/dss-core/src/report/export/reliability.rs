//! `Export BusReliability` / `Export BranchReliability` (Pascal
//! `ExportResults.pas` `ExportBusReliability` (:3612) / `ExportBranchReliability`
//! (:3637)): the per-bus and per-branch reliability indices a prior `RelCalc`
//! (Pascal `DoLambdaCalcs` → `CalcReliabilityIndices`) populated.
//!
//! Both are read-only over the circuit: `RelCalc` already ran the backward
//! fault-rate/customer sweep and the forward interruption sweep, leaving the
//! `Bus_*` accumulators on every bus and the `Branch*`/`Accumulated*` fields on
//! every PD element. Without a prior `RelCalc` the fields are their zero defaults
//! (an all-zero report — the same degenerate output the oracle produces).

use crate::circuit::Circuit;
use crate::exec::registry::DssClass;
use crate::report::format;
use crate::util::check_for_blanks;

/// Build the `Export BusReliability` body (Pascal `ExportBusReliability`): one row
/// per bus (`BusList` order) — `BusFltRate`, `Bus_Num_Interrupt`,
/// `BusTotalNumCustomers`, `BusCustInterrupts`, `Bus_Int_Duration`,
/// `BusTotalMiles`, each `%-.11g` (11 sig; the customer count is `%d`).
pub(crate) fn export_bus_reliability(ckt: &Circuit) -> String {
    let mut s = String::from(
        "Bus, Lambda, Num-Interruptions, Num-Customers, Cust-Interruptions, Duration, Total-Miles\n",
    );
    for bus in &ckt.buses {
        // Pascal `CheckForBlanks(AnsiUpperCase(BusList.NameOfIndex(i)))`.
        s.push_str(&format!(
            "{}, {}, {}, {}, {}, {}, {}\n",
            check_for_blanks(&bus.name.to_uppercase()),
            format::g(bus.bus_flt_rate, 11),
            format::g(bus.bus_num_interrupt, 11),
            bus.bus_total_num_customers,
            format::g(bus.bus_cust_interrupts, 11),
            format::g(bus.bus_int_duration, 11),
            format::g(bus.bus_total_miles, 11),
        ));
    }
    s
}

/// Build the `Export BranchReliability` body (Pascal `ExportBranchReliability`):
/// one row per **enabled** PDElement (creation order). Two passes over the
/// PDElements: the first finds `MaxCustomers` (the max `BusTotalNumCustomers` over
/// every enabled branch's FROM bus, for the Duke recloser-siting `Cust-Miles`
/// column); the second writes each branch's fault-rate/customer/interrupt/mileage
/// indices + `SAIFI` (`BusCustInterrupts / BusTotalNumCustomers`, 0 when no
/// customers). Read-only — reads the `RelCalc`-populated fields.
pub(crate) fn export_branch_reliability(classes: &[DssClass], ckt: &Circuit) -> String {
    let mut s = String::from(
        "Element, Lambda, \"Accumulated-Lambda\", Num-Customers, Total-Customers, \
         Num-Interrupts, Cust-Interruptions, Cust-Durations, Total-Miles, Cust-Miles, SAIFI\n",
    );

    // The FROM bus of a PD element's metered terminal (Pascal
    // `Buses^[Terminals[FromTerminal - 1].BusRef]`; `from_terminal` is 0-based).
    let from_bus = |cd: &crate::elements::ckt::CktElementData| {
        &ckt.buses
            [cd.terminals[cd.from_terminal.expect("PD element has a metered terminal")].bus_idx()]
    };

    // Pass 1: `MaxCustomers` over all enabled PDElements (for `Cust-Miles`).
    let mut max_customers = 0i32;
    for &r in &ckt.pd_elements {
        if let Some(elem) = classes[r.class_ord()].arena.try_ckt_elem(r.index())
            && elem.cd().enabled
        {
            let ntot = from_bus(elem.cd()).bus_total_num_customers;
            if ntot > max_customers {
                max_customers = ntot;
            }
        }
    }

    // Pass 2: write the per-branch report (PDELEMENTS only).
    for &r in &ckt.pd_elements {
        let class_name = classes[r.class_ord()].props.class_name();
        let obj = &classes[r.class_ord()].arena[r.index()];
        let Some(elem) = classes[r.class_ord()].arena.try_ckt_elem(r.index()) else {
            continue;
        };
        let cd = elem.cd();
        if !cd.enabled {
            continue;
        }
        let bus = from_bus(cd);
        let saifi = if bus.bus_total_num_customers > 0 {
            bus.bus_cust_interrupts / bus.bus_total_num_customers as f64
        } else {
            0.0
        };
        s.push_str(&format!(
            "{}.{}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}\n",
            class_name,
            obj.data().name(),
            format::g(cd.branch_flt_rate, 11),
            format::g(cd.accumulated_br_flt_rate, 11),
            cd.branch_num_customers,
            cd.branch_total_customers,
            format::g(bus.bus_num_interrupt, 11),
            format::g(cd.branch_total_customers as f64 * bus.bus_num_interrupt, 11),
            format::g(bus.bus_cust_durations, 11),
            format::g(cd.accumulated_miles_downstream, 11),
            format::g(
                (max_customers - cd.branch_total_customers) as f64
                    * cd.accumulated_miles_downstream,
                11,
            ),
            format::g(saifi, 11),
        ));
    }
    s
}

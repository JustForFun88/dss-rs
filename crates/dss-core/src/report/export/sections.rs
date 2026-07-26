//! `Export Sections` (Pascal `ExportResults.pas` `ExportSections` (:3873)): the
//! per-feeder-section reliability data a prior `RelCalc`
//! (`CalcReliabilityIndices`) computed and persisted on each EnergyMeter
//! (`SectionCount` + `FeederSections`).
//!
//! Read-only over the meters: without a prior `RelCalc` every meter's
//! `SectionCount` is 0 and the report is header-only, exactly like Pascal
//! (`FeederSections = NIL`, the `1..SectionCount` loop never runs). Pascal also
//! sets `ActiveCircuit.ActiveCktElement` to each section's head branch as a
//! side effect of building `FullName`; the Rust formatter is read-only and
//! skips that (no gate observes the active element across an `Export`).

use crate::circuit::Circuit;
use crate::elements::meter::EnergyMeter;
use crate::elements::traits::ElemId;
use crate::exec::registry::DssClass;
use crate::report::format;

/// Pascal `getOCPDeviceTypeString` (`ExportResults.pas:3859`).
fn ocp_device_type_string(icode: i32) -> &'static str {
    match icode {
        1 => "FUSE",
        2 => "RECLOSER",
        3 => "RELAY",
        _ => "Unknown",
    }
}

/// Build the `Export Sections` body (Pascal `ExportSections`): one row per
/// feeder section of the selected meter (`meter = Some(..)`, the
/// `meter=<name>` form) or of every meter in creation order (`None`). Each row:
/// meter name, 1-based section id, the head branch's 1-based `SequenceList`
/// index, the OCP device type, customer/branch counts, and the section
/// fault-rate/repair aggregates (`%-.6g`), ending with the quoted head-branch
/// `FullName`.
pub(crate) fn export_sections(
    classes: &[DssClass],
    ckt: &Circuit,
    meter: Option<ElemId>,
) -> String {
    // Header: verbatim, including Pascal's trailing space before the newline.
    let mut s = String::from(
        "Meter, SectionID, SeqIndex, DeviceType, NumCustomers, NumBranches, AvgRepairHrs, \
         TotalDownlineCust, SectFaultRate, SumFltRatesXRepairHrs, SumBranchFltRates, HeadBranch \n",
    );

    let targets: Vec<ElemId> = match meter {
        Some(r) => vec![r],
        None => ckt.energy_meters.clone(),
    };
    for r in targets {
        let obj = &classes[r.class_ord()].arena[r.index()];
        let meter_name = obj.data().name().to_string();
        let em = obj
            .as_any()
            .downcast_ref::<EnergyMeter>()
            .expect("energy_meters holds EnergyMeter objects");
        // `SectionCount` and `FeederSections` are written together by a
        // successful `CalcReliabilityIndices` (and the count alone is zeroed on
        // its no-OCP abort), so `1..=count` always indexes the same run's array.
        for i in 1..=em.section_count().max(0) as usize {
            let sec = &em.feeder_sections()[i];
            // Pascal `sequenceList.Get(SeqIndex)` (1-based) → `FullName`,
            // `EncloseQuotes` = double quotes (`Utilities.pas:395`).
            let head = em.sequence_list()[sec.seq_index - 1];
            let head_full = format!(
                "{}.{}",
                classes[head.class_ord()].props.class_name(),
                classes[head.class_ord()].arena[head.index()].data().name()
            );
            s.push_str(&format!(
                "{}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, \"{}\"\n",
                meter_name,
                i,
                sec.seq_index,
                ocp_device_type_string(sec.ocp_device_type),
                sec.n_customers,
                sec.n_branches,
                format::g(sec.average_repair_time, 6),
                sec.total_customers,
                format::g(sec.sect_fault_rate, 6),
                format::g(sec.sum_flt_rates_x_repair_hrs, 6),
                format::g(sec.sum_branch_flt_rates, 6),
                head_full,
            ));
        }
    }
    s
}

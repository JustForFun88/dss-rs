//! EnergyMeter reliability indices (WP6.6) — Pascal `TExecHelper.DoLambdaCalcs`
//! / `TEnergyMeterObj.CalcReliabilityIndices` (EnergyMeter.pas l.2411): the
//! backward fault-rate sweep, the forward interruption sweep (counting the
//! feeder sections delimited by OCP devices), then SAIFI/SAIDI/CAIDI.

use crate::circuit::{Bus, Circuit};
use crate::elements::ckt::ElemFlags;
use crate::elements::meter::energymeter::FeederSection;
use crate::elements::pc::load::Load;
use crate::elements::traits::{CktElement, ElemId, ElemStore, TypedStore};

use super::meter_mut;

// ===================== Reliability (WP6.6) ==================================

/// FROM bus (0-based index into `ckt.buses`) of a PD element's metered terminal.
fn pd_from_bus(store: &dyn ElemStore, r: ElemId) -> usize {
    let cd = store.ckt_elem(r).cd();
    cd.terminals[cd
        .from_terminal
        .expect("zone PD element has a metered terminal")]
    .bus_idx()
}

/// Pascal `TExecHelper.DoLambdaCalcs` (ExecHelper.pas l.4847): zero every bus's
/// `BusFltRate`/`Bus_Num_Interrupt`, then run `CalcReliabilityIndices` on each
/// EnergyMeter. The caller (the executive) has already checked at least one
/// meter exists and parsed the `AssumeRestoration` flag. Returns the per-meter
/// error messages (Pascal calls `DoSimpleMsg` and continues the loop).
pub(crate) fn calc_all_reliability_indices(
    ckt: &mut Circuit,
    store: &mut dyn ElemStore,
    assume_restoration: bool,
) -> Vec<String> {
    // initialize bus quantities
    for b in ckt.buses.iter_mut() {
        b.bus_flt_rate = 0.0;
        b.bus_num_interrupt = 0.0;
    }

    let mut errors = Vec::new();
    let meters = ckt.energy_meters.clone();
    for meter_ref in meters {
        // Pascal `pMeter.AssumeRestoration := AssumeRestoration` before the calc;
        // the field is also read by the next zone build's customer roll-up.
        meter_mut(store, meter_ref).set_assume_restoration(assume_restoration);
        if let Err(e) = calc_reliability_indices(meter_ref, assume_restoration, ckt, store) {
            errors.push(e);
        }
    }
    errors
}

/// Pascal `TEnergyMeterObj.CalcReliabilityIndices` (EnergyMeter.pas l.2411):
/// the backward fault-rate sweep, the forward interruption sweep (which counts
/// the feeder *sections* delimited by OCP devices), then SAIFI/SAIDI/CAIDI.
///
/// A zone with no OCP device (no enabled Relay/Recloser/Fuse on any branch) has
/// `SectionCount == 0` and the sweep aborts with error 52902 exactly as the
/// oracle does. Since WP7.2 step 3 those controls set `Flg.HasOCPDevice` on
/// their controlled element, so a protected zone reaches the live section/SAIFI
/// math below.
fn calc_reliability_indices(
    meter_ref: ElemId,
    assume_restoration: bool,
    ckt: &mut Circuit,
    store: &mut dyn ElemStore,
) -> Result<(), String> {
    let meter_full = format!("EnergyMeter.{}", store.obj(meter_ref).data().name());
    let (seq, load_list, source_num_int, source_int_dur) = {
        let em = meter_mut(store, meter_ref);
        (
            em.sequence_list().to_vec(),
            em.load_list().to_vec(),
            em.source_num_interruptions(),
            em.source_int_duration(),
        )
    };
    if seq.is_empty() {
        // Pascal `not Assigned(SequenceList)` (zone never built).
        return Err(format!("{meter_full} Zone not defined properly."));
    }

    // Zero reliability accumulators (each PD element zeros its FROM bus).
    for &r in seq.iter().rev() {
        let from_bus = pd_from_bus(store, r);
        ckt.buses[from_bus].zero_reliability_accums();
    }

    // Backward sweep: CalcFltRate (sets BranchFltRate) + AccumFltRate.
    for &r in seq.iter().rev() {
        let rel = store.ckt_elem(r).reliability_data();
        let (from_t, from_bus, to_bus, branch_total, has_ocp) = {
            let cd = store.ckt_elem(r).cd();
            // `from_t`/`to_t` are 0-based terminal indices.
            let from_t = cd
                .from_terminal
                .expect("zone PD element has a metered terminal");
            let to_t = if from_t == 1 { 0 } else { 1 };
            (
                from_t,
                cd.terminals[from_t].bus_idx(),
                cd.terminals[to_t].bus_idx(),
                cd.branch_total_customers,
                cd.flags.contains(ElemFlags::HAS_OCP_DEVICE),
            )
        };
        let accumulated_br = ckt.buses[to_bus].bus_flt_rate + rel.branch_flt_rate;
        let accumulated_miles = ckt.buses[to_bus].bus_total_miles + rel.miles_this_line;
        {
            let cd = store.ckt_elem_mut(r).cd_mut();
            cd.to_terminal = Some(if from_t == 1 { 0 } else { 1 });
            cd.branch_flt_rate = rel.branch_flt_rate;
            cd.accumulated_br_flt_rate = accumulated_br;
            cd.accumulated_miles_downstream = accumulated_miles;
        }
        ckt.buses[from_bus].bus_total_num_customers += branch_total;
        ckt.buses[from_bus].bus_total_miles += accumulated_miles;
        // A fault interrupter isolates all downline faults from the FROM bus.
        if !has_ocp {
            ckt.buses[from_bus].bus_flt_rate += accumulated_br;
        }
    }

    // Forward sweep: number of interruptions + section assignment. The buses it
    // touches are exactly this meter's zone — recorded so the duration loop
    // below can stay inside it (the clean fix, G2.2a).
    let mut section_count: i32 = 0;
    let mut zone_buses: Vec<usize> = Vec::with_capacity(seq.len() + 1);
    {
        let first = seq[0];
        let cd = store.ckt_elem(first).cd();
        let from_bus = cd.terminals[cd
            .from_terminal
            .expect("zone PD element has a metered terminal")]
        .bus_idx();
        let pbus = &mut ckt.buses[from_bus];
        pbus.bus_num_interrupt = source_num_int;
        pbus.bus_cust_interrupts = source_num_int * pbus.bus_total_num_customers as f64;
        pbus.bus_int_duration = source_int_dur;
        pbus.bus_section_id = section_count; // section before 1st OCP device is 0
        zone_buses.push(from_bus);
    }
    for &r in &seq {
        let (from_t, from_bus, to_bus, accumulated_br, has_ocp, has_auto) = {
            let cd = store.ckt_elem(r).cd();
            // `from_t`/`to_t` are 0-based terminal indices.
            let from_t = cd
                .from_terminal
                .expect("zone PD element has a metered terminal");
            let to_t = if from_t == 1 { 0 } else { 1 };
            (
                from_t,
                cd.terminals[from_t].bus_idx(),
                cd.terminals[to_t].bus_idx(),
                cd.accumulated_br_flt_rate,
                cd.flags.contains(ElemFlags::HAS_OCP_DEVICE),
                cd.flags.contains(ElemFlags::HAS_AUTO_OCP_DEVICE),
            )
        };
        let from_num_int = ckt.buses[from_bus].bus_num_interrupt;
        let from_section = ckt.buses[from_bus].bus_section_id;
        // No interrupting device → same num of interruptions downline.
        ckt.buses[to_bus].bus_num_interrupt = from_num_int;
        if has_ocp {
            if assume_restoration && has_auto {
                ckt.buses[to_bus].bus_num_interrupt = accumulated_br;
            } else {
                ckt.buses[to_bus].bus_num_interrupt += accumulated_br;
            }
            section_count += 1;
            ckt.buses[to_bus].bus_section_id = section_count;
        } else {
            ckt.buses[to_bus].bus_section_id = from_section;
        }
        zone_buses.push(to_bus);
        let section_id = ckt.buses[to_bus].bus_section_id;
        let cd = store.ckt_elem_mut(r).cd_mut();
        cd.to_terminal = Some(if from_t == 1 { 0 } else { 1 });
        cd.branch_section_id = section_id;
    }

    if section_count == 0 {
        // No OCP devices (Phase 6 always lands here, matching the oracle).
        // Pascal mutated the meter's `SectionCount` field during the forward
        // sweep, so the abort leaves 0 there (a later `Export Sections` writes
        // no rows) while the stale `FeederSections` array is untouched.
        meter_mut(store, meter_ref).set_section_count(0);
        return Err(
            "Error: No Overcurrent Protection device (Relay, Recloser, or Fuse) defined. \
             Aborting Reliability calc."
                .to_string(),
        );
    }

    // Allocate + init the feeder-section array (indices 0..=section_count).
    let mut sections = vec![FeederSection::default(); section_count as usize + 1];

    // Backward sweep: N·FaultRates and section properties. `idx` is the Pascal
    // 1-based `SequenceList` index (`for idx := Count downto 1`), recorded as the
    // section's `SeqIndex`.
    for idx in (1..=seq.len()).rev() {
        let r = seq[idx - 1];
        let rel = store.ckt_elem(r).reliability_data();
        let (
            from_bus,
            to_t,
            branch_section_id,
            branch_flt,
            branch_num,
            branch_total,
            accum_br,
            has_ocp,
            ocp_type,
        ) = {
            let cd = store.ckt_elem(r).cd();
            (
                cd.terminals[cd
                    .from_terminal
                    .expect("zone PD element has a metered terminal")]
                .bus_idx(),
                cd.to_terminal.expect("reliability sweep set to_terminal"),
                cd.branch_section_id,
                cd.branch_flt_rate,
                cd.branch_num_customers,
                cd.branch_total_customers,
                cd.accumulated_br_flt_rate,
                cd.flags.contains(ElemFlags::HAS_OCP_DEVICE),
                cd.ocp_device_type,
            )
        };
        // CalcCustInterrupts.
        ckt.buses[from_bus].bus_cust_interrupts +=
            ckt.buses[from_bus].bus_num_interrupt * branch_total as f64;

        if branch_section_id <= 0 {
            continue;
        }
        let to_bus = store.ckt_elem(r).cd().terminals[to_t].bus_idx();
        let to_num_int = ckt.buses[to_bus].bus_num_interrupt;
        let s = &mut sections[branch_section_id as usize];
        s.n_customers += branch_num;
        s.n_branches += 1;
        s.sum_branch_flt_rates += to_num_int * branch_flt;
        s.sum_flt_rates_x_repair_hrs += to_num_int * branch_flt * rel.hrs_to_repair;
        if has_ocp {
            // Pascal `pSection.OCPDeviceType := GetOCPDeviceType(PD_Elem)` — the
            // 1/2/3 ordinal recorded on the element when its Relay/Recloser/Fuse
            // resolved (WP7.2 step 3). `SeqIndex` is the 1-based sequence index.
            s.ocp_device_type = ocp_type;
            s.seq_index = idx;
            s.total_customers = branch_total;
            s.sect_fault_rate = accum_br;
        }
    }

    // Average interruption duration per section (idx 0 excluded).
    for s in sections.iter_mut().skip(1) {
        s.average_repair_time = s.sum_flt_rates_x_repair_hrs / s.sum_branch_flt_rates;
    }

    // Bus interruption durations — over **this meter's own zone**, the buses
    // its forward sweep above numbered (`zone_buses`). `FeederSections` is this
    // meter's array, sized to this meter's `SectionCount` (r4133
    // `Version8/Source/Meters/EnergyMeter.pas:2507`, dss_capi
    // `EnergyMeter.pas:2461`), and a `BusSectionID` is only an index into it
    // while the bus belongs to the zone that wrote it — which is exactly the
    // scope the per-section loop just above already keeps to (r4133 `:2561-2563`).
    //
    // Upstream instead walks **every circuit bus** (r4133
    // `Version8/Source/Meters/EnergyMeter.pas:2567-2574`, dss_capi
    // `EnergyMeter.pas:2521-2526`), and a foreign `BusSectionID` survives to be
    // read there because the zeroing that clears it is itself per-zone (r4133
    // `:2472` walks this meter's `SequenceList`). Two regimes (full analysis in
    // `investigations/reliability_bus_int_duration_oob_bug_report.md`):
    //   (a) in-range id (`≤ section_count`) — a **deterministic** cross-zone
    //       overwrite: the later meter replaces an earlier meter's bus duration
    //       with the average repair time of its *own* unrelated section, so the
    //       reported durations depend on meter order. Both gating oracles carry
    //       it; neither lane reproduces it (`GOLDEN_REBASE_PLAN.md` G2.2a;
    //       `issue-02`), so a meter's durations no longer depend on which meters
    //       ran before it.
    //   (b) out-of-range id — Pascal reads `FeederSections[id]` past the
    //       `section_count + 1` allocation: an OOB heap read, **proven
    //       nondeterministic** (the report probes it across fresh processes —
    //       the first slot past the array reads a stable 0 from zeroed slack,
    //       the next slots read live garbage: 1.5e-311, 3.1e-314, 2.5e-290,
    //       6.0e-118). Safe Rust cannot and must not reproduce it in *either*
    //       lane: `.get()` returns `None`, so the bus keeps its own-zone
    //       duration. There is no defined upstream value to pin, so no gate can
    //       observe the difference — and with the zone-scoped walk a foreign id
    //       is not read at all.
    let set_duration = |b: &mut Bus| {
        if b.bus_section_id > 0
            && let Some(s) = sections.get(b.bus_section_id as usize)
        {
            b.bus_int_duration = source_int_dur + s.average_repair_time;
        }
    };
    for &bi in &zone_buses {
        set_duration(&mut ckt.buses[bi]);
    }

    // SAIFI / SAIFIkW / CustInterrupts from the load list.
    let mut saifi_kw = 0.0;
    let mut cust_interrupts = 0.0;
    let mut dbl_ncusts = 0.0;
    let mut dbl_kw = 0.0;
    for &load_ref in &load_list {
        let (num_cust, rel_w, kw_base, pbus) = {
            let load = store
                .typed::<Load>(load_ref)
                .expect("load_list holds Load objects");
            (
                load.num_customers as f64,
                load.rel_weighting,
                load.kw_base,
                load.cd().terminals[0].bus_idx(),
            )
        };
        let bus_num_int = ckt.buses[pbus].bus_num_interrupt;
        let bus_total_num = ckt.buses[pbus].bus_total_num_customers;
        let bus_int_dur = ckt.buses[pbus].bus_int_duration;
        cust_interrupts += num_cust * rel_w * bus_num_int;
        saifi_kw += kw_base * rel_w * bus_num_int;
        dbl_ncusts += num_cust * rel_w;
        dbl_kw += kw_base * rel_w;
        ckt.buses[pbus].bus_cust_durations =
            (bus_total_num as f64 + num_cust) * rel_w * bus_int_dur * bus_num_int;
    }

    // SAIDI from sections (idx 0 ignored).
    let mut saidi = 0.0;
    for s in sections.iter().skip(1) {
        saidi += s.sect_fault_rate * s.average_repair_time * s.total_customers as f64;
    }

    let mut saifi = 0.0;
    let mut caidi = 0.0;
    if dbl_ncusts > 0.0 {
        saifi = cust_interrupts / dbl_ncusts;
        saidi /= dbl_ncusts;
    }
    if saifi > 0.0 {
        caidi = saidi / saifi;
    }
    if dbl_kw > 0.0 {
        saifi_kw /= dbl_kw;
    }

    let em = meter_mut(store, meter_ref);
    em.set_reliability_results(saifi, saifi_kw, saidi, caidi, cust_interrupts);
    // Persist the section data on the meter (Pascal keeps `SectionCount` +
    // `FeederSections` as fields; `Export Sections` reads them back).
    em.set_section_count(section_count);
    em.set_feeder_sections(sections);
    Ok(())
}

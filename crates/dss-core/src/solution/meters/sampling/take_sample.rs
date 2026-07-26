//! TakeSample — Pascal `TEnergyMeter.ResetAll` / `SampleAll` /
//! `TEnergyMeterObj.TakeSample` (EnergyMeter.pas l.851 / l.900 / l.1289). The
//! meter's branch tree and registers are moved out of the meter object for the
//! walk (the store keeps the meter borrowed), the zone's PD/PC elements are
//! read/mutated through the store, and the registers are written back at the
//! end. Split out of `sampling/mod.rs` (no behavioral change).

use num_complex::Complex64;

use crate::circuit::{Circuit, ElemKind};
use crate::elements::meter::energymeter::{EnergyMeter, NUM_EM_VBASE, reg};
use crate::elements::pc::generator::Generator;
use crate::elements::pc::load::Load;
use crate::elements::pc::{PVSystem, Storage};
use crate::elements::traits::{CktElement, ElemId, ElemStore, SysCtx, TypedStore};

use super::super::meter_mut;

/// Pascal `TEnergyMeter.ResetAll` (l.851): close/recreate the demand-interval
/// machinery (the `DI_yr_<year>` directory + `DI_Totals` stream, when `Set
/// DemandInterval=yes`), reset every meter's registers and the `SystemMeter`,
/// plus the Generator/Storage/PVSystem `ResetRegistersAll` tail (l.895-897) —
/// those DER registers accumulate in `SampleAll`'s tail below, so they reset
/// here too.
pub(crate) fn reset_all_meters(
    ckt: &mut Circuit,
    store: &mut dyn ElemStore,
    output_directory: &std::path::Path,
    errors: &mut crate::diag::ErrorLog,
) {
    super::super::demand_interval::reset_all_di(ckt, store, output_directory, errors);
    let meters = ckt.energy_meters.clone();
    for meter_ref in meters {
        meter_mut(store, meter_ref).reset_registers();
    }
    ckt.em_di.system_meter.reset();
    // Pascal `TEnergyMeter.ResetAll` l.895-897 (the reset is not gated on any
    // meter existing).
    for r in ckt.generators.clone() {
        store
            .typed_mut::<Generator>(r)
            .expect("generators holds Generator")
            .reset_registers();
    }
    for r in ckt.storages.clone() {
        store
            .typed_mut::<Storage>(r)
            .expect("storages holds Storage")
            .reset_registers();
    }
    for r in ckt.pv_systems.clone() {
        store
            .typed_mut::<PVSystem>(r)
            .expect("pv_systems holds PVSystem")
            .reset_registers();
    }
}

/// Pascal `TEnergyMeter.SampleAll` (l.900): sample every enabled meter (each
/// followed by its `WriteDemandIntervalData` tail under `SaveDemandInterval`),
/// then the `SystemMeter` sample + the demand-interval totals/report rows
/// (l.911-925), then the Generator/Storage/PVSystem `SampleAll` tail
/// (l.928-931) — the DER energy registers are accumulated **here**,
/// unconditionally (not gated on a meter existing), which is why `Export
/// Generators`/`Storage_Meters`/`PVSystem_Meters` see nonzero registers even
/// with no `EnergyMeter` defined.
pub(crate) fn take_sample_all(ckt: &mut Circuit, store: &mut dyn ElemStore, sys: &SysCtx) {
    let save_di = ckt.em_di.save_demand_interval;
    let meters = ckt.energy_meters.clone();
    for meter_ref in meters {
        let enabled = store
            .typed::<EnergyMeter>(meter_ref)
            .expect("energy_meters holds EnergyMeter objects")
            .enabled();
        if enabled {
            take_sample_one(meter_ref, ckt, store, sys);
            // The `TEnergyMeterObj.TakeSample` tail (l.1689): the per-meter DI
            // row + the class `DI_RegisterTotals` accumulation.
            if save_di {
                super::super::demand_interval::write_meter_demand_interval_data(
                    meter_ref, ckt, store,
                );
            }
        }
    }
    super::super::demand_interval::sample_all_di_tail(ckt, store, sys);
    sample_all_der(ckt, store, sys);
}

/// The DER `SampleAll` tail of `TEnergyMeter.SampleAll` (Generator/Storage/
/// PVSystem, l.928-931). Each class walks its elements accumulating the energy
/// registers `Export Generators`/`Storage_Meters`/`PVSystem_Meters` dump. Pascal's
/// `<Class>.SampleAll` guards `if enabled then TakeSample`; here the `!enabled`
/// early-return lives **inside** each `take_sample`, so the guard is honored
/// without an outer filter (a disabled element is a cheap no-op).
fn sample_all_der(ckt: &Circuit, store: &mut dyn ElemStore, sys: &SysCtx) {
    let interval_hrs = ckt.solution.interval_hrs;
    let trapezoidal = ckt.trapezoidal_integration;
    let positive_sequence = ckt.positive_sequence;
    let price_signal = ckt.price_signal;
    for r in ckt.generators.clone() {
        store
            .typed_mut::<Generator>(r)
            .expect("generators holds Generator")
            .take_sample(interval_hrs, trapezoidal, positive_sequence, price_signal);
    }
    // Pascal comment: "samples energymeter part of storage elements (not update)".
    for r in ckt.storages.clone() {
        store
            .typed_mut::<Storage>(r)
            .expect("storages holds Storage")
            .take_sample(sys, &ckt.solution.node_v, interval_hrs, trapezoidal);
    }
    for r in ckt.pv_systems.clone() {
        store
            .typed_mut::<PVSystem>(r)
            .expect("pv_systems holds PVSystem")
            .take_sample(interval_hrs, trapezoidal, positive_sequence, price_signal);
    }
}

/// PD-element kind probe used by the loss split.
fn branch_kind(store: &dyn ElemStore, r: ElemId) -> (bool, bool, usize) {
    let is_line = matches!(store.kind(r), ElemKind::Line);
    let is_xfmr = matches!(store.kind(r), ElemKind::Transformer);
    let nphases = store.ckt_elem(r).cd().nphases;
    (is_line, is_xfmr, nphases)
}

/// Pascal `TEnergyMeterObj.TakeSample` (l.1289).
fn take_sample_one(meter_ref: ElemId, ckt: &Circuit, store: &mut dyn ElemStore, sys: &SysCtx) {
    let trapezoidal = ckt.trapezoidal_integration;
    let node_v = &ckt.solution.node_v;
    let delta_hrs = ckt.solution.interval_hrs;
    let norm_min = ckt.normal_min_volts;
    let emerg_min = ckt.emerg_min_volts;

    // CheckBranchList: exit if the zone was never built. Take the tree and the
    // register accumulators out of the meter for the walk.
    let Some((mut tree, mut st)) = meter_mut(store, meter_ref).begin_take_sample(trapezoidal)
    else {
        return;
    };

    // Energy in the branch the meter is connected to.
    let metered = st
        .metered_element
        .expect("metered element present once zone built");
    let s_local = store
        .ckt_elem_mut(metered)
        .terminal_power(sys, node_v, st.metered_terminal)
        * 0.001;
    let s_local_kva = s_local.norm();
    st.integrate(reg::KWH, s_local.re, delta_hrs);
    st.integrate(reg::KVARH, s_local.im, delta_hrs);
    st.set_drag(reg::MAX_KW, s_local.re);
    st.set_drag(reg::MAX_KVA, s_local_kva);

    // Loss accumulators.
    let mut total_losses = Complex64::ZERO;
    let mut total_load_losses = Complex64::ZERO;
    let mut total_no_load_losses = Complex64::ZERO;
    let mut total_line_losses = Complex64::ZERO;
    let mut total_line_mode_losses = Complex64::ZERO;
    let mut total_zero_mode_losses = Complex64::ZERO;
    let mut total_3phase_losses = Complex64::ZERO;
    let mut total_1phase_losses = Complex64::ZERO;
    let mut total_transformer_losses = Complex64::ZERO;
    let mut vbase_total_losses = [0.0; NUM_EM_VBASE];
    let mut vbase_line_losses = [0.0; NUM_EM_VBASE];
    let mut vbase_load_losses = [0.0; NUM_EM_VBASE];
    let mut vbase_no_load_losses = [0.0; NUM_EM_VBASE];
    let mut vbase_load = [0.0; NUM_EM_VBASE];

    // Phase Voltage arrays (Pascal l.1379-1391): reset the live vbase slots.
    if st.f_phase_voltage_report {
        for i in 0..NUM_EM_VBASE {
            if st.vbase_list.get(i).copied().unwrap_or(0.0) > 0.0 {
                for j in 0..3 {
                    st.vphase_max[i * 3 + j] = 0.0;
                    st.vphase_min[i * 3 + j] = 9999.0;
                    st.vphase_accum[i * 3 + j] = 0.0;
                    st.vphase_accum_count[i * 3 + j] = 0;
                }
            }
        }
    }

    let mut max_excess_kw_norm = 0.0_f64;
    let mut max_excess_kw_emerg = 0.0_f64;

    // ---- EEN/UE overload pass --------------------------------------------
    if st.local_only {
        let elem = store.ckt_elem_mut(metered);
        max_excess_kw_norm = elem
            .excess_kva_norm(sys, node_v, st.metered_terminal)
            .re
            .abs();
        max_excess_kw_emerg = elem
            .excess_kva_emerg(sys, node_v, st.metered_terminal)
            .re
            .abs();
    } else {
        let mut cur = tree.first();
        while let Some(branch) = cur {
            let (from_terminal, shunts) = {
                let n = tree.present_node();
                (n.from_terminal, n.shunts.clone())
            };
            let parent = tree.parent();

            // ExcesskVANorm/Emerg set the PD element's Overload_EEN/UE flags.
            let (een, ue) = {
                let elem = store.ckt_elem_mut(branch);
                let een = elem.excess_kva_norm(sys, node_v, from_terminal).re.abs();
                let ue = elem.excess_kva_emerg(sys, node_v, from_terminal).re.abs();
                (een, ue)
            };
            if st.zone_is_radial {
                if ue > max_excess_kw_emerg {
                    max_excess_kw_emerg = ue;
                }
                if een > max_excess_kw_norm {
                    max_excess_kw_norm = een;
                }
            } else {
                max_excess_kw_emerg += ue;
                max_excess_kw_norm += een;
            }

            // Roll the parent's overload into this branch (use the larger).
            if let Some(p) = parent {
                let (p_een, p_ue) = {
                    let pc = store.ckt_elem(p).cd();
                    (pc.overload_een, pc.overload_ue)
                };
                let cd = store.ckt_elem_mut(branch).cd_mut();
                cd.overload_een = cd.overload_een.max(p_een);
                cd.overload_ue = cd.overload_ue.max(p_ue);
            }

            // Mark loads (not generators) by the degree of overload.
            let (ov_een, ov_ue) = {
                let cd = store.ckt_elem(branch).cd();
                (cd.overload_een, cd.overload_ue)
            };
            for pc in &shunts {
                if let Some(load) = store.typed_mut::<Load>(*pc) {
                    load.een_factor = if ov_een > 0.0 && st.zone_is_radial && !st.voltage_ue_only {
                        ov_een
                    } else {
                        0.0
                    };
                    load.ue_factor = if ov_ue > 0.0 && st.zone_is_radial && !st.voltage_ue_only {
                        ov_ue
                    } else {
                        0.0
                    };
                }
            }
            cur = tree.go_forward();
        }
    }

    // ---- Load / generation / loss accumulation pass ----------------------
    let mut total_zone_kw = 0.0_f64;
    let mut total_zone_kvar = 0.0_f64;
    let mut total_load_een = 0.0_f64;
    let mut total_load_ue = 0.0_f64;
    let mut total_gen_kw = 0.0_f64;
    let mut total_gen_kvar = 0.0_f64;

    let mut cur = tree.first();
    while let Some(branch) = cur {
        let (volt_base_index, from_bus, shunts) = {
            let n = tree.present_node();
            (n.volt_base_index, n.from_bus, n.shunts.clone())
        };
        let vbi = volt_base_index; // 1-based; 0 = none

        for pc in &shunts {
            let is_load = matches!(store.kind(*pc), ElemKind::Load);
            let is_gen = matches!(store.kind(*pc), ElemKind::Generator);
            if is_load && !st.local_only {
                let load = store.typed_mut::<Load>(*pc).expect("checked is_load");
                let load_kw = accumulate_load(
                    load,
                    sys,
                    node_v,
                    st.excess_flag,
                    norm_min,
                    emerg_min,
                    &mut total_zone_kw,
                    &mut total_zone_kvar,
                    &mut total_load_een,
                    &mut total_load_ue,
                );
                if st.f_vbase_losses && vbi > 0 {
                    vbase_load[vbi as usize - 1] += load_kw;
                }
            } else if is_gen {
                let gen_obj = store.typed_mut::<Generator>(*pc).expect("checked is_gen");
                // Pascal `Accumulate_Gen` (l.2198): the `var` params are bound
                // to the gen totals, not the zone-load totals.
                let s = -gen_obj.terminal_power(sys, node_v, 1) * 0.001;
                total_gen_kw += s.re;
                total_gen_kvar += s.im;
            }
        }

        if st.f_losses {
            let (is_line, is_xfmr, nphases) = branch_kind(store, branch);
            let (s_total, s_load, s_no_load) = {
                let elem = store.ckt_elem_mut(branch);
                let (t, l, n) = elem.get_losses_split(sys, node_v);
                (t * 0.001, l * 0.001, n * 0.001)
            };
            total_losses += s_total;
            total_load_losses += s_load;
            total_no_load_losses += s_no_load;

            if is_line && st.f_line_losses {
                total_line_losses += s_total;
                if st.f_seq_losses {
                    let (mut pos, neg, zero) = {
                        let elem = store.ckt_elem_mut(branch);
                        elem.get_seq_losses(sys, node_v)
                    };
                    pos += neg; // add line modes together
                    pos *= 0.001;
                    let zero = zero * 0.001;
                    total_line_mode_losses += pos;
                    total_zero_mode_losses += zero;
                }
                if st.f_3phase_losses {
                    if nphases == 3 {
                        total_3phase_losses += s_total;
                    } else {
                        total_1phase_losses += s_total;
                    }
                }
            } else if is_xfmr && st.f_xfmr_losses {
                total_transformer_losses += s_total;
            }

            if st.f_vbase_losses && vbi > 0 {
                let k = vbi as usize - 1;
                vbase_total_losses[k] += s_total.re;
                if is_line {
                    vbase_line_losses[k] += s_total.re;
                } else if is_xfmr {
                    vbase_load_losses[k] += s_load.re;
                    vbase_no_load_losses[k] += s_no_load.re;
                }
            }

            // Min/max/average pu voltage of phases 1..3 at the FROM bus
            // (Pascal l.1566-1593; inside the `if FLosses` block, like the
            // vbase losses). Note the Pascal quirk: `Cabs(V)/kVBase` with no
            // `0.001`, so the accumulators carry 1000·pu — the report writer
            // multiplies by 0.001.
            if st.f_phase_voltage_report
                && vbi > 0
                && let Some(fb) = from_bus
                && ckt.buses[fb].kv_base > 0.0
            {
                let bus = &ckt.buses[fb];
                for i in 0..bus.num_nodes_this_bus() {
                    let j = bus.get_num(i);
                    if !(1..=3).contains(&j) {
                        continue;
                    }
                    let pu_v = node_v[bus.get_ref(i)].norm() / bus.kv_base;
                    let idx = (vbi as usize - 1) * 3 + (j as usize - 1);
                    if pu_v > st.vphase_max[idx] {
                        st.vphase_max[idx] = pu_v;
                    }
                    if pu_v < st.vphase_min[idx] {
                        st.vphase_min[idx] = pu_v;
                    }
                    st.vphase_accum[idx] += pu_v;
                    st.vphase_accum_count[idx] += 1;
                }
            }
        }

        cur = tree.go_forward();
    }

    // ---- Integrate losses, load, generation ------------------------------
    st.integrate(reg::LOAD_EEN, total_load_een, delta_hrs);
    st.integrate(reg::LOAD_UE, total_load_ue, delta_hrs);
    st.integrate(reg::ZONE_LOSSES_KWH, total_losses.re, delta_hrs);
    st.integrate(reg::ZONE_LOSSES_KVARH, total_losses.im, delta_hrs);
    st.integrate(reg::LOAD_LOSSES_KWH, total_load_losses.re, delta_hrs);
    st.integrate(reg::LOAD_LOSSES_KVARH, total_load_losses.im, delta_hrs);
    st.integrate(reg::NO_LOAD_LOSSES_KWH, total_no_load_losses.re, delta_hrs);
    st.integrate(
        reg::NO_LOAD_LOSSES_KVARH,
        total_no_load_losses.im,
        delta_hrs,
    );
    st.integrate(reg::LINE_LOSSES_KWH, total_line_losses.re, delta_hrs);
    st.integrate(
        reg::LINE_MODE_LINE_LOSS,
        total_line_mode_losses.re,
        delta_hrs,
    );
    st.integrate(
        reg::ZERO_MODE_LINE_LOSS,
        total_zero_mode_losses.re,
        delta_hrs,
    );
    st.integrate(
        reg::THREE_PHASE_LINE_LOSS,
        total_3phase_losses.re,
        delta_hrs,
    );
    st.integrate(reg::ONE_PHASE_LINE_LOSS, total_1phase_losses.re, delta_hrs);
    st.integrate(
        reg::TRANSFORMER_LOSSES_KWH,
        total_transformer_losses.re,
        delta_hrs,
    );
    for i in 0..NUM_EM_VBASE {
        st.integrate(reg::VBASE_START + 1 + i, vbase_total_losses[i], delta_hrs);
        st.integrate(
            reg::VBASE_START + 1 + NUM_EM_VBASE + i,
            vbase_line_losses[i],
            delta_hrs,
        );
        st.integrate(
            reg::VBASE_START + 1 + 2 * NUM_EM_VBASE + i,
            vbase_load_losses[i],
            delta_hrs,
        );
        st.integrate(
            reg::VBASE_START + 1 + 3 * NUM_EM_VBASE + i,
            vbase_no_load_losses[i],
            delta_hrs,
        );
        st.integrate(
            reg::VBASE_START + 1 + 4 * NUM_EM_VBASE + i,
            vbase_load[i],
            delta_hrs,
        );
    }

    st.integrate(reg::ZONE_KWH, total_zone_kw, delta_hrs);
    st.integrate(reg::ZONE_KVARH, total_zone_kvar, delta_hrs);
    st.integrate(reg::GEN_KWH, total_gen_kw, delta_hrs);
    st.integrate(reg::GEN_KVARH, total_gen_kvar, delta_hrs);
    let gen_kva = (total_gen_kvar.powi(2) + total_gen_kw.powi(2)).sqrt();
    let load_kva = (total_zone_kvar.powi(2) + total_zone_kw.powi(2)).sqrt();

    // ---- Drag-hand maxima ------------------------------------------------
    st.set_drag(reg::LOSSES_MAX_KW, total_losses.re.abs());
    st.set_drag(reg::LOSSES_MAX_KVAR, total_losses.im.abs());
    st.set_drag(reg::MAX_LOAD_LOSSES, total_load_losses.re.abs());
    st.set_drag(reg::MAX_NO_LOAD_LOSSES, total_no_load_losses.re.abs());
    st.set_drag(reg::ZONE_MAX_KW, total_zone_kw);
    st.set_drag(reg::ZONE_MAX_KVA, load_kva);
    st.set_drag(reg::GEN_MAX_KW, total_gen_kw);
    st.set_drag(reg::GEN_MAX_KVA, gen_kva);

    // ---- Overload energy -------------------------------------------------
    let zone_kw = if st.local_only {
        s_local.re
    } else {
        total_zone_kw
    };
    let mut s_local_kva = s_local_kva;
    if st.max_zone_kva_norm > 0.0 {
        if s_local_kva == 0.0 {
            s_local_kva = st.max_zone_kva_norm;
        }
        st.integrate(
            reg::OVERLOAD_KWH_NORM,
            (zone_kw * (1.0 - st.max_zone_kva_norm / s_local_kva)).max(0.0),
            delta_hrs,
        );
    } else {
        st.integrate(reg::OVERLOAD_KWH_NORM, max_excess_kw_norm, delta_hrs);
    }
    if st.max_zone_kva_emerg > 0.0 {
        if s_local_kva == 0.0 {
            s_local_kva = st.max_zone_kva_emerg;
        }
        st.integrate(
            reg::OVERLOAD_KWH_EMERG,
            (zone_kw * (1.0 - st.max_zone_kva_emerg / s_local_kva)).max(0.0),
            delta_hrs,
        );
    } else {
        st.integrate(reg::OVERLOAD_KWH_EMERG, max_excess_kw_emerg, delta_hrs);
    }

    st.first_sample_after_reset = false;
    // The `SaveDemandInterval → WriteDemandIntervalData` tail runs in
    // `take_sample_all` (it needs the class DI state on the circuit, disjoint
    // from the meter borrow here).

    meter_mut(store, meter_ref).end_take_sample(tree, st);
}

/// Pascal `TEnergyMeterObj.Accumulate_Load` (l.2208): add the load's terminal-1
/// power to the zone totals and its EEN/UE contribution; returns the load kW.
#[allow(clippy::too_many_arguments)]
fn accumulate_load(
    load: &mut Load,
    sys: &SysCtx,
    node_v: &[Complex64],
    excess_flag: bool,
    norm_min: f64,
    emerg_min: f64,
    total_zone_kw: &mut f64,
    total_zone_kvar: &mut f64,
    total_load_een: &mut f64,
    total_load_ue: &mut f64,
) -> f64 {
    let s_load = load.terminal_power(sys, node_v, 1) * 0.001;
    let kw_load = s_load.re;
    *total_zone_kw += kw_load;
    *total_zone_kvar += s_load.im;

    let exceeds = load.exceeds_normal(sys, node_v, norm_min, emerg_min);
    let unserved = load.unserved(sys, node_v, norm_min, emerg_min);
    let (load_een, load_ue) = if excess_flag {
        (
            if exceeds {
                kw_load * load.een_factor
            } else {
                0.0
            },
            if unserved {
                kw_load * load.ue_factor
            } else {
                0.0
            },
        )
    } else {
        (
            if exceeds { kw_load } else { 0.0 },
            if unserved { kw_load } else { 0.0 },
        )
    };
    *total_load_een += load_een;
    *total_load_ue += load_ue;
    kw_load
}

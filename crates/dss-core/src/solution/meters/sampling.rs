//! EnergyMeter sampling and load allocation — Pascal `TEnergyMeter.SampleAll` /
//! `TEnergyMeterObj.TakeSample` (EnergyMeter.pas l.900 / l.1289) and
//! `TExecHelper.DoAllocateLoadsCmd`. The meter's branch tree and registers are
//! moved out of the meter object for the walk (the store keeps the meter
//! borrowed), the zone's PD/PC elements are read/mutated through the store, and
//! the registers are written back at the end.

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::meter::energymeter::{EnergyMeter, NUM_EM_VBASE, reg};
use crate::elements::meter::sensor::Sensor;
use crate::elements::pc::generator::Generator;
use crate::elements::pc::load::Load;
use crate::elements::pd::line::Line;
use crate::elements::pd::transformer::Transformer;
use crate::elements::traits::{CktElement, ElemRef, ElemStore, SysCtx};
use crate::solution::SolveEnv;
use crate::solution::solution::{solve, sys_ctx};

use super::downcast_meter;

// ---------------------------------------------------------------------------
// TakeSample — Pascal `TEnergyMeter.SampleAll` / `TEnergyMeterObj.TakeSample`
// (EnergyMeter.pas l.900 / l.1289). The meter's branch tree and registers are
// moved out of the meter object for the walk (the store keeps the meter
// borrowed), the zone's PD/PC elements are read/mutated through the store, and
// the registers are written back at the end.
// ---------------------------------------------------------------------------

/// Pascal `TEnergyMeter.ResetAll` (l.851): reset every meter's registers.
/// (Demand-interval files are Phase 8; the `SystemMeter` core and the
/// Generator/Storage/PVSystem `ResetRegistersAll` calls are WP6.8 / later.)
pub(crate) fn reset_all_meters(ckt: &mut Circuit, store: &mut dyn ElemStore) {
    let meters = ckt.energy_meters.clone();
    for meter_ref in meters {
        downcast_meter(store, meter_ref).reset_registers();
    }
}

/// Pascal `TEnergyMeter.SampleAll` (l.900): sample every enabled meter.
pub(crate) fn take_sample_all(ckt: &mut Circuit, store: &mut dyn ElemStore, sys: &SysCtx) {
    let meters = ckt.energy_meters.clone();
    for meter_ref in meters {
        let enabled = store
            .obj(meter_ref)
            .as_any()
            .downcast_ref::<EnergyMeter>()
            .expect("energy_meters holds EnergyMeter objects")
            .enabled();
        if enabled {
            take_sample_one(meter_ref, ckt, store, sys);
        }
    }
}

/// Pascal `TExecHelper.DoAllocateLoadsCmd` allocation loop (ExecHelper.pas
/// l.2605). The caller has already forced `LoadMultiplier = 1.0`. Solve a guess
/// from the present factors, then iterate: recompute every meter/sensor
/// allocation factor, run each meter's zone load adjustment, and re-solve.
pub(crate) fn allocate_loads(ckt: &mut Circuit, env: &mut SolveEnv, max_iters: usize) {
    let _ = solve(ckt, env); // guess based on present allocation factors
    for _ in 0..max_iters {
        let sys = sys_ctx(ckt);
        calc_allocation_factors_all(ckt, env.store, &sys);
        allocate_load_all(ckt, env.store);
        let _ = solve(ckt, env); // update the solution
    }
}

/// `CalcAllocationFactors` over every EnergyMeter then every Sensor (Pascal
/// order). Each reads its metered element's currents vs the measured peak.
fn calc_allocation_factors_all(ckt: &Circuit, store: &mut dyn ElemStore, sys: &SysCtx) {
    let node_v = &ckt.solution.node_v;
    for meter_ref in ckt.energy_meters.clone() {
        let Some(mr) = downcast_meter(store, meter_ref).metered_element() else {
            continue;
        };
        let (meter_obj, metered_obj) = store.pair_mut(meter_ref, mr);
        let ce = metered_obj
            .as_ckt_element_mut()
            .expect("metered element is a circuit element");
        meter_obj
            .as_any_mut()
            .downcast_mut::<EnergyMeter>()
            .expect("energy_meters holds EnergyMeter objects")
            .med
            .calc_allocation_factors(ce, sys, node_v);
    }
    for sensor_ref in ckt.sensors.clone() {
        let metered = store
            .obj(sensor_ref)
            .as_any()
            .downcast_ref::<Sensor>()
            .expect("sensors holds Sensor objects")
            .metered_element();
        let Some(mr) = metered else { continue };
        let (sensor_obj, metered_obj) = store.pair_mut(sensor_ref, mr);
        let ce = metered_obj
            .as_ckt_element_mut()
            .expect("metered element is a circuit element");
        sensor_obj
            .as_any_mut()
            .downcast_mut::<Sensor>()
            .expect("sensors holds Sensor objects")
            .med
            .calc_allocation_factors(ce, sys, node_v);
    }
}

/// `AllocateLoad` over every EnergyMeter zone.
fn allocate_load_all(ckt: &Circuit, store: &mut dyn ElemStore) {
    for meter_ref in ckt.energy_meters.clone() {
        allocate_load_for_meter(meter_ref, ckt, store);
    }
}

/// Pascal `TEnergyMeterObj.AllocateLoad` (EnergyMeter.pas l.2147): walk the
/// meter's zone loads and scale each by its upstream sensor's allocation factor
/// (single-phase loads use the connected-phase factor; poly-phase loads use the
/// average factor). The sensor may be a Sensor object **or** an EnergyMeter.
fn allocate_load_for_meter(meter_ref: ElemRef, ckt: &Circuit, store: &mut dyn ElemStore) {
    // Pascal walks `BranchList` (`First`/`GoForward`) and, per branch, its shunt
    // objects (`FirstObject`/`NextObject`), filtering to `LOAD_ELEMENT`. The
    // meter's `load_list` is exactly that set (only Loads are pushed during the
    // zone build, in the same branch-tree order) and the per-load scaling is
    // independent, so iterating it is equivalent — no other shunt type responds.
    let loads = downcast_meter(store, meter_ref).load_list().to_vec();
    for load_ref in loads {
        let (nphases, sensor_ref, alloc_factor, node_ref0) = {
            let load = store
                .obj(load_ref)
                .as_any()
                .downcast_ref::<Load>()
                .expect("load_list holds Load objects");
            (
                load.cd.nphases,
                load.cd.sensor_obj,
                load.allocation_factor(),
                load.cd.node_ref.first().copied().unwrap_or(0),
            )
        };
        // Pascal dereferences `LoadElem.SensorObj` unconditionally; every zone
        // load is given a sensor (the meter, or an upstream Sensor) by the zone
        // build, so this is never NIL there. We guard defensively rather than
        // panic if the back-pointer is somehow unset.
        let Some(sref) = sensor_ref else { continue };
        let Some((s_nphases, avg, phs)) = sensor_alloc_data(store, sref) else {
            continue;
        };
        let new_factor = if nphases == 1 {
            // Connected phase from NodeRef[1] → MapNodeToBus → NodeNum (1..3).
            let connected_phase = ckt
                .map_node_to_bus
                .get(node_ref0)
                .map(|nb| nb.node_num)
                .unwrap_or(0);
            if connected_phase > 0 && connected_phase < 4 {
                // Pascal indexes `PhsAllocationFactor[ConnectedPhase]` directly
                // (1-based → `connected_phase - 1` here). A connected phase past
                // the sensor's phase count is a malformed circuit (e.g. a 3-phase
                // node on a 2-phase sensor) where Pascal reads past the array; we
                // fall back to the no-change factor 1.0 instead.
                let f = if s_nphases == 1 {
                    phs.first().copied().unwrap_or(1.0)
                } else {
                    phs.get((connected_phase - 1) as usize)
                        .copied()
                        .unwrap_or(1.0)
                };
                Some(alloc_factor * f)
            } else {
                None // Pascal leaves the factor unchanged outside phases 1..3
            }
        } else {
            Some(alloc_factor * avg)
        };
        if let Some(nf) = new_factor {
            store
                .obj_mut(load_ref)
                .as_any_mut()
                .downcast_mut::<Load>()
                .expect("load_list holds Load objects")
                .set_allocation_factor(nf);
        }
    }
}

/// The (nphases, AvgAllocFactor, PhsAllocationFactor) of a metering device,
/// which may be a [`Sensor`] or an [`EnergyMeter`] (both embed `MeterElementData`).
fn sensor_alloc_data(store: &dyn ElemStore, r: ElemRef) -> Option<(usize, f64, Vec<f64>)> {
    let obj = store.obj(r);
    if let Some(s) = obj.as_any().downcast_ref::<Sensor>() {
        Some((
            s.med.cd.nphases,
            s.med.avg_alloc_factor,
            s.med.phs_allocation_factor.clone(),
        ))
    } else {
        obj.as_any().downcast_ref::<EnergyMeter>().map(|em| {
            (
                em.med.cd.nphases,
                em.med.avg_alloc_factor,
                em.med.phs_allocation_factor.clone(),
            )
        })
    }
}

/// PD-element kind probe used by the loss split.
fn branch_kind(store: &dyn ElemStore, r: ElemRef) -> (bool, bool, usize) {
    let any = store.obj(r).as_any();
    let is_line = any.downcast_ref::<Line>().is_some();
    let is_xfmr = any.downcast_ref::<Transformer>().is_some();
    let nphases = store.ckt_elem(r).cd().nphases;
    (is_line, is_xfmr, nphases)
}

/// Pascal `TEnergyMeterObj.TakeSample` (l.1289).
fn take_sample_one(meter_ref: ElemRef, ckt: &Circuit, store: &mut dyn ElemStore, sys: &SysCtx) {
    let trapezoidal = ckt.trapezoidal_integration;
    let node_v = &ckt.solution.node_v;
    let delta_hrs = ckt.solution.interval_hrs;
    let norm_min = ckt.normal_min_volts;
    let emerg_min = ckt.emerg_min_volts;

    // CheckBranchList: exit if the zone was never built. Take the tree and the
    // register accumulators out of the meter for the walk.
    let Some((mut tree, mut st)) = downcast_meter(store, meter_ref).begin_take_sample(trapezoidal)
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
                if let Some(load) = store.obj_mut(*pc).as_any_mut().downcast_mut::<Load>() {
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
        let (volt_base_index, shunts) = {
            let n = tree.present_node();
            (n.volt_base_index, n.shunts.clone())
        };
        let vbi = volt_base_index; // 1-based; 0 = none

        for pc in &shunts {
            let is_load = store.obj(*pc).as_any().downcast_ref::<Load>().is_some();
            let is_gen = store
                .obj(*pc)
                .as_any()
                .downcast_ref::<Generator>()
                .is_some();
            if is_load && !st.local_only {
                let load = store
                    .obj_mut(*pc)
                    .as_any_mut()
                    .downcast_mut::<Load>()
                    .expect("checked is_load");
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
                let gen_obj = store
                    .obj_mut(*pc)
                    .as_any_mut()
                    .downcast_mut::<Generator>()
                    .expect("checked is_gen");
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
            // FPhaseVoltageReport is off by default and only feeds the demand-
            // interval phase-voltage files (Phase 8); not accumulated here.
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
    // SaveDemandInterval (WriteDemandIntervalData) is Phase 8.

    downcast_meter(store, meter_ref).end_take_sample(tree, st);
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

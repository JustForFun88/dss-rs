//! Trait impls: `CktElement` (reliability/losses, the Stage-B-deferred YPrim
//! build) and `DssObject` (typed property getters/setters, the per-winding
//! struct arrays, `PropertySideEffects`, `MakeLike`).

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::traits::{CktElement, ReliabilityData, SysCtx};
use crate::obj::base::{DssObjData, DssObject};
use crate::support::cmatrix::CMatrix;

use super::{AutoTrans, prop, xsc_size};

impl CktElement for AutoTrans {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    fn recalc_element_data(&mut self, _sys: &SysCtx) {
        self.recalc();
    }

    /// Pascal `TPDElement.CalcFltRate` (base): `Faultrate · pctperm · 0.01`.
    fn reliability_data(&self) -> ReliabilityData {
        ReliabilityData {
            branch_flt_rate: self.fault_rate * self.pct_perm * 0.01,
            hrs_to_repair: self.hrs_to_repair,
            miles_this_line: 0.0,
        }
    }

    fn norm_amps(&self) -> f64 {
        self.norm_amps
    }
    fn emerg_amps(&self) -> f64 {
        self.emerg_amps
    }

    /// Pascal `TAutoTransObj.GetLosses` (`AutoTrans.pas:1674`): no-load losses are
    /// the power into `Yprim_Shunt` from each terminal; load losses are the
    /// remainder of the total. Identical to the Transformer split.
    fn get_losses_split(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> (Complex64, Complex64, Complex64) {
        if !self.cd.enabled || self.cd.node_ref.is_empty() {
            return (Complex64::ZERO, Complex64::ZERO, Complex64::ZERO);
        }
        let total = self.losses(sys, node_v); // side effect: computes Iterminal
        let yorder = self.cd.yorder;
        self.cd.compute_vterminal(node_v);
        let mut no_load = Complex64::ZERO;
        if let Some(yshunt) = &self.cd.yprim_shunt {
            let mut temp = vec![Complex64::ZERO; yorder];
            yshunt.mv_mult(&mut temp, &self.cd.vterminal);
            for (v, t) in self.cd.vterminal.iter().zip(temp.iter()).take(yorder) {
                no_load += v * t.conj();
            }
        }
        let load = total - no_load;
        (total, load, no_load)
    }

    /// Pascal `TAutoTransObj.CalcYPrim`.
    ///
    /// **WPG.15 Stage A — NOT_PORTED (owner: WPG.15 Stage B).** The auto solve
    /// path (`SetNodeRef` node aliasing, `BuildYPrimComponent`, the GIC-branch
    /// selection and the `GetCurrents` series/common fold) is not yet wired.
    /// Build a zero YPrim so the Y assembly and any current reads stay
    /// well-defined, and push a loud error so a solve of an AutoTrans-bearing
    /// circuit aborts (the ymatrix builder lifts queued `CalcYPrim` errors to
    /// `SolutionAbort`) rather than silently producing wrong results.
    fn calc_yprim(&mut self, sys: &SysCtx) {
        let yorder = self.cd.yorder;
        self.cd.yprim_freq = sys.frequency;
        self.cd.yprim_series = Some(CMatrix::new(yorder));
        self.cd.yprim_shunt = Some(CMatrix::new(yorder));
        self.cd.yprim = Some(CMatrix::new(yorder));
        self.cd.obj.push_error(format!(
            "AutoTrans.{}: the autotransformer solve path is NOT_PORTED \
             (WPG.15 Stage B); YPrim not built.",
            self.cd.obj.name()
        ));
        self.cd.yprim_invalid = false;
    }
}

impl DssObject for AutoTrans {
    fn data(&self) -> &DssObjData {
        &self.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.cd.obj
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn as_ckt_element(&self) -> Option<&dyn CktElement> {
        Some(self)
    }
    fn as_ckt_element_mut(&mut self) -> Option<&mut dyn CktElement> {
        Some(self)
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases as i32,
            WINDINGS => self.num_windings,
            WDG => self.active_winding,
            CONN => self.windings[self.aw()].connection,
            NUMTAPS => self.windings[self.aw()].num_taps,
            LEADLAG => self.hv_leads_lv as i32,
            CORE => self.core_type,
            _ => unreachable!("AutoTrans has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases = value.max(0) as usize,
            WINDINGS => self.num_windings = value,
            WDG => self.active_winding = value,
            CONN => {
                let w = self.aw();
                self.windings[w].connection = value;
            }
            NUMTAPS => {
                let w = self.aw();
                self.windings[w].num_taps = value;
            }
            LEADLAG => self.hv_leads_lv = value != 0,
            CORE => self.core_type = value,
            _ => unreachable!("AutoTrans has no integer property {idx}"),
        }
    }

    fn get_f64(&self, idx: usize) -> f64 {
        use prop::*;
        let w = self.aw();
        match idx {
            KV => self.windings[w].kvll,
            KVA => self.windings[w].kva,
            TAP => self.windings[w].putap,
            PCTR => self.windings[w].rpu,
            RDCOHMS => self.windings[w].rdcohms,
            MAXTAP => self.windings[w].max_tap,
            MINTAP => self.windings[w].min_tap,
            XHX => self.puxhx,
            XHT => self.puxht,
            XXT => self.puxxt,
            THERMAL => self.thermal_time_const,
            N => self.n_thermal,
            M => self.m_thermal,
            FLRISE => self.flrise,
            HSRISE => self.hsrise,
            PCTLOADLOSS => self.pct_load_loss,
            PCTNOLOADLOSS => self.pct_no_load_loss,
            NORMHKVA => self.norm_max_hkva,
            EMERGHKVA => self.emerg_max_hkva,
            PCTIMAG => self.pct_imag,
            PPM_ANTIFLOAT => self.ppm_float_factor,
            NORMAMPS => self.norm_amps,
            EMERGAMPS => self.emerg_amps,
            FAULTRATE => self.fault_rate,
            PCTPERM => self.pct_perm,
            REPAIR => self.hrs_to_repair,
            BASE_FREQ => self.cd.base_frequency,
            _ => unreachable!("AutoTrans has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        let w = self.aw();
        match idx {
            KV => self.windings[w].kvll = value,
            KVA => self.windings[w].kva = value,
            TAP => self.windings[w].putap = value,
            PCTR => self.windings[w].rpu = value,
            RDCOHMS => self.windings[w].rdcohms = value,
            MAXTAP => self.windings[w].max_tap = value,
            MINTAP => self.windings[w].min_tap = value,
            XHX => self.puxhx = value,
            XHT => self.puxht = value,
            XXT => self.puxxt = value,
            THERMAL => self.thermal_time_const = value,
            N => self.n_thermal = value,
            M => self.m_thermal = value,
            FLRISE => self.flrise = value,
            HSRISE => self.hsrise = value,
            PCTLOADLOSS => self.pct_load_loss = value,
            PCTNOLOADLOSS => self.pct_no_load_loss = value,
            NORMHKVA => self.norm_max_hkva = value,
            EMERGHKVA => self.emerg_max_hkva = value,
            PCTIMAG => self.pct_imag = value,
            PPM_ANTIFLOAT => self.ppm_float_factor = value,
            NORMAMPS => self.norm_amps = value,
            EMERGAMPS => self.emerg_amps = value,
            FAULTRATE => self.fault_rate = value,
            PCTPERM => self.pct_perm = value,
            REPAIR => self.hrs_to_repair = value,
            BASE_FREQ => self.cd.base_frequency = value,
            _ => unreachable!("AutoTrans has no double property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use prop::*;
        match idx {
            SUB => self.is_substation,
            XRCONST => self.xrconst,
            ENABLED => self.cd.enabled,
            _ => unreachable!("AutoTrans has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use prop::*;
        match idx {
            SUB => self.is_substation = value,
            XRCONST => self.xrconst = value,
            ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("AutoTrans has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use prop::*;
        match idx {
            SUBNAME => self.substation_name.clone(),
            BANK => self.xfmr_bank.clone(),
            WDGCURRENTS => self.winding_currents_result(),
            _ => unreachable!("AutoTrans has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        use prop::*;
        match idx {
            SUBNAME => self.substation_name = value,
            BANK => self.xfmr_bank = value,
            // WdgCurrents is a read-only result property (silent ignore).
            WDGCURRENTS => {}
            _ => unreachable!("AutoTrans has no string property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        match idx {
            prop::XSCARRAY => Some(&self.xsc),
            _ => unreachable!("AutoTrans has no array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        match idx {
            prop::XSCARRAY => self.xsc = value,
            _ => unreachable!("AutoTrans has no array property {idx}"),
        }
    }

    fn array_size(&self, idx: usize) -> usize {
        match idx {
            prop::XSCARRAY => xsc_size(self.num_windings),
            _ => unreachable!("AutoTrans has no function-sized array {idx}"),
        }
    }

    fn get_struct_f64_array(&self, idx: usize) -> Vec<f64> {
        use prop::*;
        self.windings
            .iter()
            .map(|w| match idx {
                KVS => w.kvll,
                KVAS => w.kva,
                TAPS => w.putap,
                PCTRS => w.rpu,
                _ => unreachable!("AutoTrans has no struct array {idx}"),
            })
            .collect()
    }
    fn set_struct_f64_array(&mut self, idx: usize, values: &[Option<f64>]) {
        use prop::*;
        for (w, v) in self.windings.iter_mut().zip(values) {
            let Some(v) = v else { continue };
            match idx {
                KVS => w.kvll = *v,
                KVAS => w.kva = *v,
                TAPS => w.putap = *v,
                PCTRS => w.rpu = *v,
                _ => unreachable!("AutoTrans has no struct array {idx}"),
            }
        }
        self.active_winding = self.num_windings;
    }

    fn get_struct_i32_array(&self, idx: usize) -> Vec<i32> {
        match idx {
            prop::CONNS => self.windings.iter().map(|w| w.connection).collect(),
            _ => unreachable!("AutoTrans has no struct enum array {idx}"),
        }
    }
    fn set_struct_i32_array(&mut self, idx: usize, values: &[i32]) {
        match idx {
            prop::CONNS => {
                for (w, v) in self.windings.iter_mut().zip(values) {
                    w.connection = *v;
                }
            }
            _ => unreachable!("AutoTrans has no struct enum array {idx}"),
        }
        self.active_winding = self.num_windings;
    }

    fn set_active_struct_bus(&mut self, value: &str) {
        let t = self.aw() + 1;
        // Pascal `SetBus(iwdg, s)` override (winding-2 neutral defaulting,
        // `AutoTrans.pas:721`) lands in WPG.15 Stage B; the rewrite branch fires
        // only on an explicit non-zero neutral on winding 2 — no corpus deck
        // hits it, and `process_bus_defs` already grounds the extra conductors.
        self.cd.set_bus(t, value);
    }
    fn get_active_struct_bus(&self) -> String {
        self.cd.get_bus(self.aw() + 1).to_string()
    }
    fn set_struct_buses(&mut self, values: &[Option<String>]) {
        for (i, v) in values.iter().enumerate() {
            if let Some(v) = v {
                self.cd.set_bus(i + 1, v);
            }
        }
        self.active_winding = self.num_windings;
    }
    fn get_struct_buses(&self) -> Vec<String> {
        (1..=self.num_windings.max(0) as usize)
            .map(|t| self.cd.get_bus(t).to_string())
            .collect()
    }

    /// Pascal `TAutoTransObj.PropertySideEffects` (`AutoTrans.pas:574`).
    fn side_effects(&mut self, idx: usize, prev_int: i32) {
        use prop::*;
        match idx {
            PHASES => {
                if self.cd.nphases as i32 != prev_int {
                    let nc = 2 * self.cd.nphases;
                    self.cd.set_nconds(nc);
                }
            }
            CONN => {
                // Force winding 1 = Series, winding 2 = Wye regardless of input.
                match self.active_winding {
                    1 => self.windings[0].connection = 2,
                    2 => self.windings[1].connection = 0,
                    _ => {}
                }
                self.cd.yorder = self.cd.nconds * self.cd.nterms;
            }
            CONNS => {
                for i in 1..=self.num_windings.max(0) as usize {
                    match i {
                        1 => self.windings[0].connection = 2,
                        2 => self.windings[1].connection = 0,
                        _ => {}
                    }
                }
                self.cd.yorder = self.cd.nconds * self.cd.nterms;
            }
            WINDINGS => self.realloc_windings(prev_int),
            KVA => {
                if self.active_winding == 1 {
                    let k = self.windings[0].kva;
                    for w in self.windings.iter_mut().skip(1) {
                        w.kva = k;
                    }
                    self.norm_max_hkva = 1.1 * k;
                    self.emerg_max_hkva = 1.5 * k;
                } else if self.num_windings == 2 {
                    self.windings[0].kva = self.windings[1].kva;
                }
            }
            PCTR | PCTRS => {
                if self.windings.len() >= 2 {
                    self.pct_load_loss = (self.windings[0].rpu + self.windings[1].rpu) * 100.0;
                }
            }
            RDCOHMS => {
                let w = self.aw();
                self.windings[w].rdc_specified = true;
            }
            KVAS => {
                let k = self.windings[0].kva;
                self.norm_max_hkva = 1.1 * k;
                self.emerg_max_hkva = 1.5 * k;
            }
            XHX | XHT | XXT => {
                // SpecSet1 {XHX,XHT,XXT} vs SpecSet2 {XSCArray}.
                self.cd.obj.clear_seq(XSCARRAY);
                self.xhx_changed = true;
            }
            PCTLOADLOSS => {
                // Assume load loss split evenly between windings 1 and 2.
                if self.windings.len() >= 2 {
                    let r = self.pct_load_loss / 2.0 / 100.0;
                    self.windings[0].rpu = r;
                    self.windings[1].rpu = r;
                }
            }
            XSCARRAY => {
                for p in [XHX, XHT, XXT] {
                    self.cd.obj.clear_seq(p);
                }
            }
            _ => {}
        }

        // YPrim invalidation on anything that changes impedance values.
        if matches!(
            idx,
            CONN | KV
                | KVA
                | TAP
                | PCTR
                | RDCOHMS
                | CORE
                | BUSES
                | CONNS
                | KVS
                | KVAS
                | TAPS
                | XHX
                | XHT
                | XXT
                | PCTLOADLOSS
                | PCTNOLOADLOSS
                | PCTIMAG
                | PPM_ANTIFLOAT
                | PCTRS
                | XSCARRAY
        ) {
            self.cd.yprim_invalid = true;
        }
    }

    /// Pascal base `EndEdit` → `RecalcElementData`.
    fn end_edit(&mut self) {
        self.recalc();
    }

    /// Pascal `TAutoTransObj.MakeLike` (`AutoTrans.pas:773`).
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(o) = other.as_any().downcast_ref::<AutoTrans>() else {
            return;
        };
        self.cd.make_like_base(&o.cd);
        self.cd.nphases = o.cd.nphases;
        self.set_num_windings(o.num_windings);
        let nc = 2 * self.cd.nphases; // forces terminal/conductor reallocation
        self.cd.set_nconds(nc);
        self.cd.yprim_invalid = true;

        self.windings.clone_from(&o.windings);
        self.set_term_ref();

        self.puxhx = o.puxhx;
        self.puxht = o.puxht;
        self.puxxt = o.puxxt;
        let n = xsc_size(self.num_windings);
        for i in 0..n {
            self.xsc[i] = o.xsc[i];
        }
        self.zb = o.zb.clone();
        self.y_1volt = o.y_1volt.clone();
        self.y_term = o.y_term.clone();
        self.y_1volt_nl = o.y_1volt_nl.clone();
        self.y_term_nl = o.y_term_nl.clone();

        self.thermal_time_const = o.thermal_time_const;
        self.n_thermal = o.n_thermal;
        self.m_thermal = o.m_thermal;
        self.flrise = o.flrise;
        self.hsrise = o.hsrise;
        self.pct_load_loss = o.pct_load_loss;
        self.pct_no_load_loss = o.pct_no_load_loss;
        self.norm_max_hkva = o.norm_max_hkva;
        self.emerg_max_hkva = o.emerg_max_hkva;
        self.xrconst = o.xrconst;

        self.xfmr_bank = o.xfmr_bank.clone();
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

//! Trait impls: `CktElement` (reliability/losses, Yprim build), the
//! `ControlledTransformer` RegControl surface, and `DssObject` (typed property
//! getters/setters, the per-winding struct arrays, `MakeLike`).

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::general::xfmr_code::XfmrCodeObj;
use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, ElemRef, ReliabilityData, SysCtx};
use crate::obj::base::{DssObjData, DssObject};
use crate::support::cmatrix::CMatrix;
use crate::util::sqrt3;

use super::{ControlledTransformer, CoreType, Transformer, prop, xsc_size};

impl CktElement for Transformer {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    fn present_tap(&self, terminal: usize) -> Option<f64> {
        Some(Transformer::present_tap(self, terminal))
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
    fn num_amp_ratings(&self) -> i32 {
        self.num_amp_ratings
    }
    fn amp_ratings(&self) -> &[f64] {
        &self.amp_ratings
    }

    /// Pascal `TTransfObj.GetLosses` (Transformer.pas l.1635): no-load losses
    /// are the power into `Yprim_Shunt` from each terminal; load losses are the
    /// remainder of the total.
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

    /// Pascal `TTransfObj.CalcYPrim`: stamp `Y_Term`/`Y_Term_NL` into the
    /// series/shunt YPrim via `TermRef`, add neutral branches, then apply the
    /// open-conductor corrections.
    fn calc_yprim(&mut self, sys: &SysCtx) {
        let yorder = self.cd.yorder;
        let nw = self.num_windings.max(0) as usize;
        let np = self.cd.nphases;

        let mut yp_series = CMatrix::new(yorder);
        let mut yp_shunt = CMatrix::new(yorder);
        let mut yprim = CMatrix::new(yorder);

        self.cd.yprim_freq = sys.frequency;
        let freq_mult = sys.frequency / self.cd.base_frequency;
        if freq_mult != self.y_terminal_freqmult {
            self.calc_y_terminal(freq_mult);
        }

        Self::build_yprim_component(&mut yp_series, &self.y_term, &self.term_ref, nw, np);
        Self::build_yprim_component(&mut yp_shunt, &self.y_term_nl, &self.term_ref, nw, np);
        Self::add_neutral_to_y(
            &mut yp_series,
            &self.windings,
            self.cd.nconds,
            self.ppm_float_factor,
            freq_mult,
        );

        yprim.copy_from(&yp_series);
        yprim.add_from(&yp_shunt);

        self.cd.yprim_series = Some(yp_series);
        self.cd.yprim_shunt = Some(yp_shunt);
        self.cd.yprim = Some(yprim);

        self.cd.apply_yprim_open_conductor_calcs();
        self.cd.yprim_invalid = false;
    }

    /// Pascal `TTransfObj.MakePosSequence` (Transformer.pas:1685-1752). Convert
    /// the default 3-phase transformer into an equivalent positive-sequence
    /// single-phase transformer: all windings wye, buses stripped, per-winding
    /// kV = kVLL/√3 (unless the winding is already single-phase *and* wye), and
    /// the kVA / NormHkVA / EmergHkVA divided by `FNphases`.
    ///
    /// For a 1- or 2-phase transformer it first checks every winding sits on
    /// phase 1 (`OnPhase1`, via the parsed terminal node numbers): if any
    /// winding is off phase 1 the transformer is disabled and left untouched
    /// (no `inherited`, dotted bus names preserved).
    fn make_pos_sequence(&mut self, ctx: &PosSeqCtx) -> PosSeqPlan {
        use prop::*;

        let nw = self.num_windings.max(0) as usize;
        let nphases = self.cd.nphases;

        // First, determine if we can convert this one. For 1- or 2-phase, any
        // winding not connected to phase one → disable and bail (no inherited).
        if nphases == 1 || nphases == 2 {
            for iw in 1..=nw {
                let nodes = ctx.terminal_nodes.get(iw - 1);
                let on_phase1 = match nodes {
                    None => true, // no parsed nodes (N = 0) → treated as phase 1
                    Some(list) if list.is_empty() => true,
                    Some(list) => list.contains(&1),
                };
                if !on_phase1 {
                    // We won't use this one.
                    return PosSeqPlan {
                        actions: vec![PosSeqAction::Disable],
                        run_base: false,
                    };
                }
            }
        }

        // Construct the positive-sequence definition: all wye, buses as-is,
        // kV = kVLL/√3 unless the winding is single-phase wye, kVA/NormHkVA/
        // EmergHkVA per phase.
        let new_conns: Vec<i32> = vec![0; nw];
        let new_buses: Vec<String> = (1..=nw).map(|i| self.cd.get_bus(i).to_string()).collect();
        let new_kvs: Vec<Option<f64>> = self
            .windings
            .iter()
            .take(nw)
            .map(|w| {
                if nphases > 1 || w.connection != 0 {
                    Some(w.kvll / sqrt3())
                } else {
                    Some(w.kvll)
                }
            })
            .collect();
        let new_kvas: Vec<Option<f64>> = self
            .windings
            .iter()
            .take(nw)
            .map(|w| Some(w.kva / nphases as f64))
            .collect();
        let new_norm = self.norm_max_hkva / nphases as f64;
        let new_emerg = self.emerg_max_hkva / nphases as f64;

        let actions = vec![
            PosSeqAction::BeginEdit,
            PosSeqAction::SetI32(PHASES, 1),
            PosSeqAction::SetStructI32s(CONNS, new_conns),
            PosSeqAction::SetStructBuses(new_buses),
            PosSeqAction::SetStructF64s(KVS, new_kvs),
            PosSeqAction::SetStructF64s(KVAS, new_kvas),
            PosSeqAction::SetF64(NORMHKVA, new_norm),
            PosSeqAction::SetF64(EMERGHKVA, new_emerg),
            PosSeqAction::EndEdit,
        ];
        PosSeqPlan::with_actions(actions)
    }
}

impl ControlledTransformer for Transformer {
    fn name(&self) -> &str {
        self.cd.obj.name()
    }
    fn full_name(&self) -> String {
        format!("Transformer.{}", self.cd.obj.name())
    }
    fn n_phases(&self) -> usize {
        self.cd.nphases
    }
    fn n_conds(&self) -> usize {
        self.cd.nconds
    }
    fn y_order(&self) -> usize {
        self.cd.yorder
    }
    fn wdg_connection(&self, term: usize) -> i32 {
        Transformer::wdg_connection(self, term)
    }
    fn rotate_phases(&self, iphs: usize) -> usize {
        self.rotate_phases_1based(iphs)
    }
    fn base_voltage(&self, term: usize) -> f64 {
        Transformer::base_voltage(self, term)
    }
    fn present_tap(&self, w: usize) -> f64 {
        Transformer::present_tap(self, w)
    }
    fn min_tap(&self, w: usize) -> f64 {
        self.winding_tap_data(w).2
    }
    fn max_tap(&self, w: usize) -> f64 {
        self.winding_tap_data(w).1
    }
    fn tap_increment(&self, w: usize) -> f64 {
        self.winding_tap_data(w).3
    }
    fn set_present_tap(&mut self, w: usize, value: f64) -> bool {
        Transformer::set_present_tap(self, w, value)
    }
    fn power_into_re(&mut self, term: usize, node_v: &[Complex64], sys: &SysCtx) -> f64 {
        self.power_into(term, node_v, sys).re
    }
    fn winding_voltages(&mut self, term: usize, node_v: &[Complex64], vbuffer: &mut [Complex64]) {
        self.get_winding_voltages(term, node_v, vbuffer);
    }
    fn terminal_currents(&mut self, node_v: &[Complex64], sys: &SysCtx, cbuffer: &mut [Complex64]) {
        self.get_currents(sys, node_v, cbuffer);
    }
}

impl DssObject for Transformer {
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
            CORE => self.core_type.ordinal(),
            SEASONS => self.num_amp_ratings,
            BHPOINTS => self.bh_points,
            _ => unreachable!("Transformer has no integer property {idx}"),
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
            CORE => self.core_type = CoreType::from_ordinal(value).unwrap_or(self.core_type),
            SEASONS => self.num_amp_ratings = value,
            BHPOINTS => self.bh_points = value,
            _ => unreachable!("Transformer has no integer property {idx}"),
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
            RNEUT => self.windings[w].rneut,
            XNEUT => self.windings[w].xneut,
            MAXTAP => self.windings[w].max_tap,
            MINTAP => self.windings[w].min_tap,
            RDCOHMS => self.windings[w].rdcohms,
            XHL | X12 => self.xhl,
            XHT | X13 => self.xht,
            XLT | X23 => self.xlt,
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
            _ => unreachable!("Transformer has no double property {idx}"),
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
            RNEUT => self.windings[w].rneut = value,
            XNEUT => self.windings[w].xneut = value,
            MAXTAP => self.windings[w].max_tap = value,
            MINTAP => self.windings[w].min_tap = value,
            RDCOHMS => self.windings[w].rdcohms = value,
            XHL | X12 => self.xhl = value,
            XHT | X13 => self.xht = value,
            XLT | X23 => self.xlt = value,
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
            _ => unreachable!("Transformer has no double property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use prop::*;
        match idx {
            SUB => self.is_substation,
            XRCONST => self.xrconst,
            ENABLED => self.cd.enabled,
            _ => unreachable!("Transformer has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use prop::*;
        match idx {
            SUB => self.is_substation = value,
            XRCONST => self.xrconst = value,
            ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("Transformer has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use prop::*;
        match idx {
            SUBNAME => self.substation_name.clone(),
            BANK => self.xfmr_bank.clone(),
            XFMRCODE => self.xfmr_code_name.clone(),
            WDGCURRENTS => self.winding_currents_result(),
            _ => unreachable!("Transformer has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        use prop::*;
        match idx {
            SUBNAME => self.substation_name = value,
            BANK => self.xfmr_bank = value,
            // WdgCurrents is a read-only result property (silent ignore).
            WDGCURRENTS => {}
            _ => unreachable!("Transformer has no string property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        match idx {
            prop::XSCARRAY => Some(&self.xsc),
            prop::RATINGS => Some(&self.kva_ratings),
            // Pascal `GetDSSArray` renders a NIL pointer as '' (Utilities.pas:1857);
            // an unallocated (BHpoints=0) BH array is the NIL equivalent.
            prop::BHCURRENT => (!self.bh_current.is_empty()).then_some(&self.bh_current[..]),
            prop::BHFLUX => (!self.bh_flux.is_empty()).then_some(&self.bh_flux[..]),
            _ => unreachable!("Transformer has no array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        match idx {
            prop::XSCARRAY => self.xsc = value,
            prop::RATINGS => self.kva_ratings = value,
            prop::BHCURRENT => self.bh_current = value,
            prop::BHFLUX => self.bh_flux = value,
            _ => unreachable!("Transformer has no array property {idx}"),
        }
    }

    fn array_size(&self, idx: usize) -> usize {
        match idx {
            prop::XSCARRAY => xsc_size(self.num_windings),
            _ => unreachable!("Transformer has no function-sized array {idx}"),
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
                // Per-winding scalars rendered as a JSON array under `ON_ARRAY`
                // (DoubleOnStructArrayProperty; DSSObjectHelper.pas:1014).
                RNEUT => w.rneut,
                XNEUT => w.xneut,
                MAXTAP => w.max_tap,
                MINTAP => w.min_tap,
                RDCOHMS => w.rdcohms,
                _ => unreachable!("Transformer has no struct array {idx}"),
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
                _ => unreachable!("Transformer has no struct array {idx}"),
            }
        }
        self.active_winding = self.num_windings;
    }

    fn get_struct_i32_array(&self, idx: usize) -> Vec<i32> {
        match idx {
            prop::CONNS => self.windings.iter().map(|w| w.connection).collect(),
            // NumTaps rendered as a JSON per-winding array under `ON_ARRAY`
            // (IntegerOnStructArrayProperty; DSSObjectHelper.pas:1054).
            prop::NUMTAPS => self.windings.iter().map(|w| w.num_taps).collect(),
            _ => unreachable!("Transformer has no struct enum array {idx}"),
        }
    }
    fn set_struct_i32_array(&mut self, idx: usize, values: &[i32]) {
        match idx {
            prop::CONNS => {
                for (w, v) in self.windings.iter_mut().zip(values) {
                    w.connection = *v;
                }
            }
            _ => unreachable!("Transformer has no struct enum array {idx}"),
        }
        self.active_winding = self.num_windings;
    }

    fn set_active_struct_bus(&mut self, value: &str) {
        let t = self.aw() + 1;
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

    /// `xfmrcode=`: store the resolved code's name + ElemRef and copy its data
    /// immediately (Pascal `FetchXfmrCode`).
    fn set_object_ref(
        &mut self,
        idx: usize,
        name: String,
        resolved: Option<(ElemRef, &dyn DssObject)>,
    ) {
        match idx {
            prop::XFMRCODE => {
                self.xfmr_code_name = name;
                self.xfmr_code_ref = resolved.map(|(r, _)| r);
                if let Some((_, obj)) = resolved
                    && let Some(code) = obj.as_any().downcast_ref::<XfmrCodeObj>()
                {
                    self.fetch_xfmr_code(code);
                }
            }
            _ => unreachable!("Transformer has no resolved object-ref property {idx}"),
        }
    }

    /// Pascal `TTransfObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, prev_int: i32) {
        use prop::*;
        match idx {
            PHASES => {
                if self.cd.nphases as i32 != prev_int {
                    let nc = self.cd.nphases + 1;
                    self.cd.set_nconds(nc);
                }
            }
            CONN => {
                self.cd.yorder = self.cd.nconds * self.cd.nterms;
                self.cd.yprim_invalid = true;
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
            KVAS => {
                let k = self.windings[0].kva;
                self.norm_max_hkva = 1.1 * k;
                self.emerg_max_hkva = 1.5 * k;
            }
            XHL | XHT | XLT | X12 | X13 | X23 => {
                self.cd.obj.clear_seq(XSCARRAY);
                self.cd.obj.clear_seq(XFMRCODE);
                self.xhl_changed = true;
            }
            PCTLOADLOSS => {
                if self.windings.len() >= 2 {
                    let r = self.pct_load_loss / 2.0 / 100.0;
                    self.windings[0].rpu = r;
                    self.windings[1].rpu = r;
                }
            }
            RDCOHMS => {
                let w = self.aw();
                self.windings[w].rdc_specified = true;
            }
            SEASONS => self
                .kva_ratings
                .resize(self.num_amp_ratings.max(0) as usize, 0.0),
            // r4064 (90962ae8): BHpoints reallocates both BH arrays, zeroed
            // (Pascal FreeMem + AllocMem(SizeOf(Double)*BHPoints)).
            BHPOINTS => {
                let n = self.bh_points.max(0) as usize;
                self.bh_current = vec![0.0; n];
                self.bh_flux = vec![0.0; n];
            }
            XSCARRAY => {
                for p in [XHL, XHT, XLT, X12, X13, X23, XFMRCODE] {
                    self.cd.obj.clear_seq(p);
                }
            }
            _ => {}
        }

        // YPrim invalidation on anything that changes impedance values.
        if matches!(
            idx,
            TAP | TAPS
                | KV
                | KVA
                | PCTR
                | RNEUT
                | XNEUT
                | BUSES
                | CONNS
                | KVS
                | KVAS
                | PCTLOADLOSS
                | PCTNOLOADLOSS
                | PCTIMAG
                | PPM_ANTIFLOAT
                | PCTRS
                | XHL
                | XHT
                | XLT
                | X12
                | X13
                | X23
                | XSCARRAY
        ) {
            self.cd.yprim_invalid = true;
        }
    }

    /// Pascal base `EndEdit` → `RecalcElementData` (Transformer does not
    /// override `EndEdit`, unlike Line).
    fn end_edit(&mut self) {
        self.recalc();
    }

    /// Pascal `TTransfObj.MakeLike`.
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(o) = other.as_any().downcast_ref::<Transformer>() else {
            return;
        };
        self.cd.make_like_base(&o.cd);
        self.cd.nphases = o.cd.nphases;
        self.set_num_windings(o.num_windings);
        let nc = self.cd.nphases + 1;
        self.cd.set_nconds(nc); // forces terminal/conductor reallocation
        self.cd.yprim_invalid = true;

        self.windings.clone_from(&o.windings);
        self.set_term_ref();

        self.xhl = o.xhl;
        self.xht = o.xht;
        self.xlt = o.xlt;
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
        self.xfmr_code_name = o.xfmr_code_name.clone();
        self.xfmr_code_ref = o.xfmr_code_ref;

        self.num_amp_ratings = o.num_amp_ratings;
        self.kva_ratings.clone_from(&o.kva_ratings);

        // r4064 (90962ae8): TControlledTransformerObj.MakeLike copies BHpoints
        // and the two BH arrays.
        self.bh_points = o.bh_points;
        self.bh_current.clone_from(&o.bh_current);
        self.bh_flux.clone_from(&o.bh_flux);
    }

    /// Target side of RegControl's deferred `TapNum` write (Pascal
    /// `Set_TapNum` pokes `tr.PresentTap[w]` directly).
    fn apply_ref_action(&mut self, action: &crate::obj::base::RefAction) {
        match action {
            crate::obj::base::RefAction::SetTransformerTap { winding, tap, .. } => {
                self.set_present_tap(*winding, *tap);
            }
            // `SetSwitchClosed`/`SetConductorsClosed`/`SetOcpDevice`/
            // `SetElementBus` are applied generically by the executive (they act
            // on the CktElement base), never routed here.
            crate::obj::base::RefAction::SetSwitchClosed { .. }
            | crate::obj::base::RefAction::SetConductorsClosed { .. }
            | crate::obj::base::RefAction::SetOcpDevice { .. }
            | crate::obj::base::RefAction::SetElementBus { .. } => {}
        }
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

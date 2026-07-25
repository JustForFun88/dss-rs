//! Trait impls: `CktElement` (reliability/losses, YPrim build), the
//! `ControlledTransformer` RegControl surface, and `DssObject` (typed property
//! getters/setters, the per-winding struct arrays, `PropertySideEffects`,
//! `MakeLike`).

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::pd::transformer::{ControlledTransformer, CoreType};
use crate::elements::pd::winding::Connection;
use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, ReliabilityData, SysCtx};
use crate::obj::base::{DssObjData, DssObject};
use crate::support::cmatrix::CMatrix;
use crate::util::sqrt3;

use super::{AutoTrans, prop, xsc_size};

impl CktElement for AutoTrans {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    fn present_tap(&self, terminal: usize) -> Option<f64> {
        Some(AutoTrans::present_tap(self, terminal))
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

    /// Pascal `TDSSCktElement.Get_Losses` **AUTOTRANS_ELEMENT special case**
    /// (`CktElement.pas:618`): sum complex power into only the *first* `Nphases`
    /// conductors of each terminal and **skip the second-half** conductors
    /// (`Inc(k, Nphases)`). The series winding's second node is aliased onto the
    /// common winding's node and `GetCurrents` folds the series current into the
    /// X terminal, so summing all `Yorder` conductors (the base path) would
    /// double-count the series power (≈ V_X·conj(I_series), ~150 MW here). The
    /// base `losses()` must NOT be used for the auto.
    fn losses(&mut self, sys: &SysCtx, node_v: &[Complex64]) -> Complex64 {
        if !self.cd.enabled || self.cd.node_ref.is_empty() {
            return Complex64::ZERO;
        }
        self.compute_iterminal(sys, node_v);
        let cd = self.cd();
        let np = cd.nphases;
        let mut result = Complex64::ZERO;
        // Sum into only the first `Nphases` conductors of each terminal; `nconds =
        // 2·nphases`, so `[np..]` of each per-terminal slice is the skipped
        // second-half (return) conductors (`Inc(k, Nphases)`).
        for t in 0..cd.nterms {
            let (nodes, curr) = (cd.term_nodes(t), cd.term_i(t));
            for c in 0..np {
                let n = nodes[c];
                if n > 0 {
                    result += node_v[n] * curr[c].conj();
                }
            }
        }
        if sys.positive_sequence {
            result *= 3.0;
        }
        result
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

    /// Pascal `TDSSCktElement.SetNodeRef` override (`AutoTrans.pas:875`) — the
    /// "Magic happens here": after the base copy, for terminal 2 with a Series
    /// winding 1, alias the series winding's second node onto the common
    /// winding's first (`NodeRef[Fnphases+i] := NodeRef[i+Fnconds]`), keeping the
    /// flat array and terminal-2's `TermNodeRef` in sync (both writes are
    /// reproduced 1:1 — `NodeRef` 1-based, `TermNodeRef` 0-based).
    fn set_node_ref(&mut self, iterm: usize, node_ref_array: &[usize]) {
        self.cd.set_node_ref(iterm, node_ref_array);
        if iterm == 2 && self.windings[0].connection == Connection::Series {
            let np = self.cd.nphases;
            let nconds = self.cd.nconds;
            for i in 0..np {
                let src = self.cd.node_ref[nconds + i];
                self.cd.node_ref[np + i] = src;
                self.cd.terminals[iterm - 1].term_node_ref[np + i] = src;
            }
        }
    }

    /// Pascal `TAutoTransObj.GetCurrents` override (`AutoTrans.pas:1663`): the
    /// base PD current (`Iterminal = Yprim·Vterminal`), then **fold the series
    /// (wdg 1) current into the X terminal** — `Curr[i+Fnconds] += Curr[i+
    /// Fnphases]` — so the reported X-terminal current is the combined
    /// series+common winding current.
    fn get_currents(&mut self, _sys: &SysCtx, node_v: &[Complex64], curr: &mut [Complex64]) {
        {
            let cd = self.cd_mut();
            if !cd.enabled || cd.node_ref.is_empty() {
                curr.fill(Complex64::ZERO);
                return;
            }
            cd.compute_vterminal(node_v);
            match &cd.yprim {
                Some(yprim) => yprim.mv_mult(curr, &cd.vterminal),
                None => {
                    curr.fill(Complex64::ZERO);
                    return;
                }
            }
        }
        let np = self.cd.nphases;
        let nconds = self.cd.nconds;
        for i in 0..np {
            curr[nconds + i] += curr[np + i];
        }
    }

    /// Pascal `TAutoTransObj.CalcYPrim` (`AutoTrans.pas:1199`): rebuild `Y_Term`
    /// at the solution frequency if it changed, stamp it (and `Y_Term_NL`) into
    /// the series/shunt YPrim via `TermRef`, combine, then apply the
    /// open-conductor corrections. Unlike the Transformer there is **no**
    /// `AddNeutralToY` (the auto has no brought-out neutral impedance).
    fn calc_yprim(&mut self, sys: &SysCtx) {
        let yorder = self.cd.yorder;
        let nw = self.num_windings.max(0) as usize;
        let np = self.cd.nphases;

        let mut yp_series = CMatrix::new(yorder);
        let mut yp_shunt = CMatrix::new(yorder);
        let mut yprim = CMatrix::new(yorder);

        self.cd.yprim_freq = sys.frequency;
        // Pascal `CalcY_Terminal` reads the global `Solution.Frequency` for the
        // GIC gate; refresh the executive-synced copy so the (possible) rebuild
        // below and any later `RecalcElementData` see the live frequency.
        self.live_frequency = sys.frequency;
        let freq_mult = sys.frequency / self.cd.base_frequency;
        if freq_mult != self.y_terminal_freqmult {
            self.calc_y_terminal(freq_mult, sys.frequency);
        }

        Self::build_yprim_component(&mut yp_series, &self.y_term, &self.term_ref, nw, np);
        Self::build_yprim_component(&mut yp_shunt, &self.y_term_nl, &self.term_ref, nw, np);

        yprim.copy_from(&yp_series);
        yprim.add_from(&yp_shunt);

        self.cd.yprim_series = Some(yp_series);
        self.cd.yprim_shunt = Some(yp_shunt);
        self.cd.yprim = Some(yprim);

        self.cd.apply_yprim_open_conductor_calcs();
        self.cd.yprim_invalid = false;
    }

    /// Pascal `TAutoTransObj.MakePosSequence` (AutoTrans.pas:1724-1791). The
    /// autotransformer mirror of `TTransfObj.MakePosSequence`: the sole
    /// difference is the kV test compares the winding connection against
    /// `TAutoTransConnection.Wye` (code 0, Common/Wye) rather than the plain
    /// transformer wye — numerically identical here (both wye = 0), so the
    /// converted values match. `new_conns` are all Wye (0) and buses are kept
    /// verbatim; for a 1/2-phase auto any winding off phase 1 disables it (no
    /// `inherited`).
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

        // Construct the positive-sequence definition: all Common/Wye (0), buses
        // as-is, kV = kVLL/√3 unless the winding is single-phase wye, kVA /
        // NormHkVA / EmergHkVA per phase.
        let new_conns: Vec<i32> = vec![0; nw];
        let new_buses: Vec<String> = (1..=nw).map(|i| self.cd.get_bus(i).to_string()).collect();
        let new_kvs: Vec<Option<f64>> = self
            .windings
            .iter()
            .take(nw)
            .map(|w| {
                // Pascal: (NPhases > 1) or (Connection <> TAutoTransConnection.Wye)
                if nphases > 1 || w.connection != Connection::Wye {
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

impl ControlledTransformer for AutoTrans {
    fn name(&self) -> &str {
        self.cd.obj.name()
    }
    fn full_name(&self) -> String {
        format!("AutoTrans.{}", self.cd.obj.name())
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
        AutoTrans::wdg_connection(self, term)
    }
    fn rotate_phases(&self, iphs: usize) -> usize {
        self.rotate_phases_1based(iphs)
    }
    fn base_voltage(&self, term: usize) -> f64 {
        AutoTrans::base_voltage(self, term)
    }
    fn present_tap(&self, w: usize) -> f64 {
        AutoTrans::present_tap(self, w)
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
        AutoTrans::set_present_tap(self, w, value)
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
            CONN => self.windings[self.aw()].connection.ordinal(),
            NUMTAPS => self.windings[self.aw()].num_taps,
            LEADLAG => self.hv_leads_lv as i32,
            CORE => self.core_type.ordinal(),
            BHPOINTS => self.bh_points,
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
                if let Some(c) = Connection::from_ordinal(value) {
                    self.windings[w].connection = c;
                }
            }
            NUMTAPS => {
                let w = self.aw();
                self.windings[w].num_taps = value;
            }
            LEADLAG => self.hv_leads_lv = value != 0,
            CORE => self.core_type = CoreType::from_ordinal(value).unwrap_or(self.core_type),
            BHPOINTS => self.bh_points = value,
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
            // Pascal `GetDSSArray` renders a NIL pointer as '' (Utilities.pas:1857);
            // an unallocated (BHpoints=0) BH array is the NIL equivalent.
            prop::BHCURRENT => (!self.bh_current.is_empty()).then_some(&self.bh_current[..]),
            prop::BHFLUX => (!self.bh_flux.is_empty()).then_some(&self.bh_flux[..]),
            _ => unreachable!("AutoTrans has no array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        match idx {
            prop::XSCARRAY => self.xsc = value,
            prop::BHCURRENT => self.bh_current = value,
            prop::BHFLUX => self.bh_flux = value,
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
                // `ON_ARRAY` per-winding scalars with no plural form (Pascal
                // `DoubleOnStructArrayProperty`): the schema/JSON `preferArray`
                // sweep renders the full per-winding array.
                RDCOHMS => w.rdcohms,
                MAXTAP => w.max_tap,
                MINTAP => w.min_tap,
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
            prop::CONNS => self
                .windings
                .iter()
                .map(|w| w.connection.ordinal())
                .collect(),
            // `NumTaps` is an `ON_ARRAY` per-winding integer scalar (Pascal
            // `IntegerOnStructArrayProperty`): the schema `preferArray` sweep
            // reads the full per-winding array.
            prop::NUMTAPS => self.windings.iter().map(|w| w.num_taps).collect(),
            _ => unreachable!("AutoTrans has no struct enum array {idx}"),
        }
    }
    fn set_struct_i32_array(&mut self, idx: usize, values: &[i32]) {
        match idx {
            prop::CONNS => {
                for (w, v) in self.windings.iter_mut().zip(values) {
                    if let Some(c) = Connection::from_ordinal(*v) {
                        w.connection = c;
                    }
                }
            }
            _ => unreachable!("AutoTrans has no struct enum array {idx}"),
        }
        self.active_winding = self.num_windings;
    }

    fn set_active_struct_bus(&mut self, value: &str) {
        let t = self.aw() + 1;
        self.set_bus_auto(t, value);
    }
    fn get_active_struct_bus(&self) -> String {
        self.cd.get_bus(self.aw() + 1).to_string()
    }
    fn set_struct_buses(&mut self, values: &[Option<String>]) {
        for (i, v) in values.iter().enumerate() {
            if let Some(v) = v {
                self.set_bus_auto(i + 1, v);
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
                    1 => self.windings[0].connection = Connection::Series,
                    2 => self.windings[1].connection = Connection::Wye,
                    _ => {}
                }
                self.cd.yorder = self.cd.nconds * self.cd.nterms;
            }
            CONNS => {
                for i in 1..=self.num_windings.max(0) as usize {
                    match i {
                        1 => self.windings[0].connection = Connection::Series,
                        2 => self.windings[1].connection = Connection::Wye,
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
            // r4064 (90962ae8): BHpoints reallocates both BH arrays, zeroed.
            BHPOINTS => {
                let n = self.bh_points.max(0) as usize;
                self.bh_current = vec![0.0; n];
                self.bh_flux = vec![0.0; n];
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

        // r4064 (90962ae8): TControlledTransformerObj.MakeLike copies BHpoints
        // and the two BH arrays.
        self.bh_points = o.bh_points;
        self.bh_current.clone_from(&o.bh_current);
        self.bh_flux.clone_from(&o.bh_flux);
    }

    /// Target side of RegControl's deferred `TapNum` write (Pascal `Set_TapNum`
    /// pokes `tr.PresentTap[w]` directly).
    fn apply_ref_action(&mut self, action: &crate::obj::base::RefAction) {
        match action {
            crate::obj::base::RefAction::SetTransformerTap { winding, tap, .. } => {
                self.set_present_tap(*winding, *tap);
            }
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

//! The CIM `DERIEEEType1` (IEEE 1547) dynamics exporter — Pascal
//! `Common/ExportCIMXML.pas` `TIEEE1547Controller` (l.155-197, 2318-3183) and its
//! driver (`ExportCDPSM` l.3614-3634). One reused controller instance pulls the
//! per-control settings from each enabled `InvControl` then each enabled
//! `ExpControl`, emitting a `DERIEEEType1` block (nameplate + applied nameplate +
//! the five settings groups) into the **Dyn** profile.
//!
//! GAPS_PLAN WPG.18 Stage F. The `DERDynamics.RemoteInputSignal` /
//! `RemoteInputSignal` path (Pascal `FindSignalTerminals`, only reached when a
//! control carries a `MonBus`) is ported here but is **not byte-gated**: no
//! oracle corpus deck runs `export cim100` with a monitored-bus InvControl, and
//! the local-monitoring case (`MonBuses` empty ⇒ no signals) is what the Stage F
//! `cim_der` gate exercises. See [`Ieee1547Controller::find_signal_terminals`].

use crate::circuit::Circuit;
use crate::elements::pc::pvsystem::PVSystem;
use crate::elements::pc::storage::Storage;
use crate::elements::traits::{CktElement, ElemId};
use crate::exec::registry::DssClass;
use crate::obj::base::DssObject;

use super::export::{cktelem_dss_obj_type, phase_string};
use super::writer::{self, ProfileChoice, Writer};
use super::{CimExporter, Uuid, UuidChoice};

/// Pascal `CatBQmin` (`ExportCIMXML.pas:241`) — the IEEE 1547 Category-B kvar
/// threshold that classifies a volt-var curve / QMaxLead as catB.
const CAT_B_QMIN: f64 = 0.43;

/// Pascal `TRemoteSignalObject` (`ExportCIMXML.pas:144`): a monitored remote
/// signal for a control's `MonBus`, bound to a `(pElem, trm, phase)` after
/// `FindSignalTerminals`.
struct RemoteSignal {
    bus_name: String,
    phase: String,
    elem: Option<ElemId>,
    trm: i32,
    local_name: String,
    uuid: Uuid,
}

/// The `PullFrom*` snapshot of one control's inputs (released before `write_cim`
/// takes its `&mut Circuit`/`&mut [DssClass]` borrows).
struct InvSnap {
    name: String,
    der_names: Vec<String>,
    mon_buses: Vec<String>,
    vvc: Option<Vec<(f64, f64)>>,
    voltwatt: Option<Vec<(f64, f64)>>,
    voltwattch: Option<Vec<(f64, f64)>>,
    wattvar: Option<Vec<(f64, f64)>>,
    lpf_tau: f64,
    control_mode: i32,
    combi_mode: i32,
    drc_roll_avg_window_length: i32,
    ar_gra_low_v: f64,
    ar_gra_hi_v: f64,
}

struct ExpSnap {
    name: String,
    der_names: Vec<String>,
    qmax_lead: f64,
    qmax_lag: f64,
    vreg_tau: f64,
    tresponse: f64,
    q_v_slope: f64,
}

/// Pascal `TIEEE1547Controller` (`ExportCIMXML.pas:155`): the accumulated
/// nameplate + settings state, reused across every control in one export (so
/// `PullFromExpControl`'s **appending** `pDERNames.Add` — vs `PullFromInvControl`'s
/// replacing `Assign` — carries prior DER names forward; a defined upstream
/// behavior, reproduced 1:1).
#[derive(Default)]
struct Ieee1547Controller {
    // ND_* nameplate (DERNameplateData)
    nd_ac_vmax: f64,
    nd_ac_vmin: f64,
    nd_normal_op_cat_kind: String,
    // AD_* applied nameplate (DERNameplateDataApplied)
    ad_p_max: f64,
    ad_p_max_over_pf: f64,
    ad_over_pf: f64,
    ad_p_max_under_pf: f64,
    ad_under_pf: f64,
    ad_s_max: f64,
    ad_q_max_inj: f64,
    ad_q_max_abs: f64,
    ad_p_max_charge: f64,
    ad_apparent_power_charge_max: f64,
    ad_ac_vnom: f64,
    // VoltVar
    vv_v_ref: f64,
    vv_v_ref_olrt: f64,
    vv_curve_v1: f64,
    vv_curve_v2: f64,
    vv_curve_v3: f64,
    vv_curve_v4: f64,
    vv_olrt: f64,
    vv_curve_q1: f64,
    vv_curve_q2: f64,
    vv_curve_q3: f64,
    vv_curve_q4: f64,
    // ConstQ / ConstPF
    q_reactive_power: f64,
    pf_power_factor: f64,
    pf_const_pf_excitation_kind: String,
    // VoltWatt
    vw_olrt: f64,
    vw_curve_v1: f64,
    vw_curve_v2: f64,
    vw_curve_p1: f64,
    vw_curve_p2gen: f64,
    vw_curve_p2load: f64,
    // WattVar
    wv_curve_p1gen: f64,
    wv_curve_p2gen: f64,
    wv_curve_p3gen: f64,
    wv_curve_p1load: f64,
    wv_curve_p2load: f64,
    wv_curve_p3load: f64,
    wv_curve_q1gen: f64,
    wv_curve_q2gen: f64,
    wv_curve_q3gen: f64,
    wv_curve_q1load: f64,
    wv_curve_q2load: f64,
    wv_curve_q3load: f64,
    // enables
    vv_enabled: bool,
    wv_enabled: bool,
    pf_enabled: bool,
    q_enabled: bool,
    vw_enabled: bool,
    vv_v_ref_auto_mode_enabled: bool,
    b_nameplate_set: bool,
    // current control identity + accumulated lists
    inv_name: String,
    inv_uuid: Uuid,
    der_names: Vec<String>,
    mon_buses: Vec<String>,
    signals: Vec<RemoteSignal>,
}

impl Ieee1547Controller {
    /// Pascal `Create` (`2469`): `SetDefaults(FALSE)`.
    fn new() -> Self {
        let mut c = Ieee1547Controller {
            inv_uuid: Uuid::nil(),
            ..Default::default()
        };
        c.set_defaults(false);
        c
    }

    /// Pascal `SetDefaults` (`ExportCIMXML.pas:2881`).
    fn set_defaults(&mut self, cat_b: bool) {
        self.b_nameplate_set = false;
        self.nd_ac_vmax = 1.05;
        self.nd_ac_vmin = 0.95;
        self.ad_p_max = 0.0;
        self.ad_p_max_over_pf = 0.0;
        self.ad_over_pf = 0.0;
        self.ad_p_max_under_pf = 0.0;
        self.ad_under_pf = 0.0;
        self.ad_s_max = 0.0;
        self.ad_p_max_charge = 0.0;
        self.ad_apparent_power_charge_max = 0.0;
        self.ad_ac_vnom = 0.0;
        self.ad_q_max_inj = 0.44;
        if cat_b {
            self.nd_normal_op_cat_kind = "catB".to_string();
            self.ad_q_max_abs = 0.44;
            self.vv_curve_v1 = 0.92;
            self.vv_curve_v2 = 0.98;
            self.vv_curve_v3 = 1.02;
            self.vv_curve_v4 = 1.08;
            self.vv_curve_q1 = 0.44;
            self.vv_curve_q2 = 0.0;
            self.vv_curve_q3 = 0.0;
            self.vv_curve_q4 = -0.44;
            self.vv_olrt = 5.0;
            self.wv_curve_q3load = 0.44;
        } else {
            self.nd_normal_op_cat_kind = "catA".to_string();
            self.ad_q_max_abs = 0.25;
            self.vv_curve_v1 = 0.90;
            self.vv_curve_v2 = 1.00;
            self.vv_curve_v3 = 1.00;
            self.vv_curve_v4 = 1.10;
            self.vv_curve_q1 = 0.25;
            self.vv_curve_q2 = 0.0;
            self.vv_curve_q3 = 0.0;
            self.vv_curve_q4 = -0.25;
            self.vv_olrt = 10.0;
            self.wv_curve_q3load = 0.25;
        }
        self.vv_v_ref = 1.0;
        self.vv_v_ref_olrt = 300.0;
        self.q_reactive_power = 0.0;
        self.pf_power_factor = 1.0;
        self.vw_olrt = 10.0;
        self.vw_curve_v1 = 1.06;
        self.vw_curve_v2 = 1.10;
        self.vw_curve_p1 = 1.0;
        self.vw_curve_p2gen = 0.2;
        self.vw_curve_p2load = 0.0;

        self.wv_curve_p1gen = 0.2;
        self.wv_curve_p2gen = 0.5;
        self.wv_curve_p3gen = 1.0;
        self.wv_curve_p1load = 0.0;
        self.wv_curve_p2load = 0.0;
        self.wv_curve_p3load = 0.0;
        self.wv_curve_q1gen = 0.0;
        self.wv_curve_q2gen = 0.0;
        self.wv_curve_q3gen = 0.44;
        self.wv_curve_q1load = 0.0;
        self.wv_curve_q2load = 0.0;

        self.pf_const_pf_excitation_kind = "inj".to_string();
        self.vv_enabled = false;
        self.wv_enabled = false;
        self.pf_enabled = true;
        self.q_enabled = false;
        self.vw_enabled = false;
        self.vv_v_ref_auto_mode_enabled = false;
    }

    /// Pascal `PullFromInvControl` (`ExportCIMXML.pas:2493`).
    // The mode/combi arms (`2786-2843`) mirror the Pascal `if…else if` chain 1:1;
    // some arms are deliberately identical (e.g. `combi = 2` and `mode = 1` both
    // enable only volt-var) — kept as distinct arms for the port, not collapsed.
    // `approx_constant`: the `2.3026` below is Pascal's truncated `ln(10)`
    // (compat-tagged at its use site), reproduced verbatim for byte parity.
    #[allow(clippy::if_same_then_else, clippy::approx_constant)]
    fn pull_from_inv_control(&mut self, s: &InvSnap, inv_uuid: Uuid) {
        self.inv_name = s.name.clone();
        self.inv_uuid = inv_uuid;
        // `pDERNames.Assign` — replaces.
        self.der_names = s.der_names.clone();
        // `if MonBusesNameList.Count > 0 then Assign else Clear`.
        self.mon_buses = s.mon_buses.clone();

        let cat_b = s
            .vvc
            .as_ref()
            .is_some_and(|xy| xy.iter().any(|&(_, y)| y < -CAT_B_QMIN));
        self.set_defaults(cat_b);

        // TODO(compat): Pascal's `LPFTau * 2.3026` (`ExportCIMXML.pas:2523`,
        // r4133 `:2164`) — a truncated `ln(10)` (2.302585…); reproduced
        // verbatim so `vRefOlrt` byte-matches. Clean fix:
        // `std::f64::consts::LN_10`.
        //
        // Stage F status (F.3x): the same *documented* constant as
        // `ExpControl`'s `Tresponse / 2.3026` — see the escape argument at
        // `elements/control/exp_control/accessors.rs`. Not a lane split; it
        // would additionally move the `cim_der{,_DYN}.xml` byte goldens, which
        // are compared byte-exact in **both** lanes. Escape-recorded.
        self.vv_olrt = s.lpf_tau * 2.3026;
        self.vw_olrt = self.vv_olrt;

        if let Some(xy) = &s.vvc {
            let (mut b_valid, mut b1, mut b2, mut b3, mut b4) = (false, false, false, false, false);
            let mut i = 1usize;
            while i <= xy.len() {
                let v = xy[i - 1].0;
                if (0.77..=1.25).contains(&v) {
                    b_valid = true;
                }
                if b_valid {
                    if !b1 {
                        self.vv_curve_v1 = v;
                        self.vv_curve_q1 = xy[i - 1].1;
                        b1 = true;
                    } else if !b2 {
                        if v > 1.05 {
                            self.vv_curve_v2 = 1.0;
                            self.vv_curve_q2 = 0.0;
                            if v > 1.08 {
                                self.vv_curve_v3 = 1.0;
                                self.vv_curve_q3 = 0.0;
                                b3 = true;
                                self.vv_curve_v4 = v;
                                self.vv_curve_q4 = xy[i - 1].1;
                                b4 = true;
                            }
                        } else {
                            self.vv_curve_v2 = v;
                            self.vv_curve_q2 = xy[i - 1].1;
                        }
                        b2 = true;
                    } else if !b3 {
                        self.vv_curve_v3 = v;
                        self.vv_curve_q3 = xy[i - 1].1;
                        b3 = true;
                    } else if !b4 {
                        self.vv_curve_v4 = v;
                        self.vv_curve_q4 = xy[i - 1].1;
                        b4 = true;
                    }
                }
                i += 1;
            }
        }

        if let Some(xy) = &s.voltwatt {
            let (mut b_valid, mut b1, mut b2) = (false, false, false);
            let mut i = 1usize;
            while i <= xy.len() {
                let v = xy[i - 1].0;
                let p = xy[i - 1].1;
                if (1.00..=1.10).contains(&v) {
                    b_valid = true;
                }
                if b_valid {
                    if !b1 {
                        self.vw_curve_v1 = v;
                        self.vw_curve_p1 = p;
                        b1 = true;
                    } else if !b2 {
                        self.vw_curve_v2 = v;
                        if p < 0.0 {
                            self.vw_curve_p2gen = 0.2;
                            self.vw_curve_p2load = p;
                        } else {
                            self.vw_curve_p2gen = p;
                            self.vw_curve_p2load = 0.0;
                        }
                        b2 = true;
                    }
                }
                i += 1;
            }
        }

        if let Some(xy) = &s.voltwattch {
            let mut p = 0.0;
            for &(_, y) in xy {
                if y > p {
                    p = y;
                }
            }
            if -p < self.vw_curve_p2load {
                self.vw_curve_p2load = -p;
            }
        }

        if let Some(xy) = &s.wattvar {
            let mut b_valid = false;
            let (mut b1, mut b2, mut b3, mut b4, mut b5, mut b6) =
                (false, false, false, false, false, false);
            let mut i = 1usize;
            while i <= xy.len() {
                let p = xy[i - 1].0;
                let q = xy[i - 1].1;
                if (-1.0..=1.0).contains(&p) {
                    b_valid = true;
                }
                if b_valid {
                    if !b1 {
                        if p <= -0.5 {
                            self.wv_curve_p3load = p;
                            self.wv_curve_q3load = q;
                        } else {
                            self.wv_curve_p3load = -1.0;
                            self.wv_curve_q3load = 0.0;
                            i -= 1; // re-scan
                        }
                        b1 = true;
                    } else if !b2 {
                        if p <= -0.4 {
                            self.wv_curve_p2load = p;
                            self.wv_curve_q2load = q;
                        } else {
                            self.wv_curve_p2load = -0.5;
                            self.wv_curve_q2load = 0.0;
                            i -= 1;
                        }
                        b2 = true;
                    } else if !b3 {
                        if p <= 0.0 {
                            self.wv_curve_p1load = p;
                            self.wv_curve_q1load = q;
                        } else {
                            self.wv_curve_p1load = -0.2;
                            self.wv_curve_q1load = 0.0;
                            i -= 1;
                        }
                        b3 = true;
                    } else if !b4 {
                        if p <= 0.7 {
                            self.wv_curve_p1gen = p;
                            self.wv_curve_q1gen = q;
                        } else {
                            self.wv_curve_p1gen = 0.2;
                            self.wv_curve_q1gen = 0.0;
                            i -= 1;
                        }
                        b4 = true;
                    } else if !b5 {
                        if p <= 0.8 {
                            self.wv_curve_p2gen = p;
                            self.wv_curve_q2gen = q;
                        } else {
                            self.wv_curve_p2gen = 0.5;
                            self.wv_curve_q2gen = 0.0;
                            i -= 1;
                        }
                        b5 = true;
                    } else if !b6 {
                        if p <= 1.0 {
                            self.wv_curve_p3gen = p;
                            self.wv_curve_q3gen = q;
                        } else {
                            self.wv_curve_p3gen = 1.0;
                            self.wv_curve_q3gen = 0.0;
                            i -= 1;
                        }
                        b6 = true;
                    }
                }
                i += 1;
            }
            // edge cases when default zero watt-var points were not input.
            if self.wv_curve_p1gen >= self.wv_curve_p2gen {
                self.wv_curve_p1gen = self.wv_curve_p2gen - 0.1;
            }
            if self.wv_curve_p1load <= self.wv_curve_p2load {
                self.wv_curve_p1load = self.wv_curve_p2load + 0.1;
            }
        }

        // Mode/combi → which settings group is enabled (`2786-2843`).
        let mode = s.control_mode;
        let combi = s.combi_mode;
        if combi == 1 {
            self.pf_enabled = false;
            self.vv_enabled = true;
            self.vw_enabled = true;
        } else if combi == 2 {
            self.pf_enabled = false;
            self.vv_enabled = true;
        } else if mode == 1 {
            self.pf_enabled = false;
            self.vv_enabled = true;
        } else if mode == 2 {
            self.pf_enabled = false;
            self.vw_enabled = true;
        } else if mode == 3 {
            // approximating AVR with DRC
            self.pf_enabled = false;
            self.vv_enabled = true;
            self.vv_v_ref_auto_mode_enabled = true;
            self.vv_v_ref_olrt = s.drc_roll_avg_window_length as f64;
            let qvslope = 0.5 * (s.ar_gra_low_v + s.ar_gra_hi_v);
            let cat_b = cat_b || qvslope > 12.5; // catA max slope 12.5
            let q = if cat_b { 0.44 } else { 0.25 };
            self.vv_curve_q1 = q;
            self.vv_curve_q2 = self.vv_curve_q1;
            self.vv_curve_q3 = -self.vv_curve_q1;
            self.vv_curve_q4 = self.vv_curve_q3;
            self.vv_curve_v1 = 0.50;
            self.vv_curve_v2 = 1.0 - self.vv_curve_q2 / qvslope;
            self.vv_curve_v3 = 1.0 - self.vv_curve_q3 / qvslope;
            self.vv_curve_v4 = 1.50;
        } else if mode == 5 {
            self.pf_enabled = false;
            self.wv_enabled = true;
        }
    }

    /// Pascal `PullFromExpControl` (`ExportCIMXML.pas:2846`).
    fn pull_from_exp_control(&mut self, s: &ExpSnap, exp_uuid: Uuid) {
        self.inv_name = s.name.clone();
        self.inv_uuid = exp_uuid;
        // NOTE: `pDERNames.Add` (append) — NOT a replacing `Assign`; the reused
        // controller carries any prior control's DER names forward (a defined
        // upstream behavior, reproduced 1:1). `pMonBuses.Clear`.
        for n in &s.der_names {
            self.der_names.push(n.clone());
        }
        self.mon_buses.clear();

        self.set_defaults(s.qmax_lead > CAT_B_QMIN); // catB estimate

        self.pf_enabled = false;
        self.vv_enabled = true;
        self.vv_v_ref_auto_mode_enabled = true;
        self.vv_v_ref_olrt = s.vreg_tau;
        self.vv_olrt = s.tresponse;
        self.vv_curve_q1 = s.qmax_lead;
        self.vv_curve_q2 = self.vv_curve_q1;
        self.vv_curve_q3 = -s.qmax_lag;
        self.vv_curve_q4 = self.vv_curve_q3;
        self.vv_curve_v1 = 0.50;
        self.vv_curve_v2 = 1.0 - self.vv_curve_q2 / s.q_v_slope;
        self.vv_curve_v3 = 1.0 - self.vv_curve_q3 / s.q_v_slope;
        self.vv_curve_v4 = 1.50;
    }

    /// Pascal `FinishNameplate` (`2958`).
    fn finish_nameplate(&mut self) {
        self.ad_over_pf = self.ad_p_max_over_pf / self.ad_s_max;
        self.ad_under_pf = self.ad_p_max_under_pf / self.ad_s_max;
        self.b_nameplate_set = true;
    }

    /// Pascal `SetStorageNameplate` (`2965`).
    fn set_storage_nameplate(&mut self, p: &StoragePlate) {
        self.ad_ac_vnom = p.present_kv * 1000.0;
        self.nd_ac_vmax = p.present_kv * p.vmaxpu * 1000.0;
        // NOTE: Pascal uses `Vmaxpu` for acVmin too (`2971`); reproduced 1:1.
        self.nd_ac_vmin = p.present_kv * p.vmaxpu * 1000.0;
        self.ad_s_max = p.kva_rating * 1000.0;
        self.ad_p_max = (p.kw_rating * p.pct_kw_out / 100.0) * 1000.0;
        self.ad_p_max_over_pf =
            (p.kva_rating * p.kva_rating - p.kvar_limit * p.kvar_limit).sqrt() * 1000.0;
        self.ad_p_max_under_pf =
            (p.kva_rating * p.kva_rating - p.kvar_limit_neg * p.kvar_limit_neg).sqrt() * 1000.0;
        self.ad_p_max_charge = (p.kw_rating * p.pct_kw_in / 100.0) * 1000.0;
        self.ad_apparent_power_charge_max = p.kva_rating * 1000.0;
        self.ad_q_max_inj = p.kvar_limit.min(p.kva_rating) * 1000.0;
        self.ad_q_max_abs = p.kvar_limit_neg.min(p.kva_rating) * 1000.0;
        self.finish_nameplate();
    }

    /// Pascal `SetPhotovoltaicNameplate` (`2984`).
    fn set_photovoltaic_nameplate(&mut self, p: &PvPlate) {
        let qmaxinj = if p.kvar_limit_set {
            p.kvar_limit
        } else {
            0.25 * p.kva_rating // catA default
        };
        let qmaxabs = if p.kvar_limit_neg_set {
            p.kvar_limit_neg
        } else {
            0.25 * p.kva_rating
        };
        self.ad_ac_vnom = p.present_kv * 1000.0;
        self.nd_ac_vmax = p.present_kv * p.vmaxpu * 1000.0;
        self.nd_ac_vmin = p.present_kv * p.vminpu * 1000.0;
        self.ad_s_max = p.kva_rating * 1000.0;
        self.ad_p_max = p.pmpp * 1000.0;
        self.ad_p_max_over_pf = (p.kva_rating * p.kva_rating - qmaxinj * qmaxinj).sqrt() * 1000.0;
        self.ad_p_max_under_pf = (p.kva_rating * p.kva_rating - qmaxabs * qmaxabs).sqrt() * 1000.0;
        self.ad_p_max_charge = 0.0;
        self.ad_apparent_power_charge_max = 0.0;
        self.ad_q_max_inj = qmaxinj * 1000.0;
        self.ad_q_max_abs = qmaxabs * 1000.0;
        self.finish_nameplate();
    }

    /// Pascal `FindSignalTerminals` (`2376`). With no `MonBus` the signal list is
    /// empty (the gated local-monitoring case). The `MonBus` branch resolves the
    /// first PD (then PC) element at the monitored bus that phase-matches; ported
    /// faithfully but not byte-gated (no oracle deck runs export-cim with a
    /// `MonBus`) — the PD/PC scan follows the circuit element lists in creation
    /// order (`getPDEatBus`/`getPCEatBus`'s node-touch criteria without the exact
    /// class-list ordering, which only matters when several elements at one bus
    /// phase-match).
    fn find_signal_terminals(&mut self, ckt: &Circuit, classes: &[DssClass]) {
        self.signals.clear();
        if self.mon_buses.is_empty() {
            return;
        }
        // Only the first MonBus (IEEE 1547 forbids different main buses).
        let raw = self.mon_buses[0].clone();
        let mut sig = RemoteSignal {
            bus_name: raw.clone(),
            phase: "A".to_string(),
            elem: None,
            trm: -1,
            local_name: format!("{}_1", self.inv_name),
            uuid: Uuid::nil(),
        };
        if let Some(dot) = raw.find('.') {
            let phase_part = &raw[dot..];
            sig.phase = if phase_part.contains('3') {
                "C".to_string()
            } else if phase_part.contains('2') {
                "B".to_string()
            } else {
                "A".to_string()
            };
            sig.bus_name = raw[..dot].to_string();
        }

        // Bind to the first PD element at the (dot-stripped) bus that phase-
        // matches; failing that, the first PC element at the raw bus.
        let pd_bus = ckt.bus_list.find(&sig.bus_name);
        let found = pd_bus.is_some_and(|b| scan_bus_for_signal(&mut sig, ckt, classes, b, true));
        let pc_bus = if found { None } else { ckt.bus_list.find(&raw) };
        if let Some(b) = pc_bus {
            scan_bus_for_signal(&mut sig, ckt, classes, b, false);
        }
        self.signals.push(sig);
    }

    /// Pascal `WriteCIM` (`ExportCIMXML.pas:3024`) — the `DERIEEEType1` block into
    /// the Dyn profile.
    fn write_cim(
        &mut self,
        buf: &mut Writer,
        cim: &mut CimExporter,
        ckt: &mut Circuit,
        classes: &mut [DssClass],
    ) {
        let prf = ProfileChoice::Dyn;
        self.find_signal_terminals(ckt, classes);
        // Assign each signal a UUID now (Pascal did it in the RemoteSignal ctor).
        for (i, sig) in self.signals.iter_mut().enumerate() {
            sig.uuid = cim.get_dev_uuid(UuidChoice::I1547Signal, &sig.local_name, i as i32 + 1);
        }

        writer::start_instance(buf, prf, "DERIEEEType1", self.inv_uuid, &self.inv_name);
        writer::boolean_node(buf, prf, "DynamicsFunctionBlock.enabled", true);
        writer::boolean_node(buf, prf, "DERIEEEType1.phaseToGroundApplicable", true);
        writer::boolean_node(buf, prf, "DERIEEEType1.phaseToNeutralApplicable", false);
        writer::boolean_node(buf, prf, "DERIEEEType1.phaseToPhaseApplicable", false);

        if self.der_names.is_empty() {
            // Reference + nameplate from every **enabled** Storage then PVSystem
            // (Pascal `WriteCIM` `if pBat.Enabled` `3044` / `if pPV.Enabled`
            // `3052` — the element lists carry disabled units too, so the guard
            // is load-bearing). Unlike the named-DER branch below, which emits
            // regardless (Pascal `SetElementActive` + unconditional `RefNode`).
            for &r in &ckt.storages.clone() {
                let enabled = classes[r.class_ord()]
                    .arena
                    .get::<Storage>(r.index())
                    .is_some_and(|s| s.cd.enabled);
                if !enabled {
                    continue;
                }
                let uuid = classes[r.class_ord()].arena[r.index()].data_mut().uuid();
                if let Some(plate) = storage_plate(classes, r) {
                    writer::ref_node(buf, prf, "DERDynamics.PowerElectronicsConnection", uuid);
                    self.set_storage_nameplate(&plate);
                }
            }
            for &r in &ckt.pv_systems.clone() {
                let enabled = classes[r.class_ord()]
                    .arena
                    .get::<PVSystem>(r.index())
                    .is_some_and(|p| p.cd.enabled);
                if !enabled {
                    continue;
                }
                let uuid = classes[r.class_ord()].arena[r.index()].data_mut().uuid();
                if let Some(plate) = pv_plate(classes, r) {
                    writer::ref_node(buf, prf, "DERDynamics.PowerElectronicsConnection", uuid);
                    self.set_photovoltaic_nameplate(&plate);
                }
            }
        } else {
            for name in &self.der_names.clone() {
                let Some(r) = find_elem(classes, name) else {
                    continue;
                };
                let uuid = classes[r.class_ord()].arena[r.index()].data_mut().uuid();
                writer::ref_node(buf, prf, "DERDynamics.PowerElectronicsConnection", uuid);
                self.set_element_nameplate(classes, r);
            }
        }

        for sig in &self.signals {
            writer::ref_node(buf, prf, "DERDynamics.RemoteInputSignal", sig.uuid);
        }
        writer::end_instance(buf, prf, "DERIEEEType1");

        for sig in &self.signals {
            writer::start_instance(buf, prf, "RemoteInputSignal", sig.uuid, &sig.local_name);
            writer::remote_input_signal_enum(buf, prf, "remoteBusVoltageAmplitude");
            let term_uuid = signal_terminal_uuid(cim, classes, sig);
            writer::ref_node(buf, prf, "RemoteInputSignal.Terminal", term_uuid);
            writer::end_instance(buf, prf, "RemoteInputSignal");
        }

        // DERNameplateData (ND_*).
        let plate_uuid = cim.get_dev_uuid(UuidChoice::I1547NameplateData, &self.inv_name, 1);
        writer::start_instance(buf, prf, "DERNameplateData", plate_uuid, &self.inv_name);
        writer::ref_node(buf, prf, "DERNameplateData.DERIEEEType1", self.inv_uuid);
        writer::normal_op_cat_enum(buf, prf, &self.nd_normal_op_cat_kind);
        writer::boolean_node(buf, prf, "DERNameplateData.supportsConstPFmode", true);
        writer::boolean_node(buf, prf, "DERNameplateData.supportsConstQmode", true);
        writer::boolean_node(buf, prf, "DERNameplateData.supportsQVmode", true);
        let cat_b = self.nd_normal_op_cat_kind == "catB";
        writer::boolean_node(buf, prf, "DERNameplateData.supportsPVmode", cat_b);
        writer::boolean_node(buf, prf, "DERNameplateData.supportsQPmode", cat_b);
        writer::boolean_node(buf, prf, "DERNameplateData.supportsPFmode", false);
        writer::double_node(buf, prf, "DERNameplateData.acVmax", self.nd_ac_vmax);
        writer::double_node(buf, prf, "DERNameplateData.acVmin", self.nd_ac_vmin);
        writer::end_instance(buf, prf, "DERNameplateData");

        // DERNameplateDataApplied (AD_*).
        let applied_uuid =
            cim.get_dev_uuid(UuidChoice::I1547NameplateDataApplied, &self.inv_name, 1);
        writer::start_instance(
            buf,
            prf,
            "DERNameplateDataApplied",
            applied_uuid,
            &self.inv_name,
        );
        writer::ref_node(
            buf,
            prf,
            "DERNameplateDataApplied.DERNameplateData",
            plate_uuid,
        );
        writer::double_node(buf, prf, "DERNameplateDataApplied.pMax", self.ad_p_max);
        writer::double_node(
            buf,
            prf,
            "DERNameplateDataApplied.pMaxOverPF",
            self.ad_p_max_over_pf,
        );
        writer::double_node(buf, prf, "DERNameplateDataApplied.overPF", self.ad_over_pf);
        writer::double_node(
            buf,
            prf,
            "DERNameplateDataApplied.pMaxUnderPF",
            self.ad_p_max_under_pf,
        );
        writer::double_node(
            buf,
            prf,
            "DERNameplateDataApplied.underPF",
            self.ad_under_pf,
        );
        writer::double_node(buf, prf, "DERNameplateDataApplied.sMax", self.ad_s_max);
        writer::double_node(
            buf,
            prf,
            "DERNameplateDataApplied.qMaxInj",
            self.ad_q_max_inj,
        );
        writer::double_node(
            buf,
            prf,
            "DERNameplateDataApplied.qMaxAbs",
            self.ad_q_max_abs,
        );
        writer::double_node(
            buf,
            prf,
            "DERNameplateDataApplied.pMaxCharge",
            self.ad_p_max_charge,
        );
        writer::double_node(
            buf,
            prf,
            "DERNameplateDataApplied.apparentPowerChargeMax",
            self.ad_apparent_power_charge_max,
        );
        writer::double_node(buf, prf, "DERNameplateDataApplied.acVnom", self.ad_ac_vnom);
        writer::end_instance(buf, prf, "DERNameplateDataApplied");

        // VoltVarSettings.
        let vv_uuid = cim.get_dev_uuid(UuidChoice::I1547VoltVar, &self.inv_name, 1);
        writer::start_instance(buf, prf, "VoltVarSettings", vv_uuid, &self.inv_name);
        writer::ref_node(buf, prf, "VoltVarSettings.DERIEEEType1", self.inv_uuid);
        writer::boolean_node(buf, prf, "VoltVarSettings.enabled", self.vv_enabled);
        writer::boolean_node(
            buf,
            prf,
            "VoltVarSettings.vRefAutoModeEnabled",
            self.vv_v_ref_auto_mode_enabled,
        );
        writer::double_node(buf, prf, "VoltVarSettings.vRef", self.vv_v_ref);
        writer::double_node(buf, prf, "VoltVarSettings.vRefOlrt", self.vv_v_ref_olrt);
        writer::double_node(buf, prf, "VoltVarSettings.curveV1", self.vv_curve_v1);
        writer::double_node(buf, prf, "VoltVarSettings.curveV2", self.vv_curve_v2);
        writer::double_node(buf, prf, "VoltVarSettings.curveV3", self.vv_curve_v3);
        writer::double_node(buf, prf, "VoltVarSettings.curveV4", self.vv_curve_v4);
        writer::double_node(buf, prf, "VoltVarSettings.curveQ1", self.vv_curve_q1);
        writer::double_node(buf, prf, "VoltVarSettings.curveQ2", self.vv_curve_q2);
        writer::double_node(buf, prf, "VoltVarSettings.curveQ3", self.vv_curve_q3);
        writer::double_node(buf, prf, "VoltVarSettings.curveQ4", self.vv_curve_q4);
        writer::double_node(buf, prf, "VoltVarSettings.olrt", self.vv_olrt);
        writer::end_instance(buf, prf, "VoltVarSettings");

        // WattVarSettings.
        let wv_uuid = cim.get_dev_uuid(UuidChoice::I1547WattVar, &self.inv_name, 1);
        writer::start_instance(buf, prf, "WattVarSettings", wv_uuid, &self.inv_name);
        writer::ref_node(buf, prf, "WattVarSettings.DERIEEEType1", self.inv_uuid);
        writer::boolean_node(buf, prf, "WattVarSettings.enabled", self.wv_enabled);
        writer::double_node(buf, prf, "WattVarSettings.curveP1gen", self.wv_curve_p1gen);
        writer::double_node(buf, prf, "WattVarSettings.curveP2gen", self.wv_curve_p2gen);
        writer::double_node(buf, prf, "WattVarSettings.curveP3gen", self.wv_curve_p3gen);
        writer::double_node(buf, prf, "WattVarSettings.curveQ1gen", self.wv_curve_q1gen);
        writer::double_node(buf, prf, "WattVarSettings.curveQ2gen", self.wv_curve_q2gen);
        writer::double_node(buf, prf, "WattVarSettings.curveQ3gen", self.wv_curve_q3gen);
        writer::double_node(
            buf,
            prf,
            "WattVarSettings.curveP1load",
            self.wv_curve_p1load,
        );
        writer::double_node(
            buf,
            prf,
            "WattVarSettings.curveP2load",
            self.wv_curve_p2load,
        );
        writer::double_node(
            buf,
            prf,
            "WattVarSettings.curveP3load",
            self.wv_curve_p3load,
        );
        writer::double_node(
            buf,
            prf,
            "WattVarSettings.curveQ1load",
            self.wv_curve_q1load,
        );
        writer::double_node(
            buf,
            prf,
            "WattVarSettings.curveQ2load",
            self.wv_curve_q2load,
        );
        writer::double_node(
            buf,
            prf,
            "WattVarSettings.curveQ3load",
            self.wv_curve_q3load,
        );
        writer::end_instance(buf, prf, "WattVarSettings");

        // ConstantPowerFactorSettings.
        let pf_uuid = cim.get_dev_uuid(UuidChoice::I1547ConstPF, &self.inv_name, 1);
        writer::start_instance(
            buf,
            prf,
            "ConstantPowerFactorSettings",
            pf_uuid,
            &self.inv_name,
        );
        writer::ref_node(
            buf,
            prf,
            "ConstantPowerFactorSettings.DERIEEEType1",
            self.inv_uuid,
        );
        writer::boolean_node(
            buf,
            prf,
            "ConstantPowerFactorSettings.enabled",
            self.pf_enabled,
        );
        writer::power_factor_excitation_enum(buf, prf, &self.pf_const_pf_excitation_kind);
        writer::double_node(
            buf,
            prf,
            "ConstantPowerFactorSettings.powerFactor",
            self.pf_power_factor,
        );
        writer::end_instance(buf, prf, "ConstantPowerFactorSettings");

        // ConstantReactivePowerSettings.
        let q_uuid = cim.get_dev_uuid(UuidChoice::I1547ConstQ, &self.inv_name, 1);
        writer::start_instance(
            buf,
            prf,
            "ConstantReactivePowerSettings",
            q_uuid,
            &self.inv_name,
        );
        writer::ref_node(
            buf,
            prf,
            "ConstantReactivePowerSettings.DERIEEEType1",
            self.inv_uuid,
        );
        writer::boolean_node(
            buf,
            prf,
            "ConstantReactivePowerSettings.enabled",
            self.q_enabled,
        );
        writer::double_node(
            buf,
            prf,
            "ConstantReactivePowerSettings.reactivePower",
            self.q_reactive_power,
        );
        writer::end_instance(buf, prf, "ConstantReactivePowerSettings");

        // VoltWattSettings.
        let vw_uuid = cim.get_dev_uuid(UuidChoice::I1547VoltWatt, &self.inv_name, 1);
        writer::start_instance(buf, prf, "VoltWattSettings", vw_uuid, &self.inv_name);
        writer::ref_node(buf, prf, "VoltWattSettings.DERIEEEType1", self.inv_uuid);
        writer::boolean_node(buf, prf, "VoltWattSettings.enabled", self.vw_enabled);
        writer::double_node(buf, prf, "VoltWattSettings.curveV1", self.vw_curve_v1);
        writer::double_node(buf, prf, "VoltWattSettings.curveV2", self.vw_curve_v2);
        writer::double_node(buf, prf, "VoltWattSettings.curveP1", self.vw_curve_p1);
        writer::double_node(buf, prf, "VoltWattSettings.curveP2gen", self.vw_curve_p2gen);
        writer::double_node(
            buf,
            prf,
            "VoltWattSettings.curveP2load",
            self.vw_curve_p2load,
        );
        writer::double_node(buf, prf, "VoltWattSettings.olrt", self.vw_olrt);
        writer::end_instance(buf, prf, "VoltWattSettings");
    }

    /// Pascal `SetElementNameplate` (`3013`): dispatch by class, once.
    fn set_element_nameplate(&mut self, classes: &[DssClass], r: ElemId) {
        if self.b_nameplate_set {
            return;
        }
        if let Some(plate) = pv_plate(classes, r) {
            self.set_photovoltaic_nameplate(&plate);
        } else if let Some(plate) = storage_plate(classes, r) {
            self.set_storage_nameplate(&plate);
        }
        // Pascal's trailing unconditional `FinishNameplate` — idempotent when a
        // Set*Nameplate already ran; sets the flag (and 0/0 pf) when neither did.
        self.finish_nameplate();
    }
}

struct PvPlate {
    present_kv: f64,
    vmaxpu: f64,
    vminpu: f64,
    kva_rating: f64,
    pmpp: f64,
    kvar_limit: f64,
    kvar_limit_neg: f64,
    kvar_limit_set: bool,
    kvar_limit_neg_set: bool,
}

struct StoragePlate {
    present_kv: f64,
    vmaxpu: f64,
    kva_rating: f64,
    kw_rating: f64,
    pct_kw_out: f64,
    pct_kw_in: f64,
    kvar_limit: f64,
    kvar_limit_neg: f64,
}

fn pv_plate(classes: &[DssClass], r: ElemId) -> Option<PvPlate> {
    let pv = classes[r.class_ord()].arena.get::<PVSystem>(r.index())?;
    Some(PvPlate {
        present_kv: pv.kv_pvsystem_base,
        vmaxpu: pv.base.vmaxpu,
        vminpu: pv.base.vminpu,
        kva_rating: pv.f_kva_rating,
        pmpp: pv.f_pmpp,
        kvar_limit: pv.f_kvar_limit,
        kvar_limit_neg: pv.f_kvar_limit_neg,
        kvar_limit_set: pv.base.kvar_limit_set,
        kvar_limit_neg_set: pv.base.kvar_limit_neg_set,
    })
}

fn storage_plate(classes: &[DssClass], r: ElemId) -> Option<StoragePlate> {
    let st = classes[r.class_ord()].arena.get::<Storage>(r.index())?;
    Some(StoragePlate {
        present_kv: st.present_kv(),
        vmaxpu: st.base.vmaxpu,
        kva_rating: st.f_kva_rating,
        kw_rating: st.kw_rating,
        pct_kw_out: st.pct_kw_out,
        pct_kw_in: st.pct_kw_in,
        kvar_limit: st.f_kvar_limit,
        kvar_limit_neg: st.f_kvar_limit_neg,
    })
}

/// Resolve `Class.Name` to an [`ElemId`] (Pascal `SetElementActive`).
fn find_elem(classes: &[DssClass], full_name: &str) -> Option<ElemId> {
    let (cls_name, obj_name) = full_name.split_once('.')?;
    let cls = classes
        .iter()
        .position(|c| c.props.class_name().eq_ignore_ascii_case(cls_name))?;
    let idx = classes[cls]
        .arena
        .objs()
        .position(|o| o.data().name().eq_ignore_ascii_case(obj_name))?;
    Some(ElemId::new(cls, idx))
}

/// The signal element's terminal UUID (Pascal `GetTermUuid(sig.pElem, sig.trm)`).
fn signal_terminal_uuid(cim: &mut CimExporter, classes: &[DssClass], sig: &RemoteSignal) -> Uuid {
    let Some(r) = sig.elem else {
        return Uuid::nil();
    };
    let ce = classes[r.class_ord()]
        .arena
        .try_ckt_elem(r.index())
        .expect("signal element");
    let class_name = classes[r.class_ord()].props.class_name();
    let dss_obj_type = cktelem_dss_obj_type(class_name).unwrap_or(0);
    let name = ce.cd().obj.name().to_string();
    cim.get_term_uuid(dss_obj_type, &name, sig.trm)
}

/// Scan the PD (`pd = true`) or PC elements at `bus_idx` for the first terminal
/// that [`check_signal_match`]es `sig`; binds it and returns `true` on a hit.
fn scan_bus_for_signal(
    sig: &mut RemoteSignal,
    ckt: &Circuit,
    classes: &[DssClass],
    bus_idx: usize,
    pd: bool,
) -> bool {
    for r in elements_at_bus(ckt, classes, bus_idx, pd) {
        let Some(ce) = classes[r.class_ord()].arena.try_ckt_elem(r.index()) else {
            continue;
        };
        for k in 1..=ce.cd().nterms {
            if check_signal_match(sig, ckt, ce, r, k) {
                return true;
            }
        }
    }
    false
}

/// Pascal `CheckSignalMatch` (`ExportCIMXML.pas:2335`): does terminal `seq` of
/// `ce` connect the signal's bus with a matching phase? Records the binding into
/// `sig` on a hit.
fn check_signal_match(
    sig: &mut RemoteSignal,
    ckt: &Circuit,
    ce: &dyn CktElement,
    r: ElemId,
    seq: usize,
) -> bool {
    let trm_bus = ce.cd().get_bus(seq);
    let trm_bus = trm_bus.split('.').next().unwrap_or(trm_bus);
    if !sig.bus_name.eq_ignore_ascii_case(trm_bus) {
        return false;
    }
    let bus_ref = ce.cd().terminals[seq - 1].bus_ref;
    let kvbase = bus_ref
        .and_then(|b| ckt.buses.get(b))
        .map(|b| b.kv_base)
        .unwrap_or(0.0);
    let elm_phases = phase_string(ce.cd().get_bus(seq), ce.cd().nphases, kvbase, true);
    if elm_phases.contains(&sig.phase) {
        sig.trm = seq as i32;
        sig.elem = Some(r);
        true
    } else if elm_phases.contains('1') && sig.phase == "A" {
        sig.trm = seq as i32;
        sig.elem = Some(r);
        sig.phase = "s1".to_string();
        true
    } else if elm_phases.contains('2') && sig.phase == "B" {
        sig.trm = seq as i32;
        sig.elem = Some(r);
        sig.phase = "s2".to_string();
        true
    } else {
        false
    }
}

/// The PD (`pd = true`) or PC elements touching `bus_idx` — Pascal
/// `getPDEatBus`/`getPCEatBus` (`Circuit.pas:1712/1797`) reduced to its bus-name
/// criterion: a PD element is included when it touches the bus and `bus1 != bus2`;
/// a PC element when its `bus1` is the bus. Creation order (see
/// [`Ieee1547Controller::find_signal_terminals`]).
///
/// **Not the same walk as [`Dss::all_bus_elements`]**
/// (`exec/view.rs::build_bus_elements`), which publishes the port's
/// `Bus.AllPCEatBus`/`AllPDEatBus` surface, and the two must not be merged:
///
/// * this one walks the port's own [`Circuit::pd_elements`] /
///   [`Circuit::pc_elements`] membership, so it never sees a `Fault` (on
///   neither list) and puts `Capacitor`/`Reactor` on the PD side only; the
///   at-bus surface asks the **upstream class tree**
///   ([`ElemKind::is_power_delivery`] / [`ElemKind::is_power_conversion`],
///   which adds `Fault` to PD and the two shunts to both);
/// * this one keeps r4133's own terminal-1/2 name test, which is what the CIM
///   `IEEE1547` signal scan wants; the at-bus surface answers **any** terminal
///   (GOLDEN_REBASE D26 `S4`).
///
/// Re-pointing this function at the at-bus walk would widen the signal scan
/// (`Ieee1547Controller::find_signal_terminals` → `scan_bus_for_signal`) to
/// `Fault`/`Vsource`/`Isource` and could move CIM export bytes, which WP-G1
/// does not do.
///
/// [`Dss::all_bus_elements`]: crate::exec::Dss::all_bus_elements
/// [`ElemKind::is_power_delivery`]: crate::circuit::ElemKind::is_power_delivery
/// [`ElemKind::is_power_conversion`]: crate::circuit::ElemKind::is_power_conversion
fn elements_at_bus(ckt: &Circuit, classes: &[DssClass], bus_idx: usize, pd: bool) -> Vec<ElemId> {
    let want = ckt.buses[bus_idx].name.to_ascii_lowercase();
    let strip = |s: &str| s.split('.').next().unwrap_or(s).to_ascii_lowercase();
    let list = if pd {
        &ckt.pd_elements
    } else {
        &ckt.pc_elements
    };
    let mut out = Vec::new();
    for &r in list {
        let Some(ce) = classes[r.class_ord()].arena.try_ckt_elem(r.index()) else {
            continue;
        };
        let b1 = strip(ce.cd().get_bus(1));
        if pd {
            let b2 = strip(ce.cd().get_bus(2));
            if (b1 == want || b2 == want) && b1 != b2 {
                out.push(r);
            }
        } else if b1 == want {
            out.push(r);
        }
    }
    out
}

/// Pascal `ExportCDPSM` l.3614-3634: build one controller and pull each enabled
/// `InvControl` then each enabled `ExpControl`, writing a `DERIEEEType1` per pull.
pub(super) fn write_ieee1547_controllers(
    buf: &mut Writer,
    classes: &mut [DssClass],
    ckt: &mut Circuit,
    cim: &mut CimExporter,
) {
    let inv_refs = class_refs(classes, "InvControl");
    let exp_refs = class_refs(classes, "ExpControl");
    if inv_refs.is_empty() && exp_refs.is_empty() {
        return;
    }
    let mut ctrl = Ieee1547Controller::new();

    for r in inv_refs {
        let Some(snap) = inv_snap(classes, r) else {
            continue;
        };
        let uuid = classes[r.class_ord()].arena[r.index()].data_mut().uuid();
        ctrl.pull_from_inv_control(&snap, uuid);
        ctrl.write_cim(buf, cim, ckt, classes);
    }
    for r in exp_refs {
        let Some(snap) = exp_snap(classes, r) else {
            continue;
        };
        let uuid = classes[r.class_ord()].arena[r.index()].data_mut().uuid();
        ctrl.pull_from_exp_control(&snap, uuid);
        ctrl.write_cim(buf, cim, ckt, classes);
    }
}

/// The `(cls, idx)` refs of every object of class `name`, in creation order.
fn class_refs(classes: &[DssClass], name: &str) -> Vec<ElemId> {
    classes
        .iter()
        .position(|c| c.props.class_name().eq_ignore_ascii_case(name))
        .map(|cls| {
            (0..classes[cls].arena.len())
                .map(|idx| ElemId::new(cls, idx))
                .collect()
        })
        .unwrap_or_default()
}

/// Snapshot an enabled `InvControl`'s CIM inputs (`None` if disabled).
fn inv_snap(classes: &[DssClass], r: ElemId) -> Option<InvSnap> {
    use crate::elements::control::inv_control::InvControl;
    let inv = classes[r.class_ord()].arena.get::<InvControl>(r.index())?;
    if !inv.cd().enabled {
        return None;
    }
    let pts = |c: Option<&crate::elements::general::xy_curve::XyCurveObj>| {
        c.map(|xy| {
            xy.x_values()
                .iter()
                .zip(xy.y_values())
                .map(|(&x, &y)| (x, y))
                .collect::<Vec<_>>()
        })
    };
    Some(InvSnap {
        name: inv.data().name().to_string(),
        der_names: inv.der_name_list().to_vec(),
        mon_buses: inv.mon_buses_name_list().to_vec(),
        vvc: pts(inv.vvc_curve()),
        voltwatt: pts(inv.voltwatt_curve()),
        voltwattch: pts(inv.voltwattch_curve()),
        wattvar: pts(inv.wattvar_curve()),
        lpf_tau: inv.lpf_tau(),
        control_mode: inv.control_mode().ordinal(),
        combi_mode: inv.combi_mode().ordinal(),
        drc_roll_avg_window_length: inv.drc_roll_avg_window_length(),
        ar_gra_low_v: inv.ar_gra_low_v(),
        ar_gra_hi_v: inv.ar_gra_hi_v(),
    })
}

/// Snapshot an enabled `ExpControl`'s CIM inputs (`None` if disabled).
fn exp_snap(classes: &[DssClass], r: ElemId) -> Option<ExpSnap> {
    use crate::elements::control::exp_control::ExpControl;
    let exp = classes[r.class_ord()].arena.get::<ExpControl>(r.index())?;
    if !exp.cd().enabled {
        return None;
    }
    Some(ExpSnap {
        name: exp.data().name().to_string(),
        der_names: exp.der_name_list().to_vec(),
        qmax_lead: exp.qmax_lead(),
        qmax_lag: exp.qmax_lag(),
        vreg_tau: exp.vreg_tau(),
        tresponse: exp.tresponse(),
        q_v_slope: exp.q_v_slope(),
    })
}

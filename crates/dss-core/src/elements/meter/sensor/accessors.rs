//! The metered-element snapshot capture plus the `CktElement` and `DssObject`
//! trait impls for `Sensor` (typed property accessors, `set_object_ref`,
//! `PropertySideEffects`, `EndEdit`, `MakeLike`).

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::meter::meter_element::MeteredSnapshot;
use crate::elements::pos_seq::{PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, ElemId, SysCtx};
use crate::obj::arena::ResolvedObj;
use crate::obj::base::{DssObjData, DssObject};

use super::{Sensor, prop};

/// Capture the parse-relevant shape of the metered element (any class).
fn capture(full_name: String, obj: &dyn DssObject) -> MeteredSnapshot {
    let elem = obj
        .as_ckt_element()
        .expect("element= resolves to a ckt elem");
    let cd = elem.cd();
    MeteredSnapshot {
        full_name,
        nphases: cd.nphases,
        nconds: cd.nconds,
        nterms: cd.nterms,
        yorder: cd.yorder,
        buses: (1..=cd.nterms).map(|i| cd.get_bus(i).to_string()).collect(),
        ..Default::default()
    }
}

impl CktElement for Sensor {
    fn cd(&self) -> &CktElementData {
        &self.med.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.med.cd
    }

    fn recalc_element_data(&mut self, _sys: &SysCtx) {
        let mut errors = crate::diag::ErrorLog::new();
        self.recalc(&mut errors);
        for e in errors {
            self.med.cd.obj.push_error(e);
        }
    }

    /// `TSensorObj.CalcYPrim` is empty — a sensor never stamps admittance.
    fn calc_yprim(&mut self, _sys: &SysCtx) {}

    /// `TSensorObj.GetCurrents` returns zeros.
    fn get_currents(&mut self, _sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
        curr.fill(Complex64::ZERO);
    }

    /// Pascal `TSensorObj.MakePosSequence` (`Meters/Sensor.pas:478`): resync the
    /// sensor to the metered element's bus / phase / conductor counts, then
    /// `ClearSensor` / `ValidSensor := TRUE` / `AllocateSensorObjArrays` /
    /// `ZeroSensorArrays` / `RecalcVbase`, and run the base bus rename
    /// (`inherited`). Pascal NIL-guards `MeteredElement`; `ctx.monitored` is
    /// `None` in the same case.
    fn make_pos_sequence(&mut self, ctx: &PosSeqCtx) -> PosSeqPlan {
        if let Some(m) = &ctx.monitored {
            // Setbus(1, MeteredElement.GetBus(MeteredTerminal))
            let mt = self.med.metered_terminal as usize;
            let bus = mt
                .checked_sub(1)
                .and_then(|k| m.bus_names.get(k))
                .cloned()
                .unwrap_or_default();
            self.med.cd.set_bus(1, &bus);
            // FNphases := MeteredElement.NPhases; Nconds := MeteredElement.Nconds
            self.med.cd.nphases = m.nphases;
            self.med.cd.set_nconds(m.nconds);
            self.clear_sensor();
            self.valid_sensor = true;
            // AllocateSensorObjArrays + ZeroSensorArrays (calc buffers sized to
            // the metered element's Yorder).
            self.allocate_and_zero_arrays(m.yorder);
            self.recalc_vbase();
        }
        // inherited MakePosSequence -> base bus rename.
        PosSeqPlan::base()
    }

    /// Pascal `TMeterElement.MeteredElement` — resolved so the exec applier can
    /// build [`PosSeqCtx::monitored`] before calling [`Self::make_pos_sequence`].
    fn monitored_element_ref(&self) -> Option<ElemId> {
        self.med.metered_element
    }
}

impl Sensor {
    pub(crate) fn make_like(&mut self, other: &Self) {
        let o = other;
        // Pascal `TSensorObj.MakeLike` copies **only** the shape/metered fields
        // and the base frequency — kVBase/conn/%Error/Weight/DeltaDirection and
        // the measured arrays stay at the new object's ctor defaults / NIL.
        self.med.cd.make_like_base(&o.med.cd);
        self.med.cd.nphases = o.med.cd.nphases;
        self.med.cd.set_nconds(o.med.cd.nconds);
        self.med.metered_element = o.med.metered_element;
        self.med.metered_terminal = o.med.metered_terminal;
        self.med.metered_snap = o.med.metered_snap.clone();
        self.element_full_name = o.element_full_name.clone();
        self.med.cd.base_frequency = o.med.cd.base_frequency;
    }
}

impl DssObject for Sensor {
    fn data(&self) -> &DssObjData {
        &self.med.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.med.cd.obj
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
            TERMINAL => self.med.metered_terminal,
            CONN => self.f_conn,
            DELTA_DIRECTION => self.f_delta_direction,
            _ => unreachable!("Sensor has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            TERMINAL => self.med.metered_terminal = value,
            CONN => self.f_conn = value,
            DELTA_DIRECTION => self.f_delta_direction = value,
            _ => unreachable!("Sensor has no integer property {idx}"),
        }
    }

    fn get_f64(&self, idx: usize) -> f64 {
        use prop::*;
        match idx {
            KVBASE => self.kv_base,
            PCT_ERROR => self.pct_error,
            WEIGHT => self.weight,
            BASE_FREQ => self.med.cd.base_frequency,
            _ => unreachable!("Sensor has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            KVBASE => self.kv_base = value,
            PCT_ERROR => self.pct_error = value,
            WEIGHT => self.weight = value,
            BASE_FREQ => self.med.cd.base_frequency = value,
            _ => unreachable!("Sensor has no double property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use prop::*;
        match idx {
            // BooleanActionProperty stores nothing → always `No`.
            CLEAR => false,
            ENABLED => self.med.cd.enabled,
            _ => unreachable!("Sensor has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use prop::*;
        match idx {
            CLEAR => {
                if value {
                    self.clear_sensor();
                }
            }
            ENABLED => self.med.cd.set_enabled(value),
            _ => unreachable!("Sensor has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        match idx {
            prop::ELEMENT => self.element_full_name.clone(),
            _ => unreachable!("Sensor has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        match idx {
            prop::ELEMENT => self.element_full_name = value,
            _ => unreachable!("Sensor has no string property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        use prop::*;
        // An unallocated array (Pascal NIL pointer) dumps as "" — return `None`
        // so the renderer reproduces that, rather than an empty `[]`.
        let arr: &[f64] = match idx {
            KVS => &self.med.sensor_voltage,
            CURRENTS => &self.med.sensor_current,
            KWS => &self.sensor_kw,
            KVARS => &self.sensor_kvar,
            _ => unreachable!("Sensor has no double-array property {idx}"),
        };
        (!arr.is_empty()).then_some(arr)
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        use prop::*;
        match idx {
            KVS => self.med.sensor_voltage = value,
            CURRENTS => self.med.sensor_current = value,
            KWS => self.sensor_kw = value,
            KVARS => self.sensor_kvar = value,
            _ => unreachable!("Sensor has no double-array property {idx}"),
        }
    }
    fn array_size(&self, _idx: usize) -> usize {
        // All four measured arrays are sized to Fnphases.
        self.med.cd.nphases
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.med.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.med.cd.get_bus(terminal).to_string()
    }

    /// Resolve `element=` (any circuit class by full name): snapshot the metered
    /// element for `RecalcElementData`.
    fn set_object_ref(&mut self, idx: usize, name: String, resolved: Option<ResolvedObj<'_>>) {
        match idx {
            prop::ELEMENT => {
                self.element_full_name = name.clone();
                match resolved {
                    Some(o) => {
                        self.med.metered_element = Some(o.id());
                        self.med.metered_element_changed = true;
                        self.med.metered_snap = Some(capture(name, o.obj()));
                    }
                    None => {
                        self.med.metered_element = None;
                        self.med.metered_snap = None;
                    }
                }
            }
            _ => unreachable!("Sensor has no object-ref property {idx}"),
        }
    }

    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        use prop::*;
        match idx {
            ELEMENT | TERMINAL => {
                self.needs_recalc = true;
                self.med.metered_element_changed = true;
            }
            KVBASE => self.needs_recalc = true,
            KVS => self.v_specified = true,
            CURRENTS => self.i_specified = true,
            KWS => {
                self.p_specified = true;
                self.update_current_vector();
            }
            KVARS => {
                self.q_specified = true;
                self.update_current_vector();
            }
            CONN => {
                self.recalc_vbase();
                self.needs_recalc = true;
            }
            DELTA_DIRECTION => {
                self.f_delta_direction = if self.f_delta_direction >= 0 { 1 } else { -1 };
                self.needs_recalc = true;
            }
            _ => {}
        }
    }

    fn end_edit(&mut self, _sys: &crate::elements::traits::SysCtx) {
        // Pascal `EndEdit`: recalc only when a recalc-triggering datum changed.
        if !self.needs_recalc {
            return;
        }
        let mut errors = crate::diag::ErrorLog::new();
        self.recalc(&mut errors);
        for e in errors {
            self.med.cd.obj.push_error(e);
        }
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

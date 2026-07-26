//! The `impl DssObject` property surface for [`Capacitor`] — scalar/array
//! getters & setters, bus names, `PropertySideEffects`, `EndEdit`, `MakeLike`.

use super::Capacitor;
use crate::obj::base::{DssObjData, DssObject};

impl Capacitor {
    /// Pascal `TCapacitorObj.MakeLike`.
    pub(crate) fn make_like(&mut self, other: &Self) {
        self.cd.make_like_base(&other.cd);
        if self.cd.nphases != other.cd.nphases {
            self.cd.nphases = other.cd.nphases;
            let n = other.cd.nphases;
            self.cd.set_nconds(n); // force reallocation of terminals/conductors
            self.cd.yorder = self.cd.nconds * self.cd.nterms;
            self.cd.yprim_invalid = true;
        }

        self.set_num_steps(other.fnumsteps);
        let n = self.n_steps();
        self.fc[..n].copy_from_slice(&other.fc[..n]);
        self.fkvarrating[..n].copy_from_slice(&other.fkvarrating[..n]);
        self.fr[..n].copy_from_slice(&other.fr[..n]);
        self.fxl[..n].copy_from_slice(&other.fxl[..n]);
        self.fharm[..n].copy_from_slice(&other.fharm[..n]);
        self.fstates[..n].copy_from_slice(&other.fstates[..n]);

        self.kvrating = other.kvrating;
        self.connection = other.connection;
        self.spec_type = other.spec_type;
        self.cmatrix = other.cmatrix.clone();

        // TPDElement.MakeLike copies the rating fields.
        self.norm_amps = other.norm_amps;
        self.emerg_amps = other.emerg_amps;
        self.fault_rate = other.fault_rate;
        self.pct_perm = other.pct_perm;
        self.hrs_to_repair = other.hrs_to_repair;
    }
}

impl DssObject for Capacitor {
    fn data(&self) -> &DssObjData {
        &self.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.cd.obj
    }

    fn get_f64(&self, idx: usize) -> f64 {
        use super::prop::*;
        match idx {
            KV => self.kvrating,
            NORMAMPS => self.norm_amps,
            EMERGAMPS => self.emerg_amps,
            FAULTRATE => self.fault_rate,
            PCTPERM => self.pct_perm,
            REPAIR => self.hrs_to_repair,
            BASE_FREQ => self.cd.base_frequency,
            _ => unreachable!("Capacitor has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use super::prop::*;
        match idx {
            KV => self.kvrating = value,
            NORMAMPS => self.norm_amps = value,
            EMERGAMPS => self.emerg_amps = value,
            FAULTRATE => self.fault_rate = value,
            PCTPERM => self.pct_perm = value,
            REPAIR => self.hrs_to_repair = value,
            BASE_FREQ => self.cd.base_frequency = value,
            _ => unreachable!("Capacitor has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use super::prop::*;
        match idx {
            PHASES => self.cd.nphases as i32,
            CONN => self.connection,
            NUMSTEPS => self.fnumsteps,
            _ => unreachable!("Capacitor has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use super::prop::*;
        match idx {
            PHASES => self.cd.nphases = value.max(0) as usize,
            CONN => self.connection = value,
            NUMSTEPS => self.fnumsteps = value,
            _ => unreachable!("Capacitor has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        match idx {
            super::prop::ENABLED => self.cd.enabled,
            _ => unreachable!("Capacitor has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        match idx {
            super::prop::ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("Capacitor has no boolean property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        use super::prop::*;
        match idx {
            KVAR => Some(&self.fkvarrating),
            CUF => Some(&self.fc),
            R => Some(&self.fr),
            XL => Some(&self.fxl),
            HARM => Some(&self.fharm),
            CMATRIX => self.cmatrix.as_deref(),
            _ => unreachable!("Capacitor has no double-array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        use super::prop::*;
        match idx {
            KVAR => self.fkvarrating = value,
            CUF => self.fc = value,
            R => self.fr = value,
            XL => self.fxl = value,
            HARM => self.fharm = value,
            CMATRIX => self.cmatrix = Some(value),
            _ => unreachable!("Capacitor has no double-array property {idx}"),
        }
    }

    /// `TCapacitorObj.MakePosSequence` drives `SetDoubles(ord(TProp.kvar), …)`
    /// (`Capacitor.pas:793`); `kvar` is a plain `DoubleArrayProperty`, but the
    /// exec applier reaches it through the shared `SetStructF64s` action → this
    /// hook (as the Transformer's struct-array `kVs`/`kVAs` do). Each `Some` is
    /// the already-scaled entry, `None` keeps the prior value (Pascal semantics);
    /// the array is (re)sized to the supplied count, matching `SetObjDoubles`.
    fn set_struct_f64_array(&mut self, idx: usize, values: &[Option<f64>]) {
        use super::prop::*;
        match idx {
            KVAR => {
                let old = std::mem::take(&mut self.fkvarrating);
                self.fkvarrating = values
                    .iter()
                    .enumerate()
                    .map(|(i, v)| v.unwrap_or_else(|| old.get(i).copied().unwrap_or(0.0)))
                    .collect();
            }
            _ => unreachable!("Capacitor has no struct-double-array property {idx}"),
        }
    }

    fn get_i32_array(&self, idx: usize) -> Option<&[i32]> {
        match idx {
            super::prop::STATES => Some(&self.fstates),
            _ => unreachable!("Capacitor has no integer-array property {idx}"),
        }
    }
    fn set_i32_array(&mut self, idx: usize, value: Vec<i32>) {
        match idx {
            super::prop::STATES => self.fstates = value,
            _ => unreachable!("Capacitor has no integer-array property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.cd.get_bus(terminal).to_string()
    }

    /// Pascal `TCapacitorObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, prev_int: i32) {
        use super::prop::*;
        match idx {
            BUS1 => {
                // Default Bus2 to the grounded-zero node of Bus1 (wye shunt) if
                // Bus2 has not been explicitly set.
                if !self.bus2_defined && self.cd.nterms == 2 {
                    let s = self.cd.get_bus(1).to_string();
                    let base = match s.find('.') {
                        Some(p) => s[..p].to_string(),
                        None => s,
                    };
                    let mut s2 = base;
                    for _ in 0..self.cd.nphases {
                        s2.push_str(".0");
                    }
                    self.cd.set_bus(2, &s2);
                    self.is_shunt = true;
                    self.cd.obj.clear_seq(BUS2); // reset for the save function
                }
            }
            CONN => match self.connection {
                1 => {
                    // Delta: force one terminal.
                    self.cd.set_nterms(1);
                    let nc = if self.cd.nphases == 1 || self.cd.nphases == 2 {
                        self.cd.nphases + 1
                    } else {
                        self.cd.nphases
                    };
                    self.cd.set_nconds(nc);
                }
                _ => {
                    // Wye.
                    if self.cd.nterms != 2 {
                        self.cd.set_nterms(2);
                    }
                    let np = self.cd.nphases;
                    self.cd.set_nconds(np);
                }
            },
            BUS2 => {
                self.num_term = 2;
                if !strip_extension(self.cd.get_bus(1))
                    .eq_ignore_ascii_case(&strip_extension(self.cd.get_bus(2)))
                {
                    self.is_shunt = false;
                    self.bus2_defined = true;
                }
            }
            PHASES => {
                if self.cd.nphases as i32 != prev_int {
                    let nc =
                        if self.connection == 1 && (self.cd.nphases == 1 || self.cd.nphases == 2) {
                            self.cd.nphases + 1
                        } else {
                            self.cd.nphases
                        };
                    self.cd.set_nconds(nc);
                    self.cd.yorder = self.cd.nterms * self.cd.nconds;
                } else if self.connection == 1 && self.cd.nconds != self.cd.nphases + 1 {
                    self.cd.set_nconds(self.cd.nphases + 1);
                    self.cd.yorder = self.cd.nterms * self.cd.nconds;
                }
            }
            KVAR => self.spec_type = 1,
            CMATRIX => self.spec_type = 3,
            CUF => self.spec_type = 2,
            NUMSTEPS => self.side_effect_numsteps(prev_int),
            XL => {
                for i in 0..self.n_steps() {
                    if self.fxl[i] != 0.0 && self.fr[i] == 0.0 {
                        self.fr[i] = self.fxl[i].abs() / 1000.0;
                    }
                }
                self.do_harmonic_recalc = false;
            }
            HARM => self.do_harmonic_recalc = true,
            STATES => self.find_last_step_in_service(),
            NORMAMPS => self.norm_amps_specified = true,
            EMERGAMPS => self.emerg_amps_specified = true,
            _ => {}
        }

        // YPrim invalidation on anything that changes the impedance values.
        if matches!(
            idx,
            PHASES | KVAR | KV | CONN | CMATRIX | CUF | NUMSTEPS | STATES
        ) {
            self.cd.yprim_invalid = true;
        }
    }

    /// Pascal base `EndEdit` → `RecalcElementData` (Capacitor does not override).
    fn end_edit(&mut self, _sys: &crate::elements::traits::SysCtx) {
        self.recalc();
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

/// Pascal `StripExtension`: the bus name with its `.node.node…` suffix removed.
fn strip_extension(s: &str) -> String {
    match s.find('.') {
        Some(p) => s[..p].to_string(),
        None => s.to_string(),
    }
}

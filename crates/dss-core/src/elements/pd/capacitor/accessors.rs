//! The `impl DssObject` property surface for [`Capacitor`] — scalar/array
//! getters & setters, bus names, `PropertySideEffects`, `EndEdit`, `MakeLike`.

use super::{Capacitor, CapacitorSpecType};
use crate::obj::base::{DssObjData, DssObject};

impl Capacitor {
    /// Make this Capacitor a copy of `other`, keeping its own name and buses.
    pub(crate) fn make_like(&mut self, other: &Self) {
        self.cd.make_like_base(&other.cd);
        // The copy takes the phases, the connection, the terminals, the
        // conductors and the admittance of `other`, as the element written with
        // its data has them (`the_power_flow_after_a_reshape_solves_the_new_shape`,
        // `a_copy_solves_as_the_circuit_written_out`).
        self.cd.yprim_invalid = true;
        let phases_gained = other.cd.nphases > self.cd.nphases;
        self.connection = other.connection;
        self.cd.nphases = other.cd.nphases;
        self.set_terminals_for_connection(phases_gained);
        self.cd.set_shape(other.cd.nconds, self.connection);
        self.cd.yorder = self.cd.nconds * self.cd.nterms;

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

        // A typed rating, `none` included, stays typed: the copy does not derive
        // it from kvar again.
        self.norm_amps = other.norm_amps;
        self.emerg_amps = other.emerg_amps;
        self.norm_amps_specified = other.norm_amps_specified;
        self.emerg_amps_specified = other.emerg_amps_specified;
        self.cd.obj.copy_typed_ratings_from(
            &other.cd.obj,
            &[super::prop::NORMAMPS, super::prop::EMERGAMPS],
        );
        self.fault_rate = other.fault_rate;
        self.pct_perm = other.pct_perm;
        self.hrs_to_repair = other.hrs_to_repair;
    }

    /// The terminals of the connection, placed as a new element of that shape
    /// has them: one in delta, which keeps the `bus2` the user gave for a later
    /// wye, and two in wye, the second on that `bus2` or else on the ground
    /// nodes of `bus1`, grounded again for the new count when `phases_gained`
    /// (`a_wye_capacitor_or_reactor_has_its_second_terminal_where_a_new_one_has_it`).
    fn set_terminals_for_connection(&mut self, phases_gained: bool) {
        if self.connection == 1 {
            self.cd.set_nterms(1);
        } else if self.cd.nterms != 2 {
            self.cd.set_nterms(2);
            match self.user_bus2.clone() {
                Some(bus) => self.cd.set_bus(2, &bus),
                None => self.ground_bus2(),
            }
        } else if phases_gained && self.user_bus2.is_none() {
            self.ground_bus2();
        }
        self.mark_shunt();
    }

    /// A shunt unless it has two terminals and the user gave `bus2` on another
    /// bus than `bus1` with a phase off the ground node
    /// (`a_bank_moved_by_bus1_keeps_the_bus2_the_user_gave`).
    fn mark_shunt(&mut self) {
        self.is_shunt = self.cd.nterms < 2
            || self.user_bus2.as_deref().is_none_or(|bus2| {
                strip_extension(self.cd.get_bus(1)).eq_ignore_ascii_case(&strip_extension(bus2))
                    || grounds_every_phase(bus2, self.cd.nphases)
            });
    }

    /// Put terminal 2 of a two-terminal element on the ground nodes of `bus1`,
    /// one per phase, a grounded-wye shunt.
    fn ground_bus2(&mut self) {
        if self.cd.nterms == 2 {
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
            self.cd.obj.clear_seq(super::prop::BUS2); // reset for the save function
        }
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
            FAULTRATE => self.fault_rate,
            PCTPERM => self.pct_perm,
            REPAIR => self.hrs_to_repair,
            BASE_FREQ => self.cd.base_frequency,
            _ => unreachable!("Capacitor has no double property {idx}"),
        }
    }
    fn get_rating(&self, idx: usize) -> crate::obj::Rating {
        match idx {
            super::prop::NORMAMPS => self.norm_amps,
            super::prop::EMERGAMPS => self.emerg_amps,
            _ => unreachable!("no rating property {idx}"),
        }
    }
    fn set_rating(&mut self, idx: usize, value: crate::obj::Rating) {
        match idx {
            super::prop::NORMAMPS => self.norm_amps = value,
            super::prop::EMERGAMPS => self.emerg_amps = value,
            _ => unreachable!("no rating property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use super::prop::*;
        match idx {
            KV => self.kvrating = value,
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
        // `MakePosSequence`'s `CMatrix` and closed-delta `Cuf` arms drive `Cuf`
        // the same way (`solve.rs`).
        match idx {
            KVAR => {
                let old = std::mem::take(&mut self.fkvarrating);
                self.fkvarrating = values
                    .iter()
                    .enumerate()
                    .map(|(i, v)| v.unwrap_or_else(|| old.get(i).copied().unwrap_or(0.0)))
                    .collect();
            }
            CUF => {
                let old = std::mem::take(&mut self.fc);
                self.fc = values
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
        if terminal == 2 {
            self.user_bus2 = Some(value.to_ascii_lowercase());
        }
        self.cd.set_bus(terminal, value);
    }
    /// A delta element names the `bus2` the user gave it, which a later wye
    /// connects.
    fn get_bus_name(&self, terminal: usize) -> String {
        match &self.user_bus2 {
            Some(bus) if terminal == 2 && self.cd.nterms < 2 => bus.clone(),
            _ => self.cd.get_bus(terminal).to_string(),
        }
    }

    /// Apply what the property `idx` changes in the shape, the terminals and
    /// the specification of the Capacitor.
    fn side_effects(&mut self, idx: usize, prev_int: i32) {
        use super::prop::*;
        match idx {
            BUS1 => {
                if self.user_bus2.is_none() {
                    self.ground_bus2();
                }
                self.mark_shunt();
            }
            CONN => {
                self.set_terminals_for_connection(false);
                match self.connection {
                    1 => {
                        let nc = if self.cd.nphases == 1 || self.cd.nphases == 2 {
                            self.cd.nphases + 1
                        } else {
                            self.cd.nphases
                        };
                        self.cd.set_shape(nc, 1);
                    }
                    _ => {
                        let np = self.cd.nphases;
                        self.cd.set_shape(np, 0);
                    }
                }
            }
            BUS2 => {
                self.num_term = 2;
                self.mark_shunt();
            }
            PHASES => {
                if self.cd.nphases as i32 != prev_int {
                    let nc =
                        if self.connection == 1 && (self.cd.nphases == 1 || self.cd.nphases == 2) {
                            self.cd.nphases + 1
                        } else {
                            self.cd.nphases
                        };
                    self.cd.set_shape(nc, self.connection);
                    self.cd.yorder = self.cd.nterms * self.cd.nconds;
                    self.set_terminals_for_connection(self.cd.nphases as i32 > prev_int);
                } else if self.connection == 1 && self.cd.nconds != self.cd.nphases + 1 {
                    self.cd.set_nconds(self.cd.nphases + 1);
                    self.cd.yorder = self.cd.nterms * self.cd.nconds;
                }
            }
            KVAR => self.spec_type = CapacitorSpecType::Kvar,
            CMATRIX => self.spec_type = CapacitorSpecType::CMatrix,
            CUF => self.spec_type = CapacitorSpecType::Cuf,
            NUMSTEPS => self.side_effect_numsteps(prev_int),
            XL => {
                // An `XL` that is not finite derives no `R`: the recalc
                // refuses it by name and a later finite `XL` derives `R`
                // (`a_refused_element_is_built_again_once_its_input_is_valid`).
                for i in 0..self.n_steps() {
                    if self.fxl[i].is_finite() && self.fxl[i] != 0.0 && self.fr[i] == 0.0 {
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
}

/// Pascal `StripExtension`: the bus name with its `.node.node…` suffix removed.
fn strip_extension(s: &str) -> String {
    match s.find('.') {
        Some(p) => s[..p].to_string(),
        None => s.to_string(),
    }
}

/// Whether `bus` puts each of the first `nphases` conductors on the ground node.
/// A conductor without a node in `bus` takes its own phase node.
fn grounds_every_phase(bus: &str, nphases: usize) -> bool {
    let mut nodes = bus.split('.').skip(1);
    (0..nphases).all(|_| {
        nodes
            .next()
            .is_some_and(|n| n.trim().parse::<i64>() == Ok(0))
    })
}

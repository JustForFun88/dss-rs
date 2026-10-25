//! The `impl DssObject` property surface, side effects, and `MakeLike`.

use num_complex::Complex64;

use super::{Reactor, ReactorSpecType};
use crate::elements::general::xy_curve::XyCurveObj;
use crate::obj::arena::ResolvedObj;
use crate::obj::base::{DssObjData, DssObject};

impl Reactor {
    /// Pascal `TReactorObj.Z` (the series impedance complex). Read-only accessor
    /// for the CIM export (`SeriesCompensator.r`/`.x`/`.r0`/`.x0`, GAPS_PLAN
    /// WPG.18 Stage D).
    pub fn z(&self) -> Complex64 {
        self.z
    }

    /// Pascal `TDSSCktElement.NormAmps`. Read-only accessor for the CIM export
    /// (`WriteTerminals` operational limits).
    pub fn norm_amps(&self) -> crate::obj::Rating {
        self.norm_amps
    }

    /// Pascal `TDSSCktElement.EmergAmps`. Read-only accessor for the CIM export.
    pub fn emerg_amps(&self) -> crate::obj::Rating {
        self.emerg_amps
    }
}

impl Reactor {
    /// Make this Reactor a copy of `other`, keeping its own name and buses.
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

        self.rp = other.rp;
        self.rp_specified = other.rp_specified;
        self.is_parallel = other.is_parallel;
        self.kvarrating = other.kvarrating;
        self.kvrating = other.kvrating;
        self.connection = other.connection;
        self.spec_type = other.spec_type;
        self.z = other.z;
        self.z1 = other.z1;
        self.z2 = other.z2;
        self.z0 = other.z0;
        self.z2_specified = other.z2_specified;
        self.z0_specified = other.z0_specified;
        self.rmatrix = other.rmatrix.clone();
        self.xmatrix = other.xmatrix.clone();
        self.r_curve_name = other.r_curve_name.clone();
        self.r_curve = other.r_curve.clone();
        self.l_curve_name = other.l_curve_name.clone();
        self.l_curve = other.l_curve.clone();

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

    /// Put terminal 2 of an element with more than one terminal on the ground
    /// nodes of `bus1`, one per phase, a grounded-wye shunt.
    fn ground_bus2(&mut self) {
        if self.cd.nterms > 1 {
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

impl DssObject for Reactor {
    fn data(&self) -> &DssObjData {
        &self.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.cd.obj
    }

    fn get_f64(&self, idx: usize) -> f64 {
        use super::prop::*;
        match idx {
            KVAR => self.kvarrating,
            KV => self.kvrating,
            R => self.z.re,
            X => self.z.im,
            RP => self.rp,
            LMH => self.l,
            FAULTRATE => self.fault_rate,
            PCTPERM => self.pct_perm,
            REPAIR => self.hrs_to_repair,
            BASE_FREQ => self.cd.base_frequency,
            _ => unreachable!("Reactor has no double property {idx}"),
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
            KVAR => self.kvarrating = value,
            KV => self.kvrating = value,
            R => self.z.re = value,
            X => self.z.im = value,
            RP => self.rp = value,
            LMH => self.l = value,
            FAULTRATE => self.fault_rate = value,
            PCTPERM => self.pct_perm = value,
            REPAIR => self.hrs_to_repair = value,
            BASE_FREQ => self.cd.base_frequency = value,
            _ => unreachable!("Reactor has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use super::prop::*;
        match idx {
            PHASES => self.cd.nphases as i32,
            CONN => self.connection,
            _ => unreachable!("Reactor has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use super::prop::*;
        match idx {
            PHASES => self.cd.nphases = value.max(0) as usize,
            CONN => self.connection = value,
            _ => unreachable!("Reactor has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use super::prop::*;
        match idx {
            PARALLEL => self.is_parallel,
            ENABLED => self.cd.enabled,
            _ => unreachable!("Reactor has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use super::prop::*;
        match idx {
            PARALLEL => self.is_parallel = value,
            ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("Reactor has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use super::prop::*;
        match idx {
            RCURVE => self.r_curve_name.clone(),
            LCURVE => self.l_curve_name.clone(),
            _ => unreachable!("Reactor has no string property {idx}"),
        }
    }

    fn get_complex(&self, idx: usize) -> (f64, f64) {
        use super::prop::*;
        let c = match idx {
            Z1 => self.z1,
            Z2 => self.z2,
            Z0 => self.z0,
            Z => self.z,
            _ => unreachable!("Reactor has no complex property {idx}"),
        };
        (c.re, c.im)
    }
    fn set_complex(&mut self, idx: usize, re: f64, im: f64) {
        use super::prop::*;
        let c = Complex64::new(re, im);
        match idx {
            Z1 => self.z1 = c,
            Z2 => self.z2 = c,
            Z0 => self.z0 = c,
            Z => self.z = c,
            _ => unreachable!("Reactor has no complex property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        use super::prop::*;
        match idx {
            RMATRIX => self.rmatrix.as_deref(),
            XMATRIX => self.xmatrix.as_deref(),
            _ => unreachable!("Reactor has no double-array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        use super::prop::*;
        match idx {
            RMATRIX => self.rmatrix = Some(value),
            XMATRIX => self.xmatrix = Some(value),
            _ => unreachable!("Reactor has no double-array property {idx}"),
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

    /// Resolve the `RCurve`/`LCurve` XYcurve references (snapshot-clone, like
    /// the PVSystem/VCCS curve refs).
    fn set_object_ref(&mut self, idx: usize, name: String, resolved: Option<ResolvedObj<'_>>) {
        use super::prop::*;
        let xy_curve = || resolved.and_then(|o| o.cloned::<XyCurveObj>());
        match idx {
            RCURVE => {
                self.r_curve_name = name;
                self.r_curve = xy_curve();
            }
            LCURVE => {
                self.l_curve_name = name;
                self.l_curve = xy_curve();
            }
            _ => unreachable!("Reactor has no resolved object-ref property {idx}"),
        }
    }

    /// Apply what the property `idx` changes in the shape, the terminals and
    /// the specification of the Reactor.
    fn side_effects(&mut self, idx: usize, prev_int: i32) {
        use super::prop::*;
        match idx {
            BUS1 => {
                if self.user_bus2.is_none() {
                    self.ground_bus2();
                }
                self.mark_shunt();
            }
            BUS2 => self.mark_shunt(),
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
            KVAR => self.spec_type = ReactorSpecType::Kvar,
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
            RMATRIX | XMATRIX => self.spec_type = ReactorSpecType::Matrices,
            X => self.spec_type = ReactorSpecType::RplusJx,
            RP => self.rp_specified = true,
            Z1 => {
                // have to set Z1 to get this mode
                self.spec_type = ReactorSpecType::SymComponents;
                if !self.z2_specified {
                    self.z2 = self.z1;
                }
                if !self.z0_specified {
                    self.z0 = self.z1;
                }
            }
            Z2 => self.z2_specified = true,
            Z0 => self.z0_specified = true,
            Z => self.spec_type = ReactorSpecType::RplusJx,
            LMH => {
                self.spec_type = ReactorSpecType::RplusJx;
                self.z.im = self.l * (2.0 * std::f64::consts::PI) * self.cd.base_frequency;
            }
            NORMAMPS => self.norm_amps_specified = true,
            EMERGAMPS => self.emerg_amps_specified = true,
            _ => {}
        }

        // YPrim invalidation on anything that changes the impedance values.
        if matches!(
            idx,
            PHASES
                | KVAR
                | KV
                | CONN
                | RMATRIX
                | XMATRIX
                | PARALLEL
                | R
                | X
                | RP
                | Z1
                | Z2
                | Z0
                | Z
                | LMH
        ) {
            self.cd.yprim_invalid = true;
        }
    }

    /// Pascal base `EndEdit` → `RecalcElementData` (Reactor does not override).
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

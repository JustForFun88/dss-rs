//! Switchable-step bookkeeping (`NumSteps`/`States`/`LastStepInService`) and the
//! [`ControlledCapacitor`] surface CapControl drives (Pascal `TCapacitorObj`
//! switching methods). Step indices are 1-based as in Pascal.

use super::Capacitor;

impl Capacitor {
    /// Per-step switch states (`States[1..NumSteps]`, 0=open/1=closed). Read by
    /// mode-6 monitors (`Meters/Monitor.pas` TakeSample).
    pub fn states(&self) -> &[i32] {
        &self.fstates
    }

    /// Pascal `TCapacitorObj.Totalkvar` (property, `Capacitor.pas:153` = `FTotalkvar`):
    /// the bank's total kvar rating. Read-only accessor for the CIM export
    /// (`LinearShuntCompensator.bPerSection`, GAPS_PLAN WPG.18 Stage D).
    pub fn total_kvar(&self) -> f64 {
        self.ftotalkvar
    }

    /// Pascal `TCapacitorObj.NomKV` (property, `Capacitor.pas:154` = `kvrating`):
    /// the bank's nominal line-to-line kV. Read-only accessor for the CIM export.
    pub fn nom_kv(&self) -> f64 {
        self.kvrating
    }

    /// Pascal `TCapacitorObj.NumSteps` (property, `Capacitor.pas:151` = `FNumSteps`):
    /// the switchable-step count. Read-only accessor for the CIM export.
    pub fn num_steps(&self) -> i32 {
        self.fnumsteps
    }

    /// Pascal `TCapacitorObj.NumTerminals` (property, `Capacitor.pas:157` =
    /// `NumTerm`): the "is this a **series** (2-terminal) capacitor" flag — `2`
    /// only when `bus2` was explicitly defined, else `1` (the constructor default,
    /// a grounded shunt cap). Distinct from the generic `Nterms` (always 2). Used
    /// by the incidence-matrix series-capacitor filter.
    pub fn num_terminals(&self) -> i32 {
        self.num_term
    }

    /// Pascal `TCapacitorObj.Connection` (`FConnection`: `0` = Wye, `1` = Delta).
    /// Read-only accessor for the CIM export (`ShuntConnectionKindNode`).
    pub fn connection(&self) -> i32 {
        self.connection
    }

    /// Pascal `TDSSCktElement.NormAmps`. Read-only accessor for the CIM export
    /// (`WriteTerminals` operational limits).
    pub fn norm_amps(&self) -> f64 {
        self.norm_amps
    }

    /// Pascal `TDSSCktElement.EmergAmps`. Read-only accessor for the CIM export.
    pub fn emerg_amps(&self) -> f64 {
        self.emerg_amps
    }

    /// Number of steps as `usize`.
    pub(super) fn n_steps(&self) -> usize {
        self.fnumsteps.max(0) as usize
    }

    /// Pascal `set_NumSteps`: programmatic setter (ctor/MakeLike). Property edits
    /// write `FNumSteps` directly then call the side effect; this guards against
    /// no-op/invalid values, mirroring the Pascal property setter.
    pub(super) fn set_num_steps(&mut self, value: i32) {
        if value <= 0 || self.fnumsteps == value {
            return;
        }
        let prev = self.fnumsteps;
        self.fnumsteps = value;
        self.side_effect_numsteps(prev);
    }

    /// Pascal `PropertySideEffects(numsteps)`: reallocate the per-step arrays;
    /// when growing a single-step bank into a multi-step one, split the ratings
    /// to keep the same net size and energize every step.
    pub(super) fn side_effect_numsteps(&mut self, prev_int: i32) {
        let n = self.n_steps();
        let mut rstep = 0.0;
        let mut xlstep = 0.0;
        if prev_int == 1 {
            self.ftotalkvar = self.fkvarrating[0];
            rstep = self.fr[0] * self.fnumsteps as f64;
            xlstep = self.fxl[0] * self.fnumsteps as f64;
        }

        // Reallocate arrays (preserve element 0; new slots zero-filled).
        self.fc.resize(n, 0.0);
        self.fxl.resize(n, 0.0);
        self.fkvarrating.resize(n, 0.0);
        self.fr.resize(n, 0.0);
        self.fharm.resize(n, 0.0);
        self.fstates.resize(n, 0);

        if prev_int == 1 {
            match self.spec_type {
                1 => {
                    let step_size = self.ftotalkvar / self.fnumsteps as f64;
                    for v in self.fkvarrating.iter_mut() {
                        *v = step_size;
                    }
                }
                2 => {
                    let c0 = self.fc[0];
                    for v in self.fc.iter_mut().skip(1) {
                        *v = c0;
                    }
                }
                _ => {}
            }
            match self.spec_type {
                1 => {
                    for v in self.fr.iter_mut() {
                        *v = rstep;
                    }
                    for v in self.fxl.iter_mut() {
                        *v = xlstep;
                    }
                }
                2 | 3 => {
                    let (r0, xl0) = (self.fr[0], self.fxl[0]);
                    for v in self.fr.iter_mut().skip(1) {
                        *v = r0;
                    }
                    for v in self.fxl.iter_mut().skip(1) {
                        *v = xl0;
                    }
                }
                _ => {}
            }
            for v in self.fstates.iter_mut() {
                *v = 1; // turn 'em all ON
            }
            self.set_last_step_in_service(self.fnumsteps);
            let h0 = self.fharm[0];
            for v in self.fharm.iter_mut().skip(1) {
                *v = h0; // tune 'em all the same as the first
            }
        }
    }

    /// Pascal `FindLastStepInService`: the highest energized step.
    pub(super) fn find_last_step_in_service(&mut self) {
        self.flast_step_in_service = 0;
        for i in (1..=self.n_steps()).rev() {
            if self.fstates[i - 1] == 1 {
                self.flast_step_in_service = i as i32;
                break;
            }
        }
    }

    /// Pascal `set_LastStepInService`: energize steps `1..=value`, open the rest.
    fn set_last_step_in_service(&mut self, value: i32) {
        let n = self.n_steps();
        let v = value.clamp(0, n as i32) as usize;
        for i in 0..v {
            self.fstates[i] = 1;
        }
        for i in v..n {
            self.fstates[i] = 0;
        }
        if value != self.flast_step_in_service {
            self.cd.yprim_invalid = true;
        }
        self.flast_step_in_service = value;
    }

    /// Pascal `set_States(Idx, Value)` (1-based `idx`): set step `idx` on/off,
    /// invalidating `YPrim` only when the state actually changes (the non-
    /// incremental-Y path).
    fn set_states(&mut self, idx: usize, value: i32) {
        if self.fstates[idx - 1] != value {
            self.fstates[idx - 1] = value;
            self.cd.yprim_invalid = true;
        }
    }

    /// Pascal `TCapacitorObj.AddStep`: energize the next step (starting from the
    /// last step in service); `false` if all steps are already in.
    fn add_step(&mut self) -> bool {
        if self.flast_step_in_service == self.fnumsteps {
            false
        } else {
            self.flast_step_in_service += 1;
            self.set_states(self.flast_step_in_service as usize, 1);
            true
        }
    }

    /// Pascal `TCapacitorObj.SubtractStep`: de-energize the highest step; returns
    /// `false` once the bank is fully open (signals "bank OPEN").
    fn subtract_step(&mut self) -> bool {
        if self.flast_step_in_service == 0 {
            false
        } else {
            self.set_states(self.flast_step_in_service as usize, 0);
            self.flast_step_in_service -= 1;
            self.flast_step_in_service != 0
        }
    }

    /// Pascal `TCapacitorObj.AvailableSteps`.
    fn available_steps(&self) -> i32 {
        self.fnumsteps - self.flast_step_in_service
    }

    /// Pascal `Closed[0]` getter on terminal 1 (`TDSSCktElement.Get_ConductorClosed(0)`):
    /// true iff every phase conductor of terminal 1 is closed.
    fn terminal1_closed(&self) -> bool {
        let t = &self.cd.terminals[0];
        (0..self.cd.nphases).all(|i| t.conductors_closed[i])
    }

    /// Pascal `Closed[0] := value` on terminal 1
    /// (`TDSSCktElement.Set_ConductorClosed(0, value)`): open/close all phase
    /// conductors of terminal 1 and invalidate `YPrim`.
    fn set_terminal1_closed(&mut self, value: bool) {
        let nph = self.cd.nphases;
        let t = &mut self.cd.terminals[0];
        for i in 0..nph {
            t.conductors_closed[i] = value;
        }
        self.cd.yprim_invalid = true;
    }
}

/// The controlled-capacitor surface CapControl's `Sample`/`DoPendingAction`
/// read and mutate (Pascal `TCapacitorObj` switching methods). A trait so the
/// CapControl switching logic is unit-testable against a lightweight mock;
/// [`Capacitor`] is the production implementor. Step indices are 1-based as in
/// Pascal.
pub trait ControlledCapacitor {
    /// `ControlledElement.FullName` for the event log (`Capacitor.<name>`).
    fn full_name(&self) -> String;
    /// `NumSteps`.
    fn num_steps(&self) -> i32;
    /// `AvailableSteps` (`NumSteps − LastStepInService`).
    fn available_steps(&self) -> i32;
    /// `Totalkvar` of the bank.
    fn total_kvar(&self) -> f64;
    /// `Connection` (0 = wye, 1 = delta) — selects the L-L voltage for control.
    fn connection(&self) -> i32;
    /// `Closed[0]`: every phase of terminal 1 closed.
    fn is_closed(&self) -> bool;
    /// `Closed[0] := value`: open/close all phases of terminal 1 (invalidates Y).
    fn set_closed(&mut self, value: bool);
    /// `AddStep`: energize the next step; `false` if all steps already in.
    fn add_step(&mut self) -> bool;
    /// `SubtractStep`: de-energize the highest step; `false` once fully open.
    fn subtract_step(&mut self) -> bool;
}

impl ControlledCapacitor for Capacitor {
    fn full_name(&self) -> String {
        format!("Capacitor.{}", self.cd.obj.name())
    }
    fn num_steps(&self) -> i32 {
        self.fnumsteps
    }
    fn available_steps(&self) -> i32 {
        Capacitor::available_steps(self)
    }
    fn total_kvar(&self) -> f64 {
        self.ftotalkvar
    }
    fn connection(&self) -> i32 {
        self.connection
    }
    fn is_closed(&self) -> bool {
        self.terminal1_closed()
    }
    fn set_closed(&mut self, value: bool) {
        self.set_terminal1_closed(value);
    }
    fn add_step(&mut self) -> bool {
        Capacitor::add_step(self)
    }
    fn subtract_step(&mut self) -> bool {
        Capacitor::subtract_step(self)
    }
}

//! The GenDispatcher control algorithm: building the resolved generator pointer
//! list, the `Sample` redispatch, and the parse-time `RecalcElementData`. Split
//! out of `gen_dispatcher/mod.rs` (no behavioral change).

use super::{GenDispatchEnv, GenDispatcher};

impl GenDispatcher {
    /// Pascal `TGenDispatcherObj.MakeGenList`: build (or rebuild) the resolved
    /// generator pointer list. A named list resolves each entry against the
    /// generator class (keeping only enabled ones, preserving their existing
    /// weights); an empty name list scans every enabled generator and allocates
    /// uniform weights. Returns whether the list ended up non-empty.
    fn make_gen_list(&mut self, env: &dyn GenDispatchEnv) -> bool {
        self.gen_pointer_list.clear();

        if self.list_size > 0 {
            // Name list is defined — use it.
            let mut refs = Vec::with_capacity(self.gen_name_list.len());
            for name in &self.gen_name_list {
                if let Some(g) = env.find_enabled_gen(name) {
                    refs.push(g);
                }
            }
            self.gen_pointer_list = refs;
        } else {
            // Search the entire circuit for enabled generators.
            self.gen_pointer_list = env.all_enabled_gens();
            // Allocate uniform weights.
            self.list_size = self.gen_pointer_list.len() as i32;
            self.weights = vec![1.0; self.list_size as usize];
        }

        // Add up total weights.
        self.total_weight = self
            .weights
            .iter()
            .take(self.list_size.max(0) as usize)
            .sum();

        !self.gen_pointer_list.is_empty()
    }

    /// Pascal `TGenDispatcherObj.Sample`: read the monitored power, and if it is
    /// more than `HalfkWBand` outside `kWLimit` (resp. `kvarLimit`), redispatch
    /// every generator by its weighted share of the deficit. Returns whether any
    /// generator's base changed (the caller then sets `LoadsNeedUpdating` and
    /// pushes a present-time control action, Pascal's `if … then` tail).
    pub(crate) fn sample(&mut self, env: &mut dyn GenDispatchEnv) -> bool {
        // If the list is not defined, make one from all generators in circuit.
        if self.gen_pointer_list.is_empty() {
            self.make_gen_list(env);
        }
        if self.list_size <= 0 {
            return false;
        }

        let s = env.monitored_power(); // power in the active terminal
        let p_diff = s.re * 0.001 - self.f_kw_limit;
        let q_diff = s.im * 0.001 - self.f_kvar_limit;

        let mut changed = false;

        if p_diff.abs() > self.half_kw_band {
            // PDiff is the kW needed to get back into band.
            for (i, &g) in self.gen_pointer_list.iter().enumerate() {
                let cur = env.gen_kw_base(g);
                let gen_kw = (cur + p_diff * (self.weights[i] / self.total_weight)).max(1.0);
                if gen_kw != cur {
                    env.set_gen_kw_base(g, gen_kw);
                    changed = true;
                }
            }
        }

        if q_diff.abs() > self.half_kw_band {
            // QDiff is the kvar needed to get back into band.
            for (i, &g) in self.gen_pointer_list.iter().enumerate() {
                let cur = env.gen_kvar_base(g);
                let gen_kvar = (cur + q_diff * (self.weights[i] / self.total_weight)).max(0.0);
                if gen_kvar != cur {
                    env.set_gen_kvar_base(g, gen_kvar);
                    changed = true;
                }
            }
        }

        changed
    }

    /// Pascal `TGenDispatcherObj.RecalcElementData` (parse-time subset): validate
    /// the monitored element and attach the control's single terminal to the
    /// monitored terminal's bus.
    pub(super) fn recalc(&mut self) {
        let Some(mon) = self.mon_snap.clone() else {
            // Pascal `DoSimpleMsg('Monitored Element in %s is not set', 372)`.
            self.ccd.cd.obj.push_error(crate::diag::DssDiagnostic::msg(
                format!(
                    "Monitored Element in GenDispatcher.{} is not set",
                    self.ccd.cd.obj.name()
                ),
                Some(372),
            ));
            return;
        };

        if self.ccd.element_terminal > mon.nterms as i32 {
            // Pascal `DoErrorMsg(... 'Terminal no. "%d" does not exist.' 371)`.
            self.ccd.cd.obj.push_error(crate::diag::DssDiagnostic::msg(
                format!(
                    "GenDispatcher: \"{}\": Terminal no. \"{}\" does not exist. Re-specify terminal no.",
                    self.ccd.cd.obj.name(),
                    self.ccd.element_terminal
                ),
                Some(371),
            ));
            return;
        }

        // Set the name of the control's 1st terminal's connected bus.
        let t = self.ccd.element_terminal;
        let bus = if t >= 1 && (t as usize) <= mon.buses.len() {
            mon.buses[(t - 1) as usize].clone()
        } else {
            String::new() // Pascal GetBus(i) out of range yields ''
        };
        self.ccd.cd.set_bus(1, &bus);
    }
}

//! `RecalcElementData`, the zone-build write-back (`MakeMeterZoneLists` results
//! installed via `install_zone`), and `AssignVoltBaseRegisterNames`.

use super::{EnergyMeter, MAX_VBASE_COUNT, NUM_EM_REGISTERS, VBASE_START};
use crate::circuit::ckt_tree::CktTree;
use crate::elements::traits::ElemRef;

impl EnergyMeter {
    /// Pascal `RecalcElementData`: validate the metered element (must be a PD
    /// element), check the terminal, and — when the element changed — adopt its
    /// phase/conductor counts, set the meter's bus and throw the branch list
    /// away (it is rebuilt by the next zone reset).
    pub fn recalc(&mut self, errors: &mut crate::diag::ErrorLog) {
        // Pascal `RecalcElementData` clears NeedsRecalc before validating.
        self.needs_recalc = false;
        let Some(snap) = self.metered_snap.clone() else {
            errors.push(format!(
                "EnergyMeter: \"{}\" Circuit Element not set. Element must be defined previously.",
                self.med.cd.obj.name()
            ));
            return;
        };
        if !snap.is_pd {
            errors.push(format!(
                "EnergyMeter: \"{}\" Circuit Element \"{}\" is not a Power Delivery (PD) element. Element must be a PD element.",
                self.med.cd.obj.name(),
                snap.full_name
            ));
            self.med.metered_element = None;
            return;
        }
        if self.med.metered_terminal as usize > snap.nterms {
            errors.push(format!(
                "EnergyMeter: \"{}\" Terminal no. \"{}\" does not exist. Respecify terminal no.",
                self.med.cd.obj.name(),
                self.med.metered_terminal
            ));
            return;
        }
        if self.med.metered_element_changed {
            let bus = snap
                .buses
                .get(self.med.metered_terminal as usize - 1)
                .cloned()
                .unwrap_or_default();
            self.med.cd.set_bus(1, &bus);
            self.med.cd.nphases = snap.nphases;
            self.med.cd.set_nconds(snap.nconds);
            self.med.allocate_sensor_arrays(snap.nphases * snap.nterms);
            self.branch_list = None;
            self.med.metered_element_changed = false;
        }
    }

    /// Pascal `AssignVoltBaseRegisterNames` (l.3082): name the per-voltage-base
    /// loss registers from the accumulated `VBaseList` (in line-to-line kV),
    /// filling unused slots with `Aux<n>`.
    fn assign_volt_base_register_names(&mut self) {
        let sqrt3 = 3.0_f64.sqrt();
        let mut ireg = 1;
        for i in 0..MAX_VBASE_COUNT {
            if self.vbase_list[i] > 0.0 {
                let vbase = self.vbase_list[i] * sqrt3;
                let base = VBASE_START + i; // 0-based slot of "<vbase> kV Losses"
                self.register_names[base] = format!("{} kV Losses", fmt_3g(vbase));
                self.register_names[base + MAX_VBASE_COUNT] =
                    format!("{} kV Line Loss", fmt_3g(vbase));
                self.register_names[base + 2 * MAX_VBASE_COUNT] =
                    format!("{} kV Load Loss", fmt_3g(vbase));
                self.register_names[base + 3 * MAX_VBASE_COUNT] =
                    format!("{} kV No Load Loss", fmt_3g(vbase));
                self.register_names[base + 4 * MAX_VBASE_COUNT] =
                    format!("{} kV Load Energy", fmt_3g(vbase));
            } else {
                for k in 0..5 {
                    self.register_names[VBASE_START + i + k * MAX_VBASE_COUNT] =
                        format!("Aux{ireg}");
                    ireg += 1;
                }
            }
        }
        // Pascal's trailing `Aux` loop spans
        // `1 + VBaseStart + 5·MaxVBaseCount .. NumEMRegisters`, which is empty
        // for the standard register layout (68..=67); kept for fidelity.
        #[allow(clippy::reversed_empty_ranges)]
        for i in (VBASE_START + 5 * MAX_VBASE_COUNT)..NUM_EM_REGISTERS {
            self.register_names[i] = format!("Aux{ireg}");
            ireg += 1;
        }
    }

    // --- Zone-build write-back (driven by `solution::meters`) --------------

    /// Install the freshly-built zone topology and (re)assign the voltage-base
    /// register names. `vbase_list`/`vbase_count` come from the walk's local
    /// `AddToVoltBaseList` accumulator.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn install_zone(
        &mut self,
        branch_list: Option<CktTree>,
        sequence_list: Vec<ElemRef>,
        sequence_nodes: Vec<usize>,
        load_list: Vec<ElemRef>,
        zone_ends: Vec<(ElemRef, usize)>,
        zone_pce: Vec<ElemRef>,
        vbase_list: Vec<f64>,
        vbase_count: usize,
    ) {
        self.branch_list = branch_list;
        self.sequence_list = sequence_list;
        self.sequence_nodes = sequence_nodes;
        self.load_list = load_list;
        self.zone_ends = zone_ends;
        self.zone_pce = zone_pce;
        self.vbase_list = vbase_list;
        self.vbase_count = vbase_count;
        self.assign_volt_base_register_names();
    }
}

/// Pascal `'%.3g'` formatting (used by `AssignVoltBaseRegisterNames`).
fn fmt_3g(v: f64) -> String {
    crate::util::fmt_g(v, 3)
}

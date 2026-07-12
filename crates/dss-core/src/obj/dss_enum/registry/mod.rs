//! `EnumRegistry::new` — the table of enum instances the engine builds in
//! `TDSSContext.Create` (DSSClass.pas). One entry per ported class that
//! references a mapped-string enum; ids are local indices into `enums`.
//!
//! Split by domain (no behavioral change): `new` threads one `push` closure
//! through per-domain `register` helpers — [`pd`], [`pc`], [`control`],
//! [`general`], [`solution`] — each returning the [`EnumId`]s it allocated. The
//! push order (hence the numeric ids) is deliberately irrelevant: ids are local
//! handles stored in named fields, looked up only via [`EnumRegistry::get`].

use super::{DssEnum, EnumId, EnumRegistry};

mod control;
mod general;
mod pc;
mod pd;
mod solution;

impl EnumRegistry {
    pub fn new() -> Self {
        let mut enums = Vec::new();
        let mut push = |e: DssEnum| -> EnumId {
            enums.push(e);
            enums.len() - 1
        };

        // Pascal TDSSContext.Create order is irrelevant here; ids are local, so
        // each domain registers its own enums in any order.
        let general = general::register(&mut push);
        let pd = pd::register(&mut push);
        let pc = pc::register(&mut push);
        let control = control::register(&mut push);
        let solution = solution::register(&mut push);

        Self {
            enums,
            units: general.units,
            earth_model: pd.earth_model,
            scan_type: solution.scan_type,
            sequence: solution.sequence,
            connection: pc.connection,
            vsource_model: pc.vsource_model,
            load_model: pc.load_model,
            load_status: pc.load_status,
            line_type: pd.line_type,
            solve_mode: solution.solve_mode,
            solve_alg: solution.solve_alg,
            control_mode: solution.control_mode,
            random_mode: solution.random_mode,
            default_load_model: solution.default_load_model,
            ckt_model: solution.ckt_model,
            load_shape_class: solution.load_shape_class,
            core_type: pd.core_type,
            lead_lag: pd.lead_lag,
            autotrans_connection: pd.autotrans_connection,
            gic_transformer_type: pd.gic_transformer_type,
            reg_control_phase: control.reg_control_phase,
            mon_phase: control.mon_phase,
            cap_control_type: control.cap_control_type,
            load_shape_action: general.load_shape_action,
            load_shape_interp: general.load_shape_interp,
            t_shape_action: general.t_shape_action,
            price_shape_action: general.price_shape_action,
            gen_disp_mode: pc.gen_disp_mode,
            gen_status: pc.gen_status,
            gen_model: pc.gen_model,
            pvsystem_model: pc.pvsystem_model,
            inv_control_mode: pc.inv_control_mode,
            storage_state: pc.storage_state,
            storage_dispatch_mode: pc.storage_dispatch_mode,
            ind_mach_slip_option: pc.ind_mach_slip_option,
            vsc_mode: pc.vsc_mode,
            upfc_mode: pc.upfc_mode,
            windgen_model: pc.windgen_model,
            windgen_qmode: pc.windgen_qmode,
            monitor_action: general.monitor_action,
            energy_meter_action: general.energy_meter_action,
            storage_ctrl_discharge_mode: control.storage_ctrl_discharge_mode,
            storage_ctrl_charge_mode: control.storage_ctrl_charge_mode,
            swt_control_action: control.swt_control_action,
            swt_control_state: control.swt_control_state,
            fuse_action: control.fuse_action,
            fuse_state: control.fuse_state,
            recloser_action: control.recloser_action,
            recloser_state: control.recloser_state,
            relay_type: control.relay_type,
            relay_action: control.relay_action,
            relay_state: control.relay_state,
            invcontrol_mode: control.invcontrol_mode,
            invcontrol_combi: control.invcontrol_combi,
            invcontrol_voltage_curvex: control.invcontrol_voltage_curvex,
            invcontrol_voltwatt_yaxis: control.invcontrol_voltwatt_yaxis,
            invcontrol_roc: control.invcontrol_roc,
            invcontrol_reac_power: control.invcontrol_reac_power,
            invcontrol_model: control.invcontrol_model,
            espvl_control_type: control.espvl_control_type,
            add_type: solution.add_type,
            dynamic_exp_domain: general.dynamic_exp_domain,
        }
    }

    pub fn get(&self, id: EnumId) -> &DssEnum {
        &self.enums[id]
    }
}

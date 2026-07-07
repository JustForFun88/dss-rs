//! [`Dss`] construction: the class registry built in `Dss::new` (Pascal
//! `TDSSContext.Create`) and `CreateDefaultDSSItems`. Split out of
//! `exec/mod.rs` (no behavioral change).

use super::*;

impl Dss {
    pub fn new() -> Self {
        let enums = EnumRegistry::new();
        let commands = CommandList::new(EXEC_COMMANDS.iter().copied());
        let option_list = CommandList::new(EXEC_OPTIONS.iter().copied());
        let export_commands = CommandList::new(crate::report::EXPORT_OPTIONS.iter().copied());
        let show_commands = CommandList::new(crate::report::SHOW_OPTIONS.iter().copied());

        // Class registry. More classes are registered here as they are ported.
        let classes = vec![
            DssClass::dss_object(tcc_curve::class_props(&enums), |name| {
                Box::new(tcc_curve::TccCurveObj::new(name))
            }),
            DssClass::dss_object(spectrum::class_props(&enums), |name| {
                Box::new(spectrum::SpectrumObj::new(name))
            }),
            DssClass::dss_object(line_code::class_props(&enums), |name| {
                Box::new(line_code::LineCodeObj::new(name))
            }),
            DssClass::dss_object(growth_shape::class_props(), |name| {
                Box::new(growth_shape::GrowthShapeObj::new(name))
            }),
            DssClass::dss_object(xfmr_code::class_props(&enums), |name| {
                Box::new(xfmr_code::XfmrCodeObj::new(name))
            }),
            DssClass::dss_object(xy_curve::class_props(&enums), |name| {
                Box::new(xy_curve::XyCurveObj::new(name))
            }),
            DssClass::dss_object(load_shape::class_props(&enums), |name| {
                Box::new(load_shape::LoadShapeObj::new(name))
            }),
            DssClass::dss_object(temp_shape::class_props(&enums), |name| {
                Box::new(temp_shape::TShapeObj::new(name))
            }),
            DssClass::dss_object(price_shape::class_props(&enums), |name| {
                Box::new(price_shape::PriceShapeObj::new(name))
            }),
            // Conductor catalog (Pascal DSSClassDefs.pas: WireData, CNData,
            // TSData register after Spectrum, before LineGeometry).
            DssClass::dss_object(conductor_data::wire_data::class_props(&enums), |name| {
                Box::new(conductor_data::WireDataObj::new(name))
            }),
            DssClass::dss_object(conductor_data::cn_data::class_props(&enums), |name| {
                Box::new(conductor_data::CnDataObj::new(name))
            }),
            DssClass::dss_object(conductor_data::ts_data::class_props(&enums), |name| {
                Box::new(conductor_data::TsDataObj::new(name))
            }),
            // LineSpacing registers after TSData, before LineGeometry
            // (Pascal DSSClassDefs.pas).
            DssClass::dss_object(line_spacing::class_props(&enums), |name| {
                Box::new(line_spacing::LineSpacingObj::new(name))
            }),
            // LineGeometry registers after LineSpacing (Pascal DSSClassDefs.pas).
            DssClass::dss_object(line_geometry::class_props(&enums), |name| {
                Box::new(line_geometry::LineGeometryObj::new(name))
            }),
            // DynamicExp is a DSS_OBJECT registered before Generator/PVSystem/
            // Storage (Pascal DSSClassDefs.pas:225 — "This needs to be before
            // Generator, PVsystem, Storage"); since the Rust registry groups all
            // DSS_OBJECT classes ahead of the circuit-element classes, placing it
            // here satisfies that ordering. Registration order does not affect node
            // ordering (it adds no nodes).
            DssClass::dss_object(dynamic_exp::class_props(&enums), |name| {
                Box::new(dynamic_exp::DynamicExpObj::new(name))
            }),
            DssClass::ckt_class(
                vsource::class_props(&enums),
                |name| Box::new(vsource::VSource::new(name)),
                ElemKind::Source,
            ),
            DssClass::ckt_class(
                line::class_props(&enums),
                |name| Box::new(line::Line::new(name)),
                ElemKind::Line,
            ),
            DssClass::ckt_class(
                load::class_props(&enums),
                |name| Box::new(load::Load::new(name)),
                ElemKind::Load,
            ),
            DssClass::ckt_class(
                transformer::class_props(&enums),
                |name| Box::new(transformer::Transformer::new(name)),
                ElemKind::Transformer,
            ),
            DssClass::ckt_class(
                capacitor::class_props(&enums),
                |name| Box::new(capacitor::Capacitor::new(name)),
                ElemKind::Capacitor,
            ),
            DssClass::ckt_class(
                reactor::class_props(&enums),
                |name| Box::new(reactor::Reactor::new(name)),
                ElemKind::Reactor,
            ),
            // Fault registers right after Reactor (Pascal DSSClassDefs.pas:222).
            DssClass::ckt_class(
                fault::class_props(&enums),
                |name| Box::new(fault::Fault::new(name)),
                ElemKind::Fault,
            ),
            DssClass::ckt_class(
                reg_control::class_props(&enums),
                |name| Box::new(reg_control::RegControl::new(name)),
                ElemKind::Control,
            ),
            DssClass::ckt_class(
                cap_control::class_props(&enums),
                |name| Box::new(cap_control::CapControl::new(name)),
                ElemKind::Control,
            ),
            DssClass::ckt_class(
                generator::class_props(&enums),
                |name| Box::new(generator::Generator::new(name)),
                ElemKind::Generator,
            ),
            // GenDispatcher is registered right after Generator
            // (Pascal DSSClassDefs.pas:231).
            DssClass::ckt_class(
                gen_dispatcher::class_props(&enums),
                |name| Box::new(gen_dispatcher::GenDispatcher::new(name)),
                ElemKind::Control,
            ),
            // StorageController follows GenDispatcher (Pascal DSSClassDefs.pas:237;
            // the Storage element at :234 is Phase 7, so it is skipped here).
            DssClass::ckt_class(
                storage_controller::class_props(&enums),
                |name| Box::new(storage_controller::StorageController::new(name)),
                ElemKind::Control,
            ),
            // Relay is the most general protection control (Pascal
            // DSSClassDefs.pas:240, before Recloser). Registration order does not
            // affect node ordering, which follows element creation order.
            DssClass::ckt_class(
                relay::class_props(&enums),
                |name| Box::new(relay::Relay::new(name)),
                ElemKind::Control,
            ),
            // Recloser is a protection control on the WP5.7 sweep (Pascal
            // DSSClassDefs.pas:243, before Fuse). Registration order does not
            // affect node ordering, which follows element creation order.
            DssClass::ckt_class(
                recloser::class_props(&enums),
                |name| Box::new(recloser::Recloser::new(name)),
                ElemKind::Control,
            ),
            // Fuse is a TControlElem (joins the control sweep) despite living in
            // the Pascal PDElements tree; registered with the protection controls
            // (Pascal DSSClassDefs.pas:246, after Recloser). Registration order
            // does not affect node ordering, which follows element creation order.
            DssClass::ckt_class(
                fuse::class_props(&enums),
                |name| Box::new(fuse::Fuse::new(name)),
                ElemKind::Control,
            ),
            // SwtControl is the last of the protection controls (Pascal
            // DSSClassDefs.pas:249).
            DssClass::ckt_class(
                swt_control::class_props(&enums),
                |name| Box::new(swt_control::SwtControl::new(name)),
                ElemKind::Control,
            ),
            // Storage (Pascal DSSClassDefs.pas:234 Storage_ELEMENT) and PVSystem
            // (:252 PVSYSTEM_ELEMENT) register after the protection controls,
            // before InvControl. Registration order does not affect node
            // ordering, which follows element creation order.
            DssClass::ckt_class(
                storage::class_props(&enums),
                |name| Box::new(storage::Storage::new(name)),
                ElemKind::Storage,
            ),
            DssClass::ckt_class(
                pvsystem::class_props(&enums),
                |name| Box::new(pvsystem::PVSystem::new(name)),
                ElemKind::PVSystem,
            ),
            // UPFC + UPFCControl register after PVSystem, before IndMach012 (Pascal
            // DSSClassDefs.pas:255/258 UPFC_ELEMENT/UPFC_CONTROL). Registration
            // order does not affect node ordering, which follows element creation
            // order.
            DssClass::ckt_class(
                upfc::class_props(&enums),
                |name| Box::new(upfc::Upfc::new(name)),
                ElemKind::Upfc,
            ),
            DssClass::ckt_class(
                upfc_control::class_props(&enums),
                |name| Box::new(upfc_control::UpfcControl::new(name)),
                ElemKind::Control,
            ),
            // ESPVLControl registers directly after UPFCControl (Pascal
            // DSSClassDefs.pas:261, before IndMach012). Registration order does
            // not affect node ordering, which follows element creation order.
            DssClass::ckt_class(
                espvl_control::class_props(&enums),
                |name| Box::new(espvl_control::EspvlControl::new(name)),
                ElemKind::Control,
            ),
            // IndMach012 registers after PVSystem, before InvControl (Pascal
            // DSSClassDefs.pas:264 INDMACH012_ELEMENT; the GICsource/AutoTrans
            // classes around it are unported). Registration order does not affect
            // node ordering, which follows element creation order.
            DssClass::ckt_class(
                ind_mach012::class_props(&enums),
                |name| Box::new(ind_mach012::IndMach012::new(name)),
                ElemKind::IndMach012,
            ),
            // VSConverter (Pascal DSSClassDefs.pas VS_CONVERTER) — a power-flow
            // AC/DC bridge PC element; no node-order dependence (creation order).
            DssClass::ckt_class(
                vs_converter::class_props(&enums),
                |name| Box::new(vs_converter::VsConverter::new(name)),
                ElemKind::VsConverter,
            ),
            // VCCS (Pascal DSSClassDefs.pas VCCS_ELEMENT) — a voltage-controlled
            // current-source PC element (HW inverter model) with z-filter dynamics;
            // no node-order dependence (creation order).
            DssClass::ckt_class(
                vccs::class_props(&enums),
                |name| Box::new(vccs::Vccs::new(name)),
                ElemKind::Vccs,
            ),
            // InvControl registers after PVSystem (Pascal DSSClassDefs.pas:273;
            // the UPFC/GICsource/AutoTrans classes between PVSystem and InvControl
            // are unported, so among ported classes it follows IndMach012).
            // Registration order does not affect node ordering, which follows
            // element creation order.
            DssClass::ckt_class(
                inv_control::class_props(&enums),
                |name| Box::new(inv_control::InvControl::new(name)),
                ElemKind::Control,
            ),
            // ExpControl registers directly after InvControl (Pascal
            // DSSClassDefs.pas:276). Registration order does not affect node
            // ordering, which follows element creation order.
            DssClass::ckt_class(
                exp_control::class_props(),
                |name| Box::new(exp_control::ExpControl::new(name)),
                ElemKind::Control,
            ),
            // Monitor is registered after Generator (Pascal DSSClassDefs.pas:288).
            DssClass::ckt_class(
                monitor::class_props(&enums),
                |name| Box::new(monitor::Monitor::new(name)),
                ElemKind::Meter,
            ),
            DssClass::ckt_class(
                energymeter::class_props(&enums),
                |name| Box::new(energymeter::EnergyMeter::new(name)),
                ElemKind::EnergyMeter,
            ),
            // Sensor is registered after EnergyMeter (Pascal DSSClassDefs.pas:294).
            DssClass::ckt_class(
                sensor::class_props(&enums),
                |name| Box::new(sensor::Sensor::new(name)),
                ElemKind::Sensor,
            ),
        ];
        let class_by_name = classes
            .iter()
            .enumerate()
            .map(|(i, c)| (c.props.class_name().to_lowercase(), i))
            .collect();

        let mut dss = Self {
            classes,
            class_by_name,
            commands,
            option_list,
            export_commands,
            show_commands,
            parser: Parser::new(),
            aux_parser: Parser::new(),
            vars: ParserVars::new(),
            enums,
            errors: Vec::new(),
            active_class: None,
            active_ckt_element: None,
            last_result: String::new(),
            circuit: None,
            default_base_freq: 60.0,
            default_earth_model: 3, // DERI (Pascal `DSSClass.pas:1284`)
            max_allocation_iterations: 2,
            current_dir: std::env::current_dir().unwrap_or_default(),
            output_directory: std::env::current_dir().unwrap_or_default(),
            last_result_file: String::new(),
            in_redirect: false,
            redirect_abort: false,
            cim: crate::cim::CimExporter::default(),
            dss_objs: Vec::new(),
        };
        dss.create_default_dss_items();
        dss
    }

    /// Pascal `TExecutive.CreateDefaultDSSItems`: the default loadshapes,
    /// growthshapes, spectra and TCC curves every context starts with (created
    /// at context creation and again after `Clear`). The command strings are
    /// verbatim from `Executive.pas`.
    pub(super) fn create_default_dss_items(&mut self) {
        const DEFAULT_ITEMS: &[&str] = &[
            // this load shape used for generator dispatching, etc. Loads may refer to it, also.
            "new loadshape.default npts=24 1.0 mult=(.677 .6256 .6087 .5833 .58028 .6025 .657 .7477 .832 .88 .94 .989 .985 .98 .9898 .999 1 .958 .936 .913 .876 .876 .828 .756)",
            "new growthshape.default 2 year=\"1 20\" mult=(1.025 1.025)", // 20 years at 2.5%
            "new spectrum.default 7  Harmonic=(1 3 5 7 9 11 13)  %mag=(100 33 20 14 11 9 7) Angle=(0 0 0 0 0 0 0)",
            "new spectrum.defaultload 7  Harmonic=(1 3 5 7 9 11 13)  %mag=(100 1.5 20 14 1 9 7) Angle=(0 180 180 180 180 180 180)",
            "new spectrum.defaultgen 7  Harmonic=(1 3 5 7 9 11 13)  %mag=(100 5 3 1.5 1 .7 .5) Angle=(0 0 0 0 0 0 0)",
            "new spectrum.defaultvsource 1  Harmonic=(1 )  %mag=(100 ) Angle=(0 ) ",
            "new spectrum.linear 1  Harmonic=(1 )  %mag=(100 ) Angle=(0 ) ",
            "new spectrum.pwm6 13  Harmonic=(1 3 5 7 9 11 13 15 17 19 21 23 25) %mag=(100 4.4 76.5 62.7 2.9 24.8 12.7 0.5 7.1 8.4 0.9 4.4 3.3) Angle=(-103 -5 28 -180 -33 -59 79 36 -253 -124 3 -30 86)",
            "new spectrum.dc6 10  Harmonic=(1 3 5 7 9 11 13 15 17 19)  %mag=(100 1.2 33.6 1.6 0.4 8.7  1.2  0.3  4.5 1.3) Angle=(-75 28 156 29 -91 49 54 148 -57 -46)",
            "New TCC_Curve.A 5 c_array=(1, 2.5, 4.5, 8.0, 14.)  t_array=(0.15 0.07 .05 .045 .045) ",
            "New TCC_Curve.D 5 c_array=(1, 2.5, 4.5, 8.0, 14.)  t_array=(6 0.7 .2 .06 .02)",
            "New TCC_Curve.TLink 7 c_array=(2 2.1 3 4 6 22 50)  t_array=(300 100 10.1 4.0 1.4 0.1  0.02)",
            "New TCC_Curve.KLink 6 c_array=(2 2.2 3 4 6 30)    t_array=(300 20 4 1.3 0.41 0.02)",
            "New \"TCC_Curve.uv1547\" npts=2 C_array=(0.5, 0.9, ) T_array=(0.166, 2, )",
            "New \"TCC_Curve.ov1547\" npts=2 C_array=(1.1, 1.2, ) T_array=(2, 0.166, )",
            "New \"TCC_Curve.mod_inv\" npts=15 C_array=(1.1, 1.3, 1.5, 2, 3, 4, 5, 6, 7, 8, 9, 10, 20, 50, 100, ) T_array=(27.1053, 9.9029, 6.439, 3.8032, 2.4322, 1.9458, 1.6883, 1.5255, 1.4117, 1.3267, 1.2604, 1.2068, 0.9481, 0.7468, 0.6478, )",
            "New \"TCC_Curve.very_inv\" npts=15 C_array=(1.1, 1.3, 1.5, 2, 3, 4, 5, 6, 7, 8, 9, 10, 20, 50, 100, ) T_array=(93.872, 28.9113, 16.179, 7.0277, 2.9423, 1.7983, 1.3081, 1.0513, 0.8995, 0.8023, 0.7361, 0.6891, 0.5401, 0.4988, 0.493, )",
            "New \"TCC_Curve.ext_inv\" npts=15 C_array=(1.1, 1.3, 1.5, 2, 3, 4, 5, 6, 7, 8, 9, 10, 20, 50, 100, ) T_array=(134.4074, 40.9913, 22.6817, 9.5217, 3.6467, 2.0017, 1.2967, 0.9274, 0.7092, 0.5693, 0.4742, 0.4065, 0.1924, 0.133, 0.1245, )",
            "New \"TCC_Curve.definite\" npts=3 C_array=(1, 1.001, 100, ) T_array=(300, 1, 1, )",
        ];
        for cmd in DEFAULT_ITEMS {
            self.command(cmd);
        }
        debug_assert!(
            self.errors.is_empty(),
            "default DSS items must parse cleanly: {:?}",
            self.errors
        );
    }
}

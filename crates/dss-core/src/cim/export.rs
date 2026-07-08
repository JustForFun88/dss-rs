//! Pascal `TCIMExporter.ExportCDPSM` (`Common/ExportCIMXML.pas:3203-4707`) — the
//! CIM100 XML export control flow. GAPS_PLAN WPG.18 Stage A ports the **entire**
//! flow top-to-bottom (decision 7): the scaffolding (regions/substation/feeder/
//! location, the six `OperationalLimitType`s, the `BaseVoltage`+op-limit-set
//! sweep over `LegalVoltageBases`, the bus → `TopologicalNode`/`ConnectivityNode`
//! sweep, the swing-bus `TopologicalIsland`, the fixed `LoadResponseCharacteristic`
//! catalog, and the closing `OperationalLimitSet`/`CurrentLimit` sweep) plus the
//! **EnergySource** (Vsource) per-object sweep — every other class arm is a
//! scoped [`not_ported_if_any`] error that fires only when the circuit actually
//! contains instances of that not-yet-ported class (never a silent drop; see
//! GAPS_PLAN WPG.18 decision 7 and the prohibition in the WP brief). Combined
//! mode only (`Export CIM100`, `ExportOptions.pas` ptr 21); `Export
//! CIM100Fragments` (ptr 20) is NOT_PORTED at the `exec/report.rs` dispatch
//! until Stage F.

use std::collections::HashMap;

use crate::circuit::Circuit;
use crate::elements::pc::VSource;
use crate::exec::registry::DssClass;

use super::writer::{self, ProfileChoice};
use super::{CimExporter, Uuid, UuidChoice};

/// Pascal `DSSClassDefs.pas`: `SOURCE = 3*8 = 24`, `NON_PCPD_ELEM = 1`; Vsource's
/// class constructor passes `SOURCE or NON_PCPD_ELEM` = 25 as its `DSSObjType`
/// (`Vsource.pas:211`). Oracle-probed 2026-07-09 (`export uuids` after `export
/// cim100`): the terminal hashed key is `"25=<name>=<seq>"`.
const VSOURCE_DSS_OBJ_TYPE: i32 = 25;

/// One entry of the Pascal `TCIMOpLimitObject` list (`ExportCIMXML.pas:65-71`,
/// `806-821`): a per-current-rating `OperationalLimitSet` created on-the-fly by
/// [`write_reference_terminals`] the first time a given `(norm, emerg)` pair is
/// seen, and flushed by the closing sweep (`ExportCIMXML.pas:4658-4680`). Local
/// to one `export_cdpsm` call — Pascal's `OpLimitList`/`OpLimitHash` are always
/// freshly started and freed within the one call (`StartOpLimitList`/
/// `FreeOpLimitList`), unlike the persistent `UuidHash`, so there is nothing to
/// carry on [`CimExporter`] itself.
struct OpLimit {
    uuid: Uuid,
    local_name: String,
    norm_amps: f64,
    emerg_amps: f64,
}

/// Pascal `ActiveCircuit.DSSClassList.Get(ClassNames.Find(name))` narrowed to
/// just the element count (`clsXfCd.ElementCount()`, `clsLnCd.ElementList`
/// walks, …): every not-yet-ported catalog-class guard needs only "is this
/// class populated", never the objects themselves.
fn class_len(classes: &[DssClass], name: &str) -> usize {
    classes
        .iter()
        .find(|c| c.props.class_name().eq_ignore_ascii_case(name))
        .map(|c| c.objects.len())
        .unwrap_or(0)
}

/// GAPS_PLAN WPG.18 decision 7: a scoped, loud `NOT_PORTED` error for one
/// not-yet-ported class arm, firing only when the circuit actually contains
/// instances of it (`count > 0`) — an empty list is Pascal's own no-op, ported
/// as a silent no-op here too. Never a silent drop: the caller still completes
/// every other (ported) section of the file.
fn not_ported_if_any(errors: &mut Vec<String>, count: usize, class_desc: &str, stage: &str) {
    if count > 0 {
        errors.push(format!(
            "Export CIM100: {class_desc} export not ported yet (GAPS_PLAN WPG.18 {stage})."
        ));
    }
}

/// Pascal `TCIMExporterHelper.WritePositions` (`ExportCIMXML.pas:2044`).
#[allow(clippy::too_many_arguments)]
fn write_positions(
    buf: &mut String,
    ckt: &Circuit,
    cim: &mut CimExporter,
    parent_class_name: &str,
    elem_local_name: &str,
    nterm: usize,
    bus_specs: &[String],
    bus_refs: &[usize],
    geo_uuid: Uuid,
    crs_uuid: Uuid,
) {
    writer::start_free_instance(buf, ProfileChoice::Geo, "Location", geo_uuid);
    writer::string_node(
        buf,
        ProfileChoice::Geo,
        "IdentifiedObject.mRID",
        &geo_uuid.to_cim_string(),
    );
    writer::string_node(
        buf,
        ProfileChoice::Geo,
        "IdentifiedObject.name",
        &format!("{elem_local_name}_Loc"),
    );
    writer::ref_node(
        buf,
        ProfileChoice::Geo,
        "Location.CoordinateSystem",
        crs_uuid,
    );
    writer::end_instance(buf, ProfileChoice::Geo, "Location");

    for j in 1..=nterm {
        let bus_spec = &bus_specs[j - 1];
        if writer::is_ground_bus(bus_spec) {
            continue;
        }
        let bus_ref = bus_refs[j - 1];
        let pos_uuid = cim.get_dev_uuid(
            UuidChoice::PosPt,
            &format!("{parent_class_name}.{elem_local_name}"),
            j as i32,
        );
        writer::start_free_instance(buf, ProfileChoice::Geo, "PositionPoint", pos_uuid);
        writer::ref_node(buf, ProfileChoice::Geo, "PositionPoint.Location", geo_uuid);
        writer::integer_node(
            buf,
            ProfileChoice::Geo,
            "PositionPoint.sequenceNumber",
            j as i64,
        );
        writer::string_node(
            buf,
            ProfileChoice::Geo,
            "PositionPoint.xPosition",
            &crate::util::float_to_str(ckt.buses[bus_ref].x),
        );
        writer::string_node(
            buf,
            ProfileChoice::Geo,
            "PositionPoint.yPosition",
            &crate::util::float_to_str(ckt.buses[bus_ref].y),
        );
        writer::end_instance(buf, ProfileChoice::Geo, "PositionPoint");
    }
}

/// Pascal `TCIMExporterHelper.WriteReferenceTerminals` (`ExportCIMXML.pas:2073`).
#[allow(clippy::too_many_arguments)]
fn write_reference_terminals(
    buf: &mut String,
    ckt: &mut Circuit,
    cim: &mut CimExporter,
    op_limits: &mut Vec<OpLimit>,
    op_limit_idx: &mut HashMap<String, usize>,
    dss_obj_type: i32,
    elem_name: &str,
    ref_uuid: Uuid,
    nterm: usize,
    bus_specs: &[String],
    bus_refs: &[usize],
    norm: f64,
    emerg: f64,
) {
    let mut emerg = emerg;
    for j in 1..=nterm {
        let bus_spec = &bus_specs[j - 1];
        if writer::is_ground_bus(bus_spec) {
            continue;
        }
        let bus_ref = bus_refs[j - 1];
        let term_name = format!("{elem_name}_T{j}");
        let term_uuid = cim.get_term_uuid(dss_obj_type, elem_name, j as i32);
        writer::start_free_instance(buf, ProfileChoice::Fun, "Terminal", term_uuid);
        writer::string_node(
            buf,
            ProfileChoice::Fun,
            "IdentifiedObject.mRID",
            &term_uuid.to_cim_string(),
        );
        writer::string_node(buf, ProfileChoice::Fun, "IdentifiedObject.name", &term_name);
        writer::ref_node(
            buf,
            ProfileChoice::Fun,
            "Terminal.ConductingEquipment",
            ref_uuid,
        );
        writer::integer_node(
            buf,
            ProfileChoice::Fun,
            "ACDCTerminal.sequenceNumber",
            j as i64,
        );
        let bus_uuid = crate::cim::get_or_create_uuid(&mut ckt.buses[bus_ref].uuid);
        writer::write_cim_ln(
            buf,
            ProfileChoice::Topo,
            &format!(
                r#"  <cim:Terminal.ConnectivityNode rdf:resource="urn:uuid:{}"/>"#,
                bus_uuid.to_cim_string()
            ),
        );
        if j == 1 && norm > 0.0 {
            if emerg < norm {
                emerg = norm;
            }
            let limit_name = writer::op_lim_i_name(norm, emerg);
            let key = limit_name.to_lowercase();
            let limit_uuid = match op_limit_idx.get(&key) {
                Some(&idx) => op_limits[idx].uuid,
                None => {
                    let uuid = cim.get_dev_uuid(UuidChoice::OpLimI, &limit_name, 0);
                    op_limit_idx.insert(key, op_limits.len());
                    op_limits.push(OpLimit {
                        uuid,
                        local_name: limit_name,
                        norm_amps: norm,
                        emerg_amps: emerg,
                    });
                    uuid
                }
            };
            writer::ref_node(
                buf,
                ProfileChoice::Fun,
                "ACDCTerminal.OperationalLimitSet",
                limit_uuid,
            );
        }
        writer::end_instance(buf, ProfileChoice::Fun, "Terminal");
    }
}

/// Pascal `TCIMExporterHelper.WriteTerminals` (`ExportCIMXML.pas:2117`):
/// `WriteReferenceTerminals(pElem, pElem.UUID, norm, emerg)` +
/// `WritePositions(pElem, geoUUID, crsUUID)`.
#[allow(clippy::too_many_arguments)]
fn write_terminals(
    buf: &mut String,
    ckt: &mut Circuit,
    cim: &mut CimExporter,
    op_limits: &mut Vec<OpLimit>,
    op_limit_idx: &mut HashMap<String, usize>,
    dss_obj_type: i32,
    parent_class_name: &str,
    elem_name: &str,
    elem_uuid: Uuid,
    nterm: usize,
    bus_specs: &[String],
    bus_refs: &[usize],
    geo_uuid: Uuid,
    crs_uuid: Uuid,
    norm: f64,
    emerg: f64,
) {
    write_reference_terminals(
        buf,
        ckt,
        cim,
        op_limits,
        op_limit_idx,
        dss_obj_type,
        elem_name,
        elem_uuid,
        nterm,
        bus_specs,
        bus_refs,
        norm,
        emerg,
    );
    write_positions(
        buf,
        ckt,
        cim,
        parent_class_name,
        elem_name,
        nterm,
        bus_specs,
        bus_refs,
        geo_uuid,
        crs_uuid,
    );
}

/// Pascal `TCIMExporterHelper.WriteLoadModel` (`ExportCIMXML.pas:1998`): one
/// `LoadResponseCharacteristic` instance for the 7 fixed DSS-like load models
/// (`ExportCIMXML.pas:4419-4446` — written **unconditionally**, independent of
/// whether any `Load` references them; oracle-probed 2026-07-09 on a
/// Vsource-only deck).
#[allow(clippy::too_many_arguments)]
fn write_load_model(
    buf: &mut String,
    name: &str,
    id: Uuid,
    z_p: f64,
    i_p: f64,
    p_p: f64,
    z_q: f64,
    i_q: f64,
    p_q: f64,
    e_p: f64,
    e_q: f64,
) {
    let cim_id = id.to_cim_string();
    writer::write_cim_ln(
        buf,
        ProfileChoice::Fun,
        &format!(r#"<cim:LoadResponseCharacteristic rdf:about="urn:uuid:{cim_id}">"#),
    );
    writer::string_node(buf, ProfileChoice::Fun, "IdentifiedObject.mRID", &cim_id);
    writer::string_node(buf, ProfileChoice::Fun, "IdentifiedObject.name", name);
    // Pascal: `exponentModel := (eP <> 0) or (eQ <> 0)`.
    writer::boolean_node(
        buf,
        ProfileChoice::Fun,
        "LoadResponseCharacteristic.exponentModel",
        e_p != 0.0 || e_q != 0.0,
    );
    writer::double_node(
        buf,
        ProfileChoice::Fun,
        "LoadResponseCharacteristic.pConstantImpedance",
        z_p,
    );
    writer::double_node(
        buf,
        ProfileChoice::Fun,
        "LoadResponseCharacteristic.pConstantCurrent",
        i_p,
    );
    writer::double_node(
        buf,
        ProfileChoice::Fun,
        "LoadResponseCharacteristic.pConstantPower",
        p_p,
    );
    writer::double_node(
        buf,
        ProfileChoice::Fun,
        "LoadResponseCharacteristic.qConstantImpedance",
        z_q,
    );
    writer::double_node(
        buf,
        ProfileChoice::Fun,
        "LoadResponseCharacteristic.qConstantCurrent",
        i_q,
    );
    writer::double_node(
        buf,
        ProfileChoice::Fun,
        "LoadResponseCharacteristic.qConstantPower",
        p_q,
    );
    writer::double_node(
        buf,
        ProfileChoice::Fun,
        "LoadResponseCharacteristic.pVoltageExponent",
        e_p,
    );
    writer::double_node(
        buf,
        ProfileChoice::Fun,
        "LoadResponseCharacteristic.qVoltageExponent",
        e_q,
    );
    writer::double_node(
        buf,
        ProfileChoice::Fun,
        "LoadResponseCharacteristic.pFrequencyExponent",
        0.0,
    );
    writer::double_node(
        buf,
        ProfileChoice::Fun,
        "LoadResponseCharacteristic.qFrequencyExponent",
        0.0,
    );
    writer::write_cim_ln(buf, ProfileChoice::Fun, "</cim:LoadResponseCharacteristic>");
}

/// Pascal `TCIMExporter.ExportCDPSM` (`ExportCIMXML.pas:3203`), combined mode
/// (`Combined = TRUE`, `ExportOptions.pas` ptr 21). Returns the full XML text;
/// the caller (`exec/report.rs`) owns writing it to the resolved file path
/// (matching every other `Export` formatter in this codebase). Scoped
/// `NOT_PORTED` errors for not-yet-ported class arms are appended to `errors`
/// (GAPS_PLAN WPG.18 decision 7) — the file is still completed with every
/// ported section present.
#[allow(clippy::too_many_arguments)]
pub(crate) fn export_cdpsm(
    classes: &mut [DssClass],
    ckt: &mut Circuit,
    cim: &mut CimExporter,
    errors: &mut Vec<String>,
    substation: &str,
    sub_geographic_region: &str,
    geographic_region: &str,
    fdr_uuid: Uuid,
    sub_uuid: Uuid,
    sub_geo_uuid: Uuid,
    rgn_uuid: Uuid,
) -> String {
    let sqrt3 = 3.0_f64.sqrt();
    let two_pi = 2.0 * std::f64::consts::PI;

    // `if not assigned(UuidList) then StartUuidList(...)` (`ExportCIMXML.pas:
    // 3302-3307`) — in practice always already started by `DefaultCircuitUUIDs`
    // (`ExportOptions.pas:188`, which every `Export` keyword runs first), ported
    // for completeness.
    if !cim.is_started() {
        let xfmrcode_count = class_len(classes, "xfmrcode");
        let i1 = xfmrcode_count * 6;
        let i2 = ckt.transformers.len() * 11;
        cim.start_uuid_list(i1 + i2);
    }
    // `StartBankList`/`StartECPList`/`StartOpLimitList` (`ExportCIMXML.pas:3308-
    // 3310`): always-fresh, always-freed-at-the-end *local* scratch state (no
    // "if not assigned" guard, unlike UuidList) — modeled as plain Rust locals
    // (`op_limits`/`op_limit_idx` below); Bank/ECP lists arrive with Stage E/B.

    let mut buf = String::new();
    let cim_ver_uuid = cim.get_dev_uuid(UuidChoice::CIMVer, "IEC", 1);
    writer::start_cim_file(&mut buf, ProfileChoice::Fun, cim_ver_uuid);

    let ckt_name = ckt.name.clone();

    // CoordinateSystem (`ExportCIMXML.pas:3316-3322`).
    let crs_uuid = cim.get_dev_uuid(UuidChoice::CoordSys, "Local", 1);
    writer::start_instance(
        &mut buf,
        ProfileChoice::Geo,
        "CoordinateSystem",
        crs_uuid,
        &format!("{ckt_name}_CrsUrn"),
    );
    writer::string_node(
        &mut buf,
        ProfileChoice::Geo,
        "CoordinateSystem.crsUrn",
        "OpenDSSLocalBusCoordinates",
    );
    writer::end_instance(&mut buf, ProfileChoice::Geo, "CoordinateSystem");

    // GeographicalRegion / SubGeographicalRegion / Substation (`3324-3342`).
    writer::start_instance(
        &mut buf,
        ProfileChoice::Fun,
        "GeographicalRegion",
        rgn_uuid,
        geographic_region,
    );
    writer::end_instance(&mut buf, ProfileChoice::Fun, "GeographicalRegion");

    writer::start_instance(
        &mut buf,
        ProfileChoice::Fun,
        "SubGeographicalRegion",
        sub_geo_uuid,
        sub_geographic_region,
    );
    writer::ref_node(
        &mut buf,
        ProfileChoice::Fun,
        "SubGeographicalRegion.Region",
        rgn_uuid,
    );
    writer::end_instance(&mut buf, ProfileChoice::Fun, "SubGeographicalRegion");

    writer::start_instance(
        &mut buf,
        ProfileChoice::Fun,
        "Substation",
        sub_uuid,
        substation,
    );
    writer::ref_node(
        &mut buf,
        ProfileChoice::Fun,
        "Substation.Region",
        sub_geo_uuid,
    );
    writer::end_instance(&mut buf, ProfileChoice::Fun, "Substation");

    // Location + Feeder (`3344-3355`).
    let loc_uuid = cim.get_dev_uuid(UuidChoice::FdrLoc, &ckt_name, 1);
    writer::start_instance(
        &mut buf,
        ProfileChoice::Geo,
        "Location",
        loc_uuid,
        &format!("{ckt_name}_Location"),
    );
    writer::ref_node(
        &mut buf,
        ProfileChoice::Geo,
        "Location.CoordinateSystem",
        crs_uuid,
    );
    writer::end_instance(&mut buf, ProfileChoice::Geo, "Location");

    ckt.uuid = Some(fdr_uuid);
    writer::start_instance(&mut buf, ProfileChoice::Fun, "Feeder", fdr_uuid, &ckt_name);
    writer::ref_node(
        &mut buf,
        ProfileChoice::Fun,
        "Feeder.NormalEnergizingSubstation",
        sub_uuid,
    );
    writer::ref_node(
        &mut buf,
        ProfileChoice::Fun,
        "PowerSystemResource.Location",
        loc_uuid,
    );
    writer::end_instance(&mut buf, ProfileChoice::Fun, "Feeder");

    // The whole system is one topo island; the swing bus name is resolved once
    // the first enabled Vsource is found, below (`3357-3362`).
    let island_uuid = cim.get_dev_uuid(UuidChoice::TopoIsland, "Island", 1);
    let island_name = format!("{ckt_name}_Island");

    // The six fixed OperationalLimitTypes (`3364-3410`).
    let norm_limit_uuid = cim.get_dev_uuid(UuidChoice::OpLimT, "NormalAmps", 1);
    writer::start_instance(
        &mut buf,
        ProfileChoice::Fun,
        "OperationalLimitType",
        norm_limit_uuid,
        &format!("{ckt_name}_NormAmpsType"),
    );
    writer::double_node(
        &mut buf,
        ProfileChoice::Fun,
        "OperationalLimitType.acceptableDuration",
        5.0e9,
    );
    writer::op_limit_direction_enum(&mut buf, ProfileChoice::Fun, "absoluteValue");
    writer::end_instance(&mut buf, ProfileChoice::Fun, "OperationalLimitType");

    let emerg_limit_uuid = cim.get_dev_uuid(UuidChoice::OpLimT, "EmergencyAmps", 1);
    writer::start_instance(
        &mut buf,
        ProfileChoice::Fun,
        "OperationalLimitType",
        emerg_limit_uuid,
        &format!("{ckt_name}_EmergencyAmpsType"),
    );
    writer::double_node(
        &mut buf,
        ProfileChoice::Fun,
        "OperationalLimitType.acceptableDuration",
        2.0 * 3600.0,
    );
    writer::op_limit_direction_enum(&mut buf, ProfileChoice::Fun, "absoluteValue");
    writer::end_instance(&mut buf, ProfileChoice::Fun, "OperationalLimitType");

    let range_a_hi_limit_uuid = cim.get_dev_uuid(UuidChoice::OpLimT, "AHi", 1);
    writer::start_instance(
        &mut buf,
        ProfileChoice::Fun,
        "OperationalLimitType",
        range_a_hi_limit_uuid,
        &format!("{ckt_name}_RangeAHiType"),
    );
    writer::double_node(
        &mut buf,
        ProfileChoice::Fun,
        "OperationalLimitType.acceptableDuration",
        5.0e9,
    );
    writer::op_limit_direction_enum(&mut buf, ProfileChoice::Fun, "high");
    writer::end_instance(&mut buf, ProfileChoice::Fun, "OperationalLimitType");

    let range_a_lo_limit_uuid = cim.get_dev_uuid(UuidChoice::OpLimT, "ALo", 1);
    writer::start_instance(
        &mut buf,
        ProfileChoice::Fun,
        "OperationalLimitType",
        range_a_lo_limit_uuid,
        &format!("{ckt_name}_RangeALoType"),
    );
    writer::double_node(
        &mut buf,
        ProfileChoice::Fun,
        "OperationalLimitType.acceptableDuration",
        5.0e9,
    );
    writer::op_limit_direction_enum(&mut buf, ProfileChoice::Fun, "low");
    writer::end_instance(&mut buf, ProfileChoice::Fun, "OperationalLimitType");

    let range_b_hi_limit_uuid = cim.get_dev_uuid(UuidChoice::OpLimT, "BHi", 1);
    writer::start_instance(
        &mut buf,
        ProfileChoice::Fun,
        "OperationalLimitType",
        range_b_hi_limit_uuid,
        &format!("{ckt_name}_RangeBHiType"),
    );
    writer::double_node(
        &mut buf,
        ProfileChoice::Fun,
        "OperationalLimitType.acceptableDuration",
        24.0 * 3600.0,
    );
    writer::op_limit_direction_enum(&mut buf, ProfileChoice::Fun, "high");
    writer::end_instance(&mut buf, ProfileChoice::Fun, "OperationalLimitType");

    let range_b_lo_limit_uuid = cim.get_dev_uuid(UuidChoice::OpLimT, "BLo", 1);
    writer::start_instance(
        &mut buf,
        ProfileChoice::Fun,
        "OperationalLimitType",
        range_b_lo_limit_uuid,
        &format!("{ckt_name}_RangeBLoType"),
    );
    writer::double_node(
        &mut buf,
        ProfileChoice::Fun,
        "OperationalLimitType.acceptableDuration",
        24.0 * 3600.0,
    );
    writer::op_limit_direction_enum(&mut buf, ProfileChoice::Fun, "low");
    writer::end_instance(&mut buf, ProfileChoice::Fun, "OperationalLimitType");

    // BaseVoltage + OperationalLimitSet + 4 VoltageLimits per LegalVoltageBases
    // entry (`3412-3458`).
    for &kvbase in &ckt.legal_voltage_bases.clone() {
        let s = writer::base_v_name(kvbase);
        let base_v_uuid = cim.get_dev_uuid(UuidChoice::BaseV, &s, 1);
        writer::start_instance(&mut buf, ProfileChoice::Fun, "BaseVoltage", base_v_uuid, &s);
        writer::double_node(
            &mut buf,
            ProfileChoice::Fun,
            "BaseVoltage.nominalVoltage",
            1000.0 * kvbase,
        );
        writer::end_instance(&mut buf, ProfileChoice::Fun, "BaseVoltage");

        let op_lim_v_name = writer::op_lim_v_name(kvbase);
        let op_lim_v_uuid = cim.get_dev_uuid(UuidChoice::OpLimV, &op_lim_v_name, 1);
        writer::start_instance(
            &mut buf,
            ProfileChoice::Fun,
            "OperationalLimitSet",
            op_lim_v_uuid,
            &op_lim_v_name,
        );
        writer::end_instance(&mut buf, ProfileChoice::Fun, "OperationalLimitSet");

        let a_hi_uuid = cim.get_dev_uuid(UuidChoice::OpLimAHi, &s, 1);
        writer::start_instance(
            &mut buf,
            ProfileChoice::Fun,
            "VoltageLimit",
            a_hi_uuid,
            &format!("{op_lim_v_name}_RangeAHi"),
        );
        writer::ref_node(
            &mut buf,
            ProfileChoice::Fun,
            "OperationalLimit.OperationalLimitSet",
            op_lim_v_uuid,
        );
        writer::ref_node(
            &mut buf,
            ProfileChoice::Fun,
            "OperationalLimit.OperationalLimitType",
            range_a_hi_limit_uuid,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Fun,
            "VoltageLimit.value",
            1.05 * 1000.0 * kvbase,
        );
        writer::end_instance(&mut buf, ProfileChoice::Fun, "VoltageLimit");

        let a_lo_uuid = cim.get_dev_uuid(UuidChoice::OpLimALo, &s, 1);
        writer::start_instance(
            &mut buf,
            ProfileChoice::Fun,
            "VoltageLimit",
            a_lo_uuid,
            &format!("{op_lim_v_name}_RangeALo"),
        );
        writer::ref_node(
            &mut buf,
            ProfileChoice::Fun,
            "OperationalLimit.OperationalLimitSet",
            op_lim_v_uuid,
        );
        writer::ref_node(
            &mut buf,
            ProfileChoice::Fun,
            "OperationalLimit.OperationalLimitType",
            range_a_lo_limit_uuid,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Fun,
            "VoltageLimit.value",
            0.95 * 1000.0 * kvbase,
        );
        writer::end_instance(&mut buf, ProfileChoice::Fun, "VoltageLimit");

        let b_hi_uuid = cim.get_dev_uuid(UuidChoice::OpLimBHi, &s, 1);
        writer::start_instance(
            &mut buf,
            ProfileChoice::Fun,
            "VoltageLimit",
            b_hi_uuid,
            &format!("{op_lim_v_name}_RangeBHi"),
        );
        writer::ref_node(
            &mut buf,
            ProfileChoice::Fun,
            "OperationalLimit.OperationalLimitSet",
            op_lim_v_uuid,
        );
        writer::ref_node(
            &mut buf,
            ProfileChoice::Fun,
            "OperationalLimit.OperationalLimitType",
            range_b_hi_limit_uuid,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Fun,
            "VoltageLimit.value",
            1.0583333 * 1000.0 * kvbase,
        );
        writer::end_instance(&mut buf, ProfileChoice::Fun, "VoltageLimit");

        let b_lo_uuid = cim.get_dev_uuid(UuidChoice::OpLimBLo, &s, 1);
        writer::start_instance(
            &mut buf,
            ProfileChoice::Fun,
            "VoltageLimit",
            b_lo_uuid,
            &format!("{op_lim_v_name}_RangeBLo"),
        );
        writer::ref_node(
            &mut buf,
            ProfileChoice::Fun,
            "OperationalLimit.OperationalLimitSet",
            op_lim_v_uuid,
        );
        writer::ref_node(
            &mut buf,
            ProfileChoice::Fun,
            "OperationalLimit.OperationalLimitType",
            range_b_lo_limit_uuid,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Fun,
            "VoltageLimit.value",
            0.9166667 * 1000.0 * kvbase,
        );
        writer::end_instance(&mut buf, ProfileChoice::Fun, "VoltageLimit");
    }

    // Buses -> TopologicalNode/ConnectivityNode (`3460-3483`; the Pascal
    // `Buses[i].localName := BusList.NameOfIndex(i)` assignment is a no-op here
    // — Rust's `Bus.name` is already the same lowercased display name).
    for i in 0..ckt.buses.len() {
        let bus_name = ckt.buses[i].name.clone();
        let geo_uuid = cim.get_dev_uuid(UuidChoice::Topo, &bus_name, 1);
        writer::start_free_instance(&mut buf, ProfileChoice::Topo, "TopologicalNode", geo_uuid);
        writer::string_node(
            &mut buf,
            ProfileChoice::Topo,
            "IdentifiedObject.mRID",
            &geo_uuid.to_cim_string(),
        );
        writer::string_node(
            &mut buf,
            ProfileChoice::Topo,
            "IdentifiedObject.name",
            &bus_name,
        );
        writer::ref_node(
            &mut buf,
            ProfileChoice::Topo,
            "TopologicalNode.TopologicalIsland",
            island_uuid,
        );
        writer::end_instance(&mut buf, ProfileChoice::Topo, "TopologicalNode");

        let bus_uuid = crate::cim::get_or_create_uuid(&mut ckt.buses[i].uuid);
        writer::start_free_instance(&mut buf, ProfileChoice::Topo, "ConnectivityNode", bus_uuid);
        writer::string_node(
            &mut buf,
            ProfileChoice::Topo,
            "IdentifiedObject.mRID",
            &bus_uuid.to_cim_string(),
        );
        writer::string_node(
            &mut buf,
            ProfileChoice::Topo,
            "IdentifiedObject.name",
            &bus_name,
        );
        writer::ref_node(
            &mut buf,
            ProfileChoice::Topo,
            "ConnectivityNode.TopologicalNode",
            geo_uuid,
        );
        let op_lim_v_uuid = cim.get_op_lim_v_uuid(sqrt3 * ckt.buses[i].kv_base);
        writer::ref_node(
            &mut buf,
            ProfileChoice::Topo,
            "ConnectivityNode.OperationalLimitSet",
            op_lim_v_uuid,
        );
        writer::write_cim_ln(
            &mut buf,
            ProfileChoice::Topo,
            &format!(
                r#"  <cim:ConnectivityNode.ConnectivityNodeContainer rdf:resource="urn:uuid:{}"/>"#,
                fdr_uuid.to_cim_string()
            ),
        );
        writer::end_instance(&mut buf, ProfileChoice::Topo, "ConnectivityNode");
    }

    // Swing bus == first enabled Vsource (`3485-3501`).
    for &r in &ckt.sources {
        let obj = &classes[r.cls].objects[r.idx];
        let Some(vsrc) = obj.as_any().downcast_ref::<VSource>() else {
            continue;
        };
        if !vsrc.cd.enabled {
            continue;
        }
        let bus_ref = vsrc.cd.terminals[0].bus_ref;
        let bus_name = ckt.buses[bus_ref].name.clone();
        let swing_geo_uuid = cim.get_dev_uuid(UuidChoice::Topo, &bus_name, 1);
        writer::start_instance(
            &mut buf,
            ProfileChoice::Topo,
            "TopologicalIsland",
            island_uuid,
            &island_name,
        );
        writer::ref_node(
            &mut buf,
            ProfileChoice::Topo,
            "TopologicalIsland.AngleRefTopologicalNode",
            swing_geo_uuid,
        );
        writer::end_instance(&mut buf, ProfileChoice::Topo, "TopologicalIsland");
        break;
    }

    // Generators / PVSystems / StorageElements / IEEE1547 (InvControl+ExpControl)
    // (`3503-3634`) — Stage F.
    not_ported_if_any(errors, ckt.generators.len(), "Generator", "Stage F");
    not_ported_if_any(errors, ckt.pv_systems.len(), "PVSystem", "Stage F");
    not_ported_if_any(errors, ckt.storages.len(), "Storage", "Stage F");
    not_ported_if_any(
        errors,
        class_len(classes, "InvControl") + class_len(classes, "ExpControl"),
        "InvControl/ExpControl (IEEE1547Controller)",
        "Stage F",
    );

    // EnergySource sweep (`3636-3682`) — Stage A.
    for &r in &ckt.sources.clone() {
        let (
            enabled,
            nphases,
            kv_base,
            per_unit,
            angle,
            z_avg_diag,
            z_avg_off_diag,
            elem_name,
            nterm,
            bus_specs,
            bus_refs,
        ) = {
            let obj = &classes[r.cls].objects[r.idx];
            let Some(vsrc) = obj.as_any().downcast_ref::<VSource>() else {
                continue;
            };
            let z = vsrc
                .z
                .as_ref()
                .expect("Vsource.z built by RecalcElementData for a solved circuit");
            (
                vsrc.cd.enabled,
                vsrc.cd.nphases,
                vsrc.kv_base,
                vsrc.per_unit,
                vsrc.angle,
                z.avg_diagonal(),
                z.avg_off_diagonal(),
                vsrc.cd.obj.name().to_string(),
                vsrc.cd.nterms,
                vsrc.cd.bus_names.clone(),
                vsrc.cd
                    .terminals
                    .iter()
                    .map(|t| t.bus_ref)
                    .collect::<Vec<_>>(),
            )
        };
        if !enabled {
            continue;
        }

        let rs = z_avg_diag.re;
        let rm = z_avg_off_diag.re;
        let xs = z_avg_diag.im;
        let xm = z_avg_off_diag.im;
        let v1 = nphases as f64;
        let (r1, x1, r0, x0) = if v1 > 1.0 {
            (rs - rm, xs - xm, rs + (v1 - 1.0) * rm, xs + (v1 - 1.0) * xm)
        } else {
            (rs, xs, rs, xs)
        };

        let vsrc_uuid = classes[r.cls].objects[r.idx].data_mut().uuid();
        let bus_ref0 = bus_refs[0];

        writer::start_instance(
            &mut buf,
            ProfileChoice::Fun,
            "EnergySource",
            vsrc_uuid,
            &elem_name,
        );
        writer::circuit_node(&mut buf, ProfileChoice::Fun, fdr_uuid);
        let vbase_uuid = cim.get_base_v_uuid(sqrt3 * ckt.buses[bus_ref0].kv_base);
        writer::ref_node(
            &mut buf,
            ProfileChoice::Fun,
            "ConductingEquipment.BaseVoltage",
            vbase_uuid,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "EnergySource.nominalVoltage",
            1000.0 * kv_base,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ssh,
            "EnergySource.voltageMagnitude",
            1000.0 * kv_base * per_unit,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ssh,
            "EnergySource.voltageAngle",
            two_pi * angle / 360.0,
        );
        writer::double_node(&mut buf, ProfileChoice::Ep, "EnergySource.r", r1);
        writer::double_node(&mut buf, ProfileChoice::Ep, "EnergySource.x", x1);
        writer::double_node(&mut buf, ProfileChoice::Ep, "EnergySource.r0", r0);
        writer::double_node(&mut buf, ProfileChoice::Ep, "EnergySource.x0", x0);
        let geo_uuid = cim.get_dev_uuid(UuidChoice::SrcLoc, &elem_name, 1);
        writer::ref_node(
            &mut buf,
            ProfileChoice::Geo,
            "PowerSystemResource.Location",
            geo_uuid,
        );
        writer::end_instance(&mut buf, ProfileChoice::Fun, "EnergySource");
        // Pascal leaves `AttachPhases` commented out for EnergySource
        // (`ExportCIMXML.pas:3680`) — no AttachXxxPhases call here.

        // `WriteTerminals(pVsrc, geoUUID, crsUUID)` — no `op_limits` machinery is
        // exercised here (`norm`/`emerg` default to 0.0), but the plumbing is
        // ported in full for the (Stage C+ populated) closing sweep below.
        let mut op_limits: Vec<OpLimit> = Vec::new();
        let mut op_limit_idx: HashMap<String, usize> = HashMap::new();
        write_terminals(
            &mut buf,
            ckt,
            cim,
            &mut op_limits,
            &mut op_limit_idx,
            VSOURCE_DSS_OBJ_TYPE,
            "Vsource",
            &elem_name,
            vsrc_uuid,
            nterm,
            &bus_specs,
            &bus_refs,
            geo_uuid,
            crs_uuid,
            0.0,
            0.0,
        );
        // Stage A's own EnergySource sweep never populates `op_limits` (norm=0
        // above), so nothing from this call needs to reach the closing sweep;
        // Stage C/D/E (Lines/Reactors/Transformers) thread a shared list once
        // their per-object loops replace their `not_ported_if_any` arms.
        debug_assert!(op_limits.is_empty());
    }

    // ShuntCapacitors (`3684-3782`, incl. inline CapControl→RegulatingControl) —
    // Stage D.
    not_ported_if_any(
        errors,
        ckt.shunt_capacitors.len(),
        "Capacitor (ShuntCompensator)",
        "Stage D",
    );

    // Transformers + AutoTransformers + their banks (`3785-4197`) — Stage E.
    not_ported_if_any(
        errors,
        ckt.transformers.len(),
        "Transformer (PowerTransformer)",
        "Stage E",
    );
    not_ported_if_any(
        errors,
        ckt.auto_transformers.len(),
        "AutoTrans (PowerTransformer)",
        "Stage E",
    );

    // RegControls -> RatioTapChanger/TapChangerControl (`4198-4273`) — Stage E.
    not_ported_if_any(
        errors,
        class_len(classes, "RegControl"),
        "RegControl (RatioTapChanger)",
        "Stage E",
    );

    // Series reactors -> SeriesCompensator (`4274-4293`) — Stage D.
    not_ported_if_any(
        errors,
        ckt.reactors.len(),
        "Reactor (SeriesCompensator)",
        "Stage D",
    );

    // Lines/switches -> ACLineSegment/LoadBreakSwitch/Fuse/Breaker/Recloser
    // (`4294-4409`) — Stage C.
    not_ported_if_any(
        errors,
        ckt.lines.len(),
        "Line (ACLineSegment/switch)",
        "Stage C",
    );

    // The 7 fixed DSS-like load models (`4410-4447`) — Stage A, unconditional.
    let id1_const_kva = cim.get_dev_uuid(UuidChoice::LoadResp, "ConstkVA", 1);
    let id2_const_z = cim.get_dev_uuid(UuidChoice::LoadResp, "ConstZ", 1);
    let id3_const_p_quad_q = cim.get_dev_uuid(UuidChoice::LoadResp, "ConstPQuadQ", 1);
    let id4_lin_p_quad_q = cim.get_dev_uuid(UuidChoice::LoadResp, "LinPQuadQ", 1);
    let id5_const_i = cim.get_dev_uuid(UuidChoice::LoadResp, "ConstI", 1);
    let id6_const_p_const_q = cim.get_dev_uuid(UuidChoice::LoadResp, "ConstQ", 1);
    let id7_const_p_const_x = cim.get_dev_uuid(UuidChoice::LoadResp, "ConstX", 1);

    write_load_model(
        &mut buf,
        "Constant kVA",
        id1_const_kva,
        0.0,
        0.0,
        100.0,
        0.0,
        0.0,
        100.0,
        0.0,
        0.0,
    );
    write_load_model(
        &mut buf,
        "Constant Z",
        id2_const_z,
        100.0,
        0.0,
        0.0,
        100.0,
        0.0,
        0.0,
        0.0,
        0.0,
    );
    write_load_model(
        &mut buf,
        "Motor",
        id3_const_p_quad_q,
        0.0,
        0.0,
        100.0,
        100.0,
        0.0,
        0.0,
        0.0,
        0.0,
    );
    write_load_model(
        &mut buf,
        "Mix Motor/Res",
        id4_lin_p_quad_q,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        2.0,
    );
    write_load_model(
        &mut buf,
        "Constant I",
        id5_const_i,
        0.0,
        100.0,
        0.0,
        0.0,
        100.0,
        0.0,
        0.0,
        0.0,
    );
    write_load_model(
        &mut buf,
        "Variable P, Fixed Q",
        id6_const_p_const_q,
        0.0,
        0.0,
        100.0,
        0.0,
        0.0,
        100.0,
        0.0,
        0.0,
    );
    write_load_model(
        &mut buf,
        "Variable P, Fixed X",
        id7_const_p_const_x,
        0.0,
        0.0,
        100.0,
        100.0,
        0.0,
        0.0,
        0.0,
        0.0,
    );

    // EnergyConsumer sweep (`4448-4492`) — Stage B.
    not_ported_if_any(errors, ckt.loads.len(), "Load (EnergyConsumer)", "Stage B");

    // Conductor/cable/geometry catalogs (`4493-4627`) — Stage C.
    not_ported_if_any(
        errors,
        class_len(classes, "linecode"),
        "LineCode (Per-Length impedance)",
        "Stage C",
    );
    not_ported_if_any(
        errors,
        class_len(classes, "wiredata"),
        "WireData (OverheadWireInfo)",
        "Stage C",
    );
    not_ported_if_any(
        errors,
        class_len(classes, "tsdata"),
        "TSData (TapeShieldCableInfo)",
        "Stage C",
    );
    not_ported_if_any(
        errors,
        class_len(classes, "cndata"),
        "CNData (ConcentricNeutralCableInfo)",
        "Stage C",
    );
    not_ported_if_any(
        errors,
        class_len(classes, "linegeometry"),
        "LineGeometry (WireSpacingInfo)",
        "Stage C",
    );
    not_ported_if_any(
        errors,
        class_len(classes, "linespacing"),
        "LineSpacing (WireSpacingInfo)",
        "Stage C",
    );

    // EnergyConnectionProfile sweep (`4628-4656`): populated only by
    // AddLoadECP/AddSolarECP/AddStorageECP/AddGeneratorECP, none of which run
    // yet (Stage B/F) — nothing to iterate today, so this section is a
    // structural no-op (matching Pascal's own `if pECP = NIL then break` on an
    // all-empty list), not a `NOT_PORTED` arm.

    // Closing OperationalLimitSet/CurrentLimit sweep (`4658-4680`) — Stage A
    // plumbing; empty today (nothing above populates `op_limits` — Stage
    // C/D/E's Line/Reactor/Transformer terminals are the only producers via
    // `write_reference_terminals`'s `norm > 0.0` branch).
    let op_limits: Vec<OpLimit> = Vec::new();
    for limit in &op_limits {
        writer::start_instance(
            &mut buf,
            ProfileChoice::Fun,
            "OperationalLimitSet",
            limit.uuid,
            &limit.local_name,
        );
        writer::end_instance(&mut buf, ProfileChoice::Fun, "OperationalLimitSet");

        let norm_uuid = cim.get_dev_uuid(UuidChoice::NormAmps, &limit.local_name, 1);
        writer::start_instance(
            &mut buf,
            ProfileChoice::Fun,
            "CurrentLimit",
            norm_uuid,
            &format!("{}_Norm", limit.local_name),
        );
        writer::ref_node(
            &mut buf,
            ProfileChoice::Fun,
            "OperationalLimit.OperationalLimitSet",
            limit.uuid,
        );
        writer::ref_node(
            &mut buf,
            ProfileChoice::Fun,
            "OperationalLimit.OperationalLimitType",
            norm_limit_uuid,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Fun,
            "CurrentLimit.value",
            limit.norm_amps,
        );
        writer::end_instance(&mut buf, ProfileChoice::Fun, "CurrentLimit");

        let emerg_uuid = cim.get_dev_uuid(UuidChoice::EmergAmps, &limit.local_name, 1);
        writer::start_instance(
            &mut buf,
            ProfileChoice::Fun,
            "CurrentLimit",
            emerg_uuid,
            &format!("{}_Emerg", limit.local_name),
        );
        writer::ref_node(
            &mut buf,
            ProfileChoice::Fun,
            "OperationalLimit.OperationalLimitSet",
            limit.uuid,
        );
        writer::ref_node(
            &mut buf,
            ProfileChoice::Fun,
            "OperationalLimit.OperationalLimitType",
            emerg_limit_uuid,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Fun,
            "CurrentLimit.value",
            limit.emerg_amps,
        );
        writer::end_instance(&mut buf, ProfileChoice::Fun, "CurrentLimit");
    }

    // `FreeUuidList` stays commented out upstream (deferred for UUID export,
    // `ExportCIMXML.pas:4697`) — the persistent hash list on `cim` is untouched.
    // `FreeBankList`/`FreeECPList`/`FreeOpLimitList` are Rust's natural drop of
    // the local scratch state above.

    // `FD_Destroy` (`4752`), combined mode.
    writer::write_cim_ln(&mut buf, ProfileChoice::Fun, "</rdf:RDF>");
    buf
}

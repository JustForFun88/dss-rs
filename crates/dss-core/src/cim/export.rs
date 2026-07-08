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
use crate::elements::pc::load::{Connection, Load, LoadModel};
use crate::exec::registry::DssClass;
use crate::obj::base::DssObject;

use super::writer::{self, ProfileChoice};
use super::{CimExporter, Uuid, UuidChoice};

/// Pascal `DSSClassDefs.pas`: `SOURCE = 3*8 = 24`, `NON_PCPD_ELEM = 1`; Vsource's
/// class constructor passes `SOURCE or NON_PCPD_ELEM` = 25 as its `DSSObjType`
/// (`Vsource.pas:211`). Oracle-probed 2026-07-09 (`export uuids` after `export
/// cim100`): the terminal hashed key is `"25=<name>=<seq>"`.
const VSOURCE_DSS_OBJ_TYPE: i32 = 25;

/// Pascal `DSSClassDefs.pas`: `LOAD_ELEMENT = 7*8 = 56`; `TPCClass.Create` ORs in
/// `PC_ELEMENT = 3` (`PCClass.pas:71`), so a Load's `DSSObjType` is `56 or 3 = 59`
/// — the integer prefix of its `GetTermUuid` key (`"59=<name>=<seq>"`).
const LOAD_DSS_OBJ_TYPE: i32 = 59;

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

/// Pascal `TECPObject` (`ExportCIMXML.pas:74-94`): one `EnergyConnectionProfile`
/// (a distinct set of DSS load-shape/spectrum references), created on first sight
/// of a matching key by `AddLoadECP`/`AddSolarECP`/`AddStorageECP`/
/// `AddGeneratorECP` and flushed by the closing `EnergyConnectionProfile` sweep
/// (`ExportCIMXML.pas:4628-4656`). `connections` collects the UUIDs of every
/// EnergyConnection (Load/PVSystem/Storage/Generator) sharing this profile.
/// Local to one `export_cdpsm` call (Pascal's `ECPList`/`ECPHash` are freshly
/// started and freed within the one call).
struct Ecp {
    uuid: Uuid,
    /// Pascal `pECP.localName` — the `<connType>:<shape…>:<spectrum>` key, also
    /// the instance's `IdentifiedObject.name`.
    local_name: String,
    daily: String,
    duty: String,
    yearly: String,
    growth: String,
    spectrum: String,
    cvr: String,
    tdaily: String,
    tduty: String,
    tyearly: String,
    connections: Vec<Uuid>,
}

/// The `ECPList`/`ECPHash` pair as a keyed, insertion-ordered list (Pascal
/// `AddECP`/`GetECP`, `ExportCIMXML.pas:910-929`): `idx` maps the localName key
/// to the `list` slot so a second element with the same shape/spectrum profile
/// re-uses the existing `Ecp` and merely appends its connection UUID.
#[derive(Default)]
struct EcpList {
    list: Vec<Ecp>,
    idx: HashMap<String, usize>,
}

/// Pascal `TCIMExporterHelper.AddLoadECP` (`ExportCIMXML.pas:1130`): if the load
/// carries any DSS load-shape/growth/CVR/spectrum reference, find-or-create the
/// `EnergyConnectionProfile` keyed on those names and append this load's UUID as
/// one of its EnergyConnections. `spectrum` is the resolved-spectrum name
/// (`NameIfNotNil(obj.SpectrumObj)`, e.g. `defaultload`); it always contributes
/// to the **key** but only becomes the `dssSpectrum` node when non-default.
#[allow(clippy::too_many_arguments)]
fn add_load_ecp(
    ecps: &mut EcpList,
    cim: &mut CimExporter,
    load_uuid: Uuid,
    daily: &str,
    duty: &str,
    growth: &str,
    yearly: &str,
    cvr: &str,
    spectrum: &str,
) {
    // Pascal condition (`1135-1140`): any shape set, or a non-default spectrum
    // (`obj.SpectrumObj <> DSS.SpectrumClass.DefaultLoad`; the load default is
    // named `defaultload`).
    let spectrum_non_default = !spectrum.eq_ignore_ascii_case("defaultload");
    if daily.is_empty()
        && duty.is_empty()
        && growth.is_empty()
        && yearly.is_empty()
        && cvr.is_empty()
        && !spectrum_non_default
    {
        return;
    }
    // `Format('Load:%s:%s:%s:%s:%s:%s', [daily, duty, growth, yearly, cvr,
    // spectrum])` — the spectrum position is `NameIfNotNil(obj.SpectrumObj)`,
    // the resolved name (so the default `defaultload` still appears in the key).
    let key = format!("Load:{daily}:{duty}:{growth}:{yearly}:{cvr}:{spectrum}");
    let slot = match ecps.idx.get(&key) {
        Some(&s) => s,
        None => {
            let uuid = cim.get_dev_uuid(UuidChoice::ECProfile, &key, 0);
            let s = ecps.list.len();
            ecps.list.push(Ecp {
                uuid,
                local_name: key.clone(),
                daily: daily.to_string(),
                duty: duty.to_string(),
                yearly: yearly.to_string(),
                growth: growth.to_string(),
                // Pascal assigns `pECP.spectrum` (the `dssSpectrum` node) only
                // for a non-default spectrum (`1162-1163`); `cvr` is set
                // unconditionally from `NameIfNotNil`.
                spectrum: if spectrum_non_default {
                    spectrum.to_string()
                } else {
                    String::new()
                },
                cvr: cvr.to_string(),
                // Load ECPs never set the PVSystem T-shape fields.
                tdaily: String::new(),
                tduty: String::new(),
                tyearly: String::new(),
                connections: Vec::new(),
            });
            ecps.idx.insert(key, s);
            s
        }
    };
    ecps.list[slot].connections.push(load_uuid);
}

/// Pascal `TCIMExporterHelper.PhaseString` (`ExportCIMXML.pas:491`): the CIM
/// phase letters (`ABC`/`AB`/… or the split-secondary `s1`/`s2`/`s12`) for one
/// terminal, order-insensitive. `phs` is the raw bus-spec string (with its `.N`
/// node suffixes), `nphases`/`bus_kvbase` come from the element + its terminal
/// bus. A bus-spec with no dot ⇒ all phases (`ABC`).
fn phase_string(phs: &str, nphases: usize, bus_kvbase: f64, allow_sec: bool) -> String {
    let mut b_sec = false;
    if allow_sec {
        if nphases == 2 && bus_kvbase < 0.25 {
            b_sec = true;
        }
        if nphases == 1 && bus_kvbase < 0.13 {
            b_sec = true;
        }
    }
    let Some(dot) = phs.find('.') else {
        return "ABC".to_string();
    };
    let phs = &phs[dot + 1..];
    if phs.contains('3') {
        b_sec = false; // a three-phase secondary, not split-phase
    }
    if b_sec {
        if phs.contains('1') {
            let mut val = "s1".to_string();
            if phs.contains('2') {
                val.push('2');
            }
            return val;
        }
        if phs.contains('2') {
            return "s2".to_string();
        }
        return String::new();
    }
    let mut val = String::new();
    if phs.contains('1') {
        val.push('A');
    }
    if phs.contains('2') {
        val.push('B');
    }
    if phs.contains('3') {
        val.push('C');
    }
    if phs.contains('4') {
        val.push('N');
    }
    val
}

/// Pascal free function `DeltaPhaseString` (`ExportCIMXML.pas:638`): the phase
/// pair (or single phase) a delta-connected sub-3-phase element spans, from its
/// terminal-1 bus-spec `.N` ordering. `nphases = 3` (or no dot) ⇒ `ABC`.
fn delta_phase_string(phs: &str, nphases: usize) -> String {
    let dot = phs.find('.');
    if dot.is_none() || nphases == 3 {
        return "ABC".to_string();
    }
    let phs = &phs[dot.unwrap() + 1..];
    if nphases == 1 {
        if phs.contains("1.2") || phs.contains("2.1") {
            "A".to_string()
        } else if phs.contains("2.3") || phs.contains("3.2") {
            "B".to_string()
        } else if phs.contains("1.3") || phs.contains("3.1") {
            "C".to_string()
        } else {
            String::new()
        }
    } else if phs.contains("1.2.3") {
        "AB".to_string()
    } else if phs.contains("1.3.2") {
        "CB".to_string()
    } else if phs.contains("2.1.3") {
        "AC".to_string()
    } else if phs.contains("2.3.1") {
        "BC".to_string()
    } else if phs.contains("3.1.2") {
        "CA".to_string()
    } else if phs.contains("3.2.1") {
        "BA".to_string()
    } else {
        String::new()
    }
}

/// One `EnergyConsumerPhase` instance (the shared body of Pascal
/// `AttachSecondaryPhases` `1736` and the per-phase loop in `AttachLoadPhases`
/// `1790-1801`): a per-phase split of a non-3-phase load, keyed
/// `LoadPhase=<load>_<phs>=1`.
#[allow(clippy::too_many_arguments)]
fn write_energy_consumer_phase(
    buf: &mut String,
    cim: &mut CimExporter,
    load_name: &str,
    load_uuid: Uuid,
    geo_uuid: Uuid,
    phs: &str,
    p: f64,
    q: f64,
) {
    let local_name = format!("{load_name}_{phs}");
    let phase_uuid = cim.get_dev_uuid(UuidChoice::LoadPhase, &local_name, 1);
    writer::start_instance(
        buf,
        ProfileChoice::Fun,
        "EnergyConsumerPhase",
        phase_uuid,
        &local_name,
    );
    writer::phase_kind_node(buf, ProfileChoice::Fun, "EnergyConsumerPhase", phs);
    writer::double_node(buf, ProfileChoice::Ssh, "EnergyConsumerPhase.p", p);
    writer::double_node(buf, ProfileChoice::Ssh, "EnergyConsumerPhase.q", q);
    writer::ref_node(
        buf,
        ProfileChoice::Fun,
        "EnergyConsumerPhase.EnergyConsumer",
        load_uuid,
    );
    writer::ref_node(
        buf,
        ProfileChoice::Geo,
        "PowerSystemResource.Location",
        geo_uuid,
    );
    writer::end_instance(buf, ProfileChoice::Fun, "EnergyConsumerPhase");
}

/// Pascal `TCIMExporterHelper.AttachLoadPhases` (`ExportCIMXML.pas:1749`): write
/// the per-phase `EnergyConsumerPhase` breakdown for a **non-3-phase** load
/// (3-phase loads carry no phase objects). `bus_spec0`/`bus_kvbase0` are the
/// terminal-1 raw bus-spec and its bus base voltage.
#[allow(clippy::too_many_arguments)]
fn attach_load_phases(
    buf: &mut String,
    cim: &mut CimExporter,
    load_name: &str,
    load_uuid: Uuid,
    geo_uuid: Uuid,
    nphases: usize,
    load_class: i32,
    kv_load_base: f64,
    connection: Connection,
    bus_spec0: &str,
    bus_kvbase0: f64,
    kw_base: f64,
    kvar_base: f64,
) {
    if nphases == 3 {
        return;
    }
    // TODO(compat): Pascal's `bAllowSec := (pLoad.LoadClass <= 1)` is a coarse
    // filter for PNNL-taxonomy secondary loads; reproduced verbatim.
    let allow_sec = load_class <= 1;
    let p = 1000.0 * kw_base / nphases as f64;
    let q = 1000.0 * kvar_base / nphases as f64;
    let s = if connection == Connection::Delta {
        delta_phase_string(bus_spec0, nphases)
    } else {
        phase_string(bus_spec0, nphases, bus_kvbase0, allow_sec)
    };
    // Filter out split secondary loads (`1773-1788`): nominal-0.208 kV 2-phase
    // (balanced s1/s2) or nominal-0.12 kV 1-phase.
    if kv_load_base < 0.25 && allow_sec {
        if nphases == 2 {
            write_energy_consumer_phase(buf, cim, load_name, load_uuid, geo_uuid, "s1", p, q);
            write_energy_consumer_phase(buf, cim, load_name, load_uuid, geo_uuid, "s2", p, q);
        } else {
            write_energy_consumer_phase(buf, cim, load_name, load_uuid, geo_uuid, &s, p, q);
        }
        return;
    }
    for phs in s.chars() {
        write_energy_consumer_phase(
            buf,
            cim,
            load_name,
            load_uuid,
            geo_uuid,
            &phs.to_string(),
            p,
            q,
        );
    }
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

    // Shared local scratch (Pascal `OpLimitList`/`OpLimitHash` +
    // `ECPList`/`ECPHash`, started at `3308-3310`, freed at `4698-4700`): the
    // per-(norm,emerg) `OperationalLimitSet`s that `write_reference_terminals`
    // creates on the fly, and the per-shape/spectrum `EnergyConnectionProfile`s
    // that `add_load_ecp` (and, from Stage F, the DER ECP helpers) accumulate.
    // Threaded through every per-object sweep and flushed by the two closing
    // sweeps below.
    let mut op_limits: Vec<OpLimit> = Vec::new();
    let mut op_limit_idx: HashMap<String, usize> = HashMap::new();
    let mut ecps = EcpList::default();

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

        // `WriteTerminals(pVsrc, geoUUID, crsUUID)` — EnergySource passes
        // `norm`/`emerg` = 0.0, so this never creates an `OperationalLimitSet`;
        // the shared `op_limits` is threaded through for the Stage C+ producers.
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

    // EnergyConsumer sweep (`4448-4491`) — Stage B.
    for &r in &ckt.loads.clone() {
        struct LoadSnap {
            enabled: bool,
            load_model: LoadModel,
            kw_base: f64,
            kvar_base: f64,
            num_customers: i32,
            connection: Connection,
            nphases: usize,
            load_class: i32,
            kv_load_base: f64,
            name: String,
            nterm: usize,
            bus_specs: Vec<String>,
            bus_refs: Vec<usize>,
            daily: String,
            duty: String,
            growth: String,
            yearly: String,
            cvr: String,
            spectrum: String,
        }
        let snap = {
            let obj = &classes[r.cls].objects[r.idx];
            let Some(load) = obj.as_any().downcast_ref::<Load>() else {
                continue;
            };
            // `NameIfNotNil(obj.XShapeObj)` — the resolved shape object's
            // (lowercased) name, '' when unset (`AddLoadECP` `1157-1161`).
            let shape_name = |o: Option<&dyn DssObject>| -> String {
                o.map(|s| s.data().name().to_string()).unwrap_or_default()
            };
            LoadSnap {
                enabled: load.cd.enabled,
                load_model: load.load_model,
                kw_base: load.kw_base,
                kvar_base: load.kvar_base,
                num_customers: load.num_customers,
                connection: load.connection,
                nphases: load.cd.nphases,
                load_class: load.load_class,
                kv_load_base: load.kv_load_base,
                name: load.cd.obj.name().to_string(),
                nterm: load.cd.nterms,
                bus_specs: load.cd.bus_names.clone(),
                bus_refs: load.cd.terminals.iter().map(|t| t.bus_ref).collect(),
                daily: shape_name(load.daily_shape_obj.as_ref().map(|o| o as &dyn DssObject)),
                duty: shape_name(load.duty_shape_obj.as_ref().map(|o| o as &dyn DssObject)),
                growth: shape_name(load.growth_shape_obj.as_ref().map(|o| o as &dyn DssObject)),
                yearly: shape_name(load.yearly_shape_obj.as_ref().map(|o| o as &dyn DssObject)),
                cvr: shape_name(load.cvr_shape_obj.as_ref().map(|o| o as &dyn DssObject)),
                // `NameIfNotNil(obj.SpectrumObj)` — the resolved spectrum name
                // (the default `defaultload` still appears in the ECP key).
                spectrum: load.spectrum.clone(),
            }
        };
        if !snap.enabled {
            continue;
        }
        let load_uuid = classes[r.cls].objects[r.idx].data_mut().uuid();

        writer::start_instance(
            &mut buf,
            ProfileChoice::Fun,
            "EnergyConsumer",
            load_uuid,
            &snap.name,
        );
        writer::circuit_node(&mut buf, ProfileChoice::Fun, fdr_uuid);
        // VbaseNode (`2124`): terminal-1 bus base × √3.
        let bus_ref0 = snap.bus_refs[0];
        let vbase_uuid = cim.get_base_v_uuid(sqrt3 * ckt.buses[bus_ref0].kv_base);
        writer::ref_node(
            &mut buf,
            ProfileChoice::Fun,
            "ConductingEquipment.BaseVoltage",
            vbase_uuid,
        );
        // The load-model → LoadResponseCharacteristic map (`4456-4471`); model 8
        // (Zipv) has no CIM arm and writes no `EnergyConsumer.LoadResponse`.
        let load_resp = match snap.load_model {
            LoadModel::ConstPQ => Some(id1_const_kva),
            LoadModel::ConstZ => Some(id2_const_z),
            LoadModel::Motor => Some(id3_const_p_quad_q),
            LoadModel::Cvr => Some(id4_lin_p_quad_q),
            LoadModel::ConstI => Some(id5_const_i),
            LoadModel::ConstPFixedQ => Some(id6_const_p_const_q),
            LoadModel::ConstPFixedX => Some(id7_const_p_const_x),
            LoadModel::Zipv => None,
        };
        if let Some(id) = load_resp {
            writer::ref_node(
                &mut buf,
                ProfileChoice::Fun,
                "EnergyConsumer.LoadResponse",
                id,
            );
        }
        writer::double_node(
            &mut buf,
            ProfileChoice::Ssh,
            "EnergyConsumer.p",
            1000.0 * snap.kw_base,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ssh,
            "EnergyConsumer.q",
            1000.0 * snap.kvar_base,
        );
        writer::integer_node(
            &mut buf,
            ProfileChoice::Fun,
            "EnergyConsumer.customerCount",
            snap.num_customers as i64,
        );
        if snap.connection == Connection::Wye {
            writer::shunt_connection_kind_node(&mut buf, ProfileChoice::Fun, "EnergyConsumer", "Y");
            // TODO(compat): Pascal hard-codes `grounded := TRUE` for wye loads
            // (`4478`, "TODO - check bus 2").
            writer::boolean_node(
                &mut buf,
                ProfileChoice::Fun,
                "EnergyConsumer.grounded",
                true,
            );
        } else {
            writer::shunt_connection_kind_node(&mut buf, ProfileChoice::Fun, "EnergyConsumer", "D");
            writer::boolean_node(
                &mut buf,
                ProfileChoice::Fun,
                "EnergyConsumer.grounded",
                false,
            );
        }
        let geo_uuid = cim.get_dev_uuid(UuidChoice::LoadLoc, &snap.name, 1);
        writer::ref_node(
            &mut buf,
            ProfileChoice::Geo,
            "PowerSystemResource.Location",
            geo_uuid,
        );
        writer::end_instance(&mut buf, ProfileChoice::Fun, "EnergyConsumer");

        let bus_kvbase0 = ckt.buses[bus_ref0].kv_base;
        attach_load_phases(
            &mut buf,
            cim,
            &snap.name,
            load_uuid,
            geo_uuid,
            snap.nphases,
            snap.load_class,
            snap.kv_load_base,
            snap.connection,
            &snap.bus_specs[0],
            bus_kvbase0,
            snap.kw_base,
            snap.kvar_base,
        );
        write_terminals(
            &mut buf,
            ckt,
            cim,
            &mut op_limits,
            &mut op_limit_idx,
            LOAD_DSS_OBJ_TYPE,
            "Load",
            &snap.name,
            load_uuid,
            snap.nterm,
            &snap.bus_specs,
            &snap.bus_refs,
            geo_uuid,
            crs_uuid,
            0.0,
            0.0,
        );
        add_load_ecp(
            &mut ecps,
            cim,
            load_uuid,
            &snap.daily,
            &snap.duty,
            &snap.growth,
            &snap.yearly,
            &snap.cvr,
            &snap.spectrum,
        );
    }

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

    // EnergyConnectionProfile sweep (`4628-4656`) — Stage B: one instance per
    // distinct DSS shape/spectrum profile, populated by `add_load_ecp` above
    // (and, from Stage F, the DER ECP helpers). Each `if pECP.<f> <> ''` guard
    // is a "write only when non-empty" node.
    for ecp in &ecps.list {
        writer::start_instance(
            &mut buf,
            ProfileChoice::Ssh,
            "EnergyConnectionProfile",
            ecp.uuid,
            &ecp.local_name,
        );
        let str_node = |buf: &mut String, node: &str, val: &str| {
            if !val.is_empty() {
                writer::string_node(buf, ProfileChoice::Ssh, node, val);
            }
        };
        str_node(&mut buf, "EnergyConnectionProfile.dssDaily", &ecp.daily);
        str_node(&mut buf, "EnergyConnectionProfile.dssDuty", &ecp.duty);
        str_node(&mut buf, "EnergyConnectionProfile.dssYearly", &ecp.yearly);
        str_node(
            &mut buf,
            "EnergyConnectionProfile.dssLoadGrowth",
            &ecp.growth,
        );
        str_node(
            &mut buf,
            "EnergyConnectionProfile.dssSpectrum",
            &ecp.spectrum,
        );
        str_node(
            &mut buf,
            "EnergyConnectionProfile.dssLoadCvrCurve",
            &ecp.cvr,
        );
        str_node(&mut buf, "EnergyConnectionProfile.dssPVTDaily", &ecp.tdaily);
        str_node(&mut buf, "EnergyConnectionProfile.dssPVTDuty", &ecp.tduty);
        str_node(
            &mut buf,
            "EnergyConnectionProfile.dssPVTYearly",
            &ecp.tyearly,
        );
        for &conn in &ecp.connections {
            writer::ref_node(
                &mut buf,
                ProfileChoice::Ssh,
                "EnergyConnectionProfile.EnergyConnections",
                conn,
            );
        }
        writer::end_instance(&mut buf, ProfileChoice::Ssh, "EnergyConnectionProfile");
    }

    // Closing OperationalLimitSet/CurrentLimit sweep (`4658-4680`): flush the
    // per-(norm,emerg) limits that `write_reference_terminals` created on the
    // fly (Stage C/D/E PD-element terminals are the producers via the
    // `norm > 0.0` branch; EnergySource/EnergyConsumer pass 0.0).
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

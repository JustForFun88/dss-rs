//! Pascal `TCIMExporter.ExportCDPSM` (`Common/ExportCIMXML.pas:3203-4707`) — the
//! CIM100 XML export control flow, ported top-to-bottom (GAPS_PLAN WPG.18
//! decision 7): the scaffolding (regions/substation/feeder/location, the six
//! `OperationalLimitType`s, the `BaseVoltage`+op-limit-set sweep over
//! `LegalVoltageBases`, the bus → `TopologicalNode`/`ConnectivityNode` sweep, the
//! swing-bus `TopologicalIsland`, the fixed `LoadResponseCharacteristic` catalog,
//! and the closing `OperationalLimitSet`/`CurrentLimit` sweep) plus every
//! per-class sweep — EnergySource, DER (Generator/PVSystem/Storage), the IEEE1547
//! controller ([`super::ieee1547`]), capacitors, reactors, lines/switches,
//! transformers/AutoTrans/regulators ([`super::power_xfmr`]), loads, and the
//! conductor/xfmr catalogs. Both output modes run through [`writer::Writer`]:
//! combined (`Export CIM100`, `ExportOptions.pas` ptr 21) and fragments (`Export
//! CIM100Fragments`, ptr 20).

use std::collections::HashMap;

use crate::circuit::Circuit;
use crate::elements::control::CapControl;
use crate::elements::general::conductor_data::{CableGeom, ConductorGeom, ConductorKind};
use crate::elements::general::line_code::{LineCodeObj, prop as lc_prop};
use crate::elements::general::line_geometry::LineGeometryObj;
use crate::elements::general::line_spacing::LineSpacingObj;
use crate::elements::pc::VSource;
use crate::elements::pc::generator::Generator;
use crate::elements::pc::load::{Connection, Load, LoadModel};
use crate::elements::pc::pvsystem::PVSystem;
use crate::elements::pc::storage::Storage;
use crate::elements::pd::capacitor::Capacitor;
use crate::elements::pd::line::Line;
use crate::elements::pd::reactor::Reactor;
use crate::elements::traits::{CktElement, ElemId};
use crate::exec::registry::DssClass;
use crate::obj::base::DssObject;
use crate::support::line_units::LineUnits;

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

/// Pascal `DSSClassDefs.pas`: `LINE_ELEMENT = 6*8 = 48`; `TPDClass.Create` ORs in
/// `PD_ELEMENT = 2` (`PDClass.pas:76`), so a Line's `DSSObjType` is `48 or 2 = 50`
/// — the integer prefix of its `GetTermUuid` key (`"50=<name>=<seq>"`).
const LINE_DSS_OBJ_TYPE: i32 = 50;

/// Pascal `DSSClassDefs.pas`: `CAP_ELEMENT = 13*8 = 104`; `TPDClass.Create` ORs in
/// `PD_ELEMENT = 2`, so a Capacitor's `DSSObjType` is `104 or 2 = 106` — the
/// integer prefix of its `GetTermUuid` key (`"106=<name>=<seq>"`).
const CAP_DSS_OBJ_TYPE: i32 = 106;

/// Pascal `DSSClassDefs.pas`: `REACTOR_ELEMENT = 17*8 = 136`; `TPDClass.Create` ORs
/// in `PD_ELEMENT = 2`, so a Reactor's `DSSObjType` is `136 or 2 = 138`.
const REACTOR_DSS_OBJ_TYPE: i32 = 138;

/// Pascal `DSSClassDefs.pas`: `XFMR_ELEMENT = 4*8 = 32`; `TPDClass.Create` ORs in
/// `PD_ELEMENT = 2`, so a Transformer's `DSSObjType` is `32 or 2 = 34` — the
/// integer prefix of its `GetTermUuid` key (`"34=<name>=<seq>"`).
pub(crate) const XFMR_DSS_OBJ_TYPE: i32 = 34;

/// Pascal `DSSClassDefs.pas`: `AUTOTRANS_ELEMENT = 37*8 = 296`; `TPDClass.Create`
/// ORs in `PD_ELEMENT = 2`, so an AutoTrans's `DSSObjType` is `296 or 2 = 298`.
pub(crate) const AUTOTRANS_DSS_OBJ_TYPE: i32 = 298;

/// Pascal `DSSClassDefs.pas`: `GEN_ELEMENT = 10*8 = 80`; `TPCClass.Create` ORs in
/// `PC_ELEMENT = 3`, so a Generator's `DSSObjType` is `80 or 3 = 83` — the
/// `GetTermUuid` key prefix (`"83=<name>=<seq>"`).
const GEN_DSS_OBJ_TYPE: i32 = 83;

/// Pascal `DSSClassDefs.pas`: `STORAGE_ELEMENT = 21*8 = 168`; `| PC_ELEMENT 3`
/// → `171` (`SetElementNameplate` tests `PC_ELEMENT + STORAGE_ELEMENT`).
const STORAGE_DSS_OBJ_TYPE: i32 = 171;

/// Pascal `DSSClassDefs.pas`: `PVSYSTEM_ELEMENT = 24*8 = 192`; `| PC_ELEMENT 3`
/// → `195` (`SetElementNameplate` tests `PC_ELEMENT + PVSYSTEM_ELEMENT`).
const PVSYSTEM_DSS_OBJ_TYPE: i32 = 195;

/// Pascal `ECapControlType` ordinals (`CapControl.pas:92`), the discriminant the
/// CapControl→RegulatingControl arm switches on. `FOLLOWCONTROL = 5` and
/// `USERCONTROL = 6` have no `RegulatingControlEnum` mode line in the Pascal
/// `case` (`ExportCIMXML.pas:3762-3775`); the Rust `cap_control_type` enum never
/// reaches `USERCONTROL` (no `"user"` string maps to it — safe Rust ports no
/// user-model DLLs), so only 0..=5 are reachable here.
const CAP_CTRL_CURRENT: i32 = 0;
const CAP_CTRL_VOLTAGE: i32 = 1;
const CAP_CTRL_KVAR: i32 = 2;
const CAP_CTRL_TIME: i32 = 3;
const CAP_CTRL_PF: i32 = 4;

/// The Pascal `TDSSCktElement.DSSObjType` integer (`DSSClassDefs.pas`
/// element-type constant OR the PD/PC/NON-PCPD category bits) for a circuit
/// element's class, used as the `GetTermUuid` key prefix (`ExportCIMXML.pas:
/// 1299`: `IntToStr(pElem.DSSObjType)`). The Rust port carries no runtime
/// `DSSObjType` field (the per-object sweeps hard-code their own class constant),
/// so a CapControl's *monitored* element — a generic circuit element resolved
/// only at export time — is mapped from its class name here. Returns `None` for
/// a class not yet covered so the caller can fire a loud error rather than emit a
/// silently-wrong key (Stages E/F extend this table as their classes land).
pub(crate) fn cktelem_dss_obj_type(class_name: &str) -> Option<i32> {
    Some(match class_name.to_ascii_lowercase().as_str() {
        "vsource" => VSOURCE_DSS_OBJ_TYPE,
        "line" => LINE_DSS_OBJ_TYPE,
        "load" => LOAD_DSS_OBJ_TYPE,
        "capacitor" => CAP_DSS_OBJ_TYPE,
        "reactor" => REACTOR_DSS_OBJ_TYPE,
        "transformer" => XFMR_DSS_OBJ_TYPE,
        "autotrans" => AUTOTRANS_DSS_OBJ_TYPE,
        "generator" => GEN_DSS_OBJ_TYPE,
        "storage" => STORAGE_DSS_OBJ_TYPE,
        "pvsystem" => PVSYSTEM_DSS_OBJ_TYPE,
        _ => return None,
    })
}

/// One entry of the Pascal `TCIMOpLimitObject` list (`ExportCIMXML.pas:65-71`,
/// `806-821`): a per-current-rating `OperationalLimitSet` created on-the-fly by
/// [`write_reference_terminals`] the first time a given `(norm, emerg)` pair is
/// seen, and flushed by the closing sweep (`ExportCIMXML.pas:4658-4680`). Local
/// to one `export_cdpsm` call — Pascal's `OpLimitList`/`OpLimitHash` are always
/// freshly started and freed within the one call (`StartOpLimitList`/
/// `FreeOpLimitList`), unlike the persistent `UuidHash`, so there is nothing to
/// carry on [`CimExporter`] itself.
pub(crate) struct OpLimit {
    pub(crate) uuid: Uuid,
    pub(crate) local_name: String,
    pub(crate) norm_amps: f64,
    pub(crate) emerg_amps: f64,
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

/// The shared find-or-create body of the DER `Add*ECP` helpers (`AddSolarECP`
/// `1170`, `AddStorageECP` `1211`, `AddGeneratorECP` `1243`): if `has_ref`
/// (Pascal's "any shape/spectrum set" guard), find-or-create the
/// `EnergyConnectionProfile` for `key` and append `conn_uuid`. `fields` supplies
/// the pre-resolved node values; the T-shape trio is empty for Gen/Storage.
#[allow(clippy::too_many_arguments)]
fn add_der_ecp(
    ecps: &mut EcpList,
    cim: &mut CimExporter,
    conn_uuid: Uuid,
    has_ref: bool,
    key: String,
    daily: &str,
    duty: &str,
    yearly: &str,
    spectrum: &str,
    tdaily: &str,
    tduty: &str,
    tyearly: &str,
) {
    if !has_ref {
        return;
    }
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
                growth: String::new(),
                spectrum: spectrum.to_string(),
                cvr: String::new(),
                tdaily: tdaily.to_string(),
                tduty: tduty.to_string(),
                tyearly: tyearly.to_string(),
                connections: Vec::new(),
            });
            ecps.idx.insert(key, s);
            s
        }
    };
    ecps.list[slot].connections.push(conn_uuid);
}

/// Pascal `TCIMExporterHelper.AddGeneratorECP` (`ExportCIMXML.pas:1243`): keyed
/// `Gen:<daily>:<duty>:<yearly>:<spectrum>`; fires when any shape is set or the
/// spectrum is non-default (`defaultgen`). Like `AddLoadECP`, the spectrum node
/// is suppressed for the default (present in the key, absent in the output).
#[allow(clippy::too_many_arguments)]
fn add_generator_ecp(
    ecps: &mut EcpList,
    cim: &mut CimExporter,
    gen_uuid: Uuid,
    daily: &str,
    duty: &str,
    yearly: &str,
    spectrum: &str,
) {
    let spectrum_non_default = !spectrum.eq_ignore_ascii_case("defaultgen");
    let has_ref =
        !daily.is_empty() || !duty.is_empty() || !yearly.is_empty() || spectrum_non_default;
    let key = format!("Gen:{daily}:{duty}:{yearly}:{spectrum}");
    let spectrum_node = if spectrum_non_default { spectrum } else { "" };
    add_der_ecp(
        ecps,
        cim,
        gen_uuid,
        has_ref,
        key,
        daily,
        duty,
        yearly,
        spectrum_node,
        "",
        "",
        "",
    );
}

/// Pascal `TCIMExporterHelper.AddSolarECP` (`ExportCIMXML.pas:1170`): keyed
/// `PV:<daily>:<duty>:<yearly>:<Tdaily>:<Tduty>:<Tyearly>:<spectrum>`; fires when
/// any of those references is set (the PVSystem spectrum defaults to nil, so
/// `NameIfNotNil` is `''` and it is written verbatim — no default-suppression).
#[allow(clippy::too_many_arguments)]
fn add_solar_ecp(
    ecps: &mut EcpList,
    cim: &mut CimExporter,
    pv_uuid: Uuid,
    daily: &str,
    duty: &str,
    yearly: &str,
    tdaily: &str,
    tduty: &str,
    tyearly: &str,
    spectrum: &str,
) {
    let has_ref = !daily.is_empty()
        || !duty.is_empty()
        || !yearly.is_empty()
        || !tdaily.is_empty()
        || !tduty.is_empty()
        || !tyearly.is_empty()
        || !spectrum.is_empty();
    let key = format!("PV:{daily}:{duty}:{yearly}:{tdaily}:{tduty}:{tyearly}:{spectrum}");
    add_der_ecp(
        ecps, cim, pv_uuid, has_ref, key, daily, duty, yearly, spectrum, tdaily, tduty, tyearly,
    );
}

/// Pascal `TCIMExporterHelper.AddStorageECP` (`ExportCIMXML.pas:1211`): keyed
/// `Bat:<daily>:<duty>:<yearly>:<spectrum>`; fires when any of those is set (the
/// storage spectrum also defaults to nil → `''`, written verbatim).
fn add_storage_ecp(
    ecps: &mut EcpList,
    cim: &mut CimExporter,
    bat_uuid: Uuid,
    daily: &str,
    duty: &str,
    yearly: &str,
    spectrum: &str,
) {
    let has_ref =
        !daily.is_empty() || !duty.is_empty() || !yearly.is_empty() || !spectrum.is_empty();
    let key = format!("Bat:{daily}:{duty}:{yearly}:{spectrum}");
    add_der_ecp(
        ecps, cim, bat_uuid, has_ref, key, daily, duty, yearly, spectrum, "", "", "",
    );
}

/// Pascal `TCIMExporterHelper.PhaseString` (`ExportCIMXML.pas:491`): the CIM
/// phase letters (`ABC`/`AB`/… or the split-secondary `s1`/`s2`/`s12`) for one
/// terminal, order-insensitive. `phs` is the raw bus-spec string (with its `.N`
/// node suffixes), `nphases`/`bus_kvbase` come from the element + its terminal
/// bus. A bus-spec with no dot ⇒ all phases (`ABC`).
pub(crate) fn phase_string(phs: &str, nphases: usize, bus_kvbase: f64, allow_sec: bool) -> String {
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
    buf: &mut writer::Writer,
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
    buf: &mut writer::Writer,
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

/// Pascal `TCIMExporterHelper.AttachCapPhases` (`ExportCIMXML.pas:1703`): the
/// per-phase `LinearShuntCompensatorPhase` breakdown for a **non-3-phase**
/// capacitor (3-phase banks carry no phase objects). `sections` is the SSH
/// `ShuntCompensator.sections` value (in-service step count) the caller already
/// computed; `bus_spec0`/`bus_kvbase0` are the terminal-1 raw bus-spec and its
/// bus base voltage (for `PhaseString`).
#[allow(clippy::too_many_arguments)]
fn attach_cap_phases(
    buf: &mut writer::Writer,
    cim: &mut CimExporter,
    cap_name: &str,
    cap_uuid: Uuid,
    geo_uuid: Uuid,
    nphases: usize,
    total_kvar: f64,
    nom_kv: f64,
    num_steps: i32,
    connection: i32,
    bus_spec0: &str,
    bus_kvbase0: f64,
    sections: f64,
) {
    if nphases == 3 {
        return;
    }
    let bph = 0.001 * total_kvar / nom_kv / nom_kv / num_steps as f64 / nphases as f64;
    // Pascal: `s := PhaseString(pCap, 1)`, overridden by `DeltaPhaseString` when
    // the bank is delta-connected.
    let s = if connection == 1 {
        delta_phase_string(bus_spec0, nphases)
    } else {
        phase_string(bus_spec0, nphases, bus_kvbase0, true)
    };
    for phs in s.chars() {
        let phs = phs.to_string();
        let local_name = format!("{cap_name}_{phs}");
        let phase_uuid = cim.get_dev_uuid(UuidChoice::CapPhase, &local_name, 1);
        writer::start_instance(
            buf,
            ProfileChoice::Fun,
            "LinearShuntCompensatorPhase",
            phase_uuid,
            &local_name,
        );
        writer::phase_kind_node(buf, ProfileChoice::Fun, "ShuntCompensatorPhase", &phs);
        writer::double_node(
            buf,
            ProfileChoice::Ep,
            "LinearShuntCompensatorPhase.bPerSection",
            bph,
        );
        writer::double_node(
            buf,
            ProfileChoice::Ep,
            "LinearShuntCompensatorPhase.gPerSection",
            0.0,
        );
        writer::integer_node(
            buf,
            ProfileChoice::Ep,
            "ShuntCompensatorPhase.normalSections",
            num_steps as i64,
        );
        writer::integer_node(
            buf,
            ProfileChoice::Ep,
            "ShuntCompensatorPhase.maximumSections",
            num_steps as i64,
        );
        writer::double_node(
            buf,
            ProfileChoice::Ssh,
            "ShuntCompensatorPhase.sections",
            sections,
        );
        writer::ref_node(
            buf,
            ProfileChoice::Fun,
            "ShuntCompensatorPhase.ShuntCompensator",
            cap_uuid,
        );
        writer::ref_node(
            buf,
            ProfileChoice::Geo,
            "PowerSystemResource.Location",
            geo_uuid,
        );
        writer::end_instance(buf, ProfileChoice::Fun, "LinearShuntCompensatorPhase");
    }
}

/// Pascal `ActiveCircuit.DSSClassList.Get(ClassNames.Find(name))` narrowed to
/// just the element count (`clsXfCd.ElementCount()`, `clsLnCd.ElementList`
/// walks, …): every not-yet-ported catalog-class guard needs only "is this
/// class populated", never the objects themselves.
pub(crate) fn class_len(classes: &[DssClass], name: &str) -> usize {
    classes
        .iter()
        .find(|c| c.props.class_name().eq_ignore_ascii_case(name))
        .map(|c| c.arena.len())
        .unwrap_or(0)
}

/// One per-phase DER object — the shared body of Pascal
/// `AttachSecondary{Gen,Solar,Storage}Phases` and the per-phase loop of
/// `Attach{Generator,Solar,Storage}Phases` (`ExportCIMXML.pas:1806-1996`).
/// `root` is `SynchronousMachinePhase` (Generator) or
/// `PowerElectronicsConnectionPhase` (PV/Storage); `ref_field` the back-ref node;
/// `uuid_choice` the phase-UUID family (`GenPhase`/`SolarPhase`/`BatteryPhase`).
#[allow(clippy::too_many_arguments)]
fn write_der_phase(
    buf: &mut writer::Writer,
    cim: &mut CimExporter,
    root: &str,
    ref_field: &str,
    uuid_choice: UuidChoice,
    name: &str,
    elem_uuid: Uuid,
    geo_uuid: Uuid,
    phs: &str,
    p: f64,
    q: f64,
) {
    let local_name = format!("{name}_{phs}");
    let phase_uuid = cim.get_dev_uuid(uuid_choice, &local_name, 1);
    writer::start_instance(buf, ProfileChoice::Fun, root, phase_uuid, &local_name);
    writer::phase_kind_node(buf, ProfileChoice::Fun, root, phs);
    writer::double_node(buf, ProfileChoice::Ssh, &format!("{root}.p"), p);
    writer::double_node(buf, ProfileChoice::Ssh, &format!("{root}.q"), q);
    writer::ref_node(buf, ProfileChoice::Fun, ref_field, elem_uuid);
    writer::ref_node(
        buf,
        ProfileChoice::Geo,
        "PowerSystemResource.Location",
        geo_uuid,
    );
    writer::end_instance(buf, ProfileChoice::Fun, root);
}

/// Pascal `Attach{Generator,Solar,Storage}Phases` (`ExportCIMXML.pas:1819/1883/
/// 1947`): the per-phase breakdown for a **non-3-phase** DER (3-phase units carry
/// no phase objects). `s := DeltaPhaseString` when delta, else `PhaseString(_, 1)`
/// (`bAllowSec = TRUE`); a `< 0.25 kV` unit is split-secondary (2-phase → s1/s2,
/// else the single ordered string). `bus_kvbase0` is terminal-1's bus base.
#[allow(clippy::too_many_arguments)]
fn attach_der_phases(
    buf: &mut writer::Writer,
    cim: &mut CimExporter,
    root: &str,
    ref_field: &str,
    uuid_choice: UuidChoice,
    nphases: usize,
    is_delta: bool,
    present_kv: f64,
    present_kw: f64,
    present_kvar: f64,
    bus_spec0: &str,
    bus_kvbase0: f64,
    name: &str,
    elem_uuid: Uuid,
    geo_uuid: Uuid,
) {
    if nphases == 3 {
        return;
    }
    let p = 1000.0 * present_kw / nphases as f64;
    let q = 1000.0 * present_kvar / nphases as f64;
    let s = if is_delta {
        delta_phase_string(bus_spec0, nphases)
    } else {
        phase_string(bus_spec0, nphases, bus_kvbase0, true)
    };
    let phase = |buf: &mut writer::Writer, cim: &mut CimExporter, phs: &str, p, q| {
        write_der_phase(
            buf,
            cim,
            root,
            ref_field,
            uuid_choice,
            name,
            elem_uuid,
            geo_uuid,
            phs,
            p,
            q,
        );
    };
    // `< 0.25 kV` → split secondary (no `bAllowSec`/`LoadClass` gate, unlike loads).
    if present_kv < 0.25 {
        if nphases == 2 {
            phase(buf, cim, "s1", p, q);
            phase(buf, cim, "s2", p, q);
        } else {
            phase(buf, cim, &s, p, q);
        }
        return;
    }
    for c in s.chars() {
        phase(buf, cim, &c.to_string(), p, q);
    }
}

/// Pascal `TCIMExporterHelper.WritePositions` (`ExportCIMXML.pas:2044`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_positions(
    buf: &mut writer::Writer,
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
    buf: &mut writer::Writer,
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
            let key = limit_name.to_ascii_lowercase();
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
    buf: &mut writer::Writer,
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
    buf: &mut writer::Writer,
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

/// The UUID of a **master** named catalog object (`Class.name`) from the class
/// list — the CIM ref must point at the master the catalog sweep writes (whose
/// UUID the fixture preloads by `Class.name`, `exec/uuids_cmd.rs`), never the
/// snapshot clone a `Line` carries (its own separate, un-preloaded UUID slot).
/// Names are stored lowercased; the clone's `name()` matches the master's.
fn class_obj_uuid(classes: &mut [DssClass], class_name: &str, obj_name: &str) -> Option<Uuid> {
    let ci = classes
        .iter()
        .position(|c| c.props.class_name().eq_ignore_ascii_case(class_name))?;
    let oi = classes[ci]
        .arena
        .objs()
        .position(|o| o.data().name().eq_ignore_ascii_case(obj_name))?;
    Some(classes[ci].arena[oi].data_mut().uuid())
}

/// The catalog class name of a conductor snapshot (`WireData`/`CNData`/`TSData`),
/// for resolving its master UUID (`Pascal FetchConductorData` returns the real
/// typed object). `None` for any non-conductor object.
fn conductor_class_name(cond: &dyn DssObject) -> Option<&'static str> {
    match cond.as_conductor()?.conductor_kind() {
        ConductorKind::Wire => Some("WireData"),
        ConductorKind::Cn => Some("CNData"),
        ConductorKind::Ts => Some("TSData"),
    }
}

/// Pascal `TCIMExporterHelper.PhaseOrderString` (`ExportCIMXML.pas:548`): the
/// ordered CIM phase letters for one terminal (the transposition variant used by
/// `AttachLinePhases`/`AttachSwitchPhases`), from the raw bus-spec `.N` ordering.
/// `phs` is the terminal's bus-spec string; `nphases`/`bus_kvbase` its element +
/// bus. No dot ⇒ `ABC`.
pub(crate) fn phase_order_string(
    phs: &str,
    nphases: usize,
    bus_kvbase: f64,
    allow_sec: bool,
) -> String {
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
        // Pascal leaves Result unset here (the compiler default '' ); mirror it.
        return String::new();
    }
    if phs.contains("1.2.3") {
        "ABC".to_string()
    } else if phs.contains("1.3.2") {
        "ACB".to_string()
    } else if phs.contains("2.3.1") {
        "BCA".to_string()
    } else if phs.contains("2.1.3") {
        "BAC".to_string()
    } else if phs.contains("3.2.1") {
        "CBA".to_string()
    } else if phs.contains("3.1.2") {
        "CAB".to_string()
    } else if phs.contains("1.2") {
        "AB".to_string()
    } else if phs.contains("1.3") {
        "AC".to_string()
    } else if phs.contains("2.3") {
        "BC".to_string()
    } else if phs.contains("2.1") {
        "BA".to_string()
    } else if phs.contains("3.2") {
        "CB".to_string()
    } else if phs.contains("3.1") {
        "CA".to_string()
    } else if phs.contains('1') {
        "A".to_string()
    } else if phs.contains('2') {
        "B".to_string()
    } else {
        "C".to_string()
    }
}

/// Pascal `TCIMExporterHelper.ParseSwitchClass` (`ExportCIMXML.pas:451`): pick the
/// CIM switch class + ratings from an attached protective device. Default
/// `LoadBreakSwitch` (ratings = the line's NormAmps); a controlling **Fuse**
/// (priority) → `Fuse` (rated = `RatedCurrent`, breaking = 0); else a **Relay** →
/// `Breaker`; else a **Recloser** → `Recloser` (both keep the default ratings).
/// Controls scanned in `Circuit.controls` (creation order — the same object set
/// as Pascal's per-class `ActiveCircuit.Fuses/Relays/Reclosers` lists).
fn parse_switch_class(
    classes: &mut [DssClass],
    ckt: &Circuit,
    line_ref: crate::elements::traits::ElemId,
    line_norm_amps: f64,
) -> (String, f64, f64) {
    // Does any control of `class` drive this line? Returns the matched control's
    // `ElemId` so a class-specific property can be read afterwards (never reads
    // any property here — Relay/Recloser have no double at Fuse's prop-6 slot, so
    // an unconditional `get_f64(6)` would panic on them; Pascal reads
    // `RatedCurrent` only inside the Fuse branch).
    let controlling = |class: &str| -> Option<crate::elements::traits::ElemId> {
        for &c in &ckt.controls {
            if !classes[c.class_ord()]
                .props
                .class_name()
                .eq_ignore_ascii_case(class)
            {
                continue;
            }
            let controlled = classes[c.class_ord()]
                .arena
                .try_ckt_elem(c.index())
                .and_then(|e| e.controlled_element());
            if controlled == Some(line_ref) {
                return Some(c);
            }
        }
        None
    };
    if let Some(c) = controlling("Fuse") {
        // Fuse wins: rated = RatedCurrent (prop 6), breaking = 0.
        let rated_current = classes[c.class_ord()].arena[c.index()].get_f64(6);
        return ("Fuse".to_string(), rated_current, 0.0);
    }
    if controlling("Relay").is_some() {
        return ("Breaker".to_string(), line_norm_amps, line_norm_amps);
    }
    if controlling("Recloser").is_some() {
        return ("Recloser".to_string(), line_norm_amps, line_norm_amps);
    }
    (
        "LoadBreakSwitch".to_string(),
        line_norm_amps,
        line_norm_amps,
    )
}

/// Pascal `TCIMExporterHelper.WriteWireData` (`ExportCIMXML.pas:2277`). The
/// `ConductorMaterialEnum` call is a no-op upstream (the writer is commented
/// out, `ExportCIMXML.pas:1464`), so no material node is emitted. `class_name`
/// is the object's `DSSClassName` (`WireData`/`CNData`/`TSData`), `norm_amps`
/// its `NormAmps`.
fn write_wire_data(
    buf: &mut writer::Writer,
    class_name: &str,
    name: &str,
    geom: &ConductorGeom,
    norm_amps: f64,
) {
    // DisplayName is never populated (no field), so the else branch always fires.
    writer::string_node(
        buf,
        ProfileChoice::Cat,
        "WireInfo.sizeDescription",
        &format!("{class_name}_{name}"),
    );
    let v1 = LineUnits::from_code(geom.gmr_units).to_meters();
    writer::double_node(buf, ProfileChoice::Cat, "WireInfo.gmr", geom.gmr * v1);
    let v1 = LineUnits::from_code(geom.radius_units).to_meters();
    writer::double_node(buf, ProfileChoice::Cat, "WireInfo.radius", geom.radius * v1);
    let v1 = LineUnits::from_code(geom.res_units).to_per_meter();
    writer::double_node(buf, ProfileChoice::Cat, "WireInfo.rDC20", geom.rdc * v1);
    writer::double_node(buf, ProfileChoice::Cat, "WireInfo.rAC25", geom.rac * v1);
    writer::double_node(buf, ProfileChoice::Cat, "WireInfo.rAC50", geom.rac * v1);
    writer::double_node(buf, ProfileChoice::Cat, "WireInfo.rAC75", geom.rac * v1);
    writer::double_node(
        buf,
        ProfileChoice::Cat,
        "WireInfo.ratedCurrent",
        norm_amps.max(0.0),
    );
    writer::integer_node(buf, ProfileChoice::Cat, "WireInfo.strandCount", 0);
    writer::integer_node(buf, ProfileChoice::Cat, "WireInfo.coreStrandCount", 0);
    writer::double_node(buf, ProfileChoice::Cat, "WireInfo.coreRadius", 0.0);
}

/// Pascal `TCIMExporterHelper.WriteCableData` (`ExportCIMXML.pas:2232`). The
/// `CableOuterJacketEnum`/`CableConstructionEnum` calls are no-ops upstream
/// (commented out). `eps_r`/`ins_layer`/`dia_ins`/`dia_cable` are the shared
/// `TCableData` fields, `radius_units` the `RadiusUnits`.
fn write_cable_data(
    buf: &mut writer::Writer,
    radius_units: i32,
    eps_r: f64,
    ins_layer: f64,
    dia_ins: f64,
    dia_cable: f64,
) {
    let v1 = LineUnits::from_code(radius_units).to_meters();
    writer::boolean_node(buf, ProfileChoice::Cat, "WireInfo.insulated", true);
    writer::double_node(
        buf,
        ProfileChoice::Cat,
        "WireInfo.insulationThickness",
        v1 * ins_layer,
    );
    writer::conductor_insulation_enum(buf, ProfileChoice::Cat, "crosslinkedPolyethylene");
    writer::boolean_node(buf, ProfileChoice::Cat, "CableInfo.isStrandFill", false);
    writer::double_node(
        buf,
        ProfileChoice::Cat,
        "CableInfo.diameterOverCore",
        v1 * (dia_ins - 2.0 * ins_layer),
    );
    writer::double_node(
        buf,
        ProfileChoice::Cat,
        "CableInfo.diameterOverInsulation",
        v1 * dia_ins,
    );
    writer::double_node(
        buf,
        ProfileChoice::Cat,
        "CableInfo.diameterOverJacket",
        v1 * dia_cable,
    );
    writer::double_node(
        buf,
        ProfileChoice::Cat,
        "CableInfo.nominalTemperature",
        90.0,
    );
    writer::double_node(
        buf,
        ProfileChoice::Cat,
        "CableInfo.relativePermittivity",
        eps_r,
    );
}

/// Pascal `TCIMExporterHelper.WriteTapeData` (`ExportCIMXML.pas:2250`). The
/// `CableShieldMaterialEnum` call is a no-op upstream (commented out).
fn write_tape_data(
    buf: &mut writer::Writer,
    radius_units: i32,
    dia_shield: f64,
    tape_layer: f64,
    tape_lap: f64,
) {
    let v1 = LineUnits::from_code(radius_units).to_meters();
    writer::double_node(
        buf,
        ProfileChoice::Cat,
        "CableInfo.diameterOverScreen",
        v1 * (dia_shield - 2.0 * tape_layer),
    );
    writer::double_node(
        buf,
        ProfileChoice::Cat,
        "TapeShieldCableInfo.tapeLap",
        tape_lap,
    );
    writer::double_node(
        buf,
        ProfileChoice::Cat,
        "TapeShieldCableInfo.tapeThickness",
        v1 * tape_layer,
    );
    writer::boolean_node(buf, ProfileChoice::Cat, "CableInfo.sheathAsNeutral", true);
}

/// Pascal `TCIMExporterHelper.WriteConcData` (`ExportCIMXML.pas:2262`).
#[allow(clippy::too_many_arguments)]
fn write_conc_data(
    buf: &mut writer::Writer,
    radius_units: i32,
    res_units: i32,
    dia_cable: f64,
    dia_strand: f64,
    gmr_strand: f64,
    r_strand: f64,
    k_strand: i32,
) {
    let v1 = LineUnits::from_code(radius_units).to_meters();
    writer::double_node(
        buf,
        ProfileChoice::Cat,
        "CableInfo.diameterOverScreen",
        v1 * (dia_cable - 2.0 * dia_strand),
    );
    writer::double_node(
        buf,
        ProfileChoice::Cat,
        "ConcentricNeutralCableInfo.diameterOverNeutral",
        v1 * dia_cable,
    );
    writer::double_node(
        buf,
        ProfileChoice::Cat,
        "ConcentricNeutralCableInfo.neutralStrandRadius",
        v1 * 0.5 * dia_strand,
    );
    writer::double_node(
        buf,
        ProfileChoice::Cat,
        "ConcentricNeutralCableInfo.neutralStrandGmr",
        v1 * gmr_strand,
    );
    let v1 = LineUnits::from_code(res_units).to_per_meter();
    writer::double_node(
        buf,
        ProfileChoice::Cat,
        "ConcentricNeutralCableInfo.neutralStrandRDC20",
        v1 * r_strand,
    );
    writer::integer_node(
        buf,
        ProfileChoice::Cat,
        "ConcentricNeutralCableInfo.neutralStrandCount",
        k_strand as i64,
    );
    writer::boolean_node(buf, ProfileChoice::Cat, "CableInfo.sheathAsNeutral", false);
}

/// One conductor slot for [`LineSnap`]: the master `(name, class)` a phase's
/// `ACLineSegmentPhase.WireInfo` reference resolves through, or `None` (Pascal
/// NIL `FetchConductorData`).
type ConductorRef = Option<(String, &'static str)>;

/// A `Line`'s CIM-relevant state, snapshotted so the class-list borrow is
/// released before the master-UUID lookups (`class_obj_uuid`) run.
struct LineSnap {
    name: String,
    uuid: Uuid,
    is_switch: bool,
    nphases: usize,
    nterm: usize,
    closed: bool,
    sym_components_model: bool,
    r1: f64,
    x1: f64,
    r0: f64,
    x0: f64,
    c1: f64,
    c0: f64,
    len: f64,
    base_frequency: f64,
    user_length_units: LineUnits,
    line_code_units: LineUnits,
    line_code_name: Option<String>,
    geometry_name: Option<String>,
    spacing_name: Option<String>,
    z: Option<crate::support::cmatrix::CMatrix>,
    yc: Option<crate::support::cmatrix::CMatrix>,
    norm_amps: f64,
    emerg_amps: f64,
    num_cond_avail: i32,
    conductor_refs: Vec<ConductorRef>,
    bus_specs: Vec<String>,
    bus_refs: Vec<usize>,
    bus_kvbases: Vec<f64>,
}

/// Pascal `AttachLinePhases` (`ExportCIMXML.pas:1627`): the per-phase
/// `ACLineSegmentPhase` breakdown (called for every line except balanced
/// 3-phase symmetric-components). Each phase references its master `WireInfo`
/// conductor when one exists (`i <= NumConductorsAvailable`).
fn attach_line_phases(
    buf: &mut writer::Writer,
    classes: &mut [DssClass],
    cim: &mut CimExporter,
    snap: &LineSnap,
) {
    let mut s = phase_order_string(&snap.bus_specs[0], snap.nphases, snap.bus_kvbases[0], true);
    if snap.num_cond_avail as usize > s.chars().count() {
        s.push('N'); // so we can specify the neutral conductor
    }
    let loc_uuid = cim.get_dev_uuid(UuidChoice::LineLoc, &snap.name, 1);
    for (i0, ch) in s.chars().enumerate() {
        let seq = i0 + 1;
        let phs = match ch {
            's' => continue,
            '1' => "s1".to_string(),
            '2' => "s2".to_string(),
            c => c.to_string(),
        };
        let local_name = format!("{}_{}", snap.name, phs);
        let phase_uuid = cim.get_dev_uuid(UuidChoice::LinePhase, &local_name, 1);
        writer::start_instance(
            buf,
            ProfileChoice::Fun,
            "ACLineSegmentPhase",
            phase_uuid,
            &local_name,
        );
        writer::phase_kind_node(buf, ProfileChoice::Fun, "ACLineSegmentPhase", &phs);
        writer::integer_node(
            buf,
            ProfileChoice::Fun,
            "ACLineSegmentPhase.sequenceNumber",
            seq as i64,
        );
        if seq <= snap.num_cond_avail as usize
            && let Some(Some((cond_name, cond_class))) = snap.conductor_refs.get(i0)
            && let Some(wire_uuid) = class_obj_uuid(classes, cond_class, cond_name)
        {
            writer::write_cim_ln(
                buf,
                ProfileChoice::Cat,
                &format!(
                    r#"  <cim:ACLineSegmentPhase.WireInfo rdf:resource="urn:uuid:{}"/>"#,
                    wire_uuid.to_cim_string()
                ),
            );
        }
        writer::ref_node(
            buf,
            ProfileChoice::Fun,
            "ACLineSegmentPhase.ACLineSegment",
            snap.uuid,
        );
        writer::ref_node(
            buf,
            ProfileChoice::Geo,
            "PowerSystemResource.Location",
            loc_uuid,
        );
        writer::end_instance(buf, ProfileChoice::Fun, "ACLineSegmentPhase");
    }
}

/// Pascal `AttachSwitchPhases` (`ExportCIMXML.pas:1661`): the per-phase
/// `SwitchPhase` breakdown supporting transpositions (skipped for a balanced
/// 3-phase switch whose two terminals share the phase order).
fn attach_switch_phases(buf: &mut writer::Writer, cim: &mut CimExporter, snap: &LineSnap) {
    let s1 = phase_order_string(&snap.bus_specs[0], snap.nphases, snap.bus_kvbases[0], true);
    let s2 = phase_order_string(&snap.bus_specs[1], snap.nphases, snap.bus_kvbases[1], true);
    if snap.nphases == 3 && s1.chars().count() == 3 && s1 == s2 {
        return;
    }
    let loc_uuid = cim.get_dev_uuid(UuidChoice::LineLoc, &snap.name, 1);
    let map = |c: char| -> String {
        match c {
            '1' => "s1".to_string(),
            '2' => "s2".to_string(),
            other => other.to_string(),
        }
    };
    let s1c: Vec<char> = s1.chars().collect();
    let s2c: Vec<char> = s2.chars().collect();
    for i in 0..s1c.len() {
        // Pascal walks `s2[i]` to Length(s1); a well-formed switch has equal
        // phase-order lengths (same NPhases), so `s2c[i]` is in range — stop if
        // not (an out-of-range Pascal read is undefined, not reproducible).
        let (Some(&c1), Some(&c2)) = (s1c.get(i), s2c.get(i)) else {
            break;
        };
        if c1 == 's' || c2 == 's' {
            continue;
        }
        let phs1 = map(c1);
        let phs2 = map(c2);
        let local_name = format!("{}_{}", snap.name, phs1);
        let phase_uuid = cim.get_dev_uuid(UuidChoice::LinePhase, &local_name, 1);
        writer::start_instance(
            buf,
            ProfileChoice::Fun,
            "SwitchPhase",
            phase_uuid,
            &local_name,
        );
        writer::boolean_node(buf, ProfileChoice::Ssh, "SwitchPhase.closed", snap.closed);
        writer::boolean_node(
            buf,
            ProfileChoice::Fun,
            "SwitchPhase.normalOpen",
            !snap.closed,
        );
        writer::phase_side_node(buf, ProfileChoice::Fun, "SwitchPhase", 1, &phs1);
        writer::phase_side_node(buf, ProfileChoice::Fun, "SwitchPhase", 2, &phs2);
        writer::ref_node(buf, ProfileChoice::Fun, "SwitchPhase.Switch", snap.uuid);
        writer::ref_node(
            buf,
            ProfileChoice::Geo,
            "PowerSystemResource.Location",
            loc_uuid,
        );
        writer::end_instance(buf, ProfileChoice::Fun, "SwitchPhase");
    }
}

/// A catalog conductor's [`ConductorGeom`] plus its `NormAmps` (the base
/// `TConductorData` current rating, not carried by `ConductorGeom`); `None` for
/// a non-conductor object.
fn conductor_geom_amps(cond: &dyn DssObject) -> Option<(ConductorGeom, f64)> {
    cond.as_conductor().map(|c| (c.geom(), c.amps().0))
}

/// The index of the (case-insensitive) class in the class list, or `None`.
pub(crate) fn class_index(classes: &[DssClass], name: &str) -> Option<usize> {
    classes
        .iter()
        .position(|c| c.props.class_name().eq_ignore_ascii_case(name))
}

/// Pascal LineCode catalog sweep (`ExportCIMXML.pas:4493-4547`): a
/// `PerLengthSequenceImpedance` (symmetric-components 3-phase) or a
/// `PerLengthPhaseImpedance` + lower-triangular `PhaseImpedanceData` per
/// LineCode. The `Units=UNITS_NONE` fix-up loop (`4495-4509`) adopts the units
/// of the first enabled `Line` referencing this code (mutating `pLnCd.Units`).
fn write_line_code_catalog(
    buf: &mut writer::Writer,
    classes: &mut [DssClass],
    cim: &mut CimExporter,
    ckt: &Circuit,
) {
    let Some(ci) = class_index(classes, "linecode") else {
        return;
    };
    let two_pi = 2.0 * std::f64::consts::PI;
    let n = classes[ci].arena.len();
    // Units fix-up (mutates in place, matching Pascal `4495-4509`).
    for oi in 0..n {
        let is_none = classes[ci]
            .arena
            .get::<LineCodeObj>(oi)
            .map(|l| l.units() == 0)
            .unwrap_or(false);
        if !is_none {
            continue;
        }
        let lc_name = classes[ci].arena[oi].data().name().to_string();
        if let Some(code) = find_line_units_for_linecode(classes, ckt, &lc_name) {
            classes[ci].arena[oi].set_i32(lc_prop::UNITS, code);
        }
    }
    for oi in 0..n {
        let uuid = classes[ci].arena[oi].data_mut().uuid();
        let name = classes[ci].arena[oi].data().name().to_string();
        let (units, sym, nph, r1, x1, r0, x0, c1, c0, basef, z, yc) = {
            let Some(lc) = classes[ci].arena.get::<LineCodeObj>(oi) else {
                continue;
            };
            (
                lc.units(),
                lc.sym_components_model(),
                lc.nphases(),
                lc.r1(),
                lc.x1(),
                lc.r0(),
                lc.x0(),
                lc.c1(),
                lc.c0(),
                lc.base_frequency(),
                lc.z().cloned(),
                lc.yc().cloned(),
            )
        };
        let v1 = LineUnits::from_code(units).to_per_meter();
        if sym && nph == 3 {
            let v2 = 1.0e-9 * two_pi * basef; // nF -> mhos
            writer::start_instance(
                buf,
                ProfileChoice::Ep,
                "PerLengthSequenceImpedance",
                uuid,
                &name,
            );
            writer::double_node(
                buf,
                ProfileChoice::Ep,
                "PerLengthSequenceImpedance.r",
                r1 * v1,
            );
            writer::double_node(
                buf,
                ProfileChoice::Ep,
                "PerLengthSequenceImpedance.x",
                x1 * v1,
            );
            writer::double_node(
                buf,
                ProfileChoice::Ep,
                "PerLengthSequenceImpedance.bch",
                c1 * v1 * v2,
            );
            writer::double_node(
                buf,
                ProfileChoice::Ep,
                "PerLengthSequenceImpedance.gch",
                0.0,
            );
            writer::double_node(
                buf,
                ProfileChoice::Ep,
                "PerLengthSequenceImpedance.r0",
                r0 * v1,
            );
            writer::double_node(
                buf,
                ProfileChoice::Ep,
                "PerLengthSequenceImpedance.x0",
                x0 * v1,
            );
            writer::double_node(
                buf,
                ProfileChoice::Ep,
                "PerLengthSequenceImpedance.b0ch",
                c0 * v1 * v2,
            );
            writer::double_node(
                buf,
                ProfileChoice::Ep,
                "PerLengthSequenceImpedance.g0ch",
                0.0,
            );
            writer::end_instance(buf, ProfileChoice::Ep, "PerLengthSequenceImpedance");
        } else {
            writer::start_instance(
                buf,
                ProfileChoice::Ep,
                "PerLengthPhaseImpedance",
                uuid,
                &name,
            );
            writer::integer_node(
                buf,
                ProfileChoice::Ep,
                "PerLengthPhaseImpedance.conductorCount",
                nph as i64,
            );
            writer::end_instance(buf, ProfileChoice::Ep, "PerLengthPhaseImpedance");
            let mut seq = 1;
            for i in 1..=(nph.max(0) as usize) {
                for j in 1..=i {
                    let zdata_uuid = cim.get_dev_uuid(UuidChoice::ZData, &name, seq);
                    writer::start_free_instance(
                        buf,
                        ProfileChoice::Ep,
                        "PhaseImpedanceData",
                        zdata_uuid,
                    );
                    writer::ref_node(
                        buf,
                        ProfileChoice::Ep,
                        "PhaseImpedanceData.PhaseImpedance",
                        uuid,
                    );
                    writer::integer_node(
                        buf,
                        ProfileChoice::Ep,
                        "PhaseImpedanceData.row",
                        i as i64,
                    );
                    writer::integer_node(
                        buf,
                        ProfileChoice::Ep,
                        "PhaseImpedanceData.column",
                        j as i64,
                    );
                    let zij = z.as_ref().map(|m| m.get(i - 1, j - 1)).unwrap_or_default();
                    let ycij = yc.as_ref().map(|m| m.get(i - 1, j - 1)).unwrap_or_default();
                    writer::double_node(
                        buf,
                        ProfileChoice::Ep,
                        "PhaseImpedanceData.r",
                        zij.re * v1,
                    );
                    writer::double_node(
                        buf,
                        ProfileChoice::Ep,
                        "PhaseImpedanceData.x",
                        zij.im * v1,
                    );
                    writer::double_node(
                        buf,
                        ProfileChoice::Ep,
                        "PhaseImpedanceData.b",
                        ycij.im * v1,
                    );
                    writer::end_instance(buf, ProfileChoice::Ep, "PhaseImpedanceData");
                    seq += 1;
                }
            }
        }
    }
}

/// The `UserLengthUnits` code of the first enabled `Line` referencing `lc_name`
/// (the LineCode units fix-up source, Pascal `4497-4508`).
fn find_line_units_for_linecode(classes: &[DssClass], ckt: &Circuit, lc_name: &str) -> Option<i32> {
    for &r in &ckt.lines {
        if let Some(line) = classes[r.class_ord()].arena.get::<Line>(r.index())
            && line.cd.enabled
            && line.line_code_ref.is_some()
            && line.line_code_name.eq_ignore_ascii_case(lc_name)
        {
            return Some(line.user_length_units.code());
        }
    }
    None
}

/// Pascal WireData catalog sweep (`ExportCIMXML.pas:4549-4555`): one
/// `OverheadWireInfo` per `WireData`.
fn write_wire_data_catalog(buf: &mut writer::Writer, classes: &mut [DssClass]) {
    let Some(ci) = class_index(classes, "wiredata") else {
        return;
    };
    for oi in 0..classes[ci].arena.len() {
        let uuid = classes[ci].arena[oi].data_mut().uuid();
        let name = classes[ci].arena[oi].data().name().to_string();
        let Some((geom, norm)) = conductor_geom_amps(classes[ci].arena.obj(oi)) else {
            continue;
        };
        writer::start_instance(buf, ProfileChoice::Cat, "OverheadWireInfo", uuid, &name);
        write_wire_data(buf, "WireData", &name, &geom, norm);
        writer::boolean_node(buf, ProfileChoice::Cat, "WireInfo.insulated", false);
        writer::end_instance(buf, ProfileChoice::Cat, "OverheadWireInfo");
    }
}

/// Pascal TSData catalog sweep (`ExportCIMXML.pas:4557-4564`): one
/// `TapeShieldCableInfo` per `TSData`.
fn write_ts_data_catalog(buf: &mut writer::Writer, classes: &mut [DssClass]) {
    let Some(ci) = class_index(classes, "tsdata") else {
        return;
    };
    for oi in 0..classes[ci].arena.len() {
        let uuid = classes[ci].arena[oi].data_mut().uuid();
        let name = classes[ci].arena[oi].data().name().to_string();
        let Some((geom, norm)) = conductor_geom_amps(classes[ci].arena.obj(oi)) else {
            continue;
        };
        writer::start_instance(buf, ProfileChoice::Cat, "TapeShieldCableInfo", uuid, &name);
        write_wire_data(buf, "TSData", &name, &geom, norm);
        if let Some(CableGeom::Ts {
            eps_r,
            ins_layer,
            dia_ins,
            dia_cable,
            dia_shield,
            tape_layer,
            tape_lap,
        }) = &geom.cable
        {
            write_cable_data(
                buf,
                geom.radius_units,
                *eps_r,
                *ins_layer,
                *dia_ins,
                *dia_cable,
            );
            write_tape_data(buf, geom.radius_units, *dia_shield, *tape_layer, *tape_lap);
        }
        writer::end_instance(buf, ProfileChoice::Cat, "TapeShieldCableInfo");
    }
}

/// Pascal CNData catalog sweep (`ExportCIMXML.pas:4566-4573`): one
/// `ConcentricNeutralCableInfo` per `CNData`.
fn write_cn_data_catalog(buf: &mut writer::Writer, classes: &mut [DssClass]) {
    let Some(ci) = class_index(classes, "cndata") else {
        return;
    };
    for oi in 0..classes[ci].arena.len() {
        let uuid = classes[ci].arena[oi].data_mut().uuid();
        let name = classes[ci].arena[oi].data().name().to_string();
        let Some((geom, norm)) = conductor_geom_amps(classes[ci].arena.obj(oi)) else {
            continue;
        };
        writer::start_instance(
            buf,
            ProfileChoice::Cat,
            "ConcentricNeutralCableInfo",
            uuid,
            &name,
        );
        write_wire_data(buf, "CNData", &name, &geom, norm);
        if let Some(CableGeom::Cn {
            eps_r,
            ins_layer,
            dia_ins,
            dia_cable,
            k_strand,
            dia_strand,
            gmr_strand,
            r_strand,
            semicon_layer: _, // not a CIM field
        }) = &geom.cable
        {
            write_cable_data(
                buf,
                geom.radius_units,
                *eps_r,
                *ins_layer,
                *dia_ins,
                *dia_cable,
            );
            write_conc_data(
                buf,
                geom.radius_units,
                geom.res_units,
                *dia_cable,
                *dia_strand,
                *gmr_strand,
                *r_strand,
                *k_strand,
            );
        }
        writer::end_instance(buf, ProfileChoice::Cat, "ConcentricNeutralCableInfo");
    }
}

/// Pascal LineGeometry catalog sweep (`ExportCIMXML.pas:4575-4599`): one
/// `WireSpacingInfo` + a `WirePosition` per conductor. `isCable` reads the first
/// conductor's `PhaseChoice`. Coordinates are per-conductor `Units[i]`.
fn write_line_geometry_catalog(
    buf: &mut writer::Writer,
    classes: &mut [DssClass],
    cim: &mut CimExporter,
) {
    let Some(ci) = class_index(classes, "linegeometry") else {
        return;
    };
    for oi in 0..classes[ci].arena.len() {
        let uuid = classes[ci].arena[oi].data_mut().uuid();
        let name = classes[ci].arena[oi].data().name().to_string();
        let (nwires, is_overhead, xs, ys, us) = {
            let Some(g) = classes[ci].arena.get::<LineGeometryObj>(oi) else {
                continue;
            };
            (
                g.nwires(),
                g.conductor_is_overhead(1),
                g.fx().to_vec(),
                g.fy().to_vec(),
                g.funits().to_vec(),
            )
        };
        writer::start_instance(buf, ProfileChoice::Cat, "WireSpacingInfo", uuid, &name);
        writer::conductor_usage_enum(buf, ProfileChoice::Cat, "distribution");
        writer::integer_node(buf, ProfileChoice::Cat, "WireSpacingInfo.phaseWireCount", 1);
        writer::double_node(
            buf,
            ProfileChoice::Cat,
            "WireSpacingInfo.phaseWireSpacing",
            0.0,
        );
        writer::boolean_node(
            buf,
            ProfileChoice::Cat,
            "WireSpacingInfo.isCable",
            !is_overhead,
        );
        writer::end_instance(buf, ProfileChoice::Cat, "WireSpacingInfo");
        for i in 1..=(nwires.max(0) as usize) {
            let wp_local = format!("WP_{name}_{i}");
            let wp_uuid = cim.get_dev_uuid(UuidChoice::WirePos, &wp_local, 1); // 1 for pGeom
            writer::start_instance(buf, ProfileChoice::Cat, "WirePosition", wp_uuid, &wp_local);
            writer::ref_node(
                buf,
                ProfileChoice::Cat,
                "WirePosition.WireSpacingInfo",
                uuid,
            );
            writer::integer_node(
                buf,
                ProfileChoice::Cat,
                "WirePosition.sequenceNumber",
                i as i64,
            );
            let v1 = LineUnits::from_code(us[i - 1]).to_meters();
            writer::double_node(
                buf,
                ProfileChoice::Cat,
                "WirePosition.xCoord",
                xs[i - 1] * v1,
            );
            writer::double_node(
                buf,
                ProfileChoice::Cat,
                "WirePosition.yCoord",
                ys[i - 1] * v1,
            );
            writer::end_instance(buf, ProfileChoice::Cat, "WirePosition");
        }
    }
}

/// Pascal LineSpacing catalog sweep (`ExportCIMXML.pas:4601-4625`): one
/// `WireSpacingInfo` + a `WirePosition` per conductor. `isCable` reads
/// `Ycoord[1] > 0`. The single `Units` applies to every coordinate. The
/// `WirePosition` UUIDs are keyed with `seq = 2` (Pascal's "2 for pSpac").
fn write_line_spacing_catalog(
    buf: &mut writer::Writer,
    classes: &mut [DssClass],
    cim: &mut CimExporter,
) {
    let Some(ci) = class_index(classes, "linespacing") else {
        return;
    };
    for oi in 0..classes[ci].arena.len() {
        let uuid = classes[ci].arena[oi].data_mut().uuid();
        let name = classes[ci].arena[oi].data().name().to_string();
        let (nwires, units, xs, ys) = {
            let Some(s) = classes[ci].arena.get::<LineSpacingObj>(oi) else {
                continue;
            };
            (
                s.nwires(),
                s.spacing_units(),
                s.xcoord().to_vec(),
                s.ycoord().to_vec(),
            )
        };
        let v1 = LineUnits::from_code(units).to_meters();
        writer::start_instance(buf, ProfileChoice::Cat, "WireSpacingInfo", uuid, &name);
        writer::conductor_usage_enum(buf, ProfileChoice::Cat, "distribution");
        writer::integer_node(buf, ProfileChoice::Cat, "WireSpacingInfo.phaseWireCount", 1);
        writer::double_node(
            buf,
            ProfileChoice::Cat,
            "WireSpacingInfo.phaseWireSpacing",
            0.0,
        );
        // Pascal `if Ycoord[1] > 0.0 then isCable=FALSE else TRUE`.
        let is_cable = ys.first().copied().unwrap_or(0.0) <= 0.0;
        writer::boolean_node(buf, ProfileChoice::Cat, "WireSpacingInfo.isCable", is_cable);
        writer::end_instance(buf, ProfileChoice::Cat, "WireSpacingInfo");
        for i in 1..=(nwires.max(0) as usize) {
            let wp_local = format!("WP_{name}_{i}");
            let wp_uuid = cim.get_dev_uuid(UuidChoice::WirePos, &wp_local, 2); // 2 for pSpac
            writer::start_instance(buf, ProfileChoice::Cat, "WirePosition", wp_uuid, &wp_local);
            writer::ref_node(
                buf,
                ProfileChoice::Cat,
                "WirePosition.WireSpacingInfo",
                uuid,
            );
            writer::integer_node(
                buf,
                ProfileChoice::Cat,
                "WirePosition.sequenceNumber",
                i as i64,
            );
            writer::double_node(
                buf,
                ProfileChoice::Cat,
                "WirePosition.xCoord",
                xs[i - 1] * v1,
            );
            writer::double_node(
                buf,
                ProfileChoice::Cat,
                "WirePosition.yCoord",
                ys[i - 1] * v1,
            );
            writer::end_instance(buf, ProfileChoice::Cat, "WirePosition");
        }
    }
}

/// Pascal `TCIMExporter.ExportCDPSM` (`ExportCIMXML.pas:3203`). Drives the whole
/// export through a [`writer::Writer`] (`combined` = `Export CIM100` ptr 21, else
/// `Export CIM100Fragments` ptr 20) and returns it; the caller
/// (`exec/report.rs`) finalizes it (`into_combined` / `into_fragments`) and owns
/// writing the file(s), matching every other `Export` formatter in this codebase.
/// A loud error is appended to `errors` only for a genuine unsupported case (a
/// CapControl monitored-element class with no `DSSObjType` mapping) — every
/// element class is otherwise fully ported.
#[allow(clippy::too_many_arguments)]
pub(crate) fn export_cdpsm(
    classes: &mut [DssClass],
    ckt: &mut Circuit,
    cim: &mut CimExporter,
    errors: &mut crate::diag::ErrorLog,
    substation: &str,
    sub_geographic_region: &str,
    geographic_region: &str,
    fdr_uuid: Uuid,
    sub_uuid: Uuid,
    sub_geo_uuid: Uuid,
    rgn_uuid: Uuid,
    combined: bool,
) -> writer::Writer {
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

    // `FD_Create` (`ExportCIMXML.pas:4729`): open the writer + emit the
    // per-file `StartCIMFile` preamble(s). `combined = true` → one FUN buffer
    // (`Export CIM100`); `false` → seven per-profile files (`Export
    // CIM100Fragments`).
    let cim_ver_uuid = cim.get_dev_uuid(UuidChoice::CIMVer, "IEC", 1);
    let mut buf = writer::Writer::new(combined, cim_ver_uuid);

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
        let Some(vsrc) = classes[r.class_ord()].arena.get::<VSource>(r.index()) else {
            continue;
        };
        if !vsrc.cd.enabled {
            continue;
        }
        let bus_ref = vsrc.cd.terminals[0].bus_idx();
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

    // Generators -> SynchronousMachine (`3503-3522`) — Stage F.
    for &r in &ckt.generators.clone() {
        struct GenSnap {
            enabled: bool,
            name: String,
            nphases: usize,
            is_delta: bool,
            present_kw: f64,
            present_kvar: f64,
            present_kv: f64,
            kva_rating: f64,
            nterm: usize,
            bus_specs: Vec<String>,
            bus_refs: Vec<usize>,
            daily: String,
            duty: String,
            yearly: String,
            spectrum: String,
        }
        let snap = {
            let Some(g) = classes[r.class_ord()].arena.get::<Generator>(r.index()) else {
                continue;
            };
            let nm = |o: Option<&dyn DssObject>| -> String {
                o.map(|s| s.data().name().to_string()).unwrap_or_default()
            };
            GenSnap {
                enabled: g.cd.enabled,
                name: g.cd.obj.name().to_string(),
                nphases: g.cd.nphases,
                is_delta: g.connection as i32 == 1,
                present_kw: g.present_kw(),
                present_kvar: g.present_kvar(),
                present_kv: g.kv_generator_base,
                kva_rating: g.kva_rating,
                nterm: g.cd.nterms,
                bus_specs: g.cd.bus_names.clone(),
                bus_refs: g.cd.terminals.iter().map(|t| t.bus_idx()).collect(),
                daily: nm(g.daily_shape_obj.as_ref().map(|o| o as &dyn DssObject)),
                duty: nm(g.duty_shape_obj.as_ref().map(|o| o as &dyn DssObject)),
                yearly: nm(g.yearly_shape_obj.as_ref().map(|o| o as &dyn DssObject)),
                spectrum: g.spectrum.clone(),
            }
        };
        if !snap.enabled {
            continue;
        }
        let gen_uuid = classes[r.class_ord()].arena[r.index()].data_mut().uuid();
        let bus_kvbase0 = ckt.buses[snap.bus_refs[0]].kv_base;

        writer::start_instance(
            &mut buf,
            ProfileChoice::Fun,
            "SynchronousMachine",
            gen_uuid,
            &snap.name,
        );
        writer::circuit_node(&mut buf, ProfileChoice::Fun, fdr_uuid);
        writer::double_node(
            &mut buf,
            ProfileChoice::Ssh,
            "RotatingMachine.p",
            snap.present_kw * 1000.0,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ssh,
            "RotatingMachine.q",
            snap.present_kvar * 1000.0,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "RotatingMachine.ratedS",
            snap.kva_rating * 1000.0,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "RotatingMachine.ratedU",
            snap.present_kv * 1000.0,
        );
        // `SynchMachTypeEnum`/`SynchMachModeEnum` are commented out upstream
        // (`3514-3515`) — no node emitted.
        let geo_uuid = cim.get_dev_uuid(UuidChoice::MachLoc, &snap.name, 1);
        writer::ref_node(
            &mut buf,
            ProfileChoice::Geo,
            "PowerSystemResource.Location",
            geo_uuid,
        );
        writer::end_instance(&mut buf, ProfileChoice::Fun, "SynchronousMachine");
        attach_der_phases(
            &mut buf,
            cim,
            "SynchronousMachinePhase",
            "SynchronousMachinePhase.SynchronousMachine",
            UuidChoice::GenPhase,
            snap.nphases,
            snap.is_delta,
            snap.present_kv,
            snap.present_kw,
            snap.present_kvar,
            &snap.bus_specs[0],
            bus_kvbase0,
            &snap.name,
            gen_uuid,
            geo_uuid,
        );
        write_terminals(
            &mut buf,
            ckt,
            cim,
            &mut op_limits,
            &mut op_limit_idx,
            GEN_DSS_OBJ_TYPE,
            "Generator",
            &snap.name,
            gen_uuid,
            snap.nterm,
            &snap.bus_specs,
            &snap.bus_refs,
            geo_uuid,
            crs_uuid,
            0.0,
            0.0,
        );
        add_generator_ecp(
            &mut ecps,
            cim,
            gen_uuid,
            &snap.daily,
            &snap.duty,
            &snap.yearly,
            &snap.spectrum,
        );
    }

    // PVSystems -> PhotovoltaicUnit + PowerElectronicsConnection (`3524-3569`).
    for &r in &ckt.pv_systems.clone() {
        struct PvSnap {
            enabled: bool,
            name: String,
            nphases: usize,
            is_delta: bool,
            present_kw: f64,
            present_kvar: f64,
            present_kv: f64,
            pmpp: f64,
            pct_cut_in: f64,
            pct_cut_out: f64,
            kva_rating: f64,
            vmin_pu: f64,
            var_mode: i32,
            cim_dyn: bool,
            fkvar_limit: f64,
            fkvar_limit_neg: f64,
            kvar_limit_set: bool,
            kvar_limit_neg_set: bool,
            nterm: usize,
            bus_specs: Vec<String>,
            bus_refs: Vec<usize>,
            daily: String,
            duty: String,
            yearly: String,
            tdaily: String,
            tduty: String,
            tyearly: String,
            spectrum: String,
        }
        let snap = {
            let Some(pv) = classes[r.class_ord()].arena.get::<PVSystem>(r.index()) else {
                continue;
            };
            let nm = |o: Option<&dyn DssObject>| -> String {
                o.map(|s| s.data().name().to_string()).unwrap_or_default()
            };
            PvSnap {
                enabled: pv.cd.enabled,
                name: pv.cd.obj.name().to_string(),
                nphases: pv.cd.nphases,
                is_delta: pv.base.connection as i32 == 1,
                present_kw: pv.present_kw(),
                present_kvar: pv.present_kvar(),
                present_kv: pv.kv_pvsystem_base,
                pmpp: pv.f_pmpp,
                pct_cut_in: pv.base.fpct_cut_in,
                pct_cut_out: pv.base.fpct_cut_out,
                kva_rating: pv.f_kva_rating,
                vmin_pu: pv.base.vminpu,
                var_mode: pv.base.var_mode,
                cim_dyn: pv.base.using_cim_dynamics(),
                fkvar_limit: pv.f_kvar_limit,
                fkvar_limit_neg: pv.f_kvar_limit_neg,
                kvar_limit_set: pv.base.kvar_limit_set,
                kvar_limit_neg_set: pv.base.kvar_limit_neg_set,
                nterm: pv.cd.nterms,
                bus_specs: pv.cd.bus_names.clone(),
                bus_refs: pv.cd.terminals.iter().map(|t| t.bus_idx()).collect(),
                daily: nm(pv
                    .base
                    .daily_shape_obj
                    .as_ref()
                    .map(|o| o as &dyn DssObject)),
                duty: nm(pv.base.duty_shape_obj.as_ref().map(|o| o as &dyn DssObject)),
                yearly: nm(pv
                    .base
                    .yearly_shape_obj
                    .as_ref()
                    .map(|o| o as &dyn DssObject)),
                tdaily: nm(pv.daily_t_shape_obj.as_ref().map(|o| o as &dyn DssObject)),
                tduty: nm(pv.duty_t_shape_obj.as_ref().map(|o| o as &dyn DssObject)),
                tyearly: nm(pv.yearly_t_shape_obj.as_ref().map(|o| o as &dyn DssObject)),
                spectrum: nm(pv.spectrum_obj.as_ref().map(|o| o as &dyn DssObject)),
            }
        };
        if !snap.enabled {
            continue;
        }
        let pv_uuid = classes[r.class_ord()].arena[r.index()].data_mut().uuid();
        let bus_kvbase0 = ckt.buses[snap.bus_refs[0]].kv_base;

        let pv_panels_uuid = cim.get_dev_uuid(UuidChoice::PVPanels, &snap.name, 1);
        writer::start_instance(
            &mut buf,
            ProfileChoice::Fun,
            "PhotovoltaicUnit",
            pv_panels_uuid,
            &snap.name,
        );
        let geo_uuid = cim.get_dev_uuid(UuidChoice::SolarLoc, &snap.name, 1);
        writer::ref_node(
            &mut buf,
            ProfileChoice::Geo,
            "PowerSystemResource.Location",
            geo_uuid,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "PowerElectronicsUnit.maxP",
            snap.pmpp * 1000.0,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "PowerElectronicsUnit.minP",
            (snap.pct_cut_in.min(snap.pct_cut_out) * snap.kva_rating / 100.0) * 1000.0,
        );
        writer::end_instance(&mut buf, ProfileChoice::Fun, "PhotovoltaicUnit");

        writer::start_instance(
            &mut buf,
            ProfileChoice::Fun,
            "PowerElectronicsConnection",
            pv_uuid,
            &snap.name,
        );
        writer::circuit_node(&mut buf, ProfileChoice::Fun, fdr_uuid);
        writer::ref_node(
            &mut buf,
            ProfileChoice::Fun,
            "PowerElectronicsConnection.PowerElectronicsUnit",
            pv_panels_uuid,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "PowerElectronicsConnection.maxIFault",
            1.0 / snap.vmin_pu,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ssh,
            "PowerElectronicsConnection.p",
            snap.present_kw * 1000.0,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ssh,
            "PowerElectronicsConnection.q",
            snap.present_kvar * 1000.0,
        );
        writer::converter_control_enum(&mut buf, ProfileChoice::Ssh, snap.var_mode, snap.cim_dyn);
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "PowerElectronicsConnection.ratedS",
            snap.kva_rating * 1000.0,
        );
        let rated_u = if snap.nphases == 1 {
            snap.present_kv * 1000.0 * sqrt3
        } else {
            snap.present_kv * 1000.0
        };
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "PowerElectronicsConnection.ratedU",
            rated_u,
        );
        let max_q = if !snap.kvar_limit_set {
            snap.kva_rating * 1000.0 * 0.25
        } else {
            snap.fkvar_limit * 1000.0
        };
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "PowerElectronicsConnection.maxQ",
            max_q,
        );
        let min_q = if !snap.kvar_limit_neg_set {
            -snap.kva_rating * 1000.0 * 0.25
        } else {
            -snap.fkvar_limit_neg * 1000.0
        };
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "PowerElectronicsConnection.minQ",
            min_q,
        );
        writer::ref_node(
            &mut buf,
            ProfileChoice::Geo,
            "PowerSystemResource.Location",
            geo_uuid,
        );
        writer::end_instance(&mut buf, ProfileChoice::Fun, "PowerElectronicsConnection");
        attach_der_phases(
            &mut buf,
            cim,
            "PowerElectronicsConnectionPhase",
            "PowerElectronicsConnectionPhase.PowerElectronicsConnection",
            UuidChoice::SolarPhase,
            snap.nphases,
            snap.is_delta,
            snap.present_kv,
            snap.present_kw,
            snap.present_kvar,
            &snap.bus_specs[0],
            bus_kvbase0,
            &snap.name,
            pv_uuid,
            geo_uuid,
        );
        // PV/Storage: `WriteReferenceTerminals` then `WritePositions` (the
        // localName swap to the panel/cell name is a no-op — Rust's name == the
        // DER name); not the folded `WriteTerminals`.
        write_reference_terminals(
            &mut buf,
            ckt,
            cim,
            &mut op_limits,
            &mut op_limit_idx,
            PVSYSTEM_DSS_OBJ_TYPE,
            &snap.name,
            pv_uuid,
            snap.nterm,
            &snap.bus_specs,
            &snap.bus_refs,
            0.0,
            0.0,
        );
        write_positions(
            &mut buf,
            ckt,
            cim,
            "PVSystem",
            &snap.name,
            snap.nterm,
            &snap.bus_specs,
            &snap.bus_refs,
            geo_uuid,
            crs_uuid,
        );
        add_solar_ecp(
            &mut ecps,
            cim,
            pv_uuid,
            &snap.daily,
            &snap.duty,
            &snap.yearly,
            &snap.tdaily,
            &snap.tduty,
            &snap.tyearly,
            &snap.spectrum,
        );
    }

    // StorageElements -> BatteryUnit + PowerElectronicsConnection (`3571-3612`).
    for &r in &ckt.storages.clone() {
        struct BatSnap {
            enabled: bool,
            name: String,
            nphases: usize,
            is_delta: bool,
            present_kw: f64,
            present_kvar: f64,
            present_kv: f64,
            vmin_pu: f64,
            var_mode: i32,
            cim_dyn: bool,
            kw_rating: f64,
            pct_kw_rated: f64,
            kwh_rating: f64,
            kwh_stored: f64,
            storage_state: i32,
            fkva_rating: f64,
            fkvar_limit: f64,
            fkvar_limit_neg: f64,
            nterm: usize,
            bus_specs: Vec<String>,
            bus_refs: Vec<usize>,
            daily: String,
            duty: String,
            yearly: String,
            spectrum: String,
        }
        let snap = {
            let Some(st) = classes[r.class_ord()].arena.get::<Storage>(r.index()) else {
                continue;
            };
            let nm = |o: Option<&dyn DssObject>| -> String {
                o.map(|s| s.data().name().to_string()).unwrap_or_default()
            };
            BatSnap {
                enabled: st.cd.enabled,
                name: st.cd.obj.name().to_string(),
                nphases: st.cd.nphases,
                is_delta: st.base.connection as i32 == 1,
                present_kw: st.present_kw(),
                present_kvar: st.present_kvar(),
                present_kv: st.present_kv(),
                vmin_pu: st.base.vminpu,
                var_mode: st.base.var_mode,
                cim_dyn: st.base.using_cim_dynamics(),
                kw_rating: st.kw_rating,
                pct_kw_rated: st.pct_kw_rated,
                kwh_rating: st.kwh_rating,
                kwh_stored: st.kwh_stored,
                storage_state: st.f_state,
                fkva_rating: st.f_kva_rating,
                fkvar_limit: st.f_kvar_limit,
                fkvar_limit_neg: st.f_kvar_limit_neg,
                nterm: st.cd.nterms,
                bus_specs: st.cd.bus_names.clone(),
                bus_refs: st.cd.terminals.iter().map(|t| t.bus_idx()).collect(),
                daily: nm(st
                    .base
                    .daily_shape_obj
                    .as_ref()
                    .map(|o| o as &dyn DssObject)),
                duty: nm(st.base.duty_shape_obj.as_ref().map(|o| o as &dyn DssObject)),
                yearly: nm(st
                    .base
                    .yearly_shape_obj
                    .as_ref()
                    .map(|o| o as &dyn DssObject)),
                spectrum: nm(st.spectrum_obj.as_ref().map(|o| o as &dyn DssObject)),
            }
        };
        if !snap.enabled {
            continue;
        }
        let bat_uuid = classes[r.class_ord()].arena[r.index()].data_mut().uuid();
        let bus_kvbase0 = ckt.buses[snap.bus_refs[0]].kv_base;

        let battery_uuid = cim.get_dev_uuid(UuidChoice::Battery, &snap.name, 1);
        writer::start_instance(
            &mut buf,
            ProfileChoice::Fun,
            "BatteryUnit",
            battery_uuid,
            &snap.name,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "PowerElectronicsUnit.maxP",
            snap.kw_rating * snap.pct_kw_rated * 1000.0,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "PowerElectronicsUnit.minP",
            -snap.kw_rating * snap.pct_kw_rated * 1000.0,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ssh,
            "BatteryUnit.ratedE",
            snap.kwh_rating * 1000.0,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ssh,
            "BatteryUnit.storedE",
            snap.kwh_stored * 1000.0,
        );
        writer::battery_state_enum(&mut buf, ProfileChoice::Ssh, snap.storage_state);
        let geo_uuid = cim.get_dev_uuid(UuidChoice::BatteryLoc, &snap.name, 1);
        writer::ref_node(
            &mut buf,
            ProfileChoice::Geo,
            "PowerSystemResource.Location",
            geo_uuid,
        );
        writer::end_instance(&mut buf, ProfileChoice::Fun, "BatteryUnit");

        writer::start_instance(
            &mut buf,
            ProfileChoice::Fun,
            "PowerElectronicsConnection",
            bat_uuid,
            &snap.name,
        );
        writer::circuit_node(&mut buf, ProfileChoice::Fun, fdr_uuid);
        writer::ref_node(
            &mut buf,
            ProfileChoice::Fun,
            "PowerElectronicsConnection.PowerElectronicsUnit",
            battery_uuid,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "PowerElectronicsConnection.maxIFault",
            1.0 / snap.vmin_pu,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ssh,
            "PowerElectronicsConnection.p",
            snap.present_kw * 1000.0,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ssh,
            "PowerElectronicsConnection.q",
            snap.present_kvar * 1000.0,
        );
        writer::converter_control_enum(&mut buf, ProfileChoice::Ssh, snap.var_mode, snap.cim_dyn);
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "PowerElectronicsConnection.ratedS",
            snap.fkva_rating * 1000.0,
        );
        let rated_u = if snap.nphases == 1 {
            snap.present_kv * 1000.0 * sqrt3
        } else {
            snap.present_kv * 1000.0
        };
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "PowerElectronicsConnection.ratedU",
            rated_u,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "PowerElectronicsConnection.maxQ",
            snap.fkvar_limit.min(snap.fkva_rating) * 1000.0,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "PowerElectronicsConnection.minQ",
            -snap.fkvar_limit_neg.min(snap.fkva_rating) * 1000.0,
        );
        writer::ref_node(
            &mut buf,
            ProfileChoice::Geo,
            "PowerSystemResource.Location",
            geo_uuid,
        );
        writer::end_instance(&mut buf, ProfileChoice::Fun, "PowerElectronicsConnection");
        attach_der_phases(
            &mut buf,
            cim,
            "PowerElectronicsConnectionPhase",
            "PowerElectronicsConnectionPhase.PowerElectronicsConnection",
            UuidChoice::BatteryPhase,
            snap.nphases,
            snap.is_delta,
            snap.present_kv,
            snap.present_kw,
            snap.present_kvar,
            &snap.bus_specs[0],
            bus_kvbase0,
            &snap.name,
            bat_uuid,
            geo_uuid,
        );
        write_reference_terminals(
            &mut buf,
            ckt,
            cim,
            &mut op_limits,
            &mut op_limit_idx,
            STORAGE_DSS_OBJ_TYPE,
            &snap.name,
            bat_uuid,
            snap.nterm,
            &snap.bus_specs,
            &snap.bus_refs,
            0.0,
            0.0,
        );
        write_positions(
            &mut buf,
            ckt,
            cim,
            "Storage",
            &snap.name,
            snap.nterm,
            &snap.bus_specs,
            &snap.bus_refs,
            geo_uuid,
            crs_uuid,
        );
        add_storage_ecp(
            &mut ecps,
            cim,
            bat_uuid,
            &snap.daily,
            &snap.duty,
            &snap.yearly,
            &snap.spectrum,
        );
    }

    // IEEE1547 (InvControl + ExpControl) -> DERIEEEType1 (`3614-3634`) — Stage F.
    super::ieee1547::write_ieee1547_controllers(&mut buf, classes, ckt, cim);

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
            let Some(vsrc) = classes[r.class_ord()].arena.get::<VSource>(r.index()) else {
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
                    .map(|t| t.bus_idx())
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

        let vsrc_uuid = classes[r.class_ord()].arena[r.index()].data_mut().uuid();
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

    // ShuntCapacitors (`3684-3731`) -> LinearShuntCompensator (+ AttachCapPhases) —
    // Stage D.
    for &r in &ckt.shunt_capacitors.clone() {
        struct CapSnap {
            enabled: bool,
            nphases: usize,
            total_kvar: f64,
            nom_kv: f64,
            num_steps: i32,
            connection: i32,
            states: Vec<i32>,
            name: String,
            nterm: usize,
            norm_amps: f64,
            emerg_amps: f64,
            bus_specs: Vec<String>,
            bus_refs: Vec<usize>,
        }
        let snap = {
            let Some(cap) = classes[r.class_ord()].arena.get::<Capacitor>(r.index()) else {
                continue;
            };
            CapSnap {
                enabled: cap.cd.enabled,
                nphases: cap.cd.nphases,
                total_kvar: cap.total_kvar(),
                nom_kv: cap.nom_kv(),
                num_steps: cap.num_steps(),
                connection: cap.connection(),
                states: cap.states().to_vec(),
                name: cap.cd.obj.name().to_string(),
                nterm: cap.cd.nterms,
                norm_amps: cap.norm_amps(),
                emerg_amps: cap.emerg_amps(),
                bus_specs: cap.cd.bus_names.clone(),
                bus_refs: cap.cd.terminals.iter().map(|t| t.bus_idx()).collect(),
            }
        };
        if !snap.enabled {
            continue;
        }
        let cap_uuid = classes[r.class_ord()].arena[r.index()].data_mut().uuid();
        let bus_ref0 = snap.bus_refs[0];
        let bus_kvbase0 = ckt.buses[bus_ref0].kv_base;

        writer::start_instance(
            &mut buf,
            ProfileChoice::Fun,
            "LinearShuntCompensator",
            cap_uuid,
            &snap.name,
        );
        writer::circuit_node(&mut buf, ProfileChoice::Fun, fdr_uuid);
        // VbaseNode (`2124`): terminal-1 bus base × √3.
        let vbase_uuid = cim.get_base_v_uuid(sqrt3 * bus_kvbase0);
        writer::ref_node(
            &mut buf,
            ProfileChoice::Fun,
            "ConductingEquipment.BaseVoltage",
            vbase_uuid,
        );

        let val = 0.001 * snap.total_kvar / snap.nom_kv / snap.nom_kv / snap.num_steps as f64;
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "ShuntCompensator.nomU",
            1000.0 * snap.nom_kv,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "LinearShuntCompensator.bPerSection",
            val,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "LinearShuntCompensator.gPerSection",
            0.0,
        );

        // Pascal `TCapacitorConnection.Wye = 0`.
        if snap.connection == 0 {
            writer::shunt_connection_kind_node(
                &mut buf,
                ProfileChoice::Fun,
                "ShuntCompensator",
                "Y",
            );
            // TODO(compat): Pascal hard-codes `grounded := TRUE` for wye banks
            // (`3700`, "TODO - check bus 2").
            writer::boolean_node(
                &mut buf,
                ProfileChoice::Fun,
                "ShuntCompensator.grounded",
                true,
            );
            writer::double_node(
                &mut buf,
                ProfileChoice::Ep,
                "LinearShuntCompensator.b0PerSection",
                val,
            );
        } else {
            writer::shunt_connection_kind_node(
                &mut buf,
                ProfileChoice::Fun,
                "ShuntCompensator",
                "D",
            );
            // TODO(compat): the delta branch emits `grounded` under the
            // `LinearShuntCompensator.` prefix while the wye branch uses
            // `ShuntCompensator.` — an upstream inconsistency reproduced verbatim
            // (`ExportCIMXML.pas:3706` vs `3700`).
            writer::boolean_node(
                &mut buf,
                ProfileChoice::Fun,
                "LinearShuntCompensator.grounded",
                false,
            );
            writer::double_node(
                &mut buf,
                ProfileChoice::Ep,
                "LinearShuntCompensator.b0PerSection",
                0.0,
            );
        }
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "LinearShuntCompensator.g0PerSection",
            0.0,
        );
        writer::integer_node(
            &mut buf,
            ProfileChoice::Ep,
            "ShuntCompensator.normalSections",
            snap.num_steps as i64,
        );
        writer::integer_node(
            &mut buf,
            ProfileChoice::Ep,
            "ShuntCompensator.maximumSections",
            snap.num_steps as i64,
        );

        // `aVRDelay` = the OnDelay of the last CapControl whose `This_Capacitor`
        // is this bank (Pascal loops all CapControls, last match wins, `3714-3718`).
        let mut avr_delay = 0.0;
        for &cr in &ckt.controls {
            if let Some(cc) = classes[cr.class_ord()].arena.get::<CapControl>(cr.index())
                && cc.controlled_element() == Some(r)
            {
                avr_delay = cc.on_delay_val();
            }
        }
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "ShuntCompensator.aVRDelay",
            avr_delay,
        );

        // `sections` (SSH) = count of in-service steps (`States[i] > 0`).
        let mut sections = 0.0;
        for i in 1..=snap.num_steps.max(0) {
            if snap.states[(i - 1) as usize] > 0 {
                sections += 1.0;
            }
        }
        writer::double_node(
            &mut buf,
            ProfileChoice::Ssh,
            "ShuntCompensator.sections",
            sections,
        );

        let geo_uuid = cim.get_dev_uuid(UuidChoice::CapLoc, &snap.name, 1);
        writer::ref_node(
            &mut buf,
            ProfileChoice::Geo,
            "PowerSystemResource.Location",
            geo_uuid,
        );
        writer::end_instance(&mut buf, ProfileChoice::Fun, "LinearShuntCompensator");

        attach_cap_phases(
            &mut buf,
            cim,
            &snap.name,
            cap_uuid,
            geo_uuid,
            snap.nphases,
            snap.total_kvar,
            snap.nom_kv,
            snap.num_steps,
            snap.connection,
            &snap.bus_specs[0],
            bus_kvbase0,
            sections,
        );
        write_terminals(
            &mut buf,
            ckt,
            cim,
            &mut op_limits,
            &mut op_limit_idx,
            CAP_DSS_OBJ_TYPE,
            "Capacitor",
            &snap.name,
            cap_uuid,
            snap.nterm,
            &snap.bus_specs,
            &snap.bus_refs,
            geo_uuid,
            crs_uuid,
            snap.norm_amps,
            snap.emerg_amps,
        );
    }

    // CapControls -> RegulatingControl (`3733-3781`) — Stage D.
    for &cr in &ckt.controls.clone() {
        struct CcSnap {
            cc_name: String,
            cap_ref: ElemId,
            cap_name: String,
            mon_name: String,
            mon_obj_type: Option<i32>,
            mon_nphases: usize,
            mon_bus_spec0: String,
            mon_bus_kvbase0: f64,
            element_terminal: i32,
            pt_phase: i32,
            control_type: i32,
            on_value: f64,
            off_value: f64,
            pf_on: f64,
            pf_off: f64,
            ct_ratio: f64,
            pt_ratio: f64,
            enabled: bool,
        }
        let snap = {
            let Some(cc) = classes[cr.class_ord()].arena.get::<CapControl>(cr.index()) else {
                continue;
            };
            let cap_ref = cc
                .controlled_element()
                .expect("CapControl.This_Capacitor set for a solved circuit");
            let mon_ref = cc
                .ccd
                .monitored_element
                .expect("CapControl.MonitoredElement set for a solved circuit");
            let mon_obj = classes[mon_ref.class_ord()].arena.obj(mon_ref.index());
            let mon_elem = classes[mon_ref.class_ord()]
                .arena
                .try_ckt_elem(mon_ref.index())
                .expect("CapControl monitored element is a circuit element");
            let mon_cd = mon_elem.cd();
            CcSnap {
                cc_name: cc.ccd.cd.obj.name().to_string(),
                cap_ref,
                cap_name: classes[cap_ref.class_ord()].arena[cap_ref.index()]
                    .data()
                    .name()
                    .to_string(),
                mon_name: mon_obj.data().name().to_string(),
                mon_obj_type: cktelem_dss_obj_type(classes[mon_ref.class_ord()].props.class_name()),
                mon_nphases: mon_cd.nphases,
                mon_bus_spec0: mon_cd.bus_names[0].clone(),
                mon_bus_kvbase0: ckt.buses[mon_cd.terminals[0].bus_idx()].kv_base,
                element_terminal: cc.ccd.element_terminal,
                pt_phase: cc.pt_phase(),
                control_type: cc.control_type(),
                on_value: cc.on_value(),
                off_value: cc.off_value(),
                pf_on: cc.pf_on_value(),
                pf_off: cc.pf_off_value(),
                ct_ratio: cc.ct_ratio_val(),
                pt_ratio: cc.pt_ratio_val(),
                enabled: cc.ccd.cd.enabled,
            }
        };
        let Some(mon_obj_type) = snap.mon_obj_type else {
            errors.push(
                "Export CIM100: CapControl monitored-element class has no DSSObjType \
                 mapping yet (GAPS_PLAN WPG.18 — extend cktelem_dss_obj_type)."
                    .to_string(),
            );
            continue;
        };
        let cc_uuid = classes[cr.class_ord()].arena[cr.index()].data_mut().uuid();
        let cap_uuid = classes[snap.cap_ref.class_ord()].arena[snap.cap_ref.index()]
            .data_mut()
            .uuid();

        writer::start_instance(
            &mut buf,
            ProfileChoice::Fun,
            "RegulatingControl",
            cc_uuid,
            &snap.cc_name,
        );
        // Location -> the controlled capacitor's location UUID (`3736`).
        let cap_loc_uuid = cim.get_dev_uuid(UuidChoice::CapLoc, &snap.cap_name, 1);
        writer::ref_node(
            &mut buf,
            ProfileChoice::Geo,
            "PowerSystemResource.Location",
            cap_loc_uuid,
        );
        writer::ref_node(
            &mut buf,
            ProfileChoice::Fun,
            "RegulatingControl.RegulatingCondEq",
            cap_uuid,
        );
        // Terminal -> the monitored element's `ElementTerminal` (`3738-3739`).
        let term_uuid = cim.get_term_uuid(mon_obj_type, &snap.mon_name, snap.element_terminal);
        writer::ref_node(
            &mut buf,
            ProfileChoice::Fun,
            "RegulatingControl.Terminal",
            term_uuid,
        );
        // MonitoredPhaseNode from `FirstPhaseString(MonitoredElement, 1)` shifted
        // by `PTPhase` (`3740-3744`). `FirstPhaseString` (`1391`) is the first
        // letter of `PhaseString`, or `'A'` when empty.
        let first = {
            let s = phase_string(
                &snap.mon_bus_spec0,
                snap.mon_nphases,
                snap.mon_bus_kvbase0,
                true,
            );
            s.chars().next().unwrap_or('A')
        };
        let mon_phase = if snap.pt_phase > 0 {
            (first as u8 + snap.pt_phase as u8 - 1) as char
        } else {
            first
        };
        writer::monitored_phase_node(&mut buf, ProfileChoice::Fun, &mon_phase.to_string());

        // val / v1 / v2 by control type (`3745-3761`).
        let mut val = 1.0;
        let (v1, v2);
        if snap.control_type == CAP_CTRL_PF {
            v1 = snap.pf_on;
            v2 = snap.pf_off;
        } else {
            v1 = snap.on_value;
            v2 = snap.off_value;
            if snap.control_type == CAP_CTRL_KVAR {
                val = 1000.0;
            }
            if snap.control_type == CAP_CTRL_CURRENT {
                val = snap.ct_ratio;
            }
            if snap.control_type == CAP_CTRL_VOLTAGE {
                val = snap.pt_ratio;
            }
        }
        // RegulatingControlEnum by control type (`3762-3775`); `FOLLOWCONTROL`
        // (and the Rust-unreachable `USERCONTROL`) have no `.mode` arm.
        match snap.control_type {
            CAP_CTRL_CURRENT => {
                writer::regulating_control_enum(&mut buf, ProfileChoice::Ep, "currentFlow")
            }
            CAP_CTRL_VOLTAGE => {
                writer::regulating_control_enum(&mut buf, ProfileChoice::Ep, "voltage")
            }
            CAP_CTRL_KVAR => {
                writer::regulating_control_enum(&mut buf, ProfileChoice::Ep, "reactivePower")
            }
            CAP_CTRL_TIME => {
                writer::regulating_control_enum(&mut buf, ProfileChoice::Ep, "timeScheduled")
            }
            CAP_CTRL_PF => {
                writer::regulating_control_enum(&mut buf, ProfileChoice::Ep, "powerFactor")
            }
            _ => {}
        }
        writer::boolean_node(
            &mut buf,
            ProfileChoice::Ep,
            "RegulatingControl.discrete",
            true,
        );
        writer::boolean_node(
            &mut buf,
            ProfileChoice::Ep,
            "RegulatingControl.enabled",
            snap.enabled,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "RegulatingControl.targetValue",
            val * 0.5 * (v1 + v2),
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "RegulatingControl.targetDeadband",
            val * (v2 - v1),
        );
        writer::end_instance(&mut buf, ProfileChoice::Fun, "RegulatingControl");
    }

    // Transformers + AutoTransformers + their banks (`3785-4197`) — Stage E.
    super::power_xfmr::write_transformers(
        &mut buf,
        classes,
        ckt,
        cim,
        &mut op_limits,
        &mut op_limit_idx,
        crs_uuid,
        fdr_uuid,
        sqrt3,
    );

    // RegControls -> RatioTapChanger/TapChangerControl (`4198-4273`) — Stage E.
    super::power_xfmr::write_reg_controls(&mut buf, classes, ckt, cim);

    // Series reactors -> SeriesCompensator (`4274-4292`) — Stage D.
    for &r in &ckt.reactors.clone() {
        struct ReacSnap {
            enabled: bool,
            name: String,
            nterm: usize,
            z_re: f64,
            z_im: f64,
            norm_amps: f64,
            emerg_amps: f64,
            bus_specs: Vec<String>,
            bus_refs: Vec<usize>,
        }
        let snap = {
            let Some(reac) = classes[r.class_ord()].arena.get::<Reactor>(r.index()) else {
                continue;
            };
            let z = reac.z();
            ReacSnap {
                enabled: reac.cd.enabled,
                name: reac.cd.obj.name().to_string(),
                nterm: reac.cd.nterms,
                z_re: z.re,
                z_im: z.im,
                norm_amps: reac.norm_amps(),
                emerg_amps: reac.emerg_amps(),
                bus_specs: reac.cd.bus_names.clone(),
                bus_refs: reac.cd.terminals.iter().map(|t| t.bus_idx()).collect(),
            }
        };
        if !snap.enabled {
            continue;
        }
        let reac_uuid = classes[r.class_ord()].arena[r.index()].data_mut().uuid();
        let bus_ref0 = snap.bus_refs[0];

        writer::start_instance(
            &mut buf,
            ProfileChoice::Fun,
            "SeriesCompensator",
            reac_uuid,
            &snap.name,
        );
        writer::circuit_node(&mut buf, ProfileChoice::Fun, fdr_uuid);
        // VbaseNode (`2124`): terminal-1 bus base × √3.
        let vbase_uuid = cim.get_base_v_uuid(sqrt3 * ckt.buses[bus_ref0].kv_base);
        writer::ref_node(
            &mut buf,
            ProfileChoice::Fun,
            "ConductingEquipment.BaseVoltage",
            vbase_uuid,
        );
        let geo_uuid = cim.get_dev_uuid(UuidChoice::ReacLoc, &snap.name, 1);
        writer::ref_node(
            &mut buf,
            ProfileChoice::Geo,
            "PowerSystemResource.Location",
            geo_uuid,
        );
        // r0/x0 duplicate r/x (Pascal `pReac.Z.re`/`.im` for all four, `4285-4288`).
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "SeriesCompensator.r",
            snap.z_re,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "SeriesCompensator.x",
            snap.z_im,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "SeriesCompensator.r0",
            snap.z_re,
        );
        writer::double_node(
            &mut buf,
            ProfileChoice::Ep,
            "SeriesCompensator.x0",
            snap.z_im,
        );
        writer::end_instance(&mut buf, ProfileChoice::Fun, "SeriesCompensator");
        // Pascal leaves `AttachLinePhases` commented out for reactors (3-phase
        // series reactors only, `ExportCIMXML.pas:4290`) — no phase objects.
        write_terminals(
            &mut buf,
            ckt,
            cim,
            &mut op_limits,
            &mut op_limit_idx,
            REACTOR_DSS_OBJ_TYPE,
            "Reactor",
            &snap.name,
            reac_uuid,
            snap.nterm,
            &snap.bus_specs,
            &snap.bus_refs,
            geo_uuid,
            crs_uuid,
            snap.norm_amps,
            snap.emerg_amps,
        );
    }

    // Lines/switches -> ACLineSegment/LoadBreakSwitch/Fuse/Breaker/Recloser
    // (`4294-4409`) — Stage C.
    for &r in &ckt.lines.clone() {
        let snap = {
            let Some(line) = classes[r.class_ord()].arena.get::<Line>(r.index()) else {
                continue;
            };
            if !line.cd.enabled {
                continue;
            }
            let bus_refs: Vec<usize> = line.cd.terminals.iter().map(|t| t.bus_idx()).collect();
            let bus_kvbases: Vec<f64> = bus_refs.iter().map(|&b| ckt.buses[b].kv_base).collect();
            let has_line_code = line.line_code_ref.is_some();
            let has_geometry = line.geometry_obj.is_some();
            let spacing_specified =
                line.line_spacing_obj.is_some() && !line.line_wire_data.is_empty();
            // Pascal `NumConductorData` / `FetchConductorData` (Line.pas:2116).
            let num_cond_avail = if spacing_specified {
                line.line_spacing_obj
                    .as_ref()
                    .map(|s| s.nwires())
                    .unwrap_or(0)
            } else if let Some(g) = &line.geometry_obj {
                g.nwires()
            } else {
                0
            };
            let mut conductor_refs: Vec<ConductorRef> = Vec::new();
            for i in 1..=(num_cond_avail.max(0) as usize) {
                let cond: Option<&dyn DssObject> = if spacing_specified {
                    line.line_wire_data.get(i - 1).and_then(|o| o.as_deref())
                } else if let Some(g) = &line.geometry_obj {
                    g.conductor(i)
                } else {
                    None
                };
                conductor_refs.push(cond.and_then(|c| {
                    conductor_class_name(c).map(|cls| (c.data().name().to_string(), cls))
                }));
            }
            LineSnap {
                name: line.cd.obj.name().to_string(),
                uuid: Uuid::create_v4(), // replaced below with the object's UUID
                is_switch: line.is_switch,
                nphases: line.cd.nphases,
                nterm: line.cd.nterms,
                closed: line.cd.terminal_all_phases_closed(1),
                sym_components_model: line.sym_components_model,
                r1: line.r1,
                x1: line.x1,
                r0: line.r0,
                x0: line.x0,
                c1: line.c1,
                c0: line.c0,
                len: line.len,
                base_frequency: line.cd.base_frequency,
                user_length_units: line.user_length_units,
                line_code_units: line.line_code_units,
                line_code_name: has_line_code.then(|| line.line_code_name.clone()),
                geometry_name: has_geometry.then(|| line.geometry_name.clone()),
                spacing_name: spacing_specified
                    .then(|| {
                        line.line_spacing_obj
                            .as_ref()
                            .map(|s| s.data().name().to_string())
                    })
                    .flatten(),
                z: line.z.clone(),
                yc: line.yc.clone(),
                norm_amps: line.norm_amps,
                emerg_amps: line.emerg_amps,
                num_cond_avail,
                conductor_refs,
                bus_specs: line.cd.bus_names.clone(),
                bus_refs,
                bus_kvbases,
            }
        };
        let line_uuid = classes[r.class_ord()].arena[r.index()].data_mut().uuid();
        let mut snap = snap;
        snap.uuid = line_uuid;

        let v1 = snap.user_length_units.to_meters();
        let geo_uuid = cim.get_dev_uuid(UuidChoice::LineLoc, &snap.name, 1);
        let bus_ref0 = snap.bus_refs[0];
        let vbase_uuid = cim.get_base_v_uuid(sqrt3 * ckt.buses[bus_ref0].kv_base);

        if snap.is_switch {
            let (swt_cls, rated_amps, breaking_amps) =
                parse_switch_class(classes, ckt, r, snap.norm_amps);
            writer::start_instance(
                &mut buf,
                ProfileChoice::Fun,
                &swt_cls,
                snap.uuid,
                &snap.name,
            );
            writer::circuit_node(&mut buf, ProfileChoice::Fun, fdr_uuid);
            writer::ref_node(
                &mut buf,
                ProfileChoice::Fun,
                "ConductingEquipment.BaseVoltage",
                vbase_uuid,
            );
            if breaking_amps > 0.0 {
                writer::double_node(
                    &mut buf,
                    ProfileChoice::Ep,
                    "ProtectedSwitch.breakingCapacity",
                    breaking_amps,
                );
            }
            writer::double_node(
                &mut buf,
                ProfileChoice::Ep,
                "Switch.ratedCurrent",
                rated_amps,
            );
            // Disabled lines are skipped above, so the enabled branch always applies.
            writer::boolean_node(
                &mut buf,
                ProfileChoice::Fun,
                "Switch.normalOpen",
                !snap.closed,
            );
            writer::boolean_node(&mut buf, ProfileChoice::Ssh, "Switch.open", !snap.closed);
            writer::boolean_node(&mut buf, ProfileChoice::Fun, "Switch.retained", true);
            writer::ref_node(
                &mut buf,
                ProfileChoice::Geo,
                "PowerSystemResource.Location",
                geo_uuid,
            );
            writer::end_instance(&mut buf, ProfileChoice::Fun, &swt_cls);
            attach_switch_phases(&mut buf, cim, &snap);
        } else {
            let mut bval = false;
            let mut puz_local = String::new();
            let mut puz_uuid = None;
            writer::start_instance(
                &mut buf,
                ProfileChoice::Fun,
                "ACLineSegment",
                snap.uuid,
                &snap.name,
            );
            writer::circuit_node(&mut buf, ProfileChoice::Fun, fdr_uuid);
            writer::ref_node(
                &mut buf,
                ProfileChoice::Fun,
                "ConductingEquipment.BaseVoltage",
                vbase_uuid,
            );
            if let Some(lc_name) = snap.line_code_name.clone() {
                let mut vlen = v1;
                if snap.user_length_units == LineUnits::None {
                    vlen = snap.line_code_units.to_meters();
                }
                writer::double_node(
                    &mut buf,
                    ProfileChoice::Fun,
                    "Conductor.length",
                    snap.len * vlen,
                );
                if let Some(lc_uuid) = class_obj_uuid(classes, "LineCode", &lc_name) {
                    writer::write_cim_ln(
                        &mut buf,
                        ProfileChoice::Ep,
                        &format!(
                            r#"  <cim:ACLineSegment.PerLengthImpedance rdf:resource="urn:uuid:{}"/>"#,
                            lc_uuid.to_cim_string()
                        ),
                    );
                }
            } else if let Some(geom_name) = snap.geometry_name.clone() {
                writer::double_node(
                    &mut buf,
                    ProfileChoice::Fun,
                    "Conductor.length",
                    snap.len * v1,
                );
                if let Some(geom_uuid) = class_obj_uuid(classes, "LineGeometry", &geom_name) {
                    writer::write_cim_ln(
                        &mut buf,
                        ProfileChoice::Cat,
                        &format!(
                            r#"  <cim:ACLineSegment.WireSpacingInfo rdf:resource="urn:uuid:{}"/>"#,
                            geom_uuid.to_cim_string()
                        ),
                    );
                }
            } else if let Some(sp_name) = snap.spacing_name.clone() {
                writer::double_node(
                    &mut buf,
                    ProfileChoice::Fun,
                    "Conductor.length",
                    snap.len * v1,
                );
                if let Some(sp_uuid) = class_obj_uuid(classes, "LineSpacing", &sp_name) {
                    writer::write_cim_ln(
                        &mut buf,
                        ProfileChoice::Cat,
                        &format!(
                            r#"  <cim:ACLineSegment.WireSpacingInfo rdf:resource="urn:uuid:{}"/>"#,
                            sp_uuid.to_cim_string()
                        ),
                    );
                }
            } else if snap.sym_components_model && snap.nphases == 3 {
                let val = 1.0e-9 * two_pi * snap.base_frequency; // nF -> mhos
                writer::double_node(&mut buf, ProfileChoice::Fun, "Conductor.length", 1.0);
                writer::double_node(
                    &mut buf,
                    ProfileChoice::Ep,
                    "ACLineSegment.r",
                    snap.len * snap.r1,
                );
                writer::double_node(
                    &mut buf,
                    ProfileChoice::Ep,
                    "ACLineSegment.x",
                    snap.len * snap.x1,
                );
                writer::double_node(
                    &mut buf,
                    ProfileChoice::Ep,
                    "ACLineSegment.bch",
                    snap.len * snap.c1 * val,
                );
                writer::double_node(&mut buf, ProfileChoice::Ep, "ACLineSegment.gch", 0.0);
                writer::double_node(
                    &mut buf,
                    ProfileChoice::Ep,
                    "ACLineSegment.r0",
                    snap.len * snap.r0,
                );
                writer::double_node(
                    &mut buf,
                    ProfileChoice::Ep,
                    "ACLineSegment.x0",
                    snap.len * snap.x0,
                );
                writer::double_node(
                    &mut buf,
                    ProfileChoice::Ep,
                    "ACLineSegment.b0ch",
                    snap.len * snap.c0 * val,
                );
                // TODO(compat): Pascal writes `ACLineSegment.b0ch` a second time,
                // = 0.0 (`ExportCIMXML.pas:4367`, an upstream typo for `g0ch`);
                // reproduced verbatim so the golden matches.
                writer::double_node(&mut buf, ProfileChoice::Ep, "ACLineSegment.b0ch", 0.0);
            } else {
                bval = true;
                puz_local = format!("{}_PUZ", snap.name);
                let id = cim.get_dev_uuid(UuidChoice::PUZ, &snap.name, 1);
                puz_uuid = Some(id);
                writer::write_cim_ln(
                    &mut buf,
                    ProfileChoice::Ep,
                    &format!(
                        r#"  <cim:ACLineSegment.PerLengthImpedance rdf:resource="urn:uuid:{}"/>"#,
                        id.to_cim_string()
                    ),
                );
                writer::double_node(
                    &mut buf,
                    ProfileChoice::Fun,
                    "Conductor.length",
                    snap.len * v1,
                );
            }
            writer::ref_node(
                &mut buf,
                ProfileChoice::Geo,
                "PowerSystemResource.Location",
                geo_uuid,
            );
            writer::end_instance(&mut buf, ProfileChoice::Fun, "ACLineSegment");
            if !(snap.sym_components_model && snap.nphases == 3) {
                attach_line_phases(&mut buf, classes, cim, &snap);
            }
            if bval {
                let id = puz_uuid.expect("PUZ path sets puz_uuid");
                writer::start_instance(
                    &mut buf,
                    ProfileChoice::Ep,
                    "PerLengthPhaseImpedance",
                    id,
                    &puz_local,
                );
                writer::integer_node(
                    &mut buf,
                    ProfileChoice::Ep,
                    "PerLengthPhaseImpedance.conductorCount",
                    snap.nphases as i64,
                );
                writer::end_instance(&mut buf, ProfileChoice::Ep, "PerLengthPhaseImpedance");
                let z = snap.z.as_ref();
                let yc = snap.yc.as_ref();
                let mut seq = 1;
                for i in 1..=snap.nphases {
                    for j in 1..=i {
                        let zdata_uuid = cim.get_dev_uuid(UuidChoice::ZData, &puz_local, seq);
                        writer::start_free_instance(
                            &mut buf,
                            ProfileChoice::Ep,
                            "PhaseImpedanceData",
                            zdata_uuid,
                        );
                        writer::ref_node(
                            &mut buf,
                            ProfileChoice::Ep,
                            "PhaseImpedanceData.PhaseImpedance",
                            id,
                        );
                        writer::integer_node(
                            &mut buf,
                            ProfileChoice::Ep,
                            "PhaseImpedanceData.row",
                            i as i64,
                        );
                        writer::integer_node(
                            &mut buf,
                            ProfileChoice::Ep,
                            "PhaseImpedanceData.column",
                            j as i64,
                        );
                        // Pascal divides by 1609.34 (hard-coded meters-per-mile).
                        let zij = z.map(|m| m.get(i - 1, j - 1)).unwrap_or_default();
                        let ycij = yc.map(|m| m.get(i - 1, j - 1)).unwrap_or_default();
                        writer::double_node(
                            &mut buf,
                            ProfileChoice::Ep,
                            "PhaseImpedanceData.r",
                            zij.re / 1609.34,
                        );
                        writer::double_node(
                            &mut buf,
                            ProfileChoice::Ep,
                            "PhaseImpedanceData.x",
                            zij.im / 1609.34,
                        );
                        writer::double_node(
                            &mut buf,
                            ProfileChoice::Ep,
                            "PhaseImpedanceData.b",
                            ycij.im / 1609.34,
                        );
                        writer::end_instance(&mut buf, ProfileChoice::Ep, "PhaseImpedanceData");
                        seq += 1;
                    }
                }
            }
        }
        write_terminals(
            &mut buf,
            ckt,
            cim,
            &mut op_limits,
            &mut op_limit_idx,
            LINE_DSS_OBJ_TYPE,
            "Line",
            &snap.name,
            snap.uuid,
            snap.nterm,
            &snap.bus_specs,
            &snap.bus_refs,
            geo_uuid,
            crs_uuid,
            snap.norm_amps,
            snap.emerg_amps,
        );
    }

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
            let Some(load) = classes[r.class_ord()].arena.get::<Load>(r.index()) else {
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
                bus_refs: load.cd.terminals.iter().map(|t| t.bus_idx()).collect(),
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
        let load_uuid = classes[r.class_ord()].arena[r.index()].data_mut().uuid();

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
    write_line_code_catalog(&mut buf, classes, cim, ckt);
    write_wire_data_catalog(&mut buf, classes);
    write_ts_data_catalog(&mut buf, classes);
    write_cn_data_catalog(&mut buf, classes);
    write_line_geometry_catalog(&mut buf, classes, cim);
    write_line_spacing_catalog(&mut buf, classes, cim);

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
        let str_node = |buf: &mut writer::Writer, node: &str, val: &str| {
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

    // `FD_Destroy` (`4752`): the closing `</rdf:RDF>` per file is appended by the
    // writer's finalizer ([`writer::Writer::into_combined`] /
    // [`writer::Writer::into_fragments`]) — the caller picks the mode-matching one.
    buf
}

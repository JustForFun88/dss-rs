//! The CIM100 XML writer core — Pascal `Common/ExportCIMXML.pas` l.376-450
//! (`WriteCimLn`/`StartInstance`/`StartFreeInstance`/`EndInstance`), l.1309-1339
//! (`GetBaseVName`/`GetOpLimVName`/`GetOpLimIName`), l.1340-1626 (the scalar/enum
//! node helpers actually used through Stage A), and l.3188-3201 (`StartCIMFile`).
//! GAPS_PLAN WPG.18 Stage A: **combined mode only** (`Separate = false`) — every
//! profile writes into the one buffer, matching `WriteCimLn`'s `else` branch.
//! Stage F (`Export CIM100Fragments`) adds the `Separate = true` per-profile
//! file-splitting.

use super::Uuid;

/// Pascal `ProfileChoice` (`ExportCIMXML.pas:39`). Threaded through every writer
/// call for forward-compat with Stage F fragments; Stage A's combined-mode
/// [`write_cim_ln`] ignores it (everything lands in the one `F_FUN` buffer).
#[allow(dead_code)] // Cat/Dyn are constructed starting Stage C/F
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileChoice {
    Fun,
    Ep,
    Geo,
    Topo,
    Cat,
    Ssh,
    Dyn,
}

/// The CIM namespace (`ExportCIMXML.pas:240`, the CIM100 iteration — the
/// commented-out CIM17 line above it is dead).
pub const CIM_NS: &str = "http://iec.ch/TC57/CIM100";

/// Pascal `TCIMExporter.WriteCimLn` (`ExportCIMXML.pas:376`), combined-mode
/// (`Separate = false`) branch: every profile writes to the single buffer.
pub fn write_cim_ln(buf: &mut String, _prf: ProfileChoice, s: &str) {
    buf.push_str(s);
    buf.push('\n');
}

/// Pascal `TCIMExporter.StartInstance` (`ExportCIMXML.pas:410`).
pub fn start_instance(
    buf: &mut String,
    prf: ProfileChoice,
    root: &str,
    uuid: Uuid,
    local_name: &str,
) {
    let cim_id = uuid.to_cim_string();
    write_cim_ln(
        buf,
        prf,
        &format!(r#"<cim:{root} rdf:about="urn:uuid:{cim_id}">"#),
    );
    write_cim_ln(
        buf,
        prf,
        &format!("  <cim:IdentifiedObject.mRID>{cim_id}</cim:IdentifiedObject.mRID>"),
    );
    write_cim_ln(
        buf,
        prf,
        &format!("  <cim:IdentifiedObject.name>{local_name}</cim:IdentifiedObject.name>"),
    );
}

/// Pascal `TCIMExporter.StartFreeInstance` (`ExportCIMXML.pas:422`): like
/// [`start_instance`] but for objects with no `TNamedObject` (bare UUID + no
/// automatic mRID/name lines — the caller writes those itself when needed).
pub fn start_free_instance(buf: &mut String, prf: ProfileChoice, root: &str, uuid: Uuid) {
    write_cim_ln(
        buf,
        prf,
        &format!(
            r#"<cim:{root} rdf:about="urn:uuid:{}">"#,
            uuid.to_cim_string()
        ),
    );
}

/// Pascal `TCIMExporter.EndInstance` (`ExportCIMXML.pas:432`), combined-mode.
pub fn end_instance(buf: &mut String, prf: ProfileChoice, root: &str) {
    write_cim_ln(buf, prf, &format!("</cim:{root}>"));
}

/// Pascal `TCIMExporterHelper.DoubleNode` (`ExportCIMXML.pas:1340`): `%.8g`
/// (`report::format::g` at 8 significant digits — the WP8.5-audited FPC `%g`).
pub fn double_node(buf: &mut String, prf: ProfileChoice, node: &str, val: f64) {
    let s = crate::report::format::g(val, 8);
    write_cim_ln(buf, prf, &format!("  <cim:{node}>{s}</cim:{node}>"));
}

/// Pascal `TCIMExporterHelper.IntegerNode` (`ExportCIMXML.pas:1345`).
pub fn integer_node(buf: &mut String, prf: ProfileChoice, node: &str, val: i64) {
    write_cim_ln(buf, prf, &format!("  <cim:{node}>{val}</cim:{node}>"));
}

/// Pascal `TCIMExporterHelper.BooleanNode` (`ExportCIMXML.pas:1350`): lowercase
/// `true`/`false`.
pub fn boolean_node(buf: &mut String, prf: ProfileChoice, node: &str, val: bool) {
    let s = if val { "true" } else { "false" };
    write_cim_ln(buf, prf, &format!("  <cim:{node}>{s}</cim:{node}>"));
}

/// Pascal `TCIMExporterHelper.StringNode` (`ExportCIMXML.pas:1526`).
pub fn string_node(buf: &mut String, prf: ProfileChoice, node: &str, val: &str) {
    write_cim_ln(buf, prf, &format!("  <cim:{node}>{val}</cim:{node}>"));
}

/// Pascal `TCIMExporterHelper.RefNode`/`UuidNode` (`ExportCIMXML.pas:1361/1366`):
/// both render identically (`<cim:Node rdf:resource="urn:uuid:ID"/>`) — `RefNode`
/// took a `TNamedObject` and read its `.CIM_ID`, `UuidNode` took a raw `TUuid`;
/// since our objects carry their UUID directly (no `TNamedObject` shim), one
/// function serves both call sites.
pub fn ref_node(buf: &mut String, prf: ProfileChoice, node: &str, uuid: Uuid) {
    write_cim_ln(
        buf,
        prf,
        &format!(
            r#"  <cim:{node} rdf:resource="urn:uuid:{}"/>"#,
            uuid.to_cim_string()
        ),
    );
}

/// Pascal `TCIMExporterHelper.CircuitNode` (`ExportCIMXML.pas:1386`):
/// `Equipment.EquipmentContainer` referencing the Feeder (the circuit).
pub fn circuit_node(buf: &mut String, prf: ProfileChoice, feeder_uuid: Uuid) {
    write_cim_ln(
        buf,
        prf,
        &format!(
            r#"  <cim:Equipment.EquipmentContainer rdf:resource="urn:uuid:{}"/>"#,
            feeder_uuid.to_cim_string()
        ),
    );
}

/// Pascal `TCIMExporterHelper.PhaseKindNode` (`ExportCIMXML.pas:1602`): a
/// `<Root>.phase` reference into `SinglePhaseKind.<val>` (per-phase objects).
pub fn phase_kind_node(buf: &mut String, prf: ProfileChoice, root: &str, val: &str) {
    write_cim_ln(
        buf,
        prf,
        &format!(r#"  <cim:{root}.phase rdf:resource="{CIM_NS}#SinglePhaseKind.{val}"/>"#),
    );
}

/// Pascal `TCIMExporterHelper.PhaseSideNode` (`ExportCIMXML.pas:1608`): a
/// `<Root>.phaseSide<Side>` reference into `SinglePhaseKind.<val>` (SwitchPhase).
pub fn phase_side_node(buf: &mut String, prf: ProfileChoice, root: &str, side: i64, val: &str) {
    write_cim_ln(
        buf,
        prf,
        &format!(
            r#"  <cim:{root}.phaseSide{side} rdf:resource="{CIM_NS}#SinglePhaseKind.{val}"/>"#
        ),
    );
}

/// Pascal `TCIMExporterHelper.ConductorUsageEnum` (`ExportCIMXML.pas:1452`):
/// `<cim:WireSpacingInfo.usage rdf:resource="…#WireUsageKind.<val>"/>`.
pub fn conductor_usage_enum(buf: &mut String, prf: ProfileChoice, val: &str) {
    write_cim_ln(
        buf,
        prf,
        &format!(r#"  <cim:WireSpacingInfo.usage rdf:resource="{CIM_NS}#WireUsageKind.{val}"/>"#),
    );
}

/// Pascal `TCIMExporterHelper.ConductorInsulationEnum` (`ExportCIMXML.pas:1446`):
/// `<cim:WireInfo.insulationMaterial rdf:resource="…#WireInsulationKind.<val>"/>`.
pub fn conductor_insulation_enum(buf: &mut String, prf: ProfileChoice, val: &str) {
    write_cim_ln(
        buf,
        prf,
        &format!(
            r#"  <cim:WireInfo.insulationMaterial rdf:resource="{CIM_NS}#WireInsulationKind.{val}"/>"#
        ),
    );
}

/// Pascal `TCIMExporterHelper.ShuntConnectionKindNode` (`ExportCIMXML.pas:1614`):
/// `<Root>.phaseConnection` into `PhaseShuntConnectionKind.<val>` (D/Y/Yn/I).
pub fn shunt_connection_kind_node(buf: &mut String, prf: ProfileChoice, root: &str, val: &str) {
    write_cim_ln(
        buf,
        prf,
        &format!(
            r#"  <cim:{root}.phaseConnection rdf:resource="{CIM_NS}#PhaseShuntConnectionKind.{val}"/>"#
        ),
    );
}

/// Pascal `TCIMExporterHelper.RegulatingControlEnum` (`ExportCIMXML.pas:1434`):
/// `<cim:RegulatingControl.mode rdf:resource="…#RegulatingControlModeKind.<val>"/>`.
pub fn regulating_control_enum(buf: &mut String, prf: ProfileChoice, val: &str) {
    write_cim_ln(
        buf,
        prf,
        &format!(
            r#"  <cim:RegulatingControl.mode rdf:resource="{CIM_NS}#RegulatingControlModeKind.{val}"/>"#
        ),
    );
}

/// Pascal `TCIMExporterHelper.MonitoredPhaseNode` (`ExportCIMXML.pas:1488`):
/// `<cim:RegulatingControl.monitoredPhase rdf:resource="…#PhaseCode.<val>"/>`.
pub fn monitored_phase_node(buf: &mut String, prf: ProfileChoice, val: &str) {
    write_cim_ln(
        buf,
        prf,
        &format!(
            r#"  <cim:RegulatingControl.monitoredPhase rdf:resource="{CIM_NS}#PhaseCode.{val}"/>"#
        ),
    );
}

/// Pascal `TCIMExporterHelper.OpLimitDirectionEnum` (`ExportCIMXML.pas:1494`).
pub fn op_limit_direction_enum(buf: &mut String, prf: ProfileChoice, val: &str) {
    write_cim_ln(
        buf,
        prf,
        &format!(
            r#"  <cim:OperationalLimitType.direction rdf:resource="{CIM_NS}#OperationalLimitDirectionKind.{val}"/>"#
        ),
    );
}

/// Pascal `GetBaseVName` (`ExportCIMXML.pas:1309`): `FloatToStrF(val, ffFixed, 6,
/// 4)` = fixed notation, 4 fractional digits (`ffFixed` ignores `Precision`).
pub fn base_v_name(val: f64) -> String {
    format!("BaseV_{val:.4}")
}

/// Pascal `GetOpLimVName` (`ExportCIMXML.pas:1320`).
pub fn op_lim_v_name(val: f64) -> String {
    format!("OpLimV_{val:.4}")
}

/// Pascal `GetOpLimIName` (`ExportCIMXML.pas:1330`): `FloatToStrF(_, ffFixed, 6,
/// 1)` for both operands, joined by `_`.
pub fn op_lim_i_name(norm: f64, emerg: f64) -> String {
    format!("OpLimI_{norm:.1}_{emerg:.1}")
}

/// Pascal `IsGroundBus` (`ExportCIMXML.pas:2025`): `True` (ground) unless the raw
/// bus-spec string (with its `.N` node suffixes, e.g. `"sourcebus.1.2.3"`)
/// contains a plain **substring** `.1`/`.2`/`.3` (not a per-node-token parse —
/// `"bus.10"` matches `.1`, a literal Pascal quirk reproduced as-is) — or has no
/// dot at all (a bare bus name is never a ground spec).
pub fn is_ground_bus(s: &str) -> bool {
    if s.contains(".1") || s.contains(".2") || s.contains(".3") {
        return false;
    }
    s.contains('.')
}

/// Pascal `TCIMExporterHelper.StartCIMFile` (`ExportCIMXML.pas:3188`): the fixed
/// XML preamble + the `IEC61970CIMVersion` singleton instance every CIM file
/// (combined, or each per-profile file in Stage F fragments mode) opens with.
/// Writes the `IEC61970CIMVersion` block **directly** (not via
/// [`start_instance`]/[`end_instance`] — the Pascal call site bypasses
/// `WriteCimLn`'s profile dispatch and writes straight to the stream `F` being
/// initialized): no `mRID`/`name` lines, just `rdf:about` + the two fixed
/// fields + the closing tag. `cim_ver_uuid` is the caller's
/// `GetDevUuid(CIMVer, 'IEC', 1)` (the UUID substrate lives in `cim::mod`, not
/// here, to keep this module a pure formatter).
pub fn start_cim_file(buf: &mut String, prf: ProfileChoice, cim_ver_uuid: Uuid) {
    write_cim_ln(buf, prf, r#"<?xml version="1.0" encoding="utf-8"?>"#);
    write_cim_ln(buf, prf, "<!-- un-comment this line to enable validation");
    write_cim_ln(buf, prf, "-->");
    write_cim_ln(
        buf,
        prf,
        &format!(
            r#"<rdf:RDF xmlns:cim="{CIM_NS}#" xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">"#
        ),
    );
    write_cim_ln(buf, prf, "<!--");
    write_cim_ln(buf, prf, "-->");
    let cim_id = cim_ver_uuid.to_cim_string();
    write_cim_ln(
        buf,
        prf,
        &format!(r#"<cim:IEC61970CIMVersion rdf:about="urn:uuid:{cim_id}">"#),
    );
    write_cim_ln(
        buf,
        prf,
        "  <cim:IEC61970CIMVersion.version>IEC61970CIM100</cim:IEC61970CIMVersion.version>",
    );
    write_cim_ln(
        buf,
        prf,
        "  <cim:IEC61970CIMVersion.date>2019-04-01</cim:IEC61970CIMVersion.date>",
    );
    write_cim_ln(buf, prf, "</cim:IEC61970CIMVersion>");
}

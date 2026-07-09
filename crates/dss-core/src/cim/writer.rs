//! The CIM100 XML writer core — Pascal `Common/ExportCIMXML.pas` l.376-450
//! (`WriteCimLn`/`StartInstance`/`StartFreeInstance`/`EndInstance`), l.1309-1339
//! (`GetBaseVName`/`GetOpLimVName`/`GetOpLimIName`), l.1340-1626 (the scalar/enum
//! node helpers), l.3188-3201 (`StartCIMFile`), and l.4729-4770
//! (`FD_Create`/`FD_Destroy`). The [`Writer`] carries **both** output modes:
//! combined (`Export CIM100`, `Separate = false`) writes every profile into the
//! one `F_FUN` buffer; fragments (`Export CIM100Fragments`, `Separate = true`,
//! GAPS_PLAN WPG.18 Stage F) routes each line to its own per-profile buffer with
//! the auto-`StartFreeInstance` / close-every-open-profile logic.

use super::Uuid;

/// Pascal `ProfileChoice` (`ExportCIMXML.pas:39`). Threaded through every writer
/// call; combined-mode [`write_cim_ln`] ignores it (everything lands in the one
/// `F_FUN` buffer), fragments mode routes to the matching per-profile file.
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

impl ProfileChoice {
    /// Dense index into the [`Writer`]'s per-profile arrays.
    fn idx(self) -> usize {
        match self {
            ProfileChoice::Fun => 0,
            ProfileChoice::Ep => 1,
            ProfileChoice::Geo => 2,
            ProfileChoice::Topo => 3,
            ProfileChoice::Cat => 4,
            ProfileChoice::Ssh => 5,
            ProfileChoice::Dyn => 6,
        }
    }
}

/// The seven profiles paired with their fragments-mode file suffix (Pascal
/// `FD_Create`, `ExportCIMXML.pas:4738-4744`: `_FUN`/`_GEO`/`_TOPO`/`_SSH`/
/// `_CAT`/`_EP`/`_DYN`). The order is `FD_Destroy`'s (`4754-4763`) — FUN first;
/// each file is self-contained so order is cosmetic.
const PROFILE_FILES: [(ProfileChoice, &str); 7] = [
    (ProfileChoice::Fun, "FUN"),
    (ProfileChoice::Geo, "GEO"),
    (ProfileChoice::Topo, "TOPO"),
    (ProfileChoice::Ssh, "SSH"),
    (ProfileChoice::Cat, "CAT"),
    (ProfileChoice::Ep, "EP"),
    (ProfileChoice::Dyn, "DYN"),
];

/// The CIM namespace (`ExportCIMXML.pas:240`, the CIM100 iteration — the
/// commented-out CIM17 line above it is dead).
pub const CIM_NS: &str = "http://iec.ch/TC57/CIM100";

/// The stateful CIM writer — Pascal `TCIMExporter`'s output half (`Separate`,
/// `F_FUN…F_DYN`, `roots[]`, `ids[]`). In combined mode only `bufs[0]` (FUN) is
/// used; in fragments mode all seven per-profile buffers plus the `roots`/`ids`
/// bookkeeping drive `WriteCimLn`'s auto-`StartFreeInstance` and `EndInstance`'s
/// close-every-open-profile behavior.
pub struct Writer {
    /// Pascal `Separate` (`= not Combined`, `FD_Create:4733`).
    separate: bool,
    /// Per-profile output buffers (combined: only `[0]`/FUN).
    bufs: [String; 7],
    /// Pascal `roots[prf]`: the currently-open instance's root tag per profile
    /// (empty = no open instance), driving the fragments auto-open/close.
    roots: [String; 7],
    /// Pascal `ids[prf]`: the currently-open instance's UUID per profile (read
    /// by the auto-`StartFreeInstance` when a child line opens a foreign profile).
    ids: [Uuid; 7],
}

impl Writer {
    /// Pascal `FD_Create` (`ExportCIMXML.pas:4729`): open the output(s) and write
    /// each file's `StartCIMFile` preamble. Combined → one FUN buffer; fragments
    /// → all seven (the `F_DYN` file is opened passing `EpPrf` upstream, an
    /// oddity with no output effect — the preamble is profile-independent).
    pub fn new(combined: bool, cim_ver_uuid: Uuid) -> Self {
        let mut w = Writer {
            separate: !combined,
            bufs: std::array::from_fn(|_| String::new()),
            roots: std::array::from_fn(|_| String::new()),
            ids: [Uuid::nil(); 7],
        };
        if w.separate {
            for i in 0..7 {
                write_preamble(&mut w.bufs[i], cim_ver_uuid);
            }
        } else {
            write_preamble(&mut w.bufs[0], cim_ver_uuid);
        }
        w
    }

    /// Pascal `FD_Destroy` (`ExportCIMXML.pas:4752`), combined mode: close the
    /// FUN root and return the one produced file.
    pub fn into_combined(mut self) -> String {
        self.bufs[0].push_str("</rdf:RDF>\n");
        std::mem::take(&mut self.bufs[0])
    }

    /// Pascal `FD_Destroy` (`ExportCIMXML.pas:4752`), fragments mode: close every
    /// profile root and return `(suffix, content)` for each of the seven files.
    pub fn into_fragments(mut self) -> Vec<(&'static str, String)> {
        PROFILE_FILES
            .iter()
            .map(|&(prf, suffix)| {
                let i = prf.idx();
                self.bufs[i].push_str("</rdf:RDF>\n");
                (suffix, std::mem::take(&mut self.bufs[i]))
            })
            .collect()
    }
}

/// Pascal `TCIMExporter.WriteCimLn` (`ExportCIMXML.pas:376`). Combined mode
/// (`Separate = false`): every profile writes to the single FUN buffer.
/// Fragments mode: a non-FUN line whose profile has no open instance first
/// auto-opens a `StartFreeInstance` cloning the FUN profile's current root/id
/// (the "avoid stack overflow" comment `l.412` — the reason `StartInstance` sets
/// `roots`/`ids` *before* it writes), then routes the line to that profile's file.
pub fn write_cim_ln(buf: &mut Writer, prf: ProfileChoice, s: &str) {
    if buf.separate && prf != ProfileChoice::Fun && buf.roots[prf.idx()].is_empty() {
        let root = buf.roots[ProfileChoice::Fun.idx()].clone();
        let id = buf.ids[ProfileChoice::Fun.idx()];
        start_free_instance(buf, prf, &root, id);
    }
    let i = if buf.separate { prf.idx() } else { 0 };
    buf.bufs[i].push_str(s);
    buf.bufs[i].push('\n');
}

/// Pascal `TCIMExporter.StartInstance` (`ExportCIMXML.pas:410`). In fragments
/// mode it records this profile's open root/id **before** writing (the Pascal
/// "must be first to avoid stack overflow in WriteCimLn" ordering).
pub fn start_instance(
    buf: &mut Writer,
    prf: ProfileChoice,
    root: &str,
    uuid: Uuid,
    local_name: &str,
) {
    if buf.separate {
        buf.roots[prf.idx()] = root.to_string();
        buf.ids[prf.idx()] = uuid;
    }
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
/// automatic mRID/name lines — the caller writes those itself when needed). Also
/// the fragments-mode auto-open target from [`write_cim_ln`]; records the open
/// root/id first (same ordering rule as [`start_instance`]).
pub fn start_free_instance(buf: &mut Writer, prf: ProfileChoice, root: &str, uuid: Uuid) {
    if buf.separate {
        buf.roots[prf.idx()] = root.to_string();
        buf.ids[prf.idx()] = uuid;
    }
    write_cim_ln(
        buf,
        prf,
        &format!(
            r#"<cim:{root} rdf:about="urn:uuid:{}">"#,
            uuid.to_cim_string()
        ),
    );
}

/// Pascal `TCIMExporter.EndInstance` (`ExportCIMXML.pas:432`). Combined mode
/// closes the one root; fragments mode closes **every** profile that has an open
/// instance, writing the *passed* `root` tag into each (the Pascal quirk — it
/// reuses `Root`, not each profile's own `roots[i]`) and clearing its slot.
pub fn end_instance(buf: &mut Writer, prf: ProfileChoice, root: &str) {
    if !buf.separate {
        write_cim_ln(buf, prf, &format!("</cim:{root}>"));
        return;
    }
    for i in 0..7 {
        if !buf.roots[i].is_empty() {
            buf.bufs[i].push_str(&format!("</cim:{root}>\n"));
            buf.roots[i] = String::new();
        }
    }
}

/// Pascal `TCIMExporterHelper.DoubleNode` (`ExportCIMXML.pas:1340`): `%.8g`
/// (`report::format::g` at 8 significant digits — the WP8.5-audited FPC `%g`).
pub fn double_node(buf: &mut Writer, prf: ProfileChoice, node: &str, val: f64) {
    let s = crate::report::format::g(val, 8);
    write_cim_ln(buf, prf, &format!("  <cim:{node}>{s}</cim:{node}>"));
}

/// Pascal `TCIMExporterHelper.IntegerNode` (`ExportCIMXML.pas:1345`).
pub fn integer_node(buf: &mut Writer, prf: ProfileChoice, node: &str, val: i64) {
    write_cim_ln(buf, prf, &format!("  <cim:{node}>{val}</cim:{node}>"));
}

/// Pascal `TCIMExporterHelper.BooleanNode` (`ExportCIMXML.pas:1350`): lowercase
/// `true`/`false`.
pub fn boolean_node(buf: &mut Writer, prf: ProfileChoice, node: &str, val: bool) {
    let s = if val { "true" } else { "false" };
    write_cim_ln(buf, prf, &format!("  <cim:{node}>{s}</cim:{node}>"));
}

/// Pascal `TCIMExporterHelper.StringNode` (`ExportCIMXML.pas:1526`).
pub fn string_node(buf: &mut Writer, prf: ProfileChoice, node: &str, val: &str) {
    write_cim_ln(buf, prf, &format!("  <cim:{node}>{val}</cim:{node}>"));
}

/// Pascal `TCIMExporterHelper.RefNode`/`UuidNode` (`ExportCIMXML.pas:1361/1366`):
/// both render identically (`<cim:Node rdf:resource="urn:uuid:ID"/>`) — `RefNode`
/// took a `TNamedObject` and read its `.CIM_ID`, `UuidNode` took a raw `TUuid`;
/// since our objects carry their UUID directly (no `TNamedObject` shim), one
/// function serves both call sites.
pub fn ref_node(buf: &mut Writer, prf: ProfileChoice, node: &str, uuid: Uuid) {
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
pub fn circuit_node(buf: &mut Writer, prf: ProfileChoice, feeder_uuid: Uuid) {
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
pub fn phase_kind_node(buf: &mut Writer, prf: ProfileChoice, root: &str, val: &str) {
    write_cim_ln(
        buf,
        prf,
        &format!(r#"  <cim:{root}.phase rdf:resource="{CIM_NS}#SinglePhaseKind.{val}"/>"#),
    );
}

/// Pascal `TCIMExporterHelper.PhaseSideNode` (`ExportCIMXML.pas:1608`): a
/// `<Root>.phaseSide<Side>` reference into `SinglePhaseKind.<val>` (SwitchPhase).
pub fn phase_side_node(buf: &mut Writer, prf: ProfileChoice, root: &str, side: i64, val: &str) {
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
pub fn conductor_usage_enum(buf: &mut Writer, prf: ProfileChoice, val: &str) {
    write_cim_ln(
        buf,
        prf,
        &format!(r#"  <cim:WireSpacingInfo.usage rdf:resource="{CIM_NS}#WireUsageKind.{val}"/>"#),
    );
}

/// Pascal `TCIMExporterHelper.ConductorInsulationEnum` (`ExportCIMXML.pas:1446`):
/// `<cim:WireInfo.insulationMaterial rdf:resource="…#WireInsulationKind.<val>"/>`.
pub fn conductor_insulation_enum(buf: &mut Writer, prf: ProfileChoice, val: &str) {
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
pub fn shunt_connection_kind_node(buf: &mut Writer, prf: ProfileChoice, root: &str, val: &str) {
    write_cim_ln(
        buf,
        prf,
        &format!(
            r#"  <cim:{root}.phaseConnection rdf:resource="{CIM_NS}#PhaseShuntConnectionKind.{val}"/>"#
        ),
    );
}

/// Pascal `TCIMExporterHelper.WindingConnectionKindNode` (`ExportCIMXML.pas:
/// 1620`): `<cim:PowerTransformerEnd.connectionKind rdf:resource="…#
/// WindingConnection.<val>"/>` (D, Y, Z, Yn, Zn, A, I).
pub fn winding_connection_kind_node(buf: &mut Writer, prf: ProfileChoice, val: &str) {
    write_cim_ln(
        buf,
        prf,
        &format!(
            r#"  <cim:PowerTransformerEnd.connectionKind rdf:resource="{CIM_NS}#WindingConnection.{val}"/>"#
        ),
    );
}

/// Pascal `TCIMExporterHelper.WindingConnectionEnum` (`ExportCIMXML.pas:1440`):
/// `<cim:TransformerEndInfo.connectionKind rdf:resource="…#WindingConnection.
/// <val>"/>` — the `TransformerEndInfo` (catalog) counterpart of
/// [`winding_connection_kind_node`].
pub fn winding_connection_enum(buf: &mut Writer, prf: ProfileChoice, val: &str) {
    write_cim_ln(
        buf,
        prf,
        &format!(
            r#"  <cim:TransformerEndInfo.connectionKind rdf:resource="{CIM_NS}#WindingConnection.{val}"/>"#
        ),
    );
}

/// Pascal `TCIMExporterHelper.TransformerControlEnum` (`ExportCIMXML.pas:1482`):
/// the `RatioTapChanger.tculControlMode` line is **commented out** upstream, so
/// this emits **nothing**. Kept as a call site (a no-op) to mirror the Pascal
/// flow 1:1 (`ExportCIMXML.pas:4252` calls `TransformerControlEnum(FunPrf,
/// 'volt')`).
pub fn transformer_control_enum(_buf: &mut Writer, _prf: ProfileChoice, _val: &str) {}

/// Pascal `TCIMExporterHelper.RegulatingControlEnum` (`ExportCIMXML.pas:1434`):
/// `<cim:RegulatingControl.mode rdf:resource="…#RegulatingControlModeKind.<val>"/>`.
pub fn regulating_control_enum(buf: &mut Writer, prf: ProfileChoice, val: &str) {
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
pub fn monitored_phase_node(buf: &mut Writer, prf: ProfileChoice, val: &str) {
    write_cim_ln(
        buf,
        prf,
        &format!(
            r#"  <cim:RegulatingControl.monitoredPhase rdf:resource="{CIM_NS}#PhaseCode.{val}"/>"#
        ),
    );
}

/// Pascal `TCIMExporterHelper.OpLimitDirectionEnum` (`ExportCIMXML.pas:1494`).
pub fn op_limit_direction_enum(buf: &mut Writer, prf: ProfileChoice, val: &str) {
    write_cim_ln(
        buf,
        prf,
        &format!(
            r#"  <cim:OperationalLimitType.direction rdf:resource="{CIM_NS}#OperationalLimitDirectionKind.{val}"/>"#
        ),
    );
}

/// Pascal `TCIMExporterHelper.BatteryStateEnum` (`ExportCIMXML.pas:1408`):
/// `<cim:BatteryUnit.batteryState rdf:resource="…#BatteryStateKind.<state>"/>`.
/// `state` = `charging` (STORE_CHARGING = −1) / `discharging` (STORE_DISCHARGING
/// = +1) / else `waiting`.
pub fn battery_state_enum(buf: &mut Writer, prf: ProfileChoice, storage_state: i32) {
    let s = if storage_state == -1 {
        "charging"
    } else if storage_state == 1 {
        "discharging"
    } else {
        "waiting"
    };
    write_cim_ln(
        buf,
        prf,
        &format!(
            r#"  <cim:BatteryUnit.batteryState rdf:resource="{CIM_NS}#BatteryStateKind.{s}"/>"#
        ),
    );
}

/// Pascal `TCIMExporterHelper.ConverterControlEnum` (`ExportCIMXML.pas:4774`):
/// `<cim:PowerElectronicsConnection.controlMode rdf:resource="…#
/// ConverterControlModeKind.<mode>"/>`. `dynamic` when the DER uses CIM
/// dynamics; else `constantReactivePower` for `VARMODEKVAR` (= 1); else the
/// default `constantPowerFactor` (`VARMODEPF`).
pub fn converter_control_enum(
    buf: &mut Writer,
    prf: ProfileChoice,
    var_mode: i32,
    cim_dynamics: bool,
) {
    let s = if cim_dynamics {
        "dynamic"
    } else if var_mode == 1 {
        "constantReactivePower"
    } else {
        "constantPowerFactor"
    };
    write_cim_ln(
        buf,
        prf,
        &format!(
            r#"  <cim:PowerElectronicsConnection.controlMode rdf:resource="{CIM_NS}#ConverterControlModeKind.{s}"/>"#
        ),
    );
}

/// Pascal `TCIMExporterHelper.NormalOpCatEnum` (`ExportCIMXML.pas:1501`):
/// `<cim:DERNameplateData.normalOPcatKind rdf:resource="…#NormalOPcatKind.
/// <val>"/>` (`catA`/`catB`, DERIEEEType1 CIM dynamics).
pub fn normal_op_cat_enum(buf: &mut Writer, prf: ProfileChoice, val: &str) {
    write_cim_ln(
        buf,
        prf,
        &format!(
            r#"  <cim:DERNameplateData.normalOPcatKind rdf:resource="{CIM_NS}#NormalOPcatKind.{val}"/>"#
        ),
    );
}

/// Pascal `TCIMExporterHelper.PowerFactorExcitationEnum` (`ExportCIMXML.pas:
/// 1513`): `<cim:ConstantPowerFactorSettings.constantPowerFactorExcitationKind
/// rdf:resource="…#ConstantPowerFactorSettingKind.<val>"/>`.
pub fn power_factor_excitation_enum(buf: &mut Writer, prf: ProfileChoice, val: &str) {
    write_cim_ln(
        buf,
        prf,
        &format!(
            r#"  <cim:ConstantPowerFactorSettings.constantPowerFactorExcitationKind rdf:resource="{CIM_NS}#ConstantPowerFactorSettingKind.{val}"/>"#
        ),
    );
}

/// Pascal `TCIMExporterHelper.RemoteInputSignalEnum` (`ExportCIMXML.pas:1519`):
/// `<cim:RemoteInputSignal.remoteSignalType rdf:resource="…#RemoteSignalKind.
/// <val>"/>`.
pub fn remote_input_signal_enum(buf: &mut Writer, prf: ProfileChoice, val: &str) {
    write_cim_ln(
        buf,
        prf,
        &format!(
            r#"  <cim:RemoteInputSignal.remoteSignalType rdf:resource="{CIM_NS}#RemoteSignalKind.{val}"/>"#
        ),
    );
}

/// Pascal `GetBaseVName` (`ExportCIMXML.pas:1309`): `FloatToStrF(val, ffFixed, 6,
/// 4)` = fixed notation, 4 fractional digits (`ffFixed` ignores `Precision`).
/// Uses the ties-away [`ff_fixed`] (like [`op_lim_i_name`]), not native `{:.4}`
/// (ties-to-even): identical on every realistic base voltage, but correct on an
/// exact 4-decimal dyadic tie (odd multiple of 0.03125).
pub fn base_v_name(val: f64) -> String {
    format!("BaseV_{}", ff_fixed(val, 4))
}

/// Pascal `GetOpLimVName` (`ExportCIMXML.pas:1320`): ties-away [`ff_fixed`] (see
/// [`base_v_name`]).
pub fn op_lim_v_name(val: f64) -> String {
    format!("OpLimV_{}", ff_fixed(val, 4))
}

/// FPC `FloatToStrF(v, ffFixed, 6, decimals)` reproduced faithfully for the CIM
/// device-UUID *names* (which are byte-compared, unlike the tolerance-parsed
/// report cells). The one place it differs from Rust's native `{v:.N}` is the
/// **exact-tie** case: FPC rounds ties **away from zero**, Rust's formatter ties
/// **to even**. `f64::round()` is ties-away, so scale → round → rebuild the
/// string from the scaled integer (a second `{:.N}` could re-round). The 6-sig
/// `Precision` is a no-op for the CIM magnitudes (amps < 1e5 at 1 decimal), so it
/// is not modelled. Oracle-proven 2026-07-09: IEEE13's regulator EmergAmps =
/// 2499/2.4 = **exactly** 1041.25 → oracle `1041.3`, native `{:.1}` → `1041.2`.
fn ff_fixed(v: f64, decimals: usize) -> String {
    let factor = 10u64.pow(decimals as u32);
    let scaled = (v.abs() * factor as f64).round() as u64;
    let sign = if v < 0.0 && scaled != 0 { "-" } else { "" };
    let int_part = scaled / factor;
    if decimals == 0 {
        format!("{sign}{int_part}")
    } else {
        let frac = scaled % factor;
        format!("{sign}{int_part}.{frac:0>decimals$}")
    }
}

/// Pascal `GetOpLimIName` (`ExportCIMXML.pas:1330`): `FloatToStrF(_, ffFixed, 6,
/// 1)` for both operands, joined by `_` (FPC ties-away rounding — see
/// [`ff_fixed`]).
pub fn op_lim_i_name(norm: f64, emerg: f64) -> String {
    format!("OpLimI_{}_{}", ff_fixed(norm, 1), ff_fixed(emerg, 1))
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
fn write_preamble(buf: &mut String, cim_ver_uuid: Uuid) {
    let mut ln = |s: &str| {
        buf.push_str(s);
        buf.push('\n');
    };
    ln(r#"<?xml version="1.0" encoding="utf-8"?>"#);
    ln("<!-- un-comment this line to enable validation");
    ln("-->");
    ln(
        r#"<rdf:RDF xmlns:cim="http://iec.ch/TC57/CIM100#" xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">"#,
    );
    ln("<!--");
    ln("-->");
    let cim_id = cim_ver_uuid.to_cim_string();
    ln(&format!(
        r#"<cim:IEC61970CIMVersion rdf:about="urn:uuid:{cim_id}">"#
    ));
    ln("  <cim:IEC61970CIMVersion.version>IEC61970CIM100</cim:IEC61970CIMVersion.version>");
    ln("  <cim:IEC61970CIMVersion.date>2019-04-01</cim:IEC61970CIMVersion.date>");
    ln("</cim:IEC61970CIMVersion>");
}

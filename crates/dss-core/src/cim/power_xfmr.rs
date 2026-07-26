//! CIM100 transformer / autotransformer / regulator export (GAPS_PLAN WPG.18
//! **Stage E**) — Pascal `Common/ExportCIMXML.pas:3783-4270`. Split out of
//! [`super::export`] per `SPLITTING_RULES` as the transformer arm landed (the
//! `ExportCDPSM` control flow calls [`write_transformers`] then
//! [`write_reg_controls`] where its Stage-D `NOT_PORTED` guards sat).
//!
//! Three transformer cases (`ExportCIMXML.pas:3934-3937`): (1) balanced
//! three-phase with no `XfmrCode` → `PowerTransformerEnd` + mesh/core, no tank;
//! (2) with an `XfmrCode` → `TransformerTank`/`TransformerTankEnd` referencing a
//! `TransformerTankInfo`; (3) otherwise → synthesize a `CIMXfmrCode_<name>`
//! tank-info from the transformer's own winding web (Pascal
//! `TXfmrCodeObj.PullFromTransformer`) and write it like case 2. AutoTrans are
//! always balanced-three-phase `PowerTransformerEnd`s (vector group `YNa`/
//! `YNad1`). Every transformer/autotrans is collected into a `PowerTransformer`
//! *bank* ([`CimBank`] = Pascal `TCIMBankObject`), whose vector group is built
//! from the accumulated winding phases/connections.

use std::collections::HashMap;

use crate::circuit::Circuit;
use crate::elements::control::reg_control::RegControl;
use crate::elements::general::xfmr_code::XfmrCodeObj;
use crate::elements::pd::auto_trans::AutoTrans;
use crate::elements::pd::transformer::Transformer;
use crate::elements::pd::winding::{Connection, Winding};
use crate::elements::traits::CktElement;
use crate::exec::registry::DssClass;

use super::export::{
    AUTOTRANS_DSS_OBJ_TYPE, OpLimit, XFMR_DSS_OBJ_TYPE, class_index, phase_order_string,
    phase_string, write_positions,
};
use super::writer::{self, ProfileChoice};
use super::{CimExporter, Uuid, UuidChoice, get_or_create_uuid};

/// Pascal `TCIMBankObject` (`ExportCIMXML.pas:698-805`): the `PowerTransformer`
/// that groups one or more `Transformer`/`AutoTrans` units (the "bank"). Its
/// `vectorGroup` is accumulated from each added unit's winding phases,
/// connections, ground and clock-angle, then built once (`BuildVectorGroup`)
/// before the bank is written.
struct CimBank {
    /// Pascal `localName` — the bank name (`XfmrBank`, or `'=' + unit.Name` when
    /// the unit declares no bank). The leading `=` is stripped only at write.
    local_name: String,
    /// Pascal `pBank.UUID = GetDevUuid(Bank, sBank, 0)`.
    uuid: Uuid,
    /// Pascal `nWindings` — the max winding count over the units in this bank.
    n_windings: usize,
    /// Pascal `bAuto` — set once any autotransformer joins the bank.
    b_auto: bool,
    /// Pascal `pd_unit.Name` — the *last* unit added; the bank's Location key
    /// (`GetDevUuid(XfLoc, pd_unit.Name, 1)`) resolves through it.
    pd_unit_name: String,
    // Per-winding accumulators (index 0-based; length `max_windings`).
    connections: Vec<i32>,
    angles: Vec<i32>,
    phase_a: Vec<i32>,
    phase_b: Vec<i32>,
    phase_c: Vec<i32>,
    ground: Vec<i32>,
    /// Built by [`CimBank::build_vector_group`].
    vector_group: String,
}

impl CimBank {
    /// Pascal `TCIMBankObject.Create(MaxWdg)` (`ExportCIMXML.pas:698`).
    fn new(max_wdg: usize, local_name: String, uuid: Uuid) -> Self {
        Self {
            local_name,
            uuid,
            n_windings: 0,
            b_auto: false,
            pd_unit_name: String::new(),
            connections: vec![0; max_wdg],
            angles: vec![0; max_wdg],
            phase_a: vec![0; max_wdg],
            phase_b: vec![0; max_wdg],
            phase_c: vec![0; max_wdg],
            ground: vec![0; max_wdg],
            vector_group: String::new(),
        }
    }

    /// Pascal `TCIMBankObject.AddTransformer` (`ExportCIMXML.pas:759`).
    fn add_transformer(&mut self, xf: &XfSnap) {
        if xf.num_windings > self.n_windings {
            self.n_windings = xf.num_windings;
        }
        self.pd_unit_name = xf.name.clone();
        for i in 1..=xf.num_windings {
            let w = &xf.wdgs[i - 1];
            if w.phase_str.contains('A') {
                self.phase_a[i - 1] = 1;
            }
            if w.phase_str.contains('B') {
                self.phase_b[i - 1] = 1;
            }
            if w.phase_str.contains('C') {
                self.phase_c[i - 1] = 1;
            }
            self.connections[i - 1] = w.w.connection.ordinal();
            if self.connections[i - 1] != self.connections[0] {
                self.angles[i - 1] = 1;
            }
            if (w.w.rneut >= 0.0 || w.w.xneut > 0.0) && self.connections[i - 1] < 1 {
                self.ground[i - 1] = 1;
            }
        }
    }

    /// Pascal `TCIMBankObject.AddAutoTransformer` (`ExportCIMXML.pas:786`):
    /// 3-phase, 2 or 3 windings; winding 2 is grounded.
    fn add_auto_transformer(&mut self, au: &AutoSnap) {
        if au.num_windings > self.n_windings {
            self.n_windings = au.num_windings;
        }
        self.b_auto = true;
        self.pd_unit_name = au.name.clone();
        for i in 1..=au.num_windings {
            self.phase_a[i - 1] = 1;
            self.phase_b[i - 1] = 1;
            self.phase_c[i - 1] = 1;
            self.connections[i - 1] = au.wdgs[i - 1].conn;
            if i == 2 {
                self.ground[i - 1] = 1;
            }
        }
    }

    /// Pascal `TCIMBankObject.BuildVectorGroup` (`ExportCIMXML.pas:724`).
    fn build_vector_group(&mut self) {
        if self.b_auto {
            self.vector_group = if self.n_windings < 3 {
                "YNa".to_string()
            } else {
                "YNad1".to_string()
            };
            return;
        }
        let mut vg = String::new();
        let mut i = 0; // dynamic arrays are zero-based
        while i < self.n_windings {
            if self.phase_a[i] > 0 && self.phase_b[i] > 0 && self.phase_c[i] > 0 {
                vg.push(if self.connections[i] > 0 { 'd' } else { 'y' });
                if self.ground[i] > 0 {
                    vg.push('n');
                }
                if self.angles[i] > 0 {
                    vg.push_str(&self.angles[i].to_string());
                }
            } else {
                vg.push('i');
            }
            i += 1;
        }
        if !vg.is_empty() {
            // Pascal: AnsiUpperCase(LeftStr(vg,1)) + RightStr(vg, len-1).
            let mut chars = vg.chars();
            let first = chars.next().unwrap().to_ascii_uppercase();
            vg = format!("{first}{}", chars.as_str());
        }
        self.vector_group = vg;
    }
}

/// One winding's export data for a regular transformer (Pascal `Winding[i]`
/// scalars plus the element-level phase/terminal/ground data the CIM ends read).
struct WdgData {
    /// The `Winding` record scalars (`kvll`/`kva`/`connection`/`rneut`/`xneut`/
    /// `rpu`/`num_taps`).
    w: Winding,
    /// Pascal `PhaseString(pXf, i)` — the bank vector-group phase letters.
    phase_str: String,
    /// Pascal `PhaseOrderString(pXf, i)` — the tank `orderedPhases`.
    phase_order: String,
    /// `Terminals[i-1].BusRef` (0-based; `usize::MAX` unset).
    term_bus_ref: usize,
    /// `Buses[BusRef].kVBase`.
    term_bus_kvbase: f64,
    /// `Buses[BusRef].CIM_ID` (rendered), the terminal's `ConnectivityNode`.
    term_bus_cim_id: String,
    /// `NodeRef[(i-1)*NConds + 1]` (Pascal `j1`; 0 = ground).
    node_j1: usize,
    /// `NodeRef[(i-1)*NConds + Nphases + 1]` (Pascal `j2` / the main-sweep `j`).
    node_j2: usize,
}

/// Snapshot of one enabled `Transformer` (all data the CIM arm reads, captured
/// before the mutable write phase to sidestep the class-registry borrow).
struct XfSnap {
    name: String,
    /// Object mRID (`pXf.CIM_ID`).
    uuid: Uuid,
    nphases: usize,
    num_windings: usize,
    xfmr_bank: String,
    /// `Some(tank-info UUID)` iff `xfmrcode=` resolved (case 2); `None` = case 1/3.
    code_uuid: Option<Uuid>,
    bus_specs: Vec<String>,
    wdgs: Vec<WdgData>,
    norm_amps: f64,
    emerg_amps: f64,
    // Case-3 synthesis source (the transformer's own web → `CIMXfmrCode_<name>`).
    xsc: Vec<f64>,
    pct_no_load_loss: f64,
    pct_imag: f64,
    norm_max_hkva: f64,
    emerg_max_hkva: f64,
}

impl XfSnap {
    /// Pascal `sBank`: `XfmrBank`, or `'=' + Name` when no bank declared.
    fn s_bank(&self) -> String {
        if self.xfmr_bank.is_empty() {
            format!("={}", self.name)
        } else {
            self.xfmr_bank.clone()
        }
    }
    /// `bTanks` (`ExportCIMXML.pas:3997-3999`): `FALSE` only for case 1
    /// (no code and three-phase).
    fn b_tanks(&self) -> bool {
        self.code_uuid.is_some() || self.nphases != 3
    }
}

/// One winding's export data for an autotransformer (always balanced 3-phase).
struct AutoWdg {
    kvll: f64,
    kva: f64,
    res: f64,
    conn: i32,
    term_bus_ref: usize,
    term_bus_kvbase: f64,
    term_bus_cim_id: String,
}

/// Snapshot of one enabled `AutoTrans`. (The autotransformer object's own UUID
/// is never referenced by the Pascal arm — ends/terminals hang off the bank and
/// the `Wdg`/`Term` device UUIDs — so it is not captured.)
struct AutoSnap {
    name: String,
    num_windings: usize,
    xfmr_bank: String,
    bus_specs: Vec<String>,
    wdgs: Vec<AutoWdg>,
    xsc: Vec<f64>,
    pct_no_load_loss: f64,
    pct_imag: f64,
    norm_amps: f64,
    emerg_amps: f64,
}

/// The winding-web view [`write_xfmr_code`] reads (Pascal `TXfmrCodeObj`
/// properties): a real `XfmrCode`, or a transformer's own web synthesized as
/// `CIMXfmrCode_<name>` (case 3, Pascal `PullFromTransformer`).
struct XfmrCodeData {
    name: String,
    /// The `TransformerTankInfo` mRID: a real code's object UUID, or the
    /// synthesized `GetDevUuid(TankInfo, name, 1)`.
    tank_info_uuid: Uuid,
    num_windings: usize,
    fnphases: i32,
    windings: Vec<Winding>,
    xsc: Vec<f64>,
    pct_imag: f64,
    pct_no_load_loss: f64,
    norm_max_hkva: f64,
    emerg_max_hkva: f64,
}

/// The case-3 synthesized `XfmrCode` name: `'CIMXfmrCode_' + pXf.Name`, then
/// **lowercased** — Pascal `clsXfCd.NewObject(sBank)` (`ExportCIMXML.pas:3948`)
/// stores the object name through the class hash list, which lowercases it (the
/// oracle emits `cimxfmrcode_<name>` and keys `TankInfo=cimxfmrcode_<name>=1`).
fn synth_code_name(xf_name: &str) -> String {
    format!("CIMXfmrCode_{xf_name}").to_ascii_lowercase()
}

/// Find-or-create the bank named `s_bank` (Pascal `GetBank`/`AddBank` over a
/// case-insensitive `THashList`); banks are written in insertion order.
fn ensure_bank(
    banks: &mut Vec<CimBank>,
    bank_idx: &mut HashMap<String, usize>,
    cim: &mut CimExporter,
    max_wdg: usize,
    s_bank: &str,
) -> usize {
    let key = s_bank.to_ascii_lowercase();
    if let Some(&i) = bank_idx.get(&key) {
        return i;
    }
    let uuid = cim.get_dev_uuid(UuidChoice::Bank, s_bank, 0);
    let i = banks.len();
    banks.push(CimBank::new(max_wdg, s_bank.to_string(), uuid));
    bank_idx.insert(key, i);
    i
}

/// Pascal `TCIMExporterHelper.WriteXfmrCode` (`ExportCIMXML.pas:2133`): the
/// `TransformerTankInfo` + one `TransformerEndInfo` per winding + a `NoLoadTest`
/// + one `ShortCircuitTest` per winding pair.
fn write_xfmr_code(buf: &mut writer::Writer, cim: &mut CimExporter, code: &XfmrCodeData) {
    let nw = code.num_windings;
    let w1_kva = code.windings[0].kva;
    writer::start_instance(
        buf,
        ProfileChoice::Cat,
        "TransformerTankInfo",
        code.tank_info_uuid,
        &code.name,
    );
    writer::end_instance(buf, ProfileChoice::Cat, "TransformerTankInfo");
    let rat_short = code.norm_max_hkva / w1_kva;
    let rat_emerg = code.emerg_max_hkva / w1_kva;
    for i in 1..=nw {
        let w = &code.windings[i - 1];
        let mut zbase = w.kvll;
        zbase = 1000.0 * zbase * zbase / w1_kva;
        let end_uuid = cim.get_dev_uuid(UuidChoice::WdgInf, &code.name, i as i32);
        writer::start_instance(
            buf,
            ProfileChoice::Cat,
            "TransformerEndInfo",
            end_uuid,
            &format!("{}_{i}", code.name),
        );
        writer::ref_node(
            buf,
            ProfileChoice::Cat,
            "TransformerEndInfo.TransformerTankInfo",
            code.tank_info_uuid,
        );
        writer::integer_node(
            buf,
            ProfileChoice::Cat,
            "TransformerEndInfo.endNumber",
            i as i64,
        );
        if code.fnphases < 3 {
            writer::winding_connection_enum(buf, ProfileChoice::Cat, "I");
            let clock = if i == 3 && w.kvll < 0.3 { 6 } else { 0 };
            writer::integer_node(
                buf,
                ProfileChoice::Cat,
                "TransformerEndInfo.phaseAngleClock",
                clock,
            );
        } else {
            if w.connection == Connection::Delta {
                writer::winding_connection_enum(buf, ProfileChoice::Cat, "D");
            } else if w.rneut > 0.0 || w.xneut > 0.0 {
                writer::winding_connection_enum(buf, ProfileChoice::Cat, "Yn");
            } else {
                writer::winding_connection_enum(buf, ProfileChoice::Cat, "Y");
            }
            let clock = if w.connection != code.windings[0].connection {
                1
            } else {
                0
            };
            writer::integer_node(
                buf,
                ProfileChoice::Cat,
                "TransformerEndInfo.phaseAngleClock",
                clock,
            );
        }
        writer::double_node(
            buf,
            ProfileChoice::Cat,
            "TransformerEndInfo.ratedU",
            1000.0 * w.kvll,
        );
        writer::double_node(
            buf,
            ProfileChoice::Cat,
            "TransformerEndInfo.ratedS",
            1000.0 * w.kva,
        );
        writer::double_node(
            buf,
            ProfileChoice::Cat,
            "TransformerEndInfo.shortTermS",
            1000.0 * w.kva * rat_short,
        );
        writer::double_node(
            buf,
            ProfileChoice::Cat,
            "TransformerEndInfo.emergencyS",
            1000.0 * w.kva * rat_emerg,
        );
        writer::double_node(
            buf,
            ProfileChoice::Cat,
            "TransformerEndInfo.r",
            w.rpu * zbase,
        );
        writer::double_node(
            buf,
            ProfileChoice::Cat,
            "TransformerEndInfo.insulationU",
            0.0,
        );
        writer::end_instance(buf, ProfileChoice::Cat, "TransformerEndInfo");
    }
    // NoLoadTest (`ExportCIMXML.pas:2185-2198`).
    let oc_uuid = cim.get_dev_uuid(UuidChoice::OcTest, &code.name, 1);
    writer::start_instance(
        buf,
        ProfileChoice::Cat,
        "NoLoadTest",
        oc_uuid,
        &format!("{}_1", code.name),
    );
    let wdg1_inf = cim.get_dev_uuid(UuidChoice::WdgInf, &code.name, 1);
    writer::ref_node(buf, ProfileChoice::Cat, "NoLoadTest.EnergisedEnd", wdg1_inf);
    writer::double_node(
        buf,
        ProfileChoice::Cat,
        "NoLoadTest.energisedEndVoltage",
        1000.0 * code.windings[0].kvll,
    );
    let pct_iexc =
        (code.pct_imag * code.pct_imag + code.pct_no_load_loss * code.pct_no_load_loss).sqrt();
    writer::double_node(
        buf,
        ProfileChoice::Cat,
        "NoLoadTest.excitingCurrent",
        pct_iexc,
    );
    writer::double_node(
        buf,
        ProfileChoice::Cat,
        "NoLoadTest.excitingCurrentZero",
        pct_iexc,
    );
    let loss = 0.01 * code.pct_no_load_loss * w1_kva;
    writer::double_node(buf, ProfileChoice::Cat, "NoLoadTest.loss", loss);
    writer::double_node(buf, ProfileChoice::Cat, "NoLoadTest.lossZero", loss);
    writer::double_node(
        buf,
        ProfileChoice::Cat,
        "TransformerTest.basePower",
        1000.0 * w1_kva,
    );
    writer::double_node(buf, ProfileChoice::Cat, "TransformerTest.temperature", 50.0);
    writer::end_instance(buf, ProfileChoice::Cat, "NoLoadTest");
    // ShortCircuitTest per winding pair (`ExportCIMXML.pas:2199-2227`).
    let mut seq = 0usize;
    for i in 1..=nw {
        for j in (i + 1)..=nw {
            seq += 1;
            let sc_uuid = cim.get_dev_uuid(UuidChoice::ScTest, &code.name, seq as i32);
            writer::start_instance(
                buf,
                ProfileChoice::Cat,
                "ShortCircuitTest",
                sc_uuid,
                &format!("{}_{seq}", code.name),
            );
            let end_i = cim.get_dev_uuid(UuidChoice::WdgInf, &code.name, i as i32);
            let end_j = cim.get_dev_uuid(UuidChoice::WdgInf, &code.name, j as i32);
            writer::ref_node(
                buf,
                ProfileChoice::Cat,
                "ShortCircuitTest.EnergisedEnd",
                end_i,
            );
            writer::ref_node(
                buf,
                ProfileChoice::Cat,
                "ShortCircuitTest.GroundedEnds",
                end_j,
            );
            writer::integer_node(
                buf,
                ProfileChoice::Cat,
                "ShortCircuitTest.energisedEndStep",
                (code.windings[i - 1].num_taps / 2) as i64,
            );
            writer::integer_node(
                buf,
                ProfileChoice::Cat,
                "ShortCircuitTest.groundedEndStep",
                (code.windings[j - 1].num_taps / 2) as i64,
            );
            let test_kva = w1_kva;
            let mut zbase = code.windings[i - 1].kvll;
            zbase = 1000.0 * zbase * zbase / test_kva;
            let x = code.xsc[seq - 1];
            let r = code.windings[i - 1].rpu + code.windings[j - 1].rpu;
            let val = (r * r + x * x).sqrt() * zbase;
            writer::double_node(
                buf,
                ProfileChoice::Cat,
                "ShortCircuitTest.leakageImpedance",
                val,
            );
            writer::double_node(
                buf,
                ProfileChoice::Cat,
                "ShortCircuitTest.leakageImpedanceZero",
                val,
            );
            writer::double_node(
                buf,
                ProfileChoice::Cat,
                "ShortCircuitTest.loss",
                r * test_kva,
            );
            writer::double_node(
                buf,
                ProfileChoice::Cat,
                "ShortCircuitTest.lossZero",
                r * test_kva,
            );
            writer::double_node(
                buf,
                ProfileChoice::Cat,
                "TransformerTest.basePower",
                1000.0 * test_kva,
            );
            writer::double_node(buf, ProfileChoice::Cat, "TransformerTest.temperature", 50.0);
            writer::end_instance(buf, ProfileChoice::Cat, "ShortCircuitTest");
        }
    }
}

/// Pascal `TCIMExporterHelper.XfmrTankPhasesAndGround` (`ExportCIMXML.pas:1531`):
/// the `TransformerEnd.grounded`/`rground`/`xground` and
/// `TransformerTankEnd.orderedPhases` for a tank winding.
fn xfmr_tank_phases_and_ground(buf: &mut writer::Writer, w: &WdgData) {
    let mut reverse_ground = false;
    let mut wye_ground = false;
    let mut wye_unground = false;
    if w.w.connection == Connection::Delta {
        // delta
        writer::boolean_node(buf, ProfileChoice::Fun, "TransformerEnd.grounded", false);
    } else if w.node_j2 == 0 {
        // last conductor grounded solidly
        writer::boolean_node(buf, ProfileChoice::Fun, "TransformerEnd.grounded", true);
        writer::double_node(buf, ProfileChoice::Ep, "TransformerEnd.rground", 0.0);
        writer::double_node(buf, ProfileChoice::Ep, "TransformerEnd.xground", 0.0);
        wye_ground = true;
    } else if w.node_j1 == 0 {
        // first conductor grounded solidly, reversed
        writer::boolean_node(buf, ProfileChoice::Fun, "TransformerEnd.grounded", true);
        writer::double_node(buf, ProfileChoice::Ep, "TransformerEnd.rground", 0.0);
        writer::double_node(buf, ProfileChoice::Ep, "TransformerEnd.xground", 0.0);
        reverse_ground = true;
    } else if w.w.rneut < 0.0 {
        // probably wye ungrounded
        writer::boolean_node(buf, ProfileChoice::Fun, "TransformerEnd.grounded", false);
        wye_unground = true;
    } else {
        writer::boolean_node(buf, ProfileChoice::Fun, "TransformerEnd.grounded", true);
        writer::double_node(buf, ProfileChoice::Ep, "TransformerEnd.rground", w.w.rneut);
        writer::double_node(buf, ProfileChoice::Ep, "TransformerEnd.xground", w.w.xneut);
    }
    let ordered = if w.phase_order == "s1" {
        "s1N".to_string()
    } else if w.phase_order == "s2" {
        "Ns2".to_string()
    } else if reverse_ground {
        format!("N{}", w.phase_order)
    } else if wye_ground || wye_unground {
        format!("{}N", w.phase_order)
    } else {
        w.phase_order.clone()
    };
    writer::write_cim_ln(
        buf,
        ProfileChoice::Fun,
        &format!(
            r#"  <cim:TransformerTankEnd.orderedPhases rdf:resource="{}#OrderedPhaseCodeKind.{ordered}"/>"#,
            writer::CIM_NS
        ),
    );
}

/// The HV-winding (terminal 1) `OperationalLimitSet` — Pascal
/// `ExportCIMXML.pas:3916-3927/4138-4150`: find-or-create the `(norm, emerg)`
/// current-limit and reference it on terminal 1.
fn hv_current_limit(
    buf: &mut writer::Writer,
    cim: &mut CimExporter,
    op_limits: &mut Vec<OpLimit>,
    op_limit_idx: &mut HashMap<String, usize>,
    norm: f64,
    emerg: f64,
) {
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

/// Pascal `ExportCIMXML.pas:3783-4195` — the transformer / autotransformer /
/// bank arm of `ExportCDPSM` (combined mode). `crs_uuid`/`fdr_uuid`/`sqrt3` are
/// the caller's already-computed constants.
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_transformers(
    buf: &mut writer::Writer,
    classes: &mut [DssClass],
    ckt: &mut Circuit,
    cim: &mut CimExporter,
    op_limits: &mut Vec<OpLimit>,
    op_limit_idx: &mut HashMap<String, usize>,
    crs_uuid: Uuid,
    fdr_uuid: Uuid,
    sqrt3: f64,
) {
    // Aux-list sizing (`3783-3790`): maxWdg = max(3, max NumWindings over enabled
    // transformers). Only regular transformers widen it (autos are ≤ 3).
    let mut max_wdg = 3usize;
    for &r in &ckt.transformers.clone() {
        let Some(t) = classes[r.class_ord()].arena.get::<Transformer>(r.index()) else {
            continue;
        };
        if t.cd.enabled {
            max_wdg = max_wdg.max(t.num_windings().max(0) as usize);
        }
    }

    // --- snapshot every enabled autotransformer + transformer (drops the class
    // borrow before the mutable write phase). ---
    let auto_refs = ckt.auto_transformers.clone();
    let xf_refs = ckt.transformers.clone();

    let mut autos: Vec<AutoSnap> = Vec::new();
    for r in &auto_refs {
        let snap = {
            let Some(au) = classes[r.class_ord()].arena.get::<AutoTrans>(r.index()) else {
                continue;
            };
            if !au.cd.enabled {
                continue;
            }
            let nw = au.num_windings().max(0) as usize;
            let mut wdgs = Vec::with_capacity(nw);
            for i in 1..=nw {
                let bus_ref = au.cd.terminals[i - 1].bus_idx();
                let kvbase = ckt.buses[bus_ref].kv_base;
                let cim_id = get_or_create_uuid(&mut ckt.buses[bus_ref].uuid).to_cim_string();
                wdgs.push(AutoWdg {
                    kvll: au.winding_kvll(i),
                    kva: au.wdg_kva(i),
                    res: au.wdg_resistance(i),
                    conn: au.wdg_connection(i),
                    term_bus_ref: bus_ref,
                    term_bus_kvbase: kvbase,
                    term_bus_cim_id: cim_id,
                });
            }
            let mut xsc = Vec::new();
            for s in 1..=((nw.max(1) - 1) * nw.max(1) / 2) {
                xsc.push(au.xsc_val(s));
            }
            AutoSnap {
                name: au.cd.obj.name().to_string(),
                num_windings: nw,
                xfmr_bank: au.xfmr_bank().to_string(),
                bus_specs: au.cd.bus_names.clone(),
                wdgs,
                xsc,
                pct_no_load_loss: au.pct_no_load_loss(),
                pct_imag: au.pct_imag(),
                norm_amps: au.norm_amps(),
                emerg_amps: au.emerg_amps(),
            }
        };
        autos.push(snap);
    }

    let mut xfs: Vec<XfSnap> = Vec::new();
    for r in &xf_refs {
        let snap = {
            let Some(t) = classes[r.class_ord()].arena.get::<Transformer>(r.index()) else {
                continue;
            };
            if !t.cd.enabled {
                continue;
            }
            let nphases = t.cd.nphases;
            let nw = t.num_windings().max(0) as usize;
            let mut wdgs = Vec::with_capacity(nw);
            for i in 1..=nw {
                let bus_ref = t.cd.terminals[i - 1].bus_idx();
                let kvbase = ckt.buses[bus_ref].kv_base;
                let cim_id = get_or_create_uuid(&mut ckt.buses[bus_ref].uuid).to_cim_string();
                let bus_spec = &t.cd.bus_names[i - 1];
                // Winding `i` is terminal `i-1`; its NodeRef slice gives the phase-1
                // node (`node_j1`) and the neutral node at conductor `nphases`
                // (`node_j2`). `node_ref` may be empty pre-solve → 0 for both.
                let (node_j1, node_j2) = if t.cd.node_ref.is_empty() {
                    (0, 0)
                } else {
                    let nodes = t.cd.term_nodes(i - 1);
                    (
                        nodes.first().copied().unwrap_or(0),
                        nodes.get(nphases).copied().unwrap_or(0),
                    )
                };
                wdgs.push(WdgData {
                    w: t.windings()[i - 1],
                    phase_str: phase_string(bus_spec, nphases, kvbase, true),
                    phase_order: phase_order_string(bus_spec, nphases, kvbase, true),
                    term_bus_ref: bus_ref,
                    term_bus_kvbase: kvbase,
                    term_bus_cim_id: cim_id,
                    node_j1,
                    node_j2,
                });
            }
            XfSnap {
                name: t.cd.obj.name().to_string(),
                uuid: Uuid::create_v4(), // placeholder; replaced below
                nphases,
                num_windings: nw,
                xfmr_bank: t.xfmr_bank().to_string(),
                code_uuid: None, // resolved below (needs mutable classes)
                bus_specs: t.cd.bus_names.clone(),
                wdgs,
                norm_amps: t.norm_amps(),
                emerg_amps: t.emerg_amps(),
                xsc: t.xsc().to_vec(),
                pct_no_load_loss: t.pct_no_load_loss(),
                pct_imag: t.pct_imag(),
                norm_max_hkva: t.norm_max_hkva(),
                emerg_max_hkva: t.emerg_max_hkva(),
            }
        };
        let mut snap = snap;
        snap.uuid = classes[r.class_ord()].arena[r.index()].data_mut().uuid();
        // Resolve the XfmrCode object UUID (case 2) if `xfmrcode=` resolved (and
        // it really resolves to an `XfmrCode` object — else treated as no code).
        let code_ref = classes[r.class_ord()]
            .arena
            .get::<Transformer>(r.index())
            .and_then(|t| t.xfmr_code_ref());
        snap.code_uuid = match code_ref {
            Some(cr)
                if classes[cr.class_ord()]
                    .arena
                    .get::<XfmrCodeObj>(cr.index())
                    .is_some() =>
            {
                Some(classes[cr.class_ord()].arena[cr.index()].data_mut().uuid())
            }
            _ => None,
        };
        xfs.push(snap);
    }

    // --- AutoTransformer sweep (`3804-3932`). ---
    let mut banks: Vec<CimBank> = Vec::new();
    let mut bank_idx: HashMap<String, usize> = HashMap::new();
    for au in &autos {
        let s_bank = if au.xfmr_bank.is_empty() {
            format!("={}", au.name)
        } else {
            au.xfmr_bank.clone()
        };
        let bi = ensure_bank(&mut banks, &mut bank_idx, cim, max_wdg, &s_bank);
        banks[bi].add_auto_transformer(au);
        let geo_uuid = cim.get_dev_uuid(UuidChoice::XfLoc, &au.name, 1);
        write_positions(
            buf,
            ckt,
            cim,
            "AutoTrans",
            &au.name,
            au.num_windings,
            &au.bus_specs,
            &au.wdgs.iter().map(|w| w.term_bus_ref).collect::<Vec<_>>(),
            geo_uuid,
            crs_uuid,
        );
        let bank_uuid = banks[bi].uuid;
        // Core admittance (`3841-3851`).
        let val0 = au.wdgs[0].kvll;
        let zbase0 = 1000.0 * val0 * val0 / au.wdgs[0].kva;
        let core_uuid = cim.get_dev_uuid(UuidChoice::XfCore, &au.name, 1);
        writer::start_instance(
            buf,
            ProfileChoice::Ep,
            "TransformerCoreAdmittance",
            core_uuid,
            &format!("{}_Yc", au.name),
        );
        let g = au.pct_no_load_loss / 100.0 / zbase0;
        writer::double_node(buf, ProfileChoice::Ep, "TransformerCoreAdmittance.g", g);
        writer::double_node(buf, ProfileChoice::Ep, "TransformerCoreAdmittance.g0", g);
        let b = -au.pct_imag / 100.0 / zbase0; // inductive B < 0
        writer::double_node(buf, ProfileChoice::Ep, "TransformerCoreAdmittance.b", b);
        writer::double_node(buf, ProfileChoice::Ep, "TransformerCoreAdmittance.b0", b);
        let wdg1_uuid = cim.get_dev_uuid(UuidChoice::Wdg, &au.name, 1);
        writer::ref_node(
            buf,
            ProfileChoice::Ep,
            "TransformerCoreAdmittance.TransformerEnd",
            wdg1_uuid,
        );
        writer::end_instance(buf, ProfileChoice::Ep, "TransformerCoreAdmittance");
        // Mesh impedances (`3852-3871`).
        let mut seq = 1usize;
        for i in 1..=au.num_windings {
            for k in (i + 1)..=au.num_windings {
                let vali = au.wdgs[i - 1].kvll;
                let zbase = 1000.0 * vali * vali / au.wdgs[0].kva; // always winding-1 kVA
                let mesh_uuid = cim.get_dev_uuid(UuidChoice::XfMesh, &au.name, seq as i32);
                writer::start_instance(
                    buf,
                    ProfileChoice::Ep,
                    "TransformerMeshImpedance",
                    mesh_uuid,
                    &format!("{}_Zsc_{seq}", au.name),
                );
                let r = zbase * (au.wdgs[i - 1].res + au.wdgs[k - 1].res);
                writer::double_node(buf, ProfileChoice::Ep, "TransformerMeshImpedance.r", r);
                writer::double_node(buf, ProfileChoice::Ep, "TransformerMeshImpedance.r0", r);
                let x = zbase * au.xsc[seq - 1];
                seq += 1;
                writer::double_node(buf, ProfileChoice::Ep, "TransformerMeshImpedance.x", x);
                writer::double_node(buf, ProfileChoice::Ep, "TransformerMeshImpedance.x0", x);
                let from_uuid = cim.get_dev_uuid(UuidChoice::Wdg, &au.name, i as i32);
                let to_uuid = cim.get_dev_uuid(UuidChoice::Wdg, &au.name, k as i32);
                writer::ref_node(
                    buf,
                    ProfileChoice::Ep,
                    "TransformerMeshImpedance.FromTransformerEnd",
                    from_uuid,
                );
                writer::ref_node(
                    buf,
                    ProfileChoice::Ep,
                    "TransformerMeshImpedance.ToTransformerEnd",
                    to_uuid,
                );
                writer::end_instance(buf, ProfileChoice::Ep, "TransformerMeshImpedance");
            }
        }
        // Ends + terminals (`3872-3930`).
        for i in 1..=au.num_windings {
            let wdg_uuid = cim.get_dev_uuid(UuidChoice::Wdg, &au.name, i as i32);
            writer::start_instance(
                buf,
                ProfileChoice::Fun,
                "PowerTransformerEnd",
                wdg_uuid,
                &format!("{}_End_{i}", au.name),
            );
            writer::ref_node(
                buf,
                ProfileChoice::Fun,
                "PowerTransformerEnd.PowerTransformer",
                bank_uuid,
            );
            let w = &au.wdgs[i - 1];
            writer::double_node(
                buf,
                ProfileChoice::Ep,
                "PowerTransformerEnd.ratedS",
                1000.0 * w.kva,
            );
            writer::double_node(
                buf,
                ProfileChoice::Ep,
                "PowerTransformerEnd.ratedU",
                1000.0 * w.kvll,
            );
            let zbase = 1000.0 * w.kvll * w.kvll / w.kva;
            writer::double_node(
                buf,
                ProfileChoice::Ep,
                "PowerTransformerEnd.r",
                zbase * w.res,
            );
            if i == 1 {
                writer::winding_connection_kind_node(buf, ProfileChoice::Fun, "Y");
                writer::integer_node(
                    buf,
                    ProfileChoice::Fun,
                    "PowerTransformerEnd.phaseAngleClock",
                    0,
                );
                writer::boolean_node(buf, ProfileChoice::Fun, "TransformerEnd.grounded", false);
            } else if i == 2 {
                writer::winding_connection_kind_node(buf, ProfileChoice::Fun, "A");
                writer::integer_node(
                    buf,
                    ProfileChoice::Fun,
                    "PowerTransformerEnd.phaseAngleClock",
                    0,
                );
                writer::boolean_node(buf, ProfileChoice::Fun, "TransformerEnd.grounded", true);
                writer::double_node(buf, ProfileChoice::Ep, "TransformerEnd.rground", 0.0);
                writer::double_node(buf, ProfileChoice::Ep, "TransformerEnd.xground", 0.0);
            } else {
                writer::winding_connection_kind_node(buf, ProfileChoice::Fun, "D");
                writer::integer_node(
                    buf,
                    ProfileChoice::Fun,
                    "PowerTransformerEnd.phaseAngleClock",
                    1,
                );
                writer::boolean_node(buf, ProfileChoice::Fun, "TransformerEnd.grounded", false);
            }
            writer::integer_node(
                buf,
                ProfileChoice::Fun,
                "TransformerEnd.endNumber",
                i as i64,
            );
            let term_uuid = cim.get_term_uuid(AUTOTRANS_DSS_OBJ_TYPE, &au.name, i as i32);
            writer::ref_node(
                buf,
                ProfileChoice::Fun,
                "TransformerEnd.Terminal",
                term_uuid,
            );
            let basev_uuid = cim.get_base_v_uuid(sqrt3 * w.term_bus_kvbase);
            writer::ref_node(
                buf,
                ProfileChoice::Fun,
                "TransformerEnd.BaseVoltage",
                basev_uuid,
            );
            writer::end_instance(buf, ProfileChoice::Fun, "PowerTransformerEnd");
            // Terminal.
            writer::start_instance(
                buf,
                ProfileChoice::Fun,
                "Terminal",
                term_uuid,
                &format!("{}_T{i}", au.name),
            );
            writer::ref_node(
                buf,
                ProfileChoice::Fun,
                "Terminal.ConductingEquipment",
                bank_uuid,
            );
            writer::integer_node(
                buf,
                ProfileChoice::Fun,
                "ACDCTerminal.sequenceNumber",
                i as i64,
            );
            writer::write_cim_ln(
                buf,
                ProfileChoice::Topo,
                &format!(
                    r#"  <cim:Terminal.ConnectivityNode rdf:resource="urn:uuid:{}"/>"#,
                    w.term_bus_cim_id
                ),
            );
            if i == 1 {
                hv_current_limit(
                    buf,
                    cim,
                    op_limits,
                    op_limit_idx,
                    au.norm_amps,
                    au.emerg_amps,
                );
            }
            writer::end_instance(buf, ProfileChoice::Fun, "Terminal");
        }
    }

    // --- Case-3 pre-pass + write all XfmrCodes (`3941-3962`). ---
    // First the real `XfmrCode` class objects, then the synthesized case-3
    // `CIMXfmrCode_<name>` codes (transformer order) — matching Pascal's
    // `clsXfCd.ElementList` after the pre-pass appends the synthesized ones.
    if let Some(ci) = class_index(classes, "xfmrcode") {
        let n = classes[ci].arena.len();
        for k in 0..n {
            let code = {
                let obj = &classes[ci].arena[k];
                let Some(c) = classes[ci].arena.get::<XfmrCodeObj>(k) else {
                    continue;
                };
                XfmrCodeData {
                    name: obj.data().name().to_string(),
                    tank_info_uuid: Uuid::create_v4(), // replaced below
                    num_windings: c.num_windings().max(0) as usize,
                    fnphases: c.fnphases(),
                    windings: c.windings().to_vec(),
                    xsc: c.xsc().to_vec(),
                    pct_imag: c.pct_imag(),
                    pct_no_load_loss: c.pct_no_load_loss(),
                    norm_max_hkva: c.norm_max_hkva(),
                    emerg_max_hkva: c.emerg_max_hkva(),
                }
            };
            let mut code = code;
            code.tank_info_uuid = classes[ci].arena[k].data_mut().uuid();
            write_xfmr_code(buf, cim, &code);
        }
    }
    for xf in &xfs {
        // Case 3: no code and not three-phase → synthesize CIMXfmrCode_<name>.
        if xf.code_uuid.is_none() && xf.nphases != 3 {
            let name = synth_code_name(&xf.name);
            let tank_info_uuid = cim.get_dev_uuid(UuidChoice::TankInfo, &name, 1);
            let code = XfmrCodeData {
                name,
                tank_info_uuid,
                num_windings: xf.num_windings,
                fnphases: xf.nphases as i32,
                windings: xf.wdgs.iter().map(|w| w.w).collect(),
                xsc: xf.xsc.clone(),
                pct_imag: xf.pct_imag,
                pct_no_load_loss: xf.pct_no_load_loss,
                norm_max_hkva: xf.norm_max_hkva,
                emerg_max_hkva: xf.emerg_max_hkva,
            };
            write_xfmr_code(buf, cim, &code);
        }
    }

    // --- Transformer main sweep, three cases (`3985-4155`). ---
    for xf in &xfs {
        let s_bank = xf.s_bank();
        let bi = ensure_bank(&mut banks, &mut bank_idx, cim, max_wdg, &s_bank);
        banks[bi].add_transformer(xf);
        let bank_uuid = banks[bi].uuid;
        let geo_uuid = cim.get_dev_uuid(UuidChoice::XfLoc, &xf.name, 1);
        let b_tanks = xf.b_tanks();
        // The tank-info reference: case 2 = the code's UUID; case 3 = the
        // synthesized `CIMXfmrCode_<name>` UUID.
        let tank_info_uuid = match xf.code_uuid {
            Some(u) => Some(u),
            None if xf.nphases != 3 => {
                Some(cim.get_dev_uuid(UuidChoice::TankInfo, &synth_code_name(&xf.name), 1))
            }
            None => None,
        };

        if b_tanks {
            writer::start_instance(
                buf,
                ProfileChoice::Fun,
                "TransformerTank",
                xf.uuid,
                &xf.name,
            );
            writer::circuit_node(buf, ProfileChoice::Fun, fdr_uuid);
            writer::ref_node(
                buf,
                ProfileChoice::Fun,
                "TransformerTank.TransformerTankInfo",
                tank_info_uuid.expect("bTanks ⇒ tank-info UUID present"),
            );
            writer::ref_node(
                buf,
                ProfileChoice::Fun,
                "TransformerTank.PowerTransformer",
                bank_uuid,
            );
            writer::ref_node(
                buf,
                ProfileChoice::Geo,
                "PowerSystemResource.Location",
                geo_uuid,
            );
            writer::end_instance(buf, ProfileChoice::Fun, "TransformerTank");
            write_positions(
                buf,
                ckt,
                cim,
                "Transformer",
                &xf.name,
                xf.num_windings,
                &xf.bus_specs,
                &xf.wdgs.iter().map(|w| w.term_bus_ref).collect::<Vec<_>>(),
                geo_uuid,
                crs_uuid,
            );
        } else {
            write_positions(
                buf,
                ckt,
                cim,
                "Transformer",
                &xf.name,
                xf.num_windings,
                &xf.bus_specs,
                &xf.wdgs.iter().map(|w| w.term_bus_ref).collect::<Vec<_>>(),
                geo_uuid,
                crs_uuid,
            );
        }

        if !b_tanks {
            // Mesh impedances + core admittance (case 1, `4035-4068`).
            let val0 = xf.wdgs[0].w.kvll;
            let zbase0 = 1000.0 * val0 * val0 / xf.wdgs[0].w.kva;
            let core_uuid = cim.get_dev_uuid(UuidChoice::XfCore, &xf.name, 1);
            writer::start_instance(
                buf,
                ProfileChoice::Ep,
                "TransformerCoreAdmittance",
                core_uuid,
                &format!("{}_Yc", xf.name),
            );
            let g = xf.pct_no_load_loss / 100.0 / zbase0;
            writer::double_node(buf, ProfileChoice::Ep, "TransformerCoreAdmittance.g", g);
            writer::double_node(buf, ProfileChoice::Ep, "TransformerCoreAdmittance.g0", g);
            let b = xf.pct_imag / 100.0 / zbase0;
            writer::double_node(buf, ProfileChoice::Ep, "TransformerCoreAdmittance.b", b);
            writer::double_node(buf, ProfileChoice::Ep, "TransformerCoreAdmittance.b0", b);
            let wdg1_uuid = cim.get_dev_uuid(UuidChoice::Wdg, &xf.name, 1);
            writer::ref_node(
                buf,
                ProfileChoice::Ep,
                "TransformerCoreAdmittance.TransformerEnd",
                wdg1_uuid,
            );
            writer::end_instance(buf, ProfileChoice::Ep, "TransformerCoreAdmittance");
            let mut seq = 1usize;
            for i in 1..=xf.num_windings {
                for k in (i + 1)..=xf.num_windings {
                    let vali = xf.wdgs[i - 1].w.kvll;
                    let zbase = 1000.0 * vali * vali / xf.wdgs[0].w.kva;
                    let mesh_uuid = cim.get_dev_uuid(UuidChoice::XfMesh, &xf.name, seq as i32);
                    writer::start_instance(
                        buf,
                        ProfileChoice::Ep,
                        "TransformerMeshImpedance",
                        mesh_uuid,
                        &format!("{}_Zsc_{seq}", xf.name),
                    );
                    let r = zbase * (xf.wdgs[i - 1].w.rpu + xf.wdgs[k - 1].w.rpu);
                    writer::double_node(buf, ProfileChoice::Ep, "TransformerMeshImpedance.r", r);
                    writer::double_node(buf, ProfileChoice::Ep, "TransformerMeshImpedance.r0", r);
                    let x = zbase * xf.xsc[seq - 1];
                    seq += 1;
                    writer::double_node(buf, ProfileChoice::Ep, "TransformerMeshImpedance.x", x);
                    writer::double_node(buf, ProfileChoice::Ep, "TransformerMeshImpedance.x0", x);
                    let from_uuid = cim.get_dev_uuid(UuidChoice::Wdg, &xf.name, i as i32);
                    let to_uuid = cim.get_dev_uuid(UuidChoice::Wdg, &xf.name, k as i32);
                    writer::ref_node(
                        buf,
                        ProfileChoice::Ep,
                        "TransformerMeshImpedance.FromTransformerEnd",
                        from_uuid,
                    );
                    writer::ref_node(
                        buf,
                        ProfileChoice::Ep,
                        "TransformerMeshImpedance.ToTransformerEnd",
                        to_uuid,
                    );
                    writer::end_instance(buf, ProfileChoice::Ep, "TransformerMeshImpedance");
                }
            }
        }

        // Ends + terminals (`4070-4153`).
        for i in 1..=xf.num_windings {
            let w = &xf.wdgs[i - 1];
            let wdg_uuid = cim.get_dev_uuid(UuidChoice::Wdg, &xf.name, i as i32);
            if b_tanks {
                writer::start_instance(
                    buf,
                    ProfileChoice::Fun,
                    "TransformerTankEnd",
                    wdg_uuid,
                    &format!("{}_End_{i}", xf.name),
                );
                xfmr_tank_phases_and_ground(buf, w);
                writer::ref_node(
                    buf,
                    ProfileChoice::Fun,
                    "TransformerTankEnd.TransformerTank",
                    xf.uuid,
                );
            } else {
                writer::start_instance(
                    buf,
                    ProfileChoice::Fun,
                    "PowerTransformerEnd",
                    wdg_uuid,
                    &format!("{}_End_{i}", xf.name),
                );
                writer::ref_node(
                    buf,
                    ProfileChoice::Fun,
                    "PowerTransformerEnd.PowerTransformer",
                    bank_uuid,
                );
                writer::double_node(
                    buf,
                    ProfileChoice::Ep,
                    "PowerTransformerEnd.ratedS",
                    1000.0 * w.w.kva,
                );
                writer::double_node(
                    buf,
                    ProfileChoice::Ep,
                    "PowerTransformerEnd.ratedU",
                    1000.0 * w.w.kvll,
                );
                let zbase = 1000.0 * w.w.kvll * w.w.kvll / w.w.kva;
                writer::double_node(
                    buf,
                    ProfileChoice::Ep,
                    "PowerTransformerEnd.r",
                    zbase * w.w.rpu,
                );
                if w.w.connection == Connection::Delta {
                    writer::winding_connection_kind_node(buf, ProfileChoice::Fun, "D");
                } else if w.w.rneut > 0.0 || w.w.xneut > 0.0 {
                    writer::winding_connection_kind_node(buf, ProfileChoice::Fun, "Yn");
                } else {
                    writer::winding_connection_kind_node(buf, ProfileChoice::Fun, "Y");
                }
                let clock = if w.w.connection != xf.wdgs[0].w.connection {
                    1
                } else {
                    0
                };
                writer::integer_node(
                    buf,
                    ProfileChoice::Fun,
                    "PowerTransformerEnd.phaseAngleClock",
                    clock,
                );
                if w.w.connection == Connection::Delta {
                    writer::boolean_node(buf, ProfileChoice::Fun, "TransformerEnd.grounded", false);
                } else if w.node_j2 == 0 {
                    writer::boolean_node(buf, ProfileChoice::Fun, "TransformerEnd.grounded", true);
                    writer::double_node(buf, ProfileChoice::Ep, "TransformerEnd.rground", 0.0);
                    writer::double_node(buf, ProfileChoice::Ep, "TransformerEnd.xground", 0.0);
                } else if w.w.rneut < 0.0 {
                    writer::boolean_node(buf, ProfileChoice::Fun, "TransformerEnd.grounded", false);
                } else {
                    writer::boolean_node(buf, ProfileChoice::Fun, "TransformerEnd.grounded", true);
                    writer::double_node(
                        buf,
                        ProfileChoice::Ep,
                        "TransformerEnd.rground",
                        w.w.rneut,
                    );
                    writer::double_node(
                        buf,
                        ProfileChoice::Ep,
                        "TransformerEnd.xground",
                        w.w.xneut,
                    );
                }
            }
            writer::integer_node(
                buf,
                ProfileChoice::Fun,
                "TransformerEnd.endNumber",
                i as i64,
            );
            let term_uuid = cim.get_term_uuid(XFMR_DSS_OBJ_TYPE, &xf.name, i as i32);
            writer::ref_node(
                buf,
                ProfileChoice::Fun,
                "TransformerEnd.Terminal",
                term_uuid,
            );
            let basev_uuid = cim.get_base_v_uuid(sqrt3 * w.term_bus_kvbase);
            writer::ref_node(
                buf,
                ProfileChoice::Fun,
                "TransformerEnd.BaseVoltage",
                basev_uuid,
            );
            if b_tanks {
                writer::end_instance(buf, ProfileChoice::Fun, "TransformerTankEnd");
            } else {
                writer::end_instance(buf, ProfileChoice::Fun, "PowerTransformerEnd");
            }
            // Terminal.
            writer::start_instance(
                buf,
                ProfileChoice::Fun,
                "Terminal",
                term_uuid,
                &format!("{}_T{i}", xf.name),
            );
            writer::ref_node(
                buf,
                ProfileChoice::Fun,
                "Terminal.ConductingEquipment",
                bank_uuid,
            );
            writer::integer_node(
                buf,
                ProfileChoice::Fun,
                "ACDCTerminal.sequenceNumber",
                i as i64,
            );
            writer::write_cim_ln(
                buf,
                ProfileChoice::Topo,
                &format!(
                    r#"  <cim:Terminal.ConnectivityNode rdf:resource="urn:uuid:{}"/>"#,
                    w.term_bus_cim_id
                ),
            );
            if i == 1 {
                hv_current_limit(
                    buf,
                    cim,
                    op_limits,
                    op_limit_idx,
                    xf.norm_amps,
                    xf.emerg_amps,
                );
            }
            writer::end_instance(buf, ProfileChoice::Fun, "Terminal");
        }
    }

    // --- Bank write (`4157-4173`). ---
    for bank in &mut banks {
        bank.build_vector_group();
        // Strip a leading '=' (temporary, no-bank name); still unique.
        let name = bank
            .local_name
            .strip_prefix('=')
            .unwrap_or(&bank.local_name)
            .to_string();
        writer::start_instance(
            buf,
            ProfileChoice::Fun,
            "PowerTransformer",
            bank.uuid,
            &name,
        );
        writer::circuit_node(buf, ProfileChoice::Fun, fdr_uuid);
        writer::string_node(
            buf,
            ProfileChoice::Fun,
            "PowerTransformer.vectorGroup",
            &bank.vector_group,
        );
        let loc_uuid = cim.get_dev_uuid(UuidChoice::XfLoc, &bank.pd_unit_name, 1);
        writer::ref_node(
            buf,
            ProfileChoice::Geo,
            "PowerSystemResource.Location",
            loc_uuid,
        );
        writer::end_instance(buf, ProfileChoice::Fun, "PowerTransformer");
    }
}

/// Pascal `ExportCIMXML.pas:4198-4270` — the RegControl arm of `ExportCDPSM`:
/// each RegControl on a `Transformer` (autotransformer-controlled ones are
/// skipped, `4202`) → a `TapChangerControl` + `RatioTapChanger`.
pub(crate) fn write_reg_controls(
    buf: &mut writer::Writer,
    classes: &mut [DssClass],
    ckt: &Circuit,
    cim: &mut CimExporter,
) {
    for cr in &ckt.controls.clone() {
        // Snapshot the RegControl + its controlled transformer.
        let snap = {
            let Some(reg) = classes[cr.class_ord()].arena.get::<RegControl>(cr.index()) else {
                continue;
            };
            let Some(tref) = reg.controlled_ref() else {
                continue;
            };
            // Skip if the controlled element is not a plain Transformer
            // (AutoTrans-RegControl skipped upstream, `4202`).
            let Some(tr) = classes[tref.class_ord()]
                .arena
                .get::<Transformer>(tref.index())
            else {
                continue;
            };
            let trw = reg.tr_winding();
            let trw_u = trw.max(1) as usize; // 1-based tapped winding
            // `v1 = tr.BaseVoltage[TrWinding] / PTRatio` (`4207`).
            let v1 = tr.base_voltage(trw_u) / reg.pt_ratio();
            // `FirstPhaseString(tr, TrWinding)` = LeftStr(PhaseString(tr,w),1)|'A'.
            let wi = (trw_u - 1).min(tr.cd.bus_names.len().saturating_sub(1));
            let phs = phase_string(
                &tr.cd.bus_names[wi],
                tr.cd.nphases,
                ckt.buses[tr.cd.terminals[wi].bus_idx()].kv_base,
                true,
            );
            let first_phase = phs.chars().next().unwrap_or('A').to_string();
            let tap_num = reg.tap_num_live(tr);
            RegSnap {
                local_name: reg.ccd.cd.obj.name().to_string(),
                enabled: reg.ccd.cd.enabled,
                tr_name: tr.cd.obj.name().to_string(),
                tr_winding: trw,
                v1,
                first_phase,
                vreg: reg.vreg(),
                bandwidth: reg.bandwidth(),
                ldc_active: reg.ldc_active(),
                r: reg.ldc_r(),
                x: reg.ldc_x(),
                is_reversible: reg.is_reversible(),
                reverse_neutral: reg.reverse_neutral(),
                rev_delay: reg.rev_delay(),
                rev_power_threshold: reg.rev_power_threshold(),
                rev_r: reg.rev_r(),
                rev_x: reg.rev_x(),
                rev_vreg: reg.rev_vreg(),
                rev_bandwidth: reg.rev_bandwidth(),
                vlimit_active: reg.vlimit() > 0.0,
                vlimit: reg.vlimit(),
                max_tap: tr.winding_tap_data(trw_u).1,
                min_tap: tr.winding_tap_data(trw_u).2,
                tap_increment: tr.winding_tap_data(trw_u).3,
                num_taps: tr.winding_num_taps(trw_u),
                time_delay: reg.ccd.time_delay,
                tap_delay: reg.tap_delay(),
                pt_ratio: reg.pt_ratio(),
                ct_rating: reg.ct_rating(),
                tap_num,
            }
        };
        let reg_uuid = classes[cr.class_ord()].arena[cr.index()].data_mut().uuid();
        write_one_reg_control(buf, cim, &snap, reg_uuid);
    }
}

/// Snapshot of one Transformer-controlling RegControl for the CIM arm.
struct RegSnap {
    local_name: String,
    enabled: bool,
    tr_name: String,
    tr_winding: i32,
    v1: f64,
    first_phase: String,
    vreg: f64,
    bandwidth: f64,
    ldc_active: bool,
    r: f64,
    x: f64,
    is_reversible: bool,
    reverse_neutral: bool,
    rev_delay: f64,
    rev_power_threshold: f64,
    rev_r: f64,
    rev_x: f64,
    rev_vreg: f64,
    rev_bandwidth: f64,
    vlimit_active: bool,
    vlimit: f64,
    max_tap: f64,
    min_tap: f64,
    tap_increment: f64,
    num_taps: i32,
    time_delay: f64,
    tap_delay: f64,
    pt_ratio: f64,
    ct_rating: f64,
    tap_num: i32,
}

fn write_one_reg_control(
    buf: &mut writer::Writer,
    cim: &mut CimExporter,
    reg: &RegSnap,
    reg_uuid: Uuid,
) {
    // TapChangerControl (`4208-4245`).
    let ctrl_uuid = cim.get_dev_uuid(UuidChoice::TapCtrl, &reg.local_name, 1);
    writer::start_instance(
        buf,
        ProfileChoice::Fun,
        "TapChangerControl",
        ctrl_uuid,
        &format!("{}_Ctrl", reg.local_name),
    );
    writer::regulating_control_enum(buf, ProfileChoice::Fun, "voltage");
    let term_uuid = cim.get_term_uuid(XFMR_DSS_OBJ_TYPE, &reg.tr_name, reg.tr_winding);
    writer::ref_node(
        buf,
        ProfileChoice::Fun,
        "RegulatingControl.Terminal",
        term_uuid,
    );
    writer::monitored_phase_node(buf, ProfileChoice::Fun, &reg.first_phase);
    writer::boolean_node(
        buf,
        ProfileChoice::Fun,
        "RegulatingControl.enabled",
        reg.enabled,
    );
    writer::boolean_node(buf, ProfileChoice::Ep, "RegulatingControl.discrete", true);
    writer::double_node(
        buf,
        ProfileChoice::Ep,
        "RegulatingControl.targetValue",
        reg.vreg,
    );
    writer::double_node(
        buf,
        ProfileChoice::Ep,
        "RegulatingControl.targetDeadband",
        reg.bandwidth,
    );
    writer::boolean_node(
        buf,
        ProfileChoice::Ep,
        "TapChangerControl.lineDropCompensation",
        reg.ldc_active,
    );
    writer::double_node(buf, ProfileChoice::Ep, "TapChangerControl.lineDropR", reg.r);
    writer::double_node(buf, ProfileChoice::Ep, "TapChangerControl.lineDropX", reg.x);
    if reg.is_reversible {
        writer::boolean_node(buf, ProfileChoice::Ep, "TapChangerControl.reversible", true);
        writer::boolean_node(
            buf,
            ProfileChoice::Ep,
            "TapChangerControl.reverseToNeutral",
            reg.reverse_neutral,
        );
        writer::double_node(
            buf,
            ProfileChoice::Ep,
            "TapChangerControl.reversingDelay",
            reg.rev_delay,
        );
        writer::double_node(
            buf,
            ProfileChoice::Ep,
            "TapChangerControl.reversingPowerThreshold",
            reg.rev_power_threshold,
        );
        writer::double_node(
            buf,
            ProfileChoice::Ep,
            "TapChangerControl.reverseLineDropR",
            reg.rev_r,
        );
        writer::double_node(
            buf,
            ProfileChoice::Ep,
            "TapChangerControl.reverseLineDropX",
            reg.rev_x,
        );
        writer::double_node(
            buf,
            ProfileChoice::Ep,
            "RegulatingControl.reverseTargetValue",
            reg.rev_vreg,
        );
        writer::double_node(
            buf,
            ProfileChoice::Ep,
            "RegulatingControl.reverseTargetDeadband",
            reg.rev_bandwidth,
        );
    } else {
        writer::boolean_node(
            buf,
            ProfileChoice::Ep,
            "TapChangerControl.reversible",
            false,
        );
    }
    if reg.vlimit_active {
        writer::double_node(
            buf,
            ProfileChoice::Ep,
            "TapChangerControl.maxLimitVoltage",
            reg.vlimit,
        );
    } else {
        writer::double_node(
            buf,
            ProfileChoice::Ep,
            "TapChangerControl.maxLimitVoltage",
            reg.max_tap * reg.v1,
        );
    }
    writer::double_node(
        buf,
        ProfileChoice::Ep,
        "TapChangerControl.minLimitVoltage",
        reg.min_tap * reg.v1,
    );
    let loc_uuid = cim.get_dev_uuid(UuidChoice::XfLoc, &reg.tr_name, 1);
    writer::ref_node(
        buf,
        ProfileChoice::Geo,
        "PowerSystemResource.Location",
        loc_uuid,
    );
    writer::end_instance(buf, ProfileChoice::Fun, "TapChangerControl");

    // RatioTapChanger (`4247-4268`).
    writer::start_instance(
        buf,
        ProfileChoice::Fun,
        "RatioTapChanger",
        reg_uuid,
        &reg.local_name,
    );
    let wdg_uuid = cim.get_dev_uuid(UuidChoice::Wdg, &reg.tr_name, reg.tr_winding);
    writer::ref_node(
        buf,
        ProfileChoice::Fun,
        "RatioTapChanger.TransformerEnd",
        wdg_uuid,
    );
    writer::ref_node(
        buf,
        ProfileChoice::Fun,
        "TapChanger.TapChangerControl",
        ctrl_uuid,
    );
    writer::double_node(
        buf,
        ProfileChoice::Ep,
        "RatioTapChanger.stepVoltageIncrement",
        100.0 * reg.tap_increment,
    );
    writer::transformer_control_enum(buf, ProfileChoice::Fun, "volt");
    writer::integer_node(
        buf,
        ProfileChoice::Ep,
        "TapChanger.highStep",
        (reg.num_taps / 2) as i64,
    );
    writer::integer_node(
        buf,
        ProfileChoice::Ep,
        "TapChanger.lowStep",
        -(reg.num_taps / 2) as i64,
    );
    writer::integer_node(buf, ProfileChoice::Ep, "TapChanger.neutralStep", 0);
    writer::integer_node(buf, ProfileChoice::Ep, "TapChanger.normalStep", 0);
    writer::double_node(
        buf,
        ProfileChoice::Ep,
        "TapChanger.neutralU",
        reg.v1 * reg.pt_ratio,
    );
    writer::double_node(
        buf,
        ProfileChoice::Ep,
        "TapChanger.initialDelay",
        reg.time_delay,
    );
    writer::double_node(
        buf,
        ProfileChoice::Ep,
        "TapChanger.subsequentDelay",
        reg.tap_delay,
    );
    writer::boolean_node(buf, ProfileChoice::Ep, "TapChanger.ltcFlag", true);
    writer::boolean_node(
        buf,
        ProfileChoice::Ssh,
        "TapChanger.controlEnabled",
        reg.enabled,
    );
    writer::double_node(
        buf,
        ProfileChoice::Ssh,
        "TapChanger.step",
        reg.tap_num as f64,
    );
    writer::double_node(buf, ProfileChoice::Ep, "TapChanger.ptRatio", reg.pt_ratio);
    writer::double_node(
        buf,
        ProfileChoice::Ep,
        "TapChanger.ctRatio",
        reg.ct_rating / 0.2,
    );
    writer::double_node(buf, ProfileChoice::Ep, "TapChanger.ctRating", reg.ct_rating);
    let loc_uuid = cim.get_dev_uuid(UuidChoice::XfLoc, &reg.tr_name, 1);
    writer::ref_node(
        buf,
        ProfileChoice::Geo,
        "PowerSystemResource.Location",
        loc_uuid,
    );
    writer::end_instance(buf, ProfileChoice::Fun, "RatioTapChanger");
}

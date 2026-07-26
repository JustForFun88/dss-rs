//! `circuit_to_json` — the whole-circuit AltDSS JSON dump, a loop-for-loop port
//! of `Obj_Circuit_ToJSON_` (`.inputs/dss_capi/src/CAPI/CAPI_Obj.pas:2513-2672`)
//! and its `saveOpenTerminalsJSON` helper (`:2470-2511`), with the bus-level
//! renderer `alt_Bus_ToJSON_` (`CAPI_Alt.pas:2820-2832`).
//!
//! The circuit is serialized **always pretty** (`circ.FormatJSON()` = indent 2,
//! `CAPI_Obj.pas:2656`), regardless of the `Pretty` option bit — so the whole
//! tree (including every embedded object) is written with [`write_pretty`]. Each
//! embedded object comes from the same [`obj_to_json_data`] used by
//! `Obj_ToJSON`/`Batch_ToJSON`, driven by the same `joptions`.
//!
//! ## PostCommands number formats (each a `TODO(compat)`)
//! The ~33 `Set …` PostCommands reproduce the exact FPC `Format` specs the
//! oracle uses (`%-g` → default 15-significant `%g`, `%-.4g` → 4-significant,
//! `%8.2f` → width-8 fixed 2-decimal, `IntToStr`, `StrYorN` → `Yes`/`No`,
//! `GetDSSArray`/`IntArrayToString`). These are pinned byte-for-byte by the
//! circuit goldens; the clean fix (a canonical machine format) would be
//! indistinguishable from a porting bug against those goldens.

use std::collections::HashMap;

use crate::circuit::Circuit;
use crate::exec::registry::DssClass;
use crate::obj::base::DssObject;
use crate::obj::dss_enum::EnumRegistry;
use crate::report::format::{fixed_w_fpc, g};
use crate::report::save::dump::commands::PASCAL_CLASS_ORDER;

use super::build::obj_to_json_data;
use super::{Json, JsonOpts, write_pretty};

/// `ALTDSS_SCHEMA_ID` (`CAPI_Obj.pas`): the AltDSS JSON schema URL.
const ALTDSS_SCHEMA_ID: &str = "https://dss-extensions.org/altdss-schema/2023-12-13.schema.json";

/// Pascal `StrYOrN` (`Utilities.pas:173`): `Yes`/`No`.
fn str_y_or_n(b: bool) -> &'static str {
    if b { "Yes" } else { "No" }
}

/// Pascal `GetDSSArray(dbls, scale=1)` (`Utilities.pas:1533-1556`): `[` then one
/// ` %g` (a leading space + FPC default-15-significant `%g`) per value, then `]`
/// — e.g. `[ 0.208 0.48 12.47]`.
///
/// TODO(compat): the `%g` fidelity (15-significant general format). The clean fix
/// is a canonical numeric format; the goldens pin this exact spelling.
///
/// Pascal returns the empty string (not `[]`) for a NIL/empty array (`dbls = NIL`
/// → `Result := ''`); the empty `ArrayOfDouble` overload passes `@dbls[0] = NIL`.
/// Reproduced 1:1. Unreachable via current commands (LegalVoltageBases keeps its
/// DSS defaults and `set voltagebases=()` is a no-op; the harmonic list defaults
/// to `do_all_harmonics`), so it is not exercised by a golden — faithful only.
fn get_dss_array(dbls: &[f64]) -> String {
    if dbls.is_empty() {
        return String::new();
    }
    let mut s = String::from("[");
    for &v in dbls {
        s.push(' ');
        s.push_str(&g(v, 15));
    }
    s.push(']');
    s
}

/// Pascal `IntArrayToString` (`Utilities.pas:358`): `[NULL]` when empty, else
/// `[a, b, c]`.
fn int_array_to_string(arr: &[i32]) -> String {
    if arr.is_empty() {
        return "[NULL]".to_string();
    }
    let mut s = String::from("[");
    for (i, v) in arr.iter().enumerate() {
        if i != 0 {
            s.push_str(", ");
        }
        s.push_str(&v.to_string());
    }
    s.push(']');
    s
}

/// Pascal `CheckForBlanks` (`Utilities.pas:1212`): quote a name that contains a
/// space (unless it already opens with a bracket/quote).
fn check_for_blanks(s: &str) -> String {
    crate::util::check_for_blanks(s)
}

/// Pascal `alt_Bus_ToJSON_` (`CAPI_Alt.pas:2820-2832`): the per-bus JSON object.
/// `Name`, then `X`/`Y` (only when coordinates are defined), `kVLN` (only when
/// the base kV is non-zero) and `Keep` (only when set).
fn bus_to_json(name: &str, bus: &crate::circuit::Bus) -> Json {
    let mut m: Vec<(String, Json)> = Vec::with_capacity(5);
    m.push(("Name".to_string(), Json::Str(name.to_string())));
    if bus.coord_defined {
        m.push(("X".to_string(), Json::Float(bus.x)));
        m.push(("Y".to_string(), Json::Float(bus.y)));
    }
    if bus.kv_base != 0.0 {
        m.push(("kVLN".to_string(), Json::Float(bus.kv_base)));
    }
    if bus.keep {
        m.push(("Keep".to_string(), Json::Bool(true)));
    }
    Json::Obj(m)
}

/// Pascal `saveOpenTerminalsJSON` (`CAPI_Obj.pas:2470-2511`): append an `Open …`
/// command per open terminal/conductor of every circuit element that is not
/// fully closed. Walks `ckt.CktElements` in creation order.
fn save_open_terminals(ckt: &Circuit, classes: &[DssClass], cmds: &mut Vec<Json>) {
    for r in &ckt.ckt_elements {
        let obj = classes[r.class_ord()].arena.obj(r.index());
        let Some(elem) = classes[r.class_ord()].arena.try_ckt_elem(r.index()) else {
            continue;
        };
        let cd = elem.cd();
        if cd.all_conductors_closed() {
            continue;
        }
        let full_name = format!(
            "{}.{}",
            classes[r.class_ord()].props.class_name(),
            obj.data().name()
        );
        let name = check_for_blanks(&full_name);
        for term_idx in 0..cd.nterms {
            let num_cond_open = (0..cd.nconds)
                .filter(|&i| !cd.conductor_closed(term_idx + 1, i + 1))
                .count();
            if num_cond_open == 0 {
                continue;
            }
            if num_cond_open == cd.nconds {
                // Open all conductors in the terminal, easy path.
                cmds.push(Json::Str(format!("Open {name} {}", term_idx + 1)));
                continue;
            }
            // Open specific conductors.
            for i in 0..cd.nconds {
                if cd.conductor_closed(term_idx + 1, i + 1) {
                    continue;
                }
                cmds.push(Json::Str(format!("Open {name} {} {}", term_idx + 1, i + 1)));
            }
        }
    }
}

/// Build the `PreCommands` array (`CAPI_Obj.pas:2533-2551`): the optional
/// save stamp, the `CktModel`/`AllowDuplicates`/`LongLineCorrection`
/// conditionals, then `EarthModel` and `VoltageBases`.
fn pre_commands(
    ckt: &Circuit,
    enums: &EnumRegistry,
    default_earth_model: i32,
    opts: JsonOpts,
) -> Vec<Json> {
    let mut cmds: Vec<Json> = Vec::new();

    if !opts.contains(JsonOpts::SKIP_TIMESTAMP) {
        // Pascal emits `! Last saved by AltDSS/<VersionString> on <ISO8601 now>`
        // — a build/date-dependent stamp. This port emits a fixed deterministic
        // comment instead (a comment, ignored on re-compile). The goldens always
        // pass SkipTimestamp, so this line is never gated; matching Pascal's
        // exact non-deterministic bytes is neither possible nor useful.
        cmds.push(Json::Str(
            "! Saved by dss-rs (AltDSS JSON export, Pascal DSS C-API 1:1 port)".to_string(),
        ));
    }

    if ckt.positive_sequence {
        // Pascal `CktModelEnum.OrdinalToString(Integer(ckt.PositiveSequence))`.
        // TODO(compat): `PositiveSequence` is a Pascal `LongBool`, so
        // `Integer(True)` is **-1** (all-ones), which falls outside the enum's
        // [0,1] ordinal range → `OrdinalToString` returns `''`. The line is
        // therefore always `Set CktModel=` (empty) when positive-sequence is on.
        // Reproduced 1:1 (`ordinal_to_string(-1)` yields the same empty string).
        // The clean fix is `OrdinalToString(1)` → `Positive`.
        let model = enums.get(enums.ckt_model).ordinal_to_string(-1);
        cmds.push(Json::Str(format!("Set CktModel={model}")));
    }
    if ckt.duplicates_allowed {
        cmds.push(Json::Str("Set AllowDuplicates=True".to_string()));
    }
    if ckt.long_line_correction {
        cmds.push(Json::Str("Set LongLineCorrection=True".to_string()));
    }

    let earth = enums
        .get(enums.earth_model)
        .ordinal_to_string(default_earth_model);
    cmds.push(Json::Str(format!("Set EarthModel={earth}")));
    cmds.push(Json::Str(format!(
        "Set VoltageBases={}",
        get_dss_array(&ckt.legal_voltage_bases)
    )));

    cmds
}

/// Build the `PostCommands` array (`CAPI_Obj.pas:2568-2607`): the ~33 solution /
/// options `Set …` strings with their exact FPC formats, then the
/// `saveOpenTerminalsJSON` `Open …` lines.
fn post_commands(ckt: &Circuit, classes: &[DssClass], enums: &EnumRegistry) -> Vec<Json> {
    let sol = &ckt.solution;
    let aa = &ckt.auto_add_obj;
    let mut cmds: Vec<Json> = Vec::new();

    // Scope the mutable-borrowing closure so the borrow ends before
    // `save_open_terminals` takes its own `&mut cmds` below.
    {
        let mut push = |s: String| cmds.push(Json::Str(s));

        push(format!(
            "Set ControlMode={}",
            enums
                .get(enums.control_mode)
                .ordinal_to_string(sol.control_mode.ordinal())
        ));
        push(format!(
            "Set Random={}",
            enums
                .get(enums.random_mode)
                .ordinal_to_string(sol.random_type.ordinal())
        ));
        // `%-g` = FPC default-15-significant general format (TODO(compat), see module).
        push(format!("Set frequency={}", g(sol.frequency, 15)));
        push(format!("Set stepsize={}", g(sol.h, 15)));
        push(format!("Set number={}", sol.number_of_times));
        push(format!(
            "Set tolerance={}",
            g(sol.convergence_tolerance, 15)
        ));
        push(format!("Set maxiterations={}", sol.max_iterations));
        push(format!("Set miniterations={}", sol.min_iterations));
        push(format!(
            "Set loadmodel={}",
            enums
                .get(enums.default_load_model)
                .ordinal_to_string(sol.load_model.ordinal())
        ));
        push(format!("Set loadmult={}", g(ckt.load_multiplier, 15)));
        push(format!("Set Normvminpu={}", g(ckt.normal_min_volts, 15)));
        push(format!("Set Normvmaxpu={}", g(ckt.normal_max_volts, 15)));
        push(format!("Set Emergvminpu={}", g(ckt.emerg_min_volts, 15)));
        push(format!("Set Emergvmaxpu={}", g(ckt.emerg_max_volts, 15)));
        // `%-.4g` = 4-significant general format (TODO(compat)).
        let daily_mean = ckt
            .default_daily_shape_obj
            .as_ref()
            .map_or(0.0, |s| s.mean());
        let daily_std = ckt
            .default_daily_shape_obj
            .as_ref()
            .map_or(0.0, |s| s.std_dev());
        push(format!("Set %mean={}", g(daily_mean * 100.0, 4)));
        push(format!("Set %stddev={}", g(daily_std * 100.0, 4)));
        // `NameIfNotNil(LoadDurCurveObj)` → the name or the empty string.
        let ld_curve = ckt
            .load_dur_curve_obj
            .as_ref()
            .map(|s| s.data().name().to_string())
            .unwrap_or_default();
        push(format!("Set LDCurve={ld_curve}"));
        push(format!(
            "Set %growth={}",
            g((ckt.default_growth_rate - 1.0) * 100.0, 4)
        ));
        push(format!("Set genkw={}", g(aa.gen_kw, 15)));
        push(format!("Set genpf={}", g(aa.gen_pf, 15)));
        push(format!("Set capkvar={}", g(aa.cap_kvar, 15)));
        push(format!(
            "Set addtype={}",
            enums
                .get(enums.add_type)
                .ordinal_to_string(aa.add_type.ordinal())
        ));
        push(format!("Set zonelock={}", str_y_or_n(ckt.zones_locked)));
        // `%8.2f` = width-8 fixed 2-decimal, right-justified (TODO(compat)).
        // Byte-exact FPC `ffFixed`: 15-sig intermediate + ties-away rounding
        // (see `fixed_w_fpc`); pinned by `circuit_positive_seq`'s fractional
        // weights, which Rust's native `{:.2}` renders differently.
        push(format!("Set ueweight={}", fixed_w_fpc(ckt.ue_weight, 8, 2)));
        push(format!(
            "Set lossweight={}",
            fixed_w_fpc(ckt.loss_weight, 8, 2)
        ));
        push(format!("Set ueregs={}", int_array_to_string(&ckt.ue_regs)));
        push(format!(
            "Set lossregs={}",
            int_array_to_string(&ckt.loss_regs)
        ));
        push(format!(
            "Set algorithm={}",
            enums
                .get(enums.solve_alg)
                .ordinal_to_string(sol.algorithm.ordinal())
        ));
        push(format!(
            "Set Trapezoidal={}",
            str_y_or_n(ckt.trapezoidal_integration)
        ));
        push(format!("Set genmult={}", g(ckt.gen_multiplier, 15)));
        push(format!("Set Basefrequency={}", g(ckt.fundamental, 15)));
        if sol.do_all_harmonics {
            push("Set harmonics=ALL".to_string());
        } else {
            push(format!(
                "Set harmonics={}",
                get_dss_array(&sol.harmonic_list)
            ));
        }
        push(format!("Set maxcontroliter={}", sol.max_control_iterations));
    }

    save_open_terminals(ckt, classes, &mut cmds);
    cmds
}

/// Pascal `Obj_Circuit_ToJSON_`: build and serialize the whole-circuit JSON dump.
/// Always pretty (`FormatJSON()`), regardless of `opts.PRETTY`.
#[allow(clippy::too_many_arguments)]
pub fn circuit_to_json(
    ckt: &Circuit,
    classes: &[DssClass],
    class_by_name: &HashMap<String, usize>,
    enums: &EnumRegistry,
    default_base_freq: f64,
    default_earth_model: i32,
    opts: JsonOpts,
) -> String {
    let export_default_objs = opts.contains(JsonOpts::INCLUDE_DEFAULT_OBJS);

    let mut circ: Vec<(String, Json)> = vec![
        (
            "$schema".to_string(),
            Json::Str(ALTDSS_SCHEMA_ID.to_string()),
        ),
        ("Name".to_string(), Json::Str(ckt.name.clone())),
        (
            "DefaultBaseFreq".to_string(),
            Json::Float(default_base_freq),
        ),
        (
            "PreCommands".to_string(),
            Json::Arr(pre_commands(ckt, enums, default_earth_model, opts)),
        ),
    ];

    // `Bus` array (unless SkipBuses): `for i := 1 to NumBuses`.
    if !opts.contains(JsonOpts::SKIP_BUSES) {
        let mut buses: Vec<Json> = Vec::with_capacity(ckt.buses.len());
        for (i, bus) in ckt.buses.iter().enumerate() {
            let name = ckt.bus_list.name(i).unwrap_or("");
            buses.push(bus_to_json(name, bus));
        }
        circ.push(("Bus".to_string(), Json::Arr(buses)));
    }

    circ.push((
        "PostCommands".to_string(),
        Json::Arr(post_commands(ckt, classes, enums)),
    ));

    // One key per DSS class → array of `obj_to_json_data`, in `DSSClassList`
    // order (`PASCAL_CLASS_ORDER`; the Rust registry groups DSS_OBJECT classes
    // ahead of circuit-element classes internally). `DefaultAndUnedited` objects
    // are skipped unless `IncludeDefaultObjs`; an empty class array is omitted.
    for class_name in PASCAL_CLASS_ORDER {
        let Some(&ci) = class_by_name.get(&class_name.to_ascii_lowercase()) else {
            continue;
        };
        let cls = &classes[ci];
        let mut arr: Vec<Json> = Vec::with_capacity(cls.arena.len());
        for obj in cls.arena.objs() {
            if export_default_objs || !obj.data().default_and_unedited() {
                arr.push(obj_to_json_data(&cls.props, obj, enums, opts));
            }
        }
        if !arr.is_empty() {
            circ.push((cls.props.class_name().to_string(), Json::Arr(arr)));
        }
    }

    let root = Json::Obj(circ);
    let mut out = String::new();
    write_pretty(&root, 0, &mut out);
    out
}

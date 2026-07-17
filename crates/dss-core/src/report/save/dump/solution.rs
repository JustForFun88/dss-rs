//! `TSolutionObj.DumpProperties` (Pascal `Solution.pas:1768-1891`): the
//! `! OPTIONS` / `Set …` option listing appended to every whole-circuit
//! `Dump` and produced alone by `Dump solution`. The `Leaf`-gated lines print
//! only on the Dump paths (both pass `Leaf=TRUE`); the Save `IncludeOptions`
//! path (WP8.5 step 5) reuses this with `Leaf=FALSE`. `Complete` (bare
//! `Dump debug`) additionally dumps the factored system Y in compressed
//! column order.

use num_complex::Complex64;
use std::collections::BTreeMap;

use crate::circuit::{AddType, Circuit};
use crate::exec::{SystemYCsc, int_array_to_string};
use crate::obj::base::DssObject;
use crate::obj::dss_enum::EnumRegistry;
use crate::report::format;
use crate::util::float_to_str;

/// Pascal `StrYorN` (`Utilities.pas:173`).
fn y_or_n(b: bool) -> &'static str {
    if b { "Yes" } else { "No" }
}

/// Pascal `DSSGlobals.DefaultEditor` platform default (`DSSGlobals.pas:
/// 749-769`): Windows is the constant `NotePad.exe` (no env lookup — the
/// pinned-oracle default the golden pins); macOS/Linux take `EDITOR` with the
/// `open -t`/`xdg-open` fallbacks.
fn default_editor() -> String {
    if cfg!(windows) {
        "NotePad.exe".to_string()
    } else {
        let fallback = if cfg!(target_os = "macos") {
            "open -t"
        } else {
            "xdg-open"
        };
        match std::env::var("EDITOR") {
            Ok(e) if !e.is_empty() => e,
            _ => fallback.to_string(),
        }
    }
}

/// Pascal `TSolutionObj.DumpProperties(F, Complete, Leaf)`, ported
/// line-for-line. `y_csc` is the assembled system Y (`Dss::system_y_csc`
/// coordinates) for the `Complete` tail; `None` skips the Y dump exactly like
/// the pre-`BuildY` state (Pascal would factor a NIL handle — never reached
/// on the gated path, which dumps only solved circuits).
pub(crate) fn dump_solution_properties(
    out: &mut String,
    ckt: &Circuit,
    enums: &EnumRegistry,
    complete: bool,
    leaf: bool,
    y_csc: Option<&SystemYCsc>,
) {
    let sol = &ckt.solution;
    out.push_str("! OPTIONS\n");
    out.push_str(&format!("! NumNodes = {}\n", ckt.num_nodes));
    if leaf {
        out.push_str(&format!(
            "Set Mode={}\n",
            enums
                .get(enums.solve_mode)
                .ordinal_to_string(sol.mode.ordinal())
        ));
    }
    out.push_str(&format!(
        "Set ControlMode={}\n",
        enums
            .get(enums.control_mode)
            .ordinal_to_string(sol.control_mode)
    ));
    out.push_str(&format!(
        "Set Random={}\n",
        enums
            .get(enums.random_mode)
            .ordinal_to_string(sol.random_type)
    ));
    if leaf {
        out.push_str(&format!("Set hour={}\n", sol.int_hour));
        out.push_str(&format!("Set sec={}\n", float_to_str(sol.t)));
        out.push_str(&format!("Set year={}\n", sol.year));
    }
    out.push_str(&format!("Set frequency={}\n", float_to_str(sol.frequency)));
    out.push_str(&format!("Set stepsize={}\n", float_to_str(sol.h)));
    out.push_str(&format!("Set number={}\n", sol.number_of_times));
    if leaf {
        out.push_str(&format!("Set circuit={}\n", ckt.name));
        out.push_str(&format!("Set editor={}\n", default_editor()));
    }
    out.push_str(&format!(
        "Set tolerance={}\n",
        float_to_str(sol.convergence_tolerance)
    ));
    out.push_str(&format!("Set maxiterations={}\n", sol.max_iterations));
    out.push_str(&format!("Set miniterations={}\n", sol.min_iterations));
    out.push_str(&format!(
        "Set loadmodel={}\n",
        enums
            .get(enums.default_load_model)
            .ordinal_to_string(sol.load_model)
    ));

    out.push_str(&format!(
        "Set loadmult={}\n",
        float_to_str(ckt.load_multiplier)
    ));
    out.push_str(&format!(
        "Set Normvminpu={}\n",
        float_to_str(ckt.normal_min_volts)
    ));
    out.push_str(&format!(
        "Set Normvmaxpu={}\n",
        float_to_str(ckt.normal_max_volts)
    ));
    out.push_str(&format!(
        "Set Emergvminpu={}\n",
        float_to_str(ckt.emerg_min_volts)
    ));
    out.push_str(&format!(
        "Set Emergvmaxpu={}\n",
        float_to_str(ckt.emerg_max_volts)
    ));
    // `%-.4g` of DefaultDailyShapeObj.Mean/StdDev × 100.
    let (mean, std_dev) = ckt
        .default_daily_shape_obj
        .as_ref()
        .map(|s| (s.mean(), s.std_dev()))
        .unwrap_or((0.0, 0.0));
    out.push_str(&format!("Set %mean={}\n", format::g(mean * 100.0, 4)));
    out.push_str(&format!("Set %stddev={}\n", format::g(std_dev * 100.0, 4)));
    // `NameIfNotNil(LoadDurCurveObj)` (`Solution.pas:1816`): the name of the
    // `Set LDCurve=` LoadShape (GAPS WPG.3), empty when unset — same render as
    // the `exec/report.rs` register-export header.
    let ldcurve = ckt
        .load_dur_curve_obj
        .as_ref()
        .map(|s| s.data().name().to_string())
        .unwrap_or_default();
    out.push_str(&format!("Set LDCurve={ldcurve}\n"));
    out.push_str(&format!(
        "Set %growth={}\n",
        format::g((ckt.default_growth_rate - 1.0) * 100.0, 4)
    ));

    out.push_str(&format!(
        "Set genkw={}\n",
        float_to_str(ckt.auto_add_obj.gen_kw)
    ));
    out.push_str(&format!(
        "Set genpf={}\n",
        float_to_str(ckt.auto_add_obj.gen_pf)
    ));
    out.push_str(&format!(
        "Set capkvar={}\n",
        float_to_str(ckt.auto_add_obj.cap_kvar)
    ));
    // Pascal writes `Set addtype=` then the branch word; the `case` has no
    // else (both branches covered: AddType is only ever GENADD/CAPADD).
    out.push_str("Set addtype=");
    out.push_str(if ckt.auto_add_obj.add_type == AddType::Cap {
        "capacitor\n"
    } else {
        "generator\n"
    });
    if leaf {
        out.push_str(&format!(
            "Set allowduplicates={}\n",
            y_or_n(ckt.duplicates_allowed)
        ));
    }
    out.push_str(&format!("Set zonelock={}\n", y_or_n(ckt.zones_locked)));
    out.push_str(&format!(
        "Set ueweight={}\n",
        format::fixed_w(ckt.ue_weight, 8, 2)
    ));
    out.push_str(&format!(
        "Set lossweight={}\n",
        format::fixed_w(ckt.loss_weight, 8, 2)
    ));
    out.push_str(&format!(
        "Set ueregs={}\n",
        int_array_to_string(&ckt.ue_regs)
    ));
    out.push_str(&format!(
        "Set lossregs={}\n",
        int_array_to_string(&ckt.loss_regs)
    ));
    if leaf {
        out.push_str("Set voltagebases=(");
        for &b in &ckt.legal_voltage_bases {
            out.push_str(&format::fixed_w(b, 10, 2));
        }
        out.push_str(")\n");
    }
    out.push_str(&format!(
        "Set algorithm={}\n",
        enums
            .get(enums.solve_alg)
            .ordinal_to_string(sol.algorithm.ordinal())
    ));
    out.push_str(&format!(
        "Set Trapezoidal={}\n",
        y_or_n(ckt.trapezoidal_integration)
    ));
    out.push_str(&format!(
        "Set genmult={}\n",
        float_to_str(ckt.gen_multiplier)
    ));

    out.push_str(&format!(
        "Set Basefrequency={}\n",
        float_to_str(ckt.fundamental)
    ));

    out.push_str("Set harmonics=(");
    if sol.do_all_harmonics {
        out.push_str("ALL");
    } else {
        for &harm in &sol.harmonic_list {
            out.push_str(&format!("{}, ", float_to_str(harm)));
        }
    }
    out.push_str(")\n");
    out.push_str(&format!(
        "Set maxcontroliter={}\n",
        sol.max_control_iterations
    ));
    out.push('\n');

    if complete && let Some((n, coords)) = y_csc {
        // Pascal factors `hY` and walks KLU's compressed columns
        // (`GetCompressedMatrix`): every stored entry, column-major with
        // ascending rows — duplicate stamps compressed, values unchanged
        // (the same walk `Show Y` pins, `report/show/matrix.rs`, but with NO
        // triangle filter: the full symmetric storage prints).
        let mut m: BTreeMap<(usize, usize), Complex64> = BTreeMap::new();
        for &(r, c, v) in coords {
            *m.entry((c, r)).or_insert(Complex64::ZERO) += v;
        }
        let _ = n;
        out.push_str("System Y Matrix (Lower Triangle by Columns)\n");
        out.push('\n');
        out.push_str("  Row  Col               G               B\n");
        out.push('\n');
        for ((c, r), v) in m {
            // `Format('[%4d,%4d] = %12.5g + j%12.5g', …)`, 1-based.
            out.push_str(&format!(
                "[{},{}] = {} + j{}\n",
                format::fixed_w_int(r as i64 + 1, 4),
                format::fixed_w_int(c as i64 + 1, 4),
                format::g_w(v.re, 12, 5),
                format::g_w(v.im, 12, 5),
            ));
        }
    }
}

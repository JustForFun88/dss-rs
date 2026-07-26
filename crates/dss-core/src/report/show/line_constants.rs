//! `Show LineConstants` (Pascal `ShowResults.pas` `ShowLineConstants`): for every
//! `LineGeometry`, the per-unit-length R / jX / susceptance / L / C matrices at a
//! given frequency / line-units / earth-resistivity, plus — for order-3 geometries —
//! the equivalent symmetrical-component summary (Z1/Z0, C1/C0, surge impedance,
//! propagation velocity). Produces **two** files: the human-readable report
//! (`<CircuitName_>LineConstants.txt`) and a `LineConstantsCode.dss` LineCode
//! script (same dir, no circuit prefix). The Carson recompute reuses the WP7.1
//! `LineGeometryObj::z_matrix`/`yc_matrix` (recomputed at the requested freq/rho).

use crate::elements::general::line_geometry::LineGeometryObj;
use crate::exec::registry::DssClass;
use crate::support::line_units::LineUnits;

/// Pascal global `TwoPi = 2.0 * PI` (full precision — the report's L/C post-scaling,
/// distinct from the Carson engine's internal truncated `TWOPI`).
const TWO_PI: f64 = std::f64::consts::TAU;

/// Pascal `Format('%.6g', [v])`.
fn g6(v: f64) -> String {
    crate::report::format::g(v, 6)
}

/// Build the two `Show LineConstants` files (Pascal `ShowLineConstants`): returns
/// `(main_report, linecodes_dss, errors)`. `earth_model` is `DSS.DefaultEarthModel`
/// (passed as `DSS.ActiveEarthModel` into the Carson recompute); `earth_name` is
/// `EarthModelEnum.OrdinalToString(earth_model)`. Iterates the `LineGeometry`
/// objects in creation order, mutating each (`RhoEarth :=`, recompute) exactly like
/// Pascal pokes the shared catalog object. `errors` carries Pascal's #9934
/// "Error computing line constants for …" message when a geometry's Carson recompute
/// fails (the caller extends its error log with it).
pub(crate) fn show_line_constants(
    classes: &mut [DssClass],
    freq: f64,
    units: i32,
    rho: f64,
    earth_model: i32,
    earth_name: &str,
) -> (String, String, Vec<crate::diag::DssDiagnostic>) {
    let units_str = LineUnits::from_code(units).as_str();
    let mut f = String::new(); // LineConstants.txt
    let mut f2 = String::new(); // LineConstantsCode.dss
    let mut errors: Vec<crate::diag::DssDiagnostic> = Vec::new();

    f.push_str("LINE CONSTANTS\n");
    f.push_str(&format!(
        "Frequency = {} Hz, Earth resistivity = {} ohm-m\n",
        g6(freq),
        g6(rho)
    ));
    f.push_str(&format!("Earth Model = {earth_name}\n"));
    f.push('\n');

    f2.push_str("!--- OpenDSS Linecodes file generated from Show LINECONSTANTS command\n");
    f2.push_str(&format!(
        "!--- Frequency = {} Hz, Earth resistivity = {} ohm-m\n",
        g6(freq),
        g6(rho)
    ));
    f2.push_str(&format!("!--- Earth Model = {earth_name}\n"));

    let Some(ci) = classes
        .iter()
        .position(|c| c.props.class_name().eq_ignore_ascii_case("LineGeometry"))
    else {
        return (f, f2, errors); // no geometries → header-only files
    };

    for oi in 0..classes[ci].arena.len() {
        let name = classes[ci].arena[oi].data().name().to_string();
        let Some(geom) = classes[ci].arena.get_mut::<LineGeometryObj>(oi) else {
            continue;
        };
        // Pascal `pelem.RhoEarth := Rho; Z := pelem.Zmatrix[freq,1.0,Units]; YC := …`.
        geom.set_rho_earth(rho);
        let (z, yc) = match (
            geom.z_matrix(freq, 1.0, units, earth_model),
            geom.yc_matrix(freq, 1.0, units, earth_model),
        ) {
            (Ok(z), Ok(yc)) => (z, yc),
            // Pascal `ShowResults.pas:3295-3298`: on a compute exception, log #9934
            // and continue (Pascal then faults on the NIL `Z` — a crash we do NOT
            // reproduce; the safe skip keeps the diagnostic without the AV). This is
            // unreachable for a validly-parsed geometry (those errors fire at parse).
            (z, y) => {
                let msg = z.err().or_else(|| y.err()).unwrap_or_default();
                errors.push(crate::diag::DssDiagnostic::msg(
                    format!(
                        "Error computing line constants for LineGeometry.{name}; Error message: {msg}"
                    ),
                    Some(9934),
                ));
                continue;
            }
        };
        write_geometry(&mut f, &mut f2, &name, &z, &yc, freq, units, units_str);
    }

    (f, f2, errors)
}

/// One geometry's block in both files.
#[allow(clippy::too_many_arguments)]
fn write_geometry(
    f: &mut String,
    f2: &mut String,
    name: &str,
    z: &crate::support::cmatrix::CMatrix,
    yc: &crate::support::cmatrix::CMatrix,
    freq: f64,
    units: i32,
    units_str: &str,
) {
    let order = z.order();

    f.push('\n');
    f.push_str("--------------------------------------------------\n");
    f.push_str(&format!("Geometry Code = {name}\n"));
    f.push('\n');

    // R / jX matrices, lower triangle (`%.6g, ` per cell).
    f.push_str(&format!("R MATRIX, ohms per {units_str}\n"));
    write_lower(f, order, |i, j| g6(z.get(i, j).re));
    f.push('\n');
    f.push_str(&format!("jX MATRIX, ohms per {units_str}\n"));
    write_lower(f, order, |i, j| g6(z.get(i, j).im));
    f.push('\n');
    f.push_str(&format!("Susceptance (jB) MATRIX, S per {units_str}\n"));
    write_lower(f, order, |i, j| g6(yc.get(i, j).im));

    // L matrix (mH): Z.im / (freq·2π/1e3).
    let wl = freq * TWO_PI / 1.0e3;
    f.push('\n');
    f.push_str(&format!("L MATRIX, mH per {units_str}\n"));
    write_lower(f, order, |i, j| g6(z.get(i, j).im / wl));

    // C matrix (nF): YC.im / (freq·2π/1e9).
    let wc = freq * TWO_PI / 1.0e9;
    f.push('\n');
    f.push_str(&format!("C MATRIX, nF per {units_str}\n"));
    write_lower(f, order, |i, j| g6(yc.get(i, j).im / wc));

    // The LineCode script (F2): New Linecode + R/X/C matrices (`|`-separated rows).
    f2.push('\n');
    f2.push_str(&format!(
        "New Linecode.{name} nphases={order}  Units={units_str}\n"
    ));
    f2.push_str("~ Rmatrix=[");
    write_matrix_row(f2, order, |i, j| g6(z.get(i, j).re));
    f2.push_str("]\n");
    f2.push_str("~ Xmatrix=[");
    write_matrix_row(f2, order, |i, j| g6(z.get(i, j).im));
    f2.push_str("]\n");
    f2.push_str("~ Cmatrix=[");
    write_matrix_row(f2, order, |i, j| g6(yc.get(i, j).im / wc));
    f2.push_str("]\n");

    // Equivalent symmetrical-component summary (order-3 only).
    if order == 3 {
        write_seq_summary(f, z, yc, freq, units, units_str);
    }
}

/// A lower-triangle dump into `f`: for i in 0..order, j in 0..=i, `<cell>, ` then a
/// newline (Pascal `FSWrite(Format('%.6g, ',…))` + `FSWriteln`).
fn write_lower(f: &mut String, order: usize, mut cell: impl FnMut(usize, usize) -> String) {
    for i in 0..order {
        for j in 0..=i {
            f.push_str(&cell(i, j));
            f.push_str(", ");
        }
        f.push('\n');
    }
}

/// A `~ Rmatrix=[…]`-style body: lower triangle with `<cell>  ` (two trailing
/// spaces) per element and a `|` between rows (Pascal `if i < Z.order then '|'`).
fn write_matrix_row(f: &mut String, order: usize, mut cell: impl FnMut(usize, usize) -> String) {
    for i in 0..order {
        for j in 0..=i {
            f.push_str(&cell(i, j));
            f.push_str("  ");
        }
        if i < order - 1 {
            f.push('|');
        }
    }
}

/// The order-3 equivalent symmetrical-component block (Pascal `ShowLineConstants`
/// `if Z.order = 3`): Z1/Z0 (+ L1/L0), C1/C0, surge impedance, propagation velocity.
/// **Inverts a copy of `z`** for the common-mode series impedance (Pascal does
/// `Z.Invert` after Z1/Z0, which are computed from the pre-invert matrix).
fn write_seq_summary(
    f: &mut String,
    z: &crate::support::cmatrix::CMatrix,
    yc: &crate::support::cmatrix::CMatrix,
    freq: f64,
    units: i32,
    units_str: &str,
) {
    use num_complex::Complex64;

    f.push('\n');
    f.push_str("-------------------------------------------------------------------\n");
    f.push_str("-------------------Equiv Symmetrical Component --------------------\n");
    f.push_str("-------------------------------------------------------------------\n");
    f.push('\n');

    // Zs = Σ diag; Zm = Σ strict-lower off-diag (Pascal j := 1 to i-1).
    let mut zs = Complex64::ZERO;
    let mut zm = Complex64::ZERO;
    for i in 0..3 {
        zs += z.get(i, i);
    }
    for i in 0..3 {
        for j in 0..i {
            zm += z.get(i, j);
        }
    }
    let z1 = (zs - zm) / 3.0;
    let z0 = (zm * 2.0 + zs) / 3.0;
    let w = freq * TWO_PI / 1000.0;
    f.push('\n');
    f.push_str(&format!(
        "Z1, ohms per {units_str} = {} + j {} (L1 = {} mH) \n",
        g6(z1.re),
        g6(z1.im),
        g6(z1.im / w)
    ));
    f.push_str(&format!(
        "Z0, ohms per {units_str} = {} + j {} (L0 = {} mH) \n",
        g6(z0.re),
        g6(z0.im),
        g6(z0.im / w)
    ));
    f.push('\n');

    // Common-mode series impedance: invert a copy of Z, sum all 3×3, XCM = Cinv(Σ).im.
    let mut zinv = z.clone();
    let inv_ok = zinv.invert().is_ok();
    let mut ycm = Complex64::ZERO;
    if inv_ok {
        for i in 0..3 {
            for j in 0..3 {
                ycm += zinv.get(i, j);
            }
        }
    }
    let xcm = ycm.inv().im;

    // Capacitance sequence: CS = Σ diag Im(YC); CM = Σ strict-lower Im(YC).
    let wc = freq * TWO_PI / 1.0e9;
    let mut cs = 0.0;
    let mut cm = 0.0;
    for i in 0..3 {
        cs += yc.get(i, i).im;
    }
    for i in 0..3 {
        for j in 0..i {
            cm += yc.get(i, j).im;
        }
    }
    let c1 = (cs - cm) / 3.0 / wc; // nF
    let c0 = (cs + 2.0 * cm) / 3.0 / wc;

    // Common-mode shunt capacitance: Σ all YC, CCM = Im / wc.
    let mut ycm2 = Complex64::ZERO;
    for i in 0..3 {
        for j in 0..3 {
            ycm2 += yc.get(i, j);
        }
    }
    let ccm = ycm2.im / wc;

    f.push_str(&format!("C1, nF per {units_str} = {}\n", g6(c1)));
    f.push_str(&format!("C0, nF per {units_str} = {}\n", g6(c0)));
    f.push('\n');

    let w = freq * TWO_PI;
    f.push_str("Surge Impedance:\n");
    f.push_str(&format!(
        "  Positive sequence = {} ohms\n",
        g6((z1.im / w / (c1 * 1.0e-9)).sqrt())
    ));
    f.push_str(&format!(
        "  Zero sequence     = {} ohms\n",
        g6((z0.im / w / (c0 * 1.0e-9)).sqrt())
    ));
    f.push_str(&format!(
        "  Common Mode       = {} ohms\n",
        g6((xcm / w / (ccm * 1.0e-9)).sqrt())
    ));
    f.push('\n');

    let to_per_m = LineUnits::from_code(units).to_per_meter();
    f.push_str("Propagation Velocity (Percent of speed of light):\n");
    f.push_str(&format!(
        "  Positive sequence = {} \n",
        g6(1.0 / (z1.im / w * (c1 * 1.0e-9)).sqrt() / 299792458.0 / to_per_m * 100.0)
    ));
    f.push_str(&format!(
        "  Zero sequence     = {} \n",
        g6(1.0 / (z0.im / w * (c0 * 1.0e-9)).sqrt() / 299792458.0 / to_per_m * 100.0)
    ));
    f.push('\n');
}

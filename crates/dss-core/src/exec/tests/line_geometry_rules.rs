//! How a LineGeometry reads its property stream (user decision 2026-10-04):
//!
//! 1. `cond=` may come in any order.
//! 2. Conductor data (`wire=`, `cncable=`, `tscable=`, `x=`, `h=`) without a
//!    selected conductor is an error. Only an explicit `cond=N` selects one.
//!    After `New` and after `like=` none is selected, a one-conductor geometry
//!    included.
//! 3. `units=` before the first `cond=` sets the default only.
//! 4. `units=` after `cond=N` belongs to conductor N wherever it stands before
//!    the next `cond=`, and becomes the default for conductors selected later
//!    without their own.
//! 5. Line breaks and `~` mean nothing.
//! 6. `cond=` out of range is an error and leaves no conductor selected.
//! 7. A conductor without data is one error at calculation, naming it.
//! 8. A unit is never "not set": without a default it is feet.
//!
//! The rules do not name a `cond=` whose value is not a number. It is read like
//! one out of range (rule 6): an error that leaves no conductor selected.
//!
//! Every script and variant is a tracked deck under
//! `crates/dss-core/tests/data/line_geometry_rules/` (`library.dss` holds the
//! conductor data). Some rule pins and the readback, JSON and Y-build pins
//! build a small geometry inline. A pin compiles its deck, reads the
//! geometry's conductors from the class arena and the Line's impedance, and
//! compares the impedance with physics evaluated at the positions the rules
//! give: the simple Carson reactance of bare wires ([`carson_x`]), the
//! textbook cable model ([`cable_z`]) and the shunt capacitance of one
//! conductor ([`shunt_c11`]). These formulas take two models of the engine,
//! the earth-return depth of its simple Carson model ([`carson_x`]) and its
//! tape-shield resistance with the TSData lap correction ([`cable_z`]), so
//! the pins hold the unit placement under those models. A deck that cannot be
//! solved is pinned on its messages. Anything a pin writes (a Save, a dump, a
//! line-constants report) goes to its own scratch directory, never next to the
//! decks.

use std::path::{Path, PathBuf};

use super::common::query;
use crate::elements::general::line_geometry::LineGeometryObj;
use crate::elements::pd::line::{Line, prop as line_prop};
use crate::exec::*;
use crate::obj::base::DssObject;
use crate::support::line_units::LineUnits;

// ----- decks, scratch directories, readers ---------------------------------

/// The tracked deck `name` of this folder.
fn deck(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/line_geometry_rules")
        .join(name)
}

/// `p` with forward slashes, as a DSS command quotes it.
fn slash(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// A per-test scratch directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let d = std::env::temp_dir().join(format!("dss_lgrules_{tag}_{}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        std::fs::create_dir_all(&d).unwrap_or_else(|e| panic!("mkdir {}: {e}", d.display()));
        Scratch(d)
    }

    /// A subdirectory, created.
    fn dir(&self, sub: &str) -> PathBuf {
        let d = self.0.join(sub);
        std::fs::create_dir_all(&d).unwrap_or_else(|e| panic!("mkdir {}: {e}", d.display()));
        d
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

/// The file `name` of `dir`.
fn read(dir: &Path, name: &str) -> String {
    let file = dir.join(name);
    std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("read {}: {e}", file.display()))
}

/// A fresh engine with deck `name` compiled.
fn compile(name: &str) -> Dss {
    let mut dss = Dss::new();
    dss.command(&format!("compile \"{}\"", slash(&deck(name))));
    dss
}

/// The error log's texts, in order.
fn errors(dss: &Dss) -> Vec<String> {
    dss.errors().iter().map(|e| e.message.clone()).collect()
}

fn solved(dss: &Dss) -> bool {
    dss.circuit().is_some_and(|c| c.is_solved)
}

/// One conductor of a geometry as the arena holds it.
#[derive(Debug, Clone, PartialEq)]
struct Conductor {
    object: Option<String>,
    x: f64,
    h: f64,
    unit: &'static str,
}

fn conductors(dss: &Dss, geom: &str) -> Vec<Conductor> {
    let ci = dss.class_by_name["linegeometry"];
    let oi = dss.classes[ci].name_to_idx[geom.to_ascii_lowercase().as_str()];
    let g = dss.classes[ci]
        .arena
        .get::<LineGeometryObj>(oi)
        .expect("a LineGeometry");
    (0..g.nwires().max(0) as usize)
        .map(|i| Conductor {
            object: g.conductor(i + 1).map(|c| c.name().to_string()),
            x: g.fx()[i],
            h: g.fy()[i],
            unit: LineUnits::from_code(g.conductor_unit(i)).as_str(),
        })
        .collect()
}

/// Each conductor's unit, in conductor order.
fn units(dss: &Dss, geom: &str) -> Vec<&'static str> {
    conductors(dss, geom).iter().map(|c| c.unit).collect()
}

/// One part of a Line's impedance per unit length (`xmatrix` when `real` is
/// false, `rmatrix` when true), `[row][col]`, in Ω/mi for the 1-mile Lines here.
fn zpart(dss: &Dss, line: &str, real: bool) -> Vec<Vec<f64>> {
    let ci = dss.class_by_name["line"];
    let oi = dss.classes[ci].name_to_idx[line.to_ascii_lowercase().as_str()];
    let l = dss.classes[ci].arena.get::<Line>(oi).expect("a Line");
    let (vals, n) = l
        .get_matrix_part(line_prop::XMATRIX, real)
        .unwrap_or_else(|| panic!("Line.{line} has no impedance"));
    let len = l.prop_scale(line_prop::XMATRIX, true);
    (0..n)
        .map(|i| (0..n).map(|j| vals[j * n + i] / len).collect())
        .collect()
}

fn xmatrix(dss: &Dss, line: &str) -> Vec<Vec<f64>> {
    zpart(dss, line, false)
}

/// `? LineGeometry.<geom>.Cond`.
fn cond(dss: &mut Dss, geom: &str) -> String {
    query(dss, &format!("LineGeometry.{geom}.Cond"))
}

/// Replay deck `name` line by line and read `Cond` after every line of
/// `geom`'s definition: its `New` line and the `~` lines right after it.
fn cond_trace(name: &str, geom: &str) -> Vec<String> {
    let text = std::fs::read_to_string(deck(name)).expect("the deck");
    let mut dss = Dss::new();
    let new_line = format!("new linegeometry.{geom} ");
    let mut inside = false;
    let mut trace = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('!') {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("redirect ") {
            dss.command(&format!("redirect \"{}\"", slash(&deck("library.dss"))));
            continue;
        }
        dss.command(line);
        if lower.starts_with(&new_line) || lower == new_line.trim_end() {
            inside = true;
        } else if !lower.starts_with('~') {
            inside = false;
        }
        if inside {
            trace.push(cond(&mut dss, geom));
        }
    }
    trace
}

/// Replay deck `name` line by line up to, not including, its first line that
/// starts with `stop`.
fn replay_until(name: &str, stop: &str) -> Dss {
    let text = std::fs::read_to_string(deck(name)).expect("the deck");
    let mut dss = Dss::new();
    let mut reached = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with(stop) {
            reached = true;
            break;
        }
        if line.is_empty() || line.starts_with('!') {
            continue;
        }
        if line.to_ascii_lowercase().starts_with("redirect ") {
            dss.command(&format!("redirect \"{}\"", slash(&deck("library.dss"))));
            continue;
        }
        dss.command(line);
    }
    assert!(reached, "{name} has no line starting with {stop}");
    dss
}

// ----- the expected messages ------------------------------------------------

/// Rule 2: conductor data with no conductor selected.
fn no_cond(geom: &str, prop: &str) -> String {
    format!(
        "LineGeometry.{geom}.{prop}: conductor data without cond=. No conductor is selected, so \
         the value is not applied. Select one with cond=N first."
    )
}

/// Rule 6: `cond=value` out of range. `has` is "has 4 conductors" or the
/// no-conductors wording.
fn out_of_range(geom: &str, value: i32, has: &str) -> String {
    format!(
        "LineGeometry.{geom}.Cond: cond={value} is out of range, the geometry {has}. No \
         conductor is selected."
    )
}

/// Rule 7 for one conductor, as the Y build reports it from `CalcVoltageBases`.
fn missing_at_calc(geom: &str, conductor: usize) -> String {
    format!(
        "Error Encountered in CalcVoltageBases: LineGeometry.{geom}: conductor {conductor} has \
         no wire, cncable or tscable."
    )
}

// ----- physics ---------------------------------------------------------------

/// GMR of the two wires of `library.dss` in metres: ACSR_556_5 0.37320 in,
/// ACSR_4/0 0.09768 in.
const GMR_P: f64 = 0.37320 * 0.0254;
const GMR_N: f64 = 0.09768 * 0.0254;

/// Metres per unit, for the units the decks use.
fn metres(unit: &str) -> f64 {
    match unit {
        "mm" => 0.001,
        "in" => 0.0254,
        "ft" => 0.3048,
        "m" => 1.0,
        _ => panic!("no length unit {unit}"),
    }
}

/// An overhead conductor where the rules put it: its wire's GMR (m) and its
/// coordinates in `unit`.
#[derive(Clone, Copy)]
struct Placed {
    gmr: f64,
    x: f64,
    h: f64,
    unit: &'static str,
}

/// A phase wire (ACSR_556_5).
fn phase(x: f64, h: f64, unit: &'static str) -> Placed {
    Placed {
        gmr: GMR_P,
        x,
        h,
        unit,
    }
}

/// The neutral wire (ACSR_4/0).
fn neutral(x: f64, h: f64, unit: &'static str) -> Placed {
    Placed {
        gmr: GMR_N,
        x,
        h,
        unit,
    }
}

/// Script C's four conductors in feet: phases at x = -4, 0, 4 ft, h = 28 ft,
/// the neutral at (0, 24) ft.
fn c0_in(units: [&'static str; 4]) -> Vec<Placed> {
    vec![
        phase(-4.0, 28.0, units[0]),
        phase(0.0, 28.0, units[1]),
        phase(4.0, 28.0, units[2]),
        neutral(0.0, 24.0, units[3]),
    ]
}

/// The simple Carson series reactance at 60 Hz and ρ = 100 Ω·m, in Ω per mile:
/// `X_ij = (ω μ0 / 2π) ln(De / D_ij)`, `X_ii = (ω μ0 / 2π) ln(De / GMR_i)`, with
/// `μ0 = 4π·10⁻⁷` H/m and the engine's earth-return depth
/// `De = 658.8530451057239 √(ρ / f)` m. The first terms of Carson's series give
/// `De = 2 e^(1/2 − γ) / √(ω μ0 / ρ) = 658.8716 √(ρ / f)` m, 2.8e-5 relative
/// more, which raises every entry by 3.4e-6 Ω/mi. A misread unit moves an
/// off-diagonal entry by 0.04 Ω/mi or more, so the pins hold the units with
/// either constant. No other engine number.
fn carson_x(conds: &[Placed]) -> Vec<Vec<f64>> {
    let distance = |a: &Placed, b: &Placed| {
        let (ma, mb) = (metres(a.unit), metres(b.unit));
        (a.x * ma - b.x * mb).hypot(a.h * ma - b.h * mb)
    };
    carson_x_of(conds.len(), |i, j| {
        if i == j {
            conds[i].gmr
        } else {
            distance(&conds[i], &conds[j])
        }
    })
}

/// [`carson_x`] of an equivalent spacing: the first `nphases` of `gmr` are
/// phases, every phase pair and every neutral pair `d_pp` metres apart, every
/// phase-neutral pair `d_pn` metres. The heights do not enter the simple
/// Carson reactance.
fn carson_x_equivalent(gmr: &[f64], nphases: usize, d_pp: f64, d_pn: f64) -> Vec<Vec<f64>> {
    carson_x_of(gmr.len(), |i, j| {
        if i == j {
            gmr[i]
        } else if (i < nphases) == (j < nphases) {
            d_pp
        } else {
            d_pn
        }
    })
}

/// The simple Carson reactance of `n` conductors, Ω/mi, where `d(i, j)` is
/// the distance of conductors `i` and `j` in metres and `d(i, i)` the GMR of
/// conductor `i`.
fn carson_x_of(n: usize, d: impl Fn(usize, usize) -> f64) -> Vec<Vec<f64>> {
    let f = 60.0_f64;
    let rho = 100.0_f64;
    let mu0 = 4.0e-7 * std::f64::consts::PI;
    let k = f * mu0; // ω μ0 / 2π
    let de = 658.853_045_105_723_9 * (rho / f).sqrt();
    let mile = 1609.344;
    (0..n)
        .map(|i| (0..n).map(|j| k * (de / d(i, j)).ln() * mile).collect())
        .collect()
}

/// The band between the engine's reactance and [`carson_x`], Ω/mi. The
/// engine's `μ0 = 12.56637e-7` sits 4.9e-8 relative below `4π·10⁻⁷`, at most
/// 8e-8 Ω/mi on these matrices. [`carson_x`] takes the engine's earth-return
/// depth, so the band leaves out the 3.4e-6 Ω/mi of Carson's series value. A
/// misread unit moves an off-diagonal entry by 0.04 Ω/mi or more.
const X_BAND: f64 = 1e-6;

/// Line `line`'s reactance equals [`carson_x`] of `placed` entry by entry, and
/// each `(i, j, value)` of `spot` (1-based) as well.
fn assert_x(dss: &Dss, line: &str, placed: &[Placed], spot: &[(usize, usize, f64)], ctx: &str) {
    assert_x_is(dss, line, &carson_x(placed), spot, ctx);
}

/// Line `line`'s reactance equals `want` entry by entry within [`X_BAND`], and
/// each `(i, j, value)` of `spot` (1-based) as well.
fn assert_x_is(dss: &Dss, line: &str, want: &[Vec<f64>], spot: &[(usize, usize, f64)], ctx: &str) {
    let x = xmatrix(dss, line);
    assert_eq!(x.len(), want.len(), "{ctx}: Line.{line} order");
    for i in 0..x.len() {
        for j in 0..x.len() {
            assert!(
                (x[i][j] - want[i][j]).abs() <= X_BAND,
                "{ctx}: Line.{line} X{}{} = {} Ω/mi, simple Carson at the ruled positions gives {}",
                i + 1,
                j + 1,
                x[i][j],
                want[i][j]
            );
        }
    }
    for &(i, j, v) in spot {
        assert!(
            (x[i - 1][j - 1] - v).abs() <= X_BAND,
            "{ctx}: Line.{line} X{i}{j} = {}, expected {v}",
            x[i - 1][j - 1]
        );
    }
}

/// What a position of a cable geometry holds (the data of `library.dss`).
#[derive(Clone, Copy, PartialEq)]
enum Holds {
    /// CNData `250_1/3`: a core inside 13 concentric neutral strands.
    Cn,
    /// TSData `1/0TS`: a core inside a copper tape shield.
    Ts,
    /// The bare neutral ACSR_4/0.
    Bare,
}

/// A position of a cable geometry: what it holds and its coordinates in `unit`.
#[derive(Clone, Copy)]
struct Laid {
    holds: Holds,
    x: f64,
    h: f64,
    unit: &'static str,
}

/// A position holding `holds` at (`x`, `h`) in `unit`.
fn laid(holds: Holds, x: f64, h: f64, unit: &'static str) -> Laid {
    Laid { holds, x, h, unit }
}

/// The series impedance of a cable geometry at 60 Hz and ρ = 100 Ω·m, `(R, X)`
/// in Ω per mile, by the textbook model (Kersting, *Distribution System
/// Modeling and Analysis*, ch. 4) with the simple Carson earth return of
/// [`carson_x`]. Each cable adds its neutral as a conductor of its own:
/// - concentric neutral: `k = 13` strands of diameter `d_s = 0.064` in and
///   2.816666667 Ω/kft on a circle of radius `R = (d_od - d_s) / 2`
///   (`d_od = 1.16` in), resistance `r_s / k`, GMR `(GMR_s k R^(k-1))^(1/k)`
///   with `GMR_s = 0.7788 d_s / 2`, at `R` from its own core and at
///   `(D^k - R^k)^(1/k)` from a conductor `D` away.
/// - tape shield: diameter `d_s = 0.85` in, thickness `T = 0.005` in, lap
///   20 %, resistance `ρ √((100 - lap) / 50) / (π d_s T)` (the textbook
///   `ρ / (π d_s T)` with the lap correction of TSData) with copper's
///   `ρ = 2.3718e-8` Ω·m, GMR `(d_s - T) / 2`, at its GMR from its own core and
///   at `D` from a conductor `D` away.
///
/// Entry `ij` is `R_i δij + ω μ0 / 8 + j (ω μ0 / 2π) ln(De / D_ij)`, with the
/// conductor's GMR for `D_ii`. A core or wire takes its `Rac`, `1.02 Rdc` where
/// only `Rdc` is given. The neutrals are Kron reduced, and with `reduce` the
/// bare neutrals too. Two engine models enter, the earth-return depth of
/// [`carson_x`] and the tape resistance above, whose lap correction the
/// textbook's `ρ / (π d_s T)` does not have, so the pins hold the unit
/// placement under the engine's earth and tape models.
fn cable_z(conds: &[Laid], reduce: bool) -> (Vec<Vec<f64>>, Vec<Vec<f64>>) {
    use num_complex::Complex64;
    use std::f64::consts::PI;
    let (f, rho) = (60.0_f64, 100.0_f64);
    let mu0 = 4.0e-7 * PI;
    let de = 658.853_045_105_723_9 * (rho / f).sqrt();
    let (inch, kft, mile) = (0.0254, 304.8, 1609.344);
    let z = |r: f64, d: f64| Complex64::new(r + 2.0 * PI * f * mu0 / 8.0, f * mu0 * (de / d).ln());
    let (k, d_s) = (13.0_f64, 0.064 * inch);
    let r_cn = (1.16 * inch - d_s) / 2.0;
    let (d_ts, t_ts) = (0.85 * inch, 0.005 * inch);
    let gmr_ts = (d_ts - t_ts) / 2.0;
    // A cable's neutral: resistance, GMR, distance from its own core.
    let neutral = |holds: Holds| match holds {
        Holds::Cn => (
            2.816_666_667 / kft / k,
            (0.7788 * d_s / 2.0 * k * r_cn.powf(k - 1.0)).powf(1.0 / k),
            r_cn,
        ),
        _ => (
            2.3718e-8 * ((100.0 - 20.0) / 50.0_f64).sqrt() / (PI * d_ts * t_ts),
            gmr_ts,
            gmr_ts,
        ),
    };
    let n = conds.len();
    let at: Vec<(f64, f64)> = conds
        .iter()
        .map(|c| (c.x * metres(c.unit), c.h * metres(c.unit)))
        .collect();
    let dist = |i: usize, j: usize| (at[i].0 - at[j].0).hypot(at[i].1 - at[j].1);
    let cables: Vec<usize> = (0..n).filter(|&i| conds[i].holds != Holds::Bare).collect();
    let size = n + cables.len();
    let mut m = vec![vec![Complex64::new(0.0, 0.0); size]; size];
    let set = |m: &mut [Vec<Complex64>], i: usize, j: usize, v: Complex64| {
        m[i][j] = v;
        m[j][i] = v;
    };
    for (i, c) in conds.iter().enumerate() {
        let (r, gmr) = match c.holds {
            Holds::Cn => (0.076_705 / kft, 0.205_68 * inch),
            Holds::Ts => (1.02 * 0.184_204_546 / kft, 0.133_20 * inch),
            Holds::Bare => (1.02 * 0.112_121_212 / kft, GMR_N),
        };
        set(&mut m, i, i, z(r, gmr));
        for j in 0..i {
            set(&mut m, i, j, z(0.0, dist(i, j)));
        }
    }
    for (a, &i) in cables.iter().enumerate() {
        let (r, gmr, own) = neutral(conds[i].holds);
        set(&mut m, n + a, n + a, z(r, gmr));
        for (b, &j) in cables[..a].iter().enumerate() {
            set(&mut m, n + a, n + b, z(0.0, dist(i, j)));
        }
        for j in 0..n {
            let d = match (j == i, conds[i].holds) {
                (true, _) => own,
                (false, Holds::Cn) => (dist(i, j).powf(k) - r_cn.powf(k)).powf(1.0 / k),
                (false, _) => dist(i, j),
            };
            set(&mut m, n + a, j, z(0.0, d));
        }
    }
    let mut keep: Vec<usize> = (0..n).collect();
    if reduce {
        keep.retain(|&i| conds[i].holds != Holds::Bare);
    }
    for p in (0..size).filter(|p| !keep.contains(p)) {
        for i in (0..size).filter(|&i| i != p) {
            for j in (0..size).filter(|&j| j != p) {
                let v = m[i][p] * m[p][j] / m[p][p];
                m[i][j] -= v;
            }
        }
    }
    let part = |re: bool| -> Vec<Vec<f64>> {
        keep.iter()
            .map(|&i| {
                keep.iter()
                    .map(|&j| if re { m[i][j].re } else { m[i][j].im } * mile)
                    .collect()
            })
            .collect()
    };
    (part(true), part(false))
}

/// The band between the engine's tape-shield impedance and [`cable_z`], Ω/mi.
/// The engine takes 0.3183 for 1/π in the tape's resistance, 3.1e-5 relative
/// low, which moves an entry of these matrices by at most 1.5e-5 Ω/mi. A
/// misread unit moves one by 0.04 Ω/mi or more.
const TS_BAND: f64 = 2e-5;

/// Line `line`'s resistance and reactance equal [`cable_z`] of `laid` entry by
/// entry within `band`, and each `(i, j, r, x)` of `spot` (1-based) as well.
fn assert_cable(
    dss: &Dss,
    line: &str,
    (laid, reduce): (&[Laid], bool),
    band: f64,
    spot: &[(usize, usize, f64, f64)],
    ctx: &str,
) {
    let (r, x) = (zpart(dss, line, true), zpart(dss, line, false));
    let (want_r, want_x) = cable_z(laid, reduce);
    assert_eq!(r.len(), want_r.len(), "{ctx}: Line.{line} order");
    for i in 0..r.len() {
        for j in 0..r.len() {
            assert!(
                (r[i][j] - want_r[i][j]).abs() <= band && (x[i][j] - want_x[i][j]).abs() <= band,
                "{ctx}: Line.{line} Z{}{} = {} + j{} Ω/mi, the cable model at the ruled positions \
                 gives {} + j{}",
                i + 1,
                j + 1,
                r[i][j],
                x[i][j],
                want_r[i][j],
                want_x[i][j]
            );
        }
    }
    for &(i, j, re, im) in spot {
        let (gr, gx) = (r[i - 1][j - 1], x[i - 1][j - 1]);
        assert!(
            (gr - re).abs() <= band && (gx - im).abs() <= band,
            "{ctx}: Line.{line} Z{i}{j} = {gr} + j{gx}, expected {re} + j{im}"
        );
    }
}

/// The radius of ACSR_556_5 in metres (DIAM 0.927 in).
const RADIUS_P: f64 = 0.927 / 2.0 * 0.0254;

/// The shunt capacitance of one overhead conductor of radius `r` at height `h`
/// (both in metres) over the earth's surface, in nF per mile:
/// `C11 = 2π ε0 / ln(2h / r)`, with `ε0 = 8.8541878128e-12` F/m. No engine code.
fn shunt_c11(h: f64, r: f64) -> f64 {
    let eps0 = 8.854_187_812_8e-12;
    2.0 * std::f64::consts::PI * eps0 / (2.0 * h / r).ln() * 1609.344 * 1e9
}

/// The relative band between the engine's shunt capacitance and [`shunt_c11`].
/// The engine's `ε0 = 8.854e-12` F/m sits 2.1e-5 relative below the CODATA
/// value. A height of 28 read in metres instead of feet moves C11 by 16 %.
const C_BAND: f64 = 3e-5;

/// Line `line`'s shunt capacitance, `[row][col]`, in nF/mi for the 1-mile Lines
/// here.
fn cmatrix(dss: &Dss, line: &str) -> Vec<Vec<f64>> {
    let ci = dss.class_by_name["line"];
    let oi = dss.classes[ci].name_to_idx[line.to_ascii_lowercase().as_str()];
    let l = dss.classes[ci].arena.get::<Line>(oi).expect("a Line");
    let (vals, n) = l
        .get_matrix_part(line_prop::CMATRIX, false)
        .unwrap_or_else(|| panic!("Line.{line} has no capacitance"));
    let scale = l.prop_scale(line_prop::CMATRIX, true);
    (0..n)
        .map(|i| (0..n).map(|j| vals[j * n + i] / scale).collect())
        .collect()
}

/// One-conductor Line `line`'s C11 equals [`shunt_c11`] at height `h` m.
fn assert_c11(dss: &Dss, line: &str, h: f64, ctx: &str) {
    let c = cmatrix(dss, line);
    let want = shunt_c11(h, RADIUS_P);
    assert_eq!(c.len(), 1, "{ctx}: Line.{line} order");
    assert!(
        (c[0][0] - want).abs() <= C_BAND * want,
        "{ctx}: Line.{line} C11 = {} nF/mi, the conductor at h = {h} m gives {want}",
        c[0][0]
    );
}

/// Deck `name` compiles with no error, solves, gives geometry `g` the units
/// `want`, and Line `L` the reactance of `placed`.
fn assert_solvable(
    name: &str,
    want: &[&str],
    placed: &[Placed],
    spot: &[(usize, usize, f64)],
) -> Dss {
    let dss = compile(name);
    assert!(dss.errors().is_empty(), "{name}: {:?}", errors(&dss));
    assert!(solved(&dss), "{name} did not solve");
    assert_eq!(units(&dss, "g"), want, "{name}: units per conductor");
    assert_x(&dss, "L", placed, spot, name);
    dss
}

// ----- Save -------------------------------------------------------------------

/// `Save circuit` into `dir`, then the line the Save writes for geometry
/// `geom`, lowercased.
fn save(dss: &mut Dss, dir: &Path, geom: &str) -> String {
    let before = dss.errors().len();
    dss.command(&format!("save circuit dir=\"{}\"", slash(dir)));
    assert_eq!(
        dss.errors().len(),
        before,
        "save: {:?}",
        &errors(dss)[before..]
    );
    let text = read(dir, "LineGeometry.dss");
    assert!(!text.to_ascii_lowercase().contains("units=none"), "{text}");
    assert!(!text.to_ascii_lowercase().contains(" cond=0"), "{text}");
    let head = format!("new \"linegeometry.{geom}\"");
    text.lines()
        .map(str::to_ascii_lowercase)
        .find(|l| l.starts_with(&head))
        .unwrap_or_else(|| panic!("no {head} line in {text}"))
}

/// The conductor rows of a saved geometry line as `(cond, kind=name, x, h,
/// units)` tokens, and the closing ` cond=k` that restores the selection, if any.
fn saved_rows(line: &str) -> (Vec<[String; 5]>, Option<String>) {
    let tokens: Vec<&str> = line.split(' ').collect();
    let mut rows = Vec::new();
    let mut restore = None;
    let mut i = 0;
    while i < tokens.len() {
        if let Some(c) = tokens[i].strip_prefix("cond=") {
            let rest = &tokens[i + 1..];
            if rest.len() >= 4 && !rest[0].starts_with("cond=") && rest[3].starts_with("units=") {
                rows.push([
                    c.to_string(),
                    rest[0].to_string(),
                    rest[1].to_string(),
                    rest[2].to_string(),
                    rest[3].to_string(),
                ]);
                i += 5;
                continue;
            }
            restore = Some(c.to_string());
        }
        i += 1;
    }
    (rows, restore)
}

/// A fresh engine with the Save in `dir` compiled and solved.
fn reload(dir: &Path) -> Dss {
    let mut back = Dss::new();
    back.command(&format!("compile \"{}\"", slash(&dir.join("Master.dss"))));
    back.command("solve");
    assert!(back.errors().is_empty(), "reload: {:?}", errors(&back));
    back
}

// ----- the scripts A to I ------------------------------------------------------

/// Script A: `units=m` on the line of conductor 2 only belongs to conductor 2
/// and becomes the default for conductor 3. Conductor 1 keeps feet.
#[test]
fn script_a_units_on_line_2_belong_to_conductor_2_and_carry_on() {
    assert_solvable(
        "a_units_on_line_2_only.dss",
        &["ft", "m", "m"],
        &[
            phase(-4.0, 28.0, "ft"),
            phase(0.0, 28.0, "m"),
            phase(4.0, 28.0, "m"),
        ],
        &[(2, 1, 0.458_102_1), (3, 2, 0.650_345_1)],
    );
}

/// Script B1: the `units=mm` of the New line is the default only and lands on
/// conductor 1, selected without its own. `units=` after `cond=2` and `cond=4`
/// belong to those conductors.
#[test]
fn script_b1_units_after_cond_belong_to_that_conductor() {
    assert_solvable(
        "b1_units_after_cond.dss",
        &["mm", "m", "m", "ft"],
        &[
            phase(-4.0, 28.0, "mm"),
            phase(0.0, 28.0, "m"),
            phase(4.0, 28.0, "m"),
            neutral(0.0, 24.0, "ft"),
        ],
        &[(2, 1, 0.414_346_6), (4, 1, 0.577_561_4)],
    );
}

/// Script B2: a `units=` written before the line's `cond=` belongs to the
/// conductor selected before it (rule 4: until the next `cond=`).
#[test]
fn script_b2_units_before_cond_belong_to_the_previous_conductor() {
    assert_solvable(
        "b2_units_before_cond.dss",
        &["m", "m", "ft", "ft"],
        &[
            phase(-4.0, 28.0, "m"),
            phase(0.0, 28.0, "m"),
            phase(4.0, 28.0, "ft"),
            neutral(0.0, 24.0, "ft"),
        ],
        &[(2, 1, 0.650_345_1), (4, 3, 0.752_457_3)],
    );
}

/// Script C: `cond=` in the order 4, 1, 3, 2 builds the geometry of script C
/// in order (C0). Its Save rows equal C0's, and its line closes on its
/// selection, conductor 2.
#[test]
fn script_c_cond_in_any_order() {
    let scratch = Scratch::new("c");
    let mut c = assert_solvable(
        "c_any_cond_order.dss",
        &["ft"; 4],
        &c0_in(["ft"; 4]),
        &[(2, 1, 0.794_511_1)],
    );
    let mut c0 = compile("c0_cond_in_order.dss");
    assert_eq!(conductors(&c, "g"), conductors(&c0, "g"));
    let (rows_c, restore_c) = saved_rows(&save(&mut c, &scratch.dir("c"), "g"));
    let (rows_c0, restore_c0) = saved_rows(&save(&mut c0, &scratch.dir("c0"), "g"));
    assert_eq!(rows_c, rows_c0);
    assert_eq!(rows_c.len(), 4);
    assert_eq!(restore_c.as_deref(), Some("2"));
    assert_eq!(restore_c0, None);
}

/// Script C in conductor order.
#[test]
fn script_c0_cond_in_order() {
    assert_solvable(
        "c0_cond_in_order.dss",
        &["ft"; 4],
        &c0_in(["ft"; 4]),
        &[
            (2, 1, 0.794_511_1),
            (4, 1, 0.752_457_3),
            (4, 4, 1.546_497_0),
        ],
    );
}

/// Script D: `units=m` after the first `cond=` selected (3) becomes the
/// default, so conductors 1 and 2, selected later without their own, are in
/// metres too.
#[test]
fn script_d_units_on_the_first_selected_conductor_carry_to_the_rest() {
    assert_solvable(
        "d_out_of_order_after_a_default.dss",
        &["m", "m", "m"],
        &[
            phase(-4.0, 28.0, "m"),
            phase(0.0, 28.0, "m"),
            phase(4.0, 28.0, "m"),
        ],
        &[(2, 1, 0.650_345_1)],
    );
}

/// Script E: `cond=11` on four conductors is refused and selects none, so the
/// `wire=`/`x=`/`h=` after it are refused too and conductor 1 keeps (-4, 28).
#[test]
fn script_e_cond_out_of_range_is_refused() {
    let mut dss = compile("e_cond_out_of_range.dss");
    assert_eq!(
        errors(&dss),
        [
            out_of_range("g", 11, "has 4 conductors"),
            no_cond("g", "Wire"),
            no_cond("g", "X"),
            no_cond("g", "H"),
        ]
    );
    assert_eq!(dss.errors()[0].code, Some(10102));
    assert!(solved(&dss));
    let g = conductors(&dss, "g");
    assert_eq!((g[0].x, g[0].h), (-4.0, 28.0));
    assert_eq!(units(&dss, "g"), ["ft"; 4]);
    assert_x(&dss, "L", &c0_in(["ft"; 4]), &[(2, 1, 0.794_511_1)], "E");
    assert_eq!(cond(&mut dss, "g"), "2");
    assert_eq!(
        cond_trace("e_cond_out_of_range.dss", "g"),
        ["0", "4", "1", "0", "3", "2"]
    );
}

/// Script F: conductor 2 never gets a wire. The solve reports exactly one
/// error, naming conductor 2, and nothing is solved. The Save writes the rows
/// of conductors 1, 3 and 4 in feet.
#[test]
fn script_f_a_skipped_conductor_is_one_error() {
    let scratch = Scratch::new("f");
    let mut dss = compile("f_conductor_2_skipped.dss");
    assert_eq!(errors(&dss), [missing_at_calc("g", 2)]);
    assert!(!solved(&dss));
    assert!(dss.circuit().unwrap().solution.solution_abort);
    // Conductor 2, never selected, reads the default feet (rule 8).
    assert_eq!(units(&dss, "g"), ["ft"; 4]);
    let (rows, restore) = saved_rows(&save(&mut dss, &scratch.dir("save"), "g"));
    let picked: Vec<(&str, &str)> = rows
        .iter()
        .map(|r| (r[0].as_str(), r[4].as_str()))
        .collect();
    assert_eq!(
        picked,
        [("1", "units=ft"), ("3", "units=ft"), ("4", "units=ft")]
    );
    assert_eq!(restore, None); // the selection, 4, is the last row
}

/// Script G: the conductor data on the New line, before any `cond=`, is
/// refused property by property, and conductor 1, left without a wire, is the
/// one error of the solve.
#[test]
fn script_g_data_on_the_new_line_is_refused() {
    let scratch = Scratch::new("g");
    let mut dss = compile("g_data_on_the_new_line_with_units.dss");
    assert_eq!(
        errors(&dss),
        [
            no_cond("g", "Wire"),
            no_cond("g", "X"),
            no_cond("g", "H"),
            missing_at_calc("g", 1),
        ]
    );
    let g = conductors(&dss, "g");
    assert_eq!(
        g[0],
        Conductor {
            object: None,
            x: 0.0,
            h: 0.0,
            unit: "ft"
        }
    );
    let (rows, _) = saved_rows(&save(&mut dss, &scratch.dir("save"), "g"));
    let picked: Vec<(&str, &str)> = rows
        .iter()
        .map(|r| (r[0].as_str(), r[4].as_str()))
        .collect();
    assert_eq!(
        picked,
        [("2", "units=ft"), ("3", "units=ft"), ("4", "units=ft")]
    );
}

/// Script G2: script G without `units=`. The rows of conductors 2 to 4 are in
/// feet, the default, never metres.
#[test]
fn script_g2_data_on_the_new_line_without_units_is_refused() {
    let scratch = Scratch::new("g2");
    let mut dss = compile("g2_data_on_the_new_line_without_units.dss");
    assert_eq!(
        errors(&dss),
        [
            no_cond("g", "Wire"),
            no_cond("g", "X"),
            no_cond("g", "H"),
            missing_at_calc("g", 1),
        ]
    );
    assert_eq!(units(&dss, "g"), ["ft"; 4]);
    assert_eq!(conductors(&dss, "g")[0].object, None);
    let (rows, _) = saved_rows(&save(&mut dss, &scratch.dir("save"), "g"));
    let picked: Vec<(&str, &str)> = rows
        .iter()
        .map(|r| (r[0].as_str(), r[4].as_str()))
        .collect();
    assert_eq!(
        picked,
        [("2", "units=ft"), ("3", "units=ft"), ("4", "units=ft")]
    );
}

/// Script H: a one-conductor geometry written without `cond=` is no
/// exception: its data is refused and its conductor is the one calculation
/// error. The Save writes no conductor row.
#[test]
fn script_h_one_conductor_needs_cond_too() {
    let scratch = Scratch::new("h");
    let mut dss = compile("h_one_conductor_without_cond.dss");
    assert_eq!(
        errors(&dss),
        [
            no_cond("g", "Wire"),
            no_cond("g", "X"),
            no_cond("g", "H"),
            missing_at_calc("g", 1),
        ]
    );
    assert_eq!(units(&dss, "g"), ["ft"]);
    let (rows, restore) = saved_rows(&save(&mut dss, &scratch.dir("save"), "g"));
    assert!(rows.is_empty(), "{rows:?}");
    assert_eq!(restore, None);
}

/// Script H written with `cond=1`. A single conductor's reactance does not
/// depend on where it hangs, its shunt capacitance does: the height of 28 ft
/// gives C11 12.299668 nF/mi.
#[test]
fn script_h2_one_conductor_with_cond() {
    let dss = assert_solvable(
        "h2_one_conductor_with_cond.dss",
        &["ft"],
        &[phase(0.0, 28.0, "ft")],
        &[(1, 1, 1.383_848_5)],
    );
    assert_c11(&dss, "L", 28.0 * 0.3048, "H2");
    assert!((shunt_c11(28.0 * 0.3048, RADIUS_P) - 12.299_668).abs() < 1e-6);
}

/// Script I: conductor 1 selected again takes its new unit and coordinate.
/// Conductors 2 and 3 keep feet.
#[test]
fn script_i_a_conductor_selected_again_takes_its_new_unit() {
    let dss = assert_solvable(
        "i_cond_given_twice.dss",
        &["m", "ft", "ft"],
        &[
            phase(-6.0, 28.0, "m"),
            phase(0.0, 28.0, "ft"),
            phase(4.0, 28.0, "ft"),
        ],
        &[(2, 1, 0.452_833_0)],
    );
    let g = conductors(&dss, "g");
    assert_eq!((g[0].x, g[0].h), (-6.0, 28.0));
}

// ----- the further variants -----------------------------------------------------

/// The geometry of script C written on one line, one property per line, and
/// as separate `Edit` commands reads the same: same conductors, same reactance.
fn assert_reads_like_c0(name: &str) {
    let dss = assert_solvable(name, &["ft"; 4], &c0_in(["ft"; 4]), &[(2, 1, 0.794_511_1)]);
    let c0 = compile("c0_cond_in_order.dss");
    assert_eq!(conductors(&dss, "g"), conductors(&c0, "g"), "{name}");
    let (x, x0) = (xmatrix(&dss, "L"), xmatrix(&c0, "L"));
    for i in 0..4 {
        for j in 0..4 {
            assert!(
                (x[i][j] - x0[i][j]).abs() <= 1e-12 * x0[i][j].abs(),
                "{name}: X{}{}",
                i + 1,
                j + 1
            );
        }
    }
}

#[test]
fn variant_one_line_reads_like_many_lines() {
    assert_reads_like_c0("v1_one_line.dss");
}

#[test]
fn variant_one_property_per_line() {
    assert_reads_like_c0("v2_one_property_per_line.dss");
}

#[test]
fn variant_separate_edit_commands() {
    assert_reads_like_c0("v3_separate_edit_commands.dss");
}

/// Script B2 with each `units=` an `Edit` command of its own reads like B2: the
/// unit belongs to the conductor the command before it selected, and the same
/// conductors give the same reactance.
fn assert_reads_like_b2(name: &str) {
    let dss = assert_solvable(
        name,
        &["m", "m", "ft", "ft"],
        &[
            phase(-4.0, 28.0, "m"),
            phase(0.0, 28.0, "m"),
            phase(4.0, 28.0, "ft"),
            neutral(0.0, 24.0, "ft"),
        ],
        &[(2, 1, 0.650_345_1), (4, 3, 0.752_457_3)],
    );
    let b2 = compile("b2_units_before_cond.dss");
    assert_eq!(conductors(&dss, "g"), conductors(&b2, "g"), "{name}");
}

#[test]
fn variant_b2_as_separate_edit_commands() {
    assert_reads_like_b2("v3b_b2_as_separate_edit_commands.dss");
}

/// `cond=` in descending order: the `units=m` of conductor 4, the first one
/// selected, carries to 3, 2 and 1.
#[test]
fn variant_descending_cond_carries_the_first_unit() {
    assert_solvable(
        "v4_descending_cond.dss",
        &["m"; 4],
        &c0_in(["m"; 4]),
        &[(2, 1, 0.650_345_1), (4, 1, 0.608_291_3)],
    );
}

/// `like=` selects no conductor, so the `x=` after it is refused and the copy
/// equals its source.
#[test]
fn variant_like_then_data_without_cond_is_refused() {
    let dss = compile("v5_like_then_data_without_cond.dss");
    assert_eq!(errors(&dss), [no_cond("g", "X")]);
    assert!(solved(&dss));
    assert_eq!(conductors(&dss, "g"), conductors(&dss, "base"));
    assert_eq!(conductors(&dss, "g")[0].x, -4.0);
    assert_x(&dss, "L", &c0_in(["ft"; 4]), &[(2, 1, 0.794_511_1)], "V5");
    assert_eq!(
        cond_trace("v5_like_then_data_without_cond.dss", "g"),
        ["0", "0"]
    );
}

/// A `like=` whose source does not exist selects no conductor either: the
/// `x=` after it is refused, conductor 2, selected before it, stays at x = 0,
/// and the Save writes no selection.
#[test]
fn variant_like_not_found_selects_nothing() {
    let name = "v5b_like_not_found_then_data.dss";
    let scratch = Scratch::new("v5b");
    let mut dss = compile(name);
    assert_eq!(
        errors(&dss),
        [
            "Error in LineGeometry MakeLike: \"nosuchgeom\" not found.".to_string(),
            no_cond("g", "X"),
        ]
    );
    assert!(solved(&dss));
    let c0 = compile("c0_cond_in_order.dss");
    assert_eq!(conductors(&dss, "g"), conductors(&c0, "g"));
    assert_eq!(conductors(&dss, "g")[1].x, 0.0);
    assert_x(&dss, "L", &c0_in(["ft"; 4]), &[(2, 1, 0.794_511_1)], "V5b");
    assert_eq!(
        cond_trace(name, "g"),
        ["0", "1", "2", "3", "4", "2", "0", "0"]
    );
    let (rows, restore) = saved_rows(&save(&mut dss, &scratch.dir("save"), "g"));
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[1], ["2", "wire=acsr_556_5", "x=0", "h=28", "units=ft"]);
    assert_eq!(restore, None);
}

/// A `like=` copy takes its source's default unit. Conductor 3, which neither
/// source wrote, reads its `x=` and `h=` in metres in the copy of a source
/// whose `units=m` is the default (`gm`, m m m, X32 0.6503451 Ω/mi), and in
/// feet in the copy of a source that wrote no unit (`gf`, ft ft ft, X32
/// 0.7945111). A copy that kept its own default of feet would read `gm` as
/// m m ft. `g`, a geometry that already holds three conductors in feet, takes
/// `base_m`'s default by `like=` too, so its conductor 3 written again reads
/// in metres, m m m, as on r4133, where dss_capi reads m m ft (X32 0.458037).
/// The oracles apply a `~` after `like=` to the source, so the decks fed to
/// them write conductor 3 of `gm` and `gf` by an `Edit` of its own.
#[test]
fn variant_like_copy_takes_the_source_unit() {
    let name = "v5c_like_takes_the_source_unit.dss";
    let mut dss = compile(name);
    assert!(dss.errors().is_empty(), "{name}: {:?}", errors(&dss));
    assert!(solved(&dss), "{name} did not solve");
    assert_eq!(units(&dss, "gm"), ["m"; 3]);
    assert_eq!(units(&dss, "gf"), ["ft"; 3]);
    assert_eq!(units(&dss, "g"), ["m"; 3]);
    assert_eq!(query(&mut dss, "LineGeometry.gm.Units"), "m");
    assert_eq!(query(&mut dss, "LineGeometry.gf.Units"), "ft");
    assert_eq!(query(&mut dss, "LineGeometry.g.Units"), "m");
    let row = |u| {
        [
            phase(-4.0, 28.0, u),
            phase(0.0, 28.0, u),
            phase(4.0, 28.0, u),
        ]
    };
    assert_x(
        &dss,
        "Lm",
        &row("m"),
        &[(2, 1, 0.650_345_1), (3, 2, 0.650_345_1)],
        "V5c gm",
    );
    assert_x(&dss, "Lg", &row("m"), &[(3, 2, 0.650_345_1)], "V5c g");
    assert_x(
        &dss,
        "Lf",
        &row("ft"),
        &[(2, 1, 0.794_511_1), (3, 2, 0.794_511_1)],
        "V5c gf",
    );
}

/// `units=m` between `x=` and `h=` of conductor 2 covers both of them: the
/// geometry of script A.
#[test]
fn variant_units_between_two_data_of_one_conductor() {
    let scratch = Scratch::new("v6");
    let mut v6 = assert_solvable(
        "v6_units_between_two_data.dss",
        &["ft", "m", "m"],
        &[
            phase(-4.0, 28.0, "ft"),
            phase(0.0, 28.0, "m"),
            phase(4.0, 28.0, "m"),
        ],
        &[(2, 1, 0.458_102_1)],
    );
    let mut a = compile("a_units_on_line_2_only.dss");
    let (rows_v6, _) = saved_rows(&save(&mut v6, &scratch.dir("v6"), "g"));
    let (rows_a, _) = saved_rows(&save(&mut a, &scratch.dir("a"), "g"));
    assert_eq!(rows_v6, rows_a);
}

/// Cable geometries follow the same rules, each Line against [`cable_z`] at
/// the positions the rules give: the concentric-neutral geometry in feet
/// (CNData `250_1/3` six inches apart, four feet deep), the tape-shield
/// geometry in inches and the mixed geometry (inches, with a bare neutral in
/// feet, reduced). The last two also equal their twins written by hand in
/// metres.
#[test]
fn variant_cable_geometries() {
    use Holds::{Bare, Cn, Ts};
    let dss = compile("v7_cable_geometries.dss");
    assert!(dss.errors().is_empty(), "{:?}", errors(&dss));
    assert!(solved(&dss));
    assert_eq!(units(&dss, "cn"), ["ft"; 3]);
    assert_eq!(units(&dss, "ts"), ["in"; 3]);
    assert_eq!(units(&dss, "ts_m"), ["m"; 3]);
    assert_eq!(units(&dss, "mixed"), ["in", "in", "in", "ft"]);
    assert_eq!(units(&dss, "mixed_m"), ["m"; 4]);
    let cn = [-0.5, 0.0, 0.5].map(|x| laid(Cn, x, -4.0, "ft"));
    assert_cable(
        &dss,
        "cn",
        (&cn, false),
        X_BAND,
        &[(2, 1, 0.317_819_1, 0.028_323_1)],
        "v7 cn",
    );
    let ts = [-6.0, 0.0, 6.0].map(|x| laid(Ts, x, -48.0, "in"));
    assert_cable(
        &dss,
        "ts",
        (&ts, false),
        TS_BAND,
        &[(2, 1, 0.535_156_1, 0.679_076_0)],
        "v7 ts",
    );
    let mixed = [
        laid(Cn, -6.0, -48.0, "in"),
        laid(Ts, 0.0, -48.0, "in"),
        laid(Cn, 6.0, -48.0, "in"),
        laid(Bare, 1.0, -4.0, "ft"),
    ];
    assert_cable(
        &dss,
        "mixed",
        (&mixed, true),
        TS_BAND,
        &[(2, 1, 0.284_507_4, 0.139_567_9)],
        "v7 mixed",
    );
    for (line, twin) in [("ts", "ts_m"), ("mixed", "mixed_m")] {
        for real in [true, false] {
            let (a, b) = (zpart(&dss, line, real), zpart(&dss, twin, real));
            assert_eq!(a.len(), 3, "{line}");
            for i in 0..3 {
                for j in 0..3 {
                    assert!(
                        (a[i][j] - b[i][j]).abs() <= 1e-9 * b[i][j].abs().max(1e-6),
                        "{line} vs {twin} ({}): Z{}{} {} vs {}",
                        if real { "R" } else { "X" },
                        i + 1,
                        j + 1,
                        a[i][j],
                        b[i][j]
                    );
                }
            }
        }
    }
}

/// `cncables=` and `tscables=` with one cable per phase, followed by `wires=`
/// with no conductor selected: the phases hold cables, so the `wires=` list is
/// the bare neutral, as the help of `cncables` and `wires` describes, and so is
/// a `wires=` after a `conductors=` list with cable phases and after a `like=`
/// copy of a cable fill (`cn_like`). Each geometry equals its twin written one
/// conductor at a time. With bare phases the `wires=` list is every conductor,
/// a cable outside the phases included (`bare_all`), a reading pinned without a
/// Line: the position that held the cable keeps the cable model after the wire
/// replaces it, a question outside these rules. The concentric-neutral and
/// tape-shield Lines equal [`cable_z`] for three cables six inches apart, four
/// feet deep, and the bare ACSR_4/0 neutral six inches beside the third cable,
/// reduced to the cores.
#[test]
fn variant_plural_cables_then_neutral_wires() {
    use Holds::{Bare, Cn, Ts};
    let dss = compile("v8_plural_cables_then_neutral_wires.dss");
    assert!(dss.errors().is_empty(), "{:?}", errors(&dss));
    assert!(solved(&dss));
    for geom in ["cn", "cn_twin", "cw", "cn_like", "ts", "ts_twin"] {
        assert_eq!(units(&dss, geom), ["ft"; 4], "{geom}");
        assert_eq!(
            conductors(&dss, geom)[3].object.as_deref(),
            Some("acsr_4/0"),
            "{geom}"
        );
    }
    assert_eq!(conductors(&dss, "cn"), conductors(&dss, "cn_twin"));
    assert_eq!(conductors(&dss, "cw"), conductors(&dss, "cn_twin"));
    assert_eq!(conductors(&dss, "ts"), conductors(&dss, "ts_twin"));
    assert_eq!(conductors(&dss, "cn_like"), conductors(&dss, "cn_twin"));
    let objects = |geom: &str| -> Vec<Option<String>> {
        conductors(&dss, geom)
            .into_iter()
            .map(|c| c.object)
            .collect()
    };
    assert_eq!(objects("cn_src")[3], None);
    let wire = |name: &str| Some(name.to_string());
    assert_eq!(
        objects("bare_all"),
        [
            wire("acsr_556_5"),
            wire("acsr_556_5"),
            wire("acsr_556_5"),
            wire("acsr_4/0")
        ]
    );
    let laid_as = |holds| {
        let mut row = [-0.5, 0.0, 0.5, 1.0].map(|x| laid(holds, x, -4.0, "ft"));
        row[3].holds = Bare;
        row
    };
    assert_cable(
        &dss,
        "cn",
        (&laid_as(Cn), true),
        X_BAND,
        &[
            (1, 1, 0.738_155_6, 0.464_518_9),
            (2, 1, 0.251_504_6, 0.051_760_4),
            (3, 1, 0.199_562_0, -0.011_290_7),
            (3, 3, 0.694_716_0, 0.413_012_2),
        ],
        "v8 cn",
    );
    assert_cable(
        &dss,
        "ts",
        (&laid_as(Ts), true),
        TS_BAND,
        &[(2, 1, 0.320_000_2, 0.378_375_7)],
        "v8 ts",
    );
    for (line, twin) in [
        ("cn", "cn_twin"),
        ("cw", "cn_twin"),
        ("cn_like", "cn_twin"),
        ("ts", "ts_twin"),
    ] {
        for real in [true, false] {
            let (a, b) = (zpart(&dss, line, real), zpart(&dss, twin, real));
            assert_eq!(a.len(), 3, "{line}");
            for i in 0..3 {
                for j in 0..3 {
                    assert!(
                        (a[i][j] - b[i][j]).abs() <= 1e-12 * b[i][j].abs().max(1e-6),
                        "{line} vs {twin} ({}): Z{}{} {} vs {}",
                        if real { "R" } else { "X" },
                        i + 1,
                        j + 1,
                        a[i][j],
                        b[i][j]
                    );
                }
            }
        }
    }
}

/// `nphases` defaults to 3, as the documentation says. `cn`, written with no
/// `nphases=`, takes its three `cncables=` in the phase positions and its
/// `wires=` as the neutral, and reads like `cn_twin`, which writes `nphases=3`
/// (the V8 values). `pair` has two conductors and no `nphases=`: both are
/// phases and its Line has two, as with `pair_twin`'s `nphases=2` (r4133 X11
/// 0.5018593, X21 0.04507474 Ω/mi, R11 0.9072926 on `pair_twin`). An
/// explicit `nphases=0` leaves no phase position, so the cable list of `cn0`
/// is refused. r4133 holds `nphases` 0 for a geometry that writes none: it
/// refuses `cn`'s `wires=` (10103), fails the solve with 303 and 482 access
/// violations, and reads `pair` as three phases.
#[test]
fn nphases_defaults_to_three() {
    use Holds::{Bare, Cn};
    let mut dss = compile("n3_nphases_defaults_to_three.dss");
    assert_eq!(
        errors(&dss),
        [
            "LineGeometry.cn0: Unexpected number (3) of objects; expected 0 objects: the \
             geometry has no phases."
        ]
    );
    assert!(solved(&dss));
    for geom in ["cn", "pair"] {
        assert_eq!(
            query(&mut dss, &format!("LineGeometry.{geom}.NPhases")),
            "3"
        );
    }
    assert_eq!(query(&mut dss, "Line.pair.Phases"), "2");
    assert_eq!(conductors(&dss, "cn"), conductors(&dss, "cn_twin"));
    assert_eq!(conductors(&dss, "pair"), conductors(&dss, "pair_twin"));
    let mut cn = [-0.5, 0.0, 0.5, 1.0].map(|x| laid(Cn, x, -4.0, "ft"));
    cn[3].holds = Bare;
    assert_cable(
        &dss,
        "cn",
        (&cn, true),
        X_BAND,
        &[
            (1, 1, 0.738_155_6, 0.464_518_9),
            (2, 1, 0.251_504_6, 0.051_760_4),
        ],
        "n3 cn",
    );
    let pair = [-0.5, 0.5].map(|x| laid(Cn, x, -4.0, "ft"));
    for line in ["pair", "pair_twin"] {
        assert_cable(
            &dss,
            line,
            (&pair, true),
            X_BAND,
            &[
                (1, 1, 0.907_292_6, 0.501_859_3),
                (2, 1, 0.391_838_2, 0.045_074_74),
            ],
            "n3 pair",
        );
    }
    for (line, twin) in [("cn", "cn_twin"), ("pair", "pair_twin")] {
        for real in [true, false] {
            assert_eq!(zpart(&dss, line, real), zpart(&dss, twin, real), "{line}");
        }
    }
}

/// A cable list of another length than the phase count is refused and fills
/// nothing: four cables on three phases, as `cncables=` and `tscables=`, and
/// two. With conductor 2 selected the three cables still go to the phase
/// positions and conductor 2 stays selected. A `wires=` list of another length
/// than its positions is refused and fills nothing: one name where bare phases
/// and a cable last make four positions (`bare`), two for the one neutral after
/// cable phases (`two_wires`). A `wires=` with no position to fill is refused
/// with the reason: after cables on every conductor, also when `nphases`
/// exceeds `nconds`, and on a geometry with no conductors.
#[test]
fn a_cable_or_wire_list_of_the_wrong_length_is_refused() {
    let mut dss = compile("v8b_cable_and_wire_counts.dss");
    let count = |geom: &str, got: usize, want: usize, why: &str| {
        format!(
            "LineGeometry.{geom}: Unexpected number ({got}) of objects; expected {want} objects{why}."
        )
    };
    let no_neutral = ": the phases hold cables and no conductor is a neutral";
    let no_conductor = ": the geometry has no conductors. Set NConds first";
    assert_eq!(
        errors(&dss),
        [
            count("four", 4, 3, ""),
            count("four_ts", 4, 3, ""),
            count("two", 2, 3, ""),
            count("full", 1, 0, no_neutral),
            count("full", 2, 0, no_neutral),
            count("over", 1, 0, no_neutral),
            count("bare", 1, 4, ""),
            count("two_wires", 2, 1, ""),
            count("empty", 1, 0, no_conductor),
            count("empty", 1, 0, no_conductor),
        ]
    );
    let held = |dss: &Dss, geom: &str| -> Vec<Option<String>> {
        conductors(dss, geom)
            .into_iter()
            .map(|c| c.object)
            .collect()
    };
    let cn = || Some("250_1/3".to_string());
    for geom in ["four", "four_ts", "two"] {
        assert_eq!(held(&dss, geom), [None, None, None, None], "{geom}");
    }
    assert_eq!(held(&dss, "sel"), [cn(), cn(), cn(), None]);
    assert_eq!(cond(&mut dss, "sel"), "2");
    assert_eq!(held(&dss, "full"), [cn(), cn(), cn()]);
    assert_eq!(held(&dss, "over"), [cn(), cn()]);
    let wire = || Some("acsr_556_5".to_string());
    assert_eq!(held(&dss, "bare"), [wire(), wire(), wire(), cn()]);
    assert_eq!(held(&dss, "two_wires"), [cn(), cn(), cn(), None]);
    assert!(held(&dss, "empty").is_empty());
}

/// A default on the New line followed by conductor 1's own unit: the own unit
/// becomes the default for conductors 2 and 3, and the New line's unit lands on
/// no conductor.
#[test]
fn variant_a_unit_before_cond_does_not_stick_to_the_last_conductor() {
    assert_solvable(
        "j08_default_then_own_unit.dss",
        &["ft"; 3],
        &[
            phase(-4.0, 28.0, "ft"),
            phase(0.0, 28.0, "ft"),
            phase(4.0, 28.0, "ft"),
        ],
        &[(2, 1, 0.794_511_1)],
    );
}

/// Conductor 2's `units=m` becomes the default, and conductor 1, selected
/// again after it, keeps the feet it took when first selected: its `x=-5` and
/// its `h=28` are feet, ft m m, X21 0.4579689 Ω/mi. Filling the default into
/// conductor 1 again would give m m m and X21 0.6232686.
#[test]
fn variant_a_conductor_selected_again_keeps_its_own_unit() {
    let dss = assert_solvable(
        "j_reselect_keeps_its_own_unit.dss",
        &["ft", "m", "m"],
        &[
            phase(-5.0, 28.0, "ft"),
            phase(0.0, 28.0, "m"),
            phase(4.0, 28.0, "m"),
        ],
        &[
            (2, 1, 0.457_968_9),
            (3, 1, 0.453_640_4),
            (3, 2, 0.650_345_1),
        ],
    );
    let g = conductors(&dss, "g");
    assert_eq!((g[0].x, g[0].h), (-5.0, 28.0));
}

/// Two Lines on script F's geometry report the missing conductor once.
#[test]
fn variant_two_lines_on_one_broken_geometry_give_one_error() {
    let dss = compile("f2_two_lines_share_a_skipped_conductor.dss");
    assert_eq!(errors(&dss), [missing_at_calc("g", 2)]);
    assert!(!solved(&dss));
}

/// Two broken geometries on three Lines (L1 on `g2`, L2 and L3 on `g1`) are
/// one error of each solve: each geometry named once, in the order of the
/// Lines (the reverse order would name `g1` first), wrapped once by the
/// command.
#[test]
fn two_broken_geometries_are_one_error_in_build_order() {
    let mut dss = compile("f3_two_broken_geometries.dss");
    let both = "LineGeometry.g2: conductor 1 has no wire, cncable or tscable. LineGeometry.g1: \
                conductor 2 has no wire, cncable or tscable.";
    assert_eq!(
        errors(&dss),
        [format!("Error Encountered in CalcVoltageBases: {both}")]
    );
    assert!(!solved(&dss));
    assert!(dss.circuit().unwrap().solution.solution_abort);
    dss.command("Solve");
    assert_eq!(
        errors(&dss)[1..],
        [format!("Error Encountered in Solve: {both}")]
    );
    assert_eq!(dss.errors()[1].code, Some(482));
    assert!(!solved(&dss));
}

/// A second `New` of an existing geometry edits it with no conductor selected,
/// so its `x=` is refused and the geometry stays script C. It keeps the
/// geometry's default unit: the `units=m` of the earlier definition of `h` is
/// the unit of the conductor its redefinition writes without one, so Line `Lh`
/// hangs at 28 m, C11 10.573827 nF/mi.
#[test]
fn variant_new_again_selects_nothing() {
    let dss = compile("n1_new_again_selects_nothing.dss");
    assert_eq!(errors(&dss), [no_cond("g", "X")]);
    assert!(solved(&dss));
    let g = conductors(&dss, "g");
    assert_eq!((g[3].x, g[3].h), (0.0, 24.0));
    assert_eq!(units(&dss, "g"), ["ft"; 4]);
    assert_x(&dss, "L", &c0_in(["ft"; 4]), &[(2, 1, 0.794_511_1)], "N1");
    assert_eq!(units(&dss, "h"), ["m"]);
    assert_x(&dss, "Lh", &[phase(0.0, 28.0, "m")], &[], "N1");
    assert_c11(&dss, "Lh", 28.0, "N1");
    assert!((shunt_c11(28.0, RADIUS_P) - 10.573_827).abs() < 1e-6);
    let trace = cond_trace("n1_new_again_selects_nothing.dss", "g");
    assert_eq!(trace, ["0", "1", "2", "3", "4", "0"]);
}

/// `nconds=` re-allocates the conductors and selects none, on a `~` line (`g`)
/// and as an `Edit` (`e`): the `x=5` after it is refused, and conductor 2,
/// selected before it, keeps the x 0 of the re-allocation. Written again with
/// conductor 2 given no `x=`, both geometries are script C in feet, so the
/// `x=5` placed on conductor 2 would move X21 from 0.7945111 Ω/mi.
#[test]
fn variant_nconds_selects_nothing() {
    let name = "n2_nconds_selects_nothing.dss";
    let mut edited = replay_until(name, "Edit LineGeometry.e cond=1");
    assert_eq!(cond(&mut edited, "e"), "0");
    let e = conductors(&edited, "e");
    assert_eq!((e[1].object.as_deref(), e[1].x, e[1].h), (None, 0.0, 0.0));
    assert_eq!(
        cond_trace(name, "g"),
        ["0", "1", "2", "0", "0", "1", "2", "3", "4"]
    );

    let dss = compile(name);
    assert_eq!(errors(&dss), [no_cond("g", "X"), no_cond("e", "X")]);
    assert!(solved(&dss));
    for (geom, line) in [("g", "L"), ("e", "Le")] {
        assert_eq!(units(&dss, geom), ["ft"; 4], "{geom}");
        let xs: Vec<f64> = conductors(&dss, geom).iter().map(|c| c.x).collect();
        assert_eq!(xs, [-4.0, 0.0, 4.0, 0.0], "{geom}");
        assert_x(&dss, line, &c0_in(["ft"; 4]), &[(2, 1, 0.794_511_1)], "N2");
    }
}

// ----- one pin per rule ---------------------------------------------------------

/// Rule 1: `cond=` may come in any order: no out-of-order deck raises an
/// error, and each selects the conductor it names.
#[test]
fn cond_may_come_in_any_order() {
    for name in [
        "c_any_cond_order.dss",
        "d_out_of_order_after_a_default.dss",
        "v4_descending_cond.dss",
    ] {
        assert!(compile(name).errors().is_empty(), "{name}");
    }
    assert_eq!(
        cond_trace("c_any_cond_order.dss", "g"),
        ["0", "4", "1", "3", "2"]
    );
    assert_eq!(
        cond_trace("v4_descending_cond.dss", "g"),
        ["0", "4", "3", "2", "1"]
    );
}

/// Rule 2: conductor data with no conductor selected is refused, stores
/// nothing, and has no side effect: a `cncable=` refused after `New` leaves the
/// geometry overhead (a later `cond=1 wire=` and its reactance are the
/// overhead ones, and its shunt capacitance is that of the default feet,
/// C11 12.299668 nF/mi at 28 ft).
#[test]
fn conductor_data_without_cond_is_refused() {
    for (name, props) in [
        (
            "g_data_on_the_new_line_with_units.dss",
            &["Wire", "X", "H"][..],
        ),
        (
            "g2_data_on_the_new_line_without_units.dss",
            &["Wire", "X", "H"],
        ),
        ("h_one_conductor_without_cond.dss", &["Wire", "X", "H"]),
        ("v5_like_then_data_without_cond.dss", &["X"]),
        ("n1_new_again_selects_nothing.dss", &["X"]),
    ] {
        let dss = compile(name);
        let refused: Vec<String> = props.iter().map(|p| no_cond("g", p)).collect();
        assert_eq!(errors(&dss)[..refused.len()], refused[..], "{name}");
    }
    let mut dss = compile("h2_one_conductor_with_cond.dss");
    dss.command("New LineGeometry.k nconds=1 nphases=1 cncable=250_1/3 tscable=1/0TS x=1 h=2");
    assert_eq!(
        errors(&dss),
        [
            no_cond("k", "CNCable"),
            no_cond("k", "TSCable"),
            no_cond("k", "X"),
            no_cond("k", "H"),
        ]
    );
    assert_eq!(
        conductors(&dss, "k"),
        [Conductor {
            object: None,
            x: 0.0,
            h: 0.0,
            unit: "ft"
        }]
    );
    dss.command("~ cond=1 wire=ACSR_556_5 x=0 h=28");
    dss.command("Edit Line.L geometry=k");
    dss.command("Solve");
    assert_eq!(errors(&dss).len(), 4, "{:?}", errors(&dss));
    assert_x(
        &dss,
        "L",
        &[phase(0.0, 28.0, "ft")],
        &[(1, 1, 1.383_848_5)],
        "k",
    );
    assert_eq!(units(&dss, "k"), ["ft"]);
    assert_c11(&dss, "L", 28.0 * 0.3048, "k");
}

/// Rule 3: a `units=` before the first `cond=` is the default only: it lands on
/// no conductor, and conductors selected without their own unit take it.
#[test]
fn units_before_the_first_cond_set_the_default_only() {
    // D: the `units=ft` of the New line is replaced by conductor 3's `m`
    // before conductors 1 and 2 are selected.
    let d = compile("d_out_of_order_after_a_default.dss");
    assert_eq!(units(&d, "g"), ["m"; 3]);
    // J08: the `units=m` of the New line lands on no conductor.
    let j = compile("j08_default_then_own_unit.dss");
    assert_eq!(units(&j, "g"), ["ft"; 3]);
    // G: conductors 2 to 4, selected without their own, take the default.
    let g = compile("g_data_on_the_new_line_with_units.dss");
    assert_eq!(units(&g, "g"), ["ft"; 4]);
    // B1: the default mm lands on conductor 1, selected without its own.
    let b1 = compile("b1_units_after_cond.dss");
    assert_eq!(units(&b1, "g")[0], "mm");
}

/// Rule 4: a `units=` after `cond=N` belongs to conductor N wherever it stands
/// before the next `cond=`, and carries to conductors selected later without
/// their own. A conductor selected again keeps the unit it has (J) unless a
/// `units=` follows (I).
#[test]
fn units_after_cond_belong_to_that_conductor_and_carry_forward() {
    for (name, want) in [
        ("a_units_on_line_2_only.dss", &["ft", "m", "m"][..]),
        ("b1_units_after_cond.dss", &["mm", "m", "m", "ft"]),
        ("b2_units_before_cond.dss", &["m", "m", "ft", "ft"]),
        (
            "v3b_b2_as_separate_edit_commands.dss",
            &["m", "m", "ft", "ft"],
        ),
        ("v6_units_between_two_data.dss", &["ft", "m", "m"]),
        ("i_cond_given_twice.dss", &["m", "ft", "ft"]),
        ("j_reselect_keeps_its_own_unit.dss", &["ft", "m", "m"]),
    ] {
        assert_eq!(units(&compile(name), "g"), want, "{name}");
    }
}

/// Rule 5: line breaks, `~` and separate `Edit` commands mean nothing, neither
/// for the conductor data nor for the conductor a `units=` belongs to.
#[test]
fn line_breaks_and_continuations_mean_nothing() {
    for name in [
        "v1_one_line.dss",
        "v2_one_property_per_line.dss",
        "v3_separate_edit_commands.dss",
    ] {
        assert_reads_like_c0(name);
    }
    assert_reads_like_b2("v3b_b2_as_separate_edit_commands.dss");
}

/// Rule 6: `cond=` out of range is an error (code 10102) and leaves no
/// conductor selected: `cond=11` on four conductors, `cond=1` before
/// `nconds=`, `cond=0`, and `cond=2` and `cond=-1` on one conductor. The `x=`
/// after them is refused.
#[test]
fn cond_out_of_range_is_refused_and_selects_nothing() {
    let dss = compile("r6_cond_zero_and_cond_before_nconds.dss");
    assert_eq!(
        errors(&dss),
        [
            out_of_range("g", 1, "has no conductors yet, set NConds first"),
            out_of_range("g", 0, "has 3 conductors"),
            no_cond("g", "X"),
        ]
    );
    assert_eq!(dss.errors()[0].code, Some(10102));
    assert_eq!(dss.errors()[1].code, Some(10102));
    assert_eq!(dss.errors()[2].code, None);
    assert!(solved(&dss));
    assert_x(
        &dss,
        "L",
        &[
            phase(-4.0, 28.0, "ft"),
            phase(0.0, 28.0, "ft"),
            phase(4.0, 28.0, "ft"),
        ],
        &[(2, 1, 0.794_511_1)],
        "R6",
    );
    assert_eq!(
        cond_trace("r6_cond_zero_and_cond_before_nconds.dss", "g"),
        ["0", "0", "0", "1", "2", "3"]
    );
    let e = compile("e_cond_out_of_range.dss");
    assert_eq!(errors(&e)[0], out_of_range("g", 11, "has 4 conductors"));
    let mut one = compile("h2_one_conductor_with_cond.dss");
    for value in [2, -1] {
        one.command("Edit LineGeometry.g cond=1");
        let before = one.errors().len();
        one.command(&format!("Edit LineGeometry.g cond={value}"));
        assert_eq!(
            errors(&one)[before..],
            [out_of_range("g", value, "has 1 conductor")]
        );
        assert_eq!(one.errors()[before].code, Some(10102));
        assert_eq!(cond(&mut one, "g"), "0", "cond={value}");
    }
}

/// A `cond=` whose value is not a number selects no conductor, as one out of
/// range does: the data on its line and on the next line are refused, and the
/// geometry stays script C in order. `cond= x=1` reads `x` as the value of
/// `cond` and `1` as the next property, `wire=`, which is refused too, so
/// conductor 2 keeps its wire.
#[test]
fn cond_that_is_not_a_number_selects_nothing() {
    let dss = compile("r6_cond_not_a_number.dss");
    let not_a_number = |value: &str| format!("Invalid inline math entry: \"{value}\"");
    assert_eq!(
        errors(&dss),
        [
            not_a_number("abc"),
            no_cond("g", "X"),
            not_a_number("abc"),
            no_cond("g", "X"),
            not_a_number("x"),
            "LineGeometry.g.Wire: WireData object \"1\" not found.".to_string(),
            no_cond("g", "Wire"),
        ]
    );
    assert!(solved(&dss));
    let c0 = compile("c0_cond_in_order.dss");
    assert_eq!(conductors(&dss, "g"), conductors(&c0, "g"));
    assert_x(
        &dss,
        "L",
        &c0_in(["ft"; 4]),
        &[(2, 1, 0.794_511_1), (3, 2, 0.794_511_1)],
        "R6N",
    );
    assert_eq!(
        cond_trace("r6_cond_not_a_number.dss", "g"),
        ["0", "1", "2", "3", "4", "1", "0", "3", "0", "0", "2", "0"]
    );
}

/// `cond=` takes a conductor number. A value that is not a whole number
/// (`1.5`, `2.9`) and a whole number past the integer range (`1e10`,
/// `2147483648`) are refused naming the value as written, select no conductor,
/// and the `x=` after each is refused too, so the geometry stays script C. Both
/// oracles round `1.5` to conductor 2 and `2.9` to 3 with no message and ignore
/// the other two, so their `x=` lands on conductor 3 (r4133 X21 0.6514911,
/// X32 0.8294189 Ω/mi), and dss_capi reports those two as `Invalid value
/// (1410065408)` and `(-2147483648)`.
#[test]
fn cond_that_is_not_a_whole_number_selects_nothing() {
    let mut dss = compile("r6c_cond_not_a_whole_number.dss");
    let refused = |value: &str, what: &str| {
        format!(
            "LineGeometry.g.Cond: cond={value} {what}, the geometry has 4 conductors. No \
             conductor is selected."
        )
    };
    assert_eq!(
        errors(&dss),
        [
            refused("1.5", "is not a conductor number"),
            no_cond("g", "X"),
            refused("2.9", "is not a conductor number"),
            no_cond("g", "X"),
            refused("1e10", "is out of range"),
            no_cond("g", "X"),
            refused("2147483648", "is out of range"),
            no_cond("g", "X"),
        ]
    );
    assert!(solved(&dss));
    assert_eq!(cond(&mut dss, "g"), "0");
    let c0 = compile("c0_cond_in_order.dss");
    assert_eq!(conductors(&dss, "g"), conductors(&c0, "g"));
    assert_x(
        &dss,
        "L",
        &c0_in(["ft"; 4]),
        &[(2, 1, 0.794_511_1), (3, 2, 0.794_511_1)],
        "R6c",
    );
    // A whole number written as a decimal selects its conductor.
    dss.command("Edit LineGeometry.g cond=2.0 x=0");
    assert_eq!(cond(&mut dss, "g"), "2");
}

/// Rule 7: a conductor without a wire, cncable or tscable is one error at
/// calculation, naming it: in the solve (F, and F's geometry on two Lines), in
/// `Show LineConstants`, and for two and for three missing conductors in one
/// message.
#[test]
fn a_conductor_without_data_is_one_error_naming_it() {
    let scratch = Scratch::new("rule7");
    let f = compile("f_conductor_2_skipped.dss");
    assert_eq!(errors(&f), [missing_at_calc("g", 2)]);
    let f2 = compile("f2_two_lines_share_a_skipped_conductor.dss");
    assert_eq!(errors(&f2), [missing_at_calc("g", 2)]);

    let mut dss = compile("f_conductor_2_skipped.dss");
    dss.command(&format!("Set DataPath=\"{}\"", slash(&scratch.dir("show"))));
    dss.command("Show LineConstants freq=60 units=mi rho=100");
    assert_eq!(
        errors(&dss)[1..],
        [
            "Error computing line constants for LineGeometry.g; Error message: LineGeometry.g: \
          conductor 2 has no wire, cncable or tscable."
        ]
    );
    assert_eq!(dss.errors()[1].code, Some(9934));

    let mut two = compile("h2_one_conductor_with_cond.dss");
    two.command(&format!("Set DataPath=\"{}\"", slash(&scratch.dir("two"))));
    two.command("New LineGeometry.two nconds=3 nphases=3");
    two.command("~ cond=1 wire=ACSR_556_5 x=0 h=28");
    two.command("Show LineConstants freq=60 units=mi rho=100");
    assert_eq!(
        errors(&two),
        [
            "Error computing line constants for LineGeometry.two; Error message: LineGeometry.two: \
          conductors 2 and 3 have no wire, cncable or tscable."
        ]
    );

    let mut four = compile("h2_one_conductor_with_cond.dss");
    four.command(&format!("Set DataPath=\"{}\"", slash(&scratch.dir("four"))));
    four.command("New LineGeometry.four nconds=4 nphases=3");
    four.command("~ cond=1 wire=ACSR_556_5 x=0 h=28");
    four.command("Show LineConstants freq=60 units=mi rho=100");
    assert_eq!(
        errors(&four),
        [
            "Error computing line constants for LineGeometry.four; Error message: \
             LineGeometry.four: conductors 2, 3 and 4 have no wire, cncable or tscable."
        ]
    );
}

/// Rule 8: a unit is never read as metres silently. Script C, with no `units=`
/// at all, is in feet. A `units=m` before the first `nconds=` of a new
/// geometry stays the default, so R8's conductors are in metres (the metre
/// numbers of script C, the same reactance), and `units=none` is refused with
/// conductor 1 keeping metres, and so is `units=feet`, a word outside the
/// unit names. A redefinition reads its unit-less numbers in
/// feet (R9). A spacing with no length unit is refused
/// ([`a_spacing_without_a_unit_is_refused`]). The buried neutral that `wires=`
/// fills saves in feet while no `cond=` has selected it, read by a replay up
/// to the `Edit` that places it, and follows the default to metres when a
/// `units=m` after `cond=1` makes metres the default (rule 4).
#[test]
fn a_unit_is_never_read_as_metres() {
    let scratch = Scratch::new("rule8");
    let r9 = compile("r9_redefinition_starts_from_feet.dss");
    for g in ["g", "e", "f"] {
        assert_eq!(units(&r9, g), ["ft"; 3], "R9 {g}");
    }
    let c0 = compile("c0_cond_in_order.dss");
    assert_eq!(units(&c0, "g"), ["ft"; 4]);

    let mut r8 = compile("r8_units_none_and_units_before_nconds.dss");
    assert_eq!(
        errors(&r8),
        [
            "LineGeometry.g.Units: \"none\" is not one of the length units mi, kft, km, m, ft, \
             in, cm, mm."
        ]
    );
    assert_eq!(units(&r8, "g"), ["m"; 3]);
    assert_x(
        &r8,
        "L",
        &[
            phase(-1.2192, 8.5344, "m"),
            phase(0.0, 8.5344, "m"),
            phase(1.2192, 8.5344, "m"),
        ],
        &[(2, 1, 0.794_511_1)],
        "R8",
    );
    r8.command("Edit LineGeometry.g cond=1 units=feet");
    assert_eq!(
        errors(&r8)[1..],
        [
            "LineGeometry.g.Units: \"feet\" is not one of the length units mi, kft, km, m, ft, \
             in, cm, mm."
        ]
    );
    assert_eq!(units(&r8, "g"), ["m"; 3]);

    let arrays = compile("arrays_keep_the_selection.dss");
    assert_eq!(units(&arrays, "buried"), ["ft"; 3]);
    let mut unplaced = replay_until("arrays_keep_the_selection.dss", "Edit LineGeometry.buried");
    let (rows, restore) = saved_rows(&save(&mut unplaced, &scratch.dir("save"), "buried"));
    assert_eq!(rows[2], ["3", "wire=acsr_4/0", "x=0", "h=0", "units=ft"]);
    assert_eq!(restore.as_deref(), Some("2"));
    unplaced.command("Edit LineGeometry.buried cond=1 units=m");
    assert_eq!(units(&unplaced, "buried"), ["m", "ft", "m"]);
    let (rows, restore) = saved_rows(&save(&mut unplaced, &scratch.dir("metres"), "buried"));
    assert_eq!(rows[2], ["3", "wire=acsr_4/0", "x=0", "h=0", "units=m"]);
    assert_eq!(restore.as_deref(), Some("1"));
}

/// A CIM export places each conductor of script A in its own unit: conductor 1
/// in feet, conductors 2 and 3 in metres.
#[test]
fn cim_places_each_conductor_in_its_own_unit() {
    let scratch = Scratch::new("cim");
    let mut a = compile("a_units_on_line_2_only.dss");
    a.command(&format!("Set DataPath=\"{}\"", slash(&scratch.dir("cim"))));
    a.command("Export CIM100");
    assert!(a.errors().is_empty(), "{:?}", errors(&a));
    let xml = read(&scratch.0.join("cim"), "lgrules_CIM100x.xml");
    let position = |i: usize| -> (f64, f64) {
        let block = xml
            .split("<cim:WirePosition ")
            .find(|b| b.contains(&format!(">WP_g_{i}<")))
            .unwrap_or_else(|| panic!("no WirePosition WP_g_{i}"));
        let read = |tag: &str| -> f64 {
            let open = format!("<cim:WirePosition.{tag}>");
            let at = block.find(&open).expect(tag) + open.len();
            block[at..]
                .split('<')
                .next()
                .unwrap()
                .trim()
                .parse()
                .expect(tag)
        };
        (read("xCoord"), read("yCoord"))
    };
    let close =
        |(x, y): (f64, f64), (wx, wy): (f64, f64)| (x - wx).abs() < 1e-9 && (y - wy).abs() < 1e-9;
    assert!(
        close(position(1), (-4.0 * 0.3048, 28.0 * 0.3048)),
        "{:?}",
        position(1)
    );
    assert!(close(position(2), (0.0, 28.0)), "{:?}", position(2));
    assert!(close(position(3), (4.0, 28.0)), "{:?}", position(3));
}

// ----- the selection under the other forms ----------------------------------------

/// `spacing=`, `wires=`, the buried-neutral `wires=` and `conductors=` leave
/// the selection as it was: none after `New` (so the `x=` after them is
/// refused), conductor 2 after `cond=2` (`buried`, and `gc`, whose `x=-2`
/// moves conductor 2, not the last one `conductors=` writes). A replay
/// up to each `Edit` line shows the buried neutral and `gn` as those forms
/// leave them, unplaced. Once placed, every geometry solves on its own Line:
/// `g` and `gn` as script C, `gc` with conductor 2 at (-2, 28) ft against the
/// simple Carson formula, and `buried` against [`cable_z`]: CNData `250_1/3`
/// at x = -0.5 and 0.6 ft, four feet deep, the bare ACSR_4/0 neutral at
/// (1.5, -4) ft, the concentric neutrals reduced and the bare neutral kept.
#[test]
fn wires_spacing_and_conductors_leave_the_selection_alone() {
    let name = "arrays_keep_the_selection.dss";
    let unplaced = replay_until(name, "Edit LineGeometry.buried");
    let buried = conductors(&unplaced, "buried");
    assert_eq!((buried[1].x, buried[1].h), (0.6, -4.0));
    assert_eq!(buried[2].object.as_deref(), Some("acsr_4/0"));
    assert_eq!((buried[2].x, buried[2].h), (0.0, 0.0));
    let unplaced = replay_until(name, "Edit LineGeometry.gn");
    assert!(
        conductors(&unplaced, "gn")
            .iter()
            .all(|c| c.x == 0.0 && c.h == 0.0 && c.object.is_some())
    );

    let dss = compile(name);
    assert_eq!(errors(&dss), [no_cond("g", "X"), no_cond("gn", "X")]);
    assert!(solved(&dss));
    let g = conductors(&dss, "g");
    let xs: Vec<f64> = g.iter().map(|c| c.x).collect();
    assert_eq!(xs, [-4.0, 0.0, 4.0, 0.0]);
    for geom in ["g", "gc", "gn"] {
        assert_eq!(units(&dss, geom), ["ft"; 4], "{geom}");
    }
    assert_x(
        &dss,
        "L",
        &c0_in(["ft"; 4]),
        &[(2, 1, 0.794_511_1)],
        "arrays g",
    );
    assert_x(
        &dss,
        "Ln",
        &c0_in(["ft"; 4]),
        &[(2, 1, 0.794_511_1)],
        "arrays gn",
    );
    let gc: Vec<f64> = conductors(&dss, "gc").iter().map(|c| c.x).collect();
    assert_eq!(gc, [-4.0, -2.0, 4.0, 0.0]);
    let mut gc_placed = c0_in(["ft"; 4]);
    gc_placed[1].x = -2.0;
    assert_x(
        &dss,
        "Lc",
        &gc_placed,
        &[
            (2, 1, 0.878_618_8),
            (3, 2, 0.745_311_3),
            (4, 2, 0.780_972_8),
            (4, 4, 1.546_497_0),
        ],
        "arrays gc",
    );

    let buried = conductors(&dss, "buried");
    assert_eq!(units(&dss, "buried"), ["ft"; 3]);
    assert_eq!((buried[1].x, buried[2].x, buried[2].h), (0.6, 1.5, -4.0));
    let cables = [
        laid(Holds::Cn, -0.5, -4.0, "ft"),
        laid(Holds::Cn, 0.6, -4.0, "ft"),
        laid(Holds::Bare, 1.5, -4.0, "ft"),
    ];
    assert_cable(
        &dss,
        "Lb",
        (&cables, false),
        X_BAND,
        &[
            (1, 1, 0.909_463_9, 0.506_545_2),
            (2, 1, 0.387_798_0, 0.041_435_0),
            (3, 1, 0.350_779_8, 0.123_658_1),
            (3, 2, 0.380_058_7, 0.210_543_8),
            (3, 3, 0.910_097_4, 0.924_260_9),
        ],
        "arrays buried",
    );
}

/// `Dump` walks the conductors and puts the selection back: after `cond=2`, a
/// dump and an `Edit … x=7` move conductor 2. The dump prints every conductor
/// and a tail that reads the last one.
#[test]
fn dump_leaves_the_selection_where_it_was() {
    let scratch = Scratch::new("dump");
    let mut dss = compile("c0_cond_in_order.dss");
    dss.command("Edit LineGeometry.g cond=2");
    dss.command(&format!("Set DataPath=\"{}\"", slash(&scratch.dir("dump"))));
    dss.command("Dump LineGeometry.g");
    dss.command("Edit LineGeometry.g x=7");
    assert!(dss.errors().is_empty(), "{:?}", errors(&dss));
    assert_eq!(cond(&mut dss, "g"), "2");
    let xs: Vec<f64> = conductors(&dss, "g").iter().map(|c| c.x).collect();
    assert_eq!(xs, [-4.0, 7.0, 4.0, 0.0]);
    let text = read(&scratch.0.join("dump"), "lgrules_PropertyDump.txt");
    let rows: Vec<&str> = text.lines().filter(|l| l.starts_with("~ ")).collect();
    let want = [
        "~ NConds=4",
        "~ NPhases=3",
        "~ Cond=1",
        "~ Wire=acsr_556_5",
        "~ X=-4",
        "~ H=28",
        "~ Units=ft",
        "~ Cond=2",
        "~ Wire=acsr_556_5",
        "~ X=0",
        "~ H=28",
        "~ Units=ft",
        "~ Cond=3",
        "~ Wire=acsr_556_5",
        "~ X=4",
        "~ H=28",
        "~ Units=ft",
        "~ Cond=4",
        "~ Wire=acsr_4/0",
        "~ X=0",
        "~ H=24",
        "~ Units=ft",
    ];
    assert_eq!(rows[..want.len()], want);
    assert!(rows.contains(&"~ CNCable=acsr_4/0"), "{rows:?}");
    assert!(rows.contains(&"~ TSCable=acsr_4/0"), "{rows:?}");
}

/// A YPrim that cannot be built is the one error of each solve, for a Line on
/// a geometry and for a Line with its own spacing and wires (the message names
/// the Line and its spacing): two conductors in one place give one wrapped
/// message per solve, and no singular-matrix error follows.
#[test]
fn a_y_build_abort_is_the_only_error_of_the_solve() {
    let on_geometry = [
        "New LineGeometry.bad nconds=3 nphases=3 cond=1 wire=w x=0 h=10 units=m cond=2 wire=w \
         x=0 h=10 cond=3 wire=w x=2 h=10",
        "New Line.l1 bus1=sourcebus bus2=b2 phases=3 geometry=bad length=1 units=km",
    ];
    let on_spacing = [
        "New LineSpacing.bad nconds=3 nphases=3 x=[0 0 2] h=[10 10 10] units=m",
        "New Line.l1 bus1=sourcebus bus2=b2 phases=3 spacing=bad wires=[w w w] length=1 \
         units=km",
    ];
    for (lines, same_place) in [
        (
            on_geometry,
            "Error in LineGeometry.bad: Conductors 1 and 2 occupy the same space.",
        ),
        (
            on_spacing,
            "Error in Line.l1 (spacing=bad): Conductors 1 and 2 occupy the same space.",
        ),
    ] {
        let mut dss = Dss::new();
        dss.command("New circuit.geo basekv=12.47 phases=3");
        dss.command(
            "New WireData.w runits=m gmrunits=m radunits=m rac=0.0003 gmrac=0.005 radius=0.01 \
             normamps=400",
        );
        for line in lines {
            dss.command(line);
        }
        dss.command("New Load.ld bus1=b2 phases=3 kv=12.47 kw=300 pf=0.95 model=1");
        dss.command("Set voltagebases=[12.47]");
        dss.command("CalcVoltageBases");
        assert_eq!(
            errors(&dss),
            [format!(
                "Error Encountered in CalcVoltageBases: {same_place}"
            )]
        );
        assert!(dss.circuit().unwrap().solution.solution_abort);
        dss.command("Solve");
        assert_eq!(
            errors(&dss)[1..],
            [format!("Error Encountered in Solve: {same_place}")]
        );
        assert_eq!(dss.errors()[1].code, Some(482));
        assert!(dss.circuit().unwrap().solution.solution_abort);
    }
}

/// A Line whose impedance cannot be built beside a Capacitor that refuses its
/// stamp: the build drops its Y, as for the refusal alone, the Capacitor's
/// message is logged, and the build's error names the Line.
#[test]
fn a_broken_geometry_beside_a_refused_stamp_names_both() {
    let mut dss = Dss::new();
    dss.command("New circuit.geo basekv=12.47 phases=3");
    dss.command(
        "New WireData.w runits=m gmrunits=m radunits=m rac=0.0003 gmrac=0.005 radius=0.01 \
         normamps=400",
    );
    dss.command(
        "New LineGeometry.bad nconds=3 nphases=3 cond=1 wire=w x=0 h=10 units=m cond=2 wire=w \
         x=0 h=10 cond=3 wire=w x=2 h=10",
    );
    dss.command("New Line.l1 bus1=sourcebus bus2=b2 phases=3 geometry=bad length=1 units=km");
    dss.command("New Capacitor.cd3 bus1=b2 phases=3 conn=delta cmatrix=[10 | -2 10 | -2 -2 10]");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    assert_eq!(
        errors(&dss),
        [
            "Capacitor.cd3: a cmatrix bank of 3 phases cannot be connected in delta. Specify it \
             with conn=wye. Aborting solution.",
            "Error Encountered in CalcVoltageBases: Error in LineGeometry.bad: Conductors 1 and 2 \
             occupy the same space.",
        ]
    );
    let solution = &dss.circuit().unwrap().solution;
    assert!(solution.solution_abort);
    assert!(solution.y_system.is_none(), "the refused build left a Y");
}

/// A Y rebuild outside the solve logs the build's one error itself:
/// [`Dss::rebuild_system_y`] on script F adds rule 7's message for conductor 2
/// once, as the build returns it.
#[test]
fn a_y_rebuild_outside_the_solve_logs_its_error() {
    let mut dss = compile("f_conductor_2_skipped.dss");
    let before = dss.errors().len();
    dss.rebuild_system_y();
    assert_eq!(
        errors(&dss)[before..],
        ["LineGeometry.g: conductor 2 has no wire, cncable or tscable."]
    );
}

/// The two A-Diakoptics Y rebuilds log the build's one error the same way: the
/// coordinator's [`Dss::ad_build_y`] and a child's [`Dss::rebuild_child_y`] on
/// script F each add rule 7's message for conductor 2 once.
#[test]
fn a_diakoptics_y_rebuild_logs_its_error() {
    let logged = [Dss::ad_build_y as fn(&mut Dss), Dss::rebuild_child_y].map(|rebuild| {
        let mut dss = compile("f_conductor_2_skipped.dss");
        let before = dss.errors().len();
        rebuild(&mut dss);
        errors(&dss).split_off(before)
    });
    let rule_7 = ["LineGeometry.g: conductor 2 has no wire, cncable or tscable."];
    assert_eq!(logged, [rule_7, rule_7]);
}

/// R9: a redefinition with `nconds=` reads its unit-less numbers in feet,
/// whatever unit the definition it replaces was written in: a second `New`
/// (`g`), an `Edit` (`e`), and a second `New` with `units=m` before its
/// `nconds=` (`f`, where a new geometry would keep the metres, R8). With no
/// conductor selected after a re-allocation `Units` reads ft.
#[test]
fn variant_a_redefinition_starts_from_feet() {
    let mut dss = compile("r9_redefinition_starts_from_feet.dss");
    assert!(dss.errors().is_empty(), "{:?}", errors(&dss));
    assert!(solved(&dss));
    let ft = [
        phase(-4.0, 28.0, "ft"),
        phase(0.0, 28.0, "ft"),
        phase(4.0, 28.0, "ft"),
    ];
    for (g, l) in [("g", "Lg"), ("e", "Le"), ("f", "Lf")] {
        assert_eq!(units(&dss, g), ["ft"; 3], "{g}");
        assert_x(&dss, l, &ft, &[(2, 1, 0.794_511_1)], g);
    }
    dss.command("Edit LineGeometry.e units=m");
    assert_eq!(query(&mut dss, "LineGeometry.e.Units"), "m");
    dss.command("Edit LineGeometry.e nconds=3");
    assert_eq!(query(&mut dss, "LineGeometry.e.Units"), "ft");
    dss.command("New LineGeometry.f units=m nconds=3");
    assert_eq!(query(&mut dss, "LineGeometry.f.Units"), "ft");
    assert!(dss.errors().is_empty(), "{:?}", errors(&dss));
}

/// R10: a redefinition with `nconds=` keeps nothing of the equivalent spacing
/// of the definition it replaces (spacing `eq` in metres: phases 1.2 m apart
/// and 1.5 m from the neutral). `gs`, given `spacing=eq` again, reads the
/// distances in metres like `ga`: X21 0.7964372 Ω/mi, where feet would give
/// 0.9406. `gd`, given positions of its own, reads those: X31 0.7123296 Ω/mi
/// for its 2.4 m, where the replaced spacing gives 0.7964372. `gr`, given
/// neither, has no conductor heights, and its Line is the one error of the
/// solve. On `gs` that Line solves to `ga`'s voltages.
#[test]
fn a_redefinition_drops_the_equivalent_spacing() {
    let mut dss = compile("r10_redefinition_drops_the_equivalent_spacing.dss");
    let gr = "Error in LineGeometry.gr: Conductor 1 height must be  > 0. ";
    assert_eq!(
        errors(&dss),
        [format!("Error Encountered in CalcVoltageBases: {gr}")]
    );
    assert!(!solved(&dss));
    dss.command("Solve");
    assert_eq!(
        errors(&dss)[1..],
        [format!("Error Encountered in Solve: {gr}")]
    );
    assert!(!solved(&dss));
    dss.command("Edit Line.Lr geometry=gs");
    dss.command("Solve");
    assert_eq!(dss.errors().len(), 2, "{:?}", errors(&dss));
    assert!(solved(&dss));
    let eq = carson_x_equivalent(&[GMR_P, GMR_P, GMR_P, GMR_N], 3, 1.2, 1.5);
    for l in ["La", "Ls", "Lr"] {
        assert_x_is(&dss, l, &eq, &[(2, 1, 0.796_437_2), (4, 1, 0.769_360_6)], l);
    }
    let own = [
        phase(-1.2, 8.5, "m"),
        phase(0.0, 8.5, "m"),
        phase(1.2, 8.5, "m"),
        neutral(0.0, 7.3, "m"),
    ];
    assert_eq!(units(&dss, "gd"), ["m"; 4]);
    assert_x(
        &dss,
        "Ld",
        &own,
        &[
            (2, 1, 0.796_437_2),
            (3, 1, 0.712_329_6),
            (4, 1, 0.754_383_4),
        ],
        "gd",
    );
    let ckt = dss.circuit().expect("the circuit");
    let v = |bus: &str| {
        let b = ckt
            .buses
            .iter()
            .find(|b| b.name.eq_ignore_ascii_case(bus))
            .expect("the bus");
        b.ref_no[..3]
            .iter()
            .map(|&n| ckt.solution.node_v[n])
            .collect::<Vec<_>>()
    };
    let ba = v("ba");
    for bus in ["bs", "br"] {
        for (a, b) in ba.iter().zip(v(bus)) {
            assert!(
                (a - b).norm() <= 1e-9 * a.norm(),
                "{bus}: {b} against ba {a}"
            );
        }
    }
}

/// A refused `nconds=` (0 or negative) leaves the geometry as it was: its
/// conductors, their units and the selection.
#[test]
fn a_refused_nconds_leaves_the_geometry_as_it_was() {
    let mut dss = compile("c0_cond_in_order.dss");
    dss.command("Edit LineGeometry.g cond=2");
    dss.command("Edit LineGeometry.g nconds=0");
    dss.command("Edit LineGeometry.g nconds=-1");
    assert_eq!(
        errors(&dss),
        [
            "LineGeometry.g.NConds: Value (0) cannot be zero.",
            "LineGeometry.g.NConds: Value (-1) cannot be negative."
        ]
    );
    assert_eq!(cond(&mut dss, "g"), "2");
    dss.command("Edit Line.L geometry=g");
    dss.command("Solve");
    assert_eq!(dss.errors().len(), 2, "{:?}", errors(&dss));
    assert_eq!(units(&dss, "g"), ["ft"; 4]);
    assert_x(
        &dss,
        "L",
        &c0_in(["ft"; 4]),
        &[(2, 1, 0.794_511_1)],
        "refused nconds",
    );
}

/// A geometry whose only `nconds=` was refused has no conductors, so Save
/// writes no `NConds=` for it and the JSON export leaves `NConds` out: its
/// reload and its import log nothing. A geometry with conductors saves the
/// count it kept after a refused `nconds=0`.
#[test]
fn a_refused_nconds_is_not_saved() {
    use crate::report::export::json::JsonOpts;
    let scratch = Scratch::new("refused_nconds");
    let mut dss = compile("c0_cond_in_order.dss");
    dss.command("New LineGeometry.z nconds=0");
    dss.command("Edit LineGeometry.g nconds=0");
    assert_eq!(
        errors(&dss),
        [
            "LineGeometry.z.NConds: Value (0) cannot be zero.",
            "LineGeometry.g.NConds: Value (0) cannot be zero."
        ]
    );
    let dir = scratch.dir("save");
    assert_eq!(save(&mut dss, &dir, "z"), "new \"linegeometry.z\"");
    let saved = read(&dir, "LineGeometry.dss").to_ascii_lowercase();
    assert!(
        saved.contains("new \"linegeometry.g\" nconds=4 "),
        "{saved}"
    );
    let mut back = reload(&dir);
    assert_eq!(query(&mut back, "LineGeometry.z.NConds"), "0");
    assert_eq!(conductors(&back, "g"), conductors(&dss, "g"));
    for opts in [JsonOpts::NONE, JsonOpts::FULL] {
        let record = dss.obj_to_json("LineGeometry.z", opts).expect("z");
        assert!(!record.contains("\"NConds\""), "{opts:?}: {record}");
    }
    let record = dss
        .obj_to_json("LineGeometry.z", JsonOpts::NONE)
        .expect("z");
    let json = format!(r#"{{"Name":"lgjson","LineGeometry":[{record}]}}"#);
    let mut imported = Dss::new();
    assert_eq!(imported.circuit_from_json(&json), Ok(()));
    assert!(
        imported.errors().is_empty(),
        "{record} imports with {:?}",
        errors(&imported)
    );
}

/// S7: a `spacing=` whose LineSpacing has no length unit (`units=none`, or a
/// word that names none) is refused with one error naming the spacing, and
/// nothing of it is copied or kept. The spacing in feet is copied.
#[test]
fn a_spacing_without_a_unit_is_refused() {
    let scratch = Scratch::new("s7");
    let mut dss = compile("s7_spacing_without_a_unit.dss");
    let refused = |g: &str, sp: &str| {
        format!(
            "LineGeometry.{g}.Spacing: LineSpacing.{sp} has no length unit, so nothing is \
             copied. Give the spacing one of mi, kft, km, m, ft, in, cm, mm."
        )
    };
    assert_eq!(
        errors(&dss),
        [refused("gnone", "sp_none"), refused("gfeet", "sp_feet")]
    );
    assert!(solved(&dss));
    for g in ["gnone", "gfeet"] {
        assert!(
            conductors(&dss, g)
                .iter()
                .all(|c| c.x == 0.0 && c.h == 0.0 && c.object.is_some()),
            "{g}"
        );
        assert_eq!(query(&mut dss, &format!("LineGeometry.{g}.Spacing")), "");
        let (rows, _) = saved_rows(&save(&mut dss, &scratch.dir(g), g));
        assert!(rows.iter().all(|r| r[4] == "units=ft"), "{g}: {rows:?}");
    }
    assert_eq!(units(&dss, "gft"), ["ft"; 3]);
    assert_x(
        &dss,
        "L",
        &[
            phase(-4.0, 28.0, "ft"),
            phase(0.0, 28.0, "ft"),
            phase(4.0, 28.0, "ft"),
        ],
        &[(2, 1, 0.794_511_1)],
        "S7",
    );
}

/// A Line's own `spacing=` reads its LineSpacing's unit the way a geometry's
/// `spacing=` does. A spacing in feet, metres or inches is converted (`Lft` and
/// `Lin` X21 0.7945111 Ω/mi, `Lm` 0.6503451). A spacing with no length unit
/// (`units=none`, and `units=feet`, which the LineSpacing reads as none) is one
/// error naming the Line, the spacing and the unit, and the Line stays as it
/// was: `Ln` and `Lfe` are default Lines like `Ldef`, whose `wires=` then has no
/// spacing to fill, and `Lkeep` keeps its spacing in feet. Both oracles read
/// those spacings as metres, X21 0.6503451 on r4133.
#[test]
fn a_line_refuses_a_spacing_without_a_unit() {
    let mut dss = compile("s8_line_spacing_units.dss");
    let refused = |line: &str, sp: &str| {
        format!(
            "Line.{line}.Spacing: LineSpacing.{sp} has no length unit (units=none), so the Line \
             is left as it was. Give the spacing one of mi, kft, km, m, ft, in, cm, mm."
        )
    };
    let no_spacing = |line: &str| {
        format!("You must assign the LineSpacing before the Wires Property (\"Line.{line}\").")
    };
    assert_eq!(
        errors(&dss),
        [
            refused("ln", "sp_none"),
            no_spacing("ln"),
            refused("lfe", "sp_feet"),
            no_spacing("lfe"),
            refused("lkeep", "sp_feet"),
        ]
    );
    assert!(solved(&dss));
    let row = |u| {
        [
            phase(-4.0, 28.0, u),
            phase(0.0, 28.0, u),
            phase(4.0, 28.0, u),
        ]
    };
    assert_x(&dss, "Lft", &row("ft"), &[(2, 1, 0.794_511_1)], "S8 ft");
    assert_x(&dss, "Lm", &row("m"), &[(2, 1, 0.650_345_1)], "S8 m");
    let inches = [
        phase(-48.0, 336.0, "in"),
        phase(0.0, 336.0, "in"),
        phase(48.0, 336.0, "in"),
    ];
    assert_x(&dss, "Lin", &inches, &[(2, 1, 0.794_511_1)], "S8 in");
    assert_x(&dss, "Lkeep", &row("ft"), &[(2, 1, 0.794_511_1)], "S8 kept");
    assert_eq!(query(&mut dss, "Line.Lkeep.Spacing"), "sp_ft");
    for line in ["Ln", "Lfe"] {
        assert_eq!(
            query(&mut dss, &format!("Line.{line}.Spacing")),
            "",
            "{line}"
        );
        assert_eq!(xmatrix(&dss, line), xmatrix(&dss, "Ldef"), "{line}");
    }
}

/// A refusal is logged at its token, in token order, underlining the value it
/// refuses.
#[test]
fn refusals_are_logged_at_their_token() {
    let mut dss = compile("c0_cond_in_order.dss");
    dss.command("New LineGeometry.t nconds=4 nphases=3");
    dss.command("~ x=9 cond=11 h=3");
    assert_eq!(
        errors(&dss),
        [
            no_cond("t", "X"),
            out_of_range("t", 11, "has 4 conductors"),
            no_cond("t", "H")
        ]
    );
    let spans: Vec<Option<(usize, usize)>> = dss
        .errors()
        .iter()
        .map(|d| d.span.as_ref().map(|s| (s.offset(), s.len())))
        .collect();
    assert_eq!(spans, [Some((4, 1)), Some((11, 2)), Some((16, 1))]);
}

/// A JSON record of an existing LineGeometry edits it with no conductor
/// selected, as a second `New` does: conductor data without `Cond` is refused
/// and no conductor moves.
#[test]
fn json_record_of_an_existing_geometry_selects_nothing() {
    let mut dss = Dss::new();
    let json = r#"{"Name":"lgjson","PreCommands":[
        "New WireData.w Rdc=0.1 Runits=km GMRac=0.01 GMRunits=m radius=0.02 radunits=m normamps=400",
        "New LineGeometry.g nconds=4 nphases=3 cond=1 wire=w x=-4 h=28 cond=2 wire=w x=0 h=28 cond=3 wire=w x=4 h=28 cond=4 wire=w x=0 h=24"],
        "LineGeometry":[{"Name":"g","X":5}]}"#;
    assert_eq!(dss.circuit_from_json(json), Ok(()));
    assert_eq!(errors(&dss), [no_cond("g", "X")]);
    let xs: Vec<f64> = conductors(&dss, "g").iter().map(|c| c.x).collect();
    assert_eq!(xs, [-4.0, 0.0, 4.0, 0.0]);
}

/// The JSON export of a geometry with no conductor selected (after a second
/// `New`, after `nconds=`, or a `like=` copy) writes none of the selected
/// conductor's values, in the default and the full form: `Cond` 0, the
/// missing conductor object and the zero `X` and `H` would each be refused by
/// the import (rules 6 and 2). The default record, and V5's whole circuit,
/// import with no error. With a conductor selected the record still carries
/// it and its values.
#[test]
fn json_export_with_no_selection_writes_no_conductor_values() {
    use crate::report::export::json::JsonOpts;
    let pre = [
        "New WireData.w Rdc=0.1 Runits=km GMRac=0.01 GMRunits=m radius=0.02 radunits=m normamps=400",
        "New LineGeometry.g nconds=4 nphases=3 cond=1 wire=w x=-4 h=28 cond=2 wire=w x=0 h=28 \
         cond=3 wire=w x=4 h=28 cond=4 wire=w x=0 h=24",
    ];
    let source = |tail: Option<&str>| {
        let mut dss = Dss::new();
        dss.command("new circuit.lgjson");
        for c in pre.iter().copied().chain(tail) {
            dss.command(c);
        }
        assert!(dss.errors().is_empty(), "{tail:?}: {:?}", errors(&dss));
        dss
    };
    let record = source(None)
        .obj_to_json("LineGeometry.g", JsonOpts::NONE)
        .expect("g");
    for member in ["\"Cond\":4", "\"Wire\":\"w\"", "\"X\":", "\"H\":"] {
        assert!(record.contains(member), "{member} not in {record}");
    }
    for tail in ["New LineGeometry.g", "Edit LineGeometry.g nconds=4"] {
        let mut src = source(Some(tail));
        assert_eq!(cond(&mut src, "g"), "0", "{tail}");
        for opts in [JsonOpts::NONE, JsonOpts::FULL] {
            let record = src.obj_to_json("LineGeometry.g", opts).expect("g");
            for key in ["Cond", "Wire", "X", "H", "CNCable", "TSCable"] {
                assert!(
                    !record.contains(&format!("\"{key}\"")),
                    "{tail} {opts:?}: {key} in {record}"
                );
            }
        }
        let record = src
            .obj_to_json("LineGeometry.g", JsonOpts::NONE)
            .expect("g");
        let json = format!(
            r#"{{"Name":"lgjson","PreCommands":["{}"],"LineGeometry":[{record}]}}"#,
            pre[0]
        );
        let mut back = Dss::new();
        assert_eq!(back.circuit_from_json(&json), Ok(()));
        assert!(
            back.errors().is_empty(),
            "{tail}: {record} imports with {:?}",
            errors(&back)
        );
        assert_eq!(cond(&mut back, "g"), "0", "{tail}");
        assert_eq!(conductors(&back, "g").len(), 4, "{tail}");
    }
    // A `like=` copy selects none either (V5): its record writes no
    // conductor value, and the circuit's export imports with no error.
    let mut v5 = compile("v5_like_then_data_without_cond.dss");
    assert_eq!(cond(&mut v5, "g"), "0");
    let record = v5.obj_to_json("LineGeometry.g", JsonOpts::NONE).expect("g");
    for key in ["Cond", "Wire", "X", "H", "CNCable", "TSCable"] {
        assert!(
            !record.contains(&format!("\"{key}\"")),
            "V5: {key} in {record}"
        );
    }
    let circuit = v5.circuit_to_json(JsonOpts::NONE).expect("the circuit");
    let mut back = Dss::new();
    assert_eq!(back.circuit_from_json(&circuit), Ok(()));
    assert!(
        back.errors().is_empty(),
        "V5 imports with {:?}",
        errors(&back)
    );
    assert_eq!(cond(&mut back, "g"), "0");
    assert_eq!(cond(&mut back, "base"), cond(&mut v5, "base"));
}
/// With no conductor selected `Cond` reads 0, `X` and `H` read 0, the
/// conductor names read empty and `Units` reads the default. The four
/// LineGeometry scenarios of the property readback gate, with every property
/// they read: a new geometry, a `spacing=` + `wires=` geometry, a `like=` copy
/// and a buried neutral after `cond=2`. A new geometry reads `NPhases` 3, the
/// documented default.
#[test]
fn readback_shows_no_conductor_after_new_like_and_arrays() {
    let acsr = "New WireData.acsr Rdc=0.0526 GMRac=0.0244 GMRunits=ft radius=0.0306 \
                radunits=ft normamps=530 Runits=ft";
    let cn1 = "New CNData.cn1 k=16 DiaStrand=0.064 GmrStrand=0.0208 Rstrand=0.0145 EpsR=2.3 \
               InsLayer=0.22 DiaIns=1.06 DiaCable=1.16 Rdc=0.0997 GMRac=0.0375 radius=0.0511 \
               Runits=in radunits=in gmrunits=in normamps=350";
    let three = "[acsr, acsr, acsr]";
    let scenarios = [
        (
            "default",
            vec!["New LineGeometry.g1"],
            vec![
                ("NConds", "0"),
                ("NPhases", "3"),
                ("Cond", "0"),
                ("Wire", ""),
                ("NormAmps", "none"),
                ("EmergAmps", "none"),
                ("Reduce", "No"),
                ("Spacing", ""),
                ("Wires", "[]"),
                ("CNCable", ""),
                ("TSCable", ""),
                ("CNCables", "[]"),
                ("TSCables", "[]"),
                ("Seasons", "1"),
                ("Ratings", "[ none]"),
                ("LineType", "oh"),
                ("Like", ""),
            ],
        ),
        (
            "spacing",
            vec![
                acsr,
                "New LineSpacing.sp nconds=3 nphases=3 x=(-1.2909 0 1.2909) h=(28.6 28.6 28.6) \
                 units=ft",
                "New LineGeometry.g1 nconds=3 nphases=3 spacing=sp wires=[acsr acsr acsr]",
            ],
            vec![
                ("NConds", "3"),
                ("NPhases", "3"),
                ("Cond", "0"),
                ("Wire", ""),
                ("X", "0"),
                ("H", "0"),
                ("Units", "ft"),
                ("NormAmps", "530"),
                ("EmergAmps", "795"),
                ("Reduce", "No"),
                ("Spacing", "sp"),
                ("Wires", three),
                ("CNCable", ""),
                ("TSCable", ""),
                ("CNCables", three),
                ("TSCables", three),
                ("Seasons", "1"),
                ("Ratings", "[ none]"),
                ("LineType", "oh"),
                ("Like", ""),
            ],
        ),
        (
            "makelike",
            vec![
                acsr,
                "New LineGeometry.base nconds=3 nphases=3 cond=1 wire=acsr x=-1.29 h=13.7 \
                 units=m cond=2 wire=acsr x=0 h=13.7 cond=3 wire=acsr x=1.29 h=13.7 reduce=y",
                "New LineGeometry.g1 like=base",
            ],
            vec![
                ("NConds", "3"),
                ("NPhases", "3"),
                ("Cond", "0"),
                ("Wire", ""),
                ("X", "0"),
                ("H", "0"),
                // The source's default unit comes with the copy.
                ("Units", "m"),
                ("NormAmps", "530"),
                ("EmergAmps", "795"),
                ("Reduce", "Yes"),
                ("Spacing", ""),
                ("Wires", three),
                ("CNCable", ""),
                ("TSCable", ""),
                ("CNCables", three),
                ("TSCables", three),
                ("Seasons", "1"),
                ("Ratings", "[ none]"),
                ("LineType", "oh"),
                ("Like", ""),
            ],
        ),
        (
            "buried",
            vec![
                cn1,
                acsr,
                "New LineGeometry.g1 nconds=3 nphases=2 cond=1 cncable=cn1 x=-0.5 h=-4 units=ft \
                 cond=2 cncable=cn1 x=0.5 h=-4 wires=[acsr]",
            ],
            vec![
                ("NConds", "3"),
                ("NPhases", "2"),
                ("Cond", "2"),
                ("Wire", "cn1"),
                ("X", "0.5"),
                ("H", "-4"),
                ("Units", "ft"),
                ("NormAmps", "350"),
                ("EmergAmps", "525"),
                ("Reduce", "No"),
                ("Spacing", ""),
                ("Wires", "[cn1, cn1, acsr]"),
                ("CNCable", "cn1"),
                ("TSCable", "cn1"),
                ("CNCables", "[cn1, cn1, acsr]"),
                ("TSCables", "[cn1, cn1, acsr]"),
                ("Seasons", "1"),
                ("Ratings", "[ none]"),
                ("LineType", "oh"),
                ("Like", ""),
            ],
        ),
    ];
    for (name, commands, props) in scenarios {
        let mut dss = Dss::new();
        dss.command("clear");
        dss.command("new circuit.propsprobe");
        for c in commands {
            dss.command(c);
        }
        assert!(dss.errors().is_empty(), "{name}: {:?}", errors(&dss));
        for (prop, want) in props {
            assert_eq!(
                query(&mut dss, &format!("LineGeometry.g1.{prop}")),
                want,
                "{name}: {prop}"
            );
        }
    }
}

/// A `cond=` before `nconds=` is refused (rule 6), and the spacing and
/// `wires=` after it fill all three conductors, so the conductor table comes
/// ahead of `NConds=` in the order the properties were set. The Save writes
/// `NConds=` first, and the reload gives the same conductors and reactance
/// with no error.
#[test]
fn save_writes_nconds_first() {
    let scratch = Scratch::new("nconds_first");
    let name = "r6b_cond_before_nconds_with_spacing.dss";
    let mut dss = compile(name);
    assert_eq!(
        errors(&dss),
        [out_of_range(
            "g",
            1,
            "has no conductors yet, set NConds first"
        )]
    );
    assert_eq!(dss.errors()[0].code, Some(10102));
    assert!(solved(&dss));
    assert_eq!(units(&dss, "g"), ["ft"; 3]);
    let placed = [
        phase(-4.0, 28.0, "ft"),
        phase(0.0, 28.0, "ft"),
        phase(4.0, 28.0, "ft"),
    ];
    assert_x(&dss, "L", &placed, &[(2, 1, 0.794_511_1)], "R6b");
    let dir = scratch.dir("save");
    let line = save(&mut dss, &dir, "g");
    let body = line
        .strip_prefix("new \"linegeometry.g\"")
        .expect("the New line");
    assert!(body.starts_with(" nconds=3 cond=1 "), "{line}");
    let back = reload(&dir);
    assert_eq!(conductors(&back, "g"), conductors(&dss, "g"));
    assert_x(&back, "L", &placed, &[(2, 1, 0.794_511_1)], "R6b reloaded");
}

/// The Save of every deck that solves recompiles to the same conductors
/// (objects, coordinates, units) and the same reactance, and writes no
/// `units=none` and no ` Cond=0`.
#[test]
fn save_round_trips_every_case() {
    let scratch = Scratch::new("save_all");
    let cases: [(&str, &[&str], &[&str]); 31] = [
        ("a_units_on_line_2_only.dss", &["g"], &["L"]),
        ("b1_units_after_cond.dss", &["g"], &["L"]),
        ("b2_units_before_cond.dss", &["g"], &["L"]),
        ("c_any_cond_order.dss", &["g"], &["L"]),
        ("c0_cond_in_order.dss", &["g"], &["L"]),
        ("d_out_of_order_after_a_default.dss", &["g"], &["L"]),
        ("e_cond_out_of_range.dss", &["g"], &["L"]),
        ("h2_one_conductor_with_cond.dss", &["g"], &["L"]),
        ("i_cond_given_twice.dss", &["g"], &["L"]),
        ("j08_default_then_own_unit.dss", &["g"], &["L"]),
        ("j_reselect_keeps_its_own_unit.dss", &["g"], &["L"]),
        (
            "n1_new_again_selects_nothing.dss",
            &["g", "h"],
            &["L", "Lh"],
        ),
        ("n2_nconds_selects_nothing.dss", &["g", "e"], &["L", "Le"]),
        ("r6_cond_zero_and_cond_before_nconds.dss", &["g"], &["L"]),
        ("r6_cond_not_a_number.dss", &["g"], &["L"]),
        ("r6b_cond_before_nconds_with_spacing.dss", &["g"], &["L"]),
        ("r8_units_none_and_units_before_nconds.dss", &["g"], &["L"]),
        (
            "r9_redefinition_starts_from_feet.dss",
            &["g", "e", "f"],
            &["Lg", "Le", "Lf"],
        ),
        (
            "s7_spacing_without_a_unit.dss",
            &["gft", "gnone", "gfeet"],
            &["L"],
        ),
        ("v1_one_line.dss", &["g"], &["L"]),
        ("v2_one_property_per_line.dss", &["g"], &["L"]),
        ("v3_separate_edit_commands.dss", &["g"], &["L"]),
        ("v3b_b2_as_separate_edit_commands.dss", &["g"], &["L"]),
        ("v4_descending_cond.dss", &["g"], &["L"]),
        ("v5_like_then_data_without_cond.dss", &["g", "base"], &["L"]),
        ("v5b_like_not_found_then_data.dss", &["g"], &["L"]),
        (
            "v5c_like_takes_the_source_unit.dss",
            &["gm", "gf", "g"],
            &["Lm", "Lf", "Lg"],
        ),
        ("v6_units_between_two_data.dss", &["g"], &["L"]),
        (
            "v7_cable_geometries.dss",
            &["cn", "ts", "ts_m", "mixed", "mixed_m"],
            &["cn", "ts", "ts_m", "mixed", "mixed_m"],
        ),
        (
            "v8_plural_cables_then_neutral_wires.dss",
            &["cn", "cn_twin", "cw", "ts", "ts_twin"],
            &["cn", "cn_twin", "cw", "ts", "ts_twin"],
        ),
        (
            "arrays_keep_the_selection.dss",
            &["g", "buried", "gc", "gn"],
            &["L", "Lb", "Lc", "Ln"],
        ),
    ];
    for (i, (name, geoms, lines)) in cases.into_iter().enumerate() {
        let mut dss = compile(name);
        assert!(solved(&dss), "{name}");
        let dir = scratch.dir(&i.to_string());
        save(&mut dss, &dir, geoms[0]);
        let back = reload(&dir);
        for geom in geoms {
            assert_eq!(
                conductors(&back, geom),
                conductors(&dss, geom),
                "{name}: {geom}"
            );
        }
        for line in lines {
            for real in [true, false] {
                let (a, b) = (zpart(&dss, line, real), zpart(&back, line, real));
                assert_eq!(a.len(), b.len(), "{name}: {line}");
                for (ra, rb) in a.iter().zip(&b) {
                    for (va, vb) in ra.iter().zip(rb) {
                        assert!(
                            (va - vb).abs() <= 1e-12 * va.abs().max(1e-9),
                            "{name}: Line.{line} {va} vs {vb}"
                        );
                    }
                }
            }
        }
    }
}

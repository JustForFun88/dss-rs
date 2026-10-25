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
//! one out of range (rule 6): an error that leaves no conductor selected. A
//! property before the first `nconds=` of a new geometry is refused, `units=`
//! takes one exact unit word, and a `wires=` list takes its count from the
//! data, never from the selection. A conductor without an `x` or an `h` is
//! rule 7's error too, unless the geometry is built on an equivalent spacing,
//! whose distances keep the unit of their spacing. A LineSpacing takes no
//! property before its first `nconds=` either, and each `nconds=` starts it
//! over in feet with no position.
//!
//! Every script and variant is a tracked deck under
//! `crates/dss-core/tests/data/line_geometry_rules/` (`library.dss` holds the
//! conductor data). Some rule pins and the readback, JSON and Y-build pins
//! build a small geometry inline. A pin compiles its deck, reads the
//! geometry's conductors from the class arena and the Line's impedance, and
//! compares the impedance with physics evaluated at the positions the rules
//! give: the simple Carson reactance of bare wires ([`carson_x`]), the
//! textbook cable model ([`cable_z`]) and the shunt capacitance of one
//! conductor ([`shunt_c11`]) or several ([`shunt_c`]). These formulas take two
//! models of the engine, the earth-return depth of its simple Carson model
//! ([`carson_x`]) and its tape-shield resistance with the TSData lap
//! correction ([`cable_z`]), so the pins hold the unit placement under those
//! models. A deck that cannot be
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

/// One conductor of a geometry as the arena holds it, `None` for a value never
/// given.
#[derive(Debug, Clone, PartialEq)]
struct Conductor {
    object: Option<String>,
    x: Option<f64>,
    h: Option<f64>,
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

pub(super) fn xmatrix(dss: &Dss, line: &str) -> Vec<Vec<f64>> {
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

/// A property written before the first `nconds=` of a new geometry.
fn before_nconds(geom: &str, prop: &str) -> String {
    format!(
        "LineGeometry.{geom}.{prop}: the geometry has no conductors yet, so the value is not \
         applied. Give nconds= first."
    )
}

/// A LineSpacing property written before its first `nconds=`.
fn spacing_before_nconds(spacing: &str, prop: &str) -> String {
    format!(
        "LineSpacing.{spacing}.{prop}: nconds= is not given yet, so the value is not applied. \
         Give nconds= first."
    )
}

/// Rule 6: `cond=value` out of range. `has` is "has 4 conductors" or "has 1
/// conductor".
fn out_of_range(geom: &str, value: i32, has: &str) -> String {
    format!(
        "LineGeometry.{geom}.Cond: cond={value} is out of range, the geometry {has}. No \
         conductor is selected."
    )
}

/// What rule 7 names for a conductor that holds nothing.
const NOTHING: &str = "no wire, cncable or tscable, no x and no h";

/// Rule 7 for one conductor that holds nothing, as the Y build reports it from
/// `CalcVoltageBases`.
fn missing_at_calc(geom: &str, conductor: usize) -> String {
    format!(
        "Error Encountered in CalcVoltageBases: LineGeometry.{geom}: conductor {conductor} has \
         {NOTHING}."
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
pub(super) fn carson_x_of(n: usize, d: impl Fn(usize, usize) -> f64) -> Vec<Vec<f64>> {
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
pub(super) const X_BAND: f64 = 1e-6;

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

/// The shunt capacitance of overhead conductors of radius `r` at the
/// positions `(x, h)` (metres, `h` over the earth's surface), in nF per mile.
/// The Maxwell potential coefficients `P_ii = ln(2 h_i / r) / (2π ε0)` and
/// `P_ij = ln(D'_ij / D_ij) / (2π ε0)`, with `D_ij` the distance of conductors
/// `i` and `j` and `D'_ij` the distance of conductor `i` from the image of
/// conductor `j` below the surface, give `C = P⁻¹`, inverted here by
/// Gauss-Jordan elimination with partial pivoting. `ε0 = 8.8541878128e-12`
/// F/m. No engine code.
fn shunt_c(positions: &[(f64, f64)], r: f64) -> Vec<Vec<f64>> {
    let n = positions.len();
    let eps0 = 8.854_187_812_8e-12;
    let k = 1.0 / (2.0 * std::f64::consts::PI * eps0);
    // [P | I], reduced to [I | P⁻¹].
    let mut m: Vec<Vec<f64>> = (0..n)
        .map(|i| {
            let (xi, hi) = positions[i];
            let mut row: Vec<f64> = (0..n)
                .map(|j| {
                    let (xj, hj) = positions[j];
                    if i == j {
                        k * (2.0 * hi / r).ln()
                    } else {
                        k * ((xi - xj).hypot(hi + hj) / (xi - xj).hypot(hi - hj)).ln()
                    }
                })
                .collect();
            row.extend((0..n).map(|j| if i == j { 1.0 } else { 0.0 }));
            row
        })
        .collect();
    for col in 0..n {
        let pivot = (col..n)
            .max_by(|&a, &b| m[a][col].abs().total_cmp(&m[b][col].abs()))
            .expect("a pivot row");
        m.swap(col, pivot);
        let d = m[col][col];
        assert!(d != 0.0, "singular potential coefficients");
        for v in m[col].iter_mut() {
            *v /= d;
        }
        let pivot_row = m[col].clone();
        for (row, values) in m.iter_mut().enumerate() {
            if row != col {
                let f = values[col];
                for (v, p) in values.iter_mut().zip(&pivot_row) {
                    *v -= f * p;
                }
            }
        }
    }
    let scale = 1609.344 * 1e9;
    m.iter()
        .map(|row| row[n..].iter().map(|v| v * scale).collect())
        .collect()
}

/// The relative band between the engine's shunt capacitance and [`shunt_c11`]
/// or [`shunt_c`]. The engine's `ε0 = 8.854e-12` F/m sits 2.1e-5 relative
/// below the CODATA value, which scales every entry alike. A height of 28 read
/// in metres instead of feet moves C11 by 16 %.
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
    saved_line(dir, geom)
}

/// The line the Save in `dir` writes for geometry `geom`, lowercased.
fn saved_line(dir: &Path, geom: &str) -> String {
    let text = read(dir, "LineGeometry.dss");
    assert!(!text.to_ascii_lowercase().contains("units=none"), "{text}");
    assert!(!text.to_ascii_lowercase().contains(" cond=0"), "{text}");
    let head = format!("new \"linegeometry.{geom}\"");
    text.lines()
        .map(str::to_ascii_lowercase)
        .find(|l| l.starts_with(&head))
        .unwrap_or_else(|| panic!("no {head} line in {text}"))
}

/// The conductor rows of a saved geometry line as `(cond, kind=name, x=.., h=..,
/// units=..)` tokens, `""` for a token the row does not write, and the closing
/// ` cond=k` that restores the selection, if any. A row is `cond=c`, an
/// optional `wire=`/`cncable=`/`tscable=`, an optional `x=`, an optional `h=`
/// and `units=`.
fn saved_rows(line: &str) -> (Vec<[String; 5]>, Option<String>) {
    let tokens: Vec<&str> = line.split(' ').collect();
    let optional: [(usize, &[&str]); 3] = [
        (1, &["wire=", "cncable=", "tscable="]),
        (2, &["x="]),
        (3, &["h="]),
    ];
    let mut rows = Vec::new();
    let mut restore = None;
    let mut i = 0;
    while i < tokens.len() {
        if let Some(c) = tokens[i].strip_prefix("cond=") {
            let mut row: [String; 5] = Default::default();
            row[0] = c.to_string();
            let mut k = i + 1;
            for (slot, prefixes) in optional {
                if let Some(t) = tokens
                    .get(k)
                    .filter(|t| prefixes.iter().any(|p| t.starts_with(p)))
                {
                    row[slot] = t.to_string();
                    k += 1;
                }
            }
            if let Some(t) = tokens.get(k).filter(|t| t.starts_with("units=")) {
                row[4] = t.to_string();
                rows.push(row);
                i = k + 1;
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
    let back = reload_unsolved(dir);
    assert!(back.errors().is_empty(), "reload: {:?}", errors(&back));
    back
}

/// [`reload`] of a Save whose circuit does not solve: the errors are the
/// caller's to read.
fn reload_unsolved(dir: &Path) -> Dss {
    let mut back = Dss::new();
    back.command(&format!("compile \"{}\"", slash(&dir.join("Master.dss"))));
    back.command("solve");
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

/// Script A's full shunt capacitance equals [`shunt_c`] at the positions the
/// rules give: conductor 1 at (-4, 28) ft, conductors 2 and 3 at (0, 28) and
/// (4, 28) m, radius 0.4635 in (ACSR_556_5, DIAM 0.927 in). The derivation
/// gives, in nF/mi, C11 12.4165035, C21 -0.7139112, C31 -0.6644440, C22
/// 11.7549853, C32 -3.6162713 and C33 11.7494940. The coupling terms carry
/// the units: the three conductors all in feet give C21 -4.4398746, all in
/// metres -3.1830860. Every entry is compared within [`C_BAND`] of its
/// magnitude, the off-diagonal ones being negative.
#[test]
fn capacitance_follows_each_conductors_unit() {
    let dss = compile("a_units_on_line_2_only.dss");
    assert!(dss.errors().is_empty(), "{:?}", errors(&dss));
    assert!(solved(&dss), "script A did not solve");
    let ft = 0.3048;
    let want = shunt_c(
        &[(-4.0 * ft, 28.0 * ft), (0.0, 28.0), (4.0, 28.0)],
        RADIUS_P,
    );
    for (i, j, value) in [
        (1, 1, 12.416_503_5),
        (2, 1, -0.713_911_2),
        (3, 1, -0.664_444_0),
        (2, 2, 11.754_985_3),
        (3, 2, -3.616_271_3),
        (3, 3, 11.749_494_0),
    ] {
        assert!(
            (want[i - 1][j - 1] - value).abs() < 1e-6 && (want[j - 1][i - 1] - value).abs() < 1e-6,
            "derivation C{i}{j} = {}, expected {value}",
            want[i - 1][j - 1]
        );
    }
    let all_ft = shunt_c(
        &[
            (-4.0 * ft, 28.0 * ft),
            (0.0, 28.0 * ft),
            (4.0 * ft, 28.0 * ft),
        ],
        RADIUS_P,
    );
    let all_m = shunt_c(&[(-4.0, 28.0), (0.0, 28.0), (4.0, 28.0)], RADIUS_P);
    assert!(
        (all_ft[1][0] + 4.439_874_6).abs() < 1e-6,
        "{}",
        all_ft[1][0]
    );
    assert!((all_m[1][0] + 3.183_086_0).abs() < 1e-6, "{}", all_m[1][0]);
    let c = cmatrix(&dss, "L");
    assert_eq!(c.len(), 3, "Line.L order");
    for (i, (row, want_row)) in c.iter().zip(&want).enumerate() {
        for (j, (got, w)) in row.iter().zip(want_row).enumerate() {
            assert!(
                (got - w).abs() <= C_BAND * w.abs(),
                "Line.L C{}{} = {got} nF/mi, the rules' positions give {w}",
                i + 1,
                j + 1
            );
        }
    }
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
    assert_eq!((g[0].x, g[0].h), (Some(-4.0), Some(28.0)));
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
            x: None,
            h: None,
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
    assert_eq!((g[0].x, g[0].h), (Some(-6.0), Some(28.0)));
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
    assert_eq!(conductors(&dss, "g")[0].x, Some(-4.0));
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
    assert_eq!(conductors(&dss, "g")[1].x, Some(0.0));
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
/// exceeds `nconds`. On a geometry with no conductors both lists are refused
/// as every property before `nconds=` is.
#[test]
fn a_cable_or_wire_list_of_the_wrong_length_is_refused() {
    let mut dss = compile("v8b_cable_and_wire_counts.dss");
    let count = |geom: &str, got: usize, want: usize, why: &str| {
        format!(
            "LineGeometry.{geom}: Unexpected number ({got}) of objects; expected {want} objects{why}."
        )
    };
    let no_neutral = ": the phases hold cables and no conductor is a neutral";
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
            before_nconds("empty", "CNCables"),
            before_nconds("empty", "Wires"),
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

/// `wires=` takes its count from the data, never from the selection. After
/// `cncables=` in the phase positions the list is the bare neutral whichever
/// conductor is selected: conductor 1, a cable phase (`sel1`), conductor 4
/// (`sel4`) or none (`none`). Each holds ACSR_4/0 on conductor 4, reads like
/// `cn_twin`, written one conductor at a time, and its Line equals
/// [`cable_z`] (the V8 values). With bare phases and a cable last the list is
/// every conductor, so one name with conductor 4 selected is refused
/// (`bare4`) and conductor 4 keeps its cable. One cable phase is enough (the
/// any-phase reading of the help): with a cable on phase 1 and bare wires on
/// phases 2 and 3 the list is the bare neutral, so one name fills conductor
/// 4, written by `conductors=` (`part`) or one conductor at a time with a
/// bare conductor selected (`partc`), and a name for every conductor is
/// refused (`part4`), the cable staying. The cable may sit on any phase: on
/// phase 3 (`part3`, `part3_4`) the lists read the same. Counting the
/// neutrals only when every phase holds a cable would refuse `part`, `partc`
/// and `part3` and write `part4` and `part3_4` over the cable.
#[test]
fn wires_count_follows_the_data_not_the_selection() {
    use Holds::{Bare, Cn};
    let dss = compile("w1_wires_count_follows_the_data.dss");
    assert_eq!(
        errors(&dss),
        [
            "LineGeometry.bare4: Unexpected number (1) of objects; expected 4 objects.",
            "LineGeometry.part4: Unexpected number (4) of objects; expected 1 objects.",
            "LineGeometry.part3_4: Unexpected number (4) of objects; expected 1 objects.",
        ]
    );
    assert!(solved(&dss));
    let mut laid_cn = [-0.5, 0.0, 0.5, 1.0].map(|x| laid(Cn, x, -4.0, "ft"));
    laid_cn[3].holds = Bare;
    for geom in ["sel1", "sel4", "none"] {
        assert_eq!(
            conductors(&dss, geom),
            conductors(&dss, "cn_twin"),
            "{geom}"
        );
        assert_eq!(
            conductors(&dss, geom)[3].object.as_deref(),
            Some("acsr_4/0"),
            "{geom}"
        );
        assert_cable(
            &dss,
            geom,
            (&laid_cn, true),
            X_BAND,
            &[
                (1, 1, 0.738_155_6, 0.464_518_9),
                (2, 1, 0.251_504_6, 0.051_760_4),
                (3, 1, 0.199_562_0, -0.011_290_7),
                (3, 3, 0.694_716_0, 0.413_012_2),
            ],
            geom,
        );
    }
    let objects = |geom: &str| -> Vec<Option<String>> {
        conductors(&dss, geom)
            .into_iter()
            .map(|c| c.object)
            .collect()
    };
    let wire = || Some("acsr_556_5".to_string());
    let cn = || Some("250_1/3".to_string());
    assert_eq!(objects("bare4"), [wire(), wire(), wire(), cn()]);
    for geom in ["part", "partc"] {
        assert_eq!(
            objects(geom),
            [cn(), wire(), wire(), Some("acsr_4/0".to_string())],
            "{geom}"
        );
    }
    assert_eq!(objects("part4"), [cn(), wire(), wire(), wire()]);
    assert_eq!(
        objects("part3"),
        [wire(), wire(), cn(), Some("acsr_4/0".to_string())]
    );
    assert_eq!(objects("part3_4"), [wire(), wire(), cn(), wire()]);
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
    assert_eq!((g[0].x, g[0].h), (Some(-5.0), Some(28.0)));
}

/// A bare `cond=N` changes no unit: the conductor takes the geometry's default
/// unit at its first selection, unless `units=` is written for it. In `c`
/// conductor 2 is first selected bare under the default feet, so the `units=m`
/// of conductor 1 after it leaves it in feet: (0, 10) m and (1, 10) ft, 6.9587
/// m apart, X21 0.5831588 Ω/mi. In `d` conductor 2 is first selected after the
/// default became metres, and in `e` it is first selected bare and given
/// `units=m` later, so both are 1 m apart, X21 0.8185604 Ω/mi. Both oracles
/// read the same units (r4133 X21 0.5831588 and 0.8185604 on `Lc` and on `Ld`
/// and `Le`, dss_capi 0.583093787813716 and 0.818495370392536 with its own
/// earth-return depth).
#[test]
fn a_bare_cond_keeps_the_default_unit_of_its_first_selection() {
    let dss = compile("q3_bare_cond.dss");
    assert!(dss.errors().is_empty(), "{:?}", errors(&dss));
    assert!(solved(&dss));
    assert_eq!(units(&dss, "c"), ["m", "ft"]);
    assert_eq!(units(&dss, "d"), ["m", "m"]);
    assert_eq!(units(&dss, "e"), ["m", "m"]);
    let c = [phase(0.0, 10.0, "m"), phase(1.0, 10.0, "ft")];
    assert_x(&dss, "Lc", &c, &[(2, 1, 0.583_158_8)], "c");
    let in_m = [phase(0.0, 10.0, "m"), phase(1.0, 10.0, "m")];
    assert_x(&dss, "Ld", &in_m, &[(2, 1, 0.818_560_4)], "d");
    assert_x(&dss, "Le", &in_m, &[(2, 1, 0.818_560_4)], "e");
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
    let both = format!(
        "LineGeometry.g2: conductor 1 has {NOTHING}. LineGeometry.g1: conductor 2 has {NOTHING}."
    );
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
    assert_eq!((g[3].x, g[3].h), (Some(0.0), Some(24.0)));
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
/// selected before it, has no `x` after the re-allocation. Written again with
/// conductor 2 given no `x=`, both geometries lack conductor 2's `x`, which is
/// rule 7's one error of the solve, `g` then `e` in build order (the `x=5`
/// placed on conductor 2 would let them solve). Their Save reloads to the same
/// conductors and the same error.
#[test]
fn variant_nconds_selects_nothing() {
    let scratch = Scratch::new("n2");
    let name = "n2_nconds_selects_nothing.dss";
    let mut edited = replay_until(name, "Edit LineGeometry.e cond=1");
    assert_eq!(cond(&mut edited, "e"), "0");
    let e = conductors(&edited, "e");
    assert_eq!((e[1].object.as_deref(), e[1].x, e[1].h), (None, None, None));
    assert_eq!(
        cond_trace(name, "g"),
        ["0", "1", "2", "0", "0", "1", "2", "3", "4"]
    );

    let mut dss = compile(name);
    let both = "LineGeometry.g: conductor 2 has no x. LineGeometry.e: conductor 2 has no x.";
    assert_eq!(
        errors(&dss),
        [
            no_cond("g", "X"),
            no_cond("e", "X"),
            format!("Error Encountered in CalcVoltageBases: {both}"),
        ]
    );
    assert!(!solved(&dss));
    for geom in ["g", "e"] {
        assert_eq!(units(&dss, geom), ["ft"; 4], "{geom}");
        let xs: Vec<Option<f64>> = conductors(&dss, geom).iter().map(|c| c.x).collect();
        assert_eq!(xs, [Some(-4.0), None, Some(4.0), Some(0.0)], "{geom}");
    }

    let dir = scratch.dir("save");
    save(&mut dss, &dir, "g");
    let back = reload_unsolved(&dir);
    for geom in ["g", "e"] {
        assert_eq!(conductors(&back, geom), conductors(&dss, geom), "{geom}");
    }
    assert_eq!(
        errors(&back),
        [
            format!("Error Encountered in CalcVoltageBases: {both}"),
            format!("Error Encountered in Solve: {both}"),
        ]
    );
    assert!(!solved(&back));
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
            x: None,
            h: None,
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
/// conductor selected: `cond=11` on four conductors, `cond=0`, and `cond=2`
/// and `cond=-1` on one conductor. The `x=` after them is refused. A `cond=1`
/// before `nconds=` is refused as every property there is, with no code.
#[test]
fn cond_out_of_range_is_refused_and_selects_nothing() {
    let dss = compile("r6_cond_zero_and_cond_before_nconds.dss");
    assert_eq!(
        errors(&dss),
        [
            before_nconds("g", "Cond"),
            out_of_range("g", 0, "has 3 conductors"),
            no_cond("g", "X"),
        ]
    );
    assert_eq!(dss.errors()[0].code, None);
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
    assert_eq!(e.errors()[0].code, Some(10102));
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

/// A property before the first `nconds=` of a new geometry is refused, naming
/// it, in token order, with no code, and writes nothing: `g`'s New line writes
/// `units=m`, `nphases=2`, `normamps=500`, `reduce=y` and `cond=1` ahead of
/// `nconds=3`. So `NPhases` stays 3, `NormAmps` comes from the wire (730),
/// `Reduce` stays No and the conductors are in feet: X21 0.7945111 Ω/mi, where
/// metres would give 0.6503452. A `like=` copy of `g` holds conductors, so
/// `h`'s `units=m` is accepted (the conductors keep their own feet). A `like=`
/// whose source is not found leaves `k` without conductors, so its `x=` is
/// refused as before `nconds=`. A second `New` of `g` holds conductors too:
/// its `units=m` ahead of `nconds=3` is accepted and `nconds=` returns the
/// default to feet.
#[test]
fn a_property_before_the_first_nconds_is_refused() {
    let mut dss = compile("p1_property_before_nconds.dss");
    let mut want: Vec<String> = ["Units", "NPhases", "NormAmps", "Reduce", "Cond"]
        .iter()
        .map(|p| before_nconds("g", p))
        .collect();
    want.push("Error in LineGeometry MakeLike: \"nosuch\" not found.".to_string());
    want.push(before_nconds("k", "X"));
    assert_eq!(errors(&dss), want);
    for (i, e) in dss.errors().iter().enumerate() {
        assert_eq!(e.code, None, "{i}: {}", e.message);
    }
    assert!(solved(&dss));
    assert_eq!(query(&mut dss, "LineGeometry.g.NPhases"), "3");
    assert_eq!(query(&mut dss, "LineGeometry.g.NormAmps"), "730");
    assert_eq!(query(&mut dss, "LineGeometry.g.Reduce"), "No");
    assert_eq!(units(&dss, "g"), ["ft"; 3]);
    assert_x(
        &dss,
        "L",
        &[
            phase(-4.0, 28.0, "ft"),
            phase(0.0, 28.0, "ft"),
            phase(4.0, 28.0, "ft"),
        ],
        &[(2, 1, 0.794_511_1)],
        "P1",
    );
    assert_eq!(query(&mut dss, "LineGeometry.h.Units"), "m");
    assert_eq!(units(&dss, "h"), ["ft"; 3]);
    assert!(conductors(&dss, "k").is_empty());
    let before = dss.errors().len();
    dss.command("New LineGeometry.g units=m nconds=3");
    assert_eq!(errors(&dss)[before..], [] as [String; 0]);
    assert_eq!(query(&mut dss, "LineGeometry.g.Units"), "ft");
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
/// (1410065408)` and `(-2147483648)`. A whole number outside 1 to 4 written
/// as a decimal (`5.0`) is refused naming it as written, and every
/// out-of-range refusal carries code 10102, in and past the integer range
/// alike.
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
    let codes: Vec<_> = dss.errors().iter().map(|e| e.code).collect();
    assert_eq!(
        codes,
        [None, None, None, None, Some(10102), None, Some(10102), None]
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
    // A whole number outside the conductors, written as a decimal, is named
    // as written and selects nothing.
    dss.command("Edit LineGeometry.g cond=2");
    let before = dss.errors().len();
    dss.command("Edit LineGeometry.g cond=5.0");
    assert_eq!(errors(&dss)[before..], [refused("5.0", "is out of range")]);
    assert_eq!(dss.errors()[before].code, Some(10102));
    assert_eq!(cond(&mut dss, "g"), "0");
    // A whole number written as a decimal selects its conductor.
    dss.command("Edit LineGeometry.g cond=2.0 x=0");
    assert_eq!(cond(&mut dss, "g"), "2");
}

/// Rule 7: a conductor without a wire, cncable or tscable is one error at
/// calculation, naming it and what it lacks: in the solve (F, and F's geometry
/// on two Lines), in `Show LineConstants`, and for two and for three missing
/// conductors in one message.
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
        [format!(
            "Error computing line constants for LineGeometry.g; Error message: \
                 LineGeometry.g: conductor 2 has {NOTHING}."
        )]
    );
    assert_eq!(dss.errors()[1].code, Some(9934));

    let mut two = compile("h2_one_conductor_with_cond.dss");
    two.command(&format!("Set DataPath=\"{}\"", slash(&scratch.dir("two"))));
    two.command("New LineGeometry.two nconds=3 nphases=3");
    two.command("~ cond=1 wire=ACSR_556_5 x=0 h=28");
    two.command("Show LineConstants freq=60 units=mi rho=100");
    assert_eq!(
        errors(&two),
        [format!(
            "Error computing line constants for LineGeometry.two; Error message: \
                 LineGeometry.two: conductors 2 and 3 have {NOTHING}."
        )]
    );

    let mut four = compile("h2_one_conductor_with_cond.dss");
    four.command(&format!("Set DataPath=\"{}\"", slash(&scratch.dir("four"))));
    four.command("New LineGeometry.four nconds=4 nphases=3");
    four.command("~ cond=1 wire=ACSR_556_5 x=0 h=28");
    four.command("Show LineConstants freq=60 units=mi rho=100");
    assert_eq!(
        errors(&four),
        [format!(
            "Error computing line constants for LineGeometry.four; Error message: \
                 LineGeometry.four: conductors 2, 3 and 4 have {NOTHING}."
        )]
    );
}

/// Rule 7 names a conductor without an `x` or an `h` too, unless the geometry
/// is built on an equivalent spacing: `nox` (one conductor without `x`), `noh`
/// (conductor 3 without `h`), `mixed` (conductor 1 without `x`, conductor 3
/// without `h`), the bare neutral `buried` takes from `wires=` with no
/// position, and `ge`, on the equivalent spacing with no wire, named for its
/// wires only. They are one error at `CalcVoltageBases` and at `Solve`, each
/// geometry once in build order, with no singular-matrix error after. `ga` on
/// the equivalent spacing and `gd` on a detailed spacing are not named. Given
/// what they lack, all seven solve: the equivalent ones at X21 0.7964372 Ω/mi,
/// `gd` and the three overhead phases at script C's 0.7945111.
#[test]
fn a_conductor_without_a_position_is_the_rule_7_error() {
    let mut dss = compile("x1_conductor_without_a_position.dss");
    let all = "LineGeometry.nox: conductor 1 has no x. LineGeometry.noh: conductor 3 has no h. \
               LineGeometry.mixed: conductor 1 has no x. Conductor 3 has no h. \
               LineGeometry.buried: conductor 3 has no x and no h. LineGeometry.ge: conductors \
               1, 2, 3 and 4 have no wire, cncable or tscable.";
    assert_eq!(
        errors(&dss),
        [format!("Error Encountered in CalcVoltageBases: {all}")]
    );
    assert!(!solved(&dss));
    dss.command("Solve");
    assert_eq!(
        errors(&dss)[1..],
        [format!("Error Encountered in Solve: {all}")]
    );
    assert!(!solved(&dss));

    for (geom, line, given) in [
        ("nox", "Lnox", "cond=1 x=0"),
        ("noh", "Lnoh", "cond=3 h=28"),
        ("mixed", "Lmixed", "cond=1 x=-4 cond=3 h=28"),
        ("buried", "Lb", "cond=3 x=1.5 h=-4"),
        (
            "ge",
            "Le",
            "wires=[ACSR_556_5 ACSR_556_5 ACSR_556_5 ACSR_4/0]",
        ),
    ] {
        dss.command(&format!("Edit LineGeometry.{geom} {given}"));
        dss.command(&format!("Edit Line.{line} geometry={geom}"));
    }
    dss.command("Solve");
    assert_eq!(dss.errors().len(), 2, "{:?}", errors(&dss));
    assert!(solved(&dss));
    let eq = carson_x_equivalent(&[GMR_P, GMR_P, GMR_P, GMR_N], 3, 1.2, 1.5);
    for l in ["La", "Le"] {
        assert_x_is(&dss, l, &eq, &[(2, 1, 0.796_437_2)], l);
    }
    assert_x(&dss, "Ld", &c0_in(["ft"; 4]), &[(2, 1, 0.794_511_1)], "gd");
    let three = [
        phase(-4.0, 28.0, "ft"),
        phase(0.0, 28.0, "ft"),
        phase(4.0, 28.0, "ft"),
    ];
    for l in ["Lnoh", "Lmixed"] {
        assert_x(&dss, l, &three, &[(2, 1, 0.794_511_1)], l);
    }
    assert_c11(&dss, "Lnox", 28.0 * 0.3048, "nox");
}

/// Rule 8: a unit is never read as metres silently. Script C, with no `units=`
/// at all, is in feet. A `units=m` before the first `nconds=` of a new
/// geometry is refused, so R8's conductors read their metre numbers in feet
/// (X21 0.9386771 Ω/mi, where metres give script C's 0.7945111), and
/// `units=none` is refused with conductor 1 keeping feet, and so is
/// `units=feet`, a word outside the unit names. A redefinition reads its
/// unit-less numbers in feet (R9). A spacing refuses a unit word outside its
/// list and keeps feet ([`a_spacing_refuses_a_unit_word_outside_its_list`]).
/// The buried neutral that `wires=`
/// fills saves in feet, with no position, while no `cond=` has selected it,
/// read by a replay up to the `Edit` that places it, and follows the default
/// to metres when a `units=m` after `cond=1` makes metres the default (rule 4).
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
            before_nconds("g", "Units"),
            "LineGeometry.g.Units: \"none\" is not one of the length units mi, kft, km, m, ft, \
             in, cm, mm."
                .to_string(),
        ]
    );
    assert_eq!(units(&r8, "g"), ["ft"; 3]);
    assert_x(
        &r8,
        "L",
        &[
            phase(-1.2192, 8.5344, "ft"),
            phase(0.0, 8.5344, "ft"),
            phase(1.2192, 8.5344, "ft"),
        ],
        &[(2, 1, 0.938_677_1)],
        "R8",
    );
    r8.command("Edit LineGeometry.g cond=1 units=feet");
    assert_eq!(
        errors(&r8)[2..],
        [
            "LineGeometry.g.Units: \"feet\" is not one of the length units mi, kft, km, m, ft, \
             in, cm, mm."
        ]
    );
    assert_eq!(units(&r8, "g"), ["ft"; 3]);

    let arrays = compile("arrays_keep_the_selection.dss");
    assert_eq!(units(&arrays, "buried"), ["ft"; 3]);
    let mut unplaced = replay_until("arrays_keep_the_selection.dss", "Edit LineGeometry.buried");
    let (rows, restore) = saved_rows(&save(&mut unplaced, &scratch.dir("save"), "buried"));
    assert_eq!(rows[2], ["3", "wire=acsr_4/0", "", "", "units=ft"]);
    assert_eq!(restore.as_deref(), Some("2"));
    unplaced.command("Edit LineGeometry.buried cond=1 units=m");
    assert_eq!(units(&unplaced, "buried"), ["m", "ft", "m"]);
    let (rows, restore) = saved_rows(&save(&mut unplaced, &scratch.dir("metres"), "buried"));
    assert_eq!(rows[2], ["3", "wire=acsr_4/0", "", "", "units=m"]);
    assert_eq!(restore.as_deref(), Some("1"));
}

/// A unit word is read whole: `units=mils` on conductor 2's line is neither
/// `mi` nor anything else, so it is refused naming the word and conductor 2
/// takes the default feet (X21 0.7945111 Ω/mi, where miles would put it 28
/// miles up).
#[test]
fn a_unit_word_is_read_whole() {
    let dss = compile("u1_a_unit_word_is_read_whole.dss");
    assert_eq!(
        errors(&dss),
        [
            "LineGeometry.g.Units: \"mils\" is not one of the length units mi, kft, km, m, ft, \
             in, cm, mm."
        ]
    );
    assert!(solved(&dss));
    assert_eq!(units(&dss, "g"), ["ft"; 3]);
    assert_x(
        &dss,
        "L",
        &[
            phase(-4.0, 28.0, "ft"),
            phase(0.0, 28.0, "ft"),
            phase(4.0, 28.0, "ft"),
        ],
        &[(2, 1, 0.794_511_1)],
        "U1",
    );
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

/// The CIM export writes a coordinate only where it was given: conductor 2 of
/// `p`, given `h=28` and no `x=`, has a `yCoord` (8.5344 m) and no `xCoord`,
/// and conductor 3, given `x=1` and no `h=`, an `xCoord` (0.3048 m) and no
/// `yCoord`.
#[test]
fn cim_writes_no_coordinate_never_given() {
    let scratch = Scratch::new("cim_none");
    let mut dss = compile("h2_one_conductor_with_cond.dss");
    dss.command(
        "New LineGeometry.p nconds=3 nphases=3 cond=1 wire=ACSR_556_5 x=-1 h=28 cond=2 \
         wire=ACSR_556_5 h=28 cond=3 wire=ACSR_556_5 x=1",
    );
    dss.command(&format!("Set DataPath=\"{}\"", slash(&scratch.dir("cim"))));
    dss.command("Export CIM100");
    assert!(dss.errors().is_empty(), "{:?}", errors(&dss));
    let xml = read(&scratch.0.join("cim"), "lgrules_CIM100x.xml");
    let position = |i: usize| -> String {
        xml.split("<cim:WirePosition ")
            .find(|b| b.contains(&format!(">WP_p_{i}<")))
            .and_then(|b| b.split("</cim:WirePosition>").next())
            .unwrap_or_else(|| panic!("no WirePosition WP_p_{i}"))
            .to_string()
    };
    let (x, y) = ("<cim:WirePosition.xCoord>", "<cim:WirePosition.yCoord>");
    assert!(
        position(1).contains(x) && position(1).contains(y),
        "{}",
        position(1)
    );
    assert!(!position(2).contains(x), "{}", position(2));
    assert!(
        position(2).contains(&format!("{y}8.5344<")),
        "{}",
        position(2)
    );
    assert!(
        position(3).contains(&format!("{x}0.3048<")),
        "{}",
        position(3)
    );
    assert!(!position(3).contains(y), "{}", position(3));
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
    assert_eq!((buried[1].x, buried[1].h), (Some(0.6), Some(-4.0)));
    assert_eq!(buried[2].object.as_deref(), Some("acsr_4/0"));
    assert_eq!((buried[2].x, buried[2].h), (None, None));
    let unplaced = replay_until(name, "Edit LineGeometry.gn");
    assert!(
        conductors(&unplaced, "gn")
            .iter()
            .all(|c| c.x.is_none() && c.h.is_none() && c.object.is_some())
    );

    let dss = compile(name);
    assert_eq!(errors(&dss), [no_cond("g", "X"), no_cond("gn", "X")]);
    assert!(solved(&dss));
    let g = conductors(&dss, "g");
    let xs: Vec<Option<f64>> = g.iter().map(|c| c.x).collect();
    assert_eq!(xs, [Some(-4.0), Some(0.0), Some(4.0), Some(0.0)]);
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
    let gc: Vec<Option<f64>> = conductors(&dss, "gc").iter().map(|c| c.x).collect();
    assert_eq!(gc, [Some(-4.0), Some(-2.0), Some(4.0), Some(0.0)]);
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
    assert_eq!(
        (buried[1].x, buried[2].x, buried[2].h),
        (Some(0.6), Some(1.5), Some(-4.0))
    );
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
    let xs: Vec<Option<f64>> = conductors(&dss, "g").iter().map(|c| c.x).collect();
    assert_eq!(xs, [Some(-4.0), Some(7.0), Some(4.0), Some(0.0)]);
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
/// stamp: the voltage-base pass builds the series Y and drops it, as for the
/// refusal alone, the Capacitor's message is logged, and the build's error
/// names the Line. No system Y is built either.
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
    assert!(
        solution.y_series.is_none(),
        "the refused series build left a Y"
    );
    assert!(solution.y_system.is_none(), "a system Y was built");
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
        [format!("LineGeometry.g: conductor 2 has {NOTHING}.")]
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
    let rule_7 = [format!("LineGeometry.g: conductor 2 has {NOTHING}.")];
    assert_eq!(logged, [rule_7.clone(), rule_7]);
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
/// neither, has no conductor positions, rule 7's one error of the solve. On
/// `gs` that Line solves to `ga`'s voltages.
#[test]
fn a_redefinition_drops_the_equivalent_spacing() {
    let mut dss = compile("r10_redefinition_drops_the_equivalent_spacing.dss");
    let gr = "LineGeometry.gr: conductors 1, 2, 3 and 4 have no x and no h.";
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

/// Equivalent distances keep the unit of the spacing they came from (spacing
/// `eq` in metres: phases 1.2 m apart and 1.5 m from the neutral). `gu`, given
/// `units=ft` after the spacing with no conductor selected, and `gl`, a
/// `like=` copy of `ga`, read the distances in metres like `ga`: X21 0.7964372
/// Ω/mi and X41 0.7693606, where feet would give X21 0.9406032. The `units=ft`
/// moves `gu`'s default unit only. Line `Lq`, with its own `spacing=` and
/// `wires=` on the three-wire spacing `eq3` in metres, reads X21 0.7964372 too.
#[test]
fn equivalent_distances_keep_their_own_unit() {
    let mut dss = compile("q1_equivalent_distances_keep_their_unit.dss");
    assert!(dss.errors().is_empty(), "{:?}", errors(&dss));
    assert!(solved(&dss));
    let wires = [GMR_P, GMR_P, GMR_P, GMR_N];
    let eq = carson_x_equivalent(&wires, 3, 1.2, 1.5);
    for l in ["La", "Lu", "Ll"] {
        assert_x_is(&dss, l, &eq, &[(2, 1, 0.796_437_2), (4, 1, 0.769_360_6)], l);
    }
    let feet = carson_x_equivalent(&wires, 3, 1.2 * 0.3048, 1.5 * 0.3048);
    assert!((feet[1][0] - 0.940_603_2).abs() <= X_BAND, "{}", feet[1][0]);
    assert_eq!(query(&mut dss, "LineGeometry.ga.Units"), "m");
    assert_eq!(query(&mut dss, "LineGeometry.gu.Units"), "ft");
    assert_eq!(query(&mut dss, "LineGeometry.gl.Units"), "m");
    let eq3 = carson_x_equivalent(&[GMR_P; 3], 3, 1.2, 1.5);
    assert_x_is(&dss, "Lq", &eq3, &[(2, 1, 0.796_437_2)], "Lq");
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

/// The refusal of a unit word outside a LineSpacing's list, as the spacing
/// logs it at its own token.
fn spacing_word_refused(spacing: &str, word: &str) -> String {
    format!(
        "LineSpacing.{spacing}.Units: \"{word}\" is not one of the length units mi, kft, km, m, \
         ft, in, cm, mm."
    )
}

/// S7: a LineSpacing refuses `units=none` and `units=feet` at its own token,
/// naming the word, and keeps feet. A geometry's `spacing=` then copies the
/// spacing in feet: `gnone` and `gfeet` hold its positions in feet and build
/// like `gft` (X21 0.7945111 Ω/mi at 4 ft, [`carson_x`]). Both oracles read
/// `feet` and `none` as no unit and compute those geometries in metres, X21
/// 0.6503451 on r4133.
#[test]
fn a_spacing_refuses_a_unit_word_outside_its_list() {
    let scratch = Scratch::new("s7");
    let mut dss = compile("s7_spacing_without_a_unit.dss");
    assert_eq!(
        errors(&dss),
        [
            spacing_word_refused("sp_none", "none"),
            spacing_word_refused("sp_feet", "feet"),
        ]
    );
    assert!(solved(&dss));
    for sp in ["sp_none", "sp_feet"] {
        assert_eq!(
            query(&mut dss, &format!("LineSpacing.{sp}.Units")),
            "ft",
            "{sp}"
        );
    }
    let row = [
        phase(-4.0, 28.0, "ft"),
        phase(0.0, 28.0, "ft"),
        phase(4.0, 28.0, "ft"),
    ];
    for (g, sp, line) in [
        ("gnone", "sp_none", "Lnone"),
        ("gfeet", "sp_feet", "Lfeet"),
        ("gft", "sp_ft", "L"),
    ] {
        let held: Vec<_> = conductors(&dss, g)
            .iter()
            .map(|c| (c.x, c.h, c.unit, c.object.is_some()))
            .collect();
        assert_eq!(
            held,
            [
                (Some(-4.0), Some(28.0), "ft", true),
                (Some(0.0), Some(28.0), "ft", true),
                (Some(4.0), Some(28.0), "ft", true),
            ],
            "{g}"
        );
        assert_eq!(query(&mut dss, &format!("LineGeometry.{g}.Spacing")), sp);
        let (rows, _) = saved_rows(&save(&mut dss, &scratch.dir(g), g));
        assert!(rows.iter().all(|r| r[4] == "units=ft"), "{g}: {rows:?}");
        assert_x(&dss, line, &row, &[(2, 1, 0.794_511_1)], g);
    }
}

/// A Line's own `spacing=` reads its LineSpacing's unit the way a geometry's
/// `spacing=` does. A spacing in feet, metres or inches is converted (`Lft` and
/// `Lin` X21 0.7945111 Ω/mi, `Lm` 0.6503451). A LineSpacing refuses
/// `units=none` and `units=feet` at its own token and keeps feet, so `Ln`,
/// `Lfe` and `Lkeep`, whose `Edit` moves it to the spacing written `feet` and
/// gives its wires again, build in feet at X21 0.7945111, with no message at
/// the Lines. Both oracles
/// read those spacings as metres, X21 0.6503451 on r4133. A spacing whose unit
/// code names no length unit is refused by the Line, which keeps what it had
/// ([`a_line_refuses_a_spacing_whose_unit_code_names_no_length`]).
#[test]
fn a_line_on_a_spacing_with_a_refused_unit_builds_in_feet() {
    let mut dss = compile("s8_line_spacing_units.dss");
    assert_eq!(
        errors(&dss),
        [
            spacing_word_refused("sp_none", "none"),
            spacing_word_refused("sp_feet", "feet"),
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
    for line in ["Ln", "Lfe", "Lkeep"] {
        assert_x(&dss, line, &row("ft"), &[(2, 1, 0.794_511_1)], line);
    }
    for (line, sp) in [("Ln", "sp_none"), ("Lfe", "sp_feet"), ("Lkeep", "sp_feet")] {
        assert_eq!(
            query(&mut dss, &format!("Line.{line}.Spacing")),
            sp,
            "{line}"
        );
    }
}

/// A Line's `spacing=` refuses a spacing whose unit code names no length unit
/// (set directly, since the spacing's own `units=` refuses every word outside
/// its list) at its token, naming the Line, the spacing and the code, and the
/// Line keeps what it had: `Lkeep` keeps `sp_feet`, its three phases, its wires
/// and X21 0.7945111 Ω/mi in feet. `sp2` has two phases, so a spacing applied
/// anyway would change the Line's phases.
#[test]
fn a_line_refuses_a_spacing_whose_unit_code_names_no_length() {
    use crate::elements::general::line_spacing::{LineSpacingObj, prop as spacing_prop};
    let mut dss = compile("s8_line_spacing_units.dss");
    assert!(solved(&dss));
    dss.command("New LineSpacing.sp2 nconds=2 nphases=2 x=[-4 4] h=[28 28] units=ft");
    let wires = "[acsr_556_5, acsr_556_5, acsr_556_5]";
    assert_eq!(query(&mut dss, "Line.Lkeep.wires"), wires);
    let row = [
        phase(-4.0, 28.0, "ft"),
        phase(0.0, 28.0, "ft"),
        phase(4.0, 28.0, "ft"),
    ];
    for (code, written) in [(0, "none"), (9, "9")] {
        let ci = dss.class_by_name["linespacing"];
        let oi = dss.classes[ci].name_to_idx["sp2"];
        dss.classes[ci]
            .arena
            .get_mut::<LineSpacingObj>(oi)
            .expect("a LineSpacing")
            .set_i32(spacing_prop::UNITS, code);
        let before = dss.errors().len();
        let cmd = "Edit Line.Lkeep spacing=sp2";
        dss.command(cmd);
        assert_eq!(
            errors(&dss)[before..],
            [format!(
                "Line.lkeep.Spacing: LineSpacing.sp2 has no length unit (units={written}), so \
                 the Line is left as it was. Give the spacing one of mi, kft, km, m, ft, in, cm, \
                 mm."
            )],
            "{code}"
        );
        let underlined: Vec<&str> = dss.errors()[before..]
            .iter()
            .map(|d| {
                let span = d.span.as_ref().expect("a span");
                &cmd[span.offset()..span.offset() + span.len()]
            })
            .collect();
        assert_eq!(underlined, ["sp2"], "{code}");
        assert_eq!(query(&mut dss, "Line.Lkeep.Spacing"), "sp_feet", "{code}");
        assert_eq!(query(&mut dss, "Line.Lkeep.phases"), "3", "{code}");
        assert_eq!(query(&mut dss, "Line.Lkeep.wires"), wires, "{code}");
        dss.command("Solve");
        assert!(solved(&dss), "{code}");
        assert_eq!(errors(&dss).len(), before + 1, "{code}");
        assert_x(&dss, "Lkeep", &row, &[(2, 1, 0.794_511_1)], written);
    }
}

/// A Line's `wires=` names one wire per open position of its spacing, so a
/// wire with no position is refused, naming the Line and the count, and the
/// Line keeps what it had. `z` (`nconds=0`) and `n` (`nconds=-5`, read as no
/// position) have no conductor position, so `Lz` and `Ln` keep no wire, the
/// rest of their line, and solve on the Line's default sequence impedances
/// like `Ldef`, which names no spacing, after the message. On
/// `s3` the three cable phases fill every position, so `Lc` refuses a bare
/// neutral, keeps its three cables and solves like `Ld`, which writes no
/// `wires=`. r4133 takes each wire with no message and drops it (`? wires`
/// reads `w`, `Lz` and `Ln` solve on the default sequence impedances, xmatrix
/// diagonal 0.2153, and `Lc` like `Ld`). The pinned dss_capi refuses `Lz` and
/// `Lc` with "Unexpected number (1) of wires; expected 0 objects." and fails
/// `nconds=-5` with 303 (measured 2026-10-23 with the epri-worker on the
/// r4133 DLL and the pinned dss-python).
#[test]
fn a_line_refuses_a_wire_with_no_position() {
    let mut dss = Dss::new();
    for c in [
        "New Circuit.nopos basekv=12.47 phases=3 bus1=src",
        "New WireData.w GMRac=0.0244 GMRunits=ft Rac=0.306 Runits=mi Diam=0.721 Radunits=in",
        "New CNData.cn k=13 DiaStrand=0.064 Rstrand=2.816666 epsR=2.3 InsLayer=0.220 \
         DiaIns=1.06 DiaCable=1.16 Rac=0.076705 GMRac=0.20568 diam=0.573 Runits=kft \
         Radunits=in GMRunits=in",
        "New LineSpacing.z nconds=0 nphases=3 units=ft",
        "New LineSpacing.n nconds=-5 nphases=3 units=ft",
        "New LineSpacing.s3 nconds=3 nphases=3 x=[-0.5 0 0.5] h=[-4 -4 -4] units=ft",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", errors(&dss));
    for c in [
        "New Line.Lz bus1=src.1.2.3 bus2=bz.1.2.3 spacing=z wires=[w] length=1 units=mi",
        "New Line.Ln bus1=src.1.2.3 bus2=bn.1.2.3 spacing=n wires=[w] length=1 units=mi",
        "New Line.Lc bus1=src.1.2.3 bus2=bc.1.2.3 spacing=s3 cncables=[cn cn cn] wires=[w] \
         length=1 units=mi",
        "New Line.Ld bus1=src.1.2.3 bus2=bd.1.2.3 spacing=s3 cncables=[cn cn cn] length=1 \
         units=mi",
        "New Line.Ldef bus1=src.1.2.3 bus2=bdef.1.2.3 length=1 units=mi",
    ] {
        dss.command(c);
    }
    let refused =
        |line: &str| format!("Line.{line}: Unexpected number (1) of wires; expected 0 objects.");
    assert_eq!(errors(&dss), [refused("lz"), refused("ln"), refused("lc")]);
    for (line, spacing) in [("Lz", "z"), ("Ln", "n")] {
        for (prop, want) in [
            ("wires", "[]"),
            ("spacing", spacing),
            ("phases", "3"),
            ("length", "1"),
            ("units", "mi"),
        ] {
            assert_eq!(
                query(&mut dss, &format!("Line.{line}.{prop}")),
                want,
                "{line} {prop}"
            );
        }
    }
    for line in ["Lc", "Ld"] {
        assert_eq!(
            query(&mut dss, &format!("Line.{line}.wires")),
            "[cn, cn, cn]",
            "{line}"
        );
        assert_eq!(
            query(&mut dss, &format!("Line.{line}.cncables")),
            "[cn, cn, cn]",
            "{line}"
        );
    }
    for c in ["Set voltagebases=[12.47]", "CalcVoltageBases", "Solve"] {
        dss.command(c);
    }
    assert!(solved(&dss));
    assert_eq!(errors(&dss).len(), 3, "{:?}", errors(&dss));
    for real in [false, true] {
        assert_eq!(zpart(&dss, "Lc", real), zpart(&dss, "Ld", real), "{real}");
        for line in ["Lz", "Ln"] {
            assert_eq!(
                zpart(&dss, line, real),
                zpart(&dss, "Ldef", real),
                "{line} {real}"
            );
        }
    }
}

/// A refused `wires=` list changes nothing on the Line, whichever refusal it
/// meets, no open position or a count other than the open positions: a
/// geometry, a line code or ratings written before it stay, and `wires`
/// enters no set order. `Bg` keeps its geometry and solves like `Bgg`, which
/// names no spacing, at X11 1.42425544092922 Ω/mi. `Bl` keeps its line code
/// and solves like `Bl0`, which writes no `wires=`. `Br` (cables on every
/// position) and `Lo` (a list of two, then four, for three positions) keep
/// their typed `normamps` and `emergamps` through the solve, the Save and the
/// JSON export, where the spacing model would reset them from the conductors.
/// `Lz` exports no conductor list, so its JSON record imports back with no
/// message. A wire list on a Line with no spacing is refused the same way:
/// `Lns` keeps no wire, exports no conductor list and imports back with no
/// message. r4133 takes every list on a spacing with no message and switches
/// the Line to its spacing model: `Bg` and `Bl` lose their geometry and line
/// code, `Bg` solves on the default sequence impedances (xmatrix diagonal
/// 0.2153), and `Br` and `Lo` read `normamps` and `emergamps` -1 after the
/// solve. It faults on `Lns`'s `New` line with an access violation (303),
/// keeping `w` and losing the rest of the line. The pinned dss_capi refuses
/// every list (`Bg` and `Bl` with 18102 "You must
/// assign the LineSpacing before the Wires Property") and keeps the geometry
/// and the line code, but `Br` and `Lo` read -1 after the solve too (measured
/// 2026-10-23 with the epri-worker on the r4133 DLL and the pinned
/// dss-python).
#[test]
fn a_refused_wire_list_changes_nothing_on_the_line() {
    use crate::report::export::json::JsonOpts;
    let wire = "New WireData.w GMRac=0.0244 GMRunits=ft Rac=0.306 Runits=mi Diam=0.721 \
                Radunits=in";
    let mut dss = Dss::new();
    for c in [
        "New Circuit.keep basekv=12.47 phases=3 bus1=src",
        wire,
        "New CNData.cn k=13 DiaStrand=0.064 Rstrand=2.816666 epsR=2.3 InsLayer=0.220 \
         DiaIns=1.06 DiaCable=1.16 Rac=0.076705 GMRac=0.20568 diam=0.573 Runits=kft \
         Radunits=in GMRunits=in",
        "New LineSpacing.z nconds=0 nphases=3 units=ft",
        "New LineSpacing.c3 nconds=3 nphases=3 x=[-0.5 0 0.5] h=[-4 -4 -4] units=ft",
        "New LineSpacing.s3 nconds=3 nphases=3 x=[-4 0 4] h=[28 28 28] units=ft",
        "New LineGeometry.g3 nconds=3 nphases=3 units=ft cond=1 wire=w x=-4 h=28 cond=2 \
         wire=w x=0 h=28 cond=3 wire=w x=4 h=28",
        "New LineCode.lc nphases=3 r1=0.3 x1=0.6 r0=0.3 x0=0.6 c1=0 c0=0 units=km",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", errors(&dss));
    for c in [
        "New Line.Bg bus1=src.1.2.3 bus2=bg.1.2.3 spacing=z geometry=g3 wires=[w] length=1 \
         units=mi",
        "New Line.Bgg bus1=src.1.2.3 bus2=bgg.1.2.3 geometry=g3 length=1 units=mi",
        "New Line.Bl bus1=src.1.2.3 bus2=bl.1.2.3 spacing=z linecode=lc wires=[w] length=1 \
         units=km",
        "New Line.Bl0 bus1=src.1.2.3 bus2=bl0.1.2.3 spacing=z linecode=lc length=1 units=km",
        "New Line.Br bus1=src.1.2.3 bus2=br.1.2.3 spacing=c3 cncables=[cn cn cn] normamps=999 \
         emergamps=1999 wires=[w] length=1 units=mi",
        "New Line.Lo bus1=src.1.2.3 bus2=lo.1.2.3 spacing=s3 wires=[w w w] normamps=500 \
         emergamps=700 length=1 units=mi",
        "Edit Line.Lo wires=[w w]",
        "Edit Line.Lo wires=[w w w w]",
        "New Line.Lz bus1=src.1.2.3 bus2=bz.1.2.3 spacing=z wires=[w] length=1 units=mi",
        "New Line.Lns bus1=src.1.2.3 bus2=bns.1.2.3 wires=[w] length=1 units=mi",
    ] {
        dss.command(c);
    }
    let count = |line: &str, n: usize, open: usize| {
        format!("Line.{line}: Unexpected number ({n}) of wires; expected {open} objects.")
    };
    assert_eq!(
        errors(&dss),
        [
            count("bg", 1, 0),
            count("bl", 1, 0),
            count("br", 1, 0),
            count("lo", 2, 3),
            count("lo", 4, 3),
            count("lz", 1, 0),
            "You must assign the LineSpacing before the Wires Property (\"Line.lns\").".to_string(),
        ]
    );
    assert_eq!(query(&mut dss, "Line.Bg.geometry"), "g3");
    assert_eq!(query(&mut dss, "Line.Bl.linecode"), "lc");
    assert_eq!(query(&mut dss, "Line.Lo.wires"), "[w, w, w]");
    assert_eq!(query(&mut dss, "Line.Lns.wires"), "[]");

    // The set order: `Lz` and `Lns` export no conductor list and import back
    // clean.
    let records: Vec<String> = ["Lz", "Lns"]
        .iter()
        .map(|line| {
            let record = dss
                .obj_to_json(&format!("Line.{line}"), JsonOpts::NONE)
                .expect("a Line");
            assert!(!record.contains("\"Conductors\""), "{record}");
            record
        })
        .collect();
    let json = format!(
        r#"{{"Name":"keepjson","PreCommands":["{wire}","New LineSpacing.z nconds=0 nphases=3 units=ft"],"Line":[{}]}}"#,
        records.join(",")
    );
    let mut back = Dss::new();
    assert_eq!(back.circuit_from_json(&json), Ok(()));
    assert!(back.errors().is_empty(), "{:?}", errors(&back));
    for (prop, want) in [
        ("Lz.spacing", "z"),
        ("Lns.wires", "[]"),
        ("Lns.units", "mi"),
    ] {
        assert_eq!(query(&mut back, &format!("Line.{prop}")), want, "{prop}");
    }

    for c in ["Set voltagebases=[12.47]", "CalcVoltageBases", "Solve"] {
        dss.command(c);
    }
    assert!(solved(&dss));
    assert_eq!(errors(&dss).len(), 7, "{:?}", errors(&dss));
    for real in [false, true] {
        assert_eq!(zpart(&dss, "Bg", real), zpart(&dss, "Bgg", real), "{real}");
        assert_eq!(zpart(&dss, "Bl", real), zpart(&dss, "Bl0", real), "{real}");
    }
    let x11 = xmatrix(&dss, "Bg")[0][0];
    assert!((x11 - 1.424_255_440_929_22).abs() <= X_BAND, "{x11}");

    let scratch = Scratch::new("refused_wires");
    let dir = scratch.dir("save");
    dss.command(&format!("save circuit dir=\"{}\"", slash(&dir)));
    let saved = read(&dir, "Line.dss");
    let number = |record: &str, key: &str| -> f64 {
        let at = record
            .find(&format!("\"{key}\":"))
            .unwrap_or_else(|| panic!("no {key} in {record}"))
            + key.len()
            + 3;
        let text = record[at..].split([',', '}']).next().unwrap_or_default();
        text.parse()
            .unwrap_or_else(|e| panic!("{key} {text} in {record}: {e}"))
    };
    for (line, norm, emerg, row) in [
        (
            "Br",
            999.0,
            1999.0,
            "New \"Line.br\" Bus1=src.1.2.3 Bus2=br.1.2.3 Spacing=c3 CNCables=[cn, cn, cn] \
             NormAmps=999 EmergAmps=1999 Length=1 Units=mi",
        ),
        (
            "Lo",
            500.0,
            700.0,
            "New \"Line.lo\" Bus1=src.1.2.3 Bus2=lo.1.2.3 Spacing=s3 Wires=[w, w, w] \
             NormAmps=500 EmergAmps=700 Length=1 Units=mi",
        ),
    ] {
        assert_eq!(
            query(&mut dss, &format!("Line.{line}.normamps")),
            norm.to_string(),
            "{line}"
        );
        assert_eq!(
            query(&mut dss, &format!("Line.{line}.emergamps")),
            emerg.to_string(),
            "{line}"
        );
        assert!(saved.lines().any(|l| l == row), "{row} in {saved}");
        let record = dss
            .obj_to_json(&format!("Line.{line}"), JsonOpts::NONE)
            .expect("a Line");
        assert_eq!(number(&record, "NormAmps"), norm, "{record}");
        assert_eq!(number(&record, "EmergAmps"), emerg, "{record}");
    }
}

/// A refused `conductors=` list changes nothing either: `Lq`'s list names no
/// class, `Lf`'s a wire that is not found and `Lv`'s a class other than
/// WireData, CNData and TSData, so each is refused at its token, the Line
/// keeps its spacing with no conductor model, and a `wires=` of three
/// overhead wires after it fills the three positions and solves like `Lw`,
/// which gives the wires at once. A LineGeometry's refused `conductors=` logs
/// its refusal alone, writes no conductor and seeds no rating: `gd`, whose
/// conductor 2 holds a wire rated 530/800 A, keeps its wires `[, w2, ]` and
/// the ratings of that wire, its only phase conductor, through a list that
/// names no class, one that names a wire not found, and two that start with
/// a valid wire and go on with an item that names no class or another
/// class. An item with no class or
/// another class is refused with code 10103. r4133 logs the Lines' lists
/// (181023, 181025 and 181024) and keeps each as written (`? conductors`
/// echoes it), and the `wires=[w w w]` after it, which `? wires` reads back,
/// leaves them faulting at `CalcVoltageBases` (303) and `Solve` (482) with an
/// access violation, where `Lw` with the same wires solves at X11 1.424255
/// Ω/mi. On a LineGeometry r4133 keeps the items before the first item with
/// no class and logs 10103 there. Given to `gd` as defined here, `[w w w]`
/// keeps nothing (`? wires` reads `[ w2  ]` before and after it), and
/// `[WireData.w w WireData.w]` writes `w` to conductor 1 (`[w w2  ]`). It
/// writes the wires of a list with an item not found and logs 10103 for the
/// missing one: `[WireData.nosuch WireData.w WireData.w]` reads
/// `[nosuch w w ]`, and conductor 2 becomes `w`. It reads an item of another
/// class after a valid item, in silence, as the previous wire under the
/// item's name: `[WireData.w Foo.w WireData.w]` logs nothing and reads
/// `[w w w ]`. The ratings stay at the not-set 0 through every list. It
/// computes with what it kept. On a geometry of three `w2`,
/// `[WireData.w WireData.nosuch WireData.w]` reads `[w nosuch w ]`, and a
/// Line on it reads X11 and X33 1.424255 Ω/mi, the `w` value, and X22
/// 1.399185, the `w2` value kept under the missing name.
/// `[WireData.w Foo.w2 WireData.w]` reads `[w w2 w ]` there, and a Line on it
/// reads X22 1.424255, the `w` value under the name `w2`. The pinned dss_capi
/// has no `conductors` property (110) (measured 2026-10-23 with the
/// epri-worker on the r4133 DLL and the pinned dss-python, one process per
/// list).
#[test]
fn a_refused_conductor_list_changes_nothing() {
    let mut dss = Dss::new();
    for c in [
        "New Circuit.cond basekv=12.47 phases=3 bus1=src",
        "New WireData.w GMRac=0.0244 GMRunits=ft Rac=0.306 Runits=mi Diam=0.721 Radunits=in",
        "New WireData.w2 GMRac=0.03 GMRunits=ft Rac=0.2 Runits=mi Diam=0.8 Radunits=in \
         normamps=530 emergamps=800",
        "New LineSpacing.s3 nconds=3 nphases=3 x=[-4 0 4] h=[28 28 28] units=ft",
        "New Line.Lq bus1=src.1.2.3 bus2=bq.1.2.3 spacing=s3 conductors=[w w w] length=1 \
         units=mi",
        "Edit Line.Lq wires=[w w w]",
        "New Line.Lf bus1=src.1.2.3 bus2=bf.1.2.3 spacing=s3 \
         conductors=[WireData.nosuch WireData.nosuch WireData.nosuch] length=1 units=mi",
        "Edit Line.Lf wires=[w w w]",
        "New Line.Lv bus1=src.1.2.3 bus2=bv.1.2.3 spacing=s3 conductors=[Foo.w Foo.w Foo.w] \
         length=1 units=mi",
        "Edit Line.Lv wires=[w w w]",
        "New Line.Lw bus1=src.1.2.3 bus2=bw.1.2.3 spacing=s3 wires=[w w w] length=1 units=mi",
        "New LineGeometry.gq nconds=3 nphases=3 conductors=[w w w]",
        "New LineGeometry.gd nconds=3 nphases=3 units=ft cond=2 wire=w2 x=0 h=28",
    ] {
        dss.command(c);
    }
    let ratings = |dss: &mut Dss| {
        ["normamps", "emergamps"].map(|p| query(dss, &format!("LineGeometry.gd.{p}")))
    };
    assert_eq!(ratings(&mut dss), ["530", "800"]);
    assert_eq!(query(&mut dss, "LineGeometry.gd.wires"), "[, w2, ]");
    for list in [
        "[w w w]",
        "[WireData.nosuch WireData.w WireData.w]",
        "[WireData.w w WireData.w]",
        "[WireData.w Foo.w WireData.w]",
    ] {
        dss.command(&format!("Edit LineGeometry.gd conductors={list}"));
        assert_eq!(ratings(&mut dss), ["530", "800"], "{list}");
        assert_eq!(
            query(&mut dss, "LineGeometry.gd.wires"),
            "[, w2, ]",
            "{list}"
        );
    }
    assert_eq!(
        errors(&dss),
        [
            "Line.lq.Conductors: You must define the Conductor class for all the valid items \
             in the array.",
            "Line.lf.Conductors: WireData object \"WireData.nosuch\" not found.",
            "Line.lv.Conductors: Invalid class (foo) for item. Valid classes: \
             (WireData|CNData|TSData)",
            "LineGeometry.gq.Conductors: You must define the Conductor class for all the valid \
             items in the array.",
            "LineGeometry.gd.Conductors: You must define the Conductor class for all the valid \
             items in the array.",
            "LineGeometry.gd.Conductors: WireData object \"WireData.nosuch\" not found.",
            "LineGeometry.gd.Conductors: You must define the Conductor class for all the valid \
             items in the array.",
            "LineGeometry.gd.Conductors: Invalid class (foo) for item. Valid classes: \
             (WireData|CNData|TSData)",
        ]
    );
    let codes: Vec<Option<u32>> = dss.errors().iter().map(|d| d.code).collect();
    assert_eq!(
        codes,
        [
            Some(10103),
            None,
            Some(10103),
            Some(10103),
            Some(10103),
            None,
            Some(10103),
            Some(10103)
        ]
    );
    for line in ["Lq", "Lf", "Lv"] {
        assert_eq!(
            query(&mut dss, &format!("Line.{line}.wires")),
            "[w, w, w]",
            "{line}"
        );
    }
    for c in ["Set voltagebases=[12.47]", "CalcVoltageBases", "Solve"] {
        dss.command(c);
    }
    assert!(solved(&dss));
    assert_eq!(errors(&dss).len(), 8, "{:?}", errors(&dss));
    for real in [false, true] {
        for line in ["Lq", "Lf", "Lv"] {
            assert_eq!(
                zpart(&dss, line, real),
                zpart(&dss, "Lw", real),
                "{line} {real}"
            );
        }
    }
}

/// A list refused because an object is not found, and a `cncables=` or
/// `tscables=` list on a Line with no conductor position, change nothing
/// either: the list writes nothing, enters no set order and runs no side
/// effect, on the Line and on a LineGeometry, whose `wires=` shares the
/// parse. `Ng` keeps its geometry and solves like `Ngg`, at X11
/// 1.42425544092922 Ω/mi, and `Nl` keeps its line code and solves like
/// `Nl0`. `Nr` keeps its wires and its typed `normamps` and `emergamps`
/// through the solve, the Save and the JSON export. `Kg` exports no conductor
/// list, so its JSON record imports back with no message. `Kc`, `Kt` and `Kl`
/// keep their geometry and line code (402, as a `conductors=` list with no
/// position). `Kn`'s cables are not found, so no conductor model is set and
/// its overhead wires fill the spacing like `Lw`'s. `gw` keeps its wires and
/// the zero ratings typed after them, which a conductor would fill, and its
/// JSON record writes no `Wires`. The pinned dss_capi refuses these lists
/// (40303 for a wire not found, 402 for `Kc`, `Kt` and `Kl`) and keeps what
/// was there: `Ng`, `Kg` and `Kt` keep `g3`, `Nl` and `Kl` keep `lc`, `Nr`
/// reads 500, 700 and `[w, w, w]` after the solve, and `gw` its ratings and
/// wires. It refuses `Kn`'s wires "expected 0 objects" after its cables are
/// not found and faults at `CalcVoltageBases`. r4133 logs 18103 for the
/// wires of `Ng` and `Nl` (none for `Nr`'s), keeps them by name (`? wires`
/// reads `nosuch nosuch nosuch`) and drops the geometry and the line code,
/// so `Ng` and `Nl` fault at `CalcVoltageBases` and `Solve` with an access
/// violation, where the same Lines without the list solve. So does `Kn`,
/// whose cables it first logs as not defined (18105) and keeps by name
/// (`? cncables` reads `nosuch nosuch nosuch`). `Nr` reads -1 for both ratings
/// after the solve. r4133 faults on the `New` line of `Kg`, `Kc`, `Kt` and
/// `Kl`, which read no geometry or line code after it (measured 2026-10-23
/// with the epri-worker on the r4133 DLL and the pinned dss-python, one
/// process per Line).
#[test]
fn a_missing_object_or_a_cable_list_with_no_position_changes_nothing() {
    use crate::report::export::json::JsonOpts;
    let wire = "New WireData.w GMRac=0.0244 GMRunits=ft Rac=0.306 Runits=mi Diam=0.721 \
                Radunits=in";
    let geometry = "New LineGeometry.g3 nconds=3 nphases=3 units=ft cond=1 wire=w x=-4 h=28 \
                    cond=2 wire=w x=0 h=28 cond=3 wire=w x=4 h=28";
    let mut dss = Dss::new();
    for c in [
        "New Circuit.missing basekv=12.47 phases=3 bus1=src",
        wire,
        "New WireData.w2 GMRac=0.03 GMRunits=ft Rac=0.2 Runits=mi Diam=0.8 Radunits=in \
         normamps=530 emergamps=800",
        "New CNData.cn k=13 DiaStrand=0.064 Rstrand=2.816666 epsR=2.3 InsLayer=0.220 \
         DiaIns=1.06 DiaCable=1.16 Rac=0.076705 GMRac=0.20568 diam=0.573 Runits=kft \
         Radunits=in GMRunits=in",
        "New TSData.ts DiaShield=0.88 TapeLayer=0.005 TapeLap=20 EpsR=2.3 InsLayer=0.22 \
         DiaIns=0.8 DiaCable=0.95 Rac=0.97 Runits=mi GMRac=0.1332 GMRunits=in Diam=0.368 \
         Radunits=in",
        "New LineSpacing.s3 nconds=3 nphases=3 x=[-4 0 4] h=[28 28 28] units=ft",
        geometry,
        "New LineCode.lc nphases=3 r1=0.3 x1=0.6 r0=0.3 x0=0.6 c1=0 c0=0 units=km",
        "New LineGeometry.gw nconds=3 nphases=3 units=ft cond=1 wire=w2 x=-4 h=28 cond=2 \
         wire=w2 x=0 h=28 cond=3 wire=w2 x=4 h=28 normamps=0 emergamps=0",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", errors(&dss));
    for c in [
        "New Line.Ng bus1=src.1.2.3 bus2=ng.1.2.3 spacing=s3 geometry=g3 \
         wires=[nosuch nosuch nosuch] length=1 units=mi",
        "New Line.Ngg bus1=src.1.2.3 bus2=ngg.1.2.3 geometry=g3 length=1 units=mi",
        "New Line.Nl bus1=src.1.2.3 bus2=nl.1.2.3 spacing=s3 linecode=lc \
         wires=[nosuch nosuch nosuch] length=1 units=km",
        "New Line.Nl0 bus1=src.1.2.3 bus2=nl0.1.2.3 spacing=s3 linecode=lc length=1 units=km",
        "New Line.Nr bus1=src.1.2.3 bus2=nr.1.2.3 spacing=s3 wires=[w w w] normamps=500 \
         emergamps=700 length=1 units=mi",
        "Edit Line.Nr wires=[w nosuch w]",
        "New Line.Kg bus1=src.1.2.3 bus2=kg.1.2.3 geometry=g3 wires=[nosuch nosuch nosuch] \
         length=1 units=mi",
        "New Line.Kc bus1=src.1.2.3 bus2=kc.1.2.3 geometry=g3 cncables=[cn cn cn] length=1 \
         units=mi",
        "New Line.Kt bus1=src.1.2.3 bus2=kt.1.2.3 geometry=g3 tscables=[ts ts ts] length=1 \
         units=mi",
        "New Line.Kl bus1=src.1.2.3 bus2=kl.1.2.3 linecode=lc cncables=[cn cn cn] length=1 \
         units=km",
        "New Line.Kl0 bus1=src.1.2.3 bus2=kl0.1.2.3 linecode=lc length=1 units=km",
        "New Line.Kn bus1=src.1.2.3 bus2=kn.1.2.3 spacing=s3 cncables=[nosuch nosuch nosuch] \
         wires=[w w w] length=1 units=mi",
        "New Line.Lw bus1=src.1.2.3 bus2=lw.1.2.3 spacing=s3 wires=[w w w] length=1 units=mi",
        "Edit LineGeometry.gw wires=[w nosuch w]",
    ] {
        dss.command(c);
    }
    let not_found =
        |line: &str| format!("Line.{line}.Wires: WireData object \"nosuch\" not found.");
    let no_position = |line: &str, prop: &str| {
        format!(
            "Line.{line}.{prop}: No objects are expected! Check if the order of property \
             assignments is correct."
        )
    };
    assert_eq!(
        errors(&dss),
        [
            not_found("ng"),
            not_found("nl"),
            not_found("nr"),
            not_found("kg"),
            no_position("kc", "CNCables"),
            no_position("kt", "TSCables"),
            no_position("kl", "CNCables"),
            "Line.kn.CNCables: CNData object \"nosuch\" not found.".to_string(),
            "LineGeometry.gw.Wires: WireData object \"nosuch\" not found.".to_string(),
        ]
    );
    let codes: Vec<Option<u32>> = dss.errors().iter().map(|d| d.code).collect();
    assert_eq!(codes[4..7], [Some(402); 3]);
    for (prop, want) in [
        ("Line.Ng.geometry", "g3"),
        ("Line.Nl.linecode", "lc"),
        ("Line.Nr.wires", "[w, w, w]"),
        ("Line.Kg.geometry", "g3"),
        ("Line.Kc.geometry", "g3"),
        ("Line.Kt.geometry", "g3"),
        ("Line.Kl.linecode", "lc"),
        ("Line.Kn.wires", "[w, w, w]"),
        ("LineGeometry.gw.wires", "[w2, w2, w2]"),
        ("LineGeometry.gw.normamps", "none"),
        ("LineGeometry.gw.emergamps", "none"),
    ] {
        assert_eq!(query(&mut dss, prop), want, "{prop}");
    }

    // The set order: `Kg` exports no conductor list and imports back clean.
    let kg = dss.obj_to_json("Line.Kg", JsonOpts::NONE).expect("Kg");
    assert!(!kg.contains("\"Conductors\""), "{kg}");
    let json =
        format!(r#"{{"Name":"missingjson","PreCommands":["{wire}","{geometry}"],"Line":[{kg}]}}"#);
    let mut back = Dss::new();
    assert_eq!(back.circuit_from_json(&json), Ok(()));
    assert!(back.errors().is_empty(), "{:?}", errors(&back));
    assert_eq!(query(&mut back, "Line.Kg.geometry"), "g3");

    for c in ["Set voltagebases=[12.47]", "CalcVoltageBases", "Solve"] {
        dss.command(c);
    }
    assert!(solved(&dss));
    assert_eq!(errors(&dss).len(), 9, "{:?}", errors(&dss));
    for real in [false, true] {
        for (line, like) in [
            ("Ng", "Ngg"),
            ("Kg", "Ngg"),
            ("Kc", "Ngg"),
            ("Kt", "Ngg"),
            ("Nl", "Nl0"),
            ("Kl", "Kl0"),
            ("Kn", "Lw"),
        ] {
            assert_eq!(
                zpart(&dss, line, real),
                zpart(&dss, like, real),
                "{line} {real}"
            );
        }
    }
    let x11 = xmatrix(&dss, "Ng")[0][0];
    assert!((x11 - 1.424_255_440_929_22).abs() <= X_BAND, "{x11}");

    let gw = dss
        .obj_to_json("LineGeometry.gw", JsonOpts::NONE)
        .expect("gw");
    assert!(!gw.contains("\"Wires\""), "{gw}");

    let scratch = Scratch::new("missing_objects");
    let dir = scratch.dir("save");
    dss.command(&format!("save circuit dir=\"{}\"", slash(&dir)));
    assert_eq!(errors(&dss).len(), 9, "{:?}", errors(&dss));
    let saved = read(&dir, "Line.dss");
    let row = "New \"Line.nr\" Bus1=src.1.2.3 Bus2=nr.1.2.3 Spacing=s3 Wires=[w, w, w] \
               NormAmps=500 EmergAmps=700 Length=1 Units=mi";
    assert!(saved.lines().any(|l| l == row), "{row} in {saved}");
    for (prop, want) in [("normamps", "500"), ("emergamps", "700")] {
        assert_eq!(query(&mut dss, &format!("Line.Nr.{prop}")), want, "{prop}");
    }
    let record = dss.obj_to_json("Line.Nr", JsonOpts::NONE).expect("Nr");
    for (key, want) in [("NormAmps", 500.0), ("EmergAmps", 700.0)] {
        let at = record
            .find(&format!("\"{key}\":"))
            .unwrap_or_else(|| panic!("no {key} in {record}"))
            + key.len()
            + 3;
        let text = record[at..].split([',', '}']).next().unwrap_or_default();
        assert_eq!(text.parse::<f64>(), Ok(want), "{key} in {record}");
    }
}

/// A Line command logs its refusals in token order, each at its token. `Kw`'s
/// `cncables=` comes before its spacing, so it is refused (402) and the
/// overhead wires after the spacing fill its three positions like `Lw`'s.
/// `Kw2` gives two wires for those three positions, refused after the cables.
/// In `Kp`'s edit the phase change a geometry Line refuses comes before the
/// wires it refuses for want of a spacing. The pinned dss_capi drops the rest
/// of the `Kw` command at its 402 (`? spacing` `''`, `units` none) and r4133
/// faults on it with an access violation (measured 2026-10-23 with the
/// epri-worker on the r4133 DLL and the pinned dss-python).
#[test]
fn a_line_logs_its_refusals_in_token_order() {
    let mut dss = Dss::new();
    for c in [
        "New Circuit.order basekv=12.47 phases=3 bus1=src",
        "New WireData.w GMRac=0.0244 GMRunits=ft Rac=0.306 Runits=mi Diam=0.721 Radunits=in",
        "New CNData.cn k=13 DiaStrand=0.064 Rstrand=2.816666 epsR=2.3 InsLayer=0.220 \
         DiaIns=1.06 DiaCable=1.16 Rac=0.076705 GMRac=0.20568 diam=0.573 Runits=kft \
         Radunits=in GMRunits=in",
        "New LineSpacing.s3 nconds=3 nphases=3 x=[-4 0 4] h=[28 28 28] units=ft",
        "New LineGeometry.g3 nconds=3 nphases=3 units=ft cond=1 wire=w x=-4 h=28 cond=2 \
         wire=w x=0 h=28 cond=3 wire=w x=4 h=28",
        "New Line.Kp bus1=src.1.2.3 bus2=kp.1.2.3 geometry=g3 length=1 units=mi",
        "New Line.Lw bus1=src.1.2.3 bus2=lw.1.2.3 spacing=s3 wires=[w w w] length=1 units=mi",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", errors(&dss));
    for c in [
        "New Line.Kw bus1=src.1.2.3 bus2=kw.1.2.3 cncables=[cn cn cn] spacing=s3 \
         wires=[w w w] length=1 units=mi",
        "New Line.Kw2 bus1=src.1.2.3 bus2=kw2.1.2.3 cncables=[cn cn cn] spacing=s3 \
         wires=[w w] length=1 units=mi",
    ] {
        dss.command(c);
    }
    let cables = |line: &str| {
        format!(
            "Line.{line}.CNCables: No objects are expected! Check if the order of property \
             assignments is correct."
        )
    };
    assert_eq!(
        errors(&dss),
        [
            cables("kw"),
            cables("kw2"),
            "Line.kw2: Unexpected number (2) of wires; expected 3 objects.".to_string(),
        ]
    );
    assert_eq!(query(&mut dss, "Line.Kw.wires"), "[w, w, w]");

    let before = dss.errors().len();
    let cmd = "Edit Line.Kp phases=2 wires=[w]";
    dss.command(cmd);
    assert_eq!(
        errors(&dss)[before..],
        [
            "Illegal change of number of phases for \"Line.kp\"",
            "You must assign the LineSpacing before the Wires Property (\"Line.kp\")."
        ]
    );
    let span = dss.errors()[before].span.as_ref().expect("a span");
    assert_eq!(&cmd[span.offset()..span.offset() + span.len()], "2");
    for (prop, want) in [("phases", "3"), ("geometry", "g3")] {
        assert_eq!(query(&mut dss, &format!("Line.Kp.{prop}")), want, "{prop}");
    }

    for c in ["Set voltagebases=[12.47]", "CalcVoltageBases", "Solve"] {
        dss.command(c);
    }
    assert!(solved(&dss));
    assert_eq!(errors(&dss).len(), before + 2, "{:?}", errors(&dss));
    for real in [false, true] {
        assert_eq!(zpart(&dss, "Kw", real), zpart(&dss, "Lw", real), "{real}");
    }
}

/// A JSON record logs a Line's queued message after the object, while the
/// command logs it at its token. `Line.cl` on a three-phase line code asks for
/// `phases=2`, which the Line refuses through the queue, and `normamps=abc`,
/// refused at once. The command logs the phase refusal first, in token order,
/// and the JSON record of the same Line logs it behind the `NormAmps`
/// refusal. A command of another class logs a queued message after the edit:
/// a one-phase LineCode's `kron=y` is refused behind the `normamps=abc` that
/// follows it. Both Lines keep their 3 phases.
#[test]
fn a_json_record_logs_a_queued_message_after_the_object() {
    let lcm = "New LineCode.lcm nphases=3 rmatrix=[0.3 |0.1 0.3 |0.1 0.1 0.3] \
               xmatrix=[0.6 |0.2 0.6 |0.2 0.2 0.6] units=km";
    let phases = "Illegal change of number of phases for \"Line.cl\"";
    let amps = "Invalid inline math entry: \"abc\"";
    let mut cmd = Dss::new();
    for c in [
        "New Circuit.qo basekv=12.47 phases=3 bus1=src",
        lcm,
        "New Line.cl bus1=src.1.2.3 bus2=b.1.2.3 linecode=lcm phases=2 normamps=abc length=1 \
         units=km",
    ] {
        cmd.command(c);
    }
    assert_eq!(errors(&cmd), [phases, amps]);
    assert_eq!(query(&mut cmd, "Line.cl.phases"), "3");

    let json = format!(
        r#"{{"Name":"qo","PreCommands":["{lcm}"],"Line":[{{"Name":"cl","Bus1":"src.1.2.3","Bus2":"b.1.2.3","LineCode":"lcm","Phases":2,"NormAmps":"abc","Length":1,"Units":"km"}}]}}"#
    );
    let mut record = Dss::new();
    assert_eq!(record.circuit_from_json(&json), Ok(()));
    assert_eq!(errors(&record), [amps, phases]);
    assert_eq!(query(&mut record, "Line.cl.phases"), "3");

    let mut code = Dss::new();
    for c in [
        "New Circuit.kr basekv=12.47 phases=3 bus1=src",
        "New LineCode.k1 nphases=1 rmatrix=[0.1] xmatrix=[0.2] cmatrix=[3] kron=y normamps=abc",
    ] {
        code.command(c);
    }
    assert_eq!(
        errors(&code),
        [
            amps,
            "Cannot perform Kron Reduction on a 1-phase LineCode: LineCode.k1"
        ]
    );
}

/// A LineSpacing coordinate list holds the values given, in order: a
/// conductor the list does not reach has no position (`short` gives two x for
/// three conductors, `one` a height and no x, `noh` no height, and `r` a full
/// x list replaced by a list of one). `?` reads the values given, `''` when
/// none. A geometry or a Line built on the spacing copies each gap, so rule 7
/// names the conductor at calculation. r4133 solves every one in silence, on
/// memory nobody wrote (on `r`, the rest of the earlier list). The pinned
/// dss_capi places a missing coordinate at 0: it solves `short` and `one` in
/// silence and refuses `noh` (a zero height) and `r` (two conductors at one
/// point). The geometry reads `x` empty for the conductor and its Save writes
/// no `X=` for it. The spacing's Save, JSON records (default and full) and CIM
/// `WirePosition`s write only the values given, and the Save and the default
/// record reload to the same gaps. An empty JSON `X` list gives no x either.
/// With the gaps filled every Line solves at the simple Carson reactance of the
/// positions given ([`carson_x`]), and the equivalent spacing `eq`, which holds
/// no position, at its distances ([`carson_x_equivalent`]).
#[test]
fn a_short_spacing_list_keeps_its_gaps() {
    use crate::report::export::json::JsonOpts;
    let scratch = Scratch::new("short_lists");
    let mut dss = compile("ls2_short_spacing_lists.dss");
    let all = "LineGeometry.gs: conductor 3 has no x. Line.ls (spacing=short): conductor 3 has \
               no x. LineGeometry.g1: conductor 1 has no x. Line.lo (spacing=one): conductor 1 \
               has no x. LineGeometry.gh: conductors 1, 2 and 3 have no h. Line.lsh \
               (spacing=noh): conductors 1, 2 and 3 have no h. LineGeometry.gr: conductors 2 \
               and 3 have no x.";
    assert_eq!(
        errors(&dss),
        [format!("Error Encountered in CalcVoltageBases: {all}")]
    );
    assert!(!solved(&dss));

    for (prop, want) in [
        ("short.x", "[ -4 4]"),
        ("short.h", "[ 28 28 28]"),
        ("one.x", ""),
        ("one.h", "[ 28]"),
        ("noh.x", "[ -4 0 4]"),
        ("noh.h", ""),
        ("r.x", "[ 9]"),
        ("r.h", "[ 28 28 28]"),
        ("eq.x", ""),
        ("eq.h", ""),
    ] {
        assert_eq!(
            query(&mut dss, &format!("LineSpacing.{prop}")),
            want,
            "{prop}"
        );
    }
    let gs = conductors(&dss, "gs");
    assert_eq!((gs[2].x, gs[2].h), (None, Some(28.0)));
    let gr = conductors(&dss, "gr");
    assert_eq!(
        gr.iter().map(|c| c.x).collect::<Vec<_>>(),
        [Some(9.0), None, None]
    );
    dss.command("Edit LineGeometry.gs cond=3");
    assert_eq!(query(&mut dss, "LineGeometry.gs.x"), "");
    assert_eq!(query(&mut dss, "LineGeometry.gs.h"), "28");

    // Save: the geometry writes no x for conductor 3, the spacings only the
    // values given, and the reload holds the same gaps.
    let dir = scratch.dir("save");
    let (rows, _) = saved_rows(&save(&mut dss, &dir, "gs"));
    assert_eq!(rows[2][2], "", "{rows:?}");
    assert_eq!(rows[2][3], "h=28", "{rows:?}");
    let spacings = read(&dir, "LineSpacing.dss");
    let saved = |name: &str| -> String {
        let head = format!("New \"LineSpacing.{name}\"");
        spacings
            .lines()
            .find(|l| l.starts_with(&head))
            .unwrap_or_else(|| panic!("no {head} line in {spacings}"))
            .to_string()
    };
    assert!(saved("short").contains(" X=[ -4 4] "), "{}", saved("short"));
    assert!(!saved("one").contains(" X="), "{}", saved("one"));
    assert!(!saved("noh").contains(" H="), "{}", saved("noh"));
    assert!(!saved("eq").contains(" X=") && !saved("eq").contains(" H="));
    let mut back = reload_unsolved(&dir);
    for (prop, want) in [
        ("short.x", "[ -4 4]"),
        ("one.x", ""),
        ("noh.h", ""),
        ("r.x", "[ 9]"),
    ] {
        assert_eq!(
            query(&mut back, &format!("LineSpacing.{prop}")),
            want,
            "reload {prop}"
        );
    }

    // JSON: the record holds the values given and imports back with the gaps.
    // The full record leaves out a list with no value given too.
    for (name, x, h) in [
        ("short", "[ -4 4]", "[ 28 28 28]"),
        ("one", "", "[ 28]"),
        ("noh", "[ -4 0 4]", ""),
    ] {
        let full = dss
            .obj_to_json(&format!("LineSpacing.{name}"), JsonOpts::FULL)
            .expect("a spacing");
        assert_eq!(full.contains("\"X\""), !x.is_empty(), "{full}");
        assert_eq!(full.contains("\"H\""), !h.is_empty(), "{full}");
        let record = dss
            .obj_to_json(&format!("LineSpacing.{name}"), JsonOpts::NONE)
            .expect("a spacing");
        assert!(
            record.starts_with(&format!(r#"{{"Name":"{name}","NConds":"#)),
            "{record}"
        );
        assert_eq!(record.contains("\"X\""), !x.is_empty(), "{record}");
        assert_eq!(record.contains("\"H\""), !h.is_empty(), "{record}");
        let json = format!(r#"{{"Name":"lgjson","LineSpacing":[{record}]}}"#);
        let mut imported = Dss::new();
        assert_eq!(imported.circuit_from_json(&json), Ok(()), "{record}");
        assert!(
            imported.errors().is_empty(),
            "{record}: {:?}",
            errors(&imported)
        );
        assert_eq!(
            query(&mut imported, &format!("LineSpacing.{name}.x")),
            x,
            "{record}"
        );
        assert_eq!(
            query(&mut imported, &format!("LineSpacing.{name}.h")),
            h,
            "{record}"
        );
        assert_eq!(
            query(&mut imported, &format!("LineSpacing.{name}.nconds")),
            query(&mut dss, &format!("LineSpacing.{name}.nconds")),
            "{record}"
        );
    }
    // An empty JSON list gives no coordinate: `?` reads empty and neither
    // record writes an `X`, though the list is in the set order.
    let mut cleared = Dss::new();
    let json = r#"{"Name":"lgcleared","LineSpacing":[
        {"Name":"z","NConds":3,"NPhases":3,"X":[],"H":[28,28,28],"Units":"ft"}]}"#;
    assert_eq!(cleared.circuit_from_json(json), Ok(()));
    assert!(cleared.errors().is_empty(), "{:?}", errors(&cleared));
    assert_eq!(query(&mut cleared, "LineSpacing.z.x"), "");
    for opts in [JsonOpts::NONE, JsonOpts::FULL] {
        let record = cleared.obj_to_json("LineSpacing.z", opts).expect("z");
        let z: serde_json::Value = serde_json::from_str(&record).expect("a JSON record");
        assert!(z.get("X").is_none(), "{opts:?}: {record}");
        assert_eq!(
            z["H"],
            serde_json::json!([28.0, 28.0, 28.0]),
            "{opts:?}: {record}"
        );
    }
    let short: serde_json::Value = serde_json::from_str(
        &dss.obj_to_json("LineSpacing.short", JsonOpts::NONE)
            .expect("short"),
    )
    .expect("a JSON record");
    assert_eq!(short["NConds"], serde_json::json!(3), "{short}");
    assert_eq!(short["X"], serde_json::json!([-4.0, 4.0]), "{short}");

    // The gaps filled and each Line given its geometry again, every Line
    // solves.
    for c in [
        "Edit LineGeometry.gs cond=3 x=0",
        "Edit LineGeometry.g1 cond=1 x=0",
        "Edit LineGeometry.gh cond=1 h=28 cond=2 h=28 cond=3 h=28",
        "Edit LineGeometry.gr cond=2 x=13 cond=3 x=17",
        "Edit Line.Lg geometry=gs",
        "Edit Line.L1 geometry=g1",
        "Edit Line.Lh geometry=gh",
        "Edit Line.Lr geometry=gr",
        "Edit Line.Ls geometry=gf",
        "Edit Line.Lo geometry=g1",
        "Edit Line.Lsh geometry=gf",
        "Solve",
    ] {
        dss.command(c);
    }
    assert_eq!(dss.errors().len(), 1, "{:?}", errors(&dss));
    assert!(solved(&dss));
    let row = |a: f64, b: f64, c: f64| {
        [
            phase(a, 28.0, "ft"),
            phase(b, 28.0, "ft"),
            phase(c, 28.0, "ft"),
        ]
    };
    let four_ft = 0.794_511_1;
    assert_x(
        &dss,
        "Lg",
        &row(-4.0, 4.0, 0.0),
        &[(3, 1, four_ft), (3, 2, four_ft)],
        "gs",
    );
    for line in ["Lf", "Ls", "Lh", "Lsh"] {
        assert_x(&dss, line, &row(-4.0, 0.0, 4.0), &[(2, 1, four_ft)], line);
    }
    assert_x(&dss, "Lr", &row(9.0, 13.0, 17.0), &[(2, 1, four_ft)], "gr");
    for line in ["L1", "Lo"] {
        assert_x(&dss, line, &[phase(0.0, 28.0, "ft")], &[], line);
    }
    let eq = carson_x_equivalent(&[GMR_P, GMR_P, GMR_P, GMR_N], 3, 1.2, 1.5);
    assert_x_is(&dss, "La", &eq, &[(2, 1, 0.796_437_2)], "ga");

    // CIM: a coordinate never given is not written, and `isCable` only from a
    // first height given.
    dss.command(&format!("Set DataPath=\"{}\"", slash(&scratch.dir("cim"))));
    dss.command("Export CIM100");
    assert_eq!(dss.errors().len(), 1, "{:?}", errors(&dss));
    let xml = read(&scratch.0.join("cim"), "lgrules_CIM100x.xml");
    let block = |tag: &str, name: &str| -> String {
        xml.split(&format!("<cim:{tag} "))
            .find(|b| b.contains(&format!(">{name}<")))
            .and_then(|b| b.split(&format!("</cim:{tag}>")).next())
            .unwrap_or_else(|| panic!("no {tag} {name}"))
            .to_string()
    };
    let (x, y) = ("<cim:WirePosition.xCoord>", "<cim:WirePosition.yCoord>");
    let cable = "<cim:WireSpacingInfo.isCable>";
    let short3 = block("WirePosition", "WP_short_3");
    assert!(
        !short3.contains(x) && short3.contains(&format!("{y}8.5344<")),
        "{short3}"
    );
    let one1 = block("WirePosition", "WP_one_1");
    assert!(!one1.contains(x) && one1.contains(y), "{one1}");
    for i in 1..=3 {
        let noh = block("WirePosition", &format!("WP_noh_{i}"));
        assert!(noh.contains(x) && !noh.contains(y), "{noh}");
    }
    assert!(!block("WireSpacingInfo", "noh").contains(cable));
    assert!(block("WireSpacingInfo", "short").contains(&format!("{cable}false<")));
}

/// An empty value inside a LineSpacing coordinate list is refused, naming its
/// place, and the spacing keeps the list it had: a gap in the middle has no
/// notation in `?`, Save or JSON. A trailing comma ends the list. A `null` or
/// a blank string inside a list of the JSON import is the same empty value,
/// refused with the same message, so no later position moves up into its
/// place, and a trailing `null` or blank string ends the list. Each JSON item
/// is one position: a string holding one number is that number, and an item
/// holding several values or none (`"0 4"`, `"1,2"`, a list, an object,
/// `true`) is refused, naming its place, where a split or dropped item would
/// move the later values to other conductors. Only a list is a list: an
/// object, `true`, `false`, a number or a string as the whole value is
/// refused. A record without `NConds` meets the refusal before `nconds=`
/// first. The pinned dss_capi refuses each of these too ("Invalid float value
/// : 0 4", "Cannot convert data from object value", "Expected an array of
/// numbers, not an object", and 5021 "Invalid type cast" for `true`, `false`,
/// `4` and `"1 2 3"` as the whole value) except the item `true`, which it
/// reads as 1. It also refuses a trailing blank string, 5021 "Invalid float
/// value", which the end-of-list rule drops here like a trailing `null`
/// (measured 2026-10-23 with the pinned dss-python).
#[test]
fn a_spacing_list_with_an_empty_value_is_refused() {
    let mut dss = Dss::new();
    dss.command("new circuit.lgempty");
    dss.command("New LineSpacing.d nconds=3 nphases=3 x=[-4 0 4] h=[28 28 28] units=ft");
    dss.command("Edit LineSpacing.d x=[1,,3]");
    dss.command("Edit LineSpacing.d h=[ ,28 28]");
    assert_eq!(
        errors(&dss),
        [
            "LineSpacing.d.X: value 2 of the list is empty, so the list is not applied.",
            "LineSpacing.d.H: value 1 of the list is empty, so the list is not applied."
        ]
    );
    assert_eq!(query(&mut dss, "LineSpacing.d.x"), "[ -4 0 4]");
    assert_eq!(query(&mut dss, "LineSpacing.d.h"), "[ 28 28 28]");
    dss.command("Edit LineSpacing.d x=[1 2,]");
    assert_eq!(dss.errors().len(), 2, "{:?}", errors(&dss));
    assert_eq!(query(&mut dss, "LineSpacing.d.x"), "[ 1 2]");

    let mut imported = Dss::new();
    let json = r#"{"Name":"lgemptyjson","LineSpacing":[
        {"Name":"j","NConds":3,"NPhases":3,"X":[-4,null,4],"H":[null,28,28],"Units":"ft"}]}"#;
    assert_eq!(imported.circuit_from_json(json), Ok(()));
    assert_eq!(
        errors(&imported),
        [
            "LineSpacing.j.X: value 2 of the list is empty, so the list is not applied.",
            "LineSpacing.j.H: value 1 of the list is empty, so the list is not applied."
        ]
    );
    assert_eq!(query(&mut imported, "LineSpacing.j.x"), "");
    assert_eq!(query(&mut imported, "LineSpacing.j.h"), "");

    let mut trailing = Dss::new();
    let json = r#"{"Name":"lgemptyjson","LineSpacing":[
        {"Name":"t","NConds":3,"NPhases":3,"X":[-4,0,null],"H":[28,28,28],"Units":"ft"}]}"#;
    assert_eq!(trailing.circuit_from_json(json), Ok(()));
    assert!(trailing.errors().is_empty(), "{:?}", errors(&trailing));
    assert_eq!(query(&mut trailing, "LineSpacing.t.x"), "[ -4 0]");
    assert_eq!(query(&mut trailing, "LineSpacing.t.h"), "[ 28 28 28]");

    let import_x = |x: &str| -> Dss {
        let mut dss = Dss::new();
        let json = format!(
            r#"{{"Name":"lgitems","LineSpacing":[
            {{"Name":"i","NConds":3,"NPhases":3,"X":{x},"H":[28,28,28],"Units":"ft"}}]}}"#
        );
        assert_eq!(dss.circuit_from_json(&json), Ok(()), "{x}");
        dss
    };
    for (x, why) in [
        (r#"[-4,"0 4",8]"#, "value 2 of the list is not a number"),
        (r#"["1,2",3]"#, "value 1 of the list is not a number"),
        (r#"[1,"2 3"]"#, "value 2 of the list is not a number"),
        (r#"[-4,"abc",4]"#, "value 2 of the list is not a number"),
        (r#"[-4,0,{"a":1}]"#, "value 3 of the list is not a number"),
        (r#"[[1],0,4]"#, "value 1 of the list is not a number"),
        (r#"[true,0,4]"#, "value 1 of the list is not a number"),
        (r#"{"a":1}"#, "the value is not a list of numbers"),
        ("true", "the value is not a list of numbers"),
        ("false", "the value is not a list of numbers"),
        ("4", "the value is not a list of numbers"),
        (r#""1 2 3""#, "the value is not a list of numbers"),
    ] {
        let mut dss = import_x(x);
        assert_eq!(
            errors(&dss),
            [format!(
                "LineSpacing.i.X: {why}, so the list is not applied."
            )],
            "{x}"
        );
        assert_eq!(query(&mut dss, "LineSpacing.i.x"), "", "{x}");
        assert_eq!(query(&mut dss, "LineSpacing.i.h"), "[ 28 28 28]", "{x}");
    }
    let mut blank = import_x(r#"[-4," ",4]"#);
    assert_eq!(
        errors(&blank),
        ["LineSpacing.i.X: value 2 of the list is empty, so the list is not applied."]
    );
    assert_eq!(query(&mut blank, "LineSpacing.i.x"), "");
    assert_eq!(query(&mut blank, "LineSpacing.i.h"), "[ 28 28 28]");
    let mut trailing_blank = import_x(r#"[-4,0," "]"#);
    assert!(
        trailing_blank.errors().is_empty(),
        "{:?}",
        errors(&trailing_blank)
    );
    assert_eq!(query(&mut trailing_blank, "LineSpacing.i.x"), "[ -4 0]");
    assert_eq!(query(&mut trailing_blank, "LineSpacing.i.h"), "[ 28 28 28]");
    let mut numbers = import_x(r#"["-4"," 0 ","1e1"]"#);
    assert!(numbers.errors().is_empty(), "{:?}", errors(&numbers));
    assert_eq!(query(&mut numbers, "LineSpacing.i.x"), "[ -4 0 10]");

    let mut no_count = Dss::new();
    let json = r#"{"Name":"lgitems","LineSpacing":[{"Name":"i","X":[-4,"0 4",8]}]}"#;
    assert_eq!(no_count.circuit_from_json(json), Ok(()));
    assert_eq!(errors(&no_count), [spacing_before_nconds("i", "X")]);
    assert_eq!(query(&mut no_count, "LineSpacing.i.x"), "");
}

/// A LineSpacing coordinate is a finite length, so a value that is infinite,
/// NaN or past the f64 range is refused, naming its place, and the spacing
/// keeps the list it had: an infinite position places no conductor, and
/// every distance to another conductor and every mutual reactance on it is
/// infinite. The text form refuses `1e400`, `inf` and `nan` in `x=` and
/// `h=`, and the JSON import refuses the items `"inf"`, `"NaN"`, `"1e400"`
/// and the number `1e400`, naming such an item ahead of an empty item before
/// it (`[-4,null,1e400]`). The pinned
/// dss_capi stores each of them with no message: the four JSON items read
/// `[ -4 +Inf 4]`, `[ -4 Nan 4]`, `[ -4 +Inf 4]` and `[ -4 +Inf 4]`, and the
/// text `x=[-4 1e400 4]`, `inf` and `nan` read `[ -4 +Inf 4]`, `[ -4 +Inf 4]`
/// and `[ -4 Nan 4]`. r4133 stops the command at the text `inf` or `nan` with
/// 303, a floating-point conversion error, keeping the values before it (a
/// new spacing given `x=[-4 inf 4] h=[28 28 28]` reads `x` `[-4,0,0]` and `h`
/// `[0,0,0]`), and stores `1e400` as `[-4,INF,4]` (measured 2026-10-23 with
/// the epri-worker on the r4133 DLL and the pinned dss-python).
#[test]
fn a_spacing_coordinate_is_a_finite_length() {
    let mut dss = Dss::new();
    dss.command("new circuit.lgfinite");
    dss.command("New LineSpacing.t nconds=3 nphases=3 x=[-4 0 4] h=[28 28 28] units=ft");
    assert!(dss.errors().is_empty(), "{:?}", errors(&dss));
    for (edit, prop, at) in [
        ("x=[-4 1e400 4]", "X", 2),
        ("x=[-4 inf 4]", "X", 2),
        ("x=[-4 nan 4]", "X", 2),
        ("h=[28 28 1e400]", "H", 3),
    ] {
        let before = dss.errors().len();
        dss.command(&format!("Edit LineSpacing.t {edit}"));
        assert_eq!(
            errors(&dss)[before..],
            [format!(
                "LineSpacing.t.{prop}: value {at} of the list is not a finite number, so the \
                 list is not applied."
            )],
            "{edit}"
        );
        assert_eq!(query(&mut dss, "LineSpacing.t.x"), "[ -4 0 4]", "{edit}");
        assert_eq!(query(&mut dss, "LineSpacing.t.h"), "[ 28 28 28]", "{edit}");
    }
    for (x, at) in [
        (r#"[-4,"inf",4]"#, 2),
        (r#"[-4,"NaN",4]"#, 2),
        (r#"[-4,"1e400",4]"#, 2),
        ("[-4,1e400,4]", 2),
        ("[-4,null,1e400]", 3),
    ] {
        let mut imported = Dss::new();
        let json = format!(
            r#"{{"Name":"lgfinite","LineSpacing":[
            {{"Name":"i","NConds":3,"NPhases":3,"X":{x},"H":[28,28,28],"Units":"ft"}}]}}"#
        );
        assert_eq!(imported.circuit_from_json(&json), Ok(()), "{x}");
        assert_eq!(
            errors(&imported),
            [format!(
                "LineSpacing.i.X: value {at} of the list is not a finite number, so the list is \
                 not applied."
            )],
            "{x}"
        );
        assert_eq!(query(&mut imported, "LineSpacing.i.x"), "", "{x}");
        assert_eq!(
            query(&mut imported, "LineSpacing.i.h"),
            "[ 28 28 28]",
            "{x}"
        );
    }
}

/// A LineSpacing property before its first `nconds=` is refused, naming it,
/// and writes nothing: the spacing holds the documented default of 3 wires,
/// but a list or a unit given ahead of the count would be read against a
/// count the deck has not stated. `a` takes no `nconds=` and holds no
/// position, `b` keeps feet, and `h` refuses an `Edit` before any `nconds=`.
/// A `like=` copy of a spacing with `nconds=` and a second `New` of one take
/// data at once. A spacing with no `nconds=` exports its name only, and a
/// refusal and a unit-word refusal in one command are logged in token order,
/// each underlining its own value. `q` writes its phases and its equivalent
/// spacing ahead of `nconds=4`: each of the six is refused by name, so the
/// spacing keeps 3 phases and no equivalent spacing, where an accepted one
/// would be dropped in silence by the `nconds=` after it.
#[test]
fn a_spacing_property_before_nconds_is_refused() {
    use crate::report::export::json::JsonOpts;
    let mut dss = compile("ls1_spacing_order.dss");
    let want: Vec<String> = [
        ("a", "X"),
        ("a", "H"),
        ("a", "Units"),
        ("b", "Units"),
        ("h", "X"),
    ]
    .iter()
    .map(|(sp, p)| spacing_before_nconds(sp, p))
    .collect();
    assert_eq!(errors(&dss), want);
    for (i, e) in dss.errors().iter().enumerate() {
        assert_eq!(e.code, None, "{i}: {}", e.message);
    }
    for (prop, want) in [
        ("a.nconds", "3"),
        ("a.x", ""),
        ("a.h", ""),
        ("a.units", "ft"),
        ("b.units", "ft"),
        ("b.x", "[ -5 0 5]"),
        ("b.h", "[ 28 28 28]"),
        ("h.x", ""),
        ("k.nconds", "3"),
        ("k.x", "[ -1 0 1]"),
        ("k.h", "[ 28 28 28]"),
    ] {
        assert_eq!(
            query(&mut dss, &format!("LineSpacing.{prop}")),
            want,
            "{prop}"
        );
    }
    for opts in [JsonOpts::NONE, JsonOpts::FULL] {
        let record = dss.obj_to_json("LineSpacing.a", opts).expect("a");
        assert_eq!(record, r#"{"Name":"a"}"#, "{opts:?}");
    }

    let before = dss.errors().len();
    let cmd = "New LineSpacing.m x=[1 2 3] nconds=3 units=miles";
    dss.command(cmd);
    assert_eq!(
        errors(&dss)[before..],
        [
            spacing_before_nconds("m", "X"),
            "LineSpacing.m.Units: \"miles\" is not one of the length units mi, kft, km, m, ft, \
             in, cm, mm."
                .to_string()
        ]
    );
    let underlined: Vec<&str> = dss.errors()[before..]
        .iter()
        .map(|d| {
            let span = d.span.as_ref().expect("a span");
            &cmd[span.offset()..span.offset() + span.len()]
        })
        .collect();
    assert_eq!(underlined, ["1 2 3", "miles"]);
    assert_eq!(query(&mut dss, "LineSpacing.m.x"), "");
    assert_eq!(query(&mut dss, "LineSpacing.m.units"), "ft");

    let before = dss.errors().len();
    dss.command(
        "New LineSpacing.q nphases=2 detailed=no EqDistPhPh=1.2 EqDistPhN=1.5 \
         AvgPhaseHeight=8.5 AvgNeutralHeight=7.3 nconds=4",
    );
    let want: Vec<String> = [
        "NPhases",
        "Detailed",
        "EqDistPhPh",
        "EqDistPhN",
        "AvgPhaseHeight",
        "AvgNeutralHeight",
    ]
    .iter()
    .map(|p| spacing_before_nconds("q", p))
    .collect();
    assert_eq!(errors(&dss)[before..], want);
    for (prop, want) in [
        ("q.nconds", "4"),
        ("q.nphases", "3"),
        ("q.detailed", "Yes"),
        ("q.eqdistphph", "0"),
    ] {
        assert_eq!(
            query(&mut dss, &format!("LineSpacing.{prop}")),
            want,
            "{prop}"
        );
    }
}

/// Each `nconds=` of a LineSpacing keeps no position, returns the unit to
/// feet and drops the equivalent spacing (`detailed` Yes, every distance 0).
/// A position or a distance kept would be re-read in feet, 3.28 times shorter
/// than written: X21 of `p` at 1.2 ft is not X21 at 1.2 m ([`carson_x`]), and
/// zero distances kept under `detailed=no` would give `ln(De/0)`. So a
/// geometry on a re-set spacing gets rule 7 naming every conductor, and with
/// the data given again, now in feet, each Line solves at the reactance of
/// feet.
#[test]
fn a_spacing_nconds_keeps_no_position_and_returns_to_feet() {
    let mut dss = compile("ls3_nconds_starts_over.dss");
    let all = "LineGeometry.gp: conductors 1, 2 and 3 have no x and no h. LineGeometry.ge: \
               conductors 1, 2, 3 and 4 have no x and no h.";
    assert_eq!(
        errors(&dss),
        [format!("Error Encountered in CalcVoltageBases: {all}")]
    );
    assert!(!solved(&dss));
    for (prop, want) in [
        ("p.x", ""),
        ("p.h", ""),
        ("p.units", "ft"),
        ("e.detailed", "Yes"),
        ("e.eqdistphph", "0"),
        ("e.eqdistphn", "0"),
        ("e.avgphaseheight", "0"),
        ("e.avgneutralheight", "0"),
        ("e.units", "ft"),
    ] {
        assert_eq!(
            query(&mut dss, &format!("LineSpacing.{prop}")),
            want,
            "{prop}"
        );
    }

    for c in [
        "Edit LineSpacing.p x=[-1.2 0 1.2] h=[8.5 8.5 8.5]",
        "Edit LineSpacing.e detailed=no EqDistPhPh=1.2 EqDistPhN=1.5 AvgPhaseHeight=8.5 \
         AvgNeutralHeight=7.3",
        "Edit LineGeometry.gp spacing=p",
        "Edit LineGeometry.ge spacing=e",
        "Edit Line.Lp geometry=gp",
        "Edit Line.Le geometry=ge",
        "Solve",
    ] {
        dss.command(c);
    }
    assert_eq!(dss.errors().len(), 1, "{:?}", errors(&dss));
    assert!(solved(&dss));
    let row = |unit| {
        [
            phase(-1.2, 8.5, unit),
            phase(0.0, 8.5, unit),
            phase(1.2, 8.5, unit),
        ]
    };
    assert_x(&dss, "Lp", &row("ft"), &[], "p");
    let (in_ft, in_m) = (carson_x(&row("ft")), carson_x(&row("m")));
    assert!(
        (in_ft[1][0] - in_m[1][0]).abs() > 0.04,
        "{in_ft:?} {in_m:?}"
    );
    let ft = metres("ft");
    let eq = carson_x_equivalent(&[GMR_P, GMR_P, GMR_P, GMR_N], 3, 1.2 * ft, 1.5 * ft);
    assert_x_is(&dss, "Le", &eq, &[], "e");
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
    let xs: Vec<Option<f64>> = conductors(&dss, "g").iter().map(|c| c.x).collect();
    assert_eq!(xs, [Some(-4.0), Some(0.0), Some(4.0), Some(0.0)]);
}

/// The JSON import refuses what the text refuses: it reaches the same
/// property edit and applies `NConds` first, so the rule on properties before
/// `nconds=` shows for a record without `NConds`. A new geometry's record
/// `{"Name":"j","NPhases":2}` is refused naming `NPhases`, and the exported
/// record of a complete geometry with `"Units":"metre"` added is refused
/// naming the word, its conductors keeping feet. A LineSpacing record without
/// `NConds` is refused the same way. The import returns `Ok` and the refusals
/// are in the error log, as the text path logs them.
#[test]
fn json_import_refuses_what_the_text_refuses() {
    use crate::report::export::json::JsonOpts;
    let wire = "New WireData.w Rdc=0.1 Runits=km GMRac=0.01 GMRunits=m radius=0.02 radunits=m \
                normamps=400";
    let import = |records: &str| {
        let json =
            format!(r#"{{"Name":"lgjson","PreCommands":["{wire}"],"LineGeometry":[{records}]}}"#);
        let mut dss = Dss::new();
        assert_eq!(dss.circuit_from_json(&json), Ok(()), "{records}");
        dss
    };
    let mut fresh = import(r#"{"Name":"j","NPhases":2}"#);
    assert_eq!(errors(&fresh), [before_nconds("j", "NPhases")]);
    assert_eq!(query(&mut fresh, "LineGeometry.j.NPhases"), "3");
    assert_eq!(query(&mut fresh, "LineGeometry.j.NConds"), "0");

    let mut src = Dss::new();
    src.command("new circuit.lgjson");
    src.command(wire);
    src.command(
        "New LineGeometry.g nconds=3 nphases=3 cond=1 wire=w x=-4 h=28 cond=2 wire=w x=0 h=28 \
         cond=3 wire=w x=4 h=28",
    );
    assert!(src.errors().is_empty(), "{:?}", errors(&src));
    let record = src
        .obj_to_json("LineGeometry.g", JsonOpts::NONE)
        .expect("g");
    // The record writes no `Units` while it is the default, so the key is added.
    assert!(!record.contains("\"Units\""), "{record}");
    let metre = format!(
        "{},\"Units\":\"metre\"}}",
        record.strip_suffix('}').expect("a JSON object")
    );
    let back = import(&metre);
    assert_eq!(
        errors(&back),
        [
            "LineGeometry.g.Units: \"metre\" is not one of the length units mi, kft, km, m, ft, \
             in, cm, mm."
        ]
    );
    assert_eq!(units(&back, "g"), ["ft"; 3]);

    let mut spacing = Dss::new();
    let json = r#"{"Name":"lgjson","LineSpacing":[{"Name":"s","X":[1,2,3]}]}"#;
    assert_eq!(spacing.circuit_from_json(json), Ok(()));
    assert_eq!(errors(&spacing), [spacing_before_nconds("s", "X")]);
    assert_eq!(query(&mut spacing, "LineSpacing.s.x"), "");
}

/// The JSON export of a geometry with no conductor selected (after a second
/// `New`, after `nconds=`, or a `like=` copy) writes none of the selected
/// conductor's values, in the default and the full form: `Cond` 0 and the
/// missing conductor object would each be refused by the import (rules 6 and
/// 2), and `X` and `H` hold no value. The default record, and V5's whole
/// circuit, import with no error. With a conductor selected the record still
/// carries it and its values.
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
/// A geometry without conductors writes its name only, in the default and
/// the full form: the import refuses every property before `NConds`, and
/// `NConds` 0 itself. The record imports with no error into a geometry that
/// still has no conductors.
#[test]
fn json_export_of_a_geometry_without_conductors_writes_its_name_only() {
    use crate::report::export::json::JsonOpts;
    let mut dss = Dss::new();
    dss.command("new circuit.lgjson");
    dss.command("New LineGeometry.g");
    assert!(dss.errors().is_empty(), "{:?}", errors(&dss));
    for opts in [JsonOpts::NONE, JsonOpts::FULL] {
        let record = dss.obj_to_json("LineGeometry.g", opts).expect("g");
        assert_eq!(record, r#"{"Name":"g"}"#, "{opts:?}");
        let json = format!(r#"{{"Name":"lgjson","LineGeometry":[{record}]}}"#);
        let mut back = Dss::new();
        assert_eq!(back.circuit_from_json(&json), Ok(()), "{opts:?}");
        assert!(
            back.errors().is_empty(),
            "{opts:?}: {record} imports with {:?}",
            errors(&back)
        );
        assert_eq!(query(&mut back, "LineGeometry.g.NConds"), "0", "{opts:?}");
    }
}

/// An `x` or `h` reads empty where it holds no value: with no conductor
/// selected (after `New`, a `like=` copy and `nconds=`), where `Cond` reads 0,
/// and for a selected conductor whose value was never given. `Dump` prints
/// `~ X=` for that conductor. Its JSON record carries `Cond`, the conductor
/// and `H` but no `X`, and imports with no error, the `x` still never given.
#[test]
fn x_and_h_read_empty_when_never_given() {
    use crate::report::export::json::JsonOpts;
    let wire = "New WireData.w Rdc=0.1 Runits=km GMRac=0.01 GMRunits=m radius=0.02 radunits=m \
                normamps=400";
    let scratch = Scratch::new("empty");
    let mut dss = Dss::new();
    dss.command("new circuit.lgempty");
    dss.command(wire);
    for c in [
        "New LineGeometry.src nconds=3 nphases=3 cond=1 wire=w x=-4 h=28 cond=2 wire=w x=0 h=28 \
         cond=3 wire=w x=4 h=28",
        "New LineGeometry.fresh",
        "New LineGeometry.copy like=src",
        "New LineGeometry.re like=src",
        "Edit LineGeometry.re nconds=3",
        "New LineGeometry.part nconds=3 nphases=3 cond=1 wire=w x=-4 h=28 cond=3 wire=w x=4 \
         h=28 cond=2 wire=w h=28",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", errors(&dss));
    for g in ["fresh", "copy", "re"] {
        assert_eq!(cond(&mut dss, g), "0", "{g}");
        for p in ["X", "H"] {
            assert_eq!(
                query(&mut dss, &format!("LineGeometry.{g}.{p}")),
                "",
                "{g} {p}"
            );
        }
    }
    assert_eq!(cond(&mut dss, "part"), "2");
    assert_eq!(query(&mut dss, "LineGeometry.part.X"), "");
    assert_eq!(query(&mut dss, "LineGeometry.part.H"), "28");

    dss.command(&format!("Set DataPath=\"{}\"", slash(&scratch.dir("dump"))));
    dss.command("Dump LineGeometry.part");
    assert!(dss.errors().is_empty(), "{:?}", errors(&dss));
    let text = read(&scratch.0.join("dump"), "lgempty_PropertyDump.txt");
    let rows: Vec<&str> = text.lines().filter(|l| l.starts_with("~ ")).collect();
    let want = ["~ Cond=2", "~ Wire=w", "~ X=", "~ H=28", "~ Units=ft"];
    assert_eq!(rows[7..12], want, "{rows:?}");
    assert_eq!(cond(&mut dss, "part"), "2");

    let record = dss
        .obj_to_json("LineGeometry.part", JsonOpts::NONE)
        .expect("part");
    // The lanes spell the float differently, so `H` is read back after the import.
    for member in ["\"Cond\":2", "\"Wire\":\"w\"", "\"H\":"] {
        assert!(record.contains(member), "{member} not in {record}");
    }
    assert!(!record.contains("\"X\""), "{record}");
    let json =
        format!(r#"{{"Name":"lgempty","PreCommands":["{wire}"],"LineGeometry":[{record}]}}"#);
    let mut back = Dss::new();
    assert_eq!(back.circuit_from_json(&json), Ok(()));
    assert!(
        back.errors().is_empty(),
        "{record} imports with {:?}",
        errors(&back)
    );
    assert_eq!(cond(&mut back, "part"), "2");
    assert_eq!(query(&mut back, "LineGeometry.part.X"), "");
    assert_eq!(query(&mut back, "LineGeometry.part.H"), "28");
    assert_eq!(
        conductors(&back, "part")[1],
        Conductor {
            object: Some("w".into()),
            x: None,
            h: Some(28.0),
            unit: "ft"
        }
    );
}

/// With no conductor selected `Cond` reads 0, `X` and `H` read empty, the
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
                ("X", ""),
                ("H", ""),
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
                ("X", ""),
                ("H", ""),
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
                ("X", ""),
                ("H", ""),
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

/// A `cond=` before `nconds=` is refused and enters no set order, and the
/// spacing and `wires=` after `nconds=` fill all three conductors. The Save
/// writes `NConds=` first, then `NPhases=` and the conductor table, and the
/// reload gives the same conductors and reactance with no error.
#[test]
fn save_writes_nconds_first() {
    let scratch = Scratch::new("nconds_first");
    let name = "r6b_cond_before_nconds_with_spacing.dss";
    let mut dss = compile(name);
    assert_eq!(errors(&dss), [before_nconds("g", "Cond")]);
    assert_eq!(dss.errors()[0].code, None);
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
    assert!(body.starts_with(" nconds=3 nphases=3 cond=1 "), "{line}");
    let back = reload(&dir);
    assert_eq!(conductors(&back, "g"), conductors(&dss, "g"));
    assert_x(&back, "L", &placed, &[(2, 1, 0.794_511_1)], "R6b reloaded");
}

/// Save writes `X=` and `h=` only where they were given, and a row for a
/// conductor that holds a position and no conductor object: conductor 1
/// complete, conductor 2 with `x=1 h=28` and no wire, conductor 3 with a wire
/// and `h=28` only. The reload holds the same conductors and gives the same
/// rule 7 error, at the `CalcVoltageBases` of its `Master.dss` and at `Solve`.
/// A conductor with one coordinate and no conductor object gets its row too:
/// `q`, on no Line, holds `h=28` alone on conductor 1 and `x=2` alone on
/// conductor 2, and reloads with each position as typed.
#[test]
fn save_writes_each_given_position_and_no_other() {
    let scratch = Scratch::new("positions");
    let mut dss = compile("h2_one_conductor_with_cond.dss");
    dss.command("New LineGeometry.p nconds=3 nphases=3");
    dss.command("~ cond=1 wire=ACSR_556_5 x=-1 h=28 cond=2 x=1 h=28 cond=3 wire=ACSR_556_5 h=28");
    dss.command("New LineGeometry.q nconds=2 nphases=2");
    dss.command("~ cond=1 h=28 cond=2 x=2");
    dss.command("New Line.Lp bus1=src.1.2.3 bus2=bp.1.2.3 geometry=p length=1 units=mi");
    dss.command("Solve");
    let rule_7 =
        "LineGeometry.p: conductor 2 has no wire, cncable or tscable. Conductor 3 has no x.";
    assert_eq!(
        errors(&dss),
        [format!("Error Encountered in Solve: {rule_7}")]
    );
    let dir = scratch.dir("save");
    let (rows, restore) = saved_rows(&save(&mut dss, &dir, "p"));
    assert_eq!(
        rows,
        [
            ["1", "wire=acsr_556_5", "x=-1", "h=28", "units=ft"],
            ["2", "", "x=1", "h=28", "units=ft"],
            ["3", "wire=acsr_556_5", "", "h=28", "units=ft"],
        ]
    );
    assert_eq!(restore, None);
    let (rows, restore) = saved_rows(&saved_line(&dir, "q"));
    assert_eq!(
        rows,
        [
            ["1", "", "", "h=28", "units=ft"],
            ["2", "", "x=2", "", "units=ft"],
        ]
    );
    assert_eq!(restore, None);
    let back = reload_unsolved(&dir);
    assert_eq!(conductors(&back, "p"), conductors(&dss, "p"));
    assert_eq!(
        conductors(&back, "p")[1],
        Conductor {
            object: None,
            x: Some(1.0),
            h: Some(28.0),
            unit: "ft"
        }
    );
    assert_eq!(conductors(&back, "q"), conductors(&dss, "q"));
    assert_eq!(
        conductors(&back, "q"),
        [
            Conductor {
                object: None,
                x: None,
                h: Some(28.0),
                unit: "ft"
            },
            Conductor {
                object: None,
                x: Some(2.0),
                h: None,
                unit: "ft"
            },
        ]
    );
    assert_eq!(
        errors(&back),
        [
            format!("Error Encountered in CalcVoltageBases: {rule_7}"),
            format!("Error Encountered in Solve: {rule_7}"),
        ]
    );
}

/// The Save of every deck that solves recompiles to the same conductors
/// (objects, coordinates, units) and the same reactance, and writes no
/// `units=none` and no ` Cond=0`.
#[test]
fn save_round_trips_every_case() {
    let scratch = Scratch::new("save_all");
    let cases: [(&str, &[&str], &[&str]); 30] = [
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

/// The Deri series impedance between conductors 1 and 2 of script C (4 ft
/// apart, both at 28 ft) at 60 Hz over 100 Ω·m earth, in Ω/mi:
/// `Z12 = (j ω μ0 / 2π) ln(D' / D)`, where `D' = √(dx² + (2h + 2p)²)` reaches
/// the image of conductor 2 below the complex depth `p = √(ρ / (j ω μ0))`. It
/// gives R21 0.0937284 and X21 0.8054776 Ω/mi, where the simple Carson model
/// gives R21 0.0953015 and X21 0.7945111.
fn deri_z21_c0() -> num_complex::Complex64 {
    use num_complex::Complex64;
    let (f, rho) = (60.0_f64, 100.0_f64);
    let w_mu0 = 2.0 * std::f64::consts::PI * f * 4.0e-7 * std::f64::consts::PI;
    let (dx, h) = (4.0 * 0.3048, 28.0 * 0.3048);
    let p = (Complex64::new(rho, 0.0) / Complex64::new(0.0, w_mu0)).sqrt();
    let image = (Complex64::new(dx * dx, 0.0) + (2.0 * h + 2.0 * p).powi(2)).sqrt();
    let per_metre = Complex64::new(0.0, w_mu0 / (2.0 * std::f64::consts::PI)) * (image / dx).ln();
    per_metre * 1609.344
}

/// `Show LineConstants` computes each geometry on a copy at the report's
/// frequency, resistivity and earth model, so a Line built on the geometry
/// after the report keeps its own. After a report at 50 Hz over 10 Ω·m, `Lx`
/// reads `rho` 100 and the simple Carson reactance at 100 Ω·m ([`carson_x`],
/// X21 0.7945111 Ω/mi, where 10 Ω·m gives 0.6548113). After a report under
/// the circuit's Carson model, `Lx` with `earthmodel=deri` keeps the Deri
/// impedance ([`deri_z21_c0`]). Each reads, entry by entry, what the same Line
/// reads without the report. Both oracles hand the report's resistivity and
/// earth model to the Line.
#[test]
fn a_line_constants_report_changes_no_line() {
    let scratch = Scratch::new("report_copy");
    let build = |report: Option<&str>, earth: &str, tag: &str| {
        let mut dss = compile("c0_cond_in_order.dss");
        dss.command(&format!("Set DataPath=\"{}\"", slash(&scratch.dir(tag))));
        if let Some(report) = report {
            dss.command(report);
        }
        dss.command(&format!(
            "New Line.Lx bus1=src.1.2.3.0 bus2=bx.1.2.3.0 geometry=g length=1 units=mi{earth}"
        ));
        dss.command("CalcVoltageBases");
        assert!(dss.errors().is_empty(), "{tag}: {:?}", errors(&dss));
        dss
    };

    let mut rho = build(
        Some("Show LineConstants freq=50 units=mi rho=10"),
        "",
        "rho",
    );
    let plain = build(None, "", "plain");
    assert_eq!(query(&mut rho, "Line.Lx.rho"), "100");
    assert_x(
        &rho,
        "Lx",
        &c0_in(["ft"; 4]),
        &[(2, 1, 0.794_511_1)],
        "after a report over 10 ohm-m",
    );
    assert_eq!(xmatrix(&rho, "Lx"), xmatrix(&plain, "Lx"));
    assert_eq!(zpart(&rho, "Lx", true), zpart(&plain, "Lx", true));

    let deri = build(
        Some("Show LineConstants freq=60 units=mi rho=100"),
        " earthmodel=deri",
        "deri",
    );
    let deri_plain = build(None, " earthmodel=deri", "deri_plain");
    let z21 = deri_z21_c0();
    let (r21, x21) = (zpart(&deri, "Lx", true)[1][0], xmatrix(&deri, "Lx")[1][0]);
    assert!(
        (r21 - z21.re).abs() <= X_BAND,
        "R21 {r21}, Deri gives {}",
        z21.re
    );
    assert!(
        (x21 - z21.im).abs() <= X_BAND,
        "X21 {x21}, Deri gives {}",
        z21.im
    );
    assert!((r21 - 0.093_728_36).abs() <= X_BAND, "R21 {r21}");
    assert_eq!(xmatrix(&deri, "Lx"), xmatrix(&deri_plain, "Lx"));
    assert_eq!(zpart(&deri, "Lx", true), zpart(&deri_plain, "Lx", true));
}

/// `Show LineConstants` prints the matrices of the earth model it names. Two
/// reports at 60 Hz over 100 Ω·m, the first under Deri and the second under
/// the simple Carson model, and a Carson report after a Line built with
/// `earthmodel=deri` each write R21 and X21 of their own model into
/// `LineConstantsCode.dss`: Deri by [`deri_z21_c0`], simple Carson by
/// R21 = π² f 10⁻⁷ Ω/m (0.0953015 Ω/mi) and X21 by [`carson_x`] (0.7945111
/// Ω/mi). The file prints six significant digits, so each value holds within
/// half a unit of its sixth digit beside [`X_BAND`]. Both engines are script
/// C up to its Line `L`, so no Line computed the geometry before the reports.
/// On that setup both oracles print the Deri values of the first report or of
/// the Line under the Carson header (R21 0.0937284, X21 0.805478 Ω/mi).
#[test]
fn a_line_constants_report_prints_the_model_it_names() {
    let scratch = Scratch::new("report_model");
    let report = |dss: &mut Dss, tag: &str| {
        let dir = scratch.dir(tag);
        dss.command(&format!("Set DataPath=\"{}\"", slash(&dir)));
        dss.command("Show LineConstants freq=60 units=mi rho=100");
        assert!(dss.errors().is_empty(), "{tag}: {:?}", errors(dss));
        let text = read(&dir, "LineConstantsCode.dss");
        let entry_21 = |key: &str| -> f64 {
            let line = text
                .lines()
                .find(|l| l.contains(key))
                .unwrap_or_else(|| panic!("{tag}: no {key} in {text}"));
            line.split('|')
                .nth(1)
                .and_then(|row| row.split_whitespace().next())
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(|| panic!("{tag}: no entry 21 in {line}"))
        };
        let model = text
            .lines()
            .find(|l| l.starts_with("!--- Earth Model"))
            .unwrap_or_default()
            .to_string();
        (model, entry_21("Rmatrix="), entry_21("Xmatrix="))
    };

    let mut two = replay_until("c0_cond_in_order.dss", "New Line.L");
    two.command("Set EarthModel=Deri");
    let first = report(&mut two, "deri");
    two.command("Set EarthModel=Carson");
    let second = report(&mut two, "carson");
    let mut line = replay_until("c0_cond_in_order.dss", "New Line.L");
    line.command("Set EarthModel=Carson");
    line.command(
        "New Line.Lx bus1=src.1.2.3.0 bus2=bx.1.2.3.0 geometry=g length=1 units=mi \
         earthmodel=deri",
    );
    line.command("CalcVoltageBases");
    let after_line = report(&mut line, "after_line");

    let deri = deri_z21_c0();
    let carson_r21 = std::f64::consts::PI.powi(2) * 60.0e-7 * 1609.344;
    let carson_x21 = carson_x(&c0_in(["ft"; 4]))[1][0];
    let near = |printed: f64, want: f64| {
        let digit = 10f64.powf(want.abs().log10().floor() - 5.0);
        (printed - want).abs() <= 0.5 * digit + X_BAND
    };
    for (tag, (model, r21, x21), (name, want_r, want_x)) in [
        ("first report", first, ("Deri", deri.re, deri.im)),
        ("second report", second, ("Carson", carson_r21, carson_x21)),
        (
            "report after a Deri Line",
            after_line,
            ("Carson", carson_r21, carson_x21),
        ),
    ] {
        assert_eq!(model, format!("!--- Earth Model = {name}"), "{tag}");
        assert!(near(r21, want_r), "{tag}: R21 {r21}, {name} gives {want_r}");
        assert!(near(x21, want_x), "{tag}: X21 {x21}, {name} gives {want_x}");
    }
}

/// A `nconds=` whose conductor table cannot be allocated is refused, naming
/// the count, and the process goes on: a new geometry keeps no conductors, so
/// the property after it is refused, and script C's geometry keeps its four
/// conductors, their units and the selection, and its Line still solves at
/// X21 0.7945111 Ω/mi. A count past the integer range is refused before it is
/// converted, naming it as written, in both lanes, and so is a negative one.
/// The JSON import refuses such a count the same way, one in the integer range
/// and one past it. Every count here needs matrices larger than the address
/// space, so the refusal allocates nothing.
#[test]
fn a_count_beyond_the_memory_is_refused() {
    let refused = |geom: &str, n: &str| {
        format!(
            "LineGeometry.{geom}.NConds: nconds={n} needs more memory than is available, so \
             the value is not applied."
        )
    };
    let mut dss = compile("c0_cond_in_order.dss");
    assert!(dss.errors().is_empty(), "{:?}", errors(&dss));
    dss.command("New LineGeometry.h nconds=1000000000");
    dss.command("~ nphases=3");
    dss.command("Edit LineGeometry.g cond=2");
    dss.command("Edit LineGeometry.g nconds=1000000000");
    assert_eq!(
        errors(&dss),
        [
            refused("h", "1000000000"),
            before_nconds("h", "NPhases"),
            refused("g", "1000000000"),
        ]
    );
    assert_eq!(query(&mut dss, "LineGeometry.h.NConds"), "0");
    assert_eq!(query(&mut dss, "LineGeometry.g.NConds"), "4");
    assert_eq!(cond(&mut dss, "g"), "2");
    assert_eq!(units(&dss, "g"), ["ft"; 4]);
    dss.command("Edit Line.L geometry=g");
    dss.command("Solve");
    assert_eq!(dss.errors().len(), 3, "{:?}", errors(&dss));
    assert_x(
        &dss,
        "L",
        &c0_in(["ft"; 4]),
        &[(2, 1, 0.794_511_1)],
        "refused count",
    );
    for count in ["1e10", "4294967299", "-4294967293"] {
        dss.command(&format!("Edit LineGeometry.g nconds={count}"));
    }
    assert_eq!(
        errors(&dss)[3..],
        [
            refused("g", "1e10"),
            refused("g", "4294967299"),
            "LineGeometry.g.NConds: Value (-4294967293) cannot be negative.".to_string(),
        ]
    );
    assert_eq!(query(&mut dss, "LineGeometry.g.NConds"), "4");
    assert_eq!(units(&dss, "g"), ["ft"; 4]);

    for (count, written) in [("1e9", "1000000000"), ("1e10", "10000000000")] {
        let mut json = Dss::new();
        let record =
            format!(r#"{{"Name":"lgjson","LineGeometry":[{{"Name":"g","NConds":{count}}}]}}"#);
        assert_eq!(json.circuit_from_json(&record), Ok(()), "{count}");
        let errs = errors(&json);
        assert_eq!(errs.len(), 1, "{count}: {errs:?}");
        assert_eq!(errs[0], refused("g", written), "{count}");
        assert_eq!(query(&mut json, "LineGeometry.g.NConds"), "0", "{count}");
    }
}

/// The schema gives `Cond` no default, as its help says: no conductor is
/// selected until `cond=N`, and the import refuses `Cond` 0 (rule 6).
#[test]
fn the_schema_gives_cond_no_default() {
    let schema: serde_json::Value =
        serde_json::from_str(&Dss::new().extract_schema_json()).expect("the schema is JSON");
    let cond = &schema["$defs"]["LineGeometry"]["properties"]["Cond"];
    assert_eq!(cond["type"], "integer", "{cond}");
    assert!(cond.get("default").is_none(), "{cond}");
    assert!(
        cond["description"]
            .as_str()
            .is_some_and(|d| d.contains("There is no default")),
        "{cond}"
    );
}

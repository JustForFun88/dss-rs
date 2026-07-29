//! FPC-RTL float→ASCII battery gate (audit follow-up, WP8.5 step 3b) — and,
//! since Stage F.4, the **expected-value pin of the F-FMT `%g` row**.
//!
//! `tests/golden/fmt_battery.csv` holds 13 198 f64 bit patterns rendered by
//! the **real FPC 3.2.2 RTL** (the pinned oracle backend's compiler) through
//! every float→string entry point the port mirrors — see
//! `tools/fpc/fmt_battery/README.md`. A regression in the Grisu1 pipeline
//! silently corrupts every Show/Export/Dump/Save report in the parity lane, so
//! it is pinned directly, not only incidentally through the report goldens.
//!
//! Three obligations, all discharged in **either** build (the F-FMT mechanism
//! keeps both kernels compiled):
//!
//! 1. [`fmt_battery_matches_fpc_rtl`] — the *parity* kernel
//!    (`util::fmt_g_fpc_impl`, plus the un-split `report::format::fpc_sci_w`)
//!    is byte-equal to the FPC RTL on all 13 198 × 7 renders. Stronger than
//!    before F.4, which asserted whatever the seam happened to select.
//! 2. [`native_kernel_differs_from_fpc_only_by_the_rounding_rule`] — the
//!    *default* kernel (`util::fmt_g_native_impl`) agrees with FPC everywhere
//!    except the documented two-stage-rounding population, whose size and worst
//!    case are pinned here.
//! 3. [`fmt_g_is_the_lane_kernel`] — the seam really selects the lane's kernel,
//!    so the two assertions above cannot collapse onto the same function.

use dss_core::compat::ORACLE_PARITY;
use dss_core::report::format::fpc_sci_w;
use dss_core::util::{float_to_str, fmt_g, fmt_g_fpc_impl, fmt_g_native_impl};

/// `(f64 value, the seven FPC RTL renders)` for every battery row.
fn battery() -> Vec<(f64, [String; 7])> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/golden/fmt_battery.csv"
    );
    let csv = std::fs::read_to_string(path).expect("read tests/golden/fmt_battery.csv");
    let mut out = Vec::new();
    for line in csv.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split(';').collect();
        assert_eq!(fields.len(), 8, "malformed battery row: {line}");
        let bits = u64::from_str_radix(fields[0], 16).expect("hex bits");
        let renders = std::array::from_fn(|i| fields[i + 1].to_string());
        out.push((f64::from_bits(bits), renders));
    }
    assert_eq!(
        out.len(),
        13_198,
        "battery row count changed — regenerate deliberately"
    );
    out
}

/// FPC `FloatToStr` expressed on the **parity kernel** rather than on the seam
/// (`util::float_to_str` follows the lane, so it cannot carry this assertion in
/// the default build).
fn float_to_str_fpc(v: f64) -> String {
    fmt_g_fpc_impl(v, 15).replace('e', "E")
}

const COLS: [&str; 7] = [
    "FloatToStr",
    "fmt_g(2)",
    "fmt_g(5)",
    "fmt_g(8)",
    "fmt_g(15)",
    "fpc_sci_w(0)",
    "fpc_sci_w(14)",
];

#[test]
fn fmt_battery_matches_fpc_rtl() {
    let rows = battery();
    let mut renders = 0usize;
    for (v, want) in &rows {
        let got = [
            float_to_str_fpc(*v),
            fmt_g_fpc_impl(*v, 2),
            fmt_g_fpc_impl(*v, 5),
            fmt_g_fpc_impl(*v, 8),
            fmt_g_fpc_impl(*v, 15),
            fpc_sci_w(*v, 0),
            fpc_sci_w(*v, 14),
        ];
        for (i, g) in got.iter().enumerate() {
            assert_eq!(
                g,
                &want[i],
                "{} diverges from the FPC RTL for v={v:e} ({:016x})",
                COLS[i],
                v.to_bits(),
            );
            renders += 1;
        }
    }
    assert_eq!(renders, rows.len() * 7);
}

/// The default kernel's disagreement with FPC is exactly the rounding rule the
/// F-FMT row exists for — nothing structural.
///
/// Three things are asserted about every divergence, over all four `%g`
/// precisions of the battery:
///
/// * the **notation** is unchanged (both kernels share the same
///   fixed-vs-scientific window), so a divergence can never widen a column of a
///   fixed-width table;
/// * the **digit count** moves by at most one (a round-up out of the leading
///   digit shortens `9.9…` to `10`, which FPC does too);
/// * the difference is a **last-digit** one: the two strings re-parse to `f64`s
///   no further apart than one unit in the last printed place. That is what
///   makes the default lane's parsed-numeric golden comparison meaningful —
///   F-FMT re-spells numbers, it never moves them. The bound comes from the
///   printed precision itself, not from a tolerance.
///
/// The population size is pinned, not merely bounded, so a change to either
/// kernel shows up as a number rather than as silence.
#[test]
fn native_kernel_differs_from_fpc_only_by_the_rounding_rule() {
    let rows = battery();
    let mut diverged = 0usize;
    let mut worst = 0.0f64;
    let mut widest = 0usize;
    let mut widest_at: Option<(f64, usize, String, String)> = None;
    let mut example: Option<(f64, usize, String, String)> = None;

    for (v, _) in &rows {
        for sig in [2usize, 5, 8, 15] {
            let fpc = fmt_g_fpc_impl(*v, sig);
            let native = fmt_g_native_impl(*v, sig);
            if fpc == native {
                continue;
            }
            diverged += 1;

            assert_eq!(
                fpc.contains('E'),
                native.contains('E'),
                "notation moved for v={v:e} sig={sig}: {fpc} vs {native}"
            );
            let grew = native.len().saturating_sub(fpc.len());
            if grew > widest {
                widest = grew;
                widest_at = Some((*v, sig, fpc.clone(), native.clone()));
            }

            let a: f64 = fpc.replace('E', "e").parse().expect("FPC render parses");
            let b: f64 = native
                .replace('E', "e")
                .parse()
                .expect("native render parses");
            // One unit in the last printed place, plus the ≤½-ulp each side
            // that re-parsing the two decimal strings into `f64` costs.
            let unit = last_place_unit(&fpc) + 2.0 * f64::EPSILON * a.abs().max(b.abs());
            assert!(
                (a - b).abs() <= unit,
                "not a last-digit difference for v={v:e} sig={sig}: {fpc} vs {native}"
            );
            let rel = if a == 0.0 { 0.0 } else { ((a - b) / a).abs() };
            if rel > worst {
                worst = rel;
                example = Some((*v, sig, fpc.clone(), native.clone()));
            }
        }
    }

    // Measured over the committed battery (13 198 values × 4 precisions):
    // FPC's `GRISU1_F2A_HALF_ROUNDUP` + `GRISU1_F2A_AGRESSIVE_ROUNDUP`
    // re-round of an already-rounded 17-digit decimal disagrees with a single
    // correct rounding on this many renders. Move the number only in the commit
    // that changes a kernel — never to make a run pass.
    assert_eq!(
        diverged, FPC_VS_NATIVE_DIVERGENCES,
        "the FPC-vs-native `%g` divergence population moved (worst rel {worst:e}, \
         e.g. {example:?}) — re-measure and record it, do not silence it"
    );
    // A last-digit difference at the battery's coarsest precision (2
    // significant digits) is at most ~5e-2 relative; nothing here may exceed
    // that, which is what makes "re-spelled, not moved" a checked claim.
    assert!(
        worst < 5.1e-2,
        "divergence beyond a last-digit difference: {worst:e} at {example:?}"
    );
    // Column widths. A divergence can make the default render **one character**
    // longer, and only in one way: FPC rounded *up* into a trailing zero, which
    // its (and our) trailing-zero stripping then removed — measured at
    // `0.4020128841512195` (sig 15), where FPC prints the 14-digit
    // `0.40201288415122` and a single correct rounding keeps the 15-digit
    // `0.402012884151219`.
    //
    // That is not a column hazard: both kernels are capped at the same
    // significant-digit budget (`util::fpc_general_digits`) and share the
    // notation window, so the default render never exceeds the width the
    // *parity* kernel already needs at that precision — it just fails to get
    // shorter. A value of `2` here would mean something structural moved.
    assert_eq!(
        widest, 1,
        "the default-vs-parity render width difference moved — a wider default \
         render can glue two cells of a fixed-width `Show` table together, so \
         re-measure rather than widen ({widest_at:?})"
    );
}

/// The measured FPC-vs-native `%g` divergence count over the committed battery
/// at precisions 2/5/8/15 — see
/// [`native_kernel_differs_from_fpc_only_by_the_rounding_rule`].
const FPC_VS_NATIVE_DIVERGENCES: usize = 225;

/// One unit in the last printed decimal place of `s` (e.g. `1.25` → 0.01,
/// `1.2E-3` → 1e-4).
fn last_place_unit(s: &str) -> f64 {
    let (mantissa, exp) = match s.split_once('E') {
        Some((m, e)) => (m, e.parse::<i32>().expect("exponent")),
        None => (s, 0),
    };
    let frac = mantissa.split_once('.').map_or(0, |(_, f)| f.len()) as i32;
    10f64.powi(exp - frac)
}

/// The seam selects the lane's kernel — the F-FMT `%g` row's lane assertion.
///
/// `0.8258283333333335` is the value the parity kernel's own documentation
/// names (`loadshape.default`'s computed `FMean`): its 17-digit ties-to-even
/// form ends `…3350`, which FPC then re-rounds *up* half-away-from-zero, while a
/// single correct rounding of the true `f64` keeps `…333`.
#[test]
fn fmt_g_is_the_lane_kernel() {
    let v = 0.8258283333333335_f64;
    assert_eq!(fmt_g_fpc_impl(v, 15), "0.825828333333334");
    assert_eq!(fmt_g_native_impl(v, 15), "0.825828333333333");
    let want = if ORACLE_PARITY {
        "0.825828333333334"
    } else {
        "0.825828333333333"
    };
    assert_eq!(fmt_g(v, 15), want, "the seam must select the lane's kernel");
    // …and the whole `float_to_str` property surface rides on it.
    assert_eq!(float_to_str(v), want);
}

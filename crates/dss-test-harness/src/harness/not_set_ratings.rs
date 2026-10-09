//! A rating that is not set, against an oracle that prints a number for it.
//!
//! The engine reads a current rating nobody gave as its own value and renders
//! it `none` (user decision 2026-10-04). Both gating oracles store such a rating
//! as a number and print it: `-1` where the class starts from a "not defined"
//! marker (a Line on a spacing whose phase wire carries no rating), `0` where
//! the class starts from zero (a Fault, a GICTransformer, a Line on a geometry
//! of unrated wires, whose seasonal ratings start `[ 0]`).
//!
//! [`LANE_NOT_SET_RATINGS`] names each `(class, property)` and the number its
//! class prints. A cell is excused only when the engine prints `none` where the
//! oracle prints exactly that number, in both lanes and on both channels, and
//! for an array only entry by entry: every `none` faces the class's number and
//! every other entry is equal by value. Any other difference still fails. The
//! engine's spellings are pinned by
//! `exec::tests::ratings::unset_ratings_read_none_by_default`,
//! `line_on_an_unrated_phase_wire_is_not_set` and
//! `line_on_a_geometry_of_unrated_wires_is_not_set`.
//!
//! The excused cells are counted per row and per channel, and a
//! whole-population run of the corpus gate must excuse exactly
//! [`NOT_SET_RATING_HITS`] ([`assert_not_set_ratings_are_the_measured_population`]):
//! a row that excuses one cell more fails as surely as a row that excuses
//! none, so a set rating that starts reading `none` in a gated deck cannot
//! hide behind a row that is live anyway.

use super::PropsChannel;
use std::sync::atomic::{AtomicUsize, Ordering};

/// One `(class, property)` whose rating the engine reads as not set where an
/// oracle prints `sentinels`.
#[derive(Debug, Clone, Copy)]
pub struct NotSetRatingRow {
    pub class: &'static str,
    pub prop: &'static str,
    /// The numbers the oracle prints for a rating of this class that is not
    /// set.
    pub sentinels: &'static [&'static str],
    /// The `exec::tests::ratings` test that pins the engine's `none` for each
    /// of `sentinels`, in the same order.
    pub pins: &'static [&'static str],
}

const fn row(
    class: &'static str,
    prop: &'static str,
    sentinels: &'static [&'static str],
    pins: &'static [&'static str],
) -> NotSetRatingRow {
    NotSetRatingRow {
        class,
        prop,
        sentinels,
        pins,
    }
}

const DEFAULTS: &[&str] = &["unset_ratings_read_none_by_default"];
const SPACING_AND_GEOMETRY: &[&str] = &[
    "line_on_an_unrated_phase_wire_is_not_set",
    "line_on_a_geometry_of_unrated_wires_is_not_set",
];

/// The corpus property compare's rows. `Line.NormAmps`/`EmergAmps` print `-1`
/// on a spacing Line and `0` on a geometry Line, `Line.Ratings` prints a `0`
/// entry for every season of a geometry Line, and a Fault and a GICTransformer
/// print `0`.
pub const LANE_NOT_SET_RATINGS: &[NotSetRatingRow] = &[
    row("Fault", "EmergAmps", &["0"], DEFAULTS),
    row("Fault", "NormAmps", &["0"], DEFAULTS),
    row("GICTransformer", "EmergAmps", &["0"], DEFAULTS),
    row("GICTransformer", "NormAmps", &["0"], DEFAULTS),
    row("Line", "EmergAmps", &["-1", "0"], SPACING_AND_GEOMETRY),
    row("Line", "NormAmps", &["-1", "0"], SPACING_AND_GEOMETRY),
    row(
        "Line",
        "Ratings",
        &["0"],
        &["line_on_a_geometry_of_unrated_wires_is_not_set"],
    ),
];

/// The tokens of a rendered value: a scalar is one token, an array (`[ 400
/// none]`, `[0,]`, `[0, 0, ]`) its entries without brackets and separators.
fn tokens(s: &str) -> Vec<&str> {
    s.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|t| !t.is_empty())
        .collect()
}

fn same_number(a: &str, b: &str) -> bool {
    match (a.parse::<f64>(), b.parse::<f64>()) {
        (Ok(x), Ok(y)) => x == y || (x - y).abs() <= 1e-12 + 1e-9 * y.abs(),
        _ => false,
    }
}

/// Whether `rust` is the not-set reading of `oracle`: the same number of
/// entries, at least one `none`, every `none` facing one of `sentinels` and
/// every other entry equal to the oracle's by value.
pub fn reads_not_set(rust: &str, oracle: &str, sentinels: &[&str]) -> bool {
    let (r, o) = (tokens(rust), tokens(oracle));
    if r.len() != o.len() || !r.iter().any(|t| t.eq_ignore_ascii_case("none")) {
        return false;
    }
    r.iter().zip(&o).all(|(rt, ot)| {
        if rt.eq_ignore_ascii_case("none") {
            sentinels.iter().any(|s| same_number(ot, s))
        } else {
            rt == ot || same_number(rt, ot)
        }
    })
}

/// The cells a whole-population run of the corpus gate excuses, per row of
/// [`LANE_NOT_SET_RATINGS`] (same order), as `(capi_v0145, r4133)`. Measured
/// 2026-10-04 in both lanes, which agree.
pub const NOT_SET_RATING_HITS: [(usize, usize); LANE_NOT_SET_RATINGS.len()] = [
    (42, 388),
    (42, 388),
    (0, 22),
    (0, 22),
    (254, 24),
    (254, 24),
    (197, 231),
];

/// Cells [`excuse`] excused, per row of [`LANE_NOT_SET_RATINGS`] and per
/// channel (`[capi_v0145, r4133]`).
static HITS: [[AtomicUsize; 2]; LANE_NOT_SET_RATINGS.len()] =
    [const { [AtomicUsize::new(0), AtomicUsize::new(0)] }; LANE_NOT_SET_RATINGS.len()];

fn channel_slot(channel: PropsChannel) -> usize {
    match channel {
        PropsChannel::CapiV0145 => 0,
        PropsChannel::R4133 => 1,
    }
}

/// The corpus property compare's seam: whether the value compare of this cell
/// is excused as a rating that is not set (and count it on `channel`).
pub fn excuse(class: &str, prop: &str, rust: &str, oracle: &str, channel: PropsChannel) -> bool {
    let Some(i) = LANE_NOT_SET_RATINGS
        .iter()
        .position(|r| r.class.eq_ignore_ascii_case(class) && r.prop.eq_ignore_ascii_case(prop))
    else {
        return false;
    };
    if reads_not_set(rust, oracle, LANE_NOT_SET_RATINGS[i].sentinels) {
        HITS[i][channel_slot(channel)].fetch_add(1, Ordering::Relaxed);
        true
    } else {
        false
    }
}

/// Per-row excused cells as `(capi_v0145, r4133)`, in
/// [`LANE_NOT_SET_RATINGS`] order.
pub fn hits() -> Vec<(usize, usize)> {
    HITS.iter()
        .map(|h| (h[0].load(Ordering::Relaxed), h[1].load(Ordering::Relaxed)))
        .collect()
}

/// The count lock for [`LANE_NOT_SET_RATINGS`] on a whole-population run:
/// every row excuses exactly its [`NOT_SET_RATING_HITS`] cells on each
/// channel. A `DSS_GATE_ONLY` run makes no claim, since a filter can leave out
/// any case a row covers. The r4133 column is held only where that channel
/// runs (`dss-epri` is Windows-only).
pub fn assert_not_set_ratings_are_the_measured_population() {
    if std::env::var("DSS_GATE_ONLY").is_ok() {
        return;
    }
    check_hits(&hits(), cfg!(windows));
}

fn check_hits(hits: &[(usize, usize)], r4133_runs: bool) {
    let mut wrong = Vec::new();
    for ((row, &(capi, r4133)), &(want_capi, want_r4133)) in LANE_NOT_SET_RATINGS
        .iter()
        .zip(hits)
        .zip(&NOT_SET_RATING_HITS)
    {
        if capi != want_capi {
            wrong.push(format!(
                "{}.{} capi_v0145: {capi} excused, {want_capi} measured",
                row.class, row.prop
            ));
        }
        if r4133_runs && r4133 != want_r4133 {
            wrong.push(format!(
                "{}.{} r4133: {r4133} excused, {want_r4133} measured",
                row.class, row.prop
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "not-set rating exclusion off its measured population: {}. Fewer cells mean a \
         divergence that is no longer there, more mean a rating that now reads `none` where \
         the oracle prints a number: find which before NOT_SET_RATING_HITS moves.",
        wrong.join("; ")
    );
}

#[cfg(test)]
mod tests {
    use super::{LANE_NOT_SET_RATINGS, NOT_SET_RATING_HITS, check_hits, reads_not_set};
    use dss_core::elements::pd;
    use dss_core::obj::dss_enum::EnumRegistry;
    use dss_core::obj::props::PropType;

    #[test]
    fn a_scalar_is_excused_only_as_none_against_its_sentinel() {
        assert!(reads_not_set("none", "-1", &["-1", "0"]));
        assert!(reads_not_set("none", "0", &["-1", "0"]));
        assert!(!reads_not_set("none", "0", &["-1"]));
        assert!(!reads_not_set("none", "400", &["-1", "0"]));
        assert!(
            !reads_not_set("0", "0", &["0"]),
            "an equal cell is not a not-set cell"
        );
        assert!(!reads_not_set("-2", "-1", &["-1"]));
    }

    #[test]
    fn an_array_is_excused_entry_by_entry_in_either_oracle_spelling() {
        assert!(reads_not_set("[ none]", "[ 0]", &["0"]));
        assert!(reads_not_set("[ none]", "[0,]", &["0"]));
        assert!(reads_not_set(
            "[ 600 none none]",
            "[ 600 0 0]",
            &["-1", "0"]
        ));
        assert!(!reads_not_set("[ 600 none]", "[ 601 0]", &["0"]));
        assert!(!reads_not_set("[ none]", "[ 0 0]", &["0"]));
        assert!(!reads_not_set("[ none]", "[ -1]", &["0"]));
        assert!(!reads_not_set("[ 400]", "[400,]", &["0"]));
    }

    /// Every row names a rating property of an engine class: a renamed or
    /// retyped property cannot leave a row that matches nothing.
    #[test]
    fn every_row_names_a_rating_property() {
        let enums = EnumRegistry::new();
        for r in LANE_NOT_SET_RATINGS {
            let props = match r.class {
                "Fault" => pd::fault::class_props(&enums),
                "GICTransformer" => pd::gic_transformer::class_props(&enums),
                "Line" => pd::line::class_props(&enums),
                other => panic!("no class table for {other}"),
            };
            let idx = props
                .property_index(r.prop)
                .unwrap_or_else(|| panic!("{}.{} is not a property", r.class, r.prop));
            assert!(
                matches!(
                    props.prop(idx).ptype,
                    PropType::Rating | PropType::RatingArray
                ),
                "{}.{} is not a rating property",
                r.class,
                r.prop
            );
            assert_eq!(
                props.property_name(idx),
                r.prop,
                "{}: exact spelling",
                r.class
            );
        }
    }

    /// The count lock passes on the measured population only, and fails a row
    /// that excused one cell more or one cell fewer on either channel.
    #[test]
    fn the_count_lock_fails_a_row_off_its_measured_population_in_either_direction() {
        let measured = NOT_SET_RATING_HITS.to_vec();
        check_hits(&measured, true);
        for (i, row) in LANE_NOT_SET_RATINGS.iter().enumerate() {
            for (channel, r4133) in [("capi_v0145", false), ("r4133", true)] {
                for step in [1isize, -1] {
                    let mut off = measured.clone();
                    let cell = if r4133 { &mut off[i].1 } else { &mut off[i].0 };
                    let Some(moved) = cell.checked_add_signed(step) else {
                        continue;
                    };
                    *cell = moved;
                    let msg = crate::harness::panic_message(|| check_hits(&off, true));
                    let name = format!("{}.{} {channel}", row.class, row.prop);
                    assert!(msg.contains(&name), "{name} {step:+}: {msg}");
                }
            }
        }
    }

    /// Where the r4133 channel does not run, its column is not held.
    #[test]
    fn the_count_lock_holds_the_r4133_column_only_where_that_channel_runs() {
        let mut no_r4133: Vec<(usize, usize)> = NOT_SET_RATING_HITS
            .iter()
            .map(|&(capi, _)| (capi, 0))
            .collect();
        check_hits(&no_r4133, false);
        no_r4133[0].0 += 1;
        let msg = crate::harness::panic_message(|| check_hits(&no_r4133, false));
        assert!(msg.contains("Fault.EmergAmps capi_v0145"), "{msg}");
    }

    /// Every row is dispositioned sentinel by sentinel: each number it accepts
    /// names the engine test that pins the `none` it lets through, and that
    /// test exists, so a sentinel cannot join a row without its pin.
    #[test]
    fn every_sentinel_of_a_row_names_the_pin_of_its_none() {
        let pins = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../dss-core/src/exec/tests/ratings.rs"
        ))
        .expect("the rating pins are readable");
        for r in LANE_NOT_SET_RATINGS {
            assert_eq!(
                r.pins.len(),
                r.sentinels.len(),
                "{}.{}: one pin per sentinel",
                r.class,
                r.prop
            );
            for pin in r.pins {
                assert_eq!(
                    pins.matches(&format!("fn {pin}() {{")).count(),
                    1,
                    "{}.{}: no test `{pin}` in exec/tests/ratings.rs",
                    r.class,
                    r.prop
                );
            }
        }
    }

    /// Every row excuses a cell on at least one channel: a row whose measured
    /// count is zero on both names no divergence.
    #[test]
    fn every_row_excuses_a_cell_on_some_channel() {
        for (row, &(capi, r4133)) in LANE_NOT_SET_RATINGS.iter().zip(&NOT_SET_RATING_HITS) {
            assert!(
                capi + r4133 > 0,
                "{}.{} excuses nothing",
                row.class,
                row.prop
            );
        }
    }
}

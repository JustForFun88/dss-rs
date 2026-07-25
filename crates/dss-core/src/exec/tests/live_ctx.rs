//! Live-context threading guard (WP `bug-livectx`). The WP removed every
//! `default_recalc_ctx()` substitution so a PC element's create-time and
//! edit-time `RecalcElementData` reads the LIVE `ActiveCircuit.Solution` state
//! (Pascal `SetNominalGeneration` reads `Solution.Mode` / `GenMultiplier` / …),
//! not a synthetic parse-default snapshot.
//!
//! Normal decks define elements BEFORE the solution leaves its default state, so
//! the WP is bit-neutral on the corpus + goldens — those can only *indirectly*
//! guard the threading. These tests make the delta OBSERVABLE: they move the live
//! `GenMultiplier` off its 1.0 default, then create / edit a Generator and read
//! the recalc-derived `p_nominal_per_phase` BEFORE any solve re-recalcs it (in
//! Snapshot mode `factor = GenMultiplier`, so `Pnom = 1000·kW·factor / nphases`).
//! A regression that re-substituted `default_recalc_ctx()` (`GenMultiplier=1`) at
//! either the create (`recalc_pc_create`) or edit (`end_edit`) site would drop
//! the factor and fail these — the silent-revert the audits flagged as otherwise
//! uncaught (it is masked on every existing gate).

use crate::exec::*;

/// The recalc-derived per-phase real power of `Generator.g1` — read directly from
/// the concrete element (crate-internal), the field `SetNominalGeneration` writes.
fn gen_pnom(dss: &Dss) -> f64 {
    let ci = dss.class_by_name["generator"];
    let oi = dss.classes[ci].name_to_idx["g1"];
    dss.classes[ci].arena[oi]
        .as_any()
        .downcast_ref::<crate::elements::pc::generator::Generator>()
        .expect("g1 is a Generator")
        .p_nominal_per_phase
}

/// The create path (`New`) runs `recalc_pc_create` then `end_edit`, both on the
/// LIVE ctx. Setting `GenMultiplier=2` before the `New` must double the
/// create-time nominal — proving the executive threads the live ctx, not a
/// parse-default `GenMultiplier=1`.
#[test]
fn create_time_recalc_reads_live_gen_multiplier() {
    // Baseline: default GenMultiplier=1.0 → Pnom = 1000·kW / 3 phases.
    let base = {
        let mut dss = Dss::new();
        dss.command("New circuit.t basekv=12.47");
        dss.command("New Generator.g1 bus1=sourcebus phases=3 kv=12.47 kw=1000 model=1");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        gen_pnom(&dss)
    };
    assert!(
        (base - 1000.0 * 1000.0 / 3.0).abs() < 1e-6,
        "baseline pnom {base}"
    );

    // GenMultiplier=2.0 moved LIVE *before* the generator is created.
    let scaled = {
        let mut dss = Dss::new();
        dss.command("New circuit.t basekv=12.47");
        dss.command("Set genmult=2");
        dss.command("New Generator.g1 bus1=sourcebus phases=3 kv=12.47 kw=1000 model=1");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        gen_pnom(&dss)
    };
    assert!(
        (scaled - 2.0 * base).abs() < 1e-6,
        "live GenMultiplier=2 must double create-time pnominal: {scaled} vs base {base}"
    );
}

/// The edit path (`Edit`) runs `end_edit` on the LIVE ctx. Moving
/// `GenMultiplier` between the `New` and a later `Edit` must be picked up by the
/// edit-time recalc — guarding the `end_edit` call site independently of create.
#[test]
fn end_edit_recalc_reads_live_gen_multiplier() {
    let mut dss = Dss::new();
    dss.command("New circuit.t basekv=12.47");
    dss.command("New Generator.g1 bus1=sourcebus phases=3 kv=12.47 kw=1000 model=1");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let base = gen_pnom(&dss);
    assert!(
        (base - 1000.0 * 1000.0 / 3.0).abs() < 1e-6,
        "baseline {base}"
    );

    // Move GenMultiplier live, then EDIT: end_edit's recalc must read 4.0.
    dss.command("Set genmult=4");
    dss.command("Edit Generator.g1 kw=1000");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let edited = gen_pnom(&dss);
    assert!(
        (edited - 4.0 * base).abs() < 1e-6,
        "live GenMultiplier=4 at Edit must quadruple pnominal: {edited} vs base {base}"
    );
}

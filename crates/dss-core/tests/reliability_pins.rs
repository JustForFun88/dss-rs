//! **GOLDEN_REBASE G1.6(i) — the `Meters` reliability surface's gate-level
//! pins.**
//!
//! `harness::RELIABILITY_SKIP_FIELDS` stops the value comparison of two
//! per-phase arrays — `Meters.CalcCurrent` and `Meters.AllocFactors` — on both
//! oracle channels, because both oracles read them out of **uninitialized
//! memory** until a deck has run the executive `AllocateLoads`:
//! `TMeterElement.AllocateSensorArrays` `ReallocMem`s `CalculatedCurrent` and
//! `PhsAllocationFactor` **without zeroing them** (r4133
//! `Version8/Source/Meters/MeterElement.pas:45-52`; capi 0.14.5 carries the
//! identical code), and the only thing that ever writes them is
//! `TMeterElement.CalcAllocationFactors` (`:54-72`), whose sole driver is
//! `TExecHelper.DoAllocateLoadsCmd` (r4133
//! `Version8/Source/Executive/ExecHelper.pas:2624-2683`).
//!
//! CLAUDE.md's discipline is that such an exclusion is pinned by its own
//! expected-value test naming **both** numbers, or it is a mask over nothing.
//! Here the oracle half of "both numbers" is heap garbage that changes between
//! processes — measured on `controls:combo/combo_protection.dss` with three
//! fresh `epri-worker` processes (G1.6(i) part R):
//! `[2.806806272625585e-309, 2.121995791e-314, 2.37e-322]` in run 1 and
//! `[…, …, 2.4e-322]` in runs 2-3 — so an envelope is impossible and only a pin
//! can hold the port's value. That pin is
//! [`meter_alloc_factors_are_zero_until_allocateloads_runs`], which every row of
//! the table names.
//!
//! The exclusion's predicate is the **port's own** regime
//! (`harness::reliability_skip_applies`: all factors still exactly zero), so it
//! evaporates the moment a deck allocates. `controls:energymeter/midi_relcalc.dss`
//! is that deck — the only corpus case that runs `AllocateLoads`, added by this
//! sub-step for exactly this reason — and
//! [`meter_allocation_factors_are_the_peak_current_over_the_metered_current`]
//! is the affirmative half: there both fields are defined, live-compared on both
//! channels, and equal to the peak-over-current identity to the last bit.
//!
//! # The read-order contract and the registry guard
//!
//! Two more families of static test live at the end of this file. The first
//! asserts the three rules every transport that captures this surface must
//! obey — `SetActiveSection` before every section field, `Meters.Totals` last
//! (it destroys the meter cursor), and the payload's slot between the meters
//! and the PD elements — literally, over the two capture bodies, with a
//! negative drive that rejects corrupted copies of them. The second closes the
//! loop on `harness::RELIABILITY_SKIP_FIELDS`' `pin` strings: the harness can
//! check that they are non-empty, but only a scan from outside can check that
//! the `#[test]` each one names still exists.
//!
//! # G1.6(ii) — the per-bus half
//!
//! The trailing block of this file pins the eight per-bus reliability columns
//! (`Bus.Lambda / N_interrupts / Int_Duration / Cust_Interrupts /
//! Cust_Duration / TotalMiles / N_Customers / SectionID`) that
//! `harness::compare_bus_reliability` compares exactly, per bus, on both
//! oracle channels: four expected-value tables carrying the port's and both
//! oracles' numbers, the record of the `Bus.Int_Duration` zone question, a
//! four-sided column census, and the bus capture's own read-order contract
//! with its negative drive. It has its own banner comment; the G1.6(i) rules
//! above are untouched by it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use dss_core::exec::Dss;

// ---------------------------------------------------------------------------
// Plumbing (the `pd_elements_pins.rs` shape — same corpus, same guarantees)
// ---------------------------------------------------------------------------

/// The repository root — the worktree that holds `crates/`, `tools/` and
/// `tests/`.
fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

/// Every regular file under `dir`, recursively, with its byte length.
fn walk_dir(dir: &Path, out: &mut BTreeMap<PathBuf, u64>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let path = e.path();
        match e.file_type() {
            Ok(t) if t.is_dir() => walk_dir(&path, out),
            Ok(t) if t.is_file() => {
                out.insert(path, e.metadata().map(|m| m.len()).unwrap_or_default());
            }
            _ => {}
        }
    }
}

/// Keeps the corpus deck directory exactly as it was found — the miniature of
/// `corpus_gate/runner.rs`'s `CorpusGuard`, copied from `pd_elements_pins.rs`
/// for the same reason: the decks compiled here are definition-only today, so
/// the guard is insurance against a later edit that adds an `Export`/`Show`.
struct DeckDirGuard {
    dir: PathBuf,
    before: BTreeMap<PathBuf, u64>,
}

impl DeckDirGuard {
    fn new(dir: &Path) -> Self {
        let mut before = BTreeMap::new();
        walk_dir(dir, &mut before);
        Self {
            dir: dir.to_path_buf(),
            before,
        }
    }
}

impl Drop for DeckDirGuard {
    fn drop(&mut self) {
        let mut after = BTreeMap::new();
        walk_dir(&self.dir, &mut after);
        let mut changed = Vec::new();
        for (path, len) in &after {
            match self.before.get(path) {
                None => {
                    let _ = std::fs::remove_file(path);
                }
                Some(was) if was != len => changed.push(path.display().to_string()),
                Some(_) => {}
            }
        }
        // Never mask the real failure by panicking while another panic unwinds.
        if !std::thread::panicking() {
            assert!(
                changed.is_empty(),
                "a pinned deck rewrote corpus bytes (not merely added output \
                 files), which this guard cannot restore: {changed:?}"
            );
        }
    }
}

/// Compile and solve one corpus deck (path relative to `tests/corpus/`),
/// requiring a clean run.
fn deck(rel: &str) -> (Dss, DeckDirGuard) {
    deck_steps(rel, 1)
}

/// [`deck`], but solving `steps` times — the corpus gate's own step protocol
/// (`corpus_gate/runner.rs` solves once per checkpoint and drives `RelCalc`
/// after the last one), so a pin can reproduce exactly the state the live
/// comparison saw.
fn deck_steps(rel: &str, steps: usize) -> (Dss, DeckDirGuard) {
    assert!(steps >= 1, "a case has at least one step");
    let path = repo_root().join("tests").join("corpus").join(rel);
    assert!(path.is_file(), "corpus deck is missing: {}", path.display());
    let guard = DeckDirGuard::new(path.parent().expect("a deck has a directory"));
    let mut dss = Dss::new();
    dss.command(&format!("compile \"{}\"", path.display()));
    for _ in 0..steps {
        dss.command("solve");
    }
    assert!(
        dss.errors().is_empty(),
        "{rel} must compile and solve clean: {:?}",
        dss.errors()
    );
    (dss, guard)
}

// ---------------------------------------------------------------------------
// The `RELIABILITY_SKIP_FIELDS` pin
// ---------------------------------------------------------------------------

/// **The pin every `harness::RELIABILITY_SKIP_FIELDS` row names.**
///
/// Both numbers, on the deck where they were measured
/// (`controls:combo/combo_protection.dss`, the `r4133`-gated case whose
/// `Meters.AllocFactors` proved the defect):
///
/// | side | `Meters.AllocFactors` |
/// |---|---|
/// | port (here) | `[0.0, 0.0, 0.0]` — exactly zero, every run |
/// | r4133 oracle, run 1 | `[2.806806272625585e-309, 2.121995791e-314, 2.37e-322]` |
/// | r4133 oracle, runs 2-3 | `[2.806806272625585e-309, 2.121995791e-314, 2.4e-322]` |
///
/// The oracle's third slot **changes between processes**, which is what makes
/// the cell un-envelopable and therefore a harness exclusion rather than a
/// `tests/corpus/ledger.json` row (coordinator decision D4; the
/// `PD_SKIP_FIELDS` precedent). The port's value is right by construction:
/// `MeterElementData::new` zero-initializes both arrays
/// (`elements/meter/meter_element.rs:106,111`) and only
/// `calc_allocation_factors` (`:117-136`, the port of
/// `MeterElement.pas:54-72`) writes them.
///
/// The reason the port's zeros are the *correct* answer and not merely a
/// different arbitrary value: `CalcAllocationFactors` has demonstrably not run
/// on this deck — it has no `AllocateLoads` — so no defined value exists, and
/// zero is what a safe engine reports for storage it has not written.
#[test]
fn meter_alloc_factors_are_zero_until_allocateloads_runs() {
    let (dss, _guard) = deck("controls/combo/combo_protection.dss");
    let walk = dss.meter_reliability();
    assert!(
        !walk.is_empty(),
        "combo_protection.dss defines an EnergyMeter, so the walk must not be empty"
    );
    for m in &walk {
        assert!(
            m.alloc_factors.iter().all(|x| *x == 0.0),
            "meter {}: the port must report exactly zero allocation factors on a deck that \
             never ran `AllocateLoads` (got {:?}); the r4133 oracle reports uninitialized heap \
             there — [2.806806272625585e-309, 2.121995791e-314, 2.37e-322] in one process and \
             2.4e-322 in the third slot in the next two",
            m.name,
            m.alloc_factors,
        );
        assert!(
            m.calc_current.iter().all(|x| *x == 0.0),
            "meter {}: same for the calculated currents (got {:?})",
            m.name,
            m.calc_current,
        );
        assert_eq!(
            m.alloc_factors.len(),
            m.calc_current.len(),
            "meter {}: both arrays are NPhases long",
            m.name,
        );
        assert!(
            !m.alloc_factors.is_empty(),
            "meter {}: NPhases is at least one, so an all-zero array is a measured value and \
             not an empty one (an empty array would make the assertions above vacuous)",
            m.name,
        );
    }

    // The exclusion's predicate is the port's own regime, so it must *stop*
    // applying on the one deck that allocates — otherwise the two fields would
    // be excluded everywhere and the table would mask a surface nobody ever
    // compares.
    let (allocated, _guard2) = deck("controls/energymeter/midi_relcalc.dss");
    let m = &allocated.meter_reliability()[0];
    assert!(
        m.alloc_factors.iter().all(|x| *x != 0.0),
        "controls:energymeter/midi_relcalc.dss runs `AllocateLoads`, so every allocation factor \
         must be non-zero there and `RELIABILITY_SKIP_FIELDS` must not apply: {:?}",
        m.alloc_factors,
    );
}

// ---------------------------------------------------------------------------
// The affirmative half: the surface where the two arrays are compared for real
// ---------------------------------------------------------------------------

/// **`controls:energymeter/midi_relcalc.dss` — the allocation identity, with
/// both oracles' numbers.**
///
/// `TMeterElement.CalcAllocationFactors` (r4133
/// `Version8/Source/Meters/MeterElement.pas:54-72`) is
/// `PhsAllocationFactor[i] := SensorCurrent[i] / Cabs(CalculatedCurrent[i])`,
/// with the `ELSE PhsAllocationFactor^[i] := 1.0` branch when the magnitude is
/// zero. `SensorCurrent` is the deck's `peakcurrent=[120, 100, 90]`, so the pin
/// is a pure identity on the port's own two arrays — and it is exact: the port
/// performs the same single f64 division.
///
/// The measured values, per phase, on the three engines (part F3; the two
/// oracles were read through the same transports the live gate uses):
///
/// | k | port `calc_current` | capi 0.14.5 | EPRI r4133 |
/// |---|---|---|---|
/// | 0 | `115.69585353499207` | `115.69585353499478` | `115.69585353499097` |
/// | 1 | `84.8661220964024`   | `84.86612209639719`  | `84.86612209639911` |
/// | 2 | `85.38036092989663`  | `85.38036092989765`  | `85.38036092989825` |
///
/// | k | port `alloc_factors` | capi 0.14.5 | EPRI r4133 |
/// |---|---|---|---|
/// | 0 | `1.0372022534386347` | `1.0372022534386103` | `1.0372022534386445` |
/// | 1 | `1.1783264927129167` | `1.178326492712989`  | `1.1783264927129624` |
/// | 2 | `1.0541065769667621` | `1.0541065769667495` | `1.0541065769667421` |
///
/// The two **independent oracle engines** differ from each other by up to
/// `3.30e-14` relative here (phase 0) while agreeing bit-for-bit on every other
/// cell of the same payload — every scalar, all nine fields of all three
/// sections, and all 67 `Meters.Totals` slots. That is the proof that these two
/// arrays alone are solve-derived: they are `|GetCurrents|` of the metered
/// element at the solve `CalcAllocationFactors` ran on, and they therefore
/// carry the current tier (`harness::reliability_array_band`, derived in
/// `tests/TOLERANCE_NOTES.md`), not this surface's exactness rule. The port's
/// worst gap against either oracle is `6.14e-14` relative (phase 1 vs capi),
/// i.e. `1.9x` the spread the two oracles already leave between themselves —
/// faer-vs-KLU on the same footing as KLU-vs-KLU, four orders inside the micro
/// tier's `i_rel = 1e-9`.
#[test]
fn meter_allocation_factors_are_the_peak_current_over_the_metered_current() {
    // `peakcurrent=[120, 100, 90]` in the deck; the EnergyMeter constructor's
    // default would be 400 A on every phase, so a mis-parsed property would
    // change every factor by ~4x.
    const PEAK: [f64; 3] = [120.0, 100.0, 90.0];
    // The spread the two oracles leave between themselves on this deck (worst
    // phase, |capi - r4133| / |r4133|). The port's own worst gap against either
    // of them is 6.14e-14 = 1.9x that, so the pin admits 10x — enough that a
    // solver-rounding difference never reds it, ~4 orders tighter than the
    // micro tier the live gate applies, and far too tight for any real change
    // in the allocation arithmetic to slip through.
    const ORACLE_SPREAD_REL: f64 = 3.30e-14;
    // capi 0.14.5, measured through `tools/oracle/oracle_server.py`'s own read
    // path (`Meters.CalcCurrent`, `Meters.AllocFactors`).
    const CAPI_CALC: [f64; 3] = [115.69585353499478, 84.86612209639719, 85.38036092989765];
    const CAPI_ALLOC: [f64; 3] = [1.0372022534386103, 1.178326492712989, 1.0541065769667495];
    // EPRI r4133, measured through the `epri-worker` DDLL bridge (`MetersV` 6
    // and 8).
    const R4133_CALC: [f64; 3] = [115.69585353499097, 84.86612209639911, 85.38036092989825];
    const R4133_ALLOC: [f64; 3] = [1.0372022534386445, 1.1783264927129624, 1.0541065769667421];

    let (mut dss, _guard) = deck("controls/energymeter/midi_relcalc.dss");
    let walk = dss.meter_reliability();
    assert_eq!(walk.len(), 1, "the deck defines exactly one EnergyMeter");
    let m = &walk[0];
    assert_eq!(m.name, "em");
    assert_eq!(m.calc_current.len(), 3, "a 3-phase metered element");
    assert_eq!(m.alloc_factors.len(), 3);

    for k in 0..3 {
        // The identity, exactly — one f64 division, the same on all three
        // engines.
        assert_eq!(
            m.alloc_factors[k],
            PEAK[k] / m.calc_current[k],
            "phase {k}: AllocFactors must be SensorCurrent / |CalculatedCurrent| to the last \
             bit (peak {}, |I| {})",
            PEAK[k],
            m.calc_current[k],
        );
        for (tag, calc, alloc) in [
            ("capi_v0145", CAPI_CALC[k], CAPI_ALLOC[k]),
            ("r4133", R4133_CALC[k], R4133_ALLOC[k]),
        ] {
            let allowed = 10.0 * ORACLE_SPREAD_REL * calc.abs();
            assert!(
                (m.calc_current[k] - calc).abs() <= allowed,
                "phase {k}: port calc_current {} vs {tag} {calc} (|diff| {:.3e} > {allowed:.3e})",
                m.calc_current[k],
                (m.calc_current[k] - calc).abs(),
            );
            let allowed = 10.0 * ORACLE_SPREAD_REL * alloc.abs();
            assert!(
                (m.alloc_factors[k] - alloc).abs() <= allowed,
                "phase {k}: port alloc_factors {} vs {tag} {alloc} (|diff| {:.3e} > \
                 {allowed:.3e})",
                m.alloc_factors[k],
                (m.alloc_factors[k] - alloc).abs(),
            );
        }
    }

    // The rest of the payload this deck exists to expose: three feeder sections
    // behind three Reclosers, every reliability input a deck literal. These are
    // the cells the live gate compares EXACTLY on both channels (the two
    // oracles are bit-identical on all of them, measured in part F3).
    dss_relcalc(&mut dss);
}

/// `RelCalc` is what fills the section fields; the gate drives it once per case
/// on the last step (`corpus_gate/runner.rs`), so a pin that wants them has to
/// drive it too. Kept separate so the identity above is provably independent of
/// it: `CalcCurrent`/`AllocFactors` come from `AllocateLoads`, not from
/// `RelCalc`.
fn dss_relcalc(dss: &mut Dss) {
    let before = dss.errors().len();
    dss.command("RelCalc");
    assert_eq!(
        dss.errors().len(),
        before,
        "the deck has three Reclosers, so no meter zone aborts at errno 52902: {:?}",
        &dss.errors()[before..],
    );
    let m = &dss.meter_reliability()[0];
    assert_eq!(
        m.num_sections, 3,
        "three Reclosers -> three feeder sections"
    );
    assert_eq!(m.total_customers, 30, "17 + 9 + 4 zone customers");
    let secs: Vec<(i32, i32, i32, i32, i32)> = m
        .sections
        .iter()
        .map(|s| {
            (
                s.idx,
                s.num_section_customers,
                s.num_section_branches,
                s.sect_seq_idx,
                s.ocp_device_type,
            )
        })
        .collect();
    assert_eq!(
        secs,
        vec![(1, 0, 1, 1, 2), (2, 13, 2, 2, 2), (3, 17, 1, 4, 2)],
        "the discrete section table both oracles report bit-identically \
         (OCPDeviceType 2 = Recloser)"
    );
}
// ---------------------------------------------------------------------------
// The `RelCalc` indices themselves — both oracles' numbers (part F4)
// ---------------------------------------------------------------------------

/// **`modes:time/midi_duty_ctrl.dss` — every reliability index, with both
/// oracles' numbers.**
///
/// This is the sub-step's clean `both`-channel witness: the two independent
/// oracle engines return **bit-identical** doubles for every value below, and
/// the port returns the same bits again. That is what licenses the exactness
/// rule `harness::compare_reliability` applies (`rel = abs = 0`); a floor here
/// would be masking an order bug, since the arithmetic is nothing but sums of
/// integer customer counts times deck literals in a fixed zone-walk order
/// (r4133 `Version8/Source/Meters/EnergyMeter.pas:2470-2616`; port
/// `solution/meters/reliability.rs`).
///
/// | value | port (here) | capi 0.14.5 | EPRI r4133 |
/// |---|---|---|---|
/// | `TotalCustomers` | 2 | 2 | 2 |
/// | `SAIFI` | `0.05600000000000001` | idem | idem |
/// | `SAIFIkW` | `0.05600000000000001` | idem | idem |
/// | `SAIDI` | `0.16799999999999998` | idem | idem |
/// | `CustInterrupts` | `0.11200000000000002` | idem | idem |
/// | `NumSections` | 1 | 1 | 1 |
/// | section 1 `OCPDeviceType` | 2 | 2 | 2 |
/// | section 1 `NumSectionCustomers` | 2 | 2 | 2 |
/// | section 1 `NumSectionBranches` | 3 | 3 | 3 |
/// | section 1 `SectSeqIdx` | 1 | 1 | 1 |
/// | section 1 `SectTotalCust` | 2 | 2 | 2 |
/// | section 1 `SumBranchFltRates` | `0.0031360000000000008` | idem | idem |
/// | section 1 `AvgRepairTime` | `2.999999999999999` | idem | idem |
/// | section 1 `FaultRateXRepairHrs` | `0.009408` | idem | idem |
///
/// The capi column was read through `tools/oracle/oracle_server.py`'s own read
/// path (G1.6(i) part R), the r4133 column through the `epri-worker` DDLL
/// bridge (`MetersI` 20/21, `MetersF` 0-3, `MetersI` 22-27 per section). Both
/// are re-confirmed live by the corpus gate, which compares every one of them
/// on both channels for this case.
///
/// The three zone lists are pinned in their **order** too, because since part
/// F4 `compare_reliability` asserts the sequence and not merely the membership:
/// both oracles emit `["Line.feed", "Line.lat2", "Line.lat1"]`.
///
/// The payload is **step-invariant**: solving the deck's 20 duty-cycle steps
/// (what the gate does) and solving it once give identical values, asserted
/// below. `RelCalc` reads structure and deck literals, not accumulated energy.
#[test]
fn relcalc_indices_match_both_oracles_on_the_duty_deck() {
    // Both oracle channels returned exactly these bits.
    const SAIFI: f64 = 0.05600000000000001;
    const SAIFI_KW: f64 = 0.05600000000000001;
    const SAIDI: f64 = 0.16799999999999998;
    const CUST_INTERRUPTS: f64 = 0.11200000000000002;
    const SUM_BRANCH_FLT_RATES: f64 = 0.0031360000000000008;
    const AVG_REPAIR_TIME: f64 = 2.999999999999999;
    const FAULT_RATE_X_REPAIR_HRS: f64 = 0.009408;

    // The gate's own protocol on this case: `n_steps = 20`, then one `RelCalc`
    // on the last step.
    let (mut dss, _guard) = deck_steps("modes/time/midi_duty_ctrl.dss", 20);
    let before = dss.errors().len();
    dss.command("RelCalc");
    assert_eq!(
        dss.errors().len(),
        before,
        "the zone holds a Recloser, so nothing aborts at errno 52902: {:?}",
        &dss.errors()[before..],
    );

    let walk = dss.meter_reliability();
    assert_eq!(walk.len(), 1, "the deck defines exactly one EnergyMeter");
    let m = &walk[0];
    assert_eq!(m.name, "em");
    assert_eq!(m.total_customers, 2);
    assert_eq!(m.saifi, SAIFI, "SAIFI");
    assert_eq!(m.saifi_kw, SAIFI_KW, "SAIFIkW");
    assert_eq!(m.saidi, SAIDI, "SAIDI");
    assert_eq!(m.cust_interrupts, CUST_INTERRUPTS, "CustInterrupts");
    assert_eq!(m.num_sections, 1);

    assert_eq!(m.sections.len(), 1);
    let s = &m.sections[0];
    assert_eq!(
        (
            s.idx,
            s.ocp_device_type,
            s.num_section_customers,
            s.num_section_branches,
            s.sect_seq_idx,
            s.sect_total_cust,
        ),
        (1, 2, 2, 3, 1, 2),
        "the discrete section row (OCPDeviceType 2 = Recloser)"
    );
    assert_eq!(s.sum_branch_flt_rates, SUM_BRANCH_FLT_RATES);
    assert_eq!(s.avg_repair_time, AVG_REPAIR_TIME);
    assert_eq!(s.fault_rate_x_repair_hrs, FAULT_RATE_X_REPAIR_HRS);

    // Ordered, not merely a set — the contract part F4 turned on.
    assert_eq!(m.branches, ["Line.feed", "Line.lat2", "Line.lat1"]);
    assert_eq!(m.ends, ["Line.lat2", "Line.lat1"]);
    assert_eq!(m.pce, ["Load.lc", "Load.lb"]);

    // Step invariance: the same payload after a single solve.
    let (mut one, _g2) = deck_steps("modes/time/midi_duty_ctrl.dss", 1);
    one.command("RelCalc");
    assert_eq!(
        one.meter_reliability(),
        walk,
        "the reliability payload must not depend on how many steps were solved"
    );
}

/// **CAIDI — the one reliability index neither oracle API exposes (B14).**
///
/// There is no `CAIDI` mode on either channel: dss-python's `IMeters` has no
/// such property and the r4133 DDLL `MetersF` selector stops at 6
/// (`Version8/Source/DDLL/DMeters.pas:329-399` — SAIFI, SAIFIkW, SAIDI,
/// CustInterrupts, AvgRepairTime, FaultRateXRepairHrs, SumBranchFltRates). That does **not** make it
/// "not comparable": `TEnergyMeterObj` publishes it as EnergyMeter property
/// `CAIDI` — 0-based index 22 in `AllPropertyNames`, i.e. property **23** —
/// which `harness::compare_all_properties` already compares live on the capi
/// channel and, since RP4.1, on r4133 as well. This pin records the arithmetic
/// behind that cell and both numbers:
///
/// * the port's f64 `CAIDI` is `2.999999999999999`, exactly `SAIDI / SAIFI`
///   (`0.16799999999999998 / 0.05600000000000001`) — one IEEE division, the
///   same on all three engines;
/// * rendered at the property channel's 11 significant digits it is the string
///   `"3"`, which is exactly what the capi oracle reported for
///   `EnergyMeter.em[22]` when `RelCalc` moved that cell from `"0"`
///   (G1.6(i) part R).
///
/// So the value is gated twice over: numerically here, and live through the
/// property surface on both channels.
#[test]
fn caidi_is_saidi_over_saifi_on_a_reliability_deck() {
    let (mut dss, _guard) = deck_steps("modes/time/midi_duty_ctrl.dss", 20);
    dss.command("RelCalc");
    let m = dss.meter_reliability().remove(0);
    assert_eq!(m.caidi, 2.999999999999999, "CAIDI, in full f64");
    assert_eq!(
        m.caidi,
        m.saidi / m.saifi,
        "CAIDI is SAIDI / SAIFI to the last bit ({} / {})",
        m.saidi,
        m.saifi,
    );

    let props = dss
        .element_properties("EnergyMeter.em")
        .expect("the deck defines EnergyMeter.em");
    let (idx, (name, val)) = props
        .iter()
        .enumerate()
        .find(|(_, (n, _))| n.eq_ignore_ascii_case("CAIDI"))
        .map(|(i, p)| (i, p.clone()))
        .expect("EnergyMeter publishes a CAIDI property");
    assert_eq!(
        (idx, name.as_str(), val.as_str()),
        (22, "CAIDI", "3"),
        "the property channel's cell: 0-based index 22 (property 23), rendered \"3\" at 11 \
         significant digits, which is what the capi oracle reported"
    );
}

/// **The `RelCalc` abort is symmetric — `controls:energymeter/midi_energymeter.dss`.**
///
/// `CalcReliabilityIndices` refuses to run on a meter whose zone contains no
/// overcurrent device: `IF SectionCount = 0 THEN … DoSimpleMsg('No Overcurrent
/// Protection device (Relay, Recloser, or Fuse) defined. Aborting Reliability
/// calc.', 52902)` (r4133 `Version8/Source/Meters/EnergyMeter.pas:2500-2504`).
/// This deck has two EnergyMeters and no OCP device at all, so all three
/// engines abort, and the gate needs that abort to be an **observable** rather
/// than a failure: both transports tolerate errno 52902 in their own narrow
/// scope (`oracle_server.py::_RELCALC_TOLERATED_ERRNOS`,
/// `dss-epri::Engine::relcalc`) and `harness::compare_reliability` asserts the
/// boolean and the message.
///
/// Both numbers, and the one deliberate asymmetry:
///
/// * port: two error lines (one per failing meter —
///   `solution/meters/reliability.rs:53-57` collects per meter), each
///   byte-identical to the oracles' text; every index left at `0.0` and
///   `NumSections = 0` on both meters;
/// * capi 0.14.5 raises `DSSException(52902, 'Error: No Overcurrent Protection
///   device (Relay, Recloser, or Fuse) defined. Aborting Reliability calc.')`
///   **once per command**, and r4133 sets the same errno and text. The counts
///   therefore differ legitimately, which is why the comparator compares the
///   boolean and the message and never the count.
/// * `TotalCustomers` still moves — `24` on `em`, `0` on `em2` — because it is
///   read off the zone head's bus and is not produced by the aborted calc
///   (measured on both channels, G1.6(i) part R).
#[test]
fn relcalc_abort_is_symmetric_on_a_zone_without_ocp() {
    const MSG: &str = "Error: No Overcurrent Protection device (Relay, Recloser, or Fuse) \
                       defined. Aborting Reliability calc.";

    let (mut dss, _guard) = deck_steps("controls/energymeter/midi_energymeter.dss", 24);
    let before = dss.errors().len();
    dss.command("RelCalc");
    let new: Vec<String> = dss.errors()[before..]
        .iter()
        .map(|e| e.message.clone())
        .collect();
    assert_eq!(
        new.len(),
        2,
        "one abort line per failing meter, not one per command: {new:?}"
    );
    for line in &new {
        assert_eq!(
            line.trim(),
            MSG,
            "the abort text is byte-identical on all three engines"
        );
    }

    let walk = dss.meter_reliability();
    assert_eq!(
        walk.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
        ["em", "em2"],
        "both meters are enabled, so both are walked"
    );
    for m in &walk {
        assert_eq!(
            m.num_sections, 0,
            "meter {}: no OCP device -> no section",
            m.name
        );
        assert!(m.sections.is_empty(), "meter {}", m.name);
        assert_eq!(m.saifi, 0.0, "meter {}", m.name);
        assert_eq!(m.saifi_kw, 0.0, "meter {}", m.name);
        assert_eq!(m.saidi, 0.0, "meter {}", m.name);
        assert_eq!(m.cust_interrupts, 0.0, "meter {}", m.name);
    }
    assert_eq!(
        (walk[0].total_customers, walk[1].total_customers),
        (24, 0),
        "TotalCustomers is the zone head's bus count and survives the abort"
    );
}

// ---------------------------------------------------------------------------
// G1.6b's deferred non-vacuity demo, discharged here
// ---------------------------------------------------------------------------

/// **The four `RelCalc`-fed `PDElements` fields are live, and oracle-compared.**
///
/// G1.6b landed the 14-field `PDElements` walk with `section_id`, `total_miles`,
/// `lambda` and `accumulated_l` reading `0` on every gated case, because no live
/// corpus deck ran `RelCalc` — so comparing them proved only that the port does
/// not populate them early, and `harness::compare_pd_elements`' doc recorded the
/// non-vacuity demo as owed by G1.6(i). This is that demo.
///
/// Two halves, both required:
///
/// 1. **Live values.** After the executive `RelCalc` the four fields carry real
///    accumulated sums (`PDElement.pas:89-178`), measured here:
///    `modes:time/midi_duty_ctrl.dss` 3 of 3 PD rows
///    (`Line.feed`: `section_id 1`, `total_miles 2.8`, `lambda 0.02`,
///    `accumulated_l 0.05600000000000001`),
///    `modes:makeposseq/makeposseq_ctrl.dss` 4 of 6
///    (`Line.l2`: `1`, `1.2437423844746678`, `0.02`, `0.02002`) and
///    `controls:combo/midi_protection.dss` 37 of 41
///    (`Line.bb1_2`: `1`, `14.204545454545457`, `0.04`, `1.0605`).
/// 2. **Oracle-compared, on both channels.** Those values are not checked
///    against a stored oracle copy here — the live gate does that, exactly and
///    on every gated case, and it does so *because* these three cases carry
///    `compare_reliability: true`, which is what makes `corpus_gate/runner.rs`
///    drive `RelCalc` before `compare_pd_elements` runs. That flag is what the
///    second half asserts, per channel: `midi_duty_ctrl` gates on `both`,
///    `makeposseq_ctrl` on `capi_v0145`, `midi_protection` on `r4133`. Unflag
///    any of them and the four fields silently fall back to comparing `0 == 0`
///    — the regression this half exists to catch.
///
/// The complementary port-side pins (a shunt PD element stays `0` even after a
/// successful `RelCalc`, because it lives on the meter's PC list and never
/// enters `SequenceList`) are in `tests/pd_elements_pins.rs`.
/// One row of [`pd_elements_relcalc_fields_are_live_after_relcalc`]: a corpus
/// deck, the gate's step count for it, its manifest family and gating channel,
/// and one witness PD row with its `(section_id, total_miles, lambda,
/// accumulated_l)` after `RelCalc`.
struct RelCalcLiveCase {
    deck: &'static str,
    steps: usize,
    family: &'static str,
    engines: &'static str,
    witness: &'static str,
    want: (i32, f64, f64, f64),
}

#[test]
fn pd_elements_relcalc_fields_are_live_after_relcalc() {
    let cases = [
        RelCalcLiveCase {
            deck: "modes/time/midi_duty_ctrl.dss",
            steps: 20,
            family: "modes",
            engines: "both",
            witness: "Line.feed",
            want: (1, 2.8, 0.02, 0.05600000000000001),
        },
        RelCalcLiveCase {
            deck: "modes/makeposseq/makeposseq_ctrl.dss",
            steps: 1,
            family: "modes",
            engines: "capi_v0145",
            witness: "Line.l2",
            want: (1, 1.2437423844746678, 0.02, 0.02002),
        },
        RelCalcLiveCase {
            deck: "controls/combo/midi_protection.dss",
            steps: 24,
            family: "controls",
            engines: "r4133",
            witness: "Line.bb1_2",
            want: (1, 14.204545454545457, 0.04, 1.0605),
        },
    ];

    for RelCalcLiveCase {
        deck: rel,
        steps,
        family,
        engines,
        witness,
        want,
    } in cases
    {
        let (mut dss, _guard) = deck_steps(rel, steps);
        dss.command("RelCalc");
        let walk = dss.pd_elements();
        let live = walk
            .iter()
            .filter(|v| {
                v.section_id != 0
                    || v.total_miles != 0.0
                    || v.lambda != 0.0
                    || v.accumulated_l != 0.0
            })
            .count();
        assert!(
            live > 0,
            "{rel}: after `RelCalc` at least one PD row must carry a live reliability field, \
             or the four fields are compared as 0 == 0 and the surface is vacuous"
        );
        let row = walk
            .iter()
            .find(|v| v.name.eq_ignore_ascii_case(witness))
            .unwrap_or_else(|| panic!("{rel}: no PD row named {witness}"));
        assert_eq!(
            (
                row.section_id,
                row.total_miles,
                row.lambda,
                row.accumulated_l
            ),
            want,
            "{rel}: {witness}'s (section_id, total_miles, lambda, accumulated_l)"
        );

        // The other half: this case really does drive `RelCalc` in the gate.
        assert_manifest_gates_reliability(family, rel, engines);
    }
}

/// Assert that the corpus manifest carries `compare_reliability: true` on the
/// named case and that it gates on the expected channel(s) — the two facts that
/// make [`pd_elements_relcalc_fields_are_live_after_relcalc`]'s claim
/// "oracle-compared on both channels" true.
fn assert_manifest_gates_reliability(family: &str, deck_rel: &str, engines: &str) {
    let manifest = repo_root()
        .join("tests")
        .join("corpus")
        .join(family)
        .join("manifest.json");
    let text = std::fs::read_to_string(&manifest)
        .unwrap_or_else(|e| panic!("{}: {e}", manifest.display()));
    let json: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", manifest.display()));
    // A manifest `path` is relative to its own family directory.
    let want_path = deck_rel
        .strip_prefix(&format!("{family}/"))
        .expect("the deck path starts with its family");
    let case = json["cases"]
        .as_array()
        .expect("`cases` is an array")
        .iter()
        .find(|c| c["path"].as_str() == Some(want_path))
        .unwrap_or_else(|| panic!("{want_path} is missing from {}", manifest.display()));
    assert_eq!(
        case["compare_reliability"].as_bool(),
        Some(true),
        "{want_path} must keep `compare_reliability: true`: it is what makes the gate drive \
         the executive `RelCalc` before `compare_pd_elements`, and therefore what makes \
         `section_id`/`total_miles`/`lambda`/`accumulated_l` non-vacuous on this channel"
    );
    assert_eq!(
        case["engines"].as_str(),
        Some(engines),
        "{want_path} must keep gating on `{engines}`"
    );
}

// ---------------------------------------------------------------------------
// The read-order contract, asserted over both capture sources
// ---------------------------------------------------------------------------
//
// `GOLDEN_REBASE_PLAN.md` §1.1(a) / coordinator decision D3. Three rules bind
// every transport that captures this surface, and none of them is expressible
// in a type system — one transport is Python (`tools/oracle/oracle_server.py`),
// the other a `libloading` bridge (`crates/dss-epri/src/capture.rs`), and a
// capture that broke a rule would still LOOK fine: it would simply compare the
// port against a stale section, or against a walk that lost its meters.
//
// 1. **`SetActiveSection` before every section field.** The selection lives on
//    the METER (`COM_ActiveSection`, capi `CAPI/CAPI_Meters.pas:729-740`;
//    `pMeter.ActiveSection`, r4133 `Version8/Source/DDLL/DMeters.pas:254-262`)
//    and the `First`/`Next` walk never resets it, so a section read without a
//    preceding selection answers from the previously selected section — or,
//    with none selected, from `InvalidActiveSection`
//    (`CAPI_Meters.pas:122-134`), which under `DSS_CAPI_EXT_ERRORS` raises
//    `#5055`.
// 2. **`Meters.Totals` LAST, after the walk has ended.** It calls
//    `TotalizeMeters` (capi `CAPI_Meters.pas:279-290` -> `Common/Circuit.pas`
//    `:2347-2360`; r4133 `DMeters.pas:558-573` -> `Circuit.pas:2520-2538`),
//    which walks `EnergyMeters.First`/`Next` itself and destroys the cursor.
//    fastdss says so verbatim (`DSS-Python@origin/fastdss:`
//    `tests/save_outputs.py:330-332`, "This breaks the iteration") and G1.6(i)
//    part R measured it: a mid-walk `Totals` read on
//    `controls:energymeter/midi_energymeter.dss` truncates the clean walk
//    `['em', 'em2']` to `['em']`.
// 3. **The payload's slot is between the meters and the PD elements**, on both
//    transports, so the two channels issue the same reads at the same point of
//    a step.
//
// The scans below assert all three literally, plus each transport's full read
// sequence; [`the_reliability_read_order_guard_rejects_a_contaminated_capture`]
// re-runs them over deliberately corrupted copies of the real bodies so a scan
// that silently matched nothing cannot pass.

/// One repository source file, read whole.
fn read_source(rel: &str) -> String {
    let path = repo_root().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Drop every `marker`-introduced line comment (the capture bodies document
/// their own reads, and those citations must not read as reads).
fn strip_line_comments(src: &str, marker: &str) -> String {
    src.lines()
        .map(|l| match l.find(marker) {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn leading_ident(s: &str) -> &str {
    let end = s
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(s.len());
    &s[..end]
}

/// `tools/oracle/oracle_server.py::capture_reliability`, docstring and `#`
/// comments removed.
fn capi_reliability_body() -> String {
    let src = read_source("tools/oracle/oracle_server.py");
    let start = src
        .find("def capture_reliability(ckt, aborted: bool, message: str) -> dict:")
        .expect("oracle_server.py has no capture_reliability");
    let body = &src[start..];
    let end = body
        .find("\n    return {")
        .expect("capture_reliability must end in the payload dict");
    let body = &body[..end];
    // The docstring quotes `Meters.Totals`, `SetActiveSection` and the Pascal
    // arms; it is prose, not reads.
    let body = match (body.find("\"\"\""), body.rfind("\"\"\"")) {
        (Some(a), Some(b)) if b > a => format!("{}{}", &body[..a], &body[b + 3..]),
        _ => body.to_string(),
    };
    strip_line_comments(&body, "#")
}

/// `crates/dss-epri/src/capture.rs::capture_reliability`, `//` comments removed
/// (its `///` doc block sits above the signature and is never in the slice).
fn r4133_reliability_body() -> String {
    let src = read_source("crates/dss-epri/src/capture.rs");
    let start = src
        .find("pub fn capture_reliability(")
        .expect("capture.rs has no capture_reliability");
    let body = &src[start..];
    let end = body
        .find("\n}")
        .expect("capture_reliability must close at column 0");
    strip_line_comments(&body[..end], "//")
}

/// `crates/dss-epri/src/capture.rs::run_case`'s body.
fn r4133_run_case_body() -> String {
    let src = read_source("crates/dss-epri/src/capture.rs");
    let start = src
        .find("pub fn run_case(engine: &Engine, req: &RunRequest)")
        .expect("capture.rs has no run_case");
    let body = &src[start..];
    let end = body.find("\n}").expect("run_case must close at column 0");
    strip_line_comments(&body[..end], "//")
}

/// Every read the capi capture issues through its `Meters` handle `m`, in
/// source order (Python evaluates statements and a dict literal's values left
/// to right, so source order **is** read order).
fn capi_read_sequence(body: &str) -> Vec<String> {
    let bytes = body.as_bytes();
    let mut out = Vec::new();
    for (i, _) in body.match_indices("m.") {
        // `m` must be the whole identifier, not the tail of another one.
        if i > 0 {
            let prev = bytes[i - 1] as char;
            if prev.is_ascii_alphanumeric() || prev == '_' {
                continue;
            }
        }
        let ident = leading_ident(&body[i + "m.".len()..]);
        if !ident.is_empty() {
            out.push(format!("m.{ident}"));
        }
    }
    out
}

/// Every typed `Meters` mode accessor the r4133 capture calls, in source order.
/// The filter is the `meter` prefix, so `engine.assert_clean("reliability")` —
/// an error-channel check, not a read — is correctly not a read.
fn r4133_read_sequence(body: &str) -> Vec<String> {
    body.match_indices("engine.meter")
        .map(|(i, _)| leading_ident(&body[i + "engine.".len()..]).to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// The capi capture's read sequence, `First`/`Next`/`Totals` included.
const CAPI_RELIABILITY_ORDER: &[&str] = &[
    "m.First",
    "m.Name",
    "m.AllocFactors",
    "m.AllEndElements",
    "m.SAIFIKW",
    "m.SAIDI",
    "m.TotalCustomers",
    "m.SAIFI",
    "m.CustInterrupts",
    "m.CalcCurrent",
    "m.AllBranchesInZone",
    "m.ZonePCE",
    "m.NumSections",
    "m.SetActiveSection",
    "m.NumSectionCustomers",
    "m.NumSectionBranches",
    "m.SectSeqIdx",
    "m.SectTotalCust",
    "m.OCPDeviceType",
    "m.SumBranchFltRates",
    "m.AvgRepairTime",
    "m.FaultRateXRepairHrs",
    "m.Next",
    "m.Totals",
];

/// The r4133 capture's read sequence. The per-meter prefix is the capi one
/// field for field; the section block is the same eight fields in the r4133
/// `_columns` order rather than discrete-then-continuous, which is free — a
/// section field is a pure getter on the already-selected section and mutates
/// nothing (`DMeters.pas:254-262`, `:329-399`), so only its position relative
/// to `SetActiveSection` is contractual. The two orders are therefore compared
/// as SETS across transports, and literally within one
/// ([`the_two_reliability_captures_read_the_same_fields`]).
const R4133_RELIABILITY_ORDER: &[&str] = &[
    "meters_first",
    "meter_name",
    "meters_alloc_factors",
    "meter_all_end_elements",
    "meters_saifi_kw",
    "meters_saidi",
    "meters_total_customers",
    "meters_saifi",
    "meters_cust_interrupts",
    "meters_calc_current",
    "meter_all_branches_in_zone",
    "meter_zone_pce",
    "meters_num_sections",
    "meters_set_active_section",
    "meters_num_section_customers",
    "meters_sect_seq_idx",
    "meters_sum_branch_flt_rates",
    "meters_avg_repair_time",
    "meters_sect_total_cust",
    "meters_ocp_device_type",
    "meters_fault_rate_x_repair_hrs",
    "meters_num_section_branches",
    "meters_next",
    "meters_totals",
];

/// One transport's spelling of the four reads the three rules are about.
struct ReadNames {
    who: &'static str,
    first: &'static str,
    next: &'static str,
    totals: &'static str,
    set_active_section: &'static str,
    section_fields: &'static [&'static str],
}

const CAPI_NAMES: ReadNames = ReadNames {
    who: "tools/oracle/oracle_server.py::capture_reliability",
    first: "m.First",
    next: "m.Next",
    totals: "m.Totals",
    set_active_section: "m.SetActiveSection",
    section_fields: &[
        "m.NumSectionCustomers",
        "m.NumSectionBranches",
        "m.SectSeqIdx",
        "m.SectTotalCust",
        "m.OCPDeviceType",
        "m.SumBranchFltRates",
        "m.AvgRepairTime",
        "m.FaultRateXRepairHrs",
    ],
};

const R4133_NAMES: ReadNames = ReadNames {
    who: "crates/dss-epri/src/capture.rs::capture_reliability",
    first: "meters_first",
    next: "meters_next",
    totals: "meters_totals",
    set_active_section: "meters_set_active_section",
    section_fields: &[
        "meters_num_section_customers",
        "meters_num_section_branches",
        "meters_sect_seq_idx",
        "meters_sect_total_cust",
        "meters_ocp_device_type",
        "meters_sum_branch_flt_rates",
        "meters_avg_repair_time",
        "meters_fault_rate_x_repair_hrs",
    ],
};

fn only_index(seq: &[String], name: &str, n: &ReadNames) -> Result<usize, String> {
    let hits: Vec<usize> = seq
        .iter()
        .enumerate()
        .filter(|(_, s)| s.as_str() == name)
        .map(|(i, _)| i)
        .collect();
    match hits.as_slice() {
        [i] => Ok(*i),
        _ => Err(format!(
            "{}: `{name}` must be read exactly once per capture, found {} \
             occurrence(s) in {seq:?}",
            n.who,
            hits.len()
        )),
    }
}

/// The three rules, checked on a read sequence rather than on a literal list —
/// so a transport may reorder its order-free reads and still be held to them.
fn check_reliability_read_rules(seq: &[String], n: &ReadNames) -> Result<(), String> {
    if seq.first().map(String::as_str) != Some(n.first) {
        return Err(format!(
            "{}: the first read must be `{}` (the walk has to restart after \
             `RelCalc`, which itself walked `EnergyMeters` to the end — r4133 \
             `Executive/ExecHelper.pas:4417`, `:4439-4441`), found {:?}",
            n.who,
            n.first,
            seq.first()
        ));
    }
    let sas = only_index(seq, n.set_active_section, n)?;
    let next = only_index(seq, n.next, n)?;
    let totals = only_index(seq, n.totals, n)?;

    for (i, name) in seq.iter().enumerate() {
        if n.section_fields.contains(&name.as_str()) && i < sas {
            return Err(format!(
                "{}: `{name}` is read at position {i}, BEFORE `{}` at {sas}. A \
                 section field read without a preceding selection answers from \
                 the previously selected section (capi \
                 `CAPI/CAPI_Meters.pas:729-740`, r4133 `DMeters.pas:254-262`) \
                 or, with none selected, from `InvalidActiveSection` \
                 (`CAPI_Meters.pas:122-134`, `#5055` under \
                 `DSS_CAPI_EXT_ERRORS`).",
                n.who, n.set_active_section
            ));
        }
    }
    if next < sas {
        return Err(format!(
            "{}: `{}` at {next} precedes `{}` at {sas} — the section loop must \
             sit inside the meter walk.",
            n.who, n.next, n.set_active_section
        ));
    }
    if totals != seq.len() - 1 || totals < next {
        return Err(format!(
            "{}: `{}` is read at position {totals} of {} (after `{}` at \
             {next}? {}). It must be the LAST read of the capture: it calls \
             `TotalizeMeters`, which walks `EnergyMeters.First`/`Next` itself \
             and destroys the cursor (capi `CAPI_Meters.pas:279-290` -> \
             `Common/Circuit.pas:2347-2360`; r4133 `DMeters.pas:558-573` -> \
             `Circuit.pas:2520-2538`). Measured: a mid-walk read truncates \
             `controls:energymeter/midi_energymeter.dss`'s walk from \
             ['em', 'em2'] to ['em'].",
            n.who,
            n.totals,
            seq.len(),
            n.next,
            totals > next
        ));
    }
    Ok(())
}

fn check_literal_order(seq: &[String], expected: &[&str], who: &str) -> Result<(), String> {
    if seq.len() == expected.len() && seq.iter().zip(expected).all(|(a, b)| a == b) {
        return Ok(());
    }
    Err(format!(
        "{who}: the reliability capture's read order changed.\n  expected: \
         {expected:?}\n  found:    {seq:?}\n\
         The three binding rules are `SetActiveSection` before every section \
         field, `Totals` last, and the per-meter fields in the fastdss \
         `IMeters._columns` order the other transport also uses."
    ))
}

/// The capi transport's literal read sequence.
#[test]
fn the_capi_reliability_capture_reads_in_the_pinned_order() {
    let seq = capi_read_sequence(&capi_reliability_body());
    if let Err(msg) = check_literal_order(&seq, CAPI_RELIABILITY_ORDER, CAPI_NAMES.who) {
        panic!("{msg}");
    }
}

/// The r4133 transport's literal read sequence.
#[test]
fn the_r4133_reliability_capture_reads_in_the_pinned_order() {
    let seq = r4133_read_sequence(&r4133_reliability_body());
    if let Err(msg) = check_literal_order(&seq, R4133_RELIABILITY_ORDER, R4133_NAMES.who) {
        panic!("{msg}");
    }
}

/// Both transports obey the three rules — stated over the sequence, so this
/// survives a legitimate reordering of the order-free reads that the two
/// literal tests above would (correctly) ask to be re-pinned.
#[test]
fn both_reliability_captures_select_a_section_and_read_totals_last() {
    for (seq, names) in [
        (capi_read_sequence(&capi_reliability_body()), &CAPI_NAMES),
        (r4133_read_sequence(&r4133_reliability_body()), &R4133_NAMES),
    ] {
        if let Err(msg) = check_reliability_read_rules(&seq, names) {
            panic!("{msg}");
        }
    }
}

/// The two transports read the SAME fields — the per-meter prefix in the same
/// order, the section block as the same set — so a channel-to-channel
/// divergence on this surface can never be a capture-shape artefact.
#[test]
fn the_two_reliability_captures_read_the_same_fields() {
    // `m.SAIFIKW` -> `saifikw`, `meters_saifi_kw` -> `saifikw`: fold both
    // spellings onto one token.
    let canon = |s: &str| -> String {
        s.trim_start_matches("m.")
            .trim_start_matches("meters_")
            .trim_start_matches("meter_")
            .replace('_', "")
            .to_ascii_lowercase()
    };
    let capi: Vec<String> = CAPI_RELIABILITY_ORDER.iter().map(|s| canon(s)).collect();
    let r4133: Vec<String> = R4133_RELIABILITY_ORDER.iter().map(|s| canon(s)).collect();
    assert_eq!(capi.len(), r4133.len(), "{capi:?}\n{r4133:?}");

    let sas = capi
        .iter()
        .position(|s| s == "setactivesection")
        .expect("the capi order selects a section");
    assert_eq!(
        r4133.iter().position(|s| s == "setactivesection"),
        Some(sas),
        "the selection sits at a different position on the two transports"
    );
    assert_eq!(
        capi[..=sas],
        r4133[..=sas],
        "the per-meter prefix must be the identical field order on both \
         transports (the fastdss `IMeters._columns` order)"
    );
    // The section block, then the `Next`/`Totals` tail.
    let tail = capi.len() - 2;
    let mut a: Vec<&String> = capi[sas + 1..tail].iter().collect();
    let mut b: Vec<&String> = r4133[sas + 1..tail].iter().collect();
    a.sort();
    b.sort();
    assert_eq!(a, b, "the two transports read different section fields");
    assert_eq!(&capi[tail..], &["next".to_string(), "totals".to_string()]);
    assert_eq!(&r4133[tail..], &["next".to_string(), "totals".to_string()]);
}

/// Assert `anchors` occur exactly once each, in this order, in `body`.
fn assert_call_order(body: &str, anchors: &[&str], who: &str) {
    let mut last = 0usize;
    for (n, anchor) in anchors.iter().enumerate() {
        let hits: Vec<usize> = body.match_indices(anchor).map(|(i, _)| i).collect();
        assert_eq!(
            hits.len(),
            1,
            "{who}: `{anchor}` must appear exactly once in the capture slot, \
             found {} occurrence(s)",
            hits.len()
        );
        assert!(
            n == 0 || hits[0] > last,
            "{who}: `{anchor}` must come after `{}`. The reliability capture's \
             slot — after the meter walk, before the PDElements walk — is what \
             makes the two transports comparable: its last read \
             (`Meters.Totals`) destroys the meter cursor, so it may not precede \
             the meter capture, and it must run before the PDElements walk \
             hijacks the active element (`ParentPDElement`).",
            anchors[n - 1]
        );
        last = hits[0];
    }
}

/// The capi payload is built between the meters and the PD elements.
#[test]
fn the_capi_reliability_capture_runs_between_the_meters_and_the_pd_elements() {
    assert_call_order(
        &read_source("tools/oracle/oracle_server.py"),
        &[
            "\"meters\": capture_all_meters(",
            "capture_reliability(ckt, rel_aborted, rel_message)",
            "\"pd_elements\": capture_pd_elements(",
        ],
        "tools/oracle/oracle_server.py::run_case",
    );
}

/// …and so does the r4133 payload, in the identical slot.
#[test]
fn the_r4133_reliability_capture_runs_between_the_meters_and_the_pd_elements() {
    assert_call_order(
        &r4133_run_case_body(),
        &[
            "capture_meters(engine)",
            "capture_reliability(engine, rel)",
            "capture_pd_elements(engine)",
        ],
        "crates/dss-epri/src/capture.rs::run_case",
    );
}

/// **The static guards are not vacuous.** Every scan is re-run over corrupted
/// copies of the *real* bodies — the fastdss mistake itself (`Totals` read
/// inside the walk), a dropped `SetActiveSection`, a reordered per-meter field
/// — and must reject each one. A scan that silently matched nothing would pass
/// all six tests above and prove nothing.
#[test]
fn the_reliability_read_order_guard_rejects_a_contaminated_capture() {
    // 1. `Totals` hoisted into the walk — exactly what fastdss warns about.
    let capi_bad = capi_reliability_body().replacen(
        "num_sections = int(m.NumSections)",
        "num_sections = int(m.NumSections)\n        _t = m.Totals",
        1,
    );
    let seq = capi_read_sequence(&capi_bad);
    let err = check_reliability_read_rules(&seq, &CAPI_NAMES)
        .expect_err("the capi scan accepted a mid-walk `Totals` read");
    assert!(err.contains("exactly once"), "{err}");
    assert!(
        check_literal_order(&seq, CAPI_RELIABILITY_ORDER, "probe").is_err(),
        "the capi literal scan accepted a mid-walk `Totals` read"
    );

    let r4133_bad = r4133_reliability_body().replacen(
        "let num_sections = engine.meters_num_sections()?;",
        "let num_sections = engine.meters_num_sections()?;\n        let _t = engine.meters_totals()?;",
        1,
    );
    let seq = r4133_read_sequence(&r4133_bad);
    assert!(
        check_reliability_read_rules(&seq, &R4133_NAMES).is_err(),
        "the r4133 scan accepted a mid-walk `Totals` read"
    );

    // 2. A dropped section selection.
    let capi_bad = capi_reliability_body().replacen("m.SetActiveSection(k)", "pass", 1);
    let err = check_reliability_read_rules(&capi_read_sequence(&capi_bad), &CAPI_NAMES)
        .expect_err("the capi scan accepted a capture that never selects a section");
    assert!(err.contains("SetActiveSection"), "{err}");

    let r4133_bad =
        r4133_reliability_body().replacen("engine.meters_set_active_section(idx)?", "Ok(())", 1);
    assert!(
        check_reliability_read_rules(&r4133_read_sequence(&r4133_bad), &R4133_NAMES).is_err(),
        "the r4133 scan accepted a capture that never selects a section"
    );

    // 3. A reordered per-meter field is caught by the literal scans.
    let capi_bad = capi_reliability_body().replacen("m.SAIFIKW", "m.CustInterrupts", 1);
    assert!(
        check_literal_order(
            &capi_read_sequence(&capi_bad),
            CAPI_RELIABILITY_ORDER,
            "probe"
        )
        .is_err(),
        "the capi literal scan accepted a reordered per-meter field"
    );
    let r4133_bad =
        r4133_reliability_body().replacen("engine.meters_saifi_kw()", "engine.meters_saidi()", 1);
    assert!(
        check_literal_order(
            &r4133_read_sequence(&r4133_bad),
            R4133_RELIABILITY_ORDER,
            "probe"
        )
        .is_err(),
        "the r4133 literal scan accepted a reordered per-meter field"
    );

    // 4. And the slot scan reads exactly the one call it claims to order.
    let moved = r4133_run_case_body().replacen("capture_reliability(engine, rel)", "", 1);
    assert_eq!(
        moved
            .match_indices("capture_reliability(engine, rel)")
            .count(),
        0,
        "the run_case slice must contain exactly the one call the slot test reads"
    );
}

// ---------------------------------------------------------------------------
// The `RELIABILITY_SKIP_FIELDS` registry: every row's `pin` is a real `#[test]`
// ---------------------------------------------------------------------------
//
// `harness/mod.rs`'s own unit tests already police the table's vocabulary (a
// row names a real field and a real channel, and its `cite`/`pin` are
// non-empty) and its fail-on-stale rule. What no test could reach from inside
// the harness is the other end of the `pin` string: whether the expected-value
// test it names EXISTS. `pin` is a plain `&'static str`, so a renamed or
// deleted pin leaves the exclusion standing over nothing — precisely the "mask
// over nothing" CLAUDE.md forbids. This scan closes the loop from the outside,
// the way [`the_capi_reliability_capture_reads_in_the_pinned_order`] closes it
// on the capture bodies.

/// The files a `RELIABILITY_SKIP_FIELDS` pin may live in: this test binary and
/// the in-engine reliability pins.
const RELIABILITY_PIN_SOURCES: &[&str] = &[
    "crates/dss-core/tests/reliability_pins.rs",
    "crates/dss-core/src/exec/tests/reliability.rs",
    // The harness' own offline drives (G1.6(ii) audit settlement): the
    // per-column non-vacuity of `compare_bus_reliability` and the fail-loud
    // decode of a non-finite cell are pinned where the comparator and the caps
    // live, so the docs that name them are covered by the same guard.
    "crates/dss-core/tests/harness/mod.rs",
];

/// Every `pin: "…"` of `harness::RELIABILITY_SKIP_FIELDS`, read out of the
/// table's own source (the harness is a `mod` of other test binaries, not a
/// library this one links, so the table is reached textually).
fn reliability_skip_pins() -> Vec<String> {
    let src = read_source("crates/dss-core/tests/harness/mod.rs");
    let start = src
        .find("pub const RELIABILITY_SKIP_FIELDS: &[ReliabilitySkipRow] = &[")
        .expect("harness/mod.rs has no RELIABILITY_SKIP_FIELDS table");
    let table = &src[start..];
    let end = table
        .find("\n];")
        .expect("the RELIABILITY_SKIP_FIELDS table must close at column 0");
    let table = &table[..end];
    let mut pins = Vec::new();
    for (i, _) in table.match_indices("pin: \"") {
        let rest = &table[i + "pin: \"".len()..];
        let q = rest.find('"').expect("an unterminated `pin` string");
        pins.push(rest[..q].to_string());
    }
    pins
}

/// Whether `name` is declared as a `#[test] fn name(` in one of the pin
/// sources.
fn is_a_test_fn(name: &str) -> bool {
    let needle = format!("fn {name}(");
    RELIABILITY_PIN_SOURCES.iter().any(|rel| {
        let src = read_source(rel);
        src.match_indices(&needle).any(|(i, _)| {
            // The attribute sits on one of the few lines above the signature.
            let from = i.saturating_sub(400);
            src[from..i].rsplit("\n\n").next().is_some_and(|head| {
                head.contains("#[test]") && !head[head.find("#[test]").unwrap()..].contains("fn ")
            })
        })
    })
}

/// **No `kind=large*` case gates the reliability surface.**
///
/// `corpus_gate::manifest`'s doc calls the absence of a `force_reliability`
/// rule a decision under the same cost guard `force_properties` /
/// `force_pdelements` apply. Those two have real `kind=large*` predicates and
/// pinned populations; `compare_reliability` is manifest-set only, so nothing
/// but this test stops a `large` deck being flagged and paying for a whole
/// extra `RelCalc` + payload on three engines (G1.6(i) audit settlement,
/// finding AC-9: the word 'guard' now names something).
#[test]
fn no_large_case_gates_the_reliability_surface() {
    let mut large = 0usize;
    let mut flagged = 0usize;
    let mut offenders: Vec<String> = Vec::new();
    for rel in RELIABILITY_MANIFESTS {
        let text = read_source(rel);
        let json: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("{rel}: {e}"));
        for case in json["cases"].as_array().expect("`cases` is an array") {
            let kind = case["kind"].as_str().unwrap_or_default();
            let is_large = kind.starts_with("large");
            let is_rel = case["compare_reliability"].as_bool() == Some(true);
            large += usize::from(is_large);
            flagged += usize::from(is_rel);
            if is_large && is_rel {
                offenders.push(format!(
                    "{rel}: {} (kind={kind})",
                    case["path"].as_str().unwrap_or("?")
                ));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "these `kind=large*` case(s) set `compare_reliability`: {offenders:?}. \
         The surface drives an extra executive `RelCalc` and captures a full \
         meter payload on all three engines; keep it off the large decks, or \
         re-pin this guard in the same commit that deliberately admits one."
    );
    // Non-vacuous in both directions: the scan really saw large decks AND
    // really saw the flag somewhere.
    assert!(
        large >= 80,
        "the scan found only {large} `kind=large*` case(s)"
    );
    assert!(
        flagged >= 6,
        "the scan found only {flagged} flagged case(s)"
    );
}

/// Every corpus manifest the population is built from.
const RELIABILITY_MANIFESTS: &[&str] = &[
    "tests/corpus/asymmetric/manifest.json",
    "tests/corpus/controls/manifest.json",
    "tests/corpus/modes/manifest.json",
    "tests/corpus/manifests/solvable_now.json",
];
/// The G1.6(i) pins that `TESTING.md`, `tests/TOLERANCE_NOTES.md` and the
/// phase record quote by NAME.
///
/// The four `RELIABILITY_SKIP_FIELDS` rows are guarded by
/// [`every_reliability_skip_row_pin_names_a_real_test`] because an exclusion
/// whose pin is gone is a mask over nothing. These are the other direction:
/// a doc sentence that names a pin is only evidence while that pin exists,
/// and nothing stopped a rename from leaving the sentence pointing at
/// nothing (G1.6(i) audit settlement, finding AT-7).
const RELIABILITY_PINS_QUOTED_IN_DOCS: &[&str] = &[
    "a_non_finite_reliability_cell_fails_the_decode_on_both_transports",
    "bus_int_duration_stays_in_the_meters_zone_on_the_live_population",
    "bus_reliability_columns_match_both_oracles_on_the_duty_deck",
    "bus_reliability_columns_on_the_capi_only_and_r4133_only_decks",
    "bus_reliability_sections_and_durations_on_the_relcalc_deck",
    "bus_reliability_survives_the_52902_abort",
    "caidi_is_saidi_over_saifi_on_a_reliability_deck",
    "every_bus_reliability_column_is_compared_per_bus",
    "every_bus_reliability_column_is_read_by_some_transport",
    "meter_alloc_factors_are_zero_until_allocateloads_runs",
    "meter_allocation_factors_are_the_peak_current_over_the_metered_current",
    "meter_totals_is_the_masked_register_sum",
    "pd_elements_relcalc_fields_are_live_after_relcalc",
    "relcalc_abort_is_symmetric_on_a_zone_without_ocp",
    "relcalc_assume_restoration_changes_auto_ocp_interruptions",
    "relcalc_is_not_idempotent_and_the_gate_runs_it_once",
    "relcalc_recomputes_the_customer_totals_it_depends_on",
    "reliability_accumulators_are_correctly_rounded_f64_sums",
];

/// The three documents whose sentences name the pins above.
const RELIABILITY_PIN_DOCS: &[&str] = &[
    "TESTING.md",
    "tests/TOLERANCE_NOTES.md",
    "docs/phase-records/golden-rebase.md",
];

/// **Every pin the docs name exists, and every name in the list is really
/// quoted somewhere** — the guard fails in both directions, so neither the
/// list nor a doc sentence can rot on its own.
#[test]
fn every_reliability_pin_named_in_the_docs_exists() {
    let docs: Vec<String> = RELIABILITY_PIN_DOCS
        .iter()
        .map(|d| read_source(d))
        .collect();
    for pin in RELIABILITY_PINS_QUOTED_IN_DOCS {
        assert!(
            is_a_test_fn(pin),
            "the docs name `{pin}` as an expected-value pin, but no `#[test] fn \
             {pin}` exists in {RELIABILITY_PIN_SOURCES:?}. A doc sentence \
             pointing at a renamed or deleted test is evidence of nothing \
             (CLAUDE.md; `GOLDEN_REBASE_PLAN.md` §1.1(e))."
        );
        assert!(
            docs.iter().any(|d| d.contains(*pin)),
            "`{pin}` is listed here as doc-quoted but appears in none of \
             {RELIABILITY_PIN_DOCS:?} — drop it from the list, or restore the \
             sentence that named it"
        );
    }
    // Not vacuous in either direction.
    assert!(!RELIABILITY_PINS_QUOTED_IN_DOCS.is_empty());
    assert!(!is_a_test_fn(
        "relcalc_is_not_idempotent_and_the_gate_runs_it_twice"
    ));
    assert!(docs.iter().all(|d| !d.is_empty()));
}
/// **Every `RELIABILITY_SKIP_FIELDS` row names a `#[test]` that exists.**
///
/// The four shipped rows all name
/// [`meter_alloc_factors_are_zero_until_allocateloads_runs`], which is the pin
/// holding the port's `[0.0, 0.0, 0.0]` against the measured oracle garbage.
/// If that test is renamed away, the exclusion becomes a mask over nothing and
/// this fails.
#[test]
fn every_reliability_skip_row_pin_names_a_real_test() {
    let pins = reliability_skip_pins();
    assert!(
        !pins.is_empty(),
        "the RELIABILITY_SKIP_FIELDS scan found no `pin` at all — the table's \
         shape changed and this guard went blind"
    );
    assert_eq!(
        pins.len(),
        4,
        "RELIABILITY_SKIP_FIELDS has {} row(s), expected the four shipped ones \
         (capi/r4133 x calc_current/alloc_factors). A new row is welcome — \
         re-pin this count in the same commit that adds it, so the table and \
         its pins move together: {pins:?}",
        pins.len()
    );
    for pin in &pins {
        assert!(
            is_a_test_fn(pin),
            "RELIABILITY_SKIP_FIELDS names the pin `{pin}`, but no `#[test] fn \
             {pin}` exists in {RELIABILITY_PIN_SOURCES:?}. An exclusion whose \
             expected-value test has been renamed or deleted is a mask over \
             nothing (CLAUDE.md; `GOLDEN_REBASE_PLAN.md` §1.1(e))."
        );
    }
    // The scan is not vacuous in either direction.
    assert!(
        !is_a_test_fn("meter_alloc_factors_are_zero_until_allocateloads_runs_typo"),
        "the pin scan accepts a name that does not exist"
    );
    assert!(
        !is_a_test_fn("assert_manifest_gates_reliability"),
        "the pin scan accepts a plain helper fn as a `#[test]`"
    );
}

// ===========================================================================
// GOLDEN_REBASE G1.6(ii) — the eight per-bus reliability columns
// ===========================================================================
//
// `Export BusReliability` renders six of the eight at 11 significant digits
// against two byte goldens (`report/export/reliability.rs:21-38`,
// `tests/golden/reports/export_busreliability{,_multimeter}.txt`);
// `Cust_Duration` and `SectionID` were witnessed by nothing at all before this
// sub-step. Since G1.6(ii) `harness::compare_bus_reliability` compares all
// eight, per bus, in full f64, on every gating channel of the six flagged
// cases. These pins are the expected-value half of that: the port's own
// numbers, beside the two oracles' numbers, on every regime the population
// reaches — a completed multi-section zone, a single-section zone, an
// r4133-only deck, a capi-only deck and the errno-52902 abort.
//
// The exactness rule (`rel = abs = 0`, derived in `tests/TOLERANCE_NOTES.md`)
// rests on a measurement, not on hope: the two independent oracle engines
// return **bit-identical** doubles for all 400 cells the two channels share
// (G1.6(ii) parts F1/F4, `tmp` probes over the shipped transports), and four of
// the eight columns are already pinned bit-for-bit against both oracles by
// transitivity through
// [`relcalc_indices_match_both_oracles_on_the_duty_deck`]. A band here would
// mask an order bug in a zone walk whose whole arithmetic is sums of deck
// literals.

/// One pinned bus row, in the `origin/fastdss` `dss/IBus.py:19-53` `_columns`
/// order both transports read in — the list's duplicate `Cust_Interrupts`
/// entry (`:27`) collapsed to one read: `(name, cust_duration,
/// cust_interrupts, int_duration, lambda_, n_customers, n_interrupts,
/// section_id, total_miles)`.
type BusRow = (&'static str, f64, f64, f64, f64, i32, f64, i32, f64);

/// Compile a corpus deck, solve it `steps` times and run the executive
/// `RelCalc` — the corpus gate's own protocol
/// (`corpus_gate/runner.rs:403-428` drives `RelCalc` once, after the last
/// step). `aborts` is the number of errno-52902 lines the deck's meters raise
/// (one per meter without an OCP device — see
/// [`relcalc_abort_is_symmetric_on_a_zone_without_ocp`]).
fn deck_relcalc(rel: &str, steps: usize, aborts: usize) -> (Dss, DeckDirGuard) {
    let (mut dss, guard) = deck_steps(rel, steps);
    let before = dss.errors().len();
    dss.command("RelCalc");
    let new: Vec<String> = dss.errors()[before..]
        .iter()
        .map(|e| e.message.trim().to_string())
        .collect();
    assert_eq!(
        new.len(),
        aborts,
        "{rel}: expected {aborts} `RelCalc` error line(s), got {new:?}"
    );
    (dss, guard)
}

/// Assert `Dss::bus_reliability()` equals `want` cell for cell, **exactly**.
///
/// No tolerance parameter exists on purpose: `harness::compare_bus_reliability`
/// compares this surface at `rel = abs = 0` on both channels, so a pin that
/// admitted a band would be weaker than the gate it stands behind. The bus
/// sequence is asserted first and separately — a walk that lost or reordered a
/// bus must not be reported as eight column failures.
///
/// `assert_eq!` on `f64` is exact, so a `NaN` cell would (correctly) fail here.
/// None of the six flagged cases produces one: `Int_Duration` inherits the
/// unguarded `SumFltRatesXRepairHrs / SumBranchFltRates` division (r4133
/// `Version8/Source/Meters/EnergyMeter.pas:2561-2563`), but every section in
/// this population has at least one branch with a non-zero `faultrate`
/// (measured, parts F1/F4: 720/720 values finite on all six decks).
fn assert_bus_rows(dss: &Dss, want: &[BusRow], who: &str) {
    let got = dss.bus_reliability();
    assert_eq!(
        got.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(),
        want.iter().map(|r| r.0).collect::<Vec<_>>(),
        "{who}: the `BusList` walk itself moved (bus count or order)"
    );
    for (b, r) in got.iter().zip(want) {
        assert_eq!(
            (
                b.name.as_str(),
                b.cust_duration,
                b.cust_interrupts,
                b.int_duration,
                b.lambda_,
                b.n_customers,
                b.n_interrupts,
                b.section_id,
                b.total_miles,
            ),
            *r,
            "{who}: bus `{}` — columns are (name, cust_duration, \
             cust_interrupts, int_duration, lambda_, n_customers, \
             n_interrupts, section_id, total_miles), compared EXACTLY",
            b.name
        );
    }
}

/// The mechanism behind `Bus.Int_Duration`, re-derived on the deck rather than
/// hardcoded: for every bus that belongs to a feeder section, r4133 sets
/// `Bus_Int_Duration := Source_IntDuration + FeederSections[SectionID].
/// AverageRepairTime` (`Version8/Source/Meters/EnergyMeter.pas:2571-2573`; port
/// `solution/meters/reliability.rs`'s `set_duration`). `Source_IntDuration` is
/// one meter-level number, so the difference `int_duration -
/// avg_repair_time(section)` must be the SAME on every sectioned bus — and on
/// every deck of this population it is exactly `0.0`, since none of them sets a
/// source interruption duration.
///
/// This is what makes the pinned `int_duration` literals more than a
/// transcription: they are tied to the independently pinned section table.
fn assert_int_duration_is_the_sections_repair_time(dss: &Dss, who: &str) {
    let meters = dss.meter_reliability();
    let with_sections: Vec<_> = meters.iter().filter(|m| m.num_sections > 0).collect();
    let mut seen = 0usize;
    for b in dss.bus_reliability() {
        if b.section_id <= 0 {
            assert_eq!(
                b.int_duration, 0.0,
                "{who}: bus `{}` has no section, so nothing ever wrote its \
                 duration",
                b.name
            );
            continue;
        }
        let m = with_sections
            .iter()
            .find(|m| b.section_id <= m.num_sections)
            .unwrap_or_else(|| {
                panic!(
                    "{who}: bus `{}` carries SectionID {} but no meter has that \
                     many sections",
                    b.name, b.section_id
                )
            });
        let s = &m.sections[(b.section_id - 1) as usize];
        assert_eq!(s.idx, b.section_id, "{who}: section table is 1-based");
        assert_eq!(
            b.int_duration - s.avg_repair_time,
            0.0,
            "{who}: bus `{}` (section {}) — `Int_Duration` must be \
             `Source_IntDuration + AverageRepairTime`, and no deck in this \
             population sets a source interruption duration (r4133 \
             `EnergyMeter.pas:2571-2573`)",
            b.name,
            b.section_id
        );
        seen += 1;
    }
    assert!(
        seen > 0,
        "{who}: no bus carries a section id — this check saw nothing"
    );
}

/// **`modes:time/midi_duty_ctrl.dss` — all eight per-bus columns, with both
/// oracles' numbers.**
///
/// The sub-step's clean `both`-channel witness, and the deck whose meter
/// indices are already pinned against both oracles by
/// [`relcalc_indices_match_both_oracles_on_the_duty_deck`]. Every cell below
/// was returned **bit-identically** by the pinned dss-python / dss_capi 0.14.5
/// oracle (`tools/oracle/oracle_server.py::capture_bus_reliability`) and by the
/// EPRI r4133 DDLL bridge
/// (`crates/dss-epri/src/capture.rs::capture_bus_reliability`, `BUSF` 6-11 /
/// `BUSI` 4-5), and the port returns the same bits again:
///
/// | bus | `Cust_Duration` | `Cust_Interrupts` | `Int_Duration` | `Lambda` | `N_Customers` | `N_interrupts` | `SectionID` | `TotalMiles` |
/// |---|---|---|---|---|---|---|---|---|
/// | `src` | 0 | 0 | 0 | 0 | 2 | 0 | 0 | `2.8` |
/// | `mid` | 0 | `0.11200000000000002` | `2.999999999999999` | `0.036000000000000004` | 2 | `0.05600000000000001` | 1 | `1.8` |
/// | `loadb` | `0.16799999999999998` | 0 | `2.999999999999999` | 0 | 0 | `0.05600000000000001` | 1 | 0 |
/// | `loadc` | `0.16799999999999998` | 0 | `2.999999999999999` | 0 | 0 | `0.05600000000000001` | 1 | 0 |
///
/// (capi column == r4133 column == port column, cell for cell — measured over
/// the two shipped transports, G1.6(ii) parts F1 and F4.)
///
/// Four of the eight are *already* pinned against both oracles one level up,
/// and this test closes that loop explicitly: on this fixture
/// `mid.N_interrupts == SAIFI`, `mid.Cust_Interrupts == CustInterrupts`,
/// `Int_Duration == section 1's AvgRepairTime` on all three sectioned buses and
/// `loadb/loadc.Cust_Duration == SAIDI`. Those identities are numerical
/// coincidences of this deck (`dblNcusts == 2`, `Source_IntDuration == 0`), used
/// as evidence that the bus columns and the meter indices come from the same
/// arithmetic — never as a general rule.
#[test]
fn bus_reliability_columns_match_both_oracles_on_the_duty_deck() {
    const WANT: &[BusRow] = &[
        ("src", 0.0, 0.0, 0.0, 0.0, 2, 0.0, 0, 2.8),
        (
            "mid",
            0.0,
            0.11200000000000002,
            2.999999999999999,
            0.036000000000000004,
            2,
            0.05600000000000001,
            1,
            1.8,
        ),
        (
            "loadb",
            0.16799999999999998,
            0.0,
            2.999999999999999,
            0.0,
            0,
            0.05600000000000001,
            1,
            0.0,
        ),
        (
            "loadc",
            0.16799999999999998,
            0.0,
            2.999999999999999,
            0.0,
            0,
            0.05600000000000001,
            1,
            0.0,
        ),
    ];
    let who = "modes:time/midi_duty_ctrl.dss";
    let (dss, _guard) = deck_relcalc("modes/time/midi_duty_ctrl.dss", 20, 0);
    assert_bus_rows(&dss, WANT, who);
    assert_int_duration_is_the_sections_repair_time(&dss, who);

    // The transitive half: the same numbers, reached through the meter payload
    // that `relcalc_indices_match_both_oracles_on_the_duty_deck` already holds
    // against both oracles.
    let m = &dss.meter_reliability()[0];
    let bus = |name: &str| {
        dss.bus_reliability()
            .into_iter()
            .find(|b| b.name == name)
            .unwrap_or_else(|| panic!("{who} has no bus `{name}`"))
    };
    assert_eq!(
        bus("mid").n_interrupts,
        m.saifi,
        "mid.N_interrupts == SAIFI"
    );
    assert_eq!(
        bus("mid").cust_interrupts,
        m.cust_interrupts,
        "mid.Cust_Interrupts == CustInterrupts"
    );
    assert_eq!(
        bus("loadb").cust_duration,
        m.saidi,
        "loadb.Cust_Duration == SAIDI"
    );
    assert_eq!(
        bus("loadc").cust_duration,
        m.saidi,
        "loadc.Cust_Duration == SAIDI"
    );

    // Step invariance, the gate's protocol against the cheap one.
    let (one, _g2) = deck_relcalc("modes/time/midi_duty_ctrl.dss", 1, 0);
    assert_eq!(
        one.bus_reliability(),
        dss.bus_reliability(),
        "the per-bus payload must not depend on how many steps were solved"
    );
}

/// **`controls:energymeter/midi_relcalc.dss` — the richest `both`-channel
/// fixture: three sections, three distinct `Int_Duration`s, `Cust_Duration` on
/// three buses.**
///
/// The only case in the population where `SectionID` takes every value in
/// `1..=3`, and the one whose durations are not a round number — which is what
/// makes it the sharpest test of the exact compare. Every cell below was
/// returned bit-identically by both oracles (parts F1/F4) and by the port:
///
/// | bus | `Cust_Duration` | `Cust_Interrupts` | `Int_Duration` | `Lambda` | `N_Customers` | `N_interrupts` | `SectionID` | `TotalMiles` |
/// |---|---|---|---|---|---|---|---|---|
/// | `src` | 0 | 0 | 0 | 0 | 30 | 0 | 0 | `4.1` |
/// | `mid` | 0 | `2.0475000000000003` | `4.5` | 0 | 30 | `0.06825` | 1 | `2.5999999999999996` |
/// | `la` | `9.3400125` | 0 | `2.5` | 0 | 0 | `0.16905` | 3 | 0 |
/// | `lb` | `6.048007583148559` | `0.8141999999999999` | `2.856984478935699` | `0.0435` | 4 | `0.20354999999999998` | 2 | `0.6` |
/// | `lc` | `2.5587724390243904` | 0 | `2.856984478935699` | 0 | 0 | `0.20354999999999998` | 2 | 0 |
///
/// The section table this deck's durations come from is pinned independently
/// (three Reclosers -> sections `(1, 0, 1, 1, 2)`, `(2, 13, 2, 2, 2)`,
/// `(3, 17, 1, 4, 2)`; see the `dss_relcalc` helper), and
/// [`assert_int_duration_is_the_sections_repair_time`] ties the two together
/// here.
#[test]
fn bus_reliability_sections_and_durations_on_the_relcalc_deck() {
    const WANT: &[BusRow] = &[
        ("src", 0.0, 0.0, 0.0, 0.0, 30, 0.0, 0, 4.1),
        (
            "mid",
            0.0,
            2.0475000000000003,
            4.5,
            0.0,
            30,
            0.06825,
            1,
            2.5999999999999996,
        ),
        ("la", 9.3400125, 0.0, 2.5, 0.0, 0, 0.16905, 3, 0.0),
        (
            "lb",
            6.048007583148559,
            0.8141999999999999,
            2.856984478935699,
            0.0435,
            4,
            0.20354999999999998,
            2,
            0.6,
        ),
        (
            "lc",
            2.5587724390243904,
            0.0,
            2.856984478935699,
            0.0,
            0,
            0.20354999999999998,
            2,
            0.0,
        ),
    ];
    let who = "controls:energymeter/midi_relcalc.dss";
    let (dss, _guard) = deck_relcalc("controls/energymeter/midi_relcalc.dss", 1, 0);
    assert_bus_rows(&dss, WANT, who);
    assert_int_duration_is_the_sections_repair_time(&dss, who);

    // Three sections really are reached, and each one carries its own repair
    // time — so the identity above is not being satisfied by one value.
    let m = &dss.meter_reliability()[0];
    assert_eq!(m.num_sections, 3);
    let mut times: Vec<f64> = m.sections.iter().map(|s| s.avg_repair_time).collect();
    times.sort_by(f64::total_cmp);
    times.dedup();
    assert_eq!(
        times.len(),
        3,
        "the three sections must have three distinct repair times: {:?}",
        m.sections
            .iter()
            .map(|s| s.avg_repair_time)
            .collect::<Vec<_>>()
    );
    let mut ids: Vec<i32> = dss
        .bus_reliability()
        .iter()
        .map(|b| b.section_id)
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(
        ids,
        vec![0, 1, 2, 3],
        "SectionID reaches 0..=3 on this deck"
    );
}

/// **`controls:energymeter/midi_energymeter.dss` — the errno-52902 abort is a
/// COMPARED state, not an untested one.**
///
/// Both meters abort for want of an OCP device
/// ([`relcalc_abort_is_symmetric_on_a_zone_without_ocp`]), so the four
/// interruption columns and `SectionID` stay at their `TDSSBus.Create` zeros on
/// all 35 buses — while `Lambda`, `TotalMiles` and `N_Customers` are fully
/// populated, because they come from the backward `MakeMeterZoneLists` sweep
/// (r4133 `Version8/Source/PDElements/PDElement.pas:106-181`) that runs long
/// before `CalcReliabilityIndices` gives up. `SectionID` reads `0` rather than
/// the `-1` "not set" sentinel because `CalcNum_Int` stamps the whole
/// `SequenceList` *before* the `SectionCount = 0` check
/// (r4133 `EnergyMeter.pas:2488-2504`).
///
/// The whole 35-row table below is the port's, and every cell of it was
/// returned bit-identically by BOTH oracles (parts F1/F4 — this deck gates on
/// `both`). It is pinned in full rather than by witnesses because it is the
/// only fixture where a *partial* population is the expected answer: 25 of the
/// 35 buses carry a non-zero `Lambda` and 25 carry a customer count — the same
/// size, different members (`rb` has a fault rate and no customer, `l5m` a
/// customer and no fault rate) — so a port that "helpfully" zeroed or filled
/// either group would still look plausible row by row.
#[test]
fn bus_reliability_survives_the_52902_abort() {
    const WANT: &[BusRow] = &[
        (
            "src",
            0.0,
            0.0,
            0.0,
            1.4605200000000005,
            24,
            0.0,
            0,
            13.825757575757578,
        ),
        (
            "bb1",
            0.0,
            0.0,
            0.0,
            1.4605200000000005,
            24,
            0.0,
            0,
            13.825757575757578,
        ),
        (
            "bb2",
            0.0,
            0.0,
            0.0,
            1.4205200000000004,
            24,
            0.0,
            0,
            13.446969696969699,
        ),
        (
            "bb3",
            0.0,
            0.0,
            0.0,
            1.2605200000000003,
            20,
            0.0,
            0,
            11.931818181818183,
        ),
        (
            "bb4",
            0.0,
            0.0,
            0.0,
            1.1405200000000002,
            19,
            0.0,
            0,
            10.795454545454547,
        ),
        (
            "bb5",
            0.0,
            0.0,
            0.0,
            0.9805200000000002,
            17,
            0.0,
            0,
            9.280303030303031,
        ),
        ("bb6", 0.0, 0.0, 0.0, 0.0, 0, 0.0, 0, 0.0),
        ("bb7", 0.0, 0.0, 0.0, 0.08, 2, 0.0, 0, 0.7575757575757576),
        ("bb8", 0.0, 0.0, 0.0, 0.14, 2, 0.0, 0, 1.3257575757575757),
        ("bb9", 0.0, 0.0, 0.0, 0.1405, 2, 0.0, 0, 1.3257575757575757),
        (
            "bb10",
            0.0,
            0.0,
            0.0,
            0.22050000000000003,
            2,
            0.0,
            0,
            2.083333333333333,
        ),
        ("bb11", 0.0, 0.0, 0.0, 0.2605, 2, 0.0, 0, 2.462121212121212),
        ("bb12", 0.0, 0.0, 0.0, 0.5605, 11, 0.0, 0, 5.303030303030303),
        ("bb13", 0.0, 0.0, 0.0, 0.2, 8, 0.0, 0, 1.8939393939393938),
        ("bb14", 0.0, 0.0, 0.0, 0.08, 6, 0.0, 0, 0.7575757575757576),
        ("bb15", 0.0, 0.0, 0.0, 0.04, 3, 0.0, 0, 0.3787878787878788),
        ("rb", 0.0, 0.0, 0.0, 0.3005, 0, 0.0, 0, 2.840909090909091),
        ("l1m", 0.0, 0.0, 0.0, 0.04, 3, 0.0, 0, 0.3787878787878788),
        ("l1e", 0.0, 0.0, 0.0, 0.0, 0, 0.0, 0, 0.0),
        ("l2m", 0.0, 0.0, 0.0, 0.04, 1, 0.0, 0, 0.3787878787878788),
        ("l2e", 0.0, 0.0, 0.0, 0.0, 0, 0.0, 0, 0.0),
        ("l3m", 0.0, 0.0, 0.0, 0.04, 2, 0.0, 0, 0.3787878787878788),
        ("l3e", 0.0, 0.0, 0.0, 0.0, 0, 0.0, 0, 0.0),
        (
            "l4m",
            0.0,
            0.0,
            0.0,
            0.7805200000000002,
            16,
            0.0,
            0,
            7.386363636363637,
        ),
        (
            "l4n",
            0.0,
            0.0,
            0.0,
            0.7405200000000002,
            15,
            0.0,
            0,
            7.007575757575758,
        ),
        (
            "l4e",
            0.0,
            0.0,
            0.0,
            0.7005200000000001,
            14,
            0.0,
            0,
            6.628787878787879,
        ),
        ("l5m", 0.0, 0.0, 0.0, 0.0, 1, 0.0, 0, 0.0),
        ("l5lv", 0.0, 0.0, 0.0, 0.0, 0, 0.0, 0, 0.0),
        ("l6m", 0.0, 0.0, 0.0, 0.6205, 11, 0.0, 0, 5.871212121212121),
        ("l6n", 0.0, 0.0, 0.0, 0.6605000000000001, 12, 0.0, 0, 6.25),
        (
            "l6e",
            0.0,
            0.0,
            0.0,
            0.7005000000000001,
            13,
            0.0,
            0,
            6.628787878787879,
        ),
        ("l7e", 0.0, 0.0, 0.0, 0.0, 0, 0.0, 0, 0.0),
        ("l8mv", 0.0, 0.0, 0.0, 0.0, 0, 0.0, 0, 0.0),
        ("l8lv", 0.0, 0.0, 0.0, 0.0, 0, 0.0, 0, 0.0),
        ("l9e", 0.0, 0.0, 0.0, 0.0, 0, 0.0, 0, 0.0),
    ];
    let who = "controls:energymeter/midi_energymeter.dss";
    // Two meters, two abort lines — one per failing meter.
    let (dss, _guard) = deck_relcalc("controls/energymeter/midi_energymeter.dss", 24, 2);
    assert_bus_rows(&dss, WANT, who);

    // The regime, stated once rather than read out of the table: nothing the
    // aborted calc would have written was written, and the sweep's three
    // columns are alive on most of the circuit.
    let rows = dss.bus_reliability();
    assert_eq!(rows.len(), 35);
    assert!(
        rows.iter().all(|b| b.section_id == 0
            && b.n_interrupts == 0.0
            && b.int_duration == 0.0
            && b.cust_interrupts == 0.0
            && b.cust_duration == 0.0),
        "the aborted calc must write nothing at all"
    );
    assert_eq!(
        rows.iter().filter(|b| b.lambda_ != 0.0).count(),
        25,
        "the backward sweep populated `Lambda` on 25 of the 35 buses"
    );
    assert_eq!(
        rows.iter().filter(|b| b.n_customers > 0).count(),
        25,
        "and `N_Customers` on 25 of them"
    );
    // The two sets have the same SIZE and different MEMBERS, which is why both
    // are named: `rb` carries a fault rate and no customer, `l5m` a customer
    // and no fault rate. A port that populated one column out of the other
    // would still satisfy either count on its own.
    let lam: Vec<&str> = rows
        .iter()
        .filter(|b| b.lambda_ != 0.0)
        .map(|b| b.name.as_str())
        .collect();
    let cust: Vec<&str> = rows
        .iter()
        .filter(|b| b.n_customers > 0)
        .map(|b| b.name.as_str())
        .collect();
    assert!(lam.contains(&"rb") && !cust.contains(&"rb"));
    assert!(cust.contains(&"l5m") && !lam.contains(&"l5m"));
    assert_eq!(
        dss.meter_reliability()
            .iter()
            .map(|m| (m.name.clone(), m.num_sections))
            .collect::<Vec<_>>(),
        vec![("em".to_string(), 0), ("em2".to_string(), 0)],
        "both meters aborted, so neither allocated a section"
    );
}

/// **The two single-channel decks — `modes:makeposseq/makeposseq_ctrl.dss`
/// (`capi_v0145` only) and `controls:combo/combo_protection.dss` (`r4133`
/// only).**
///
/// The three pins above all sit on `both`-gated cases, where a wrong port value
/// would be caught twice over. These two are each compared against exactly one
/// oracle, so a pin carries the most weight here — and together they cover the
/// reason both single-channel classes exist: capi 0.14.5 cannot compile
/// `combo_protection` (`Fuse.curvemultiplier` is not a parameter it knows,
/// error 110 — measured, part R), while `makeposseq_ctrl` is capi-gated over an
/// unrelated SwtControl per-phase-state divergence already carried in
/// `tests/corpus/ledger.json`.
///
/// `modes:makeposseq/makeposseq_ctrl.dss` vs the capi 0.14.5 oracle:
///
/// | bus | `Cust_Duration` | `Cust_Interrupts` | `Int_Duration` | `Lambda` | `N_Customers` | `N_interrupts` | `SectionID` | `TotalMiles` |
/// |---|---|---|---|---|---|---|---|---|
/// | `src` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
/// | `b1` | 0 | 0 | 0 | `0.02` | 2 | 0 | 0 | `1.8651135767120017` |
/// | `b2` | 0 | 0 | 0 | 0 | 1 | 0 | 0 | `1.2437423844746678` |
/// | `b3` | 0 | `0.02002` | `3.0` | `2e-05` | 1 | `0.02002` | 1 | `0.622371192237334` |
/// | `b4` | 0 | `0.02002` | `3.0` | 0 | 1 | `0.02002` | 1 | `0.621371192237334` |
/// | `b5` | `0.12006` | 0 | `3.0` | 0 | 0 | `0.04002` | 2 | 0 |
///
/// `controls:combo/combo_protection.dss` vs the EPRI r4133 oracle:
///
/// | bus | `Cust_Duration` | `Cust_Interrupts` | `Int_Duration` | `Lambda` | `N_Customers` | `N_interrupts` | `SectionID` | `TotalMiles` |
/// |---|---|---|---|---|---|---|---|---|
/// | `src` | 0 | 0 | 0 | 0 | 2 | 0 | 0 | `3.5` |
/// | `b1` | 0 | `0.02` | `3.0` | 0 | 2 | `0.01` | 1 | `3.0` |
/// | `b2` | 0 | `0.1` | `3.0` | `0.02` | 2 | `0.05` | 2 | `2.0` |
/// | `b3` | `0.21000000000000002` | 0 | `3.0` | 0 | 0 | `0.07` | 3 | 0 |
/// | `b4` | `0.15000000000000002` | 0 | `3.0` | 0 | 0 | `0.05` | 2 | 0 |
///
/// The `makeposseq_ctrl` row `b1 TotalMiles = 1.8651135767120017` is that
/// deck's one irrational length (a `MakePosSequence`-reduced line) and `b3`'s
/// `Lambda = 2e-05` its one sub-milli failure rate — both bit-exact against the
/// oracle, which is what an exact compare on a running sum is worth.
#[test]
fn bus_reliability_columns_on_the_capi_only_and_r4133_only_decks() {
    const MAKEPOSSEQ: &[BusRow] = &[
        ("src", 0.0, 0.0, 0.0, 0.0, 0, 0.0, 0, 0.0),
        ("b1", 0.0, 0.0, 0.0, 0.02, 2, 0.0, 0, 1.8651135767120017),
        ("b2", 0.0, 0.0, 0.0, 0.0, 1, 0.0, 0, 1.2437423844746678),
        (
            "b3",
            0.0,
            0.02002,
            3.0,
            2e-05,
            1,
            0.02002,
            1,
            0.622371192237334,
        ),
        (
            "b4",
            0.0,
            0.02002,
            3.0,
            0.0,
            1,
            0.02002,
            1,
            0.621371192237334,
        ),
        ("b5", 0.12006, 0.0, 3.0, 0.0, 0, 0.04002, 2, 0.0),
    ];
    const COMBO: &[BusRow] = &[
        ("src", 0.0, 0.0, 0.0, 0.0, 2, 0.0, 0, 3.5),
        ("b1", 0.0, 0.02, 3.0, 0.0, 2, 0.01, 1, 3.0),
        ("b2", 0.0, 0.1, 3.0, 0.02, 2, 0.05, 2, 2.0),
        ("b3", 0.21000000000000002, 0.0, 3.0, 0.0, 0, 0.07, 3, 0.0),
        ("b4", 0.15000000000000002, 0.0, 3.0, 0.0, 0, 0.05, 2, 0.0),
    ];

    let who = "modes:makeposseq/makeposseq_ctrl.dss";
    let (dss, guard) = deck_relcalc("modes/makeposseq/makeposseq_ctrl.dss", 1, 0);
    assert_bus_rows(&dss, MAKEPOSSEQ, who);
    assert_int_duration_is_the_sections_repair_time(&dss, who);
    assert_manifest_gates_reliability(
        "modes",
        "modes/makeposseq/makeposseq_ctrl.dss",
        "capi_v0145",
    );
    drop(dss);
    drop(guard);

    let who = "controls:combo/combo_protection.dss";
    let (dss, _guard) = deck_relcalc("controls/combo/combo_protection.dss", 20, 0);
    assert_bus_rows(&dss, COMBO, who);
    assert_int_duration_is_the_sections_repair_time(&dss, who);
    assert_manifest_gates_reliability("controls", "controls/combo/combo_protection.dss", "r4133");
}

/// One flagged case of the reliability population: its manifest family, its
/// path inside `tests/corpus/`, the gating channel(s) the manifest declares,
/// the gate's step count, and the section-id histogram measured on both oracle
/// channels (parts F1/F4).
struct RelPopulationCase {
    family: &'static str,
    deck: &'static str,
    engines: &'static str,
    steps: usize,
    aborts: usize,
    /// `(section_id, bus count)` pairs, ascending — the port's, and both
    /// oracles', measured.
    section_ids: &'static [(i32, usize)],
}

/// The six cases that carry `compare_reliability: true`, pinned so that
/// "on the live population" below names something checkable. Fail-on-stale in
/// both directions: a seventh flagged case, or one of these losing its flag,
/// reds [`bus_int_duration_stays_in_the_meters_zone_on_the_live_population`].
const RELIABILITY_POPULATION: &[RelPopulationCase] = &[
    RelPopulationCase {
        family: "controls",
        deck: "controls/combo/combo_protection.dss",
        engines: "r4133",
        steps: 20,
        aborts: 0,
        section_ids: &[(0, 1), (1, 1), (2, 2), (3, 1)],
    },
    RelPopulationCase {
        family: "controls",
        deck: "controls/combo/midi_protection.dss",
        engines: "r4133",
        steps: 24,
        aborts: 0,
        section_ids: &[(0, 2), (1, 23), (2, 7), (3, 3)],
    },
    RelPopulationCase {
        family: "controls",
        deck: "controls/energymeter/midi_energymeter.dss",
        engines: "both",
        steps: 24,
        aborts: 2,
        section_ids: &[(0, 35)],
    },
    RelPopulationCase {
        family: "controls",
        deck: "controls/energymeter/midi_relcalc.dss",
        engines: "both",
        steps: 1,
        aborts: 0,
        section_ids: &[(0, 1), (1, 1), (2, 2), (3, 1)],
    },
    RelPopulationCase {
        family: "modes",
        deck: "modes/makeposseq/makeposseq_ctrl.dss",
        engines: "capi_v0145",
        steps: 1,
        aborts: 0,
        section_ids: &[(0, 3), (1, 2), (2, 1)],
    },
    RelPopulationCase {
        family: "modes",
        deck: "modes/time/midi_duty_ctrl.dss",
        engines: "both",
        steps: 20,
        aborts: 0,
        section_ids: &[(0, 1), (1, 3)],
    },
];

/// **`Bus.Int_Duration` stays inside its own meter's zone — and on this
/// population the upstream all-buses walk cannot tell the difference.**
///
/// The standing G2.2a claim, re-asserted where the live gate can see it. r4133
/// `Version8/Source/Meters/EnergyMeter.pas:2567-2574` (capi 0.14.5
/// `EnergyMeter.pas:2521-2526`) writes the duration by walking **every circuit
/// bus** and indexing that bus's `BusSectionID` into *this* meter's
/// `FeederSections`, an array sized to *this* meter's `SectionCount`; the
/// zeroing that would have cleared a foreign id is itself per-zone (`:2472`).
/// The port walks only the zone its own forward sweep numbered
/// (`solution/meters/reliability.rs`, `set_duration` over `zone_buses`), which
/// is CLAUDE.md's "no upstream bug is reproduced in any lane".
///
/// The difference is observable only when some bus outside a meter's zone
/// carries a section id that meter could index — which needs **two** meters,
/// at least one of them with an allocated section. This test asserts the
/// structural reason that never happens on the flagged population: of the six
/// cases, five define exactly one meter, and the sixth
/// (`controls:energymeter/midi_energymeter.dss`) has two meters that both abort
/// at errno 52902, so neither allocates a section and every bus keeps
/// `SectionID = 0`. With at most one meter holding sections, every in-range
/// section id in the circuit was stamped by that meter's own sweep, so the
/// zone-scoped walk and the all-buses walk agree cell for cell — both oracles
/// therefore return the port's numbers, and the surface needs no ledger row.
///
/// # The Q4 measurement (part R, 2026-09-05) — recorded here rather than in prose
///
/// The brief predicted the four `DOCTechNote` multi-meter decks would be in
/// regime **(b)** of the bug (an out-of-range id -> an OOB heap read, proven
/// nondeterministic in
/// `investigations/reliability_bus_int_duration_oob_bug_report.md`), which
/// would have forced an unconditional exclusion with no oracle number to pin.
/// It was measured instead of assumed: `DOCTechNote/1_1.dss` in **three** fresh
/// capi processes and `2_2.dss` in **two** came back bit-identical on all eight
/// columns for all 390 buses. The cause is structural — on all four decks every
/// bus's `SectionID` is in `{0, 1}` (histogram `{0: 218, 1: 172}`) while the
/// per-meter `NumSections` histogram is `{1: 68, 0: 9}` (`2_2` also has one
/// meter at 8), so a foreign id is **always in range**: regime **(a)**, the
/// deterministic cross-zone overwrite. Had those decks been flagged, the
/// settlement would have been an ordinary field-by-field ledger exclusion plus
/// an expected-value pin — not an unconditional one. They are out of the
/// population by G1.6(i) decision D-i-2 (the `kind=large*` cost guard, held by
/// [`no_large_case_gates_the_reliability_surface`]), so nothing is owed either
/// way; the measurement is recorded because it inverted the prediction.
#[test]
fn bus_int_duration_stays_in_the_meters_zone_on_the_live_population() {
    // The population is exactly the six cases pinned above — checked against
    // the manifests before anything is concluded "on the live population".
    let mut flagged: Vec<String> = Vec::new();
    for rel in RELIABILITY_MANIFESTS {
        let text = read_source(rel);
        let json: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("{rel}: {e}"));
        let family = rel
            .trim_start_matches("tests/corpus/")
            .split('/')
            .next()
            .expect("a manifest path has a family segment");
        for case in json["cases"].as_array().expect("`cases` is an array") {
            if case["compare_reliability"].as_bool() == Some(true) {
                flagged.push(format!("{family}/{}", case["path"].as_str().unwrap_or("?")));
            }
        }
    }
    flagged.sort();
    let mut want: Vec<String> = RELIABILITY_POPULATION
        .iter()
        .map(|c| c.deck.to_string())
        .collect();
    want.sort();
    assert_eq!(
        flagged, want,
        "the reliability population moved. Every claim below is about exactly \
         these cases, so re-measure and re-pin them in the same commit that \
         changes the set."
    );

    for c in RELIABILITY_POPULATION {
        assert_manifest_gates_reliability(c.family, c.deck, c.engines);
        let (dss, _guard) = deck_relcalc(c.deck, c.steps, c.aborts);

        // The measured section-id histogram, both oracles' and the port's.
        let rows = dss.bus_reliability();
        let mut hist: BTreeMap<i32, usize> = BTreeMap::new();
        for b in &rows {
            *hist.entry(b.section_id).or_default() += 1;
        }
        let got: Vec<(i32, usize)> = hist.into_iter().collect();
        assert_eq!(got, c.section_ids, "{}: the `SectionID` histogram", c.deck);
        assert!(
            rows.iter().all(|b| b.section_id >= 0),
            "{}: the `-1` \"not set\" sentinel must never survive a completed \
             `RelCalc` (r4133 `PDElement.pas:326` writes it, \
             `EnergyMeter.pas:2494` and `PDElement.pas:179-181` overwrite it)",
            c.deck
        );

        // The structural reason the two walks agree: at most one meter holds
        // sections, so no foreign section id is ever indexable.
        let meters = dss.meter_reliability();
        let with_sections: Vec<&str> = meters
            .iter()
            .filter(|m| m.num_sections > 0)
            .map(|m| m.name.as_str())
            .collect();
        assert!(
            with_sections.len() <= 1,
            "{}: {} meters hold sections ({with_sections:?}). With two, a bus \
             outside a meter's zone can carry an id that meter would index — \
             the upstream cross-zone overwrite — and this surface would need a \
             field-by-field ledger exclusion plus a pin, not this assertion.",
            c.deck,
            with_sections.len()
        );
        let max_sections = meters.iter().map(|m| m.num_sections).max().unwrap_or(0);
        let max_id = rows.iter().map(|b| b.section_id).max().unwrap_or(0);
        assert!(
            max_id <= max_sections,
            "{}: a bus carries SectionID {max_id} but the largest section \
             table has {max_sections} entries — an out-of-range id is exactly \
             regime (b), the nondeterministic OOB read",
            c.deck
        );
    }
}

/// **Every column of `BusReliabilityView` is read by BOTH transports, and the
/// wire struct carries the same nine keys.**
///
/// A static census across the four sides of this surface — the engine view
/// (`crates/dss-core/src/exec/view.rs`), the harness's wire row
/// (`crates/dss-core/tests/harness/mod.rs`), the capi capture
/// (`tools/oracle/oracle_server.py`) and the r4133 capture
/// (`crates/dss-epri/src/capture.rs`). The comparator's `rel_bus_fields!` macro
/// already stops a field being renamed on one Rust side without the other; what
/// no compiler can see is a column added to the view and forgotten on a
/// *transport*, which would silently compare the port against nothing.
///
/// `name` is the one asymmetry, and it is deliberate: the capi transport reads
/// it back per bus (`b.Name`), while the r4133 bridge binds no `BUSS` family
/// and takes the name from the walk (`Circuit.AllBusNames`), exactly as
/// `capture_all_buses` does. Both facts are asserted rather than assumed.
#[test]
fn every_bus_reliability_column_is_read_by_some_transport() {
    let view = struct_field_names("crates/dss-core/src/exec/view.rs", "BusReliabilityView");
    let wire = struct_field_names("crates/dss-core/tests/harness/mod.rs", "BusReliabilityCap");
    let mut want: Vec<String> = vec!["name".to_string()];
    want.extend(BUS_RELIABILITY_COLUMNS.iter().map(|s| s.to_string()));
    want.sort();
    let mut sorted_view = view.clone();
    sorted_view.sort();
    let mut sorted_wire = wire.clone();
    sorted_wire.sort();
    assert_eq!(
        sorted_view, want,
        "`BusReliabilityView` must carry `name` plus the eight `IBus._columns` \
         reliability fields"
    );
    assert_eq!(
        sorted_wire, want,
        "`BusReliabilityCap` (the wire row both transports emit) must carry \
         the same nine keys as the view"
    );
    assert_eq!(
        view, wire,
        "view and wire row must also agree on the FIELD ORDER, which is the \
         `IBus._columns` read order"
    );

    let capi: Vec<String> = capi_bus_read_sequence(&capi_bus_reliability_body())
        .iter()
        .map(|s| canonical_bus_read(s))
        .collect();
    let r4133: Vec<String> = r4133_bus_read_sequence(&r4133_bus_reliability_body())
        .iter()
        .map(|s| canonical_bus_read(s))
        .collect();
    for col in BUS_RELIABILITY_COLUMNS {
        let key = canonical_bus_read(col);
        assert_eq!(
            capi.iter().filter(|s| **s == key).count(),
            1,
            "`{col}` is read {} time(s) by the capi transport, expected exactly \
             one (the `IBus._columns` duplicate is collapsed): {capi:?}",
            capi.iter().filter(|s| **s == key).count()
        );
        assert_eq!(
            r4133.iter().filter(|s| **s == key).count(),
            1,
            "`{col}` is read {} time(s) by the r4133 transport, expected \
             exactly one: {r4133:?}",
            r4133.iter().filter(|s| **s == key).count()
        );
    }
    assert!(
        capi.contains(&"name".to_string()),
        "the capi transport reads the bus name back per bus: {capi:?}"
    );
    assert!(
        !r4133.contains(&"name".to_string()),
        "the r4133 bridge binds no `BUSS` family — its name comes from the \
         `Circuit.AllBusNames` walk, not from a per-bus read: {r4133:?}"
    );
    assert!(
        r4133.contains(&"allbusnames".to_string()),
        "…and that walk must be the r4133 transport's first read: {r4133:?}"
    );

    // Not vacuous: a column that exists nowhere is not "found" by the scans.
    assert!(
        !capi.contains(&canonical_bus_read("cust_minutes")),
        "the capi scan reports a column that does not exist"
    );
    assert!(
        struct_field_names("crates/dss-core/src/exec/view.rs", "BusReliabilityView")
            .iter()
            .all(|f| f != "cust_minutes")
    );
}

/// The `pub <ident>:` field names of one `pub struct <name> {` block, in source
/// order. Doc comments (`///`) and `//` comments are stripped first, so a
/// doc-quoted field spelling is never mistaken for a field.
fn struct_field_names(rel: &str, name: &str) -> Vec<String> {
    let src = read_source(rel);
    let needle = format!("pub struct {name} {{");
    let start = src
        .find(&needle)
        .unwrap_or_else(|| panic!("{rel} has no `{needle}`"));
    let body = &src[start + needle.len()..];
    let end = body
        .find("\n}")
        .unwrap_or_else(|| panic!("{rel}: `{name}` must close at column 0"));
    let body = strip_line_comments(&body[..end], "//");
    let names: Vec<String> = body
        .lines()
        .filter_map(|l| l.trim().strip_prefix("pub "))
        .filter_map(|l| l.split(':').next())
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    assert!(!names.is_empty(), "{rel}: `{name}` has no `pub` field");
    names
}

// ---------------------------------------------------------------------------
// The per-bus capture's read-order contract, over both capture sources
// ---------------------------------------------------------------------------
//
// `GOLDEN_REBASE_PLAN.md` §1.1(a) / coordinator decision D3. The eight columns
// are **group C** — order-free with respect to everything else in a checkpoint:
// each is a plain field read off the active `TDSSBus` behind a "is a bus
// active" guard (capi `CAPI/CAPI_Bus.pas:462-526`, `:604-622`; r4133
// `Version8/Source/DDLL/DBus.pas:129-170` `BUSF` 6-11 and `:60-73` `BUSI` 4-5),
// so none goes through `ComputeIterminal`/`GetCurrents` and none writes engine
// state. Two rules still bind every transport, and neither is expressible in a
// type system — one transport is Python, the other a `libloading` bridge:
//
// 1. **The bus is SELECTED before every read of it**, and the selection is
//    verified. `SetActiveBus` returns the 0-based `BusList` index; a failed
//    lookup leaves the PREVIOUS bus active (r4133 `Common/DSSGlobals.pas`
//    `:739-757`), and because all eight arms initialize their result to
//    `0`/`0.0` BEFORE the `ActiveBusIndex > 0` guard, an unselected bus answers
//    a perfectly plausible zero row. Only the index assertion — never the shape
//    of a value — proves a row belongs to its bus.
// 2. **Each of the eight columns is read exactly once, in the `IBus._columns`
//    order.** The fastdss list (`DSS-Python@origin/fastdss:dss/IBus.py:19-53`)
//    carries `Cust_Interrupts` twice (`:26`, `:27`); both transports collapse
//    the duplicate, because reading a pure field twice proves nothing and would
//    make this pin ambiguous.
//
// The bus block also sits AFTER `Meters.Totals` in both transports, so the
// reliability payload reads meters-then-buses on both channels; that is
// asserted here too, and it is what keeps G1.6(i)'s "Totals is the last
// `Meters` read" rule literally true (the bus reads touch no `Meters` handle).
//
// [`the_bus_reliability_read_order_guard_rejects_a_contaminated_capture`]
// re-runs every scan below over deliberately corrupted copies of the real
// bodies, so a scan that silently matched nothing cannot pass.

/// The eight columns, in `IBus._columns` order with the duplicate collapsed —
/// spelled as the port's `dss_core::exec::BusReliabilityView` fields.
const BUS_RELIABILITY_COLUMNS: &[&str] = &[
    "cust_duration",
    "cust_interrupts",
    "int_duration",
    "lambda_",
    "n_customers",
    "n_interrupts",
    "section_id",
    "total_miles",
];

/// Fold a transport's spelling of one read onto a channel-independent token:
/// `b.Cust_Duration`, `engine.bus_cust_duration` and the bare column name
/// `cust_duration` all become `custduration`.
fn canonical_bus_read(token: &str) -> String {
    let t = token.rsplit('.').next().unwrap_or(token);
    let t = t.strip_prefix("bus_").unwrap_or(t);
    let t = t.strip_prefix("circuit_").unwrap_or(t);
    t.replace('_', "").to_ascii_lowercase()
}

/// `tools/oracle/oracle_server.py::capture_bus_reliability`, docstring and `#`
/// comments removed.
fn capi_bus_reliability_body() -> String {
    let src = read_source("tools/oracle/oracle_server.py");
    let start = src
        .find("def capture_bus_reliability(ckt) -> list:")
        .expect("oracle_server.py has no capture_bus_reliability");
    let body = &src[start..];
    let end = body
        .find("\n    return rows")
        .expect("capture_bus_reliability must end in `return rows`");
    let body = &body[..end];
    // The docstring cites the Pascal arms and the fastdss list; that is prose,
    // not reads.
    let body = match (body.find("\"\"\""), body.rfind("\"\"\"")) {
        (Some(a), Some(b)) if b > a => format!("{}{}", &body[..a], &body[b + 3..]),
        _ => body.to_string(),
    };
    strip_line_comments(&body, "#")
}

/// `crates/dss-epri/src/capture.rs::capture_bus_reliability`, `//` comments
/// removed (its `///` doc block sits above the signature and is never sliced).
fn r4133_bus_reliability_body() -> String {
    let src = read_source("crates/dss-epri/src/capture.rs");
    let start = src
        .find("pub fn capture_bus_reliability(")
        .expect("capture.rs has no capture_bus_reliability");
    let body = &src[start..];
    let end = body
        .find("\n}")
        .expect("capture_bus_reliability must close at column 0");
    strip_line_comments(&body[..end], "//")
}

/// Every `<handle>.<ident>` the body issues, in source order, over the handles
/// a transport uses. Python evaluates statements and a dict literal's values
/// left to right and Rust evaluates `let` bindings in order, so source order
/// **is** read order on both sides.
fn handle_read_sequence(body: &str, handles: &[&str]) -> Vec<String> {
    let bytes = body.as_bytes();
    let mut out: Vec<(usize, String)> = Vec::new();
    for handle in handles {
        let pat = format!("{handle}.");
        for (i, _) in body.match_indices(pat.as_str()) {
            // The handle must be a whole identifier, not the tail of another.
            if i > 0 {
                let prev = bytes[i - 1] as char;
                if prev.is_ascii_alphanumeric() || prev == '_' || prev == '.' {
                    continue;
                }
            }
            let ident = leading_ident(&body[i + pat.len()..]);
            if !ident.is_empty() {
                out.push((i, format!("{handle}.{ident}")));
            }
        }
    }
    out.sort_by_key(|(i, _)| *i);
    out.into_iter().map(|(_, s)| s).collect()
}

/// The capi bus capture's reads: the `ckt` handle (the walk and the selection)
/// and the `b` handle (the active bus).
fn capi_bus_read_sequence(body: &str) -> Vec<String> {
    handle_read_sequence(body, &["ckt", "b"])
}

/// The r4133 bus capture's reads. `assert_clean` is filtered out for the same
/// reason [`r4133_read_sequence`] filters it: it inspects the error channel and
/// reads no bus field.
fn r4133_bus_read_sequence(body: &str) -> Vec<String> {
    handle_read_sequence(body, &["engine"])
        .into_iter()
        .filter(|s| s != "engine.assert_clean")
        .collect()
}

/// The capi transport's literal read sequence.
const CAPI_BUS_RELIABILITY_ORDER: &[&str] = &[
    "ckt.AllBusNames",
    "ckt.SetActiveBus",
    "ckt.ActiveBus",
    "b.Name",
    "b.Cust_Duration",
    "b.Cust_Interrupts",
    "b.Int_Duration",
    "b.Lambda",
    "b.N_Customers",
    "b.N_interrupts",
    "b.SectionID",
    "b.TotalMiles",
];

/// The r4133 transport's literal read sequence. It is the capi one without the
/// two capi-only reads: the bridge has no `ActiveBus` handle to fetch and binds
/// no `BUSS` family, so the name comes from the walk.
const R4133_BUS_RELIABILITY_ORDER: &[&str] = &[
    "engine.circuit_all_bus_names",
    "engine.set_active_bus",
    "engine.bus_cust_duration",
    "engine.bus_cust_interrupts",
    "engine.bus_int_duration",
    "engine.bus_lambda",
    "engine.bus_n_customers",
    "engine.bus_n_interrupts",
    "engine.bus_section_id",
    "engine.bus_total_miles",
];

/// One transport's spelling of the reads the two rules are about.
struct BusReadNames {
    who: &'static str,
    all_names: &'static str,
    select: &'static str,
    /// Reads that are neither the walk, the selection nor one of the eight
    /// columns — the capi handle fetch and its per-bus name read.
    extra: &'static [&'static str],
}

const CAPI_BUS_NAMES: BusReadNames = BusReadNames {
    who: "tools/oracle/oracle_server.py::capture_bus_reliability",
    all_names: "ckt.AllBusNames",
    select: "ckt.SetActiveBus",
    extra: &["ckt.ActiveBus", "b.Name"],
};

const R4133_BUS_NAMES: BusReadNames = BusReadNames {
    who: "crates/dss-epri/src/capture.rs::capture_bus_reliability",
    all_names: "engine.circuit_all_bus_names",
    select: "engine.set_active_bus",
    extra: &[],
};

/// The two rules, checked over a read sequence rather than over a literal list
/// — so a transport may legitimately re-spell its reads and still be held to
/// them.
fn check_bus_reliability_read_rules(seq: &[String], n: &BusReadNames) -> Result<(), String> {
    if seq.is_empty() {
        return Err(format!(
            "{}: the scan found no read at all — the slicer matched nothing, \
             which would make every check below vacuously true",
            n.who
        ));
    }
    if seq.first().map(String::as_str) != Some(n.all_names) {
        return Err(format!(
            "{}: the first read must be `{}` — the walk is over the engine's \
             own `BusList`, and its index is what the selection is checked \
             against, found {:?}",
            n.who,
            n.all_names,
            seq.first()
        ));
    }
    let sel: Vec<usize> = seq
        .iter()
        .enumerate()
        .filter(|(_, s)| s.as_str() == n.select)
        .map(|(i, _)| i)
        .collect();
    let sel = match sel.as_slice() {
        [i] => *i,
        _ => {
            return Err(format!(
                "{}: `{}` must appear exactly once (one selection per bus, at \
                 the head of the walk), found {} occurrence(s) in {seq:?}",
                n.who,
                n.select,
                sel.len()
            ));
        }
    };
    for (i, name) in seq.iter().enumerate() {
        let is_column = BUS_RELIABILITY_COLUMNS
            .iter()
            .any(|c| canonical_bus_read(c) == canonical_bus_read(name));
        if is_column && i < sel {
            return Err(format!(
                "{}: `{name}` is read at position {i}, BEFORE `{}` at {sel}. \
                 Every one of the eight arms initializes its result to `0`/`0.0` \
                 before the `ActiveBusIndex > 0` guard (r4133 `DBus.pas:60-73`, \
                 `:129-170`; capi `CAPI_Bus.pas:462-526`), so a read without a \
                 preceding selection answers a plausible zero row from the \
                 PREVIOUSLY selected bus.",
                n.who, n.select
            ));
        }
    }
    for col in BUS_RELIABILITY_COLUMNS {
        let key = canonical_bus_read(col);
        let hits = seq.iter().filter(|s| canonical_bus_read(s) == key).count();
        if hits != 1 {
            return Err(format!(
                "{}: column `{col}` is read {hits} time(s), expected exactly \
                 one — the `IBus._columns` duplicate `Cust_Interrupts` \
                 (`dss/IBus.py:26-27`) is collapsed on both transports: {seq:?}",
                n.who
            ));
        }
    }
    let known: Vec<String> = BUS_RELIABILITY_COLUMNS
        .iter()
        .map(|c| canonical_bus_read(c))
        .chain([
            canonical_bus_read(n.all_names),
            canonical_bus_read(n.select),
        ])
        .chain(n.extra.iter().map(|e| canonical_bus_read(e)))
        .collect();
    for s in seq {
        if !known.contains(&canonical_bus_read(s)) {
            return Err(format!(
                "{}: `{s}` is a read this contract does not know about. Add it \
                 to the pinned order deliberately (with its Pascal citation and \
                 its order class) or drop it; an unpinned read is an unpinned \
                 capture.",
                n.who
            ));
        }
    }
    Ok(())
}

/// The capi bus capture's literal read sequence.
#[test]
fn the_capi_bus_reliability_capture_reads_in_the_pinned_order() {
    let seq = capi_bus_read_sequence(&capi_bus_reliability_body());
    if let Err(msg) = check_literal_order(&seq, CAPI_BUS_RELIABILITY_ORDER, CAPI_BUS_NAMES.who) {
        panic!("{msg}");
    }
    // …and the block's slot: after `Meters.Totals`, so the payload reads
    // meters-then-buses and no `Meters` read can follow `Totals`.
    assert_call_order(
        &capi_reliability_body(),
        &[
            "totals = [float(x) for x in m.Totals]",
            "buses = capture_bus_reliability(ckt)",
        ],
        "tools/oracle/oracle_server.py::capture_reliability",
    );
}

/// The r4133 bus capture's literal read sequence, and the identical slot.
#[test]
fn the_r4133_bus_reliability_capture_reads_in_the_pinned_order() {
    let seq = r4133_bus_read_sequence(&r4133_bus_reliability_body());
    if let Err(msg) = check_literal_order(&seq, R4133_BUS_RELIABILITY_ORDER, R4133_BUS_NAMES.who) {
        panic!("{msg}");
    }
    assert_call_order(
        &r4133_reliability_body(),
        &[
            "let totals = engine.meters_totals()?;",
            "let buses = capture_bus_reliability(engine)?;",
        ],
        "crates/dss-epri/src/capture.rs::capture_reliability",
    );
}

/// Both bus captures select before they read, and read each column once.
#[test]
fn both_bus_reliability_captures_select_the_bus_before_reading_it() {
    for (seq, names) in [
        (
            capi_bus_read_sequence(&capi_bus_reliability_body()),
            &CAPI_BUS_NAMES,
        ),
        (
            r4133_bus_read_sequence(&r4133_bus_reliability_body()),
            &R4133_BUS_NAMES,
        ),
    ] {
        if let Err(msg) = check_bus_reliability_read_rules(&seq, names) {
            panic!("{msg}");
        }
    }
}

/// The two bus captures read the SAME eight columns in the SAME order, so a
/// channel-to-channel divergence on this surface can never be a capture-shape
/// artefact. The capi-only reads (`ckt.ActiveBus`, `b.Name`) are folded away:
/// the r4133 bridge fetches no handle and takes the name from the walk.
#[test]
fn the_two_bus_reliability_captures_read_the_same_fields() {
    let capi: Vec<String> = capi_bus_read_sequence(&capi_bus_reliability_body())
        .iter()
        .map(|s| canonical_bus_read(s))
        .filter(|s| s != "activebus" && s != "name")
        .collect();
    let r4133: Vec<String> = r4133_bus_read_sequence(&r4133_bus_reliability_body())
        .iter()
        .map(|s| canonical_bus_read(s))
        .collect();
    assert_eq!(
        capi, r4133,
        "the two transports must issue the same walk, the same selection and \
         the same eight column reads, in the same order"
    );
    let mut want: Vec<String> = vec!["allbusnames".to_string(), "setactivebus".to_string()];
    want.extend(
        BUS_RELIABILITY_COLUMNS
            .iter()
            .map(|c| canonical_bus_read(c)),
    );
    assert_eq!(
        capi, want,
        "…and that shared sequence is the walk, the selection, then the eight \
         `IBus._columns` reliability columns"
    );
}

/// **The bus-capture scans are not vacuous.** Every rule is re-run over
/// corrupted copies of the *real* capture bodies — a column read hoisted above
/// its selection, a dropped selection, a duplicated read, a reordered column —
/// and must reject each one, on both transports. A scan that silently matched
/// nothing would pass the four tests above and prove nothing, so the empty
/// sequence is rejected explicitly too.
#[test]
fn the_bus_reliability_read_order_guard_rejects_a_contaminated_capture() {
    // 0. The degenerate case the other drives cannot reach.
    let err = check_bus_reliability_read_rules(&[], &CAPI_BUS_NAMES)
        .expect_err("the scan accepted an empty read sequence");
    assert!(err.contains("no read at all"), "{err}");

    // 1. A column read hoisted above the selection — the failure mode that
    //    silently attributes one bus's row to another.
    let capi_bad = capi_bus_reliability_body().replacen(
        "        idx = ckt.SetActiveBus(name)",
        "        _x = b.Lambda\n        idx = ckt.SetActiveBus(name)",
        1,
    );
    let seq = capi_bus_read_sequence(&capi_bad);
    let err = check_bus_reliability_read_rules(&seq, &CAPI_BUS_NAMES)
        .expect_err("the capi scan accepted a column read before the selection");
    assert!(err.contains("BEFORE"), "{err}");
    assert!(
        check_literal_order(&seq, CAPI_BUS_RELIABILITY_ORDER, "probe").is_err(),
        "the capi literal scan accepted a hoisted column read"
    );

    let r4133_bad = r4133_bus_reliability_body().replacen(
        "        let idx = engine.set_active_bus(name);",
        "        let _x = engine.bus_lambda()?;\n        let idx = engine.set_active_bus(name);",
        1,
    );
    let seq = r4133_bus_read_sequence(&r4133_bad);
    let err = check_bus_reliability_read_rules(&seq, &R4133_BUS_NAMES)
        .expect_err("the r4133 scan accepted a column read before the selection");
    assert!(err.contains("BEFORE"), "{err}");

    // 2. A dropped selection.
    let capi_bad = capi_bus_reliability_body().replacen("ckt.SetActiveBus(name)", "i", 1);
    let err = check_bus_reliability_read_rules(&capi_bus_read_sequence(&capi_bad), &CAPI_BUS_NAMES)
        .expect_err("the capi scan accepted a capture that never selects a bus");
    assert!(err.contains("exactly once"), "{err}");

    let r4133_bad =
        r4133_bus_reliability_body().replacen("engine.set_active_bus(name)", "i as i32", 1);
    assert!(
        check_bus_reliability_read_rules(&r4133_bus_read_sequence(&r4133_bad), &R4133_BUS_NAMES)
            .is_err(),
        "the r4133 scan accepted a capture that never selects a bus"
    );

    // 3. A column read twice — the `IBus._columns` duplicate, un-collapsed.
    let capi_bad = capi_bus_reliability_body().replacen(
        "\"int_duration\": float(b.Int_Duration),",
        "\"int_duration\": float(b.Int_Duration),\n                \"dup\": float(b.Cust_Interrupts),",
        1,
    );
    let err = check_bus_reliability_read_rules(&capi_bus_read_sequence(&capi_bad), &CAPI_BUS_NAMES)
        .expect_err("the capi scan accepted a column read twice");
    assert!(err.contains("read 2 time(s)"), "{err}");

    let r4133_bad = r4133_bus_reliability_body().replacen(
        "        let int_duration = engine.bus_int_duration()?;",
        "        let int_duration = engine.bus_int_duration()?;\n        let _dup = engine.bus_cust_interrupts()?;",
        1,
    );
    assert!(
        check_bus_reliability_read_rules(&r4133_bus_read_sequence(&r4133_bad), &R4133_BUS_NAMES)
            .is_err(),
        "the r4133 scan accepted a column read twice"
    );

    // 4. A read this contract does not know about.
    let r4133_bad = r4133_bus_reliability_body().replacen(
        "        let total_miles = engine.bus_total_miles()?;",
        "        let total_miles = engine.bus_total_miles()?;\n        let _v = engine.bus_kv_base()?;",
        1,
    );
    let err =
        check_bus_reliability_read_rules(&r4133_bus_read_sequence(&r4133_bad), &R4133_BUS_NAMES)
            .expect_err("the r4133 scan accepted an unpinned read");
    assert!(err.contains("does not know about"), "{err}");

    // 5. A reordered column is caught by the literal scans on both transports.
    let capi_bad = capi_bus_reliability_body().replacen("b.Cust_Duration", "b.TotalMiles", 1);
    assert!(
        check_literal_order(
            &capi_bus_read_sequence(&capi_bad),
            CAPI_BUS_RELIABILITY_ORDER,
            "probe"
        )
        .is_err(),
        "the capi literal scan accepted a reordered column"
    );
    let r4133_bad = r4133_bus_reliability_body().replacen(
        "engine.bus_cust_duration()",
        "engine.bus_lambda()",
        1,
    );
    assert!(
        check_literal_order(
            &r4133_bus_read_sequence(&r4133_bad),
            R4133_BUS_RELIABILITY_ORDER,
            "probe"
        )
        .is_err(),
        "the r4133 literal scan accepted a reordered column"
    );

    // 6. Each corruption above really did change the body it was applied to.
    assert_ne!(
        capi_bus_reliability_body(),
        capi_bad,
        "a drive that changed nothing proves nothing"
    );
    assert_ne!(r4133_bus_reliability_body(), r4133_bad);
}

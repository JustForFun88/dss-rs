//! The G1.0 mode-capability acceptance test for the r4133 bridge
//! (`GOLDEN_REBASE_PLAN.md` WP-G1, sub-step G1.0; coordinator decision D2).
//!
//! The vendored `OpenDSSDirect.dll` exports no per-property `*_Get_*` symbol, so
//! "does this DLL revision serve the property WP-G1 needs?" cannot be answered
//! by a `GetProcAddress` miss. It is answered here instead, once for all of
//! WP-G1: every row of [`dss_epri::modes::WP_G1_MODES`] is driven against the
//! **real** DLL on a solved IEEE13 and must classify
//! [`ModeStatus::Served`] — that is D2's "either a working capture proven on one
//! gated r4133 case, or a recorded miss", executed as a table walk. The
//! discriminator is the companion case: a mode index past every family's last
//! `case` arm must classify `UnknownMode`, carrying that family's own sentinel
//! literal.
//!
//! The two [`dss_epri::modes::DO_NOT_CALL`] rows are exercised through the same
//! entry points the accessors use and must be refused **before** any FFI. That
//! refusal is not merely asserted: `Bus` V:17 (`ZSC012Matrix`,
//! `DBus.pas:803-838`, nil `Zsc` with no `Assigned` guard) was measured to kill
//! the worker process outright on a bus with no fault study, so a leak of the
//! refusal would abort this test binary rather than fail an assertion.
//!
//! Needs no oracle installed — only the git-tracked r4133 DLL and the vendored
//! IEEE13 deck.

#![cfg(windows)]

use std::collections::BTreeSet;
use std::path::PathBuf;

use dss_epri::dss::Engine;
use dss_epri::modes::{self, ModeEffect, ModeKind, ModeSpec, ModeStatus};

/// A mode index past the last `case` arm of every DDLL family (the largest in
/// the whole DDLL is `SolutionI(51)`), so it always reaches the `else` branch.
const BOGUS_MODE: i32 = 987;

/// The families WP-G1 reads, with the ABI shapes each one actually exports
/// (`crate::families::REGISTRY`): `Topology` has no `F` entry point and
/// `PDElements` has no `V`.
const WP_G1_FAMILIES: &[(&str, &[ModeKind])] = &[
    (
        "CktElement",
        &[ModeKind::I, ModeKind::F, ModeKind::S, ModeKind::V],
    ),
    ("Bus", &[ModeKind::I, ModeKind::F, ModeKind::S, ModeKind::V]),
    (
        "Circuit",
        &[ModeKind::I, ModeKind::F, ModeKind::S, ModeKind::V],
    ),
    (
        "Meters",
        &[ModeKind::I, ModeKind::F, ModeKind::S, ModeKind::V],
    ),
    ("Topology", &[ModeKind::I, ModeKind::S, ModeKind::V]),
    (
        "Solution",
        &[ModeKind::I, ModeKind::F, ModeKind::S, ModeKind::V],
    ),
    ("PDElements", &[ModeKind::I, ModeKind::F, ModeKind::S]),
];

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = <root>/crates/dss-epri (baked at build time).
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// A solved IEEE13 with an EnergyMeter attached, so the `Meters` rows read a
/// real meter rather than the family's "no active meter" defaults.
fn solved_ieee13() -> Engine {
    let dll = dss_epri::smoke::dll_path();
    assert!(dll.is_file(), "r4133 DLL not found: {}", dll.display());
    let engine = Engine::new(&dll).expect("load the vendored r4133 DLL");
    let case = workspace_root()
        .join("tests/corpus/electricdss-tst/Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss")
        .to_string_lossy()
        .replace('\\', "/");
    engine.clear().expect("clear");
    engine.compile(&case, false).expect("compile IEEE13");
    engine
        .post("New EnergyMeter.m1 element=Line.650632 terminal=1")
        .expect("attach an EnergyMeter");
    engine.solve(false).expect("solve IEEE13");
    assert!(engine.converged(), "IEEE13 must converge");
    engine
}

/// Re-select the fixture the family rows read from. Called before **every**
/// mode: fifteen rows are [`ModeEffect::Impure`] and move a cursor or a memoized
/// structure (`PDElements.ParentPDElement` moves `ActiveCktElement` itself; the
/// five `Circuit` loss/power rows, the two `CktElement.Has*Control` rows and
/// `Meters.Totals` walk a `PointerList` to exhaustion or re-totalize; the six
/// `Topology` rows build and memoize `GetTopology` and move its cursor), so
/// without this a later row would silently read a different object.
fn select_fixture(e: &Engine) {
    // `Meters.First` (`MetersI(0)`, `DMeters.pas:32-52`) sets `ActiveCktElement`
    // to the meter object itself, so it must run BEFORE the element selection —
    // otherwise every `CktElement`/`PDElements` row would read `EnergyMeter.m1`
    // (measured: NumTerminals 1, NumPhases 3, TotalPowers [0.0, 0.0]) instead of
    // the line.
    assert!(e.meters_first(), "the deck must have an active EnergyMeter");
    e.set_active_element("Line.650632");
    assert!(e.set_active_bus("671") >= 0, "bus 671 must exist in IEEE13");
}

/// The whole G1.0 mode-capability proof, in one `#[test]`.
///
/// The DDLL is a process-global, single-threaded singleton (`crate::dss`
/// module doc): two `#[test]`s driving it from cargo's thread pool corrupt each
/// other's circuit, so every phase below runs sequentially against **one**
/// engine, the way `tests/protocol.rs` drives its worker.
#[test]
fn r4133_mode_capability_is_complete_for_wp_g1() {
    let e = solved_ieee13();
    the_fixture_selects_the_line_the_bus_and_the_meter(&e);
    every_wp_g1_mode_is_served(&e);
    a_bogus_mode_is_an_unknown_mode_on_every_wp_g1_family_and_shape(&e);
    the_bus_v_sentinel_is_only_caught_by_containment(&e);
    the_do_not_call_modes_are_refused_before_any_ffi(&e);
    every_wp_g1_mode_has_a_typed_accessor_that_reads_the_solved_deck(&e);
    distinguishing_readings_separate_same_shape_modes_within_a_family(&e);
    r4133_solution_flags_are_zero_one_ints(&e);
    the_parent_read_hijacks_the_active_element_and_the_capture_reads_it_last(&e);
    // Nothing in the walk left a non-zero errno behind for the next caller.
    let (errno, desc) = e.poll_error();
    assert_eq!(errno, 0, "the mode walk left errno {errno} set: {desc}");
}

/// The fixture the family rows read from is what it claims to be, pinned by
/// exact readings of the vendored r4133 DLL on the vendored IEEE13
/// (2026-09-04). This is not decoration: `Meters.First` moves
/// `ActiveCktElement` to the meter, and an earlier ordering of
/// [`select_fixture`] silently pointed every `CktElement`/`PDElements` row at
/// `EnergyMeter.m1` (NumTerminals 1, TotalPowers `[0.0, 0.0]`) while still
/// classifying every mode `Served`.
fn the_fixture_selects_the_line_the_bus_and_the_meter(e: &Engine) {
    select_fixture(e);
    assert_eq!(e.ckt_element_num_terminals().unwrap(), 2, "Line.650632");
    assert_eq!(e.ckt_element_num_phases().unwrap(), 3);
    select_fixture(e);
    assert_eq!(e.ckt_element_node_order().unwrap(), vec![1, 2, 3, 1, 2, 3]);
    select_fixture(e);
    assert_eq!(
        e.ckt_element_energy_meter().unwrap(),
        "m1",
        "the element must be inside the EnergyMeter zone"
    );
    select_fixture(e);
    assert_eq!(
        e.pd_elements_fault_rate().unwrap(),
        0.1,
        "the PDElements cursor must see the Line (its default faultrate), not the meter"
    );
    select_fixture(e);
    // `CktElementI(12)`'s codomain is {0, 1} (`DCktElement.pas:137`, `:263`), so
    // the strict decode must read the live element as enabled — under a `!= 0`
    // decode the family's `-1` unknown-mode sentinel (`:308`) would read the
    // same way, which is what routes a capture into `CktElementV(19)`'s
    // unguarded `NodeRef^[i]` (`:1099`).
    assert!(
        e.ckt_element_enabled().unwrap(),
        "Line.650632 is enabled in IEEE13"
    );
    select_fixture(e);
    assert_eq!(e.bus_distance().unwrap(), 1.2192, "bus 671's DistFromMeter");
    select_fixture(e);
    assert_eq!(e.solution_iterations().unwrap(), 2, "IEEE13 snapshot solve");
}

/// G1.0 audit settlement (T4): the walk above proves *capability* — that each
/// mode exists and decodes into its declared shape — not *identity*: a mode
/// index transposed with a sibling of the same family and shape would still
/// classify `Served`. These are exact readings of the vendored r4133 DLL on the
/// vendored IEEE13 fixture (measured 2026-09-04) chosen so that a transposition
/// inside a family moves the number, family by family.
///
/// `Meters` is deliberately absent: every one of its `I:20..27` / `F:0..6`
/// reliability registers reads `0` / `0.0` on this fixture (no `RelCalc`), so no
/// pin there could discriminate. G1.6 wires that surface and gets its own
/// values; the full per-mode value validation is D2's job for each surface
/// sub-step, not this rail's.
fn distinguishing_readings_separate_same_shape_modes_within_a_family(e: &Engine) {
    // Circuit V:0 (whole-circuit PD losses, W) vs V:1 (Line losses only, kW) vs
    // V:3 (source power) — three `myType = 3` rows of one family.
    select_fixture(e);
    assert_eq!(
        e.circuit_losses().unwrap(),
        vec![112_391.709_058_989_2, 327_860.856_436_449_6],
        "Circuit.Losses (V:0)"
    );
    select_fixture(e);
    assert_eq!(
        e.circuit_line_losses().unwrap(),
        vec![106.484_751_187_618_73, 317.175_285_717_855_73],
        "Circuit.LineLosses (V:1) — a different mode of the same family and shape"
    );
    select_fixture(e);
    assert_eq!(
        e.circuit_total_power().unwrap(),
        vec![-3_567.053_778_419_098_3, -1_736.439_510_362_667_5],
        "Circuit.TotalPower (V:3)"
    );
    // Topology I:0 vs its two I siblings, which are both 0 on this radial deck.
    select_fixture(e);
    assert_eq!(
        e.topology_num_loops().unwrap(),
        1,
        "Topology.NumLoops (I:0)"
    );
    // PDElements: two `I` and two `F` rows that are pairwise distinct.
    select_fixture(e);
    assert_eq!(
        e.pd_elements_total_customers().unwrap(),
        15,
        "PDElements.TotalCustomers (I:5)"
    );
    select_fixture(e);
    assert_eq!(
        e.pd_elements_from_terminal().unwrap(),
        1,
        "PDElements.FromTerminal (I:7)"
    );
    select_fixture(e);
    assert_eq!(
        e.pd_elements_pct_permanent().unwrap(),
        20.0,
        "PDElements.PctPermanent (F:2)"
    );
    select_fixture(e);
    assert_eq!(
        e.pd_elements_repair_time().unwrap(),
        3.0,
        "PDElements.RepairTime (F:6)"
    );
}

/// **The G1.0 acceptance**: every mode WP-G1 will read is served by the
/// vendored r4133 DLL — no expected-miss list, no per-channel mask.
///
/// The same walk records each row's `myType` for the `V` rows through
/// [`Engine::read_mode`], which fails when the observed tag is not the one the
/// `case` arm assigns, so this proves the declared *shape* too, not only that
/// the mode exists.
fn every_wp_g1_mode_is_served(e: &Engine) {
    let mut misses = Vec::new();
    for row in modes::WP_G1_MODES {
        select_fixture(e);
        match e.probe_mode(row.family, row.kind, row.mode) {
            Ok(ModeStatus::Served) => {}
            Ok(other) => misses.push(format!("{row}: {other}")),
            Err(err) => misses.push(format!("{row}: probe failed: {err}")),
        }
        // The typed path agrees: the reply decodes into the row's declared
        // shape (`read_mode` rejects a `myType` the arm does not assign).
        select_fixture(e);
        if let Err(err) = e.read_mode(row) {
            misses.push(format!("{row}: typed read failed: {err}"));
        }
    }
    assert!(
        misses.is_empty(),
        "{} of {} WP-G1 modes are not served by this DLL:\n  {}",
        misses.len(),
        modes::WP_G1_MODES.len(),
        misses.join("\n  ")
    );
}

/// The discriminator for the test above: a mode index past every family's last
/// `case` arm must classify [`ModeStatus::UnknownMode`] on **every** WP-G1
/// family and shape, carrying that family's own sentinel — the `-1` / `-1.0`
/// scalars, the per-family `S` literal, and a `V` phrase matched by
/// containment.
fn a_bogus_mode_is_an_unknown_mode_on_every_wp_g1_family_and_shape(e: &Engine) {
    select_fixture(e);
    let mut checked = 0usize;
    for (family, kinds) in WP_G1_FAMILIES {
        for &kind in *kinds {
            let status = e
                .probe_mode(family, kind, BOGUS_MODE)
                .unwrap_or_else(|err| panic!("{family}/{kind}:{BOGUS_MODE} probe failed: {err}"));
            let ModeStatus::UnknownMode { sentinel } = &status else {
                panic!("{family}/{kind}:{BOGUS_MODE} classified {status}, expected a miss");
            };
            match kind {
                ModeKind::I => assert_eq!(sentinel, "-1", "{family} I sentinel"),
                ModeKind::F => assert_eq!(sentinel, "-1", "{family} F sentinel"),
                ModeKind::S => assert_eq!(
                    sentinel,
                    modes::s_sentinel(family).expect("a measured S literal"),
                    "{family} S sentinel"
                ),
                ModeKind::V => assert!(
                    modes::SENTINEL_V_PHRASES
                        .iter()
                        .any(|p| sentinel.contains(p)),
                    "{family} V sentinel {sentinel:?} carries none of the measured phrases"
                ),
            }
            checked += 1;
        }
    }
    // 4 shapes × 7 families, less Topology's missing F and PDElements' missing V.
    assert_eq!(checked, 26, "the bogus-mode sweep skipped a shape");
}

/// `DBusV`'s unknown-mode branch (`DBus.pas:899-903`) is the only one in the
/// DDLL that omits the `setlength(myStrArray, 0)` its siblings do
/// (`DCktElement.pas:1249`, `DCircuit.pas:791`), so it **appends** to the
/// DLL-global string buffer. Priming that buffer with another family's sentinel
/// and then probing `Bus` reproduces the concatenation live, which is why
/// [`modes::classify_v`] matches by containment and never by equality.
fn the_bus_v_sentinel_is_only_caught_by_containment(e: &Engine) {
    select_fixture(e);
    // Prime the shared buffer with `CircuitV`'s sentinel …
    let primer = e
        .probe_mode("Circuit", ModeKind::V, BOGUS_MODE)
        .expect("Circuit V probe");
    let ModeStatus::UnknownMode { sentinel: primer } = primer else {
        panic!("Circuit V:{BOGUS_MODE} must be a miss, got {primer}");
    };
    assert_eq!(primer, "Error, parameter not recognized");
    // … then read `Bus`, which writes its own phrase without clearing it.
    let status = e
        .probe_mode("Bus", ModeKind::V, BOGUS_MODE)
        .expect("Bus V probe");
    let ModeStatus::UnknownMode { sentinel } = status else {
        panic!("Bus V:{BOGUS_MODE} must be a miss, got {status}");
    };
    assert_eq!(
        sentinel, "Error, parameter not recognizedCommand not recognized",
        "the measured 2026-09-04 concatenation moved"
    );
    // The rule this pins: equality against the family's own phrase misses it,
    // containment catches it.
    assert_ne!(sentinel, "Command not recognized");
    assert!(sentinel.contains("Command not recognized"));
    assert!(modes::classify_v(4, std::slice::from_ref(&sentinel)).is_unknown_mode());
}

/// Neither [`modes::DO_NOT_CALL`] row may reach the DLL through the accessor
/// path or the probe. The proof that the refusal really is pre-FFI is the test
/// binary still running: `Bus` V:17 nil-derefs inside the DLL and kills the
/// process outright on a bus with no fault study (measured, G1.0 probe).
fn the_do_not_call_modes_are_refused_before_any_ffi(e: &Engine) {
    select_fixture(e);
    for (family, kind, mode, why) in modes::DO_NOT_CALL {
        let status = e
            .probe_mode(family, *kind, *mode)
            .unwrap_or_else(|err| panic!("{family}/{kind}:{mode} probe errored: {err}"));
        assert!(
            matches!(status, ModeStatus::DoNotCall(w) if w == *why),
            "{family}/{kind}:{mode} was not refused: {status}"
        );
        // The accessor path refuses too: a hand-built row for the same triple
        // must not dispatch.
        let forged = ModeSpec {
            family,
            kind: *kind,
            mode: *mode,
            name: "forged.DoNotCall",
            pas: "see modes::DO_NOT_CALL",
            v_type: Some(1),
            effect: ModeEffect::Pure,
        };
        let err = e
            .read_mode(&forged)
            .expect_err("read_mode must refuse a do-not-call row");
        assert!(
            err.to_string().contains("do-not-call"),
            "unexpected refusal text: {err}"
        );
    }
    // Still alive, and the engine is still usable: nothing was dispatched.
    assert!(e.converged());
}

/// Every typed accessor is exercised once on the solved deck, and the set of
/// rows they cover is exactly [`modes::WP_G1_MODES`] — so a row can neither be
/// added without a reader nor read by a function nothing calls.
fn every_wp_g1_mode_has_a_typed_accessor_that_reads_the_solved_deck(e: &Engine) {
    let mut seen: BTreeSet<&'static str> = BTreeSet::new();

    macro_rules! chk {
        ($row:ident, $call:expr) => {{
            select_fixture(e);
            let _ = $call.unwrap_or_else(|err| panic!("{}: {err}", modes::$row));
            assert!(
                seen.insert(modes::$row.name),
                "{} read twice",
                modes::$row.name
            );
        }};
    }

    // CktElement
    chk!(CKT_ELEMENT_NUM_TERMINALS, e.ckt_element_num_terminals());
    chk!(CKT_ELEMENT_NUM_CONDUCTORS, e.ckt_element_num_conductors());
    chk!(CKT_ELEMENT_NUM_PHASES, e.ckt_element_num_phases());
    chk!(
        CKT_ELEMENT_HAS_SWITCH_CONTROL,
        e.ckt_element_has_switch_control()
    );
    chk!(
        CKT_ELEMENT_HAS_VOLT_CONTROL,
        e.ckt_element_has_volt_control()
    );
    chk!(CKT_ELEMENT_NUM_CONTROLS, e.ckt_element_num_controls());
    chk!(CKT_ELEMENT_OCP_DEV_INDEX, e.ckt_element_ocp_dev_index());
    chk!(CKT_ELEMENT_OCP_DEV_TYPE, e.ckt_element_ocp_dev_type());
    chk!(CKT_ELEMENT_ENABLED, e.ckt_element_enabled());
    chk!(CKT_ELEMENT_HAS_OCP_DEVICE, e.ckt_element_has_ocp_device());
    chk!(CKT_ELEMENT_ENERGY_METER, e.ckt_element_energy_meter());
    chk!(CKT_ELEMENT_PHASE_LOSSES, e.ckt_element_phase_losses());
    chk!(CKT_ELEMENT_SEQ_VOLTAGES, e.ckt_element_seq_voltages());
    chk!(CKT_ELEMENT_SEQ_CURRENTS, e.ckt_element_seq_currents());
    chk!(CKT_ELEMENT_SEQ_POWERS, e.ckt_element_seq_powers());
    chk!(CKT_ELEMENT_RESIDUALS, e.ckt_element_residuals());
    chk!(
        CKT_ELEMENT_CPLX_SEQ_VOLTAGES,
        e.ckt_element_cplx_seq_voltages()
    );
    chk!(
        CKT_ELEMENT_CPLX_SEQ_CURRENTS,
        e.ckt_element_cplx_seq_currents()
    );
    chk!(CKT_ELEMENT_NODE_ORDER, e.ckt_element_node_order());
    chk!(
        CKT_ELEMENT_CURRENTS_MAG_ANG,
        e.ckt_element_currents_mag_ang()
    );
    chk!(
        CKT_ELEMENT_VOLTAGES_MAG_ANG,
        e.ckt_element_voltages_mag_ang()
    );
    chk!(CKT_ELEMENT_TOTAL_POWERS, e.ckt_element_total_powers());
    // Bus
    chk!(BUS_DISTANCE, e.bus_distance());
    chk!(BUS_SEQ_VOLTAGES, e.bus_seq_voltages());
    chk!(BUS_VOC, e.bus_voc());
    chk!(BUS_ISC, e.bus_isc());
    chk!(BUS_PU_VOLTAGES, e.bus_pu_voltages());
    chk!(BUS_ZSC_MATRIX, e.bus_zsc_matrix());
    chk!(BUS_ZSC1, e.bus_zsc1());
    chk!(BUS_ZSC0, e.bus_zsc0());
    chk!(BUS_YSC_MATRIX, e.bus_ysc_matrix());
    chk!(BUS_CPLX_SEQ_VOLTAGES, e.bus_cplx_seq_voltages());
    chk!(BUS_VLL, e.bus_vll());
    chk!(BUS_PU_VLL, e.bus_pu_vll());
    chk!(BUS_VMAG_ANGLE, e.bus_vmag_angle());
    chk!(BUS_PU_VMAG_ANGLE, e.bus_pu_vmag_angle());
    chk!(BUS_ALL_PCE_AT_BUS, e.bus_all_pce_at_bus());
    chk!(BUS_ALL_PDE_AT_BUS, e.bus_all_pde_at_bus());
    // Circuit
    chk!(CIRCUIT_LOSSES, e.circuit_losses());
    chk!(CIRCUIT_LINE_LOSSES, e.circuit_line_losses());
    chk!(CIRCUIT_SUBSTATION_LOSSES, e.circuit_substation_losses());
    chk!(CIRCUIT_TOTAL_POWER, e.circuit_total_power());
    chk!(CIRCUIT_ALL_ELEMENT_LOSSES, e.circuit_all_element_losses());
    chk!(CIRCUIT_ALL_BUS_MAG_PU, e.circuit_all_bus_mag_pu());
    chk!(CIRCUIT_ALL_BUS_DISTANCES, e.circuit_all_bus_distances());
    chk!(CIRCUIT_ALL_NODE_DISTANCES, e.circuit_all_node_distances());
    // Meters
    chk!(METERS_TOTAL_CUSTOMERS, e.meters_total_customers());
    chk!(METERS_NUM_SECTIONS, e.meters_num_sections());
    chk!(METERS_OCP_DEVICE_TYPE, e.meters_ocp_device_type());
    chk!(
        METERS_NUM_SECTION_CUSTOMERS,
        e.meters_num_section_customers()
    );
    chk!(METERS_NUM_SECTION_BRANCHES, e.meters_num_section_branches());
    chk!(METERS_SECT_SEQ_IDX, e.meters_sect_seq_idx());
    chk!(METERS_SECT_TOTAL_CUST, e.meters_sect_total_cust());
    chk!(METERS_SAIFI, e.meters_saifi());
    chk!(METERS_SAIFI_KW, e.meters_saifi_kw());
    chk!(METERS_SAIDI, e.meters_saidi());
    chk!(METERS_CUST_INTERRUPTS, e.meters_cust_interrupts());
    chk!(METERS_AVG_REPAIR_TIME, e.meters_avg_repair_time());
    chk!(
        METERS_FAULT_RATE_X_REPAIR_HRS,
        e.meters_fault_rate_x_repair_hrs()
    );
    chk!(METERS_SUM_BRANCH_FLT_RATES, e.meters_sum_branch_flt_rates());
    chk!(METERS_TOTALS, e.meters_totals());
    chk!(METERS_CALC_CURRENT, e.meters_calc_current());
    chk!(METERS_ALLOC_FACTORS, e.meters_alloc_factors());
    // Topology
    chk!(TOPOLOGY_NUM_LOOPS, e.topology_num_loops());
    chk!(
        TOPOLOGY_NUM_ISOLATED_BRANCHES,
        e.topology_num_isolated_branches()
    );
    chk!(TOPOLOGY_NUM_ISOLATED_LOADS, e.topology_num_isolated_loads());
    chk!(TOPOLOGY_ALL_LOOPED_PAIRS, e.topology_all_looped_pairs());
    chk!(
        TOPOLOGY_ALL_ISOLATED_BRANCHES,
        e.topology_all_isolated_branches()
    );
    chk!(TOPOLOGY_ALL_ISOLATED_LOADS, e.topology_all_isolated_loads());
    // Solution
    chk!(SOLUTION_MODE, e.solution_mode());
    chk!(SOLUTION_HOUR, e.solution_hour());
    chk!(SOLUTION_YEAR, e.solution_year());
    chk!(SOLUTION_ITERATIONS, e.solution_iterations());
    chk!(SOLUTION_CONTROL_ITERATIONS, e.solution_control_iterations());
    chk!(SOLUTION_SYSTEM_Y_CHANGED, e.solution_system_y_changed());
    chk!(SOLUTION_TOTAL_ITERATIONS, e.solution_total_iterations());
    chk!(
        SOLUTION_MOST_ITERATIONS_DONE,
        e.solution_most_iterations_done()
    );
    chk!(
        SOLUTION_CONTROL_ACTIONS_DONE,
        e.solution_control_actions_done()
    );
    chk!(SOLUTION_SECONDS, e.solution_seconds());
    chk!(SOLUTION_LOAD_MULT, e.solution_load_mult());
    chk!(SOLUTION_DBL_HOUR, e.solution_dbl_hour());
    chk!(SOLUTION_INC_MATRIX, e.solution_inc_matrix());
    chk!(SOLUTION_INC_MATRIX_ROWS, e.solution_inc_matrix_rows());
    chk!(SOLUTION_INC_MATRIX_COLS, e.solution_inc_matrix_cols());
    chk!(SOLUTION_LAPLACIAN, e.solution_laplacian());
    // PDElements
    chk!(PD_ELEMENTS_FIRST, e.pd_elements_first());
    chk!(PD_ELEMENTS_NEXT, e.pd_elements_next());
    chk!(PD_ELEMENTS_NAME, e.pd_elements_name());
    chk!(PD_ELEMENTS_IS_SHUNT, e.pd_elements_is_shunt());
    chk!(PD_ELEMENTS_NUM_CUSTOMERS, e.pd_elements_num_customers());
    chk!(PD_ELEMENTS_TOTAL_CUSTOMERS, e.pd_elements_total_customers());
    chk!(
        PD_ELEMENTS_PARENT_PD_ELEMENT,
        e.pd_elements_parent_pd_element()
    );
    chk!(PD_ELEMENTS_FROM_TERMINAL, e.pd_elements_from_terminal());
    chk!(PD_ELEMENTS_SECTION_ID, e.pd_elements_section_id());
    chk!(PD_ELEMENTS_FAULT_RATE, e.pd_elements_fault_rate());
    chk!(PD_ELEMENTS_PCT_PERMANENT, e.pd_elements_pct_permanent());
    chk!(PD_ELEMENTS_LAMBDA, e.pd_elements_lambda());
    chk!(PD_ELEMENTS_ACCUMULATED_L, e.pd_elements_accumulated_l());
    chk!(PD_ELEMENTS_REPAIR_TIME, e.pd_elements_repair_time());
    chk!(PD_ELEMENTS_TOTAL_MILES, e.pd_elements_total_miles());

    let expected: BTreeSet<&'static str> = modes::WP_G1_MODES.iter().map(|m| m.name).collect();
    let missing: Vec<&str> = expected.difference(&seen).copied().collect();
    assert!(
        missing.is_empty(),
        "these WP-G1 rows have no typed accessor call: {missing:?}"
    );
    assert_eq!(seen.len(), modes::WP_G1_MODES.len());
}

/// **G1.9 bridge pin.** The two `Solution` flag rows come back from r4133 as
/// `0|1` *ints*, not booleans: `SolutionI(37)` is
/// `if ...SystemYChanged then Result:=1 else Result:=0`
/// (`DDLL/DSolution.pas:192-197`) and `SolutionI(42)` is
/// `Result:=0; ... if ...ControlActionsDone then Result := 1`
/// (`:226-230`).
///
/// `capture::capture_solution_scalars` converts both with `!= 0` so this
/// transport's `CaseResult` JSON stays shape-identical to
/// `oracle_server.capture_solution_scalars`, whose dss-python reads are already
/// Python `bool`s (decision D4: a sentinel/shape difference between the two
/// channels is a bridge normalization plus one pin, never a ledger row). This
/// is that pin: it fixes both the `{0, 1}` codomain the conversion relies on and
/// the values the vendored DLL actually returns on the solved fixture.
///
/// The two readings are the ones measured across the whole G1.9 sample
/// (14 decks x every step, snapshot and daily): `SystemYChanged` is `0` after
/// every solve — the solve rebuilds Y and clears the flag, including here where
/// the fixture adds an EnergyMeter between `Compile` and `Solve` — and
/// `ControlActionsDone` is `1`, the control loop having run to completion.
fn r4133_solution_flags_are_zero_one_ints(e: &Engine) {
    select_fixture(e);
    let syc = e.solution_system_y_changed().expect("SolutionI(37)");
    select_fixture(e);
    let cad = e.solution_control_actions_done().expect("SolutionI(42)");
    assert!(
        syc == 0 || syc == 1,
        "SystemYChanged must be the 0|1 int DSolution.pas:192-197 assigns, got {syc} — \
         the `!= 0` normalization in capture.rs would silently mis-read anything else"
    );
    assert!(
        cad == 0 || cad == 1,
        "ControlActionsDone must be the 0|1 int DSolution.pas:226-230 assigns, got {cad}"
    );
    assert_eq!(syc, 0, "a converged solve leaves SystemYChanged clear");
    assert_eq!(cad, 1, "a converged solve leaves ControlActionsDone set");
}

/// **G1.9 table pin.** The five `Circuit` aggregate rows are *not* pure reads,
/// and the table must say so: each one walks a `TPointerList` to exhaustion and
/// leaves its cursor at the end, and each one refreshes the `Iterminal` cache of
/// every element it walks (`Get_Losses`/`Get_Power` call `ComputeIterminal` —
/// `Common/CktElement.pas:743` and `:677-680`).
///
/// This is not cosmetic bookkeeping. `Circuit.Losses` moves exactly the
/// `PDElements` cursor that `Circuit.NextPDElement` resumes from
/// (`Common/Circuit.pas:2436-2443`), and `Circuit.SubstationLosses` moves the
/// `Transformers` cursor the discrete capture drives — so a caller that reads
/// one of these rows mid-walk and trusts a `Pure` label reads the wrong object.
/// [`select_fixture`] re-selects before every row precisely because of this
/// class of row.
///
/// Needs no DLL: it is a statement about [`modes`]' own table.
#[test]
fn the_five_circuit_aggregate_rows_are_impure() {
    let rows: [(&str, &ModeSpec); 5] = [
        ("PDElements", &modes::CIRCUIT_LOSSES),
        ("Lines", &modes::CIRCUIT_LINE_LOSSES),
        ("Transformers", &modes::CIRCUIT_SUBSTATION_LOSSES),
        ("Sources", &modes::CIRCUIT_TOTAL_POWER),
        ("CktElements", &modes::CIRCUIT_ALL_ELEMENT_LOSSES),
    ];
    for (list, row) in rows {
        let ModeEffect::Impure(why) = row.effect else {
            panic!(
                "{row} walks ActiveCircuit.{list}.First/Next to exhaustion and calls \
                 ComputeIterminal on what it walks (r4133 DDLL/DCircuit.pas), so it cannot be \
                 ModeEffect::Pure"
            );
        };
        assert!(
            why.contains(list),
            "{row}: the Impure payload must name the {list} list it moves, got: {why}"
        );
        assert!(
            why.contains("ComputeIterminal"),
            "{row}: the Impure payload must name the Iterminal refresh, got: {why}"
        );
    }
}

/// The behavioural half of GOLDEN_REBASE G1.6b's read-order contract: the
/// `ParentPDElement` trap is real on this DLL, and
/// [`dss_epri::capture::capture_pd_elements`] does not fall into it.
///
/// `PDElementsI(6)` (`DPDELements.pas:88-97`) does
/// `ActiveCktElement := ActivePDElement.ParentPDElement` and never restores it,
/// so every field read after it in the same record returns the **parent's**
/// value. fastdss reads it second (`IPDElements._columns`) and contaminates 215
/// of the 138-element IEEE123 walk's own cells (measured on both oracle
/// channels, 2026-09-04), which is why the capture reads it last.
///
/// Every number below was read off the vendored r4133 DLL on this fixture
/// (2026-09-04); the trap is only *observable* on a pair whose values differ, so
/// the `assert_ne!` guards this test against becoming a tautology.
fn the_parent_read_hijacks_the_active_element_and_the_capture_reads_it_last(e: &Engine) {
    select_fixture(e);
    // (1) The trap, live. `Line.632670` serves 10 downstream customers; its
    // parent `Line.650632` serves 15.
    e.set_active_element("Line.632670");
    let own = e.pd_elements_total_customers().unwrap();
    assert_eq!(own, 10, "Line.632670's own BranchTotalCustomers");
    assert_eq!(
        e.pd_elements_parent_pd_element().unwrap(),
        1,
        "Line.650632's ClassIndex (1-based, per-class creation order)"
    );
    let after = e.pd_elements_total_customers().unwrap();
    assert_eq!(
        after, 15,
        "the read AFTER the parent read returns the parent's value"
    );
    assert_ne!(
        after, own,
        "the parent and the child must differ, or this test proves nothing"
    );
    assert_eq!(
        e.pd_elements_name().unwrap(),
        "Line.650632",
        "the name read off the hijacked cursor is the parent's"
    );

    // (2) The capture is immune: it reads the parent last, so the record keeps
    // the element's own twelve values and carries the parent separately.
    let walk = dss_epri::capture::capture_pd_elements(e).expect("capture_pd_elements");
    assert_eq!(
        walk.len(),
        19,
        "IEEE13 + EnergyMeter.m1 has 19 enabled PD elements"
    );
    assert_eq!(
        walk[0].name, "Transformer.sub",
        "the walk is the circuit's PDElements pointer-list order"
    );
    let rec = walk
        .iter()
        .find(|r| r.name == "Line.632670")
        .expect("Line.632670 is in the walk");
    assert_eq!(
        rec.total_customers, own,
        "the capture kept the element's own value, not the parent's {after}"
    );
    assert_eq!(rec.num_customers, 3);
    assert_eq!(rec.from_terminal, 1, "FromTerminal is 1-based upstream");
    assert!(!rec.is_shunt);
    assert_eq!(rec.fault_rate, 0.1);
    assert_eq!(rec.pct_permanent, 20.0);
    assert_eq!(rec.repair_time, 3.0);
    assert_eq!(rec.parent_class_index, 1);
    assert_eq!(rec.parent_name, "Line.650632");
    // No `RelCalc` ran on this deck, so the four reliability-sweep fields are
    // untouched zeros (GOLDEN_REBASE G1.6(i) is the sub-step that fills them).
    assert_eq!(
        (
            rec.section_id,
            rec.lambda,
            rec.accumulated_l,
            rec.total_miles
        ),
        (0, 0.0, 0.0, 0.0)
    );

    // (3) A root branch has no parent: the DDLL leaves `ActiveCktElement` alone
    // there (`DPDELements.pas:92`), so the capture must skip the name read
    // rather than echo the element's own name.
    let root = walk
        .iter()
        .find(|r| r.name == "Line.650632")
        .expect("Line.650632 is in the walk");
    assert_eq!(root.total_customers, 15);
    assert_eq!(
        (root.parent_class_index, root.parent_name.as_str()),
        (0, "")
    );

    // (4) The shunt classification is a real two-valued field on this fixture.
    let shunts: Vec<&str> = walk
        .iter()
        .filter(|r| r.is_shunt)
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(shunts, vec!["Capacitor.cap1", "Capacitor.cap2"]);
}

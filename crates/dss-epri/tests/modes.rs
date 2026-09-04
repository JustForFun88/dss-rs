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
/// mode: three rows are [`ModeEffect::Impure`] and move a cursor
/// (`PDElements.ParentPDElement` moves `ActiveCktElement` itself), so without
/// this a later row would silently read a different object.
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
    assert_eq!(e.bus_distance().unwrap(), 1.2192, "bus 671's DistFromMeter");
    select_fixture(e);
    assert_eq!(e.solution_iterations().unwrap(), 2, "IEEE13 snapshot solve");
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

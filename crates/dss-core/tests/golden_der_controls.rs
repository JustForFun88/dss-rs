//! DER + smart-inverter-controls golden gate (PHASE7_PLAN.md WP7.3–WP7.5; part
//! of the former Phase-7 golden bucket): command-replay scenarios that pin
//! PVSystem (panel/inverter model, curves, clamps), Storage (SOC trajectories,
//! charge/discharge, clamps), StorageController dispatch, InvControl (volt-var,
//! volt-watt, watt-pf, watt-var, DRC, AVR and combinations, over PVSystem and
//! Storage, snapshot and daily/24h) and ExpControl against the pinned oracle,
//! from `tests/golden/der_controls/` (one `<scenario>.json` per scenario; the
//! gate runs every file in the dir).
//!
//! Pins: converged + iteration count + node order exact, node voltages 1e-6
//! rel, the scenario Line's YPrim entry-by-entry, every element's terminal
//! currents/powers, each Storage's post-solve SOC state (`? Storage.<name>.*`
//! readback), and the per-step monitor trajectory of every multi-step run.
//!
//! Regenerate only manually: `python tools/golden/gen_der_lines_harmonics.py`.

mod harness;

#[test]
fn der_controls_scenarios_match_oracle() {
    harness::scenario::check_family(
        "der_controls",
        &[
            "pvsystem_snapshot",
            "pvsystem_curves",
            "pvsystem_clamps",
            "storage_snapshot",
            "storage_clamps",
            "storage_daily",
            "storage_daily_charge",
            "storagecontroller_peakshave",
            "storagecontroller_daily",
            "invcontrol_voltvar",
            "invcontrol_voltvar_avg",
            "invcontrol_voltwatt",
            "invcontrol_voltwatt_adaptive",
            "invcontrol_voltwatt_daily",
            "invcontrol_vv_vw",
            "invcontrol_drc",
            "invcontrol_vv_drc",
            "invcontrol_wattpf",
            "invcontrol_wattvar",
            "invcontrol_wattvar_asym",
            "invcontrol_wattvar_qlim",
            "invcontrol_avr",
            "invcontrol_avr_daily",
            "invcontrol_avr_kvarlim",
            "invcontrol_avr_storage",
            "invcontrol_avr_storage_wattprio",
            "invcontrol_wattpf_storage",
            "invcontrol_wattvar_storage",
            "invcontrol_avr_24h",
            "invcontrol_wattpf_24h",
            "invcontrol_wattvar_24h",
            "invcontrol_avr_storage_24h",
            "invcontrol_wattpf_storage_24h",
            "invcontrol_wattvar_storage_24h",
            "invcontrol_voltvar_avg_24h",
            "invcontrol_voltvar_mixed_24h",
            "invcontrol_voltvar_lpf",
            "invcontrol_voltvar_risefall",
            "invcontrol_voltwatt_lpf",
            "invcontrol_voltwatt_risefall",
            "invcontrol_voltvar_monbus",
            "expcontrol_daily",
            "expcontrol_daily_preferq",
            "expcontrol_duty",
            "expcontrol_24h",
        ],
    );
}

//! Golden: the ported `support::flicker::flicker_meter` (Monitor mode-4
//! `DoFlickerCalculations` core) vs the **official EPRI r3723** engine on the
//! upstream `Examples/Matlab/pst.dss` flicker demo.
//!
//! Why r3723 and not the pinned oracle: the pinned dss_capi oracle
//! (`Monitor.pas:1655`) indexes `Terminals[MeteredTerminal]` 1-based on a 0-based
//! array, so any mode-4 `export monitor` is an unconditional access violation —
//! it cannot produce a flicker result at all. The official Delphi engine keeps
//! `Terminals` 1-based and runs the calc correctly; this golden captures it (the
//! D9 reference channel) via `tools/golden/gen_flicker.py`.
//!
//! The comparison is **solve-decoupled and f32-exact**: the golden stores r3723's
//! raw per-phase RMS magnitudes (the exact f32 inputs its own
//! `DoFlickerCalculations` fed into `FlickerMeter`), and the test feeds those same
//! magnitudes into the port and compares the flicker + Pst channels bit-for-bit.
//! No power-flow enters, so there is no solver floor: the port's
//! f64-arithmetic/f32-storage model is bit-exact vs Delphi Win64 (`Extended` ≡
//! `Double`). Regenerate only manually with the Oddie venv:
//! `tools/opendss/.venv/Scripts/python.exe tools/golden/gen_flicker.py`.

use std::path::PathBuf;

use dss_core::support::flicker::flicker_meter;
use serde::Deserialize;

#[derive(Deserialize)]
struct FlickerGolden {
    schema: u32,
    n: usize,
    nphases: usize,
    fbase: f64,
    kvbase: f64,
    times: Vec<f32>,
    raw_mag: Vec<Vec<f32>>,
    flk: Vec<Vec<f32>>,
    pst: Vec<Vec<f32>>,
}

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/flicker/pst_demo.json")
}

#[test]
fn flicker_meter_matches_r3723_bit_exact() {
    let txt = std::fs::read_to_string(golden_path()).expect("read pst_demo.json golden");
    let g: FlickerGolden = serde_json::from_str(&txt).expect("parse flicker golden");
    assert_eq!(g.schema, 1, "unexpected flicker golden schema");
    assert!(g.n >= 2 && g.nphases >= 1);
    assert_eq!(g.raw_mag.len(), g.nphases);

    // Pascal `Vbase: Single := 1000*kVBase` (narrowed to f32 before use).
    let vbase = ((1000.0 * g.kvbase) as f32) as f64;
    // Npst = 1 + Trunc(t_N / 600) (Monitor.pas:1651).
    let npst = 1 + (g.times[g.n - 1] / 600.0).trunc() as usize;

    for p in 0..g.nphases {
        let mut prms = g.raw_mag[p].clone();
        let mut ppst = vec![0.0_f32; npst];
        flicker_meter(g.fbase, vbase, &g.times, &mut prms, &mut ppst);

        // Flicker level channel (p_rms overwritten in place) — f32-exact.
        for (i, (&got, &want)) in prms.iter().zip(&g.flk[p]).enumerate() {
            assert_eq!(
                got.to_bits(),
                want.to_bits(),
                "flicker level phase {p} sample {i}: {got} vs r3723 {want}"
            );
        }

        // Pst channel: reconstruct the DoFlickerCalculations ipst/tpst stepping
        // (Monitor.pas:1662-1681) and compare f32-exact.
        let mut t_pst = 0.0_f32;
        let mut ipst = 0usize;
        for i in 0..g.n {
            if (g.times[i] - t_pst) >= 600.0 {
                ipst += 1;
                t_pst = g.times[i];
            }
            let got = if ipst >= 1 && ipst <= npst {
                ppst[ipst - 1]
            } else {
                0.0
            };
            assert_eq!(
                got.to_bits(),
                g.pst[p][i].to_bits(),
                "Pst phase {p} sample {i} (ipst {ipst}): {got} vs r3723 {}",
                g.pst[p][i]
            );
        }
    }
}

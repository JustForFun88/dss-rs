//! MULTITHREADING_PLAN M1 baseline / DE_PASCALIZE P15 proof: end-to-end
//! compile+solve of the vendored IEEE-8500 node feeder.
//!
//! Measures the full snapshot path — parse/build the model, assemble Y, LU
//! factor, and run the fixed-point loop to convergence — on the largest
//! standard test feeder. This is the headline "does the whole solve get
//! faster" number quoted by later stages; the Y-assembly and factor/solve
//! phases are isolated in `ybuild_8500` and `lu_factor_solve`.
//!
//! Run: `cargo bench --bench snapshot_8500`.

use std::path::PathBuf;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use dss_core::exec::Dss;

/// Absolute path to the vendored 8500-node `master.dss`.
fn master_8500() -> String {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "electricdss-tst",
        "Version8",
        "Distrib",
        "IEEETestCases",
        "8500-Node",
        "master.dss",
    ]
    .iter()
    .collect();
    assert!(
        path.is_file(),
        "8500-node master missing: {}",
        path.display()
    );
    path.to_string_lossy().replace('\\', "/")
}

/// Compile + solve the balanced 8500-node case once, from a fresh engine.
fn compile_and_solve(master: &str) {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile \"{master}\""));
    dss.command("New Energymeter.m1 Line.ln5815900-1 1");
    dss.command("Set Maxiterations=20");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "8500 snapshot errors: {:?}",
        dss.errors()
    );
    assert!(
        dss.circuit().expect("circuit").is_solved,
        "8500 snapshot did not converge"
    );
}

fn bench(c: &mut Criterion) {
    let master = master_8500();
    let mut group = c.benchmark_group("snapshot_8500");
    // 8500-node compile+solve is seconds-scale; keep the run bounded.
    group
        .sample_size(10)
        .measurement_time(Duration::from_secs(20));
    group.bench_function("compile_solve", |b| {
        b.iter(|| compile_and_solve(&master));
    });
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);

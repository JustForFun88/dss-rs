//! MULTITHREADING_PLAN M1 baseline / DE_PASCALIZE P15 proof: isolated
//! `build_y_matrix` on the IEEE-8500 node feeder.
//!
//! Setup compiles + solves the case once (so buses/nodes/YPrims exist), then
//! the measured routine forces a full whole-matrix Y rebuild via
//! [`Dss::rebuild_system_y`]. This is where P15 item 1 (reuse the `SparseSet`
//! across rebuilds, retain the symbolic factorization) and items 2-4 (dedup
//! mapping cache, direct column-major stamping, `build_scaled` reuse) show up:
//! the baseline re-allocates the sparse set and re-hashes the dedup map on
//! every rebuild, the reworked path re-accumulates into cached buffers.
//!
//! Run: `cargo bench --bench ybuild_8500`.

use std::path::PathBuf;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use dss_core::exec::Dss;

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

fn solved_8500(master: &str) -> Dss {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile \"{master}\""));
    dss.command("New Energymeter.m1 Line.ln5815900-1 1");
    dss.command("Set Maxiterations=20");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "8500 setup errors: {:?}",
        dss.errors()
    );
    dss
}

fn bench(c: &mut Criterion) {
    let master = master_8500();
    let mut dss = solved_8500(&master);
    let mut group = c.benchmark_group("ybuild_8500");
    group
        .sample_size(20)
        .measurement_time(Duration::from_secs(15));
    group.bench_function("rebuild_whole_y", |b| {
        b.iter(|| dss.rebuild_system_y());
    });
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);

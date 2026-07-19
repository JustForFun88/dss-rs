//! MULTITHREADING_PLAN M1 baseline / DE_PASCALIZE P15 proof: `dss-sparse` LU
//! factor + triangular solve at 8500-node scale.
//!
//! Setup compiles + solves the IEEE-8500 case and reads back the assembled,
//! unfactored system Y (`Dss::system_y_csc`). The measured routine drives one
//! `SparseSet` through the per-step cycle the engine performs when Y changes:
//! `zero()` -> restamp every nonzero -> `factor()` (assemble + row-equilibrate +
//! symbolic + numeric LU) -> triangular `solve()`. Reusing one `SparseSet`
//! across iterations is exactly what P15 item 1 targets: the reworked path
//! retains the symbolic factorization and the dedup skeleton across rebuilds,
//! the baseline re-analyzes and re-hashes every time.
//!
//! Run: `cargo bench --bench lu_factor_solve`.

use std::path::PathBuf;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use dss_core::exec::Dss;
use dss_sparse::SparseSet;
use num_complex::Complex64;

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

/// Compile + solve the 8500-node case and return `(n, [(row, col, value)])` of
/// its assembled system Y.
fn assembled_y() -> (usize, Vec<(usize, usize, Complex64)>) {
    let master = master_8500();
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
    dss.system_y_csc().expect("assembled system Y")
}

fn bench(c: &mut Criterion) {
    let (n, coords) = assembled_y();
    let b = vec![Complex64::new(1.0, 0.0); n];
    let mut x = vec![Complex64::ZERO; n];
    let mut s = SparseSet::new(n);

    let mut group = c.benchmark_group("lu_factor_solve");
    group
        .sample_size(20)
        .measurement_time(Duration::from_secs(15));
    group.bench_function("zero_restamp_factor_solve", |bench| {
        bench.iter(|| {
            s.zero();
            for &(r, cc, v) in &coords {
                s.add_element(r, cc, v);
            }
            s.solve(&b, &mut x).expect("solve");
        });
    });
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);

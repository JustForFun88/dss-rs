//! FPC-RTL float→ASCII battery gate (audit follow-up, WP8.5 step 3b).
//!
//! `tests/golden/fmt_battery.csv` holds 13 198 f64 bit patterns rendered by
//! the **real FPC 3.2.2 RTL** (the pinned oracle backend's compiler) through
//! every float→string entry point the port mirrors — see
//! `tools/fpc/fmt_battery/README.md`. Byte-equality on all renders: a
//! regression in the shared Grisu1 pipeline (`util::fmt_g` and friends)
//! silently corrupts every Show/Export/Dump/Save report, so it is pinned
//! directly, not only incidentally through the report goldens.

use dss_core::report::format::fpc_sci_w;
use dss_core::util::{float_to_str, fmt_g};

#[test]
fn fmt_battery_matches_fpc_rtl() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/golden/fmt_battery.csv"
    );
    let csv = std::fs::read_to_string(path).expect("read tests/golden/fmt_battery.csv");
    let mut rows = 0usize;
    let mut renders = 0usize;
    for line in csv.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split(';').collect();
        assert_eq!(fields.len(), 8, "malformed battery row: {line}");
        let bits = u64::from_str_radix(fields[0], 16).expect("hex bits");
        let v = f64::from_bits(bits);
        let got = [
            float_to_str(v),
            fmt_g(v, 2),
            fmt_g(v, 5),
            fmt_g(v, 8),
            fmt_g(v, 15),
            fpc_sci_w(v, 0),
            fpc_sci_w(v, 14),
        ];
        const COLS: [&str; 7] = [
            "FloatToStr",
            "fmt_g(2)",
            "fmt_g(5)",
            "fmt_g(8)",
            "fmt_g(15)",
            "fpc_sci_w(0)",
            "fpc_sci_w(14)",
        ];
        for (i, g) in got.iter().enumerate() {
            assert_eq!(
                g,
                fields[i + 1],
                "{} diverges from the FPC RTL for bits={} (v={v:e})",
                COLS[i],
                fields[0],
            );
            renders += 1;
        }
        rows += 1;
    }
    assert_eq!(
        rows, 13_198,
        "battery row count changed — regenerate deliberately"
    );
    assert_eq!(renders, rows * 7);
}

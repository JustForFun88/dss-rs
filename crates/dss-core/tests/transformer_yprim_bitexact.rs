//! Regression guard for the complex-division port bug (the parity kernel
//! `compat::cdiv`, formerly `cmatrix::cdiv_fpc`).
//!
//! The Gauss-Jordan inversion in `TcMatrix.Invert` divides by the pivot with
//! FPC `ucomplex`'s `/` operator, which is **Smith's overflow-safe abs-ratio**
//! algorithm — not the naive `(ac+bd)/(c²+d²)` form `num_complex`'s `/` uses.
//! The two round the last bit differently; the gap is invisible in robust
//! entries but shows as 1–3 ULP in the cancellation-sensitive (resistance) part
//! of an inverted impedance matrix. With the FPC-faithful division, the YYD
//! 3-winding transformer's `Yprim` is **bit-for-bit** identical to the pinned
//! oracle (dss-python-backend 0.14.5). These bits were captured from that oracle
//! via `ckt.ActiveCktElement.Yprim`; a re-introduced naive `/` flips them and is
//! within `corpus_live`'s tolerance, so only this exact-bit pin catches it.

use dss_core::exec::Dss;

/// Column-major (`out[col*6 + row]`) `(re.to_bits(), im.to_bits())` of
/// `Transformer.YYDA.Yprim`, straight from the pinned FPC oracle.
const EXPECTED: [(u64, u64); 36] = [
    // col 0
    (0x3f545edb5aaaaa1d, 0xbfa58087a1ad1663),
    (0xbf545edb5aaaaa1d, 0x3fa58087a1ad1663),
    (0xbf7e7f614a6d3b3b, 0x3fcc580d56962a16),
    (0x3f7e7f614a6d3b3b, 0xbfcc580d56962a16),
    (0x3f68a06a69dd7595, 0xbf78fcd1aea9cb6b),
    (0xbf68a06a69dd7595, 0x3f78fcd1aea9cb6b),
    // col 1
    (0xbf545edb5aaaaa1d, 0x3fa58087a1ad1663),
    (0x3f545edb5aaaaa1d, 0xbfa58087a1ad1663),
    (0x3f7e7f614a6d3b3b, 0xbfcc580d56962a16),
    (0xbf7e7f614a6d3b3b, 0x3fcc580d56962a16),
    (0xbf68a06a69dd7595, 0x3f78fcd1aea9cb6b),
    (0x3f68a06a69dd7595, 0xbf78fcd1aea9cb6b),
    // col 2
    (0xbf7e7f614a6d3b3b, 0x3fcc580d56962a16),
    (0x3f7e7f614a6d3b3b, 0xbfcc580d56962a16),
    (0x3fa8ab1d1967be72, 0xbff799d23516d07a),
    (0xbfa8ab1d1967be72, 0x3ff799d23516d07a),
    (0xbf9e1c7a31103e86, 0x3ff02240ce22ca12),
    (0x3f9e1c7a31103e86, 0xbff02240ce22ca12),
    // col 3
    (0x3f7e7f614a6d3b3b, 0xbfcc580d56962a16),
    (0xbf7e7f614a6d3b3b, 0x3fcc580d56962a16),
    (0xbfa8ab1d1967be72, 0x3ff799d23516d07a),
    (0x3fa8ab1d1967be72, 0xbff799d23516d07a),
    (0x3f9e1c7a31103e86, 0xbff02240ce22ca12),
    (0xbf9e1c7a31103e86, 0x3ff02240ce22ca12),
    // col 4
    (0x3f68a06a69dd7595, 0xbf78fcd1aea9cb6b),
    (0xbf68a06a69dd7595, 0x3f78fcd1aea9cb6b),
    (0xbf9e1c7a31103e86, 0x3ff02240ce22ca12),
    (0x3f9e1c7a31103e86, 0xbff02240ce22ca12),
    (0x3fa64204ab6a03c2, 0xc008cd6e9b508db7),
    (0xbfa64204ab6a03c2, 0x4008cd6e9b508db7),
    // col 5
    (0xbf68a06a69dd7595, 0x3f78fcd1aea9cb6b),
    (0x3f68a06a69dd7595, 0xbf78fcd1aea9cb6b),
    (0x3f9e1c7a31103e86, 0xbff02240ce22ca12),
    (0xbf9e1c7a31103e86, 0x3ff02240ce22ca12),
    (0xbfa64204ab6a03c2, 0x4008cd6e9b508db7),
    (0x3fa64204ab6a03c2, 0xc008cd6e9b508db7),
];

#[test]
fn yyda_yprim_is_bit_identical_to_fpc_oracle() {
    let deck = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/corpus/electricdss-tst/Version8/Distrib/IEEETestCases/4Bus-YYD/YYD-Master-step1.DSS"
    );
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile \"{deck}\""));
    assert!(
        dss.errors().is_empty(),
        "compile errors: {:?}",
        dss.errors()
    );

    let (yorder, flat) = dss
        .element_yprim("Transformer.YYDA")
        .expect("transformer YYDA has a Yprim");
    assert_eq!(yorder, 6, "YYDA Yprim order");
    assert_eq!(flat.len(), 36);

    for (k, (v, &(re_bits, im_bits))) in flat.iter().zip(EXPECTED.iter()).enumerate() {
        let (row, col) = (k % 6, k / 6);
        assert_eq!(
            v.re.to_bits(),
            re_bits,
            "Yprim[{row},{col}] re bits differ: {:016x} vs oracle {re_bits:016x} \
             (re-introduced naive complex division in CMatrix?)",
            v.re.to_bits()
        );
        assert_eq!(
            v.im.to_bits(),
            im_bits,
            "Yprim[{row},{col}] im bits differ: {:016x} vs oracle {im_bits:016x}",
            v.im.to_bits()
        );
    }
}

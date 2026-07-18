//! Per-class `SpecSets` / `SpecSetNames` — a schema-side port of the
//! `TDSSClass.SpecSets` / `SpecSetNames` fields (`DSSClass.pas`), the data the
//! class walk (`prepareClassJsonSchema`, `CAPI_Schema.pas:1030-1104`) turns into
//! each class def's `oneOf` block.
//!
//! A spec set names an alternative way to specify a class (e.g. a LineCode by
//! symmetrical components *or* by matrices). Each set lists the member property
//! **names** (resolved to indices by the walker); which members are *required*
//! within the set is read off the property's
//! [`REQUIRED_IN_SPEC_SET`](crate::obj::props::PropFlags::REQUIRED_IN_SPEC_SET)
//! flag (Pascal `PropertyFlags[propIndex]`), exactly as the walker does — the
//! set membership lives here, the per-property required bit stays on the
//! `PropDef` where the existing convention (GrowthShape) already carries it.
//!
//! Sourced from the vendored Pascal `DefineProperties` `SpecSetNames`/`SpecSets`
//! (never from the oracle JSON). The names are exactly the Pascal
//! `PropertyName[..]` forms; the walker maps them to the port's ordinals.

/// One spec set: its title (`SpecSetNames[j]`) and its member property names
/// (`SpecSets[j]`, in order).
pub(super) struct SpecSet {
    pub(super) name: &'static str,
    pub(super) props: &'static [&'static str],
}

/// The spec sets for `class_name` (case-insensitive), in `SpecSets` order, or an
/// empty slice for a class with none. Extended one class-batch at a time.
pub(super) fn spec_sets(class_name: &str) -> &'static [SpecSet] {
    match class_name {
        "LineCode" => &[
            SpecSet {
                name: "Z0, Z1, C0, C1",
                props: &["R1", "X1", "R0", "X0", "C1", "C0"],
            },
            // Aborted by the walker: `B1`/`B0` are Redundant, so their `prop_json`
            // is never built (matches the oracle, which emits only two `oneOf`).
            SpecSet {
                name: "Z0, Z1, B0, B1",
                props: &["R1", "X1", "R0", "X0", "B1", "B0"],
            },
            SpecSet {
                name: "ZMatrix, CMatrix",
                props: &["RMatrix", "XMatrix", "CMatrix"],
            },
        ],
        "LoadShape" => &[
            SpecSet {
                name: "PMult, QMult, Hour",
                props: &["PMult", "QMult", "Hour"],
            },
            SpecSet {
                name: "PMult, QMult, Interval",
                props: &["PMult", "QMult", "Interval"],
            },
            SpecSet {
                name: "PQCSVFile",
                props: &["PQCSVFile"],
            },
            SpecSet {
                name: "CSVFile",
                props: &["CSVFile"],
            },
            SpecSet {
                name: "SngFile",
                props: &["SngFile"],
            },
            SpecSet {
                name: "DblFile",
                props: &["DblFile"],
            },
        ],
        "TShape" => &[
            SpecSet {
                name: "Temp, Hour",
                props: &["Temp", "Hour"],
            },
            SpecSet {
                name: "Temp, Interval",
                props: &["Temp", "Interval"],
            },
            SpecSet {
                name: "CSVFile",
                props: &["CSVFile"],
            },
            SpecSet {
                name: "SngFile",
                props: &["SngFile"],
            },
            SpecSet {
                name: "DblFile",
                props: &["DblFile"],
            },
        ],
        "PriceShape" => &[
            SpecSet {
                name: "Price, Hour",
                props: &["Price", "Hour"],
            },
            SpecSet {
                name: "Price, Interval",
                props: &["Price", "Interval"],
            },
            SpecSet {
                name: "CSVFile",
                props: &["CSVFile"],
            },
            SpecSet {
                name: "SngFile",
                props: &["SngFile"],
            },
            SpecSet {
                name: "DblFile",
                props: &["DblFile"],
            },
        ],
        "XYcurve" => &[
            SpecSet {
                name: "Xarray, Yarray",
                props: &["XArray", "YArray"],
            },
            SpecSet {
                name: "CSVFile",
                props: &["CSVFile"],
            },
            SpecSet {
                name: "SngFile",
                props: &["SngFile"],
            },
            SpecSet {
                name: "DblFile",
                props: &["DblFile"],
            },
        ],
        // `Spectrum.pas:144-150` — `SpecSetNames`/`SpecSets`. Member names are
        // the Pascal `PropertyName` forms (`%Mag` resolves to the `pctMag` prop;
        // the walker emits its `json_key` = `pctMag`).
        "Spectrum" => &[
            SpecSet {
                name: "Harmonic, Angle, pctMag",
                props: &["Harmonic", "Angle", "%Mag"],
            },
            SpecSet {
                name: "CSVFile",
                props: &["CSVFile"],
            },
        ],
        "GrowthShape" => &[
            SpecSet {
                name: "Year, Mult",
                props: &["Year", "Mult"],
            },
            SpecSet {
                name: "CSVFile",
                props: &["CSVFile"],
            },
            SpecSet {
                name: "SngFile",
                props: &["SngFile"],
            },
            SpecSet {
                name: "DblFile",
                props: &["DblFile"],
            },
        ],
        _ => &[],
    }
}

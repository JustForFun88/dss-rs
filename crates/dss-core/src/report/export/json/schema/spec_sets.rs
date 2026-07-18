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
        // `Load.pas:368-381` — the load specification sets.
        "Load" => &[
            SpecSet {
                name: "kW, PF",
                props: &["kW", "PF"],
            },
            SpecSet {
                name: "kW, kvar",
                props: &["kW", "kvar"],
            },
            SpecSet {
                name: "kVA, PF",
                props: &["kVA", "PF"],
            },
            SpecSet {
                name: "xfkVA, AllocationFactor, PF",
                props: &["XfkVA", "AllocationFactor", "PF"],
            },
            SpecSet {
                name: "kWh, kWhDays, CFactor, PF",
                props: &["kWh", "PF", "kWhDays", "CFactor"],
            },
        ],
        // `Vsource.pas:228-241` — the source-impedance specification sets. The
        // 5th set (`R0, X0, R1, X1`) aborts in the walker: those props are
        // `Redundant`, so their `prop_json` is never built (matches the oracle,
        // which emits only four `oneOf`).
        "Vsource" => &[
            SpecSet {
                name: "MVAsc3, MVAsc1, x1r1, x0r0",
                props: &["MVASC3", "MVASC1", "X1R1", "X0R0"],
            },
            SpecSet {
                name: "Isc3, Isc1, x1r1, x0r0",
                props: &["Isc3", "Isc1", "X1R1", "X0R0"],
            },
            SpecSet {
                name: "BaseMVA, puZ0, puZ1, puZ2",
                props: &["BaseMVA", "puZ0", "puZ1", "puZ2"],
            },
            SpecSet {
                name: "Z0, Z1, Z2",
                props: &["Z0", "Z1", "Z2"],
            },
            SpecSet {
                name: "R0, X0, R1, X1",
                props: &["R0", "X0", "R1", "X1"],
            },
        ],
        // `Capacitor.pas:208-217` — `SpecSetNames`/`SpecSets`. The `Cuf` member is
        // the `Cuf` prop (Pascal `cuf`).
        "Capacitor" => &[
            SpecSet {
                name: "kvar, kV",
                props: &["kvar", "kV"],
            },
            SpecSet {
                name: "cmatrix",
                props: &["CMatrix"],
            },
            SpecSet {
                name: "cuf, kV",
                props: &["Cuf", "kV"],
            },
        ],
        // `Reactor.pas:202-222` — note the **7 names vs 6 sets** offset: the
        // `R, X, ...` and `R, LmH, ...` sets abort (their `R` member is `Redundant`,
        // so its `prop_json` is never built), which shifts the surviving sets onto
        // the *next* names — the last-two emitted sets carry the mismatched titles
        // `R, LmH, Rcurve, Lcurve` (over Z0/Z1/Z2) and `Z0, Z1, Z2` (over
        // RMatrix/XMatrix), exactly as the oracle shows. The `Rmatrix, Xmatrix`
        // name (index 6) has no set and is dropped. Reproduced 1:1 by pairing each
        // set with `SpecSetNames[j]`.
        "Reactor" => &[
            SpecSet {
                name: "kV, kvar, Rcurve, Lcurve",
                props: &["kV", "kvar", "RCurve", "LCurve"],
            },
            SpecSet {
                name: "Z, Rcurve, Lcurve",
                props: &["Z", "RCurve", "LCurve"],
            },
            SpecSet {
                name: "R, Rcurve, Lcurve",
                props: &["R", "X", "RCurve", "LCurve"],
            },
            SpecSet {
                name: "R, X, Rcurve, Lcurve",
                props: &["R", "LmH", "RCurve", "LCurve"],
            },
            SpecSet {
                name: "R, LmH, Rcurve, Lcurve",
                props: &["Z0", "Z1", "Z2"],
            },
            SpecSet {
                name: "Z0, Z1, Z2",
                props: &["RMatrix", "XMatrix"],
            },
        ],
        // `Fault.pas:163-171` — `SpecSetNames`/`SpecSets`.
        "Fault" => &[
            SpecSet {
                name: "r",
                props: &["R"],
            },
            SpecSet {
                name: "Gmatrix",
                props: &["GMatrix"],
            },
        ],
        // `Transformer.pas:379-388` — `SpecSetNames`/`SpecSets`. The `kV` member
        // redirects (array_alternative) to `kVs`; the walker keeps the scalar name
        // as the oneOf key.
        "Transformer" => &[
            SpecSet {
                name: "XfmrCode",
                props: &["XfmrCode"],
            },
            SpecSet {
                name: "X12, X13, X23, kV",
                props: &["X12", "X13", "X23", "kV"],
            },
            SpecSet {
                name: "XscArray, kV",
                props: &["XSCArray", "kV"],
            },
        ],
        // `Storage.pas:546-553` — `SpecSetNames`/`SpecSets`.
        "Storage" => &[
            SpecSet {
                name: "kWRated, PF",
                props: &["kWRated", "PF"],
            },
            SpecSet {
                name: "kWRated, kvar",
                props: &["kWRated", "kvar"],
            },
        ],
        // `Generator.pas:504-511` — `SpecSetNames`/`SpecSets`.
        "Generator" => &[
            SpecSet {
                name: "kW, pf",
                props: &["kW", "PF"],
            },
            SpecSet {
                name: "kW, kvar",
                props: &["kW", "kvar"],
            },
        ],
        // Pascal `PVsystem.pas:432-439` `SpecSetNames`/`SpecSets`: PF vs kvar
        // control (each its own single-member set, required within the set).
        "PVSystem" => &[
            SpecSet {
                name: "PF",
                props: &["PF"],
            },
            SpecSet {
                name: "kvar",
                props: &["kvar"],
            },
        ],
        _ => &[],
    }
}

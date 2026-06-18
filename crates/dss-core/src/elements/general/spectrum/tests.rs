use super::*;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::PropEngine;
use dss_parser::{Parser, ParserVars};

fn edit_and_dump(edits: &[(&str, &str)]) -> Vec<(String, String)> {
    let cls = class_props(&EnumRegistry::new());
    let mut obj = SpectrumObj::new("s");
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let enums = EnumRegistry::new();
    let mut errors = Vec::new();
    for (name, value) in edits {
        let idx = cls.property_index(name).expect("known property");
        let mut eng = PropEngine {
            parser: &mut parser,
            vars: &vars,
            enums: &enums,
            errors: &mut errors,
            foreign: None,
        };
        cls.edit_property(&mut obj, idx, value, &mut eng).unwrap();
    }
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    (1..=cls.num_properties())
        .map(|i| {
            (
                cls.property_name(i).to_string(),
                cls.get_value(&obj, i, &enums),
            )
        })
        .collect()
}

#[test]
fn defaults_match_oracle() {
    // Oracle: NumHarm='0', Harmonic='', %Mag='', Angle='', CSVFile='', Like=''
    let dump = edit_and_dump(&[]);
    let expected = [
        ("NumHarm", "0"),
        ("Harmonic", ""),
        ("%Mag", ""),
        ("Angle", ""),
        ("CSVFile", ""),
        ("Like", ""),
    ];
    for (got, (name, val)) in dump.iter().zip(expected) {
        assert_eq!(got.0, name);
        assert_eq!(got.1, val);
    }
}

#[test]
fn pct_mag_scale_round_trips() {
    // %Mag stored per-unit (×0.01), displayed as percent (÷0.01).
    let dump = edit_and_dump(&[
        ("NumHarm", "3"),
        ("harmonic", "1 5 7"),
        ("%mag", "100 20 50"),
        ("angle", "0 0 0"),
    ]);
    assert_eq!(dump[2], ("%Mag".into(), "[ 100 20 50]".into()));
    assert_eq!(dump[1], ("Harmonic".into(), "[ 1 5 7]".into()));
}

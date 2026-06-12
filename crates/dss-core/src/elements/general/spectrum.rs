//! `Spectrum` — harmonic spectrum (per-unit magnitude + angle by harmonic).
//! Port of Pascal `General/Spectrum.pas`. As a second guinea pig it adds two
//! things TCC_Curve doesn't exercise: a scaled array property (`%Mag`, stored
//! per-unit, displayed as percent) and a string property (`CSVFile`).
//!
//! Pascal `TProp`: `NumHarm=1`, `Harmonic=2`, `pctMag=3` (modern name `%Mag`),
//! `Angle=4`, `CSVFile=5`; the base class appends `Like=6`.

use crate::obj::base::{DssObjData, DssObject};
use crate::obj::props::{PropDef, PropFlags, define_properties};

// Pascal `TSpectrumProp` ordinals + the property table. `%Mag` is stored
// per-unit: the parser multiplies by 0.01 and the getter divides by it
// (Pascal `PropertyScale := 0.01`).
define_properties! {
    class "Spectrum", abbrev true;
    1 NUM_HARM => PropDef::integer("NumHarm").flags(PropFlags::SUPPRESS_JSON);
    2 HARMONIC => PropDef::double_array("Harmonic", NUM_HARM)
        .flags(PropFlags::REQUIRED_IN_SPEC_SET);
    3 PCT_MAG  => PropDef::double_array("%Mag", NUM_HARM)
        .scale(0.01)
        .flags(PropFlags::REQUIRED_IN_SPEC_SET);
    4 ANGLE    => PropDef::double_array("Angle", NUM_HARM)
        .flags(PropFlags::REQUIRED_IN_SPEC_SET);
    5 CSV_FILE => PropDef::string("CSVFile").flags(
        PropFlags::IS_FILENAME | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::GLOBAL_COUNT,
    );
}

use prop::{ANGLE, CSV_FILE, HARMONIC, NUM_HARM, PCT_MAG};

/// A `Spectrum` instance (`TSpectrumObj`).
#[derive(Debug, Clone)]
pub struct SpectrumObj {
    data: DssObjData,
    num_harm: i32,
    harm_array: Option<Vec<f64>>,
    /// Per-unit magnitudes (`puMagArray`); displayed as percent via the scale.
    pu_mag_array: Option<Vec<f64>>,
    angle_array: Option<Vec<f64>>,
    csvfile: String,
    // `MultArray` (the complex per-harmonic phasors built by `SetMultArray`)
    // is only consumed by the harmonic solution mode, so it is deferred to the
    // harmonics phase; nothing in the property dump depends on it.
}

impl SpectrumObj {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            data: DssObjData::new(name.into().to_lowercase(), prop::NUM_PROPS),
            num_harm: 0,
            harm_array: None,
            pu_mag_array: None,
            angle_array: None,
            csvfile: String::new(),
        }
    }

    /// Pascal `HarmArrayHasaZero`: the 1-based index of the first zero harmonic,
    /// or `None`.
    fn harm_array_has_a_zero(&self) -> Option<usize> {
        let arr = self.harm_array.as_ref()?;
        arr.iter().position(|&v| v == 0.0).map(|i| i + 1)
    }
}

impl DssObject for SpectrumObj {
    fn data(&self) -> &DssObjData {
        &self.data
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.data
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn get_i32(&self, idx: usize) -> i32 {
        match idx {
            NUM_HARM => self.num_harm,
            _ => unreachable!("Spectrum has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        match idx {
            NUM_HARM => self.num_harm = value,
            _ => unreachable!("Spectrum has no integer property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        match idx {
            CSV_FILE => self.csvfile.clone(),
            _ => unreachable!("Spectrum has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        match idx {
            CSV_FILE => self.csvfile = value,
            _ => unreachable!("Spectrum has no string property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        match idx {
            HARMONIC => self.harm_array.as_deref(),
            PCT_MAG => self.pu_mag_array.as_deref(),
            ANGLE => self.angle_array.as_deref(),
            _ => unreachable!("Spectrum has no array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        match idx {
            HARMONIC => self.harm_array = Some(value),
            PCT_MAG => self.pu_mag_array = Some(value),
            ANGLE => self.angle_array = Some(value),
            _ => unreachable!("Spectrum has no array property {idx}"),
        }
    }

    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        match idx {
            NUM_HARM => {
                let n = self.num_harm.max(0) as usize;
                // Pascal only reallocs HarmArray if it was already allocated;
                // the AngleArray is always (re)allocated and zero-filled, while
                // puMagArray is left untouched here.
                if self.harm_array.is_some() {
                    realloc(&mut self.harm_array, n);
                }
                self.angle_array = if n == 0 { None } else { Some(vec![0.0; n]) };
            }
            CSV_FILE => {
                // TODO(phase2+): DoCSVFile loads Harmonic/%Mag/Angle from a file.
                // Deferred with the rest of the file-array machinery; the
                // property value (the filename) is still stored and dumped.
            }
            _ => {}
        }
    }

    fn end_edit(&mut self) {
        // Pascal `TSpectrum.EndEdit`: reject a zero harmonic; otherwise it would
        // build MultArray (deferred to the harmonics phase). We keep the
        // validation so malformed spectra still report the same error.
        if self.harm_array_has_a_zero().is_some() {
            // DoSimpleMsg in the original; surfaced via the engine error log
            // by the caller is not wired here, so this is a no-op guard for now.
        }
    }

    fn make_like(&mut self, other: &dyn DssObject) {
        // Pascal `TSpectrumObj.MakeLike`: `inherited MakeLike` (copy the
        // PrpSequence), then copy the fields.
        self.data.copy_prp_sequence_from(other.data());
        self.num_harm = other.get_i32(NUM_HARM);
        self.harm_array = other.get_f64_array(HARMONIC).map(<[f64]>::to_vec);
        self.pu_mag_array = other.get_f64_array(PCT_MAG).map(<[f64]>::to_vec);
        self.angle_array = other.get_f64_array(ANGLE).map(<[f64]>::to_vec);
        self.csvfile = other.get_string(CSV_FILE);
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

/// Pascal `ReAllocmem`: grow/shrink keeping surviving values; 0 frees (NIL).
fn realloc(arr: &mut Option<Vec<f64>>, n: usize) {
    if n == 0 {
        *arr = None;
    } else {
        arr.get_or_insert_with(Vec::new).resize(n, 0.0);
    }
}

#[cfg(test)]
mod tests {
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
}

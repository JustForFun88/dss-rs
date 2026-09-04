//! Generic FFI **family registry** — the capability-parity surface (EPRI
//! capability Round 2). The DDLL exposes every element/class/solution/settings
//! interface as a uniform quartet of `cdecl` entry points
//! `XxxI/XxxF/XxxS/XxxV(mode, arg)` (the four ABI shapes of [`crate::ffi`]);
//! dss-python is a generated wrapper over exactly these calls. Binding the whole
//! quartet table and dispatching `(family, kind, mode, arg)` generically therefore
//! reaches **every mode of every family** — capability-complete against the C-API,
//! without hand-enumerating the thousands of individual modes. The gate's typed
//! accessors in [`crate::dss`] stay the byte-frozen capture path; this table is
//! the additive *capability* channel driven by the worker `ffi` command.
//!
//! The registry lists each family's actual DLL export symbols (case-sensitive
//! `GetProcAddress`): the export names are not always `NameI` (`Bus` is `BUSI`,
//! `Loads` is `DSSLoads`, `DSSProperties` is a bare `S`), so they are spelled
//! out. `None` marks an ABI shape a family lacks (e.g. `Monitors` has no `F`).
//!
//! # SAFETY (module-wide invariant)
//! Every function pointer here is loaded from the same leaked [`libloading`]
//! `Library` that backs [`crate::ffi::DllFns`]; the module SAFETY invariant of
//! [`crate::ffi`] (the DLL stays mapped for the whole process, single-threaded,
//! V-buffers copied out immediately) applies unchanged. Each listed symbol's ABI
//! is one of the four transcribed `Fn*` shapes, verified against the r4133 export
//! table (the vendored `OpenDSSDirect.dll` dump) and the DDLL `interface` decls.

use libloading::os::windows::Library;

use crate::ffi::{FnF, FnF2, FnI, FnS, FnV, sym};

/// One family's four possible entry points. Any may be `None` (the family lacks
/// that ABI shape). `Copy`/`Send`: bare `fn` pointers borrow nothing.
///
/// The `F` shape comes in two ABIs: the uniform one-double [`FnF`] (`f`) and the
/// two-double [`FnF2`] (`f2`) that exactly the [`TWO_DOUBLE_F`] families declare.
/// A family carries **at most one** of them — enforced at load.
#[derive(Clone, Copy, Default)]
pub struct Family {
    pub i: Option<FnI>,
    pub f: Option<FnF>,
    /// The two-double `F` entry point ([`FnF2`]); `Some` only for the
    /// [`TWO_DOUBLE_F`] families, and then `f` is `None`.
    pub f2: Option<FnF2>,
    pub s: Option<FnS>,
    pub v: Option<FnV>,
}

impl Family {
    /// `true` when the family has an `F` entry point of **either** ABI.
    pub fn has_f(&self) -> bool {
        self.f.is_some() || self.f2.is_some()
    }
}

/// The families whose `F` export takes **two** doubles rather than one:
/// `CircuitF(mode; arg1, arg2: double)` (`DCircuit.pas:27`, impl `:193`) and
/// `CmathLibF(mode; arg1, arg2: double)` (`DCmathLib.pas:5`, impl `:12`). An
/// exhaustive sweep of the vendored DDLL `interface` sections (G1.0, 2026-09-04)
/// found no third one, and every `XxxI`/`XxxS`/`XxxV` uniform. [`FamilyTable::load`]
/// binds these two through [`FnF2`] and leaves [`Family::f`] `None`; the
/// mismatch was live-measured before the fix (see [`FnF2`]).
pub const TWO_DOUBLE_F: [&str; 2] = ["Circuit", "CmathLib"];

/// A `\0`-terminated export symbol name, or `None` if the family lacks the shape.
type Sym = Option<&'static [u8]>;

/// The full uniform-family registry: `(family, I, F, S, V)` DLL export symbols.
/// 42 families / 147 entry points — the entire uniform DDLL surface (everything
/// the Oddie/dss-python bridge could reach except the standalone `DSSPut_Command`
/// / `Error*` / Y-matrix helpers, which [`crate::ffi`]/[`crate::dss`] bind typed).
const REGISTRY: &[(&str, Sym, Sym, Sym, Sym)] = &[
    (
        "ActiveClass",
        Some(b"ActiveClassI\0"),
        None,
        Some(b"ActiveClassS\0"),
        Some(b"ActiveClassV\0"),
    ),
    (
        "Bus",
        Some(b"BUSI\0"),
        Some(b"BUSF\0"),
        Some(b"BUSS\0"),
        Some(b"BUSV\0"),
    ),
    (
        "CapControls",
        Some(b"CapControlsI\0"),
        Some(b"CapControlsF\0"),
        Some(b"CapControlsS\0"),
        Some(b"CapControlsV\0"),
    ),
    (
        "Capacitors",
        Some(b"CapacitorsI\0"),
        Some(b"CapacitorsF\0"),
        Some(b"CapacitorsS\0"),
        Some(b"CapacitorsV\0"),
    ),
    (
        "Circuit",
        Some(b"CircuitI\0"),
        Some(b"CircuitF\0"),
        Some(b"CircuitS\0"),
        Some(b"CircuitV\0"),
    ),
    (
        "CktElement",
        Some(b"CktElementI\0"),
        Some(b"CktElementF\0"),
        Some(b"CktElementS\0"),
        Some(b"CktElementV\0"),
    ),
    (
        "CmathLib",
        None,
        Some(b"CmathLibF\0"),
        None,
        Some(b"CmathLibV\0"),
    ),
    (
        "CtrlQueue",
        Some(b"CtrlQueueI\0"),
        None,
        None,
        Some(b"CtrlQueueV\0"),
    ),
    (
        "DSS",
        Some(b"DSSI\0"),
        None,
        Some(b"DSSS\0"),
        Some(b"DSSV\0"),
    ),
    (
        "DSSElement",
        Some(b"DSSElementI\0"),
        None,
        Some(b"DSSElementS\0"),
        Some(b"DSSElementV\0"),
    ),
    (
        "DSSExecutive",
        Some(b"DSSExecutiveI\0"),
        None,
        Some(b"DSSExecutiveS\0"),
        None,
    ),
    // GUI progress form — headless no-op (skip-by-design for assertions), but
    // still bound so the table literally covers everything the bridge could call.
    (
        "DSSProgress",
        Some(b"DSSProgressI\0"),
        None,
        Some(b"DSSProgressS\0"),
        None,
    ),
    ("DSSProperties", None, None, Some(b"DSSProperties\0"), None),
    (
        "Fuses",
        Some(b"FusesI\0"),
        Some(b"FusesF\0"),
        Some(b"FusesS\0"),
        Some(b"FusesV\0"),
    ),
    (
        "GICSources",
        Some(b"GICSourcesI\0"),
        Some(b"GICSourcesF\0"),
        Some(b"GICSourcesS\0"),
        Some(b"GICSourcesV\0"),
    ),
    (
        "Generators",
        Some(b"GeneratorsI\0"),
        Some(b"GeneratorsF\0"),
        Some(b"GeneratorsS\0"),
        Some(b"GeneratorsV\0"),
    ),
    (
        "Isource",
        Some(b"IsourceI\0"),
        Some(b"IsourceF\0"),
        Some(b"IsourceS\0"),
        Some(b"IsourceV\0"),
    ),
    (
        "LineCodes",
        Some(b"LineCodesI\0"),
        Some(b"LineCodesF\0"),
        Some(b"LineCodesS\0"),
        Some(b"LineCodesV\0"),
    ),
    (
        "Lines",
        Some(b"LinesI\0"),
        Some(b"LinesF\0"),
        Some(b"LinesS\0"),
        Some(b"LinesV\0"),
    ),
    // The Loads interface's integer entry is the oddly-named `DSSLoads`.
    (
        "Loads",
        Some(b"DSSLoads\0"),
        Some(b"DSSLoadsF\0"),
        Some(b"DSSLoadsS\0"),
        Some(b"DSSLoadsV\0"),
    ),
    (
        "LoadShape",
        Some(b"LoadShapeI\0"),
        Some(b"LoadShapeF\0"),
        Some(b"LoadShapeS\0"),
        Some(b"LoadShapeV\0"),
    ),
    (
        "Meters",
        Some(b"MetersI\0"),
        Some(b"MetersF\0"),
        Some(b"MetersS\0"),
        Some(b"MetersV\0"),
    ),
    (
        "Monitors",
        Some(b"MonitorsI\0"),
        None,
        Some(b"MonitorsS\0"),
        Some(b"MonitorsV\0"),
    ),
    (
        "PDElements",
        Some(b"PDElementsI\0"),
        Some(b"PDElementsF\0"),
        Some(b"PDElementsS\0"),
        None,
    ),
    (
        "PVsystems",
        Some(b"PVsystemsI\0"),
        Some(b"PVsystemsF\0"),
        Some(b"PVsystemsS\0"),
        Some(b"PVsystemsV\0"),
    ),
    (
        "Parallel",
        Some(b"ParallelI\0"),
        None,
        None,
        Some(b"ParallelV\0"),
    ),
    (
        "Parser",
        Some(b"ParserI\0"),
        Some(b"ParserF\0"),
        Some(b"ParserS\0"),
        Some(b"ParserV\0"),
    ),
    (
        "Reactors",
        Some(b"ReactorsI\0"),
        Some(b"ReactorsF\0"),
        Some(b"ReactorsS\0"),
        Some(b"ReactorsV\0"),
    ),
    (
        "Reclosers",
        Some(b"ReclosersI\0"),
        Some(b"ReclosersF\0"),
        Some(b"ReclosersS\0"),
        Some(b"ReclosersV\0"),
    ),
    (
        "ReduceCkt",
        Some(b"ReduceCktI\0"),
        Some(b"ReduceCktF\0"),
        Some(b"ReduceCktS\0"),
        None,
    ),
    (
        "RegControls",
        Some(b"RegControlsI\0"),
        Some(b"RegControlsF\0"),
        Some(b"RegControlsS\0"),
        Some(b"RegControlsV\0"),
    ),
    (
        "Relays",
        Some(b"RelaysI\0"),
        None,
        Some(b"RelaysS\0"),
        Some(b"RelaysV\0"),
    ),
    (
        "Sensors",
        Some(b"SensorsI\0"),
        Some(b"SensorsF\0"),
        Some(b"SensorsS\0"),
        Some(b"SensorsV\0"),
    ),
    (
        "Settings",
        Some(b"SettingsI\0"),
        Some(b"SettingsF\0"),
        Some(b"SettingsS\0"),
        Some(b"SettingsV\0"),
    ),
    (
        "Solution",
        Some(b"SolutionI\0"),
        Some(b"SolutionF\0"),
        Some(b"SolutionS\0"),
        Some(b"SolutionV\0"),
    ),
    (
        "Storages",
        Some(b"StoragesI\0"),
        Some(b"StoragesF\0"),
        Some(b"StoragesS\0"),
        Some(b"StoragesV\0"),
    ),
    (
        "SwtControls",
        Some(b"SwtControlsI\0"),
        Some(b"SwtControlsF\0"),
        Some(b"SwtControlsS\0"),
        Some(b"SwtControlsV\0"),
    ),
    (
        "Topology",
        Some(b"TopologyI\0"),
        None,
        Some(b"TopologyS\0"),
        Some(b"TopologyV\0"),
    ),
    (
        "Transformers",
        Some(b"TransformersI\0"),
        Some(b"TransformersF\0"),
        Some(b"TransformersS\0"),
        Some(b"TransformersV\0"),
    ),
    (
        "Vsources",
        Some(b"VsourcesI\0"),
        Some(b"VsourcesF\0"),
        Some(b"VsourcesS\0"),
        Some(b"VsourcesV\0"),
    ),
    (
        "WindGens",
        Some(b"WindGensI\0"),
        Some(b"WindGensF\0"),
        Some(b"WindGensS\0"),
        Some(b"WindGensV\0"),
    ),
    (
        "XYCurves",
        Some(b"XYCurvesI\0"),
        Some(b"XYCurvesF\0"),
        Some(b"XYCurvesS\0"),
        Some(b"XYCurvesV\0"),
    ),
];

/// The loaded family registry: `(name, entry points)`, in [`REGISTRY`] order.
/// `Send`/`Sync` automatically (bare `fn` pointers).
#[derive(Clone, Default)]
pub struct FamilyTable {
    entries: Vec<(&'static str, Family)>,
}

impl FamilyTable {
    /// Bind every family's present entry points from `lib`.
    ///
    /// # Safety
    /// Each listed symbol must have the ABI of its kind's `Fn*` type (upheld: the
    /// symbols are the r4133 export table, each an `XxxI/F/S/V(mode, arg)` cdecl).
    /// The returned pointers are valid only while `lib` stays loaded, which
    /// [`crate::ffi::Dll`] guarantees by leaking it for the session.
    pub unsafe fn load(lib: &Library) -> Result<FamilyTable, String> {
        // Every `TWO_DOUBLE_F` name must be a real registry family that really
        // has an `F` symbol — otherwise the split would silently drop an entry
        // point (or name a family that does not exist).
        for want in TWO_DOUBLE_F {
            match REGISTRY.iter().find(|(n, ..)| *n == want) {
                None => return Err(format!("TWO_DOUBLE_F names unknown family {want:?}")),
                Some((_, _, f, _, _)) if f.is_none() => {
                    return Err(format!(
                        "TWO_DOUBLE_F names {want:?}, which has no F symbol"
                    ));
                }
                Some(_) => {}
            }
        }
        let mut entries = Vec::with_capacity(REGISTRY.len());
        for &(name, i, f, s, v) in REGISTRY {
            let two_double = TWO_DOUBLE_F.contains(&name);
            // SAFETY: each `sym::<Fn*>` matches the listed symbol's transcribed
            // ABI (module invariant); a missing symbol is a hard error (below),
            // never a silent skip — a wrong DLL must fail loudly. The `F` symbol
            // of a `TWO_DOUBLE_F` family is transcribed as `FnF2` instead
            // (`DCircuit.pas:27` / `DCmathLib.pas:5`).
            let fam = Family {
                i: match i {
                    Some(n) => Some(unsafe { sym::<FnI>(lib, n) }?),
                    None => None,
                },
                f: match f {
                    Some(n) if !two_double => Some(unsafe { sym::<FnF>(lib, n) }?),
                    _ => None,
                },
                f2: match f {
                    Some(n) if two_double => Some(unsafe { sym::<FnF2>(lib, n) }?),
                    _ => None,
                },
                s: match s {
                    Some(n) => Some(unsafe { sym::<FnS>(lib, n) }?),
                    None => None,
                },
                v: match v {
                    Some(n) => Some(unsafe { sym::<FnV>(lib, n) }?),
                    None => None,
                },
            };
            // No family may carry both `F` ABIs, and the split must not lose an
            // entry point the registry declared.
            if fam.f.is_some() && fam.f2.is_some() {
                return Err(format!("family {name} bound both F ABIs"));
            }
            if f.is_some() != fam.has_f() {
                return Err(format!(
                    "family {name} lost its F entry point in the ABI split"
                ));
            }
            entries.push((name, fam));
        }
        Ok(FamilyTable { entries })
    }

    /// Case-insensitive family lookup.
    pub fn get(&self, name: &str) -> Option<&Family> {
        self.entries
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, f)| f)
    }

    /// `(family, kinds)` pairs for the `caps` handshake, where `kinds` is the
    /// present-shape subset of `"ifsv"` (e.g. `"isv"` for Monitors). A
    /// [`TWO_DOUBLE_F`] family reports `f` like any other — the ABI split is an
    /// internal detail of how the same entry point is called, not a lost shape.
    pub fn manifest(&self) -> Vec<(&'static str, String)> {
        self.entries
            .iter()
            .map(|(n, f)| {
                let mut k = String::new();
                if f.i.is_some() {
                    k.push('i');
                }
                if f.has_f() {
                    k.push('f');
                }
                if f.s.is_some() {
                    k.push('s');
                }
                if f.v.is_some() {
                    k.push('v');
                }
                (*n, k)
            })
            .collect()
    }

    /// Total bound family entry points (for the smoke's coverage assertion).
    /// The two `F` ABIs count as the one entry point they are, so the total
    /// stays 147 across the [`TWO_DOUBLE_F`] split.
    pub fn entry_point_count(&self) -> usize {
        self.entries
            .iter()
            .map(|(_, f)| {
                f.i.is_some() as usize
                    + f.has_f() as usize
                    + f.s.is_some() as usize
                    + f.v.is_some() as usize
            })
            .sum()
    }

    /// Number of families in the table.
    pub fn family_count(&self) -> usize {
        self.entries.len()
    }
}

/// A decoded V-protocol result, tagged by the DLL's `myType`
/// (1=int, 2=double, 3=complex re/im pairs, 4=string, 5=bytes).
#[derive(Debug, Clone, PartialEq)]
pub enum VData {
    /// `myType = 1` — flat `i32` array.
    Ints(Vec<i32>),
    /// `myType = 2` — flat `f64` array (real).
    Doubles(Vec<f64>),
    /// `myType = 3` — complex array, interleaved `[re, im, ...]`.
    Complex(Vec<f64>),
    /// `myType = 4` — `\0`-separated string array (raw; no gate-path monitor strip).
    Strings(Vec<String>),
    /// `myType = 5` (or any unknown tag) — raw byte stream.
    Bytes(Vec<u8>),
}

impl VData {
    /// The `myType` tag this variant corresponds to.
    pub fn type_tag(&self) -> i32 {
        match self {
            VData::Ints(_) => 1,
            VData::Doubles(_) => 2,
            VData::Complex(_) => 3,
            VData::Strings(_) => 4,
            VData::Bytes(_) => 5,
        }
    }

    /// Element count (floats for `Complex` count `re`+`im` slots).
    pub fn len(&self) -> usize {
        match self {
            VData::Ints(v) => v.len(),
            VData::Doubles(v) | VData::Complex(v) => v.len(),
            VData::Strings(v) => v.len(),
            VData::Bytes(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Decode a raw V buffer (`myType` tag + bytes) into [`VData`].
pub fn decode_v(tag: i32, bytes: &[u8]) -> VData {
    match tag {
        1 => VData::Ints(
            bytes
                .chunks_exact(4)
                .map(|c| i32::from_le_bytes(c.try_into().unwrap()))
                .collect(),
        ),
        2 => VData::Doubles(
            bytes
                .chunks_exact(8)
                .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
                .collect(),
        ),
        3 => VData::Complex(
            bytes
                .chunks_exact(8)
                .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
                .collect(),
        ),
        4 => VData::Strings(decode_string_raw(bytes)),
        _ => VData::Bytes(bytes.to_vec()),
    }
}

/// `\0`-split a `myType = 4` buffer into strings, dropping the single trailing
/// empty segment (the Pascal always writes a trailing `Char(0)`). Distinct from
/// [`crate::dss::decode_string_array`] (the gate-capture path): **no**
/// monitor-header leading-space strip, so this is a faithful raw dump of any
/// string family (node order, class names, property lists, ...).
pub fn decode_string_raw(bytes: &[u8]) -> Vec<String> {
    if bytes.is_empty() {
        return Vec::new();
    }
    let b = if *bytes.last().unwrap() == 0 {
        &bytes[..bytes.len() - 1]
    } else {
        bytes
    };
    b.split(|&c| c == 0)
        .map(|seg| String::from_utf8_lossy(seg).into_owned())
        .collect()
}

/// Encode a [`VData`] into `(myType tag, little-endian bytes)` for a V-protocol
/// **SET** mode (the caller hands the array in via `myPointer` + `mySize`; e.g.
/// `LoadShapeV(2)` PMult write). String/byte set uses `\0`-joined / raw bytes.
pub fn encode_v_set(data: &VData) -> (i32, Vec<u8>) {
    match data {
        VData::Ints(v) => (1, v.iter().flat_map(|x| x.to_le_bytes()).collect()),
        VData::Doubles(v) => (2, v.iter().flat_map(|x| x.to_le_bytes()).collect()),
        VData::Complex(v) => (3, v.iter().flat_map(|x| x.to_le_bytes()).collect()),
        VData::Strings(v) => {
            let mut b = Vec::new();
            for s in v {
                b.extend_from_slice(s.as_bytes());
                b.push(0);
            }
            (4, b)
        }
        VData::Bytes(v) => (5, v.clone()),
    }
}

/// The `mySize` value a V-protocol **SET** must pass in: an **element (point)
/// count**, not a byte count. The r4133 SET path clamps `LoopLimit :=
/// min(mySize, NumPoints)` and advances `myPointer` one element per iteration
/// (`DLoadShape.pas` PMult write, `DXYCurves.pas`), so a byte count (8× for
/// doubles) defeats the clamp and makes the DLL over-read the buffer whenever
/// the supplied array is shorter than the target's point count. `Complex`
/// counts logical points (re/im pairs); `Bytes` has no numeric element and
/// falls back to its byte length.
pub fn vset_len(data: &VData) -> i32 {
    let n = match data {
        VData::Ints(v) => v.len(),
        VData::Doubles(v) => v.len(),
        VData::Complex(v) => v.len() / 2,
        VData::Strings(v) => v.len(),
        VData::Bytes(v) => v.len(),
    };
    n as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_v_by_type_tag() {
        // tag 1: ints
        let b: Vec<u8> = [1i32, -2, 3].iter().flat_map(|x| x.to_le_bytes()).collect();
        assert_eq!(decode_v(1, &b), VData::Ints(vec![1, -2, 3]));
        // tag 2: real doubles
        let b: Vec<u8> = [1.5f64, 2.5].iter().flat_map(|x| x.to_le_bytes()).collect();
        assert_eq!(decode_v(2, &b), VData::Doubles(vec![1.5, 2.5]));
        // tag 3: complex (interleaved re/im) — same bytes, different variant
        let b: Vec<u8> = [1.0f64, -1.0, 2.0, -2.0]
            .iter()
            .flat_map(|x| x.to_le_bytes())
            .collect();
        assert_eq!(decode_v(3, &b), VData::Complex(vec![1.0, -1.0, 2.0, -2.0]));
        // tag 5 and any unknown -> raw bytes
        assert_eq!(decode_v(5, &[1, 2, 3]), VData::Bytes(vec![1, 2, 3]));
        assert_eq!(decode_v(99, &[7]), VData::Bytes(vec![7]));
    }

    #[test]
    fn string_raw_drops_one_trailing_nul_no_space_strip() {
        // Pascal writes `name\0` per element (buffer ends with \0).
        assert_eq!(
            decode_string_raw(b"Load.l\0Vsource.source\0"),
            vec!["Load.l".to_string(), "Vsource.source".to_string()]
        );
        // Unlike the gate path, a leading space is PRESERVED (raw dump).
        assert_eq!(
            decode_string_raw(b" V1\0V2\0"),
            vec![" V1".to_string(), "V2".to_string()]
        );
        assert!(decode_string_raw(b"").is_empty());
    }

    #[test]
    fn encode_v_set_round_trips_through_decode() {
        for d in [
            VData::Ints(vec![4, 5, 6]),
            VData::Doubles(vec![5.0, 6.0, 7.0]),
            VData::Complex(vec![1.0, 2.0, 3.0, 4.0]),
        ] {
            let (tag, bytes) = encode_v_set(&d);
            assert_eq!(tag, d.type_tag());
            assert_eq!(decode_v(tag, &bytes), d);
        }
        // strings: `\0`-joined out, split back.
        let (tag, bytes) = encode_v_set(&VData::Strings(vec!["a".into(), "bb".into()]));
        assert_eq!(tag, 4);
        assert_eq!(
            decode_v(tag, &bytes),
            VData::Strings(vec!["a".into(), "bb".into()])
        );
    }

    /// The two-double `F` split is declared over the registry, offline: both
    /// names exist, both really have an `F` symbol, and the registry still
    /// declares 42 families / 147 entry points (so the split can only move an
    /// `F` between two ABIs, never drop one). The live counterpart is
    /// [`FamilyTable::load`]'s hard error.
    #[test]
    fn two_double_f_names_real_f_bearing_families_and_moves_no_entry_point() {
        for want in TWO_DOUBLE_F {
            let row = REGISTRY
                .iter()
                .find(|(n, ..)| *n == want)
                .unwrap_or_else(|| panic!("TWO_DOUBLE_F names unknown family {want:?}"));
            assert!(
                row.2.is_some(),
                "TWO_DOUBLE_F names {want:?}, which has no F symbol"
            );
        }
        assert_eq!(REGISTRY.len(), 42, "family registry drifted");
        let points: usize = REGISTRY
            .iter()
            .map(|(_, i, f, s, v)| {
                i.is_some() as usize
                    + f.is_some() as usize
                    + s.is_some() as usize
                    + v.is_some() as usize
            })
            .sum();
        assert_eq!(points, 147, "entry-point count drifted");
    }

    #[test]
    fn vset_len_is_an_element_count_not_a_byte_count() {
        // Doubles/Ints: one element each (NOT 8x / 4x the byte length).
        assert_eq!(vset_len(&VData::Doubles(vec![5.0, 6.0, 7.0])), 3);
        assert_eq!(vset_len(&VData::Ints(vec![4, 5, 6, 7])), 4);
        // Complex: logical points (re/im pairs).
        assert_eq!(vset_len(&VData::Complex(vec![1.0, 2.0, 3.0, 4.0])), 2);
        assert_eq!(vset_len(&VData::Strings(vec!["a".into(), "bb".into()])), 2);
    }
}

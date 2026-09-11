//! `GOLDEN_REBASE_PLAN.md` WP-G1 sub-step **G1.10c** — the CONTENTS of the
//! run-created demand-interval tree, compared live on every gating channel of
//! the unified corpus gate.
//!
//! # The surface
//!
//! For one (case, channel, port-run): the text of every FILE member of the
//! demand-interval tree the run created under the case directory
//! (`<OutputDirectory><CaseName>/DI_yr_<year>/*.csv`, r4133
//! `Version8/Source/Meters/EnergyMeter.pas:824` — `DI_Dir := CasePath + '\DI_yr_'
//! + Year`; the port
//! `crates/dss-core/src/solution/meters/demand_interval.rs:941-974`). Members
//! are selected out of the very created-file classification G1.10a compares
//! ([`dss_epri::guard::is_di_member`]) and arrive here as
//! `normalized name → file text` on both sides ([`DiTree`]), so this surface can
//! never disagree with that one about *what the run wrote* — only about what is
//! inside it.
//!
//! Purely observational: no command is added to any run on any transport
//! (`CloseDI` is never injected — the R part measured that every producer
//! flushes the last CLOSED cycle to disk by itself: the DI writers are in-memory
//! handlers on all three producers, r4133 `Meters/EnergyMeter.pas:2921-2932`
//! (`CloseDemandIntervalFile`), the port `demand_interval.rs:515-570`).
//!
//! Provenance: new coverage, not catch-up. The upstream harness reads the same
//! CSVs out of its two output archives but can never fail on one — a name
//! missing on the other side is skipped (fastdss `tests/compare_outputs.py`
//! `:416-421`) and a cell mismatch is PRINTED with the `raise` commented out
//! (`:517-527`).
//!
//! # Why the DI golden's policy does not gate this surface
//!
//! `golden_reports.rs::di_policy` (rel `5e-8`, abs `0`) is the floor of ONE
//! 24-step daily IEEE13 fixture, and it stays exactly that: this module does not
//! call it, move it or re-tune it. The live population is 720…8 760-step yearly
//! runs on 1 255…2 998-bus feeders, where the two ORACLES disagree with EACH
//! OTHER on 23 065 ckt7 cells and 1 440 123Bus cells above that band — a policy
//! no engine can satisfy is not a floor (coordinator decision **D42(1)**).
//!
//! # The rule: a DI cell is compared at its own quantity's calibrated tier
//!
//! A DI cell is not "a number in a CSV": it is an EnergyMeter register, a power,
//! a per-unit voltage, a rating, a count or a name — and the gate already
//! carries a calibrated floor for each of those, per scenario kind
//! ([`super::tol_for`]). Each column is therefore classified by the physical
//! quantity its writer produces ([`DiClass`]) and compared at that class's
//! existing tier. **No `Tolerances` value moves and no new constant is defined**
//! — the derivation is recorded in `tests/TOLERANCE_NOTES.md` §G1.10c.
//!
//! The classification is **positional**, never a regex over free text:
//!
//! * a register file's columns are `TEnergyMeterObj.RegisterNames` in order
//!   (r4133 `EnergyMeter.pas:1009-1044` for the 32 fixed names,
//!   `:3090-3119` `AssignVoltBaseRegisterNames` for the five voltage-base
//!   blocks; the port's own table is
//!   `crates/dss-core/src/elements/meter/energymeter/mod.rs:604` +
//!   `energymeter/zone.rs:58-90`), so [`FIXED_REGISTERS`] is indexed by register
//!   position and its length is the port's own [`NUM_EM_REGISTERS`];
//! * the three fixed-shape reports carry their whole header as a literal in the
//!   engine ([`OVERLOAD_COLUMNS`] ← r4133 `:3849`, [`VOLT_EXCEPTION_COLUMNS`] ←
//!   `:3862-3863`, [`SYSTEM_METER_COLUMNS`] ← `:3430-3431` / `:3472-3473`), so
//!   the table here is asserted against the file's own header.
//!
//! A column the table does not cover **fails the case loudly** — there is no
//! default band anywhere in this module. Exhaustiveness over the live population
//! is therefore not a hope: every compared file asserts its own header against
//! the table before a single cell is read.
//!
//! # Ledger key, and what an exclusion can never reach
//!
//! An exclusion selects ONE COLUMN of ONE file: the key is
//! `<normalized member name>:<column name>`, both ASCII-lowercased
//! (`example_ckt7/di_yr_0/di_totals_1.csv:kvarh`). The file SET, the header, the
//! row count and the per-row field count are compared **unconditionally**
//! whatever the ledger says, and the key is consulted only for a file that has
//! at least one data row — so a scope that masks nothing stays un-hit and
//! `ledger.json`'s fail-on-stale rule reports it.
//!
//! # Vacuity
//!
//! [`di_census`] is re-derived on every run — cases with a non-empty tree,
//! (case, channel) comparisons performed, files compared, cells compared — and
//! is pinned fail-on-stale in BOTH directions by the scheduler, so "the
//! comparator ran and compared nothing" cannot pass. An EMPTY tree is a
//! legitimate answer (a deck that issues `CloseDI` without `DemandInterval`
//! writes no DI file on any of the three producers); an ABSENT capture on a case
//! that set the flag is not, and [`super::capture_guard`] fails it here.
//!
//! # Platform
//!
//! Declared under `#[cfg(windows)]` in `harness/mod.rs` for the same reason
//! [`super::run_files`] is: the surface is only ever compared against the gate's
//! two oracle channels and the `r4133` transport is a Win64 DLL. The corpus gate
//! refuses `compare_di` loudly on any other platform rather than comparing
//! nothing (`corpus_gate::runner::compare_with_result`).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use dss_core::elements::meter::energymeter::{NUM_EM_REGISTERS, NUM_EM_VBASE};

use super::capture_guard;
use super::{Tolerances, assert_value_matches_tol, report_lines, split_fields};

/// One producer's demand-interval tree: `normalized member name → file text`.
///
/// The key is exactly G1.10a's created-file member name (`/`-joined,
/// ASCII-case-folded, case-dir-relative —
/// [`dss_epri::guard::normalize_created_name`]), so the two surfaces name the
/// same artifact the same way.
pub type DiTree = BTreeMap<String, String>;

// ---------------------------------------------------------------------------
// The quantity classes.
// ---------------------------------------------------------------------------

/// The physical quantity a DI column carries — and therefore which of the
/// gate's existing calibrated tiers ([`super::tol_for`]) its cells are compared
/// at. Coordinator decision **D42(1)**.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiClass {
    /// The row index (`Hour` / `Time` / `Year`) or the row's meter name
    /// (`Name`): exact.
    Index,
    /// An EnergyMeter energy register (kWh / kvarh / EEN / UE / every
    /// `… Losses kWh` and the per-voltage-base blocks): `energy_rel` /
    /// `energy_abs`, the class the gate already applies to these very registers
    /// (`harness::compare_meter`).
    Energy,
    /// A demand (`Max …` / `Peak …`) register — a kW/kVA power: `i_rel` /
    /// `i_abs`.
    Power,
    /// A per-unit voltage (`Min/Max [LV] Voltage`): `v_rel` / `v_abs`.
    Voltage,
    /// A current or a current ratio (`I1(A)`, `% Normal`): `i_rel` / `i_abs`.
    Current,
    /// A rating or a count (`Normal Amps`, `kVBase`, the four exception
    /// counts): exact.
    Exact,
    /// A name (`Element`, `Min/Max [LV] Bus`): ASCII-case-insensitive string
    /// equality — never a number compare, so two bus names that happen to be
    /// numeric (`610`) are compared as the names they are.
    Text,
}

impl DiClass {
    /// The `(rel, abs)` band this class is compared at for the case's tier, or
    /// `None` for [`DiClass::Text`] (compared as a string).
    fn band(self, tol: &Tolerances) -> Option<(f64, f64)> {
        match self {
            DiClass::Index | DiClass::Exact => Some((0.0, 0.0)),
            DiClass::Energy => Some((tol.energy_rel, tol.energy_abs)),
            DiClass::Power | DiClass::Current => Some((tol.i_rel, tol.i_abs)),
            DiClass::Voltage => Some((tol.v_rel, tol.v_abs)),
            DiClass::Text => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            DiClass::Index => "index",
            DiClass::Energy => "energy",
            DiClass::Power => "power",
            DiClass::Voltage => "voltage",
            DiClass::Current => "current",
            DiClass::Exact => "exact",
            DiClass::Text => "text",
        }
    }
}

// ---------------------------------------------------------------------------
// The register layout — positional over the port's own `RegisterNames`.
// ---------------------------------------------------------------------------

/// `ord(EMRegister.VBaseStart)` — the first voltage-base register's position,
/// derived from the port's own two public constants rather than restated
/// (r4133 `EnergyMeter.pas:134` `Reg_VBaseStart = 32`).
const VBASE_START: usize = NUM_EM_REGISTERS - 5 * NUM_EM_VBASE;

/// The 32 fixed EnergyMeter registers, in register order, with the quantity each
/// one accumulates: r4133 `Meters/EnergyMeter.pas:1009-1042`, the port
/// `crates/dss-core/src/elements/meter/energymeter/mod.rs:604-638`
/// (`default_register_names`). The order IS the DI file's column order
/// (r4133 `:2963`, `:3323`, `:3530`, `:3605`), which is what makes this table
/// positional.
///
/// `Max …` / `Zone Max …` are demand registers (kW / kVA) and carry the power
/// class; everything else here is an accumulated energy.
/// [`the_fixed_register_table_is_the_ports_own_register_names`] drives this
/// table against a live engine's own `RegisterNames`, so a renamed or reordered
/// register reds here instead of silently re-classing a column.
const FIXED_REGISTERS: [(&str, DiClass); 32] = [
    ("kWh", DiClass::Energy),
    ("kvarh", DiClass::Energy),
    ("Max kW", DiClass::Power),
    ("Max kVA", DiClass::Power),
    ("Zone kWh", DiClass::Energy),
    ("Zone kvarh", DiClass::Energy),
    ("Zone Max kW", DiClass::Power),
    ("Zone Max kVA", DiClass::Power),
    ("Overload kWh Normal", DiClass::Energy),
    ("Overload kWh Emerg", DiClass::Energy),
    ("Load EEN", DiClass::Energy),
    ("Load UE", DiClass::Energy),
    ("Zone Losses kWh", DiClass::Energy),
    ("Zone Losses kvarh", DiClass::Energy),
    ("Zone Max kW Losses", DiClass::Power),
    ("Zone Max kvar Losses", DiClass::Power),
    ("Load Losses kWh", DiClass::Energy),
    ("Load Losses kvarh", DiClass::Energy),
    ("No Load Losses kWh", DiClass::Energy),
    ("No Load Losses kvarh", DiClass::Energy),
    ("Max kW Load Losses", DiClass::Power),
    ("Max kW No Load Losses", DiClass::Power),
    ("Line Losses", DiClass::Energy),
    ("Transformer Losses", DiClass::Energy),
    ("Line Mode Line Losses", DiClass::Energy),
    ("Zero Mode Line Losses", DiClass::Energy),
    ("3-phase Line Losses", DiClass::Energy),
    ("1- and 2-phase Line Losses", DiClass::Energy),
    ("Gen kWh", DiClass::Energy),
    ("Gen kvarh", DiClass::Energy),
    ("Gen Max kW", DiClass::Power),
    ("Gen Max kVA", DiClass::Power),
];

/// The five voltage-base register blocks, in block order — r4133
/// `EnergyMeter.pas:3099-3103`, the port `energymeter/zone.rs:65-74`. Every one
/// of them accumulates an energy (kWh), so the whole block carries
/// [`DiClass::Energy`]; an unused slot is named `Aux<n>` (`:3105-3113`) and
/// stays zero.
const VBASE_SUFFIXES: [&str; 5] = [
    "kV Losses",
    "kV Line Loss",
    "kV Load Loss",
    "kV No Load Loss",
    "kV Load Energy",
];

// ---------------------------------------------------------------------------
// The fixed-shape reports — table asserted against the engine's own header.
// ---------------------------------------------------------------------------

/// `DI_SystemMeter_<n>.csv` / `SystemMeter_<n>.csv` — the circuit-level meter.
/// r4133 writes the header as one literal: `EnergyMeter.pas:3430-3431` (hourly)
/// and `:3472-3473` (yearly); the port
/// `solution/meters/demand_interval.rs:495-498` / `:586-589`.
const SYSTEM_METER_COLUMNS: [(&str, DiClass); 8] = [
    ("Hour", DiClass::Index),
    ("kWh", DiClass::Energy),
    ("kvarh", DiClass::Energy),
    ("Peak kW", DiClass::Power),
    ("peak kVA", DiClass::Power),
    ("Losses kWh", DiClass::Energy),
    ("Losses kvarh", DiClass::Energy),
    ("Peak Losses kW", DiClass::Power),
];

/// `DI_Overloads_<n>.csv` — the overload report. Header literal r4133
/// `EnergyMeter.pas:3849`, rows `TEnergyMeter.WriteOverloadReport` `:3172-3292`;
/// the port `solution/meters/demand_interval.rs:364-371` / `:768`.
const OVERLOAD_COLUMNS: [(&str, DiClass); 10] = [
    ("Hour", DiClass::Index),
    ("Element", DiClass::Text),
    ("Normal Amps", DiClass::Exact),
    ("Emerg Amps", DiClass::Exact),
    ("% Normal", DiClass::Current),
    ("% Emerg", DiClass::Current),
    ("kVBase", DiClass::Exact),
    ("I1(A)", DiClass::Current),
    ("I2(A)", DiClass::Current),
    ("I3(A)", DiClass::Current),
];

/// `DI_VoltExceptions_<n>.csv` — the voltage-exception report. Header literal
/// r4133 `EnergyMeter.pas:3862-3863`, rows
/// `TEnergyMeter.WriteVoltageReport` `:3620-3695`; the port
/// `solution/meters/demand_interval.rs:373-383` / `:910`.
const VOLT_EXCEPTION_COLUMNS: [(&str, DiClass); 13] = [
    ("Hour", DiClass::Index),
    ("Undervoltages", DiClass::Exact),
    ("Min Voltage", DiClass::Voltage),
    ("Overvoltage", DiClass::Exact),
    ("Max Voltage", DiClass::Voltage),
    ("Min Bus", DiClass::Text),
    ("Max Bus", DiClass::Text),
    ("LV Undervoltages", DiClass::Exact),
    ("Min LV Voltage", DiClass::Voltage),
    ("LV Overvoltage", DiClass::Exact),
    ("Max LV Voltage", DiClass::Voltage),
    ("Min LV Bus", DiClass::Text),
    ("Max LV Bus", DiClass::Text),
];

/// The four spellings the index column of a register / system-meter file takes:
/// `"Hour"` (per-interval, r4133 `:2962`, `:3430`), `Time` (`DI_Totals`,
/// `:3317`), `Year` (the yearly totals, `:3472`, `:3602`) and `Name` (the
/// per-meter totals, `:3527`). The class is the same for all four — exact.
const INDEX_HEADERS: [&str; 4] = ["hour", "time", "year", "name"];

/// Which shape a DI member has. `Registers` is the 1 + [`NUM_EM_REGISTERS`]
/// column family — `DI_Totals`, `Totals`, `EnergyMeterTotals` and one file per
/// meter, named after the METER, which is why it is the fallback arm: a meter
/// may be called anything, and the shape (not the name) is what is then
/// asserted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiFileKind {
    Registers,
    SystemMeter,
    Overloads,
    VoltExceptions,
}

impl DiFileKind {
    /// The kind of one normalized member name (`…/di_yr_1/di_totals_1.csv`).
    fn of(name: &str) -> DiFileKind {
        match file_stem(name) {
            "di_systemmeter" | "systemmeter" => DiFileKind::SystemMeter,
            "di_overloads" => DiFileKind::Overloads,
            "di_voltexceptions" => DiFileKind::VoltExceptions,
            _ => DiFileKind::Registers,
        }
    }

    /// The declared column table of a fixed-shape report, or `None` for the
    /// register family (whose table is positional, [`FIXED_REGISTERS`]).
    fn table(self) -> Option<&'static [(&'static str, DiClass)]> {
        match self {
            DiFileKind::Registers => None,
            DiFileKind::SystemMeter => Some(&SYSTEM_METER_COLUMNS),
            DiFileKind::Overloads => Some(&OVERLOAD_COLUMNS),
            DiFileKind::VoltExceptions => Some(&VOLT_EXCEPTION_COLUMNS),
        }
    }

    /// How many fields a data row of this kind has.
    fn width(self) -> usize {
        self.table().map_or(1 + NUM_EM_REGISTERS, <[_]>::len)
    }
}

/// One column's name (as the ledger and the panic text spell it: ASCII
/// lower case, quotes stripped) and the class its cells are compared at.
pub struct ColumnSpec {
    pub name: String,
    pub class: DiClass,
}

/// The member's own stem: the base name without its `_<file number>.csv` tail
/// (r4133 `Meters/EnergyMeter.pas:3051` and `MakeDIFileName`). For the four
/// report families it is the family name; for a meter's own file it is the
/// METER's name, which is why the register family is recognised by shape.
fn file_stem(name: &str) -> &str {
    let base = name.rsplit('/').next().unwrap_or(name);
    base.strip_suffix(".csv")
        .map(|s| s.trim_end_matches(|c: char| c.is_ascii_digit()))
        .map(|s| s.strip_suffix('_').unwrap_or(s))
        .unwrap_or(base)
}

/// Strip the quoting the engines wrap most header cells in (`"Peak kW"`, but
/// bare `kWh` in the system-meter header) and fold to ASCII lower case.
fn header_name(raw: &str) -> String {
    raw.trim().trim_matches('"').trim().to_ascii_lowercase()
}

/// The class vector of one file, derived from its kind and asserted against its
/// own header.
///
/// This is where "an unclassified column fails the case" is implemented: every
/// arm either matches the declared table or panics. The one header shape that
/// carries no column names — `Time` / `Year` / `Name` alone, which r4133 writes
/// when the meter list is empty at the moment the totals file is opened
/// (`EnergyMeter.pas:3528-3530`, `:3603-3606`: the `RegisterNames` loop is
/// guarded by `if Assigned(mtr)`) — is accepted explicitly, and its columns are
/// then named by the positional table.
fn column_specs(kind: DiFileKind, header: &[String], ctx: &str) -> Vec<ColumnSpec> {
    let names: Vec<String> = header.iter().map(|h| header_name(h)).collect();
    let width = kind.width();
    assert!(
        !names.is_empty() && INDEX_HEADERS.contains(&names[0].as_str()),
        "{ctx}: the demand-interval header starts with {:?}, which is none of the \
         four index columns the engines write ({INDEX_HEADERS:?} — r4133 \
         `Meters/EnergyMeter.pas:2962`/`:3317`/`:3472`/`:3527`). The DI class \
         table is positional, so a header it cannot recognise fails the case \
         instead of being compared at a default band.",
        names.first().map(String::as_str).unwrap_or("")
    );

    if let Some(table) = kind.table() {
        assert_eq!(
            names.len(),
            width,
            "{ctx}: this demand-interval report has {} header columns, and the \
             class table for {kind:?} declares {width} ({:?}). The header is a \
             single literal in both engines (r4133 \
             `Meters/EnergyMeter.pas:3430`/`:3849`/`:3862`), so a different width \
             means the report changed and the table must be re-derived — never \
             compared at a default band.",
            names.len(),
            table.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
        );
        return table
            .iter()
            .enumerate()
            .map(|(j, (declared, class))| {
                let want = header_name(declared);
                // Column 0 is the one cell of a fixed table that legitimately
                // varies: the same report is written per interval (`"Hour"`) and
                // per year (`Year`).
                if j > 0 {
                    assert_eq!(
                        names[j], want,
                        "{ctx}: column {j} of this report is spelled {:?}, but the \
                         class table declares {declared:?} there. The table is \
                         positional over the engines' own header literal — \
                         re-derive it from r4133 rather than letting a renamed \
                         column inherit its neighbour's band.",
                        names[j]
                    );
                }
                ColumnSpec {
                    name: names[j].clone(),
                    class: *class,
                }
            })
            .collect();
    }

    // The register family: 1 index column + `NUM_EM_REGISTERS` registers.
    let named = match names.len() {
        1 => false,
        n if n == width => true,
        n => panic!(
            "{ctx}: this demand-interval register file has {n} header columns; \
             the only two shapes the engines write are the bare index column and \
             1 + {NUM_EM_REGISTERS} named registers (r4133 \
             `Meters/EnergyMeter.pas:2962-2963`, `:3317-3326`, `:3528-3530`, \
             `:3603-3606`). An unrecognised header fails the case instead of \
             being compared at a default band."
        ),
    };
    (0..width)
        .map(|j| {
            if j == 0 {
                return ColumnSpec {
                    name: names[0].clone(),
                    class: DiClass::Index,
                };
            }
            let (declared, class) = register_spec(j - 1);
            if named {
                assert!(
                    register_name_matches(j - 1, &names[j]),
                    "{ctx}: register {} of this file is spelled {:?}, but the port's \
                     own register table has {} there (r4133 \
                     `Meters/EnergyMeter.pas:1009-1042` / `:3090-3119`). The DI class \
                     table is positional over `RegisterNames`, so a register that \
                     moved or was renamed fails the case instead of inheriting its \
                     neighbour's band.",
                    j,
                    names[j],
                    declared.unwrap_or("a voltage-base loss register"),
                );
            }
            ColumnSpec {
                // A totals file written before any meter existed carries no
                // register names; the positional table then names the column.
                name: if named {
                    names[j].clone()
                } else {
                    declared
                        .map(str::to_ascii_lowercase)
                        .unwrap_or_else(|| format!("register {}", j))
                },
                class,
            }
        })
        .collect()
}

/// The SHAPE and the class vector one demand-interval member's own header
/// earns — the two derivations [`compare_di_file`] makes before it reads a
/// single cell, exposed so the live exhaustiveness pin
/// (`di_pins::the_di_class_table_covers_every_column_of_every_live_di_file`,
/// GOLDEN_REBASE G1.10c F4r) can state the claim over the port's own five DI
/// trees instead of over this table quoting itself.
///
/// The shape is `(file stem, columns classified, the header carries the column
/// names)` — exactly what [`DI_SHAPES`] records. Panics, like the comparator,
/// on a header the class table does not cover: "an unclassified column fails"
/// is one rule with one implementation, and the pin drives that one.
pub fn classify_member(
    name: &str,
    header_line: &str,
    ctx: &str,
) -> ((String, usize, bool), Vec<ColumnSpec>) {
    let header = split_fields(header_line, ',');
    let specs = column_specs(DiFileKind::of(name), &header, ctx);
    let shape = (file_stem(name).to_string(), specs.len(), header.len() > 1);
    (shape, specs)
}

/// The declared name (fixed registers only) and class of register `r` (0-based).
fn register_spec(r: usize) -> (Option<&'static str>, DiClass) {
    match FIXED_REGISTERS.get(r) {
        Some((name, class)) => (Some(name), *class),
        // The five voltage-base blocks: `<vbase> kV <suffix>` or `Aux<n>`, all
        // energy accumulations.
        None => (None, DiClass::Energy),
    }
}

/// Whether `spelled` (ASCII-lowercased, dequoted) is what register `r` is called
/// on a live engine: the fixed name below [`VBASE_START`], and above it either
/// the block's `<vbase> kV <suffix>` or the `Aux<n>` filler of an unused slot.
fn register_name_matches(r: usize, spelled: &str) -> bool {
    if let Some((name, _)) = FIXED_REGISTERS.get(r) {
        return spelled == header_name(name);
    }
    if spelled
        .strip_prefix("aux")
        .is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
    {
        return true;
    }
    let block = (r - VBASE_START) / NUM_EM_VBASE;
    VBASE_SUFFIXES
        .get(block)
        .is_some_and(|suffix| spelled.ends_with(&header_name(suffix)))
}

// ---------------------------------------------------------------------------
// The census — re-derived on every gate run, pinned by the scheduler.
// ---------------------------------------------------------------------------

/// Case labels whose demand-interval tree was non-empty on at least one channel.
static DI_CASES: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());
/// `(case label, ledger key)` for every column an applicable ledger scope
/// excluded — the gate's ledger-hit report, so a masked column is never silent.
static DI_EXCLUDED: Mutex<BTreeSet<(String, String)>> = Mutex::new(BTreeSet::new());
/// Every SHAPE the comparator met: `(file stem, columns classified, the header
/// carries the column names)`. The exhaustiveness statement D42(1) asks for
/// (`the_di_class_table_covers_every_column_of_every_live_di_file`) is then a
/// claim about the LIVE population rather than about the table quoting itself:
/// the per-file header assertion in [`column_specs`] already refuses a column
/// the table does not cover, so a shape in this set is a shape the table
/// classified, column by column.
static DI_SHAPES: Mutex<BTreeSet<(String, usize, bool)>> = Mutex::new(BTreeSet::new());
/// (case, channel) DI comparisons actually performed.
static COMPARISONS: AtomicUsize = AtomicUsize::new(0);
/// Files compared over those comparisons.
static FILES: AtomicUsize = AtomicUsize::new(0);
/// Cells compared over those files (an excluded column's cells are not counted).
static CELLS: AtomicUsize = AtomicUsize::new(0);
/// The same three counts, keyed by CASE LABEL — the accounting the scheduler's
/// fail-on-stale census is read off (`scheduler::assert_di_census_is_the_pinned_
/// population`, G1.10c F3).
///
/// The three counters above are process-wide, and this module's own `unit:`
/// fixtures share the process with the gate, so a census read off them would
/// move with the test FILTER (`cargo test … -- harness::di::` alone, or the gate
/// test alone) and could be greened by running fewer tests. Keyed by label the
/// scheduler can do what the D25/Q2 scratch census does one surface over
/// (`harness::run_files::scratch_decline_table`): partition the rows into
/// manifest cases and `unit:` fixtures, refuse a label that is neither, and pin
/// the manifest half.
static DI_ACCOUNT: Mutex<BTreeMap<String, (usize, usize, usize)>> = Mutex::new(BTreeMap::new());

/// The four numbers the scheduler pins fail-on-stale in BOTH directions
/// (coordinator decision D42; `GOLDEN_REBASE_PLAN.md` §1.1(f) non-vacuity): a
/// census of `0` comparisons or `0` cells means the surface compared nothing,
/// not that nothing diverged.
pub struct DiCensus {
    pub cases_with_tree: usize,
    pub comparisons: usize,
    pub files: usize,
    pub cells: usize,
}

fn lock<T>(m: &'static Mutex<T>) -> std::sync::MutexGuard<'static, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The census as it stands (see [`DiCensus`]).
pub fn di_census() -> DiCensus {
    DiCensus {
        cases_with_tree: lock(&DI_CASES).len(),
        comparisons: COMPARISONS.load(Ordering::Relaxed),
        files: FILES.load(Ordering::Relaxed),
        cells: CELLS.load(Ordering::Relaxed),
    }
}

/// The case labels that produced a demand-interval tree, for the fail-on-stale
/// message and the record.
pub fn di_tree_cases() -> Vec<String> {
    lock(&DI_CASES).iter().cloned().collect()
}

/// The `(file stem, columns, header carries names)` shapes this run classified
/// (see [`DI_SHAPES`]).
pub fn di_shapes() -> Vec<(String, usize, bool)> {
    lock(&DI_SHAPES).iter().cloned().collect()
}

/// `(case label, `file:column` key)` for every ledger-excluded DI column that
/// actually masked cells on this run — the gate's ledger-hit report.
pub fn di_excluded_columns() -> Vec<(String, String)> {
    lock(&DI_EXCLUDED).iter().cloned().collect()
}

/// The census BY CASE LABEL — `(label, (comparisons, files, cells))`, see
/// [`DI_ACCOUNT`]. The scheduler pins the manifest half of this table and
/// refuses a label that is neither a manifest case nor a `unit:` fixture.
pub fn di_account() -> Vec<(String, (usize, usize, usize))> {
    lock(&DI_ACCOUNT)
        .iter()
        .map(|(k, v)| (k.clone(), *v))
        .collect()
}

// ---------------------------------------------------------------------------
// The comparator.
// ---------------------------------------------------------------------------

/// Compare one channel's demand-interval tree against the port's.
///
/// * `channel` — the gating channel's tag (`capi_v0145` / `r4133`).
/// * `oracle` — the oracle transport's tree, already read off its sidecar
///   (`corpus_gate::runner`); `None` fails the case through the presence rail,
///   which is the whole point of the flag-gated `Option`.
/// * `port` — [`super::run_files::RunFileProbe::di_tree`]'s answer, read before
///   the probe sweeps.
/// * `tol` — the case's own tier (`harness::tol_for(&c.kind)`); each column is
///   compared at the band its [`DiClass`] selects out of it.
/// * `excluded` — the ledger partition, keyed `<file>:<column>`. Field by field,
///   never a whole-case skip; the call marks the scope hit, which is what keeps
///   `ledger.json` fail-on-stale honest.
/// * `label` — the gate's case label, used as the census key and in the panic.
#[track_caller]
pub fn compare_di(
    channel: &str,
    oracle: Option<&DiTree>,
    port: &DiTree,
    tol: &Tolerances,
    excluded: &dyn Fn(&str) -> bool,
    label: &str,
) {
    let ctx = format!("{label} [{channel}] demand-interval tree");
    let oracle = capture_guard::require_capture_opt("compare_di", channel, oracle, &ctx);

    // The file SET first: a missing or extra DI file is a divergence in its own
    // right, and comparing "the files both sides happen to have" would hide it
    // (the upstream harness's `except KeyError: continue`).
    let oracle_names: BTreeSet<&String> = oracle.keys().collect();
    let port_names: BTreeSet<&String> = port.keys().collect();
    assert!(
        oracle_names == port_names,
        "{ctx}: the demand-interval FILE SET differs.\n  \
         missing (the oracle wrote it, the port did not): {:?}\n  \
         extra   (the port wrote it, the oracle did not): {:?}\n  \
         oracle ({}): {oracle_names:?}\n  port   ({}): {port_names:?}\n  \
         The members are the run-created `DI_yr_<year>` tree's files, selected by \
         the same classification G1.10a compares as a set \
         (`dss_epri::guard::is_di_member`), so a difference here is a missing or \
         extra WRITER, never a capture artifact.",
        oracle_names.difference(&port_names).collect::<Vec<_>>(),
        port_names.difference(&oracle_names).collect::<Vec<_>>(),
        oracle_names.len(),
        port_names.len(),
    );

    COMPARISONS.fetch_add(1, Ordering::Relaxed);
    if !port.is_empty() {
        lock(&DI_CASES).insert(label.to_string());
    }

    let mut cells = 0usize;
    for (name, port_text) in port {
        let oracle_text = &oracle[name];
        cells += compare_di_file(name, oracle_text, port_text, tol, excluded, label, &ctx);
        FILES.fetch_add(1, Ordering::Relaxed);
    }
    // …and the same three numbers under this case's own label, for the
    // scheduler's fail-on-stale census (see [`DI_ACCOUNT`]). One update per
    // (case, channel), after the comparison: a case that FAILED never reaches
    // it, which is what keeps a census read off a partial run impossible.
    let mut acct = lock(&DI_ACCOUNT);
    let row = acct.entry(label.to_string()).or_default();
    row.0 += 1;
    row.1 += port.len();
    row.2 += cells;
}

/// One member of the tree: header verbatim, row and field counts exact, cells by
/// their column's class. Returns the number of cells compared (an excluded
/// column's cells are not counted), which the caller folds into the per-case
/// half of the census ([`DI_ACCOUNT`]).
#[track_caller]
fn compare_di_file(
    name: &str,
    oracle_text: &str,
    port_text: &str,
    tol: &Tolerances,
    excluded: &dyn Fn(&str) -> bool,
    label: &str,
    case_ctx: &str,
) -> usize {
    let ctx = format!("{case_ctx} {name}");
    // `\r`-stripped, blank lines dropped: the port writes `\n`, the Pascal
    // engines `\r\n` (`harness::report_lines`).
    let ol = report_lines(oracle_text);
    let rl = report_lines(port_text);
    assert!(
        !ol.is_empty() && !rl.is_empty(),
        "{ctx}: a demand-interval file has no header line (oracle {} lines, port \
         {} lines). Every DI writer emits its header when it opens the file \
         (r4133 `Meters/EnergyMeter.pas:2962`, `:3317`, `:3430`, `:3849`, \
         `:3862`), so an empty file is a writer bug.",
        ol.len(),
        rl.len(),
    );
    assert_eq!(
        rl[0], ol[0],
        "{ctx}: the header line differs (port vs oracle). The header is a literal \
         in every producer and carries the register names themselves, so a \
         difference is a naming or layout divergence, never a floor question."
    );
    assert_eq!(
        rl.len(),
        ol.len(),
        "{ctx}: data row count differs (port {} vs oracle {}). A DI file gets one \
         row per closed demand interval, so a row-count difference is a different \
         number of intervals — triage the run, never the cells.",
        rl.len() - 1,
        ol.len() - 1,
    );

    let (shape, specs) = classify_member(name, &ol[0], &ctx);
    lock(&DI_SHAPES).insert(shape);

    if ol.len() == 1 {
        // Header only (a report whose run had no exception): nothing to compare,
        // and no ledger key is consulted — a scope that masks nothing must stay
        // un-hit so `ledger.json`'s fail-on-stale rule reports it.
        return 0;
    }

    // The ledger partition, resolved once per file: an excluded column drops out
    // of the CELL compare only. The set, the header and the two counts above are
    // already asserted, unconditionally.
    let masked: Vec<bool> = specs
        .iter()
        .map(|spec| {
            let key = format!("{name}:{}", spec.name);
            if excluded(&key) {
                lock(&DI_EXCLUDED).insert((label.to_string(), key));
                true
            } else {
                false
            }
        })
        .collect();

    let mut compared = 0usize;
    for (i, (r, o)) in rl[1..].iter().zip(&ol[1..]).enumerate() {
        let rf = split_fields(r, ',');
        let of = split_fields(o, ',');
        assert_eq!(
            rf.len(),
            of.len(),
            "{ctx}: row {i} field count differs (port {} vs oracle {})",
            rf.len(),
            of.len(),
        );
        assert_eq!(
            of.len(),
            specs.len(),
            "{ctx}: row {i} has {} fields, and this file's class table covers {} \
             columns. Every cell is compared at its own quantity's tier, so a row \
             the table does not cover fails the case instead of being compared at \
             a default band.",
            of.len(),
            specs.len(),
        );
        for (j, (a, e)) in rf.iter().zip(&of).enumerate() {
            if masked[j] {
                continue;
            }
            // The context is passed in PIECES, not as a formatted string: a
            // full drive compares ~10 M cells, and `assert!` formats its
            // message only when it fires.
            compare_cell(&specs[j], a, e, tol, &ctx, i, j);
            compared += 1;
        }
    }
    CELLS.fetch_add(compared, Ordering::Relaxed);
    compared
}

/// One cell, at its column's class's band.
///
/// `ctx` / `row` / `col` are the location, passed unformatted so the hot path
/// allocates nothing: a full drive compares ~10 M cells and every message below
/// is built by `assert!` only when it fires.
#[track_caller]
fn compare_cell(
    spec: &ColumnSpec,
    actual: &str,
    expected: &str,
    tol: &Tolerances,
    ctx: &str,
    row: usize,
    col: usize,
) {
    let class = spec.class;
    let (a, e) = (actual.trim(), expected.trim());
    let Some((rel, abs)) = class.band(tol) else {
        // Text: ASCII-case-insensitive equality, never a number compare — a bus
        // name that happens to be numeric (`610`) is still a name.
        assert!(
            a.eq_ignore_ascii_case(e),
            "{ctx}: row {row} column {col} ({}): name differs (port {a:?} vs oracle \
             {e:?}). This column carries \
             an element or bus NAME (r4133 `Meters/EnergyMeter.pas:3278`, \
             `:3688-3695`), so it is compared as a string; a divergence is a \
             different element, never a floor question.",
            spec.name,
        );
        return;
    };
    match (a.parse::<f64>(), e.parse::<f64>()) {
        (Ok(av), Ok(ev)) => {
            let allowed = abs + rel * ev.abs();
            let diff = (av - ev).abs();
            assert!(
                diff <= allowed,
                "{ctx}: row {row} column {col} ({}): {} cell differs: port {av} vs \
                 oracle {ev} \
                 (|diff| = {diff:.6e} > allowed {allowed:.6e} = {abs:.3e} + \
                 {rel:.3e}·|oracle|, ratio {:.3}). The band is this case's own \
                 calibrated tier for that quantity (`harness::tol_for`) — never \
                 widen it: triage the divergence into `tests/corpus/ledger.json` \
                 with a `di` scope naming `<file>:<column>` plus an \
                 expected-value pin, or fix the port.",
                spec.name,
                class.label(),
                if allowed > 0.0 {
                    diff / allowed
                } else {
                    f64::INFINITY
                },
            );
        }
        (Ok(_), Err(_)) | (Err(_), Ok(_)) => panic!(
            "{ctx}: row {row} column {col} ({}): one side of a {} cell is not a \
             number (port {a:?} vs oracle {e:?})",
            spec.name,
            class.label()
        ),
        (Err(_), Err(_)) => {
            assert!(
                matches!(class, DiClass::Index | DiClass::Exact),
                "{ctx}: row {row} column {col} ({}): a {} column holds the non-numeric \
                 cell {e:?} on the oracle \
                 side. A banded class is only ever applied to a number; a column \
                 that turns out to be text must be re-classified in the table, \
                 never compared loosely.",
                spec.name,
                class.label(),
            );
            // The `Name` index column of `EnergyMeterTotals` — exact in the
            // domain the cell actually lives in. One cell per file, so the
            // formatted context here costs nothing.
            assert_value_matches_tol(
                a,
                e,
                0.0,
                0.0,
                &format!("{ctx}: row {row} column {col} ({})", spec.name),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dss_core::exec::Dss;

    /// Run `f` and return the panic message — every refusal in this module is
    /// proven to fire, not merely assumed to.
    fn panic_message(f: impl FnOnce()) -> String {
        let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
            .expect_err("the comparator must fail this input");
        if let Some(s) = payload.downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "<non-string panic payload>".to_string()
        }
    }

    fn tol() -> Tolerances {
        super::super::tol_for("feeder")
    }

    fn nothing_excluded(_: &str) -> bool {
        false
    }

    /// The system-meter report exactly as both engines write it (r4133
    /// `Meters/EnergyMeter.pas:3430-3431`).
    const SM_HEADER: &str = "\"Hour\", kWh, kvarh, \"Peak kW\", \"peak kVA\", \
                             \"Losses kWh\", \"Losses kvarh\", \"Peak Losses kW\"";
    const SM_ROW: &str = "1, 1942.0025609665, 307.231257264754, 1942.0025609665, \
                          1966.1548749376, 29.2418215237085, 58.1132789376794, \
                          29.2418215237085";

    const SM_NAME: &str = "case/di_yr_1/di_systemmeter_1.csv";
    const VE_NAME: &str = "case/di_yr_1/di_voltexceptions_1.csv";

    fn file(lines: &[&str]) -> String {
        format!("{}\n", lines.join("\n"))
    }

    fn tree(members: &[(&str, String)]) -> DiTree {
        members
            .iter()
            .map(|(n, t)| ((*n).to_string(), t.clone()))
            .collect()
    }

    /// One system-meter file, with `edit` applied to its single data row.
    fn sm_tree(row: &str) -> DiTree {
        tree(&[(SM_NAME, file(&[SM_HEADER, row]))])
    }

    /// The voltage-exception report (r4133 `:3862-3863`), one row.
    fn ve_tree(row: &str) -> DiTree {
        let header = "\"Hour\", \"Undervoltages\", \"Min Voltage\", \"Overvoltage\", \
                      \"Max Voltage\", \"Min Bus\", \"Max Bus\", \"LV Undervoltages\", \
                      \"Min LV Voltage\", \"LV Overvoltage\", \"Max LV Voltage\", \
                      \"Min LV Bus\", \"Max LV Bus\"";
        tree(&[(VE_NAME, file(&[header, row]))])
    }

    const VE_ROW: &str = "1, 0, 0.991595541066126, 0, 1.04248571038348, 11, 83, 0, \
                          1.00094269530867, 0, 1.01008700935112, 610, 610";

    /// A register file the way a meter writes it: the 32 fixed names, then the
    /// five voltage-base blocks.
    fn register_header(first: &str) -> String {
        let mut h = first.to_string();
        for r in 0..NUM_EM_REGISTERS {
            let name = match register_spec(r).0 {
                Some(n) => n.to_string(),
                None => format!("Aux{}", r - VBASE_START + 1),
            };
            h.push_str(&format!(", \"{name}\""));
        }
        h
    }

    fn register_row(index: &str, value: f64) -> String {
        let mut row = index.to_string();
        for _ in 0..NUM_EM_REGISTERS {
            row.push_str(&format!(", {value}"));
        }
        row
    }

    fn register_tree(name: &str, index_col: &str, index: &str, value: f64) -> DiTree {
        tree(&[(
            name,
            file(&[&register_header(index_col), &register_row(index, value)]),
        )])
    }

    fn compare(oracle: &DiTree, port: &DiTree) {
        compare_di(
            "capi_v0145",
            Some(oracle),
            port,
            &tol(),
            &nothing_excluded,
            "unit:di",
        );
    }

    /// The happy path over three of the four shapes at once.
    #[test]
    fn an_identical_tree_compares_equal() {
        let t = sm_tree(SM_ROW);
        compare(&t, &t);
        let v = ve_tree(VE_ROW);
        compare(&v, &v);
        let r = register_tree("case/di_yr_1/feeder_1.csv", "\"Hour\"", "1", 4.5);
        compare(&r, &r);
    }

    /// `\r\n` (the Pascal writers) and `\n` (the port) are the same file — the
    /// one normalization this surface needs, and it is `report_lines`', not a
    /// tolerance.
    #[test]
    fn the_two_line_endings_are_the_same_file() {
        let port = sm_tree(SM_ROW);
        let oracle = tree(&[(SM_NAME, format!("{SM_HEADER}\r\n{SM_ROW}\r\n"))]);
        compare(&oracle, &port);
    }

    #[test]
    fn a_missing_file_fails() {
        let oracle = tree(&[
            (SM_NAME, file(&[SM_HEADER, SM_ROW])),
            (VE_NAME, file(&["\"Hour\"", "1"])),
        ]);
        let port = sm_tree(SM_ROW);
        let msg = panic_message(move || compare(&oracle, &port));
        assert!(msg.contains("FILE SET differs"), "{msg}");
        assert!(msg.contains("di_voltexceptions_1.csv"), "{msg}");
    }

    #[test]
    fn an_extra_file_fails() {
        let oracle = sm_tree(SM_ROW);
        let port = tree(&[
            (SM_NAME, file(&[SM_HEADER, SM_ROW])),
            ("case/di_yr_1/feeder_1.csv", file(&["\"Hour\""])),
        ]);
        let msg = panic_message(move || compare(&oracle, &port));
        assert!(msg.contains("FILE SET differs"), "{msg}");
        assert!(msg.contains("feeder_1.csv"), "{msg}");
    }

    #[test]
    fn a_header_edit_fails() {
        let oracle = sm_tree(SM_ROW);
        let port = tree(&[(
            SM_NAME,
            file(&[&SM_HEADER.replace("Peak kW", "Peak KW."), SM_ROW]),
        )]);
        let msg = panic_message(move || compare(&oracle, &port));
        assert!(msg.contains("header line differs"), "{msg}");
    }

    #[test]
    fn a_row_count_change_fails() {
        let oracle = tree(&[(SM_NAME, file(&[SM_HEADER, SM_ROW, SM_ROW]))]);
        let port = sm_tree(SM_ROW);
        let msg = panic_message(move || compare(&oracle, &port));
        assert!(msg.contains("data row count differs"), "{msg}");
    }

    #[test]
    fn a_field_count_change_fails() {
        let oracle = sm_tree(SM_ROW);
        let port = sm_tree(&format!("{SM_ROW}, 1.0"));
        let msg = panic_message(move || compare(&oracle, &port));
        assert!(msg.contains("field count differs"), "{msg}");
    }

    /// A cell outside its class's band reds — and the message names the class,
    /// the band and the ratio, so the triage is the ledger or the port, never
    /// the tolerance.
    #[test]
    fn a_cell_outside_its_class_fails() {
        // `Peak kW` is a power: `i_abs + i_rel·|e|` = 1e-5 + 1e-7·1942 ≈ 2.1e-4.
        let oracle = sm_tree(SM_ROW);
        let port = sm_tree(&SM_ROW.replacen("1942.0025609665", "1942.0125609665", 2));
        let msg = panic_message(move || compare(&oracle, &port));
        assert!(msg.contains("power cell differs"), "{msg}");
        assert!(msg.contains("ratio"), "{msg}");
    }

    /// …and one inside it passes. Same numeric perturbation, two different
    /// columns: the class table is what decides, not the file.
    #[test]
    fn a_cell_inside_its_class_passes() {
        let delta = 1.0e-3;
        let kwh = 1942.0025609665_f64;
        // Column 1 (`kWh`) is an energy: 1e-4 + 1e-4·1942 ≈ 0.194 — 1e-3 passes.
        let port = sm_tree(&SM_ROW.replacen("1942.0025609665", &format!("{}", kwh + delta), 1));
        compare(&sm_tree(SM_ROW), &port);
        // The same 1e-3 in column 3 (`Peak kW`, a power at ≈2.1e-4) does not.
        let mut fields: Vec<String> = SM_ROW.split(',').map(|f| f.trim().to_string()).collect();
        fields[3] = format!("{}", kwh + delta);
        let port = sm_tree(&fields.join(", "));
        let msg = panic_message(move || compare(&sm_tree(SM_ROW), &port));
        assert!(msg.contains("power cell differs"), "{msg}");
    }

    /// A per-unit voltage is compared at `v_rel`/`v_abs`, not at the power band
    /// its neighbours use.
    #[test]
    fn a_voltage_cell_is_compared_at_the_voltage_band() {
        // 2e-6 on `Min Voltage`, against v_abs 1e-6 + v_rel·1 ≈ 1.01e-6.
        let port = ve_tree(&VE_ROW.replace("0.991595541066126", "0.991597541066126"));
        let msg = panic_message(move || compare(&ve_tree(VE_ROW), &port));
        assert!(msg.contains("voltage cell differs"), "{msg}");
        // …and the same cell at 1e-9 passes.
        let port = ve_tree(&VE_ROW.replace("0.991595541066126", "0.991595542066126"));
        compare(&ve_tree(VE_ROW), &port);
    }

    /// A name column is never a number compare: `610` and `610.0` are different
    /// buses even though they are the same float.
    #[test]
    fn the_text_class_never_compares_two_names_as_numbers() {
        let port = ve_tree(&VE_ROW.replace(", 610, 610", ", 610.0, 610"));
        let msg = panic_message(move || compare(&ve_tree(VE_ROW), &port));
        assert!(msg.contains("name differs"), "{msg}");
        // …while the ASCII case of a real name does not matter (the two oracles
        // spell element names differently — G1.10a's fold).
        let oracle = ve_tree(&VE_ROW.replace(", 610, 610", ", Bus_X, 610"));
        let port = ve_tree(&VE_ROW.replace(", 610, 610", ", BUS_x, 610"));
        compare(&oracle, &port);
    }

    /// A column the table does not cover fails the case — the rule that makes
    /// "no default band" true rather than aspirational.
    #[test]
    fn an_unclassified_column_fails() {
        let header = format!("{SM_HEADER}, \"Bogus\"");
        let row = format!("{SM_ROW}, 1.0");
        let t = tree(&[(SM_NAME, file(&[&header, &row]))]);
        let msg = panic_message({
            let t = t.clone();
            move || compare(&t, &t)
        });
        assert!(msg.contains("class table"), "{msg}");
        assert!(msg.contains("SystemMeter"), "{msg}");
    }

    /// A renamed register fails too: the register block's class vector is
    /// positional over `RegisterNames`, so a column that moved must never
    /// inherit its neighbour's band.
    #[test]
    fn a_renamed_register_fails() {
        let name = "case/di_yr_1/feeder_1.csv";
        let good = register_tree(name, "\"Hour\"", "1", 4.5);
        let bad = tree(&[(name, good[name].replace("\"kvarh\"", "\"kvar-hours\""))]);
        let msg = panic_message(move || compare(&bad, &bad.clone()));
        assert!(msg.contains("register 2"), "{msg}");
    }

    /// The totals file a run opens before any meter exists carries its index
    /// column ALONE (r4133 `:3528-3530`, `:3603-3606`), and its 68 data fields
    /// are still classified — positionally.
    #[test]
    fn a_register_file_without_names_in_its_header_is_still_classified() {
        let name = "case/di_yr_1/di_totals_1.csv";
        let text = file(&["Time", &register_row("1", 4.5)]);
        let oracle = tree(&[(name, text.clone())]);
        compare(&oracle, &oracle.clone());
        // The positional table is still in force: `Max kW` (register 3) is a
        // power, so 1e-3 on it reds.
        let mut fields: Vec<String> = register_row("1", 4.5)
            .split(',')
            .map(|f| f.trim().to_string())
            .collect();
        fields[3] = "4.501".to_string();
        let port = tree(&[(name, file(&["Time", &fields.join(", ")]))]);
        let msg = panic_message(move || compare(&oracle, &port));
        assert!(msg.contains("power cell differs"), "{msg}");
    }

    /// An exclusion drops that column's CELLS and nothing else — and it is
    /// recorded, so the ledger's fail-on-stale rule can see it fired.
    #[test]
    fn an_excluded_column_is_still_counted_as_a_ledger_hit() {
        let before = di_excluded_columns().len();
        let oracle = sm_tree(SM_ROW);
        let port = sm_tree(&SM_ROW.replacen("1942.0025609665", "1942.0125609665", 2));
        let excluded = |key: &str| key == format!("{SM_NAME}:peak kw");
        compare_di(
            "capi_v0145",
            Some(&oracle),
            &port,
            &tol(),
            &excluded,
            "unit:di-excluded",
        );
        let hits = di_excluded_columns();
        assert!(hits.len() > before, "the exclusion recorded no hit");
        assert!(
            hits.iter()
                .any(|(label, key)| label == "unit:di-excluded"
                    && key == &format!("{SM_NAME}:peak kw")),
            "{hits:?}"
        );
    }

    /// …and it can never absorb a structural difference: the header, the row
    /// count and the file set are compared whatever the ledger says.
    #[test]
    fn an_exclusion_never_reaches_the_header_or_the_row_count() {
        let excluded = |_: &str| true;
        let oracle = tree(&[(SM_NAME, file(&[SM_HEADER, SM_ROW, SM_ROW]))]);
        let port = sm_tree(SM_ROW);
        let msg = panic_message(move || {
            compare_di(
                "capi_v0145",
                Some(&oracle),
                &port,
                &tol(),
                &excluded,
                "unit:di-excluded-all",
            );
        });
        assert!(msg.contains("data row count differs"), "{msg}");
    }

    /// The presence rail: the flag is on, the channel sent nothing ⇒ the case
    /// fails instead of comparing nothing.
    #[test]
    fn an_absent_capture_fails_the_case() {
        let port = sm_tree(SM_ROW);
        let msg = panic_message(move || {
            compare_di(
                "r4133",
                None,
                &port,
                &tol(),
                &nothing_excluded,
                "unit:di-absent",
            );
        });
        assert!(msg.contains("compare_di"), "{msg}");
        assert!(msg.contains("absent"), "{msg}");
    }

    /// An EMPTY tree is a legitimate answer on both sides — a deck may issue
    /// `CloseDI` without ever enabling `DemandInterval` — and it is NOT counted
    /// as a case with a tree.
    #[test]
    fn an_empty_tree_compares_and_is_not_counted_as_a_di_case() {
        let empty = DiTree::new();
        compare_di(
            "capi_v0145",
            Some(&empty),
            &empty,
            &tol(),
            &nothing_excluded,
            "unit:di-empty",
        );
        assert!(!di_tree_cases().iter().any(|c| c == "unit:di-empty"));
    }

    /// The shape census records what the class table actually classified, which
    /// is what makes D42(1)'s exhaustiveness statement a claim about the live
    /// population: `column_specs` refuses an unrecognised column, so a shape
    /// that reached this set was classified column by column.
    #[test]
    fn the_shape_census_records_every_file_kind_it_met() {
        let sm = sm_tree(SM_ROW);
        compare(&sm, &sm);
        let reg = register_tree("case/di_yr_1/feeder_1.csv", "\"Hour\"", "1", 4.5);
        compare(&reg, &reg);
        let bare = tree(&[(
            "case/di_yr_1/di_totals_1.csv",
            file(&["Time", &register_row("1", 4.5)]),
        )]);
        compare(&bare, &bare.clone());
        let shapes = di_shapes();
        for want in [
            (
                "di_systemmeter".to_string(),
                SYSTEM_METER_COLUMNS.len(),
                true,
            ),
            ("feeder".to_string(), 1 + NUM_EM_REGISTERS, true),
            ("di_totals".to_string(), 1 + NUM_EM_REGISTERS, false),
        ] {
            assert!(shapes.contains(&want), "{want:?} not in {shapes:?}");
        }
    }

    /// The census counts what was actually compared — the non-vacuity half.
    #[test]
    fn the_census_counts_files_and_cells() {
        let before = di_census();
        let t = sm_tree(SM_ROW);
        compare_di(
            "capi_v0145",
            Some(&t),
            &t,
            &tol(),
            &nothing_excluded,
            "unit:di-census",
        );
        // The counters are process-global and the other drives in this module
        // run concurrently, so the statement is a lower bound: what THIS call
        // compared was counted. That is the whole non-vacuity claim - a
        // comparator that compared nothing cannot move them at all.
        let after = di_census();
        assert!(after.comparisons > before.comparisons);
        assert!(after.files > before.files);
        assert!(
            after.cells >= before.cells + SYSTEM_METER_COLUMNS.len(),
            "{} cells counted for a {}-column file",
            after.cells - before.cells,
            SYSTEM_METER_COLUMNS.len(),
        );
        assert!(di_tree_cases().iter().any(|c| c == "unit:di-census"));
    }

    /// The three fixed-shape tables reproduce the header literal the engines
    /// write, byte for byte — so "the table is the engine's own layout" is
    /// checked, not asserted in prose.
    #[test]
    fn the_static_column_tables_spell_the_headers_the_engines_write() {
        // r4133 `Meters/EnergyMeter.pas:3430-3431` (`Create_Meter_Space('"Hour", ')`
        // + the seven names).
        let sm: Vec<String> = SYSTEM_METER_COLUMNS
            .iter()
            .map(|(n, _)| header_name(n))
            .collect();
        assert_eq!(
            sm.join(", "),
            "hour, kwh, kvarh, peak kw, peak kva, losses kwh, losses kvarh, peak losses kw"
        );
        // `:3849`.
        let ov: Vec<String> = OVERLOAD_COLUMNS
            .iter()
            .map(|(n, _)| header_name(n))
            .collect();
        assert_eq!(
            ov.join(", "),
            "hour, element, normal amps, emerg amps, % normal, % emerg, kvbase, \
             i1(a), i2(a), i3(a)"
        );
        // `:3862-3863`.
        let ve: Vec<String> = VOLT_EXCEPTION_COLUMNS
            .iter()
            .map(|(n, _)| header_name(n))
            .collect();
        assert_eq!(
            ve.join(", "),
            "hour, undervoltages, min voltage, overvoltage, max voltage, min bus, \
             max bus, lv undervoltages, min lv voltage, lv overvoltage, \
             max lv voltage, min lv bus, max lv bus"
        );
    }

    /// The register half of the same statement, driven against a LIVE engine:
    /// the positional table is the port's own `RegisterNames`, not a copy that
    /// can drift (coordinator decision D42(1)).
    #[test]
    fn the_fixed_register_table_is_the_ports_own_register_names() {
        let mut dss = Dss::new();
        dss.command("clear");
        dss.command("new circuit.di_class_table basekv=12.47 phases=3");
        dss.command("new line.l1 bus1=sourcebus bus2=b2 length=1");
        dss.command("new energymeter.m element=line.l1 term=1");
        dss.command("solve");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        let regs = dss
            .meter_registers("m")
            .expect("the deck defines EnergyMeter.m");
        assert_eq!(
            regs.len(),
            NUM_EM_REGISTERS,
            "the DI class table is indexed by register position"
        );
        for (r, (name, _)) in regs.iter().enumerate() {
            if let Some((declared, _)) = FIXED_REGISTERS.get(r) {
                assert_eq!(
                    name, declared,
                    "register {r} is called {name:?} on the engine and {declared:?} \
                     in the DI class table — the table is positional over \
                     `RegisterNames`, so it must be re-derived, not patched"
                );
            }
            assert!(
                register_name_matches(r, &name.to_ascii_lowercase()),
                "register {r} ({name:?}) matches no arm of the DI class table"
            );
        }
    }
}

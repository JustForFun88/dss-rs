//! `GOLDEN_REBASE_PLAN.md` G1.10b — the per-report **CELL** comparison of the
//! run-produced report files (spec `tmp/g110b/spec.md` §2.3, coordinator
//! decision **D40**).
//!
//! G1.10a compares the created-file **set**; micro-part F1 of this sub-step
//! carries the selected members' **bytes** to the gate
//! ([`super::run_files::compare_run_file_contents`], which returns the matched
//! `(name, oracle, port)` triples). This module is what turns those triples into
//! a comparison: it gives every compared report kind a **column map** — per
//! column the quantity it carries and the Pascal `Format` string that renders
//! it, each cited to the r4133 writer line — and compares a cell by the D40(1)
//! rule
//!
//! ```text
//! |a − b| ≤ floor(quantity class, tol_for(&case.kind)) + ulp(print format, max(|a|,|b|))
//! ```
//!
//! **Neither term is a free parameter, and this module introduces no constant.**
//! The first term is the case's own calibrated in-memory floor
//! ([`Tolerances`], derived in `tests/TOLERANCE_NOTES.md` and used by the live
//! `element` / `bus` / `voltages` comparators on the very same numbers); the
//! second is exact arithmetic from the report's print format — `%N.df` resolves
//! to `10^-d`, `%N.sg` to `10^(1-s)·10^floor(log10 m)`. Two engines whose
//! underlying f64s agree inside the floor can still render one step apart when
//! the shared value straddles a rounding boundary, and one step is exactly what
//! the ulp term admits. The derivation, the measured worst cell per class and
//! the pins that carry them live in `tests/TOLERANCE_NOTES.md`
//! §"G1.10b run-file contents".
//!
//! **Live-only** (D40(2) guard rail 1): `tests/golden_reports.rs` keeps comparing
//! the committed golden bytes with its own exact [`ExportPolicy`] values, lifted
//! unchanged into [`super::export_policies`]; every kind here names the policy
//! its golden names, so "same report, same policy" is a compile-time fact
//! ([`ReportKind::golden_policy`]) — but the rule above is this surface's, and no
//! golden band moves either way.
//!
//! **What is NOT compared** and why is declared once in
//! [`CONTENTS_NOT_SELECTED`]; the column sets that are declined inside a
//! compared file (the FPC-vs-Delphi `%-.g` trace columns, D40(3)) are declared
//! as [`PrintFmt::DeclinedG`] in the column map itself and counted by the
//! census, never silently skipped.
//!
//! One kind's ROW COUNT is not an equality either. The Storage `DebugTrace` is
//! a log the element appends to on every terminal-current read, so the gate's
//! own capture is part of the file: the rule is stated at
//! [`ReportKind::accounts_readback_tail`] and the measured tail is reported to
//! the caller ([`CellTally::trace_tail`], [`trace_tail_census`]) for the
//! scheduler's fail-on-stale `TRACE_READBACK_RECORDS` epilogue — coordinator
//! decision **D43(1)**.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::run_files::MatchedRunFile;
use super::{
    ExportPolicy, Tolerances, export_policies, polar_angle_band, report_lines, split_fields,
    wrapped_deg,
};

// ---------------------------------------------------------------------------
// Quantity classes and print formats.
// ---------------------------------------------------------------------------

/// What a column carries — and with it, which calibrated in-memory floor bounds
/// the two engines' underlying numbers before the report rounds them.
///
/// Every arm names the unit the *report* prints in, because a floor read in the
/// wrong unit is a silent widening: `Export Voltages`' magnitude is in volts
/// (r4133 `Common/ExportResults.pas:288`, `Vmag := Cabs(Volts)`) exactly like
/// the `NodeV` the live voltage comparator bands, while `Export Powers` prints
/// kW/kvar (`:1113`, `S.re*0.001`) exactly like the `Powers` capture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quantity {
    /// An identifier, mode word or empty padding cell: compared
    /// case-insensitively as text, never as a number (the `field_eq` text arm).
    Text,
    /// A count, index, node number or flag: compared **exactly**, no floor and
    /// no print slack (it is an integer on both sides or the report layout
    /// changed).
    Integer,
    /// Volts. Floor `v_abs + v_rel·m`.
    Voltage,
    /// Amperes. Floor `i_abs + i_rel·m`.
    Current,
    /// kW / kvar. Floor `i_abs + i_rel·m`.
    ///
    /// The in-memory power band is voltage-scaled
    /// (`harness::assert_power_close`: `i_abs·max(|V_kv|,1) + i_rel·|S|`); a
    /// report row carries no paired terminal voltage, so this uses `|V_kv| = 1`
    /// — the **tighter** end of that band. Tightening is always safe; the
    /// `%11.1f` print ulp dominates it on every compared power column anyway.
    ///
    /// The unit is part of the class: `i_abs·1 kV` **is** an absolute term in
    /// kW, so a column that prints MW carries [`Quantity::PowerMega`] instead —
    /// reading a kW-calibrated `abs` against MW numbers would be a silent
    /// **1000× widening**, not a tightening.
    Power,
    /// MW / Mvar — [`Quantity::Power`] with its absolute term converted to the
    /// printed unit: floor `i_abs/1000 + i_rel·m`.
    ///
    /// The relative term is scale-free, so only `abs` moves. The two columns
    /// that need it are the Storage trace's `Qnominalperphase`/
    /// `Pnominalperphase`, which r4133 writes as `(…*3.0/1.0e6):8:2`
    /// (`PCElements/Storage.pas:2418-2419`) — i.e. MW at two decimals, where the
    /// print ulp (0.01 MW = 10 kW) dominates both the kW-calibrated `1e-5` and
    /// this converted `1e-8` by orders of magnitude. Nothing observable turns on
    /// the conversion today; it is here so the class always names the unit its
    /// floor was calibrated in.
    PowerMega,
    /// Siemens. Floor `y_abs + y_rel·m`.
    Admittance,
    /// Per-unit voltage. Floor `v_rel·m`, with **no** absolute term: `v_abs` is
    /// in volts, so in per-unit it is `v_abs/(1000·kVbase)` — below `1.5e-10` pu
    /// at every corpus base, orders under the `%9.5g`/`%.6g` print ulp that
    /// carries this column. Dropping it rather than converting it with a base
    /// the comparator does not have keeps the band honest and tight.
    Pu,
    /// A zone-build geometric constant (km, `DistFromMeter`). Floor `0`: it is
    /// not a solve output — both engines derive it from the same deck lengths —
    /// so only the print ulp separates the two renders.
    Distance,
    /// An EnergyMeter-style register accumulation. Floor
    /// `energy_abs + energy_rel·m` (the integration policy, PORTING_PLAN §4).
    Energy,
    /// A cell that re-prints a quantity some other live comparator already pins
    /// **exactly** — the bus `kVBase` of `Export Voltages`' second column, which
    /// `harness::compare_bus` compares with `rel = abs = 0` (G1.4a, and D11's
    /// `float_roundtrip` is what made that exact compare possible). Floor `0`,
    /// print ulp only.
    ExactElsewhere,
    /// The ARGUMENT, in degrees, of the magnitude `mag_back` columns earlier.
    ///
    /// Not banded like a value: the angle of a phasor whose magnitude is known
    /// only to `±allowed` is free within `±rad2deg·allowed/|I|`
    /// ([`polar_angle_band`]), and completely free once `|I| ≤ allowed` — where
    /// the two engines legitimately report angles up to 180° apart for the same
    /// cancellation residual. The magnitude used is `max(|oracle|, |port|)`,
    /// which makes the rule **two-sided** (D40(1)) and supersedes the golden's
    /// one-sided `GateSpec::PrevCol(1e-6)` on this surface: that gate reads the
    /// ORACLE's magnitude only and deliberately never fires on an exact `0`, so
    /// it cannot see the class where the oracle prints `0` and the port a
    /// `1e-11 A` residual.
    Angle { mag_back: usize },
}

/// The Pascal `Format` spec a column is rendered with — the *only* source of
/// this module's print-resolution term.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrintFmt {
    /// `%N.df` / the `:w:d` write form — `d` digits after the point.
    /// `ulp = 10^-d`.
    Fixed(i32),
    /// `%N.sg` — `s` significant digits. `ulp = 10^(1-s)·10^floor(log10 m)`.
    Sig(i32),
    /// `%d`, `:0`, `:3` or a text field: no print slack at all.
    Exact,
    /// FPC `Format('%-.g', …)` — the empty precision FPC reads as **2**
    /// significant digits and the Delphi-built r4133 DLL as ~15
    /// (r4133 `PCElements/Storage.pas:2401-2429`; the port follows FPC, see
    /// `dss_core::elements::pc::storage::trace::TRACE_G_SIG`).
    ///
    /// **The two gating ORACLES disagree with each other here**, measured:
    /// `9999` (r4133) vs `1E4` (capi and the port), `250` vs `2.5E2`,
    /// `1.32348898008484E-026` vs `6.6E-27`. At two significant digits any
    /// numeric band would be vacuous (a 2 % straddle: capi `5.5` vs the port's
    /// `5.4` for `kWTotalLosses`), so the column SET is **declined on both
    /// channels** with a census row and a both-numbers pin — never a tolerance
    /// (coordinator decision **D40(3)**). WP-G4 **G4.1** tears the `fmt_g`
    /// kernel down; once the port prints at r4133's precision the decline is
    /// re-measured and shrinks to the capi channel.
    DeclinedG,
}

impl PrintFmt {
    /// One unit in the last printed place, at magnitude `m`.
    ///
    /// Two renders of the *same* f64 differ by at most one such unit when the
    /// value straddles the format's rounding boundary (`%.2f`: `0.005−ε` →
    /// `0.00`, `0.005+ε` → `0.01`), so one full unit is both sufficient and
    /// minimal. `m` is `max(|a|,|b|)`, which puts a pair spanning a decade
    /// boundary on the larger value's last place — the larger of the two units.
    ///
    /// An exact `0` on one side needs no special case: at `m = 0` both sides are
    /// `0` and the difference is `0`; at `m > 0` the class floor's `abs` term is
    /// what admits a residual against an exact zero.
    fn ulp(self, m: f64) -> f64 {
        match self {
            PrintFmt::Fixed(d) => 10f64.powi(-d),
            PrintFmt::Sig(s) => {
                if m == 0.0 || !m.is_finite() {
                    0.0
                } else {
                    10f64.powi(1 - s + m.abs().log10().floor() as i32)
                }
            }
            PrintFmt::Exact | PrintFmt::DeclinedG => 0.0,
        }
    }
}

/// One column of a report: what it carries, how it is printed, and the name the
/// writer gives it (used verbatim in failure messages).
#[derive(Clone, Copy, Debug)]
pub struct Col {
    pub name: &'static str,
    pub q: Quantity,
    pub f: PrintFmt,
}

const fn col(name: &'static str, q: Quantity, f: PrintFmt) -> Col {
    Col { name, q, f }
}

/// The magnitude column of a paired magnitude/angle report sits immediately
/// before its angle in every r4133 writer this module maps
/// (`', %10.6g, %8.2f'` at `Common/ExportResults.pas:471`,
/// `', %d, %10.6g, %6.1f, %9.5g'` at `:288`).
const ANGLE: Quantity = Quantity::Angle { mag_back: 1 };

// ---------------------------------------------------------------------------
// The report kinds and their column maps.
// ---------------------------------------------------------------------------

/// A compared report kind: the created-file name pattern that selects it
/// ([`dss_epri::guard::RUN_FILE_CONTENTS_PATTERNS`]), the golden
/// [`ExportPolicy`] whose structure it rides, and its column map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportKind {
    /// `<CKT>_EXP_CURRENTS.CSV` — r4133 `Common/ExportResults.pas:520`.
    Currents,
    /// `<CKT>_EXP_POWERS.CSV` — r4133 `:1069`.
    Powers,
    /// `<CKT>_EXP_VOLTAGES.CSV` — r4133 `:236`.
    Voltages,
    /// `<CKT>_EXP_Profile.CSV` — r4133 `:3371`.
    Profile,
    /// `<CKT>_EXP_Y.CSV` written by the **dense** arm of `ExportY`
    /// (r4133 `:3061-3097`) — the default `Export Y`.
    YDense,
    /// `<CKT>_EXP_Y.CSV` written by the **triplet** arm (`:3052-3060`,
    /// `Export Y triplet`). Same file name, different layout: the two are told
    /// apart by the first line ([`ReportKind::of`]).
    YTriplet,
    /// `<CKT>_EXP_YPRIM.CSV` — r4133 `:2850`.
    Yprim,
    /// `EXP_PV_<NAME>.CSV` — the `/m` arm of `Export PVSystem`, r4133 `:1990`.
    Register,
    /// `STOR_<NAME>.CSV` — the Storage `DebugTrace`, r4133
    /// `PCElements/Storage.pas:1073-1085` (header) and `:2401-2429` (record).
    /// Not an export report and therefore the one kind with no golden policy.
    StorageTrace,
}

impl ReportKind {
    /// Every kind, for the census and the completeness tests.
    pub const ALL: [ReportKind; 9] = [
        ReportKind::Currents,
        ReportKind::Powers,
        ReportKind::Voltages,
        ReportKind::Profile,
        ReportKind::YDense,
        ReportKind::YTriplet,
        ReportKind::Yprim,
        ReportKind::Register,
        ReportKind::StorageTrace,
    ];

    /// The selection pattern this kind answers to — the same string the gate
    /// ships to both oracle transports, so a kind whose pattern left
    /// [`dss_epri::guard::RUN_FILE_CONTENTS_PATTERNS`] would be a kind whose
    /// bytes silently stop travelling (pinned by
    /// [`tests::every_selection_pattern_has_a_kind_and_every_kind_a_pattern`]).
    pub fn pattern(self) -> &'static str {
        match self {
            ReportKind::Currents => "*_exp_currents.csv",
            ReportKind::Powers => "*_exp_powers.csv",
            ReportKind::Voltages => "*_exp_voltages.csv",
            ReportKind::Profile => "*_exp_profile.csv",
            ReportKind::YDense | ReportKind::YTriplet => "*_exp_y.csv",
            ReportKind::Yprim => "*_exp_yprim.csv",
            ReportKind::Register => "exp_pv_*.csv",
            ReportKind::StorageTrace => "stor_*.csv",
        }
    }

    /// A short stable label for census rows and failure messages.
    pub fn label(self) -> &'static str {
        match self {
            ReportKind::Currents => "currents",
            ReportKind::Powers => "powers",
            ReportKind::Voltages => "voltages",
            ReportKind::Profile => "profile",
            ReportKind::YDense => "y(dense)",
            ReportKind::YTriplet => "y(triplet)",
            ReportKind::Yprim => "yprim",
            ReportKind::Register => "register",
            ReportKind::StorageTrace => "storage-trace",
        }
    }

    /// The golden [`ExportPolicy`] this kind shares with its byte golden — the
    /// compile-time half of "the live compare uses the same policy as the
    /// golden". The live comparator reads the policy's `sep` and `header_lines`
    /// out of it (never its own literals) and compares values by the D40(1)
    /// cell rule instead of the golden's `rel`/`abs`, which stay pinned to the
    /// committed bytes.
    ///
    /// `None` for [`ReportKind::StorageTrace`]: the Storage `DebugTrace` is not
    /// an `Export` report and has no golden at all, so its two structural
    /// parameters are declared in [`ReportKind::structure`] with their r4133
    /// citation.
    pub fn golden_policy(self) -> Option<ExportPolicy> {
        Some(match self {
            ReportKind::Currents => export_policies::currents_policy(),
            ReportKind::Powers => export_policies::powers_policy(),
            ReportKind::Voltages => export_policies::voltages_policy(),
            ReportKind::Profile => export_policies::profile_policy(),
            // The dense arm has no golden of its own; it rides the triplet
            // golden's structure (both are `ExportY`, one header line, ordered
            // rows) and supplies its own column map.
            ReportKind::YDense | ReportKind::YTriplet => export_policies::y_triplet_policy(),
            ReportKind::Yprim => export_policies::yprims_policy(),
            ReportKind::Register => export_policies::register_policy(),
            ReportKind::StorageTrace => return None,
        })
    }

    /// `(separator, header lines)` — read off [`ReportKind::golden_policy`]
    /// wherever there is one, so the live surface cannot drift from the golden's
    /// framing of the same file.
    ///
    /// `pub` because the expected-value pins
    /// (`crates/dss-core/tests/run_file_contents_pins.rs`) slice and edit a live
    /// report by (data row, field) and must frame it exactly as the comparator
    /// does; a second copy of the framing there would be a second source of
    /// truth.
    pub fn structure(self) -> (char, usize) {
        match self.golden_policy() {
            Some(p) => (p.sep, p.header_lines),
            // r4133 `PCElements/Storage.pas:1073-1085`: one header line written
            // at edit time, records appended with `', '` separators (`:2411`).
            None => (',', 1),
        }
    }

    /// Whether this kind's data row count is a property of the RUN alone, or
    /// also of the READER — coordinator decision **D43(1)**.
    ///
    /// An `Export` report is written once, in full, by the command that names
    /// it: its row count is an equality, and a differing one is a finding. The
    /// Storage `DebugTrace` is not a report but a log the element appends to,
    /// and `WriteTraceRecord` sits at the END of `GetTerminalCurrents`
    /// **outside** the `IterminalSolutionCount <> SolutionCount` cache test
    /// (r4133 `PCElements/Storage.pas:2874`, the test at `:2868-2871`; capi
    /// 0.14.5 `:2356`, test at `:2348-2353`) — so *every* terminal-current read
    /// appends a record, cached or not, solving or merely reporting.
    ///
    /// The two oracle transports read the element's terminal quantities six
    /// times after the last solve (the A/B/C capture partition, `capture_order`)
    /// and therefore append six tail records; the port's
    /// `dss_core::exec::view::snapshot_elements` recomputes ONCE at the
    /// converged `NodeV` (GOLDEN_REBASE **G2.3**, the Newton stale-`Iterminal`
    /// fix) and touches it twice. Measured live on `Storage_price.dss`, both
    /// channels: 96 identical solve records on all three engines, oracle 102
    /// rows vs port 98 (`tmp/g110b/f_F2_unexpected.md`). The gap is the
    /// footprint of the gate's own reader and the port's BETTER behaviour is
    /// what produces it, so equality would gate the capture, not the engine.
    ///
    /// For such a kind [`compare_one`] asserts `port_rows <= oracle_rows`,
    /// compares every cell of the first `port_rows` records under the D40(1)
    /// rule, and REPORTS `oracle_rows − port_rows` to the caller
    /// ([`CellTally::trace_tail`], [`trace_tail_census`]), where the scheduler
    /// epilogue holds it against the re-derived population constant
    /// `TRACE_READBACK_RECORDS` fail-on-stale in both directions. Positive
    /// accounting of the reader's footprint — never a skipped prefix, never a
    /// data-derived boundary heuristic, never a band.
    fn accounts_readback_tail(self) -> bool {
        matches!(self, ReportKind::StorageTrace)
    }

    /// Which report a selected member carries, from its normalized name and its
    /// first line.
    ///
    /// The first line is needed only for `*_exp_y.csv`: `ExportY` writes the
    /// same file name for both of its arms and they share nothing else — the
    /// triplet arm opens with the literal `Row,Col,G,B`
    /// (r4133 `Common/ExportResults.pas:3052`), the dense arm with the node
    /// count (`:3069`).
    pub fn of(name: &str, first_line: &str) -> Option<ReportKind> {
        for k in ReportKind::ALL {
            if !dss_epri::guard::selects_contents(&[k.pattern()], name) {
                continue;
            }
            return Some(match k {
                ReportKind::YDense | ReportKind::YTriplet => {
                    if first_line.trim().eq_ignore_ascii_case("Row,Col,G,B") {
                        ReportKind::YTriplet
                    } else {
                        ReportKind::YDense
                    }
                }
                other => other,
            });
        }
        None
    }
}

/// A resolved layout: the fixed leading columns, an optional repeating group
/// that covers everything after them, and the structural facts the row loop
/// checks before it looks at a value.
struct Layout {
    head: Vec<Col>,
    /// Empty = the row ends with `head`; otherwise field `j >= head.len()` is
    /// `group[(j − head.len()) % group.len()]`.
    group: Vec<Col>,
    /// The writer ends every data line with the separator, so splitting yields
    /// one trailing empty field. Asserted empty on BOTH sides and not compared.
    trailing_empty: bool,
    /// The field that identifies the row (element / bus name), checked before
    /// any value so a failure names the row rather than a column index.
    id_col: Option<usize>,
    /// `Export Yprims` interleaves single-field `Class.NAME` lines with matrix
    /// rows (r4133 `:2873`/`:2876`); a one-field row is the name line.
    name_rows: bool,
}

impl Layout {
    fn col_at(&self, j: usize) -> Option<Col> {
        if j < self.head.len() {
            Some(self.head[j])
        } else if self.group.is_empty() {
            None
        } else {
            Some(self.group[(j - self.head.len()) % self.group.len()])
        }
    }
}

/// `Export Currents`: `Element` then, per terminal, `CondWidth` magnitude/angle
/// pairs plus the terminal residual pair — every column after the name
/// alternates `%10.6g` magnitude / `%8.2f` angle (r4133
/// `Common/ExportResults.pas:458` `CalcAndWriteCurrents`, the four
/// `', %10.6g, %8.2f'` writes at `:471`, `:474`, `:475`, `:480`; header
/// `:551-555`).
fn currents_layout() -> Layout {
    Layout {
        head: vec![col("Element", Quantity::Text, PrintFmt::Exact)],
        group: vec![
            col("|I| (A)", Quantity::Current, PrintFmt::Sig(6)),
            col("Ang (deg)", ANGLE, PrintFmt::Fixed(2)),
        ],
        trailing_empty: false,
        id_col: Some(0),
        name_rows: false,
    }
}

/// `Export Powers`: `"Class.NAME", Terminal` then `P, Q` in `:11:1`, and on a
/// PD element's terminal 1 four more `:11:1` excess-kVA columns (r4133
/// `Common/ExportResults.pas:1069`, header `:1095`, values `:1113`/`:1114` and
/// `:1119-1126`). PC rows and terminals `≥ 2` are four fields wide, which the
/// fixed head covers by being longer than the row.
fn powers_layout() -> Layout {
    Layout {
        head: vec![
            col("Element", Quantity::Text, PrintFmt::Exact),
            col("Terminal", Quantity::Integer, PrintFmt::Exact),
            col("P", Quantity::Power, PrintFmt::Fixed(1)),
            col("Q", Quantity::Power, PrintFmt::Fixed(1)),
            col("P_Normal", Quantity::Power, PrintFmt::Fixed(1)),
            col("Q_Normal", Quantity::Power, PrintFmt::Fixed(1)),
            col("P_Emergency", Quantity::Power, PrintFmt::Fixed(1)),
            col("Q_Emergency", Quantity::Power, PrintFmt::Fixed(1)),
        ],
        group: vec![],
        trailing_empty: false,
        id_col: Some(0),
        name_rows: false,
    }
}

/// `Export Voltages`: `"BUS", BasekV` then one `Node, Magnitude, Angle, pu`
/// quadruple per node, zero-filled to the widest bus (r4133
/// `Common/ExportResults.pas:236`, header `:265-266`, bus row `:272`, quadruple
/// `:288`, zero fill `:291`).
///
/// `BasekV` is `kvbase·SQRT3` — a bus attribute `harness::compare_bus` already
/// pins EXACTLY on the same case, hence [`Quantity::ExactElsewhere`].
fn voltages_layout() -> Layout {
    Layout {
        head: vec![
            col("Bus", Quantity::Text, PrintFmt::Exact),
            col("BasekV", Quantity::ExactElsewhere, PrintFmt::Sig(5)),
        ],
        group: vec![
            col("Node", Quantity::Integer, PrintFmt::Exact),
            col("Magnitude (V)", Quantity::Voltage, PrintFmt::Sig(6)),
            col("Angle (deg)", ANGLE, PrintFmt::Fixed(1)),
            col("pu", Quantity::Pu, PrintFmt::Sig(5)),
        ],
        trailing_empty: false,
        id_col: Some(0),
        name_rows: false,
    }
}

/// `Export Profile`: `Name, Distance1, puV1, Distance2, puV2` in `%.6g` then the
/// seven integer plot columns (r4133 `Common/ExportResults.pas:3371`, header
/// `:3391`, row `:3356` `WriteNewLine` — `:3363` and `:3364-3366`).
fn profile_layout() -> Layout {
    Layout {
        head: vec![
            col("Name", Quantity::Text, PrintFmt::Exact),
            col("Distance1 (km)", Quantity::Distance, PrintFmt::Sig(6)),
            col("puV1", Quantity::Pu, PrintFmt::Sig(6)),
            col("Distance2 (km)", Quantity::Distance, PrintFmt::Sig(6)),
            col("puV2", Quantity::Pu, PrintFmt::Sig(6)),
            col("Color", Quantity::Integer, PrintFmt::Exact),
            col("Thickness", Quantity::Integer, PrintFmt::Exact),
            col("Linetype", Quantity::Integer, PrintFmt::Exact),
            col("Markcenter", Quantity::Integer, PrintFmt::Exact),
            col("Centercode", Quantity::Integer, PrintFmt::Exact),
            col("NodeCode", Quantity::Integer, PrintFmt::Exact),
            col("NodeWidth", Quantity::Integer, PrintFmt::Exact),
        ],
        group: vec![],
        trailing_empty: false,
        id_col: Some(0),
        name_rows: false,
    }
}

/// `Export Y` (dense): a node-count header line, then one row per node —
/// `"BUS.N"` and `NumNodes` `re, +j im` cells in `%-13.10g`, the line ending
/// with the separator (r4133 `Common/ExportResults.pas:3069`, `:3078`, `:3090`).
fn y_dense_layout() -> Layout {
    Layout {
        head: vec![col("Node", Quantity::Text, PrintFmt::Exact)],
        group: vec![
            col("G (S)", Quantity::Admittance, PrintFmt::Sig(10)),
            col("+j B (S)", Quantity::Admittance, PrintFmt::Sig(10)),
        ],
        trailing_empty: true,
        id_col: Some(0),
        name_rows: false,
    }
}

/// `Export Y triplet`: `Row,Col,G,B` with integer indices and `%.10g` values
/// (r4133 `Common/ExportResults.pas:3052`, `:3059`).
fn y_triplet_layout() -> Layout {
    Layout {
        head: vec![
            col("Row", Quantity::Integer, PrintFmt::Exact),
            col("Col", Quantity::Integer, PrintFmt::Exact),
            col("G (S)", Quantity::Admittance, PrintFmt::Sig(10)),
            col("B (S)", Quantity::Admittance, PrintFmt::Sig(10)),
        ],
        group: vec![],
        trailing_empty: false,
        id_col: None,
        name_rows: false,
    }
}

/// `Export Yprims`: a `Class.NAME` line then `Yorder` rows of `re, im,` pairs in
/// `%-13.10g`, each ending with the separator (r4133
/// `Common/ExportResults.pas:2873`, `:2876`).
fn yprim_layout() -> Layout {
    Layout {
        head: vec![],
        group: vec![
            col("re (S)", Quantity::Admittance, PrintFmt::Sig(10)),
            col("im (S)", Quantity::Admittance, PrintFmt::Sig(10)),
        ],
        trailing_empty: true,
        id_col: None,
        name_rows: true,
    }
}

/// `EXP_PV_<NAME>.CSV`: `Year, LDCurve, Hour, "NAME"` then every PVSystem
/// register in `:10:0` (r4133 `Common/ExportResults.pas:1990`
/// `WriteMultiplePVSystemMeterFiles`, header `:2017`, name `:2029`, registers
/// `:2030`).
///
/// The register columns are classed **by name off the header** the writer
/// emitted (`:2018` quotes `RegisterNames[i]`), not as one repeating group: four
/// of the six PVSystem registers are not energy accumulations (r4133
/// `PCElements/PVsystem.pas:404-409` — `kWh`, `kvarh`, `Max kW`, `Max kVA`,
/// `Hours`, `Price($)`), and `Max kW`/`Max kVA` are maxima of instantaneous
/// power, which the gate bands at `i_abs + i_rel·m` everywhere else — three
/// orders tighter than the accumulator band. Coordinator decision **D42(1)**
/// classes exactly this quantity (`Max/Peak …` powers) at `i_rel`/`i_abs` on the
/// sibling DI surface; this map says the same thing about the same registers.
/// `Hours` and `Price($)` accumulate like `kWh`/`kvarh` and keep the energy
/// class. (G1.10b audit settlement, finding AC1-2; nothing observable moves —
/// the `:10:0` print ulp of 1.0 dominates every reading on the live population.)
///
/// Because the columns come from the header there is no repeating group, so a
/// data row wider than the header it was written under is a loud refusal
/// (`compare_one`'s "outside the column map") instead of being absorbed into a
/// group and judged under a neighbouring column's class.
fn register_layout(header: &[String], ctx: &str) -> Layout {
    let mut head = vec![
        col("Year", Quantity::Integer, PrintFmt::Exact),
        col("LDCurve", Quantity::Text, PrintFmt::Exact),
        col("Hour", Quantity::Integer, PrintFmt::Exact),
        col("PVSystem", Quantity::Text, PrintFmt::Exact),
    ];
    assert!(
        header.len() > head.len(),
        "{ctx}: the PVSystem register header is {} column(s); r4133 writes the \
         four fixed columns (`Common/ExportResults.pas:2017`) and then one per \
         register (`:2018`), so a register file must carry at least five",
        header.len()
    );
    for name in &header[head.len()..] {
        let n = name.trim().trim_matches('"').trim();
        // `Max kW` / `Max kVA` (r4133 `PCElements/PVsystem.pas:406-407`) are
        // maxima of instantaneous power, not accumulations.
        let max_power = n.eq_ignore_ascii_case("Max kW") || n.eq_ignore_ascii_case("Max kVA");
        head.push(if max_power {
            col(
                "Max register (kW / kVA)",
                Quantity::Power,
                PrintFmt::Fixed(0),
            )
        } else {
            col(
                "Register (accumulated)",
                Quantity::Energy,
                PrintFmt::Fixed(0),
            )
        });
    }
    Layout {
        head,
        group: vec![],
        trailing_empty: false,
        id_col: Some(3),
        name_rows: false,
    }
}

/// The Storage `DebugTrace` record (r4133 `PCElements/Storage.pas:2401-2429`),
/// whose width depends on the element: nine fixed columns, then `|Iinj|`,
/// `|Iterm|` and `|Vterm|` per phase in `:8:1`, then one `%-.g` cell per state
/// variable, then the trailing separator.
///
/// The phase and variable counts are read off the **header** the edit-time
/// writer emitted (`:1077-1081`) rather than guessed: the header is compared
/// verbatim between the two sides first, so both agree on it by the time this
/// runs. The header additionally names `Vthev, Theta` (`:1083`) which the record
/// never writes (`:2427` is commented out upstream) — two header columns with no
/// data column, which is why the variable count subtracts them.
///
/// Columns `t` and `LoadMultiplier` (`:2411`) and every variable cell (`:2424`)
/// are `%-.g` → [`PrintFmt::DeclinedG`] (D40(3)).
fn storage_trace_layout(header: &[String], ctx: &str) -> Layout {
    let count = |p: &str| header.iter().filter(|h| h.trim().starts_with(p)).count();
    let (n_inj, n_term, n_vterm) = (count("|Iinj"), count("|Iterm"), count("|Vterm"));
    assert!(
        n_inj > 0 && n_inj == n_term && n_term == n_vterm,
        "{ctx}: the Storage trace header must carry one |Iinj_i|, |Iterm_i| and \
         |Vterm_i| column per phase (r4133 PCElements/Storage.pas:1078-1080); \
         got {n_inj}/{n_term}/{n_vterm} in {header:?}"
    );
    let fixed = 9 + 3 * n_inj;
    assert!(
        header.len() >= fixed + 2,
        "{ctx}: the Storage trace header is {} columns, shorter than the nine \
         fixed + {} phase + the two `Vthev, Theta` names r4133 \
         PCElements/Storage.pas:1077-1083 writes",
        header.len(),
        3 * n_inj
    );
    // `:1083` appends `,Vthev, Theta`; `WriteTraceRecord` never writes them.
    let n_vars = header.len() - fixed - 2;
    let mut head = vec![
        col("t", Quantity::Text, PrintFmt::DeclinedG),
        col("Iteration", Quantity::Integer, PrintFmt::Exact),
        col("LoadMultiplier", Quantity::Text, PrintFmt::DeclinedG),
        col("Mode", Quantity::Text, PrintFmt::Exact),
        col("LoadModel", Quantity::Text, PrintFmt::Exact),
        col("StorageModel", Quantity::Integer, PrintFmt::Exact),
        col(
            "Qnominalperphase (Mvar)",
            Quantity::PowerMega,
            PrintFmt::Fixed(2),
        ),
        col(
            "Pnominalperphase (MW)",
            Quantity::PowerMega,
            PrintFmt::Fixed(2),
        ),
        col("CurrentType", Quantity::Text, PrintFmt::Exact),
    ];
    for _ in 0..n_inj {
        head.push(col("|Iinj| (A)", Quantity::Current, PrintFmt::Fixed(1)));
    }
    for _ in 0..n_term {
        head.push(col("|Iterm| (A)", Quantity::Current, PrintFmt::Fixed(1)));
    }
    for _ in 0..n_vterm {
        head.push(col("|Vterm| (V)", Quantity::Voltage, PrintFmt::Fixed(1)));
    }
    for _ in 0..n_vars {
        head.push(col("Variable", Quantity::Text, PrintFmt::DeclinedG));
    }
    Layout {
        head,
        group: vec![],
        trailing_empty: true,
        id_col: None,
        name_rows: false,
    }
}

// ---------------------------------------------------------------------------
// What is deliberately NOT selected (D40(5)).
// ---------------------------------------------------------------------------

/// The report kinds a run can produce whose contents this surface does **not**
/// compare, each with the reason — a census row, never an ad-hoc comparison
/// (coordinator decision **D40(5)**).
///
/// They never reach [`compare_run_file_cells`] at all: the selection
/// ([`dss_epri::guard::RUN_FILE_CONTENTS_PATTERNS`]) does not name them, so their
/// bytes never leave the case directory. This table is where the reason lives,
/// and [`tests::nothing_declined_is_also_selected`] holds the two halves
/// consistent.
pub const CONTENTS_NOT_SELECTED: [(&str, &str); 8] = [
    (
        "monitor CSV",
        "A6: f32 channel data, and an unsampled monitor is a known oracle artifact \
         (TESTING.md §monitors). Names compared by G1.10a, contents never.",
    ),
    (
        "eventlog CSV",
        "the event log is already compared IN MEMORY on both channels \
         (G1.10a class A, coordinator decision D30(1)); re-comparing its file \
         would gate the same strings twice.",
    ),
    (
        "demand-interval tree (DI_yr_*)",
        "owned by sub-step G1.10c (the `compare_di` manifest flag).",
    ),
    (
        "Show / Dump text reports",
        "the two oracles disagree with EACH OTHER on `MaxBusNameLength` column \
         padding (29 065 differing tokens on one 8500-node file, measured \
         tmp/g110b/STOP.md §6) — the `Show` width class, WP-G4/G2.6 territory.",
    ),
    (
        "NCIM Jacobian / deltaF / deltaZ",
        "no golden `ExportPolicy` exists for them (deltaF/deltaZ are 1e-11-scale \
         residual vectors); a report without a policy is recorded, never \
         compared ad hoc.",
    ),
    (
        "cim100 XML",
        "not a CSV report and has no golden `ExportPolicy`.",
    ),
    (
        "deck-named export (`export ... file=<name>`)",
        "the file name does not identify the report kind, so no column map can \
         be chosen (8 `large*` AutoTrans cases today, none forced). The remedy, \
         if a forced case ever carries one, is a deck-derived command→file-kind \
         map — not a byte compare.",
    ),
    (
        "capacity / ycurrents / ynodelist (golden policy, no column map)",
        "a golden `ExportPolicy` exists for these three \
         (`golden_reports::export_capacity_matches_oracle`, \
         `::export_ycurrents_matches_oracle`, \
         `::export_ynodelist_matches_oracle`), but the \
         only corpus decks that export them are `large*` rows the G1.10a force \
         rule never reaches (`ckt7`, `GFM_AmpsLimit_123`), so no column map was \
         written and none could be MEASURED against the two oracles. Declaring \
         `compare_run_files` on such a row must add the map first — \
         [`tests::every_default_export_name_the_corpus_produces_is_selected_or_declined`] \
         is what makes that a red instead of a silent skip (settlement of the \
         G1.10b code audit, finding AC-4).",
    ),
];

// ---------------------------------------------------------------------------
// The census.
// ---------------------------------------------------------------------------

static FILES: AtomicUsize = AtomicUsize::new(0);
static COMPARED_CELLS: AtomicUsize = AtomicUsize::new(0);
static DECLINED_CELLS: AtomicUsize = AtomicUsize::new(0);
static PER_KIND_FILES: [AtomicUsize; ReportKind::ALL.len()] =
    [const { AtomicUsize::new(0) }; ReportKind::ALL.len()];
static PER_KIND_CELLS: [AtomicUsize; ReportKind::ALL.len()] =
    [const { AtomicUsize::new(0) }; ReportKind::ALL.len()];
static PER_KIND_DECLINED: [AtomicUsize; ReportKind::ALL.len()] =
    [const { AtomicUsize::new(0) }; ReportKind::ALL.len()];

fn kind_idx(k: ReportKind) -> usize {
    ReportKind::ALL
        .iter()
        .position(|x| *x == k)
        .expect("ReportKind::ALL lists every kind")
}

/// What one call compared, returned as well as accumulated into the process
/// census — the process counters are shared by every case the gate runs (and by
/// every unit test in this binary), so an assertion about ONE comparison reads
/// this, never the difference of two [`cell_census`] samples.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CellTally {
    pub files: usize,
    pub compared: usize,
    pub declined: usize,
    /// How many of `files` account a reader read-back tail
    /// ([`ReportKind::accounts_readback_tail`] — today the Storage
    /// `DebugTrace` and nothing else).
    pub trace_files: usize,
    /// The total `oracle_rows − port_rows` over those files: the records the
    /// ORACLE transport's own post-solve element reads appended and the port's
    /// single G2.3 recompute did not (coordinator decision **D43(1)(iii)**).
    ///
    /// One call of [`compare_run_file_cells`] is one (case, channel), so this
    /// is the quantity the scheduler epilogue holds against the re-derived
    /// population constant `TRACE_READBACK_RECORDS`; the per-file detail is in
    /// [`trace_tail_census`].
    pub trace_tail: usize,
}

/// One Storage `DebugTrace` comparison's row accounting — the observation the
/// `TRACE_READBACK_RECORDS` epilogue re-derives its population constant from
/// (**D43(1)(iii)**).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceTail {
    /// `"<case label> [<channel>] <file> contents"` — the (case, channel, file)
    /// this observation belongs to, so a moved gap names the deck that moved it.
    pub ctx: String,
    /// Data rows the gating channel's file holds.
    pub oracle_rows: usize,
    /// Data rows the port's file holds — never more than `oracle_rows`
    /// ([`compare_one`] refuses that outright).
    pub port_rows: usize,
}

impl TraceTail {
    /// `oracle_rows − port_rows`: the reader's footprint on this file.
    pub fn gap(&self) -> usize {
        self.oracle_rows - self.port_rows
    }
}

static TRACE_TAILS: Mutex<Vec<TraceTail>> = Mutex::new(Vec::new());

/// Every read-back tail this process measured, in comparison order.
///
/// The scheduler epilogue reads this to re-derive `TRACE_READBACK_RECORDS`
/// fail-on-stale in BOTH directions (the `SCRATCH_FILE_DECLINES` shape): a gap
/// that grows is a port that stopped writing solve records or an oracle
/// transport that grew an element read; a gap that shrinks to zero is a port
/// that started logging the gate's own capture. Both are visible instead of
/// silent, which is the whole point of accounting the footprint instead of
/// declining the file (D43(1), options B and C refused).
pub fn trace_tail_census() -> Vec<TraceTail> {
    TRACE_TAILS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// `(files, compared cells, declined cells)` over this process so far.
///
/// A comparison census is the only thing that keeps a contents surface from
/// silently comparing nothing — but the GATED census is not this counter: these
/// atomics are process-wide and this binary's own fixtures share them, so the
/// scheduler epilogue re-derives its population from the per-call
/// [`CellTally`] the runner records instead
/// (`corpus_gate::scheduler::assert_run_file_contents_census_is_the_pinned_population`
/// over `RUN_FILE_CONTENTS_COMPARED`/`RUN_FILE_CONTENTS_DECLINES`, fail-on-stale
/// in BOTH directions, the `SCRATCH_FILE_DECLINES` shape). What these two
/// accessors give is this module's own self-check view — used by
/// [`tests::the_census_is_per_kind`] to prove a comparison lands on its own
/// kind's row. (G1.10b audit settlement, finding AT1-4.)
pub fn cell_census() -> (usize, usize, usize) {
    (
        FILES.load(Ordering::Relaxed),
        COMPARED_CELLS.load(Ordering::Relaxed),
        DECLINED_CELLS.load(Ordering::Relaxed),
    )
}

/// The same census split by report kind: `(label, files, compared, declined)`.
pub fn cell_census_by_kind() -> Vec<(&'static str, usize, usize, usize)> {
    ReportKind::ALL
        .iter()
        .enumerate()
        .map(|(i, k)| {
            (
                k.label(),
                PER_KIND_FILES[i].load(Ordering::Relaxed),
                PER_KIND_CELLS[i].load(Ordering::Relaxed),
                PER_KIND_DECLINED[i].load(Ordering::Relaxed),
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The cell rule.
// ---------------------------------------------------------------------------

/// The calibrated in-memory floor for `q` at magnitude `m` — the first term of
/// the D40(1) rule. Every number here comes from [`Tolerances`]; this module
/// defines none of its own.
fn class_floor(q: Quantity, tol: &Tolerances, m: f64) -> f64 {
    match q {
        Quantity::Voltage => tol.v_abs + tol.v_rel * m,
        Quantity::Current | Quantity::Power => tol.i_abs + tol.i_rel * m,
        // The same band read in the unit the column prints: `i_abs` is an
        // absolute term in kW (amps × 1 kV), so in MW it is `i_abs/1000`.
        Quantity::PowerMega => tol.i_abs / 1000.0 + tol.i_rel * m,
        Quantity::Admittance => tol.y_abs + tol.y_rel * m,
        Quantity::Pu => tol.v_rel * m,
        Quantity::Energy => tol.energy_abs + tol.energy_rel * m,
        Quantity::Distance | Quantity::ExactElsewhere | Quantity::Integer => 0.0,
        Quantity::Text | Quantity::Angle { .. } => 0.0,
    }
}

/// `Export Y`'s imaginary cells are written as `+j <number>`
/// (r4133 `Common/ExportResults.pas:3090`), which no `f64` parser accepts — so
/// the golden's text fallback would compare r4133's three-digit exponent
/// `+j 2.220446049E-016` against the port's `+j 2.220446049E-16` as *strings*
/// and fail on a spelling. Strip the marker on both sides and compare the
/// numbers (coordinator decision **D40(4)**).
///
/// Returns `(payload, marker_present)`; the caller asserts the two sides agree
/// on the marker, so dropping it can never hide a layout change.
fn strip_j(field: &str) -> (&str, bool) {
    let t = field.trim();
    match t.strip_prefix("+j").or_else(|| t.strip_prefix("+J")) {
        Some(rest) => (rest.trim_start(), true),
        None => (t, false),
    }
}

/// The verdict for one cell — `Ok` carries whether it counted as compared.
enum Cell {
    Compared,
    Declined,
    /// The cell failed; the string is the measured detail for the panic.
    Fail(String),
}

/// Compare one non-angle cell under the D40(1) rule.
///
/// [`Quantity::Angle`] never arrives here — the row loop dispatches it to
/// [`compare_angle`], which needs the paired magnitude column's class and both
/// of its values.
fn compare_cell(c: Col, tol: &Tolerances, port: &str, oracle: &str) -> Cell {
    debug_assert!(
        !matches!(c.q, Quantity::Angle { .. }),
        "an angle column must go through `compare_angle`"
    );
    if c.f == PrintFmt::DeclinedG {
        return Cell::Declined;
    }
    let (pa, pj) = strip_j(port);
    let (ea, ej) = strip_j(oracle);
    if pj != ej {
        return Cell::Fail(format!(
            "the `+j` marker is on one side only (port {port:?} vs oracle {oracle:?})"
        ));
    }
    let text_eq = |what: &str| {
        if pa.eq_ignore_ascii_case(ea) {
            Cell::Compared
        } else {
            Cell::Fail(format!("{what} (port {pa:?} vs oracle {ea:?})"))
        }
    };
    if c.q == Quantity::Text {
        return text_eq("text differs");
    }
    let (Ok(a), Ok(e)) = (pa.parse::<f64>(), ea.parse::<f64>()) else {
        // A numeric column that did not parse on one or both sides: fall back to
        // the verbatim text arm exactly like `harness::field_eq`, so an empty
        // padding cell or a sentinel still has to match and never silently
        // passes.
        return text_eq("not a number on both sides and the text differs");
    };
    if c.q == Quantity::Integer {
        return if a == e {
            Cell::Compared
        } else {
            Cell::Fail(format!("integer differs: port {a} vs oracle {e}"))
        };
    }
    let m = a.abs().max(e.abs());
    let (floor, ulp) = (class_floor(c.q, tol, m), c.f.ulp(m));
    let band = floor + ulp;
    if (a - e).abs() <= band {
        Cell::Compared
    } else {
        Cell::Fail(format!(
            "port {a} vs oracle {e}: |Δ| = {:e} > {band:e} = floor({:?}) {floor:e} \
             + ulp({:?}) {ulp:e}",
            (a - e).abs(),
            c.q,
            c.f
        ))
    }
}

/// The angle arm of the cell rule, kept separate because it needs the paired
/// magnitude column's own [`Quantity`] as well as its two values.
///
/// `Δθ ≤ rad2deg·allowed/|I| + ulp(angle format)`, with
/// `allowed = floor(magnitude class, |I|)` and `|I| = max(|I_oracle|, |I_port|)`
/// (two-sided, D40(1)). [`polar_angle_band`] returns `None` once `|I| ≤ allowed`
/// — the phasor is indistinguishable from zero at the accepted precision and
/// its argument carries no information, which is exactly the measured class
/// where the oracle prints `0.00°` at an exact `0 A` and the port `45.00°` at
/// `1.02898e-11 A`.
///
/// The comparison is wrap-aware ([`wrapped_deg`]): `cdang` returns `(−180,180]`,
/// so a phasor astride the negative real axis reads `+179.99…` on one engine and
/// `−179.99…` on the other. A genuine sign flip still measures a full 180°,
/// which no band this rule emits can admit (the ceiling is `rad2deg·1`).
///
/// The magnitudes used are the PRINTED ones, quantized by their own format; the
/// quantization is ≤ ½ ulp of the magnitude column and moves `allowed/|I|` by a
/// relative ~1e-6 at six significant digits — four orders under the band itself.
fn compare_angle(
    angle_fmt: PrintFmt,
    mag_q: Quantity,
    tol: &Tolerances,
    port: f64,
    oracle: f64,
    mag: (f64, f64),
) -> Cell {
    let m = mag.0.abs().max(mag.1.abs());
    if m == 0.0 {
        return if port == oracle {
            Cell::Compared
        } else {
            Cell::Fail(format!(
                "angle of an exactly-zero magnitude differs: port {port} vs oracle {oracle} deg"
            ))
        };
    }
    let allowed = class_floor(mag_q, tol, m);
    let Some(band) = polar_angle_band(allowed, m) else {
        // |I| <= allowed: the magnitude is indistinguishable from zero at the
        // case's own floor, so the angle is free. The MAGNITUDE column is never
        // masked — it is compared by the same rule one column earlier — so the
        // channel stays two-sided.
        return Cell::Declined;
    };
    let slack = band + angle_fmt.ulp(0.0);
    let d = wrapped_deg(port - oracle);
    if d <= slack {
        Cell::Compared
    } else {
        Cell::Fail(format!(
            "port {port} vs oracle {oracle} deg: Δθ = {d:e} > {slack:e} = \
             rad2deg·allowed/|I| {band:e} + print ulp {:e} \
             (|I| = {m:e}, allowed = {allowed:e})",
            angle_fmt.ulp(0.0)
        ))
    }
}

// ---------------------------------------------------------------------------
// The comparator.
// ---------------------------------------------------------------------------

/// Compare the CELLS of every matched run-produced report.
///
/// Structure before values, always: the header lines verbatim, then the data row
/// count, then each row's field count and identity key, and only then the cells.
/// A structural failure names the row; a cell failure names the column, the two
/// numbers and the two terms of the band it exceeded — the shape the kill
/// criterion needs (D40(9): a cell outside the rule is a FINDING → STOP, never a
/// ledger row).
#[track_caller]
pub fn compare_run_file_cells(
    channel: &str,
    matched: &[MatchedRunFile],
    tol: &Tolerances,
    label: &str,
) -> CellTally {
    let mut tally = CellTally::default();
    for f in matched {
        let ctx = format!("{label} [{channel}] {} contents", f.name);
        let ol = report_lines(&f.oracle);
        let rl = report_lines(&f.port);
        let first = ol.first().map(String::as_str).unwrap_or("");
        let Some(kind) = ReportKind::of(&f.name, first) else {
            panic!(
                "{ctx}: the gate selected this file's contents but no ReportKind \
                 claims it. The selection \
                 (`dss_epri::guard::RUN_FILE_CONTENTS_PATTERNS`) and the column \
                 maps here must name the same set — see \
                 `harness::run_file_contents::CONTENTS_NOT_SELECTED` for what is \
                 deliberately left out."
            )
        };
        let FileTally {
            compared,
            declined,
            rows,
        } = compare_one(&ctx, kind, &ol, &rl, tol);
        if let Some((oracle_rows, port_rows)) = rows {
            // D43(1)(iii): the reader's footprint is REPORTED, never absorbed.
            tally.trace_files += 1;
            tally.trace_tail += oracle_rows - port_rows;
            TRACE_TAILS
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(TraceTail {
                    ctx: ctx.clone(),
                    oracle_rows,
                    port_rows,
                });
        }
        FILES.fetch_add(1, Ordering::Relaxed);
        PER_KIND_FILES[kind_idx(kind)].fetch_add(1, Ordering::Relaxed);
        COMPARED_CELLS.fetch_add(compared, Ordering::Relaxed);
        DECLINED_CELLS.fetch_add(declined, Ordering::Relaxed);
        let ki = kind_idx(kind);
        PER_KIND_CELLS[ki].fetch_add(compared, Ordering::Relaxed);
        PER_KIND_DECLINED[ki].fetch_add(declined, Ordering::Relaxed);
        tally.files += 1;
        tally.compared += compared;
        tally.declined += declined;
    }
    tally
}

/// What [`compare_one`] made of one file.
struct FileTally {
    compared: usize,
    declined: usize,
    /// `(oracle rows, port rows)` for a kind that accounts a reader read-back
    /// tail ([`ReportKind::accounts_readback_tail`]); `None` when the two row
    /// counts had to be equal.
    rows: Option<(usize, usize)>,
}

#[track_caller]
fn compare_one(
    ctx: &str,
    kind: ReportKind,
    ol: &[String],
    rl: &[String],
    tol: &Tolerances,
) -> FileTally {
    let (sep, header_lines) = kind.structure();
    assert!(
        ol.len() >= header_lines && rl.len() >= header_lines,
        "{ctx}: file shorter than the {header_lines} header line(s) \
         (oracle {} line(s), port {})",
        ol.len(),
        rl.len()
    );
    for i in 0..header_lines {
        assert_eq!(
            rl[i], ol[i],
            "{ctx}: header line {i} differs — the report's column set, order or \
             title moved"
        );
    }
    let header: Vec<String> = if header_lines >= 1 {
        split_fields(&ol[header_lines - 1], sep)
    } else {
        Vec::new()
    };
    if kind == ReportKind::Profile {
        assert!(
            !ol[0].to_ascii_lowercase().contains("title=l-l"),
            "{ctx}: this is a LINE-TO-LINE profile. Its two per-unit columns are \
             a deliberate lane divergence (`compat::profile_ll_pu_divisor`, the \
             four-digit 1732.0 divisor), excluded from the byte goldens in the \
             default lane by `harness::lane::profile_ll_policy` and pinned by \
             `export_profile_ll_pu_is_the_lane_kernel`. No corpus deck exported \
             one when G1.10b landed; wiring one needs the same field exclusion \
             here, deliberately — never a band."
        );
    }
    let layout = match kind {
        ReportKind::Currents => currents_layout(),
        ReportKind::Powers => powers_layout(),
        ReportKind::Voltages => voltages_layout(),
        ReportKind::Profile => profile_layout(),
        ReportKind::YDense => y_dense_layout(),
        ReportKind::YTriplet => y_triplet_layout(),
        ReportKind::Yprim => yprim_layout(),
        ReportKind::Register => register_layout(&header, ctx),
        ReportKind::StorageTrace => storage_trace_layout(&header, ctx),
    };
    let odata = &ol[header_lines..];
    let rdata = &rl[header_lines..];
    let rows = if kind.accounts_readback_tail() {
        // D43(1) option A: the oracle's file is the port's plus the tail the
        // transports' own post-solve element reads appended, so the port can
        // never be the longer of the two — and the cell loop below, which zips,
        // compares exactly the common `port_rows`-record prefix.
        assert!(
            rdata.len() <= odata.len(),
            "{ctx}: the port wrote MORE {} records than the oracle (port {} \
             vs oracle {}). `WriteTraceRecord` is unconditional at the end \
             of `GetTerminalCurrents` (r4133 PCElements/Storage.pas:2874, \
             capi 0.14.5 :2356), so the oracle's file is the port's plus \
             the gate's own read-back tail; a longer PORT file means it \
             started writing records the authority does not \
             (`ReportKind::accounts_readback_tail`, coordinator decision \
             D43(1))",
            kind.label(),
            rdata.len(),
            odata.len()
        );
        Some((odata.len(), rdata.len()))
    } else {
        assert_eq!(
            rdata.len(),
            odata.len(),
            "{ctx}: data row count differs (port {} vs oracle {})",
            rdata.len(),
            odata.len()
        );
        None
    };
    let (mut compared, mut declined) = (0usize, 0usize);
    for (i, (r, o)) in rdata.iter().zip(odata).enumerate() {
        let (rf, of) = (split_fields(r, sep), split_fields(o, sep));
        assert_eq!(
            rf.len(),
            of.len(),
            "{ctx}: row {i} field count differs (port {} vs oracle {}); \
             port {r:?} oracle {o:?}",
            rf.len(),
            of.len()
        );
        let n = of.len();
        if layout.name_rows && n == 1 {
            // `Export Yprims` interleaves `Class.NAME` lines with matrix rows.
            assert!(
                rf[0].eq_ignore_ascii_case(&of[0]),
                "{ctx}: row {i} element name differs (port {:?} vs oracle {:?})",
                rf[0],
                of[0]
            );
            compared += 1;
            continue;
        }
        if let Some(idc) = layout.id_col {
            let (a, e) = (rf.get(idc), of.get(idc));
            assert!(
                a.zip(e).is_some_and(|(a, e)| a.eq_ignore_ascii_case(e)),
                "{ctx}: row {i} identity (field {idc}) differs — the row set or \
                 its order moved (port {a:?} vs oracle {e:?})"
            );
        }
        let value_cols = if layout.trailing_empty {
            assert!(
                of[n - 1].is_empty() && rf[n - 1].is_empty(),
                "{ctx}: row {i} does not end with the writer's trailing \
                 separator (port {:?} vs oracle {:?})",
                rf[n - 1],
                of[n - 1]
            );
            n - 1
        } else {
            n
        };
        // A repeating group wraps, so `col_at` answers for EVERY index once the
        // group is non-empty — an extra column would be absorbed into the group
        // and judged under a neighbouring column's quantity class and print
        // format instead of being refused. The width of a group row is
        // `head + k·group` by construction of every r4133 writer this module
        // maps, so assert it. (G1.10b audit settlement, finding AT1-2; the
        // group-free kinds are refused by `col_at` returning `None` below.)
        if !layout.group.is_empty() {
            assert!(
                value_cols >= layout.head.len()
                    && (value_cols - layout.head.len()) % layout.group.len() == 0,
                "{ctx}: row {i} carries {value_cols} value column(s), which is not \
                 {} fixed + a whole number of {}-column {} groups — the report grew \
                 or lost a column inside the repeating group, where `col_at` would \
                 silently re-align it onto a neighbouring column's class",
                layout.head.len(),
                layout.group.len(),
                kind.label()
            );
        }
        for j in 0..value_cols {
            let Some(c) = layout.col_at(j) else {
                panic!(
                    "{ctx}: row {i} field {j} is outside the {} column map \
                     ({} fixed column(s), no repeating group) — the report grew \
                     a column the map does not describe",
                    kind.label(),
                    layout.head.len()
                )
            };
            let verdict = if let Quantity::Angle { mag_back } = c.q {
                let mj = j.checked_sub(mag_back);
                let mc = mj.and_then(|mj| layout.col_at(mj));
                let (Some(mj), Some(mc)) = (mj, mc) else {
                    panic!(
                        "{ctx}: row {i} field {j} ({}) is an angle but there is no \
                         column {mag_back} back to gate it on",
                        c.name
                    )
                };
                // The `+j` marker must agree on both sides here too: the
                // tokenization (D40(4)) normalizes the marker away, and
                // `compare_cell` asserts the two sides carry the same one so a
                // layout change can never hide behind it. The angle arm does not
                // go through `compare_cell`, so it states the same rule itself.
                // (G1.10b audit settlement, finding AC1-5.)
                for (f, what) in [(mj, "paired magnitude"), (j, "angle")] {
                    assert_eq!(
                        strip_j(&rf[f]).1,
                        strip_j(&of[f]).1,
                        "{ctx}: row {i} field {f} (the {what} of {}) carries the \
                         `+j` marker on one side only (port {:?} vs oracle {:?})",
                        c.name,
                        rf[f],
                        of[f]
                    );
                }
                let (Ok(mo), Ok(mp)) = (
                    strip_j(&of[mj]).0.parse::<f64>(),
                    strip_j(&rf[mj]).0.parse::<f64>(),
                ) else {
                    panic!(
                        "{ctx}: row {i} field {j} ({}) is an angle but its paired \
                         magnitude in field {mj} is not a number on both sides \
                         (port {:?} vs oracle {:?})",
                        c.name, rf[mj], of[mj]
                    )
                };
                let (Ok(a), Ok(e)) = (
                    strip_j(&rf[j]).0.parse::<f64>(),
                    strip_j(&of[j]).0.parse::<f64>(),
                ) else {
                    panic!(
                        "{ctx}: row {i} field {j} ({}) is an angle but does not \
                         parse on both sides (port {:?} vs oracle {:?})",
                        c.name, rf[j], of[j]
                    )
                };
                compare_angle(c.f, mc.q, tol, a, e, (mo, mp))
            } else {
                compare_cell(c, tol, &rf[j], &of[j])
            };
            match verdict {
                Cell::Compared => compared += 1,
                Cell::Declined => declined += 1,
                Cell::Fail(why) => panic!(
                    "{ctx}: row {i} field {j} ({} of the {} map): {why}",
                    c.name,
                    kind.label()
                ),
            }
        }
    }
    FileTally {
        compared,
        declined,
        rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::tol_for;

    /// Run `f` and return the panic message (each harness module keeps its own
    /// extractor — they compile into separate test binaries).
    fn panic_message(f: impl FnOnce()) -> String {
        let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
            .expect_err("the arm must panic");
        if let Some(m) = payload.downcast_ref::<&str>() {
            (*m).to_string()
        } else if let Some(m) = payload.downcast_ref::<String>() {
            m.clone()
        } else {
            "<non-string panic payload>".to_string()
        }
    }

    fn file(name: &str, oracle: &str, port: &str) -> MatchedRunFile {
        MatchedRunFile {
            name: name.to_string(),
            oracle: oracle.to_string(),
            port: port.to_string(),
        }
    }

    fn cmp(name: &str, oracle: &str, port: &str) -> CellTally {
        cmp_labeled("fixture", name, oracle, port)
    }

    /// [`cmp`] under a caller-chosen case label — the process-wide
    /// [`trace_tail_census`] is shared by every test in this binary, so an
    /// assertion about ONE comparison filters on its own label.
    fn cmp_labeled(label: &str, name: &str, oracle: &str, port: &str) -> CellTally {
        compare_run_file_cells(
            "unit",
            &[file(name, oracle, port)],
            &tol_for("feeder"),
            label,
        )
    }

    fn fails(name: &str, oracle: &str, port: &str) -> String {
        panic_message(|| {
            cmp(name, oracle, port);
        })
    }

    // -- the two ulp literals -------------------------------------------------

    /// `%N.df` resolves to `10^-d` and `%N.sg` to `10^(1-s)·10^floor(log10 m)`
    /// — the whole print-resolution term, one assertion per format literal the
    /// column maps use.
    #[test]
    fn the_print_ulp_is_exact_arithmetic_from_the_format() {
        // Fixed: independent of the magnitude, one unit in the last decimal.
        assert_eq!(PrintFmt::Fixed(0).ulp(0.0), 1.0);
        assert_eq!(PrintFmt::Fixed(0).ulp(1.2345e7), 1.0);
        assert_eq!(PrintFmt::Fixed(1).ulp(1342.2125), 0.1);
        assert_eq!(PrintFmt::Fixed(2).ulp(-74.06), 0.01);
        // Sig: one unit in the last significant place at this magnitude.
        assert_eq!(PrintFmt::Sig(6).ulp(0.0374458), 1e-7);
        assert_eq!(PrintFmt::Sig(6).ulp(10.5234), 1e-4);
        assert_eq!(PrintFmt::Sig(5).ulp(12.47), 1e-3);
        assert_eq!(PrintFmt::Sig(10).ulp(0.430116291), 1e-10);
        assert_eq!(PrintFmt::Sig(10).ulp(2.220446049e-16), 1e-25);
        // An exact zero prints exactly; there is no last place to move.
        assert_eq!(PrintFmt::Sig(6).ulp(0.0), 0.0);
        // No slack at all for integers, text and the declined `%-.g` set.
        assert_eq!(PrintFmt::Exact.ulp(1e9), 0.0);
        assert_eq!(PrintFmt::DeclinedG.ulp(1e9), 0.0);
    }

    /// The floor term is [`Tolerances`] and nothing else — one assertion per
    /// quantity class, at the `feeder` tier the forced G1.10b population runs at
    /// (`v_rel 1e-8 / v_abs 1e-6 / i_rel 1e-7 / i_abs 1e-5 / y_rel 1e-8 /
    /// y_abs 1e-6`, `energy 1e-4/1e-4`).
    #[test]
    fn the_class_floor_is_the_cases_own_calibrated_band() {
        let t = tol_for("feeder");
        assert_eq!(
            class_floor(Quantity::Voltage, &t, 7200.0),
            1e-6 + 1e-8 * 7200.0
        );
        assert_eq!(
            class_floor(Quantity::Current, &t, 683.0),
            1e-5 + 1e-7 * 683.0
        );
        assert_eq!(
            class_floor(Quantity::Power, &t, 1342.0),
            1e-5 + 1e-7 * 1342.0
        );
        // The MW twin: the same band read in the printed unit — `abs` divided
        // by 1000 (never multiplied: that would be a 1000× widening), `rel`
        // untouched because it is scale-free.
        assert_eq!(
            class_floor(Quantity::PowerMega, &t, 1.342),
            1e-5 / 1000.0 + 1e-7 * 1.342
        );
        // Tighter than reading the kW-calibrated `abs` against the same MW
        // number, which is what the class did before the audit settlement…
        assert!(
            class_floor(Quantity::PowerMega, &t, 1.342) < class_floor(Quantity::Power, &t, 1.342),
            "the MW class must be the TIGHTER reading of the same printed number"
        );
        // …and, scaled back to kW, EXACTLY the kW band — the unit identity the
        // class exists for.
        let mw_in_kw = class_floor(Quantity::PowerMega, &t, 1.342) * 1000.0;
        let kw = class_floor(Quantity::Power, &t, 1342.0);
        assert!(
            (mw_in_kw - kw).abs() <= 1e-12 * kw,
            "the MW floor scaled to kW is {mw_in_kw:e}, not the kW floor {kw:e}"
        );
        assert_eq!(
            class_floor(Quantity::Admittance, &t, 4.6),
            1e-6 + 1e-8 * 4.6
        );
        assert_eq!(class_floor(Quantity::Pu, &t, 1.05), 1e-8 * 1.05);
        assert_eq!(
            class_floor(Quantity::Energy, &t, 300.0),
            1e-4 + 1e-4 * 300.0
        );
        for q in [
            Quantity::Distance,
            Quantity::ExactElsewhere,
            Quantity::Integer,
            Quantity::Text,
        ] {
            assert_eq!(class_floor(q, &t, 1e6), 0.0, "{q:?} must carry no floor");
        }
    }

    // -- the map/selection bijection -----------------------------------------

    /// Every selection pattern the gate ships to the two oracle transports has a
    /// column map here, and every map answers to a shipped pattern. A kind whose
    /// pattern left the list is a kind whose bytes silently stop travelling; a
    /// pattern with no kind is a file the gate copies and then refuses.
    #[test]
    fn every_selection_pattern_has_a_kind_and_every_kind_a_pattern() {
        use std::collections::BTreeSet;
        let shipped: BTreeSet<&str> = dss_epri::guard::RUN_FILE_CONTENTS_PATTERNS
            .iter()
            .copied()
            .collect();
        let mapped: BTreeSet<&str> = ReportKind::ALL.iter().map(|k| k.pattern()).collect();
        assert_eq!(
            shipped, mapped,
            "the shipped selection and the column-map table must name the same files"
        );
        // Both `Export Y` arms share one file name and are told apart by the
        // first line, which is why the two sets are equal while the kinds are 9.
        assert_eq!(ReportKind::ALL.len(), shipped.len() + 1);
        assert_eq!(
            ReportKind::of("nev_exp_y.csv", "Row,Col,G,B"),
            Some(ReportKind::YTriplet)
        );
        assert_eq!(
            ReportKind::of("nev_exp_y.csv", "1146, "),
            Some(ReportKind::YDense)
        );
        assert_eq!(ReportKind::of("ieee13_mon_m1_1.csv", ""), None);
    }

    // `every_compared_kind_uses_the_same_policy_as_its_golden` is NOT here: it
    // is one of the sub-step's registered pins and lives with the others in
    // `crates/dss-core/tests/run_file_contents_pins.rs`, where it reads
    // [`ReportKind::structure`] and [`ReportKind::golden_policy`] through the
    // same public surface the live gate uses.

    /// The declined-kind table is a census of what never travels, so nothing in
    /// it may also be selected, and every entry must carry a reason.
    ///
    /// The disjointness is asserted over a NAME each row stands for — the
    /// spelling the corpus actually produces for that kind — run through the
    /// shipped matcher. Until the G1.10b audit settlement this clause compared
    /// the row's prose LABEL against the glob patterns
    /// (`!patterns.any(|p| p.contains(what))`), which no label can ever satisfy:
    /// the assertion was unconditionally true and a row naming a kind the gate
    /// DOES select would have passed (findings AC1-1 / AT1-1 / AT3-2).
    #[test]
    fn nothing_declined_is_also_selected() {
        // One produced name per census row, in the table's own order — the
        // same measured spellings the registered pin
        // `run_file_contents_pins::a_report_without_a_policy_is_recorded_not_compared`
        // uses (`tmp/g110b/census.txt`, `tmp/g110a/oracle_diff.txt`).
        let witness: [(&str, &str); 8] = [
            ("monitor CSV", "ieee13_mon_m1_1.csv"),
            ("eventlog CSV", "ieee13_eventlog.csv"),
            ("demand-interval tree (DI_yr_*)", "ckt7_di_yr_0.csv"),
            ("Show / Dump text reports", "testygd_curr_elem.txt"),
            (
                "NCIM Jacobian / deltaF / deltaZ",
                "kundur_two_area_jacobian.csv",
            ),
            ("cim100 XML", "ieee13_cim100.xml"),
            (
                "deck-named export (`export ... file=<name>`)",
                "auto1bus_hl_current.txt",
            ),
            (
                "capacity / ycurrents / ynodelist (golden policy, no column map)",
                "ckt7_exp_capacity.csv",
            ),
        ];
        let patterns: Vec<String> = dss_epri::guard::RUN_FILE_CONTENTS_PATTERNS
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        assert_eq!(witness.len(), CONTENTS_NOT_SELECTED.len());
        for ((what, why), (row, name)) in CONTENTS_NOT_SELECTED.iter().zip(witness) {
            assert!(!why.trim().is_empty(), "{what}: a decline needs its reason");
            assert_eq!(
                *what, row,
                "the census rows moved; the witness names below must move with them"
            );
            assert!(
                !dss_epri::guard::selects_contents(&patterns, name),
                "{what}: {name:?} is claimed by a decline row AND selected by \
                 `RUN_FILE_CONTENTS_PATTERNS` — one of the two halves moved"
            );
            assert!(
                ReportKind::of(name, "").is_none(),
                "{what}: {name:?} is claimed by a decline row but the column map \
                 now dispatches it"
            );
        }
        assert_eq!(CONTENTS_NOT_SELECTED.len(), 8);
    }

    /// **The selection and the decline census are jointly exhaustive over the
    /// export kinds the vendored corpus actually issues** — the settlement of
    /// the G1.10b code audit's finding AC-4.
    ///
    /// Before it, three default-named kinds that DO have golden policies
    /// (`capacity`, `ycurrents`, `ynodelist`, produced by the `large*` rows
    /// `ckt7` and `GFM_AmpsLimit_123`) were neither selected nor named by a
    /// census row: declaring `compare_run_files` on such a row would have
    /// skipped their contents with no census movement and no recorded reason —
    /// exactly the silence the census exists to prevent.
    ///
    /// The fixture is the measured export population of the corpus
    /// (`tmp/g110b/sweep_exports.txt`: `monitors` 230, `currents` 28,
    /// `losses` 24, `powers` 19, `voltages` 13, `monitor` 12, `eventlog` 10,
    /// `profile` 2 and one each of `pvsystem`, `mon`, `deltaf`, `deltaz`,
    /// `jacobian`, `y`, `yprims`, `capacity`, `ycurrents`, `ynodelist`,
    /// `cim100`), each mapped to the name the run produces — r4133's default
    /// `<CircuitName>_` + the `ExportOptions.pas:333-396` table entry, or a
    /// deck-chosen name where the corpus always passes `file=`. Every one of
    /// them must be either SELECTED (and carry a column map) or claimed by a
    /// [`CONTENTS_NOT_SELECTED`] row that exists.
    #[test]
    fn every_default_export_name_the_corpus_produces_is_selected_or_declined() {
        // (export sub-command, the produced file name, `None` = selected,
        //  `Some(row)` = the census row that claims it)
        let population: [(&str, &str, Option<&str>); 19] = [
            ("currents", "ieee8500_exp_currents.csv", None),
            ("powers", "ieee8500_exp_powers.csv", None),
            ("voltages", "fbs_exp_voltages.csv", None),
            ("profile", "ieee8500u_exp_profile.csv", None),
            ("y", "nev_exp_y.csv", None),
            ("yprims", "nev_exp_yprim.csv", None),
            ("pvsystem /m", "exp_pv_pv.csv", None),
            // Not an `Export` at all: the Storage `DebugTrace` writer G1.10a's
            // F4a ported (r4133 `PCElements/Storage.pas:1075`).
            ("(storage debugtrace)", "stor_storage1.csv", None),
            (
                "monitors",
                "ieee13nodeckt_mon_m1_1.csv",
                Some("monitor CSV"),
            ),
            ("monitor", "ieee13nodeckt_mon_m1_1.csv", Some("monitor CSV")),
            ("mon", "ieee13nodeckt_mon_m1_1.csv", Some("monitor CSV")),
            (
                "eventlog",
                "ppriority_exp_eventlog.csv",
                Some("eventlog CSV"),
            ),
            (
                "jacobian",
                "kundur_two_area_jacobian.csv",
                Some("NCIM Jacobian / deltaF / deltaZ"),
            ),
            (
                "deltaf",
                "kundur_two_area_deltaf.csv",
                Some("NCIM Jacobian / deltaF / deltaZ"),
            ),
            (
                "deltaz",
                "kundur_two_area_deltaz.csv",
                Some("NCIM Jacobian / deltaF / deltaZ"),
            ),
            ("cim100", "ieee13nodeckt_cim100x.xml", Some("cim100 XML")),
            // The corpus issues `export losses file=<name>` on all 24 sites, so
            // what it produces is a deck name, not `EXP_LOSSES.CSV`.
            (
                "losses file=…",
                "auto1bus_hl_current.txt",
                Some("deck-named export (`export ... file=<name>`)"),
            ),
            (
                "capacity",
                "ckt7_exp_capacity.csv",
                Some("capacity / ycurrents / ynodelist (golden policy, no column map)"),
            ),
            (
                "ycurrents",
                "nev_exp_ycurrents.csv",
                Some("capacity / ycurrents / ynodelist (golden policy, no column map)"),
            ),
        ];
        let p: Vec<String> = dss_epri::guard::RUN_FILE_CONTENTS_PATTERNS
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        for (cmd, name, row) in population {
            let selected = dss_epri::guard::selects_contents(&p, name);
            match row {
                None => {
                    assert!(selected, "`export {cmd}` → {name} must be selected");
                    assert!(
                        ReportKind::of(name, "Row,Col,G,B").is_some(),
                        "`export {cmd}` → {name} is selected but has no column map"
                    );
                }
                Some(row) => {
                    assert!(
                        !selected,
                        "`export {cmd}` → {name} is selected, but the census row \
                         {row:?} says it is not compared — one of the two moved"
                    );
                    assert!(
                        CONTENTS_NOT_SELECTED.iter().any(|(what, _)| *what == row),
                        "`export {cmd}` → {name} is not compared and the census row \
                         {row:?} that was to state the reason is gone"
                    );
                }
            }
        }
        // `ynodelist` rides the same row as `ycurrents`; keeping it here makes
        // the third name of that row a test subject too.
        assert!(!dss_epri::guard::selects_contents(
            &p,
            "nev_exp_ynodelist.csv"
        ));
    }

    /// **The `tests/TOLERANCE_NOTES.md` column-map table and the layouts here
    /// are ONE claim**: the print formats the table names for a kind are the
    /// formats that kind's [`Layout`] declares — settlement of the G1.10b audit
    /// findings AC-1/T3, where the table gave `Export Voltages`' angle column as
    /// `Fixed(2)` while the writer prints `%6.1f`
    /// (r4133 `Common/ExportResults.pas:288`) and the code maps `Fixed(1)`.
    ///
    /// A derivation table that misstates a print resolution is worse than no
    /// table: re-deriving the band from it produces a different number than the
    /// one in force. `Exact` and `DeclinedG` carry no resolution and are not
    /// part of the comparison (the table names them in prose).
    #[test]
    fn the_tolerance_notes_column_map_names_the_formats_the_layouts_declare() {
        use std::collections::BTreeSet;
        let doc = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tests")
            .join("TOLERANCE_NOTES.md");
        let text = std::fs::read_to_string(&doc)
            .unwrap_or_else(|e| panic!("{doc:?} carries the G1.10b derivation: {e}"));
        let section = text
            .split("## G1.10b run-file contents")
            .nth(1)
            .expect("the G1.10b section of TOLERANCE_NOTES.md is gone");

        let hdr: Vec<String> = STORAGE_TRACE_HEADER
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
        let layouts: [(ReportKind, Layout); 9] = [
            (ReportKind::Currents, currents_layout()),
            (ReportKind::Powers, powers_layout()),
            (ReportKind::Voltages, voltages_layout()),
            (ReportKind::Profile, profile_layout()),
            (ReportKind::YDense, y_dense_layout()),
            (ReportKind::YTriplet, y_triplet_layout()),
            (ReportKind::Yprim, yprim_layout()),
            (
                ReportKind::Register,
                register_layout(&register_header(), "fixture"),
            ),
            (
                ReportKind::StorageTrace,
                storage_trace_layout(&hdr, "fixture"),
            ),
        ];
        for (k, l) in layouts {
            let declared: BTreeSet<String> = l
                .head
                .iter()
                .chain(l.group.iter())
                .filter_map(|c| match c.f {
                    PrintFmt::Fixed(d) => Some(format!("Fixed({d})")),
                    PrintFmt::Sig(s) => Some(format!("Sig({s})")),
                    PrintFmt::Exact | PrintFmt::DeclinedG => None,
                })
                .collect();
            let prefix = format!("| `{}` |", k.label());
            let row = section
                .lines()
                .find(|ln| ln.trim_start().starts_with(&prefix))
                .unwrap_or_else(|| {
                    panic!(
                        "the TOLERANCE_NOTES column-map table has no row for `{}`",
                        k.label()
                    )
                });
            let mut named: BTreeSet<String> = BTreeSet::new();
            for tag in ["Fixed(", "Sig("] {
                let mut rest = row;
                while let Some(at) = rest.find(tag) {
                    let after = &rest[at + tag.len()..];
                    let end = after.find(')').unwrap_or_else(|| {
                        panic!("{}: an unterminated `{tag}` in the table row", k.label())
                    });
                    named.insert(format!("{tag}{})", &after[..end]));
                    rest = &after[end..];
                }
            }
            assert_eq!(
                named,
                declared,
                "{}: the TOLERANCE_NOTES column map names {named:?} but the layout \
                 declares {declared:?} — the derivation table must state the print \
                 resolution that is actually in force",
                k.label()
            );
        }
    }

    // -- the fixtures ---------------------------------------------------------

    /// The Storage trace header as the edit-time writer emits it for the
    /// `Storage_price.dss` element (r4133 `PCElements/Storage.pas:1077-1083`),
    /// captured live by micro-part F1 (`tmp/g110b/f_F1.md`).
    /// The PVSystem register header as `WriteMultiplePVSystemMeterFiles`
    /// emits it (r4133 `Common/ExportResults.pas:2017`/`:2018` over
    /// `PCElements/PVsystem.pas:404-409`), measured live on
    /// `Test/PVSystemTest.dss` (`tmp/g110b/f_F1.md`).
    const REGISTER_HEADER: &str = "Year, LDCurve, Hour, PVSystem, \"kWh\", \"kvarh\", \"Max kW\", \"Max kVA\", \"Hours\", \"Price($)\"";

    fn register_header() -> Vec<String> {
        REGISTER_HEADER
            .split(',')
            .map(|s| s.trim().to_string())
            .collect()
    }

    const STORAGE_TRACE_HEADER: &str = "t, Iteration, LoadMultiplier, Mode, LoadModel, StorageModel,  Qnominalperphase, Pnominalperphase, CurrentType, |Iinj1|, |Iinj2|, |Iinj3|, |Iterm1|, |Iterm2|, |Iterm3|, |Vterm1|, |Vterm2|, |Vterm3|, kWh, State,Vthev, Theta";

    /// `Export Currents` as `CalcAndWriteCurrents` writes it: one element, one
    /// terminal of two conductors plus the residual pair.
    const CURRENTS_HDR: &str = "Element, I1_1, Ang1_1, I1_2, Ang1_2, Iresid1, AngResid1\n";
    const CURRENTS_ROW: &str =
        "Line.L1,    683.41,   -31.05,    683.41,   148.95,         0,     0.00\n";

    fn currents(row: &str) -> String {
        format!("{CURRENTS_HDR}{row}")
    }

    #[test]
    fn a_clean_report_compares_every_cell() {
        let t = cmp(
            "ieee13_exp_currents.csv",
            &currents(CURRENTS_ROW),
            &currents(CURRENTS_ROW),
        );
        // Element name + two real magnitude/angle pairs + the residual
        // magnitude; the residual's angle is the one free cell (its magnitude
        // is an exact `0` on both sides, so it is compared exactly and counts
        // as compared — see `the_angle_of_a_two_sided_exact_zero_stays_exact`).
        assert_eq!(
            t,
            CellTally {
                files: 1,
                compared: 7,
                declined: 0,
                // an export report is written once, in full: no reader tail.
                trace_files: 0,
                trace_tail: 0,
            }
        );
        // The process census moves by the same amounts (it is what the gate
        // epilogue reads); it is shared, so only this delta is meaningful.
        let (f, c, _d) = cell_census();
        assert!(f >= t.files && c >= t.compared);
    }

    /// A 1 % scale error on a real magnitude column must RED — the band is
    /// `i_abs + i_rel·|I| + 1 ulp of %10.6g`, which at 683 A is 1.8e-4 A, four
    /// orders under the 6.8 A the error moves.
    #[test]
    fn a_one_percent_scale_error_on_a_real_column_fails() {
        let bad = CURRENTS_ROW.replace("683.41,   -31.05", "690.24,   -31.05");
        let m = fails(
            "ieee13_exp_currents.csv",
            &currents(CURRENTS_ROW),
            &currents(&bad),
        );
        assert!(m.contains("|I| (A) of the currents map"), "{m}");
        assert!(m.contains("port 690.24 vs oracle 683.41"), "{m}");
    }

    /// A `%g` last-digit re-spelling must PASS: the print ulp is exactly what it
    /// costs, and nothing else is spent on it.
    #[test]
    fn a_last_digit_respelling_of_a_six_sig_cell_passes() {
        let respelt = CURRENTS_ROW.replace("683.41,   -31.05", "683.4101,   -31.05");
        cmp(
            "ieee13_exp_currents.csv",
            &currents(CURRENTS_ROW),
            &currents(&respelt),
        );
        // …but a hundred last places is not a re-spelling.
        let too_far = CURRENTS_ROW.replace("683.41,   -31.05", "683.52,   -31.05");
        let m = fails(
            "ieee13_exp_currents.csv",
            &currents(CURRENTS_ROW),
            &currents(&too_far),
        );
        assert!(m.contains("ulp(Sig(6))"), "{m}");
    }

    /// The measured C3 class: two `%10.6g` renders one unit apart in the last
    /// significant place (`0.0374457` vs `0.0374458` A) are inside
    /// `floor + 1 ulp`; a 1e-3 A move is not.
    #[test]
    fn the_sig_print_ulp_admits_one_last_place_and_not_two_decades() {
        let o = "Element, I1_1, Ang1_1\nLine.X, 0.0374458, 12.00\n";
        cmp(
            "x_exp_currents.csv",
            o,
            "Element, I1_1, Ang1_1\nLine.X, 0.0374457, 12.00\n",
        );
        let m = fails(
            "x_exp_currents.csv",
            o,
            "Element, I1_1, Ang1_1\nLine.X, 0.0384458, 12.00\n",
        );
        assert!(m.contains("|I| (A) of the currents map"), "{m}");
        assert!(m.contains("port 0.0384458 vs oracle 0.0374458"), "{m}");
    }

    // -- the angle rule -------------------------------------------------------

    /// The measured C1 class: the oracle prints an exact `0 A` and its `0.00°`,
    /// the port a `1.02898e-11 A` cancellation residual whose argument is a real
    /// `45.00°`. The magnitude is `max` of the two sides (two-sided, D40(1)), so
    /// the angle is free — while the magnitude column itself stays compared.
    #[test]
    fn the_angle_band_takes_the_larger_of_the_two_magnitudes() {
        // The per-call tally, never a difference of two `cell_census()` samples:
        // the process counters are shared by every comparison in this binary (the
        // G1.10b pins run in one of them too), so a delta is not this call's.
        let t = cmp(
            "x_exp_currents.csv",
            "Element, Iresid1, AngResid1
Line.X, 0, 0.00
",
            "Element, Iresid1, AngResid1
Line.X, 1.02898E-11, 45.00
",
        );
        assert_eq!(t.declined, 1, "the angle counts as declined");
        assert_eq!(t.compared, 2, "the name and the magnitude compared");
        // A REAL magnitude's angle is not free: 0.03° fails at 683 A.
        let m = fails(
            "x_exp_currents.csv",
            "Element, I1_1, Ang1_1\nLine.X, 683.41, -31.05\n",
            "Element, I1_1, Ang1_1\nLine.X, 683.41, -31.08\n",
        );
        assert!(m.contains("Ang (deg) of the currents map"), "{m}");
        assert!(m.contains("Δθ = 3"), "{m}");
    }

    /// The measured C2 class: `-74.04` vs `-74.06` at `|I| = 1.6957e-5 A`. The
    /// band is `rad2deg·(i_abs + i_rel·|I|)/|I| + 0.01` = 33.79° — the current is
    /// three orders under the case's own `i_abs`, so its argument carries almost
    /// no information; the magnitudes still had to agree (Δ|I| = 7e-10 A).
    #[test]
    fn an_angle_under_the_current_floor_is_declined_not_compared() {
        let t = tol_for("feeder");
        let m = 1.6957e-5_f64;
        let allowed = class_floor(Quantity::Current, &t, m);
        let band = polar_angle_band(allowed, m).expect("|I| is above the floor");
        assert!(
            (33.78..33.80).contains(&band),
            "the measured C2 band is ~33.79 deg, got {band}"
        );
        cmp(
            "x_exp_currents.csv",
            "Element, I1_1, Ang1_1\nLine.X, 1.69570E-5, -74.06\n",
            "Element, I1_1, Ang1_1\nLine.X, 1.69570E-5, -74.04\n",
        );
    }

    /// The angle of an exactly-zero magnitude on BOTH sides stays compared
    /// EXACTLY — the property the golden's one-sided gate deliberately keeps
    /// (`0.00 == 0.00` on every zero-filled row), which the two-sided rule must
    /// not throw away.
    #[test]
    fn the_angle_of_a_two_sided_exact_zero_stays_exact() {
        cmp(
            "x_exp_currents.csv",
            "Element, I1_1, Ang1_1\nLine.X, 0, 0.00\n",
            "Element, I1_1, Ang1_1\nLine.X, 0, 0.00\n",
        );
        let m = fails(
            "x_exp_currents.csv",
            "Element, I1_1, Ang1_1\nLine.X, 0, 0.00\n",
            "Element, I1_1, Ang1_1\nLine.X, 0, 45.00\n",
        );
        assert!(
            m.contains("angle of an exactly-zero magnitude differs"),
            "{m}"
        );
    }

    /// The comparison is taken on the circle: `cdang` returns `(−180, 180]`, so
    /// a phasor astride the negative real axis reads `+179.99…` on one engine and
    /// `−179.99…` on the other for a physical gap of ~2e-8°.
    #[test]
    fn the_angle_comparison_is_wrap_aware_on_this_surface() {
        cmp(
            "x_exp_currents.csv",
            "Element, I1_1, Ang1_1\nLine.X, 683.41, 180.00\n",
            "Element, I1_1, Ang1_1\nLine.X, 683.41, -180.00\n",
        );
        // A genuine flip still measures a full 180 deg, which no band admits.
        let m = fails(
            "x_exp_currents.csv",
            "Element, I1_1, Ang1_1\nLine.X, 683.41, 90.00\n",
            "Element, I1_1, Ang1_1\nLine.X, 683.41, -90.00\n",
        );
        assert!(m.contains("Δθ = 1.8e2"), "{m}");
    }

    // -- the two normalizations ----------------------------------------------

    /// `Export Y`'s imaginary cells are `+j <number>`: the marker is tokenized on
    /// both sides so r4133's three-digit exponent meets the port's two-digit one
    /// numerically instead of as text (D40(4), measured class C5).
    #[test]
    fn the_j_marker_is_tokenized_before_the_parse() {
        assert_eq!(strip_j("+j 2.220446049E-016"), ("2.220446049E-016", true));
        assert_eq!(strip_j("-1.110223025E-16"), ("-1.110223025E-16", false));
        cmp(
            "nev_exp_y.csv",
            "4, \n\"B1.1\", 0.8178822743, +j 2.220446049E-016, \n",
            "4, \n\"B1.1\", 0.8178822743, +j 2.220446049E-16, \n",
        );
        // The marker itself is structure, not spelling: losing it on one side is
        // a layout change and must red.
        let m = fails(
            "nev_exp_y.csv",
            "4, \n\"B1.1\", 0.8178822743, +j 2.220446049E-016, \n",
            "4, \n\"B1.1\", 0.8178822743, 2.220446049E-16, \n",
        );
        assert!(m.contains("the `+j` marker is on one side only"), "{m}");
    }

    /// The measured C4 class on the admittance channel: `-2.498001805E-16` vs
    /// `-1.110223025E-016` S is a cancellation residual 10 orders under
    /// `y_abs = 1e-6`; a real stamp moving is not.
    /// **The composition of the two terms is pinned at the boundary itself**,
    /// not one decade away: a cell 0.1 % OUTSIDE `class_floor + ulp` reds and a
    /// cell 0.1 % inside passes. Before the G1.10b audit settlement every value
    /// fixture sat ≥ 10× outside the band (finding AT1-3), so a fudge at the
    /// comparison site — `floor + 10·ulp`, `2·(floor + ulp)`, a stray additive
    /// term — kept the whole file green even though both leaf functions are
    /// pinned exactly.
    ///
    /// The drive is a `Sig(6)` magnitude of the `currents` map on the `feeder`
    /// tier: band = `i_abs + i_rel·m` + `10^(1-6)·10^⌊log10 m⌋`, evaluated here
    /// from the same two functions the comparator calls, so the assertion is
    /// about their COMPOSITION and moves with a re-calibrated tier instead of
    /// freezing a literal.
    #[test]
    fn a_cell_at_the_band_boundary_decides_the_right_way() {
        let tol = tol_for("feeder");
        let o = 683.41_f64;
        let band = |m: f64| class_floor(Quantity::Current, &tol, m) + PrintFmt::Sig(6).ulp(m);
        let d = band(o);
        let row = |v: f64| {
            format!(
                "Line.L1,    {v:.12},   -31.05,    683.41,   148.95,         0,     0.00
"
            )
        };
        let oracle = currents(&row(o));
        cmp(
            "ieee13_exp_currents.csv",
            &oracle,
            &currents(&row(o + d * 0.999)),
        );
        let m = fails(
            "ieee13_exp_currents.csv",
            &oracle,
            &currents(&row(o + d * 1.001)),
        );
        assert!(m.contains("|I| (A) of the currents map"), "{m}");
        // The same one field down: the port BELOW the oracle by the same amount.
        cmp(
            "ieee13_exp_currents.csv",
            &oracle,
            &currents(&row(o - d * 0.999)),
        );
        let m = fails(
            "ieee13_exp_currents.csv",
            &oracle,
            &currents(&row(o - d * 1.001)),
        );
        assert!(m.contains("|I| (A) of the currents map"), "{m}");
    }

    /// The angle arm's boundary, same reason (finding AT1-3): the admitted slack
    /// is the paired magnitude's own band converted to degrees plus the angle
    /// column's print ulp, and a move 0.1 % past it must red. The nearest angle
    /// fixture before the settlement was 3× outside.
    #[test]
    fn an_angle_at_the_band_boundary_decides_the_right_way() {
        let tol = tol_for("feeder");
        let mag = 683.41_f64;
        let allowed = class_floor(Quantity::Current, &tol, mag);
        let slack = polar_angle_band(allowed, mag).expect("a real magnitude has a band")
            + PrintFmt::Fixed(2).ulp(0.0);
        let a = -31.05_f64;
        let row = |v: f64| {
            format!(
                "Line.L1,    683.41,   {v:.9},    683.41,   148.95,         0,     0.00
"
            )
        };
        let oracle = currents(&row(a));
        cmp(
            "ieee13_exp_currents.csv",
            &oracle,
            &currents(&row(a + slack * 0.999)),
        );
        let m = fails(
            "ieee13_exp_currents.csv",
            &oracle,
            &currents(&row(a + slack * 1.001)),
        );
        assert!(m.contains("Ang (deg) of the currents map"), "{m}");
    }

    /// The numeric half of a `+j` cell is compared like any other: D40(4)
    /// normalizes the MARKER away, never the payload (finding AT1-6 — the
    /// tokenization had negative drives only for the equal-value and
    /// marker-asymmetry cases).
    #[test]
    fn the_payload_of_a_j_cell_is_compared_after_the_marker_is_stripped() {
        let o = "4, 
\"B1.1\", 0, +j -1.110223025E-016, 
";
        cmp(
            "nev_exp_y.csv",
            o,
            "4, 
\"B1.1\", 0, +j -2.498001805E-16, 
",
        );
        let m = fails(
            "nev_exp_y.csv",
            o,
            "4, 
\"B1.1\", 0, +j -4.636999108, 
",
        );
        assert!(m.contains("+j B (S) of the y(dense) map"), "{m}");
    }

    #[test]
    fn a_near_zero_cell_is_bounded_by_its_class_floor() {
        let o = "4, \n\"B1.1\", -1.110223025E-016, +j 0, \n";
        cmp(
            "nev_exp_y.csv",
            o,
            "4, \n\"B1.1\", -2.498001805E-16, +j 0, \n",
        );
        let m = fails("nev_exp_y.csv", o, "4, \n\"B1.1\", -4.636999108, +j 0, \n");
        assert!(m.contains("G (S) of the y(dense) map"), "{m}");
    }

    // -- the `%-.g` decline ---------------------------------------------------

    const TRACE_HDR: &str = "t, Iteration, LoadMultiplier, Mode, LoadModel, StorageModel,  Qnominalperphase, Pnominalperphase, CurrentType, |Iinj1|, |Iterm1|, |Vterm1|, kWh, kWTotalLosses,Vthev, Theta\n";

    /// The Storage trace's `%-.g` columns print two significant digits from FPC
    /// and ~15 from the Delphi-built r4133 DLL, so the two ORACLES disagree with
    /// each other: the set is declined on both channels and counted (D40(3)),
    /// while the `:8:1`/`:8:2` and integer/text columns stay compared.
    #[test]
    fn the_declined_g_columns_are_counted_not_skipped() {
        let orc = format!(
            "{TRACE_HDR}1, 1, 1, Daily, PowerFlow, 1,     0.00,    -0.00, Injection, \
             0.6, \
             0.6,    277.1, 9999, 5.5, \n"
        );
        let port = format!(
            "{TRACE_HDR}1, 1, 1, Daily, PowerFlow, 1,    -0.00,    -0.00, Injection, \
             0.6, \
             0.6,    277.1, 1E4, 5.4, \n"
        );
        let t = cmp("stor_storage1.csv", &orc, &port);
        // 2 `%-.g` scalars + 2 state variables per row.
        assert_eq!(t.declined, 4, "the `%-.g` set is declined");
        // Iteration, Mode, LoadModel, StorageModel, Q, P, CurrentType, |Iinj|,
        // |Iterm|, |Vterm| = 10 compared cells.
        assert_eq!(t.compared, 10);
        // A real move in a compared trace column still reds.
        let m = fails(
            "stor_storage1.csv",
            &orc,
            &port.replace("    277.1, 1E4", "    278.1, 1E4"),
        );
        assert!(m.contains("|Vterm| (V) of the storage-trace map"), "{m}");
    }

    /// The trace layout is read off the header the edit-time writer emitted, so
    /// a different phase or variable count still lands the declined set exactly
    /// on the `%-.g` columns.
    #[test]
    fn the_trace_layout_follows_its_own_header() {
        let h: Vec<String> = "t, Iteration, LoadMultiplier, Mode, LoadModel, StorageModel,  Qnominalperphase, Pnominalperphase, CurrentType, |Iinj1|, |Iinj2|, |Iinj3|, |Iterm1|, |Iterm2|, |Iterm3|, |Vterm1|, |Vterm2|, |Vterm3|, kWh, State,Vthev, Theta"
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
        let l = storage_trace_layout(&h, "fixture");
        assert_eq!(l.head.len(), 9 + 9 + 2);
        assert_eq!(l.head[0].f, PrintFmt::DeclinedG);
        assert_eq!(l.head[2].f, PrintFmt::DeclinedG);
        assert_eq!(l.head[9].q, Quantity::Current);
        assert_eq!(l.head[15].q, Quantity::Voltage);
        assert_eq!(l.head[18].f, PrintFmt::DeclinedG);
        assert_eq!(l.head[19].f, PrintFmt::DeclinedG);
        assert!(l.trailing_empty);
        let m = panic_message(|| {
            let _ = storage_trace_layout(&["t".into(), "Iteration".into()], "fixture");
        });
        assert!(m.contains("one |Iinj_i|"), "{m}");
    }

    // -- the read-back tail (D43(1)) -----------------------------------------

    /// One `WriteTraceRecord` line for the [`TRACE_HDR`] fixture: nine fixed
    /// cells, one phase triple, two `%-.g` state variables and the writer's
    /// trailing separator (r4133 `PCElements/Storage.pas:2401-2429`).
    fn trace_row(current_type: &str, vterm: &str) -> String {
        format!(
            "1, 1, 1, Daily, PowerFlow, 1,     0.00,    -0.00, {current_type}, \
             0.6, \
             0.6,    {vterm}, 9999, 5.5, \n"
        )
    }

    /// The Storage trace's row count is a property of the READER, not of the
    /// run: `WriteTraceRecord` is unconditional at the end of
    /// `GetTerminalCurrents` (r4133 `PCElements/Storage.pas:2874`, capi 0.14.5
    /// `:2356`), so the oracle transports' six post-solve element reads append
    /// six records where the port's single G2.3 recompute appends two. The
    /// comparator therefore asserts `port_rows <= oracle_rows`, compares every
    /// cell of the common prefix, and REPORTS the gap for the scheduler's
    /// fail-on-stale `TRACE_READBACK_RECORDS` epilogue (coordinator decision
    /// **D43(1)**; live: oracle 102 rows vs port 98, 96 solve records shared).
    #[test]
    fn the_storage_trace_row_count_accounts_the_readers_footprint() {
        let solve = trace_row("Injection", "277.1");
        let tail = trace_row("TotalCurrent", "277.1");
        let oracle = format!("{TRACE_HDR}{solve}{solve}{tail}{tail}");
        let port = format!("{TRACE_HDR}{solve}{solve}");
        let t = cmp_labeled("tail-footprint", "stor_storage1.csv", &oracle, &port);
        assert_eq!((t.files, t.trace_files, t.trace_tail), (1, 1, 2));
        // The common two-record prefix is compared cell by cell: 10 compared
        // and 4 declined `%-.g` cells per record.
        assert_eq!((t.compared, t.declined), (20, 8));
        let seen: Vec<TraceTail> = trace_tail_census()
            .into_iter()
            .filter(|o| o.ctx.starts_with("tail-footprint "))
            .collect();
        assert_eq!(seen.len(), 1, "{seen:?}");
        assert_eq!(
            (seen[0].oracle_rows, seen[0].port_rows, seen[0].gap()),
            (4, 2, 2)
        );
    }

    /// The first tooth of the accounting: the port may be shorter than the
    /// oracle (the reader's footprint), never longer — a port that started
    /// writing records the authority does not is a finding, not a gap.
    #[test]
    fn a_port_trace_longer_than_the_oracles_is_a_finding() {
        let solve = trace_row("Injection", "277.1");
        let m = fails(
            "stor_storage1.csv",
            &format!("{TRACE_HDR}{solve}"),
            &format!("{TRACE_HDR}{solve}{solve}"),
        );
        assert!(
            m.contains("the port wrote MORE storage-trace records"),
            "{m}"
        );
        assert!(m.contains("port 2 vs oracle 1"), "{m}");
    }

    /// The second tooth: the shortened row count buys the prefix no slack — a
    /// corrupted cell inside the common records still reds, so "compare the
    /// prefix" can never become "skip the file".
    #[test]
    fn a_corrupted_cell_inside_the_common_trace_prefix_still_reds() {
        let solve = trace_row("Injection", "277.1");
        let tail = trace_row("TotalCurrent", "277.1");
        let oracle = format!("{TRACE_HDR}{solve}{solve}{tail}{tail}");
        let bad = trace_row("Injection", "278.1");
        let m = fails(
            "stor_storage1.csv",
            &oracle,
            &format!("{TRACE_HDR}{solve}{bad}"),
        );
        assert!(
            m.contains("row 1 field 11 (|Vterm| (V) of the storage-trace map)"),
            "{m}"
        );
    }

    /// The third tooth: a port that stops writing a SOLVE record leaves every
    /// compared cell intact (the records are identical), so the only signal is
    /// the gap — which is exactly why it is reported and pinned fail-on-stale in
    /// both directions instead of being absorbed (D43(1)(iii); option B, which
    /// declines the contents, and option C, a data-derived boundary heuristic,
    /// were refused for this reason).
    #[test]
    fn a_dropped_solve_record_moves_the_reported_tail() {
        let solve = trace_row("Injection", "277.1");
        let tail = trace_row("TotalCurrent", "277.1");
        let oracle = format!("{TRACE_HDR}{solve}{solve}{tail}{tail}");
        let whole = cmp_labeled(
            "tail-whole",
            "stor_storage1.csv",
            &oracle,
            &format!("{TRACE_HDR}{solve}{solve}"),
        );
        let dropped = cmp_labeled(
            "tail-dropped",
            "stor_storage1.csv",
            &oracle,
            &format!("{TRACE_HDR}{solve}"),
        );
        assert_eq!((whole.compared, whole.trace_tail), (20, 2));
        assert_eq!((dropped.compared, dropped.trace_tail), (10, 3));
    }

    /// The accounting is the Storage trace's alone: every `Export` report is
    /// written once in full, so its row count stays an equality in BOTH
    /// directions and its tally reports no tail.
    #[test]
    fn only_the_trace_kind_accounts_a_readback_tail() {
        for k in ReportKind::ALL {
            assert_eq!(
                k.accounts_readback_tail(),
                k == ReportKind::StorageTrace,
                "{k:?}"
            );
        }
        let o = currents(CURRENTS_ROW);
        let m = fails("x_exp_currents.csv", &format!("{o}{CURRENTS_ROW}"), &o);
        assert!(
            m.contains("data row count differs (port 1 vs oracle 2)"),
            "{m}"
        );
        let t = cmp("x_exp_currents.csv", &o, &o);
        assert_eq!((t.trace_files, t.trace_tail), (0, 0));
    }

    // -- the structural rails -------------------------------------------------

    #[test]
    fn a_header_a_row_count_a_field_count_and_a_row_identity_all_fail_first() {
        let o = currents(CURRENTS_ROW);
        let m = fails(
            "x_exp_currents.csv",
            &o,
            &currents(CURRENTS_ROW).replace("Ang1_1", "Angle1_1"),
        );
        assert!(m.contains("header line 0 differs"), "{m}");

        let m = fails(
            "x_exp_currents.csv",
            &o,
            &format!("{}{CURRENTS_ROW}", currents(CURRENTS_ROW)),
        );
        assert!(
            m.contains("data row count differs (port 2 vs oracle 1)"),
            "{m}"
        );

        let m = fails(
            "x_exp_currents.csv",
            &o,
            &currents(&CURRENTS_ROW.replace("     0.00\n", "     0.00, 7\n")),
        );
        assert!(m.contains("row 0 field count differs"), "{m}");

        let m = fails(
            "x_exp_currents.csv",
            &o,
            &currents(&CURRENTS_ROW.replace("Line.L1,", "Line.L2,")),
        );
        assert!(m.contains("row 0 identity (field 0) differs"), "{m}");
    }

    /// A re-ordered row is a row-identity failure, not a value failure — the
    /// element order is the report's contract.
    #[test]
    fn a_reordered_row_fails_on_the_identity_key() {
        let o = "Element, Terminal, P(kW), Q(kvar)\n\"Load.L1\",   1, \
                 1.0, \
                 2.0\n\"Load.L2\",   1, \
                 3.0, \
                 4.0\n";
        let p = "Element, Terminal, P(kW), Q(kvar)\n\"Load.L2\",   1, \
                 3.0, \
                 4.0\n\"Load.L1\",   1, \
                 1.0, \
                 2.0\n";
        let m = fails("x_exp_powers.csv", o, p);
        assert!(m.contains("row 0 identity (field 0) differs"), "{m}");
    }

    /// A swapped column pair must red even when each value is the other's
    /// legitimate neighbour — the map is positional and the identity key is not
    /// enough on its own.
    #[test]
    fn a_swapped_column_pair_fails() {
        let o = "Element, Terminal, P(kW), Q(kvar)\n\"Load.L1\",   1,        1.0,        2.0\n";
        let p = "Element, Terminal, P(kW), Q(kvar)\n\"Load.L1\",   1,        2.0,        1.0\n";
        let m = fails("x_exp_powers.csv", o, p);
        assert!(m.contains("P of the powers map"), "{m}");
    }

    /// The trailing separator every matrix/trace writer ends its line with is
    /// structure: a row that loses it fails instead of shifting the whole map.
    #[test]
    fn a_missing_trailing_separator_fails() {
        let m = fails(
            "x_exp_yprim.csv",
            "Vsource.SOURCE\n0.430116291  , -1.534871242 , \n",
            "Vsource.SOURCE\n0.430116291  , -1.534871242 \n",
        );
        assert!(m.contains("field count differs"), "{m}");
    }

    /// The `Export Yprims` element-name lines are compared case-insensitively
    /// and in order, so a dropped or renamed element fails.
    #[test]
    fn a_yprim_element_name_line_is_compared() {
        cmp(
            "x_exp_yprim.csv",
            "Vsource.SOURCE\n0.430116291  , -1.534871242 , \n",
            "Vsource.source\n0.430116291  , -1.534871242 , \n",
        );
        let m = fails(
            "x_exp_yprim.csv",
            "Vsource.SOURCE\n0.430116291  , -1.534871242 , \n",
            "Line.L1\n0.430116291  , -1.534871242 , \n",
        );
        assert!(m.contains("element name differs"), "{m}");
    }

    /// A file the gate selected but no map claims is a loud refusal, never a
    /// silent pass — the two tables would otherwise drift apart unnoticed.
    #[test]
    fn a_selected_file_with_no_column_map_is_refused() {
        let m = panic_message(|| {
            compare_run_file_cells(
                "unit",
                &[file("ieee13_mon_m1_1.csv", "a\n1\n", "a\n1\n")],
                &tol_for("feeder"),
                "fixture",
            );
        });
        assert!(m.contains("no ReportKind"), "{m}");
    }

    // -- table completeness ---------------------------------------------------

    /// Every column of every captured header resolves to a column map entry —
    /// the widths are the ones measured on the forced G1.10b population
    /// (`tmp/g110b/f_F1.md`): a report that grows a column the map does not
    /// describe is a loud refusal, and here it is proved that today's widths are
    /// all covered.
    ///
    /// Scope, stated because the name is broader than the body: this is an
    /// offline WIDTH fixture over the populations F1 measured, not a walk of
    /// headers captured at run time. The run-time rail is
    /// [`compare_one`]'s panic on a field outside the map — which fires on the
    /// real files, on every drive, for any width this fixture does not know.
    #[test]
    fn every_column_of_every_captured_header_is_mapped() {
        let hdr: Vec<String> = STORAGE_TRACE_HEADER
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
        // (kind, layout, measured data-row width incl. any trailing empty field)
        let cases: [(ReportKind, Layout, usize); 9] = [
            // 8500-Node: MaxCond 4, MaxTerm 3 -> 1 + 3*(4+1)*2 = 31.
            (ReportKind::Currents, currents_layout(), 31),
            // A PD element's terminal 1 is the widest row.
            (ReportKind::Powers, powers_layout(), 8),
            // 8500-Node buses carry up to 3 nodes -> 2 + 3*4 = 14.
            (ReportKind::Voltages, voltages_layout(), 14),
            (ReportKind::Profile, profile_layout(), 12),
            // NEV: 1146 nodes -> 1 + 2*1146 + the trailing empty.
            (ReportKind::YDense, y_dense_layout(), 1 + 2 * 1146 + 1),
            (ReportKind::YTriplet, y_triplet_layout(), 4),
            // A 12-order Yprim row: 24 cells + the trailing empty.
            (ReportKind::Yprim, yprim_layout(), 25),
            // 4 fixed + the six PVSystem registers.
            (
                ReportKind::Register,
                register_layout(&register_header(), "fixture"),
                10,
            ),
            (
                ReportKind::StorageTrace,
                storage_trace_layout(&hdr, "fixture"),
                9 + 9 + 2 + 1,
            ),
        ];
        for (k, l, width) in cases {
            let values = if l.trailing_empty { width - 1 } else { width };
            for j in 0..values {
                assert!(
                    l.col_at(j).is_some(),
                    "{}: field {j} of a {width}-field row is outside the map",
                    k.label()
                );
            }
            if let Some(id) = l.id_col {
                assert!(
                    id < values,
                    "{}: the identity column is off the row",
                    k.label()
                );
            }
        }
    }

    /// The per-kind census is what tells a full drive that a kind compared
    /// nothing. It is a PROCESS counter shared with every other test in this
    /// binary and the binary runs its tests in parallel, so an EXACT per-row
    /// equality is not available; what is pinned here is the DELTA this call
    /// makes — the work it reports in its returned tally lands on its own kind's
    /// row (never on another kind's and never nowhere), and the totals move with
    /// it. The absolute populations are asserted where they are re-derived, in
    /// the scheduler epilogue (`RUN_FILE_CONTENTS_COMPARED`).
    #[test]
    fn the_census_is_per_kind() {
        let row_of = |label: &str| -> (usize, usize, usize) {
            let (_, f, c, d) = cell_census_by_kind()
                .iter()
                .find(|(l, ..)| *l == label)
                .copied()
                .unwrap_or_else(|| panic!("the {label} census row exists"));
            (f, c, d)
        };
        let before = row_of("currents");
        let totals_before = cell_census();
        let t = cmp(
            "ieee13_exp_currents.csv",
            &currents(CURRENTS_ROW),
            &currents(CURRENTS_ROW),
        );
        assert_eq!((t.files, t.compared, t.declined), (1, 7, 0));
        let rows = cell_census_by_kind();
        assert_eq!(rows.len(), ReportKind::ALL.len());
        let after = row_of("currents");
        assert!(
            after.0 - before.0 >= t.files && after.1 - before.1 >= t.compared,
            "the currents row moved by {:?}, less than this call's own tally \
             {:?} — the comparison did not land on its kind's row",
            (after.0 - before.0, after.1 - before.1),
            (t.files, t.compared)
        );
        let totals_after = cell_census();
        assert!(
            totals_after.0 - totals_before.0 >= t.files
                && totals_after.1 - totals_before.1 >= t.compared,
            "the totals did not move with the per-kind row"
        );
        let (_, files, cells, declined) = rows
            .iter()
            .find(|(l, ..)| *l == "currents")
            .copied()
            .expect("the currents row exists");
        assert!(files >= t.files && cells >= t.compared && declined >= t.declined);
        let (total_f, total_c, total_d) = cell_census();
        assert!(
            rows.iter().map(|r| r.1).sum::<usize>() <= total_f
                && rows.iter().map(|r| r.2).sum::<usize>() <= total_c
                && rows.iter().map(|r| r.3).sum::<usize>() <= total_d
        );
    }

    /// The L-L profile is the one compared kind whose per-unit columns are a
    /// deliberate lane divergence; a corpus deck that starts exporting one must
    /// stop the gate rather than red at 2.93e-5 or be quietly masked.
    #[test]
    fn a_line_to_line_profile_stops_instead_of_comparing() {
        let hdr = "Name, Distance1, puV1, Distance2, puV2, Color, Thickness, Linetype, Markcenter, Centercode, NodeCode, NodeWidth,Title=L-L Voltage Profile, Distance in km\n";
        let row = "650632, 0, 1.05603, 0.6096, 1.01434,1, 2, 0, 0, 0, 16, 1\n";
        let m = fails(
            "x_exp_profile.csv",
            &format!("{hdr}{row}"),
            &format!("{hdr}{row}"),
        );
        assert!(m.contains("LINE-TO-LINE profile"), "{m}");
    }
}
